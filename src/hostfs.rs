//! Pont entre l'éditeur — qui travaille sur des emplacements RAM de
//! [`crate::fs`] — et les vrais fichiers du Mac exposés par [`crate::p9`]
//! (partage virtio-9p).
//!
//! * [`open`] lit un fichier hôte, le recopie dans un emplacement fs et
//!   retient le lien `slot -> chemin hôte`.
//! * [`sync`] réécrit sur le Mac les emplacements liés dont le contenu a
//!   changé (appelé périodiquement depuis la boucle du bureau).
//! * [`refresh_dir`] / [`each_dir`] tiennent à jour une liste de la racine
//!   du partage pour l'application « Fichiers ».

#![allow(static_mut_refs)]

use alloc::string::String;
use alloc::vec::Vec;

use crate::{fs, p9};

struct Link {
    slot: usize,
    path: String,
    hash: u64,
    /// le fichier hôte dépassait `fs::FCAP` : lecture tronquée, on
    /// n'ose pas réécrire (on écraserait la fin côté Mac).
    truncated: bool,
}

static mut LINKS: Vec<Link> = Vec::new();
static mut DIR: Vec<p9::Ent> = Vec::new();
static mut DIR_OK: bool = false;

fn fnv1a(b: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for &x in b {
        h ^= x as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Dernier segment d'un chemin (`"a/b/c.txt"` -> `"c.txt"`).
pub fn basename(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Ouvre le fichier hôte `path` dans un emplacement fs. S'il n'existe pas
/// encore sur le Mac, il y est créé (vide) : `/fichier` sert donc aussi à
/// créer. `None` seulement si le partage 9p est absent.
pub fn open(path: &str) -> Option<usize> {
    if !p9::present() {
        return None;
    }
    let short = basename(path);
    if short.is_empty() {
        return None;
    }

    let (data, existed) = match p9::read_file(path) {
        Some(d) => (d, true),
        None => {
            // création immédiate côté Mac
            if !p9::write_file(path, &[]) {
                return None;
            }
            (Vec::new(), false)
        }
    };

    let slot = fs::create(short.as_bytes())?;
    let n = data.len().min(fs::FCAP);
    {
        let f = fs::slot_mut(slot);
        f.data[..n].copy_from_slice(&data[..n]);
        f.len = n;
    }
    let h = fnv1a(&data[..n]);
    let truncated = data.len() > fs::FCAP;
    let _ = existed;

    unsafe {
        if let Some(l) = LINKS.iter_mut().find(|l| l.slot == slot) {
            l.path = String::from(path);
            l.hash = h;
            l.truncated = truncated;
        } else {
            LINKS.push(Link {
                slot,
                path: String::from(path),
                hash: h,
                truncated,
            });
        }
    }
    if truncated {
        crate::serial_println!("[hostfs] {} tronque a {} o (lecture seule)", path, n);
    } else {
        crate::serial_println!("[hostfs] ouvert {} ({} o)", path, n);
    }
    Some(slot)
}

/// Réécrit sur le Mac les fichiers liés dont le contenu a changé.
pub fn sync() {
    unsafe {
        for l in LINKS.iter_mut() {
            if l.truncated {
                continue;
            }
            let f = match fs::get(l.slot) {
                Some(f) => f,
                None => continue,
            };
            let cur = fnv1a(&f.data[..f.len]);
            if cur == l.hash {
                continue;
            }
            if p9::write_file(&l.path, &f.data[..f.len]) {
                l.hash = cur;
                crate::serial_println!("[hostfs] {} -> Mac ({} o)", l.path, f.len);
            }
        }
    }
}

/// Recharge la liste de la racine du partage (pour l'appli Fichiers).
pub fn refresh_dir() {
    if !p9::present() {
        return;
    }
    if let Some(v) = p9::list("") {
        unsafe {
            DIR = v;
            DIR_OK = true;
        }
    }
}

pub fn have_dir() -> bool {
    unsafe { DIR_OK && !DIR.is_empty() }
}

/// Applique `g(nom, est_dossier)` à chaque entrée de la racine du partage.
pub fn each_dir(mut g: impl FnMut(&str, bool)) {
    unsafe {
        for e in DIR.iter() {
            g(&e.name, e.kind == p9::DT_DIR);
        }
    }
}
