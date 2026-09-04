//! Animation de démarrage : « NOTHING OS » en points qui se révèle
//! lettre par lettre, un trait qui balaie dessous, court temps de pose,
//! puis on rend la main à l'accueil. Tout en noir, style LED comme le
//! reste de l'OS.

#![allow(static_mut_refs)]

use crate::{fb, font, time};

// indices déjà posés par home::install_palette()
const TITLE: u8 = 14; // blanc vif
const DIM: u8 = 10; // gris
const ACCENT: u8 = 13; // bleu

const A: &str = "NOT";
const B: &str = "HING OS"; // petit espace après "NOT", comme l'accueil

pub fn play() {
    let w = fb::WIDTH as i32;
    let h = fb::HEIGHT as i32;

    let cell = 13;
    let gap = cell; // espace T / H
    let wa = 8 * cell * A.len() as i32;
    let wb = 8 * cell * B.len() as i32;
    let tw = wa + gap + wb;
    let th = 8 * cell;
    let tx = (w - tw) / 2;
    let ty = (h - th) / 2 - 30;
    let n_total = (A.len() + B.len()) as f32;

    let start = time::now_secs();
    let reveal_end = 1.3_f32; // révélation du titre
    let total = 2.8_f32;

    loop {
        let t = time::now_secs() - start;
        if t >= total {
            break;
        }
        fb::clear(0);

        // --- 1. titre révélé lettre par lettre (ease-out) ---
        let r = (t / reveal_end).clamp(0.0, 1.0);
        let r = 1.0 - (1.0 - r) * (1.0 - r);
        let shown = libm::ceilf(r * n_total) as usize;

        let na = shown.min(A.len());
        font::draw_str_dots(tx, ty, &A[..na], TITLE, cell);
        if shown > A.len() {
            let nb = (shown - A.len()).min(B.len());
            font::draw_str_dots(tx + wa + gap, ty, &B[..nb], TITLE, cell);
        }

        // --- 2. trait sous le titre ---
        let by = ty + th + 46;
        if t < reveal_end {
            // pendant la révélation : le trait suit la dernière lettre
            let x2 = tx + (tw as f32 * r) as i32;
            fb::fill_rect(tx, by, (x2 - tx).max(0), 3, ACCENT);
        } else {
            // ensuite : base grise + balayage bleu qui fait un aller-retour
            fb::fill_rect(tx, by, tw, 3, DIM);
            let s = ((t - reveal_end) / (total - reveal_end)).clamp(0.0, 1.0);
            let sweep = if s < 0.5 { s * 2.0 } else { (1.0 - s) * 2.0 };
            let seg = tw / 4;
            let sx = tx + ((tw - seg) as f32 * sweep) as i32;
            fb::fill_rect(sx, by, seg, 3, ACCENT);
        }

        // --- 3. sous-titre discret, apparaît à la fin ---
        if t > reveal_end {
            let sub = "compagnon";
            let sw = font::width_scaled(sub, 2);
            font::draw_str_scaled((w - sw) / 2, by + 34, sub, DIM, 2);
        }

        fb::present();
        frame_wait();
    }
}

fn frame_wait() {
    let f = time::now_secs();
    let mut g = 0u32;
    while time::now_secs() - f < 1.0 / 60.0 && g < 3_000_000 {
        core::hint::spin_loop();
        g += 1;
    }
}
