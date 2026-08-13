//! **A-8 (F-OPT) — the optimizer's assumption stamp.**
//!
//! # The finding this pins
//!
//! The operator's 2026-08-07 feeds/speeds review, `[medium] The optimizer
//! initially evaluates a different feed world than production simulation`:
//!
//! > Candidate scoring deliberately sets both `use_predicted_feed_in_gates`
//! > and `adaptive_feed_modulation` to `false`. Normal simulation can enable
//! > either. […] "safe/faster candidate" means safe/faster in the unmodulated
//! > commanded-feed candidate model until reconciliation, not necessarily in
//! > the live emitted-feed model.
//!
//! Its recommended repair was explicitly **not** unification:
//!
//! > Keep the isolation for now, but make it explicit in every optimizer
//! > result: `candidate estimate — commanded feed, no feed modulation`.
//!
//! Nothing in `OptimizeOutcome` carried that. `MachineSnapshot` (F4.3) is the
//! only provenance the outcome had, and it records caps — no resolution, no
//! modulation state, no kinematics, no LUT routing.
//!
//! # Red-first
//!
//! Every assertion in this file is a **compile error** at the parent revision
//! (`1c0a4acc`): `OptimizeOutcome` has no `assumptions` field and none of the
//! stamp types exist. That is the strongest form of the red-first
//! reproduction — the claim cannot be made at all before the change, rather
//! than being made wrongly. Verified by `cargo check --test
//! optimizer_assumption_stamp_a8` against the parent; recorded in
//! `planning/review_2026-08-08/OPTIMIZER_ASSUMPTIONS.md` §5.
//!
//! # Why these fixtures cost no simulation
//!
//! `optimize_toolpath` stamps the assumptions **outside**
//! `optimize_toolpath_inner`, beside `MachineSnapshot`, so a refusal is
//! stamped too. `Material::Custom` refuses at step 3 — after the evaluation
//! context is built (so the LUT routing is observable) and before any
//! candidate is generated (so no sim runs). Every case here therefore
//! completes in milliseconds while exercising the real production entry
//! point, not a hand-built outcome.
//!
//! The one thing that costs a sim — whether a **retargeted** candidate
//! actually reconciles — is measured separately, in
//! `tool_load::optimize::retarget_reconciliation_a8`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{BoundaryConfig, DressupConfig, HeightsConfig, StockSource};
use rs_cam_core::compute::operation_configs::{Adaptive3dConfig, PocketConfig};
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::feeds::vendor_lut::LutOperationFamily;
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::geo::P2;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::machine_kinematics::MachineKinematics;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{LoadedModel, ProjectSession, SimulationOptions, ToolpathConfig};
use rs_cam_core::simulation_cut::SimulationCutTrace;
use rs_cam_core::tool_load::ModulationStrategyTag;
use rs_cam_core::tool_load::optimize::{
    KinematicsSource, LutQueryStamp, OutcomeKind, SimAssumptionStamp, optimize_toolpath,
};

// ── Fixture ─────────────────────────────────────────────────────────────

