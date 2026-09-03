//! Écran d'accueil de Nothing OS.
//!
//! Pas de bureau, pas d'applications : un fond noir, le nom de l'OS, le
//! personnage **Asti**, et sa **barre de nourriture**.
//!
//! Asti est rendu comme sur l'appli d'origine (« PC Pet ») : une petite
//! matrice de LED circulaire de 25×25 points. Les points « éteints »
//! dessinent le disque, les points « allumés » (plus clairs) forment le
//! visage — deux yeux et un sourire. Le tracé se fait dans un buffer de
//! luminance `f32` (mêmes primitives que l'original : `disc`, `stroke`),
//! puis on projette 2 points par cellule texte avec le demi-bloc `0xDF`
//! (couleur avant = point du haut, couleur arrière = point du bas).

// `feed` / `starve` / `set_food` pas encore appelés (il manque le timer
// et le clavier) — on les garde en place pour la suite.
#![allow(dead_code)]

use core::sync::atomic::{AtomicU8, Ordering};

use crate::vga::Color;
use crate::{clear_screen, print, put_cell, put_raw, set_color, set_position};

const SCREEN_WIDTH: usize = 80;
const OS_NAME: &str = "N O T H I N G   O S";

/// Niveau de nourriture d'Asti : 0 = affamé, 100 = repu.
static FOOD: AtomicU8 = AtomicU8::new(100);

// ---------------------------------------------------------------------
// Petite bibliothèque maths `f32` (le noyau est `no_std` : `core` ne
// fournit ni `sqrt` ni `hypot`).
// ---------------------------------------------------------------------

/// Racine carrée : germe par bidouille de bits puis 3 itérations de
/// Newton. Largement assez précis pour du placement sous-pixel.
fn sqrtf(x: f32) -> f32 {
    if x <= 0.0 {
        return 0.0;
    }
    let mut g = f32::from_bits((x.to_bits() >> 1).wrapping_add(0x1fbd_1df5));
    g = 0.5 * (g + x / g);
    g = 0.5 * (g + x / g);
    g = 0.5 * (g + x / g);
    g
}

fn hyp(dx: f32, dy: f32) -> f32 {
    sqrtf(dx * dx + dy * dy)
}

fn clamp01(v: f32) -> f32 {
    if v < 0.0 {
        0.0
    } else if v > 1.0 {
        1.0
    } else {
        v
    }
}

fn floori(x: f32) -> i32 {
    let i = x as i32;
    if (i as f32) > x {
        i - 1
    } else {
        i
    }
}

fn ceili(x: f32) -> i32 {
    let i = x as i32;
    if (i as f32) < x {
        i + 1
    } else {
        i
    }
}

// ---------------------------------------------------------------------
// Buffer de luminance 25×25 + primitives de tracé (portées telles quelles
// depuis le moteur de rendu de PC Pet).
// ---------------------------------------------------------------------

const N: usize = 25;
/// Centre de la matrice : `(N - 1) / 2`.
const CEN: f32 = 12.0;
/// Rayon du disque de LED.
const RAD: f32 = 12.4;

struct Canvas {
    buf: [f32; N * N],
}

impl Canvas {
    fn new() -> Canvas {
        Canvas { buf: [0.0; N * N] }
    }

    /// Allume le point (x, y) à la luminance `v` (on garde le max).
    fn px(&mut self, x: i32, y: i32, v: f32) {
        if x < 0 || y < 0 || x >= N as i32 || y >= N as i32 {
            return;
        }
        let i = y as usize * N + x as usize;
        let v = if v > 1.0 { 1.0 } else { v };
        if v > self.buf[i] {
            self.buf[i] = v;
        }
    }

    /// Disque plein anti-aliasé de centre (cx, cy), rayon `rad`.
    fn disc(&mut self, cx: f32, cy: f32, rad: f32, v: f32) {
        let y0 = floori(cy - rad - 1.0);
        let y1 = ceili(cy + rad + 1.0);
        let x0 = floori(cx - rad - 1.0);
        let x1 = ceili(cx + rad + 1.0);
        let mut y = y0;
        while y <= y1 {
            let mut x = x0;
            while x <= x1 {
                let a = clamp01(rad + 0.5 - hyp(x as f32 - cx, y as f32 - cy));
                if a > 0.0 {
                    self.px(x, y, a * v);
                }
                x += 1;
            }
            y += 1;
        }
    }

    /// Trait épais entre deux points (suite de petits disques).
    fn stroke(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, w: f32, v: f32) {
        let steps = {
            let s = ceili(hyp(x2 - x1, y2 - y1) * 2.0);
            if s < 1 {
                1
            } else {
                s
            }
        };
        let mut i = 0;
        while i <= steps {
            let t = i as f32 / steps as f32;
            self.disc(x1 + (x2 - x1) * t, y1 + (y2 - y1) * t, w, v);
            i += 1;
        }
    }
}

// ---------------------------------------------------------------------
// Asti : visage de repos (deux yeux « dot », sourire).
//
// Reprend la géométrie « mode visage » de l'original :
//   yeux   : disc(±4.2, -2.6), rayon 1.05 * feat
//   bouche : arc `smile` autour de (0, 4.2), demi-largeur 1.5, courbure
//            0.75, épaisseur 0.4 * feat + 0.05
// le tout à l'échelle feat = 1.45, luminance 1.0.
// ---------------------------------------------------------------------

const FEAT: f32 = 1.45;

