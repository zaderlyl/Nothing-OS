//! Décodage MP3 → PCM stéréo `i16`, via **minimp3** (crate `rmp3`, code C
//! compilé en cross-target — voir le Makefile et `cshim/`).

#![allow(dead_code)]

use alloc::vec::Vec;

use rmp3::{Decoder, Frame};

/// `Some((échantillons stéréo entrelacés, débit Hz))`.
pub fn decode(data: &[u8]) -> Option<(Vec<i16>, u32)> {
    let mut dec = Decoder::new(data);
    let mut out: Vec<i16> = Vec::new();
    let mut rate = 0u32;

    while let Some(frame) = dec.next() {
        if let Frame::Audio(a) = frame {
            rate = a.sample_rate();
            let ch = a.channels() as usize;
            let s = a.samples();
            match ch {
                2 => out.extend_from_slice(s),
                1 => {
                    for &x in s {
                        out.push(x);
                        out.push(x);
                    }
                }
                _ => {}
            }
        }
    }

    if rate == 0 || out.is_empty() {
        None
    } else {
        Some((out, rate))
    }
}
