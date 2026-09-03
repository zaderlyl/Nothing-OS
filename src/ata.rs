//! Pilote de disque ATA (IDE) en PIO, canal primaire, lecteur maître.
//! De quoi lire/écrire des secteurs de 512 octets — assez pour rendre le
//! système de fichiers persistant.
//!
//! Le "disque" est une image (`nothingos.img`) sur le Mac, que QEMU
//! présente au noyau comme un vrai disque dur. Ce n'est pas le disque de
//! macOS (impossible depuis du bare-metal), mais les fichiers survivent
//! au redémarrage de Nothing OS.

#![allow(dead_code)]

use crate::port::{inb, inw, outb, outw};

const IO: u16 = 0x1f0;
const CTRL: u16 = 0x3f6;

pub const SECTOR: usize = 512;

fn wait_ready() -> bool {
    // attend BSY=0 puis DRDY=1
    for _ in 0..4_000_000 {
        let st = unsafe { inb(IO + 7) };
        if st & 0x80 == 0 && st & 0x40 != 0 {
            return true;
        }
        core::hint::spin_loop();
    }
    false
}

fn wait_drq() -> bool {
    for _ in 0..4_000_000 {
        let st = unsafe { inb(IO + 7) };
        if st & 0x01 != 0 {
            return false; // ERR
        }
        if st & 0x08 != 0 {
            return true; // DRQ
        }
        core::hint::spin_loop();
    }
    false
}

fn select(lba: u32, count: u8) {
    unsafe {
        outb(IO + 6, 0xe0 | ((lba >> 24) & 0x0f) as u8); // maître, LBA
        outb(IO + 2, count);
        outb(IO + 3, lba as u8);
        outb(IO + 4, (lba >> 8) as u8);
        outb(IO + 5, (lba >> 16) as u8);
    }
}

/// Y a-t-il un disque sur le canal primaire ? (status != 0 après reset)
pub fn present() -> bool {
    unsafe {
        outb(IO + 6, 0xe0);
        for _ in 0..1000 {
            core::hint::spin_loop();
        }
        let st = inb(IO + 7);
        st != 0 && st != 0xff
    }
}

/// Lit `count` secteurs à partir de `lba` dans `buf` (>= count*512).
pub fn read(lba: u32, count: u8, buf: &mut [u8]) -> bool {
    if !wait_ready() {
        return false;
    }
    select(lba, count);
    unsafe { outb(IO + 7, 0x20) } // READ SECTORS
    for s in 0..count as usize {
        if !wait_drq() {
            return false;
        }
        for i in 0..SECTOR / 2 {
            let w = unsafe { inw(IO) };
            buf[s * SECTOR + i * 2] = w as u8;
            buf[s * SECTOR + i * 2 + 1] = (w >> 8) as u8;
        }
    }
    true
}

/// Écrit `count` secteurs depuis `buf` à partir de `lba`.
pub fn write(lba: u32, count: u8, buf: &[u8]) -> bool {
    if !wait_ready() {
        return false;
    }
    select(lba, count);
    unsafe { outb(IO + 7, 0x30) } // WRITE SECTORS
    for s in 0..count as usize {
        if !wait_drq() {
            return false;
        }
        for i in 0..SECTOR / 2 {
            let lo = buf[s * SECTOR + i * 2] as u16;
            let hi = buf[s * SECTOR + i * 2 + 1] as u16;
            unsafe { outw(IO, lo | (hi << 8)) }
        }
    }
    // FLUSH CACHE
    unsafe { outb(IO + 7, 0xe7) }
    wait_ready();
    let _ = CTRL;
    true
}
