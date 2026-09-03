//! Pilote minimal pour le buffer texte VGA (mode 80x25, à l'adresse
//! physique 0xb8000). Chaque caractère occupe 2 octets : le code ASCII
//! puis un octet d'attribut (couleur avant-plan / fond).

use core::fmt;

const BUFFER_WIDTH: usize = 80;
const BUFFER_HEIGHT: usize = 25;
const VGA_BUFFER_ADDR: usize = 0xb8000;

#[allow(dead_code)]
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    LightGray = 7,
    DarkGray = 8,
    LightBlue = 9,
    LightGreen = 10,
    LightCyan = 11,
    LightRed = 12,
    Pink = 13,
    Yellow = 14,
    White = 15,
}

#[derive(Clone, Copy)]
struct ColorCode(u8);

impl ColorCode {
    const fn new(foreground: Color, background: Color) -> ColorCode {
        ColorCode((background as u8) << 4 | (foreground as u8))
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct ScreenChar {
    ascii_character: u8,
    color_code: u8,
}

// SAFETY: le pointeur `buffer` vise une adresse mémoire fixe (0xb8000),
// pas de la mémoire allouée sur un thread en particulier ; l'accès
// concurrent est de toute façon empêché par le mutex qui enveloppe ce
// Writer (voir `crate::WRITER`), donc il est sûr de considérer ce type
// comme Send + Sync.
unsafe impl Send for Writer {}
unsafe impl Sync for Writer {}

pub struct Writer {
    column_position: usize,
    row_position: usize,
    color_code: ColorCode,
    buffer: *mut ScreenChar,
}

impl Writer {
    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.new_line(),
            byte => {
                if self.column_position >= BUFFER_WIDTH {
                    self.new_line();
                }

                let row = self.row_position;
                let col = self.column_position;

                let screen_char = ScreenChar {
                    ascii_character: byte,
                    color_code: self.color_code.0,
                };

                unsafe {
                    let offset = row * BUFFER_WIDTH + col;
                    core::ptr::write_volatile(self.buffer.add(offset), screen_char);
                }

                self.column_position += 1;
            }
        }
    }

    pub fn write_string(&mut self, s: &str) {
        for byte in s.bytes() {
            match byte {
                // ASCII imprimable ou saut de ligne
                0x20..=0x7e | b'\n' => self.write_byte(byte),
                // tout le reste (accents non gérés en mode texte, etc.)
                _ => self.write_byte(0xfe),
            }
        }
    }

    pub fn set_color(&mut self, foreground: Color, background: Color) {
        self.color_code = ColorCode::new(foreground, background);
    }

    /// Place le curseur à une position absolue (ligne, colonne).
    /// Les valeurs hors écran sont ramenées au bord.
    pub fn set_position(&mut self, row: usize, col: usize) {
        self.row_position = row.min(BUFFER_HEIGHT - 1);
        self.column_position = col.min(BUFFER_WIDTH - 1);
    }

    /// Écrit une cellule brute (caractère + couleurs) à une position
    /// donnée, sans bouger le curseur ni filtrer l'octet. Sert au rendu
    /// "graphique" en mode texte (demi-blocs `0xDF`/`0xDC` = 2 pixels par
    /// cellule) — voir `crate::home`.
    pub fn put_cell(&mut self, row: usize, col: usize, ch: u8, fg: Color, bg: Color) {
        if row >= BUFFER_HEIGHT || col >= BUFFER_WIDTH {
            return;
        }
        let cell = ScreenChar {
            ascii_character: ch,
            color_code: ColorCode::new(fg, bg).0,
        };
        unsafe {
            core::ptr::write_volatile(self.buffer.add(row * BUFFER_WIDTH + col), cell);
        }
    }

    fn new_line(&mut self) {
        if self.row_position + 1 < BUFFER_HEIGHT {
            self.row_position += 1;
        } else {
            // fait défiler l'écran d'une ligne vers le haut
            for row in 1..BUFFER_HEIGHT {
                for col in 0..BUFFER_WIDTH {
                    unsafe {
                        let src = self.buffer.add(row * BUFFER_WIDTH + col);
                        let dst = self.buffer.add((row - 1) * BUFFER_WIDTH + col);
                        let c = core::ptr::read_volatile(src);
                        core::ptr::write_volatile(dst, c);
                    }
                }
            }
            self.clear_row(BUFFER_HEIGHT - 1);
        }
        self.column_position = 0;
    }

