//! Asti — portage du moteur de rendu de l'appli « PC Pet »
//! (`renderer/engine.js` + `renderer/pet.js`).
//!
//! Matrice de LED circulaire 25×25. Le visage (yeux + bouche, mode
//! « face ») est tracé dans un buffer de luminance `f32` avec les mêmes
//! primitives que l'original, puis `render()` le projette en points.
//!
//! Animations portées : repos (blink, regards, twitch, bâillement,
//! sommeil), mode selon l'heure (jour / soir / nuit), poses de
//! dégustation (une par friandise) et petites humeurs spontanées
//! (content, amour, coucou, danse, tête qui tourne, sursaut...).
//! Les poses spécifiques aux applis (Discord, VS Code...) n'ont pas de
//! sens ici et ne sont pas portées.

#![allow(dead_code)]

use core::f32::consts::PI;

use libm::{cosf, fabsf, fmaxf, roundf, sinf, sqrtf};

use crate::fb;

const N: usize = 25;
const CENTER: f32 = 12.0;
const R: f32 = 12.4;
const BODY_H: f32 = 8.0;

fn hyp(dx: f32, dy: f32) -> f32 {
    sqrtf(dx * dx + dy * dy)
}
fn clamp01(v: f32) -> f32 {
    v.max(0.0).min(1.0)
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
fn wave(t: f32, hz: f32) -> f32 {
    sinf(t * PI * 2.0 * hz)
}

// ---------------------------------------------------------------------
// Buffer de luminance + primitives (engine.js §2).
// ---------------------------------------------------------------------

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
        let v = v.min(1.0);
        if v > self.buf[i] {
            self.buf[i] = v;
        }
    }

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
                        self.buf[y as usize * N + x as usize] *= 1.0 - a;
                    }
                }
                x += 1;
            }
            y += 1;
        }
    }

    fn stroke(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, w: f32, v: f32) {
        let s = ceili(hyp(x2 - x1, y2 - y1) * 2.0).max(1);
        let mut i = 0;
        while i <= s {
            let t = i as f32 / s as f32;
            self.disc(x1 + (x2 - x1) * t, y1 + (y2 - y1) * t, w, v);
            i += 1;
        }
    }

    fn line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, v: f32) {
        self.stroke(x1, y1, x2, y2, 0.55, v);
    }
}

// ---------------------------------------------------------------------
// Styles d'yeux / de bouche (engine.js : drawEye / drawMouth, mode face).
// ---------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum EyeStyle {
    Dot,
    Calm,
    Arc,
    Sleep,
    Wide,
    Sparkle,
    Heart,
    Squint,
    Half,
    Angry,
    Spiral,
}

#[derive(Clone, Copy, PartialEq)]
enum Mouth {
    Smile,
    Grin,
    Cat,
    O,
    Line,
    Flat,
    Wobble,
    None,
}

fn draw_eye(cv: &mut Canvas, x: f32, y: f32, style: EyeStyle, open: f32, t: f32, bright: f32, k: f32) {
    let fill = |cv: &mut Canvas, dx: f32, dy: f32, r: f32| cv.disc(x + dx * k, y + dy * k, r * k, bright);
    let cut = |cv: &mut Canvas, dx: f32, dy: f32, r: f32| cv.hole(x + dx * k, y + dy * k, r * k);
    let seg = |cv: &mut Canvas, ax: f32, ay: f32, bx: f32, by: f32, w: f32| {
        cv.stroke(x + ax * k, y + ay * k, x + bx * k, y + by * k, w * k + 0.05, bright);
    };
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
        EyeStyle::Dot => {
            if open > 0.35 {
                fill(cv, 0.0, 0.0, 1.05);
            } else {
                seg(cv, -1.1, 0.0, 1.1, 0.0, 0.38);
            }
        }
        EyeStyle::Calm => arc(cv, -1.0),
        EyeStyle::Arc => arc(cv, 1.0),
        EyeStyle::Sleep => seg(cv, -1.15, 0.0, 1.15, 0.0, 0.42),
        EyeStyle::Squint => seg(cv, -1.0, 0.0, 1.0, 0.0, 0.4),
        EyeStyle::Wide => {
            fill(cv, 0.0, 0.0, 1.6);
            cut(cv, 0.0, 0.0, 0.78);
        }
        EyeStyle::Sparkle => {
            fill(cv, 0.0, 0.0, 1.25);
            cut(cv, -0.4, -0.4, 0.34);
        }
        EyeStyle::Heart => {
            fill(cv, -0.62, -0.28, 0.72);
            fill(cv, 0.62, -0.28, 0.72);
            let mut s = 0.0;
            while s <= 1.0 {
                fill(cv, 0.0, s * 1.5, 0.85 * (1.0 - s) + 0.12);
                s += 0.14;
            }
        }
        EyeStyle::Half => {
            seg(cv, -1.2, -0.2, 1.2, -0.2, 0.4);
            if open > 0.4 {
                cut(cv, 0.0, 0.55, 0.5);
            }
        }
        EyeStyle::Angry => {
            // sign porté par l'appelant via k ; ici trait oblique simple
            seg(cv, -1.0, -0.7, 0.9, 0.35, 0.42);
            fill(cv, 0.15, 0.55, 0.45);
        }
        EyeStyle::Spiral => {
            let ph = t * 7.0;
            let mut a = 0.0;
            while a < PI * 2.6 {
                let rr = 0.1 + a * 0.17;
                fill(cv, cosf(a + ph) * rr, sinf(a + ph) * rr, 0.34);
                a += 0.45;
            }
        }
    }
}

