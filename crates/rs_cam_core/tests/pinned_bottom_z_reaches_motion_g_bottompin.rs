//! G-BOTTOMPIN (F1.19) — which operations let a pinned Bottom Z reach their
//! emitted motion, proved against the motion rather than asserted.
//!
//! The finding: a Heights-tab Bottom Z pinned below the stock raises a
//! caution on the 2.5D family, but those generators never read the pin, so the
//! caution describes a cut the machine does not make.
//!
//! The mechanism is one parameter list. `OperationConfig::cutting_levels`
//! takes `top_z` and nothing else, and floors the ladder at
//! `top_z - cfg.depth.abs()`. `session/compute.rs` calls it BEFORE generation
//! and hands the result to `execute_operation`. `execute.rs`'s
//! `effective_levels` returns that ladder whenever it is non-empty:
//!
//! ```text
//! if !cutting_levels.is_empty() { cutting_levels.to_vec() }
//! else { DepthStepping::new(top_z, top_z - heights.depth(), dpp).all_levels() }
//! ```
//!
//! The `else` branch is the ONLY place a pinned bottom could reach a 2.5D
//! ladder, through `ResolvedHeights::depth()`. Six generators call
//! `effective_levels` — rest, zigzag, trace, profile, pocket, adaptive — and
//! `cutting_levels` returns a non-empty ladder for every one of them, so that
//! branch never runs in production.
//!
//! This file proves three separate things, because asserting the declaration
//! against itself would prove nothing:
//!
//! 1. A pocket's emitted floor does not move when the pin moves 14 mm.
//! 2. The branch the pin WOULD reach exists and does honour the pin — so the
//!    claim is about which caller wins, not about dead code.
//! 3. Every operation whose `cutting_levels` ladder is non-empty declares
//!    `honors_pinned_bottom_z() == false`, so the declaration cannot drift
//!    away from the ladder that decides.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::ResolvedHeights;
use rs_cam_core::compute::cutter::build_cutter;
use rs_cam_core::compute::execute::execute_operation;
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::geo::{BoundingBox3, P2, P3};
use rs_cam_core::polygon::Polygon2;

const STOCK_TOP_Z: f64 = 0.0;
const STOCK_BOTTOM_Z: f64 = -18.0;
const POCKET_DEPTH_MM: f64 = 6.0;
/// Pinned 14 mm below the floor the operation's own depth dial asks for.
const PINNED_BOTTOM_Z: f64 = -20.0;

fn flat_endmill() -> ToolConfig {
    ToolConfig {
        diameter: 6.0,
        ..ToolConfig::new_default(ToolId(1), ToolType::EndMill)
    }
}

fn square_40mm() -> Vec<Polygon2> {
    vec![Polygon2::new(vec![
        P2::new(0.0, 0.0),
        P2::new(40.0, 0.0),
        P2::new(40.0, 40.0),
        P2::new(0.0, 40.0),
    ])]
}

fn pocket_op() -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        depth: POCKET_DEPTH_MM,
        depth_per_pass: 2.0,
        ..PocketConfig::default()
    })
}

/// The heights the session resolves. `bottom_z` is the only field that moves
/// between the two arms.
fn heights(bottom_z: f64, bottom_pinned: bool) -> ResolvedHeights {
    ResolvedHeights {
        clearance_z: 15.0,
        retract_z: 10.0,
        feed_z: 1.0,
        top_z: STOCK_TOP_Z,
        bottom_z,
        top_pinned: false,
        bottom_pinned,
    }
}

fn stock_bbox() -> BoundingBox3 {
    BoundingBox3 {
        min: P3::new(-5.0, -5.0, STOCK_BOTTOM_Z),
        max: P3::new(45.0, 45.0, STOCK_TOP_Z),
    }
}

/// Generate a pocket and return the deepest Z any move reaches.
fn emitted_floor_z(h: &ResolvedHeights, cutting_levels: &[f64]) -> f64 {
    let op = pocket_op();
    let tool_cfg = flat_endmill();
    let tool_def = build_cutter(&tool_cfg);
    let polygons = square_40mm();
    let cancel = AtomicBool::new(false);

    let tp = execute_operation(
        &op,
        None,
        None,
        Some(polygons.as_slice()),
        &tool_def,
        &tool_cfg,
        h,
        cutting_levels,
        &stock_bbox(),
        None,
        None,
        None,
        &cancel,
        None,
    )
    .expect("a 40 mm square pocket with a 6 mm end mill must generate");

    let floor = tp
        .moves
        .iter()
        .map(|m| m.target.z)
        .fold(f64::INFINITY, f64::min);
    assert!(
        floor.is_finite(),
        "the pocket emitted no moves, so this measures nothing"
    );
    floor
}

