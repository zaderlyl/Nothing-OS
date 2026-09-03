//! Clavier PS/2 (jeu de scancodes 1). On ne décode pas encore de texte :
//! on suit juste quelles touches sont enfoncées, pour le raccourci de
//! fermeture de l'OS.
//!
//! Les octets arrivent par le même contrôleur 8042 que la souris : c'est
//! [`crate::mouse::poll`] qui nous les transmet via [`feed`].

use crate::port::outw;

// scancodes "make" (jeu 1)
const SC_LSHIFT: u8 = 0x2a;
const SC_RSHIFT: u8 = 0x36;
const SC_TAB: u8 = 0x0f;
const SC_LGUI: u8 = 0x5b; // précédé de 0xE0 (touche Windows / Cmd sur Mac)
const SC_RGUI: u8 = 0x5c; // précédé de 0xE0

static mut LSHIFT: bool = false;
static mut RSHIFT: bool = false;
static mut TAB: bool = false;
static mut LGUI: bool = false;
static mut RGUI: bool = false;
static mut EXT: bool = false; // le dernier octet était 0xE0

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
            (false, SC_LSHIFT) => LSHIFT = down,
            (false, SC_RSHIFT) => RSHIFT = down,
            (false, SC_TAB) => TAB = down,
            (true, SC_LGUI) => LGUI = down,
            (true, SC_RGUI) => RGUI = down,
            _ => {}
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
        outw(0x604, 0x2000); // QEMU >= 2.0
        outw(0xb004, 0x2000); // QEMU ancien / Bochs
        outw(0x4004, 0x3400); // VirtualBox, au cas où
    }
    // si rien n'a marché : on fige proprement
    loop {
        unsafe { core::arch::asm!("cli; hlt") }
    }
}
