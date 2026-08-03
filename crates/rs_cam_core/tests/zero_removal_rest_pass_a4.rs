//! A4 (Checkpoint E) — a rest pass that removes NOTHING must say so.
//!
//! # What this is
//!
//! H4's §3.2 Finding 1, rendered rather than inferred: on the grooved-block
//! fixture a same-tool cascade's rest pass cost **+43.9 s (+45% of the arm's
//! runtime), 1 294 mm of cutting and 48 retract round trips** and removed
//! **zero** material — the two arms' COLUMNS were identical to every digit
//! reported, and `groove_c_minus_b.png` was uniformly zero over all 96 641
//! columns. Not "small". Zero.
//!
//! The operator's ruling: **report it, do not refuse it.** A rest pass that
//! finds nothing is sometimes exactly what an operator wants (a cascade whose
//! prior op might have left something, run to be sure). What is not
//! acceptable is paying full price in motion with nothing anywhere saying the
//! pass was inert. The claims pipeline already holds both halves — the
//! reference stock and the territory — so the finding is cheap.
//!
//! # What is measured
//!
//! How far material rises above the CUTTER'S OWN SURFACE at each sampled
//! column of the reference stock — not above the tip point, which reads a
//! false positive the size of a cusp on anything curved. Sampled along the
//! swept path at the stock grid's own cell size, through
//! `dressup::reference_engagement_of_cutting_moves`, which is also where the
//! air-cut filter's "is the tool in material" lookup lives: a second opinion
//! on that question is how two surfaces come to disagree about what a pass
//! did.
//!
//! The verdict is taken against a floor DERIVED from the reference's
//! resolution (`cell²/(2·tip radius)`), because a reference stock is a
//! sampled surface and a pass riding on ground it already cut still measures
//! a little material above the cutter. On this fixture that residual is
//! 17.8 µm against a 20.8 µm floor — thin, and stated rather than papered
//! over: a finer simulation grid widens the margin, a coarser one narrows
//! it.
//!
//! # The two arms
//!
//! One fixture, one variable — the FINISH op's raster stepover, i.e.
//! whether the prior pass left the rest pass anything to do:
//!
//! | arm | finish stepover | what the rest pass finds | finding |
//! |---|---|---|---|
//! | inert | the rest pass's own (0.5 mm) | its own previous cut, to the µm | FIRES |
//! | working | 3× coarser | ~190 µm cusp ridges between the coarse lines | silent |
//!
//! The `working` arm is what stops this from being a sentry that fires on
//! every rest pass: same machinery, same reference, real material, and it
//! must stay quiet.
//!
//! `stock_to_leave` was the obvious variable and is NOT usable here, which
//! this sentry found the hard way: `UnifiedFinish`'s SHALLOW (raster) band
//! passes the raw drop-cutter grid to `raster_toolpath_from_grid`, which
//! takes no stock-to-leave at all, so both arms of a stock-to-leave
//! comparison emit byte-identical toolpaths. That is a real defect in its
//! own right and is filed as such — see the wave-16 log entry — but it is
//! not this sentry's subject.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
// The measured numbers ARE the record. Printing them makes a regression
// readable in CI output rather than only as a threshold that tripped.
#![allow(clippy::print_stderr)]

mod common;

use std::sync::atomic::AtomicBool;

use common::meshes::height_field;
use common::session::{generate, mesh_model, pinned_heights, stock_over, toolpath_config};
use common::tools::ball_tool_config;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::StockSource;
use rs_cam_core::compute::operation_configs::{ClaimsReference, UnifiedFinishConfig};
use rs_cam_core::diagnostics::ids;
use rs_cam_core::session::{ProjectSession, SimulationOptions};
use rs_cam_core::unified_finish::ClaimsReferenceResolution;

/// Half-extent (mm) of the fixture surface.
const HALF: f64 = 20.0;

/// Stock height (mm). The surface lives in `[STOCK_Z - RELIEF, STOCK_Z]`.
const STOCK_Z: f64 = 6.0;

/// Peak-to-trough of the fixture surface (mm).
const RELIEF: f64 = 2.0;

/// The rest pass's raster stepover (mm), and the INERT arm's finish
/// stepover: identical, so the rest pass replays the finish pass's own
/// lines and can find nothing but its own cut.
const REST_STEPOVER_MM: f64 = 0.5;

