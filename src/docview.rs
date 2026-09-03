//! Consultation de documents — deux demi-panneaux qui glissent depuis les
//! bords de l'écran, **sans empilement de fenêtres**.
//!
//! * `/doc all` (ou `/doc`) : la **liste** des documents (racine du
//!   partage Mac + fichiers RAM) entre par la **gauche** ; la molette la
//!   fait défiler.
//! * un clic sur un fichier fait entrer sa **visualisation** par la
//!   **droite** ; un espace sépare les deux panneaux.
//! * cliquer en dehors « retire » le panneau du dessus (droite puis
//!   gauche) : il ressort et disparaît, rien n'est mémorisé.

#![allow(dead_code, static_mut_refs)]

use alloc::string::String;
use alloc::vec::Vec;

use crate::win::{P_ACCENT, P_CODE_BG, P_DIM, P_FRAME, P_STR, P_TEXT, P_TITLE_HI};
use crate::{dots, fb, font, fs, hostfs, p9};

const W: i32 = fb::WIDTH as i32;
const H: i32 = fb::HEIGHT as i32;

const PW: i32 = 780; // largeur d'un panneau (~demi-écran)
const RIGHT_X0: i32 = W - PW; // 1140 : bord gauche du panneau droit sorti
const PADX: i32 = 28;
const HEADER_H: i32 = 100;
const ROW_H: i32 = 46;

// --- état ---------------------------------------------------------------

struct Ent {
    name: String,
    dir: bool,
    host: bool,
}

static mut ENTRIES: Vec<Ent> = Vec::new();
static mut L_ON: bool = false;
static mut R_ON: bool = false;
static mut L_OUT: f32 = 0.0; // 0 = caché, 1 = sorti
static mut R_OUT: f32 = 0.0;
static mut L_SCROLL: i32 = 0;
static mut R_SCROLL: i32 = 0;
static mut SEL: i32 = -1;
static mut CONTENT: Vec<u8> = Vec::new();
static mut CONTENT_NAME: String = String::new();

pub fn active() -> bool {
    unsafe { L_ON || R_ON || L_OUT > 0.01 || R_OUT > 0.01 }
}

/// Le panneau gauche occupe-t-il le bord gauche ? (pour masquer la barre
/// latérale automatique du bureau)
pub fn left_covering() -> bool {
    unsafe { L_OUT > 0.04 }
}

/// Ouvre (ou recharge) la liste des documents.
pub fn open_list() {
    hostfs::refresh_dir();
    let mut v: Vec<Ent> = Vec::new();
    if hostfs::have_dir() {
        hostfs::each_dir(|name, dir| {
            v.push(Ent {
                name: String::from(name),
                dir,
                host: true,
            });
        });
    }
    fs::each(|_, f| {
        v.push(Ent {
            name: String::from(f.name()),
            dir: f.is_dir(),
            host: false,
        });
    });
    // dossiers d'abord, puis ordre alphabétique (insensible à la casse)
    v.sort_by(|a, b| match (a.dir, b.dir) {
        (true, false) => core::cmp::Ordering::Less,
        (false, true) => core::cmp::Ordering::Greater,
        _ => ci_cmp(&a.name, &b.name),
    });
    unsafe {
        ENTRIES = v;
        L_ON = true;
        L_SCROLL = 0;
        SEL = -1;
        R_ON = false;
    }
    crate::serial_println!("[doc] liste : {} entree(s)", unsafe { ENTRIES.len() });
}

pub fn close_all() {
    unsafe {
        L_ON = false;
        R_ON = false;
    }
}

fn ci_cmp(a: &str, b: &str) -> core::cmp::Ordering {
    let lo = |c: u8| if c.is_ascii_uppercase() { c + 32 } else { c };
    for (x, y) in a.bytes().zip(b.bytes()) {
        match lo(x).cmp(&lo(y)) {
            core::cmp::Ordering::Equal => continue,
            o => return o,
        }
    }
    a.len().cmp(&b.len())
}

fn approach(cur: f32, target: f32, dt: f32, speed: f32) -> f32 {
    cur + (target - cur) * (1.0 - libm::powf(0.5, dt * speed))
}

