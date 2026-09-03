//! Applications « plein écran ». `/app` (sans argument) fait glisser un
//! panneau avec la liste ; un clic ouvre l'appli en plein écran, Asti
//! restant au premier plan avec l'humeur correspondante.
//!
//! Pour l'instant : VS Code (aperçu de code), Affinity (planche de
//! dessin), Discord (chat qui fonctionne : on tape, ça s'affiche).

#![allow(dead_code, static_mut_refs)]

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::asti;
use crate::win::{P_DIM, P_TEXT};
use crate::{dots, fb, font};

const W: i32 = fb::WIDTH as i32;
const H: i32 = fb::HEIGHT as i32;

// palette dédiée (indices libres 41..=48)
const A_BG: u8 = 41;
const A_PANEL: u8 = 42;
const A_LINE: u8 = 43;
const A_ACC: u8 = 44;
const A_MSG: u8 = 45;
const A_KW: u8 = 46;
const A_STR: u8 = 47;
const A_CMT: u8 = 48;
const A_PAPER: u8 = 49;

pub fn install_palette() {
    fb::set_palette(A_BG, 22, 23, 30);
    fb::set_palette(A_PANEL, 30, 32, 42);
    fb::set_palette(A_LINE, 52, 55, 70);
    fb::set_palette(A_MSG, 40, 43, 56);
    fb::set_palette(A_KW, 120, 170, 255);
    fb::set_palette(A_STR, 150, 210, 150);
    fb::set_palette(A_CMT, 110, 116, 135);
    fb::set_palette(A_PAPER, 240, 240, 245);
    set_accent(App::None);
}

fn set_accent(app: App) {
    let (r, g, b) = match app {
        App::VsCode => (0, 122, 204),
        App::Affinity => (150, 225, 130),
        App::Discord => (88, 101, 242),
        App::None => (120, 200, 255),
    };
    fb::set_palette(A_ACC, r, g, b);
}

#[derive(Clone, Copy, PartialEq)]
pub enum App {
    None,
    VsCode,
    Affinity,
    Discord,
}

struct Item {
    app: App,
    name: &'static str,
    desc: &'static str,
    glyph: &'static [&'static str],
}

const ITEMS: [Item; 3] = [
    Item {
        app: App::VsCode,
        name: "VS Code",
        desc: "editeur de code",
        glyph: dots::CODE,
    },
    Item {
        app: App::Affinity,
        name: "Affinity",
        desc: "dessin / design",
        glyph: dots::PALETTE,
    },
    Item {
        app: App::Discord,
        name: "Discord",
        desc: "messagerie",
        glyph: dots::CHAT,
    },
];

static mut RUNNING: App = App::None;
static mut LAUNCH_ON: bool = false;
static mut LAUNCH_OUT: f32 = 0.0;

// --- Discord ---------------------------------------------------------
struct Chan {
    name: &'static str,
    msgs: Vec<(String, String)>, // (auteur, texte)
}
static mut CHANS: Vec<Chan> = Vec::new();
static mut CHAN_I: usize = 0;
static mut INPUT: String = String::new();

fn discord_init() {
    unsafe {
        if !CHANS.is_empty() {
            return;
        }
        let mk = |name, seed: &[(&str, &str)]| {
            let msgs = seed
                .iter()
                .map(|(a, t)| (a.to_string(), t.to_string()))
                .collect();
            CHANS.push(Chan { name, msgs });
        };
        mk(
            "general",
            &[
                ("Lea", "salut ! quelqu'un a test le nouveau build ?"),
                ("Sam", "oui ca boot direct en PVH maintenant"),
                ("Lea", "nickel"),
            ],
        );
        mk(
            "dev",
            &[
                ("Sam", "le pilote AC97 sort du son, enfin"),
                ("toi", "faut brancher le mp3 dessus"),
                ("Sam", "minimp3 compile en cross, ca passe"),
            ],
        );
        mk("random", &[("Lea", "asti est trop mignon quand il ecoute de la musique")]);
    }
}

// --- API ------------------------------------------------------------

pub fn active() -> bool {
    unsafe { RUNNING != App::None || LAUNCH_ON || LAUNCH_OUT > 0.01 }
}

pub fn running() -> bool {
    unsafe { RUNNING != App::None }
}

/// Ouvre le panneau de choix des applications.
pub fn open_launcher() {
    unsafe {
        LAUNCH_ON = true;
    }
}

