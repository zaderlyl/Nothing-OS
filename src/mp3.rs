//! Décodage MP3 → PCM stéréo `i16`.
//!
//! (Portage `no_std` d'un décodeur Layer III en cours — pour l'instant
//! seul le WAV est lu, voir `src/wav.rs`.)

#![allow(dead_code)]

use alloc::vec::Vec;

/// `Some((échantillons stéréo entrelacés, débit Hz))`.
pub fn decode(_data: &[u8]) -> Option<(Vec<i16>, u32)> {
    None
}