pub fn update(dt: f32) {
    unsafe {
        if !L_ON {
            R_ON = false;
        }
        L_OUT = approach(L_OUT, if L_ON { 1.0 } else { 0.0 }, dt, 11.0).clamp(0.0, 1.0);
        R_OUT = approach(R_OUT, if R_ON { 1.0 } else { 0.0 }, dt, 11.0).clamp(0.0, 1.0);
        clamp_scrolls();
    }
}

fn list_content_h() -> i32 {
    unsafe { ENTRIES.len() as i32 * ROW_H }
}

fn view_content_h() -> i32 {
    // hauteur du texte rendu (lignes repliées)
    unsafe { wrapped_lines(&CONTENT) as i32 * (16 + 6) }
}

fn clamp_scrolls() {
    unsafe {
        let lmax = (list_content_h() - (H - HEADER_H)).max(0);
        L_SCROLL = L_SCROLL.clamp(0, lmax);
        let rmax = (view_content_h() - (H - HEADER_H)).max(0);
        R_SCROLL = R_SCROLL.clamp(0, rmax);
    }
}

/// Molette : `delta` positif = vers le haut.
pub fn on_scroll(mx: i32, my: i32, delta: i32) {
    let _ = my;
    let step = delta * 40;
    unsafe {
        if R_OUT > 0.5 && mx >= RIGHT_X0 {
            R_SCROLL -= step;
        } else if L_OUT > 0.5 && mx < PW {
            L_SCROLL -= step;
        }
        clamp_scrolls();
    }
}

/// Clic. Renvoie `true` si le clic a été « consommé » par la consultation.
pub fn on_click(mx: i32, my: i32) -> bool {
    if !active() {
        return false;
    }
    unsafe {
        // panneau droit visible et clic dedans → on garde le clic
        if R_OUT > 0.5 && mx >= RIGHT_X0 {
            return true;
        }
        // panneau gauche visible et clic dedans → sélection d'un fichier
        if L_OUT > 0.5 && mx < PW {
            if my >= HEADER_H {
                let idx = (my - HEADER_H + L_SCROLL) / ROW_H;
                if idx >= 0 && (idx as usize) < ENTRIES.len() {
                    let e = &ENTRIES[idx as usize];
                    if !e.dir {
                        SEL = idx;
                        load_content();
                        R_ON = true;
                        R_SCROLL = 0;
                    }
                }
            }
            return true;
        }
        // clic en dehors → on retire le panneau du dessus
        if R_ON {
            R_ON = false;
        } else if L_ON {
            L_ON = false;
        }
        true
    }
}

fn load_content() {
    unsafe {
        let e = &ENTRIES[SEL as usize];
        let name = e.name.clone();
        let mut data = if e.host {
            p9::read_file(&name).unwrap_or_default()
        } else {
            match fs::find(name.as_bytes()) {
                Some(i) => fs::get(i).map(|f| f.data[..f.len].to_vec()).unwrap_or_default(),
                None => Vec::new(),
            }
        };
        data.truncate(32 * 1024); // aperçu : on plafonne l'affichage
        CONTENT = data;
        CONTENT_NAME = name;
    }
    crate::serial_println!("[doc] ouvre {} ({} o)", unsafe { &CONTENT_NAME }, unsafe {
        CONTENT.len()
    });
}

// --- rendu -------------------------------------------------------------

fn trunc<'a>(s: &'a str, max: usize, buf: &'a mut [u8]) -> &'a str {
    let b = s.as_bytes();
    if b.len() <= max {
        return s;
    }
    let n = max.min(buf.len());
    buf[..n - 1].copy_from_slice(&b[..n - 1]);
    buf[n - 1] = b'~';
    core::str::from_utf8(&buf[..n]).unwrap_or(s)
}

/// Nombre de lignes une fois repliées à la largeur du panneau.
fn wrapped_lines(data: &[u8]) -> usize {
    let cols = ((PW - 2 * PADX) / 16).max(1) as usize;
    let mut lines = 1;
    let mut col = 0;
    for &b in data {
        if b == b'\n' {
            lines += 1;
            col = 0;
        } else {
            col += 1;
            if col >= cols {
                lines += 1;
                col = 0;
            }
        }
    }
    lines
}

pub fn draw(_now: f32) {
    unsafe {
        if L_OUT > 0.01 {
            draw_left((-(PW as f32) * (1.0 - L_OUT)) as i32);
        }
        if R_OUT > 0.01 {
            draw_right((W as f32 - PW as f32 * R_OUT) as i32);
        }
    }
}

