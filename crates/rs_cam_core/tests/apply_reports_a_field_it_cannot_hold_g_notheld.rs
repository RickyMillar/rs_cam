//! Sentry: **the apply funnel must report a cut-geometry value it could not
//! write** (G-NOTHELD, T-12, 2026-09-16).
//!
//! ## The defect this pins
//!
//! `OperationParams::set_depth_per_pass` and `set_stepover` return `bool`.
//! Their doc comments say why:
//!
//! > Returns `false` when this config has no such field, so the caller can
//! > refuse instead of discarding the value.
//!
//! The funnel discarded it. `feeds/suggest.rs` called
//! `scratch.set_depth_per_pass(...)` and dropped the result on the floor.
//! The copy-back then read `depth_per_pass()`, got `None` for a config with
//! no such field, and wrote nothing. The funnel returned success either way.
//!
//! Ten of the operation configs implement `depth_per_pass`. There are more
//! than twice as many operation types. So for the rest, the calculator
//! computed an axial depth, wrote it to a scratch clone, and the value
//! evaporated with no warning.
//!
//! ## Why it became load-bearing
//!
//! On a constant-torque VFD a power limit does not respond to RPM at all,
//! and barely to feed, because the cutting-force model carries a large
//! feed-independent edge term. The remaining lever is a shallower pass. A
//! power-limited cut on an operation that cannot hold a depth would show the
//! operator a warning, compute the fix, silently discard it, and report
//! success. See `planning/TECH_DEBT_REGISTER.md` T-12 and
//! `planning/load_model_2026-09-16/IMPLEMENTATION_PLAN.md`.
//!
//! ## What is asserted
//!
//! Three arms, because the first alone would pass on a build that warned
//! about everything:
//!
//! 1. An operation with no `depth_per_pass` gets the warning.
//! 2. An operation that HAS one does NOT — the non-vacuity partner.
//! 3. Under `ApplyScope::Speeds` neither warns, because not writing the
//!    geometry is that scope's contract rather than a dropped value.
//!
//! The fixtures are derived, not listed: the test asks each operation
//! whether it holds the field and picks its arm from the answer. A config
//! that gains or loses `depth_per_pass` moves between arms on its own.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::suggest::{
    SuggestContext, SuggestWarning, apply_cut_geometry_to_op, apply_speeds_to_op,
    feeds_result_for_operation,
};
use rs_cam_core::feeds::{FeedsProvenance, FeedsResult};
use rs_cam_core::session::ProjectSession;

/// Every operation family, on a tool that makes the pairing valid. Mirrors
/// `pill_writes_clamped_value_g_pillclamp.rs`, which pins the same funnel
/// from the preview side.
fn fixtures() -> Vec<(OperationType, ToolType)> {
    vec![
        (OperationType::Face, ToolType::EndMill),
        (OperationType::Pocket, ToolType::EndMill),
        (OperationType::Profile, ToolType::EndMill),
        (OperationType::Adaptive, ToolType::EndMill),
        (OperationType::VCarve, ToolType::VBit),
        (OperationType::Rest, ToolType::EndMill),
        (OperationType::Inlay, ToolType::EndMill),
        (OperationType::Zigzag, ToolType::EndMill),
        (OperationType::DropCutter, ToolType::EndMill),
        (OperationType::Adaptive3d, ToolType::EndMill),
        (OperationType::SteepShallow, ToolType::BallNose),
        (OperationType::SpiralFinish, ToolType::BallNose),
        (OperationType::HorizontalFinish, ToolType::EndMill),
        (OperationType::Drill, ToolType::EndMill),
    ]
}

fn try_recipe(
    session: &ProjectSession,
    tool: &ToolConfig,
    op: &OperationConfig,
) -> Option<FeedsResult> {
    let stock = session.stock_config();
    feeds_result_for_operation(
        op,
        tool,
        &stock.material,
        session.machine(),
        rs_cam_core::feeds::embedded_vendor_lut(),
        session.post_config().spindle_strategy,
    )
    .ok()
}

/// Warnings the geometry apply raised for `param_name`, if any.
fn not_held_for(warnings: &[SuggestWarning], param: &str) -> bool {
    warnings.iter().any(|w| {
        matches!(
            w,
            SuggestWarning::CutGeometryFieldNotHeld { param_name, .. } if *param_name == param
        )
    })
}

#[test]
fn the_funnel_reports_a_depth_it_could_not_hold_g_notheld() {
    let session = ProjectSession::new_empty();
    let mut held = 0;
    let mut refused = 0;

    for (op_type, tool_type) in fixtures() {
        let tool = ToolConfig::new_default(ToolId(1), tool_type);
        let op = OperationConfig::new_default(op_type);
        let Some(result) = try_recipe(&session, &tool, &op) else {
            continue;
        };

        // Ask the operation itself, rather than carrying a hand-written
        // list that would rot the first time a config gains the field.
        let mut probe = op.clone();
        let holds_depth = probe.set_depth_per_pass(1.0);

        let mut applied = op.clone();
        let mut prov = FeedsProvenance::default();
        let warnings = apply_cut_geometry_to_op(
            &mut applied,
            &mut prov,
            &result,
            &tool,
            session.machine(),
            &session.stock_config().material,
            op.feeds_style().1,
            SuggestContext::default(),
        );

        if holds_depth {
            held += 1;
            assert!(
                !not_held_for(&warnings, "depth_per_pass"),
                "{op_type:?} holds depth_per_pass, yet the funnel reported it as not held"
            );
        } else {
            refused += 1;
            assert!(
                not_held_for(&warnings, "depth_per_pass"),
                "{op_type:?} has no depth_per_pass, yet the funnel applied cut geometry \
                 and reported success. The recommendation was discarded silently — T-12."
            );
        }
    }

    // Non-vacuity: both arms must actually have been exercised, or this
    // test would pass on a build where every operation fell into one side.
    assert!(
        held >= 3,
        "only {held} operations exercised the holds-the-field arm"
    );
    assert!(
        refused >= 3,
        "only {refused} operations exercised the refuses-the-field arm"
    );
}

#[test]
fn a_speeds_only_apply_does_not_report_a_dropped_geometry_g_notheld() {
    // Not writing the geometry is `ApplyScope::Speeds`' contract. Reporting
    // it as a dropped recommendation would be a false alarm on the most
    // common apply path, which is the default scope.
    let session = ProjectSession::new_empty();
    let mut checked = 0;

    for (op_type, tool_type) in fixtures() {
        let tool = ToolConfig::new_default(ToolId(1), tool_type);
        let op = OperationConfig::new_default(op_type);
        let Some(result) = try_recipe(&session, &tool, &op) else {
            continue;
        };

        let mut applied = op.clone();
        let mut prov = FeedsProvenance::default();
        let warnings = apply_speeds_to_op(
            &mut applied,
            &mut prov,
            &result,
            &tool,
            session.machine(),
            &session.stock_config().material,
            op.feeds_style().1,
            SuggestContext::default(),
        );

        assert!(
            !not_held_for(&warnings, "depth_per_pass") && !not_held_for(&warnings, "stepover"),
            "{op_type:?}: a speeds-only apply reported a dropped cut-geometry value"
        );
        checked += 1;
    }

    assert!(checked >= 8, "only {checked} operations checked");
}
