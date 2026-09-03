//! Gestionnaire de fenêtres. Les applications sont **réelles** : éditeur
//! de texte (agit sur [`crate::fs`]), terminal (mini shell), gestionnaire
//! de fichiers, recherche locale, PC Pet Hub, calculatrice.
//!
//! Il n'y a pas de disque : tout vit en RAM et disparaît au redémarrage.

use crate::{dots, editor, fb, font, fs, term};

const MAX: usize = 6;
const TITLE_H: i32 = 30;

// palette fenêtres (indices 64..=90)
pub const P_FRAME: u8 = 64;
pub const P_TITLE: u8 = 65;
pub const P_TITLE_HI: u8 = 66;
pub const P_BODY: u8 = 67;
pub const P_TEXT: u8 = 68;
pub const P_DIM: u8 = 69;
pub const P_ACCENT: u8 = 70;
pub const P_CODE_BG: u8 = 71;
pub const P_STR: u8 = 73;
pub const P_CLOSE: u8 = 75;

pub fn install_palette() {
    fb::set_palette(P_FRAME, 64, 68, 88);
    fb::set_palette(P_TITLE, 30, 33, 44);
    fb::set_palette(P_TITLE_HI, 48, 52, 70);
    fb::set_palette(P_BODY, 24, 26, 34);
    fb::set_palette(P_TEXT, 220, 226, 240);
    fb::set_palette(P_DIM, 122, 128, 148);
    fb::set_palette(P_ACCENT, 120, 200, 255);
    fb::set_palette(P_CODE_BG, 16, 18, 26);
    fb::set_palette(P_STR, 150, 210, 150);
    fb::set_palette(P_CLOSE, 224, 92, 92);
}

#[derive(Clone, Copy, PartialEq)]
pub enum App {
    Editor,
    Terminal,
    Files,
    Web,
    Hub,
    Calc,
    Unknown,
}

#[derive(Clone, Copy)]
struct Win {
    app: App,
    title: [u8; 40],
    tlen: usize,
    arg: [u8; 96],
    alen: usize,
    slot: usize, // éditeur : index du fichier fs
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

/// Logo en points de l'application (style « friandise »).
pub fn app_glyph(app: App) -> &'static [&'static str] {
    match app {
        App::Editor => dots::EDITOR,
        App::Terminal => dots::TERMINAL,
        App::Files => dots::FOLDER,
        App::Web => dots::WEB,
        App::Hub => dots::HEART,
        App::Calc => dots::CALC,
        App::Unknown => dots::QUESTION,
    }
}

impl Win {
    const EMPTY: Win = Win {
        app: App::Unknown,
        title: [0; 40],
        tlen: 0,
        arg: [0; 96],
        alen: 0,
        slot: 0,
        x: 0,
        y: 0,
        w: 0,
        h: 0,
    };
    fn title(&self) -> &str {
        core::str::from_utf8(&self.title[..self.tlen]).unwrap_or("?")
    }
    fn arg(&self) -> &str {
        core::str::from_utf8(&self.arg[..self.alen]).unwrap_or("")
    }
}

fn cp(dst: &mut [u8], src: &[u8]) -> usize {
    let n = src.len().min(dst.len());
    dst[..n].copy_from_slice(&src[..n]);
    n
}

pub struct Manager {
    wins: [Win; MAX],
    order: [usize; MAX],
    n: usize,
    drag: Option<(usize, i32, i32)>,
    /// le focus clavier est-il sur une fenêtre (sinon : barre de commande) ?
    kb_on_win: bool,
    spawn_i: i32,
}

impl Manager {
    pub fn new() -> Manager {
        Manager {
            wins: [Win::EMPTY; MAX],
            order: [0, 1, 2, 3, 4, 5],
            n: 0,
            drag: None,
            kb_on_win: false,
            spawn_i: 0,
        }
    }

    fn alloc_slot(&mut self) -> usize {
        if self.n < MAX {
            let s = self.n;
            self.order[self.n] = s;
            self.n += 1;
            s
        } else {
            let s = self.order[0];
            for k in 0..MAX - 1 {
                self.order[k] = self.order[k + 1];
            }
            self.order[MAX - 1] = s;
            s
        }
    }

