//! Vue « bureau distant » : lit les trames écrites par le pont Mac
//! (`bridge/discord-bridge`) dans `.nothingos-bridge/frame.bin` sur le
//! partage 9p, et renvoie les événements souris/clavier dans
//! `.nothingos-bridge/input.bin`.
//!
//! Permet d'afficher et de piloter la vraie fenêtre Discord du Mac
//! depuis Nothing OS.

#![allow(dead_code, static_mut_refs)]

use alloc::vec::Vec;

use crate::{fb, p9};

const FRAME_PATH: &str = ".nothingos-bridge/frame.bin";
const INPUT_PATH: &str = ".nothingos-bridge/input.bin";
const TILE: usize = 32;

static mut BUF: Vec<u8> = Vec::new(); // FW*FH, indices de palette (76..255)
static mut FW: usize = 0;
static mut FH: usize = 0;
static mut SEQ: u32 = 0;
static mut SEEN_AT: f32 = -100.0; // dernier instant où une trame a été reçue
static mut NOW: f32 = 0.0;
static mut ISEQ: u32 = 0;
static mut EVQ: Vec<u8> = Vec::new();
static mut EVN: u16 = 0;
static mut LAST_MX: i32 = -999;
static mut LAST_MY: i32 = -999;
static mut GOT_FULL: bool = false;
static mut ASK_AT: f32 = -100.0;
static mut DIAG: u32 = 0;
static mut ASKS: u32 = 0;

fn u16le(d: &[u8], o: usize) -> usize {
    (d[o] as usize) | ((d[o + 1] as usize) << 8)
}
fn u32le(d: &[u8], o: usize) -> u32 {
    (d[o] as u32) | ((d[o + 1] as u32) << 8) | ((d[o + 2] as u32) << 16) | ((d[o + 3] as u32) << 24)
}

/// Le pont est-il actif ? (une trame reçue il y a moins de 2 s)
pub fn live() -> bool {
    unsafe { FW > 0 && NOW - SEEN_AT < 2.0 }
}

/// À appeler à chaque image. Lit la dernière trame, pousse les événements.
pub fn poll(now: f32) {
    unsafe {
        NOW = now;
        match p9::read_file(FRAME_PATH) {
            Some(d) if d.len() >= 15 && &d[0..4] == b"NOSF" => {
                let seq = u32le(&d, 4);
                SEEN_AT = now;
                if seq != SEQ {
                    SEQ = seq;
                    let w = u16le(&d, 8);
                    let h = u16le(&d, 10);
                    let full = d[12] != 0;
                    let nt = u16le(&d, 13);
                    if w != FW || h != FH || BUF.len() != w * h {
                        FW = w;
                        FH = h;
                        BUF = alloc::vec![0u8; w * h];
                        GOT_FULL = false;
                    }
                    if full {
                        GOT_FULL = true;
                    }
                    let mut p = 15;
                    let mut tile = [0u8; TILE * TILE];
                    for _ in 0..nt {
                        if p + 6 > d.len() {
                            break;
                        }
                        let tx = u16le(&d, p);
                        let ty = u16le(&d, p + 2);
                        let rlen = u16le(&d, p + 4);
                        p += 6;
                        if p + rlen > d.len() {
                            break;
                        }
                        // décode le RLE (count, value) → tuile 32×32
                        let mut ti = 0usize;
                        let mut q = p;
                        while q + 1 < p + rlen && ti < TILE * TILE {
                            let c = d[q] as usize;
                            let v = d[q + 1];
                            q += 2;
                            let end = (ti + c).min(TILE * TILE);
                            for s in &mut tile[ti..end] {
                                *s = v;
                            }
                            ti = end;
                        }
                        p += rlen;
                        for yy in 0..TILE {
                            let row = (ty * TILE + yy) * FW + tx * TILE;
                            if row + TILE <= BUF.len() {
                                BUF[row..row + TILE]
                                    .copy_from_slice(&tile[yy * TILE..yy * TILE + TILE]);
                            }
                        }
                    }
                    if full || DIAG < 3 {
                        DIAG += 1;
                        crate::serial_println!(
                            "[remote] trame {} {}x{} full={} tuiles={} ({} o)",
                            seq, w, h, full, nt, d.len()
                        );
                    }
                }
            }
            _ => {
                if FW > 0 && now - SEEN_AT > 3.0 && now - ASK_AT > 3.0 {
                    ASK_AT = now;
                    crate::serial_println!("[remote] plus de trame — pont Mac coupe ?");
                }
            }
        }

        // tant qu'on n'a pas de trame complète, on en redemande une
        if FW > 0 && !GOT_FULL && now - ASK_AT > 0.4 {
            ASK_AT = now;
            ASKS += 1;
            request_keyframe();
            if ASKS <= 4 {
                crate::serial_println!("[remote] demande de keyframe ({})", ASKS);
            }
        }

        if EVN > 0 {
            ISEQ = ISEQ.wrapping_add(1);
            let mut out = Vec::with_capacity(10 + EVQ.len());
            out.extend_from_slice(b"NOSI");
            out.extend_from_slice(&ISEQ.to_le_bytes());
            out.extend_from_slice(&EVN.to_le_bytes());
            out.extend_from_slice(&EVQ);
            let ok = p9::write_file(INPUT_PATH, &out);
            if ISEQ <= 3 {
                crate::serial_println!(
                    "[remote] input.bin ecrit iseq={} {} ev ({})",
                    ISEQ, EVN, if ok { "ok" } else { "ECHEC" }
                );
            }
            EVQ.clear();
            EVN = 0;
        }
    }
}

