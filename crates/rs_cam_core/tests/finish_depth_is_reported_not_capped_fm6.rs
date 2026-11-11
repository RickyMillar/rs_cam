//! FM6 — a finishing depth is reported, not capped.
//!
//! Feeds matrix ruling R2 (2026-09-23,
//! `planning/feeds_matrix_2026-09-23/RULINGS.md`). The operator ruled: "I
//! want the most flexibility for the user". No vendor publishes an axial
//! finishing cap as a fraction of D for wood (EVIDENCE 5.1-3, 5.1-12), so:
//!
//! - (a) a finishing or semi-finishing pass gets no axial ceiling. Suggest
//!   ships its depth with no depth clamp warning, and the profile's rule
//!   `RigidityProfile::depth_cap_rule` returns `None` for it;
//! - (b) a roughing pass keeps the cap, at the producer's own cap (feeds
//!   contract P5: as the envelope's rigidity ceiling, which "Apply all"
//!   ships at or under);
//! - (c) on a tapered ball, Suggest and the post-simulation depth gate read
//!   ONE cap rule, and the gate reads the width at the engaged cone
//!   diameter (not the tip, not the shank). Before R2 the clamp multiplied
//!   the tip and the gate the shank (EVIDENCE 5.1-7, 3.5-15). Until P29-4
//!   the rule was the Adaptive factor x the engaged diameter; since P29-4
//!   (ruling R1) it is the force budget, from one producer
//!   (`force_budget::rigidity_depth_cap`);
//! - (d) the diagnostics list carries the depth row: an `Exceeds` as
//!   `load.depth.exceeds` (EVIDENCE 5.1-15), a finishing reading as
//!   `load.depth.reported` with no threshold, and a gate that did not
//!   measure as `load.depth.unmodeled`, never under a `*.within` id
//!   (EVIDENCE 5.1-14).
//!
//! Run: `scripts/cargo_lane.sh test -p rs_cam_core -q --test finish_depth_is_reported_not_capped_fm6`

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::cutter::build_cutter;
use rs_cam_core::compute::tool_config::ToolFact;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolMaterial, ToolType};
use rs_cam_core::diagnostics::adapters::from_tool_load::diagnostics_from_load_verdict;
use rs_cam_core::diagnostics::{DiagnosticEvidence, DiagnosticState, Severity, ids};
use rs_cam_core::feeds::envelope::{EnvelopeInput, LimitKind, PickRequest, SafeEnvelope};
use rs_cam_core::feeds::force_budget::{RigidityCapOutcome, rigidity_depth_cap};
use rs_cam_core::feeds::suggest::{
    OperationApplyInput, StockContext, SuggestContext, SuggestWarning, apply_to_operation,
};
use rs_cam_core::feeds::vendor_lut::{LutOperationFamily, LutPassRole};
use rs_cam_core::feeds::{EMBEDDED_LUT, OperationFamily, PassRole, SpindleStrategy};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::machine::{MachineProfile, RigidityDepthCap};
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::stock::simulation_cut::{
    CutKinematics, Engagement, SimulationCutSample, SimulationCutTrace,
};
use rs_cam_core::tool::{FlatEndmill, MillingCutter, ToolDefinition};
use rs_cam_core::tool_load::verdict::{DepthVerdict, LoadState};
use rs_cam_core::tool_load::{GateEnv, ToleranceBands, ToolpathLoadContext};

const TP: ToolpathId = ToolpathId::from_raw(0);

// ── fixtures (the FM1 instrument's cell shapes) ─────────────────────────

/// The `doc_finishing_factor` of the default profile when R2 removed the
/// finishing cap: the generic router's 0.08 at HEAD f5170da03
/// (`machine/mod.rs`, `generic_wood_router`). P11c made the default the
/// hobby grade set (0.10), so the reference is pinned here, not read from
/// today's default (on which the RampFinish arm sits at the cap).
const PRE_R2_DOC_FINISHING_FACTOR: f64 = 0.08;

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
        stock_z: Some(18.0),
        stock_padding: 2.0,
    }
}

