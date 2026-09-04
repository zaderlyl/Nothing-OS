//! Pilote USB minimal : contrôleur **UHCI** (PIIX3, `-usb` de QEMU) +
//! **tablette HID** (`-device usb-tablet`). La tablette est un pointeur
//! *absolu* : QEMU n'a alors plus besoin de « capturer » la souris, donc
//! le curseur du Mac reste libre (on peut attraper Asti) et le pointeur
//! de l'OS est précis, sans clic préalable.
//!
//! Tout est en *polling*, sans interruptions. Identity-map ⇒ adresse
//! virtuelle = adresse physique pour les tableaux statiques.

#![allow(dead_code, static_mut_refs)]

use crate::pci;
use crate::port::{inw, outl, outw};
use crate::time;

// --- registres UHCI (offset / base d'E/S) ---
const USBCMD: u16 = 0x00;
const USBSTS: u16 = 0x02;
const USBINTR: u16 = 0x04;
const FRNUM: u16 = 0x06;
const FRBASEADD: u16 = 0x08;
const SOFMOD: u16 = 0x0c;
const PORTSC1: u16 = 0x10;

const CMD_RS: u16 = 1 << 0; // run/stop
const CMD_HCRESET: u16 = 1 << 1;
const CMD_GRESET: u16 = 1 << 2;
const CMD_MAXP: u16 = 1 << 7;
const CMD_CF: u16 = 1 << 6;

const PORT_CCS: u16 = 1 << 0; // périphérique présent
const PORT_CSC: u16 = 1 << 1; // changement de connexion (w1c)
const PORT_PE: u16 = 1 << 2; // port activé
const PORT_PEC: u16 = 1 << 3; // changement d'activation (w1c)
const PORT_LS: u16 = 1 << 8; // low speed
const PORT_RESET: u16 = 1 << 9;

// --- structures matérielles ---
#[repr(C, align(16))]
#[derive(Clone, Copy)]
struct Td {
    link: u32,
    cs: u32,
    token: u32,
    buf: u32,
}

#[repr(C, align(16))]
struct Qh {
    head: u32,
    elem: u32,
}

#[repr(C, align(4096))]
struct FrameList([u32; 1024]);

const LINK_T: u32 = 1; // terminate
const LINK_Q: u32 = 2; // pointe sur une QH

// status/control TD
const TD_ACTIVE: u32 = 1 << 23;
const TD_IOC: u32 = 1 << 24;
const TD_LS: u32 = 1 << 26;
const TD_CERR3: u32 = 3 << 27;
const TD_SPD: u32 = 1 << 29;
const TD_ERRMASK: u32 = 0b0111_1110 << 16; // bits 17..22

// PID
const PID_SETUP: u32 = 0x2d;
const PID_IN: u32 = 0x69;
const PID_OUT: u32 = 0xe1;

static mut FL: FrameList = FrameList([0; 1024]);
static mut QH_MAIN: Qh = Qh { head: LINK_T, elem: LINK_T };
static mut TD_POOL: [Td; 4] = [Td { link: 0, cs: 0, token: 0, buf: 0 }; 4];
static mut INT_TD: Td = Td { link: 0, cs: 0, token: 0, buf: 0 };
static mut SETUP_BUF: [u8; 8] = [0; 8];
static mut DATA_BUF: [u8; 64] = [0; 64];
static mut REPORT: [u8; 8] = [0; 8];

static mut IOBASE: u16 = 0;
static mut READY: bool = false;
static mut TOGGLE: u32 = 0;
static mut DEV_ADDR: u32 = 0;

// état pointeur exposé
static mut PX: i32 = 0;
static mut PY: i32 = 0;
static mut BTN_L: bool = false;
static mut BTN_R: bool = false;
static mut WHEEL: i32 = 0;
static mut MOVED: bool = false;

fn phys(p: *const u8) -> u32 {
    p as usize as u32
}

fn r16(off: u16) -> u16 {
    unsafe { inw(IOBASE + off) }
}
fn w16(off: u16, v: u16) {
    unsafe { outw(IOBASE + off, v) }
}
fn w32(off: u16, v: u32) {
    unsafe { outl(IOBASE + off, v) }
}

fn sleep(sec: f32) {
    let t0 = time::now_secs();
    while time::now_secs() - t0 < sec {
        core::hint::spin_loop();
    }
}

pub fn present() -> bool {
    unsafe { READY }
}

