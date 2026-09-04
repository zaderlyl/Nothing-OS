//! `/web` : réponse directe sous la barre de recherche.
//!
//! Le noyau écrit la requête dans `.nothingos-web` ; `bridge/web.sh`
//! décide : question → il récupère une réponse et l'écrit dans
//! `.nothingos-web-answer` (affichée ici) ; sinon il ouvre le navigateur.

#![allow(static_mut_refs, dead_code)]

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::{fb, font, p9};

const ANS_PATH: &str = ".nothingos-web-answer";

const BG: u8 = 15; // PAL_SEARCH du bureau (fond sombre arrondi)
const TXT: u8 = 9; // PAL_TEXT
const DIM: u8 = 10; // PAL_TEXT_DIM
const ACCENT: u8 = 13; // PAL_ACCENT

static mut QUERY: String = String::new();
static mut SRC: String = String::new();
static mut LINES: Vec<String> = Vec::new();
static mut STATE: u8 = 0; // 0 rien, 1 en attente, 2 réponse, 3 recherche ouverte
static mut LAST: f32 = -100.0;
static mut SEEN: String = String::new();

pub fn pending() {
    unsafe {
        STATE = 1;
        LINES.clear();
        SRC.clear();
        SEEN.clear(); // la prochaine réponse (même identique) sera affichée
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
        let d = match p9::read_file(ANS_PATH) {
            Some(d) => d,
            None => return,
        };
        let t = match core::str::from_utf8(&d) {
            Ok(t) => t,
            Err(_) => return,
        };
        if t == SEEN {
            return;
        }
        SEEN = t.to_string();
        if t.trim().is_empty() {
            return;
        }
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
        // enlève les lignes vides de tête/queue
        while LINES.first().map(|s| s.is_empty()).unwrap_or(false) {
            LINES.remove(0);
        }
        while LINES.last().map(|s| s.is_empty()).unwrap_or(false) {
            LINES.pop();
        }
        STATE = if LINES.is_empty() { 1 } else { 2 };
    }
}

/// Dessine le cartouche sous la barre de recherche.
/// `bx, by, bw` = rectangle de la barre de recherche.
pub fn draw(bx: i32, by: i32, bw: i32, now: f32) {
    unsafe {
        if STATE == 0 {
            return;
        }
        let x = bx;
        let w = bw;
        let y = by + 8;

        let text: Vec<String> = match STATE {
            1 => alloc::vec!["recherche de la reponse...".to_string()],
            3 => alloc::vec!["pas de reponse directe - recherche ouverte dans le navigateur".to_string()],
            _ => {
                // (re)enveloppe à la largeur
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
        // petit curseur d'activité pendant l'attente
        if STATE == 1 && ((now * 2.0) as i32) % 2 == 0 {
            font::draw_str_scaled(x + pad + 12 + font::width_scaled(&text[0], 2) + 6, cy - line_h, "_", ACCENT, 2);
        }
    }
}

/// Le clic est-il dans le cartouche (pour le garder / le fermer) ?
pub fn hit(mx: i32, my: i32, bx: i32, by: i32, bw: i32) -> bool {
    unsafe {
        if STATE == 0 {
            return false;
        }
        mx >= bx - 4 && mx <= bx + bw + 4 && my >= by && my <= by + 600
    }
}

pub fn on_click(mx: i32, my: i32, bx: i32, by: i32, bw: i32) -> bool {
    if hit(mx, my, bx, by, bw) {
        dismiss();
        return true;
    }
    false
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