fn draw_mouth(cv: &mut Canvas, x: f32, y: f32, kind: Mouth, t: f32, bright: f32, k: f32) {
    let seg = |cv: &mut Canvas, ax: f32, ay: f32, bx: f32, by: f32, w: f32| {
        cv.stroke(x + ax * k, y + ay * k, x + bx * k, y + by * k, w * k + 0.05, bright);
    };
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
        Mouth::Smile => curve(cv, 1.5, 0.75, 0.4),
        Mouth::Grin => curve(cv, 1.9, 1.25, 0.45),
        Mouth::O => {
            cv.disc(x, y + 0.1 * k, 0.85 * k, bright);
            cv.hole(x, y + 0.1 * k, 0.4 * k);
        }
        Mouth::Cat => {
            for s in [-1.0_f32, 1.0] {
                let mut prev: Option<(f32, f32)> = None;
                let mut i = -0.85;
                while i <= 0.85 + 1e-4 {
                    let p = (s * 0.85 + i, 0.1 + 0.55 * (1.0 - (i / 0.85) * (i / 0.85)));
                    if let Some((px, py)) = prev {
                        seg(cv, px, py, p.0, p.1, 0.4);
                    }
                    prev = Some(p);
                    i += 0.425;
                }
            }
        }
        Mouth::Flat => {
            let w = 0.28;
            seg(cv, -1.3, w, -0.1, -w, 0.4);
            seg(cv, -0.1, -w, 1.1, w, 0.4);
        }
        Mouth::Wobble => {
            let w = wave(t, 5.0) * 0.5;
            seg(cv, -1.4, -w, 0.0, w, 0.42);
            seg(cv, 0.0, w, 1.4, -w, 0.42);
        }
    }
}

// ---------------------------------------------------------------------
// Extras (cœurs, étoiles, notes, Z, miettes...).
// ---------------------------------------------------------------------

#[derive(Clone, Copy)]
enum Extra {
    Heart(f32, f32),
    Star4(f32, f32, f32),
    Note(f32, f32),
    Z(f32, f32, f32),
    Crumb(f32),
    Stars(f32),
    Excl(f32, f32),
    Steam(f32, f32, f32),
    Hand(f32, f32, f32),
}

struct Extras {
    items: [Option<Extra>; 10],
    n: usize,
}
impl Extras {
    fn new() -> Extras {
        Extras {
            items: [None; 10],
            n: 0,
        }
    }
    fn push(&mut self, e: Extra) {
        if self.n < self.items.len() {
            self.items[self.n] = Some(e);
            self.n += 1;
        }
    }
}

