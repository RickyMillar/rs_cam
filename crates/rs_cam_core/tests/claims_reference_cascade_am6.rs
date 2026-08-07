//! A/M6 — `claims_reference` must not make a rest pass re-cut the whole part.
//!
//! # What broke
//!
//! `claims_reference` shipped defaulting to `self_probe`, which derives rest
//! ANALYTICALLY — *where can my own cutter not reach the model* — and never
//! consults simulated stock. In a same-tool cascade that names precisely what
//! the tool cannot fix, and (because `territory_clip` only runs under a
//! machined-stock reference) the rest pass silently drops its rest-island
//! confinement and covers the whole surface again. Measured live on wanaka,
//! single variable, 0.1 mm sim, after a full same-tool finish: 46 366 mm of
//! cutting under `self_probe` against 5 259 mm under `machined_stock`,
//! −88.7% from one dial, with nothing warning.
//!
//! # What these sentries hold
//!
//! 1. **The cascade gate.** Same tool, finish then rest: the rest op's
//!    cutting length must be a small fraction of the finish op's — and the
//!    forced-`self_probe` arm must be the absurd one. Run for a BALL and a
//!    TAPERED ball, because the two disagree about what "radius" means and
//!    every rest dial is a feature scale.
//! 2. **No silent fallback, both directions.** Pinning `self_probe` while a
//!    machined prior is in scope produces a user-visible finding; so does
//!    pinning `machined_stock` with none in scope; and `Auto`'s derivation
//!    is recorded either way.
//!
//! # Measurement discipline
//!
//! Both arms of every comparison are the SAME measure (`ToolpathStats::
//! cutting_distance`, mm of `FinishingCut` travel), at the same stage (post
//! generation, post dressup). Each arm runs its own full cascade — generate,
//! simulate, generate — so the standing rule against comparing across
//! regenerations is honoured by ASSERTION rather than by sharing: the finish
//! op is required to come out bit-identical in both arms before any rest
//! number is read. If it ever does not, the comparison is no longer
//! single-variable and the sentry says so instead of quietly reporting a
//! difference that came from somewhere else.
//!
//! The simulation resolution is chosen per tool as a fraction of the CUSP
//! radius, never the envelope: a Ø1-tip taper needs 0.1 mm even though its
//! shank is Ø6. Rest measured on a grid coarser than the tip is not evidence
//! about what a finish pass left (`feedback_rest_measurement_prerequisites`).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
// The measured numbers ARE the record this sentry exists to keep. Printing
// them on every run is what makes a regression readable in CI output instead
// of only as a threshold that tripped.
#![allow(clippy::print_stderr)]

mod common;

use std::sync::atomic::AtomicBool;

use common::meshes::height_field;
use common::session::{generate, mesh_model, pinned_heights, stock_over, toolpath_config};
use common::tools::{ball_tool_config, tapered_ball_tool_config};
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{ClaimsReferenceFinding, StockSource};
use rs_cam_core::compute::operation_configs::{ClaimsReference, UnifiedFinishConfig};
use rs_cam_core::compute::tool_config::ToolConfig;
use rs_cam_core::session::{ProjectSession, SimulationOptions};
use rs_cam_core::unified_finish::{ClaimsReferenceResolution, CreaseReference};

/// Half-extent (mm) of the fixture surface. Small on purpose: the cascade
/// has to generate two surface ops AND a dexel simulation per arm, four
/// times over.
const HALF: f64 = 20.0;

/// Stock height (mm). The surface lives in `[TOP - RELIEF, TOP]`.
const STOCK_Z: f64 = 6.0;

/// Peak-to-trough of the fixture surface (mm).
const RELIEF: f64 = 2.0;

/// The skin (mm) the finish pass deliberately leaves behind.
///
/// This is what makes the fixture reproduce the defect instead of hiding it.
/// With the two ops cutting to the SAME surface the rest pass has literally
/// no material anywhere, the air-cut dressup filter erases every move, and
/// both references measure zero — a green sentry over a broken engine. A
/// uniform sub-threshold skin is the small fixture's stand-in for wanaka's
/// 22 µm raster cusps: real material everywhere, all of it below the dial.
const FINISH_SKIN_MM: f64 = 0.15;