/// The FM1 instrument's tool builder: a tapered ball at 7 deg with a
/// shank 3 mm over the tip (at least 6 mm).
fn tool_of(kind: ToolType, diameter: f64) -> ToolConfig {
    let mut t = ToolConfig::test_default(ToolId::from_raw(1), kind);
    t.diameter = diameter;
    t.set_flute_count(ToolFact::Stated(2));
    t.set_cutting_length(ToolFact::Stated((diameter * 3.0).max(12.0)));
    t.set_shank_diameter(ToolFact::Stated(diameter.max(3.0)));
    t.shaft_diameter = diameter.max(3.0);
    t.set_stickout(ToolFact::Stated(t.cutting_length_planning_mm() + 8.0));
    if matches!(kind, ToolType::TaperedBallNose) {
        t.set_taper_half_angle(ToolFact::Stated(7.0));
        t.set_shank_diameter(ToolFact::Stated((diameter + 3.0).max(6.0)));
        t.shaft_diameter = t.shank_diameter_planning_mm();
    }
    t
}

/// What "Apply all" ships for one cell, and the envelope it picked from.
struct Shipped {
    operation: OperationConfig,
    envelope: SafeEnvelope,
    warnings: Vec<SuggestWarning>,
}

/// Feeds contract P5 (K2b): "Apply all" (`Command::ApplyFeeds` with the
/// envelope proposal) is the production write of the depth. The op states
/// the depth the calculator chose before P5, the family default factor x D,
/// read from its F201 row (no number typed here), so a roughing cell states
/// a depth above its rigidity cap. Before P5 this file read the retired
/// chain door.
fn suggest(op: OperationType, tool: &ToolConfig) -> Shipped {
    suggest_at(op, tool, None)
}

/// [`suggest`] with the stated depth `depth_mm` in place of the family
/// default, when given.
fn suggest_at(op: OperationType, tool: &ToolConfig, depth_mm: Option<f64>) -> Shipped {
    let m = machine();
    let mat = softwood();
    let s = stock();
    let mut operation = rs_cam_core::feeds::suggest::default_operation(op, &s);
    // P10: the fixture states the geometry the registry held before P10;
    // the depth below replaces the depth per pass.
    rs_cam_core::compute::fixture_values::fill_unset_geometry(&mut operation);
    let (family, role) = operation.feeds_style();
    let row = format!("feeds.operation_default_profile.{family:?}.{role:?}.ap_factor");
    if let Some(factor) = rs_cam_core::feeds::formula_constants()
        .iter()
        .find(|c| c.name == row.as_str())
        .map(|c| c.value)
    {
        operation.set_depth_per_pass(tool.diameter * factor);
    }
    if let Some(depth) = depth_mm {
        let _ = operation.set_depth_per_pass(depth);
    }
    let context = SuggestContext {
        stock: Some(&s),
        ..SuggestContext::default()
    };
    let envelope = SafeEnvelope::for_cut(EnvelopeInput {
        operation: &operation,
        tool,
        machine: &m,
        material: &mat,
        lut: &EMBEDDED_LUT,
        spindle_strategy: SpindleStrategy::default(),
        context,
    })
    .unwrap_or_else(|e| panic!("{op:?} on {:?} must have an envelope: {e}", tool.tool_type));
    let mut provenance = rs_cam_core::feeds::FeedsProvenance::default();
    let warnings = apply_to_operation(
        OperationApplyInput {
            tool,
            machine: &m,
            material: &mat,
            spindle_strategy: SpindleStrategy::default(),
            lut: &EMBEDDED_LUT,
            suggest: context,
        },
        Some(PickRequest::Proposal),
        None,
        &mut operation,
        &mut provenance,
    )
    .unwrap_or_else(|e| panic!("{op:?} on {:?} must ship a recipe: {e}", tool.tool_type));
    Shipped {
        operation,
        envelope,
        warnings,
    }
}

