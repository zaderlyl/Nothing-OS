//! Client **9P2000.L** minimal au-dessus du transport virtio (`src/virtio.rs`).
//!
//! Permet de lister / lire / écrire les fichiers d'un dossier du Mac que
//! QEMU partage :
//!
//! ```text
//! -fsdev local,id=fsdev0,path=$HOME/Documents,security_model=none \
//! -device virtio-9p-pci,fsdev=fsdev0,mount_tag=hostdocs,disable-modern=on
//! ```
//!
//! Grâce à l'identity-mapping, on donne directement l'adresse d'un tampon
//! statique au périphérique (voir `virtio::request`).

#![allow(dead_code)]

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::virtio;

// --- types de messages 9P2000.L ---------------------------------------
const R_LERROR: u8 = 7;
const T_LOPEN: u8 = 12;
const R_LOPEN: u8 = 13;
const T_LCREATE: u8 = 14;
const R_LCREATE: u8 = 15;
const T_READDIR: u8 = 40;
const R_READDIR: u8 = 41;
const T_VERSION: u8 = 100;
const R_VERSION: u8 = 101;
const T_ATTACH: u8 = 104;
const R_ATTACH: u8 = 105;
const T_WALK: u8 = 110;
const R_WALK: u8 = 111;
const T_READ: u8 = 116;
const R_READ: u8 = 117;
const T_WRITE: u8 = 118;
const R_WRITE: u8 = 119;
const T_CLUNK: u8 = 120;
const R_CLUNK: u8 = 121;

const NOFID: u32 = 0xffff_ffff;
const NONUNAME: u32 = 0xffff_ffff;
const ROOT_FID: u32 = 0;
const MSIZE: u32 = 8192;

// drapeaux open() Linux (host x86)
const O_WRONLY: u32 = 1;
const O_TRUNC: u32 = 0o1000;
const O_DIRECTORY: u32 = 0o200000;

// taille max de données par Tread/Twrite (marge pour les en-têtes)
const CHUNK: usize = 4096;

/// Type d'entrée renvoyé par [`list`] (valeurs `DT_*` de `<dirent.h>`).
pub const DT_DIR: u8 = 4;
pub const DT_REG: u8 = 8;

static mut READY: bool = false;
static mut TAG: u16 = 0;
static mut NEXT_FID: u32 = 16;

pub fn present() -> bool {
    unsafe { READY }
}

// --- construction d'un message ---------------------------------------
struct Msg {
    buf: Vec<u8>,
}

impl Msg {
    fn new(t: u8) -> Self {
        let mut buf = Vec::with_capacity(64);
        buf.extend_from_slice(&[0, 0, 0, 0]); // taille : remplie par call()
        buf.push(t);
        let tag = unsafe {
            TAG = TAG.wrapping_add(1);
            TAG
        };
        buf.extend_from_slice(&tag.to_le_bytes());
        Msg { buf }
    }
    fn p8(&mut self, v: u8) {
        self.buf.push(v);
    }
    fn p16(&mut self, v: u16) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    fn p32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    fn p64(&mut self, v: u64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    fn pstr(&mut self, s: &str) {
        self.p16(s.len() as u16);
        self.buf.extend_from_slice(s.as_bytes());
    }
    fn pbytes(&mut self, b: &[u8]) {
        self.buf.extend_from_slice(b);
    }

    /// Envoie le message et renvoie le corps de la réponse (après
    /// `size[4] type[1] tag[2]`) si le type reçu est bien `expect`.
    fn call(mut self, expect: u8) -> Option<Vec<u8>> {
        let n = self.buf.len() as u32;
        self.buf[0..4].copy_from_slice(&n.to_le_bytes());
        let got = virtio::request(&self.buf);
        if got < 7 {
            return None;
        }
        let r = virtio::resp();
        let rsize = u32::from_le_bytes([r[0], r[1], r[2], r[3]]) as usize;
        if rsize < 7 || rsize > got || rsize > r.len() {
            return None;
        }
        let rtype = r[4];
        if rtype != expect {
            if rtype == R_LERROR && rsize >= 11 {
                let ec = u32::from_le_bytes([r[7], r[8], r[9], r[10]]);
                crate::serial_println!("[9p] Rlerror {} (attendait {})", ec, expect);
            } else {
                crate::serial_println!("[9p] type {} inattendu (attendait {})", rtype, expect);
            }
            return None;
        }
        Some(r[7..rsize].to_vec())
    }
}

// --- lecture d'une réponse ------------------------------------------
struct Rd<'a> {
    b: &'a [u8],
    p: usize,
}

