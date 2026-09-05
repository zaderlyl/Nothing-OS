//! `/web` : réponse directe et lecture d'article, affichés sous la barre
//! de recherche.
//!
//! Le noyau écrit la requête dans `.nothingos-web` ; `bridge/web.sh` :
//!  - question → réponse Wikipédia (`.nothingos-web-answer`)
//!  - url / clic sur un lien → essaie d'extraire le texte lisible
//!    (`.nothingos-web-article`) ; sinon ouvre Firefox
//!  - sinon → recherche ouverte directement dans Firefox (pas d'API de
//!    résultats : Google Custom Search est inutilisable sans compte de
//!    facturation lié, abandonné).

#![allow(static_mut_refs, dead_code)]

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::{fb, font, p9};

const ANS_PATH: &str = ".nothingos-web-answer";
const ART_PATH: &str = ".nothingos-web-article";
const REQ_PATH: &str = ".nothingos-web";

const BG: u8 = 15; // PAL_SEARCH
const TXT: u8 = 9; // PAL_TEXT
const DIM: u8 = 10; // PAL_TEXT_DIM
const ACCENT: u8 = 13; // PAL_ACCENT

struct Article {
    url: String,
    title: String,
    lines: Vec<String>,
}

// 0 rien, 1 attente, 2 réponse, 3 recherche ouverte (navigateur),
// 5 article lisible
static mut STATE: u8 = 0;
static mut QUERY: String = String::new();
static mut SRC: String = String::new();
static mut LINES: Vec<String> = Vec::new();
static mut ARTICLE: Option<Article> = None;
static mut SCROLL: i32 = 0;

static mut LAST: f32 = -100.0;
static mut SEEN_ANS: String = String::new();
static mut SEEN_ART: String = String::new();

pub fn pending() {
    unsafe {
        STATE = 1;
        LINES.clear();
        SRC.clear();
        ARTICLE = None;
        SCROLL = 0;
        SEEN_ANS.clear();
        SEEN_ART.clear();
    }
}

pub fn dismiss() {
    unsafe {
        STATE = 0;
    }
}

pub fn visible() -> bool {
    unsafe { STATE != 0 }
}

pub fn poll(now: f32) {
    unsafe {
        if now - LAST < 0.4 {
            return;
        }
        LAST = now;

        // réponse directe (question)
        if let Some(d) = p9::read_file(ANS_PATH) {
            if let Ok(t) = core::str::from_utf8(&d) {
                if !t.is_empty() && t != SEEN_ANS {
                    SEEN_ANS = t.to_string();
                    parse_answer(t);
                }
            }
        }
        // article extrait
        if let Some(d) = p9::read_file(ART_PATH) {
            if let Ok(t) = core::str::from_utf8(&d) {
                if !t.is_empty() && t != SEEN_ART {
                    SEEN_ART = t.to_string();
                    parse_article(t);
                }
            }
        }
    }
}

unsafe fn parse_answer(t: &str) {
    QUERY.clear();
    SRC.clear();
    LINES.clear();
    let mut body = false;
    for ln in t.lines() {
        if body {
            LINES.push(ln.to_string());
        } else if ln == "---" {
            body = true;
        } else if let Some(v) = ln.strip_prefix("q=") {
            QUERY = v.to_string();
        } else if let Some(v) = ln.strip_prefix("src=") {
            SRC = v.to_string();
        } else if ln == "open" {
            STATE = 3;
            return;
        }
    }
    while LINES.first().map(|s| s.is_empty()).unwrap_or(false) {
        LINES.remove(0);
    }
    STATE = if LINES.is_empty() { 1 } else { 2 };
}

unsafe fn parse_article(t: &str) {
    let mut url = String::new();
    let mut title = String::new();
    let mut lines = Vec::new();
    let mut body = false;
    for ln in t.lines() {
        if body {
            lines.push(ln.to_string());
        } else if ln == "---" {
            body = true;
        } else if let Some(v) = ln.strip_prefix("url=") {
            url = v.to_string();
        } else if let Some(v) = ln.strip_prefix("title=") {
            title = v.to_string();
        }
    }
    if !lines.is_empty() {
        ARTICLE = Some(Article { url, title, lines });
        SCROLL = 0;
        STATE = 5;
    }
}

pub fn on_scroll(dy: i32) {
    unsafe {
        if STATE == 5 {
            SCROLL = (SCROLL - dy * 3).max(0);
        }
    }
}

/// Rectangle de la barre de recherche (doit suivre `home::draw_hero`).
fn bar_rect() -> (i32, i32, i32) {
    let w = fb::WIDTH as i32;
    let h = fb::HEIGHT as i32;
    let bw = 900;
    let bx = (w - bw) / 2;
    let ty = h * 30 / 100;
    let by = ty + 16 * 10 + 60 + 54; // + bh
    (bx, by, bw)
}

pub fn draw(now: f32) {
    unsafe {
        if STATE == 0 {
            return;
        }
        let (x, y0, w) = bar_rect();
        let y = y0 + 8;
        let h = fb::HEIGHT as i32;

        match STATE {
            5 => draw_article(x, y, w, h, now),
            _ => draw_answer(x, y, w, now),
        }
    }
}