    pub fn spawn(&mut self, app: App, title: &[u8], arg: &[u8]) {
        let s = self.alloc_slot();
        let w = &mut self.wins[s];
        *w = Win::EMPTY;
        w.app = app;
        w.tlen = cp(&mut w.title, title);
        w.alen = cp(&mut w.arg, arg);

        match app {
            App::Editor => {
                // ouvre le fichier `arg` (ou en crée un "sans-titre")
                let name: &[u8] = if arg.is_empty() {
                    b"sans-titre.txt"
                } else {
                    arg
                };
                let fslot = fs::create(name).unwrap_or(0);
                w.slot = fslot;
                w.tlen = cp(&mut w.title, name);
                editor::attach(s, fslot);
            }
            App::Terminal => term::reset(),
            _ => {}
        }

        let (ww, wh) = match app {
            App::Hub => (620, 520),
            App::Calc => (360, 460),
            App::Web => (1000, 700),
            App::Terminal => (1000, 640),
            App::Editor => (1080, 720),
            _ => (900, 600),
        };
        let off = (self.spawn_i % 5) * 40;
        w.x = ((fb::WIDTH as i32 - ww) / 2 - 100 + off).max(20);
        w.y = ((fb::HEIGHT as i32 - wh) / 2 - 30 + off).max(10);
        w.w = ww;
        w.h = wh;
        self.spawn_i += 1;
        self.focus(self.n - 1);
        self.kb_on_win = matches!(app, App::Editor | App::Terminal | App::Calc);
    }

    fn focus(&mut self, i: usize) {
        if i + 1 >= self.n {
            return;
        }
        let s = self.order[i];
        for k in i..self.n - 1 {
            self.order[k] = self.order[k + 1];
        }
        self.order[self.n - 1] = s;
    }

    pub fn focused_app(&self) -> Option<App> {
        if self.n == 0 {
            None
        } else {
            Some(self.wins[self.order[self.n - 1]].app)
        }
    }

    /// Le clavier doit-il aller à la fenêtre (et non à la barre) ?
    pub fn wants_keys(&self) -> bool {
        self.kb_on_win
            && self.n > 0
            && matches!(
                self.wins[self.order[self.n - 1]].app,
                App::Editor | App::Terminal | App::Calc
            )
    }

    pub fn feed_key(&mut self, c: u8) {
        if self.n == 0 {
            return;
        }
        let s = self.order[self.n - 1];
        match self.wins[s].app {
            App::Editor => editor::key(s, c),
            App::Terminal => term::key(c),
            App::Calc => calc_key(s, c),
            _ => {}
        }
    }

    /// Clic dans la barre de commande : le clavier repart vers elle.
    pub fn blur(&mut self) {
        self.kb_on_win = false;
    }

    pub fn on_mouse(&mut self, mx: i32, my: i32, down: bool, pressed: bool) -> bool {
        if let Some((oi, dx, dy)) = self.drag {
            if down {
                let s = self.order[oi];
                self.wins[s].x = mx - dx;
                self.wins[s].y = my - dy;
            } else {
                self.drag = None;
            }
            return true;
        }
        if !pressed {
            return false;
        }
        for i in (0..self.n).rev() {
            let s = self.order[i];
            let w = self.wins[s];
            if mx >= w.x && mx < w.x + w.w && my >= w.y && my < w.y + w.h + TITLE_H {
                let cbx = w.x + w.w - 26;
                if my < w.y + TITLE_H && mx >= cbx {
                    for k in i..self.n - 1 {
                        self.order[k] = self.order[k + 1];
                    }
                    self.n -= 1;
                    return true;
                }
                self.focus(i);
                self.kb_on_win = matches!(
                    self.wins[s].app,
                    App::Editor | App::Terminal | App::Calc
                );
                if my < w.y + TITLE_H {
                    self.drag = Some((self.n - 1, mx - w.x, my - w.y));
                }
                return true;
            }
        }
        false
    }

    pub fn draw(&self, t: f32) {
        for i in 0..self.n {
            let s = self.order[i];
            let w = self.wins[s];
            let focused = i + 1 == self.n;
            draw_window(s, &w, focused && self.kb_on_win, t);
        }
    }
}

