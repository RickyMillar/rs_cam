//! adaptive3d honours its commanded `depth_per_pass` — a **sim-free** ladder
//! sentry.
//!
//! # Why this is its own file and not another F-027 / F-031 bar
//!
//! `DELTA_sim_w5b_landing.md`'s W5B-F2 follow-up proposed re-expressing the
//! F-027 / F-031 axial bars against "the tool's commanded Z travel per pass — a
//! quantity still bounded by `dpp` under both kernels". It is indeed
//! kernel-invariant, and that is exactly why it cannot replace those bars:
//!
//! * **F-027's** defect was a planner *grid* too small in XY. Cells in the band
//!   `(mesh.max.y, stock.max.y]` were never stamped by the planner, the
//!   simulator carried them virgin, and its first stamp cleared the whole ray →
//!   30–47 mm readings. The commanded ladder was a uniform 3.0 mm step before
//!   the fix and after it.
//! * **F-031's** defect was 282 *transit* samples at up to 44.8 mm from a
//!   planner↔dressup helix gap. Same conclusion: the commanded ladder did not
//!   move.
//!
//! A bar of the form `max commanded step ≤ dpp + ε` therefore reads **green on
//! both pre-fix defects**. Adopting it under their names would delete two
//! regression nets while looking like a strengthening. It is a genuinely
//! useful, genuinely *different* sentry — nothing in the suite asserts that
//! adaptive3d's emitted Z ladder honours its commanded `depth_per_pass`, and a
//! planner regression that doubled a step would today only be visible *through*
//! the simulator's axial reading, i.e. confounded with the stamp kernel. So it
//! lives here, under its own name, and the F-027 / F-031 bars keep theirs.
//!
//! # Cost
//!
//! Generate only — no simulation. The ladder is read from the annotated
//! toolpath's `SpanKind::DepthPass` spans (`compute::spans::push_adaptive3d_spans`).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

mod common;
use common::repo_root;
use common::zladder::commanded_z_ladder;

use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::{BoundaryConfig, DressupConfig, HeightsConfig, StockSource};
use rs_cam_core::compute::operation_configs::{
    Adaptive3dConfig, Adaptive3dEntryStyle, ClearingStrategy, RegionOrdering,
};
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::session::{ProjectSession, ToolpathConfig};

/// The AS013 fixture: `ux_3d_terrain.toml` + an adaptive3d op with the round-05
/// baseline params. Deliberately the same fixture the F-027 / F-031 sentries
/// build, so this bar and theirs describe one toolpath.
fn build_as013_terrain_session() -> ProjectSession {
    let toml_path = repo_root().join("test_data/ux_3d_terrain.toml");
    let mut session = ProjectSession::load(&toml_path).expect("load ux_3d_terrain");

    let tool_id = session
        .tools()
        .iter()
        .find(|t| (t.diameter - 6.0).abs() < 1e-6)
        .map(|t| t.id.0)
        .expect("ux_3d_terrain.toml must define a 6 mm end mill");
    let model_id = session
        .models()
        .iter()
        .find(|m| m.mesh.is_some())
        .map(|m| m.id)
        .expect("ux_3d_terrain.toml must load terrain_small.stl");

    let adaptive3d = Adaptive3dConfig {
        trochoid_cap_mult: 1.6,
        engagement_measure: rs_cam_core::adaptive::EngagementMeasure::DiskArea,
        stepover: 1.2,
        depth_per_pass: 3.0,
        stock_to_leave_axial: 0.5,
        stock_to_leave_radial: 0.5,
        feed_rate: 2500.0,
        plunge_rate: 500.0,
        tolerance: 0.25,
        min_cutting_radius: 0.0,
        entry_style: Adaptive3dEntryStyle::Plunge,
        ramp_angle_deg: 3.0,
        helix_radius_factor: 0.4,
        helix_pitch: 1.0,
        fine_stepdown: 0.0,
        detect_flat_areas: false,
        region_ordering: RegionOrdering::Global,
        clearing_strategy: ClearingStrategy::ContourParallel,
        z_blend: false,
        mill_shallow_areas: false,
        shallow_angle_deg: None,
        shallow_stepdown: None,
        spindle_rpm: Some(18_000),
        min_region_cut_length_mm: 0.0,
        max_stay_down_distance_mm: Some(0.0),
        stay_down_clearance_mm: 0.5,
    };

    let tc = ToolpathConfig {
        id: ToolpathId(0),
        name: "AS013 adaptive3d".to_owned(),
        enabled: true,
        operation: OperationConfig::Adaptive3d(adaptive3d),
        dressups: DressupConfig::for_op(OperationType::Adaptive3d),
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
        rest_analysis: rs_cam_core::compute::config::RestAnalysisConfig::default(),
    };
    session
        .add_toolpath(0, tc)
        .expect("add adaptive3d toolpath");
    session
}

