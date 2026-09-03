//! Consultation de documents — deux demi-panneaux qui glissent depuis les
//! bords de l'écran, **sans empilement de fenêtres**.
//!
//! * `/doc all` (ou `/doc`) : la **liste** des documents (racine du
//!   partage Mac + fichiers RAM) entre par la **gauche** ; la molette la
//!   fait défiler.
//! * un clic sur un fichier fait entrer sa **visualisation** par la
//!   **droite** ; un espace sépare les deux panneaux.
//! * cliquer en dehors « retire » le panneau du dessus (droite puis
//!   gauche) : il ressort et disparaît, rien n'est mémorisé.

#![allow(dead_code, static_mut_refs)]

use alloc::string::String;
use alloc::vec::Vec;

use crate::win::{P_ACCENT, P_CODE_BG, P_DIM, P_FRAME, P_STR, P_TEXT, P_TITLE_HI};
use crate::{dots, fb, font, fs, p9};

const W: i32 = fb::WIDTH as i32;
const H: i32 = fb::HEIGHT as i32;

const PW: i32 = 780; // largeur d'un panneau (~demi-écran)
const RIGHT_X0: i32 = W - PW;
const PADX: i32 = 28;
const HEADER_H: i32 = 100;
const ROW_H: i32 = 46;

// --- état ---------------------------------------------------------------

struct Ent {
    name: String,
    dir: bool,
    host: bool,
}

static mut ENTRIES: Vec<Ent> = Vec::new();
static mut L_ON: bool = false;
static mut R_ON: bool = false;
static mut L_OUT: f32 = 0.0; // 0 = caché, 1 = sorti
static mut R_OUT: f32 = 0.0;
static mut L_SCROLL: i32 = 0;
static mut R_SCROLL: i32 = 0;
static mut SEL: i32 = -1;
static mut CONTENT: Vec<u8> = Vec::new();
static mut CONTENT_NAME: String = String::new();
/// Sous-dossier courant dans le partage Mac ("" = racine).
static mut CWD: String = String::new();

/// Nature du panneau droit.
#[derive(PartialEq)]
enum View {
    Text,
    Image,
    Audio,   // lecteur audio compact (mp3 / wav)
    Message, // erreur / format non géré → VIEW_MSG
}
static mut VIEW: View = View::Text;
static mut VIEW_MSG: String = String::new();
static mut IMG_W: i32 = 0;
static mut IMG_H: i32 = 0;
static mut IMG_PX: Vec<u8> = Vec::new();
/// Fichier sélectionné mais pas encore chargé (le décodage bloque une
/// image ou deux : on affiche « ... » d'abord).
static mut PENDING: bool = false;

pub fn active() -> bool {
    unsafe { L_ON || R_ON || L_OUT > 0.01 || R_OUT > 0.01 }
}

/// Le panneau gauche occupe-t-il le bord gauche ? (pour masquer la barre
/// latérale automatique du bureau)
pub fn left_covering() -> bool {
    unsafe { L_OUT > 0.04 }
}

/// Ouvre la liste des documents (revient à la racine du partage).
pub fn open_list() {
    close_right();
    unsafe {
        CWD = String::new();
        L_ON = true;
    }
    relist();
}

/// (Re)construit la liste pour le dossier courant `CWD`.
fn relist() {
    let cwd = unsafe { CWD.clone() };
    let mut v: Vec<Ent> = Vec::new();

    if p9::present() {
        if let Some(items) = p9::list(&cwd) {
            for e in items {
                v.push(Ent {
                    name: e.name,
                    dir: e.kind == p9::DT_DIR,
                    host: true,
                });
            }
        }
    }
    // les fichiers RAM (pas les dossiers : le fs local est plat) sont
    // montrés seulement à la racine
    if cwd.is_empty() {
        fs::each(|_, f| {
            if !f.is_dir() {
                v.push(Ent {
                    name: String::from(f.name()),
                    dir: false,
                    host: false,
                });
            }
        });
    }

    // dossiers d'abord, puis ordre alphabétique (insensible à la casse)
    v.sort_by(|a, b| match (a.dir, b.dir) {
        (true, false) => core::cmp::Ordering::Less,
        (false, true) => core::cmp::Ordering::Greater,
        _ => ci_cmp(&a.name, &b.name),
    });
    unsafe {
        ENTRIES = v;
        L_SCROLL = 0;
        SEL = -1;
        clamp_scrolls();
    }
    crate::serial_println!(
        "[doc] {} : {} entree(s)",
        if cwd.is_empty() { "racine" } else { &cwd },
        unsafe { ENTRIES.len() }
    );
}

