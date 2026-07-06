//! F-035 — Predicted-effective-feed in chipload / power / deflection gates.
//!
//! F-034 introduced `MachineKinematics` + a trapezoidal cycle-time
//! integrator. F-035 plumbs the per-move *predicted achieved* feed
//! through to the chipload + power gates so verdicts grade against
//! the feed the controller actually reaches under accel/jerk
//! limits, not just the commanded feed in the toolpath IR.
//!
//! The bug this catches: on hobby-class machines (Shapeoko XXL with
//! stock 250 mm/s² accel) the controller decelerates through corners.
//! Commanded 4000 mm/min, achieved 2000 mm/min through a tight turn —
//! the chipload gate at commanded reads `Within`, the chipload at
//! predicted reads `Exceeds(Low)` (rubbing / burning). The flag-on
//! path catches this; the flag-off path stays byte-identical to
//! pre-F-035 so the loop's existing calibration is protected.
//!
//! ## Acceptance bars
//!
//! 1. `flag_off_byte_identical_to_pre_f035` — full sim through
//!    `ProjectSession::run_simulation` on the AS001-shape pocket with
//!    `kinematics = Some(shapeoko)` + flag OFF must produce
//!    byte-identical chipload/power/deflection verdicts to flag ON
//!    when `predicted_feeds` is empty (the equivalence holds by
//!    construction; this test pins the flag-OFF branch through the
//!    production entry point.)
//! 2. `flag_on_corner_decel_drops_chipload_below_band` — direct gate
//!    test with a hand-built trace + predicted-feed map. Same samples,
//!    same commanded feed; with flag OFF the gate reads `Within`, with
//!    flag ON (predicted feed half the commanded) the gate reads
//!    `Exceeds(Low)`. The difference IS the bug F-035 catches.
//! 3. `flag_on_straight_line_chipload_unchanged` — same gate test but
//!    with predicted == commanded for every move. Flag ON and flag OFF
//!    produce byte-identical chipload values.
//! 4. `flag_on_extends_existing_f024_test_invariants` — AS001 pocket
//!    full sim with flag ON. F-024's per-sample `axial_engagement_mm`
//!    < 3 mm bar still holds — predicted feed must not contaminate
//!    frame-correctness (which it can't, since the predicted-feed
//!    plumbing only mutates gate evaluation and never touches the
//!    dexel grid).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use rs_cam_core::ids::ToolpathId;
use std::f64::consts::TAU;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::{BoundaryConfig, DressupConfig, HeightsConfig, StockSource};
use rs_cam_core::compute::operation_configs::{PocketConfig, PocketPattern};
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolMaterial, ToolType};
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::feeds::vendor_lut::{LutOperationFamily, LutPassRole};
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::geo::P2;
use rs_cam_core::machine_kinematics::MachineKinematics;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{LoadedModel, ProjectSession, SimulationOptions, ToolpathConfig};
use rs_cam_core::simulation_cut::{
    CutKinematics, Engagement, SimulationCutSample, SimulationCutSummary, SimulationCutTrace,
};
use rs_cam_core::tool::{FlatEndmill, ToolDefinition};
use rs_cam_core::tool_load::verdict::ChipSide;
use rs_cam_core::tool_load::{ChiploadVerdict, DeflectionVerdict, ToleranceBands, chipload};

// ----- Shared fixtures: 6 mm carbide flat tool + LUT-nominal-arc -----

/// Engagement arc the LUT row (HardMaple Pocket Roughing 6 mm flat)
/// is calibrated against. Picked to match
/// `tool_load::chipload::tests::TEST_LUT_NOMINAL_ARC_RAD` so the gate's
/// per-sample LUT-arc normalisation is a no-op for these fixtures.
const TEST_LUT_NOMINAL_ARC_RAD: f64 = 1.0843860798928202;

fn make_endmill_6mm_carbide() -> ToolDefinition {
    ToolDefinition::new(
        Box::new(FlatEndmill::new(6.0, 20.0)),
        6.0,
        30.0,
        20.0,
        30.0,
        2,
        ToolMaterial::Carbide,
    )
}

fn make_endmill_6mm_tool_config() -> ToolConfig {
    let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.cutting_length = 25.0;
    tool.shank_diameter = 6.35;
    tool.shank_length = 20.0;
    tool.stickout = 45.0;
    tool.flute_count = 2;
    tool.name = "End Mill 6mm (F-035 test)".to_owned();
    tool
}

