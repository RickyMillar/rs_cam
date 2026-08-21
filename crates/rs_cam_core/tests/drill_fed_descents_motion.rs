//! R-2, asserted against **emitted motion** — the drill cycle that ships
//! must be the drill cycle [`rs_cam_core::drill::fed_descents`] describes.
//!
//! Oracle: `planning/perf_review_2026-08-19/RESEARCH_drill_intent_erasure.md`
//! §1, §4, §7A.
//!
//! # Why this sentry exists
//!
//! `drill.rs` expands a canned cycle **once** (R-2, 2026-08-04):
//! `fed_descents(cycle, bottom_z, retract_z)` returns the fed moves, rooted
//! at the R-plane exactly as a Fanuc G83/G73 is, and both the emitter
//! (`drill_peck_full_retract` / `drill_chip_break`) and the metric
//! (`drill_metrics::build_drill_toolpath_summary`) are built from it.
//!
//! That made the *description* self-consistent. It did not pin the
//! description to what actually ships, and a downstream pass falsifies it:
//! `tsp::optimize_rapid_order` discards every rapid on the way in
//! (`tsp.rs` — *"Rapids between segments are discarded (they will be
//! regenerated)"*) and regenerates them at `safe_z`. The R-plane approach
//! and every peck re-entry are **deleted**, so the tool re-enters the hole
//! on a `G1` feed from full safe-Z. On shipped defaults that is 105 mm of
//! fed distance per hole where the cycle says 17 mm — 21.0 s instead of
//! 3.4 s, and the extra 88 mm is fed *air*.
//!
//! **Nothing reports it.** `DrillToolpathSummary::feed_time_s` is computed
//! from `fed_descents(config)`, never from the emitted moves, so it reads
//! the same number in both arms; drill toolpaths take the analytical
//! simulator branch and produce no `SimulationCutSample`s, hence no
//! per-toolpath runtime row; and `air_cut_high_threshold_pct` is `None`
//! for the drill families by policy. Every instrument agrees with the
//! docstring and none of them can see the motion.
//!
//! So this sentry compares the two things that can disagree: the
//! **stored toolpath's** fed descents against `fed_descents`. Asserting
//! against `drill_summaries` instead would be vacuous — that side reads
//! the config and lies identically in both arms
//! (`feedback_instrument_integrity`; `CLAUDE.md`'s "a gate handed an empty
//! population passes and looks healthy" applied to a metric).
//!
//! # Non-vacuity
//!
//! Every case asserts its expectation is a *multi-descent* schedule
//! (`>= 2` fed descents per hole) and that both holes were found, so the
//! comparison cannot pass on an empty or degenerate population.
//!
//! # Coverage
//!
//! * G83 `Peck` — full retract to the R-plane between pecks.
//! * G73 `ChipBreak` — a small lift between pecks. Worse *in kind* than
//!   G83 under the defect: its whole point is not to leave the hole, and
//!   the rebuild turns every lift into a trip to safe-Z plus a fed return,
//!   so the operator's choice of cycle is inverted (research §4a).
//! * A `retract_amount` larger than the peck depth, which makes G73's
//!   re-entry clearance exceed its bite — the inversion case.
//!
//! ```text
//! cargo test -p rs_cam_core --test drill_fed_descents_motion -- --nocapture
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout,
    clippy::print_stderr
)]

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{
    BoundaryConfig, DressupConfig, HeightsConfig, RestAnalysisConfig, StockSource, effective_safe_z,
};
use rs_cam_core::compute::operation_configs::{DrillConfig, DrillCycleType};
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::drill::FedDescent;
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::geo::P2;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{LoadedModel, ProjectSession, ToolpathConfig};
use rs_cam_core::toolpath::{MoveIntent, MoveType};

// ── Fixture ─────────────────────────────────────────────────────────────

/// Flat stock, `origin_z = -FLAT_Z` so the stock top lands on world Z = 0
/// (`project_2d_stock_z_frame`). Same shape the W6 census uses.
const FLAT_X: f64 = 100.0;
const FLAT_Y: f64 = 80.0;
const FLAT_Z: f64 = 12.0;

/// Two holes, far enough apart that the reorder pass has a real choice.
const HOLE_A: [f64; 2] = [20.0, 20.0];
const HOLE_B: [f64; 2] = [40.0, 20.0];

