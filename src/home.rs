//! Écran d'accueil de Nothing OS.
//!
//! Fond noir. Asti (matrice de LED, cf. [`crate::asti`]) est calé en haut
//! à droite et ne bouge pas. Juste à sa gauche, son panneau de
//! nourriture, vertical, qui se vide du haut vers le bas.
//!
//! `run()` tourne la boucle de rendu : ~30 images/s, double-buffer.

#![allow(dead_code)]

use core::sync::atomic::{AtomicU8, Ordering};

use crate::{asti, fb, time};

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

// --- palette du panneau de nourriture (indices dédiés) ---
const PAL_GAUGE_FRAME: u8 = 50;
const PAL_GAUGE_EMPTY: u8 = 51;
const PAL_GAUGE_LOW: u8 = 52;
const PAL_GAUGE_MID: u8 = 53;
const PAL_GAUGE_HIGH: u8 = 54;

pub fn install_palette() {
    fb::set_palette(PAL_GAUGE_FRAME, 90, 96, 110);
    fb::set_palette(PAL_GAUGE_EMPTY, 20, 21, 28);
    fb::set_palette(PAL_GAUGE_LOW, 230, 80, 70);
    fb::set_palette(PAL_GAUGE_MID, 235, 200, 70);
    fb::set_palette(PAL_GAUGE_HIGH, 120, 220, 130);
}

/// Panneau vertical (pilule à bouts ronds), collé à gauche du disque
/// d'Asti. Se vide du haut vers le bas.
fn draw_food_gauge() {
    let (dcx, dcy) = asti::disc_center();
    let rad = asti::disc_radius();

    let w: i32 = 12;
    let right: i32 = (dcx - rad) as i32 + 9; // tuck sous le bord du boîtier
    let x: i32 = right - w;
    let top: i32 = (dcy - rad) as i32 + 4;
    let height: i32 = (rad * 2.0) as i32 - 8;
    let cxf = (x + w / 2) as f32;
    let r = (w as f32) / 2.0;

    // gouttière (cadre + fond vide), bouts arrondis
    fb::fill_circle(cxf, top as f32, r + 1.0, PAL_GAUGE_FRAME);
    fb::fill_circle(cxf, (top + height) as f32, r + 1.0, PAL_GAUGE_FRAME);
    fb::fill_rect(x - 1, top, w + 2, height, PAL_GAUGE_FRAME);
    fb::fill_circle(cxf, top as f32, r, PAL_GAUGE_EMPTY);
    fb::fill_circle(cxf, (top + height) as f32, r, PAL_GAUGE_EMPTY);
    fb::fill_rect(x, top, w, height, PAL_GAUGE_EMPTY);

    let pct = food() as i32;
    let filled = height * pct / 100;
    let color = match pct {
        0..=20 => PAL_GAUGE_LOW,
        21..=50 => PAL_GAUGE_MID,
        _ => PAL_GAUGE_HIGH,
    };
    if filled > 0 {
        let fill_top = top + (height - filled);
        fb::fill_rect(x, fill_top, w, filled, color);
        fb::fill_circle(cxf, (top + height) as f32, r, color);
        fb::fill_circle(cxf, fill_top as f32, r, color);
    }
}

/// Boucle de rendu. Ne rend jamais la main.
pub fn run(mut brain: asti::Brain) -> ! {
    loop {
        let frame_start = time::now_secs();

        let state = brain.update(frame_start);

        let mut cv = asti::Canvas::new();
        asti::draw_creature(&mut cv, &state);

        fb::clear(0);
        asti::render(&cv);
        draw_food_gauge();
        fb::present();

        // ~30 images/s
        time::spin_until(frame_start, 1.0 / 30.0);
    }
}
