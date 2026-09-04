//! Boîte mail dans l'OS : liste des messages récents + lecteur.
//! Données publiées par `bridge/mail.sh` (lit l'app Mail du Mac).
//!
//!  - `.nothingos-mail`      : `unread=<n>` puis `<id>|<lu>|<de>|<sujet>|<date>`
//!  - `.nothingos-mail-body` : corps du message ouvert
//!  - `.nothingos-mail-cmd`  : on y écrit `<seq> open|read|archive <id>`
//!
//! Compo voulue : clic sur « Mail — N non lus » → la liste entre par la
//! DROITE ; clic sur un message → le lecteur entre par la GAUCHE,
//! par-dessus la zone d'infos. Clic à l'extérieur → tout se referme.

#![allow(static_mut_refs, dead_code)]

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::win::P_ACCENT;
use crate::{fb, font, p9};

const LIST_PATH: &str = ".nothingos-mail";
const BODY_PATH: &str = ".nothingos-mail-body";
const CMD_PATH: &str = ".nothingos-mail-cmd";

const BG: u8 = 46;
const LINE: u8 = 47;
const TXT: u8 = 48;
const DIM: u8 = 49;
const ACCENT: u8 = P_ACCENT; // 70

pub fn install_palette() {
    fb::set_palette(BG, 20, 21, 28);
    fb::set_palette(LINE, 46, 49, 62);
    fb::set_palette(TXT, 222, 227, 240);
    fb::set_palette(DIM, 118, 124, 143);
}

struct Mail {
    id: i64,
    read: bool,
    from: String,
    subject: String,
    date: String,
}

struct Body {
    id: i64,
    from: String,
    subject: String,
    date: String,
    lines: Vec<String>,
}

static mut ITEMS: Vec<Mail> = Vec::new();
static mut UNREAD: u32 = 0;
static mut BODY: Option<Body> = None;
static mut OPEN_ID: i64 = 0;
static mut WANT_LIST: bool = false;
static mut LIST_OUT: f32 = 0.0;
static mut READER_OUT: f32 = 0.0;
static mut LIST_SCROLL: i32 = 0;
static mut READER_SCROLL: i32 = 0;
static mut SEQ: u32 = 0;
static mut LAST: f32 = -100.0;
static mut GOT: bool = false;

pub fn available() -> bool {
    unsafe { GOT }
}
pub fn unread() -> u32 {
    unsafe { UNREAD }
}
pub fn active() -> bool {
    unsafe { LIST_OUT > 0.01 || READER_OUT > 0.01 || WANT_LIST || OPEN_ID != 0 }
}

pub fn open_list() {
    unsafe {
        WANT_LIST = true;
        LIST_SCROLL = 0;
    }
}
pub fn close() {
    unsafe {
        WANT_LIST = false;
        OPEN_ID = 0;
        BODY = None;
    }
}

fn cmd(verb: &str, id: i64) {
    unsafe {
        SEQ = SEQ.wrapping_add(1);
        let mut s = SEQ.to_string();
        s.push(' ');
        s.push_str(verb);
        s.push(' ');
        s.push_str(&id.to_string());
        s.push('\n');
        p9::write_file(CMD_PATH, s.as_bytes());
    }
}

pub fn poll(now: f32) {
    unsafe {
        if now - LAST < 3.0 {
            return;
        }
        LAST = now;
        if let Some(d) = p9::read_file(LIST_PATH) {
            if let Ok(t) = core::str::from_utf8(&d) {
                let mut v: Vec<Mail> = Vec::new();
                for ln in t.lines() {
                    if let Some(n) = ln.strip_prefix("unread=") {
                        UNREAD = n.trim().parse().unwrap_or(0);
                        continue;
                    }
                    let mut it = ln.splitn(5, '|');
                    let id: i64 = match it.next().and_then(|x| x.parse().ok()) {
                        Some(v) => v,
                        None => continue,
                    };
                    let read = it.next() == Some("1");
                    let from = it.next().unwrap_or("").to_string();
                    let subject = it.next().unwrap_or("(sans sujet)").to_string();
                    let date = it.next().unwrap_or("").to_string();
                    v.push(Mail { id, read, from, subject, date });
                }
                if !v.is_empty() || t.starts_with("unread=") {
                    ITEMS = v;
                    GOT = true;
                }
            }
        }
        if OPEN_ID != 0 {
            if let Some(d) = p9::read_file(BODY_PATH) {
                if let Ok(t) = core::str::from_utf8(&d) {
                    let mut b = Body {
                        id: 0,
                        from: String::new(),
                        subject: String::new(),
                        date: String::new(),
                        lines: Vec::new(),
                    };
                    let mut in_body = false;
                    for ln in t.lines() {
                        if in_body {
                            b.lines.push(ln.to_string());
                        } else if ln == "---" {
                            in_body = true;
                        } else if let Some(v) = ln.strip_prefix("id=") {
                            b.id = v.trim().parse().unwrap_or(0);
                        } else if let Some(v) = ln.strip_prefix("from=") {
                            b.from = v.to_string();
                        } else if let Some(v) = ln.strip_prefix("subject=") {
                            b.subject = v.to_string();
                        } else if let Some(v) = ln.strip_prefix("date=") {
                            b.date = v.to_string();
                        }
                    }
                    if b.id == OPEN_ID {
                        // ré-enveloppe le corps à la largeur du lecteur,
                        // sans jamais couper un mot
                        let cols = reader_cols();
                        let mut wrapped: Vec<String> = Vec::new();
                        for raw in b.lines.iter() {
                            wrap_into(raw, cols, &mut wrapped);
                        }
                        b.lines = wrapped;
                        BODY = Some(b);
                    }
                }
            }
        }
    }
}