/// The stamp is observed off the session and the caller's trace, and the
/// refusal path we ride never simulates — so the fixture only has to be
/// *well-formed*, not machinable.
///
/// `Material::Custom` is the refusal lever: `optimize_toolpath_inner` skips
/// on it at step 3, **after** building the evaluation context. That ordering
/// is what makes the LUT-routing half of the stamp observable for free.
fn session_with(op: OperationConfig, tool: ToolType, machine: MachineProfile) -> ProjectSession {
    let mut session = ProjectSession::new_empty();

    let mut stock = StockConfig {
        x: 100.0,
        y: 100.0,
        z: 20.0,
        origin_x: -50.0,
        origin_y: -50.0,
        origin_z: -20.0,
        auto_from_model: false,
        ..StockConfig::default()
    };
    stock.material = rs_cam_core::material::Material::Custom {
        name: "a8 refusal lever".to_owned(),
        feed_scale_factor: 1.0,
        kc: 10.0,
    };
    session.set_stock_config(stock);
    session.set_machine(machine);

    let tool_idx = session.add_tool(ToolConfig::new_default(ToolId(0), tool));
    let tool_id = session.tools()[tool_idx].id.0;

    let square = Polygon2::new(vec![
        P2::new(-20.0, -20.0),
        P2::new(20.0, -20.0),
        P2::new(20.0, 20.0),
        P2::new(-20.0, 20.0),
    ]);
    let model_id = session.add_model(LoadedModel {
        id: 0,
        name: "a8 square".to_owned(),
        mesh: None,
        polygons: Some(Arc::new(vec![square])),
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        path: std::path::PathBuf::from("synthetic://a8"),
        kind: Some(ModelKind::Svg),
        units: Some(ModelUnits::Millimeters),
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    });

    let op_type = op.op_type();
    let cfg = ToolpathConfig {
        id: ToolpathId(0),
        name: "a8 op".to_owned(),
        enabled: true,
        operation: op,
        dressups: DressupConfig::for_op(op_type),
        heights: HeightsConfig::default(),
        tool_id,
        model_id,
        pre_gcode: None,
        post_gcode: None,
        boundary: BoundaryConfig::default(),
        boundary_inherit: true,
        stock_source: StockSource::Fresh,
        coolant: CoolantMode::Off,
        face_selection: None,
        debug_options: ToolpathDebugOptions::default(),
        feeds_provenance: rs_cam_core::feeds::FeedsProvenance::default(),
        rest_analysis: rs_cam_core::compute::config::RestAnalysisConfig::default(),
    };
    session.add_toolpath(0, cfg).expect("add toolpath");
    session
}

fn pocket_op() -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        depth: 4.0,
        depth_per_pass: 2.0,
        feed_rate: 1500.0,
        ..Default::default()
    })
}

fn adaptive3d_op() -> OperationConfig {
    OperationConfig::Adaptive3d(Adaptive3dConfig::default())
}

/// A machine with no `kinematics` — what every built-in preset ships.
fn machine_without_kinematics() -> MachineProfile {
    MachineProfile {
        kinematics: None,
        ..MachineProfile::generic_wood_router()
    }
}

/// A machine that declares its own kinematics.
fn machine_with_kinematics() -> MachineProfile {
    MachineProfile {
        kinematics: Some(MachineKinematics::generic_wood_router()),
        ..MachineProfile::generic_wood_router()
    }
}

/// A trace the modulator demonstrably ran on: `modulation_summaries` is
/// written by `apply_adaptive_feed_modulation` and by nothing else.
fn modulated_trace() -> SimulationCutTrace {
    let mut trace = SimulationCutTrace::test_fixture();
    trace.sample_step_mm = 0.25;
    trace.modulation_summaries.insert(
        ToolpathId(0),
        rs_cam_core::tool_load::ModulationSummary {
            moves_touched: 7,
            moves_total: 9,
            median_feed_delta_pct: -12.5,
            binding_constraint_distribution: std::collections::BTreeMap::new(),
            aggressiveness: 1.0,
            strategy: ModulationStrategyTag::ConstrainedMax,
        },
    );
    trace
}

fn unmodulated_trace() -> SimulationCutTrace {
    let mut trace = SimulationCutTrace::test_fixture();
    trace.sample_step_mm = 0.5;
    trace
}

fn stamp_for(
    op: OperationConfig,
    tool: ToolType,
    machine: MachineProfile,
    trace: &SimulationCutTrace,
) -> (OutcomeKind, SimAssumptionStamp) {
    let mut session = session_with(op, tool, machine);
    let cancel = AtomicBool::new(false);
    let outcome = optimize_toolpath(&mut session, trace, 0, &cancel);
    let stamp = outcome
        .assumptions
        .clone()
        .expect("every optimize_toolpath outcome carries an assumption stamp");
    (outcome.kind, stamp)
}

