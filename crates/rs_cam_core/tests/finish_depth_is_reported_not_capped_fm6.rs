//! FM6 — a finishing depth is reported, not capped.
//!
//! Feeds matrix ruling R2 (2026-09-23,
//! `planning/feeds_matrix_2026-09-23/RULINGS.md`). The operator ruled: "I
//! want the most flexibility for the user". No vendor publishes an axial
//! finishing cap as a fraction of D for wood (EVIDENCE 5.1-3, 5.1-12), so:
//!
//! - (a) a finishing or semi-finishing pass gets no axial ceiling. Suggest
//!   ships its depth with no depth clamp warning, and the one cap producer
//!   `RigidityProfile::depth_cap_mm` returns `None` for it;
//! - (b) a roughing pass keeps the clamp, at the producer's own cap;
//! - (c) on a tapered ball, Suggest and the post-simulation depth gate read
//!   ONE diameter at the shipped depth: `feeds::geometry::depth_cap_diameter_mm`,
//!   the engaged diameter (not the tip, not the shank). Before R2 the clamp
//!   multiplied the tip and the gate the shank (EVIDENCE 5.1-7, 3.5-15). The
//!   feed's depth ladder reads the same engaged diameter as the gate's
//!   chipload ladder (EVIDENCE 6-10);
//! - (d) the diagnostics list carries the depth row: an `Exceeds` as
//!   `load.depth.exceeds` (EVIDENCE 5.1-15), a finishing reading as
//!   `load.depth.reported` with no threshold, and a gate that did not
//!   measure as `load.depth.unmodeled`, never under a `*.within` id
//!   (EVIDENCE 5.1-14).
//!
//! Run: `scripts/cargo_lane.sh test -p rs_cam_core -q --test finish_depth_is_reported_not_capped_fm6`

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::cutter::build_cutter;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolMaterial, ToolType};
use rs_cam_core::diagnostics::adapters::from_tool_load::diagnostics_from_load_verdict;
use rs_cam_core::diagnostics::{DiagnosticEvidence, DiagnosticState, Severity, ids};
use rs_cam_core::feeds::geometry::{depth_cap_diameter_mm, feed_ladder_diameter_mm};
use rs_cam_core::feeds::suggest::{
    StockContext, SuggestContext, SuggestParamsInput, SuggestWarning, SuggestedParams,
    suggest_params,
};
use rs_cam_core::feeds::vendor_lut::{LutOperationFamily, LutPassRole};
use rs_cam_core::feeds::{
    EMBEDDED_LUT, OperationFamily, PassRole, SpindleStrategy, WorkholdingRigidity,
};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::stock::simulation_cut::{
    CutKinematics, Engagement, SimulationCutSample, SimulationCutTrace,
};
use rs_cam_core::tool::{FlatEndmill, MillingCutter, ToolDefinition};
use rs_cam_core::tool_load::verdict::{DepthVerdict, LoadState};
use rs_cam_core::tool_load::{GateEnv, ToleranceBands, ToolpathLoadContext};

const TP: ToolpathId = ToolpathId(0);

// ── fixtures (the FM1 instrument's cell shapes) ─────────────────────────

fn machine() -> MachineProfile {
    MachineProfile::default()
}

fn softwood() -> Material {
    Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    }
}

fn stock() -> StockContext {
    StockContext {
        stock_top_z: 0.0,
        stock_bottom_z: -18.0,
        stock_z: 18.0,
        stock_padding: 2.0,
    }
}

/// The FM1 instrument's tool builder: a tapered ball at 7 deg with a
/// shank 3 mm over the tip (at least 6 mm).
fn tool_of(kind: ToolType, diameter: f64) -> ToolConfig {
    let mut t = ToolConfig::new_default(ToolId(1), kind);
    t.diameter = diameter;
    t.flute_count = 2;
    t.cutting_length = (diameter * 3.0).max(12.0);
    t.shank_diameter = diameter.max(3.0);
    t.shaft_diameter = diameter.max(3.0);
    t.stickout = t.cutting_length + 8.0;
    if matches!(kind, ToolType::TaperedBallNose) {
        t.taper_half_angle = 7.0;
        t.shank_diameter = (diameter + 3.0).max(6.0);
        t.shaft_diameter = t.shank_diameter;
    }
    t
}

