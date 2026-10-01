//! Tiered-finish plan F4a (`planning/tiered_finish_2026-09-30/PLAN.md`,
//! operator approval 2026-10-01): a planner tier op fits arcs at half the
//! plan's cusp height, and no other op changes.
//!
//! # The claim
//!
//! The arc fitter holds every source segment within its tolerance of the arc
//! in 3D, on either side, so an arc can move the surface by two tolerances
//! peak to peak. A tier is dialled to the cusp height `h`; `2 · t ≤ h` gives
//! `t = h / 2` (0.015 mm at the planner's 0.03 mm). The role default, 0.05
//! mm, is two to three cusps peak to peak, and the F4 A/B measured its
//! burial (`RESULTS.md`, "F4 — arc-fit A/B").
//!
//! The change is confined to the planner's tier ops. A Finish op the
//! operator adds keeps the role default of 0.05 mm.
//!
//! Nothing here generates: the plan writes configs only.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use common::tools::ball_tool_config;
use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::{ArcFitParams, DressupConfig};
use rs_cam_core::mesh::TriangleMesh;
use rs_cam_core::session::multitool::{MultitoolPlanSpec, tier_arc_tolerance_mm};
use rs_cam_core::session::{
    AddToolpathArgs, Command, ProjectSession, ProjectSessionBuilder, TierStrategy,
};

/// The role default every non-tier Finish op carries. Read from the type,
/// then pinned to the printed default in `compute/config.rs`.
const ROLE_DEFAULT_MM: f64 = 0.05;

fn dome() -> TriangleMesh {
    common::meshes::height_field(8.0, 0.5, |x, y| {
        let r_sq = x * x + y * y;
        (25.0 - r_sq).max(0.0).sqrt() - 5.0
    })
}

/// One mesh model, a Ø4 / Ø3 ball pair, and one hand-built Unified Finish
/// on the coarse tool, added before the plan. Returns the session and
/// `(coarse_id, fine_id, model_id)`.
fn session_with_hand_finish() -> (ProjectSession, usize, usize, usize) {
    let mut builder = ProjectSessionBuilder::new();
    builder = builder.stock(common::session::stock_over(9.0, 12.0));
    let coarse_idx = builder.add_tool(ball_tool_config(4.0));
    let fine_idx = builder.add_tool(ball_tool_config(3.0));
    let coarse_id = builder.tools()[coarse_idx].id.0;
    let fine_id = builder.tools()[fine_idx].id.0;
    let model_id = builder.add_model(common::session::mesh_model(dome(), "dome"));
    let mut session = builder.build();
    let hand = common::session::toolpath_config(
        "Operator's own finish",
        OperationConfig::UnifiedFinish(Default::default()),
        coarse_id,
        model_id,
    );
    let _ = session
        .apply(Command::AddToolpath(AddToolpathArgs {
            setup_index: 0,
            config: Box::new(hand),
        }))
        .expect("add the hand finish");
    (session, coarse_id, fine_id, model_id)
}

fn arc_tolerance(dressups: &DressupConfig) -> f64 {
    dressups.arc_fitting.expect("this op fits arcs").tolerance
}

/// Every tier op of a plan carries `cusp / 2`, for every strategy and at
/// two cusp heights; the hand-built Finish op beside it keeps 0.05 mm.
#[test]
fn a_tier_op_fits_arcs_at_half_the_cusp_and_a_hand_finish_keeps_the_role_default() {
    assert!(
        (ArcFitParams::default().tolerance - ROLE_DEFAULT_MM).abs() < 1e-15,
        "the role default this sentry compares against"
    );

    for cusp_mm in [MultitoolPlanSpec::default().cusp_height_mm, 0.01] {
        for strategy in [
            TierStrategy::UnifiedFinish,
            TierStrategy::Scallop,
            TierStrategy::IsoScallop,
        ] {
            let (mut session, coarse_id, fine_id, model_id) = session_with_hand_finish();
            let outcome = session
                .plan_multitool_finishing(&MultitoolPlanSpec {
                    setup_index: 0,
                    model_id,
                    tool_ids: vec![coarse_id, fine_id],
                    cusp_height_mm: cusp_mm,
                    tier_strategies: vec![strategy; 2],
                    ..MultitoolPlanSpec::default()
                })
                .expect("the ladder plans");
            assert_eq!(outcome.toolpath_ids.len(), 2);

            let want = cusp_mm / 2.0;
            assert_eq!(
                tier_arc_tolerance_mm(cusp_mm),
                Some(want),
                "the law is h / 2"
            );
            let mut tiers = 0;
            for tc in session.toolpath_configs() {
                let got = arc_tolerance(&tc.dressups);
                if tc.planner_origin.is_some() {
                    tiers += 1;
                    assert!(
                        (got - want).abs() < 1e-15,
                        "tier op '{}' ({strategy:?}, cusp {cusp_mm}): arc tolerance {got}, \
                         want {want}",
                        tc.name
                    );
                } else {
                    assert!(
                        (got - ROLE_DEFAULT_MM).abs() < 1e-15,
                        "hand op '{}': arc tolerance {got}, the role default is \
                         {ROLE_DEFAULT_MM}",
                        tc.name
                    );
                }
            }
            assert_eq!(tiers, 2, "non-vacuity: both tier ops were read");
        }
    }

    // The role and the op types keep the default: the change is not global.
    for op in [OperationType::UnifiedFinish, OperationType::Scallop] {
        assert!(
            (arc_tolerance(&DressupConfig::for_op(op)) - ROLE_DEFAULT_MM).abs() < 1e-15,
            "{op:?}: a fresh op keeps the role default"
        );
    }
}

/// A cusp height that gives no tolerance (`2 · t ≤ h` has no positive
/// solution) turns the tier's arcs off instead of fitting at zero or NaN.
#[test]
fn a_cusp_height_with_no_tolerance_fits_no_arcs() {
    for bad in [0.0, -0.03, f64::NAN, f64::INFINITY] {
        assert_eq!(tier_arc_tolerance_mm(bad), None, "cusp {bad}");
    }
    let (mut session, coarse_id, fine_id, model_id) = session_with_hand_finish();
    let _ = session
        .plan_multitool_finishing(&MultitoolPlanSpec {
            setup_index: 0,
            model_id,
            tool_ids: vec![coarse_id, fine_id],
            cusp_height_mm: 0.0,
            ..MultitoolPlanSpec::default()
        })
        .expect("the ladder plans");
    for tc in session.toolpath_configs() {
        if tc.planner_origin.is_some() {
            assert!(
                tc.dressups.arc_fitting.is_none(),
                "tier op '{}' fits arcs at cusp 0",
                tc.name
            );
        }
    }
}
