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
    endlbl: String,
    desc: String,
}

static mut EVS: Vec<Ev> = Vec::new();
static mut MAC_NOW: i64 = 0;
static mut READ_AT: f32 = -1.0;
static mut LAST: f32 = -100.0;
static mut GOT: bool = false;

static mut OPEN: Option<usize> = None;
static mut DET_OUT: f32 = 0.0;
static mut ROW_Y: [i32; 6] = [0; 6]; // Y des lignes cliquables (barre latérale)
static mut ROW_N: usize = 0;

pub fn detail_active() -> bool {
    unsafe { OPEN.is_some() || DET_OUT > 0.01 }
}

pub fn close_detail() {
    unsafe {
        OPEN = None;
    }
}

/// Clic dans la barre latérale : ouvre le détail de l'évènement survolé.
pub fn sidebar_click(mx: i32, my: i32, x0: i32, w: i32) -> bool {
    unsafe {
        if mx < x0 - 12 || mx > x0 + w + 12 {
            return false;
        }
        for i in 0..ROW_N {
            if my >= ROW_Y[i] - 12 && my < ROW_Y[i] + 48 {
                OPEN = Some(i);
                return true;
            }
        }
        false
    }
}

pub fn update(dt: f32) {
    unsafe {
        let t = if OPEN.is_some() { 1.0 } else { 0.0 };
        DET_OUT += (t - DET_OUT) * (1.0 - libm::powf(0.5, dt * 12.0));
        DET_OUT = DET_OUT.clamp(0.0, 1.0);
    }
}

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
            let mut it = line.splitn(7, '|');
            let s: i64 = match it.next().and_then(|x| x.parse().ok()) {
                Some(v) => v,
                None => continue,
            };
            let e: i64 = it.next().and_then(|x| x.parse().ok()).unwrap_or(s);
            let when = it.next().unwrap_or("").to_string();
            let summ = it.next().unwrap_or("").to_string();
            let loc = it.next().unwrap_or("").to_string();
            let endlbl = it.next().unwrap_or("").to_string();
            let desc = it.next().unwrap_or("").to_string();
            evs.push(Ev { start: s, end: e, when, summ, loc, endlbl, desc });
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

/// Rendu compact dans la barre latérale (sous « MAIL »), style discret.
/// `col_txt` / `col_dim` / `col_accent` = palette du bureau.
pub fn draw_sidebar(
    x0: i32,
    mut y: i32,
    w: i32,
    scene_now: f32,
    col_head: u8,
    col_txt: u8,
    col_dim: u8,
    col_accent: u8,
) -> i32 {
    unsafe {
        font::draw_str_scaled(x0, y, "AGENDA", col_head, 2);
        y += 40;
        if !available() {
            font::draw_str_scaled(x0, y, "chargement...", col_dim, 2);
            return y + 36;
        }
        let now_ep = epoch_now(scene_now);
        ROW_N = EVS.len().min(4);
        for (i, ev) in EVS.iter().take(4).enumerate() {
            let first = i == 0;
            let sel = OPEN == Some(i);
            ROW_Y[i] = y;
            if sel {
                fb::fill_rect(x0 - 10, y - 8, w + 20, 56, LINE);
            }
            font::draw_str_scaled(x0, y, &ev.when, if first { col_accent } else { col_dim }, 2);
            if first {
                if let Some(cd) = countdown(ev, now_ep) {
                    let cw = font::width_scaled(&cd, 2);
                    font::draw_str_scaled(x0 + w - cw, y, &cd, col_txt, 2);
                }
            }
            let maxx = x0 + w;
            let tc = if first || sel { col_txt } else { col_dim };
            let mut line = ev.summ.clone();
            if !ev.loc.is_empty() {
                line.push_str("  ");
                line.push_str(&ev.loc);
            }
            trunc(x0, y + 24, maxx, &line, tc);
            y += 60;
        }
        y
    }
}

