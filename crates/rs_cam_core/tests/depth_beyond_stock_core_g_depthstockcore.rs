//! G-DEPTHSTOCKCORE (F1.18) — the depth-beyond-stock caution lives in core,
//! so every surface that reads the core diagnostic list gets it.
//!
//! F1.6 found the defect: a pocket 15 mm deep on a 12 mm board generated and
//! simulated in silence. It shipped the rule GUI-side, in
//! `crates/rs_cam_viz/src/ui/properties/operations/mod.rs`, which reaches the
//! inspector ribbon and nothing else. MCP `get_toolpath_diagnostics` and the
//! CLI `project` report both route through
//! `diagnostics::diagnose_toolpath_inputs`, so neither could see it.
//!
//! Pre-fix, this file said:
//!
//! ```text
//! G-DEPTHSTOCKCORE: pocket depth 25.0 on an 18.0 mm board
//!   core diagnostics = ["geom.plunge_exceeds_feed"]
//!   geom.depth_beyond_stock present: false
//! ```
//!
//! ## One deliberate difference from the GUI rule
//!
//! The GUI rule compares the DEEPER of two bottoms with the stock: the
//! operation's depth dial, and the Heights tab's resolved Bottom Z. This one
//! reads the depth dial alone.
//!
//! The reason is F1.19, measured in
//! `pinned_bottom_z_reaches_motion_g_bottompin.rs`: for every operation this
//! rule applies to, a pinned Bottom Z reaches no emitted motion. Folding it
//! in would caution on a number the machine never cuts — which is the class
//! of defect the whole programme is closing.
//!
//! The three operations whose floor really CAN be a pinned bottom —
//! Adaptive3d, UnifiedFinish, Waterline — abstain here rather than guess,
//! because the `ResolvedHeights` snapshot carries no pin flag.
//!
//! ## What this does NOT do
//!
//! It does not delete the GUI-side rule. That file is a live lane in the UI
//! programme. Until the switchover the core predicate is UNCONSUMED by the
//! GUI, and the two surfaces read different rules.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

mod common;
use common::make_endmill_6mm;

use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::HeightContext;
use rs_cam_core::compute::config::{BoundaryConfig, DressupConfig, HeightsConfig, StockSource};
use rs_cam_core::compute::operation_configs::{
    Adaptive3dConfig, DropCutterConfig, PocketConfig, PocketPattern, WaterlineConfig,
};
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::diagnostics::adapters::from_static_checks::{
    ResolvedHeights, depth_beyond_stock, depth_beyond_stock_applies, diagnostics_from_static_checks,
};
use rs_cam_core::diagnostics::ids;
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::geo::P2;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{LoadedModel, ProjectSession, ToolpathConfig};

const STOCK_TOP_Z: f64 = 0.0;
const STOCK_BOTTOM_Z: f64 = -18.0;

fn flat_endmill() -> ToolConfig {
    ToolConfig {
        diameter: 6.0,
        ..ToolConfig::new_default(ToolId(1), ToolType::EndMill)
    }
}

/// A projected snapshot: `ResolvedHeights::from_context` on a bare
/// `HeightContext`.
///
/// The arms below use it to exercise the RULE, which reads five numbers and
/// a stock span. It is no longer what the session builds — N4 (2026-09-10)
/// moved that route to `ResolvedHeights::from_heights`, which resolves the
/// toolpath's own `HeightsConfig`. The two constructors put different
/// numbers in `feed_z`, `bottom_z` and `clearance_z`. They agree on `top_z`
/// and the stock span whenever Top Z is Auto, and those three values are
/// the whole input to the rule this file measures.
fn snapshot(op_depth: f64) -> ResolvedHeights {
    ResolvedHeights::from_context(&HeightContext {
        safe_z: 10.0,
        op_depth,
        stock_top_z: STOCK_TOP_Z,
        stock_bottom_z: STOCK_BOTTOM_Z,
        model_top_z: None,
        model_bottom_z: None,
    })
}

fn pocket(depth: f64) -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        depth,
        depth_per_pass: 2.0,
        ..PocketConfig::default()
    })
}

fn core_ids(op: &OperationConfig, h: &ResolvedHeights) -> Vec<String> {
    diagnostics_from_static_checks(ToolpathId(1), op, &flat_endmill(), Some(h))
        .into_iter()
        .map(|d| d.id.as_str().to_owned())
        .collect()
}