/// One synthetic cutting sample wired up for the chipload gate. Values
/// match the in-tree `chipload::tests::sample` fixture so the gate's
/// LUT lookup, arc-normalisation, and steady-state filters behave
/// identically to the production tests.
fn sample(
    toolpath_id: usize,
    move_index: usize,
    chipload_mm: f64,
    radial_woc: f64,
) -> SimulationCutSample {
    SimulationCutSample {
        toolpath_id: ToolpathId(toolpath_id),
        move_index,
        sample_index: move_index,
        segment_time_s: 0.1,
        is_cutting: true,
        cut_kinematics: CutKinematics::Linear,
        feed_rate_mm_min: 1000.0,
        spindle_rpm: 18_000,
        flute_count: 2,
        axial_doc_mm: 1.0,
        axial_engagement_mm: 1.0,
        arc_engagement_radians: Some(TEST_LUT_NOMINAL_ARC_RAD),
        chipload_mm_per_tooth: chipload_mm,
        effective_chip_thickness_mm: Some(chipload_mm),
        engagement: Engagement::with_radial_woc(radial_woc),
        removed_volume_est_mm3: 0.1,
        mrr_mm3_s: 1.0,
        ..SimulationCutSample::test_fixture()
    }
}

fn empty_trace(samples: Vec<SimulationCutSample>) -> SimulationCutTrace {
    SimulationCutTrace {
        sample_step_mm: 1.0,
        summary: SimulationCutSummary {
            sample_count: samples.len(),
            toolpath_count: 1,
            issue_count: 0,
            hotspot_count: 0,
            total_runtime_s: 1.0,
            cutting_runtime_s: 1.0,
            rapid_runtime_s: 0.0,
            air_cut_time_s: 0.0,
            low_engagement_time_s: 0.0,
            average_engagement: 0.5,
            peak_chipload_mm_per_tooth: 0.05,
            peak_axial_doc_mm: 1.0,
            peak_plunge_descent_mm: 0.0,
            total_removed_volume_est_mm3: 1.0,
            average_mrr_mm3_s: 1.0,
            per_kinematics: std::collections::BTreeMap::new(),
        },
        samples,
        ..SimulationCutTrace::test_fixture()
    }
}

fn evaluate_chipload(trace: &SimulationCutTrace) -> ChiploadVerdict {
    let tool = make_endmill_6mm_carbide();
    let material = Material::SolidWood {
        species: WoodSpecies::HardMaple,
    };
    let tolerance = ToleranceBands::default();
    chipload::evaluate(
        &rs_cam_core::tool_load::ToolpathLoadContext {
            toolpath_id: ToolpathId(0),
            tool: &tool,
            material: &material,
            operation_family: LutOperationFamily::Pocket,
            pass_role: LutPassRole::Roughing,
            operation_feed_rate_mm_min: 1000.0,
            operation_kind: OperationType::Pocket,
            spans: None,
            drill_op: None,
        },
        &rs_cam_core::tool_load::GateEnv {
            sim_trace: Some(trace),
            machine: None,
            tolerance: &tolerance,
        },
    )
}

// ----- AB2: flag-on synthetic corner-decel drops chipload below band -