/// Cherche le contrôleur UHCI, l'initialise, énumère la tablette.
/// `false` si absent → on garde la souris PS/2.
pub fn init() -> bool {
    let dev = match pci::find(0x8086, 0x7020) {
        Some(d) => d,
        None => {
            crate::serial_println!("[usb] pas de contrôleur UHCI (piix3) — souris PS/2");
            return false;
        }
    };
    dev.enable_bus_master();
    let bar4 = dev.bar(4) & 0xffff_fffc;
    if bar4 == 0 {
        crate::serial_println!("[usb] BAR4 nul");
        return false;
    }
    unsafe {
        IOBASE = bar4 as u16;
    }

    // désactive les IRQ hérités PIIX3 (registre PCI 0xC0 = LEGSUP) et
    // repasse la main au pilote OS.
    dev.write32(0xc0, 0x8f00);

    // reset global puis reset du contrôleur
    w16(USBCMD, CMD_GRESET);
    sleep(0.06);
    w16(USBCMD, 0);
    sleep(0.01);
    w16(USBCMD, CMD_HCRESET);
    let t0 = time::now_secs();
    while r16(USBCMD) & CMD_HCRESET != 0 && time::now_secs() - t0 < 0.5 {
        core::hint::spin_loop();
    }

    unsafe {
        // toutes les frames pointent sur notre QH principale
        let qh = phys(&raw const QH_MAIN as *const u8);
        for e in FL.0.iter_mut() {
            *e = qh | LINK_Q;
        }
        QH_MAIN.head = LINK_T;
        QH_MAIN.elem = LINK_T;

        w16(USBINTR, 0); // pas d'interruptions
        w16(FRNUM, 0);
        w32(FRBASEADD, phys(&raw const FL as *const u8));
        w16(SOFMOD, 0x40);
        w16(USBSTS, 0x3f); // efface le statut
        w16(USBCMD, CMD_RS | CMD_CF | CMD_MAXP);
    }

    sleep(0.05);

    // --- reset + activation du port où est la tablette ---
    let mut port = None;
    for p in 0..2u16 {
        let sc = r16(PORTSC1 + p * 2);
        if sc & PORT_CCS != 0 {
            port = Some(p);
            break;
        }
    }
    let p = match port {
        Some(p) => p,
        None => {
            crate::serial_println!("[usb] aucun périphérique sur les ports UHCI");
            return false;
        }
    };
    let reg = PORTSC1 + p * 2;
    w16(reg, PORT_RESET);
    sleep(0.06);
    w16(reg, r16(reg) & !PORT_RESET);
    sleep(0.005);
    w16(reg, PORT_PE); // active (efface aussi CSC/PEC en écrivant 0 dessus… on repasse après)
    sleep(0.02);
    // acquitte les bits de changement
    w16(reg, r16(reg) | PORT_CSC | PORT_PEC);
    sleep(0.02);
    let sc = r16(reg);
    if sc & PORT_PE == 0 {
        crate::serial_println!("[usb] port {} non activé ({:#x})", p, sc);
        return false;
    }
    let low_speed = sc & PORT_LS != 0;
    crate::serial_println!("[usb] tablette sur port {} (ls={})", p, low_speed);

    // --- énumération ---
    // SET_ADDRESS(1)
    if !control_out(0, &[0x00, 0x05, 1, 0, 0, 0, 0, 0], low_speed) {
        crate::serial_println!("[usb] SET_ADDRESS a échoué");
        return false;
    }
    sleep(0.005);
    unsafe {
        DEV_ADDR = 1;
    }
    // SET_CONFIGURATION(1)
    if !control_out(1, &[0x00, 0x09, 1, 0, 0, 0, 0, 0], low_speed) {
        crate::serial_println!("[usb] SET_CONFIGURATION a échoué");
        return false;
    }
    // SET_IDLE(0) sur l'interface 0 (classe HID) — best effort
    let _ = control_out(1, &[0x21, 0x0a, 0, 0, 0, 0, 0, 0], low_speed);

    // --- arme la lecture périodique de l'endpoint interrupt (ep1 IN) ---
    unsafe {
        TOGGLE = 0;
        arm_int(low_speed);
        QH_MAIN.elem = phys(&raw const INT_TD as *const u8);
        PX = (fb_w() / 2) as i32;
        PY = (fb_h() / 2) as i32;
        READY = true;
    }
    crate::serial_println!("[usb] tablette prête — pointeur absolu actif");
    true
}

fn fb_w() -> usize {
    crate::fb::WIDTH
}
fn fb_h() -> usize {
    crate::fb::HEIGHT
}

