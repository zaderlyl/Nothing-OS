//! Le "bureau" de Nothing OS.
//!
//! Fond noir uni. Une barre à gauche : en haut la liste des tâches à
//! faire, en bas un résumé (mail, agenda, système) et l'heure. Asti est
//! calé en haut à droite, toujours visible ; seule son **étagère de
//! friandises** se cache et se déplie au survol.
//!
//! Optimisation : la barre latérale (statique, sauf l'horloge) n'est
//! redessinée que quand c'est nécessaire ; chaque image ne repeint que
//! la zone à droite de la barre.

#![allow(dead_code)]

use core::sync::atomic::{AtomicU8, Ordering};

use crate::{asti, fb, font, mouse, rtc, shelf, time};

static FOOD: AtomicU8 = AtomicU8::new(100);

pub fn food() -> u8 {
    FOOD.load(Ordering::Relaxed)
}
pub fn set_food(value: u8) {
    FOOD.store(value.min(100), Ordering::Relaxed);
}
pub fn feed(amount: u8) {
    set_food(food().saturating_add(amount));
}
pub fn starve(amount: u8) {
    FOOD.store(food().saturating_sub(amount), Ordering::Relaxed);
}

// --- palette du bureau (indices 6..=15) ---
const PAL_SIDE: u8 = 6;
const PAL_SIDE_EDGE: u8 = 7;
const PAL_HEADER: u8 = 8;
const PAL_TEXT: u8 = 9;
const PAL_TEXT_DIM: u8 = 10;
const PAL_CURSOR: u8 = 11;
const PAL_CURSOR_EDGE: u8 = 12;
const PAL_ACCENT: u8 = 13;

pub fn install_palette() {
    fb::set_palette(PAL_SIDE, 14, 15, 20);
    fb::set_palette(PAL_SIDE_EDGE, 40, 42, 54);
    fb::set_palette(PAL_HEADER, 118, 128, 158);
    fb::set_palette(PAL_TEXT, 212, 218, 232);
    fb::set_palette(PAL_TEXT_DIM, 118, 124, 143);
    fb::set_palette(PAL_CURSOR, 245, 246, 250);
    fb::set_palette(PAL_CURSOR_EDGE, 20, 22, 30);
    fb::set_palette(PAL_ACCENT, 120, 200, 255);
}

const W: i32 = fb::WIDTH as i32;
const H: i32 = fb::HEIGHT as i32;
const SIDE_W: i32 = 500;
const PAD: i32 = 28;

// Contenu (placeholders : pas encore de persistance ni de vraies sources).
const TASKS: [(&str, bool); 6] = [
    ("Finir le pilote clavier", false),
    ("Timer PIT (faim d'Asti)", false),
    ("Jauge de nourriture", false),
    ("Ranger le bureau", true),
    ("Repondre a Lea", false),
    ("Backup du depot", true),
];

const INFOS: [(&str, &str); 4] = [
    ("Mail", "3 non lus"),
    ("Agenda", "14h00 Reunion"),
    ("Batterie", "82%"),
    ("Systeme", "OK"),
];

static mut SIDEBAR_DIRTY: bool = true;
static mut LAST_MIN: u8 = 255;

