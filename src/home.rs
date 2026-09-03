//! Le "bureau" de Nothing OS.
//!
//! Fond noir. Au centre, « NOTHING OS » écrit en points et, juste en
//! dessous, une barre de recherche. La **barre latérale** (tâches à faire
//! + résumé + heure) est cachée : elle glisse depuis la gauche quand la
//! souris frôle le bord. Asti est calé en haut à droite, toujours
//! visible ; son étagère de friandises se déplie au survol.

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
const PAL_DIVIDER: u8 = 7;
const PAL_HEADER: u8 = 8;
const PAL_TEXT: u8 = 9;
const PAL_TEXT_DIM: u8 = 10;
const PAL_CURSOR: u8 = 11;
const PAL_CURSOR_EDGE: u8 = 12;
const PAL_ACCENT: u8 = 13;
const PAL_TITLE: u8 = 14;
const PAL_SEARCH: u8 = 15;

pub fn install_palette() {
    fb::set_palette(PAL_SIDE, 10, 11, 15); // barre : quasi noir ("transparent")
    fb::set_palette(PAL_DIVIDER, 46, 48, 60); // séparateur, discret
    fb::set_palette(PAL_HEADER, 110, 120, 150);
    fb::set_palette(PAL_TEXT, 214, 220, 234);
    fb::set_palette(PAL_TEXT_DIM, 110, 116, 135);
    fb::set_palette(PAL_CURSOR, 245, 246, 250);
    fb::set_palette(PAL_CURSOR_EDGE, 20, 22, 30);
    fb::set_palette(PAL_ACCENT, 120, 200, 255);
    fb::set_palette(PAL_TITLE, 236, 240, 250);
    fb::set_palette(PAL_SEARCH, 24, 26, 34);
}

const W: i32 = fb::WIDTH as i32;
const H: i32 = fb::HEIGHT as i32;
const SIDE_W: i32 = 500;
const PAD: i32 = 28;

// --- contenu de la barre (placeholders) ---
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

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

// ---------------------------------------------------------------------
// Centre de l'écran : titre en points + barre de recherche.
// ---------------------------------------------------------------------

fn draw_hero(now: f32) {
    // --- NOTHING OS, en points, centré ---
    // Petit espace en plus entre le "T" et le "H".
    const DOT_CELL: i32 = 10;
    const GAP: i32 = 2 * DOT_CELL; // ~quart de caractère
    let w_not = 3 * 8 * DOT_CELL;
    let w_rest = 7 * 8 * DOT_CELL; // "HING OS"
    let tw = w_not + GAP + w_rest;
    let tx = (W - tw) / 2;
    let ty = H * 30 / 100;
    font::draw_str_dots(tx, ty, "NOT", PAL_TITLE, DOT_CELL);
    font::draw_str_dots(tx + w_not + GAP, ty, "HING OS", PAL_TITLE, DOT_CELL);

    // --- barre de recherche, un peu plus bas ---
    let bw = 760;
    let bh = 54;
    let bx = (W - bw) / 2;
    let by = ty + 16 * DOT_CELL + 60;
    let r = (bh / 2) as f32;

    fb::fill_circle(bx as f32, (by + bh / 2) as f32, r, PAL_DIVIDER);
    fb::fill_circle((bx + bw) as f32, (by + bh / 2) as f32, r, PAL_DIVIDER);
    fb::fill_rect(bx, by, bw, bh, PAL_DIVIDER);
    fb::fill_circle(bx as f32, (by + bh / 2) as f32, r - 2.0, PAL_SEARCH);
    fb::fill_circle((bx + bw) as f32, (by + bh / 2) as f32, r - 2.0, PAL_SEARCH);
    fb::fill_rect(bx, by + 2, bw, bh - 4, PAL_SEARCH);

    // loupe
    let (mx, my) = (bx + 26, by + bh / 2);
    fb::fill_circle(mx as f32, my as f32, 9.0, PAL_TEXT_DIM);
    fb::fill_circle(mx as f32, my as f32, 6.0, PAL_SEARCH);
    for i in 0..9 {
        fb::fill_rect(mx + 6 + i, my + 6 + i, 3, 3, PAL_TEXT_DIM);
    }

    // texte indicatif + curseur clignotant
    font::draw_str_scaled(bx + 52, by + bh / 2 - 8, "Rechercher", PAL_TEXT_DIM, 2);
    if (now * 2.0) as i32 % 2 == 0 {
        fb::fill_rect(bx + 52 + font::width_scaled("Rechercher", 2) + 6, by + 12, 3, bh - 24, PAL_TEXT);
    }
}

// ---------------------------------------------------------------------
// Barre latérale (glisse depuis la gauche).
// ---------------------------------------------------------------------

