//! Mini gestionnaire de fenêtres. Pas de vrai système de processus ni de
//! fichiers : chaque fenêtre est une *maquette* de l'application demandée
//! (éditeur, chat, navigateur, gestionnaire de fichiers...). Ça suffit
//! pour l'usage « je tape une commande, une fenêtre s'ouvre ».

use crate::{fb, font};

const MAX: usize = 6;
const TITLE_H: i32 = 30;

// --- palette fenêtres (indices 64..=90) ---
const P_FRAME: u8 = 64;
const P_TITLE: u8 = 65;
const P_TITLE_HI: u8 = 66;
const P_BODY: u8 = 67;
const P_TEXT: u8 = 68;
const P_DIM: u8 = 69;
const P_ACCENT: u8 = 70;
const P_CODE_BG: u8 = 71;
const P_CODE_KW: u8 = 72;
const P_CODE_STR: u8 = 73;
const P_CODE_FN: u8 = 74;
const P_CLOSE: u8 = 75;

pub fn install_palette() {
    fb::set_palette(P_FRAME, 60, 64, 82);
    fb::set_palette(P_TITLE, 30, 33, 44);
    fb::set_palette(P_TITLE_HI, 46, 50, 66);
    fb::set_palette(P_BODY, 22, 24, 32);
    fb::set_palette(P_TEXT, 216, 222, 236);
    fb::set_palette(P_DIM, 120, 126, 146);
    fb::set_palette(P_ACCENT, 120, 200, 255);
    fb::set_palette(P_CODE_BG, 18, 20, 28);
    fb::set_palette(P_CODE_KW, 150, 130, 240);
    fb::set_palette(P_CODE_STR, 140, 210, 140);
    fb::set_palette(P_CODE_FN, 240, 200, 120);
    fb::set_palette(P_CLOSE, 230, 90, 90);
}

#[derive(Clone, Copy, PartialEq)]
pub enum App {
    Generic,
    Code,
    Chat,
    Git,
    Web,
    Files,
    FileView,
    Hub,
}

#[derive(Clone, Copy)]
struct Win {
    app: App,
    title: [u8; 40],
    tlen: usize,
    arg: [u8; 96],
    alen: usize,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    open: bool,
}

impl Win {
    const EMPTY: Win = Win {
        app: App::Generic,
        title: [0; 40],
        tlen: 0,
        arg: [0; 96],
        alen: 0,
        x: 0,
        y: 0,
        w: 0,
        h: 0,
        open: false,
    };
    fn title(&self) -> &str {
        core::str::from_utf8(&self.title[..self.tlen]).unwrap_or("?")
    }
    fn arg(&self) -> &str {
        core::str::from_utf8(&self.arg[..self.alen]).unwrap_or("")
    }
}

fn copy_into(dst: &mut [u8], src: &[u8]) -> usize {
    let n = src.len().min(dst.len());
    dst[..n].copy_from_slice(&src[..n]);
    n
}

pub struct Manager {
    /// De bas en haut : `wins[order[0]]` derrière, `wins[order[n-1]]` devant.
    wins: [Win; MAX],
    order: [usize; MAX],
    n: usize,
    drag: Option<(usize, i32, i32)>, // (slot dans `order`, dx, dy souris→fenêtre)
    spawn_i: i32,
}

impl Manager {
    pub fn new() -> Manager {
        Manager {
            wins: [Win::EMPTY; MAX],
            order: [0, 1, 2, 3, 4, 5],
            n: 0,
            drag: None,
            spawn_i: 0,
        }
    }

    /// Ouvre une fenêtre (remplace la plus ancienne si plein).
    pub fn spawn(&mut self, app: App, title: &[u8], arg: &[u8]) {
        let slot = if self.n < MAX {
            let s = self.n;
            self.order[self.n] = s;
            self.n += 1;
            s
        } else {
            // réutilise le slot de la fenêtre du fond
            let s = self.order[0];
            for k in 0..MAX - 1 {
                self.order[k] = self.order[k + 1];
            }
            self.order[MAX - 1] = s;
            s
        };
        let w = &mut self.wins[slot];
        *w = Win::EMPTY;
        w.app = app;
        w.tlen = copy_into(&mut w.title, title);
        w.alen = copy_into(&mut w.arg, arg);
        w.open = true;
        let (ww, wh) = match app {
            App::Hub => (620, 520),
            App::Web => (1100, 720),
            App::Code => (1180, 760),
            _ => (960, 620),
        };
        let off = (self.spawn_i % 5) * 44;
        w.x = (fb::WIDTH as i32 - ww) / 2 - 120 + off;
        w.y = (fb::HEIGHT as i32 - wh) / 2 - 40 + off;
        w.w = ww;
        w.h = wh;
        self.spawn_i += 1;
        self.focus_slot(self.top_index_of(slot));
    }

    fn top_index_of(&self, slot: usize) -> usize {
        for i in 0..self.n {
            if self.order[i] == slot {
                return i;
            }
        }
        0
    }

