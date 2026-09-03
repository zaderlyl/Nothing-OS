//! Décodage de fichiers **WAV** (RIFF/PCM) vers des échantillons `i16`
//! stéréo entrelacés. Gère PCM 8/16/24/32 bits entiers et float 32.

#![allow(dead_code)]

use alloc::vec::Vec;

pub struct Pcm {
    pub samples: Vec<i16>, // stéréo entrelacé
    pub rate: u32,
}

fn le16(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
fn le32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

pub fn is_wav(data: &[u8]) -> bool {
    data.len() >= 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"WAVE"
}

pub fn decode(data: &[u8]) -> Option<Pcm> {
    if !is_wav(data) {
        return None;
    }

    let mut fmt_tag = 0u16;
    let mut channels = 0u16;
    let mut rate = 0u32;
    let mut bits = 0u16;
    let mut body: Option<&[u8]> = None;

    let mut p = 12usize;
    while p + 8 <= data.len() {
        let id = &data[p..p + 4];
        let sz = le32(data, p + 4) as usize;
        let start = p + 8;
        let end = start.saturating_add(sz).min(data.len());
        match id {
            b"fmt " if end - start >= 16 => {
                fmt_tag = le16(data, start);
                channels = le16(data, start + 2);
                rate = le32(data, start + 4);
                bits = le16(data, start + 14);
            }
            b"data" => body = Some(&data[start..end]),
            _ => {}
        }
        p = start + sz + (sz & 1);
    }

    let body = body?;
    if channels == 0 || rate == 0 || bits == 0 {
        return None;
    }
    let ch = channels as usize;
    let bytes = (bits / 8) as usize;
    if bytes == 0 {
        return None;
    }
    let frame = bytes * ch;
    let frames = body.len() / frame;

    let sample = |o: usize| -> i16 {
        match (fmt_tag, bits) {
            (_, 8) => ((body[o] as i16) - 128) << 8, // WAV 8 bits = non signé
            (_, 16) => i16::from_le_bytes([body[o], body[o + 1]]),
            (_, 24) => {
                let v = (body[o] as i32) << 8 | (body[o + 1] as i32) << 16 | (body[o + 2] as i32) << 24;
                (v >> 16) as i16
            }
            (3, 32) => {
                let f = f32::from_le_bytes([body[o], body[o + 1], body[o + 2], body[o + 3]]);
                (f.clamp(-1.0, 1.0) * 32767.0) as i16
            }
            (_, 32) => {
                let v = i32::from_le_bytes([body[o], body[o + 1], body[o + 2], body[o + 3]]);
                (v >> 16) as i16
            }
            _ => 0,
        }
    };

    let mut out = Vec::with_capacity(frames * 2);
    for i in 0..frames {
        let base = i * frame;
        let l = sample(base);
        let r = if ch >= 2 { sample(base + bytes) } else { l };
        out.push(l);
        out.push(r);
    }

    Some(Pcm { samples: out, rate })
}