fn draw_sidebar() {
    fb::fill_rect(0, 0, SIDE_W, H, PAL_SIDE);
    fb::fill_rect(SIDE_W, 0, 2, H, PAL_SIDE_EDGE);
    fb::fill_rect(0, 0, SIDE_W, 3, PAL_ACCENT);

    // --- A FAIRE ---
    let mut y = PAD + 8;
    font::draw_str_scaled(PAD, y, "A FAIRE", PAL_HEADER, 2);
    y += 46;
    for (label, done) in TASKS {
        let txt_c = if done { PAL_TEXT_DIM } else { PAL_TEXT };
        fb::fill_rect(PAD, y, 18, 18, PAL_SIDE_EDGE);
        fb::fill_rect(PAD + 2, y + 2, 14, 14, PAL_SIDE);
        if done {
            fb::fill_rect(PAD + 4, y + 4, 10, 10, PAL_ACCENT);
        }
        font::draw_str_scaled(PAD + 32, y, label, txt_c, 2);
        y += 40;
    }

    y += 20;
    fb::fill_rect(PAD, y, SIDE_W - 2 * PAD, 1, PAL_SIDE_EDGE);
    y += 26;

    // --- INFOS ---
    font::draw_str_scaled(PAD, y, "INFOS", PAL_HEADER, 2);
    y += 46;
    for (k, v) in INFOS {
        font::draw_str_scaled(PAD, y, k, PAL_TEXT_DIM, 2);
        font::draw_str_scaled(PAD + 176, y, v, PAL_TEXT, 2);
        y += 40;
    }

    // --- horloge en bas ---
    let t = rtc::now();
    let cy = H - 100;
    font::draw_num(PAD + 4, cy, t.hour as u32, 2, PAL_TEXT, 5);
    fb::fill_rect(PAD + 4 + 88, cy + 16, 8, 8, PAL_TEXT);
    fb::fill_rect(PAD + 4 + 88, cy + 46, 8, 8, PAL_TEXT);
    font::draw_num(PAD + 4 + 108, cy, t.min as u32, 2, PAL_TEXT, 5);
}

/// Redessine la barre si nécessaire (1ʳᵉ image, changement de minute, ou
/// curseur qui la survole → il faut effacer sa trace).
fn refresh_sidebar(cursor_on_side: bool) {
    let m = rtc::now().min;
    let need = unsafe { SIDEBAR_DIRTY || m != LAST_MIN } || cursor_on_side;
    if !need {
        return;
    }
    unsafe {
        SIDEBAR_DIRTY = false;
        LAST_MIN = m;
    }
    draw_sidebar();
}

/// Boucle du bureau. Ne rend jamais la main.
pub fn run(mut brain: asti::Brain) -> ! {
    mouse::init();
    shelf::init();

    let mut shelf_out = 0.0_f32;
    let mut leave_at = -10.0_f32;
    let mut last = time::now_secs();
    let mut click_latch = false;
    let mut prev_mx = 0i32;

    loop {
        let now = time::now_secs();
        let dt = (now - last).clamp(0.0, 0.1);
        last = now;

        mouse::poll();
        let m = mouse::state();

        let (dcx, dcy) = asti::disc_center(asti::HOME_OX);
        let rad = asti::disc_radius();
        let over_asti = {
            let (dx, dy) = (m.x as f32 - dcx, m.y as f32 - dcy);
            dx * dx + dy * dy < (rad + 8.0) * (rad + 8.0)
        };
        let over_shelf = shelf_out > 0.3 && shelf::hit(m.x, m.y).is_some();
        if over_asti || over_shelf {
            leave_at = now;
        }
        let want = if now - leave_at < 0.5 { 1.0 } else { 0.0 };
        shelf_out += (want - shelf_out) * (1.0 - libm::powf(0.5, dt * 8.0));
        shelf_out = shelf_out.clamp(0.0, 1.0);

        if m.left && !click_latch && shelf_out > 0.6 {
            if let Some(kind) = shelf::hit(m.x, m.y) {
                if shelf::take(kind, now) {
                    feed(kind.boost());
                    brain.react_feed(now);
                }
            }
        }
        click_latch = m.left;

        let state = brain.update(now);
        let mut cv = asti::Canvas::new();
        asti::draw_creature(&mut cv, &state, now);

        // repeint uniquement la zone à droite de la barre
        fb::fill_rect(SIDE_W + 2, 0, W - SIDE_W - 2, H, 0);
        let cursor_on_side = m.x < SIDE_W + 20 || prev_mx < SIDE_W + 20;
        refresh_sidebar(cursor_on_side);

        if shelf_out > 0.03 {
            shelf::draw(shelf_out, now);
        }
        asti::render(&cv, asti::HOME_OX);
        mouse::draw_cursor(m.x, m.y, PAL_CURSOR, PAL_CURSOR_EDGE);
        fb::present();
        prev_mx = m.x;

        while time::now_secs() - now < 1.0 / 30.0 {
            mouse::poll();
            core::hint::spin_loop();
        }
    }
}
