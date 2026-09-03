//! Écran d'accueil de Nothing OS.
//!
//! Il n'y a pas de bureau, pas d'icônes, pas de menu : juste un fond
//! noir avec le nom de l'OS, le personnage **Asti**, et sa **barre de
//! nourriture**. C'est, pour le moment, *tout* le système.
//!
//! État courant : l'écran est dessiné une fois au démarrage. Plus tard,
//! un timer (PIT, IRQ0) fera baisser la nourriture avec le temps et le
//! clavier permettra de nourrir Asti — d'où le découpage en petites
//! fonctions (`set_food`, `feed`, `starve`, `render`) prêtes à être
//! rappelées.

// `feed` / `starve` / `set_food` ne sont pas encore appelés (il manque le
// timer et le clavier) — on les garde en place pour la suite.
#![allow(dead_code)]

use core::sync::atomic::{AtomicU8, Ordering};

use crate::vga::Color;
use crate::{clear_screen, print, put_raw, set_color, set_position};

/// Largeur de l'écran texte VGA (mode 80x25).
const SCREEN_WIDTH: usize = 80;

/// Nom affiché de l'OS (lettres espacées pour l'allure "titre").
const OS_NAME: &str = "N O T H I N G   O S";

/// Personnage Asti, en art ASCII. Une entrée = une ligne ; toutes les
/// lignes doivent faire la même largeur (ici 15). N'utiliser que de
/// l'ASCII imprimable : le reste est filtré par le pilote VGA.
const ASTI: &[&str] = &[
    "    .-----.    ",
    "   /       \\   ",
    "  |  o   o  |  ",
    "  |    -    |  ",
    "   \\  \\_/  /   ",
    "    '-----'    ",
    "   _/     \\_   ",
];

/// Octets "graphiques" de la code page 437 du BIOS, pour la barre.
const BLOCK_FULL: u8 = 0xDB; // █  portion pleine
const BLOCK_LIGHT: u8 = 0xB0; // ░  portion vide

/// Niveau de nourriture d'Asti : 0 = affamé, 100 = repu.
static FOOD: AtomicU8 = AtomicU8::new(100);

/// Colonne de départ pour centrer un texte de `len` caractères.
fn centered_col(len: usize) -> usize {
    SCREEN_WIDTH.saturating_sub(len) / 2
}

/// Niveau de nourriture actuel (0..=100).
pub fn food() -> u8 {
    FOOD.load(Ordering::Relaxed)
}

/// Fixe le niveau de nourriture (borné à 100).
pub fn set_food(value: u8) {
    FOOD.store(value.min(100), Ordering::Relaxed);
}

/// Nourrit Asti de `amount` points (borné à 100).
pub fn feed(amount: u8) {
    set_food(food().saturating_add(amount));
}

/// Fait baisser la nourriture de `amount` points (borné à 0).
pub fn starve(amount: u8) {
    FOOD.store(food().saturating_sub(amount), Ordering::Relaxed);
}

/// (Re)dessine tout l'écran d'accueil sur fond noir.
pub fn render() {
    clear_screen();

    // --- Nom de l'OS -------------------------------------------------
    set_color(Color::White, Color::Black);
    set_position(2, centered_col(OS_NAME.len()));
    print!("{}", OS_NAME);

    // --- Asti ------------------------------------------------------
    let art_col = centered_col(ASTI[0].len());
    set_color(Color::LightCyan, Color::Black);
    for (i, line) in ASTI.iter().enumerate() {
        set_position(8 + i, art_col);
        print!("{}", line);
    }

    // Étiquette du personnage, juste en dessous.
    set_color(Color::DarkGray, Color::Black);
    set_position(8 + ASTI.len() + 1, centered_col(4));
    print!("Asti");

    // --- Barre de nourriture --------------------------------------
    draw_food_bar(19);
}

/// Dessine `Nourriture [████████░░░░░░░░] 62%` centré sur la ligne `row`.
fn draw_food_bar(row: usize) {
    const BAR_WIDTH: usize = 40;
    const LABEL: &str = "Nourriture ";

    let pct = food() as usize;
    let filled = pct * BAR_WIDTH / 100;
    let empty = BAR_WIDTH - filled;

    // Largeur totale : "Nourriture " + "[" + barre + "] " + "100%".
    let total = LABEL.len() + 1 + BAR_WIDTH + 2 + 4;
    let col = centered_col(total);

    set_position(row, col);
    set_color(Color::LightGray, Color::Black);
    print!("{}[", LABEL);

    // Portion pleine : verte / jaune / rouge selon le niveau.
    let fill_color = match pct {
        0..=20 => Color::LightRed,
        21..=50 => Color::Yellow,
        _ => Color::LightGreen,
    };
    set_color(fill_color, Color::Black);
    put_raw(BLOCK_FULL, filled);

    set_color(Color::DarkGray, Color::Black);
    put_raw(BLOCK_LIGHT, empty);

    set_color(Color::LightGray, Color::Black);
    print!("] {:>3}%", pct);
}
