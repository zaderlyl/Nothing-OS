//! Vignette « infos système » (coin bas-droite) : Wi-Fi, ports branchés,
//! batterie, CPU / mémoire. Les données viennent du Mac via
//! `bridge/sysinfo.sh` qui écrit `.nothingos-sys` sur le partage 9p.

#![allow(static_mut_refs, dead_code)]

use alloc::string::{String, ToString};

use crate::{fb, font, p9};

const PATH: &str = ".nothingos-sys";

const BG: u8 = 50;
const LINE: u8 = 51;
const TXT: u8 = 52;
const DIM: u8 = 53;
const OK: u8 = 54;
const WARN: u8 = 59;

pub fn install_palette() {
    fb::set_palette(BG, 18, 19, 26);
    fb::set_palette(LINE, 48, 51, 64);
    fb::set_palette(TXT, 222, 227, 240);
    fb::set_palette(DIM, 120, 126, 145);
    fb::set_palette(OK, 120, 210, 150);
    fb::set_palette(WARN, 235, 170, 90);
}

struct Sys {
    wifi: String,
    wifi_power: String,
    iface: String,
    online: bool,
    battery: i32,
    charging: bool,
    batt_state: String,
    mem_free: i32,
    cpu: i32,
    ports: String,
    host: String,
    got: bool,
}

static mut S: Sys = Sys {
    wifi: String::new(),
    wifi_power: String::new(),
    iface: String::new(),
    online: false,
    battery: -1,
    charging: false,
    batt_state: String::new(),
    mem_free: -1,
    cpu: -1,
    ports: String::new(),
    host: String::new(),
    got: false,
};

static mut LAST: f32 = -100.0;

/// Des données ont-elles déjà été reçues du Mac ?
pub fn available() -> bool {
    unsafe { S.got }
}

/// À appeler chaque image ; relit `.nothingos-sys` toutes les ~2 s.
pub fn poll(now: f32) {
    unsafe {
        if now - LAST < 2.0 {
            return;
        }
        LAST = now;
        let d = match p9::read_file(PATH) {
            Some(d) => d,
            None => return,
        };
        let text = match core::str::from_utf8(&d) {
            Ok(t) => t,
            Err(_) => return,
        };
        for line in text.lines() {
            let (k, v) = match line.split_once('=') {
                Some(kv) => kv,
                None => continue,
            };
            match k {
                "wifi" => S.wifi = v.to_string(),
                "wifi_power" => S.wifi_power = v.to_string(),
                "iface" => S.iface = v.to_string(),
                "online" => S.online = v == "1",
                "battery" => S.battery = v.parse().unwrap_or(-1),
                "charging" => S.charging = v == "1",
                "batt_state" => S.batt_state = v.to_string(),
                "mem_free" => S.mem_free = v.parse().unwrap_or(-1),
                "cpu" => S.cpu = v.parse().unwrap_or(-1),
                "ports" => S.ports = v.to_string(),
                "host" => S.host = v.to_string(),
                _ => {}
            }
        }
        S.got = true;
    }
}

// --- géométrie ---
const PW: i32 = 560;
const PH: i32 = 296;
const MARGIN: i32 = 28;

/// Rectangle de la vignette pour un avancement `slide` (0 cachée, 1 sortie).
pub fn rect(slide: f32) -> (i32, i32, i32, i32) {
    let w = fb::WIDTH as i32;
    let h = fb::HEIGHT as i32;
    let x = w - PW - MARGIN;
    let shown_y = h - PH - MARGIN;
    let hidden_y = h + 12;
    let y = hidden_y + ((shown_y - hidden_y) as f32 * slide) as i32;
    (x, y, PW, PH)
}

/// La souris est-elle dans la zone qui déclenche la vignette ?
pub fn hot(mx: i32, my: i32, slide: f32) -> bool {
    let w = fb::WIDTH as i32;
    let h = fb::HEIGHT as i32;
    if mx > w - 210 && my > h - 64 {
        return true; // coin bas-droite
    }
    if slide > 0.1 {
        let (x, y, pw, ph) = rect(slide);
        return mx >= x - 6 && mx <= x + pw + 6 && my >= y - 6 && my <= y + ph + 6;
    }
    false
}

