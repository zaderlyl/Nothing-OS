//! Lanceur d'applications. `/app` (sans argument) fait glisser un panneau
//! avec la liste ; un clic **ouvre la vraie application du Mac par-dessus
//! Nothing OS** (pas d'affichage embarqué, pas de bidouille).
//!
//! Le noyau écrit le nom demandé dans `<partage>/.nothingos-open` ; le
//! petit script `bridge/opener.sh` tourne sur le Mac, le lit et fait
//! `open -a <App>`. Asti (compagnon macOS) reste au premier plan.

#![allow(dead_code, static_mut_refs)]

use crate::asti;
use crate::win::{P_DIM, P_TEXT};
use crate::{dots, fb, font};

const W: i32 = fb::WIDTH as i32;
const H: i32 = fb::HEIGHT as i32;

const OPEN_PATH: &str = ".nothingos-open";

// palette dédiée (indices libres 41..=49)
const A_BG: u8 = 41;
const A_PANEL: u8 = 42;
const A_LINE: u8 = 43;
const A_ACC: u8 = 44;
const A_MSG: u8 = 45;

pub fn install_palette() {
    fb::set_palette(A_BG, 22, 23, 30);
    fb::set_palette(A_PANEL, 30, 32, 42);
    fb::set_palette(A_LINE, 52, 55, 70);
    fb::set_palette(A_MSG, 40, 43, 56);
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

impl App {
    fn token(self) -> &'static str {
        match self {
            App::VsCode => "vscode",
            App::Affinity => "affinity",
            App::Discord => "discord",
            App::None => "",
        }
    }
    fn label(self) -> &'static str {
        match self {
            App::VsCode => "VS Code",
            App::Affinity => "Affinity",
            App::Discord => "Discord",
            App::None => "",
        }
    }
}

struct Item {
    app: App,
    name: &'static str,
    desc: &'static str,
    glyph: &'static [&'static str],
}

const ITEMS: [Item; 3] = [
    Item { app: App::VsCode, name: "VS Code", desc: "editeur de code", glyph: dots::CODE },
    Item { app: App::Affinity, name: "Affinity", desc: "dessin / design", glyph: dots::PALETTE },
    Item { app: App::Discord, name: "Discord", desc: "messagerie", glyph: dots::CHAT },
];

static mut LAUNCH_ON: bool = false; // panneau demandé
static mut LAUNCH_OUT: f32 = 0.0; // position animée du panneau (0..1)
static mut LAST: App = App::None; // dernière appli ouverte (pour le carton)
static mut LAST_AT: f32 = -100.0; // instant de l'ouverture
static mut NOW: f32 = 0.0;
static mut SEQ: u32 = 0; // fait varier le fichier pour ré-ouvrir la même appli

const TOAST: f32 = 3.5; // durée d'affichage du carton (s)

// --- API ------------------------------------------------------------

/// Quelque chose d'`/app` est à l'écran (panneau ou carton) → Asti réagit,
/// et un clic/Échap doit être capté ici en priorité.
pub fn active() -> bool {
    unsafe { LAUNCH_ON || LAUNCH_OUT > 0.01 || NOW - LAST_AT < TOAST }
}

/// Plus jamais d'appli « plein écran » embarquée.
pub fn running() -> bool {
    false
}

/// Ouvre le panneau de choix des applications.
pub fn open_launcher() {
    unsafe { LAUNCH_ON = true }
}

/// `/app discord` — lance directement. `false` si le nom est inconnu.
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
        LAUNCH_ON = false;
        LAST = app;
        LAST_AT = NOW;
        SEQ = SEQ.wrapping_add(1);
        // "<token>\n<seq>\n" : le \n<seq> force un changement de contenu
        // pour ré-ouvrir la même appli deux fois de suite.
        let mut buf = alloc::vec::Vec::with_capacity(24);
        buf.extend_from_slice(app.token().as_bytes());
        buf.push(b'\n');
        let mut n = SEQ;
        let mut d = [0u8; 10];
        let mut i = d.len();
        loop {
            i -= 1;
            d[i] = b'0' + (n % 10) as u8;
            n /= 10;
            if n == 0 {
                break;
            }
        }
        buf.extend_from_slice(&d[i..]);
        buf.push(b'\n');
        let ok = crate::p9::write_file(OPEN_PATH, &buf);
        crate::serial_println!(
            "[apps] ouvre {} (seq {}) {}",
            app.label(),
            SEQ,
            if ok { "->" } else { "ECHEC 9p" }
        );
    }
    set_accent(app);
}