fn draw_left(lx: i32) {
    fb::fill_rect(lx, 0, PW, H, P_CODE_BG);
    fb::fill_rect(lx + PW - 3, 0, 3, H, P_FRAME);

    // liste (dessinée d'abord, l'en-tête la masquera en haut)
    let mut y = HEADER_H - unsafe { L_SCROLL };
    let sel = unsafe { SEL };
    let count = unsafe { ENTRIES.len() };
    for i in 0..count {
        if y + ROW_H > HEADER_H && y < H {
            let e = unsafe { &ENTRIES[i] };
            if i as i32 == sel {
                fb::fill_rect(lx, y, PW, ROW_H, P_TITLE_HI);
                fb::fill_rect(lx, y, 4, ROW_H, P_ACCENT);
            }
            let pat = if e.dir { dots::FOLDER } else { dots::FILE };
            dots::draw(pat, lx + PADX, y + 8, 3, P_TEXT, P_DIM);
            let mut buf = [0u8; 48];
            let label = trunc(&e.name, 42, &mut buf);
            let col = if e.dir { P_TEXT } else { P_DIM };
            font::draw_str_scaled(lx + PADX + 54, y + 13, label, col, 2);
            if e.host {
                fb::fill_rect(lx + PW - 26, y + ROW_H / 2 - 5, 10, 10, P_STR);
            }
        }
        y += ROW_H;
    }

    // en-tête
    fb::fill_rect(lx, 0, PW, HEADER_H, P_TITLE_HI);
    fb::fill_rect(lx, HEADER_H - 2, PW, 2, P_FRAME);
    dots::draw(dots::FOLDER, lx + PADX, 26, 4, P_TEXT, P_DIM);
    font::draw_str_scaled(lx + PADX + 74, 20, "DOCUMENTS", P_TEXT, 3);
    font::draw_str_scaled(lx + PADX + 74, 60, "molette : defiler   clic : ouvrir", P_DIM, 2);

    // ascenseur
    scrollbar(lx + PW - 8, list_content_h(), unsafe { L_SCROLL });
}

fn draw_right(rx: i32) {
    fb::fill_rect(rx, 0, PW, H, P_CODE_BG);
    fb::fill_rect(rx, 0, 3, H, P_FRAME);

    let cols = ((PW - 2 * PADX) / 16).max(1) as usize;
    let mut x = rx + PADX;
    let mut y = HEADER_H + 6 - unsafe { R_SCROLL };
    let mut c = 0usize;
    for &b in unsafe { &CONTENT } {
        if b == b'\n' {
            y += 22;
            x = rx + PADX;
            c = 0;
            continue;
        }
        let ch = if b == b'\t' { b' ' } else { b };
        if (0x20..=0x7e).contains(&ch) {
            if y + 16 > HEADER_H && y < H {
                font::draw_char_scaled(x, y, ch, P_TEXT, None, 2);
            }
            x += 16;
            c += 1;
            if c >= cols {
                y += 22;
                x = rx + PADX;
                c = 0;
            }
        }
    }

    // en-tête (par-dessus le texte)
    fb::fill_rect(rx, 0, PW, HEADER_H, P_TITLE_HI);
    fb::fill_rect(rx, HEADER_H - 2, PW, 2, P_FRAME);
    dots::draw(dots::FILE, rx + PADX, 26, 4, P_TEXT, P_DIM);
    let mut buf = [0u8; 44];
    let name = trunc(unsafe { &CONTENT_NAME }, 38, &mut buf);
    font::draw_str_scaled(rx + PADX + 64, 30, name, P_TEXT, 3);

    scrollbar(rx + 2, view_content_h(), unsafe { R_SCROLL });
}

fn scrollbar(x: i32, content_h: i32, scroll: i32) {
    let view_h = H - HEADER_H;
    if content_h <= view_h {
        return;
    }
    let track_h = view_h - 8;
    let knob_h = (track_h * view_h / content_h).max(24);
    let max_scroll = content_h - view_h;
    let knob_y = HEADER_H + 4 + (track_h - knob_h) * scroll / max_scroll.max(1);
    fb::fill_rect(x, HEADER_H + 4, 4, track_h, P_FRAME);
    fb::fill_rect(x, knob_y, 4, knob_h, P_ACCENT);
}
