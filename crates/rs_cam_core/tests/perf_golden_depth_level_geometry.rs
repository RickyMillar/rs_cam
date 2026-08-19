//! Phase 0B correctness golden for **G2**: a depth-stepped 2.5D operation's
//! XY geometry is identical at every Z level.
//!
//! `planning/perf_review_2026-08-19/PERF_REVIEW.md` G2: `depth.rs:282-295`
//! calls `operation(z)` once per level, and every parameter but `cut_depth`
//! is loop-invariant — `cut_depth` is consumed in exactly one place
//! (`pocket.rs:390-393`, the Z stamp). The proposed fix hoists the 2D
//! geometry above `toolpath_at_levels_with_cancel` so the per-level closure
//! only stamps Z, and claims the output is "identical by construction".
//!
//! **This file encodes that claim BEFORE the fix exists**, which is the only
//! order in which it means anything: a golden captured after a refactor
//! proves the refactor is self-consistent, not that it preserved behaviour.
//!
//! Two independent nets:
//!
//! 1. [`per_level_xy_is_invariant`] — structural, no pinned constant. Runs
//!    the depth-stepped composition at L = 8 and asserts every level's
//!    ordered XY sequence is bit-identical to level 1's. This is the
//!    invariant the hoist must preserve, stated as a property. It cannot rot
//!    and it needs no re-baselining.
//! 2. [`depth_stepped_fingerprints_match_golden`] — byte-level. Pins
//!    `(move_count, FNV-1a over the move list)` for pocket / profile /
//!    zigzag at L = 1 and L = 8. "Identical by construction" means this hash
//!    does not move; if it does, the hoist changed emission, not just
//!    scheduling.
//!
//! ```text
//! cargo test -p rs_cam_core --test perf_golden_depth_level_geometry
//! UPDATE_PERF_GOLDENS=1 cargo test -p rs_cam_core --test perf_golden_depth_level_geometry
//! ```
//!
//! The fingerprint algorithm is the repo's one canonical FNV-1a-over-`Debug`
//! hash, included from `tests/common/fingerprint.rs` rather than re-rolled —
//! `Debug` on `f64` round-trips every bit, so this is a byte-level identity
//! check on the whole move list, not a lossy summary.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

#[path = "common/fingerprint.rs"]
mod fingerprint;

use std::collections::BTreeMap;
use std::path::PathBuf;

use rs_cam_core::depth::toolpath_at_levels;
use rs_cam_core::geo::P2;
use rs_cam_core::pocket::{PocketParams, pocket_toolpath};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::profile::{ProfileParams, ProfileSide, profile_toolpath};
use rs_cam_core::toolpath::Toolpath;
use rs_cam_core::zigzag::{ZigzagParams, zigzag_toolpath};
use serde::{Deserialize, Serialize};

use fingerprint::move_fingerprint;

/// Marching-squares-scale vertex density — the ring size G2 and G4 are both
/// written against ("rings ~1400 verts at 0.5 mm cell").
const RING_VERTS: usize = 1400;
const RING_RADIUS: f64 = 40.0;
const SAFE_Z: f64 = 10.0;
const TOOL_RADIUS: f64 = 3.0;
const STEPOVER: f64 = 4.0;
const LEVEL_COUNT: usize = 8;
const LEVEL_STEP: f64 = 1.5;

/// Deterministic jittered ring — a fixed harmonic sum, no RNG, so every run
/// of this test measures the same geometry.
fn jittered_ring() -> Polygon2 {
    Polygon2::new(
        (0..RING_VERTS)
            .map(|i| {
                let t = std::f64::consts::TAU * i as f64 / RING_VERTS as f64;
                let r = RING_RADIUS + 0.6 * (7.0 * t).sin() + 0.25 * (23.0 * t).cos();
                P2::new(r * t.cos(), r * t.sin())
            })
            .collect(),
    )
}

fn levels(n: usize) -> Vec<f64> {
    (1..=n).map(|i| -LEVEL_STEP * i as f64).collect()
}

// ── The three operations under test ─────────────────────────────────────

fn pocket_at(poly: &Polygon2, z: f64) -> Toolpath {
    pocket_toolpath(
        poly,
        &PocketParams {
            tool_radius: TOOL_RADIUS,
            stepover: STEPOVER,
            cut_depth: z,
            feed_rate: 1200.0,
            plunge_rate: 400.0,
            safe_z: SAFE_Z,
            climb: true,
        },
    )
}

fn profile_at(poly: &Polygon2, z: f64) -> Toolpath {
    profile_toolpath(
        poly,
        &ProfileParams {
            tool_radius: TOOL_RADIUS,
            side: ProfileSide::Outside,
            cut_depth: z,
            feed_rate: 1200.0,
            plunge_rate: 400.0,
            safe_z: SAFE_Z,
            climb: true,
            compensate_in_controller: false,
        },
    )
}

fn zigzag_at(poly: &Polygon2, z: f64) -> Toolpath {
    zigzag_toolpath(
        poly,
        &ZigzagParams {
            tool_radius: TOOL_RADIUS,
            stepover: STEPOVER,
            cut_depth: z,
            feed_rate: 1200.0,
            plunge_rate: 400.0,
            safe_z: SAFE_Z,
            angle: 0.0,
        },
    )
}