/// The rest-territory gate (mm). ABOVE [`FINISH_SKIN_MM`], so a
/// correctly-referenced rest pass measures the skin as skippable and
/// confines itself away, while an analytically-referenced one never looks at
/// the stock at all and re-cuts the part.
const MIN_REST_DEPTH_MM: f64 = 0.60;

/// A gate BELOW the skin — the control arm. Same machined-stock reference,
/// same confinement machinery, one number moved: now nothing is skippable
/// and the pass covers the surface again. It separates "the reference
/// confined the pass" from "the reference annihilated the pass", which a
/// near-zero derived arm cannot do on its own.
const GATE_BELOW_SKIN_MM: f64 = 0.05;

/// A smooth double-bump surface occupying the TOP of the stock,
/// `z ∈ [STOCK_Z - RELIEF, STOCK_Z]`.
///
/// The frame matters more than the shape. `stock_over` roots the block at
/// `origin_z = 0`, so a surface built around `z = 0` sits entirely BELOW the
/// stock: generation still emits a full pass (fresh-stock generation never
/// consults the stock), the simulation removes nothing, and the rest op that
/// follows finds its whole toolpath in air and is filtered down to four
/// moves. That is a green-looking fixture measuring nothing — the trap the
/// F-024 dexel-frame work exists to stop, arriving from the test side.
///
/// Shape-wise: gentle enough that a ball FOLLOWS it (a ball bridges any
/// feature below its own radius and the offset surface reads flat,
/// `finish_setup.rs`'s documented blind spot), curved enough that the finish
/// pass leaves real, measurable cusps.
fn bumpy_surface() -> rs_cam_core::mesh::TriangleMesh {
    height_field(HALF, 0.5, |x, y| {
        let sx = (x / HALF * std::f64::consts::PI).cos();
        let sy = (y / HALF * std::f64::consts::PI).cos();
        STOCK_Z - RELIEF * 0.5 * (1.0 - sx * sy * 0.9)
    })
}

/// How the rest op of a cascade is dialled: which reference, and which
/// rest-territory gate.
#[derive(Clone, Copy)]
struct RestArm {
    claims_reference: ClaimsReference,
    min_rest_depth_mm: f64,
}

/// Shared dials for both ops of a cascade. The finish op and the rest op
/// differ ONLY in the claims block and the skin — same tool, same stepover,
/// same cusp — which is what makes "the rest op cuts a fraction of the
/// finish op" a statement about the reference rather than about the
/// parameters.
fn unified_cfg(arm: Option<RestArm>) -> UnifiedFinishConfig {
    UnifiedFinishConfig {
        scallop_height: 0.02,
        overlap_mm: 0.5,
        tolerance: 0.05,
        raster_stepover: 0.5,
        z_step: 0.5,
        sampling: 0.4,
        feed_rate: 2000.0,
        plunge_rate: 500.0,
        // The finish op leaves the skin; the rest op takes it to size.
        stock_to_leave: if arm.is_some() { 0.0 } else { FINISH_SKIN_MM },
        // The claims pipeline runs only under `pencil_claims`; `territory_clip`
        // is what turns the op into a rest pass, and it only runs under a
        // machined-stock reference. That coupling IS the defect's mechanism.
        pencil_claims: arm.is_some(),
        territory_clip: arm.is_some(),
        claims_reference: arm.map_or(ClaimsReference::Auto, |a| a.claims_reference),
        min_rest_depth_mm: arm.map_or(MIN_REST_DEPTH_MM, |a| a.min_rest_depth_mm),
        ..UnifiedFinishConfig::default()
    }
}