/// Lance directement une appli par son nom (`/app discord`). Renvoie
/// `false` si le nom ne correspond à aucune appli plein écran (l'appelant
/// peut alors retomber sur son ancien comportement).
pub fn launch_named(name: &[u8]) -> bool {
    let has = |t: &[u8]| name.windows(t.len()).any(|w| w.eq_ignore_ascii_case(t));
    let app = if has(b"vscode") || has(b"code") {
        App::VsCode
    } else if has(b"affinity") || has(b"design") || has(b"dessin") {
        App::Affinity
    } else if has(b"discord") {
        App::Discord
    } else {
        return false;
    };
    launch(app);
    true
}

fn launch(app: App) {
    unsafe {
        RUNNING = app;
        LAUNCH_ON = false;
    }
    set_accent(app);
    if app == App::Discord {
        discord_init();
    }
    crate::serial_println!("[apps] lancement {:?}", app as u8);
}

/// Ferme l'appli en cours ; si aucune, ferme le panneau.
pub fn close() {
    unsafe {
        if RUNNING != App::None {
            RUNNING = App::None;
            LAUNCH_ON = true; // on retombe sur la liste
        } else {
            LAUNCH_ON = false;
        }
    }
    set_accent(App::None);
}

pub fn mood() -> (Option<asti::Pose>, asti::Tint) {
    match unsafe { RUNNING } {
        App::VsCode => (Some(asti::Pose::AppCode), asti::Tint::Code),
        App::Affinity => (Some(asti::Pose::AppArt), asti::Tint::Web),
        App::Discord => (Some(asti::Pose::AppChat), asti::Tint::Chat),
        App::None => (Some(asti::Pose::AppGit), asti::Tint::Git),
    }
}

pub fn update(dt: f32) {
    unsafe {
        let target = if LAUNCH_ON { 1.0 } else { 0.0 };
        LAUNCH_OUT += (target - LAUNCH_OUT) * (1.0 - libm::powf(0.5, dt * 12.0));
        LAUNCH_OUT = LAUNCH_OUT.clamp(0.0, 1.0);
    }
}

/// Touche pour l'appli en cours (Discord : saisie). Renvoie `true` si
/// consommé.
pub fn on_key(c: u8) -> bool {
    unsafe {
        if RUNNING == App::Discord {
            match c {
                b'\n' => {
                    let txt = INPUT.trim().to_string();
                    if !txt.is_empty() && !CHANS.is_empty() {
                        CHANS[CHAN_I].msgs.push(("toi".to_string(), txt));
                    }
                    INPUT.clear();
                }
                0x08 => {
                    INPUT.pop();
                }
                0x20..=0x7e => {
                    if INPUT.len() < 180 {
                        INPUT.push(c as char);
                    }
                }
                _ => {}
            }
            return true;
        }
        RUNNING != App::None
    }
}

pub fn on_click(mx: i32, my: i32) -> bool {
    unsafe {
        // panneau de choix visible
        if LAUNCH_OUT > 0.5 {
            let px = W - PANEL_W;
            if mx >= px {
                let i = (my - LIST_Y) / ROW_H;
                if i >= 0 && (i as usize) < ITEMS.len() {
                    launch(ITEMS[i as usize].app);
                }
                return true;
            }
            // clic hors panneau → on ferme la liste
            LAUNCH_ON = false;
            return true;
        }
        if RUNNING == App::Discord {
            // barre latérale : changer de salon
            if mx < DC_SIDE {
                let i = (my - 96) / 52;
                if i >= 0 && (i as usize) < CHANS.len() {
                    CHAN_I = i as usize;
                }
            }
            return true;
        }
        RUNNING != App::None
    }
}

// --- rendu --------------------------------------------------------

const PANEL_W: i32 = 520;
const ROW_H: i32 = 100;
const LIST_Y: i32 = 548; // sous Asti (coin haut-droite)

pub fn draw(now: f32) {
    unsafe {
        match RUNNING {
            App::VsCode => draw_vscode(),
            App::Affinity => draw_affinity(now),
            App::Discord => draw_discord(now),
            App::None => {}
        }
        if LAUNCH_OUT > 0.01 {
            draw_launcher(LAUNCH_OUT);
        }
    }
}

fn draw_launcher(out: f32) {
    let x = W - (PANEL_W as f32 * out) as i32;
    fb::fill_rect(x, 0, PANEL_W, H, A_PANEL);
    fb::fill_rect(x, 0, 3, H, A_ACC);
    font::draw_str_scaled(x + 40, LIST_Y - 74, "APPLICATIONS", P_TEXT, 3);
    font::draw_str_scaled(x + 40, LIST_Y - 34, "choisis une application", P_DIM, 2);

    for (i, it) in ITEMS.iter().enumerate() {
        let ry = LIST_Y + i as i32 * ROW_H;
        dots::draw(it.glyph, x + 40, ry + 18, 4, A_ACC, P_DIM);
        font::draw_str_scaled(x + 130, ry + 14, it.name, P_TEXT, 3);
        font::draw_str_scaled(x + 130, ry + 54, it.desc, P_DIM, 2);
        fb::fill_rect(x + 40, ry + ROW_H - 10, PANEL_W - 80, 1, A_LINE);
    }
    font::draw_str_scaled(x + 40, H - 56, "Echap : fermer", P_DIM, 2);
}

