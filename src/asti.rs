//! Asti — portage direct du moteur de rendu de l'appli « PC Pet »
//! (`renderer/engine.js` + `renderer/pet.js`).
//!
//! Asti est une petite matrice de LED circulaire 25×25. Le tracé se fait
//! dans un buffer de luminance `f32` avec les mêmes primitives que
//! l'original (`disc`, `hole`, `stroke`), puis `render()` projette chaque
//! point sur l'écran graphique : point « éteint » pour le disque, points
//! plus clairs (9 niveaux) pour le visage, avec un léger halo.
//!
//! Comportement au repos porté tel quel : pose `rest` (yeux ronds +
//! sourire) entrecoupée de micro-animations (`blink`, regards) planifiées
//! par `Brain`, exactement comme dans `pet.js`.
//!
//! Variantes d'yeux/bouche et teintes non encore utilisées : réservées
//! aux poses à venir (nuit, bâillement, contextes colorés).
#![allow(dead_code)]

use core::f32::consts::PI;

use libm::{fmaxf, roundf, sinf, sqrtf};

use crate::fb;

// ---------------------------------------------------------------------
// Buffer de luminance + primitives (engine.js, section 2).
// ---------------------------------------------------------------------

const N: usize = 25;
/// Centre de la matrice : `(N - 1) / 2`.
const CENTER: f32 = 12.0;
/// Rayon du disque de LED.
const R: f32 = 12.4;

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

pub struct Canvas {
    buf: [f32; N * N],
}

impl Canvas {
    pub fn new() -> Canvas {
        Canvas { buf: [0.0; N * N] }
    }

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

