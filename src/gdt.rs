//! GDT (Global Descriptor Table) + TSS (Task State Segment).
//!
//! En long mode la segmentation ne sert quasiment plus à rien... sauf
//! une chose : le TSS reste le seul moyen de dire au CPU « pour telle
//! interruption, utilise CETTE pile-là plutôt que la pile courante ».
//!
//! C'est indispensable pour le gestionnaire de *double fault* : si le
//! double fault est provoqué par un débordement de la pile noyau (stack
//! overflow), la pile courante est déjà inutilisable. Sans pile de
//! secours, le CPU n'arrive même pas à empiler la stack frame de
//! l'interruption → il refault → *triple fault* → QEMU redémarre en
//! boucle, écran noir, aucun message.
//!
//! On met donc en place :
//!   - un TSS avec une entrée dans l'IST (Interrupt Stack Table) qui
//!     pointe vers une petite pile dédiée ;
//!   - une GDT 64-bit minimale (descripteur de code noyau + descripteur
//!     du TSS) puisqu'on ne peut charger un `ltr` que depuis une GDT.
//!
//! `interrupts.rs` récupère `DOUBLE_FAULT_IST_INDEX` pour associer cette
//! pile au handler de double fault.

use x86_64::instructions::segmentation::{Segment, CS};
use x86_64::instructions::tables::load_tss;
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;

/// Index (dans l'IST du TSS) de la pile réservée aux double faults.
pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

/// Taille de la pile de secours : 5 pages (20 Kio). Largement assez pour
/// afficher un écran d'erreur et faire `hlt`, on n'y fait rien de lourd.
const STACK_SIZE: usize = 4096 * 5;

/// Pile dédiée au gestionnaire de double fault.
///
/// `static mut` sans `Mutex` : cette zone n'est écrite QUE par le CPU
/// lui-même (quand il bascule dessus pour servir l'interruption), jamais
/// par notre code Rust. On n'en prend l'adresse qu'une fois, au boot.
static mut DOUBLE_FAULT_STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

static mut TSS: TaskStateSegment = TaskStateSegment::new();
static mut GDT: GlobalDescriptorTable = GlobalDescriptorTable::new();

/// Sélecteurs renvoyés par `init()`. Personne ne s'en sert encore (l'IDT
/// référence l'IST par un simple index, pas par un sélecteur), mais on
/// les garde sous la main pour la suite : rechargement de CS, futur
/// segment TLS, passage en ring 3...
#[allow(dead_code)]
#[derive(Clone, Copy)]
pub struct Selectors {
    pub code_selector: SegmentSelector,
    pub tss_selector: SegmentSelector,
}

/// Construit le TSS + la GDT, les charge, recharge CS et fait `ltr`.
///
/// À appeler AVANT `interrupts::init()` : l'IDT référence l'index IST
/// mis en place ici.
pub fn init() -> Selectors {
    unsafe {
        #[allow(static_mut_refs)]
        {
            // 1. Renseigne la pile de secours dans l'IST du TSS.
            let stack_start = VirtAddr::from_ptr(&raw const DOUBLE_FAULT_STACK);
            let stack_end = stack_start + STACK_SIZE as u64;
            // La pile croît vers le bas : le CPU veut le sommet (adresse haute).
            TSS.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = stack_end;

            // 2. GDT : segment de code noyau + segment système du TSS.
            let code_selector = GDT.append(Descriptor::kernel_code_segment());
            // `_unchecked` : on passe un pointeur brut plutôt qu'une
            // `&'static` sur un `static mut` (ça éviterait un warning de
            // référence potentiellement partagée). Sûr ici : le TSS n'est
            // écrit qu'au-dessus, avant ce chargement, puis figé.
            let tss_selector = GDT.append(Descriptor::tss_segment_unchecked(&raw const TSS));
            GDT.load();

            // 3. Recharge CS (le sélecteur hérité du boot asm devient
            //    invalide vis-à-vis de NOTRE GDT) et charge le TSS.
            CS::set_reg(code_selector);
            load_tss(tss_selector);

            crate::serial_println!("[toy-os] GDT + TSS charges (IST pour double fault)");

            Selectors {
                code_selector,
                tss_selector,
            }
        }
    }
}
