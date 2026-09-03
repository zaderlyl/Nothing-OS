//! Le "bureau" de Nothing OS.
//!
//! Fond plein écran, une barre de titre en haut, un curseur souris. Asti
//! vit caché contre le bord droit : quand la souris s'approche (ou passe
//! sur lui), il coulisse pour apparaître ; quand elle repart, il se
//! retire. Quand il est sorti, son étagère de friandises apparaît (voir
//! [`crate::shelf`]).

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
const PAL_BG_TOP: u8 = 6;
const PAL_BG_BOT: u8 = 7;
const PAL_BAR: u8 = 8;
const PAL_BAR_TEXT: u8 = 9;
const PAL_CURSOR: u8 = 10;
const PAL_CURSOR_EDGE: u8 = 11;

pub fn install_palette() {
    fb::set_palette(PAL_BG_TOP, 32, 38, 54);
    fb::set_palette(PAL_BG_BOT, 18, 22, 34);
    fb::set_palette(PAL_BAR, 12, 14, 22);
    fb::set_palette(PAL_BAR_TEXT, 200, 208, 226);
    fb::set_palette(PAL_CURSOR, 245, 246, 250);
    fb::set_palette(PAL_CURSOR_EDGE, 20, 22, 30);
}

const BAR_H: i32 = 13;

fn draw_desktop() {
    // fond : léger dégradé vertical
    for y in 0..fb::HEIGHT as i32 {
        let c = if y < (fb::HEIGHT as i32) / 2 {
            PAL_BG_TOP
        } else {
            PAL_BG_BOT
        };
        fb::fill_rect(0, y, fb::WIDTH as i32, 1, c);
    }

    // barre de titre
    fb::fill_rect(0, 0, fb::WIDTH as i32, BAR_H, PAL_BAR);
    font::draw_str(4, 2, "NOTHING OS", PAL_BAR_TEXT, None);
}

// Position de la grille d'Asti : sorti (visible) vs caché (contre le bord).
const OX_SHOWN: f32 = fb::WIDTH as f32 - 148.0;
const OX_HIDDEN: f32 = fb::WIDTH as f32 - 18.0;

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Boucle du bureau. Ne rend jamais la main.
pub fn run(mut brain: asti::Brain) -> ! {
    mouse::init();
    shelf::init();

    let mut out = 0.0_f32; // 0 = caché, 1 = sorti
    let mut last = time::now_secs();
    let mut leave_at = 0.0_f32; // instant où la souris a quitté la zone

    loop {
        let now = time::now_secs();
        let dt = (now - last).clamp(0.0, 0.1);
        last = now;

        mouse::poll();
        let m = mouse::state();

        // Asti veut-il être sorti ? souris dans la bande droite, ou sur lui.
        let ox = lerp(OX_HIDDEN, OX_SHOWN, out);
        let (dcx, dcy) = asti::disc_center(ox);
        let rad = asti::disc_radius();
        let over_asti = {
            let (dx, dy) = (m.x as f32 - dcx, m.y as f32 - dcy);
            dx * dx + dy * dy < (rad + 8.0) * (rad + 8.0)
        };
        let in_band = m.x > fb::WIDTH as i32 - 48;
        let over_shelf = out > 0.3 && shelf::hit(m.x, m.y).is_some();
        let near = in_band || over_asti || over_shelf;
        if near {
            leave_at = now;
        }
        // reste sorti tant que la souris est là, + 0,5 s de sursis
        let want = if now - leave_at < 0.5 { 1.0 } else { 0.0 };
        out += (want - out) * (1.0 - libm::powf(0.5, dt * 7.0));
        out = out.clamp(0.0, 1.0);

        // clic sur une friandise → on nourrit Asti
        if m.left && out > 0.6 {
            if let Some(kind) = shelf::hit(m.x, m.y) {
                if shelf::take(kind, now) {
                    feed(kind.boost());
                    brain.react_feed(now);
                }
            }
        }

        let state = brain.update(now);
        let mut cv = asti::Canvas::new();
        asti::draw_creature(&mut cv, &state, now);

        draw_desktop();
        let ox = lerp(OX_HIDDEN, OX_SHOWN, out);
        asti::render(&cv, ox);
        if out > 0.05 {
            shelf::draw(out, now);
        }
        mouse::draw_cursor(m.x, m.y, PAL_CURSOR, PAL_CURSOR_EDGE);
        fb::present();

        // cadence ~30 img/s, en continuant de vider la souris
        while time::now_secs() - now < 1.0 / 30.0 {
            mouse::poll();
            core::hint::spin_loop();
        }
    }
}