    fn clear_row(&mut self, row: usize) {
        let blank = ScreenChar {
            ascii_character: b' ',
            color_code: self.color_code.0,
        };
        for col in 0..BUFFER_WIDTH {
            unsafe {
                core::ptr::write_volatile(self.buffer.add(row * BUFFER_WIDTH + col), blank);
            }
        }
    }

    pub fn clear_screen(&mut self) {
        for row in 0..BUFFER_HEIGHT {
            self.clear_row(row);
        }
        self.row_position = 0;
        self.column_position = 0;
    }
}

impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}

// ---------------------------------------------------------------------
// Contrôleur d'attributs VGA : on désactive le clignotement.
//
// En mode texte, le bit 7 de l'octet d'attribut sert par défaut de
// "blink" : une cellule dont le fond est une couleur claire (8..15)
// clignote. Le rendu d'Asti en demi-blocs utilise justement des fonds
// clairs, donc on bascule ce bit pour qu'il désigne la couleur de fond
// (16 couleurs de fond au lieu de 8) — comportement "high intensity
// background".
// ---------------------------------------------------------------------

#[inline(always)]
unsafe fn outb(port: u16, value: u8) {
    core::arch::asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags));
}

#[inline(always)]
unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    core::arch::asm!("in al, dx", in("dx") port, out("al") value, options(nomem, nostack, preserves_flags));
    value
}

/// Désactive le clignotement du mode texte (le bit 7 de l'attribut
/// devient "fond haute intensité"). À appeler une fois avant tout rendu
/// en demi-blocs.
///
/// Protocole du contrôleur d'attributs (capricieux) :
///  1. lire 0x3DA → remet le flip-flop en mode "index" ;
///  2. écrire l'index sur 0x3C0 → le flip-flop passe en mode "données" ;
///  3. lire la valeur sur 0x3C1 (la lecture ne bouge pas le flip-flop) ;
///  4. réécrire la valeur (bit 3 effacé) sur 0x3C0 → flip-flop en "index" ;
///  5. écrire 0x20 sur 0x3C0 → remet "Palette Address Source", ce qui
///     réactive l'affichage (sans ça, écran noir).
pub fn disable_blink() {
    const ATTR_ADDR: u16 = 0x3c0;
    const ATTR_READ: u16 = 0x3c1;
    const INPUT_STATUS1: u16 = 0x3da;
    const MODE_CONTROL_INDEX: u8 = 0x10;
    const BLINK_ENABLE: u8 = 0x08; // bit 3 du "Attribute Mode Control"
    const PAS: u8 = 0x20; // "Palette Address Source"

    unsafe {
        let _ = inb(INPUT_STATUS1);
        outb(ATTR_ADDR, MODE_CONTROL_INDEX);
        let mode = inb(ATTR_READ);
        outb(ATTR_ADDR, mode & !BLINK_ENABLE);
        outb(ATTR_ADDR, PAS);
    }
}

impl Writer {
    /// Construit le writer pointant sur le buffer VGA physique.
    ///
    /// Ce n'est PAS marqué `unsafe` : construire la struct ne touche pas
    /// encore à la mémoire (le pointeur n'est que calculé). C'est
    /// pourquoi une seule instance doit exister dans tout le noyau,
    /// partagée via `crate::WRITER` (protégée par un mutex) plutôt que
    /// d'en recréer une à chaque endroit qui veut écrire à l'écran —
    /// sinon chaque instance "oublie" où en étaient les autres et elles
    /// s'écrasent mutuellement (position du curseur non partagée).
    pub const fn new() -> Writer {
        Writer {
            column_position: 0,
            row_position: 0,
            color_code: ColorCode::new(Color::LightGreen, Color::Black),
            buffer: VGA_BUFFER_ADDR as *mut ScreenChar,
        }
    }
}