/// A two-op same-tool cascade: an all-over finish, then a rest pass reading
/// the stock that finish left.
fn cascade_session(tool: ToolConfig, rest_claims: RestArm) -> ProjectSession {
    let mut session = ProjectSession::new_empty();
    session.set_stock_config(stock_over(HALF, STOCK_Z));
    let tool_idx = session.add_tool(tool);
    let tool_id = session.tools()[tool_idx].id.0;
    let model_id = session.add_model(mesh_model(bumpy_surface(), "bumps"));

    // Surface ops carry no depth dial, so an Auto `bottom_z` resolves to
    // `top_z - 0.0` and collapses every band onto the rim — both ops must pin
    // their Z range (`common::session::pinned_heights` doc).
    let heights = pinned_heights(STOCK_Z, STOCK_Z - RELIEF);

    let mut finish = toolpath_config(
        "Finish (all-over)",
        OperationConfig::UnifiedFinish(unified_cfg(None)),
        tool_id,
        model_id,
    );
    finish.heights = heights.clone();
    session.add_toolpath(0, finish).expect("add finish op");

    let mut rest = toolpath_config(
        "Rest (same tool)",
        OperationConfig::UnifiedFinish(unified_cfg(Some(rest_claims))),
        tool_id,
        model_id,
    );
    rest.heights = heights;
    // Without this the op reads FRESH stock, `territory_stock` is `None`, and
    // there is no machined prior for ANY reference to use — the comparison
    // would be measuring nothing.
    rest.stock_source = StockSource::FromRemainingStock;
    session.add_toolpath(0, rest).expect("add rest op");

    session
}

/// Cutting length (mm) of each op, plus the rest op's recorded resolution,
/// from ONE simulation shared by both arms of the comparison.
struct CascadeRun {
    finish_cutting_mm: f64,
    rest_cutting_mm: f64,
    rest_finding: ClaimsReferenceFinding,
}

/// Generate op 0, simulate, generate op 1 against that stock. Three steps,
/// in that order, because a rest op can never see stock produced by an op
/// generated earlier in the SAME pass — the snapshot only exists after a
/// simulation (`AwaitingPriorStock` doc).
fn run_cascade(tool: ToolConfig, sim_resolution: f64, rest_claims: RestArm) -> CascadeRun {
    let mut session = cascade_session(tool, rest_claims);
    let cancel = AtomicBool::new(false);

    generate(&mut session, 0);
    session
        .run_simulation(
            &SimulationOptions {
                resolution: sim_resolution,
                auto_resolution: false,
                metrics_enabled: false,
                ..SimulationOptions::default()
            },
            &cancel,
        )
        .expect("simulation of the finish pass");
    generate(&mut session, 1);

    let finish = session.get_result(0).expect("finish result");
    let rest = session.get_result(1).expect("rest result");
    CascadeRun {
        finish_cutting_mm: finish.stats.cutting_distance,
        rest_cutting_mm: rest.stats.cutting_distance,
        rest_finding: rest
            .stats
            .claims_reference
            .expect("a rest op running claims must record which reference it used"),
    }
}

