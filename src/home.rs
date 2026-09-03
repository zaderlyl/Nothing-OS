//! Le "bureau" de Nothing OS.
//!
//! Plein écran : un fond, une barre de titre, un curseur souris, et Asti
//! (cf. [`crate::asti`]) calé en haut à droite — **toujours visible**.
//! Seule son **étagère de friandises** est cachée : elle se déplie quand
//! la souris passe sur Asti (ou sur l'étagère), et se replie sinon.
//!
//! `run()` tourne la boucle de rendu (~30 img/s, double-buffer).

#![allow(dead_code)]

use core::sync::atomic::{AtomicU8, Ordering};

use crate::{asti, fb, font, mouse, shelf, time};

/// Niveau de nourriture d'Asti : 0 = affamé, 100 = repu.
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
const PAL_BG: u8 = 6;
const PAL_BG_DIM: u8 = 7;
const PAL_BAR: u8 = 8;
const PAL_BAR_TEXT: u8 = 9;
const PAL_CURSOR: u8 = 10;
const PAL_CURSOR_EDGE: u8 = 11;

pub fn install_palette() {
    fb::set_palette(PAL_BG, 30, 36, 52);
    fb::set_palette(PAL_BG_DIM, 20, 24, 38);
    fb::set_palette(PAL_BAR, 12, 14, 22);
    fb::set_palette(PAL_BAR_TEXT, 205, 212, 230);
    fb::set_palette(PAL_CURSOR, 245, 246, 250);
    fb::set_palette(PAL_CURSOR_EDGE, 20, 22, 30);
}

const BAR_H: i32 = 15;
const W: i32 = fb::WIDTH as i32;
const H: i32 = fb::HEIGHT as i32;

fn draw_desktop() {
    fb::fill_rect(0, 0, W, H, PAL_BG);
    fb::fill_rect(0, H * 2 / 3, W, H - H * 2 / 3, PAL_BG_DIM);
    fb::fill_rect(0, 0, W, BAR_H, PAL_BAR);
    font::draw_str(6, (BAR_H - 16) / 2 + 1, "NOTHING OS", PAL_BAR_TEXT, None);
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Boucle du bureau. Ne rend jamais la main.
pub fn run(mut brain: asti::Brain) -> ! {
    mouse::init();
    shelf::init();

    let mut shelf_out = 0.0_f32; // 0 = repliée, 1 = dépliée
    let mut leave_at = -10.0_f32;
    let mut last = time::now_secs();
    let mut click_latch = false;

    loop {
        let now = time::now_secs();
        let dt = (now - last).clamp(0.0, 0.1);
        last = now;

        mouse::poll();
        let m = mouse::state();

        // survol : sur Asti, ou sur l'étagère quand elle est sortie
        let (dcx, dcy) = asti::disc_center(asti::HOME_OX);
        let rad = asti::disc_radius();
        let over_asti = {
            let (dx, dy) = (m.x as f32 - dcx, m.y as f32 - dcy);
            dx * dx + dy * dy < (rad + 6.0) * (rad + 6.0)
        };
        let over_shelf = shelf_out > 0.3 && shelf::hit(m.x, m.y).is_some();
        if over_asti || over_shelf {
            leave_at = now;
        }
        // 0,5 s de sursis avant de replier
        let want = if now - leave_at < 0.5 { 1.0 } else { 0.0 };
        shelf_out += (want - shelf_out) * (1.0 - libm::powf(0.5, dt * 8.0));
        shelf_out = shelf_out.clamp(0.0, 1.0);

        // clic (front montant) sur une friandise → on nourrit Asti
        if m.left && !click_latch {
            if shelf_out > 0.6 {
                if let Some(kind) = shelf::hit(m.x, m.y) {
                    if shelf::take(kind, now) {
                        feed(kind.boost());
                        brain.react_feed(now);
                    }
                }
            }
        }
        click_latch = m.left;

        let state = brain.update(now);
        let mut cv = asti::Canvas::new();
        asti::draw_creature(&mut cv, &state, now);

        draw_desktop();
        if shelf_out > 0.03 {
            shelf::draw(shelf_out, now);
        }
        asti::render(&cv, asti::HOME_OX);
        mouse::draw_cursor(m.x, m.y, PAL_CURSOR, PAL_CURSOR_EDGE);
        fb::present();

        // cadence ~30 img/s, en continuant de vider la souris
        while time::now_secs() - now < 1.0 / 30.0 {
            mouse::poll();
            core::hint::spin_loop();
        }
    }
}