pub fn draw(slide: f32) {
    if slide < 0.02 {
        draw_handle();
        return;
    }
    let (x, y, w, hgt) = rect(slide);
    unsafe {
        fb::fill_rect(x - 2, y - 2, w + 4, hgt + 4, LINE);
        fb::fill_rect(x, y, w, hgt, BG);
        fb::fill_rect(x, y, 4, hgt, OK);

        let px = x + 34;
        let mut cy = y + 26;
        font::draw_str_scaled(px, cy, "SYSTEME", DIM, 2);
        let host = if S.host.is_empty() { "—" } else { S.host.as_str() };
        font::draw_str_scaled(x + w - 34 - font::width_scaled(host, 2), cy, host, DIM, 2);
        cy += 42;

        // Wi-Fi
        wifi_icon(px, cy - 2, S.online);
        let net = if S.wifi_power == "Off" {
            "Wi-Fi coupe".to_string()
        } else if !S.wifi.is_empty() && S.wifi != "—" {
            S.wifi.clone()
        } else if !S.iface.is_empty() {
            let mut s = "via ".to_string();
            s.push_str(&S.iface);
            s
        } else {
            "hors ligne".to_string()
        };
        font::draw_str_scaled(px + 44, cy, &net, TXT, 2);
        let tag = if S.online { "en ligne" } else { "hors ligne" };
        let tc = if S.online { OK } else { WARN };
        font::draw_str_scaled(x + w - 34 - font::width_scaled(tag, 2), cy, tag, tc, 2);
        cy += 44;

        // Ports branchés
        plug_icon(px, cy - 2);
        let ports = if S.ports.is_empty() || S.ports == "—" {
            "aucun peripherique".to_string()
        } else {
            S.ports.replace('|', ", ")
        };
        let pc = if ports.starts_with('a') { DIM } else { TXT };
        draw_fit(px + 44, cy, x + w - 34, &ports, pc);
        cy += 44;

        // Batterie
        let b = S.battery.clamp(0, 100);
        batt_icon(px, cy - 4, b, S.charging);
        let mut bt = b.to_string();
        bt.push_str(" %");
        font::draw_str_scaled(px + 66, cy, &bt, TXT, 2);
        let st = if S.charging { "sur secteur" } else { "sur batterie" };
        font::draw_str_scaled(x + w - 34 - font::width_scaled(st, 2), cy, st, if S.charging { OK } else { DIM }, 2);
        cy += 46;

        // CPU / mémoire
        gauge(px, cy, 224, "CPU", S.cpu);
        gauge(px + 264, cy, 224, "MEM.", S.mem_free);
    }
}

fn draw_handle() {
    // 3 points discrets dans le coin bas-droite : « il y a un truc ici »
    let w = fb::WIDTH as i32;
    let h = fb::HEIGHT as i32;
    for i in 0..3 {
        fb::fill_rect(w - 20 - i * 12, h - 14, 6, 6, LINE);
    }
}

/// Écrit `s` à l'échelle 2, tronqué avec « … » pour tenir avant `maxx`.
fn draw_fit(x: i32, y: i32, maxx: i32, s: &str, col: u8) {
    let avail = maxx - x;
    if font::width_scaled(s, 2) <= avail {
        font::draw_str_scaled(x, y, s, col, 2);
        return;
    }
    let cw = font::width_scaled("m", 2).max(1);
    let n = ((avail / cw) as usize).saturating_sub(1);
    let mut t: String = s.chars().take(n).collect();
    t.push('.');
    font::draw_str_scaled(x, y, &t, col, 2);
}

fn gauge(x: i32, y: i32, w: i32, label: &str, pct: i32) {
    font::draw_str_scaled(x, y, label, DIM, 2);
    let by = y + 26;
    fb::fill_rect(x, by, w, 8, LINE);
    if pct >= 0 {
        let p = (w * pct.clamp(0, 100)) / 100;
        let c = if pct > 85 { WARN } else { OK };
        fb::fill_rect(x, by, p, 8, c);
        let mut t = pct.to_string();
        t.push_str(" %");
        font::draw_str_scaled(x + w - font::width_scaled(&t, 2), y, &t, TXT, 2);
    }
}

fn wifi_icon(x: i32, y: i32, on: bool) {
    let c = if on { OK } else { DIM };
    // trois « barres » croissantes
    for i in 0..3 {
        let bh = 6 + i * 7;
        fb::fill_rect(x + i * 9, y + 20 - bh, 6, bh, c);
    }
}

fn plug_icon(x: i32, y: i32) {
    fb::fill_rect(x + 6, y + 2, 12, 14, DIM); // corps
    fb::fill_rect(x + 2, y + 5, 4, 3, DIM); // broche
    fb::fill_rect(x + 2, y + 11, 4, 3, DIM);
    fb::fill_rect(x + 18, y + 7, 8, 4, DIM); // câble
}

fn batt_icon(x: i32, y: i32, pct: i32, charging: bool) {
    fb::fill_rect(x, y, 48, 22, DIM); // contour
    fb::fill_rect(x + 2, y + 2, 44, 18, BG); // intérieur
    fb::fill_rect(x + 48, y + 7, 4, 8, DIM); // borne
    let fillw = (42 * pct.clamp(0, 100)) / 100;
    let c = if charging {
        OK
    } else if pct <= 15 {
        WARN
    } else {
        TXT
    };
    fb::fill_rect(x + 3, y + 3, fillw.max(0), 16, c);
}
