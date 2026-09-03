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

fn wait_input_clear() {
    for _ in 0..100_000 {
        if inb_status() & 0x02 == 0 {
            return;
        }
    }
}

fn wait_output_full() {
    for _ in 0..100_000 {
        if inb_status() & 0x01 != 0 {
            return;
        }
    }
}

fn inb_status() -> u8 {
    unsafe { inb(STATUS) }
}

fn ctrl_cmd(byte: u8) {
    wait_input_clear();
    unsafe { outb(CMD, byte) }
}

fn write_mouse(byte: u8) {
    wait_input_clear();
    unsafe { outb(CMD, 0xd4) }
    wait_input_clear();
    unsafe { outb(DATA, byte) }
    // ACK
    wait_output_full();
    unsafe {
        let _ = inb(DATA);
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

// assemblage des paquets de 3 octets
static mut PKT: [u8; 3] = [0; 3];
static mut PHASE: usize = 0;

pub fn init() {
    ctrl_cmd(0xa8); // active le port souris

    // octet de config : autorise l'horloge souris (on laisse l'IRQ12
    // désactivée, on est en polling)
    ctrl_cmd(0x20);
    wait_output_full();
    let mut cfg = unsafe { inb(DATA) };
    cfg &= !0x20; // bit 5 = "disable mouse clock" → 0
    ctrl_cmd(0x60);
    wait_input_clear();
    unsafe { outb(DATA, cfg) }

    write_mouse(0xf6); // réglages par défaut
    write_mouse(0xf3); // set sample rate...
    write_mouse(40); // ...40 Hz
    write_mouse(0xf4); // active le flux
}

/// Vide le port et met à jour l'état. À appeler très souvent.
pub fn poll() {
    unsafe {
        // au plus quelques dizaines d'octets par appel
        for _ in 0..64 {
            if inb(STATUS) & 0x21 != 0x21 {
                return; // rien qui vienne de la souris
            }
            let b = inb(DATA);

            if PHASE == 0 && b & 0x08 == 0 {
                continue; // resync : le 1er octet a toujours le bit 3 à 1
            }
            PKT[PHASE] = b;
            PHASE += 1;
            if PHASE == 3 {
                PHASE = 0;
                apply(PKT[0], PKT[1], PKT[2]);
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