/// Descend dans un sous-dossier du partage.
fn enter_dir(name: &str) {
    unsafe {
        if !CWD.is_empty() {
            CWD.push('/');
        }
        CWD.push_str(name);
    }
    relist();
}

/// Chemin complet (relatif au partage) d'une entrée de la liste courante.
fn full_path(name: &str) -> String {
    let cwd = unsafe { CWD.clone() };
    if cwd.is_empty() {
        String::from(name)
    } else {
        let mut s = cwd;
        s.push('/');
        s.push_str(name);
        s
    }
}

/// Segments du fil d'Ariane : "racine" puis chaque dossier de `CWD`.
fn crumbs() -> Vec<String> {
    let mut c = alloc::vec![String::from("racine")];
    let cwd = unsafe { CWD.clone() };
    for seg in cwd.split('/').filter(|s| !s.is_empty()) {
        c.push(String::from(seg));
    }
    c
}

/// Remonte au niveau `depth` du fil d'Ariane (0 = racine).
fn go_crumb(depth: usize) {
    let cwd = unsafe { CWD.clone() };
    let segs: Vec<&str> = cwd.split('/').filter(|s| !s.is_empty()).collect();
    let mut s = String::new();
    for seg in segs.iter().take(depth) {
        if !s.is_empty() {
            s.push('/');
        }
        s.push_str(seg);
    }
    unsafe {
        CWD = s;
    }
    relist();
}

pub fn close_all() {
    close_right();
    unsafe {
        L_ON = false;
    }
}

/// Ferme le panneau droit (et coupe l'audio s'il y en avait).
fn close_right() {
    crate::ac97::stop();
    unsafe {
        R_ON = false;
    }
}

fn ci_cmp(a: &str, b: &str) -> core::cmp::Ordering {
    let lo = |c: u8| if c.is_ascii_uppercase() { c + 32 } else { c };
    for (x, y) in a.bytes().zip(b.bytes()) {
        match lo(x).cmp(&lo(y)) {
            core::cmp::Ordering::Equal => continue,
            o => return o,
        }
    }
    a.len().cmp(&b.len())
}

fn approach(cur: f32, target: f32, dt: f32, speed: f32) -> f32 {
    cur + (target - cur) * (1.0 - libm::powf(0.5, dt * speed))
}

pub fn update(dt: f32) {
    if unsafe { !L_ON && R_ON } {
        close_right();
    }
    // décodage différé d'une image / d'un fichier son (une frame après le
    // clic, pour laisser s'afficher « ouverture... »)
    if unsafe { PENDING } {
        unsafe {
            PENDING = false;
        }
        load_content();
    }
    unsafe {
        L_OUT = approach(L_OUT, if L_ON { 1.0 } else { 0.0 }, dt, 11.0).clamp(0.0, 1.0);
        R_OUT = approach(R_OUT, if R_ON { 1.0 } else { 0.0 }, dt, 11.0).clamp(0.0, 1.0);
        clamp_scrolls();
    }
}

fn list_content_h() -> i32 {
    unsafe { ENTRIES.len() as i32 * ROW_H }
}

fn view_content_h() -> i32 {
    unsafe {
        match VIEW {
            View::Image => IMG_H + 24,
            View::Message | View::Audio => 0,
            View::Text => wrapped_lines(&CONTENT) as i32 * (16 + 6),
        }
    }
}

