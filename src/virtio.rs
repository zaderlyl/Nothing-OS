//! Transport virtio-pci **legacy** (0.9.5) avec une seule file (virtqueue),
//! juste ce qu'il faut pour parler au périphérique virtio-9p de QEMU.
//!
//! Grâce à l'identity-mapping, adresse virtuelle == adresse physique :
//! on peut donner l'adresse d'un tableau statique directement au
//! périphérique.

#![allow(dead_code)]

use core::sync::atomic::{fence, Ordering};

use crate::pci;
use crate::port::{inb, inl, inw, outb, outl, outw};

const VIRTIO_VENDOR: u16 = 0x1af4;
const DEV_9P: u16 = 0x1009; // virtio-9p-pci (legacy id)

// registres (offsets dans le BAR0 en espace d'E/S)
const R_HOST_FEATURES: u16 = 0x00;
const R_GUEST_FEATURES: u16 = 0x04;
const R_QUEUE_PFN: u16 = 0x08;
const R_QUEUE_NUM: u16 = 0x0c;
const R_QUEUE_SEL: u16 = 0x0e;
const R_QUEUE_NOTIFY: u16 = 0x10;
const R_STATUS: u16 = 0x12;
const R_ISR: u16 = 0x13;
const R_CONFIG: u16 = 0x14;

const S_ACK: u8 = 1;
const S_DRIVER: u8 = 2;
const S_DRIVER_OK: u8 = 4;
const S_FAILED: u8 = 0x80;

const QSIZE: usize = 128;
const F_NEXT: u16 = 1;
const F_WRITE: u16 = 2;

const BUF_CAP: usize = 8192;

#[repr(C)]
#[derive(Clone, Copy)]
struct Desc {
    addr: u64,
    len: u32,
    flags: u16,
    next: u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct UsedElem {
    id: u32,
    len: u32,
}

#[repr(C, align(4096))]
struct Ring {
    desc: [Desc; QSIZE],
    // avail
    avail_flags: u16,
    avail_idx: u16,
    avail_ring: [u16; QSIZE],
    avail_evt: u16,
    _pad: [u8; 4096 - (QSIZE * 16 + 6 + QSIZE * 2)],
    // used (aligné sur 4096)
    used_flags: u16,
    used_idx: u16,
    used_ring: [UsedElem; QSIZE],
    used_evt: u16,
}

#[repr(align(4096))]
struct Buf([u8; BUF_CAP]);

static mut RING: Ring = Ring {
    desc: [Desc {
        addr: 0,
        len: 0,
        flags: 0,
        next: 0,
    }; QSIZE],
    avail_flags: 0,
    avail_idx: 0,
    avail_ring: [0; QSIZE],
    avail_evt: 0,
    _pad: [0; 4096 - (QSIZE * 16 + 6 + QSIZE * 2)],
    used_flags: 0,
    used_idx: 0,
    used_ring: [UsedElem { id: 0, len: 0 }; QSIZE],
    used_evt: 0,
};
static mut TX: Buf = Buf([0; BUF_CAP]);
static mut RX: Buf = Buf([0; BUF_CAP]);

static mut IOBASE: u16 = 0;
static mut LAST_USED: u16 = 0;
static mut READY: bool = false;

fn r8(off: u16) -> u8 {
    unsafe { inb(IOBASE + off) }
}
fn w8(off: u16, v: u8) {
    unsafe { outb(IOBASE + off, v) }
}
fn r16(off: u16) -> u16 {
    unsafe { inw(IOBASE + off) }
}
fn w16(off: u16, v: u16) {
    unsafe { outw(IOBASE + off, v) }
}
fn r32(off: u16) -> u32 {
    unsafe { inl(IOBASE + off) }
}
fn w32(off: u16, v: u32) {
    unsafe { outl(IOBASE + off, v) }
}

pub fn present() -> bool {
    unsafe { READY }
}

/// Cherche et initialise le périphérique virtio-9p. `false` si absent.
pub fn init_9p() -> bool {
    let dev = match pci::find(VIRTIO_VENDOR, DEV_9P) {
        Some(d) => d,
        None => {
            crate::serial_println!("[nothing-os] virtio-9p : pas trouve");
            return false;
        }
    };
    dev.enable_bus_master();
    let bar0 = dev.bar(0) & 0xffff_fffc;
    unsafe {
        IOBASE = bar0 as u16;
    }

    // séquence d'init
    w8(R_STATUS, 0); // reset
    w8(R_STATUS, S_ACK);
    w8(R_STATUS, S_ACK | S_DRIVER);
    let host_features = r32(R_HOST_FEATURES);
    w32(R_GUEST_FEATURES, host_features & 1); // on garde juste MOUNT_TAG

    // file 0
    w16(R_QUEUE_SEL, 0);
    let qnum = r16(R_QUEUE_NUM);
    if qnum == 0 || (qnum as usize) < 2 {
        w8(R_STATUS, S_FAILED);
        crate::serial_println!("[nothing-os] virtio-9p : file invalide ({})", qnum);
        return false;
    }
    unsafe {
        RING.avail_idx = 0;
        RING.used_idx = 0;
        LAST_USED = 0;
        let pfn = (&raw const RING as u64) >> 12;
        w32(R_QUEUE_PFN, pfn as u32);
    }

    w8(R_STATUS, S_ACK | S_DRIVER | S_DRIVER_OK);
    unsafe {
        READY = true;
    }
    crate::serial_println!("[nothing-os] virtio-9p @ io {:#x}, file {}", bar0, qnum);
    true
}

/// Envoie `req` au périphérique et attend la réponse ; renvoie le nombre
/// d'octets reçus (0 en cas d'échec). Réponse disponible via `resp()`.
pub fn request(req: &[u8]) -> usize {
    unsafe {
        if !READY || req.len() > BUF_CAP {
            return 0;
        }
        TX.0[..req.len()].copy_from_slice(req);

        let tx_addr = &raw const TX as u64;
        let rx_addr = &raw const RX as u64;

        RING.desc[0] = Desc {
            addr: tx_addr,
            len: req.len() as u32,
            flags: F_NEXT,
            next: 1,
        };
        RING.desc[1] = Desc {
            addr: rx_addr,
            len: BUF_CAP as u32,
            flags: F_WRITE,
            next: 0,
        };

        // le périphérique lit/écrit ces champs en parallèle → accès volatils
        let avail_idx_p = &raw mut RING.avail_idx;
        let used_idx_p = &raw const RING.used_idx;
        let ring_p = &raw mut RING.avail_ring as *mut u16;

        let cur = core::ptr::read_volatile(avail_idx_p);
        core::ptr::write_volatile(ring_p.add((cur as usize) % QSIZE), 0u16); // tête = desc 0
        fence(Ordering::SeqCst);
        core::ptr::write_volatile(avail_idx_p, cur.wrapping_add(1));
        fence(Ordering::SeqCst);
        w16(R_QUEUE_NOTIFY, 0);

        let mut guard = 0u32;
        while core::ptr::read_volatile(used_idx_p) == LAST_USED && guard < 200_000_000 {
            core::hint::spin_loop();
            guard += 1;
        }
        if core::ptr::read_volatile(used_idx_p) == LAST_USED {
            return 0; // timeout
        }
        let e = RING.used_ring[(LAST_USED as usize) % QSIZE];
        LAST_USED = LAST_USED.wrapping_add(1);
        let _ = r8(R_ISR);
        (e.len as usize).min(BUF_CAP)
    }
}

/// Tampon de réponse de la dernière `request`.
pub fn resp() -> &'static [u8] {
    unsafe { &RX.0[..] }
}
