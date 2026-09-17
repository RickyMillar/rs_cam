//! Sentry: **a holder collision check that FAILED must never read as a
//! clean one** — the core twin of the CLI's G-COLFAIL (CMP-14, 2026-09-17).
//!
//! ## The defect this pins
//!
//! `ProjectSession::holder_collision_counts` mapped every toolpath through
//!
//! ```text
//! .collision_check(idx, cancel).map(|r| …len()).unwrap_or(0)
//! ```
//!
//! and its own doc said "Toolpaths whose check fails (missing mesh, etc.)
//! count as 0". The pair `(id, 0)` then flowed into
//! `ProjectEvidence::holder_collisions`, and every consumer filtered it with
//! `> 0` — so a toolpath nobody could check was indistinguishable from a
//! toolpath that was checked and found clear, on the one question that
//! wrecks a machine.
//!
//! A second `unwrap_or(0)` sat one consumer further on, in
//! `diagnostics_with_evidence`: a toolpath ABSENT from the evidence was read
//! as a measured zero, summed into the project count and tested with `> 0`.
//!
//! Commit `70a3db27` fixed the same shape in the CLI on the audit day
//! (`collision_count: Option<usize>` plus `collision_checks_failed`). This
//! file is the core twin.
//!
//! ## Why this one is behavioural
//!
//! The CLI sentry is a source scan, because that crate has no fixture whose
//! check fails for a reason other than `MissingGeometry`. Core does: the
//! cancel flag. `run_collision_check` returns `CollisionCheckError::Cancelled`
//! when the flag is already set, which is a genuine failed check and not the
//! expected 2D absence. So this file asserts the OUTPUT, not the source.
//!
//! ## The follow-up this file also pins
//!
//! The holder check has two halves and they need different things. The mesh
//! half needs a mesh; the FIXTURE half needs only the setup's fixtures. The
//! first fix read `NotApplicable` for every mesh-less toolpath, so a 2D job
//! held down by a clamp never had its holder tested against that clamp
//! either. `a_mesh_less_toolpath_still_meets_its_fixtures_g_colfail` pins
//! the measured answer, and
//! `a_mesh_less_toolpath_with_no_fixture_is_not_applicable_g_colfail` pins
//! that the 2D `NotApplicable` case survives.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::sync::atomic::AtomicBool;

use common::meshes::plateau;
use common::session::{
    mesh_model, pinned_heights, polygon_model, square_polygon, stock_under, toolpath_config,
};
use common::tools::ball_tool_config;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::{DropCutterConfig, TraceConfig};
use rs_cam_core::diagnostics::ids;
use rs_cam_core::ids::FixtureId;
use rs_cam_core::session::{
    AddFixtureArgs, Command, Fixture, FixtureKind, ProjectEvidence, ProjectSession,
    ProjectSessionBuilder, VerdictKind,
};
use rs_cam_core::stock::collision::HolderCollisionCheck;

const HALF: f64 = 20.0;
const DEPTH: f64 = 10.0;

/// Two drop-cutter toolpaths over ONE mesh model. Two, because CMP-24 is
/// measured on the same fixture: the sweep must build one spatial index for
/// the model, not one per toolpath.
fn two_toolpaths_one_model() -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new().stock(stock_under(HALF, DEPTH));
    let tool_idx = builder.add_tool(ball_tool_config(3.0));
    let tool_id = builder.tools()[tool_idx].id.0;
    let model_id = builder.add_model(mesh_model(plateau(2.0 * HALF, DEPTH), "plateau"));

    for name in ["Finish A", "Finish B"] {
        let op = OperationConfig::DropCutter(DropCutterConfig {
            stepover: 4.0,
            feed_rate: 1500.0,
            plunge_rate: 300.0,
            min_z: -DEPTH,
            ..DropCutterConfig::default()
        });
        let mut cfg = toolpath_config(name, op, tool_id, model_id);
        cfg.heights = pinned_heights(0.0, -DEPTH);
        builder
            .add_toolpath(0, cfg)
            .expect("add toolpath to a fresh session");
    }

    let mut session = builder.build();
    let cancel = AtomicBool::new(false);
    for index in 0..session.toolpath_count() {
        session
            .generate_toolpath(index, &cancel)
            .expect("generation must succeed");
    }
    session
}

