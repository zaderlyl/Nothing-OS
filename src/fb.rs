//! Mode graphique VGA 13h : 320×200, 256 couleurs, framebuffer linéaire à
//! l'adresse physique 0xA0000 (1 octet = 1 index de palette).
//!
//! On programme les registres VGA directement (pas de BIOS : on est déjà
//! en long mode). Le jeu de valeurs ci-dessous est le "dump" standard du
//! mode 13h. La palette DAC est 6-bit par canal.
//!
//! Le dessin se fait dans un back-buffer (`BACK`) puis `present()` le
//! recopie d'un bloc vers la VRAM — pas de scintillement.

use crate::port::{inb, outb};

pub const WIDTH: usize = 320;
pub const HEIGHT: usize = 200;

const FB_ADDR: usize = 0xA_0000;

// --- registres ---
const ATTR_ADDR: u16 = 0x3c0;
const MISC_W: u16 = 0x3c2;
const SEQ_ADDR: u16 = 0x3c4;
const SEQ_DATA: u16 = 0x3c5;
const GC_ADDR: u16 = 0x3ce;
const GC_DATA: u16 = 0x3cf;
const CRTC_ADDR: u16 = 0x3d4;
const CRTC_DATA: u16 = 0x3d5;
const INPUT_STATUS1: u16 = 0x3da;
const DAC_WRITE_ADDR: u16 = 0x3c8;
const DAC_DATA: u16 = 0x3c9;

const MISC: u8 = 0x63;
const SEQ: [u8; 5] = [0x03, 0x01, 0x0f, 0x00, 0x0e];
const CRTC: [u8; 25] = [
    0x5f, 0x4f, 0x50, 0x82, 0x54, 0x80, 0xbf, 0x1f, 0x00, 0x41, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x9c, 0x0e, 0x8f, 0x28, 0x40, 0x96, 0xb9, 0xa3, 0xff,
];
const GC: [u8; 9] = [0x00, 0x00, 0x00, 0x00, 0x00, 0x40, 0x05, 0x0f, 0xff];
const AC: [u8; 21] = [
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
    0x41, 0x00, 0x0f, 0x00, 0x00,
];

static mut BACK: [u8; WIDTH * HEIGHT] = [0; WIDTH * HEIGHT];

fn back() -> *mut u8 {
    &raw mut BACK as *mut u8
}

/// Bascule la carte VGA en mode 13h.
pub fn set_mode13() {
    unsafe {
        outb(MISC_W, MISC);

        for (i, &v) in SEQ.iter().enumerate() {
            outb(SEQ_ADDR, i as u8);
            outb(SEQ_DATA, v);
        }

        // Déverrouille les registres CRTC (bit 7 de l'index 0x11).
        outb(CRTC_ADDR, 0x11);
        let v = inb(CRTC_DATA);
        outb(CRTC_DATA, v & 0x7f);

        for (i, &v) in CRTC.iter().enumerate() {
            outb(CRTC_ADDR, i as u8);
            let v = if i == 0x11 { v & 0x7f } else { v };
            outb(CRTC_DATA, v);
        }

        for (i, &v) in GC.iter().enumerate() {
            outb(GC_ADDR, i as u8);
            outb(GC_DATA, v);
        }

        for (i, &v) in AC.iter().enumerate() {
            let _ = inb(INPUT_STATUS1);
            outb(ATTR_ADDR, i as u8);
            outb(ATTR_ADDR, v);
        }
        let _ = inb(INPUT_STATUS1);
        outb(ATTR_ADDR, 0x20); // réactive l'affichage
    }
}

/// Programme une entrée de la palette (composantes 0..=255, tronquées 6 bits).
pub fn set_palette(index: u8, r: u8, g: u8, b: u8) {
    unsafe {
        outb(DAC_WRITE_ADDR, index);
        outb(DAC_DATA, r >> 2);
        outb(DAC_DATA, g >> 2);
        outb(DAC_DATA, b >> 2);
    }
}

/// Recopie le back-buffer vers la VRAM.
pub fn present() {
    unsafe {
        core::ptr::copy_nonoverlapping(back(), FB_ADDR as *mut u8, WIDTH * HEIGHT);
    }
}

#[inline(always)]
pub fn put(x: i32, y: i32, color: u8) {
    if x < 0 || y < 0 || x >= WIDTH as i32 || y >= HEIGHT as i32 {
        return;
    }
    unsafe {
        *back().add(y as usize * WIDTH + x as usize) = color;
    }
}

pub fn fill_rect(x: i32, y: i32, w: i32, h: i32, color: u8) {
    for yy in y..(y + h) {
        for xx in x..(x + w) {
            put(xx, yy, color);
        }
    }
}

/// Disque plein anti-crénelé grossièrement (test au rayon).
pub fn fill_circle(cx: f32, cy: f32, rad: f32, color: u8) {
    let x0 = (cx - rad - 1.0) as i32;
    let x1 = (cx + rad + 1.0) as i32;
    let y0 = (cy - rad - 1.0) as i32;
    let y1 = (cy + rad + 1.0) as i32;
    let r2 = rad * rad;
    for y in y0..=y1 {
        for x in x0..=x1 {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            if dx * dx + dy * dy <= r2 {
                put(x, y, color);
            }
        }
    }
}
