//! Port série COM1 (0x3F8), pour logguer des messages de debug visibles
//! depuis QEMU (option `-serial stdio` ou `-serial file:...`) sans passer
//! par l'écran VGA.

use core::fmt;

const COM1: u16 = 0x3f8;

#[inline(always)]
unsafe fn outb(port: u16, value: u8) {
    core::arch::asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags));
}

#[inline(always)]
unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    core::arch::asm!("in al, dx", in("dx") port, out("al") value, options(nomem, nostack, preserves_flags));
    value
}

pub struct SerialPort;

impl SerialPort {
    pub const fn new() -> SerialPort {
        SerialPort
    }

    fn line_status_ready(&self) -> bool {
        unsafe { inb(COM1 + 5) & 0x20 != 0 }
    }

    pub fn write_byte(&mut self, byte: u8) {
        while !self.line_status_ready() {}
        unsafe { outb(COM1, byte) };
    }
}

impl fmt::Write for SerialPort {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            self.write_byte(byte);
        }
        Ok(())
    }
}