/// Transfert de contrôle SANS étape de données (SETUP + STATUS-IN).
fn control_out(addr: u32, setup: &[u8; 8], ls: bool) -> bool {
    unsafe {
        SETUP_BUF = *setup;
        let lsf = if ls { TD_LS } else { 0 };

        // TD 0 : SETUP
        TD_POOL[0].link = phys(&raw const TD_POOL[1] as *const u8);
        TD_POOL[0].cs = TD_ACTIVE | TD_CERR3 | lsf;
        TD_POOL[0].token = PID_SETUP | (addr << 8) | (0 << 15) | (0 << 19) | ((8u32 - 1) << 21);
        TD_POOL[0].buf = phys(&raw const SETUP_BUF as *const u8);

        // TD 1 : STATUS IN (toggle = 1, longueur 0)
        TD_POOL[1].link = LINK_T;
        TD_POOL[1].cs = TD_ACTIVE | TD_CERR3 | lsf | TD_IOC;
        TD_POOL[1].token = PID_IN | (addr << 8) | (0 << 15) | (1 << 19) | (0x7ff << 21);
        TD_POOL[1].buf = 0;

        QH_MAIN.elem = phys(&raw const TD_POOL[0] as *const u8);

        // attend la fin (les 2 TD inactifs) ou un timeout
        let t0 = time::now_secs();
        loop {
            let a0 = core::ptr::read_volatile(&TD_POOL[0].cs) & TD_ACTIVE;
            let a1 = core::ptr::read_volatile(&TD_POOL[1].cs) & TD_ACTIVE;
            if a0 == 0 && a1 == 0 {
                break;
            }
            if time::now_secs() - t0 > 0.2 {
                QH_MAIN.elem = LINK_T;
                return false;
            }
            core::hint::spin_loop();
        }
        QH_MAIN.elem = LINK_T;
        let err = (core::ptr::read_volatile(&TD_POOL[0].cs)
            | core::ptr::read_volatile(&TD_POOL[1].cs))
            & TD_ERRMASK;
        err == 0
    }
}

fn arm_int(ls: bool) {
    unsafe {
        let lsf = if ls { TD_LS } else { 0 };
        INT_TD.link = LINK_T;
        INT_TD.cs = TD_ACTIVE | TD_CERR3 | TD_SPD | lsf;
        INT_TD.token = PID_IN
            | (DEV_ADDR << 8)
            | (1 << 15) // endpoint 1
            | (TOGGLE << 19)
            | ((8u32 - 1) << 21);
        INT_TD.buf = phys(&raw const REPORT as *const u8);
    }
}

/// À appeler souvent. Relit le rapport HID s'il est arrivé.
pub fn poll() {
    unsafe {
        if !READY {
            return;
        }
        let cs = core::ptr::read_volatile(&INT_TD.cs);
        if cs & TD_ACTIVE != 0 {
            return; // pas de nouveau rapport
        }
        if cs & TD_ERRMASK == 0 {
            let alen = ((cs & 0x7ff) + 1) & 0x7ff;
            if alen >= 5 {
                let b = &REPORT;
                let btn = b[0];
                let rx = (b[1] as u32 | ((b[2] as u32) << 8)) as i32;
                let ry = (b[3] as u32 | ((b[4] as u32) << 8)) as i32;
                let wh = b[5] as i8 as i32;
                BTN_L = btn & 1 != 0;
                BTN_R = btn & 2 != 0;
                WHEEL += -wh; // molette vers le haut = positif
                let nx = rx * (fb_w() as i32) / 0x8000;
                let ny = ry * (fb_h() as i32) / 0x8000;
                PX = nx.clamp(0, fb_w() as i32 - 1);
                PY = ny.clamp(0, fb_h() as i32 - 1);
                MOVED = true;
            }
            TOGGLE ^= 1;
        }
        // ré-arme
        arm_int(INT_TD.cs & TD_LS != 0);
        QH_MAIN.elem = phys(&raw const INT_TD as *const u8);
    }
}

pub struct Ptr {
    pub x: i32,
    pub y: i32,
    pub left: bool,
    pub right: bool,
}

pub fn state() -> Ptr {
    unsafe {
        Ptr {
            x: PX,
            y: PY,
            left: BTN_L,
            right: BTN_R,
        }
    }
}

/// Molette accumulée depuis le dernier appel.
pub fn take_scroll() -> i32 {
    unsafe {
        let v = WHEEL;
        WHEEL = 0;
        v
    }
}
