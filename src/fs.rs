#![allow(dead_code, static_mut_refs)]
//! Système de fichiers : emplacements de taille fixe en RAM, aucun
//! allocateur. Rendu **persistant** en le sérialisant tel quel sur le
//! disque ATA (`nothingos.img` côté Mac) — voir [`save`] / [`load`].

use core::mem::size_of;

use crate::ata;

const MAXF: usize = 24;
/// Taille maxi d'un fichier.
pub const FCAP: usize = 12 * 1024;
const NCAP: usize = 48;

#[derive(Clone, Copy)]
pub struct File {
    name: [u8; NCAP],
    nlen: usize,
    dir: bool,
    pub data: [u8; FCAP],
    pub len: usize,
    used: bool,
}

impl File {
    const EMPTY: File = File {
        name: [0; NCAP],
        nlen: 0,
        dir: false,
        data: [0; FCAP],
        len: 0,
        used: false,
    };
    pub fn name(&self) -> &str {
        core::str::from_utf8(&self.name[..self.nlen]).unwrap_or("?")
    }
    pub fn is_dir(&self) -> bool {
        self.dir
    }
    pub fn content(&self) -> &str {
        core::str::from_utf8(&self.data[..self.len]).unwrap_or("<binaire>")
    }
}

static mut FILES: [File; MAXF] = [File::EMPTY; MAXF];

fn set_name(f: &mut File, name: &[u8]) {
    let n = name.len().min(NCAP);
    f.name[..n].copy_from_slice(&name[..n]);
    f.nlen = n;
}

fn seed(name: &[u8], dir: bool, body: &[u8]) {
    if let Some(i) = create_kind(name, dir) {
        let f = slot_mut(i);
        let n = body.len().min(FCAP);
        f.data[..n].copy_from_slice(&body[..n]);
        f.len = n;
    }
}

pub fn init() {
    unsafe {
        FILES = [File::EMPTY; MAXF];
    }
    seed(b"Documents", true, b"");
    seed(b"Projets", true, b"");
    seed(
        b"bienvenue.txt",
        false,
        b"Bienvenue dans Nothing OS.\n\nTout se fait a la commande :\n  /app terminal    un shell (ls, cat, edit, mkdir...)\n  /app editeur     editeur de texte\n  /fichier <nom>   ouvre un fichier dans l'editeur\n  /document        liste les fichiers\n  /web <mots>      recherche dans les fichiers (hors-ligne)\n\nAsti reste toujours au-dessus. Glisse-lui une friandise.\n",
    );
    seed(
        b"notes.txt",
        false,
        b"- finir le pilote reseau\n- ranger le bureau\n- nourrir Asti\n",
    );
    seed(b"todo.md", false, b"# A faire\n\n[ ] tests\n[x] clavier azerty\n");
}

pub fn count() -> usize {
    unsafe { FILES.iter().filter(|f| f.used).count() }
}

/// Applique `g` à chaque (index, &File) utilisé.
pub fn each(mut g: impl FnMut(usize, &File)) {
    unsafe {
        for (i, f) in FILES.iter().enumerate() {
            if f.used {
                g(i, f);
            }
        }
    }
}

pub fn get(i: usize) -> Option<&'static File> {
    unsafe {
        if i < MAXF && FILES[i].used {
            Some(&FILES[i])
        } else {
            None
        }
    }
}

pub fn slot_mut(i: usize) -> &'static mut File {
    unsafe { &mut FILES[i] }
}

pub fn find(name: &[u8]) -> Option<usize> {
    unsafe {
        for (i, f) in FILES.iter().enumerate() {
            if f.used && &f.name[..f.nlen] == name {
                return Some(i);
            }
        }
    }
    None
}

/// Crée un fichier/dossier vide. Renvoie l'index (existant si déjà là).
pub fn create(name: &[u8]) -> Option<usize> {
    create_kind(name, false)
}

pub fn create_kind(name: &[u8], dir: bool) -> Option<usize> {
    if name.is_empty() {
        return None;
    }
    if let Some(i) = find(name) {
        return Some(i);
    }
    unsafe {
        for (i, f) in FILES.iter_mut().enumerate() {
            if !f.used {
                *f = File::EMPTY;
                f.used = true;
                f.dir = dir;
                set_name(f, name);
                mark_dirty();
                return Some(i);
            }
        }
    }
    None
}

pub fn remove(name: &[u8]) -> bool {
    if let Some(i) = find(name) {
        unsafe {
            FILES[i].used = false;
        }
        mark_dirty();
        true
    } else {
        false
    }
}

// ---------------------------------------------------------------------
// Persistance sur disque ATA.
// ---------------------------------------------------------------------

const MAGIC: [u8; 8] = *b"NOS-FS01";
const IMG_BYTES: usize = size_of::<[File; MAXF]>();
const IMG_SECTORS: usize = (IMG_BYTES + ata::SECTOR - 1) / ata::SECTOR;

static mut DISK: [u8; IMG_SECTORS * ata::SECTOR] = [0; IMG_SECTORS * ata::SECTOR];
static mut DIRTY: bool = false;
static mut HAS_DISK: bool = false;

fn rw(write: bool) -> bool {
    let buf = unsafe { &mut DISK[..] };
    let mut done = 0usize;
    while done < IMG_SECTORS {
        let n = (IMG_SECTORS - done).min(63) as u8;
        let seg = &mut buf[done * ata::SECTOR..(done + n as usize) * ata::SECTOR];
        let lba = 1 + done as u32;
        let ok = if write {
            ata::write(lba, n, seg)
        } else {
            ata::read(lba, n, seg)
        };
        if !ok {
            return false;
        }
        done += n as usize;
    }
    true
}

/// Charge le système de fichiers depuis le disque. Si pas de disque ou
/// disque vierge, on garde les fichiers par défaut (créés par `init`).
pub fn load() {
    if !ata::present() {
        return;
    }
    unsafe {
        HAS_DISK = true;
    }
    let mut hdr = [0u8; ata::SECTOR];
    if !ata::read(0, 1, &mut hdr) || hdr[..8] != MAGIC {
        crate::serial_println!("[nothing-os] disque non formate : fichiers par defaut");
        return;
    }
    if rw(false) {
        unsafe {
            let src = &DISK as *const u8;
            core::ptr::copy_nonoverlapping(src, &raw mut FILES as *mut u8, IMG_BYTES);
        }
        crate::serial_println!("[nothing-os] {} fichiers charges du disque", count());
    }
}

pub fn mark_dirty() {
    unsafe {
        DIRTY = true;
    }
}

/// Écrit le système de fichiers sur le disque s'il a changé.
pub fn flush() {
    unsafe {
        if !DIRTY || !HAS_DISK {
            return;
        }
        DIRTY = false;
        let mut hdr = [0u8; ata::SECTOR];
        hdr[..8].copy_from_slice(&MAGIC);
        ata::write(0, 1, &hdr);
        core::ptr::copy_nonoverlapping(&raw const FILES as *const u8, &raw mut DISK as *mut u8, IMG_BYTES);
    }
    if rw(true) {
        crate::serial_println!("[nothing-os] disque synchronise");
    }
}