/// The rigidity ceiling of the envelope at the op's stated width.
fn rigidity_cap(s: &Shipped) -> Option<f64> {
    let ae = s.operation.stepover().unwrap_or(0.0);
    s.envelope
        .limit(LimitKind::Rigidity)
        .and_then(|l| l.ap_ceiling_at(ae))
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
        steady_feed: rs_cam_core::machine::SteadyFeedMmMin::op_feed_only(1500.0),
        operation_kind: op,
        spans: None,
        drill_op: None,
        commanded_thinning_lift: None,
        radial_stock_to_leave_mm: None,
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
            m.rigidity.depth_cap_rule(family, pass_role).is_none(),
            "{op:?}: a {role:?} pass must carry no axial cap (R2)"
        );
        let dpp = s
            .operation
            .depth_per_pass()
            .unwrap_or_else(|| panic!("{op:?} carries a depth per pass"));
        let old_cap = PRE_R2_DOC_FINISHING_FACTOR * diameter;
        assert!(
            dpp > old_cap,
            "{op:?}: the fixture is vacuous unless the shipped depth {dpp} mm is \
             above the pre-R2 finishing cap {old_cap} mm"
        );
        assert_eq!(
            rigidity_cap(&s),
            None,
            "{op:?}: a finishing envelope must carry no rigidity ceiling"
        );
        assert!(
            !s.warnings
                .iter()
                .any(|w| matches!(w, SuggestWarning::OutsideEnvelope { .. })),
            "{op:?}: a finishing depth must not be reported outside: {:?}",
            s.warnings
        );
    }
}

// ── (b) a roughing depth still clamps ───────────────────────────────────

/// A Ø6 flat Pocket: the op states 4.2 mm, above the envelope's rigidity
/// ceiling. P13c (O1, O4): the ceiling is the force-budget depth (a curve
/// over the width; the producer equality is in
/// `the_roughing_depth_meets_the_force_budget_p13c`), and "Apply all" ships
/// the removal-rate pick inside the stated box, at or under the ceiling at
/// the width it ships.
#[test]
fn a_roughing_depth_still_clamps_at_the_producer_s_cap_fm6() {
    let tool = tool_of(ToolType::EndMill, 6.0);
    // P13c: the family default (4.2 mm) is under the force-budget depth in
    // softwood, so the arm states 2 x D.
    let s = suggest_at(OperationType::Pocket, &tool, Some(2.0 * tool.diameter));
    let l = s
        .envelope
        .limit(LimitKind::Rigidity)
        .expect("a roughing pocket envelope carries the rigidity cap");
    assert_eq!(l.law, rs_cam_core::feeds::force_budget::FORCE_BUDGET_LAW);
    let capped = rigidity_cap(&s).expect("a roughing pocket envelope carries the rigidity cap");
    let stated = s.envelope.stated.expect("the pocket states its cut").ap_mm;
    let shipped = s.operation.depth_per_pass().expect("a pocket has a depth");
    assert!(
        shipped <= capped + 1e-9,
        "the shipped depth {shipped} is above the cap {capped}"
    );
    assert!(
        stated > shipped,
        "vacuous: the stated {stated} is not lowered (shipped {shipped})"
    );
}

// ── (c) one engaged diameter on a tapered ball ──────────────────────────

/// The force-budget cap that the one producer gives the tapered ball
/// Adaptive at the gate's cut: the width share 0.4 of the engaged diameter
/// at `depth_mm` (the fixture `sample` states no arc) and the chip
/// feed / (rpm x Z) of the fixture sample.
fn producer_cap(cutter: &ToolDefinition, depth_mm: f64) -> Option<RigidityCapOutcome> {
    rigidity_depth_cap(
        &machine(),
        OperationFamily::Adaptive,
        PassRole::Roughing,
        cutter,
        &softwood(),
        0.4 * cutter.lookup_diameter_at(depth_mm),
        1500.0 / (18_000.0 * 2.0),
    )
}

