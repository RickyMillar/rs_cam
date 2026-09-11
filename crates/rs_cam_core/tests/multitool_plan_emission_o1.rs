//! O-A3 — what `plan_multitool_finishing` emits
//! (`planning/multitool_2026-08-23/ORCHESTRATION_PLAN.md` Phase O item 1).
//!
//! Four claims, none of which any other sentry covers:
//!
//! 1. **The chain is an op chain**, coarse → fine, one tool per op, every one
//!    stamped with a [`PlannerOrigin`] — without which an emitted tier is
//!    indistinguishable from a hand-built op and a re-plan is either
//!    impossible or destructive (T3 finding C1).
//! 2. **Only fine tiers are confined.** Tier 0's cusp target holds everywhere
//!    by construction, so it sweeps the board; tiers ≥ 1 carry a
//!    `PlannedTierRegions` recipe naming the FULL ladder.
//! 3. **Re-plan replaces.** One plan per setup: a second call removes the
//!    first chain and says which ids it took, rather than accumulating two
//!    ladders that both claim to be the finishing pass.
//! 4. **Equal cusp across tiers**, pinned on the pair the T4 probe derived by
//!    hand — an R1.5 and an R2.0 ball at 30 µm.
//!
//! The plan is asserted to be **cheap**: it computes no tier map, so this
//! whole file runs without a drop-cutter walk. The boundary it writes is a
//! recipe, resolved at generation time — that half is
//! `tests/planned_tier_regions_boundary_o2.rs`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use common::tools::ball_tool_config;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{BoundarySource, StockSource};
use rs_cam_core::mesh::TriangleMesh;
use rs_cam_core::session::multitool::MultitoolPlanSpec;
use rs_cam_core::session::{ProjectSession, equal_cusp_stepover_mm};
use rs_cam_core::tier_map::ResidualTreatment;

/// Cusp height (mm) every tier is dialled to.
const CUSP_MM: f64 = 0.03;

/// A shallow dome on a plate — enough of a surface for the model check to
/// pass. Nothing here generates, so the terrain never matters.
fn dome() -> TriangleMesh {
    common::meshes::height_field(8.0, 0.5, |x, y| {
        let r_sq = x * x + y * y;
        (25.0 - r_sq).max(0.0).sqrt() - 5.0
    })
}

/// A session with one mesh model and a Ø4 / Ø3 ball pair, nothing else.
/// Returns the session plus `(coarse_id, fine_id, model_id)`.
fn session_with_ladder() -> (ProjectSession, usize, usize, usize) {
    let mut session = ProjectSession::new_empty();
    let _ = session.set_stock_config(common::session::stock_over(9.0, 12.0));
    let coarse_idx = session.add_tool(ball_tool_config(4.0));
    let fine_idx = session.add_tool(ball_tool_config(3.0));
    let coarse_id = session.tools()[coarse_idx].id.0;
    let fine_id = session.tools()[fine_idx].id.0;
    let model_id = session.add_model(common::session::mesh_model(dome(), "dome"));
    (session, coarse_id, fine_id, model_id)
}

fn spec(tool_ids: Vec<usize>, model_id: usize) -> MultitoolPlanSpec {
    MultitoolPlanSpec {
        setup_index: 0,
        model_id,
        tool_ids,
        cusp_height_mm: CUSP_MM,
        treatment: ResidualTreatment::SlopeCompensated,
        ..MultitoolPlanSpec::default()
    }
}

