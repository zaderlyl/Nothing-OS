//! Décodage d'images (PNG, JPEG) pour l'aperçu dans `docview`.
//!
//! Les crates `zune-png` / `zune-jpeg` (pur Rust, `no_std` + `alloc`)
//! décodent en RGB/RGBA ; on réduit tout de suite à la taille du panneau
//! et on **quantifie** vers un cube de couleurs occupant les entrées de
//! palette 76..=255 (les autres sont prises par Asti / le bureau / les
//! fenêtres).

#![allow(dead_code)]

use alloc::vec::Vec;

use zune_core::colorspace::ColorSpace;
use zune_core::options::DecoderOptions;

use crate::fb;

#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    Png,
    Jpeg,
}

/// `Some(kind)` si l'extension du nom est une image gérée.
pub fn kind_of(name: &str) -> Option<Kind> {
    let ext = name.rsplit('.').next()?;
    let is = |t: &[u8]| {
        ext.len() == t.len()
            && ext
                .bytes()
                .zip(t.iter())
                .all(|(c, &d)| c.to_ascii_lowercase() == d)
    };
    if is(b"png") {
        Some(Kind::Png)
    } else if is(b"jpg") || is(b"jpeg") || is(b"jpe") {
        Some(Kind::Jpeg)
    } else {
        None
    }
}

// --- cube de couleurs 6×6×5 sur les indices 76..=255 -------------------

const BASE: u8 = 76;

/// À appeler une fois au boot (après `fb::init`).
pub fn install_cube() {
    for r in 0..6u32 {
        for g in 0..6u32 {
            for b in 0..5u32 {
                let idx = BASE as u32 + r * 30 + g * 5 + b;
                fb::set_palette(
                    idx as u8,
                    (r * 255 / 5) as u8,
                    (g * 255 / 5) as u8,
                    (b * 255 / 4) as u8,
                );
            }
        }
    }
}

const BAYER: [[i32; 4]; 4] = [
    [0, 8, 2, 10],
    [12, 4, 14, 6],
    [3, 11, 1, 9],
    [15, 7, 13, 5],
];

fn quant(r: i32, g: i32, b: i32, x: i32, y: i32) -> u8 {
    let d = (BAYER[(y & 3) as usize][(x & 3) as usize] - 8) * 6; // tramage ordonné
    let c = |v: i32| (v + d).clamp(0, 255);
    let ri = (c(r) * 6 / 256).min(5);
    let gi = (c(g) * 6 / 256).min(5);
    let bi = (c(b) * 5 / 256).min(4);
    (BASE as i32 + ri * 30 + gi * 5 + bi) as u8
}

/// Image réduite, prête à être blittée (indices de palette).
pub struct Bitmap {
    pub w: i32,
    pub h: i32,
    pub px: Vec<u8>,
}

/// Décode `bytes` et réduit pour tenir dans `max_w × max_h` (jamais
/// d'agrandissement). `Err` avec un message court en cas d'échec.
pub fn decode_fit(
    bytes: &[u8],
    kind: Kind,
    max_w: i32,
    max_h: i32,
) -> Result<Bitmap, &'static str> {
    const MAXPX: usize = 24_000_000; // ~6000×4000 : couvre les photos de tel.
    let opts = DecoderOptions::default()
        .png_set_strip_to_8bit(true)
        .set_max_width(8000)
        .set_max_height(8000);

    let (sw, sh, ch, data): (usize, usize, usize, Vec<u8>) = match kind {
        Kind::Png => {
            let mut d = zune_png::PngDecoder::new_with_options(bytes, opts);
            d.decode_headers().map_err(|_| "PNG illisible")?;
            let (w, h) = d.get_dimensions().ok_or("PNG sans dimensions")?;
            if w == 0 || h == 0 || w.saturating_mul(h) > MAXPX {
                return Err("image trop grande pour l'apercu");
            }
            let cs = d.get_colorspace().unwrap_or(ColorSpace::RGB);
            let raw = d.decode_raw().map_err(|_| "PNG : decodage impossible")?;
            (w, h, cs.num_components().max(1), raw)
        }
        Kind::Jpeg => {
            let mut d = zune_jpeg::JpegDecoder::new_with_options(bytes, opts);
            d.decode_headers().map_err(|_| "JPEG illisible")?;
            let info = d.info().ok_or("JPEG sans infos")?;
            let (w, h) = (info.width as usize, info.height as usize);
            if w == 0 || h == 0 || w.saturating_mul(h) > MAXPX {
                return Err("image trop grande pour l'apercu");
            }
            let raw = d.decode().map_err(|_| "JPEG : decodage impossible")?;
            let cs = d.get_output_colorspace().unwrap_or(ColorSpace::RGB);
            (w, h, cs.num_components().max(1), raw)
        }
    };

    if data.len() < sw.saturating_mul(sh).saturating_mul(ch) {
        return Err("donnees image incompletes");
    }

    let mut dw = sw as i32;
    let mut dh = sh as i32;
    if dw > max_w {
        dh = (dh * max_w / dw).max(1);
        dw = max_w;
    }
    if dh > max_h {
        dw = (dw * max_h / dh).max(1);
        dh = max_h;
    }

    let mut px = Vec::with_capacity((dw * dh) as usize);
    for y in 0..dh {
        let sy = (y as usize * sh / dh as usize).min(sh - 1);
        for x in 0..dw {
            let sx = (x as usize * sw / dw as usize).min(sw - 1);
            let o = (sy * sw + sx) * ch;
            let (r, g, b) = if ch >= 3 {
                (data[o] as i32, data[o + 1] as i32, data[o + 2] as i32)
            } else {
                let v = data[o] as i32;
                (v, v, v)
            };
            px.push(quant(r, g, b, x, y));
        }
    }

    Ok(Bitmap { w: dw, h: dh, px })
}