fn draw_extra(cv: &mut Canvas, e: Extra, _t: f32, b: f32) {
    match e {
        Extra::Heart(x, y) => {
            cv.disc(x - 0.7, y, 0.8, b);
            cv.disc(x + 0.7, y, 0.8, b);
            cv.disc(x, y + 0.9, 1.1, b);
        }
        Extra::Star4(x, y, ph) => {
            let s = 0.7 + fabsf(sinf(ph * 3.0)) * 1.0;
            cv.stroke(x - s, y, x + s, y, 0.3, b);
            cv.stroke(x, y - s, x, y + s, 0.3, b);
        }
        Extra::Note(x, y) => {
            cv.disc(x, y, 0.7, b);
            cv.line(x + 0.6, y, x + 0.6, y - 2.0, b);
            cv.line(x + 0.6, y - 2.0, x + 1.5, y - 1.6, b);
        }
        Extra::Z(x, y, s) => {
            cv.line(x - 0.8 * s, y - 0.8 * s, x + 0.8 * s, y - 0.8 * s, b);
            cv.line(x + 0.8 * s, y - 0.8 * s, x - 0.8 * s, y + 0.8 * s, b);
            cv.line(x - 0.8 * s, y + 0.8 * s, x + 0.8 * s, y + 0.8 * s, b);
        }
        Extra::Crumb(p) => {
            for i in 0..3 {
                let a = p * 11.0 + i as f32 * 2.1;
                cv.disc(
                    CENTER + cosf(a) * (BODY_H * 0.7),
                    CENTER + 1.5 - fabsf(sinf(a)) * 2.0,
                    0.32,
                    b * 0.7,
                );
            }
        }
        Extra::Stars(p) => {
            for i in 0..3 {
                let a = p * 3.0 + i as f32 * 2.1;
                cv.disc(
                    CENTER + cosf(a) * (BODY_H + 1.8),
                    CENTER - BODY_H + sinf(a) * 2.5,
                    0.6,
                    b,
                );
            }
        }
        Extra::Excl(cx, cy) => {
            cv.line(cx, cy - BODY_H - 4.0, cx, cy - BODY_H - 2.0, b);
            cv.disc(cx, cy - BODY_H - 1.1, 0.6, b);
        }
        Extra::Steam(cx, cy, p) => {
            for s in [-1.0_f32, 1.0] {
                let pp = ((p * 0.9) + if s > 0.0 { 0.5 } else { 0.0 }) % 1.0;
                cv.disc(cx + s * (BODY_H + 1.0), cy - BODY_H + 1.0 - pp * 4.0, 0.5 + pp * 0.4, b * 0.45 * (1.0 - pp));
            }
        }
        Extra::Hand(cx, cy, off) => {
            cv.disc(cx + BODY_H * 0.95, cy + off, 1.4, b);
        }
    }
}

// ---------------------------------------------------------------------
// État + poses.
// ---------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    Day,
    Eve,
    Night,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Pose {
    Rest,
    Blink,
    Twitch,
    LookL,
    LookR,
    LookU,
    LookD,
    Yawn,
    Zzz,
    // dégustation
    Nom,
    Nibble,
    Gnaw,
    Gulp,
    Crunch,
    Spicy,
    Sugarrush,
    Recharge,
    Stuffed,
    // humeurs
    Happy,
    Love,
    Greet,
    Bounce,
    Dance,
    Dizzy,
    Alert,
    Sad,
    Grumpy,
    // humeurs "application" (tenues tant que la fenêtre est au 1er plan)
    AppCode,
    AppTerm,
    AppChat,
    AppGit,
    AppWeb,
    Hub,
}

pub struct State {
    pub layer: u8,
    pub pose: Pose,
    pub phase: f32,
    pub energy: f32,
    pub mode: Mode,
}