/// The WORKING arm's finish stepover (mm). Three times coarser, so it
/// leaves ridges of roughly `stepover²/(8·R) = 1.5²/(8·1.5) ≈ 0.19 mm`
/// between its lines — an order of magnitude above anything the reference's
/// own sampling can manufacture, and exactly what a rest pass exists for.
const COARSE_FINISH_STEPOVER_MM: f64 = 1.5;

/// The rest-territory gate (mm). Low on purpose: the pass must keep its
/// territory in both arms, so the only thing that differs is whether there
/// is anything in front of it.
const REST_GATE_MM: f64 = 0.05;

/// Simulation cell (mm). Well under the Ø3 ball's tip radius, per
/// `feedback_rest_measurement_prerequisites`.
const SIM_CELL_MM: f64 = 0.25;

/// The same smooth double bump the A/M6 cascade sentry uses, in the same
/// frame: occupying the TOP of the stock, so the finish pass has real work
/// and the simulated stock is a real machined surface rather than an
/// untouched block (the F-024 frame trap, arriving from the test side).
fn bumpy_surface() -> rs_cam_core::mesh::TriangleMesh {
    height_field(HALF, 0.5, |x, y| {
        let sx = (x / HALF * std::f64::consts::PI).cos();
        let sy = (y / HALF * std::f64::consts::PI).cos();
        STOCK_Z - RELIEF * 0.5 * (1.0 - sx * sy * 0.9)
    })
}

/// Dials shared by both ops. `raster_stepover` is the only thing any caller
/// varies, which is what makes the comparison single-variable.
fn unified_cfg(raster_stepover: f64, rest_pass: bool) -> UnifiedFinishConfig {
    UnifiedFinishConfig {
        scallop_height: 0.02,
        overlap_mm: 0.5,
        tolerance: 0.05,
        raster_stepover,
        z_step: 0.5,
        sampling: 0.4,
        feed_rate: 2000.0,
        plunge_rate: 500.0,
        // The claims pipeline runs only under `pencil_claims`, and
        // `territory_clip` is what makes the op a rest pass at all.
        pencil_claims: rest_pass,
        territory_clip: rest_pass,
        claims_reference: ClaimsReference::Auto,
        min_rest_depth_mm: REST_GATE_MM,
        ..UnifiedFinishConfig::default()
    }
}

/// Finish, then a rest pass reading the stock the finish left.
fn cascade_session(finish_stepover: f64) -> ProjectSession {
    let mut session = ProjectSession::new_empty();
    session.set_stock_config(stock_over(HALF, STOCK_Z));
    let tool_idx = session.add_tool(ball_tool_config(3.0));
    let tool_id = session.tools()[tool_idx].id.0;
    let model_id = session.add_model(mesh_model(bumpy_surface(), "bumps"));
    let heights = pinned_heights(STOCK_Z, STOCK_Z - RELIEF);

    let mut finish = toolpath_config(
        "Finish (all-over)",
        OperationConfig::UnifiedFinish(unified_cfg(finish_stepover, false)),
        tool_id,
        model_id,
    );
    finish.heights = heights.clone();
    session.add_toolpath(0, finish).expect("add finish op");

    let mut rest = toolpath_config(
        "Rest (same tool)",
        OperationConfig::UnifiedFinish(unified_cfg(REST_STEPOVER_MM, true)),
        tool_id,
        model_id,
    );
    rest.heights = heights;
    // Without this the op reads FRESH stock and there is no reference for
    // anything to be measured against.
    rest.stock_source = StockSource::FromRemainingStock;
    session.add_toolpath(0, rest).expect("add rest op");
    session
}

/// What the rest op of one arm did.
struct RestArmResult {
    cutting_mm: f64,
    finding: Option<rs_cam_core::compute::config::ZeroRemovalFinding>,
    diagnostic: Option<String>,
    narration: String,
}