/// A clamp standing where the holder sweeps.
///
/// The box spans the whole 2D cut in XY and reaches far above it in Z, so
/// every shank and holder segment over the cut is inside it. That is
/// deliberate: the sentry proves the fixture half RAN, so the geometry is
/// chosen to make the answer unambiguous.
fn clamp_over_the_cut() -> Fixture {
    Fixture {
        id: FixtureId(1),
        name: "G-COLFAIL clamp".to_owned(),
        kind: FixtureKind::Clamp,
        enabled: true,
        origin_x: -10.0,
        origin_y: -10.0,
        origin_z: -5.0,
        size_x: 20.0,
        size_y: 20.0,
        size_z: 60.0,
        clearance: 0.0,
    }
}

/// ONE 2D toolpath over a POLYGON model — no mesh anywhere in the project.
///
/// The fixture, when there is one, is added BEFORE generation: adding a
/// fixture invalidates the setup's results.
fn one_2d_toolpath(fixture: Option<Fixture>) -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new().stock(stock_under(HALF, DEPTH));
    let tool_idx = builder.add_tool(ball_tool_config(3.0));
    let tool_id = builder.tools()[tool_idx].id.0;
    let model_id = builder.add_model(polygon_model(vec![square_polygon(5.0)], "square"));

    let op = OperationConfig::Trace(TraceConfig {
        depth: 1.0,
        depth_per_pass: 1.0,
        ..TraceConfig::default()
    });
    builder
        .add_toolpath(0, toolpath_config("2D trace", op, tool_id, model_id))
        .expect("add toolpath to a fresh session");

    let mut session = builder.build();
    if let Some(fixture) = fixture {
        let _ = session
            .apply(Command::AddFixture(AddFixtureArgs {
                setup_index: 0,
                fixture: Box::new(fixture),
            }))
            .expect("add the fixture");
    }
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("generation must succeed");
    session
}

/// CMP-14 follow-up: a mesh-less toolpath still meets the setup's fixtures.
///
/// The defect this pins: `collision_check_with_index` refused the WHOLE
/// check with `MissingGeometry` whenever the model carried no mesh, so the
/// sweep read `NotApplicable` and the clamp was never tested. The obstacle
/// half never needed a mesh.
#[test]
fn a_mesh_less_toolpath_still_meets_its_fixtures_g_colfail() {
    let session = one_2d_toolpath(Some(clamp_over_the_cut()));
    let no_cancel = AtomicBool::new(false);
    let outcomes = session.holder_collision_counts(&no_cancel);

    assert_eq!(outcomes.len(), 1, "the 2D toolpath carries a result");
    let (id, outcome) = outcomes[0];
    match outcome {
        HolderCollisionCheck::Measured(n) => assert!(
            n > 0,
            "TP{id}: the clamp stands in the holder's path, so the fixture \
             half must report hits; got Measured(0)"
        ),
        other => panic!(
            "TP{id}: a mesh-less toolpath under a clamp must be MEASURED \
             against that clamp, got {other:?}. The mesh half does not \
             apply; the fixture half needs no mesh."
        ),
    }
}

/// The other half of the follow-up: with no mesh AND no fixture there is
/// still nothing to check against, so `NotApplicable` must stay reachable.
#[test]
fn a_mesh_less_toolpath_with_no_fixture_is_not_applicable_g_colfail() {
    let session = one_2d_toolpath(None);
    let no_cancel = AtomicBool::new(false);
    let outcomes = session.holder_collision_counts(&no_cancel);

    assert_eq!(outcomes.len(), 1, "the 2D toolpath carries a result");
    let (id, outcome) = outcomes[0];
    assert_eq!(
        outcome,
        HolderCollisionCheck::NotApplicable,
        "TP{id}: no mesh and no fixture is the 2D case the CLI expects"
    );
    assert_eq!(
        outcome.count(),
        Some(0),
        "TP{id}: a not-applicable check publishes a true zero"
    );
}