// --- VS Code (aperçu de code) ---
const CODE_SAMPLE: &[&str] = &[
    "// src/main.rs - Nothing OS",
    "#![no_std]",
    "",
    "fn rust_main() -> ! {",
    "    gdt::init();",
    "    interrupts::init();",
    "    heap::init();          // 512 Mio",
    "    let brain = Brain::new(seed());",
    "    home::run(brain)       // ne rend jamais la main",
    "}",
    "",
    "// Asti reste au-dessus de tout,",
    "// et prend l'humeur de l'appli active.",
];

fn draw_vscode() {
    fb::fill_rect(0, 0, W, H, A_BG);
    // barre d'activité + explorateur
    fb::fill_rect(0, 0, 60, H, A_PANEL);
    fb::fill_rect(60, 0, 320, H, A_PANEL);
    fb::fill_rect(60, 0, 1, H, A_LINE);
    fb::fill_rect(380, 0, 1, H, A_LINE);
    font::draw_str_scaled(84, 24, "EXPLORATEUR", P_DIM, 2);
    for (i, f) in ["src/", "  main.rs", "  home.rs", "  asti.rs", "  apps.rs", "Cargo.toml", "README.md"]
        .iter()
        .enumerate()
    {
        let c = if *f == "  apps.rs" { P_TEXT } else { P_DIM };
        font::draw_str_scaled(84, 70 + i as i32 * 34, f, c, 2);
    }
    // onglet
    fb::fill_rect(380, 0, 220, 40, A_BG);
    fb::fill_rect(380, 0, 220, 3, A_ACC);
    font::draw_str_scaled(400, 10, "main.rs", P_TEXT, 2);

    // code
    let mut y = 70;
    for (n, line) in CODE_SAMPLE.iter().enumerate() {
        font::draw_num(410, y, (n + 1) as u32, 3, A_CMT, 2);
        draw_code_line(470, y, line);
        y += 34;
    }
    // barre d'état
    fb::fill_rect(0, H - 34, W, 34, A_ACC);
    font::draw_str_scaled(16, H - 27, "Rust   UTF-8   LF   Ln 6, Col 24", A_BG, 2);
}

fn draw_code_line(x: i32, y: i32, line: &str) {
    if line.trim_start().starts_with("//") {
        font::draw_str_scaled(x, y, line, A_CMT, 2);
        return;
    }
    let kws = [
        "fn", "let", "mut", "return", "use", "mod", "pub", "struct", "impl", "const",
    ];
    let mut cx = x;
    for tok in line.split_inclusive(' ') {
        let w = tok.trim();
        let col = if kws.contains(&w) {
            A_KW
        } else if w.starts_with('"') || w.ends_with('"') {
            A_STR
        } else {
            P_TEXT
        };
        font::draw_str_scaled(cx, y, tok, col, 2);
        cx += font::width_scaled(tok, 2);
    }
}

// --- Affinity (planche de dessin) ---
fn draw_affinity(now: f32) {
    fb::fill_rect(0, 0, W, H, A_BG);
    // barre d'outils
    fb::fill_rect(0, 0, 72, H, A_PANEL);
    for i in 0..7 {
        let sy = 40 + i * 84;
        let on = i == 1;
        fb::fill_rect(14, sy, 44, 44, if on { A_ACC } else { A_LINE });
        fb::fill_rect(20, sy + 6, 32, 32, A_PANEL);
    }
    // panneau calques
    fb::fill_rect(W - 300, 0, 300, H, A_PANEL);
    fb::fill_rect(W - 300, 0, 1, H, A_LINE);
    font::draw_str_scaled(W - 276, 24, "CALQUES", P_DIM, 2);
    for (i, l) in ["Fond", "Formes", "Titre", "Retouches"].iter().enumerate() {
        let ly = 66 + i as i32 * 40;
        if i == 1 {
            fb::fill_rect(W - 300, ly - 6, 300, 36, A_MSG);
        }
        font::draw_str_scaled(W - 268, ly, l, P_TEXT, 2);
    }

    // planche
    let (bx, by, bw, bh) = (200, 120, W - 200 - 380, H - 240);
    fb::fill_rect(bx - 2, by - 2, bw + 4, bh + 4, A_LINE);
    fb::fill_rect(bx, by, bw, bh, A_PAPER);
    // "oeuvre"
    fb::fill_circle((bx + bw / 2) as f32, (by + bh / 2) as f32, 150.0, A_ACC);
    fb::fill_rect(bx + 120, by + 100, 240, 160, A_KW);
    let bob = (libm::sinf(now * 2.0) * 10.0) as i32;
    fb::fill_circle((bx + bw - 200) as f32, (by + 160 + bob) as f32, 70.0, A_STR);
    font::draw_str_dots(bx + 90, by + bh - 130, "ASTI", A_CMT, 9);

    font::draw_str_scaled(200, 60, "planche.afdesign  -  1920 x 1080", P_DIM, 2);
}