/// A Ø3.175-tip tapered ball Adaptive (7 deg, Ø6.175 shank) in softwood.
/// P29-4 (ruling R1): the Adaptive family reads the force budget, so
/// Suggest and the gate read one cap rule, the budget. Before P29-4 both
/// read the factor `adaptive_doc_factor` x the engaged diameter; P29-4
/// deleted that rule. In
/// softwood the budget has no root below the flute on this tool (the FM1
/// tapered-ball softwood rows print an empty cap), so the envelope draws
/// no rigidity ceiling at the stated width, and the gate reports the peak
/// with no cap, as the producer says.
#[test]
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "test: an unexpected variant panics"
)]
fn suggest_and_the_gate_read_one_cap_rule_on_a_tapered_ball_fm6() {
    let tool = tool_of(ToolType::TaperedBallNose, 3.175);
    let s = suggest(OperationType::Adaptive, &tool);
    let limit = s
        .envelope
        .limit(LimitKind::Rigidity)
        .unwrap_or_else(|| panic!("the roughing envelope keeps the Rigidity limit"));
    assert_eq!(
        limit.law,
        rs_cam_core::feeds::force_budget::FORCE_BUDGET_LAW,
        "the Adaptive Rigidity limit is the force budget"
    );
    assert_eq!(
        rigidity_cap(&s),
        None,
        "no budget root below the flute: no ceiling at the stated width"
    );

    let cutter = build_cutter(&tool).unwrap();
    let shipped = s.operation.depth_per_pass().expect("an adaptive has a DPP");
    let t = trace(&[0.5 * shipped, shipped]);
    let producer = producer_cap(&cutter, shipped);
    assert!(
        matches!(producer, Some(RigidityCapOutcome::NoLimitBelowFlute)),
        "the producer: {producer:?}"
    );
    match depth_gate(&cutter, OperationType::Adaptive, Some(&t)) {
        DepthVerdict::Reported { peak_mm, .. } => assert_eq!(peak_mm, shipped),
        other => panic!("no budget root: the gate reports with no cap, got {other:?}"),
    }
}

/// In the cone of the same tapered ball (6 mm deep, Adaptive), the gate
/// reads the width at the engaged cone diameter. EVIDENCE 6-10: the
/// gate's x0.704 was the ladder at the cone diameter. P29-4 (ruling R1):
/// the cap is the force budget, solved at that width (before, the test
/// read the cone diameter off the Factor cap). At the fixture width share
/// 0.4 the budget has no root below the flute on this small tool, so this
/// arm cuts a full slot (share 1) in jarrah, where a cap forms: the gate's
/// cap is the producer's, and its width is the cone diameter.
#[test]
fn the_gate_reads_the_cone_diameter_fm6() {
    const DEPTH_MM: f64 = 6.0;
    let tool = tool_of(ToolType::TaperedBallNose, 3.175);
    let cutter = build_cutter(&tool).unwrap();
    let engaged = cutter.lookup_diameter_at(DEPTH_MM);
    assert!(
        engaged > tool.diameter + 0.1 && engaged < cutter.diameter() - 0.1,
        "vacuous: 6 mm deep must sit in the cone, between the tip {} and the shank {}: \
         {engaged}",
        tool.diameter,
        cutter.diameter()
    );
    let jarrah = Material::SolidWood {
        species: WoodSpecies::Jarrah,
    };
    let t = SimulationCutTrace {
        samples: vec![SimulationCutSample {
            engagement: Engagement::with_radial_woc(1.0),
            ..sample(0, DEPTH_MM)
        }],
        ..SimulationCutTrace::test_fixture()
    };
    let m = machine();
    let want = rigidity_depth_cap(
        &m,
        OperationFamily::Adaptive,
        PassRole::Roughing,
        &cutter,
        &jarrah,
        engaged,
        1500.0 / (18_000.0 * 2.0),
    );
    let tolerance = ToleranceBands::default();
    let env = GateEnv {
        sim_trace: Some(&t),
        machine: Some(&m),
        tolerance: &tolerance,
    };
    let gate = rs_cam_core::tool_load::depth::evaluate(
        &ctx(&cutter, &jarrah, OperationType::Adaptive),
        &env,
    );
    match (want, &gate) {
        (
            Some(RigidityCapOutcome::Cap(want)),
            DepthVerdict::Within { bound, .. } | DepthVerdict::Exceeds { bound, .. },
        ) => {
            assert_eq!(*bound, want, "the gate reads the producer's cap");
            let RigidityDepthCap::ForceBudget { ae_mm, .. } = *bound;
            assert!(
                (ae_mm - engaged).abs() < 1e-9,
                "the slot width {ae_mm} is the cone diameter {engaged}"
            );
        }
        (want, gate) => panic!("a cap at the cone width: producer {want:?}, gate {gate:?}"),
    }
}

// ── (d) the diagnostics list carries the depth row ──────────────────────

