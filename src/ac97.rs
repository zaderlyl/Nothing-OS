//! Pilote **AC'97** (codec audio émulé par QEMU : `-device AC97`).
//!
//! Sortie PCM stéréo 16 bits uniquement, en *polling* (pas d'IRQ, comme
//! le reste du noyau). On tient une liste de 32 tampons (BDL) qu'on
//! réalimente au fil de la lecture depuis une source décodée en RAM
//! (`SRC`), avec ré-échantillonnage pour gérer la vitesse (x1 / x1.5 …).

#![allow(dead_code, static_mut_refs)]

use alloc::vec::Vec;

use crate::pci;
use crate::port::{inb, inw, outb, outl, outw};

const VENDOR: u16 = 0x8086;
const DEVICE: u16 = 0x2415; // 82801AA AC'97

// --- registres NABM (BAR1), boîte "PCM Out" à l'offset 0x10 ---
const PO_BDBAR: u16 = 0x10; // u32 : adresse de la BDL
const PO_CIV: u16 = 0x14; // u8  : index courant (RO)
const PO_LVI: u16 = 0x15; // u8  : dernier index valide
const PO_SR: u16 = 0x16; // u16 : statut
const PO_PICB: u16 = 0x18; // u16 : échantillons restants (RO)
const PO_CR: u16 = 0x1b; // u8  : contrôle
const GLOB_CNT: u16 = 0x2c; // u32 : contrôle global

const CR_RPBM: u8 = 0x01; // run / pause bus master
const CR_RR: u8 = 0x02; // reset registres
const SR_DCH: u16 = 0x01; // DMA arrêté

// --- registres NAM (BAR0), mixer ---
const NAM_RESET: u16 = 0x00;
const NAM_MASTER: u16 = 0x02;
const NAM_AUX_OUT: u16 = 0x04;
const NAM_PCM_OUT: u16 = 0x18;
const NAM_EXT_ID: u16 = 0x28;
const NAM_EXT_CTL: u16 = 0x2a;
const NAM_DAC_RATE: u16 = 0x2c;

const NBUF: usize = 32;
const BUF_SAMPLES: usize = 2048; // i16 par tampon (1024 trames stéréo)

#[repr(C, align(8))]
#[derive(Clone, Copy)]
struct Bd {
    addr: u32,
    ctl: u32, // bits 0-15 : nb d'échantillons ; bit 31 : IOC
}

#[repr(C, align(8))]
struct Bdl([Bd; NBUF]);

#[repr(C, align(4))]
struct Pcm([i16; NBUF * BUF_SAMPLES]);

static mut BDL: Bdl = Bdl([Bd { addr: 0, ctl: 0 }; NBUF]);
static mut PCM: Pcm = Pcm([0; NBUF * BUF_SAMPLES]);

static mut NAM: u16 = 0;
static mut NABM: u16 = 0;
static mut READY: bool = false;
static mut DEV_RATE: u32 = 48000;

static mut SRC: Vec<i16> = Vec::new(); // stéréo entrelacé
static mut SRC_RATE: u32 = 44100;
static mut POS: f64 = 0.0; // position en trames stéréo dans SRC
static mut SPEED: f32 = 1.0;
static mut PLAYING: bool = false;
static mut LAST_CIV: u8 = 0;

fn r8(o: u16) -> u8 {
    unsafe { inb(NABM + o) }
}
fn w8(o: u16, v: u8) {
    unsafe { outb(NABM + o, v) }
}
fn r16(o: u16) -> u16 {
    unsafe { inw(NABM + o) }
}
fn w16(o: u16, v: u16) {
    unsafe { outw(NABM + o, v) }
}
fn w32(o: u16, v: u32) {
    unsafe { outl(NABM + o, v) }
}
fn mix_w(o: u16, v: u16) {
    unsafe { outw(NAM + o, v) }
}
fn mix_r(o: u16) -> u16 {
    unsafe { inw(NAM + o) }
}

fn pcm_ptr(i: usize) -> u32 {
    unsafe { (&raw const PCM.0 as u32).wrapping_add((i * BUF_SAMPLES * 2) as u32) }
}

fn spin(n: u32) {
    for _ in 0..n {
        core::hint::spin_loop();
    }
}

pub fn present() -> bool {
    unsafe { READY }
}

pub fn init() -> bool {
    let dev = match pci::find(VENDOR, DEVICE) {
        Some(d) => d,
        None => {
            crate::serial_println!("[ac97] pas de carte AC97");
            return false;
        }
    };
    dev.enable_bus_master();
    unsafe {
        NAM = (dev.bar(0) & 0xffff_fffc) as u16;
        NABM = (dev.bar(1) & 0xffff_fffc) as u16;
    }

    w32(GLOB_CNT, 0x02); // sortie du cold reset
    spin(200_000);

    mix_w(NAM_RESET, 1);
    spin(50_000);
    mix_w(NAM_MASTER, 0x0000); // volume à fond (0 = aucune atténuation)
    mix_w(NAM_AUX_OUT, 0x0000);
    mix_w(NAM_PCM_OUT, 0x0000);

    // débit variable si dispo, sinon on garde le débit fixe du codec
    if mix_r(NAM_EXT_ID) & 1 != 0 {
        mix_w(NAM_EXT_CTL, mix_r(NAM_EXT_CTL) | 1);
        mix_w(NAM_DAC_RATE, 44100);
    }
    let rate = mix_r(NAM_DAC_RATE) as u32;
    unsafe {
        DEV_RATE = if rate < 8000 { 48000 } else { rate };
    }

    // reset de la boîte PCM-Out
    w8(PO_CR, CR_RR);
    for _ in 0..100_000 {
        if r8(PO_CR) & CR_RR == 0 {
            break;
        }
    }

    unsafe {
        for i in 0..NBUF {
            BDL.0[i] = Bd {
                addr: pcm_ptr(i),
                ctl: (BUF_SAMPLES as u32) | (1 << 31),
            };
        }
        PCM.0 = [0; NBUF * BUF_SAMPLES];
        w32(PO_BDBAR, &raw const BDL as u32);
        READY = true;
        PLAYING = false;
    }

    crate::serial_println!(
        "[ac97] OK — nam {:#x} nabm {:#x} debit {} Hz",
        unsafe { NAM },
        unsafe { NABM },
        unsafe { DEV_RATE }
    );
    true
}

