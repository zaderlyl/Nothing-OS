//! Clavier PS/2 (jeu de scancodes 1). Suit les touches modificatrices
//! (pour le raccourci de fermeture) **et** produit des caractères ASCII
//! pour la barre de commande.
//!
//! Les octets arrivent par le contrôleur 8042 via [`crate::mouse::poll`],
//! qui appelle [`feed`].

use crate::port::outw;

// scancodes "make" (jeu 1)
const SC_LSHIFT: u8 = 0x2a;
const SC_RSHIFT: u8 = 0x36;
const SC_TAB: u8 = 0x0f;
const SC_LGUI: u8 = 0x5b; // 0xE0 préfixe — touche Cmd / Windows
const SC_RGUI: u8 = 0x5c;
const SC_BACKSPACE: u8 = 0x0e;
const SC_ENTER: u8 = 0x1c;
const SC_SPACE: u8 = 0x39;

static mut LSHIFT: bool = false;
static mut RSHIFT: bool = false;
static mut TAB: bool = false;
static mut LGUI: bool = false;
static mut RGUI: bool = false;
static mut EXT: bool = false;

// petit tampon circulaire de caractères tapés
const BUF: usize = 64;
static mut CHARS: [u8; BUF] = [0; BUF];
static mut HEAD: usize = 0;
static mut TAIL: usize = 0;

fn push_char(c: u8) {
    unsafe {
        let next = (HEAD + 1) % BUF;
        if next != TAIL {
            CHARS[HEAD] = c;
            HEAD = next;
        }
    }
}

/// Récupère le prochain caractère tapé (0 si rien). `\n` = entrée,
/// `0x08` = retour arrière.
pub fn pop_char() -> u8 {
    unsafe {
        if TAIL == HEAD {
            return 0;
        }
        let c = CHARS[TAIL];
        TAIL = (TAIL + 1) % BUF;
        c
    }
}

/// Table scancode (jeu 1) → ASCII pour un clavier **AZERTY** (français),
/// sans / avec Maj. Les lettres accentuées sont repliées sur leur base
/// (é→e, ç→c...) puisqu'on ne fait pas d'Unicode. Le `/` des commandes
/// s'obtient avec **Maj + `:`** (ou le `/` du pavé numérique).
const MAP: [(u8, u8, u8); 47] = [
    // rangée des chiffres : & é " ' ( - è _ ç à ) =  /  1 2 3 4 5 6 7 8 9 0 ° +
    (0x02, b'&', b'1'),
    (0x03, b'e', b'2'),
    (0x04, b'"', b'3'),
    (0x05, b'\'', b'4'),
    (0x06, b'(', b'5'),
    (0x07, b'-', b'6'),
    (0x08, b'e', b'7'),
    (0x09, b'_', b'8'),
    (0x0a, b'c', b'9'),
    (0x0b, b'a', b'0'),
    (0x0c, b')', b')'),
    (0x0d, b'=', b'+'),
    // a z e r t y u i o p ^ $
    (0x10, b'a', b'A'),
    (0x11, b'z', b'Z'),
    (0x12, b'e', b'E'),
    (0x13, b'r', b'R'),
    (0x14, b't', b'T'),
    (0x15, b'y', b'Y'),
    (0x16, b'u', b'U'),
    (0x17, b'i', b'I'),
    (0x18, b'o', b'O'),
    (0x19, b'p', b'P'),
    (0x1a, b'^', b'^'),
    (0x1b, b'$', b'*'),
    // q s d f g h j k l m u *
    (0x1e, b'q', b'Q'),
    (0x1f, b's', b'S'),
    (0x20, b'd', b'D'),
    (0x21, b'f', b'F'),
    (0x22, b'g', b'G'),
    (0x23, b'h', b'H'),
    (0x24, b'j', b'J'),
    (0x25, b'k', b'K'),
    (0x26, b'l', b'L'),
    (0x27, b'm', b'M'),
    (0x28, b'u', b'%'),
    (0x2b, b'*', b'u'),
    // < w x c v b n , ; : !
    (0x56, b'<', b'>'),
    (0x2c, b'w', b'W'),
    (0x2d, b'x', b'X'),
    (0x2e, b'c', b'C'),
    (0x2f, b'v', b'V'),
    (0x30, b'b', b'B'),
    (0x31, b'n', b'N'),
    (0x32, b',', b'?'),
    (0x33, b';', b'.'),
    (0x34, b':', b'/'),
    (0x35, b'!', b'!'),
];

/// Transmet un octet reçu du contrôleur 8042 (port clavier).
pub fn feed(byte: u8) {
    unsafe {
        if byte == 0xe0 {
            EXT = true;
            return;
        }
        let ext = EXT;
        EXT = false;

        let released = byte & 0x80 != 0;
        let code = byte & 0x7f;
        let down = !released;

        match (ext, code) {
            (false, SC_LSHIFT) => {
                LSHIFT = down;
                return;
            }
            (false, SC_RSHIFT) => {
                RSHIFT = down;
                return;
            }
            (false, SC_TAB) => {
                TAB = down;
                return;
            }
            (true, SC_LGUI) => {
                LGUI = down;
                return;
            }
            (true, SC_RGUI) => {
                RGUI = down;
                return;
            }
            _ => {}
        }

        if !down {
            return;
        }
        if ext {
            match code {
                0x1c => push_char(b'\n'),     // Entrée du pavé numérique
                0x35 => push_char(b'/'),      // division du pavé numérique
                0x48 => push_char(0x11),      // flèche haut  (K_UP)
                0x50 => push_char(0x12),      // flèche bas   (K_DOWN)
                0x4b => push_char(0x13),      // flèche gauche
                0x4d => push_char(0x14),      // flèche droite
                _ => {}
            }
            return;
        }

        let shift = LSHIFT || RSHIFT;
        match code {
            0x01 => push_char(0x1b), // Échap
            SC_ENTER => push_char(b'\n'),
            SC_BACKSPACE => push_char(0x08),
            SC_SPACE => push_char(b' '),
            _ => {
                for &(sc, lo, hi) in MAP.iter() {
                    if sc == code {
                        push_char(if shift { hi } else { lo });
                        break;
                    }
                }
            }
        }
    }
}

/// Le raccourci de fermeture est-il enfoncé ? (Maj + Tab + Cmd)
pub fn close_combo() -> bool {
    unsafe { (LSHIFT || RSHIFT) && TAB && (LGUI || RGUI) }
}

/// Éteint la machine (ACPI, méthode QEMU). Ne revient pas.
pub fn power_off() -> ! {
    unsafe {
        outw(0x604, 0x2000);
        outw(0xb004, 0x2000);
        outw(0x4004, 0x3400);
    }
    loop {
        unsafe { core::arch::asm!("cli; hlt") }
    }
}
