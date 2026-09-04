//! Widget « agenda » (coin bas-gauche) : prochains évènements du flux
//! iCal, publiés par `bridge/calendar.sh` dans `.nothingos-cal`.

#![allow(static_mut_refs, dead_code)]

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::{fb, font, p9};

const PATH: &str = ".nothingos-cal";

const BG: u8 = 60;
const LINE: u8 = 61;
const TXT: u8 = 62;
const DIM: u8 = 63;

pub fn install_palette() {
    fb::set_palette(BG, 18, 19, 26);
    fb::set_palette(LINE, 48, 51, 64);
    fb::set_palette(TXT, 222, 227, 240);
    fb::set_palette(DIM, 120, 126, 145);
    // l'accent réutilise l'index 13 (PAL_ACCENT du bureau) — bleu
}
const ACCENT: u8 = 13;

struct Ev {
    start: i64,
    end: i64,
    when: String,
    summ: String,
    loc: String,
}

static mut EVS: Vec<Ev> = Vec::new();
static mut MAC_NOW: i64 = 0;
static mut READ_AT: f32 = -1.0;
static mut LAST: f32 = -100.0;
static mut GOT: bool = false;

pub fn available() -> bool {
    unsafe { GOT && !EVS.is_empty() }
}

pub fn poll(now: f32) {
    unsafe {
        if now - LAST < 5.0 {
            return;
        }
        LAST = now;
        let d = match p9::read_file(PATH) {
            Some(d) => d,
            None => return,
        };
        let text = match core::str::from_utf8(&d) {
            Ok(t) => t,
            Err(_) => return,
        };
        let mut evs: Vec<Ev> = Vec::new();
        for line in text.lines() {
            if let Some(v) = line.strip_prefix("now=") {
                MAC_NOW = v.trim().parse().unwrap_or(0);
                READ_AT = now;
                continue;
            }
            let mut it = line.splitn(5, '|');
            let s: i64 = match it.next().and_then(|x| x.parse().ok()) {
                Some(v) => v,
                None => continue,
            };
            let e: i64 = it.next().and_then(|x| x.parse().ok()).unwrap_or(s);
            let when = it.next().unwrap_or("").to_string();
            let summ = it.next().unwrap_or("").to_string();
            let loc = it.next().unwrap_or("").to_string();
            evs.push(Ev { start: s, end: e, when, summ, loc });
        }
        if !evs.is_empty() || text.starts_with("now=") {
            EVS = evs;
            GOT = true;
        }
    }
}

fn epoch_now(scene_now: f32) -> i64 {
    unsafe {
        if MAC_NOW == 0 {
            return 0;
        }
        MAC_NOW + (scene_now - READ_AT) as i64
    }
}

/// « dans 45 min » / « dans 2 h » / « en cours » pour le 1er évènement.
fn countdown(ev: &Ev, now_ep: i64) -> Option<String> {
    if now_ep == 0 {
        return None;
    }
    if now_ep >= ev.start && now_ep < ev.end {
        return Some("en cours".to_string());
    }
    let dl = ev.start - now_ep;
    if dl < 0 {
        return None;
    }
    let mins = dl / 60;
    if mins < 60 {
        let mut s = "dans ".to_string();
        s.push_str(&mins.to_string());
        s.push_str(" min");
        Some(s)
    } else if mins < 60 * 10 {
        let h = mins / 60;
        let m = mins % 60;
        let mut s = "dans ".to_string();
        s.push_str(&h.to_string());
        s.push_str(" h");
        if m >= 5 {
            s.push(' ');
            s.push_str(&m.to_string());
        }
        Some(s)
    } else {
        None
    }
}

const PW: i32 = 468;
const MARGIN: i32 = 28;
const ROW: i32 = 60;
const SHOWN: usize = 3;

pub fn draw(scene_now: f32) {
    unsafe {
        if !available() {
            return;
        }
        let n = EVS.len().min(SHOWN);
        let ph = 54 + n as i32 * ROW + 12;
        let x = MARGIN;
        let y = fb::HEIGHT as i32 - ph - MARGIN;

        fb::fill_rect(x - 2, y - 2, PW + 4, ph + 4, LINE);
        fb::fill_rect(x, y, PW, ph, BG);
        fb::fill_rect(x, y, 4, ph, ACCENT);

        let px = x + 30;
        font::draw_str_scaled(px, y + 20, "AGENDA", DIM, 2);

        let now_ep = epoch_now(scene_now);
        for (i, ev) in EVS.iter().take(n).enumerate() {
            let ry = y + 54 + i as i32 * ROW;
            let first = i == 0;
            let wc = if first { ACCENT } else { DIM };
            font::draw_str_scaled(px, ry, &ev.when, wc, 2);
            if first {
                if let Some(cd) = countdown(ev, now_ep) {
                    let cw = font::width_scaled(&cd, 2);
                    font::draw_str_scaled(x + PW - 24 - cw, ry, &cd, TXT, 2);
                }
            }
            let tc = if first { TXT } else { DIM };
            let show_loc = first && !ev.loc.is_empty();
            let lw = if show_loc {
                font::width_scaled(&ev.loc, 2) + 16
            } else {
                0
            };
            draw_fit(px, ry + 26, x + PW - 24 - lw, &ev.summ, tc);
            if show_loc {
                font::draw_str_scaled(
                    x + PW - 24 - font::width_scaled(&ev.loc, 2),
                    ry + 26,
                    &ev.loc,
                    DIM,
                    2,
                );
            }
        }
    }
}

fn draw_fit(x: i32, y: i32, maxx: i32, s: &str, col: u8) {
    let avail = maxx - x;
    if font::width_scaled(s, 2) <= avail {
        font::draw_str_scaled(x, y, s, col, 2);
        return;
    }
    let cw = font::width_scaled("m", 2).max(1);
    let n = ((avail / cw) as usize).saturating_sub(1);
    let mut t: String = s.chars().take(n).collect();
    t.push('.');
    font::draw_str_scaled(x, y, &t, col, 2);
}