impl<'a> Rd<'a> {
    fn new(b: &'a [u8]) -> Self {
        Rd { b, p: 0 }
    }
    fn g8(&mut self) -> u8 {
        if self.p >= self.b.len() {
            return 0;
        }
        let v = self.b[self.p];
        self.p += 1;
        v
    }
    fn g16(&mut self) -> u16 {
        if self.p + 2 > self.b.len() {
            self.p = self.b.len();
            return 0;
        }
        let v = u16::from_le_bytes([self.b[self.p], self.b[self.p + 1]]);
        self.p += 2;
        v
    }
    fn g32(&mut self) -> u32 {
        if self.p + 4 > self.b.len() {
            self.p = self.b.len();
            return 0;
        }
        let v = u32::from_le_bytes([
            self.b[self.p],
            self.b[self.p + 1],
            self.b[self.p + 2],
            self.b[self.p + 3],
        ]);
        self.p += 4;
        v
    }
    fn g64(&mut self) -> u64 {
        let lo = self.g32() as u64;
        let hi = self.g32() as u64;
        lo | (hi << 32)
    }
    fn skip(&mut self, n: usize) {
        self.p = (self.p + n).min(self.b.len());
    }
    fn gstr(&mut self) -> String {
        let n = self.g16() as usize;
        let end = (self.p + n).min(self.b.len());
        let s = String::from_utf8_lossy(&self.b[self.p..end]).into_owned();
        self.p = end;
        s
    }
    fn rem(&self) -> &'a [u8] {
        &self.b[self.p..]
    }
}

fn alloc_fid() -> u32 {
    unsafe {
        let f = NEXT_FID;
        NEXT_FID += 1;
        if NEXT_FID > 4000 {
            NEXT_FID = 16;
        }
        f
    }
}

fn clunk(fid: u32) {
    let mut m = Msg::new(T_CLUNK);
    m.p32(fid);
    let _ = m.call(R_CLUNK);
}

/// Découpe `"a/b/c.txt"` en `("a/b", "c.txt")`.
fn split_parent(path: &str) -> (&str, &str) {
    match path.rfind('/') {
        Some(i) => (&path[..i], &path[i + 1..]),
        None => ("", path),
    }
}

/// Descend depuis la racine jusqu'à `path` dans un fid neuf.
/// `Some(fid)` seulement si tous les éléments ont été trouvés.
fn walk(path: &str) -> Option<u32> {
    let fid = alloc_fid();
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let mut m = Msg::new(T_WALK);
    m.p32(ROOT_FID);
    m.p32(fid);
    m.p16(parts.len() as u16);
    for p in &parts {
        m.pstr(p);
    }
    let body = m.call(R_WALK)?;
    let nwqid = Rd::new(&body).g16() as usize;
    if nwqid != parts.len() {
        // échec partiel : le nouveau fid n'a pas été créé
        return None;
    }
    Some(fid)
}

/// Poignée de main : Tversion + Tattach de la racine du partage.
pub fn init() -> bool {
    if !virtio::present() {
        return false;
    }

    let mut m = Msg::new(T_VERSION);
    m.p32(MSIZE);
    m.pstr("9P2000.L");
    let body = match m.call(R_VERSION) {
        Some(b) => b,
        None => {
            crate::serial_println!("[9p] Tversion sans reponse");
            return false;
        }
    };
    let mut rd = Rd::new(&body);
    let srv_msize = rd.g32();
    let ver = rd.gstr();
    if !ver.starts_with("9P2000.L") {
        crate::serial_println!("[9p] version refusee : {}", ver);
        return false;
    }

    let mut m = Msg::new(T_ATTACH);
    m.p32(ROOT_FID);
    m.p32(NOFID);
    m.pstr("nothing"); // uname
    m.pstr(""); // aname
    m.p32(NONUNAME); // n_uname
    if m.call(R_ATTACH).is_none() {
        crate::serial_println!("[9p] Tattach echoue");
        return false;
    }

    unsafe {
        READY = true;
    }
    crate::serial_println!("[9p] partage monte (msize {})", srv_msize.min(MSIZE));
    true
}

/// Entrée de répertoire renvoyée par [`list`].
pub struct Ent {
    pub name: String,
    pub kind: u8,
}

