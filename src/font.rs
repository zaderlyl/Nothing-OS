//! Police 8×16 : on récupère celle que le BIOS a chargée dans le plan 2
//! de la mémoire vidéo (tant qu'on est encore en mode texte), on la garde
//! dans un tableau, et on la redessine pixel par pixel en mode graphique.

#![allow(dead_code)]

use crate::fb;
use crate::port::{inb, outb};

const GLYPH_H: usize = 16;

static mut FONT: [[u8; GLYPH_H]; 256] = [[0; GLYPH_H]; 256];

/// Copie la police depuis le plan 2 de la VRAM. **À appeler en mode
/// texte, avant `fb::set_mode13()`.**
pub fn capture() {
    unsafe {
        // sauvegarde des registres qu'on va tripoter
        outb(0x3c4, 0x02);
        let seq2 = inb(0x3c5);
        outb(0x3c4, 0x04);
        let seq4 = inb(0x3c5);
        outb(0x3ce, 0x04);
        let gc4 = inb(0x3cf);
        outb(0x3ce, 0x05);
        let gc5 = inb(0x3cf);
        outb(0x3ce, 0x06);
        let gc6 = inb(0x3cf);

        // accès linéaire au plan 2 (celui qui contient les glyphes)
        outb(0x3c4, 0x02);
        outb(0x3c5, 0x04);
        outb(0x3c4, 0x04);
        outb(0x3c5, 0x06);
        outb(0x3ce, 0x04);
        outb(0x3cf, 0x02);
        outb(0x3ce, 0x05);
        outb(0x3cf, 0x00);
        outb(0x3ce, 0x06);
        outb(0x3cf, 0x00);

        let vram = 0xa_0000 as *const u8;
        for c in 0..256 {
            for row in 0..GLYPH_H {
                FONT[c][row] = *vram.add(c * 32 + row);
            }
        }

        // restauration
        outb(0x3c4, 0x02);
        outb(0x3c5, seq2);
        outb(0x3c4, 0x04);
        outb(0x3c5, seq4);
        outb(0x3ce, 0x04);
        outb(0x3cf, gc4);
        outb(0x3ce, 0x05);
        outb(0x3cf, gc5);
        outb(0x3ce, 0x06);
        outb(0x3cf, gc6);
    }
}

/// Dessine un caractère (code page 437). `bg = None` → fond transparent.
pub fn draw_char(x: i32, y: i32, ch: u8, fg: u8, bg: Option<u8>) {
    draw_char_scaled(x, y, ch, fg, bg, 1);
}

/// Idem, mais chaque pixel du glyphe devient un pavé `scale × scale`.
pub fn draw_char_scaled(x: i32, y: i32, ch: u8, fg: u8, bg: Option<u8>, scale: i32) {
    let glyph = unsafe { &FONT[ch as usize] };
    for (row, bits) in glyph.iter().enumerate() {
        for col in 0..8i32 {
            let on = bits & (0x80 >> col) != 0;
            let c = if on {
                Some(fg)
            } else {
                bg
            };
            if let Some(c) = c {
                fb::fill_rect(x + col * scale, y + row as i32 * scale, scale, scale, c);
            }
        }
    }
}

/// Chaîne à l'échelle `scale` (avance de `8 * scale` par caractère).
pub fn draw_str_scaled(x: i32, y: i32, s: &str, fg: u8, scale: i32) {
    let mut cx = x;
    for &b in s.as_bytes() {
        draw_char_scaled(cx, y, b, fg, None, scale);
        cx += 8 * scale;
    }
}

/// Largeur en pixels d'une chaîne à l'échelle `scale`.
pub fn width_scaled(s: &str, scale: i32) -> i32 {
    s.len() as i32 * 8 * scale
}

/// Chaîne rendue en **points** (façon matrice de LED) : chaque pixel du
/// glyphe devient un petit disque, avec de l'espace autour. `cell` est le
/// pas entre points.
pub fn draw_str_dots(x: i32, y: i32, s: &str, fg: u8, cell: i32) {
    let r = (cell as f32) * 0.34;
    let mut cx = x;
    for &b in s.as_bytes() {
        let glyph = unsafe { &FONT[b as usize] };
        for (row, bits) in glyph.iter().enumerate() {
            for col in 0..8i32 {
                if bits & (0x80 >> col) != 0 {
                    let px = (cx + col * cell + cell / 2) as f32;
                    let py = (y + row as i32 * cell + cell / 2) as f32;
                    fb::fill_circle(px, py, r, fg);
                }
            }
        }
        cx += 8 * cell;
    }
}

/// Dessine une chaîne ASCII, avance de 8 px par caractère.
pub fn draw_str(x: i32, y: i32, s: &str, fg: u8, bg: Option<u8>) {
    let mut cx = x;
    for &b in s.as_bytes() {
        draw_char(cx, y, b, fg, bg);
        cx += 8;
    }
}

/// Entier sur `digits` chiffres (zéros à gauche), à l'échelle `scale`.
pub fn draw_num(x: i32, y: i32, mut n: u32, digits: usize, fg: u8, scale: i32) {
    for i in (0..digits).rev() {
        let d = (n % 10) as u8;
        n /= 10;
        draw_char_scaled(x + i as i32 * 8 * scale, y, b'0' + d, fg, None, scale);
    }
}