/// Charge une source PCM stéréo entrelacée (16 bits) à `rate` Hz. En pause.
pub fn load(pcm: Vec<i16>, rate: u32) {
    unsafe {
        SRC = pcm;
        SRC_RATE = rate.max(8000);
        POS = 0.0;
        SPEED = 1.0;
        PLAYING = false;
    }
}

pub fn loaded() -> bool {
    unsafe { !SRC.is_empty() }
}
pub fn playing() -> bool {
    unsafe { PLAYING }
}
pub fn speed() -> f32 {
    unsafe { SPEED }
}

pub fn set_speed(s: f32) {
    unsafe {
        SPEED = s.clamp(0.25, 4.0);
    }
}

/// Position de lecture, 0.0 → 1.0.
pub fn progress() -> f32 {
    unsafe {
        let total = (SRC.len() / 2) as f64;
        if total <= 0.0 {
            0.0
        } else {
            (POS / total).clamp(0.0, 1.0) as f32
        }
    }
}

/// Durée totale en secondes.
pub fn duration() -> f32 {
    unsafe { (SRC.len() / 2) as f32 / SRC_RATE as f32 }
}

pub fn play() {
    unsafe {
        if !READY || SRC.is_empty() {
            return;
        }
        if POS as usize * 2 >= SRC.len() {
            POS = 0.0;
        }
        // pré-remplissage complet
        for i in 0..NBUF {
            fill_buffer(i);
        }
        LAST_CIV = r8(PO_CIV);
        w8(PO_LVI, (NBUF - 1) as u8);
        w16(PO_SR, 0x1c);
        w8(PO_CR, CR_RPBM);
        PLAYING = true;
    }
}

pub fn pause() {
    unsafe {
        PLAYING = false;
        w8(PO_CR, r8(PO_CR) & !CR_RPBM);
    }
}

pub fn toggle() {
    if playing() {
        pause();
    } else {
        play();
    }
}

pub fn stop() {
    unsafe {
        pause();
        SRC = Vec::new();
        POS = 0.0;
    }
}

pub fn seek(frac: f32) {
    unsafe {
        let total = (SRC.len() / 2) as f64;
        POS = (frac.clamp(0.0, 1.0) as f64 * total).max(0.0);
    }
}

/// À appeler à chaque image : réalimente les tampons consommés.
pub fn poll() {
    unsafe {
        if !READY || !PLAYING {
            return;
        }
        let civ = r8(PO_CIV);
        let mut i = LAST_CIV;
        while i != civ {
            fill_buffer(i as usize);
            i = ((i as usize + 1) % NBUF) as u8;
        }
        LAST_CIV = civ;
        w8(PO_LVI, ((civ as usize + NBUF - 2) % NBUF) as u8);

        if r16(PO_SR) & SR_DCH != 0 {
            // sous-alimentation : on relance si on a encore du son
            w16(PO_SR, 0x1c);
            if POS as usize * 2 < SRC.len() {
                w8(PO_CR, r8(PO_CR) | CR_RPBM);
            } else {
                PLAYING = false;
            }
        }
    }
}

/// Remplit le tampon BDL `idx` en ré-échantillonnant `SRC` à la vitesse
/// courante vers le débit du codec.
unsafe fn fill_buffer(idx: usize) {
    let dst = &mut PCM.0[idx * BUF_SAMPLES..idx * BUF_SAMPLES + BUF_SAMPLES];
    let frames = BUF_SAMPLES / 2;
    let n = SRC.len();
    let step = SPEED as f64 * SRC_RATE as f64 / DEV_RATE as f64;

    for f in 0..frames {
        let p = POS;
        let i = p as usize;
        if i * 2 + 1 >= n {
            for s in &mut dst[f * 2..] {
                *s = 0;
            }
            if PLAYING {
                PLAYING = false;
            }
            return;
        }
        let frac = (p - i as f64) as f32;
        let j = if (i + 1) * 2 + 1 < n { i + 1 } else { i };
        let l = lerp(SRC[i * 2], SRC[j * 2], frac);
        let r = lerp(SRC[i * 2 + 1], SRC[j * 2 + 1], frac);
        dst[f * 2] = l;
        dst[f * 2 + 1] = r;
        POS += step;
    }
}

fn lerp(a: i16, b: i16, t: f32) -> i16 {
    (a as f32 + (b as f32 - a as f32) * t) as i16
}

/// Sinusoïde de test : `secs` secondes à `freq` Hz (auto-test au boot / `snd`).
pub fn test_tone(freq: f32, secs: f32) {
    let rate = 44100u32;
    let n = (rate as f32 * secs) as usize;
    let mut v = Vec::with_capacity(n * 2);
    for k in 0..n {
        let t = k as f32 / rate as f32;
        let s = (libm::sinf(t * freq * core::f32::consts::TAU) * 9000.0) as i16;
        v.push(s);
        v.push(s);
    }
    load(v, rate);
    play();
}