fn draw_asti(cv: &mut Canvas) {
    // Yeux.
    let eye_y = CEN - 2.6;
    for sg in [-1.0_f32, 1.0] {
        cv.disc(CEN + sg * 4.2, eye_y, 1.05 * FEAT, 1.0);
    }

    // Bouche : sourire échantillonné en 9 points, reliés par des traits.
    let (mx, my) = (CEN, CEN + 4.2);
    let half = 1.5_f32;
    let curv = 0.75_f32;
    let w = 0.4 * FEAT + 0.05;
    let step = half / 4.0;

    let mut prev: Option<(f32, f32)> = None;
    let mut i = -half;
    while i <= half + 1e-4 {
        let yy = -0.15 + curv * (1.0 - (i / half) * (i / half));
        let p = (mx + i * FEAT, my + yy * FEAT);
        if let Some(pv) = prev {
            cv.stroke(pv.0, pv.1, p.0, p.1, w, 1.0);
        }
        prev = Some(p);
        i += step;
    }
}

// ---------------------------------------------------------------------
// Rendu de la matrice → cellules texte VGA.
// ---------------------------------------------------------------------

/// Palette d'un « écran » : point éteint, deux niveaux allumés.
struct DotPalette {
    off: Color,
    dim: Color,
    lit: Color,
}

/// Défaut : noir & blanc, comme l'appli d'origine sans contexte.
const MONO: DotPalette = DotPalette {
    off: Color::DarkGray,
    dim: Color::LightGray,
    lit: Color::White,
};

fn in_circle(x: usize, y: usize) -> bool {
    hyp(x as f32 - CEN, y as f32 - CEN) <= RAD
}

/// Couleur d'un point de la matrice selon sa luminance.
fn dot_color(cv: &Canvas, pal: &DotPalette, x: usize, y: usize) -> Color {
    if y >= N || !in_circle(x, y) {
        return Color::Black;
    }
    let v = cv.buf[y * N + x];
    if v < 0.06 {
        pal.off // point "éteint" du panneau
    } else if v < 0.42 {
        pal.dim // bord adouci / halo autour d'un trait
    } else {
        pal.lit // trait du visage
    }
}

// Rendu "matrice de LED" : 1 point = 1 cellule texte, caractère `0xFE`
// (petit carré centré `■`, avec une marge intégrée → l'espace noir entre
// les points). On met 2 colonnes écran par point (l'une vide) pour
// compenser le ratio d'une cellule VGA (~9 px de large / 16 de haut) et
// garder un disque à peu près rond.
//
// La grille fait 25 lignes ; l'écran n'en a que 25 en tout. On rogne
// donc les 2 lignes de points extrêmes en haut et en bas (`Y_CROP`) —
// juste quelques points ternes tout au bord du disque — pour laisser la
// place au titre et à la barre.

/// Caractère "point" : `■` (code page 437, 0xFE).
const DOT: u8 = 0xFE;
/// Première/dernière ligne de la grille effectivement affichée.
const Y_CROP: usize = 2;
const ASTI_TOP: usize = 1;
const ASTI_LEFT: usize = (SCREEN_WIDTH - N * 2) / 2;
/// Nombre de lignes écran occupées par la matrice.
const ASTI_ROWS: usize = N - 2 * Y_CROP;

fn draw_matrix(cv: &Canvas, pal: &DotPalette) {
    for gy in Y_CROP..(N - Y_CROP) {
        let screen_row = ASTI_TOP + (gy - Y_CROP);
        for x in 0..N {
            let c = dot_color(cv, pal, x, gy);
            if c != Color::Black {
                put_cell(screen_row, ASTI_LEFT + x * 2, DOT, c, Color::Black);
            }
        }
    }
}

// ---------------------------------------------------------------------
// API « nourriture » (prête pour le futur timer + clavier).
// ---------------------------------------------------------------------

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

// ---------------------------------------------------------------------
// Écran complet.
// ---------------------------------------------------------------------

fn centered_col(len: usize) -> usize {
    SCREEN_WIDTH.saturating_sub(len) / 2
}

/// (Re)dessine tout l'écran d'accueil sur fond noir.
pub fn render() {
    clear_screen();

    // Nom de l'OS.
    set_color(Color::White, Color::Black);
    set_position(0, centered_col(OS_NAME.len()));
    print!("{}", OS_NAME);

    // Asti.
    let mut cv = Canvas::new();
    draw_asti(&mut cv);
    draw_matrix(&cv, &MONO);

    // Étiquette du personnage.
    set_color(Color::DarkGray, Color::Black);
    set_position(ASTI_TOP + ASTI_ROWS, centered_col(4));
    print!("Asti");

    // Barre de nourriture, sur la dernière ligne de l'écran.
    draw_food_bar(24);
}

/// Dessine `Nourriture [████████░░░░░░░░] 62%` centré sur la ligne `row`.
fn draw_food_bar(row: usize) {
    const BAR_WIDTH: usize = 40;
    const LABEL: &str = "Nourriture ";
    const BLOCK_FULL: u8 = 0xDB; // █
    const BLOCK_LIGHT: u8 = 0xB0; // ░

    let pct = food() as usize;
    let filled = pct * BAR_WIDTH / 100;
    let empty = BAR_WIDTH - filled;

    let total = LABEL.len() + 1 + BAR_WIDTH + 2 + 4;
    let col = centered_col(total);

    set_position(row, col);
    set_color(Color::LightGray, Color::Black);
    print!("{}[", LABEL);

    let fill_color = match pct {
        0..=20 => Color::LightRed,
        21..=50 => Color::Yellow,
        _ => Color::LightGreen,
    };
    set_color(fill_color, Color::Black);
    put_raw(BLOCK_FULL, filled);

    set_color(Color::DarkGray, Color::Black);
    put_raw(BLOCK_LIGHT, empty);

    set_color(Color::LightGray, Color::Black);
    print!("] {:>3}%", pct);
}