// --- Discord ---
const DC_SIDE: i32 = 340;
const DC_INPUT_H: i32 = 64;

fn draw_discord(now: f32) {
    fb::fill_rect(0, 0, W, H, A_BG);
    // barre serveurs
    fb::fill_rect(0, 0, 84, H, A_PANEL);
    fb::fill_circle(42.0, 46.0, 26.0, A_ACC);
    font::draw_str_scaled(30, 36, "N", P_TEXT, 3);

    // liste des salons
    fb::fill_rect(84, 0, DC_SIDE - 84, H, A_PANEL);
    fb::fill_rect(84, 0, DC_SIDE - 84, 60, A_LINE);
    font::draw_str_scaled(108, 20, "Nothing OS", P_TEXT, 2);
    unsafe {
        for (i, ch) in CHANS.iter().enumerate() {
            let y = 96 + i as i32 * 52;
            if i == CHAN_I {
                fb::fill_rect(96, y - 8, DC_SIDE - 84 - 24, 40, A_MSG);
            }
            let c = if i == CHAN_I { P_TEXT } else { P_DIM };
            font::draw_str_scaled(112, y, "#", A_ACC, 2);
            font::draw_str_scaled(140, y, ch.name, c, 2);
        }
    }

    // en-tête du salon
    let name = unsafe { CHANS.get(CHAN_I).map(|c| c.name).unwrap_or("") };
    fb::fill_rect(DC_SIDE, 0, W - DC_SIDE, 56, A_BG);
    fb::fill_rect(DC_SIDE, 56, W - DC_SIDE, 1, A_LINE);
    font::draw_str_scaled(DC_SIDE + 24, 16, "#", A_ACC, 3);
    font::draw_str_scaled(DC_SIDE + 52, 16, name, P_TEXT, 3);

    // messages (les plus récents en bas)
    let area_bot = H - DC_INPUT_H - 20;
    let mut y = area_bot;
    unsafe {
        if let Some(ch) = CHANS.get(CHAN_I) {
            for (author, text) in ch.msgs.iter().rev() {
                let lines = 1 + text.len() as i32 / 60;
                y -= 30 + lines * 26;
                if y < 76 {
                    break;
                }
                let ac = if author == "toi" { A_ACC } else { A_KW };
                font::draw_str_scaled(DC_SIDE + 28, y, author, ac, 2);
                wrap_text(DC_SIDE + 28, y + 26, W - DC_SIDE - 56, text, P_TEXT);
            }
        }
    }

    // zone de saisie
    let iy = H - DC_INPUT_H;
    fb::fill_rect(DC_SIDE + 20, iy, W - DC_SIDE - 40, DC_INPUT_H - 14, A_MSG);
    let txt = unsafe { INPUT.as_str() };
    if txt.is_empty() {
        font::draw_str_scaled(DC_SIDE + 40, iy + 14, "message #...", P_DIM, 2);
    } else {
        font::draw_str_scaled(DC_SIDE + 40, iy + 14, txt, P_TEXT, 2);
    }
    if (now * 2.0) as i32 % 2 == 0 {
        let cx = DC_SIDE + 40 + font::width_scaled(txt, 2) + 3;
        fb::fill_rect(cx, iy + 10, 3, 28, P_TEXT);
    }
    font::draw_str_scaled(24, H - 26, "Echap : fermer", P_DIM, 2);
}

fn wrap_text(x: i32, y: i32, w: i32, s: &str, col: u8) {
    let cols = (w / 16).max(1) as usize;
    let mut cx = x;
    let mut cy = y;
    let mut n = 0;
    for &b in s.as_bytes() {
        if (0x20..=0x7e).contains(&b) {
            font::draw_char_scaled(cx, cy, b, col, None, 2);
            cx += 16;
            n += 1;
            if n >= cols {
                cx = x;
                cy += 26;
                n = 0;
            }
        }
    }
}