// --- panneau détail (à droite) ---
const DW: i32 = 620;

pub fn draw_detail() {
    unsafe {
        if DET_OUT < 0.02 {
            return;
        }
        let w = fb::WIDTH as i32;
        let h = fb::HEIGHT as i32;
        let x = w - (DW as f32 * DET_OUT) as i32;
        fb::fill_rect(x - 2, 0, DW + 2, h, LINE);
        fb::fill_rect(x, 0, DW, h, BG);
        fb::fill_rect(x, 0, 4, h, ACCENT);

        let ev = match OPEN.and_then(|i| EVS.get(i)) {
            Some(e) => e,
            None => return,
        };
        let px = x + 34;
        let maxx = x + DW - 28;
        font::draw_str_scaled(px, 26, "EVENEMENT", DIM, 2);

        let mut y = 74;
        for l in wrap(&ev.summ, maxx - px) {
            font::draw_str_scaled(px, y, &l, TXT, 3);
            y += 40;
        }
        y += 14;
        let mut whenl = ev.when.clone();
        if !ev.endlbl.is_empty() {
            whenl.push_str(" - ");
            whenl.push_str(&ev.endlbl);
        }
        font::draw_str_scaled(px, y, &whenl, ACCENT, 2);
        y += 40;
        if !ev.loc.is_empty() {
            font::draw_str_scaled(px, y, "salle", DIM, 2);
            font::draw_str_scaled(px + 90, y, &ev.loc, TXT, 2);
            y += 40;
        }
        if !ev.desc.is_empty() {
            y += 12;
            fb::fill_rect(px, y, DW - 64, 1, LINE);
            y += 24;
            for l in wrap(&ev.desc, maxx - px) {
                if y > h - 80 {
                    break;
                }
                font::draw_str_scaled(px, y, &l, DIM, 2);
                y += 26;
            }
        }
        // bouton fermer
        let by = h - 58;
        fb::fill_rect(px, by, 160, 40, LINE);
        font::draw_str_scaled(px + 40, by + 10, "Fermer", TXT, 2);
    }
}

pub fn on_click(mx: i32, my: i32) -> bool {
    unsafe {
        if OPEN.is_none() {
            return false;
        }
        let w = fb::WIDTH as i32;
        let h = fb::HEIGHT as i32;
        let x = w - DW;
        if mx < x {
            OPEN = None; // clic dehors
            return true;
        }
        let by = h - 58;
        if my >= by && my <= by + 40 && mx >= x + 34 && mx <= x + 34 + 160 {
            OPEN = None;
        }
        true
    }
}

fn wrap(s: &str, px_w: i32) -> Vec<String> {
    let cw = font::width_scaled("m", 2).max(1);
    let cols = ((px_w / cw) as usize).max(8);
    let mut out: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in s.split(' ') {
        if line.is_empty() {
            line.push_str(word);
        } else if line.chars().count() + 1 + word.chars().count() <= cols {
            line.push(' ');
            line.push_str(word);
        } else {
            out.push(core::mem::take(&mut line));
            line.push_str(word);
        }
        if line.chars().count() > cols {
            out.push(core::mem::take(&mut line));
        }
    }
    if !line.is_empty() {
        out.push(line);
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

fn trunc(x: i32, y: i32, maxx: i32, s: &str, col: u8) {
    let avail = maxx - x;
    if font::width_scaled(s, 2) <= avail {
        font::draw_str_scaled(x, y, s, col, 2);
        return;
    }
    let cw = font::width_scaled("m", 2).max(1);
    let n = ((avail / cw) as usize).saturating_sub(2).max(1);
    let cut: String = s.chars().take(n).collect();
    let t = match cut.rfind(' ') {
        Some(i) if i > n / 2 => cut[..i].to_string(),
        _ => cut,
    };
    let mut t = t;
    t.push_str("...");
    font::draw_str_scaled(x, y, &t, col, 2);
}

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