    fn focus_slot(&mut self, i: usize) {
        if i + 1 >= self.n {
            return;
        }
        let s = self.order[i];
        for k in i..self.n - 1 {
            self.order[k] = self.order[k + 1];
        }
        self.order[self.n - 1] = s;
    }

    #[allow(dead_code)]
    pub fn any_open(&self) -> bool {
        self.n > 0
    }

    /// Application de la fenêtre au premier plan (pour l'humeur d'Asti).
    pub fn focused_app(&self) -> Option<App> {
        if self.n == 0 {
            None
        } else {
            Some(self.wins[self.order[self.n - 1]].app)
        }
    }

    /// Souris : focus au clic, glisser par la barre de titre, bouton fermer.
    pub fn on_mouse(&mut self, mx: i32, my: i32, down: bool, pressed: bool) {
        if let Some((oi, dx, dy)) = self.drag {
            if down {
                let s = self.order[oi];
                self.wins[s].x = mx - dx;
                self.wins[s].y = my - dy;
            } else {
                self.drag = None;
            }
            return;
        }
        if !pressed {
            return;
        }
        // du haut vers le bas
        for i in (0..self.n).rev() {
            let s = self.order[i];
            let w = self.wins[s];
            if mx >= w.x && mx < w.x + w.w && my >= w.y && my < w.y + w.h + TITLE_H {
                // bouton fermer ?
                let cbx = w.x + w.w - 26;
                if my < w.y + TITLE_H && mx >= cbx {
                    // ferme : retire du z-order
                    for k in i..self.n - 1 {
                        self.order[k] = self.order[k + 1];
                    }
                    self.n -= 1;
                    self.wins[s].open = false;
                    return;
                }
                self.focus_slot(i);
                if my < w.y + TITLE_H {
                    let oi = self.n - 1;
                    self.drag = Some((oi, mx - w.x, my - w.y));
                }
                return;
            }
        }
    }

    pub fn draw(&self, t: f32) {
        for i in 0..self.n {
            let w = self.wins[self.order[i]];
            let focused = i + 1 == self.n;
            draw_window(&w, focused, t);
        }
    }
}

fn draw_window(w: &Win, focused: bool, t: f32) {
    // cadre + ombre légère
    fb::fill_rect(w.x + 6, w.y + 8, w.w, w.h + TITLE_H, 0);
    fb::fill_rect(w.x - 1, w.y - 1, w.w + 2, w.h + TITLE_H + 2, P_FRAME);

    // barre de titre
    let tc = if focused { P_TITLE_HI } else { P_TITLE };
    fb::fill_rect(w.x, w.y, w.w, TITLE_H, tc);
    font::draw_str_scaled(w.x + 12, w.y + 7, w.title(), P_TEXT, 2);
    // bouton fermer
    fb::fill_rect(w.x + w.w - 24, w.y + 6, 18, 18, P_CLOSE);
    font::draw_str_scaled(w.x + w.w - 22, w.y + 6, "x", P_TEXT, 2);

    // corps
    let (bx, by, bw, bh) = (w.x, w.y + TITLE_H, w.w, w.h);
    fb::fill_rect(bx, by, bw, bh, P_BODY);

    match w.app {
        App::Code => draw_code(bx, by, bw, bh, w.arg(), t),
        App::Chat => draw_chat(bx, by, bw, bh),
        App::Git => draw_git(bx, by, bw, bh, t),
        App::Web => draw_web(bx, by, bw, bh, w.arg()),
        App::Files => draw_files(bx, by, bw, bh, w.arg()),
        App::FileView => draw_fileview(bx, by, bw, bh, w.arg()),
        App::Hub => draw_hub(bx, by, bw, bh),
        App::Generic => {
            font::draw_str_scaled(bx + 40, by + 40, w.arg(), P_TEXT, 3);
            font::draw_str_scaled(bx + 40, by + 96, "application (maquette)", P_DIM, 2);
        }
    }
}

fn bar(x: i32, y: i32, w: i32, c: u8) {
    fb::fill_rect(x, y, w, 12, c);
}

fn draw_code(bx: i32, by: i32, bw: i32, bh: i32, name: &str, t: f32) {
    fb::fill_rect(bx, by, bw, bh, P_CODE_BG);
    fb::fill_rect(bx, by, 60, bh, P_TITLE); // marge n° de lignes
    font::draw_str_scaled(bx + 70, by + 8, name, P_DIM, 2);
    let mut y = by + 40;
    let cols = [P_CODE_KW, P_CODE_FN, P_TEXT, P_CODE_STR, P_TEXT, P_TEXT];
    let widths = [140, 320, 200, 420, 260, 160];
    let indent = [0, 40, 40, 80, 40, 0];
    for l in 0..((bh - 60) / 26).min(18) {
        font::draw_num(bx + 14, y, (l + 1) as u32, 2, P_DIM, 2);
        let k = (l as usize) % 6;
        bar(bx + 74 + indent[k], y + 4, widths[k], cols[k]);
        y += 26;
    }
    // curseur qui clignote
    if (t * 2.0) as i32 % 2 == 0 {
        fb::fill_rect(bx + 74 + 200, by + 40 + 4 * 26, 3, 18, P_ACCENT);
    }
}