fn clamp_scrolls() {
    unsafe {
        let lmax = (list_content_h() - (H - HEADER_H)).max(0);
        L_SCROLL = L_SCROLL.clamp(0, lmax);
        let rmax = (view_content_h() - (H - HEADER_H)).max(0);
        R_SCROLL = R_SCROLL.clamp(0, rmax);
    }
}

/// Molette : `delta` positif = vers le haut.
pub fn on_scroll(mx: i32, my: i32, delta: i32) {
    let _ = my;
    let step = delta * 40;
    unsafe {
        if VIEW == View::Audio {
            // rien à faire défiler
        } else if R_OUT > 0.5 && mx >= RIGHT_X0 {
            R_SCROLL -= step;
        } else if L_OUT > 0.5 && mx < PW {
            L_SCROLL -= step;
        }
        clamp_scrolls();
    }
}

/// Clic. Renvoie `true` si le clic a été « consommé » par la consultation.
pub fn on_click(mx: i32, my: i32) -> bool {
    if !active() {
        return false;
    }
    unsafe {
        // étiquette audio : petit rectangle en bas à droite (avant le
        // « clic ailleurs » mais après la liste de gauche, plus bas)
        if VIEW == View::Audio && R_OUT > 0.5 {
            let (ax, ay, aw, ah) = audio_rect(1.0);
            if mx >= ax && mx < ax + aw && my >= ay && my < ay + ah {
                audio_click(mx, my);
                return true;
            }
        } else if R_OUT > 0.5 && mx >= RIGHT_X0 {
            // autres vues : le panneau droit consomme le clic
            return true;
        }
        // panneau gauche visible et clic dedans
        if L_OUT > 0.5 && mx < PW {
            if my < HEADER_H {
                // fil d'Ariane : remonter à un niveau
                let lx = ((-(PW as f32)) * (1.0 - L_OUT)) as i32;
                for (x0, x1, depth) in crumb_ranges(lx) {
                    if mx >= x0 && mx < x1 {
                        go_crumb(depth);
                        break;
                    }
                }
            } else {
                let idx = (my - HEADER_H + L_SCROLL) / ROW_H;
                if idx >= 0 && (idx as usize) < ENTRIES.len() {
                    let e = &ENTRIES[idx as usize];
                    if e.dir {
                        if e.host {
                            let name = e.name.clone();
                            enter_dir(&name);
                        }
                    } else {
                        SEL = idx;
                        R_ON = true;
                        R_SCROLL = 0;
                        // affiche « ... » une frame, décode ensuite
                        crate::ac97::stop();
                        CONTENT_NAME = ENTRIES[idx as usize].name.clone();
                        VIEW_MSG = String::from("ouverture...");
                        VIEW = View::Message;
                        PENDING = true;
                    }
                }
            }
            return true;
        }
        // clic en dehors → on retire le panneau du dessus
        if R_ON {
            close_right();
        } else if L_ON {
            L_ON = false;
        }
        true
    }
}

/// Au-delà, on n'ouvre même pas le fichier (garde-fou mémoire).
const MAX_OPEN: usize = 24 * 1024 * 1024;
/// Idem mais pour les images : elles sont toujours affichées (réduites
/// autant qu'il faut), il faut juste pouvoir charger le fichier.
const MAX_OPEN_IMG: usize = 160 * 1024 * 1024;
/// Limite de lecture pour l'audio (un mp3/wav de plusieurs minutes).
const MAX_OPEN_SND: usize = 96 * 1024 * 1024;
const MSG_BIG: &str = "fichier trop volumineux pour etre ouvert";
const MSG_NOPE: &str = "affichage non pris en compte";
const MSG_VIDEO: &str = "lecture video indisponible (pas de decodeur H.264)";

