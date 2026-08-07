//! The one canonical byte-level toolpath fingerprint.
//!
//! Four test files had grown their own copy of the same FNV-1a-over-`Debug`
//! hash (`transform_provenance_fingerprints`, `checkpoint_b_resolution_ab`,
//! `crease_own_region_pr6b`, `finish_resolution_policy_pr3`), differing only
//! in whether they returned the move count alongside the hash. The algorithm
//! is load-bearing — pinned constants in those files were captured with it —
//! so this module reproduces it exactly rather than "improving" it.
//!
//! **Do not change the algorithm.** FNV-1a is used deliberately in place of
//! `DefaultHasher`, whose output is explicitly not stable across toolchain
//! releases; every pinned constant in the repo would silently rot.
//!
//! Reference consumers: `finish_resolution_policy_pr3.rs` (three pinned
//! `(moves, hash)` constants captured at HEAD `606b8d5` / PR-8a / PR-8b).

#![allow(dead_code)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::fmt::Debug;

use rs_cam_core::toolpath::Toolpath;

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// FNV-1a over the `Debug` rendering of any value.
///
/// `Debug` on `f64` round-trips every bit, so this is a byte-level identity
/// check on the whole structure, not a lossy summary.
pub fn fnv1a_debug<T: Debug + ?Sized>(value: &T) -> u64 {
    let mut h: u64 = FNV_OFFSET_BASIS;
    for byte in format!("{value:?}").bytes() {
        h ^= u64::from(byte);
        h = h.wrapping_mul(FNV_PRIME);
    }
    h
}

/// Move count plus the hash of the move list — the pair form most sentries
/// pin, because a bare hash tells you nothing about *how* an output moved.
pub fn move_fingerprint(tp: &Toolpath) -> (usize, u64) {
    (tp.moves.len(), fnv1a_debug(&tp.moves))
}

/// Hash-only form, for harnesses that compare arms rather than pin values.
pub fn move_hash(tp: &Toolpath) -> u64 {
    fnv1a_debug(&tp.moves)
}