// ── 1. A result carries the stamp ───────────────────────────────────────

#[test]
fn an_optimizer_result_carries_its_simulation_assumptions() {
    let (kind, stamp) = stamp_for(
        pocket_op(),
        ToolType::EndMill,
        machine_without_kinematics(),
        &unmodulated_trace(),
    );

    // The refusal path — the point being that it is stamped too. A
    // refusal is also a claim taken at an operating point.
    assert_eq!(kind, OutcomeKind::Skipped);

    // Candidate block: the two resolutions and the two pinned flags.
    assert!(
        stamp.candidates.coarse_resolution_mm > 0.0 && stamp.candidates.refined_resolution_mm > 0.0,
        "both candidate cells must be named: {:?}",
        stamp.candidates
    );
    assert!(
        !stamp.candidates.auto_resolution,
        "the candidate sim pins auto_resolution off; the stamp must say so"
    );
    assert_eq!(
        stamp.candidates.modulation_strategy,
        ModulationStrategyTag::ConstrainedMax
    );

    // Kinematics source, LUT routing, boundary contract.
    assert_eq!(
        stamp.kinematics,
        KinematicsSource::GenericWoodRouterFallback
    );
    assert!(
        stamp.lut_query.is_some(),
        "a pocket + end mill forms a query"
    );
    assert!(
        stamp.boundary_epsilon_rel > 0.0,
        "the boundary contract in force must be recorded"
    );

    // Baseline block: honest about what the trace does not record.
    assert!(
        stamp.baseline.resolution_mm.is_none(),
        "SimulationCutTrace records no dexel cell; the stamp must report \
         NOT MEASURED rather than borrowing the candidate cell"
    );
    assert!((stamp.baseline.sample_step_mm - 0.5).abs() < 1e-12);
}

// ── 2. Two assumption sets, two stamps ──────────────────────────────────

#[test]
fn two_results_from_different_assumption_sets_carry_different_stamps() {
    let (_, a) = stamp_for(
        pocket_op(),
        ToolType::EndMill,
        machine_without_kinematics(),
        &unmodulated_trace(),
    );
    let (_, b) = stamp_for(
        adaptive3d_op(),
        ToolType::BallNose,
        machine_with_kinematics(),
        &modulated_trace(),
    );

    assert_ne!(a, b, "two different assumption sets must not stamp alike");

    // …and it must differ on each named axis independently, so the
    // inequality above cannot be carried by one field alone.
    assert_ne!(
        a.kinematics, b.kinematics,
        "kinematics source: declared vs generic-wood-router fallback"
    );
    assert_ne!(
        a.baseline.adaptive_feed_modulation, b.baseline.adaptive_feed_modulation,
        "baseline modulation evidence must separate a modulated trace from \
         one that records nothing"
    );
    assert_ne!(
        a.baseline.sample_step_mm, b.baseline.sample_step_mm,
        "baseline sampling scale"
    );
    assert_ne!(a.lut_query, b.lut_query, "the negotiated LUT query");
}

// ── 3. The baseline modulation evidence is one-sided, deliberately ──────

#[test]
fn baseline_modulation_is_observed_positively_or_not_at_all() {
    let (_, modulated) = stamp_for(
        pocket_op(),
        ToolType::EndMill,
        machine_without_kinematics(),
        &modulated_trace(),
    );
    let (_, unmodulated) = stamp_for(
        pocket_op(),
        ToolType::EndMill,
        machine_without_kinematics(),
        &unmodulated_trace(),
    );

    assert_eq!(
        modulated.baseline.adaptive_feed_modulation,
        Some(true),
        "modulation_summaries is written by apply_adaptive_feed_modulation \
         and nothing else — its presence is a positive observation"
    );
    assert_eq!(
        unmodulated.baseline.adaptive_feed_modulation, None,
        "an empty summaries map has two causes (modulation off, or a trace \
         round-tripped through serde where the map is skipped). The stamp \
         must report NOT MEASURED, never Some(false)"
    );
}

