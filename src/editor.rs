//! Éditeur de texte : édite directement le contenu d'un fichier du
//! système de fichiers RAM ([`crate::fs`]). Un état par fenêtre.

#![allow(dead_code, static_mut_refs)]

use crate::{fb, fs, font};

// codes spéciaux émis par kbd pour les flèches
pub const K_UP: u8 = 0x11;
pub const K_DOWN: u8 = 0x12;
pub const K_LEFT: u8 = 0x13;
pub const K_RIGHT: u8 = 0x14;

#[derive(Clone, Copy)]
struct Ed {
    fslot: usize,
    cur: usize,
    top_line: usize,
    used: bool,
}
const E0: Ed = Ed {
    fslot: 0,
    cur: 0,
    top_line: 0,
    used: false,
};

static mut EDS: [Ed; 6] = [E0; 6];

pub fn attach(win: usize, fslot: usize) {
    let len = fs::get(fslot).map(|f| f.len).unwrap_or(0);
    unsafe {
        EDS[win] = Ed {
            fslot,
            cur: len,
            top_line: 0,
            used: true,
        };
    }
}

pub fn fslot_of(win: usize) -> Option<usize> {
    unsafe {
        if EDS[win].used {
            Some(EDS[win].fslot)
        } else {
            None
        }
    }
}

fn insert(win: usize, c: u8) {
    let ed = unsafe { &mut EDS[win] };
    let f = fs::slot_mut(ed.fslot);
    if f.len >= fs::FCAP - 1 {
        return;
    }
    let cur = ed.cur.min(f.len);
    let mut i = f.len;
    while i > cur {
        f.data[i] = f.data[i - 1];
        i -= 1;
    }
    f.data[cur] = c;
    f.len += 1;
    ed.cur = cur + 1;
    fs::mark_dirty();
}

fn del_back(win: usize) {
    let ed = unsafe { &mut EDS[win] };
    let f = fs::slot_mut(ed.fslot);
    if ed.cur == 0 || f.len == 0 {
        return;
    }
    let cur = ed.cur.min(f.len);
    for i in (cur - 1)..f.len - 1 {
        f.data[i] = f.data[i + 1];
    }
    f.len -= 1;
    ed.cur = cur - 1;
    fs::mark_dirty();
}

fn line_col(data: &[u8], cur: usize) -> (usize, usize) {
    let mut line = 0;
    let mut col = 0;
    for &b in &data[..cur.min(data.len())] {
        if b == b'\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    (line, col)
}

fn offset_of_line(data: &[u8], target: usize) -> usize {
    let mut line = 0;
    for (i, &b) in data.iter().enumerate() {
        if line == target {
            return i;
        }
        if b == b'\n' {
            line += 1;
        }
    }
    data.len()
}

pub fn key(win: usize, c: u8) {
    let (fslot, cur) = {
        let ed = unsafe { &EDS[win] };
        (ed.fslot, ed.cur)
    };
    let flen = fs::get(fslot).map(|f| f.len).unwrap_or(0);
    match c {
        0x08 => del_back(win),
        b'\n' | 0x20..=0x7e => insert(win, if c == b'\n' { b'\n' } else { c }),
        K_LEFT => unsafe {
            if cur > 0 {
                EDS[win].cur = cur - 1;
            }
        },
        K_RIGHT => unsafe {
            if cur < flen {
                EDS[win].cur = cur + 1;
            }
        },
        K_UP | K_DOWN => {
            let f = fs::get(fslot).unwrap();
            let (line, col) = line_col(&f.data[..f.len], cur);
            let nl = if c == K_UP {
                line.saturating_sub(1)
            } else {
                line + 1
            };
            let base = offset_of_line(&f.data[..f.len], nl);
            // avance de `col` colonnes sans dépasser la fin de ligne
            let mut p = base;
            let mut k = 0;
            while p < f.len && f.data[p] != b'\n' && k < col {
                p += 1;
                k += 1;
            }
            unsafe {
                EDS[win].cur = p;
            }
        }
        _ => {}
    }
}

/// Dessine le contenu dans le rectangle du corps de la fenêtre.
pub fn draw(win: usize, x: i32, y: i32, w: i32, h: i32, focused: bool, t: f32) {
    const LH: i32 = 20;
    const PAD: i32 = 12;
    let ed = unsafe { &mut EDS[win] };
    let f = match fs::get(ed.fslot) {
        Some(f) => f,
        None => return,
    };
    let rows = ((h - 2 * PAD) / LH).max(1) as usize;
    let (cl, cc) = line_col(&f.data[..f.len], ed.cur);
    if cl < ed.top_line {
        ed.top_line = cl;
    } else if cl >= ed.top_line + rows {
        ed.top_line = cl + 1 - rows;
    }

    // fond éditeur + marge
    fb::fill_rect(x, y, w, h, 71); // P_CODE_BG
    fb::fill_rect(x, y, 52, h, 65); // marge n° de ligne

    let start = offset_of_line(&f.data[..f.len], ed.top_line);
    let mut line = ed.top_line;
    let mut sx = x + 60;
    let mut sy = y + PAD;
    font::draw_num(x + 8, sy, (line + 1) as u32, 3, 69, 1);
    for &b in &f.data[start..f.len] {
        if line >= ed.top_line + rows {
            break;
        }
        if b == b'\n' {
            line += 1;
            sx = x + 60;
            sy = y + PAD + (line - ed.top_line) as i32 * LH;
            if line < ed.top_line + rows {
                font::draw_num(x + 8, sy, (line + 1) as u32, 3, 69, 1);
            }
            continue;
        }
        if sx < x + w - 10 {
            font::draw_char(sx, sy, b, 68, None); // P_TEXT
        }
        sx += 8;
    }

    // curseur
    if focused && (t * 2.0) as i32 % 2 == 0 {
        let cx = x + 60 + cc as i32 * 8;
        let cy = y + PAD + (cl - ed.top_line) as i32 * LH;
        fb::fill_rect(cx, cy, 2, 16, 70); // P_ACCENT
    }
}