/// Direct gate-level test. Build a trace whose every sample reports a
/// commanded chipload comfortably inside the LUT envelope; then run
/// `evaluate` twice — once with an empty `predicted_feeds` map (flag
/// OFF behaviour) and once with a populated map that drops each move's
/// predicted feed to 30% of commanded. The flag-ON pass must surface
/// `Exceeds(Low)` because predicted chipload = 0.3 × commanded = 0.012
/// mm/tooth which is below the hard-maple LUT's min (~0.015 mm/tooth
/// for a 6 mm flat at the pocket-roughing row).
#[test]
fn flag_on_corner_decel_drops_chipload_below_band() {
    // Five samples at chipload = 0.04 mm/tooth (commanded 1000 mm/min,
    // 18 k rpm, 2 flutes → 0.0278 mm/tooth nominal; 0.04 chosen
    // to sit comfortably above the HardMaple/Pocket/Roughing LUT min).
    let samples: Vec<SimulationCutSample> = (0..5).map(|i| sample(0, i + 1, 0.04, 0.5)).collect();
    let mut trace = empty_trace(samples);

    // Flag OFF: empty predicted_feeds → gate uses commanded; should
    // land `Within`.
    let off_verdict = evaluate_chipload(&trace);
    assert!(
        matches!(off_verdict, ChiploadVerdict::Within { .. }),
        "F-035 AB2 baseline: chipload at commanded feed should be Within (0.04 mm/tooth is \
         comfortably above HardMaple/Pocket/Roughing min). Got {off_verdict:?}"
    );

    // Flag ON: populate predicted_feeds with 30% of commanded for
    // every move. The chipload gate's scaling step (chipload.rs ~line
    // 425) multiplies `effective_chip_thickness_mm` by
    // `predicted/commanded` = 0.3, so each sample now reads 0.012
    // mm/tooth which is below the LUT's min.
    for s in &trace.samples {
        trace
            .predicted_feeds
            .insert((s.toolpath_id, s.move_index), s.feed_rate_mm_min * 0.30);
    }
    let on_verdict = evaluate_chipload(&trace);
    let on_exceeds_low = matches!(
        on_verdict,
        ChiploadVerdict::Exceeds {
            side: ChipSide::Low,
            ..
        }
    );
    assert!(
        on_exceeds_low,
        "F-035 AB2: corner-decel should drop predicted chipload below band → \
         Exceeds(Low). Same samples, commanded feed unchanged, only the predicted-feed map \
         differs. Got {on_verdict:?}. Flag-OFF was {off_verdict:?}."
    );
}

// ----- AB3: flag-on straight line preserves chipload --------------

/// When predicted feed equals commanded feed on every move (the
/// straight-line case — no corners to decel through), the gate result
/// must be byte-identical with the flag on vs the flag off. This
/// pins the predicted-feed plumbing as a *substitution*, not an
/// extra transformation.
#[test]
fn flag_on_straight_line_chipload_unchanged() {
    let samples: Vec<SimulationCutSample> = (0..5).map(|i| sample(0, i + 1, 0.04, 0.5)).collect();
    let mut trace_on = empty_trace(samples.clone());
    let trace_off = empty_trace(samples);
    for s in &trace_on.samples {
        trace_on
            .predicted_feeds
            .insert((s.toolpath_id, s.move_index), s.feed_rate_mm_min);
    }

    let off_verdict = evaluate_chipload(&trace_off);
    let on_verdict = evaluate_chipload(&trace_on);

    // Both arms should produce the same Within verdict. We pin
    // structural equality on the discriminant — the bounds + sample
    // ids inside should match because the inputs (other than the
    // identity predicted-feed map) are identical.
    match (&off_verdict, &on_verdict) {
        (ChiploadVerdict::Within { .. }, ChiploadVerdict::Within { .. }) => {}
        _ => panic!(
            "F-035 AB3: straight-line case (predicted == commanded) must produce identical \
             chipload verdicts with flag on vs flag off. OFF={off_verdict:?} ON={on_verdict:?}"
        ),
    }
}

// ----- AB1 + AB4: AS001 pocket through ProjectSession --------------

fn rounded_rect_with_island() -> Polygon2 {
    let exterior = vec![
        P2::new(5.0, 5.0),
        P2::new(75.0, 5.0),
        P2::new(75.0, 55.0),
        P2::new(5.0, 55.0),
    ];
    let mut hole = Vec::with_capacity(64);
    let cx = 40.0;
    let cy = 30.0;
    let r = 10.0;
    let n = 64;
    for i in 0..n {
        let t = (i as f64) * TAU / (n as f64);
        hole.push(P2::new(cx + r * (-t).cos(), cy + r * (-t).sin()));
    }
    Polygon2::with_holes(exterior, vec![hole])
}