#[test]
fn the_plan_emits_a_coarse_to_fine_chain_with_provenance() {
    let (mut session, coarse_id, fine_id, model_id) = session_with_ladder();
    // Deliberately FINE FIRST: the ladder sorts on cusp radius itself, so
    // the caller's order must not reach the emitted chain.
    let outcome = session
        .plan_multitool_finishing(&spec(vec![fine_id, coarse_id], model_id))
        .expect("a two-ball ladder over a mesh must plan");

    assert_eq!(outcome.toolpath_ids.len(), 2);
    assert!(
        outcome.replaced.is_empty(),
        "nothing to replace on a first plan"
    );
    assert_eq!(session.toolpath_configs().len(), 2);

    let tcs = session.toolpath_configs();
    let origins: Vec<(u64, u8, u8)> = tcs
        .iter()
        .map(|tc| {
            let o = tc
                .planner_origin
                .as_ref()
                .expect("every emitted tier carries provenance, tier 0 included");
            (o.plan_id, o.tier, o.tier_count)
        })
        .collect();
    assert_eq!(
        origins,
        vec![(outcome.plan_id, 0, 2), (outcome.plan_id, 1, 2)],
        "coarse -> fine, one plan id, ladder length carried on each"
    );

    assert_eq!(tcs[0].tool_id, coarse_id, "tier 0 is the COARSE tool");
    assert_eq!(tcs[1].tool_id, fine_id);
    assert_eq!(tcs[0].name, "Finish tier 0 (R2.0)");
    assert_eq!(tcs[1].name, "Finish tier 1 (R1.5)");

    for tc in tcs {
        assert!(tc.enabled);
        assert_eq!(
            tc.stock_source,
            StockSource::FromRemainingStock,
            "every tier chains after roughing and after its predecessor"
        );
        assert!(
            !tc.boundary_inherit,
            "the stock default must not be able to overwrite a planner boundary"
        );
        assert!(matches!(tc.operation, OperationConfig::UnifiedFinish(_)));
    }
}

#[test]
fn only_fine_tiers_carry_a_tier_region_boundary() {
    let (mut session, coarse_id, fine_id, model_id) = session_with_ladder();
    session
        .plan_multitool_finishing(&spec(vec![coarse_id, fine_id], model_id))
        .expect("plan");
    let tcs = session.toolpath_configs();

    assert!(
        !tcs[0].boundary.enabled,
        "tier 0's cusp target holds everywhere — it sweeps the board"
    );

    assert!(tcs[1].boundary.enabled);
    match &tcs[1].boundary.source {
        BoundarySource::PlannedTierRegions {
            tool_ids,
            tier,
            cell_mm,
            treatment,
            ..
        } => {
            assert_eq!(
                tool_ids,
                &vec![coarse_id, fine_id],
                "the FULL ladder: a tier label is 'the coarsest tool on THIS \
                 ladder that holds this cell', which no subset reproduces"
            );
            assert_eq!(*tier, 1);
            assert!(
                *cell_mm >= 0.3,
                "planning resolution must never be the 0.15 mm the plan forbids"
            );
            assert_eq!(*treatment, ResidualTreatment::SlopeCompensated);
        }
        other => panic!("expected PlannedTierRegions on the fine tier, got {other:?}"),
    }
}

#[test]
fn a_re_plan_replaces_the_prior_chain_rather_than_accumulating() {
    let (mut session, coarse_id, fine_id, model_id) = session_with_ladder();
    let first = session
        .plan_multitool_finishing(&spec(vec![coarse_id, fine_id], model_id))
        .expect("first plan");
    let second = session
        .plan_multitool_finishing(&spec(vec![coarse_id, fine_id], model_id))
        .expect("re-plan");

    assert_eq!(
        second.replaced, first.toolpath_ids,
        "the re-plan must report exactly the ids it removed"
    );
    assert_eq!(
        session.toolpath_configs().len(),
        2,
        "one plan per setup: two ladders must never coexist"
    );
    assert!(
        second.plan_id > first.plan_id,
        "a plan id is never reused, so two ladders can never read as one"
    );
    for tc in session.toolpath_configs() {
        assert_eq!(
            tc.planner_origin.as_ref().map(|o| o.plan_id),
            Some(second.plan_id)
        );
    }
}