// --- géométrie d'affichage (letterbox plein écran) ---
fn layout() -> (i32, i32, f32) {
    let (w, h) = unsafe { (FW as f32, FH as f32) };
    if w <= 0.0 {
        return (0, 0, 1.0);
    }
    let sw = fb::WIDTH as f32 / w;
    let sh = fb::HEIGHT as f32 / h;
    let s = if sw < sh { sw } else { sh };
    let dw = (w * s) as i32;
    let dh = (h * s) as i32;
    ((fb::WIDTH as i32 - dw) / 2, (fb::HEIGHT as i32 - dh) / 2, s)
}

pub fn draw() {
    unsafe {
        if FW == 0 {
            return;
        }
        fb::clear(0);
        let (ox, oy, s) = layout();
        let dw = (FW as f32 * s) as i32;
        let dh = (FH as f32 * s) as i32;
        for y in 0..dh {
            let sy = ((y as f32 / s) as usize).min(FH - 1);
            let srow = sy * FW;
            let drow_y = oy + y;
            if drow_y < 0 || drow_y >= fb::HEIGHT as i32 {
                continue;
            }
            for x in 0..dw {
                let sx = ((x as f32 / s) as usize).min(FW - 1);
                fb::put(ox + x, drow_y, BUF[srow + sx]);
            }
        }
    }
}

// --- événements ---
fn push(bytes: &[u8]) {
    unsafe {
        if EVQ.len() < 4096 {
            EVQ.extend_from_slice(bytes);
            EVN += 1;
        }
    }
}

/// Convertit une position écran en position trame. `None` hors cadre.
fn to_frame(mx: i32, my: i32) -> Option<(u16, u16)> {
    let (ox, oy, s) = layout();
    let fx = ((mx - ox) as f32 / s) as i32;
    let fy = ((my - oy) as f32 / s) as i32;
    unsafe {
        if fx < 0 || fy < 0 || fx >= FW as i32 || fy >= FH as i32 {
            None
        } else {
            Some((fx as u16, fy as u16))
        }
    }
}

pub fn feed_move(mx: i32, my: i32) {
    unsafe {
        if (mx - LAST_MX).abs() + (my - LAST_MY).abs() < 3 {
            return;
        }
        LAST_MX = mx;
        LAST_MY = my;
    }
    if let Some((fx, fy)) = to_frame(mx, my) {
        push(&[b'M', fx as u8, (fx >> 8) as u8, fy as u8, (fy >> 8) as u8]);
    }
}

pub fn feed_button(mx: i32, my: i32, right: bool, down: bool) {
    if let Some((fx, fy)) = to_frame(mx, my) {
        let t = if down { b'D' } else { b'U' };
        let b = if right { 1u8 } else { 0 };
        push(&[t, b, fx as u8, (fx >> 8) as u8, fy as u8, (fy >> 8) as u8]);
    }
}

pub fn feed_wheel(mx: i32, my: i32, dy: i32) {
    if let Some((fx, fy)) = to_frame(mx, my) {
        push(&[b'W', dy as i8 as u8, fx as u8, (fx >> 8) as u8, fy as u8, (fy >> 8) as u8]);
    }
}

pub fn feed_key(c: u8, down: bool) {
    push(&[b'K', down as u8, c]);
}

/// Demande une trame complète (à l'ouverture de l'appli distante).
pub fn request_keyframe() {
    unsafe {
        GOT_FULL = false;
    }
    push(&[b'F']);
}