fn suggest(op: OperationType, tool: &ToolConfig) -> SuggestedParams {
    let m = machine();
    let mat = softwood();
    let s = stock();
    suggest_params(SuggestParamsInput {
        op_type: op,
        tool,
        machine: &m,
        material: &mat,
        workholding: WorkholdingRigidity::Medium,
        lut: &EMBEDDED_LUT,
        stock_ctx: &s,
        spindle_strategy: SpindleStrategy::default(),
        context: SuggestContext::default(),
    })
    .unwrap_or_else(|e| panic!("{op:?} on {:?} must ship a recipe: {e}", tool.tool_type))
}

fn is_depth_clamp(w: &SuggestWarning) -> bool {
    matches!(
        w,
        SuggestWarning::RoughingDepthClampedToRigidity { .. }
            | SuggestWarning::DepthClampedToCuttingLength { .. }
            | SuggestWarning::AxialDocClampedByEnvelope { .. }
    )
}

fn sample(idx: usize, depth: f64) -> SimulationCutSample {
    SimulationCutSample {
        toolpath_id: TP,
        move_index: idx,
        sample_index: idx,
        is_cutting: true,
        cut_kinematics: CutKinematics::Linear,
        feed_rate_mm_min: 1500.0,
        spindle_rpm: 18_000,
        flute_count: 2,
        axial_doc_mm: 0.0,
        axial_engagement_mm: depth,
        engagement: Engagement::with_radial_woc(0.4),
        in_transit_span: false,
        ..SimulationCutSample::test_fixture()
    }
}

fn trace(depths: &[f64]) -> SimulationCutTrace {
    SimulationCutTrace {
        samples: depths
            .iter()
            .enumerate()
            .map(|(i, d)| sample(i, *d))
            .collect(),
        ..SimulationCutTrace::test_fixture()
    }
}

fn flat6() -> ToolDefinition {
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

fn ctx<'a>(
    tool: &'a ToolDefinition,
    material: &'a Material,
    op: OperationType,
) -> ToolpathLoadContext<'a> {
    let spec = op.spec();
    ToolpathLoadContext {
        toolpath_id: TP,
        tool,
        material,
        operation_family: LutOperationFamily::Pocket,
        pass_role: match spec.feeds_pass_role {
            PassRole::Roughing => LutPassRole::Roughing,
            PassRole::SemiFinish => LutPassRole::SemiFinish,
            PassRole::Finish => LutPassRole::Finish,
        },
        operation_feed_rate_mm_min: 1500.0,
        operation_kind: op,
        spans: None,
        drill_op: None,
    }
}

fn depth_gate(
    tool: &ToolDefinition,
    op: OperationType,
    t: Option<&SimulationCutTrace>,
) -> DepthVerdict {
    let material = softwood();
    let m = machine();
    let tolerance = ToleranceBands::default();
    let env = GateEnv {
        sim_trace: t,
        machine: Some(&m),
        tolerance: &tolerance,
    };
    rs_cam_core::tool_load::depth::evaluate(&ctx(tool, &material, op), &env)
}

// ── (a) a finishing depth ships with no ceiling ─────────────────────────

