//! Terminal : un mini shell qui agit sur le système de fichiers RAM.
//! Un seul terminal actif à la fois (état global).

use crate::{fb, fs, font, home, rtc};

const LW: usize = 110; // largeur d'une ligne
const LN: usize = 200; // lignes de scrollback

static mut LINES: [[u8; LW]; LN] = [[0; LW]; LN];
static mut LLEN: [usize; LN] = [0; LN];
static mut HEAD: usize = 0; // prochaine ligne à écrire
static mut FULL: bool = false;
static mut INBUF: [u8; LW] = [0; LW];
static mut INLEN: usize = 0;
static mut SCROLL: usize = 0;

pub fn reset() {
    unsafe {
        HEAD = 0;
        FULL = false;
        INLEN = 0;
        SCROLL = 0;
    }
    out(b"Nothing OS - terminal");
    out(b"tape 'help' pour la liste des commandes");
    out(b"");
}

fn out(s: &[u8]) {
    unsafe {
        let n = s.len().min(LW);
        LINES[HEAD][..n].copy_from_slice(&s[..n]);
        LLEN[HEAD] = n;
        HEAD = (HEAD + 1) % LN;
        if HEAD == 0 {
            FULL = true;
        }
    }
}

fn out2(a: &[u8], b: &[u8]) {
    let mut buf = [0u8; LW];
    let n1 = a.len().min(LW);
    buf[..n1].copy_from_slice(&a[..n1]);
    let n2 = b.len().min(LW - n1);
    buf[n1..n1 + n2].copy_from_slice(&b[..n2]);
    out(&buf[..n1 + n2]);
}

fn eq(a: &[u8], b: &[u8]) -> bool {
    a == b
}

pub fn key(c: u8) {
    unsafe {
        match c {
            b'\n' => {
                let len = INLEN;
                let mut line = [0u8; LW];
                line[..len].copy_from_slice(&INBUF[..len]);
                out2(b"$ ", &line[..len]);
                INLEN = 0;
                SCROLL = 0;
                run(&line[..len]);
            }
            0x08 => {
                if INLEN > 0 {
                    INLEN -= 1;
                }
            }
            0x20..=0x7e => {
                if INLEN < LW - 1 {
                    INBUF[INLEN] = c;
                    INLEN += 1;
                }
            }
            _ => {}
        }
    }
}

