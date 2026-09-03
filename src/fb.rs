//! Framebuffer graphique via l'interface **Bochs VBE** (que QEMU expose
//! avec `-vga std`) : résolution d'écran réelle, 256 couleurs (palette
//! DAC), framebuffer linéaire dont l'adresse physique est lue dans le
//! BAR0 PCI de la carte VGA.
//!
//! Le dessin se fait dans un back-buffer (`BACK`) puis `present()` le
//! recopie d'un bloc vers la VRAM — pas de scintillement.

use crate::port::{inb, inl, outb, outl, outw};

pub const WIDTH: usize = 640;
pub const HEIGHT: usize = 480;

// --- Bochs VBE (Dispi) ---
const VBE_INDEX: u16 = 0x01ce;
const VBE_DATA: u16 = 0x01cf;
const VBE_XRES: u16 = 1;
const VBE_YRES: u16 = 2;
const VBE_BPP: u16 = 3;
const VBE_ENABLE: u16 = 4;
const VBE_VIRT_WIDTH: u16 = 6;
const VBE_ENABLED: u16 = 0x01;
const VBE_LFB_ENABLED: u16 = 0x40;

// --- palette DAC ---
const DAC_WRITE_ADDR: u16 = 0x3c8;
const DAC_DATA: u16 = 0x3c9;

static mut LFB: *mut u8 = core::ptr::null_mut();
static mut BACK: [u8; WIDTH * HEIGHT] = [0; WIDTH * HEIGHT];

fn back() -> *mut u8 {
    &raw mut BACK as *mut u8
}

fn vbe_write(index: u16, value: u16) {
    unsafe {
        outw(VBE_INDEX, index);
        outw(VBE_DATA, value);
    }
}

// --- accès à l'espace de configuration PCI (mécanisme n°1) ---
fn pci_read32(bus: u8, dev: u8, func: u8, off: u8) -> u32 {
    let addr = 0x8000_0000
        | (bus as u32) << 16
        | (dev as u32) << 11
        | (func as u32) << 8
        | (off as u32 & 0xfc);
    unsafe {
        outl(0xcf8, addr);
        inl(0xcfc)
    }
}

/// Cherche le framebuffer linéaire : BAR0 du premier contrôleur
/// d'affichage (classe PCI 0x03). Valeur de secours si rien trouvé.
fn find_lfb() -> u32 {
    for dev in 0..32u8 {
        if pci_read32(0, dev, 0, 0x00) == 0xffff_ffff {
            continue;
        }
        let class = pci_read32(0, dev, 0, 0x08) >> 24;
        if class == 0x03 {
            return pci_read32(0, dev, 0, 0x10) & 0xffff_fff0;
        }
    }
    0xe000_0000
}

/// Passe l'écran en mode graphique `WIDTH×HEIGHT`, 8 bits par pixel.
pub fn init() {
    let lfb = find_lfb();
    unsafe {
        LFB = lfb as *mut u8;
    }
    crate::serial_println!("[nothing-os] framebuffer @ {:#x}", lfb);

    vbe_write(VBE_ENABLE, 0);
    vbe_write(VBE_XRES, WIDTH as u16);
    vbe_write(VBE_YRES, HEIGHT as u16);
    vbe_write(VBE_BPP, 8);
    vbe_write(VBE_VIRT_WIDTH, WIDTH as u16);
    vbe_write(VBE_ENABLE, VBE_ENABLED | VBE_LFB_ENABLED);

    // certains chemins d'init laissent le DAC en mode 6 bits masqués ;
    // on s'assure que le registre de masque est ouvert
    unsafe {
        let _ = inb(0x3c6);
        outb(0x3c6, 0xff);
    }
}

/// Programme une entrée de palette (composantes 0..=255, DAC 6 bits).
pub fn set_palette(index: u8, r: u8, g: u8, b: u8) {
    unsafe {
        outb(DAC_WRITE_ADDR, index);
        outb(DAC_DATA, r >> 2);
        outb(DAC_DATA, g >> 2);
        outb(DAC_DATA, b >> 2);
    }
}

/// Recopie le back-buffer vers la VRAM.
pub fn present() {
    unsafe {
        if !LFB.is_null() {
            core::ptr::copy_nonoverlapping(back(), LFB, WIDTH * HEIGHT);
        }
    }
}

#[inline(always)]
pub fn put(x: i32, y: i32, color: u8) {
    if x < 0 || y < 0 || x >= WIDTH as i32 || y >= HEIGHT as i32 {
        return;
    }
    unsafe {
        *back().add(y as usize * WIDTH + x as usize) = color;
    }
}

pub fn fill_rect(x: i32, y: i32, w: i32, h: i32, color: u8) {
    let x0 = x.max(0);
    let y0 = y.max(0);
    let x1 = (x + w).min(WIDTH as i32);
    let y1 = (y + h).min(HEIGHT as i32);
    for yy in y0..y1 {
        let row = unsafe { back().add(yy as usize * WIDTH) };
        for xx in x0..x1 {
            unsafe { *row.add(xx as usize) = color };
        }
    }
}

/// Disque plein (test au rayon).
pub fn fill_circle(cx: f32, cy: f32, rad: f32, color: u8) {
    let x0 = (cx - rad - 1.0) as i32;
    let x1 = (cx + rad + 1.0) as i32;
    let y0 = (cy - rad - 1.0) as i32;
    let y1 = (cy + rad + 1.0) as i32;
    let r2 = rad * rad;
    for y in y0..=y1 {
        for x in x0..=x1 {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            if dx * dx + dy * dy <= r2 {
                put(x, y, color);
            }
        }
    }
}