/// The commanded step between consecutive Z levels never exceeds the configured
/// `depth_per_pass`.
///
/// `depth_per_pass` is read back **from the toolpath's own config**, not written
/// as a literal — the three F-027 / F-031 bars carry a hard-coded `3.0` that
/// would not follow a config change, and repeating that mistake in the one test
/// whose entire subject is the commanded value would be perverse.
///
/// Non-vacuity is asserted three ways before the bar: `spans_valid` (via
/// `commanded_z_ladder`, which panics rather than returning an empty ladder),
/// at least two rungs (one rung means no step exists to measure), and no
/// `pass_index` carrying two different `z_level`s (which would mean the ladder
/// is per-region and a single step per index is ill-defined).
///
/// # The ladder this fixture actually emits (measured 2026-08-21)
///
/// ```text
/// commanded Z ladder: 20 rung(s) from 20 DepthPass span(s) (19 WaterlineCleanup excluded)
///   pass  1  z = 54.5652   step = (none — first rung, no predecessor)
///   pass  2  z = 51.5652   step = 3.0000
///   …                       …
///   pass 19  z =  0.5652   step = 3.0000
///   pass 20  z =  0.5000   step = 0.0652   <-- SHORT FINAL PASS
/// ```
///
/// Two facts worth carrying forward, because both contradict what
/// `RESEARCH_f2_and_aba.md` §A.2 assumed when it proposed re-expressing the
/// F-027 / F-031 axial bars as ratios of the *local* commanded step:
///
/// * **The ladder is NOT uniform.** The last rung lands on the
///   `stock_to_leave` floor and steps 0.0652 mm. A per-pass ratio bar divides
///   the *measured removal* by that, and on the F-031 population that produces
///   a max ratio of **8.6994** against a proposed 1.35 ceiling — while the
///   unchanged numerator's absolute maximum, 3.8904 mm, sits comfortably inside
///   the shipped 4.000 mm bar. The re-expression is not a tightening, it is a
///   detonation, and it was stopped on this evidence.
/// * **`pass_index` is 1-based**, so an abstention rule written as
///   `pass_index == 0` never fires: 0 of 630 865 samples were in pass 0, and
///   the rung with no predecessor is pass 1 (18 764 samples).
///
/// Neither fact affects THIS bar — `max_step` ignores the first rung and a
/// short final pass is smaller than `dpp`, not larger — which is part of why
/// this sentry is worth having on its own.
#[test]
fn adaptive3d_emitted_z_ladder_honours_commanded_depth_per_pass() {
    let mut session = build_as013_terrain_session();
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("generate adaptive3d toolpath");

    let commanded_dpp = match &session.toolpath_configs()[0].operation {
        OperationConfig::Adaptive3d(cfg) => cfg.depth_per_pass,
        other => panic!("fixture drift: toolpath 0 is not adaptive3d, got {other:?}"),
    };

    let result = session
        .get_result(0)
        .expect("compute result for toolpath 0");
    let annotated = result.annotated();
    assert!(
        annotated.spans_valid,
        "the AS013 adaptive3d span table did not survive the dressup / TSP / arc-fit chain \
         (`spans_valid == false`). Every ladder- or pass-joined bar in this suite is vacuous \
         when that happens, so it is asserted here explicitly rather than inferred."
    );

    let ladder = commanded_z_ladder(annotated);
    eprintln!("{}", ladder.describe());

    assert!(
        ladder.passes.len() >= 2,
        "commanded ladder has {} rung(s); at least 2 are needed for a step to exist. \
         adaptive3d on this terrain fixture cuts many Z levels — 1 or 0 means the DepthPass \
         spans stopped being emitted, not that the geometry changed.",
        ladder.passes.len()
    );
    assert!(
        ladder.conflicts.is_empty(),
        "commanded ladder is ambiguous: {:?} — a `pass_index` carrying more than one `z_level` \
         means the ladder is per-region (level_index restarts inside each region) and a single \
         step per index is ill-defined. This fixture runs `RegionOrdering::Global`.",
        ladder.conflicts
    );

    // Reported, not gated: a non-uniform ladder is legitimate (this fixture's
    // last rung lands on the `stock_to_leave` floor), but it is exactly what
    // makes a per-pass RATIO bar unsound, so any future lane reading this
    // output sees it stated rather than having to re-measure.
    eprintln!(
        "ladder uniform within 1e-6: {} (min step {:?}, max step {:?}) — a `false` here means \
         a per-pass ratio bar built on this ladder would divide measured removal by a short \
         final step; see this file's doc comment.",
        ladder.is_uniform(1e-6),
        ladder.min_step(),
        ladder.max_step(),
    );

    let max_step = ladder.max_step().expect("at least one step");
    assert!(
        max_step <= commanded_dpp + 1e-6,
        "adaptive3d emitted a commanded Z step of {max_step:.4} mm against a configured \
         depth_per_pass of {commanded_dpp:.4} mm.\n{}",
        ladder.describe()
    );
}