fn ext_is(name: &str, list: &[&str]) -> bool {
    let e = match name.rsplit('.').next() {
        Some(e) => e,
        None => return false,
    };
    list.iter()
        .any(|t| e.len() == t.len() && e.bytes().zip(t.bytes()).all(|(c, d)| c.to_ascii_lowercase() == d))
}
fn is_audio(name: &str) -> bool {
    ext_is(name, &["mp3", "wav", "wave"])
}
fn is_video(name: &str) -> bool {
    ext_is(name, &["mp4", "mov", "m4v", "avi", "mkv", "webm", "wmv", "flv", "mpg", "mpeg"])
}

fn load_content() {
    crate::ac97::stop();
    unsafe {
        let e = &ENTRIES[SEL as usize];
        let name = e.name.clone();
        let host = e.host;
        CONTENT_NAME = name.clone();
        CONTENT = Vec::new();
        IMG_PX = Vec::new();
        VIEW_MSG = String::new();

        let msg = |m: &str| {
            VIEW_MSG = String::from(m);
            VIEW = View::Message;
        };

        if is_video(&name) {
            crate::serial_println!("[doc] {} : video non geree", name);
            return msg(MSG_VIDEO);
        }

        let is_img = crate::image::kind_of(&name).is_some();
        let is_snd = is_audio(&name);
        let cap = if is_img {
            MAX_OPEN_IMG
        } else if is_snd {
            MAX_OPEN_SND
        } else {
            MAX_OPEN
        };

        // 1) garde-fou taille (fichiers hôte ; les fichiers RAM sont <= FCAP)
        if host {
            if let Some(sz) = p9::size(&full_path(&name)) {
                if sz as usize > cap {
                    crate::serial_println!("[doc] {} : {} o -> trop gros", name, sz);
                    return msg(MSG_BIG);
                }
            }
        }

        // 2) lecture (plafonnée : sécurité si getattr a menti / fs local)
        let full = if host {
            match p9::read_file_max(&full_path(&name), cap) {
                Some(d) => d,
                None => return msg(MSG_BIG),
            }
        } else {
            match fs::find(name.as_bytes()) {
                Some(i) => fs::get(i).map(|f| f.data[..f.len].to_vec()).unwrap_or_default(),
                None => Vec::new(),
            }
        };

        // 3) selon le type
        if let Some(kind) = crate::image::kind_of(&name) {
            let max_w = PW - 2 * PADX;
            let max_h = H - HEADER_H - 24;
            match crate::image::decode_fit(&full, kind, max_w, max_h) {
                Ok(bm) => {
                    IMG_W = bm.w;
                    IMG_H = bm.h;
                    IMG_PX = bm.px;
                    VIEW = View::Image;
                    crate::serial_println!("[doc] image {} : {}x{}", name, bm.w, bm.h);
                }
                Err(e) if e.contains("trop grande") => msg(MSG_BIG),
                Err(_) => msg(MSG_NOPE),
            }
        } else if is_snd {
            match decode_audio(&name, &full) {
                Some((pcm, rate)) => {
                    let secs = pcm.len() as f32 / 2.0 / rate as f32;
                    crate::ac97::load(pcm, rate);
                    if crate::ac97::present() {
                        crate::ac97::play();
                    }
                    VIEW = View::Audio;
                    crate::serial_println!("[doc] audio {} : {:.0}s @ {}Hz", name, secs, rate);
                }
                None => msg(MSG_NOPE),
            }
        } else if ext_unsupported(&name) || looks_binary(&full) {
            crate::serial_println!("[doc] {} : non affichable", name);
            msg(MSG_NOPE);
        } else {
            let mut d = full;
            d.truncate(32 * 1024);
            CONTENT = d;
            VIEW = View::Text;
            crate::serial_println!("[doc] ouvre {} ({} o)", name, CONTENT.len());
        }
    }
}