/// THE A/M6 GATE, per tool shape.
///
/// Red-first: the `self_probe` arm is the pre-fix default's behaviour, and
/// it is asserted to be absurd (a "rest" pass out-cutting the finish pass it
/// follows) rather than merely different. The derived arm is the fix.
fn assert_cascade_gate(label: &str, tool: ToolConfig, sim_resolution: f64) {
    // Arm A — the pre-A/M6 default, now reachable only by pinning it.
    let forced = run_cascade(
        tool.clone(),
        sim_resolution,
        RestArm {
            claims_reference: ClaimsReference::SelfProbe,
            min_rest_depth_mm: MIN_REST_DEPTH_MM,
        },
    );
    // Arm B — the A/M6 default: `Auto`, with a machined prior in scope.
    let derived = run_cascade(
        tool.clone(),
        sim_resolution,
        RestArm {
            claims_reference: ClaimsReference::Auto,
            min_rest_depth_mm: MIN_REST_DEPTH_MM,
        },
    );
    // Arm C — the control: same derived machined-stock reference, gate moved
    // BELOW the skin. Proves the confinement is threshold-driven.
    let control = run_cascade(
        tool,
        sim_resolution,
        RestArm {
            claims_reference: ClaimsReference::Auto,
            min_rest_depth_mm: GATE_BELOW_SKIN_MM,
        },
    );

    // Same fixture, same tool, same dials, one variable: the finish pass must
    // be identical across arms, or the comparison is not single-variable.
    assert!(
        (forced.finish_cutting_mm - derived.finish_cutting_mm).abs() < 1e-6,
        "{label}: the finish op must be identical across arms \
         (self_probe {:.1} mm vs auto {:.1} mm)",
        forced.finish_cutting_mm,
        derived.finish_cutting_mm,
    );
    assert!(
        forced.finish_cutting_mm > 100.0,
        "{label}: the fixture must give the finish pass real work to do, \
         got {:.1} mm",
        forced.finish_cutting_mm,
    );

    // What the two arms resolved to, and what that did to the confinement.
    assert_eq!(
        forced.rest_finding.resolution,
        ClaimsReferenceResolution::ExplicitSelfProbeOverridingPrior,
        "{label}: pinning self_probe with a machined prior in scope",
    );
    assert!(
        forced.rest_finding.territory_clip_skipped(),
        "{label}: territory_clip must be recorded as SKIPPED under a \
         self-probe reference — that is the mechanism",
    );
    assert_eq!(
        derived.rest_finding.resolution,
        ClaimsReferenceResolution::DerivedMachinedStock,
        "{label}: Auto with a machined prior in scope must derive it",
    );
    assert!(!derived.rest_finding.territory_clip_skipped());

    let finish = forced.finish_cutting_mm;
    eprintln!(
        "[A/M6 {label}] finish {finish:.1} mm | rest: self_probe {:.1} mm, \
         derived {:.1} mm, control(gate<skin) {:.1} mm",
        forced.rest_cutting_mm, derived.rest_cutting_mm, control.rest_cutting_mm,
    );

    // THE DEFECT, red-first: under the analytic reference the rest-territory
    // confinement never runs, so the "rest" pass re-cuts the entire finish
    // pass it follows. Measured here at ~100% of the finish pass for both
    // tools; measured on wanaka as 46 366 mm where the corrected reference
    // needed 5 259 mm. This assertion is what fails if A/M6 is reverted.
    assert!(
        forced.rest_cutting_mm > 0.9 * finish,
        "{label}: the forced-self_probe arm must RE-CUT the part — got \
         {:.1} mm against a {finish:.1} mm finish pass, so the fixture is \
         not reproducing the defect and the gate below proves nothing",
        forced.rest_cutting_mm,
    );

    // THE GATE: a rest pass must cut a small fraction of the finish pass it
    // follows. Anything else is not a rest pass.
    //
    // The bound is 0.35, not something tighter, because part of what a
    // correctly-referenced rest pass finds on ANY fixture is not gate-driven
    // at all: classification cells whose rest measurement is untrusted keep
    // their coverage by design (`ClaimsConfig::min_rest_depth_mm` — the
    // detector's erosion rim must never be allowed to amputate band area),
    // and the model sits inside a 2 mm stock margin that no finish pass ever
    // machined, which is genuine rest. Measured: 0% of the finish pass for
    // the ball, 25% for the taper, whose finer cusp-derived conditioning
    // keeps more of that rim. Both are a rest pass doing rest work; neither
    // is the defect.
    assert!(
        derived.rest_cutting_mm < 0.35 * finish,
        "{label}: the DERIVED rest op must cut a small fraction of the \
         finish op — got {:.1} mm against {finish:.1} mm ({:.1}%)",
        derived.rest_cutting_mm,
        100.0 * derived.rest_cutting_mm / finish,
    );

    // The same statement as a single-variable delta: one dial, everything
    // else held. wanaka measured −88.7%; this fixture is required to show at
    // least −65%.
    assert!(
        derived.rest_cutting_mm < 0.35 * forced.rest_cutting_mm,
        "{label}: switching the reference must cut the work down — \
         self_probe {:.1} mm vs derived {:.1} mm ({:+.1}%)",
        forced.rest_cutting_mm,
        derived.rest_cutting_mm,
        100.0 * (derived.rest_cutting_mm / forced.rest_cutting_mm - 1.0),
    );

    // The control, against a derived arm that emitted little or nothing:
    // same machined-stock reference, same confinement machinery, gate moved
    // BELOW the skin. It must still emit real work. Without this, a derived
    // arm that produced nothing for an unrelated reason would satisfy every
    // assertion above while measuring nothing at all.
    assert_eq!(
        control.rest_finding.resolution,
        ClaimsReferenceResolution::DerivedMachinedStock,
        "{label}: the control must use the same reference as arm B",
    );
    assert!(
        control.rest_cutting_mm >= derived.rest_cutting_mm,
        "{label}: lowering the rest gate can only ADD territory — got \
         {:.1} mm against the confined arm's {:.1} mm",
        control.rest_cutting_mm,
        derived.rest_cutting_mm,
    );
    assert!(
        control.rest_cutting_mm > 100.0,
        "{label}: the machined-stock reference must still be able to emit \
         real work — the control cut only {:.1} mm, so the derived arm is \
         not being CONFINED, it is being emptied",
        control.rest_cutting_mm,
    );
}