unsafe fn draw_answer(x: i32, y: i32, w: i32, now: f32) {
    let text: Vec<String> = match STATE {
        1 => alloc::vec!["recherche...".to_string()],
        3 => alloc::vec!["pas de reponse directe - ouvert dans Firefox".to_string()],
        _ => {
            let cols = ((w - 72) / font::width_scaled("m", 2).max(1)) as usize;
            let mut out: Vec<String> = Vec::new();
            for para in LINES.iter() {
                wrap(para, cols.max(12), &mut out);
            }
            out
        }
    };
    let pad = 24;
    let line_h = 26;
    let head_h = if STATE == 2 { 34 } else { 0 };
    let h = pad * 2 + head_h + text.len() as i32 * line_h;

    fb::fill_rect(x - 2, y - 2, w + 4, h + 4, DIM);
    fb::fill_rect(x, y, w, h, BG);
    fb::fill_rect(x, y, 4, h, ACCENT);

    let mut cy = y + pad;
    if STATE == 2 {
        let hdr = if SRC.is_empty() { "reponse" } else { SRC.as_str() };
        font::draw_str_scaled(x + pad + 12, cy, hdr, DIM, 2);
        let close = "Fermer  (Echap)";
        let cw = font::width_scaled(close, 2);
        font::draw_str_scaled(x + w - pad - cw, cy, close, DIM, 2);
        cy += head_h;
    }
    let col = if STATE == 2 { TXT } else { DIM };
    for l in text.iter() {
        font::draw_str_scaled(x + pad + 12, cy, l, col, 2);
        cy += line_h;
    }
    if STATE == 1 && ((now * 2.0) as i32) % 2 == 0 {
        font::draw_str_scaled(x + pad + 12 + font::width_scaled(&text[0], 2) + 6, cy - line_h, "_", ACCENT, 2);
    }
}

unsafe fn draw_article(x: i32, y: i32, w: i32, screen_h: i32, _now: f32) {
    let a = match &ARTICLE {
        Some(a) => a,
        None => return,
    };
    let pad = 24;
    let bot = (screen_h - 90).min(y + 640);
    let h = bot - y + 20;

    fb::fill_rect(x - 2, y - 2, w + 4, h + 4, DIM);
    fb::fill_rect(x, y, w, h, BG);
    fb::fill_rect(x, y, 4, h, ACCENT);

    let cols = ((w - 72) / font::width_scaled("m", 2).max(1)) as usize;
    fit(x + pad + 12, y + pad, x + w - pad, &a.title, ACCENT);
    let by2 = y + pad + 30;
    let close = "Fermer (Echap)";
    let cw = font::width_scaled(close, 2);
    font::draw_str_scaled(x + w - pad - cw, by2, close, DIM, 2);
    let ff = "Ouvrir dans Firefox";
    let fw = font::width_scaled(ff, 2);
    font::draw_str_scaled(x + w - pad - cw - fw - 30, by2, ff, DIM, 2);

    let top = by2 + 30;
    fb::fill_rect(x + pad, top, w - pad * 2, 1, DIM);
    let mut ly = top + 30 - SCROLL;
    for para in a.lines.iter() {
        let mut wrapped: Vec<String> = Vec::new();
        wrap(para, cols.max(12), &mut wrapped);
        for l in wrapped {
            if ly > top && ly < bot {
                font::draw_str_scaled(x + pad + 12, ly, &l, TXT, 2);
            }
            ly += 26;
        }
        ly += 8; // espace entre paragraphes
    }
}

pub fn on_click(mx: i32, my: i32) -> bool {
    unsafe {
        if STATE == 0 {
            return false;
        }
        let (x, y0, w) = bar_rect();
        let y = y0 + 8;
        if mx < x - 4 || mx > x + w + 4 {
            dismiss();
            return true;
        }
        match STATE {
            5 => {
                let pad = 24;
                let by = y + pad + 30; // ligne des boutons (sous le titre)
                if my >= by - 10 && my < by + 26 {
                    let close = "Fermer (Echap)";
                    let cw = font::width_scaled(close, 2);
                    if mx >= x + w - pad - cw {
                        dismiss();
                        return true;
                    }
                    let ff = "Ouvrir dans Firefox";
                    let fw = font::width_scaled(ff, 2);
                    if mx >= x + w - pad - cw - fw - 30 {
                        if let Some(a) = &ARTICLE {
                            p9::write_file(".nothingos-web-firefox", a.url.as_bytes());
                            dismiss();
                        }
                        return true;
                    }
                }
                true
            }
            _ => {
                dismiss();
                true
            }
        }
    }
}

fn wrap(s: &str, cols: usize, out: &mut Vec<String>) {
    if s.is_empty() {
        return;
    }
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
}

fn fit(x: i32, y: i32, maxx: i32, s: &str, col: u8) {
    let avail = maxx - x;
    if avail <= 0 {
        return;
    }
    if font::width_scaled(s, 2) <= avail {
        font::draw_str_scaled(x, y, s, col, 2);
        return;
    }
    let cw = font::width_scaled("m", 2).max(1);
    let n = ((avail / cw) as usize).saturating_sub(2).max(1);
    let cut: String = s.chars().take(n).collect();
    let mut t = cut;
    t.push_str("...");
    font::draw_str_scaled(x, y, &t, col, 2);
}