/// Generate op 0, simulate, generate op 1 against that stock — in that
/// order, because a rest op cannot see stock produced by an op generated
/// earlier in the same pass.
fn run(finish_stepover: f64) -> RestArmResult {
    let mut session = cascade_session(finish_stepover);
    let cancel = AtomicBool::new(false);
    generate(&mut session, 0);
    session
        .run_simulation(
            &SimulationOptions {
                resolution: SIM_CELL_MM,
                auto_resolution: false,
                metrics_enabled: false,
                ..SimulationOptions::default()
            },
            &cancel,
        )
        .expect("simulation of the finish pass");
    generate(&mut session, 1);

    let rest = session.get_result(1).expect("rest result");
    // The reference has to have RESOLVED, or the arm is measuring nothing.
    assert_eq!(
        rest.stats
            .claims_reference
            .expect("a rest op running claims records its reference")
            .resolution,
        ClaimsReferenceResolution::DerivedMachinedStock,
        "both arms must run against the machined prior",
    );
    let diagnostic = session
        .diagnose_toolpath_with_trace(1, None)
        .expect("diagnose")
        .into_iter()
        .find(|d| d.id.as_str() == ids::GEOM_ZERO_REMOVAL)
        .map(|d| d.message);
    RestArmResult {
        cutting_mm: rest.stats.cutting_distance,
        finding: rest.stats.zero_removal,
        diagnostic,
        narration: session.narrate_toolpath(1).expect("narrate"),
    }
}

/// THE A4 GATE — red-first on §3.2 Finding 1's shape.
///
/// Gate, not characterisation. Held fixed: fixture, tool, every dial except
/// the rest op's `stock_to_leave`. Domain: mm of tip depth below the
/// reference stock surface, measured at generation against the simulated
/// prior stock.
#[test]
fn a_rest_pass_aimed_at_its_own_previous_cut_reports_zero_removal() {
    let inert = run(REST_STEPOVER_MM);

    // The pass is not empty — it pays full price in motion. Without this the
    // finding would be reporting on a toolpath nobody would have run anyway,
    // which is not what §3.2 measured.
    assert!(
        inert.cutting_mm > 100.0,
        "the inert arm must still emit a real pass — got {:.1} mm",
        inert.cutting_mm,
    );

    let f = inert
        .finding
        .expect("a rest pass that reaches nothing must produce the A4 finding");
    eprintln!(
        "[A4 inert] cutting {:.1} mm | deepest engagement {:.4} mm over {} \
         sampled positions",
        inert.cutting_mm, f.deepest_engagement_mm, f.sampled_positions,
    );
    assert!(
        f.sampled_positions > 0,
        "a finding with no samples behind it is not a measurement",
    );
    assert!(
        f.deepest_engagement_mm <= f.floor_mm,
        "the inert arm's tool must never get under the reference surface by \
         more than the reference's own sampling residual, got {:.4} mm \
         against a {:.4} mm floor",
        f.deepest_engagement_mm,
        f.floor_mm,
    );
    // The floor is derived, not dialled: cell² / (2 · tip radius), with the
    // Ø3 ball's 1.5 mm tip against the 0.25 mm simulation cell.
    let expected_floor = SIM_CELL_MM * SIM_CELL_MM / (2.0 * 1.5);
    assert!(
        (f.floor_mm - expected_floor).abs() < 1e-9,
        "the floor must follow the reference's resolution: expected \
         {expected_floor:.6} mm, got {:.6} mm",
        f.floor_mm,
    );
    assert!(
        f.cutting_distance_mm > 100.0,
        "the finding must carry the motion the pass costs — that IS the \
         point of reporting it",
    );

    // It reaches the operator on both report surfaces, and it is a report:
    // generation succeeded and the toolpath is intact.
    let message = inert
        .diagnostic
        .expect("the finding must reach the diagnostics list");
    assert!(
        message.contains("removes no material"),
        "the diagnostic must say what happened: {message}"
    );
    assert!(
        inert.narration.contains("Zero removal:"),
        "narration is the agent-facing surface:\n{}",
        inert.narration
    );
}

/// The other half of the gate: the same machinery on a pass with real work
/// must stay SILENT. A finding that fires on every rest pass is noise, and
/// §3.3 Finding 8 measured a rest pass on terrain that removes real islands.
#[test]
fn a_rest_pass_with_material_in_front_of_it_stays_silent() {
    let working = run(COARSE_FINISH_STEPOVER_MM);
    eprintln!(
        "[A4 working] cutting {:.1} mm | finding {:?}",
        working.cutting_mm,
        working.finding.map(|f| f.deepest_engagement_mm),
    );
    assert!(
        working.cutting_mm > 100.0,
        "the working arm must emit a real pass — got {:.1} mm",
        working.cutting_mm,
    );
    assert!(
        working.finding.is_none(),
        "a rest pass with ~0.19 mm cusp ridges in front of it removes \
         material, so the finding must not fire: {:?}",
        working.finding,
    );
    assert!(
        working.diagnostic.is_none(),
        "and nothing must reach the diagnostics list either",
    );
}
