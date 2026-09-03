//! Petits logos en points façon matrice de LED — même style que les
//! friandises d'Asti (`src/shelf.rs`). Sert d'icônes pour les fichiers,
//! les dossiers et les applications.
//!
//! `#` = point allumé, `o` = point atténué, tout le reste = éteint.

#![allow(dead_code)]

use crate::fb;

/// Dessine un motif, coin haut-gauche en (x, y), chaque point = un pavé
/// `scale × scale`. `on` / `dim` = index de palette des deux niveaux.
pub fn draw(pat: &[&str], x: i32, y: i32, scale: i32, on: u8, dim: u8) {
    for (row, line) in pat.iter().enumerate() {
        for (col, ch) in line.bytes().enumerate() {
            let c = match ch {
                b'#' => on,
                b'o' => dim,
                _ => continue,
            };
            fb::fill_rect(x + col as i32 * scale, y + row as i32 * scale, scale, scale, c);
        }
    }
}

/// (largeur, hauteur) d'un motif à l'échelle `scale`.
pub fn size(pat: &[&str], scale: i32) -> (i32, i32) {
    (
        pat.iter().map(|l| l.len()).max().unwrap_or(0) as i32 * scale,
        pat.len() as i32 * scale,
    )
}

/// Dessine centré dans le rectangle (cx, cy, cw, ch).
pub fn draw_centered(pat: &[&str], cx: i32, cy: i32, cw: i32, ch: i32, scale: i32, on: u8, dim: u8) {
    let (w, h) = size(pat, scale);
    draw(pat, cx + (cw - w) / 2, cy + (ch - h) / 2, scale, on, dim);
}

// --- catalogue de motifs -------------------------------------------------

pub const FOLDER: &[&str] = &[
    "..######....",
    ".########...",
    "############",
    "#..........#",
    "#..........#",
    "#..........#",
    "#..........#",
    "#..........#",
    "############",
];

pub const FILE: &[&str] = &[
    "########o..",
    "########o..",
    "#######oo..",
    "#.......#..",
    "#.#####.#..",
    "#.......#..",
    "#.#####.#..",
    "#.......#..",
    "#.#####.#..",
    "#.......#..",
    "#########..",
];

pub const TERMINAL: &[&str] = &[
    "############",
    "#..........#",
    "#.##.......#",
    "#...##.....#",
    "#.....##...#",
    "#...##.....#",
    "#.##.......#",
    "#....#####.#",
    "#..........#",
    "############",
];

pub const EDITOR: &[&str] = &[
    ".........##.",
    "........####",
    ".......##.##",
    "......##.##.",
    ".....##.##..",
    "....##.##...",
    "...##.##....",
    "..##.##.....",
    "####.#......",
    "###.o.......",
];

pub const CALC: &[&str] = &[
    "##########",
    "#........#",
    "#.oooooo.#",
    "#.oooooo.#",
    "#........#",
    "#.#..#..##",
    "#........#",
    "#.#..#..##",
    "#........#",
    "#.#..#..##",
    "##########",
];

pub const WEB: &[&str] = &[
    "...#####...",
    ".##..#..##.",
    ".#..#.#..#.",
    "##.#.#.#.##",
    "#.#.#.#.#.#",
    "###########",
    "#.#.#.#.#.#",
    "##.#.#.#.##",
    ".#..#.#..#.",
    ".##..#..##.",
    "...#####...",
];

pub const HEART: &[&str] = &[
    ".###...###.",
    "###########",
    "###########",
    "###########",
    ".#########.",
    "..#######..",
    "...#####...",
    "....###....",
    ".....#.....",
];

pub const QUESTION: &[&str] = &[
    "..######..",
    ".##....##.",
    "##......##",
    "......###.",
    ".....##...",
    ".....##...",
    ".....##...",
    "..........",
    ".....##...",
    ".....##...",
];