pub fn draw_creature(cv: &mut Canvas, s: &State, t: f32) {
    let mut cx = CENTER;
    let mut cy = CENTER;
    let mut tilt = 0.0_f32;
    let mut look_h = 0.0_f32;
    let mut look_v = 0.0_f32;
    let mut eye = EyeStyle::Dot;
    let mut eye_open = 1.0_f32;
    let mut mouth = Mouth::Smile;
    let mut bright = 1.0_f32;
    let mut blush = false;
    let mut ex = Extras::new();
    let p = s.phase;

    // couche 1 : pose de repos selon l'heure
    match s.mode {
        Mode::Night => {
            eye = EyeStyle::Sleep;
            mouth = Mouth::None;
            bright = 0.5;
        }
        Mode::Eve => {
            eye = EyeStyle::Calm;
            mouth = Mouth::Line;
            bright = 0.85;
        }
        Mode::Day => {}
    }

    // couche 2 : micro-animations
    if s.layer == 2 {
        let sp = sinf(p * PI);
        match s.pose {
            Pose::Blink => {
                if sp > 0.35 {
                    eye = EyeStyle::Calm;
                }
            }
            Pose::Twitch => tilt = sinf(p * PI * 3.0) * 0.15,
            Pose::LookL => {
                look_h = -3.0 * sp;
                eye = EyeStyle::Dot;
            }
            Pose::LookR => {
                look_h = 3.0 * sp;
                eye = EyeStyle::Dot;
            }
            Pose::LookU => {
                look_v = -2.2 * sp;
                eye = EyeStyle::Dot;
            }
            Pose::LookD => {
                look_v = 2.2 * sp;
                eye = EyeStyle::Dot;
            }
            Pose::Yawn => {
                mouth = Mouth::O;
                eye = EyeStyle::Calm;
            }
            Pose::Zzz => {
                eye = EyeStyle::Sleep;
                mouth = Mouth::None;
                ex.push(Extra::Z(cx + 6.0 + (p * 3.0) % 3.0, cy - 4.5 - ((p * 6.0) % 3.0) * 1.8, 1.0 + p));
            }
            _ => {}
        }
    }

    // couche 3 : réactions (dégustation, humeurs)
    if s.layer == 3 {
        eye = EyeStyle::Dot;
        bright = 1.0;
        match s.pose {
            Pose::Nom => {
                let chomp = (t * 9.0) % 1.0;
                eye = EyeStyle::Arc;
                mouth = if chomp < 0.5 { Mouth::O } else { Mouth::Grin };
                cy -= fabsf(sinf(t * 9.0)) * 0.5;
                blush = true;
                if (t % 0.7) > 0.4 {
                    ex.push(Extra::Crumb(t));
                }
                if (t % 1.6) > 1.3 {
                    ex.push(Extra::Heart(cx, cy - BODY_H - 1.5));
                }
            }
            Pose::Nibble => {
                let bite = (t * 5.0) % 1.0;
                eye = EyeStyle::Arc;
                mouth = if bite < 0.35 { Mouth::O } else { Mouth::Cat };
                blush = true;
                cy += sinf(t * 5.0) * 0.18;
                tilt = sinf(t * 2.2) * 0.06;
                if (t % 1.1) > 0.7 {
                    ex.push(Extra::Heart(cx + 4.5, cy - BODY_H - 1.0 - ((t * 1.6) % 3.0)));
                }
            }
            Pose::Gnaw => {
                eye = EyeStyle::Squint;
                mouth = Mouth::Grin;
                tilt = sinf(t * 16.0) * 0.22;
                cx += sinf(t * 16.0) * 0.7;
                cy -= fabsf(sinf(t * 8.0)) * 0.5;
                if (t % 1.4) > 1.1 {
                    ex.push(Extra::Crumb(t));
                }
            }
            Pose::Gulp => {
                let q = (t % 1.6) / 1.6;
                eye = if q < 0.55 { EyeStyle::Dot } else { EyeStyle::Arc };
                mouth = if q < 0.4 {
                    Mouth::O
                } else if q < 0.6 {
                    Mouth::Grin
                } else {
                    Mouth::Cat
                };
                tilt = if q < 0.5 { -q * 0.5 } else { -(1.0 - q) * 0.5 };
                cy += (if q < 0.5 { -q } else { -(1.0 - q) }) * 1.6;
                blush = true;
            }
            Pose::Crunch => {
                let chomp = (t * 12.0) % 1.0;
                eye = EyeStyle::Squint;
                mouth = if chomp < 0.5 { Mouth::Line } else { Mouth::O };
                cy -= fabsf(sinf(t * 12.0)) * 0.55;
                tilt = sinf(t * 12.0) * 0.04;
                if chomp < 0.15 {
                    ex.push(Extra::Crumb(t));
                }
            }
            Pose::Spicy => {
                let q = t % 2.6;
                if q < 0.5 {
                    eye = EyeStyle::Dot;
                    mouth = Mouth::O;
                } else {
                    eye = EyeStyle::Wide;
                    mouth = Mouth::O;
                    tilt = wave(t, 9.0) * 0.4;
                    cx += wave(t, 9.0) * 1.2;
                    ex.push(Extra::Steam(cx, cy, t));
                    ex.push(Extra::Excl(cx, cy));
                    if q > 1.9 {
                        blush = true;
                    }
                }
            }
            Pose::Sugarrush => {
                let q = t % 2.4;
                if q < 0.5 {
                    eye = EyeStyle::Wide;
                    mouth = Mouth::O;
                    cy -= fabsf(sinf(t * 12.0)) * 0.4;
                } else {
                    eye = EyeStyle::Spiral;
                    mouth = Mouth::Grin;
                    tilt = (t * 6.0) % (PI * 2.0);
                    cx += sinf(t * 22.0) * 0.9;
                    cy -= fabsf(wave(t, 5.0)) * 1.8;
                }
                for i in 0..3 {
                    let a = t * 5.0 + i as f32 * 2.1;
                    ex.push(Extra::Star4(cx + cosf(a) * (BODY_H + 2.0), cy - BODY_H + sinf(a) * 2.4, t * 4.0));
                }
            }
            Pose::Recharge => {
                let q = ((t % 3.0) / 2.4).min(1.0);
                bright = 0.45 + q * 0.7;
                eye = if q < 0.4 {
                    EyeStyle::Half
                } else if q < 0.85 {
                    EyeStyle::Dot
                } else {
                    EyeStyle::Wide
                };
                eye_open = if q < 0.4 { 0.4 } else { 1.0 };
                mouth = if q < 0.85 { Mouth::Line } else { Mouth::Grin };
                cy += (1.0 - q) * 1.4;
                if (t % 0.4) < 0.2 {
                    ex.push(Extra::Star4(cx, cy - BODY_H - 2.0, t * 6.0));
                }
            }
            Pose::Stuffed => {
                eye = EyeStyle::Half;
                eye_open = 0.5;
                mouth = Mouth::Smile;
                tilt = sinf(t * 1.4) * 0.09;
                cy += sinf(t * 1.4) * 0.2;
            }
            Pose::Happy => {
                let b = fabsf(wave(t, 3.0));
                cy -= b * 1.8;
                eye = EyeStyle::Heart;
                mouth = Mouth::Grin;
                blush = true;
                ex.push(Extra::Heart(cx, cy - BODY_H - 1.5 - b));
            }
            Pose::Love => {
                let b = fabsf(wave(t, 2.0));
                cy -= b * 1.6;
                eye = EyeStyle::Heart;
                mouth = Mouth::Grin;
                blush = true;
                tilt = sinf(t * 4.0) * 0.1;
                for i in 0..3 {
                    let a = t * 3.0 + i as f32 * 2.1;
                    ex.push(Extra::Heart(cx + cosf(a) * (BODY_H + 2.0), cy - BODY_H + sinf(a) * 2.4));
                }
            }
            Pose::Greet => {
                let b = fabsf(wave(t, 2.0));
                cy -= b * 1.6;
                eye = EyeStyle::Arc;
                mouth = Mouth::Smile;
                blush = true;
                ex.push(Extra::Hand(cx, cy, -1.0 - b * 2.0));
            }
            Pose::Bounce => {
                let b = fabsf(wave(t, 2.6));
                cy -= b * 3.2;
                eye = EyeStyle::Arc;
                mouth = Mouth::Grin;
                blush = true;
            }
            Pose::Dance => {
                let sway = wave(t, 2.0);
                cx += sway * 2.6;
                tilt = sway * 0.7;
                cy -= fabsf(wave(t, 4.0)) * 1.6;
                eye = EyeStyle::Arc;
                mouth = Mouth::O;
                ex.push(Extra::Note(cx + 7.0 + wave(t, 1.0), cy - 5.5 - ((t * 1.2) % 3.5)));
            }
            Pose::Dizzy => {
                eye = EyeStyle::Spiral;
                mouth = Mouth::Wobble;
                ex.push(Extra::Stars(t));
            }
            Pose::Alert => {
                let j = fabsf(wave(t, 2.2));
                cy -= j * 2.2;
                eye = EyeStyle::Wide;
                mouth = Mouth::O;
                ex.push(Extra::Excl(cx, cy));
            }
            Pose::Sad => {
                eye = EyeStyle::Calm;
                mouth = Mouth::Flat;
                look_v = 1.2;
                cy += sinf(t * 1.5) * 0.3;
            }
            Pose::Grumpy => {
                eye = EyeStyle::Angry;
                mouth = Mouth::Flat;
                tilt = wave(t, 0.6) * 0.15;
                bright = 0.8;
            }
            // --- humeurs "application" (portées de engine.js) ---
            Pose::AppCode => {
                eye = if (t % 4.6) > 4.4 { EyeStyle::Calm } else { EyeStyle::Dot };
                mouth = Mouth::Line;
                look_v = 0.5;
                look_h = sinf(t * 1.3) * 0.5;
                cy += sinf(t * 4.2) * 0.14;
                tilt = sinf(t * 0.4) * 0.02;
            }
            // terminal : ambiance "matrix" — regard qui scanne les lignes,
            // clignements vifs, légère oscillation (scène 'matrix' de PC Pet).
            Pose::AppTerm => {
                eye = if (t * 3.0) as i32 % 5 == 0 {
                    EyeStyle::Wide
                } else {
                    EyeStyle::Squint
                };
                mouth = Mouth::Line;
                look_h = (((t * 1.1) % 2.0) - 1.0) * 2.8; // balayage gauche/droite
                look_v = 0.4;
                cy += sinf(t * 6.0) * 0.1;
                tilt = sinf(t * 0.5) * 0.02;
                ex.push(Extra::Crumb(t * 0.6));
            }
            Pose::AppChat => {
                eye = EyeStyle::Arc;
                mouth = Mouth::Smile;
                blush = true;
                cx += sinf(t * 1.7) * 1.2;
                tilt = sinf(t * 1.7) * 0.12;
                if (t % 2.0) > 1.4 {
                    ex.push(Extra::Note(cx + 6.0, cy - BODY_H - 1.0 - ((t * 1.3) % 3.5)));
                }
            }
            Pose::AppGit => {
                eye = if (t % 3.4) > 3.2 { EyeStyle::Calm } else { EyeStyle::Dot };
                mouth = Mouth::Line;
                look_v = 0.2 + sinf(t * 0.6) * 0.25;
                tilt = sinf(t * 0.35) * 0.03;
            }
            Pose::AppWeb => {
                eye = EyeStyle::Dot;
                mouth = Mouth::Smile;
                look_h = sinf(t * 0.9) * 2.2;
                look_v = sinf(t * 0.5) * 0.4;
                cy += sinf(t * 3.0) * 0.1;
            }
            Pose::Hub => {
                let b = fabsf(wave(t, 2.5));
                cy -= b * 1.2;
                eye = EyeStyle::Heart;
                mouth = Mouth::Grin;
                blush = true;
            }
            _ => {}
        }
    }

    bright *= 0.6 + 0.4 * s.energy;
    let feat = 1.45_f32;
    let face_b = fmaxf(bright, if s.mode == Mode::Day { 0.92 } else { bright });

    // rotation (tilt) autour du centre
    let (co, si) = (cosf(tilt), sinf(tilt));
    let rot = |dx: f32, dy: f32| (cx + dx * co - dy * si, cy + dx * si + dy * co);

    let (eye_dx, eye_y, mouth_y) = (4.2_f32, -2.6_f32, 4.2_f32);
    for sg in [-1.0_f32, 1.0] {
        let (px, py) = rot(sg * eye_dx + look_h * 0.7, eye_y + look_v * 0.7);
        draw_eye(cv, px, py, eye, eye_open, t, face_b, feat);
    }
    if mouth != Mouth::None {
        let (mx, my) = rot(0.0, mouth_y);
        draw_mouth(cv, mx, my, mouth, t, face_b, feat);
    }
    if blush {
        for sg in [-1.0_f32, 1.0] {
            for i in -1..=1 {
                let (bx, by) = rot(sg * eye_dx * 1.15 + i as f32 * 0.95, mouth_y - 0.7);
                cv.stroke(bx - 0.35, by + 0.8, bx + 0.35, by - 0.8, 0.26, face_b * 0.8);
            }
        }
    }

    for i in 0..ex.n {
        if let Some(e) = ex.items[i] {
            draw_extra(cv, e, t, face_b);
        }
    }
}