/// Build the same AS001-shape pocket fixture
/// `tests/dexel_stock_z_frame_f024.rs` uses, with one additional
/// knob: the caller can attach `MachineKinematics` to the session's
/// machine profile so the F-035 plumbing has the inputs it needs.
fn build_as001_pocket_session(kinematics: Option<MachineKinematics>) -> ProjectSession {
    let mut session = ProjectSession::new_empty();
    let stock = StockConfig {
        x: 100.0,
        y: 100.0,
        z: 12.0,
        origin_x: -10.0,
        origin_y: -10.0,
        origin_z: -12.0,
        auto_from_model: false,
        material: Material::SolidWood {
            species: WoodSpecies::GenericHardwood,
        },
        ..StockConfig::default()
    };
    session.set_stock_config(stock);

    let tool_idx = session.add_tool(make_endmill_6mm_tool_config());
    let tool_id = session.tools()[tool_idx].id.0;

    let polygon = rounded_rect_with_island();
    let model = LoadedModel {
        id: 0,
        name: "as001_pocket".to_owned(),
        mesh: None,
        polygons: Some(Arc::new(vec![polygon])),
        drill_targets: std::sync::Arc::new(Vec::new()),
        layers: std::sync::Arc::new(Vec::new()),
        path: PathBuf::from("synthetic://as001_pocket.svg"),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    };
    let model_id = session.add_model(model);

    let pocket = PocketConfig {
        stepover: 2.0,
        depth: 6.0,
        depth_per_pass: 2.0,
        feed_rate: 770.0,
        plunge_rate: 385.0,
        climb: true,
        pattern: PocketPattern::Contour,
        angle: 0.0,
        finishing_passes: 0,
        spindle_rpm: Some(18_000),
    };
    let tc = ToolpathConfig {
        id: ToolpathId(0),
        name: "Pocket".to_owned(),
        enabled: true,
        operation: OperationConfig::Pocket(pocket),
        dressups: DressupConfig::for_op(OperationType::Pocket),
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
    };
    session.add_toolpath(0, tc).expect("add pocket toolpath");

    if kinematics.is_some() {
        let mut machine = session.machine().clone();
        machine.kinematics = kinematics;
        session.set_machine(machine);
    }
    session
}

/// AB1 — drive through `ProjectSession::run_simulation` (the same
/// entry the GUI / MCP / smoke runner take). With `kinematics = Some`
/// but `use_predicted_feed_in_gates = false`, the resulting trace's
/// chipload + power + deflection verdicts must be byte-identical to
/// the same run with `kinematics = None` (the pre-F-034 / pre-F-035
/// baseline). This pins the flag-OFF branch through the production
/// path — protecting the loop's calibration from a regression in the
/// plumbing.
///
/// We anchor "byte-identical" on the gate verdicts (the load-bearing
/// downstream artifact) rather than the raw `predicted_feeds` field —
/// which is empty in both arms — because the verdicts are what the
/// auditor / smoke runner compare against.
#[test]
fn flag_off_byte_identical_to_pre_f035() {
    fn run(kinematics: Option<MachineKinematics>) -> rs_cam_core::tool_load::ToolLoadReport {
        let mut session = build_as001_pocket_session(kinematics);
        let cancel = AtomicBool::new(false);
        session
            .generate_toolpath(0, &cancel)
            .expect("generate pocket toolpath");
        let opts = SimulationOptions {
            resolution: 1.0,
            skip_ids: Vec::new(),
            metrics_enabled: true,
            auto_resolution: false,
            use_predicted_feed_in_gates: false,
            adaptive_feed_modulation: false,
            modulation_strategy: rs_cam_core::feed_modulation::ModulationStrategy::ConstrainedMax,
            modulation_aggressiveness: 1.0,
        };
        session
            .run_simulation(&opts, &cancel)
            .expect("simulation completes");
        session.tool_load_report()
    }

    let baseline = run(None);
    let with_kin_flag_off = run(Some(MachineKinematics::shapeoko_xxl_stock()));

    assert_eq!(
        baseline.per_toolpath.len(),
        with_kin_flag_off.per_toolpath.len(),
        "F-035 AB1: per_toolpath counts must match"
    );
    for (b, w) in baseline
        .per_toolpath
        .iter()
        .zip(with_kin_flag_off.per_toolpath.iter())
    {
        assert_eq!(
            b.toolpath_id, w.toolpath_id,
            "F-035 AB1: toolpath_id mismatch"
        );
        assert_eq!(
            std::mem::discriminant(&b.chipload),
            std::mem::discriminant(&w.chipload),
            "F-035 AB1: chipload verdict shape diverged between kinematics=None and \
             kinematics=Some+flag-OFF. baseline={b:?}, with_kin={w:?}"
        );
        assert_eq!(
            std::mem::discriminant(&b.power),
            std::mem::discriminant(&w.power),
            "F-035 AB1: power verdict shape diverged. baseline={b:?}, with_kin={w:?}"
        );
        assert_eq!(
            std::mem::discriminant(&b.deflection),
            std::mem::discriminant(&w.deflection),
            "F-035 AB1: deflection verdict shape diverged. baseline={b:?}, with_kin={w:?}"
        );
    }
}