/// One 2.5D operation as `(name, geometry-at-Z)`. The whole point of G2 is
/// that the second element's only Z-dependent output is the Z stamp.
type LevelOp = (&'static str, fn(&Polygon2, f64) -> Toolpath);

fn ops() -> Vec<LevelOp> {
    vec![
        ("pocket", pocket_at as fn(&Polygon2, f64) -> Toolpath),
        ("profile", profile_at as fn(&Polygon2, f64) -> Toolpath),
        ("zigzag", zigzag_at as fn(&Polygon2, f64) -> Toolpath),
    ]
}

// ── 1. Structural invariance, no pinned constant ────────────────────────

/// Every move whose target Z sits on `level`, in emission order, as raw XY
/// bit patterns. Bits, not `f64`, because this is an identity check: two
/// values that print the same but differ in the last ULP are a real
/// divergence, and `-0.0 == 0.0` would hide one.
fn xy_bits_at(tp: &Toolpath, level: f64) -> Vec<(u64, u64)> {
    tp.moves
        .iter()
        .filter(|m| (m.target.z - level).abs() < 1e-9)
        .map(|m| (m.target.x.to_bits(), m.target.y.to_bits()))
        .collect()
}

#[test]
fn per_level_xy_is_invariant() {
    let poly = jittered_ring();
    let ls = levels(LEVEL_COUNT);

    for (name, op) in ops() {
        let composed = toolpath_at_levels(&ls, SAFE_Z, |z| op(&poly, z));
        assert!(
            !composed.moves.is_empty(),
            "[{name}] emitted no moves — the invariant would be vacuously true"
        );

        let reference = xy_bits_at(&composed, ls[0]);
        assert!(
            !reference.is_empty(),
            "[{name}] level 0 (z = {}) carries no moves — nothing to compare against",
            ls[0]
        );

        for (i, &z) in ls.iter().enumerate().skip(1) {
            let this = xy_bits_at(&composed, z);
            assert_eq!(
                this.len(),
                reference.len(),
                "[{name}] level {i} (z = {z}) emitted {} moves at its own Z, level 0 emitted {} — \
                 the 2D geometry is supposed to be loop-invariant in everything but cut_depth",
                this.len(),
                reference.len(),
            );
            if this != reference {
                let first_diff = this
                    .iter()
                    .zip(reference.iter())
                    .position(|(a, b)| a != b)
                    .unwrap_or(0);
                panic!(
                    "[{name}] level {i} (z = {z}) diverges from level 0 at index {first_diff}: \
                     got ({}, {}), level 0 had ({}, {})",
                    f64::from_bits(this[first_diff].0),
                    f64::from_bits(this[first_diff].1),
                    f64::from_bits(reference[first_diff].0),
                    f64::from_bits(reference[first_diff].1),
                );
            }
        }
    }
}

// ── 2. Byte-level fingerprints ──────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Fingerprint {
    moves: usize,
    hash: String,
}

fn fp(tp: &Toolpath) -> Fingerprint {
    let (moves, hash) = move_fingerprint(tp);
    Fingerprint {
        moves,
        // Hex, so a JSON reader never loses a bit to a float round-trip.
        hash: format!("{hash:016x}"),
    }
}

fn golden_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("perf_golden_depth_fingerprints.json")
}

fn measure() -> BTreeMap<String, Fingerprint> {
    let poly = jittered_ring();
    let mut out = BTreeMap::new();
    for (name, op) in ops() {
        for n in [1_usize, LEVEL_COUNT] {
            let ls = levels(n);
            let tp = toolpath_at_levels(&ls, SAFE_Z, |z| op(&poly, z));
            out.insert(format!("{name}_L{n}"), fp(&tp));
        }
    }
    out
}

#[test]
fn depth_stepped_fingerprints_match_golden() {
    let actual = measure();
    let path = golden_path();

    if std::env::var("UPDATE_PERF_GOLDENS").is_ok_and(|v| v != "0" && !v.is_empty()) {
        let json = serde_json::to_string_pretty(&actual).expect("serialize fingerprints");
        std::fs::create_dir_all(path.parent().expect("fixtures dir has a parent"))
            .expect("create fixtures dir");
        std::fs::write(&path, format!("{json}\n")).expect("write golden");
        eprintln!("UPDATE_PERF_GOLDENS: wrote {}", path.display());
        return;
    }

    let raw = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "golden {} unreadable ({e}); regenerate with UPDATE_PERF_GOLDENS=1 \
             cargo test -p rs_cam_core --test perf_golden_depth_level_geometry",
            path.display()
        )
    });
    let golden: BTreeMap<String, Fingerprint> =
        serde_json::from_str(&raw).expect("parse golden JSON");

    let mut failures = Vec::new();
    for (key, want) in &golden {
        match actual.get(key) {
            None => failures.push(format!("{key}: missing from this run")),
            Some(got) if got != want => failures.push(format!(
                "{key}: got {} moves / hash {}, golden {} moves / hash {}",
                got.moves, got.hash, want.moves, want.hash
            )),
            Some(_) => {}
        }
    }
    for key in actual.keys() {
        if !golden.contains_key(key) {
            failures.push(format!("{key}: new arm not in the golden"));
        }
    }

    assert!(
        failures.is_empty(),
        "depth-stepped emission moved against the Phase 0 golden ({} arm(s)).\n{}\n\n\
         G2's hoist is supposed to be identical by construction. If this fired on \
         a G2 change, the hoist changed emission, not just scheduling.",
        failures.len(),
        failures.join("\n")
    );
}