pub fn update(dt: f32) {
    unsafe {
        let lt = if WANT_LIST || OPEN_ID != 0 { 1.0 } else { 0.0 };
        let rt = if OPEN_ID != 0 { 1.0 } else { 0.0 };
        // vitesses différentes → les deux panneaux ne bougent pas en bloc
        LIST_OUT += (lt - LIST_OUT) * (1.0 - libm::powf(0.5, dt * 9.0));
        READER_OUT += (rt - READER_OUT) * (1.0 - libm::powf(0.5, dt * 12.0));
        LIST_OUT = LIST_OUT.clamp(0.0, 1.0);
        READER_OUT = READER_OUT.clamp(0.0, 1.0);
    }
}

pub fn on_scroll(mx: i32, _my: i32, dy: i32) {
    unsafe {
        // le lecteur (gauche) et la liste (droite) défilent indépendamment
        if READER_OUT > 0.5 && mx < reader_x() + RW {
            READER_SCROLL = (READER_SCROLL - dy * 3).max(0);
        } else if LIST_OUT > 0.5 {
            LIST_SCROLL = (LIST_SCROLL - dy * 2).max(0);
        }
    }
}

// --- géométrie ---
const LW: i32 = 640; // largeur liste (à droite)
const RW: i32 = 780; // largeur lecteur (à gauche)
const ROW: i32 = 78;
const LIST_TOP: i32 = 70;

fn list_x() -> i32 {
    let w = fb::WIDTH as i32;
    w - (LW as f32 * unsafe { LIST_OUT }) as i32
}
fn reader_x() -> i32 {
    -RW + (RW as f32 * unsafe { READER_OUT }) as i32
}

pub fn on_click(mx: i32, my: i32) -> bool {
    unsafe {
        let h = fb::HEIGHT as i32;
        // lecteur ouvert : boutons en bas
        if READER_OUT > 0.5 {
            let rx = reader_x();
            if mx >= rx && mx <= rx + RW {
                let by = h - 58;
                if my >= by && my <= by + 40 {
                    // 3 boutons : Lu | Archiver | Fermer
                    let bw = (RW - 96) / 3;
                    let i = (mx - rx - 32) / (bw + 16);
                    match i {
                        0 => {
                            cmd("read", OPEN_ID);
                            if let Some(m) = ITEMS.iter_mut().find(|m| m.id == OPEN_ID) {
                                m.read = true;
                            }
                        }
                        1 => {
                            cmd("archive", OPEN_ID);
                            ITEMS.retain(|m| m.id != OPEN_ID);
                        }
                        _ => {}
                    }
                    OPEN_ID = 0;
                    BODY = None;
                    return true;
                }
                return true; // clic dans le lecteur
            }
        }
        // liste ouverte : clic sur une ligne
        if LIST_OUT > 0.5 {
            let lx = list_x();
            if mx >= lx {
                let idx = (my - LIST_TOP + LIST_SCROLL) / ROW;
                if idx >= 0 && (idx as usize) < ITEMS.len() {
                    let id = ITEMS[idx as usize].id;
                    OPEN_ID = id;
                    BODY = None;
                    READER_SCROLL = 0;
                    cmd("open", id);
                }
                return true;
            }
            // clic hors des deux panneaux → fermeture
            if READER_OUT < 0.5 || mx > reader_x() + RW {
                close();
                return true;
            }
        }
        false
    }
}