// ---------------------------------------------------------------------
// Teintes + palette (inchangé).
// ---------------------------------------------------------------------

struct TintRgb {
    bg: [u8; 3],
    off: [u8; 3],
    off_a: f32,
    lit: [u8; 3],
    glow: [u8; 3],
}

#[derive(Clone, Copy, PartialEq)]
pub enum Tint {
    Null,
    Code,
    Matrix,
    Chat,
    Git,
    Web,
}

fn tint_rgb(t: Tint) -> TintRgb {
    match t {
        Tint::Null => TintRgb {
            bg: [26, 28, 38],
            off: [224, 231, 247],
            off_a: 0.11,
            lit: [233, 239, 253],
            glow: [200, 220, 255],
        },
        // éditeur : bleu (VS Code)
        Tint::Code => TintRgb {
            bg: [10, 20, 32],
            off: [0, 122, 204],
            off_a: 0.16,
            lit: [170, 210, 255],
            glow: [0, 122, 204],
        },
        // terminal : vert "matrix"
        Tint::Matrix => TintRgb {
            bg: [6, 16, 8],
            off: [80, 220, 120],
            off_a: 0.12,
            lit: [180, 255, 190],
            glow: [90, 255, 140],
        },
        // Discord : bleu-violet
        Tint::Chat => TintRgb {
            bg: [18, 19, 34],
            off: [112, 122, 238],
            off_a: 0.14,
            lit: [202, 208, 255],
            glow: [88, 101, 242],
        },
        // Git : ardoise
        Tint::Git => TintRgb {
            bg: [16, 18, 24],
            off: [139, 148, 158],
            off_a: 0.16,
            lit: [180, 195, 210],
            glow: [88, 166, 255],
        },
        // Web : vert doux
        Tint::Web => TintRgb {
            bg: [10, 24, 12],
            off: [120, 210, 130],
            off_a: 0.13,
            lit: [210, 255, 210],
            glow: [120, 220, 130],
        },
    }
}