    /// Disque plein anti-crénelé.
    fn disc(&mut self, cx: f32, cy: f32, rad: f32, v: f32) {
        let (x0, x1) = (floori(cx - rad - 1.0), ceili(cx + rad + 1.0));
        let (y0, y1) = (floori(cy - rad - 1.0), ceili(cy + rad + 1.0));
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

    /// Creuse (multiplie la luminance par `1 - a`) — pour évider un trait.
    fn hole(&mut self, cx: f32, cy: f32, rad: f32) {
        let (x0, x1) = (floori(cx - rad - 1.0), ceili(cx + rad + 1.0));
        let (y0, y1) = (floori(cy - rad - 1.0), ceili(cy + rad + 1.0));
        let mut y = y0;
        while y <= y1 {
            let mut x = x0;
            while x <= x1 {
                if x >= 0 && y >= 0 && x < N as i32 && y < N as i32 {
                    let a = clamp01(rad + 0.5 - hyp(x as f32 - cx, y as f32 - cy));
                    if a > 0.0 {
                        let i = y as usize * N + x as usize;
                        self.buf[i] *= 1.0 - a;
                    }
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
// Visage (engine.js : drawEye / drawMouth, mode "face").
// ---------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum EyeStyle {
    Dot,
    Calm,
    Arc,
    Sleep,
}

#[derive(Clone, Copy, PartialEq)]
enum Mouth {
    Smile,
    Grin,
    Line,
    O,
    None,
}

/// `drawEye`, branche `carve = false`.
fn draw_eye(cv: &mut Canvas, x: f32, y: f32, style: EyeStyle, bright: f32, k: f32) {
    let seg = |cv: &mut Canvas, ax: f32, ay: f32, bx: f32, by: f32, w: f32| {
        cv.stroke(x + ax * k, y + ay * k, x + bx * k, y + by * k, w * k + 0.05, bright);
    };
    // arc(sign) : sign=-1 → ‿ (doux), sign=+1 → ∩ (rieur)
    let arc = |cv: &mut Canvas, sign: f32| {
        let (hw, amp, w) = (1.35_f32, 1.15_f32, 0.4_f32);
        let mut prev: Option<(f32, f32)> = None;
        let mut i = -hw;
        while i <= hw + 1e-4 {
            let py = sign * amp * ((i / hw) * (i / hw) - 0.5);
            if let Some((px, ppy)) = prev {
                seg(cv, px, ppy, i, py, w);
            }
            prev = Some((i, py));
            i += hw / 4.0;
        }
    };
    match style {
        EyeStyle::Dot => cv.disc(x, y, 1.05 * k, bright),
        EyeStyle::Sleep => seg(cv, -1.15, 0.0, 1.15, 0.0, 0.42),
        EyeStyle::Calm => arc(cv, -1.0),
        EyeStyle::Arc => arc(cv, 1.0),
    }
}

/// `drawMouth`, branche `carve = false`.
fn draw_mouth(cv: &mut Canvas, x: f32, y: f32, kind: Mouth, bright: f32, k: f32) {
    let seg = |cv: &mut Canvas, ax: f32, ay: f32, bx: f32, by: f32, w: f32| {
        cv.stroke(x + ax * k, y + ay * k, x + bx * k, y + by * k, w * k + 0.05, bright);
    };
    // curve(halfW, curv, w) : arc de bouche (curv > 0 = sourire)
    let curve = |cv: &mut Canvas, half: f32, curv: f32, w: f32| {
        let mut prev: Option<(f32, f32)> = None;
        let mut i = -half;
        while i <= half + 1e-4 {
            let py = -0.15 + curv * (1.0 - (i / half) * (i / half));
            if let Some((px, ppy)) = prev {
                seg(cv, px, ppy, i, py, w);
            }
            prev = Some((i, py));
            i += half / 4.0;
        }
    };
    match kind {
        Mouth::None => {}
        Mouth::Line => seg(cv, -1.0, 0.0, 1.0, 0.0, 0.4),
        Mouth::O => {
            cv.disc(x, y + 0.1 * k, 0.85 * k, bright);
            cv.hole(x, y + 0.1 * k, 0.4 * k);
        }
        Mouth::Smile => curve(cv, 1.5, 0.75, 0.4),
        Mouth::Grin => curve(cv, 1.9, 1.25, 0.45),
    }
}

// ---------------------------------------------------------------------
// drawCreature — sous-ensemble "repos" (couches 1 et 2).
// ---------------------------------------------------------------------

/// Ce que `Brain::update` renvoie pour un instant donné.
pub struct State {
    pub layer: u8,
    pub pose: Pose,
    pub phase: f32,
    pub energy: f32,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Pose {
    Rest,
    Blink,
    LookL,
    LookR,
    LookU,
    LookD,
    /// Il mange une friandise (`nom` de engine.js).
    Eat,
}

pub fn draw_creature(cv: &mut Canvas, s: &State, t: f32) {
    let cx = CENTER;
    let mut cy = CENTER;
    let mut look_h = 0.0_f32;
    let mut look_v = 0.0_f32;
    let mut eye_style = EyeStyle::Dot;
    let mut mouth = Mouth::Smile;
    let mut bright = 1.0_f32;

    // couche 2 : micro-animations
    if s.layer == 2 {
        let p = s.phase;
        let sp = sinf(p * PI);
        match s.pose {
            Pose::Blink => {
                if sp > 0.35 {
                    eye_style = EyeStyle::Calm;
                }
            }
            Pose::LookL => look_h = -3.0 * sp,
            Pose::LookR => look_h = 3.0 * sp,
            Pose::LookU => look_v = -2.2 * sp,
            Pose::LookD => look_v = 2.2 * sp,
            _ => {}
        }
    }

    // couche 3 : réaction "il mange" (nom)
    if s.layer == 3 && s.pose == Pose::Eat {
        let chomp = (t * 9.0) % 1.0;
        eye_style = EyeStyle::Arc;
        mouth = if chomp < 0.5 { Mouth::O } else { Mouth::Grin };
        cy -= libm::fabsf(sinf(t * 9.0)) * 0.5;
    }

    bright *= 0.6 + 0.4 * s.energy;

    // géométrie "mode visage"
    let eye_dx = 4.2_f32;
    let eye_y = -2.6_f32;
    let mouth_y = 4.2_f32;
    let feat = 1.45_f32;
    let face_bright = fmaxf(bright, 0.92);

    // tilt = 0 sur toutes ces poses → rot() est une simple translation
    for sg in [-1.0_f32, 1.0] {
        let ex = cx + sg * eye_dx + look_h * 0.7;
        let ey = cy + eye_y + look_v * 0.7;
        draw_eye(cv, ex, ey, eye_style, face_bright, feat);
    }

    draw_mouth(cv, cx, cy + mouth_y, mouth, face_bright, feat);
}

// ---------------------------------------------------------------------
// Teintes (pet.js : TINTS) + palette mode 13h.
// ---------------------------------------------------------------------

struct TintRgb {
    bg: [u8; 3],
    off: [u8; 3],
    off_a: f32,
    lit: [u8; 3],
    glow: [u8; 3],
}

#[derive(Clone, Copy)]
pub enum Tint {
    /// Défaut « sans contexte » : noir & blanc bleuté.
    Null,
    /// Vert « matrix ».
    Matrix,
    /// Ambre « claude ».
    Claude,
}

fn tint_rgb(t: Tint) -> TintRgb {
    match t {
        Tint::Null => TintRgb {
            // `bg` de PC Pet (14,15,20) éclairci : sur un fond noir pur
            // il faut que le boîtier se voie comme un vrai panneau.
            bg: [26, 28, 38],
            off: [224, 231, 247],
            off_a: 0.11,
            lit: [233, 239, 253],
            glow: [200, 220, 255],
        },
        Tint::Matrix => TintRgb {
            bg: [6, 16, 8],
            off: [80, 220, 120],
            off_a: 0.10,
            lit: [180, 255, 190],
            glow: [90, 255, 140],
        },
        Tint::Claude => TintRgb {
            bg: [28, 14, 8],
            off: [236, 140, 95],
            off_a: 0.11,
            lit: [255, 206, 174],
            glow: [240, 120, 72],
        },
    }
}

// Indices de palette réservés à Asti.
const PAL_BG: u8 = 1;
const PAL_OFF: u8 = 2;
const PAL_BG_HI: u8 = 4;
const PAL_BG_LOW: u8 = 5;
const PAL_LIT: u8 = 16; // 16..=24 (9 niveaux)
const PAL_GLOW: u8 = 32; // 32..=40 (9 niveaux)
const LEVELS: usize = 9;

fn mix(fg: [u8; 3], a: f32, bg: [u8; 3]) -> (u8, u8, u8) {
    let m = |i: usize| (fg[i] as f32 * a + bg[i] as f32 * (1.0 - a)) as u8;
    (m(0), m(1), m(2))
}

/// Charge la palette pour une teinte donnée. À rappeler si la teinte change.
pub fn install_palette(t: Tint) {
    let c = tint_rgb(t);

    fb::set_palette(0, 0, 0, 0); // hors du disque
    fb::set_palette(PAL_BG, c.bg[0], c.bg[1], c.bg[2]);
    let lift = |v: u8, d: i16| (v as i16 + d).clamp(0, 255) as u8;
    fb::set_palette(PAL_BG_HI, lift(c.bg[0], 10), lift(c.bg[1], 10), lift(c.bg[2], 12));
    fb::set_palette(PAL_BG_LOW, lift(c.bg[0], -8), lift(c.bg[1], -8), lift(c.bg[2], -10));

    let (r, g, b) = mix(c.off, c.off_a, c.bg);
    fb::set_palette(PAL_OFF, r, g, b);

    let (r, g, b) = mix(c.lit, 0.18, c.bg);
    fb::set_palette(PAL_RIM, r, g, b);

    for l in 0..LEVELS {
        let q = l as f32 / (LEVELS - 1) as f32;
        let (r, g, b) = mix(c.lit, 0.24 + q * 0.76, c.bg);
        fb::set_palette(PAL_LIT + l as u8, r, g, b);
        // halo : la couleur "glow" à faible opacité, croissante avec q
        let (r, g, b) = mix(c.glow, 0.10 + q * 0.28, c.bg);
        fb::set_palette(PAL_GLOW + l as u8, r, g, b);
    }
}

// ---------------------------------------------------------------------
// renderToScreen — projection de la matrice sur l'écran.
// ---------------------------------------------------------------------

/// Taille d'un point de matrice, en pixels écran.
const CELL: f32 = 9.5;
/// `oy` fixe : Asti est calé en haut, juste sous la barre de titre.
const OY: f32 = 20.0;

/// Origine de grille quand Asti est à sa place (coin haut-droit).
pub const HOME_OX: f32 = fb::WIDTH as f32 - N as f32 * CELL - 16.0;

/// Indice de palette du liseré du boîtier.
const PAL_RIM: u8 = 3;

/// Largeur/hauteur de la grille de points, en pixels.
pub fn grid_span() -> f32 {
    N as f32 * CELL
}

/// Centre écran du disque pour une origine de grille `ox` donnée.
pub fn disc_center(ox: f32) -> (f32, f32) {
    (ox + CENTER * CELL + CELL * 0.5, OY + CENTER * CELL + CELL * 0.5)
}

pub fn disc_radius() -> f32 {
    (R + 0.6) * CELL
}

/// Dessine Asti, coin haut-gauche de la grille à `ox` (permet de le
/// faire coulisser hors de l'écran).
pub fn render(cv: &Canvas, ox: f32) {
    let (dcx, dcy) = disc_center(ox);

    // Boîtier : disque net + fin liseré + léger relief vertical (haut
    // clair, bas sombre) comme le "boîtier" de PC Pet.
    let rad = disc_radius();
    fb::fill_circle(dcx, dcy, rad, PAL_RIM);
    let r2 = (rad - 1.5) * (rad - 1.5);
    let (x0, x1) = ((dcx - rad) as i32, (dcx + rad) as i32);
    let (y0, y1) = ((dcy - rad) as i32, (dcy + rad) as i32);
    for y in y0..=y1 {
        for x in x0..=x1 {
            let (dx, dy) = (x as f32 + 0.5 - dcx, y as f32 + 0.5 - dcy);
            if dx * dx + dy * dy > r2 {
                continue;
            }
            let f = dy / rad; // -1 (haut) .. +1 (bas)
            let c = if f < -0.35 {
                PAL_BG_HI
            } else if f > 0.4 {
                PAL_BG_LOW
            } else {
                PAL_BG
            };
            fb::put(x, y, c);
        }
    }

    // Passe 1 : points éteints (tout le disque).
    for y in 0..N {
        for x in 0..N {
            if hyp(x as f32 - CENTER, y as f32 - CENTER) > R {
                continue;
            }
            let (px, py) = (ox + x as f32 * CELL + CELL * 0.5, OY + y as f32 * CELL + CELL * 0.5);
            fb::fill_circle(px, py, CELL * 0.30, PAL_OFF);
        }
    }

    // Passe 2 : halo des points allumés.
    for y in 0..N {
        for x in 0..N {
            let v = cv.buf[y * N + x];
            if v <= 0.02 || hyp(x as f32 - CENTER, y as f32 - CENTER) > R {
                continue;
            }
            let q = roundf(v * 8.0) / 8.0;
            let lvl = (q * 8.0) as usize;
            let (px, py) = (ox + x as f32 * CELL + CELL * 0.5, OY + y as f32 * CELL + CELL * 0.5);
            fb::fill_circle(px, py, CELL * 0.30 + CELL * 0.34 * q, PAL_GLOW + lvl as u8);
        }
    }

    // Passe 3 : points allumés.
    for y in 0..N {
        for x in 0..N {
            let v = cv.buf[y * N + x];
            if v <= 0.02 || hyp(x as f32 - CENTER, y as f32 - CENTER) > R {
                continue;
            }
            let q = roundf(v * 8.0) / 8.0;
            let lvl = (q * 8.0) as usize;
            let (px, py) = (ox + x as f32 * CELL + CELL * 0.5, OY + y as f32 * CELL + CELL * 0.5);
            fb::fill_circle(px, py, CELL * 0.36, PAL_LIT + lvl as u8);
        }
    }
}

// ---------------------------------------------------------------------
// Brain — planificateur des micro-animations au repos (pet.js).
// ---------------------------------------------------------------------

/// xorshift32, graine issue du TSC au boot.
struct Rng(u32);

impl Rng {
    fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
    fn f32(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32
    }
    fn range(&mut self, a: f32, b: f32) -> f32 {
        a + self.f32() * (b - a)
    }
}

pub struct Brain {
    rng: Rng,
    energy: f32,
    anim: Option<Pose>,
    start: f32,
    dur: f32,
    next: f32,
    last_t: f32,
    /// Réaction ponctuelle (ex. « il mange ») active jusqu'à `react_until`.
    react_until: f32,
}

impl Brain {
    pub fn new(seed: u32) -> Brain {
        Brain {
            rng: Rng(seed | 1),
            energy: 0.7,
            anim: None,
            start: 0.0,
            dur: 0.0,
            next: 0.0,
            last_t: 0.0,
            react_until: 0.0,
        }
    }

    /// Déclenche la réaction « il mange une friandise » pour ~1,6 s.
    pub fn react_feed(&mut self, now: f32) {
        self.react_until = now + 1.6;
        self.energy = (self.energy + 0.12).min(1.15);
    }

    fn schedule(&mut self, now: f32) {
        // pool "jour" : blink ×3, lookL/R/U/D
        const POOL: [Pose; 7] = [
            Pose::Blink,
            Pose::Blink,
            Pose::Blink,
            Pose::LookL,
            Pose::LookR,
            Pose::LookU,
            Pose::LookD,
        ];
        let a = POOL[(self.rng.f32() * POOL.len() as f32) as usize % POOL.len()];
        let (dur, gap) = match a {
            Pose::Blink => (0.160, self.rng.range(0.9, 3.2)),
            Pose::LookU | Pose::LookD => (1.2, self.rng.range(2.2, 6.0)),
            _ => (1.3, self.rng.range(2.2, 6.0)),
        };
        self.anim = Some(a);
        self.start = now;
        self.dur = dur;
        self.next = now + dur + gap;
    }

    /// Fait avancer l'état et renvoie la pose à dessiner pour `now` (s).
    pub fn update(&mut self, now: f32) -> State {
        // relaxation de l'énergie vers 0.8 (cible "jour")
        let dt = (now - self.last_t).max(0.0);
        self.last_t = now;
        let k = 1.0 - libm::powf(0.5, dt / 4.0);
        self.energy += (0.8 - self.energy) * k;

        // réaction prioritaire
        if now < self.react_until {
            return State {
                layer: 3,
                pose: Pose::Eat,
                phase: 0.0,
                energy: self.energy,
            };
        }

        if self.next == 0.0 {
            self.schedule(now);
        } else {
            if self.anim.is_some() && now - self.start > self.dur {
                self.anim = None;
            }
            if now > self.next {
                self.schedule(now);
            }
        }

        match self.anim {
            Some(pose) => State {
                layer: 2,
                pose,
                phase: ((now - self.start) / self.dur).min(1.0),
                energy: self.energy,
            },
            None => State {
                layer: 1,
                pose: Pose::Rest,
                phase: 0.0,
                energy: self.energy,
            },
        }
    }
}
