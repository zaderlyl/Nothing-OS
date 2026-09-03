//! Étagère de friandises (le "panneau des consommables").
//!
//! Colonne verticale de friandises, à gauche d'Asti, qui apparaît quand
//! il est sorti. Chaque friandise est dessinée en points facon LED
//! (motifs repris tels quels de `renderer/treats.html`). Un clic dessus
//! nourrit Asti ; la friandise disparaît puis réapparaît ~2,6 s plus tard.

use crate::fb;

#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    Cookie,
    Bone,
    Berry,
    Fish,
    Candy,
    Carrot,
    Chili,
    Donut,
    Battery,
}

pub const ALL: [Kind; 9] = [
    Kind::Cookie,
    Kind::Bone,
    Kind::Berry,
    Kind::Fish,
    Kind::Candy,
    Kind::Carrot,
    Kind::Chili,
    Kind::Donut,
    Kind::Battery,
];

impl Kind {
    fn idx(self) -> usize {
        ALL.iter().position(|&k| k == self).unwrap()
    }

    /// Points de nourriture rendus (cf. `pet.js` : boost * 100).
    pub fn boost(self) -> u8 {
        match self {
            Kind::Battery => 34,
            Kind::Carrot => 20,
            Kind::Candy => 16,
            Kind::Chili => 4,
            _ => 13,
        }
    }

    fn pattern(self) -> &'static [&'static str] {
        match self {
            Kind::Cookie => &[
                "...#####...",
                "..#######..",
                ".####o####.",
                ".#########.",
                "#####o#####",
                "###########",
                "#####o#####",
                ".####o####.",
                ".#########.",
                "..#######..",
                "...#####...",
            ],
            Kind::Bone => &[
                "##.......##",
                "####...####",
                ".#########.",
                ".#########.",
                "####...####",
                "##.......##",
            ],
            Kind::Berry => &[
                "....#.#....",
                "...#####...",
                "....###....",
                ".#########.",
                "##o#####o##",
                "###########",
                ".#o#####o#.",
                ".##o###o##.",
                "..#######..",
                "...#o#o#...",
                "....###....",
            ],
            Kind::Fish => &[
                "...####..#...",
                "..######.##..",
                ".###o#####.##",
                "####o########",
                ".###o#####.##",
                "..######.##..",
                "...####..#...",
            ],
            Kind::Candy => &[
                "#.........#",
                "##.......##",
                ".##.###.##.",
                "..#######..",
                "..#######..",
                "..#######..",
                ".##.###.##.",
                "##.......##",
                "#.........#",
            ],
            Kind::Carrot => &[
                "..#.#.#..",
                "...###...",
                "..#####..",
                "..#####..",
                "...###...",
                "...###...",
                "....#....",
                "....#....",
                "....#....",
            ],
            Kind::Chili => &[
                "....##...",
                "....#....",
                "...##....",
                "..###....",
                "..####...",
                "..#####..",
                "...#####.",
                "...####..",
                "....##...",
                ".....#...",
            ],
            Kind::Donut => &[
                "..#####..",
                ".###o###.",
                "##o...o##",
                "#o.....o#",
                "#o.....o#",
                "#o.....o#",
                "##o...o##",
                ".###o###.",
                "..#####..",
            ],
            Kind::Battery => &[
                "...###...",
                "#########",
                "#.......#",
                "#...##..#",
                "#..##...#",
                "#.#####.#",
                "#...##..#",
                "#..##...#",
                "#.......#",
                "#########",
            ],
        }
    }
}

// --- palette dédiée (indices 55..=59) ---
const PAL_PANEL: u8 = 55;
const PAL_PANEL_EDGE: u8 = 56;
const PAL_DOT: u8 = 57;
const PAL_DOT_DIM: u8 = 58;

pub fn install_palette() {
    fb::set_palette(PAL_PANEL, 18, 19, 26);
    fb::set_palette(PAL_PANEL_EDGE, 44, 46, 58);
    fb::set_palette(PAL_DOT, 233, 239, 253);
    fb::set_palette(PAL_DOT_DIM, 90, 96, 120);
}

// --- géométrie ---
const CELL: i32 = 54; // hauteur d'une case friandise
const SLOT: i32 = CELL + 10;
const PANEL_W: i32 = 64;
const TOP: i32 = (fb::HEIGHT as i32 - SLOT * ALL.len() as i32) / 2;
/// Colonne du panneau quand l'étagère est sortie (à gauche d'Asti)...
const X_SHOWN: i32 = crate::asti::HOME_OX as i32 - PANEL_W - 90;
/// ...et quand elle est repliée (coulissée hors écran, derrière Asti).
const X_HIDDEN: i32 = fb::WIDTH as i32 + 6;

static mut TAKEN_UNTIL: [f32; 9] = [0.0; 9];

pub fn init() {
    install_palette();
    unsafe {
        TAKEN_UNTIL = [0.0; 9];
    }
}

fn panel_x(out: f32) -> i32 {
    (X_HIDDEN as f32 + (X_SHOWN - X_HIDDEN) as f32 * out) as i32
}

