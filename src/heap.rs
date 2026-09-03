//! Allocateur de tas. Débloque `alloc` : `Vec`, `String`, `Box`,
//! `BTreeMap`... indispensable pour la suite (partage 9p, HTTP, FAT).
//!
//! Le tas est une zone statique de 16 Mio dans le `.bss` du noyau (donc
//! dans le premier Gio déjà identity-mappé). Allocateur à liste chaînée
//! (crate `linked_list_allocator`) : gère la libération et la
//! réutilisation, contrairement à un simple bump.

use core::alloc::Layout;

use linked_list_allocator::LockedHeap;

const HEAP_SIZE: usize = 16 * 1024 * 1024;

static mut HEAP: [u8; HEAP_SIZE] = [0; HEAP_SIZE];

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

/// À appeler une fois, tôt au boot (la pagination est déjà active).
pub fn init() {
    unsafe {
        ALLOCATOR.lock().init(&raw mut HEAP as *mut u8, HEAP_SIZE);
    }
    crate::serial_println!("[nothing-os] tas : {} Mio", HEAP_SIZE / (1024 * 1024));
}

/// Octets utilisés / libres du tas (diagnostic, ex. commande `mem`).
pub fn stats() -> (usize, usize) {
    let h = ALLOCATOR.lock();
    (h.used(), h.free())
}

#[alloc_error_handler]
fn on_oom(layout: Layout) -> ! {
    panic!("plus de memoire (demande {} octets)", layout.size());
}
