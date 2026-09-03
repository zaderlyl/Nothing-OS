//! Base de temps : on calibre le TSC (compteur d'horodatage du CPU) une
//! fois au boot, contre le canal 2 du PIT, sans avoir besoin
//! d'interruptions. Ensuite `now_secs()` donne le temps écoulé en
//! secondes (flottant).
//!
//! La calibration est bardée de garde-fous : si le PIT ne répond pas
//! comme prévu, on retombe sur une valeur par défaut plutôt que de
//! renvoyer 0 (ce qui figerait la boucle de rendu).

use core::sync::atomic::{AtomicU64, Ordering};

use crate::port::{inb, outb};

const PIT_HZ: u64 = 1_193_182;
/// Valeur de repli : QEMU tourne quasi toujours avec un TSC à 1 GHz.
const DEFAULT_HZ: u64 = 1_000_000_000;

static TSC_HZ: AtomicU64 = AtomicU64::new(DEFAULT_HZ);
static TSC_START: AtomicU64 = AtomicU64::new(0);

#[inline(always)]
fn rdtsc() -> u64 {
    unsafe { core::arch::x86_64::_rdtsc() }
}

/// Calibre le TSC et fixe l'origine des temps. À appeler une fois.
pub fn init() {
    let raw = calibrate();
    // garde-fou : entre 0,1 et 100 GHz, sinon valeur par défaut
    let hz = if raw > 100_000_000 && raw < 100_000_000_000 {
        raw
    } else {
        crate::serial_println!("[nothing-os] calibration TSC douteuse ({}), repli 1 GHz", raw);
        DEFAULT_HZ
    };
    TSC_HZ.store(hz, Ordering::Relaxed);
    TSC_START.store(rdtsc(), Ordering::Relaxed);
    crate::serial_println!("[nothing-os] TSC ~= {} MHz", hz / 1_000_000);
}

/// Graine pour un PRNG : les bits de poids faible du TSC.
pub fn seed() -> u32 {
    rdtsc() as u32
}

/// Temps écoulé depuis `init()`, en secondes. Ne renvoie jamais 0 une
/// fois `init()` passé (TSC_HZ a une valeur par défaut non nulle).
pub fn now_secs() -> f32 {
    let hz = TSC_HZ.load(Ordering::Relaxed);
    let dt = rdtsc().wrapping_sub(TSC_START.load(Ordering::Relaxed));
    dt as f32 / hz as f32
}

fn calibrate() -> u64 {
    const MS: u64 = 40;
    let count: u16 = (PIT_HZ * MS / 1000) as u16;

    unsafe {
        // Canal 2 : porte ouverte, sortie haut-parleur coupée.
        let p = inb(0x61);
        outb(0x61, (p & 0xfc) | 0x01);

        // Canal 2, accès lo/hi, mode 0 (interrupt on terminal count), binaire.
        outb(0x43, 0b1011_0000);
        outb(0x42, count as u8);
        outb(0x42, (count >> 8) as u8);

        let start = rdtsc();
        // OUT (bit 5 de 0x61) passe à 1 au décompte terminal. Borné par
        // un garde-fou : ~200 M itérations ≈ bien plus que 40 ms.
        let mut guard: u64 = 0;
        while inb(0x61) & 0x20 == 0 && guard < 200_000_000 {
            core::hint::spin_loop();
            guard += 1;
        }
        let end = rdtsc();

        if guard >= 200_000_000 {
            return 0; // le PIT n'a pas répondu → l'appelant prendra le repli
        }
        end.wrapping_sub(start) * 1000 / MS
    }
}