/// Two cells whose shipped depth is above the pre-R2 finishing cap
/// (`doc_finishing_factor` × D, 0.08 × D on the default profile): a ball
/// RampFinish (a `Finish` role, 0.5 mm on Ø3.175 against 0.254 mm) and a
/// flat Waterline (a `SemiFinish` role, 1.0 mm on Ø6 against 0.48 mm),
/// per `matrix_2026-09-23.csv`. Each ships its depth, with no depth clamp
/// warning, and the cap producer gives it no cap.
#[test]
fn a_finishing_depth_ships_above_the_old_cap_with_no_clamp_fm6() {
    let m = machine();
    for (op, kind, diameter, role) in [
        (
            OperationType::RampFinish,
            ToolType::BallNose,
            3.175,
            PassRole::Finish,
        ),
        (
            OperationType::Waterline,
            ToolType::EndMill,
            6.0,
            PassRole::SemiFinish,
        ),
    ] {
        let tool = tool_of(kind, diameter);
        let s = suggest(op, &tool);
        let (family, pass_role) = s.operation.feeds_style();
        assert_eq!(pass_role, role, "{op:?}: the registry role moved");
        assert!(
            m.rigidity
                .depth_cap_mm(family, pass_role, diameter)
                .is_none(),
            "{op:?}: a {role:?} pass must carry no axial cap (R2)"
        );
        let dpp = s
            .operation
            .depth_per_pass()
            .unwrap_or_else(|| panic!("{op:?} carries a depth per pass"));
        let old_cap = m.rigidity.doc_finishing_factor * diameter;
        assert!(
            dpp > old_cap,
            "{op:?}: the fixture is vacuous unless the shipped depth {dpp} mm is \
             above the pre-R2 finishing cap {old_cap} mm"
        );
        let clamps: Vec<&SuggestWarning> =
            s.warnings.iter().filter(|w| is_depth_clamp(w)).collect();
        assert!(
            clamps.is_empty(),
            "{op:?}: a finishing depth must not be clamped, got {clamps:?}"
        );
    }
}

// ── (b) a roughing depth still clamps ───────────────────────────────────