/// Ball control: `cusp_radius_mm() == envelope_radius_mm()`, so nothing here
/// can be an artefact of the two radii disagreeing.
#[test]
fn cascade_rest_pass_cuts_a_fraction_of_the_finish_pass_ball() {
    // Sim cell well below the Ø3 ball's 1.5 mm tip radius.
    assert_cascade_gate("ball Ø3", ball_tool_config(3.0), 0.2);
}

/// Tapered control: Ø1 tip on a Ø6 shank, the project's own finishing tool.
/// Its 6× envelope-vs-cusp split is what makes a scale mistake visible, and
/// the simulation grid has to follow the CUSP.
#[test]
fn cascade_rest_pass_cuts_a_fraction_of_the_finish_pass_tapered() {
    // Sim cell well below the Ø1 tip's 0.5 mm radius — the standing
    // instruction from §14n, not a tuning choice.
    assert_cascade_gate(
        "tapered Ø1 tip",
        tapered_ball_tool_config(1.0, 7.0, 6.0),
        0.1,
    );
}

/// No silent fallback, direction (a): choosing the analytic reference while
/// a machined prior EXISTS must be recorded as needing attention, and the
/// operator-facing sentence must say what it costs and what to do.
#[test]
fn pinning_self_probe_over_a_machined_prior_is_reported() {
    let run = run_cascade(
        ball_tool_config(3.0),
        0.2,
        RestArm {
            claims_reference: ClaimsReference::SelfProbe,
            min_rest_depth_mm: MIN_REST_DEPTH_MM,
        },
    );
    let r = run.rest_finding.resolution;
    assert_eq!(
        r,
        ClaimsReferenceResolution::ExplicitSelfProbeOverridingPrior
    );
    assert!(r.needs_attention(), "the A/M6 footgun must be loud");
    assert!(r.prior_stock_in_scope());
    assert_eq!(r.reference(), CreaseReference::SelfProbe);
    assert!(!r.is_derived(), "an explicit pin must not read as derived");
    // The remedy has to be in the sentence, not only in the enum name — this
    // string reaches the operator through the `config.claims_reference`
    // diagnostic, the MCP `runtime.claims_reference.why`, and the GUI hover.
    let why = r.why();
    assert!(why.contains("machined_stock"), "why() must name the remedy");
    assert!(why.contains("ROUGHING"), "why() must name the exception");
}