const PAL_BG: u8 = 1;
const PAL_OFF: u8 = 2;
const PAL_RIM: u8 = 3;
const PAL_BG_HI: u8 = 4;
const PAL_BG_LOW: u8 = 5;
const PAL_LIT: u8 = 16;
const PAL_GLOW: u8 = 32;
const LEVELS: usize = 9;

fn mix(fg: [u8; 3], a: f32, bg: [u8; 3]) -> (u8, u8, u8) {
    let m = |i: usize| (fg[i] as f32 * a + bg[i] as f32 * (1.0 - a)) as u8;
    (m(0), m(1), m(2))
}

pub fn install_palette(t: Tint) {
    let c = tint_rgb(t);
    fb::set_palette(0, 0, 0, 0);
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
        let (r, g, b) = mix(c.glow, 0.10 + q * 0.28, c.bg);
        fb::set_palette(PAL_GLOW + l as u8, r, g, b);
    }
}

// ---------------------------------------------------------------------
// Projection sur l'écran.
// ---------------------------------------------------------------------

const CELL: f32 = 16.0;
const OY: f32 = 62.0;
pub const HOME_OX: f32 = fb::WIDTH as f32 - N as f32 * CELL - 56.0;

pub fn grid_span() -> f32 {
    N as f32 * CELL
}
pub fn disc_center(ox: f32) -> (f32, f32) {
    (ox + CENTER * CELL + CELL * 0.5, OY + CENTER * CELL + CELL * 0.5)
}
pub fn disc_radius() -> f32 {
    (R + 0.6) * CELL
}

