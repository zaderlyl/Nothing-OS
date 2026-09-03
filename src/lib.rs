//! Nothing OS — un mini noyau "bare metal" x86_64 écrit en Rust.
//!
//! Ce crate est compilé en `staticlib` puis lié à la main avec un petit
//! bootstrap assembleur (voir boot/) qui s'occupe du multiboot, du
//! passage en long mode et de l'activation de SSE, avant d'appeler
//! `rust_main`.
//!
//! Astuce d'environnement : ce noyau est compilé pour la cible "hôte"
//! (x86_64-unknown-linux-gnu) plutôt que pour une vraie cible bare-metal,
//! car l'installation d'une toolchain nightly + rust-src n'était pas
//! possible ici. C'est un contournement qui fonctionne, mais la voie
//! "propre" pour la suite serait de repasser sur une cible bare-metal
//! (voir README).

#![no_std]
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]

extern crate alloc;

mod ac97;
mod asti;
mod ata;
mod docview;
mod dots;
mod editor;
mod fb;
mod font;
mod fs;
mod gdt;
mod heap;
mod home;
mod hostfs;
mod image;
mod interrupts;
mod kbd;
mod mouse;
mod p9;
mod pci;
mod port;
mod rtc;
mod serial;
mod shelf;
mod term;
mod time;
mod vga;
mod virtio;
mod win;

use core::panic::PanicInfo;

// ---------------------------------------------------------------------
// État global partagé : écran VGA et port série.
//
// Il n'existe qu'UNE seule instance de chacun (`WRITER`, `SERIAL1`),
// protégée par un mutex "spinlock" (crate `spin` — pas besoin d'OS pour
// ça, juste une boucle qui attend que le verrou se libère). C'est
// important dès qu'on a des interruptions : un gestionnaire
// d'interruption peut vouloir écrire à l'écran pendant qu'on est déjà en
// train d'y écrire ailleurs, et il ne faut surtout pas que ça se fasse
// via deux curseurs indépendants (sinon les écritures s'écrasent entre
// elles, comme un vrai bug qu'on a eu ici en développant l'IDT).
// ---------------------------------------------------------------------

pub static WRITER: spin::Mutex<vga::Writer> = spin::Mutex::new(vga::Writer::new());
pub static SERIAL1: spin::Mutex<serial::SerialPort> = spin::Mutex::new(serial::SerialPort::new());

// On désactive les interruptions pendant qu'on tient un verrou : sans
// ça, si une interruption arrive PENDANT qu'on écrit à l'écran (donc
// avec WRITER déjà verrouillé) et que son gestionnaire essaie lui aussi
// d'écrire à l'écran, il resterait bloqué à attendre un verrou qui ne se
// libérera jamais (interruption arrivée au milieu du code qui devait
// justement le libérer) → interblocage (deadlock) qui fige le noyau.

#[doc(hidden)]
pub fn _print(args: core::fmt::Arguments) {
    use core::fmt::Write;
    x86_64::instructions::interrupts::without_interrupts(|| {
        let _ = WRITER.lock().write_fmt(args);
    });
}

#[doc(hidden)]
pub fn _serial_print(args: core::fmt::Arguments) {
    use core::fmt::Write;
    x86_64::instructions::interrupts::without_interrupts(|| {
        let _ = SERIAL1.lock().write_fmt(args);
    });
}

/// Change la couleur des prochains caractères écrits à l'écran.
pub fn set_color(fg: vga::Color, bg: vga::Color) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        WRITER.lock().set_color(fg, bg);
    });
}

/// Efface l'écran et remet le curseur en haut à gauche.
pub fn clear_screen() {
    x86_64::instructions::interrupts::without_interrupts(|| {
        WRITER.lock().clear_screen();
    });
}

/// Écrit à l'écran (buffer texte VGA), comme `print!` en std.
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::_print(core::format_args!($($arg)*)));
}

/// Comme `print!`, avec un retour à la ligne à la fin.
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", core::format_args!($($arg)*)));
}

/// Écrit sur le port série COM1 (visible avec `make run-headless`),
/// pour des traces de debug qui n'encombrent pas l'écran.
#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => ($crate::_serial_print(core::format_args!($($arg)*)));
}

#[macro_export]
macro_rules! serial_println {
    () => ($crate::serial_print!("\n"));
    ($($arg:tt)*) => ($crate::serial_print!("{}\n", core::format_args!($($arg)*)));
}