fn run(line: &[u8]) {
    let line = trim(line);
    if line.is_empty() {
        return;
    }
    let (cmd, rest) = split(line);
    match cmd {
        b"help" => {
            out(b"ls           liste les fichiers");
            out(b"cat <f>      affiche un fichier");
            out(b"echo <txt>   ecrit du texte");
            out(b"touch <f>    cree un fichier vide");
            out(b"mkdir <d>    cree un dossier");
            out(b"rm <f>       supprime");
            out(b"write <f> <txt>  remplace le contenu");
            out(b"date         heure courante");
            out(b"feed         nourrit Asti (+10)");
            out(b"hunger       niveau de faim d'Asti");
            out(b"clear        efface l'ecran");
        }
        b"ls" => {
            fs::each(|_, f| {
                if f.is_dir() {
                    out2(f.name().as_bytes(), b"/");
                } else {
                    let mut b = [0u8; LW];
                    let nm = f.name().as_bytes();
                    let n = nm.len().min(30);
                    b[..n].copy_from_slice(&nm[..n]);
                    let sz = f.len;
                    let mut d = [0u8; 12];
                    let dn = utoa(sz as u32, &mut d);
                    b[32..32 + dn].copy_from_slice(&d[..dn]);
                    b[32 + dn..32 + dn + 1].copy_from_slice(b"o");
                    out(&b[..32 + dn + 1]);
                }
            });
        }
        b"cat" => match fs::find(rest) {
            Some(i) => {
                let f = fs::get(i).unwrap();
                // découpe par lignes
                let mut s = 0;
                let d = &f.data[..f.len];
                for e in 0..=d.len() {
                    if e == d.len() || d[e] == b'\n' {
                        out(&d[s..e]);
                        s = e + 1;
                    }
                }
            }
            None => out(b"cat: fichier introuvable"),
        },
        b"echo" => out(rest),
        b"touch" => {
            if fs::create(rest).is_some() {
                out(b"ok");
            } else {
                out(b"touch: impossible");
            }
        }
        b"mkdir" => {
            if fs::create_kind(rest, true).is_some() {
                out(b"ok");
            } else {
                out(b"mkdir: impossible");
            }
        }
        b"rm" => {
            if fs::remove(rest) {
                out(b"ok");
            } else {
                out(b"rm: introuvable");
            }
        }
        b"write" => {
            let (name, txt) = split(rest);
            if let Some(i) = fs::create(name) {
                let f = fs::slot_mut(i);
                let n = txt.len().min(fs::FCAP);
                f.data[..n].copy_from_slice(&txt[..n]);
                f.len = n;
                out(b"ok");
            } else {
                out(b"write: impossible");
            }
        }
        b"date" => {
            let t = rtc::now();
            let mut b = [0u8; 8];
            b[0] = b'0' + t.hour / 10;
            b[1] = b'0' + t.hour % 10;
            b[2] = b':';
            b[3] = b'0' + t.min / 10;
            b[4] = b'0' + t.min % 10;
            b[5] = b':';
            b[6] = b'0' + t.sec / 10;
            b[7] = b'0' + t.sec % 10;
            out(&b);
        }
        b"feed" => {
            home::feed(10);
            out(b"miam");
        }
        b"hunger" => {
            let mut d = [0u8; 4];
            let n = utoa(home::food() as u32, &mut d);
            out2(b"faim: ", &d[..n]);
        }
        b"clear" => unsafe {
            HEAD = 0;
            FULL = false;
        },
        b"whoami" => out(b"utilisateur de Nothing OS"),
        _ => out2(b"commande inconnue: ", cmd),
    }
    let _ = eq;
}

fn trim(s: &[u8]) -> &[u8] {
    let mut a = 0;
    let mut b = s.len();
    while a < b && (s[a] == b' ') {
        a += 1;
    }
    while b > a && (s[b - 1] == b' ') {
        b -= 1;
    }
    &s[a..b]
}

fn split(s: &[u8]) -> (&[u8], &[u8]) {
    match s.iter().position(|&c| c == b' ') {
        Some(i) => (&s[..i], trim(&s[i + 1..])),
        None => (s, &[]),
    }
}

fn utoa(mut v: u32, buf: &mut [u8]) -> usize {
    if v == 0 {
        buf[0] = b'0';
        return 1;
    }
    let mut tmp = [0u8; 12];
    let mut n = 0;
    while v > 0 {
        tmp[n] = b'0' + (v % 10) as u8;
        v /= 10;
        n += 1;
    }
    for i in 0..n {
        buf[i] = tmp[n - 1 - i];
    }
    n
}

pub fn draw(x: i32, y: i32, w: i32, h: i32, t: f32) {
    const LH: i32 = 18;
    fb::fill_rect(x, y, w, h, 71); // fond sombre
    let rows = ((h - 16) / LH) as usize;
    unsafe {
        let total = if FULL { LN } else { HEAD };
        let start = total.saturating_sub(rows + SCROLL);
        let mut sy = y + 8;
        for k in 0..rows.min(total) {
            let idx = (start + k) % LN;
            let ll = LLEN[idx];
            let mut sx = x + 10;
            for &b in &LINES[idx][..ll] {
                font::draw_char(sx, sy, b, 68, None);
                sx += 8;
            }
            sy += LH;
        }
        // ligne de saisie
        let mut sx = x + 10;
        font::draw_char(sx, sy, b'$', 70, None);
        sx += 16;
        for &b in &INBUF[..INLEN] {
            font::draw_char(sx, sy, b, 68, None);
            sx += 8;
        }
        if (t * 2.0) as i32 % 2 == 0 {
            fb::fill_rect(sx, sy, 8, 15, 70);
        }
    }
}