/// Extensions dont on sait qu'on ne sait pas les afficher.
fn ext_unsupported(name: &str) -> bool {
    let e = match name.rsplit('.').next() {
        Some(e) => e,
        None => return false,
    };
    const LIST: &[&str] = &[
        "svg", "pdf", "gif", "webp", "heic", "heif", "tiff", "tif", "bmp", "ico", "psd", "ai",
        "zip", "gz", "tar", "7z", "rar", "dmg", "iso", "mp3", "wav", "flac", "aac", "ogg", "m4a",
        "mp4", "mov", "avi", "mkv", "webm", "m4v", "docx", "xlsx", "pptx", "doc", "xls", "ppt",
        "key", "numbers", "pages", "sqlite", "db", "bin", "exe", "dll", "so", "dylib", "o", "a",
        "class", "jar", "ttf", "otf", "woff", "woff2",
    ];
    LIST.iter()
        .any(|t| e.len() == t.len() && e.bytes().zip(t.bytes()).all(|(c, d)| c.to_ascii_lowercase() == d))
}

/// Décode un fichier audio (wav ou mp3) en PCM stéréo + débit.
fn decode_audio(name: &str, data: &[u8]) -> Option<(Vec<i16>, u32)> {
    if crate::wav::is_wav(data) {
        let p = crate::wav::decode(data)?;
        return Some((p.samples, p.rate));
    }
    if ext_is(name, &["mp3"])
        || (data.len() > 3 && (&data[..3] == b"ID3" || (data[0] == 0xff && data[1] & 0xe0 == 0xe0)))
    {
        return crate::mp3::decode(data);
    }
    None
}

/// Le contenu ressemble-t-il à du binaire (donc illisible en texte) ?
fn looks_binary(d: &[u8]) -> bool {
    let n = d.len().min(8192);
    if n == 0 {
        return false;
    }
    let mut bad = 0usize;
    for &b in &d[..n] {
        match b {
            0 => return true,                                  // NUL -> binaire
            0x09 | 0x0a | 0x0d | 0x20..=0x7e | 0x80..=0xff => {} // texte / UTF-8
            _ => bad += 1,                                      // autres contrôles
        }
    }
    bad * 100 / n > 10
}

// --- rendu -------------------------------------------------------------

fn trunc<'a>(s: &'a str, max: usize, buf: &'a mut [u8]) -> &'a str {
    let b = s.as_bytes();
    if b.len() <= max {
        return s;
    }
    let n = max.min(buf.len());
    buf[..n - 1].copy_from_slice(&b[..n - 1]);
    buf[n - 1] = b'~';
    core::str::from_utf8(&buf[..n]).unwrap_or(s)
}

/// Nombre de lignes une fois repliées à la largeur du panneau.
fn wrapped_lines(data: &[u8]) -> usize {
    let cols = ((PW - 2 * PADX) / 16).max(1) as usize;
    let mut lines = 1;
    let mut col = 0;
    for &b in data {
        if b == b'\n' {
            lines += 1;
            col = 0;
        } else {
            col += 1;
            if col >= cols {
                lines += 1;
                col = 0;
            }
        }
    }
    lines
}

pub fn draw(_now: f32) {
    unsafe {
        if L_OUT > 0.01 {
            draw_left((-(PW as f32) * (1.0 - L_OUT)) as i32);
        }
        if R_OUT > 0.01 {
            if VIEW == View::Audio {
                draw_audio_label(R_OUT);
            } else {
                draw_right((W as f32 - PW as f32 * R_OUT) as i32);
            }
        }
    }
}

