//! Liste de tâches « À FAIRE » (barre latérale). Éditable : cocher,
//! ajouter, supprimer. Persistée dans `.nothingos-todo` sur le partage
//! 9p (une ligne par tâche : `x texte` = faite, `- texte` = à faire).

#![allow(static_mut_refs, dead_code)]

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::{font, p9};

const PATH: &str = ".nothingos-todo";

pub struct Item {
    pub text: String,
    pub done: bool,
}

static mut ITEMS: Vec<Item> = Vec::new();
static mut LOADED: bool = false;
static mut ADDING: bool = false;
static mut DRAFT: String = String::new();
// zones cliquables mémorisées au dessin (case, croix) par ligne
static mut CHK_Y: [i32; 16] = [0; 16];
static mut N_ROWS: usize = 0;
static mut ADD_Y: i32 = 0;

const DEFAULTS: [&str; 3] = [
    "- Finir le pilote clavier",
    "- Repondre a Lea",
    "x Ranger le bureau",
];

pub fn load() {
    unsafe {
        if LOADED {
            return;
        }
        LOADED = true;
        let raw = p9::read_file(PATH)
            .and_then(|d| core::str::from_utf8(&d).ok().map(|s| s.to_string()));
        let text = match raw {
            Some(t) if !t.trim().is_empty() => t,
            _ => DEFAULTS.join("\n"),
        };
        ITEMS.clear();
        for ln in text.lines() {
            let ln = ln.trim_end();
            if ln.is_empty() {
                continue;
            }
            let (done, rest) = match ln.as_bytes()[0] {
                b'x' | b'X' => (true, ln[1..].trim_start()),
                b'-' => (false, ln[1..].trim_start()),
                _ => (false, ln),
            };
            ITEMS.push(Item { text: rest.to_string(), done });
        }
    }
}

fn save() {
    unsafe {
        let mut s = String::new();
        for it in ITEMS.iter() {
            s.push_str(if it.done { "x " } else { "- " });
            s.push_str(&it.text);
            s.push('\n');
        }
        p9::write_file(PATH, s.as_bytes());
    }
}

pub fn items() -> &'static [Item] {
    unsafe { &ITEMS }
}
pub fn adding() -> bool {
    unsafe { ADDING }
}

pub fn toggle(i: usize) {
    unsafe {
        if let Some(it) = ITEMS.get_mut(i) {
            it.done = !it.done;
            save();
        }
    }
}

pub fn remove(i: usize) {
    unsafe {
        if i < ITEMS.len() {
            ITEMS.remove(i);
            save();
        }
    }
}

pub fn begin_add() {
    unsafe {
        ADDING = true;
        DRAFT.clear();
    }
}

pub fn cancel_add() {
    unsafe {
        ADDING = false;
        DRAFT.clear();
    }
}

/// Retourne `true` si la touche a été consommée (mode ajout).
pub fn feed_key(c: u8) -> bool {
    unsafe {
        if !ADDING {
            return false;
        }
        match c {
            b'\n' | b'\r' => {
                let t = DRAFT.trim();
                if !t.is_empty() {
                    ITEMS.push(Item { text: t.to_string(), done: false });
                    save();
                }
                ADDING = false;
                DRAFT.clear();
            }
            0x1b => {
                ADDING = false;
                DRAFT.clear();
            }
            0x08 => {
                DRAFT.pop();
            }
            0x20..=0x7e => {
                if DRAFT.len() < 80 {
                    DRAFT.push(c as char);
                }
            }
            _ => {}
        }
        true
    }
}

// --- rendu dans la barre latérale ---
pub const ROW_H: i32 = 40;

/// Dessine la section. `x` = gauche du texte, `w` = largeur dispo.
/// Renvoie le prochain `y`.
pub fn draw_sidebar(
    x: i32,
    mut y: i32,
    w: i32,
    now: f32,
    col_head: u8,
    col_text: u8,
    col_dim: u8,
    col_accent: u8,
    col_box: u8,
    col_bg: u8,
) -> i32 {
    unsafe {
        font::draw_str_scaled(x, y, "A FAIRE", col_head, 2);
        y += 44;
        N_ROWS = ITEMS.len().min(16);
        for (i, it) in ITEMS.iter().take(16).enumerate() {
            CHK_Y[i] = y;
            // case à cocher
            crate::fb::fill_rect(x, y, 20, 20, col_box);
            crate::fb::fill_rect(x + 2, y + 2, 16, 16, col_bg);
            if it.done {
                crate::fb::fill_rect(x + 4, y + 4, 12, 12, col_accent);
            }
            let tc = if it.done { col_dim } else { col_text };
            fit(x + 34, y, x + w - 28, &it.text, tc);
            // croix de suppression, à droite
            font::draw_str_scaled(x + w - 20, y, "x", col_dim, 2);
            y += ROW_H;
        }
        // ligne « + ajouter »
        ADD_Y = y;
        if ADDING {
            crate::fb::fill_rect(x, y - 4, w, 34, col_box);
            let blink = ((now * 2.0) as i32) % 2 == 0;
            let mut d = DRAFT.clone();
            if blink {
                d.push('_');
            }
            font::draw_str_scaled(x + 8, y, &d, col_text, 2);
        } else {
            font::draw_str_scaled(x, y, "+ ajouter une tache", col_dim, 2);
        }
        y + 44
    }
}

/// Clic dans la zone « À FAIRE ». `true` si consommé.
pub fn sidebar_click(mx: i32, my: i32, x: i32, w: i32) -> bool {
    unsafe {
        if mx < x - 8 || mx > x + w + 8 {
            return false;
        }
        for i in 0..N_ROWS {
            let ry = CHK_Y[i];
            if my >= ry - 6 && my < ry + ROW_H - 6 {
                if mx >= x + w - 34 {
                    remove(i);
                } else {
                    toggle(i);
                }
                return true;
            }
        }
        if my >= ADD_Y - 8 && my < ADD_Y + 34 {
            if ADDING {
                cancel_add();
            } else {
                begin_add();
            }
            return true;
        }
        false
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
    let n = ((avail / cw) as usize).saturating_sub(1).max(1);
    let t: String = s.chars().take(n).collect();
    font::draw_str_scaled(x, y, &t, col, 2);
}