/// Z tolerance for matching an emitted move target against a computed
/// schedule height. Both sides are produced by the same arithmetic on the
/// same inputs, so this only has to absorb accumulated `f64` noise.
const Z_EPS: f64 = 1e-6;

fn drill_session(cfg: DrillConfig) -> (ProjectSession, DrillConfig) {
    let mut session = ProjectSession::new_empty();
    session.set_stock_config(StockConfig {
        x: FLAT_X,
        y: FLAT_Y,
        z: FLAT_Z,
        origin_x: -10.0,
        origin_y: -10.0,
        origin_z: -FLAT_Z,
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
    tool.name = "drill sentry tool".to_owned();
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
        name: "drill_sentry_rect".to_owned(),
        mesh: None,
        polygons: Some(Arc::new(vec![poly])),
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        path: PathBuf::from("synthetic://drill_sentry_rect.svg"),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    });

    let op = OperationConfig::Drill(cfg.clone());
    let dressups = DressupConfig::for_op(op.op_type());
    session
        .add_toolpath(
            0,
            ToolpathConfig {
                id: ToolpathId(0),
                name: "drill sentry".to_owned(),
                enabled: true,
                operation: op,
                dressups,
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

    (session, cfg)
}

/// The fed descents the STORED toolpath actually performs, grouped by hole
/// in emission order.
///
/// A fed descent is a `MoveType::Linear` carrying `MoveIntent::Drilling`;
/// its `from_z` is wherever the previous move left the tool, whatever kind
/// of move that was. That is the whole point: the defect replaces a rapid
/// approach with fed distance, and only a reading rooted in the *previous
/// move's target* can see it.
fn emitted_descents_by_hole(
    moves: &[rs_cam_core::toolpath::Move],
) -> Vec<([f64; 2], Vec<FedDescent>)> {
    let mut out: Vec<([f64; 2], Vec<FedDescent>)> = Vec::new();
    let mut prev_z: Option<f64> = None;
    for m in moves {
        let is_drill_feed =
            matches!(m.move_type, MoveType::Linear { .. }) && m.intent == MoveIntent::Drilling;
        if is_drill_feed {
            let from_z = prev_z.expect("a drill feed cannot be the first move of a toolpath");
            let xy = [m.target.x, m.target.y];
            let descent = FedDescent {
                from_z,
                to_z: m.target.z,
            };
            match out.last_mut() {
                Some((hole_xy, list))
                    if (hole_xy[0] - xy[0]).abs() < Z_EPS && (hole_xy[1] - xy[1]).abs() < Z_EPS =>
                {
                    list.push(descent);
                }
                _ => out.push((xy, vec![descent])),
            }
        }
        prev_z = Some(m.target.z);
    }
    out
}

fn fmt_descents(list: &[FedDescent]) -> String {
    list.iter()
        .map(|d| format!("{:.3}->{:.3}", d.from_z, d.to_z))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Generate the op, then assert every hole's emitted fed descents equal
/// the schedule `drill::fed_descents` describes.
fn assert_motion_matches_schedule(case: &str, cfg: DrillConfig) {
    let (mut session, cfg) = drill_session(cfg);
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .unwrap_or_else(|e| panic!("{case}: generate failed: {e:?}"));
    let result = session.get_result(0).expect("result");
    let toolpath = result.toolpath();

    // Stock top is world Z = 0 for this fixture; the R-plane is the drill
    // adapter's own `effective_safe_z(cfg.retract_z, stock_top)`
    // (`execute.rs::generate_drill`) and the hole bottom is `top - depth`.
    let stock_top = 0.0;
    let r_plane = effective_safe_z(cfg.retract_z, stock_top);
    let bottom_z = stock_top - cfg.depth;
    let expected = rs_cam_core::drill::fed_descents(cfg.cycle.to_core(&cfg), bottom_z, r_plane);

    println!("── {case}");
    println!("   R-plane {r_plane:.3}  bottom {bottom_z:.3}  moves {}", toolpath.moves.len());
    println!("   schedule ({}): {}", expected.len(), fmt_descents(&expected));

    // Non-vacuity: the case must exercise a real multi-descent cycle, or
    // the comparison below could pass on a degenerate one-bite schedule.
    assert!(
        expected.len() >= 2,
        "{case}: schedule is not a multi-descent cycle ({} descents) — \
         the sentry would be vacuous",
        expected.len()
    );

    let by_hole = emitted_descents_by_hole(&toolpath.moves);
    for (xy, list) in &by_hole {
        println!(
            "   emitted @({:.1},{:.1}) ({}): {}",
            xy[0],
            xy[1],
            list.len(),
            fmt_descents(list)
        );
    }

    assert_eq!(
        by_hole.len(),
        2,
        "{case}: expected two holes' worth of fed descents, found {}",
        by_hole.len()
    );

    for (xy, emitted) in &by_hole {
        let where_ = format!("{case} @({:.1},{:.1})", xy[0], xy[1]);
        assert_eq!(
            emitted.len(),
            expected.len(),
            "{where_}: emitted {} fed descents, the cycle describes {}\n  \
             emitted:  {}\n  schedule: {}",
            emitted.len(),
            expected.len(),
            fmt_descents(emitted),
            fmt_descents(&expected)
        );
        for (i, (got, want)) in emitted.iter().zip(expected.iter()).enumerate() {
            assert!(
                (got.from_z - want.from_z).abs() < Z_EPS && (got.to_z - want.to_z).abs() < Z_EPS,
                "{where_}: fed descent {i} is {:.3}->{:.3}, the cycle describes \
                 {:.3}->{:.3}\n  emitted:  {}\n  schedule: {}",
                got.from_z,
                got.to_z,
                want.from_z,
                want.to_z,
                fmt_descents(emitted),
                fmt_descents(&expected)
            );
        }

        // The headline: total fed distance. Stated separately from the
        // per-descent comparison because it is the number the cycle-time
        // claim is made of, and because a failure message carrying it is
        // immediately actionable.
        let got_mm: f64 = emitted.iter().map(FedDescent::length).sum();
        let want_mm: f64 = expected.iter().map(FedDescent::length).sum();
        assert!(
            (got_mm - want_mm).abs() < 1e-6,
            "{where_}: emitted {got_mm:.3} mm of fed descent, the cycle \
             describes {want_mm:.3} mm ({:.2}x)",
            got_mm / want_mm.max(f64::EPSILON)
        );
    }
}

// ── Cases ───────────────────────────────────────────────────────────────

/// G83, shipped defaults: depth 10, peck 3, `retract_z` 2 (floored to the
/// R-plane at stock_top + 5), feed 300.
#[test]
#[ignore = "red until drill C1 (rebuild_clearance_z) lands — see RESEARCH_drill_intent_erasure.md"]
fn peck_cycle_motion_matches_the_schedule_it_describes() {
    assert_motion_matches_schedule(
        "G83 shipped defaults",
        DrillConfig {
            depth: 10.0,
            cycle: DrillCycleType::Peck,
            peck_depth: 3.0,
            selected_holes: Some(vec![HOLE_A, HOLE_B]),
            ..DrillConfig::default()
        },
    );
}

/// G73, shipped defaults: the small lift between pecks is the cycle's
/// entire purpose, so a pass that re-plants it at safe-Z inverts the
/// operator's choice (research §4a).
#[test]
#[ignore = "red until drill C1 (rebuild_clearance_z) lands — see RESEARCH_drill_intent_erasure.md"]
fn chip_break_cycle_motion_matches_the_schedule_it_describes() {
    assert_motion_matches_schedule(
        "G73 shipped defaults",
        DrillConfig {
            depth: 10.0,
            cycle: DrillCycleType::ChipBreak,
            peck_depth: 3.0,
            retract_amount: 0.5,
            selected_holes: Some(vec![HOLE_A, HOLE_B]),
            ..DrillConfig::default()
        },
    );
}

/// G73 with `retract_amount > peck_depth`: the re-entry clearance exceeds
/// the bite, so each descent starts *above* where the previous one ended
/// and the schedule's `from_z` values are not monotone with its `to_z`
/// values. A reading that assumed "re-entry = previous depth + a small
/// constant" gets this backwards; `fed_descents` is the only authority.
#[test]
#[ignore = "red until drill C1 (rebuild_clearance_z) lands — see RESEARCH_drill_intent_erasure.md"]
fn chip_break_with_retract_larger_than_peck_matches_the_schedule() {
    assert_motion_matches_schedule(
        "G73 retract 4.0 > peck 2.0",
        DrillConfig {
            depth: 10.0,
            cycle: DrillCycleType::ChipBreak,
            peck_depth: 2.0,
            retract_amount: 4.0,
            selected_holes: Some(vec![HOLE_A, HOLE_B]),
            ..DrillConfig::default()
        },
    );
}