/// No silent fallback, direction (b): `Auto` over FRESH stock resolves to
/// the analytic reference and records WHY, so "nothing was in scope" can
/// never be mistaken for "the machined reference was used".
///
/// This is deliberately NOT a second signal for the F.4 ladder case. An op
/// that asks for remaining stock and has no snapshot never generates at all
/// — it is refused upstream as `ComputeStatus::AwaitingPriorStock`, which is
/// A/M11's taxonomy and has its own fixpoint loop. What this covers is the
/// op that asked for FRESH stock: a configuration statement, not a block.
#[test]
fn auto_over_fresh_stock_resolves_to_self_probe_and_says_so() {
    let mut session = cascade_session(
        ball_tool_config(3.0),
        RestArm {
            claims_reference: ClaimsReference::Auto,
            min_rest_depth_mm: MIN_REST_DEPTH_MM,
        },
    );
    // The only change from the cascade: the rest op reads fresh stock, so no
    // machined prior is in scope even though an upstream op exists.
    session
        .toolpath_configs_mut()
        .get_mut(1)
        .expect("rest op")
        .stock_source = StockSource::Fresh;

    generate(&mut session, 0);
    generate(&mut session, 1);

    let finding = session
        .get_result(1)
        .expect("rest result")
        .stats
        .claims_reference
        .expect("claims ran, so a reference was resolved");
    assert_eq!(
        finding.resolution,
        ClaimsReferenceResolution::DerivedSelfProbeNoPrior
    );
    assert!(finding.resolution.is_derived());
    assert!(!finding.resolution.prior_stock_in_scope());
    assert_eq!(finding.resolution.reference(), CreaseReference::SelfProbe);
    // `territory_clip` was requested and cannot run — that is the difference
    // between a rest pass and an all-over pass, and it must be visible.
    assert!(finding.territory_clip_requested);
    assert!(finding.territory_clip_skipped());
    assert!(
        finding.resolution.why().contains("remaining stock"),
        "why() must name the stock-source remedy"
    );
}

/// The `None` half of the A/M9 contract: an operation that runs NO claims
/// pipeline records no resolution. "Not measured" must stay distinct from
/// "resolved to the analytic reference".
#[test]
fn an_operation_without_claims_records_no_resolution() {
    let mut session = cascade_session(
        ball_tool_config(3.0),
        RestArm {
            claims_reference: ClaimsReference::Auto,
            min_rest_depth_mm: MIN_REST_DEPTH_MM,
        },
    );
    generate(&mut session, 0);
    assert!(
        session
            .get_result(0)
            .expect("finish result")
            .stats
            .claims_reference
            .is_none(),
        "an all-over finish runs no claims pipeline and must resolve nothing"
    );
}

// ── The user-visible half of "no silent fallback" ───────────────────────

/// Diagnostics this operation's generation findings produce.
fn generation_diagnostics(
    session: &ProjectSession,
    index: usize,
) -> Vec<rs_cam_core::diagnostics::Diagnostic> {
    rs_cam_core::diagnostics::adapters::from_generation::diagnostics_from_generation(
        rs_cam_core::ids::ToolpathId(index),
        &session.get_result(index).expect("result").stats,
    )
}

/// The footgun must reach a HUMAN, not a `tracing::warn!` in a process with
/// no subscriber. "A warning nobody sees is not a warning" is this
/// programme's standing rule, and it was written after shipping a 28 mm block
/// of unmachined material for weeks.
#[test]
fn the_footgun_produces_a_caution_diagnostic_naming_the_consequence() {
    let mut session = cascade_session(
        ball_tool_config(3.0),
        RestArm {
            claims_reference: ClaimsReference::SelfProbe,
            min_rest_depth_mm: MIN_REST_DEPTH_MM,
        },
    );
    let cancel = AtomicBool::new(false);
    generate(&mut session, 0);
    session
        .run_simulation(
            &SimulationOptions {
                resolution: 0.2,
                auto_resolution: false,
                metrics_enabled: false,
                ..SimulationOptions::default()
            },
            &cancel,
        )
        .expect("simulation");
    generate(&mut session, 1);

    let diags = generation_diagnostics(&session, 1);
    let d = diags
        .iter()
        .find(|d| d.id.as_str() == rs_cam_core::diagnostics::ids::CONFIG_CLAIMS_REFERENCE)
        .expect("pinning self_probe over a machined prior must be reported");
    assert_eq!(
        d.severity,
        rs_cam_core::diagnostics::Severity::Caution,
        "the A/M6 footgun is not an Info"
    );
    // The message has to name what happened AND what it cost, or it is a
    // label rather than a warning.
    assert!(d.message.contains("analytic self-probe"));
    assert!(
        d.message.contains("territory_clip"),
        "the skipped confinement is the mechanism and must be named: {}",
        d.message
    );
    assert!(
        d.message.contains("FULL territory"),
        "the consequence — an all-over pass wearing a rest pass's name —          must be spelled out: {}",
        d.message
    );

    // The finish op resolved nothing, so it must contribute no such
    // diagnostic. "Not measured" stays distinct from "measured clean".
    assert!(
        generation_diagnostics(&session, 0)
            .iter()
            .all(|d| d.id.as_str() != rs_cam_core::diagnostics::ids::CONFIG_CLAIMS_REFERENCE),
        "an operation that runs no claims pipeline must report no reference"
    );
}