#[test]
fn a_re_plan_leaves_hand_built_ops_alone() {
    let (mut session, coarse_id, fine_id, model_id) = session_with_ladder();
    // A hand op the operator owns, added BEFORE the plan so the removal has
    // to shift indices around it.
    let hand = common::session::toolpath_config(
        "Operator's own pass",
        OperationConfig::UnifiedFinish(Default::default()),
        coarse_id,
        model_id,
    );
    session.add_toolpath(0, hand).expect("add the hand op");

    session
        .plan_multitool_finishing(&spec(vec![coarse_id, fine_id], model_id))
        .expect("first plan");
    session
        .plan_multitool_finishing(&spec(vec![coarse_id, fine_id], model_id))
        .expect("re-plan");

    let names: Vec<&str> = session
        .toolpath_configs()
        .iter()
        .map(|tc| tc.name.as_str())
        .collect();
    assert_eq!(
        names,
        vec![
            "Operator's own pass",
            "Finish tier 0 (R2.0)",
            "Finish tier 1 (R1.5)"
        ],
        "removal keys on `planner_origin`, never on position or op kind"
    );
    assert!(session.toolpath_configs()[0].planner_origin.is_none());
}

/// The equal-cusp derivation, pinned on the pair the T4 probe derived by
/// hand: tier-A retooled R1.5 → R2.0 at 30 µm moved `raster_stepover`
/// 0.6 → 0.69 (`ORCHESTRATION_PLAN.md` §0). Both tiers must carry the SAME
/// cusp height — that is what makes the seam blend two patterns of equal
/// amplitude instead of printing a step.
#[test]
fn every_tier_is_dialled_to_the_same_cusp() {
    let (mut session, coarse_id, fine_id, model_id) = session_with_ladder();
    session
        .plan_multitool_finishing(&spec(vec![coarse_id, fine_id], model_id))
        .expect("plan");

    let stepovers: Vec<f64> = session
        .toolpath_configs()
        .iter()
        .map(|tc| match &tc.operation {
            OperationConfig::UnifiedFinish(cfg) => {
                assert!(
                    (cfg.scallop_height - CUSP_MM).abs() < 1e-12,
                    "scallop_height IS the cusp height"
                );
                assert_eq!(
                    cfg.stock_to_leave, 0.0,
                    "held equal across tiers, and zero, so no seam prints a \
                     `stock_to_leave · Δcos θ` step"
                );
                cfg.raster_stepover
            }
            other => panic!("expected a unified finish, got {other:?}"),
        })
        .collect();

    assert!((stepovers[0] - equal_cusp_stepover_mm(2.0, CUSP_MM)).abs() < 1e-12);
    assert!((stepovers[1] - equal_cusp_stepover_mm(1.5, CUSP_MM)).abs() < 1e-12);
    // The literal pair, so a change to the law itself has to be deliberate.
    assert!((stepovers[0] - 0.690_217_357).abs() < 1e-9);
    assert!((stepovers[1] - 0.596_992_462).abs() < 1e-9);
    assert!(
        stepovers[0] > stepovers[1],
        "the coarser tool gets the WIDER stepover at equal cusp"
    );
}

#[test]
fn a_one_tool_ladder_and_a_missing_model_are_refused_by_name() {
    let (mut session, coarse_id, fine_id, model_id) = session_with_ladder();

    let err = session
        .plan_multitool_finishing(&spec(vec![coarse_id], model_id))
        .expect_err("a one-tool ladder is an ordinary finish pass");
    assert!(format!("{err}").contains("at least two tools"), "{err}");

    let err = session
        .plan_multitool_finishing(&spec(vec![coarse_id, fine_id], model_id + 99))
        .expect_err("an absent model must refuse, not plan against nothing");
    assert!(format!("{err}").contains("no model with id"), "{err}");

    let err = session
        .plan_multitool_finishing(&spec(vec![coarse_id, 9_999], model_id))
        .expect_err("an unknown tool id must refuse");
    assert!(format!("{err}").contains("no tool with id"), "{err}");

    assert!(
        session.toolpath_configs().is_empty(),
        "a refused plan must leave the project untouched"
    );
}