fn draw_sidebar(x0: i32) {
    fb::fill_rect(x0, 0, SIDE_W, H, PAL_SIDE);
    fb::fill_rect(x0 + SIDE_W, 0, 2, H, PAL_DIVIDER);

    let mut y = PAD + 10;
    font::draw_str_scaled(x0 + PAD, y, "A FAIRE", PAL_HEADER, 2);
    y += 46;
    for (label, done) in TASKS {
        let txt_c = if done { PAL_TEXT_DIM } else { PAL_TEXT };
        fb::fill_rect(x0 + PAD, y, 18, 18, PAL_DIVIDER);
        fb::fill_rect(x0 + PAD + 2, y + 2, 14, 14, PAL_SIDE);
        if done {
            fb::fill_rect(x0 + PAD + 4, y + 4, 10, 10, PAL_ACCENT);
        }
        font::draw_str_scaled(x0 + PAD + 32, y, label, txt_c, 2);
        y += 40;
    }

    // séparateur : court, centré dans la barre, couleur discrète
    y += 22;
    let sep_w = SIDE_W / 3;
    fb::fill_rect(x0 + (SIDE_W - sep_w) / 2, y, sep_w, 2, PAL_DIVIDER);
    y += 28;

    font::draw_str_scaled(x0 + PAD, y, "INFOS", PAL_HEADER, 2);
    y += 46;
    for (k, v) in INFOS {
        font::draw_str_scaled(x0 + PAD, y, k, PAL_TEXT_DIM, 2);
        font::draw_str_scaled(x0 + PAD + 176, y, v, PAL_TEXT, 2);
        y += 40;
    }

    // horloge en bas
    let t = rtc::now();
    let cy = H - 100;
    font::draw_num(x0 + PAD + 4, cy, t.hour as u32, 2, PAL_TEXT, 5);
    fb::fill_rect(x0 + PAD + 4 + 88, cy + 16, 8, 8, PAL_TEXT);
    fb::fill_rect(x0 + PAD + 4 + 88, cy + 46, 8, 8, PAL_TEXT);
    font::draw_num(x0 + PAD + 4 + 108, cy, t.min as u32, 2, PAL_TEXT, 5);
}

/// Boucle du bureau. Ne rend jamais la main.
pub fn run(mut brain: asti::Brain) -> ! {
    mouse::init();
    shelf::init();

    let mut shelf_out = 0.0_f32;
    let mut shelf_leave = -10.0_f32;
    let mut side_out = 0.0_f32; // 0 = cachée, 1 = visible
    let mut side_leave = -10.0_f32;
    let mut last = time::now_secs();
    let mut click_latch = false;
    let mut diag_t = last;
    let mut drag: Option<shelf::Kind> = None; // friandise en cours de glissement

    loop {
        // --- raccourci de fermeture : Maj + Tab + Cmd ---
        if crate::kbd::close_combo() {
            crate::serial_println!("[nothing-os] fermeture (Maj+Tab+Cmd)");
            crate::kbd::power_off();
        }

        let now = time::now_secs();
        let dt = (now - last).clamp(0.0, 0.1);
        last = now;

        mouse::poll();
        let m = mouse::state();

        // diagnostic souris : si x/y ne bougent jamais quand tu bouges la
        // souris, c'est que QEMU ne "capture" pas le pointeur → clique
        // dans la fenêtre (⌃⌥G pour relâcher).
        if now - diag_t >= 3.0 {
            diag_t = now;
            crate::serial_println!(
                "[nothing-os] souris x={} y={} paquets={}",
                m.x,
                m.y,
                mouse::packets()
            );
        }

        // --- étagère : suit le survol d'Asti ---
        let (dcx, dcy) = asti::disc_center(asti::HOME_OX);
        let rad = asti::disc_radius();
        let over_asti = {
            let (dx, dy) = (m.x as f32 - dcx, m.y as f32 - dcy);
            dx * dx + dy * dy < (rad + 8.0) * (rad + 8.0)
        };
        let over_shelf = shelf_out > 0.3 && shelf::hit(m.x, m.y, now).is_some();
        if over_asti || over_shelf || drag.is_some() {
            shelf_leave = now;
        }
        let want = if now - shelf_leave < 0.5 { 1.0 } else { 0.0 };
        shelf_out += (want - shelf_out) * (1.0 - libm::powf(0.5, dt * 8.0));
        shelf_out = shelf_out.clamp(0.0, 1.0);

        // --- barre latérale : souris au bord gauche ---
        let side_x = lerp(-(SIDE_W as f32) - 4.0, 0.0, side_out) as i32;
        let near_left = m.x < 18 || (side_out > 0.15 && m.x < side_x + SIDE_W + 8);
        if near_left {
            side_leave = now;
        }
        let side_want = if now - side_leave < 0.4 { 1.0 } else { 0.0 };
        side_out += (side_want - side_out) * (1.0 - libm::powf(0.5, dt * 9.0));
        side_out = side_out.clamp(0.0, 1.0);

        // --- glisser-déposer d'une friandise sur Asti ---
        if m.left && !click_latch {
            // début de glissement : appui sur une friandise
            if drag.is_none() && shelf_out > 0.5 {
                if let Some(kind) = shelf::hit(m.x, m.y, now) {
                    shelf::pick(kind);
                    drag = Some(kind);
                }
            }
        }
        if !m.left && click_latch {
            // relâché : sur Asti → on nourrit, sinon la friandise revient
            if let Some(kind) = drag.take() {
                if over_asti {
                    shelf::consume(kind, now);
                    feed(kind.boost());
                    brain.react_feed(kind, now);
                } else {
                    shelf::restore(kind);
                }
            }
        }
        click_latch = m.left;

        // --- rendu ---
        let state = brain.update(now);
        let mut cv = asti::Canvas::new();
        asti::draw_creature(&mut cv, &state, now);

        fb::clear(0);
        draw_hero(now);
        if shelf_out > 0.03 {
            shelf::draw(shelf_out, now);
        }
        asti::render(&cv, asti::HOME_OX);
        if side_out > 0.01 {
            draw_sidebar(lerp(-(SIDE_W as f32) - 4.0, 0.0, side_out) as i32);
        }
        // friandise "en vol" sous le curseur pendant le glissement
        if let Some(kind) = drag {
            let (tw, th) = shelf::treat_size(kind, 5);
            shelf::draw_treat_at(kind, m.x - tw / 2, m.y - th / 2, 5);
        }
        mouse::draw_cursor(m.x, m.y, PAL_CURSOR, PAL_CURSOR_EDGE, 3, m.left && drag.is_none());
        fb::present();

        let mut guard = 0u32;
        while time::now_secs() - now < 1.0 / 30.0 && guard < 3_000_000 {
            mouse::poll();
            core::hint::spin_loop();
            guard += 1;
        }
    }
}
