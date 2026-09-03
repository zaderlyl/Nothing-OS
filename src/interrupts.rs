//! Table des descripteurs d'interruption (IDT).
//!
//! Sans ça, la moindre exception CPU (division par zéro, accès mémoire
//! invalide, etc.) fait planter la machine en silence (triple fault ->
//! redémarrage), sans le moindre message. Ici on branche juste deux
//! gestionnaires pour commencer :
//!   - `breakpoint` (int3) : affiche un message et laisse le noyau
//!     continuer normalement (utile pour tester que l'IDT fonctionne).
//!   - `double_fault` : affiche un message façon "kernel panic" et
//!     arrête proprement plutôt que de laisser QEMU reboot en boucle.

use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

use crate::gdt;
use crate::{clear_screen, println, serial_println, set_color, vga};

// Le handler `breakpoint` n'écrit QUE sur le port série : l'écran est
// réservé à l'accueil, on ne veut pas qu'un int3 vienne le griffonner.

static mut IDT: InterruptDescriptorTable = InterruptDescriptorTable::new();

pub fn init() {
    unsafe {
        #[allow(static_mut_refs)]
        {
            IDT.breakpoint.set_handler_fn(breakpoint_handler);

            // Le double fault s'exécute sur sa propre pile (mise en place
            // dans gdt.rs) : sinon un débordement de la pile noyau ne
            // pourrait pas être servi et finirait en triple fault.
            IDT.double_fault
                .set_handler_fn(double_fault_handler)
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);

            IDT.load();
        }
    }

    serial_println!("[nothing-os] IDT chargee (breakpoint, double fault)");
}

extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    serial_println!("[nothing-os] EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    serial_println!("[nothing-os] EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);

    set_color(vga::Color::White, vga::Color::Red);
    clear_screen();
    println!("DOUBLE FAULT");
    println!("{:#?}", stack_frame);

    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}