fn draw_left(lx: i32) {
    fb::fill_rect(lx, 0, PW, H, P_CODE_BG);
    fb::fill_rect(lx + PW - 3, 0, 3, H, P_FRAME);

    // liste (dessinée d'abord, l'en-tête la masquera en haut)
    let mut y = HEADER_H - unsafe { L_SCROLL };
    let sel = unsafe { SEL };
    let count = unsafe { ENTRIES.len() };
    for i in 0..count {
        if y + ROW_H > HEADER_H && y < H {
            let e = unsafe { &ENTRIES[i] };
            if i as i32 == sel {
                fb::fill_rect(lx, y, PW, ROW_H, P_TITLE_HI);
                fb::fill_rect(lx, y, 4, ROW_H, P_ACCENT);
            }
            let pat = if e.dir { dots::FOLDER } else { dots::FILE };
            dots::draw(pat, lx + PADX, y + 8, 3, P_TEXT, P_DIM);
            let mut buf = [0u8; 48];
            let label = trunc(&e.name, 42, &mut buf);
            let col = if e.dir { P_TEXT } else { P_DIM };
            font::draw_str_scaled(lx + PADX + 54, y + 13, label, col, 2);
            if e.host {
                fb::fill_rect(lx + PW - 26, y + ROW_H / 2 - 5, 10, 10, P_STR);
            }
        }
        y += ROW_H;
    }

    // en-tête : fil d'Ariane cliquable
    fb::fill_rect(lx, 0, PW, HEADER_H, P_TITLE_HI);
    fb::fill_rect(lx, HEADER_H - 2, PW, 2, P_FRAME);
    dots::draw(dots::FOLDER, lx + PADX, 14, 4, P_TEXT, P_DIM);

    let segs = crumbs();
    let ranges = crumb_ranges(lx);
    let sep_w = font::width_scaled(" / ", 2);
    for (i, (x0, _x1, _d)) in ranges.iter().enumerate() {
        let cur = i + 1 == segs.len();
        let col = if cur { P_ACCENT } else { P_TEXT };
        font::draw_str_scaled(*x0, 16, &segs[i], col, 2);
        if !cur {
            font::draw_str_scaled(*x0 + font::width_scaled(&segs[i], 2), 16, " / ", P_DIM, 2);
        }
    }
    let _ = sep_w;

    let n = unsafe { ENTRIES.len() };
    let mut line = [0u8; 40];
    let mut k = itoa(n as u32, &mut line);
    for &c in b" elements  -  molette pour defiler" {
        if k < line.len() {
            line[k] = c;
            k += 1;
        }
    }
    font::draw_str_scaled(
        lx + PADX + 70,
        60,
        core::str::from_utf8(&line[..k]).unwrap_or(""),
        P_DIM,
        1,
    );

    // ascenseur
    scrollbar(lx + PW - 8, list_content_h(), unsafe { L_SCROLL });
}

/// Position x de chaque segment du fil d'Ariane (x0, x1, profondeur).
fn crumb_ranges(lx: i32) -> Vec<(i32, i32, usize)> {
    let segs = crumbs();
    let sep_w = font::width_scaled(" / ", 2);
    let mut out = Vec::new();
    let mut x = lx + PADX + 70;
    for (i, s) in segs.iter().enumerate() {
        let w = font::width_scaled(s, 2);
        out.push((x, x + w, i));
        x += w + sep_w;
    }
    out
}

fn itoa(mut v: u32, buf: &mut [u8]) -> usize {
    let mut tmp = [0u8; 12];
    let mut n = 0;
    loop {
        tmp[n] = b'0' + (v % 10) as u8;
        v /= 10;
        n += 1;
        if v == 0 {
            break;
        }
    }
    for i in 0..n {
        buf[i] = tmp[n - 1 - i];
    }
    n
}

