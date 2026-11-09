//! Integration step 7.3 (`planning/adaptive_research_2026-10-23/integration/
//! step7_design.md` §2, §5): the plan of the product preset is pinned to the
//! research reference.
//!
//! The product preset is C35 plan A (INTEGRATION_PLAN Q8, D52): the
//! research preset `C35_SPIRAL_PICK` with the replay pick off and the default
//! arc ends. Step 7.3d deleted the research options; `Candidate2d::Geodesic`
//! of the harness door now plans that preset only. The research module (`research_geodesic.rs`, ported
//! from the research branch at 607e52730, the same file as at 453d52571)
//! plans the two development shapes of the 2D harness
//! (`adaptive_geodesic_2d_renders_g1`) with those options, through the 2D
//! harness door `run_2d`. The pins below are that reference plan: the move
//! count and the FNV-1a hash (`common::fingerprint::fnv1a_debug`) of each
//! move's target, move type and intent. The hash leaves out
//! `Move::feed_cap`: on the research branch `Move` derives `Debug` and
//! prints `feed_cap: None`, on master it prints no cap (integration step 1),
//! so the whole-move `Debug` text differs between the two trees for the same
//! plan. The pins were checked on both trees: the research branch at
//! 607e52730 and this branch give the same numbers.
//!
//! The product door `adaptive::geodesic::geodesic_toolpath` (step 7.3b) must
//! give the same plan, bit for bit, less one research instrument: `run_2d`
//! lowers the feed of each planned turn's moves from its own readout
//! (`apply_turn_feeds`), which the door does not (the feed factor on the cut
//! replaces it, design part F). That instrument changes only the feed of
//! linear moves, so the door's plan is compared with the feed values left
//! out. A pin that moves is a port defect: report it, do not re-pin.

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use common::fingerprint::fnv1a_debug;
use rs_cam_core::adaptive::geodesic::geodesic_toolpath;
use rs_cam_core::adaptive::research_geodesic::{Candidate2d, run_2d};
use rs_cam_core::adaptive::{AdaptiveParams, KeepDownLinks, PathStrategy2d};
use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::config::{DressupConfig, DressupEntryStyle};
use rs_cam_core::compute::fixture_values::{fixture_feeds, fixture_geometry};
use rs_cam_core::compute::operation_configs::AdaptiveConfig;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::ProjectSession;
use rs_cam_core::tool::FlatEndmill;
use rs_cam_core::toolpath::Toolpath;

/// The fixture end mill of the 2D harness.
const TOOL_DIAMETER_MM: f64 = 6.0;

/// The largest closed region of the fixture's drawing and its end mill (as
/// `adaptive_geodesic_2d_renders_g1::fixture`).
fn fixture(project: &str) -> (Polygon2, FlatEndmill) {
    let path = common::repo_root().join(project);
    let session = ProjectSession::load(common::require_fixture(&path)).expect("load the fixture");
    let outline = session
        .models()
        .iter()
        .flat_map(|m| m.items.closed_regions().to_vec())
        .max_by(|a, b| a.area().total_cmp(&b.area()))
        .expect("the fixture draws a closed region");
    let tool = session
        .tools()
        .iter()
        .find(|t| (t.diameter - TOOL_DIAMETER_MM).abs() < 1e-9)
        .expect("the fixture holds the 6 mm end mill");
    (
        outline,
        FlatEndmill::new(tool.diameter, tool.cutting_length_planning_mm()),
    )
}

/// The shared params of the 2D harness (`adaptive_geodesic_2d_renders_g1::params`).
fn params(tool: &FlatEndmill) -> AdaptiveParams {
    let cfg = AdaptiveConfig::default();
    let dressups = DressupConfig::for_op(OperationType::Adaptive);
    let entry_helix_radius = if dressups.entry_style == DressupEntryStyle::Helix {
        dressups.helix_radius_for(tool).emitted_mm
    } else {
        0.0
    };
    let (feed_rate, plunge_rate) = fixture_feeds(OperationType::Adaptive);
    // P10 (ruling D8): the op has no stepover or depth per pass until it
    // has a tool; the fixture states the values from before P10.
    let geometry = |field: &str| {
        fixture_geometry(OperationType::Adaptive)
            .iter()
            .find_map(|&(name, value)| (name == field).then_some(value))
    };
    let stepover = cfg
        .stepover
        .or_else(|| geometry("stepover"))
        .expect("the fixture states the stepover");
    let depth_per_pass = cfg
        .depth_per_pass
        .or_else(|| geometry("depth_per_pass"))
        .expect("the fixture states the depth per pass");
    AdaptiveParams {
        tool_radius: tool.diameter / 2.0,
        stepover,
        cut_depth: -depth_per_pass,
        feed_rate,
        plunge_rate,
        safe_z: depth_per_pass,
        tolerance: cfg.tolerance,
        slot_clearing: cfg.slot_clearing,
        min_cutting_radius: cfg.min_cutting_radius,
        standing_material: None,
        cleanup_strategy: cfg.cleanup_strategy,
        engagement_measure: cfg.engagement_measure,
        path_strategy: PathStrategy2d::Agent,
        trochoid_cap_mult: 1.2,
        keep_down_links: KeepDownLinks::WithinPassLoad,
        entry_helix_radius,
        entry_helix_core_floor: 0.0,
    }
}

/// The move count and the hash of each move's target, move type and intent.
fn plan_fingerprint(tp: &Toolpath) -> (usize, u64) {
    let fields: Vec<_> = tp
        .moves
        .iter()
        .map(|m| (m.target, m.move_type, m.intent))
        .collect();
    (tp.moves.len(), fnv1a_debug(&fields))
}

/// [`plan_fingerprint`] with every feed value set to 0: the plan's
/// geometry, move kinds and intents.
fn shape_fingerprint(tp: &Toolpath) -> (usize, u64) {
    let fields: Vec<_> = tp
        .moves
        .iter()
        .map(|m| (m.target, m.move_type.with_feed_rate(0.0), m.intent))
        .collect();
    (tp.moves.len(), fnv1a_debug(&fields))
}

/// The reference plan of the product preset on each harness shape:
/// `(shape, fixture, (moves, hash))`.
const PINS: &[(&str, &str, (usize, u64))] = &[
    (
        "star",
        "test_data/ux_2d_star.toml",
        (2282, 13515345315611976090),
    ),
    (
        "pocket",
        "test_data/ux_2d_pocket.toml",
        (3237, 10112765154809484364),
    ),
];

#[test]
fn the_research_reference_of_the_product_preset_is_pinned() {
    for &(shape, project, pin) in PINS {
        let (outline, tool) = fixture(project);
        let run = run_2d(&outline, &params(&tool), Candidate2d::Geodesic);
        assert_eq!(
            plan_fingerprint(&run.toolpath),
            pin,
            "{shape}: the reference plan of C35 plan A moved"
        );
    }
}

/// The product door plans what the research reference plans, move by move,
/// with the feed values left out (module doc).
#[test]
fn the_product_door_plans_the_reference() {
    for &(shape, project, _) in PINS {
        let (outline, tool) = fixture(project);
        let params = params(&tool);
        let reference = run_2d(&outline, &params, Candidate2d::Geodesic).toolpath;
        let product = geodesic_toolpath(&outline, &params, &|| false).expect("no cancel");
        assert_eq!(
            shape_fingerprint(&product),
            shape_fingerprint(&reference),
            "{shape}: the product door does not plan the reference"
        );
    }
}