pub fn render(cv: &Canvas, ox: f32) {
    let (dcx, dcy) = disc_center(ox);
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
            let f = dy / rad;
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

    for y in 0..N {
        for x in 0..N {
            if hyp(x as f32 - CENTER, y as f32 - CENTER) > R {
                continue;
            }
            let (px, py) = (ox + x as f32 * CELL + CELL * 0.5, OY + y as f32 * CELL + CELL * 0.5);
            fb::fill_circle(px, py, CELL * 0.30, PAL_OFF);
        }
    }
    for pass in 0..2 {
        for y in 0..N {
            for x in 0..N {
                let v = cv.buf[y * N + x];
                if v <= 0.02 || hyp(x as f32 - CENTER, y as f32 - CENTER) > R {
                    continue;
                }
                let q = roundf(v * 8.0) / 8.0;
                let lvl = (q * 8.0) as usize;
                let (px, py) = (ox + x as f32 * CELL + CELL * 0.5, OY + y as f32 * CELL + CELL * 0.5);
                if pass == 0 {
                    fb::fill_circle(px, py, CELL * 0.30 + CELL * 0.34 * q, PAL_GLOW + lvl as u8);
                } else {
                    fb::fill_circle(px, py, CELL * 0.36, PAL_LIT + lvl as u8);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------
// Brain : planificateur des animations.
// ---------------------------------------------------------------------

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
    fn pick<T: Copy>(&mut self, s: &[T]) -> T {
        s[(self.f32() * s.len() as f32) as usize % s.len()]
    }
}

/// Friandise → pose de dégustation + durée.
pub fn feed_pose(kind: crate::shelf::Kind) -> (Pose, f32) {
    use crate::shelf::Kind::*;
    match kind {
        Cookie | Donut => (Pose::Nom, 1.9),
        Berry => (Pose::Nibble, 1.9),
        Bone => (Pose::Gnaw, 2.2),
        Fish => (Pose::Gulp, 2.0),
        Candy => (Pose::Sugarrush, 2.8),
        Carrot => (Pose::Crunch, 1.9),
        Chili => (Pose::Spicy, 2.6),
        Battery => (Pose::Recharge, 3.0),
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
    react: Option<Pose>,
    react_until: f32,
    scene_next: f32,
    mode: Mode,
    app_pose: Option<Pose>,
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
            react: None,
            react_until: 0.0,
            scene_next: 12.0,
            mode: Mode::Day,
            app_pose: None,
        }
    }

    /// Déclenche une pose de réaction (couche 3) pour `dur` secondes.
    pub fn react(&mut self, pose: Pose, dur: f32, now: f32) {
        self.react = Some(pose);
        self.react_until = now + dur;
    }

    /// Humeur tenue tant qu'une appli est au premier plan (`None` = repos).
    pub fn set_app(&mut self, pose: Option<Pose>) {
        self.app_pose = pose;
    }

    pub fn react_feed(&mut self, kind: crate::shelf::Kind, now: f32) {
        let (pose, dur) = feed_pose(kind);
        self.react(pose, dur, now);
        self.energy = (self.energy + 0.12).min(1.15);
    }

    fn set_mode(&mut self, hour: u8) {
        self.mode = if hour >= 22 || hour < 7 {
            Mode::Night
        } else if hour >= 19 {
            Mode::Eve
        } else {
            Mode::Day
        };
    }

    fn schedule(&mut self, now: f32) {
        let a = match self.mode {
            Mode::Night => self.rng.pick(&[Pose::Zzz, Pose::Zzz, Pose::Twitch]),
            Mode::Eve => self.rng.pick(&[Pose::Blink, Pose::Blink, Pose::LookL, Pose::LookR, Pose::Yawn]),
            Mode::Day => self.rng.pick(&[
                Pose::Blink,
                Pose::Blink,
                Pose::Blink,
                Pose::LookL,
                Pose::LookR,
                Pose::LookU,
                Pose::LookD,
                Pose::Twitch,
            ]),
        };
        let (dur, gap) = match a {
            Pose::Blink => (0.16, self.rng.range(0.9, 3.2)),
            Pose::Twitch => (0.22, self.rng.range(3.0, 8.0)),
            Pose::Yawn => (1.7, self.rng.range(4.0, 9.0)),
            Pose::Zzz => (2.6, self.rng.range(0.6, 2.0)),
            _ => (1.3, self.rng.range(2.2, 6.0)),
        };
        self.anim = Some(a);
        self.start = now;
        self.dur = dur;
        self.next = now + dur + gap;
    }

    pub fn update(&mut self, now: f32) -> State {
        let dt = (now - self.last_t).max(0.0);
        self.last_t = now;
        let k = 1.0 - libm::powf(0.5, dt / 4.0);
        self.energy += (0.8 - self.energy) * k;

        self.set_mode(crate::rtc::now().hour);

        // réaction en cours (dégustation / humeur déclenchée)
        if let Some(pose) = self.react {
            if now < self.react_until {
                return State {
                    layer: 3,
                    pose,
                    phase: 0.0,
                    energy: self.energy,
                    mode: self.mode,
                };
            }
            self.react = None;
        }

        // humeur "application" : tenue tant qu'une fenêtre est au 1er plan
        if let Some(pose) = self.app_pose {
            return State {
                layer: 3,
                pose,
                phase: 0.0,
                energy: self.energy,
                mode: self.mode,
            };
        }

        // petites humeurs spontanées au repos (jour uniquement)
        if self.mode == Mode::Day && now > self.scene_next {
            self.scene_next = now + self.rng.range(22.0, 55.0);
            if self.rng.f32() < 0.5 {
                let m = self.rng.pick(&[
                    Pose::Happy,
                    Pose::Love,
                    Pose::Greet,
                    Pose::Bounce,
                    Pose::Dance,
                    Pose::Dizzy,
                ]);
                let d = self.rng.range(1.8, 3.0);
                self.react(m, d, now);
            }
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
                mode: self.mode,
            },
            None => State {
                layer: 1,
                pose: Pose::Rest,
                phase: 0.0,
                energy: self.energy,
                mode: self.mode,
            },
        }
    }
}