fn draw_window(slot: usize, w: &Win, kb: bool, t: f32) {
    fb::fill_rect(w.x + 6, w.y + 8, w.w, w.h + TITLE_H, 0);
    let fc = if kb { P_ACCENT } else { P_FRAME };
    fb::fill_rect(w.x - 1, w.y - 1, w.w + 2, w.h + TITLE_H + 2, fc);

    fb::fill_rect(w.x, w.y, w.w, TITLE_H, if kb { P_TITLE_HI } else { P_TITLE });
    dots::draw_centered(app_glyph(w.app), w.x + 8, w.y, 22, TITLE_H, 2, P_TEXT, P_DIM);
    font::draw_str_scaled(w.x + 38, w.y + 7, w.title(), P_TEXT, 2);
    fb::fill_rect(w.x + w.w - 24, w.y + 6, 18, 18, P_CLOSE);
    font::draw_str_scaled(w.x + w.w - 22, w.y + 6, "x", P_TEXT, 2);

    let (bx, by, bw, bh) = (w.x, w.y + TITLE_H, w.w, w.h);
    fb::fill_rect(bx, by, bw, bh, P_BODY);

    match w.app {
        App::Editor => editor::draw(slot, bx, by, bw, bh, kb, t),
        App::Terminal => term::draw(bx, by, bw, bh, t),
        App::Files => draw_files(bx, by, bw, bh),
        App::Web => draw_web(bx, by, bw, bh, w.arg()),
        App::Hub => draw_hub(bx, by, bw, bh),
        App::Calc => draw_calc(slot, bx, by, bw, bh),
        App::Unknown => {
            dots::draw_centered(dots::QUESTION, bx, by + 40, bw, 90, 7, P_DIM, P_TITLE_HI);
            font::draw_str_scaled(bx + 40, by + 160, w.arg(), P_TEXT, 3);
            font::draw_str_scaled(bx + 40, by + 220, "application non disponible.", P_DIM, 2);
            font::draw_str_scaled(bx + 40, by + 260, "essaie: /app editeur  /app terminal  /app calc", P_DIM, 2);
        }
    }
}

fn draw_files(bx: i32, by: i32, bw: i32, _bh: i32) {
    font::draw_str_scaled(bx + 24, by + 16, "Fichiers  (/fichier <nom> pour ouvrir)", P_DIM, 2);
    let cols = ((bw - 40) / 190).max(1);
    let mut i = 0i32;

    let tile = |name: &str, dir: bool, host: bool, i: &mut i32| {
        let cx = bx + 24 + (*i % cols) * 190;
        let cy = by + 64 + (*i / cols) * 120;
        let pat = if dir { dots::FOLDER } else { dots::FILE };
        dots::draw(pat, cx, cy, 5, P_TEXT, P_DIM);
        if host {
            // pastille = fichier du Mac (partage 9p)
            fb::fill_rect(cx + 46, cy + 2, 10, 10, P_STR);
        }
        // nom tronqué pour tenir dans la colonne (~13 caractères)
        let mut buf = [0u8; 16];
        let nb = name.as_bytes();
        let label: &str = if nb.len() <= 15 {
            name
        } else {
            buf[..13].copy_from_slice(&nb[..13]);
            buf[13] = b'.';
            buf[14] = b'.';
            core::str::from_utf8(&buf[..15]).unwrap_or(name)
        };
        font::draw_str_scaled(cx, cy + 78, label, if dir { P_TEXT } else { P_DIM }, 2);
        *i += 1;
    };

    if crate::hostfs::have_dir() {
        font::draw_str_scaled(bx + 24, by + 40, "~/Documents (Mac)", P_STR, 2);
        crate::hostfs::each_dir(|name, dir| tile(name, dir, true, &mut i));
        // aligne la suite sur une nouvelle rangée
        if i % cols != 0 {
            i += cols - (i % cols);
        }
    }
    fs::each(|_, f| tile(f.name(), f.is_dir(), false, &mut i));
}

fn draw_web(bx: i32, by: i32, bw: i32, _bh: i32, query: &str) {
    fb::fill_rect(bx + 20, by + 16, bw - 40, 38, P_TITLE_HI);
    font::draw_str_scaled(bx + 30, by + 24, "recherche locale (hors-ligne) :", P_DIM, 2);
    font::draw_str_scaled(
        bx + 30 + font::width_scaled("recherche locale (hors-ligne) :", 2),
        by + 24,
        query,
        P_TEXT,
        2,
    );
    let mut y = by + 80;
    let q = query.as_bytes();
    let mut hits = 0;
    fs::each(|_, f| {
        if f.is_dir() {
            return;
        }
        if contains_ci(f.content().as_bytes(), q) || contains_ci(f.name().as_bytes(), q) {
            hits += 1;
            font::draw_str_scaled(bx + 30, y, f.name(), P_ACCENT, 2);
            let c = f.content();
            let snip = if c.len() > 70 { &c[..70] } else { c };
            font::draw_str_scaled(bx + 30, y + 24, snip, P_DIM, 2);
            y += 66;
        }
    });
    if hits == 0 {
        font::draw_str_scaled(bx + 30, y, "aucun resultat local.", P_DIM, 2);
    }
}