fn slot_y(i: usize) -> i32 {
    TOP + i as i32 * SLOT
}

/// `TAKEN_UNTIL[i]` : instant jusqu'auquel la case `i` est vide.
/// `f32::INFINITY` = friandise en cours de glissement (drag).
fn is_taken(i: usize, now: f32) -> bool {
    unsafe { now < TAKEN_UNTIL[i] }
}

/// Friandise sous le curseur si l'étagère est sortie et la case pleine.
/// Zone de sélection généreuse (toute la case, un peu de marge en x).
pub fn hit(mx: i32, my: i32, now: f32) -> Option<Kind> {
    if mx < X_SHOWN - 24 || mx > X_SHOWN + PANEL_W + 36 {
        return None;
    }
    for (i, &k) in ALL.iter().enumerate() {
        if is_taken(i, now) {
            continue;
        }
        let y = slot_y(i) - (SLOT - CELL) / 2;
        if my >= y && my < y + SLOT {
            return Some(k);
        }
    }
    None
}

/// Prend une friandise pour la glisser (elle disparaît de l'étagère,
/// sans cooldown pour l'instant).
pub fn pick(kind: Kind) {
    unsafe { TAKEN_UNTIL[kind.idx()] = f32::INFINITY }
}

/// Le glissement a réussi (déposée sur Asti) : cooldown de ~2,6 s.
pub fn consume(kind: Kind, now: f32) {
    unsafe { TAKEN_UNTIL[kind.idx()] = now + 2.6 }
}

/// Le glissement a échoué : la friandise revient dans l'étagère.
pub fn restore(kind: Kind) {
    unsafe { TAKEN_UNTIL[kind.idx()] = 0.0 }
}

/// Position du bouton « info » (PC Pet Hub), au-dessus des friandises.
fn info_rect() -> (i32, i32, i32, i32) {
    (X_SHOWN, TOP - SLOT, PANEL_W, CELL)
}

/// Le curseur est-il sur le bouton info ?
pub fn info_hit(mx: i32, my: i32) -> bool {
    let (x, y, w, h) = info_rect();
    mx >= x - 20 && mx <= x + w + 30 && my >= y - 6 && my <= y + h + 6
}

pub fn draw(out: f32, now: f32) {
    let x = panel_x(out);

    // panneau arrondi (englobe aussi le bouton info, plus haut)
    let h = SLOT * (ALL.len() as i32 + 1);
    let top = TOP - SLOT;
    fb::fill_rect(x - 3, top - 5, PANEL_W + 6, h + 6, PAL_PANEL_EDGE);
    fb::fill_rect(x - 2, top - 4, PANEL_W + 4, h + 4, PAL_PANEL);

    // bouton info : un "i" dans un cercle
    let (ix, iy) = (x + PANEL_W / 2, TOP - SLOT + CELL / 2);
    fb::fill_circle(ix as f32, iy as f32, (CELL / 2 - 6) as f32, PAL_DOT_DIM);
    fb::fill_circle(ix as f32, iy as f32, (CELL / 2 - 9) as f32, PAL_PANEL);
    fb::fill_rect(ix - 2, iy - 12, 5, 5, PAL_DOT);
    fb::fill_rect(ix - 2, iy - 3, 5, 15, PAL_DOT);
    // séparateur
    fb::fill_rect(x + 6, TOP - 6, PANEL_W - 12, 1, PAL_PANEL_EDGE);

    for (i, &k) in ALL.iter().enumerate() {
        if is_taken(i, now) {
            continue;
        }
        draw_treat(k, x, slot_y(i));
    }
}

/// Un point du motif = un pavé `SCALE×SCALE`, pour garder l'aspect
/// "matrice de LED" comme sur Asti.
const SCALE: i32 = 4;

fn draw_treat(kind: Kind, ox: i32, oy: i32) {
    let pat = kind.pattern();
    let ph = pat.len() as i32 * SCALE;
    let pw = pat[0].len() as i32 * SCALE;
    draw_treat_at(kind, ox + (PANEL_W - pw) / 2, oy + (CELL - ph) / 2, SCALE);
}

/// Dessine une friandise en points à l'échelle voulue, coin haut-gauche
/// en (x, y). Sert aussi à dessiner la friandise "en vol" pendant le
/// glisser-déposer.
pub fn draw_treat_at(kind: Kind, x: i32, y: i32, scale: i32) {
    for (row, line) in kind.pattern().iter().enumerate() {
        for (col, ch) in line.bytes().enumerate() {
            let c = match ch {
                b'#' => PAL_DOT,
                b'o' => PAL_DOT_DIM,
                _ => continue,
            };
            fb::fill_rect(x + col as i32 * scale, y + row as i32 * scale, scale, scale, c);
        }
    }
}

/// Dimensions (largeur, hauteur) d'une friandise à l'échelle `scale`.
pub fn treat_size(kind: Kind, scale: i32) -> (i32, i32) {
    let pat = kind.pattern();
    (pat[0].len() as i32 * scale, pat.len() as i32 * scale)
}
