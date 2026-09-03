//! Lecture de l'horloge temps réel (CMOS RTC), pour afficher l'heure.

#![allow(dead_code)]

use crate::port::{inb, outb};

fn read(reg: u8) -> u8 {
    unsafe {
        outb(0x70, reg);
        inb(0x71)
    }
}

fn updating() -> bool {
    read(0x0a) & 0x80 != 0
}

fn bcd_to_bin(v: u8) -> u8 {
    (v & 0x0f) + (v >> 4) * 10
}

pub struct Time {
    pub hour: u8,
    pub min: u8,
    pub sec: u8,
}

/// Heure courante (heure locale telle que vue par le CMOS).
pub fn now() -> Time {
    // on attend la fin d'une éventuelle mise à jour, puis on lit
    while updating() {
        core::hint::spin_loop();
    }
    let sec = read(0x00);
    let min = read(0x02);
    let hour = read(0x04);
    let status_b = read(0x0b);

    let (mut h, m, s);
    if status_b & 0x04 == 0 {
        // valeurs en BCD
        h = bcd_to_bin(hour & 0x7f);
        m = bcd_to_bin(min);
        s = bcd_to_bin(sec);
        if hour & 0x80 != 0 {
            h = (h % 12) + 12; // 12h -> 24h
        }
    } else {
        h = hour & 0x7f;
        m = min;
        s = sec;
        if hour & 0x80 != 0 {
            h = (h % 12) + 12;
        }
    }
    Time {
        hour: h,
        min: m,
        sec: s,
    }
}
