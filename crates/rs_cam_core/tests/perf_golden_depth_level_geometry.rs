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
//! **D3 (2026-10-16).** The per-op public wrappers and `toolpath_at_levels`
//! are deleted; production steps Z in `run_layered` since W3. Both nets now
//! run the op through the session door with every dressup off, so they
//! guard the composition that ships. Net 2 was re-blessed once for that
//! move. Measured cause: the session's entry-descent split
//! (`clip_and_split_entries`) puts one `Linking` rapid before an entry
//! plunge. Pocket gains 79 moves per level (one per entry), Zigzag 19 per
//! level (one per entry), Profile 1 in total. The cut moves did not change,
//! and net 1 (the XY of every level) passed before and after the move.
//!
//! The fingerprint algorithm is the repo's one canonical FNV-1a-over-`Debug`
//! hash, included from `tests/common/fingerprint.rs` rather than re-rolled —
//! `Debug` on `f64` round-trips every bit, so this is a byte-level identity
//! check on the whole move list, not a lossy summary.

#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr,
    reason = "test code: a panic is how a test reports a failure; the test prints its measurements for the reader"
)]

mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;

use common::fingerprint::move_fingerprint;
use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::HeightMode;
use rs_cam_core::compute::operation_configs::{PocketConfig, ProfileConfig, ZigzagConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::geo::P2;
use rs_cam_core::ops::profile::ProfileSide;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::toolpath::Toolpath;
use serde::{Deserialize, Serialize};

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
//
// D3: each op runs through the session door (`op_fixtures::generated_op`)
// with every dressup off, so `run_layered` builds the ladder and the link
// between levels. Before D3 this file composed the deleted per-op wrappers
// with the deleted `toolpath_at_levels`; production stopped calling that
// composition in W3. The ladder is the same: `EvenRedistributed` from the
// stock top at Z0 to `-LEVEL_STEP * n` in passes of `LEVEL_STEP`.

/// One op over the ring, `n` levels deep, through the session.
fn session_levels(op: OperationConfig, poly: &Polygon2) -> Toolpath {
    let mut tool = ToolConfig::new_default(ToolId::from_raw(0), ToolType::EndMill);
    tool.diameter = 2.0 * TOOL_RADIUS;
    let half = RING_RADIUS + 20.0;
    let stock = StockConfig {
        x: 2.0 * half,
        y: 2.0 * half,
        z: 20.0,
        origin_x: -half,
        origin_y: -half,
        origin_z: -20.0,
        auto_from_model: false,
        ..StockConfig::default()
    };
    let model = common::session::polygon_model(vec![poly.clone()], "ring");
    common::op_fixtures::generated_op(stock, tool, model, op, |tc| {
        tc.dressups = common::op_fixtures::no_dressups();
        tc.heights.retract_z = HeightMode::Manual(SAFE_Z);
        tc.heights.clearance_z = HeightMode::Manual(SAFE_Z);
    })
    .toolpath
}

fn pocket_levels(poly: &Polygon2, n: usize) -> Toolpath {
    session_levels(
        OperationConfig::Pocket(PocketConfig {
            stepover: STEPOVER,
            floor: rs_cam_core::compute::floor::Floor::top(LEVEL_STEP * n as f64),
            depth_per_pass: LEVEL_STEP,
            feed_rate: 1200.0,
            plunge_rate: 400.0,
            climb: true,
            ..PocketConfig::default()
        }),
        poly,
    )
}

fn profile_levels(poly: &Polygon2, n: usize) -> Toolpath {
    session_levels(
        OperationConfig::Profile(ProfileConfig {
            side: ProfileSide::Outside,
            floor: rs_cam_core::compute::floor::Floor::top(LEVEL_STEP * n as f64),
            depth_per_pass: LEVEL_STEP,
            feed_rate: 1200.0,
            plunge_rate: 400.0,
            climb: true,
            ..ProfileConfig::default()
        }),
        poly,
    )
}

fn zigzag_levels(poly: &Polygon2, n: usize) -> Toolpath {
    session_levels(
        OperationConfig::Zigzag(ZigzagConfig {
            stepover: STEPOVER,
            floor: rs_cam_core::compute::floor::Floor::top(LEVEL_STEP * n as f64),
            depth_per_pass: LEVEL_STEP,
            feed_rate: 1200.0,
            plunge_rate: 400.0,
            angle: 0.0,
            ..ZigzagConfig::default()
        }),
        poly,
    )
}

/// One 2.5D operation as `(name, n-level toolpath)`. The whole point of G2
/// is that the only Z-dependent output of each level is the Z stamp.
type LevelOp = (&'static str, fn(&Polygon2, usize) -> Toolpath);

fn ops() -> Vec<LevelOp> {
    vec![
        ("pocket", pocket_levels as fn(&Polygon2, usize) -> Toolpath),
        (
            "profile",
            profile_levels as fn(&Polygon2, usize) -> Toolpath,
        ),
        ("zigzag", zigzag_levels as fn(&Polygon2, usize) -> Toolpath),
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
        let composed = op(&poly, LEVEL_COUNT);
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
            let tp = op(&poly, n);
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
        "depth-stepped emission moved against the golden ({} arm(s)).\n{}\n\n\
         The golden is the session output (re-blessed in D3). If this fired on a \
         change to `run_layered` or to a session stage, that change moved the \
         emitted motion; name the cause before a re-bless.",
        failures.len(),
        failures.join("\n")
    );
}