/// The roughing cap of the Ø6 Pocket at the fixture sample's cut: arc
/// pi/2 (ae = r = 3 mm) and 1500 mm/min at 18 000 rpm, 2 flutes.
fn pocket_cap() -> f64 {
    let cutter = flat6();
    match rs_cam_core::feeds::force_budget::rigidity_depth_cap(
        &machine(),
        OperationFamily::Pocket,
        PassRole::Roughing,
        &cutter,
        &softwood(),
        3.0,
        1500.0 / (18_000.0 * 2.0),
    ) {
        Some(rs_cam_core::feeds::force_budget::RigidityCapOutcome::Cap(c)) => c.cap_mm(),
        other => panic!("the fixture cut has a force-budget cap: {other:?}"),
    }
}

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

/// A Ø6 Pocket cut 1 mm deeper than its roughing cap: the diagnostics
/// list carries `load.depth.exceeds` at `Caution`, with the peak and the
/// cap as its evidence. P13c (O1): the cap is the force-budget depth at the
/// peak sample's cut (arc pi/2, the sample feed), from the one producer.
#[test]
fn a_depth_exceedance_reaches_the_diagnostics_list_fm6() {
    let cap = pocket_cap();
    let deep = cap + 1.0;
    let mut t = trace(&[1.0, deep]);
    for s in &mut t.samples {
        s.arc_engagement_radians = Some(std::f64::consts::FRAC_PI_2);
    }
    let v = load_verdict(OperationType::Pocket, Some(&t));
    assert!(v.depth.is_exceeded(), "fixture: {:?}", v.depth);
    let diags = diagnostics_from_load_verdict(&v);
    let d = diags
        .iter()
        .find(|d| d.id.as_str() == ids::LOAD_DEPTH_EXCEEDS)
        .unwrap_or_else(|| panic!("no depth Exceeds finding in {diags:?}"));
    assert_eq!(d.severity, Severity::Caution);
    assert_eq!(d.state, DiagnosticState::Current);
    match &d.evidence {
        Some(DiagnosticEvidence::SampleRange {
            observed,
            threshold,
            unit,
            ..
        }) => {
            assert_eq!(*observed, deep);
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

/// Ruling B6: plywood has no force line, so the power and deflection gates
/// are `Unmodeled` on a simulated plywood cut. Each finding has its own
/// `*.unmodeled` id, as the depth gate above. Before B6 both reported under
/// `load.power.within` and `load.deflection.within`, and a reader of ids
/// alone read "within" for a gate that measured nothing.
#[test]
fn an_unmodeled_power_or_deflection_gate_is_not_reported_as_within_b6() {
    use rs_cam_core::material::PlywoodGrade;
    let tool = flat6();
    let plywood = Material::Plywood {
        grade: PlywoodGrade::BalticBirch,
    };
    // The power and deflection gates read the swept arc; the depth
    // helper's samples carry none, so this trace sets it.
    let mut t = trace(&[1.0, 1.0]);
    for s in &mut t.samples {
        s.arc_engagement_radians = Some(std::f64::consts::FRAC_PI_2);
    }
    let v = rs_cam_core::tool_load::evaluate_toolpath(
        &ctx(&tool, &plywood, OperationType::Pocket),
        Some(&t),
        Some(&machine()),
        &ToleranceBands::default(),
    );
    // The cause is the material, not a missing input.
    for (gate, reason) in [
        ("power", format!("{:?}", v.power)),
        ("deflection", format!("{:?}", v.deflection)),
    ] {
        assert!(
            reason.contains("Unmodeled") && reason.contains("MaterialUnvalidated"),
            "the plywood {gate} gate: {reason}"
        );
    }
    let diags = diagnostics_from_load_verdict(&v);
    let ids_of = |prefix: &str| -> Vec<&str> {
        diags
            .iter()
            .map(|d| d.id.as_str())
            .filter(|id| id.starts_with(prefix))
            .collect()
    };
    assert_eq!(ids_of("load.power."), vec![ids::LOAD_POWER_UNMODELED]);
    assert_eq!(
        ids_of("load.deflection."),
        vec![ids::LOAD_DEFLECTION_UNMODELED]
    );
    // Non-vacuity: the same cut in softwood measures both gates.
    let wood = load_verdict(OperationType::Pocket, Some(&t));
    assert!(!wood.power.is_unmodeled(), "softwood: {:?}", wood.power);
    let wood_diags = diagnostics_from_load_verdict(&wood);
    assert!(
        wood_diags
            .iter()
            .any(|d| d.id.as_str() == ids::LOAD_POWER_WITHIN),
        "softwood power finding: {wood_diags:?}"
    );
}