fn draw_right(rx: i32) {
    let w = PW;
    fb::fill_rect(rx, 0, w, H, P_CODE_BG);
    fb::fill_rect(rx, 0, 3, H, P_FRAME);

    match unsafe { &VIEW } {
        View::Image => draw_image(rx),
        View::Audio => {} // dessiné par draw_audio_label (hors panneau)
        View::Message => {
            let msg = unsafe { &VIEW_MSG };
            let tw = font::width_scaled(msg, 2);
            font::draw_str_scaled(rx + (w - tw) / 2, H / 2 - 10, msg, P_DIM, 2);
            dots::draw_centered(dots::QUESTION, rx, H / 2 + 40, w, 80, 6, P_DIM, P_TITLE_HI);
        }
        View::Text => {
            let cols = ((PW - 2 * PADX) / 16).max(1) as usize;
            let mut x = rx + PADX;
            let mut y = HEADER_H + 6 - unsafe { R_SCROLL };
            let mut c = 0usize;
            for &b in unsafe { &CONTENT } {
                if b == b'\n' {
                    y += 22;
                    x = rx + PADX;
                    c = 0;
                    continue;
                }
                let ch = if b == b'\t' { b' ' } else { b };
                if (0x20..=0x7e).contains(&ch) {
                    if y + 16 > HEADER_H && y < H {
                        font::draw_char_scaled(x, y, ch, P_TEXT, None, 2);
                    }
                    x += 16;
                    c += 1;
                    if c >= cols {
                        y += 22;
                        x = rx + PADX;
                        c = 0;
                    }
                }
            }
        }
    }

    // en-tête (par-dessus le contenu)
    fb::fill_rect(rx, 0, w, HEADER_H, P_TITLE_HI);
    fb::fill_rect(rx, HEADER_H - 2, w, 2, P_FRAME);
    let hg = if unsafe { VIEW == View::Audio } {
        dots::NOTE
    } else {
        dots::FILE
    };
    dots::draw(hg, rx + PADX, 26, 4, P_TEXT, P_DIM);
    let mut buf = [0u8; 44];
    let name = trunc(unsafe { &CONTENT_NAME }, 34, &mut buf);
    font::draw_str_scaled(rx + PADX + 64, 30, name, P_TEXT, 3);

    scrollbar(rx + 2, view_content_h(), unsafe { R_SCROLL });
}

// --- lecteur audio compact --------------------------------------------

fn secs_str(s: f32, out: &mut [u8]) -> usize {
    let t = if s < 0.0 { 0 } else { s as u32 };
    let (m, sec) = (t / 60, t % 60);
    let mut n = itoa(m, out);
    if n < out.len() {
        out[n] = b':';
        n += 1;
    }
    if n < out.len() {
        out[n] = b'0' + (sec / 10) as u8;
        n += 1;
    }
    if n < out.len() {
        out[n] = b'0' + (sec % 10) as u8;
        n += 1;
    }
    n
}

// Petite étiquette de lecture, en bas à droite (glisse depuis le bord).
const A_W: i32 = 540;
const A_H: i32 = 90;
const SPEEDS: [(f32, &str); 3] = [(1.0, "x1"), (1.5, "x1.5"), (2.0, "x2")];

fn audio_rect(out: f32) -> (i32, i32, i32, i32) {
    let shown = W - 28 - A_W;
    let x = shown + ((1.0 - out) * (A_W as f32 + 56.0)) as i32;
    (x, H - A_H - 52, A_W, A_H)
}

fn speed_chip(x: i32, y: i32, i: usize) -> (i32, i32, i32, i32) {
    let (cw, gap) = (50, 8);
    let x0 = x + A_W - 16 - (3 * cw + 2 * gap);
    (x0 + i as i32 * (cw + gap), y + 12, cw, 26)
}

fn prog_bar(x: i32, y: i32) -> (i32, i32, i32) {
    (x + 54, y + A_H - 24, A_W - 54 - 140)
}

