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
//! (`65fe5375`): `OptimizeOutcome` has no `assumptions` field and none of the
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
//! The one thing that costs a sim — whether a candidate actually
//! reconciles — had its own fixture with the old retarget stage. That
//! fixture was deleted with the stage on 2026-10-12 (Phase 1 of
//! `planning/optimize_review_2026-10-12/PLAN.md`).
//!
//! # One operating point (operator ruling 2026-10-12)
//!
//! Phase 2 of the same plan (§2 step 4) closed the divergence this file
//! used to pin: the baseline and every candidate now simulate with the
//! options the GUI's simulation submit builds, the feed modulator on.
//! The stamp therefore carries ONE `OperatingPointStamp`. The
//! candidate/baseline split (`CandidateSimAssumptions`,
//! `BaselineTraceAssumptions`) is gone, and arm 5 now pins the agreement
//! instead of the divergence. The sentry that a candidate at the
//! baseline parameters measures the baseline is
//! `one_measure_for_baseline_and_candidate_g_opt6.rs`.

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
use rs_cam_core::feeds::vendor_lut::LutOperationFamily;
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::geo::P2;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::machine::kinematics::MachineKinematics;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{
    LoadedModel, ProjectSession, ProjectSessionBuilder, SimulationOptions, ToolpathConfig,
};
use rs_cam_core::stock::simulation_cut::SimulationCutTrace;
use rs_cam_core::tool_load::ModulationStrategyTag;
use rs_cam_core::tool_load::optimize::{
    KinematicsSource, LutQueryStamp, OutcomeKind, SimAssumptionStamp, optimize_toolpath,
};
use rs_cam_core::trace::debug_trace::ToolpathDebugOptions;

// ── Fixture ─────────────────────────────────────────────────────────────

/// The stamp is observed off the session and the caller's trace, and the
/// refusal path we ride never simulates — so the fixture only has to be
/// *well-formed*, not machinable.
///
/// `Material::Custom` is the refusal lever: `optimize_toolpath_inner` skips
/// on it at step 3, **after** building the evaluation context. That ordering
/// is what makes the LUT-routing half of the stamp observable for free.
fn session_with(op: OperationConfig, tool: ToolType, machine: MachineProfile) -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new();

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
    };
    builder = builder.stock(stock);
    builder = builder.machine(machine);

    let tool_idx = builder.add_tool(ToolConfig::new_default(ToolId(0), tool));
    let tool_id = builder.tools()[tool_idx].id.0;

    let square = Polygon2::new(vec![
        P2::new(-20.0, -20.0),
        P2::new(20.0, -20.0),
        P2::new(20.0, 20.0),
        P2::new(-20.0, 20.0),
    ]);
    let model_id = builder.add_model(LoadedModel {
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
        planner_origin: None,
    };
    let _ = builder.add_toolpath(0, cfg).expect("add toolpath");
    builder.build()
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
            feed_scale: 1.0,
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

    // The operating point: the cell and the modulation of the one set of
    // options.
    assert!(
        stamp.operating_point.resolution_mm > 0.0,
        "the cell must be named: {:?}",
        stamp.operating_point
    );
    assert_eq!(
        stamp.operating_point.modulation_strategy,
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
    assert_ne!(a.lut_query, b.lut_query, "the negotiated LUT query");
}

// ── 3. The routing the band was negotiated through ──────────────────────

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

// ── 4. One operating point ──────────────────────────────────────────────

/// **The ruling, pinned as a fact (operator, 2026-10-12).**
///
/// Until Phase 2 of `planning/optimize_review_2026-10-12/PLAN.md` this
/// arm pinned a divergence: the candidate sim ran with the modulator OFF
/// while `SimulationOptions::default()` and the GUI ran it ON. The doc of
/// the old arm said "when the ruling lands, this test fails loudly and is
/// the place the decision gets recorded". The ruling landed: the
/// optimizer simulates with the GUI's options. This arm now pins the
/// agreement: modulation as the library default, the predicted-feed flag
/// as the library default, and the cell the session stores.
#[test]
fn the_optimizer_scores_at_the_operating_point_of_the_project_simulation() {
    let session = session_with(pocket_op(), ToolType::EndMill, machine_without_kinematics());
    let (_, stamp) = stamp_for(
        pocket_op(),
        ToolType::EndMill,
        machine_without_kinematics(),
        &unmodulated_trace(),
    );

    let library_default = SimulationOptions::default();
    assert!(
        library_default.adaptive_feed_modulation,
        "Checkpoint J flipped the library default ON; if this is false the \
         world moved and this arm needs re-attributing"
    );
    assert_eq!(
        stamp.operating_point.adaptive_feed_modulation, library_default.adaptive_feed_modulation,
        "the optimizer simulates with the modulator the GUI runs"
    );
    assert_eq!(
        stamp.operating_point.use_predicted_feed_in_gates,
        library_default.use_predicted_feed_in_gates,
    );
    assert!(
        (stamp.operating_point.resolution_mm - session.simulation_resolution_mm()).abs() < 1e-12,
        "the cell is the one the project stores (G-RESTRES)"
    );
}