// ── 4. The routing the band was negotiated through ──────────────────────

#[test]
fn the_stamp_names_the_lut_family_the_band_was_negotiated_under() {
    let (_, pocket) = stamp_for(
        pocket_op(),
        ToolType::EndMill,
        machine_without_kinematics(),
        &unmodulated_trace(),
    );
    let (_, a3d) = stamp_for(
        adaptive3d_op(),
        ToolType::BallNose,
        machine_without_kinematics(),
        &unmodulated_trace(),
    );

    // A pocket asks for what it declares.
    match pocket.lut_query {
        Some(LutQueryStamp::Routed {
            declared_family,
            queried_family,
            ..
        }) => {
            assert_eq!(declared_family, LutOperationFamily::Pocket);
            assert_eq!(queried_family, LutOperationFamily::Pocket);
        }
        other => panic!("pocket should route to a query, got {other:?}"),
    }
    assert!(!pocket.lut_query.expect("pocket query").is_rerouted());

    // Adaptive3d declares `Adaptive` and is queried as `Pocket` —
    // Checkpoint K (a4)'s reroute, previously invisible on every
    // optimizer surface.
    match a3d.lut_query {
        Some(LutQueryStamp::Routed {
            declared_family,
            declared_pass_role,
            queried_family,
            queried_pass_role,
        }) => {
            assert_eq!(declared_family, LutOperationFamily::Adaptive);
            assert_eq!(queried_family, LutOperationFamily::Pocket);
            assert_eq!(
                declared_pass_role, queried_pass_role,
                "Adaptive3d's reroute moves the family and leaves the pass role"
            );
        }
        other => panic!("adaptive3d should route to a query, got {other:?}"),
    }
    assert!(
        a3d.lut_query.expect("a3d query").is_rerouted(),
        "the reroute must be legible as a reroute, not just as a family name"
    );
}

// ── 5. The divergence the stamp exists to disclose ──────────────────────

/// **This is the finding, pinned as a fact rather than as prose.**
///
/// Checkpoint J flipped `SimulationOptions::default().adaptive_feed_modulation`
/// to `true`; Checkpoint K (g1) flipped the CLI flag to match. The
/// optimizer's candidate sim still pins `false`. So the same project can be
/// judged `Within` by the optimizer and `Exceeds` by the GUI, or the reverse,
/// with nothing on either surface naming the difference.
///
/// A-8 did **not** flip the pin — that is number-moving on every optimizer
/// outcome and belongs to an operator ruling. This test asserts the
/// divergence *exists and is disclosed*. When the ruling lands, this test
/// fails loudly and is the place the decision gets recorded.
#[test]
fn the_optimizer_scores_candidates_at_a_different_operating_point_than_the_library_default() {
    let (_, stamp) = stamp_for(
        pocket_op(),
        ToolType::EndMill,
        machine_without_kinematics(),
        &unmodulated_trace(),
    );

    let library_default = SimulationOptions::default().adaptive_feed_modulation;
    assert!(
        library_default,
        "Checkpoint J flipped the library default ON; if this is false the \
         world moved and the divergence below needs re-attributing"
    );
    assert!(
        !stamp.candidates.adaptive_feed_modulation,
        "the optimizer's candidate sim pins modulation OFF (F-036b)"
    );
    assert!(
        stamp.candidates.diverges_from_library_default_modulation(),
        "and the stamp must say the two disagree — that disclosure is the \
         whole deliverable"
    );

    // The other pinned flag still agrees with the default, and the stamp
    // must not blur the two cases together.
    assert_eq!(
        stamp.candidates.use_predicted_feed_in_gates,
        SimulationOptions::default().use_predicted_feed_in_gates,
        "F-035's pin still tracks the library default; only F-036b's is stale"
    );
}