/// Liste le dossier `path` (relatif à la racine du partage ; `""` = racine).
pub fn list(path: &str) -> Option<Vec<Ent>> {
    if !present() {
        return None;
    }
    let fid = walk(path)?;

    let mut m = Msg::new(T_LOPEN);
    m.p32(fid);
    m.p32(O_DIRECTORY);
    if m.call(R_LOPEN).is_none() {
        clunk(fid);
        return None;
    }

    let mut ents = Vec::new();
    let mut off: u64 = 0;
    loop {
        let mut m = Msg::new(T_READDIR);
        m.p32(fid);
        m.p64(off);
        m.p32(CHUNK as u32);
        let body = match m.call(R_READDIR) {
            Some(b) => b,
            None => break,
        };
        let mut rd = Rd::new(&body);
        let cnt = rd.g32() as usize;
        if cnt == 0 {
            break;
        }
        let data = rd.rem();
        let mut d = Rd::new(&data[..cnt.min(data.len())]);
        let mut last = off;
        while d.p < d.b.len() {
            d.skip(13); // qid
            let eoff = d.g64();
            let etype = d.g8();
            let name = d.gstr();
            if name.is_empty() {
                break;
            }
            last = eoff;
            if name != "." && name != ".." {
                ents.push(Ent { name, kind: etype });
            }
        }
        if last == off {
            break;
        }
        off = last;
    }

    clunk(fid);
    Some(ents)
}

/// Lit tout le contenu du fichier `path`.
pub fn read_file(path: &str) -> Option<Vec<u8>> {
    if !present() {
        return None;
    }
    let fid = walk(path)?;

    let mut m = Msg::new(T_LOPEN);
    m.p32(fid);
    m.p32(0); // O_RDONLY
    if m.call(R_LOPEN).is_none() {
        clunk(fid);
        return None;
    }

    let mut out = Vec::new();
    let mut off: u64 = 0;
    loop {
        let mut m = Msg::new(T_READ);
        m.p32(fid);
        m.p64(off);
        m.p32(CHUNK as u32);
        let body = match m.call(R_READ) {
            Some(b) => b,
            None => break,
        };
        let mut rd = Rd::new(&body);
        let cnt = rd.g32() as usize;
        if cnt == 0 {
            break;
        }
        let data = rd.rem();
        let take = cnt.min(data.len());
        out.extend_from_slice(&data[..take]);
        off += take as u64;
        if take < CHUNK {
            break;
        }
    }

    clunk(fid);
    Some(out)
}

/// (Ré)écrit `data` dans le fichier `path` (créé s'il n'existe pas).
pub fn write_file(path: &str, data: &[u8]) -> bool {
    if !present() {
        return false;
    }

    let fid = match walk(path) {
        Some(f) => {
            let mut m = Msg::new(T_LOPEN);
            m.p32(f);
            m.p32(O_WRONLY | O_TRUNC);
            if m.call(R_LOPEN).is_none() {
                clunk(f);
                return false;
            }
            f
        }
        None => {
            let (dir, name) = split_parent(path);
            if name.is_empty() {
                return false;
            }
            let pf = match walk(dir) {
                Some(f) => f,
                None => return false,
            };
            let mut m = Msg::new(T_LCREATE);
            m.p32(pf);
            m.pstr(name);
            m.p32(O_WRONLY | O_TRUNC);
            m.p32(0o644);
            m.p32(0);
            if m.call(R_LCREATE).is_none() {
                clunk(pf);
                return false;
            }
            pf // le fid parent référence maintenant le fichier ouvert
        }
    };

    let mut off: usize = 0;
    let mut ok = true;
    while off < data.len() {
        let take = (data.len() - off).min(CHUNK);
        let mut m = Msg::new(T_WRITE);
        m.p32(fid);
        m.p64(off as u64);
        m.p32(take as u32);
        m.pbytes(&data[off..off + take]);
        match m.call(R_WRITE) {
            Some(b) => {
                let w = Rd::new(&b).g32() as usize;
                if w == 0 {
                    ok = false;
                    break;
                }
                off += w;
            }
            None => {
                ok = false;
                break;
            }
        }
    }

    clunk(fid);
    ok
}

/// Petit test au boot : liste la racine du partage sur le port série.
pub fn selftest() {
    if !present() {
        return;
    }
    match list("") {
        Some(v) => {
            crate::serial_println!("[9p] racine : {} entree(s)", v.len());
            for e in v.iter().take(8) {
                let tag = if e.kind == DT_DIR { "d" } else { "-" };
                crate::serial_println!("      {} {}", tag, e.name);
            }
        }
        None => crate::serial_println!("[9p] impossible de lister la racine"),
    }
}

/// Rend un `String` à partir d'octets éventuellement non-UTF8.
pub fn lossy(b: &[u8]) -> String {
    String::from_utf8_lossy(b).to_string()
}