pub fn draw(now: f32) {
    unsafe {
        let _ = now;
        let h = fb::HEIGHT as i32;
        // --- liste (droite) ---
        if LIST_OUT > 0.01 {
            let x = list_x();
            fb::fill_rect(x, 0, LW, h, BG);
            fb::fill_rect(x, 0, 2, h, LINE);
            font::draw_str_scaled(x + 32, 22, "MAIL", DIM, 2);
            let mut hdr = UNREAD.to_string();
            hdr.push_str(" non lus");
            font::draw_str_scaled(x + 110, 22, &hdr, ACCENT, 2);

            let mut y = LIST_TOP - LIST_SCROLL;
            for m in ITEMS.iter() {
                if y > -ROW && y < h {
                    if OPEN_ID == m.id {
                        fb::fill_rect(x + 2, y, LW - 2, ROW, LINE);
                    }
                    if !m.read {
                        fb::fill_circle((x + 20) as f32, (y + 24) as f32, 5.0, ACCENT);
                    }
                    let fc = if m.read { DIM } else { TXT };
                    let dw = font::width_scaled(&m.date, 2);
                    font::draw_str_scaled(x + LW - 24 - dw, y + 12, &m.date, DIM, 2);
                    fit(x + 36, y + 12, x + LW - 24 - dw - 16, &m.from, fc);
                    fit(x + 36, y + 40, x + LW - 24, &m.subject, if m.read { DIM } else { TXT });
                    fb::fill_rect(x + 24, y + ROW - 1, LW - 48, 1, LINE);
                }
                y += ROW;
            }
        }

        // --- lecteur (gauche) ---
        if READER_OUT > 0.01 {
            let x = reader_x();
            fb::fill_rect(x, 0, RW, h, BG);
            fb::fill_rect(x + RW - 2, 0, 2, h, LINE);
            match &BODY {
                None => {
                    font::draw_str_scaled(x + 32, h / 2 - 10, "ouverture...", DIM, 2);
                }
                Some(b) => {
                    font::draw_str_scaled(x + 32, 26, &b.from, ACCENT, 2);
                    let dw = font::width_scaled(&b.date, 2);
                    font::draw_str_scaled(x + RW - 32 - dw, 26, &b.date, DIM, 2);
                    // sujet : enveloppé (jamais coupé), 1 ou 2 lignes
                    let cols = reader_cols();
                    let mut subj: Vec<String> = Vec::new();
                    wrap_into(&b.subject, cols, &mut subj);
                    let mut sy = 58;
                    for l in subj.iter().take(2) {
                        font::draw_str_scaled(x + 32, sy, l, TXT, 2);
                        sy += 26;
                    }
                    let hdr_h = sy + 6;
                    fb::fill_rect(x + 32, hdr_h, RW - 64, 1, LINE);

                    let top = hdr_h + 20;
                    let bot = h - 76;
                    let mut ly = top - READER_SCROLL;
                    for ln in b.lines.iter() {
                        if ly > top - 24 && ly < bot {
                            font::draw_str_scaled(x + 32, ly, ln, TXT, 2);
                        }
                        ly += 24;
                    }
                    // masque + barre de boutons
                    fb::fill_rect(x, bot, RW, h - bot, BG);
                    fb::fill_rect(x + 32, bot, RW - 64, 1, LINE);
                    let by = h - 58;
                    let bw = (RW - 96) / 3;
                    for (i, lbl) in ["Marquer lu", "Archiver", "Fermer"].iter().enumerate() {
                        let bx = x + 32 + i as i32 * (bw + 16);
                        fb::fill_rect(bx, by, bw, 40, LINE);
                        let tw = font::width_scaled(lbl, 2);
                        font::draw_str_scaled(bx + (bw - tw) / 2, by + 10, lbl, TXT, 2);
                    }
                }
            }
        }
    }
}

/// Largeur en caractères du corps du lecteur.
fn reader_cols() -> usize {
    let cw = font::width_scaled("m", 2).max(1);
    (((RW - 60) / cw) as usize).max(10)
}

/// Découpe `s` en lignes d'au plus `cols` caractères, aux espaces
/// (jamais au milieu d'un mot ; un mot trop long est laissé tel quel).
fn wrap_into(s: &str, cols: usize, out: &mut Vec<String>) {
    if s.is_empty() {
        out.push(String::new());
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
        // mot unique plus long qu'une ligne : on le laisse déborder (pas
        // de coupe), la ligne suivante repart proprement
        if line.chars().count() > cols {
            out.push(core::mem::take(&mut line));
        }
    }
    if !line.is_empty() {
        out.push(line);
    }
}

/// Tronque `s` à la largeur dispo en coupant au dernier espace + « … ».
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
    let t = match cut.rfind(' ') {
        Some(i) if i > n / 2 => &cut[..i],
        _ => cut.as_str(),
    };
    let mut t = t.to_string();
    t.push_str("...");
    font::draw_str_scaled(x, y, &t, col, 2);
}
