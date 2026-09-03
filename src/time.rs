//! Base de temps : on calibre le TSC (compteur d'horodatage du CPU) une
//! fois au boot, contre le canal 2 du PIT, sans avoir besoin
//! d'interruptions. Ensuite `now_secs()` donne le temps écoulé en
//! secondes (flottant), ce dont le rendu d'Asti a besoin.

use core::sync::atomic::{AtomicU64, Ordering};

use crate::port::{inb, outb};

const PIT_HZ: u64 = 1_193_182;

static TSC_HZ: AtomicU64 = AtomicU64::new(0);
static TSC_START: AtomicU64 = AtomicU64::new(0);

#[inline(always)]
fn rdtsc() -> u64 {
    unsafe { core::arch::x86_64::_rdtsc() }
}

/// Calibre le TSC et fixe l'origine des temps. À appeler une fois.
pub fn init() {
    let hz = calibrate();
    TSC_HZ.store(hz, Ordering::Relaxed);
    TSC_START.store(rdtsc(), Ordering::Relaxed);
    crate::serial_println!("[nothing-os] TSC ~= {} MHz", hz / 1_000_000);
}

/// Graine pour un PRNG : les bits de poids faible du TSC.
pub fn seed() -> u32 {
    rdtsc() as u32
}

/// Temps écoulé depuis `init()`, en secondes.
pub fn now_secs() -> f32 {
    let hz = TSC_HZ.load(Ordering::Relaxed);
    if hz == 0 {
        return 0.0;
    }
    let dt = rdtsc().wrapping_sub(TSC_START.load(Ordering::Relaxed));
    dt as f32 / hz as f32
}

/// Attend (spin) jusqu'à ce que `secs` se soient écoulés depuis `from`
/// (une valeur de `now_secs()`). Sert à cadencer la boucle de rendu.
pub fn spin_until(from: f32, secs: f32) {
    while now_secs() - from < secs {
        core::hint::spin_loop();
    }
}

fn calibrate() -> u64 {
    // ~50 ms mesurés sur le canal 2 du PIT.
    const MS: u64 = 50;
    let count: u16 = (PIT_HZ * MS / 1000) as u16;

    unsafe {
        // Canal 2 : porte (gate) ouverte, haut-parleur coupé.
        let p = inb(0x61);
        outb(0x61, (p & 0xfc) | 0x01);

        // Canal 2, accès lo/hi, mode 0 (one-shot), binaire.
        outb(0x43, 0b1011_0000);
        outb(0x42, count as u8);
        outb(0x42, (count >> 8) as u8);

        let start = rdtsc();
        // En mode 0, OUT (bit 5 de 0x61) passe à 1 quand le décompte atteint 0.
        while inb(0x61) & 0x20 == 0 {
            core::hint::spin_loop();
        }
        let end = rdtsc();

        end.wrapping_sub(start) * 1000 / MS
    }
}
