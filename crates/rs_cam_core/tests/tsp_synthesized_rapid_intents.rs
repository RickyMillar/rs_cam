//! The rapids `tsp::rebuild_group` SYNTHESIZES must say what they are —
//! and must never claim to be drilling.
//!
//! # Why
//!
//! `rebuild_group` planted its interstitial rapids through
//! `Toolpath::rapid_to`, which is
//! `rapid_to_with_intent(target, MoveIntent::Unknown)`. Every rapid that
//! survived the reorder came out untagged: 202 of them across nine
//! families on one flat fixture (pocket 32, zigzag 52, face 58, adaptive
//! 16, the three drill cycles 44). That reached transit classification for
//! gate populations — `toolpath_spans` falls back to the per-move intent
//! union when a span is dropped — and it is why the W6 retract census read
//! zero Retract-tagged moves out of a drill toolpath that emits ten.
//!
//! # The second bar, and why it is the load-bearing one
//!
//! `compute::execute::apply_dressups` strips the entry dressup from any
//! toolpath carrying a `MoveIntent::Drilling` move (G-WANAKA-DRILL-RAMP: a
//! ramped drill hole is an oval slot), and `session::compute` uses the same
//! predicate. Both read EMITTED MOTION, not the op type. So a pass that
//! synthesized a `Drilling`-tagged move on a milling toolpath would
//! silently delete that op's legitimate ramp/helix entries — a plunge where
//! the user asked for a ramp, reported by nothing.
//!
//! Copying an intent out of surrounding drill context is safe (the moves
//! were already drilling). DERIVING `Drilling` is not. This file pins the
//! difference: no non-drill toolpath may come out of the reorder carrying a
//! `Drilling` move.
//!
//! ```text
//! cargo test -p rs_cam_core --test tsp_synthesized_rapid_intents
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{
    BoundaryConfig, DressupConfig, HeightsConfig, RestAnalysisConfig, StockSource,
};
use rs_cam_core::compute::operation_configs::{
    AdaptiveConfig, PocketConfig, PocketPattern, ProfileConfig,
};
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::geo::{P2, P3};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::profile::ProfileSide;
use rs_cam_core::session::{LoadedModel, ProjectSession, ToolpathConfig};
use rs_cam_core::toolpath::{MoveIntent, MoveType, Toolpath};
use rs_cam_core::toolpath_spans::AnnotatedToolpath;

// ── Unit level: the pass itself ─────────────────────────────────────────

/// Two cutting segments at distinct XY, framed by safe-Z rapids — the
/// smallest input that reaches `rebuild_group` (`optimize_one_group` runs
/// verbatim fast paths at zero and one segment).
fn two_segment_toolpath() -> Toolpath {
    let safe_z = 10.0;
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, safe_z));
    tp.feed_to(P3::new(0.0, 0.0, -1.0), 500.0);
    tp.feed_to(P3::new(5.0, 0.0, -1.0), 1000.0);
    tp.rapid_to(P3::new(5.0, 0.0, safe_z));
    tp.rapid_to(P3::new(20.0, 0.0, safe_z));
    tp.feed_to(P3::new(20.0, 0.0, -1.0), 500.0);
    tp.feed_to(P3::new(25.0, 0.0, -1.0), 1000.0);
    tp.rapid_to(P3::new(25.0, 0.0, safe_z));
    tp
}

#[test]
fn synthesized_rapids_are_tagged_retract_or_linking() {
    let out = rs_cam_core::tsp::optimize_rapid_order(
        AnnotatedToolpath::new(two_segment_toolpath()),
        10.0,
    )
    .toolpath;

    let rapids: Vec<MoveIntent> = out
        .moves
        .iter()
        .filter(|m| matches!(m.move_type, MoveType::Rapid))
        .map(|m| m.intent)
        .collect();

    // Non-vacuity: the pass must actually have synthesized rapids, or the
    // "none is Unknown" bar below passes on an empty population.
    assert!(
        rapids.len() >= 4,
        "expected the rebuild to synthesize framing rapids, saw {}",
        rapids.len()
    );
    assert!(
        rapids
            .iter()
            .all(|i| matches!(i, MoveIntent::Retract | MoveIntent::Linking)),
        "every synthesized rapid must be Retract or Linking, got {rapids:?}"
    );
    assert!(
        rapids.contains(&MoveIntent::Retract) && rapids.contains(&MoveIntent::Linking),
        "both roles must be represented — a pass that tagged everything the \
         same way would satisfy the bar above without saying anything, {rapids:?}"
    );
}

#[test]
fn the_reorder_never_invents_a_drilling_move() {
    let input = two_segment_toolpath();
    assert!(
        !input.moves.iter().any(|m| m.intent == MoveIntent::Drilling),
        "precondition: the input carries no Drilling move"
    );

    let out = rs_cam_core::tsp::optimize_rapid_order(AnnotatedToolpath::new(input), 10.0).toolpath;

    assert!(
        !out.moves.iter().any(|m| m.intent == MoveIntent::Drilling),
        "the reorder tagged a move Drilling on a toolpath that had none — \
         `apply_dressups` and `session::compute` both treat that as \"this is \
         a drill cycle\" and strip the entry dressup, so this would delete a \
         milling op's ramp entry with no report"
    );
}

// ── Integration level: real generated families ──────────────────────────

