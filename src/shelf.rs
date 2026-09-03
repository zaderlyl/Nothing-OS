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
const CELL: i32 = 24; // hauteur d'une case friandise
const SLOT: i32 = CELL + 4;
const PANEL_W: i32 = 26;
const TOP: i32 = (fb::HEIGHT as i32 - SLOT * ALL.len() as i32) / 2 + 4;
/// Colonne du panneau quand l'étagère est sortie (à gauche d'Asti)...
const X_SHOWN: i32 = crate::asti::HOME_OX as i32 - PANEL_W - 16;
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

fn is_taken(i: usize, now: f32) -> bool {
    unsafe { now < TAKEN_UNTIL[i] }
}

/// Friandise sous le curseur (si l'étagère est sortie), sinon `None`.
pub fn hit(mx: i32, my: i32) -> Option<Kind> {
    let px = X_SHOWN;
    if mx < px - 2 || mx > px + PANEL_W + 2 {
        return None;
    }
    for (i, &k) in ALL.iter().enumerate() {
        let y = slot_y(i);
        if my >= y && my < y + CELL {
            return Some(k);
        }
    }
    None
}

/// Marque la friandise comme prise (cooldown ~2,6 s). `false` si déjà prise.
pub fn take(kind: Kind, now: f32) -> bool {
    let i = kind.idx();
    if is_taken(i, now) {
        return false;
    }
    unsafe {
        TAKEN_UNTIL[i] = now + 2.6;
    }
    true
}

pub fn draw(out: f32, now: f32) {
    let x = panel_x(out);

    // panneau arrondi
    let h = SLOT * ALL.len() as i32;
    fb::fill_rect(x - 3, TOP - 5, PANEL_W + 6, h + 6, PAL_PANEL_EDGE);
    fb::fill_rect(x - 2, TOP - 4, PANEL_W + 4, h + 4, PAL_PANEL);

    for (i, &k) in ALL.iter().enumerate() {
        if is_taken(i, now) {
            continue;
        }
        draw_treat(k, x, slot_y(i));
    }
}

/// Un point du motif = un pavé `SCALE×SCALE` avec un léger espace, pour
/// garder l'aspect "matrice de LED" comme sur Asti.
const SCALE: i32 = 2;

fn draw_treat(kind: Kind, ox: i32, oy: i32) {
    let pat = kind.pattern();
    let ph = pat.len() as i32 * SCALE;
    let pw = pat[0].len() as i32 * SCALE;
    let sx = ox + (PANEL_W - pw) / 2;
    let sy = oy + (CELL - ph) / 2;
    for (y, row) in pat.iter().enumerate() {
        for (px, ch) in row.bytes().enumerate() {
            let c = match ch {
                b'#' => PAL_DOT,
                b'o' => PAL_DOT_DIM,
                _ => continue,
            };
            let bx = sx + px as i32 * SCALE;
            let by = sy + y as i32 * SCALE;
            fb::fill_rect(bx, by, SCALE, SCALE, c);
        }
    }
}
