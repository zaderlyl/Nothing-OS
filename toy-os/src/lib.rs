//! toy-os — un mini noyau "bare metal" x86_64 écrit en Rust.
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

mod serial;
mod vga;

use core::fmt::Write;
use core::panic::PanicInfo;

/// Point d'entrée appelé par long_mode.asm une fois le CPU en 64-bit,
/// pagination et SSE activés.
#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    let mut com1 = serial::writer();
    let _ = writeln!(com1, "[toy-os] rust_main() a demarre");

    let mut screen = unsafe { vga::writer() };
    screen.clear_screen();

    screen.set_color(vga::Color::Yellow, vga::Color::Black);
    let _ = writeln!(screen, "==========================================");
    let _ = writeln!(screen, "   toy-os -- mini noyau Rust bare metal");
    let _ = writeln!(screen, "==========================================");

    screen.set_color(vga::Color::LightGreen, vga::Color::Black);
    let _ = writeln!(screen);
    let _ = writeln!(screen, "Boot OK :");
    let _ = writeln!(screen, "  - multiboot verifie");
    let _ = writeln!(screen, "  - long mode (64-bit) actif");
    let _ = writeln!(screen, "  - pagination (identity map 1 GiB) active");
    let _ = writeln!(screen, "  - SSE active");

    screen.set_color(vga::Color::LightCyan, vga::Color::Black);
    let _ = writeln!(screen);
    let _ = writeln!(screen, "Prochaines etapes possibles : IDT/interruptions,");
    let _ = writeln!(screen, "clavier PS/2, allocateur memoire, scheduler...");

    let _ = writeln!(com1, "[toy-os] affichage VGA termine, mise en boucle hlt");

    halt_loop();
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
    let mut com1 = serial::writer();
    let _ = writeln!(com1, "[toy-os] PANIC: {}", info);

    let mut screen = unsafe { vga::writer() };
    screen.set_color(vga::Color::White, vga::Color::Red);
    screen.clear_screen();
    let _ = writeln!(screen, "KERNEL PANIC");
    let _ = writeln!(screen, "{}", info);

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