fn flat_session(op: OperationConfig) -> ProjectSession {
    let mut session = ProjectSession::new_empty();
    session.set_stock_config(StockConfig {
        x: 100.0,
        y: 80.0,
        z: 12.0,
        origin_x: -10.0,
        origin_y: -10.0,
        origin_z: -12.0,
        auto_from_model: false,
        ..StockConfig::default()
    });
    let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.cutting_length = 25.0;
    tool.shank_diameter = 6.35;
    tool.shank_length = 20.0;
    tool.stickout = 45.0;
    tool.flute_count = 2;
    let tool_idx = session.add_tool(tool);
    let tool_id = session.tools()[tool_idx].id.0;

    let poly = Polygon2::new(vec![
        P2::new(5.0, 5.0),
        P2::new(75.0, 5.0),
        P2::new(75.0, 55.0),
        P2::new(5.0, 55.0),
    ]);
    let model_id = session.add_model(LoadedModel {
        id: 0,
        name: "tsp_intent_rect".to_owned(),
        mesh: None,
        polygons: Some(Arc::new(vec![poly])),
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        path: PathBuf::from("synthetic://tsp_intent_rect.svg"),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    });

    let op_type = op.op_type();
    session
        .add_toolpath(
            0,
            ToolpathConfig {
                id: ToolpathId(0),
                name: format!("{op_type:?}"),
                enabled: true,
                operation: op,
                dressups: DressupConfig::for_op(op_type),
                heights: HeightsConfig::default(),
                tool_id,
                model_id,
                pre_gcode: None,
                post_gcode: None,
                boundary: BoundaryConfig::default(),
                boundary_inherit: true,
                stock_source: StockSource::default(),
                coolant: CoolantMode::Off,
                face_selection: None,
                debug_options: ToolpathDebugOptions::default(),
                feeds_provenance: rs_cam_core::feeds::FeedsProvenance::default(),
                rest_analysis: RestAnalysisConfig::default(),
            },
        )
        .expect("add toolpath");
    session
}

/// The bar the entry-strip predicate actually depends on, asserted on
/// motion generated through the production entry point rather than on a
/// hand-built toolpath: a milling family must not come out of the dressup
/// pipeline looking like a drill cycle.
#[test]
fn no_milling_family_emits_a_drilling_tagged_move() {
    let cases: Vec<(&str, OperationConfig)> = vec![
        (
            "pocket",
            OperationConfig::Pocket(PocketConfig {
                stepover: 3.0,
                depth: 6.0,
                depth_per_pass: 3.0,
                feed_rate: 1000.0,
                plunge_rate: 400.0,
                climb: true,
                pattern: PocketPattern::Contour,
                angle: 0.0,
                finishing_passes: 0,
                spindle_rpm: Some(18_000),
            }),
        ),
        (
            "adaptive",
            OperationConfig::Adaptive(AdaptiveConfig {
                stepover: 2.0,
                depth: 6.0,
                depth_per_pass: 3.0,
                feed_rate: 1500.0,
                plunge_rate: 500.0,
                spindle_rpm: Some(18_000),
                ..AdaptiveConfig::default()
            }),
        ),
        (
            "profile",
            OperationConfig::Profile(ProfileConfig {
                side: ProfileSide::Outside,
                depth: 6.0,
                depth_per_pass: 3.0,
                feed_rate: 1000.0,
                plunge_rate: 400.0,
                climb: true,
                tab_count: 0,
                tab_width: 6.0,
                tab_height: 2.0,
                finishing_passes: 0,
                compensation: rs_cam_core::compute::operation_configs::CompensationType::InComputer,
                spindle_rpm: Some(18_000),
            }),
        ),
    ];

    let mut checked_rapids = 0usize;
    for (label, op) in cases {
        let mut session = flat_session(op);
        let cancel = AtomicBool::new(false);
        session
            .generate_toolpath(0, &cancel)
            .unwrap_or_else(|e| panic!("{label}: generate failed: {e:?}"));
        let tp = session.get_result(0).expect("result").toolpath();

        let drilling: Vec<usize> = tp
            .moves
            .iter()
            .enumerate()
            .filter(|(_, m)| m.intent == MoveIntent::Drilling)
            .map(|(i, _)| i)
            .collect();
        assert!(
            drilling.is_empty(),
            "{label} emitted {} Drilling-tagged move(s) at {:?} — \
             `apply_dressups` would read that as a drill cycle and strip the \
             op's entry dressup",
            drilling.len(),
            &drilling[..drilling.len().min(8)]
        );

        let rapids = tp
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, MoveType::Rapid))
            .count();
        let unknown = tp
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, MoveType::Rapid) && m.intent == MoveIntent::Unknown)
            .count();
        println!(
            "{label}: {} moves, {rapids} rapids, {unknown} untagged",
            tp.moves.len()
        );
        assert_eq!(
            unknown, 0,
            "{label}: {unknown} of {rapids} rapids came out of the pipeline \
             untagged"
        );
        checked_rapids += rapids;
    }

    // Non-vacuity: "no Drilling move" and "no Unknown rapid" both pass
    // trivially on a toolpath with no rapids at all.
    assert!(
        checked_rapids >= 20,
        "the census covered only {checked_rapids} rapids — too few to be evidence"
    );
}