#[test]
fn a_cancelled_sweep_reports_no_measured_zero_g_colfail() {
    let session = two_toolpaths_one_model();

    // The flag is set BEFORE the sweep, so every check fails.
    let cancelled = AtomicBool::new(true);
    let outcomes = session.holder_collision_counts(&cancelled);

    assert_eq!(outcomes.len(), 2, "both toolpaths carry a result");
    for (id, outcome) in &outcomes {
        assert_eq!(
            *outcome,
            HolderCollisionCheck::Failed,
            "TP{id}: a cancelled check must report Failed. Reporting \
             Measured(0) is the defect: it claims the holder is clear on a \
             toolpath nobody could check."
        );
        assert_eq!(
            outcome.count(),
            None,
            "TP{id}: a failed check has no count to publish"
        );
    }
}

#[test]
fn a_clean_sweep_still_measures_zero_g_colfail() {
    // The other half of the contract. `Measured(0)` must stay reachable, or
    // the fix would have replaced one wrong answer with another.
    let session = two_toolpaths_one_model();
    let no_cancel = AtomicBool::new(false);

    for (id, outcome) in session.holder_collision_counts(&no_cancel) {
        assert!(
            matches!(outcome, HolderCollisionCheck::Measured(_)),
            "TP{id}: a check that ran must report Measured, got {outcome:?}"
        );
    }
}

#[test]
fn the_diagnostic_publishes_no_count_for_a_failed_check_g_colfail() {
    let session = two_toolpaths_one_model();
    let cancelled = AtomicBool::new(true);
    let evidence = ProjectEvidence {
        holder_collisions: session.holder_collision_counts(&cancelled),
        ..ProjectEvidence::default()
    };

    let diag = session.diagnostics_with_evidence(&evidence);

    assert_eq!(
        diag.collision_checks_failed, 2,
        "the project summary must say how many checks did not answer"
    );
    for row in &diag.per_toolpath {
        assert_eq!(
            row.collision_count, None,
            "TP{}: a failed check must serialise as null, not 0",
            row.toolpath_id
        );
    }
    assert!(
        diag.verdicts
            .iter()
            .any(|v| v.kind == VerdictKind::HolderCheckFailed),
        "a failed check must raise its own verdict; silence reads as clean"
    );
    assert!(
        !diag
            .verdicts
            .iter()
            .any(|v| v.kind == VerdictKind::HolderCollision),
        "a failed check is not a collision. The two are different claims."
    );
}

#[test]
fn an_unmeasured_toolpath_is_not_a_measured_zero_g_colfail() {
    // The `:719` half. The GUI supplies only the toolpaths its last check
    // found hits on, so ABSENCE from the evidence means not measured.
    let session = two_toolpaths_one_model();
    let diag = session.diagnostics_with_evidence(&ProjectEvidence::default());

    assert_eq!(diag.collision_checks_failed, 0, "nothing FAILED here");
    for row in &diag.per_toolpath {
        assert_eq!(
            row.collision_count, None,
            "TP{}: a toolpath absent from the evidence was read as a \
             measured zero",
            row.toolpath_id
        );
    }
    assert!(
        diag.verdicts
            .iter()
            .all(|v| v.kind != VerdictKind::HolderCollision
                && v.kind != VerdictKind::HolderCheckFailed),
        "an unmeasured toolpath raises neither verdict: it is neither clean \
         nor dirty"
    );
}

#[test]
fn the_triage_reports_a_failed_check_as_a_safety_finding_g_colfail() {
    let mut session = two_toolpaths_one_model();
    let cancel = AtomicBool::new(false);
    session
        .run_simulation(&rs_cam_core::session::SimulationOptions::default(), &cancel)
        .expect("simulation completes");

    let cancelled = AtomicBool::new(true);
    let sim = session.simulation_result().expect("simulation result");
    let evidence = ProjectEvidence::from_simulation_with_holder_collisions(
        sim,
        session.holder_collision_counts(&cancelled),
    );
    let triage = session.simulation_triage(&evidence);

    assert!(
        triage
            .safety
            .iter()
            .any(|f| f.diagnostic.id.as_str() == ids::PROJECT_HOLDER_CHECK_FAILED),
        "the triage dropped the failed checks. Its `> 0` filter used to do \
         exactly that, which is how a failed check reached the operator as \
         a clean page-one answer."
    );
    assert!(
        !triage
            .safety
            .iter()
            .any(|f| f.diagnostic.id.as_str() == ids::PROJECT_HOLDER_COLLISION),
        "a failed check must not be reported under the collision id"
    );
}