/// Point d'entrée appelé par long_mode.asm une fois le CPU en 64-bit,
/// pagination et SSE activés.
#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    serial_println!("[nothing-os] rust_main() a demarre");

    gdt::init();
    interrupts::init();

    // Traces de boot : sur le port série uniquement, l'écran est réservé
    // à l'accueil (fond noir, Asti, barre de nourriture).
    serial_println!("[nothing-os] boot OK :");
    serial_println!("  - multiboot verifie");
    serial_println!("  - long mode (64-bit) actif");
    serial_println!("  - pagination (identity map 1 GiB) active");
    serial_println!("  - SSE active");
    serial_println!("  - GDT + TSS (pile dediee double fault)");
    serial_println!("  - IDT chargee (breakpoint, double fault)");

    // Auto-test de l'IDT : on déclenche un breakpoint (int3). Sans
    // gestionnaire, ça planterait ; ici le handler ne fait que logguer
    // sur le port série, puis on reprend.
    x86_64::instructions::interrupts::int3();
    serial_println!("[nothing-os] IDT ok");

    time::init();

    heap::init(); // allocateur : débloque Vec / String / Box

    // Partage de dossier avec le Mac (QEMU virtio-9p). Optionnel : si le
    // périphérique n'est pas là, on continue sans.
    if virtio::init_9p() {
        p9::init();
        p9::selftest();
        hostfs::refresh_dir();
    }

    ac97::init(); // carte son (optionnelle)

    font::capture(); // encore en mode texte : on récupère la police du BIOS
    fs::init();
    fs::load(); // remplace les fichiers par défaut si le disque a une image
    fb::init();
    asti::install_palette(asti::Tint::Null);
    home::install_palette();
    image::install_cube(); // palette 76..=255 = cube couleurs pour les images

    serial_println!("[nothing-os] mode graphique : bureau + Asti");

    let brain = asti::Brain::new(time::seed());
    home::run(brain);
}

fn halt_loop() -> ! {
    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    serial_println!("[nothing-os] PANIC: {}", info);

    set_color(vga::Color::White, vga::Color::Red);
    clear_screen();
    println!("KERNEL PANIC");
    println!("{}", info);

    halt_loop();
}

// ---------------------------------------------------------------------
// Implémentations minimales des intrinsèques mémoire (memcpy, memset,
// memcmp, memmove). Sur une cible bare-metal "normale" ces symboles sont
// fournis par compiler_builtins (feature "mem") ; comme on compile pour
// la cible hôte (qui compte normalement sur la libc pour ça), on les
// fournit nous-mêmes pour que l'édition de liens finale (faite à la main
// avec `ld`, sans libc) trouve tout ce dont elle a besoin.
// ---------------------------------------------------------------------

#[no_mangle]
pub unsafe extern "C" fn memcpy(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    let mut i = 0;
    while i < n {
        *dest.add(i) = *src.add(i);
        i += 1;
    }
    dest
}

#[no_mangle]
pub unsafe extern "C" fn memset(dest: *mut u8, c: i32, n: usize) -> *mut u8 {
    let mut i = 0;
    while i < n {
        *dest.add(i) = c as u8;
        i += 1;
    }
    dest
}

#[no_mangle]
pub unsafe extern "C" fn memcmp(s1: *const u8, s2: *const u8, n: usize) -> i32 {
    let mut i = 0;
    while i < n {
        let a = *s1.add(i);
        let b = *s2.add(i);
        if a != b {
            return a as i32 - b as i32;
        }
        i += 1;
    }
    0
}

#[no_mangle]
pub unsafe extern "C" fn bcmp(s1: *const u8, s2: *const u8, n: usize) -> i32 {
    memcmp(s1, s2, n)
}

// La libcore précompilée référence ce symbole pour le déroulage de pile
// (unwinding) en cas de panic, même si notre crate compile en `panic =
// "abort"` : ce n'est que la référence statique qui doit exister, la
// fonction n'est jamais réellement appelée puisqu'on n'unwind jamais.
#[no_mangle]
pub extern "C" fn rust_eh_personality() {}

// `alloc` précompilé (String/Vec/…) contient des chemins de code qui
// référencent ces symboles "libc/unwind". On compile en `panic = "abort"`
// et on n'unwind jamais : `_Unwind_Resume` ne doit donc jamais être
// appelé — mais la référence statique doit exister pour lier.
#[no_mangle]
pub extern "C" fn _Unwind_Resume() -> ! {
    halt_loop()
}

#[no_mangle]
pub unsafe extern "C" fn strlen(s: *const u8) -> usize {
    let mut n = 0;
    while *s.add(n) != 0 {
        n += 1;
    }
    n
}

#[no_mangle]
pub unsafe extern "C" fn memmove(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    if (dest as usize) < (src as usize) {
        memcpy(dest, src, n)
    } else {
        let mut i = n;
        while i != 0 {
            i -= 1;
            *dest.add(i) = *src.add(i);
        }
        dest
    }
}