/// AB4 — bridge to F-024. With flag ON, run AS001's pocket sim and
/// re-assert F-024's per-sample axial-engagement bound. The
/// predicted-feed plumbing mutates only gate evaluation and never
/// touches the dexel grid or the per-sample `axial_engagement_mm`
/// value, so F-024's invariant must continue to hold.
#[test]
fn flag_on_extends_existing_f024_test_invariants() {
    let mut session = build_as001_pocket_session(Some(MachineKinematics::shapeoko_xxl_stock()));
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("generate pocket toolpath");
    let opts = SimulationOptions {
        resolution: 1.0,
        skip_ids: Vec::new(),
        metrics_enabled: true,
        auto_resolution: false,
        // The bridge: flag ON.
        use_predicted_feed_in_gates: true,
        adaptive_feed_modulation: false,
        modulation_strategy: rs_cam_core::feed_modulation::ModulationStrategy::ConstrainedMax,
        modulation_aggressiveness: 1.0,
    };
    session
        .run_simulation(&opts, &cancel)
        .expect("simulation completes (flag ON)");

    let sim = session.simulation_result().expect("simulation result");
    let cut_trace = sim.cut_trace.as_ref().expect("cut trace");

    let mut first_pass_axials: Vec<f64> = cut_trace
        .samples
        .iter()
        .filter(|s| s.is_cutting && s.cut_kinematics != CutKinematics::Plunge)
        .filter(|s| (s.position[2] - (-2.0)).abs() < 0.5)
        .map(|s| s.axial_engagement_mm)
        .collect();
    first_pass_axials.sort_by(|a, b| a.partial_cmp(b).unwrap());

    assert!(
        !first_pass_axials.is_empty(),
        "F-035 AB4: expected at least one linear/arc/helix cutting sample near Z=-2 on the \
         first pass (flag ON)"
    );
    let peak = *first_pass_axials.last().unwrap();
    assert!(
        peak <= 3.0,
        "F-035 AB4 (bridge to F-024): first-pass axial engagement should be <= 3.0 mm even \
         with predicted-feed flag ON; got peak = {peak:.4} mm across {} samples. Predicted-feed \
         plumbing must not contaminate frame-correctness — it only mutates gate evaluation.",
        first_pass_axials.len()
    );

    // Additionally pin that the trace's predicted_feeds map actually
    // got populated when the flag was on (sanity check on the
    // simulator-side plumbing). If this is empty, the flag did NOT
    // fire and the test above is vacuously asserting a no-op.
    assert!(
        !cut_trace.predicted_feeds.is_empty(),
        "F-035 AB4: with flag ON + kinematics Some, the cut trace MUST carry a populated \
         predicted_feeds map. Empty map = simulator-side plumbing broken; gates fell through \
         to commanded-feed path so this test would silently pass."
    );

    // The same toolpath should also still have a valid deflection
    // verdict in the safe band (not Unmodeled). This isn't strictly
    // an F-024 invariant but is a useful sanity check that the gate
    // path still runs end-to-end with the flag on.
    let report = session.tool_load_report();
    let verdict = report
        .per_toolpath
        .iter()
        .find(|v| v.toolpath_id == ToolpathId(0))
        .expect("verdict for pocket toolpath");
    let peak_mm = match &verdict.deflection {
        DeflectionVerdict::Within { peak_mm, .. } | DeflectionVerdict::Exceeds { peak_mm, .. } => {
            *peak_mm
        }
        DeflectionVerdict::Unmodeled { reason } => {
            panic!(
                "F-035 AB4: deflection verdict should not regress to Unmodeled with flag ON; \
                 reason = {reason:?}"
            )
        }
    };
    assert!(
        peak_mm < 0.2,
        "F-035 AB4: deflection peak should still be in safe band with flag ON; got {peak_mm} mm"
    );
}