fn draw_audio_label(out: f32) {
    let (x, y, w, h) = audio_rect(out);
    fb::fill_rect(x - 2, y - 2, w + 4, h + 4, P_FRAME);
    fb::fill_rect(x, y, w, h, P_TITLE_HI);

    // bouton lecture / pause
    let (bx, by) = (x + 27, y + h / 2);
    fb::fill_circle(bx as f32, by as f32, 18.0, P_ACCENT);
    fb::fill_circle(bx as f32, by as f32, 15.0, P_TITLE_HI);
    let g = if crate::ac97::playing() {
        dots::PAUSE
    } else {
        dots::PLAY
    };
    dots::draw_centered(g, bx - 12, by - 12, 24, 24, 2, P_ACCENT, P_ACCENT);

    // nom du morceau
    let mut nb = [0u8; 40];
    let name = trunc(unsafe { &CONTENT_NAME }, 24, &mut nb);
    font::draw_str_scaled(x + 54, y + 13, name, P_TEXT, 2);

    // chips vitesse (en haut à droite)
    let cur = crate::ac97::speed();
    for (i, (v, lbl)) in SPEEDS.iter().enumerate() {
        let (sx, sy, sw, sh) = speed_chip(x, y, i);
        let on = (cur - v).abs() < 0.05;
        fb::fill_rect(sx, sy, sw, sh, if on { P_ACCENT } else { P_FRAME });
        let tw = font::width_scaled(lbl, 1);
        font::draw_str_scaled(
            sx + (sw - tw) / 2,
            sy + 9,
            lbl,
            if on { P_TITLE_HI } else { P_TEXT },
            1,
        );
    }

    // barre de progression (en bas)
    let prog = crate::ac97::progress();
    let (px, py, pw) = prog_bar(x, y);
    fb::fill_rect(px, py, pw, 6, P_FRAME);
    fb::fill_rect(px, py, (pw as f32 * prog) as i32, 6, P_ACCENT);
    let kx = px + (pw as f32 * prog) as i32;
    fb::fill_rect(kx - 2, py - 4, 4, 14, P_TEXT);

    // "1:04 / 3:12"
    let dur = crate::ac97::duration();
    let mut t = [0u8; 20];
    let mut k = secs_str(dur * prog, &mut t);
    for &c in b" / " {
        if k < t.len() {
            t[k] = c;
            k += 1;
        }
    }
    k += secs_str(dur, &mut t[k..]);
    font::draw_str_scaled(
        px + pw + 12,
        py - 5,
        core::str::from_utf8(&t[..k]).unwrap_or(""),
        P_DIM,
        1,
    );
}

fn audio_click(mx: i32, my: i32) {
    let (x, y, _w, h) = audio_rect(1.0);

    let (bx, by) = (x + 27, y + h / 2);
    if (mx - bx).pow(2) + (my - by).pow(2) < 24 * 24 {
        crate::ac97::toggle();
        return;
    }
    for (i, (v, _)) in SPEEDS.iter().enumerate() {
        let (sx, sy, sw, sh) = speed_chip(x, y, i);
        if mx >= sx && mx <= sx + sw && my >= sy && my <= sy + sh {
            crate::ac97::set_speed(*v);
            return;
        }
    }
    let (px, py, pw) = prog_bar(x, y);
    if mx >= px && mx <= px + pw && (my - (py + 3)).abs() < 14 {
        crate::ac97::seek((mx - px) as f32 / pw as f32);
        return;
    }
}

fn draw_image(rx: i32) {
    let (w, h) = unsafe { (IMG_W, IMG_H) };
    if w <= 0 || h <= 0 {
        return;
    }
    let ox = rx + (PW - w) / 2;
    let avail = H - HEADER_H;
    let oy = if h <= avail {
        HEADER_H + (avail - h) / 2
    } else {
        HEADER_H + 12 - unsafe { R_SCROLL }
    };
    // damier léger derrière (transparence éventuelle)
    let px = unsafe { &IMG_PX };
    for j in 0..h {
        let sy = oy + j;
        if sy < HEADER_H || sy >= H {
            continue;
        }
        let row = (j * w) as usize;
        for i in 0..w {
            fb::put(ox + i, sy, px[row + i as usize]);
        }
    }
}

fn scrollbar(x: i32, content_h: i32, scroll: i32) {
    let view_h = H - HEADER_H;
    if content_h <= view_h {
        return;
    }
    let track_h = view_h - 8;
    let knob_h = (track_h * view_h / content_h).max(24);
    let max_scroll = content_h - view_h;
    let knob_y = HEADER_H + 4 + (track_h - knob_h) * scroll / max_scroll.max(1);
    fb::fill_rect(x, HEADER_H + 4, 4, track_h, P_FRAME);
    fb::fill_rect(x, knob_y, 4, knob_h, P_ACCENT);
}