/// The acceptance: a pocket deeper than the board reaches the CORE list.
#[test]
fn a_pocket_deeper_than_the_board_cautions_in_the_core_list() {
    let depth = 25.0;
    let op = pocket(depth);
    let h = snapshot(depth);
    let found = core_ids(&op, &h);
    eprintln!(
        "G-DEPTHSTOCKCORE: pocket depth {depth} on a {:.1} mm board\n  core diagnostics = {found:?}",
        STOCK_TOP_Z - STOCK_BOTTOM_Z
    );
    assert!(
        found.iter().any(|id| id == ids::GEOM_DEPTH_BEYOND_STOCK),
        "the core list carries no {}; it has {found:?}",
        ids::GEOM_DEPTH_BEYOND_STOCK
    );

    let finding = depth_beyond_stock(&op, &h).expect("the rule must fire");
    assert!(
        (finding.excess_mm - 7.0).abs() < 1e-9,
        "25 mm of cut into an 18 mm board overruns by 7 mm, measured {}",
        finding.excess_mm
    );
    assert_eq!(
        finding.message(),
        "Depth exceeds stock thickness by 7.00 mm",
        "the sentence must stay identical to the GUI rule's, or the \
         switchover changes what the operator reads"
    );
    assert!((finding.stock_thickness_mm - 18.0).abs() < 1e-9);
    assert!((finding.cut_floor_z - (-25.0)).abs() < 1e-9);
}

/// A cut inside the board is silent, so a green result above is not a rule
/// that always fires.
#[test]
fn a_pocket_inside_the_board_is_silent() {
    let op = pocket(6.0);
    let h = snapshot(6.0);
    assert!(depth_beyond_stock(&op, &h).is_none());
    assert!(
        !core_ids(&op, &h)
            .iter()
            .any(|id| id == ids::GEOM_DEPTH_BEYOND_STOCK)
    );
    assert!(
        depth_beyond_stock_applies(&op),
        "silence here must mean MEASURED AND CLEAN, not 'the rule skipped it'"
    );
}

/// Cutting a part out lands the floor exactly on the stock bottom. That is
/// the normal way to do it and must not caution.
#[test]
fn a_through_cut_exactly_at_the_stock_bottom_is_silent() {
    let depth = STOCK_TOP_Z - STOCK_BOTTOM_Z;
    let op = pocket(depth);
    let h = snapshot(depth);
    assert!(
        depth_beyond_stock(&op, &h).is_none(),
        "an exact through cut must not caution"
    );
}

/// F1.19's consequence. The rule reads the depth dial, never the Bottom Z
/// field, so a bottom far below the board with the depth inside it is silent.
#[test]
fn a_bottom_z_below_the_board_does_not_caution_on_its_own() {
    let op = pocket(6.0);
    let mut h = snapshot(6.0);
    h.bottom_z = -40.0; // as if the Heights tab pinned it 22 mm under the board
    let finding = depth_beyond_stock(&op, &h);
    eprintln!("G-DEPTHSTOCKCORE: bottom_z {} -> {finding:?}", h.bottom_z);
    assert!(
        finding.is_none(),
        "a pinned bottom reaches no motion on a pocket (F1.19), so it must \
         not raise a caution about the cut"
    );
}

/// The three operations whose floor really can be the pin abstain, and so
/// does the surface-riding family. Every one of these is NOT MEASURED, not
/// clean.
#[test]
fn the_operations_this_rule_cannot_answer_for_abstain() {
    let cases: Vec<(&str, OperationConfig)> = vec![
        (
            "Adaptive3d",
            OperationConfig::Adaptive3d(Adaptive3dConfig::default()),
        ),
        (
            "Waterline",
            OperationConfig::Waterline(WaterlineConfig::default()),
        ),
        (
            "DropCutter",
            OperationConfig::DropCutter(DropCutterConfig::default()),
        ),
    ];
    let h = snapshot(50.0);
    for (name, op) in cases {
        assert!(
            !depth_beyond_stock_applies(&op),
            "{name} must abstain — its floor is not `top_z - own depth`"
        );
        assert!(depth_beyond_stock(&op, &h).is_none(), "{name}");
    }
    assert!(
        OperationType::Adaptive3d.honors_pinned_bottom_z(),
        "Adaptive3d abstains BECAUSE the pin is its floor; if that stops \
         being true the reason for abstaining changes"
    );
}

/// No stock span means not measured. The rule must not invent a thickness.
#[test]
fn a_missing_stock_span_abstains() {
    let op = pocket(25.0);
    let mut h = snapshot(25.0);
    h.stock_bottom_z = None;
    assert!(depth_beyond_stock(&op, &h).is_none());
    h.stock_bottom_z = Some(STOCK_BOTTOM_Z);
    h.stock_top_z = None;
    assert!(depth_beyond_stock(&op, &h).is_none());
    assert!(
        depth_beyond_stock_applies(&op),
        "the operation is in scope; only the measurement is missing"
    );
}