fn draw_chat(bx: i32, by: i32, bw: i32, bh: i32) {
    fb::fill_rect(bx, by, 220, bh, P_TITLE); // liste des salons
    for i in 0..8 {
        bar(bx + 20, by + 24 + i * 40, 160, P_DIM);
    }
    // bulles
    let msgs = [(false, 380), (true, 300), (false, 520), (true, 180), (false, 440)];
    let mut y = by + 40;
    for (mine, wd) in msgs {
        let x = if mine { bx + bw - wd - 40 } else { bx + 260 };
        fb::fill_rect(x, y, wd, 46, if mine { P_ACCENT } else { P_TITLE_HI });
        y += 70;
    }
}

fn draw_git(bx: i32, by: i32, _bw: i32, bh: i32, t: f32) {
    let off = ((t * 20.0) as i32) % 40;
    let mut y = by + 20 - off;
    while y < by + bh {
        fb::fill_circle((bx + 40) as f32, y as f32, 6.0, P_ACCENT);
        fb::fill_rect(bx + 38, y, 4, 40, P_DIM);
        bar(bx + 70, y + 2, 260 + (y % 200), P_TEXT);
        y += 40;
    }
}

fn draw_web(bx: i32, by: i32, bw: i32, _bh: i32, query: &str) {
    // barre d'adresse
    fb::fill_rect(bx + 20, by + 16, bw - 40, 40, P_TITLE_HI);
    font::draw_str_scaled(bx + 32, by + 24, "google.com/search?q=", P_DIM, 2);
    font::draw_str_scaled(
        bx + 32 + font::width_scaled("google.com/search?q=", 2),
        by + 24,
        query,
        P_TEXT,
        2,
    );
    // "résultats"
    font::draw_str_scaled(bx + 40, by + 90, "Environ 1 000 000 resultats", P_DIM, 2);
    let mut y = by + 140;
    for _ in 0..5 {
        bar(bx + 40, y, 520, P_ACCENT);
        bar(bx + 40, y + 22, 360, P_CODE_STR);
        bar(bx + 40, y + 44, 780, P_DIM);
        bar(bx + 40, y + 64, 700, P_DIM);
        y += 120;
    }
}

fn draw_files(bx: i32, by: i32, bw: i32, bh: i32, sel: &str) {
    font::draw_str_scaled(bx + 24, by + 16, "Documents", P_DIM, 2);
    let names = ["Rapport", "Photos", "notes.txt", "budget", "cv.pdf", "musique", "projet", "todo.md"];
    let cols = (bw - 40) / 150;
    for (i, n) in names.iter().enumerate() {
        let cx = bx + 24 + (i as i32 % cols) * 150;
        let cy = by + 60 + (i as i32 / cols) * 130;
        let hit = *n == sel || n.starts_with(sel) && !sel.is_empty();
        fb::fill_rect(cx, cy, 110, 84, if hit { P_ACCENT } else { P_TITLE_HI });
        font::draw_str_scaled(cx, cy + 92, n, if hit { P_TEXT } else { P_DIM }, 2);
    }
    let _ = bh;
}

fn draw_fileview(bx: i32, by: i32, bw: i32, bh: i32, name: &str) {
    // "page" blanche centrée
    let pw = (bw * 3 / 4).min(760);
    let px = bx + (bw - pw) / 2;
    fb::fill_rect(px, by + 20, pw, bh - 40, P_TEXT);
    font::draw_str_scaled(px + 30, by + 44, name, P_TITLE, 3);
    let mut y = by + 110;
    for i in 0..((bh - 160) / 24).min(16) {
        let w = if i % 4 == 3 { pw / 2 } else { pw - 80 };
        fb::fill_rect(px + 30, y, w, 8, P_DIM);
        y += 24;
    }
}

fn draw_hub(bx: i32, by: i32, bw: i32, bh: i32) {
    font::draw_str_dots(bx + (bw - 8 * 8 * 6) / 2, by + 24, "PC PET", P_TEXT, 6);
    font::draw_str_scaled(bx + 40, by + 150, "Compagnon   : Asti", P_TEXT, 2);
    font::draw_str_scaled(bx + 40, by + 190, "Nourriture  :", P_DIM, 2);
    let f = crate::home::food() as i32;
    fb::fill_rect(bx + 250, by + 190, 3 * f, 16, P_ACCENT);
    font::draw_str_scaled(bx + 40, by + 230, "Etat        : en forme", P_TEXT, 2);
    font::draw_str_scaled(bx + 40, by + 300, "Glisse une friandise sur lui pour", P_DIM, 2);
    font::draw_str_scaled(bx + 40, by + 326, "le nourrir. Il reste au-dessus", P_DIM, 2);
    font::draw_str_scaled(bx + 40, by + 352, "de toutes les fenetres.", P_DIM, 2);
    let _ = bh;
}
