//! Scan minimal du bus PCI (mécanisme n°1, ports 0xCF8/0xCFC).

#![allow(dead_code)]

use crate::port::{inl, outl};

const CONFIG_ADDR: u16 = 0xcf8;
const CONFIG_DATA: u16 = 0xcfc;

#[derive(Clone, Copy)]
pub struct Device {
    pub bus: u8,
    pub slot: u8,
    pub func: u8,
}

impl Device {
    fn addr(&self, off: u8) -> u32 {
        0x8000_0000
            | (self.bus as u32) << 16
            | (self.slot as u32) << 11
            | (self.func as u32) << 8
            | (off as u32 & 0xfc)
    }

    pub fn read32(&self, off: u8) -> u32 {
        unsafe {
            outl(CONFIG_ADDR, self.addr(off));
            inl(CONFIG_DATA)
        }
    }

    pub fn write32(&self, off: u8, val: u32) {
        unsafe {
            outl(CONFIG_ADDR, self.addr(off));
            outl(CONFIG_DATA, val);
        }
    }

    pub fn read16(&self, off: u8) -> u16 {
        (self.read32(off) >> ((off as u32 & 2) * 8)) as u16
    }

    pub fn vendor(&self) -> u16 {
        self.read16(0x00)
    }
    pub fn device(&self) -> u16 {
        self.read16(0x02)
    }

    /// BAR `n` (0..=5) : adresse de base (bits de type masqués).
    pub fn bar(&self, n: u8) -> u32 {
        self.read32(0x10 + n * 4)
    }

    /// Active la maîtrise du bus (bit 2 de la commande) + I/O + mémoire.
    pub fn enable_bus_master(&self) {
        let cmd = self.read32(0x04);
        self.write32(0x04, cmd | 0x7);
    }

    /// Pointeur de capabilités (offset 0x34), 0 si aucune.
    pub fn caps_ptr(&self) -> u8 {
        (self.read32(0x34) & 0xff) as u8
    }
}

/// Cherche le premier périphérique (vendor, device) donné.
pub fn find(vendor: u16, device: u16) -> Option<Device> {
    for bus in 0..=255u16 {
        for slot in 0..32u8 {
            let d = Device {
                bus: bus as u8,
                slot,
                func: 0,
            };
            if d.read32(0x00) == 0xffff_ffff {
                continue;
            }
            let multi = d.read32(0x0c) & 0x0080_0000 != 0;
            let funcs = if multi { 8 } else { 1 };
            for func in 0..funcs {
                let d = Device {
                    bus: bus as u8,
                    slot,
                    func,
                };
                if d.vendor() == vendor && d.device() == device {
                    return Some(d);
                }
            }
        }
    }
    None
}