/// The derived resolution is reported too — at `Info`, because nothing is
/// wrong and this is the ONLY surface on which the choice appears. `auto`
/// resolves against whether a prior stock was in scope, which is not a field
/// of any config and is not derivable from the emitted moves.
#[test]
fn the_derived_resolution_is_reported_as_info() {
    let mut session = cascade_session(
        ball_tool_config(3.0),
        RestArm {
            claims_reference: ClaimsReference::Auto,
            min_rest_depth_mm: MIN_REST_DEPTH_MM,
        },
    );
    let cancel = AtomicBool::new(false);
    generate(&mut session, 0);
    session
        .run_simulation(
            &SimulationOptions {
                resolution: 0.2,
                auto_resolution: false,
                metrics_enabled: false,
                ..SimulationOptions::default()
            },
            &cancel,
        )
        .expect("simulation");
    generate(&mut session, 1);

    let diags = generation_diagnostics(&session, 1);
    let d = diags
        .iter()
        .find(|d| d.id.as_str() == rs_cam_core::diagnostics::ids::CONFIG_CLAIMS_REFERENCE)
        .expect("a derived resolution must still be recorded");
    assert_eq!(d.severity, rs_cam_core::diagnostics::Severity::Info);
    assert!(d.message.contains("derived"), "{}", d.message);
    assert!(d.message.contains("machined prior stock"), "{}", d.message);
    // The rough-chain caveat travels with the derivation, because `Auto`
    // cannot see the difference and the operator can.
    assert!(d.message.contains("ROUGHING"), "{}", d.message);
}

/// No silent fallback, direction (b), the EXPLICIT half: a `machined_stock`
/// dial that cannot be honoured degrades to the analytic self-probe — and
/// must say so. Before A/M6 this was a `tracing::warn!` and nothing else, so
/// the operation ran against a reference its own dial denied.
#[test]
fn pinning_machined_stock_without_a_prior_is_reported_not_swallowed() {
    let mut session = cascade_session(
        ball_tool_config(3.0),
        RestArm {
            claims_reference: ClaimsReference::MachinedStock,
            min_rest_depth_mm: MIN_REST_DEPTH_MM,
        },
    );
    // Fresh stock: the dial asks for a machined prior that cannot exist.
    session
        .toolpath_configs_mut()
        .get_mut(1)
        .expect("rest op")
        .stock_source = StockSource::Fresh;

    generate(&mut session, 0);
    generate(&mut session, 1);

    let finding = session
        .get_result(1)
        .expect("rest result")
        .stats
        .claims_reference
        .expect("claims ran, so a reference was resolved");
    let r = finding.resolution;
    assert_eq!(
        r,
        ClaimsReferenceResolution::ExplicitMachinedStockWithoutPrior
    );
    assert_eq!(r.setting(), ClaimsReference::MachinedStock);
    // The dial said machined stock; the DETECTOR read the analytic field.
    // Recording both is the whole point — they disagree.
    assert_eq!(r.reference(), CreaseReference::SelfProbe);
    assert!(!r.prior_stock_in_scope());
    assert!(r.needs_attention(), "an unhonoured dial must be loud");
    assert!(
        r.why().contains("AwaitingPriorStock"),
        "the remedy runs through A/M11's ladder and must name it: {}",
        r.why()
    );

    let d = generation_diagnostics(&session, 1)
        .into_iter()
        .find(|d| d.id.as_str() == rs_cam_core::diagnostics::ids::CONFIG_CLAIMS_REFERENCE)
        .expect("an unhonoured dial must reach the diagnostics list");
    assert_eq!(d.severity, rs_cam_core::diagnostics::Severity::Caution);
}
