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
//! because `ResolvedHeights::from_context` carries no pin flag.
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

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::HeightContext;
use rs_cam_core::compute::operation_configs::{
    Adaptive3dConfig, DropCutterConfig, PocketConfig, WaterlineConfig,
};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::diagnostics::adapters::from_static_checks::{
    ResolvedHeights, depth_beyond_stock, depth_beyond_stock_applies, diagnostics_from_static_checks,
};
use rs_cam_core::diagnostics::ids;
use rs_cam_core::ids::ToolpathId;

const STOCK_TOP_Z: f64 = 0.0;
const STOCK_BOTTOM_Z: f64 = -18.0;

fn flat_endmill() -> ToolConfig {
    ToolConfig {
        diameter: 6.0,
        ..ToolConfig::new_default(ToolId(1), ToolType::EndMill)
    }
}

/// The snapshot the session builds at `session/compute.rs` before it calls
/// `diagnose_toolpath_inputs` — `ResolvedHeights::from_context` on the
/// toolpath's own `HeightContext`.
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