fn contains_ci(hay: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || needle.len() > hay.len() {
        return false;
    }
    let lo = |c: u8| if c.is_ascii_uppercase() { c + 32 } else { c };
    'o: for i in 0..=hay.len() - needle.len() {
        for j in 0..needle.len() {
            if lo(hay[i + j]) != lo(needle[j]) {
                continue 'o;
            }
        }
        return true;
    }
    false
}

fn draw_hub(bx: i32, by: i32, bw: i32, _bh: i32) {
    font::draw_str_dots(bx + (bw - 6 * 8 * 6) / 2, by + 24, "PC PET", P_TEXT, 6);
    dots::draw_centered(dots::HEART, bx, by + 120, bw, 90, 8, P_ACCENT, P_TITLE_HI);
    font::draw_str_scaled(bx + 40, by + 236, "Compagnon : Asti", P_TEXT, 2);
    font::draw_str_scaled(bx + 40, by + 288, "Il reste au-dessus de toutes les", P_DIM, 2);
    font::draw_str_scaled(bx + 40, by + 314, "fenetres et suit l'appli active.", P_DIM, 2);
    font::draw_str_scaled(bx + 40, by + 366, "Glisse une friandise sur lui pour", P_DIM, 2);
    font::draw_str_scaled(bx + 40, by + 392, "le faire reagir.", P_DIM, 2);
}

// --- calculatrice ---

#[derive(Clone, Copy)]
struct Calc {
    acc: i64,
    cur: i64,
    op: u8,
    fresh: bool,
    used: bool,
}
const C0: Calc = Calc {
    acc: 0,
    cur: 0,
    op: 0,
    fresh: true,
    used: false,
};
static mut CALCS: [Calc; 6] = [C0; 6];

fn calc_key(slot: usize, c: u8) {
    let s = unsafe { &mut CALCS[slot] };
    s.used = true;
    match c {
        b'0'..=b'9' => {
            if s.fresh {
                s.cur = 0;
                s.fresh = false;
            }
            s.cur = s.cur.saturating_mul(10) + (c - b'0') as i64;
        }
        b'+' | b'-' | b'*' | b'/' => {
            calc_apply(s);
            s.op = c;
            s.fresh = true;
        }
        b'\n' | b'=' => {
            calc_apply(s);
            s.op = 0;
            s.acc = s.cur;
            s.fresh = true;
        }
        0x08 => {
            *s = C0;
            s.used = true;
        }
        _ => {}
    }
}

fn calc_apply(s: &mut Calc) {
    let (a, b) = (s.acc, s.cur);
    s.cur = match s.op {
        b'+' => a + b,
        b'-' => a - b,
        b'*' => a * b,
        b'/' => {
            if b != 0 {
                a / b
            } else {
                0
            }
        }
        _ => b,
    };
    s.acc = s.cur;
}

fn draw_calc(slot: usize, bx: i32, by: i32, bw: i32, _bh: i32) {
    let s = unsafe { CALCS[slot] };
    fb::fill_rect(bx + 20, by + 20, bw - 40, 70, P_CODE_BG);
    // affiche s.cur, aligné à droite
    let v = if s.fresh && s.op != 0 { s.acc } else { s.cur };
    let mut buf = [0u8; 22];
    let n = itoa(v, &mut buf);
    font::draw_str_scaled(bx + bw - 40 - n as i32 * 24, by + 36, core::str::from_utf8(&buf[..n]).unwrap_or("0"), P_TEXT, 3);
    font::draw_str_scaled(bx + 30, by + 110, "clavier : 0-9  + - * /  Entree  Ret.arr=clear", P_DIM, 2);
}

fn itoa(mut v: i64, buf: &mut [u8]) -> usize {
    let neg = v < 0;
    if neg {
        v = -v;
    }
    let mut tmp = [0u8; 22];
    let mut n = 0;
    loop {
        tmp[n] = b'0' + (v % 10) as u8;
        v /= 10;
        n += 1;
        if v == 0 {
            break;
        }
    }
    let mut o = 0;
    if neg {
        buf[0] = b'-';
        o = 1;
    }
    for i in 0..n {
        buf[o + i] = tmp[n - 1 - i];
    }
    o + n
}