/// The id is registered, so the uniqueness sentry and downstream consumers
/// that iterate `ids::ALL` see it.
#[test]
fn the_id_is_in_the_registry() {
    assert!(ids::ALL.contains(&ids::GEOM_DEPTH_BEYOND_STOCK));
    assert_eq!(ids::GEOM_DEPTH_BEYOND_STOCK, "geom.depth_beyond_stock");
}

// ── the session route, exercised rather than read ───────────────────────

/// Build the same project the operator would: an 18 mm board, one Ø6 end
/// mill, one pocket at `depth`.
fn pocket_session(depth: f64) -> ProjectSession {
    let mut session = ProjectSession::new_empty();
    let _ = session.set_stock_config(StockConfig {
        x: 100.0,
        y: 100.0,
        z: STOCK_TOP_Z - STOCK_BOTTOM_Z,
        origin_x: -10.0,
        origin_y: -10.0,
        // Negative, so the stock TOP sits at Z = 0 and 2D ops cut downward.
        origin_z: STOCK_BOTTOM_Z,
        auto_from_model: false,
        material: Material::SolidWood {
            species: WoodSpecies::GenericHardwood,
        },
        ..StockConfig::default()
    });
    let tool_idx = session
        .add_tool(make_endmill_6mm())
        .created
        .expect("add_tool reports the new tool index");
    let tool_id = session.tools()[tool_idx].id.0;
    let model_id = session
        .add_model(LoadedModel {
            id: 0,
            name: "board".to_owned(),
            mesh: None,
            polygons: Some(Arc::new(vec![Polygon2::new(vec![
                P2::new(5.0, 5.0),
                P2::new(75.0, 5.0),
                P2::new(75.0, 55.0),
                P2::new(5.0, 55.0),
            ])])),
            drill_targets: Arc::new(Vec::new()),
            layers: Arc::new(Vec::new()),
            path: PathBuf::from("synthetic://board.svg"),
            kind: None,
            units: None,
            enriched_mesh: None,
            winding_report: None,
            load_error: None,
        })
        .created
        .expect("add_model reports the new model id");
    let tc = ToolpathConfig {
        id: ToolpathId(0),
        name: "Pocket".to_owned(),
        enabled: true,
        operation: OperationConfig::Pocket(PocketConfig {
            stepover: 2.0,
            depth,
            depth_per_pass: 2.0,
            feed_rate: 770.0,
            plunge_rate: 385.0,
            climb: true,
            pattern: PocketPattern::Contour,
            angle: 0.0,
            finishing_passes: 0,
            spindle_rpm: Some(18_000),
        }),
        dressups: DressupConfig::default(),
        // Auto everywhere. The point is that the OPERATION's depth reaches
        // the caution with no Heights pin involved at all.
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
        planner_origin: None,
    };
    let _ = session.add_toolpath(0, tc).expect("add pocket toolpath");
    session
}

/// The claim this arm exists to stop me over-stating.
///
/// Everything above proves the RULE. This proves the ROUTE:
/// `ProjectSession::diagnose_toolpath` is the entry MCP
/// `get_toolpath_diagnostics` calls, and it builds its own
/// `HeightContext` through `height_context_for_toolpath`. Reading that
/// chain is not the same as running it — the stock span has to survive
/// `HeightContext` -> `ResolvedHeights::from_heights` -> the rule, and only
/// a run shows that it does.
///
/// The MCP server is down this session, so this is the MCP route minus the
/// transport. It is not a live GUI check and must not be reported as one.
#[test]
fn the_session_route_carries_the_caution_to_its_mcp_entry_point() {
    let session = pocket_session(25.0);
    let ids: Vec<String> = session
        .diagnose_toolpath(0)
        .expect("the toolpath exists")
        .into_iter()
        .map(|d| d.id.as_str().to_owned())
        .collect();
    eprintln!("G-DEPTHSTOCKCORE session route: {ids:?}");
    assert!(
        ids.iter().any(|id| id == ids::GEOM_DEPTH_BEYOND_STOCK),
        "diagnose_toolpath must carry {}; it carried {ids:?}",
        ids::GEOM_DEPTH_BEYOND_STOCK
    );

    // The control. Without it a rule that always fires would pass above.
    let shallow: Vec<String> = pocket_session(6.0)
        .diagnose_toolpath(0)
        .expect("the toolpath exists")
        .into_iter()
        .map(|d| d.id.as_str().to_owned())
        .collect();
    eprintln!("G-DEPTHSTOCKCORE session route, 6 mm pocket: {shallow:?}");
    assert!(
        !shallow.iter().any(|id| id == ids::GEOM_DEPTH_BEYOND_STOCK),
        "a 6 mm cut in an 18 mm board must not caution; got {shallow:?}"
    );
}