/// Ferme le panneau / le carton.
pub fn close() {
    unsafe {
        LAUNCH_ON = false;
        LAST_AT = -100.0;
    }
    set_accent(App::None);
}

pub fn mood() -> (Option<asti::Pose>, asti::Tint) {
    match unsafe { LAST } {
        App::VsCode => (Some(asti::Pose::AppCode), asti::Tint::Code),
        App::Affinity => (Some(asti::Pose::AppArt), asti::Tint::Web),
        App::Discord => (Some(asti::Pose::AppChat), asti::Tint::Chat),
        App::None => (Some(asti::Pose::AppGit), asti::Tint::Git),
    }
}

pub fn update(now: f32, dt: f32) {
    unsafe {
        NOW = now;
        let target = if LAUNCH_ON { 1.0 } else { 0.0 };
        LAUNCH_OUT += (target - LAUNCH_OUT) * (1.0 - libm::powf(0.5, dt * 12.0));
        LAUNCH_OUT = LAUNCH_OUT.clamp(0.0, 1.0);
    }
}

pub fn on_click(mx: i32, my: i32) -> bool {
    unsafe {
        if LAUNCH_OUT > 0.5 {
            let px = W - PANEL_W;
            if mx >= px {
                let i = (my - LIST_Y) / ROW_H;
                if i >= 0 && (i as usize) < ITEMS.len() {
                    launch(ITEMS[i as usize].app);
                }
                return true;
            }
            LAUNCH_ON = false; // clic hors panneau → on referme
            return true;
        }
        // carton visible : un clic dessus le fait disparaître
        if NOW - LAST_AT < TOAST {
            LAST_AT = -100.0;
            return true;
        }
        false
    }
}

// --- rendu --------------------------------------------------------

const PANEL_W: i32 = 520;
const ROW_H: i32 = 100;
const LIST_Y: i32 = 548;

pub fn draw(now: f32) {
    unsafe {
        if LAUNCH_OUT > 0.01 {
            draw_launcher(LAUNCH_OUT);
        } else if now - LAST_AT < TOAST {
            draw_toast(now);
        }
    }
}

fn draw_launcher(out: f32) {
    let x = W - (PANEL_W as f32 * out) as i32;
    fb::fill_rect(x, 0, PANEL_W, H, A_PANEL);
    fb::fill_rect(x, 0, 3, H, A_ACC);
    font::draw_str_scaled(x + 40, LIST_Y - 74, "APPLICATIONS", P_TEXT, 3);
    font::draw_str_scaled(x + 40, LIST_Y - 34, "s'ouvre par-dessus l'OS", P_DIM, 2);

    for (i, it) in ITEMS.iter().enumerate() {
        let ry = LIST_Y + i as i32 * ROW_H;
        dots::draw(it.glyph, x + 40, ry + 18, 4, A_ACC, P_DIM);
        font::draw_str_scaled(x + 130, ry + 14, it.name, P_TEXT, 3);
        font::draw_str_scaled(x + 130, ry + 54, it.desc, P_DIM, 2);
        fb::fill_rect(x + 40, ry + ROW_H - 10, PANEL_W - 80, 1, A_LINE);
    }
    font::draw_str_scaled(x + 40, H - 56, "Echap : fermer", P_DIM, 2);
}

fn draw_toast(now: f32) {
    let age = now - unsafe { LAST_AT };
    // apparition/disparition douce
    let a = (age * 4.0).min(1.0).min(((TOAST - age) * 3.0).max(0.0));
    if a <= 0.02 {
        return;
    }
    let app = unsafe { LAST };
    let cw = 560;
    let ch = 150;
    let cx = (W - cw) / 2;
    let cy = (H - ch) / 3 + ((1.0 - a) * 20.0) as i32;

    fb::fill_rect(cx - 2, cy - 2, cw + 4, ch + 4, A_LINE);
    fb::fill_rect(cx, cy, cw, ch, A_PANEL);
    fb::fill_rect(cx, cy, 4, ch, A_ACC);

    dots::draw(
        match app {
            App::VsCode => dots::CODE,
            App::Affinity => dots::PALETTE,
            _ => dots::CHAT,
        },
        cx + 36,
        cy + 34,
        5,
        A_ACC,
        A_MSG,
    );
    font::draw_str_scaled(cx + 150, cy + 30, app.label(), P_TEXT, 3);
    font::draw_str_scaled(cx + 150, cy + 74, "ouvert par-dessus l'OS", P_DIM, 2);
    font::draw_str_scaled(cx + 150, cy + 104, "Cmd+Tab pour revenir ici", P_DIM, 2);
}
