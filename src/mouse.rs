//! Souris PS/2, en *polling* (pas d'interruptions pour l'instant).
//!
//! On règle une cadence d'échantillonnage basse et on vide le port très
//! souvent (dans l'attente inter-image), donc on ne rate pas d'octet
//! malgré le tampon de 1 octet du contrôleur.

use crate::fb;
use crate::port::{inb, outb};

const DATA: u16 = 0x60;
const STATUS: u16 = 0x64;
const CMD: u16 = 0x64;

fn inb_status() -> u8 {
    unsafe { inb(STATUS) }
}

/// Attend que le tampon d'entrée du contrôleur soit vide (on peut écrire).
fn wait_input_clear() -> bool {
    for _ in 0..1_000_000 {
        if inb_status() & 0x02 == 0 {
            return true;
        }
        core::hint::spin_loop();
    }
    false
}

/// Attend qu'un octet soit disponible en lecture.
fn wait_output_full() -> bool {
    for _ in 0..1_000_000 {
        if inb_status() & 0x01 != 0 {
            return true;
        }
        core::hint::spin_loop();
    }
    false
}

fn ctrl_cmd(byte: u8) {
    wait_input_clear();
    unsafe { outb(CMD, byte) }
}

/// Envoie une commande à la souris et renvoie l'octet de réponse (ACK
/// `0xFA` attendu), ou `0xFF` si rien n'est venu.
fn mouse_cmd(byte: u8) -> u8 {
    wait_input_clear();
    unsafe { outb(CMD, 0xd4) } // "l'octet suivant est pour la souris"
    wait_input_clear();
    unsafe { outb(DATA, byte) }
    if wait_output_full() {
        unsafe { inb(DATA) }
    } else {
        0xff
    }
}

pub struct State {
    pub x: i32,
    pub y: i32,
    pub left: bool,
    #[allow(dead_code)]
    pub right: bool,
}

static mut X: i32 = (fb::WIDTH / 2) as i32;
static mut Y: i32 = (fb::HEIGHT / 2) as i32;
static mut LEFT: bool = false;
static mut RIGHT: bool = false;
static mut PACKETS: u32 = 0;

/// Nombre de paquets souris reçus depuis le boot (diagnostic).
pub fn packets() -> u32 {
    unsafe { PACKETS }
}

// assemblage des paquets de 3 octets
static mut PKT: [u8; 3] = [0; 3];
static mut PHASE: usize = 0;

fn flush() {
    for _ in 0..4096 {
        if unsafe { inb(STATUS) } & 0x01 == 0 {
            return;
        }
        unsafe {
            let _ = inb(DATA);
        }
    }
}

/// Séquence d'init « canonique » (osdev) : désactive les deux ports,
/// vide, reprogramme l'octet de config, réactive, puis parle à la souris.
pub fn init() {
    ctrl_cmd(0xad); // désactive le port clavier
    ctrl_cmd(0xa7); // désactive le port souris
    flush();

    // octet de configuration du contrôleur
    ctrl_cmd(0x20);
    let mut cfg = if wait_output_full() {
        unsafe { inb(DATA) }
    } else {
        0x47
    };
    cfg &= !0x30; // bits 4/5 = "horloge désactivée" → 0 (on réactive les 2)
    cfg &= !0x02; // IRQ12 souris : on reste en polling
    ctrl_cmd(0x60);
    wait_input_clear();
    unsafe { outb(DATA, cfg) }

    ctrl_cmd(0xae); // réactive le port clavier
    ctrl_cmd(0xa8); // réactive le port souris

    let reset = mouse_cmd(0xff); // reset : ACK 0xFA, puis 0xAA (test) + 0x00 (id)
    // avale le résultat du self-test s'il vient
    if wait_output_full() {
        unsafe {
            let _ = inb(DATA);
        }
    }
    if wait_output_full() {
        unsafe {
            let _ = inb(DATA);
        }
    }

    let defaults = mouse_cmd(0xf6); // réglages par défaut (100 Hz, échelle 1:1)
    let enable = mouse_cmd(0xf4); // active le flux de paquets

    flush();
    crate::serial_println!(
        "[nothing-os] souris PS/2 : reset={:#x} defaults={:#x} enable={:#x}",
        reset,
        defaults,
        enable
    );
}

/// Vide le port et met à jour l'état. À appeler très souvent.
///
/// Points clés :
///  - on lit TOUJOURS l'octet dès que le tampon est plein, même s'il
///    vient du clavier — sinon un octet clavier coincé (le tampon ne fait
///    qu'un octet) bloque définitivement les octets souris ;
///  - un paquet n'est appliqué que si son 1ᵉʳ octet a le bit 3 à 1
///    (toujours vrai pour une vraie souris) ; sinon on se resynchronise.
pub fn poll() {
    unsafe {
        for _ in 0..96 {
            let st = inb(STATUS);
            if st & 0x01 == 0 {
                return; // plus rien
            }
            let b = inb(DATA);
            if st & 0x20 == 0 {
                continue; // octet clavier → jeté, on continue de vider
            }

            if PHASE == 0 && b & 0x08 == 0 {
                continue; // pas un 1ᵉʳ octet valide → resync
            }
            PKT[PHASE] = b;
            PHASE += 1;
            if PHASE == 3 {
                PHASE = 0;
                if PKT[0] & 0x08 != 0 {
                    PACKETS += 1;
                    apply(PKT[0], PKT[1], PKT[2]);
                }
            }
        }
    }
}

unsafe fn apply(flags: u8, dx: u8, dy: u8) {
    if flags & 0xc0 != 0 {
        return; // overflow X ou Y : paquet ignoré
    }
    let mut mx = dx as i32;
    let mut my = dy as i32;
    if flags & 0x10 != 0 {
        mx -= 256;
    }
    if flags & 0x20 != 0 {
        my -= 256;
    }

    X = (X + mx).clamp(0, fb::WIDTH as i32 - 1);
    Y = (Y - my).clamp(0, fb::HEIGHT as i32 - 1); // l'axe Y souris est inversé
    LEFT = flags & 0x01 != 0;
    RIGHT = flags & 0x02 != 0;
}

pub fn state() -> State {
    unsafe {
        State {
            x: X,
            y: Y,
            left: LEFT,
            right: RIGHT,
        }
    }
}

// Curseur flèche 12×19, 1 bit par pixel (0 = transparent, 1 = tracé).
// 'X' = blanc, '.' = transparent, 'o' = contour sombre.
const CURSOR: [&str; 19] = [
    "X.........",
    "Xo........",
    "Xoo.......",
    "Xooo......",
    "Xoooo.....",
    "Xooooo....",
    "Xoooooo...",
    "Xooooooo..",
    "Xoooooooo.",
    "Xooooooooo",
    "Xoooooo...",
    "Xoo.Xoo...",
    "Xo..Xoo...",
    "X....Xoo..",
    ".....Xoo..",
    "......Xoo.",
    "......Xoo.",
    ".......X..",
    "..........",
];

/// Dessine le curseur (pointe en haut-gauche à (x, y)).
pub fn draw_cursor(x: i32, y: i32, white: u8, dark: u8) {
    for (row, line) in CURSOR.iter().enumerate() {
        for (col, ch) in line.bytes().enumerate() {
            let c = match ch {
                b'X' => white,
                b'o' => dark,
                _ => continue,
            };
            fb::put(x + col as i32, y + row as i32, c);
        }
    }
}