/// Arm 1 — the acceptance. Pin Bottom Z 14 mm deeper than the depth dial asks
/// for and the emitted floor does not move.
#[test]
fn a_pinned_bottom_z_does_not_move_a_pocket_floor() {
    let op = pocket_op();
    // This is exactly what `session/compute.rs:1199` does: the ladder comes
    // from the TOP and the operation's own depth, with no bottom in scope.
    let ladder = op.cutting_levels(STOCK_TOP_Z);
    assert!(
        !ladder.is_empty(),
        "a non-empty ladder is the precondition for this whole finding"
    );

    let auto = emitted_floor_z(&heights(STOCK_TOP_Z - POCKET_DEPTH_MM, false), &ladder);
    let pinned = emitted_floor_z(&heights(PINNED_BOTTOM_Z, true), &ladder);

    eprintln!(
        "G-BOTTOMPIN: auto bottom {:.3} -> floor {auto:.3}; pinned bottom {:.3} -> floor {pinned:.3}",
        STOCK_TOP_Z - POCKET_DEPTH_MM,
        PINNED_BOTTOM_Z
    );

    assert!(
        (auto - pinned).abs() < 1e-9,
        "the pin moved the emitted floor ({auto} -> {pinned}); if this fires, \
         a generator started reading the pin and `honors_pinned_bottom_z` is \
         now wrong for Pocket"
    );
    assert!(
        (auto - (STOCK_TOP_Z - POCKET_DEPTH_MM)).abs() < 1e-6,
        "the floor must be the depth dial's own value, not {auto}"
    );
    assert!(
        pinned > PINNED_BOTTOM_Z + 1.0,
        "the emitted floor {pinned} must stay well clear of the pinned \
         {PINNED_BOTTOM_Z}"
    );
    assert!(
        !OperationType::Pocket.honors_pinned_bottom_z(),
        "the declaration must agree with the motion just measured"
    );
}

/// Arm 2 — the branch the pin would reach is not dead code. Hand
/// `execute_operation` an EMPTY ladder, which is what the `else` arm of
/// `effective_levels` waits for, and the pin drives the floor.
///
/// Production never does this for a pocket, which is the whole point: the
/// inertness lives in the CALLER, so a future caller that stops pre-computing
/// the ladder would silently change emitted depth.
#[test]
fn the_fallback_branch_does_honour_the_pin() {
    let pinned = emitted_floor_z(&heights(PINNED_BOTTOM_Z, true), &[]);
    eprintln!("G-BOTTOMPIN: empty ladder, pinned bottom -> floor {pinned:.3}");
    assert!(
        pinned < STOCK_TOP_Z - POCKET_DEPTH_MM - 1.0,
        "with no pre-computed ladder the pin must deepen the cut; got {pinned}"
    );
}

/// Arm 3 — the declaration is tied to the ladder, not to this file's opinion.
#[test]
fn no_operation_with_a_pre_computed_ladder_claims_to_honour_the_pin() {
    let mut with_ladder = Vec::new();
    for &op_type in OperationType::ALL {
        let op = OperationConfig::new_default(op_type);
        if op.cutting_levels(STOCK_TOP_Z).is_empty() {
            continue;
        }
        with_ladder.push(op_type);
        assert!(
            !op_type.honors_pinned_bottom_z(),
            "{op_type:?} hands `execute_operation` a non-empty ladder, so \
             `effective_levels` returns it and the pin cannot reach the cut — \
             the declaration must say false"
        );
    }
    eprintln!(
        "G-BOTTOMPIN: {} operations ship a pre-computed ladder: {with_ladder:?}",
        with_ladder.len()
    );
    assert!(
        with_ladder.len() >= 7,
        "expected at least the seven depth-stepping operations, measured {}",
        with_ladder.len()
    );
}

/// Arm 4 — the three that DO honour the pin, pinned by name so that adding a
/// fourth is a deliberate act with a report behind it.
#[test]
fn exactly_three_operations_honour_a_pinned_bottom_z() {
    let honouring: Vec<OperationType> = OperationType::ALL
        .iter()
        .copied()
        .filter(|t| t.honors_pinned_bottom_z())
        .collect();
    let mut names: Vec<String> = honouring.iter().map(|t| format!("{t:?}")).collect();
    names.sort();
    eprintln!("G-BOTTOMPIN: honouring = {names:?}");
    assert_eq!(
        names,
        vec![
            "Adaptive3d".to_owned(),
            "UnifiedFinish".to_owned(),
            "Waterline".to_owned()
        ],
        "the three sites that read `heights.bottom_z` are in \
         `compute/execute.rs`; change this list only with the new site named"
    );
}