/// A Ø6 flat Pocket: the calculator asks for 4.2 mm, the clamp lowers it
/// to the producer's own roughing cap, 0.2 × 6 = 1.2 mm on the default
/// profile, bit for bit.
#[test]
fn a_roughing_depth_still_clamps_at_the_producer_s_cap_fm6() {
    let m = machine();
    let tool = tool_of(ToolType::EndMill, 6.0);
    let s = suggest(OperationType::Pocket, &tool);
    let expected = m
        .rigidity
        .depth_cap_mm(OperationFamily::Pocket, PassRole::Roughing, 6.0)
        .expect("a roughing pocket carries an axial cap")
        .cap_mm();
    let (requested, capped) = s
        .warnings
        .iter()
        .find_map(|w| match w {
            SuggestWarning::RoughingDepthClampedToRigidity { requested, capped } => {
                Some((*requested, *capped))
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("the roughing clamp must fire, got {:?}", s.warnings));
    assert!(
        requested > expected,
        "vacuous: {requested} is not above {expected}"
    );
    assert_eq!(
        capped, expected,
        "the clamp must land on the producer's cap"
    );
    // Ruling R4 WP3 (2026-09-24): the clamp still lands on the cap, and
    // then the aggressiveness dial (default 0.85, x 0.75 long-tool share at
    // the default 45 mm stickout = 0.6375) scales the depth down from it.
    // Measured: 1.2 -> 0.8333 mm. The dial record starts from the cap, so
    // the difference is on the card, not hidden.
    let shipped = s.operation.depth_per_pass().expect("a pocket has a depth");
    assert!(
        shipped <= expected + 1e-12,
        "the shipped depth {shipped} is above the cap {expected}"
    );
    let dial_from = s.warnings.iter().find_map(|w| match w {
        SuggestWarning::EngagementReducedForAggressiveness {
            dpp_from, dpp_to, ..
        } => Some((*dpp_from, *dpp_to)),
        _ => None,
    });
    assert_eq!(
        dial_from,
        Some((Some(expected), Some(shipped))),
        "the dial record must start from the cap and end at the shipped depth"
    );
}

// ── (c) one engaged diameter on a tapered ball ──────────────────────────

/// A Ø3.175-tip tapered ball Pocket (7 deg, Ø6.175 shank). The clamp
/// fires. Suggest's cap at the depth it ships and the gate's cap at a
/// measured peak equal to that depth are one number, from one diameter:
/// `depth_cap_diameter_mm`, the engaged diameter at that depth. It is
/// neither the tip nor the shank.
#[test]
fn suggest_and_the_gate_read_one_engaged_diameter_on_a_tapered_ball_fm6() {
    let m = machine();
    let tool = tool_of(ToolType::TaperedBallNose, 3.175);
    let s = suggest(OperationType::Pocket, &tool);
    let capped = s
        .warnings
        .iter()
        .find_map(|w| match w {
            SuggestWarning::RoughingDepthClampedToRigidity { capped, .. } => Some(*capped),
            _ => None,
        })
        .unwrap_or_else(|| panic!("the roughing clamp must fire, got {:?}", s.warnings));
    let shipped = s.operation.depth_per_pass().expect("a pocket has a DPP");
    assert!(
        shipped <= capped,
        "the shipped depth {shipped} must be at or under the clamp {capped}"
    );

    let cutter = build_cutter(&tool);
    let suggest_d = depth_cap_diameter_mm(&cutter, capped);
    let factor = m.rigidity.doc_roughing_factor;
    assert!(
        (capped - factor * suggest_d).abs() <= 1e-9,
        "the clamp must land where the depth equals its own cap: {capped} vs \
         {factor} x {suggest_d}"
    );
    assert!(
        (suggest_d - tool.diameter).abs() > 0.1 && (suggest_d - cutter.diameter()).abs() > 0.1,
        "vacuous: the engaged diameter {suggest_d} must differ from the tip {} and the \
         shank {}",
        tool.diameter,
        cutter.diameter()
    );

    let t = trace(&[0.5 * capped, capped]);
    match depth_gate(&cutter, OperationType::Pocket, Some(&t)) {
        DepthVerdict::Within { peak_mm, bound, .. } => {
            assert_eq!(peak_mm, capped);
            assert_eq!(
                bound.diameter_mm, suggest_d,
                "the gate and the clamp must read one diameter at one depth"
            );
            assert_eq!(bound.factor, factor);
        }
        other => panic!("the shipped depth must read Within its own cap, got {other:?}"),
    }
}

/// In the cone of the same tapered ball (6 mm deep, Adaptive), the gate's
/// cap diameter is the engaged cone diameter, and the feed's depth ladder
/// divides by the same diameter (`feed_ladder_diameter_mm`, the parity
/// twin of `lookup_diameter_at`). EVIDENCE 6-10: the gate's ×0.704 was the
/// ladder at the cone diameter; the feed now reads that diameter too.
#[test]
fn the_feed_ladder_and_the_gate_read_the_cone_diameter_fm6() {
    const DEPTH_MM: f64 = 6.0;
    let tool = tool_of(ToolType::TaperedBallNose, 3.175);
    let cutter = build_cutter(&tool);
    let t = trace(&[DEPTH_MM]);
    let bound = match depth_gate(&cutter, OperationType::Adaptive, Some(&t)) {
        DepthVerdict::Within { bound, .. } | DepthVerdict::Exceeds { bound, .. } => bound,
        other => panic!("an adaptive pass carries a cap, got {other:?}"),
    };
    let engaged = cutter.lookup_diameter_at(DEPTH_MM);
    assert_eq!(bound.diameter_mm, engaged);
    let ladder = feed_ladder_diameter_mm(
        cutter.geometry_hint(),
        DEPTH_MM,
        tool.diameter,
        tool.shank_diameter,
    );
    assert!(
        (ladder - engaged).abs() < 1e-9,
        "the feed ladder diameter {ladder} and the gate diameter {engaged} must agree"
    );
    assert!(
        engaged > tool.diameter + 0.1 && engaged < cutter.diameter() - 0.1,
        "vacuous: 6 mm deep must sit in the cone, between the tip {} and the shank {}: \
         {engaged}",
        tool.diameter,
        cutter.diameter()
    );
}

// ── (d) the diagnostics list carries the depth row ──────────────────────

fn load_verdict(
    op: OperationType,
    t: Option<&SimulationCutTrace>,
) -> rs_cam_core::tool_load::verdict::ToolpathLoadVerdict {
    let tool = flat6();
    let material = softwood();
    let m = machine();
    let tolerance = ToleranceBands::default();
    rs_cam_core::tool_load::evaluate_toolpath(&ctx(&tool, &material, op), t, Some(&m), &tolerance)
}

/// A Ø6 Pocket cut 3 mm deep against the 1.2 mm roughing cap: the
/// diagnostics list carries `load.depth.exceeds` at `Caution`, with the
/// peak and the cap as its evidence.
#[test]
fn a_depth_exceedance_reaches_the_diagnostics_list_fm6() {
    let t = trace(&[1.0, 3.0]);
    let v = load_verdict(OperationType::Pocket, Some(&t));
    assert!(v.depth.is_exceeded(), "fixture: {:?}", v.depth);
    let diags = diagnostics_from_load_verdict(&v);
    let d = diags
        .iter()
        .find(|d| d.id.as_str() == ids::LOAD_DEPTH_EXCEEDS)
        .unwrap_or_else(|| panic!("no depth Exceeds finding in {diags:?}"));
    assert_eq!(d.severity, Severity::Caution);
    assert_eq!(d.state, DiagnosticState::Current);
    let cap = machine()
        .rigidity
        .depth_cap_mm(OperationFamily::Pocket, PassRole::Roughing, 6.0)
        .unwrap()
        .cap_mm();
    match &d.evidence {
        Some(DiagnosticEvidence::SampleRange {
            observed,
            threshold,
            unit,
            ..
        }) => {
            assert_eq!(*observed, 3.0);
            assert_eq!(*threshold, Some(cap));
            assert_eq!(unit, "mm");
        }
        other => panic!("expected a SampleRange evidence, got {other:?}"),
    }
}

/// A finishing pass reads `load.depth.reported`, Info, with the peak and
/// no threshold.
#[test]
fn a_finishing_depth_is_reported_with_no_threshold_fm6() {
    let t = trace(&[0.4, 2.5]);
    let v = load_verdict(OperationType::Scallop, Some(&t));
    let status = v.depth.as_criterion_status();
    assert_eq!(status.state, LoadState::Within);
    assert_eq!(status.bound, None);
    let diags = diagnostics_from_load_verdict(&v);
    let d = diags
        .iter()
        .find(|d| d.id.as_str() == ids::LOAD_DEPTH_REPORTED)
        .unwrap_or_else(|| panic!("no depth Reported finding in {diags:?}"));
    assert_eq!(d.severity, Severity::Info);
    match &d.evidence {
        Some(DiagnosticEvidence::SampleRange {
            observed,
            threshold,
            ..
        }) => {
            assert_eq!(*observed, 2.5);
            assert_eq!(*threshold, None);
        }
        other => panic!("expected a SampleRange evidence, got {other:?}"),
    }
}

/// With no simulation the depth gate is `Unmodeled`. Its finding has its
/// own id, and no depth finding carries a `*.within` id.
#[test]
fn an_unmodeled_depth_gate_is_not_reported_as_within_fm6() {
    let v = load_verdict(OperationType::Pocket, None);
    assert!(v.depth.is_unmodeled(), "fixture: {:?}", v.depth);
    let diags = diagnostics_from_load_verdict(&v);
    let d = diags
        .iter()
        .find(|d| d.id.as_str() == ids::LOAD_DEPTH_UNMODELED)
        .unwrap_or_else(|| panic!("no depth Unmodeled finding in {diags:?}"));
    assert_eq!(d.state, DiagnosticState::NeedsSimulation);
    let depth_ids: Vec<&str> = diags
        .iter()
        .map(|d| d.id.as_str())
        .filter(|id| id.starts_with("load.depth."))
        .collect();
    assert_eq!(depth_ids, vec![ids::LOAD_DEPTH_UNMODELED]);
    assert!(!ids::LOAD_DEPTH_UNMODELED.ends_with(".within"));
}
