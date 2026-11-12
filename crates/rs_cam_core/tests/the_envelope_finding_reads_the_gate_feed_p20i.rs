//! P20i (feeds contract, 2026-11-09): the envelope finding reads the gate
//! feed and the gate power figure.
//!
//! Defects (`planning/feeds_contract_2026-11-03/phases/P20g_wanaka.md` 4.1,
//! 4.2): `project.sim_outside_envelope` judged a measured sample against the
//! drawn Power edge, which `power_limit` draws before the simulation at the
//! op's STATED feed. The emitted per-move feed never reached it. The grid of
//! `get_safe_envelope` printed the planner-cap case (`min(1.6 ae, D)`) at the
//! nominal width with no name, so a fit of the grid read a second law.
//!
//! Claims:
//!
//! - (a) A sample that the power gate grades within at its emitted feed is
//!   not placed outside the envelope for power, although its depth is above
//!   the drawn Power edge (the stated feed is higher).
//! - (b) One engagement gives one kW from the safe-envelope grid, Suggest's
//!   door (`power_at_operating_point`) and the power gate
//!   (`sample_power_kw`), within 1e-9 relative. The grid point names the
//!   deliberate case and its width (`power_case`, `power_width_mm`): the
//!   planner cap of a 3D Rough under Geodesic Adaptive is the pass ceiling
//!   `c D`. P29-5b: a 3D Rough under Contour Parallel ships the slot of its
//!   perimeter sweep, so its planner cap is never the largest case; the
//!   claim reads the geodesic plan.
//! - (c) A sample above the power budget at its emitted feed is reported by
//!   the gate and by the envelope finding.
//!
//! Red at HEAD `5888b1344`: (a) the drawn edge places the sample outside for
//! power; (b) the grid point has no `power_case`.

#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::catalog::{OperationConfig, OperationType, PlannerWidth};
use rs_cam_core::compute::fixture_values::fixture_op;
use rs_cam_core::compute::operation_configs::ClearingStrategy;
use rs_cam_core::compute::tool_config::{CentreCutting, ToolConfig, ToolFact, ToolId, ToolType};
use rs_cam_core::feeds::envelope::{
    CutPoint, Edge, EnvelopeInput, LimitKind, Placement, SafeEnvelope,
};
use rs_cam_core::feeds::suggest::{CalculatorOperatingPoint, SuggestContext};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::machine::{MachineProfile, PowerModel};
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::stock::simulation_cut::{
    AIR_CUT_RADIAL_WOC_FRACTION, CutKinematics, Engagement, SimulationCutSample, SimulationCutTrace,
};
use rs_cam_core::tool::ToolDefinition;
use rs_cam_core::tool_load::verdict::PowerVerdict;
use rs_cam_core::tool_load::{GateEnv, ToleranceBands, ToolpathLoadContext, power};

const TP: ToolpathId = ToolpathId::from_raw(0);
/// The tool of the case: 6 mm, 2 flutes. The values are the case, not a
/// real tool.
const D: f64 = 6.0;
/// The stated op feed and RPM of claims (a) and (c): a high stated chip, so
/// the drawn Power edge sits low in the depth span.
const STATED_FEED: f64 = 12_000.0;
const RPM: u32 = 18_000;
/// The relative tolerance of claim (b).
const REL: f64 = 1e-9;

fn tool_config() -> ToolConfig {
    let mut tool = ToolConfig::test_default(ToolId::from_raw(0), ToolType::EndMill);
    tool.diameter = D;
    tool.set_cutting_length(ToolFact::Stated(25.0));
    tool.set_stickout(ToolFact::Stated(30.0));
    tool.set_centre_cutting(ToolFact::Stated(CentreCutting::Yes));
    tool
}

fn tool_definition() -> ToolDefinition {
    rs_cam_core::compute::cutter::build_cutter(&tool_config()).expect("a 6 mm end mill builds")
}

fn oak() -> Material {
    Material::SolidWood {
        species: WoodSpecies::WhiteOak,
    }
}

/// The rated spindle power of the case (kW): low, so the drawn Power edge
/// is the lowest ceiling at some width. A case value, not a machine.
const RATED_KW: f64 = 0.25;

fn machine() -> MachineProfile {
    let mut machine = MachineProfile::shapeoko_makita();
    machine.power = PowerModel::ConstantPower { power_kw: RATED_KW };
    machine
}

fn envelope_of(op: &OperationConfig) -> SafeEnvelope {
    let tool = tool_config();
    let machine = machine();
    let material = oak();
    SafeEnvelope::for_cut(EnvelopeInput {
        operation: op,
        tool: &tool,
        machine: &machine,
        material: &material,
        lut: rs_cam_core::feeds::embedded_vendor_lut(),
        spindle_strategy: rs_cam_core::feeds::SpindleStrategy::default(),
        context: SuggestContext::default(),
    })
    .expect("a 6 mm end mill in white oak is backed")
}

/// The pocket of claims (a) and (c), with the stated feed and RPM.
fn stated_pocket() -> OperationConfig {
    let mut op = fixture_op(OperationType::Pocket);
    op.set_feed_rate(STATED_FEED);
    op.set_spindle_rpm(Some(RPM));
    op
}

/// One steady cutting sample at depth `ap`, width `ae`, feed `feed`.
fn sample(idx: usize, ap: f64, ae: f64, rpm: u32, feed: f64) -> SimulationCutSample {
    let r = 0.5 * D;
    let arc = (1.0 - ae / r).acos();
    let flutes = tool_definition().flute_count;
    let fz = feed / (f64::from(rpm) * f64::from(flutes));
    SimulationCutSample {
        toolpath_id: TP,
        move_index: idx,
        sample_index: idx,
        position: [f64::from(u32::try_from(idx).expect("small")), 0.0, -ap],
        cumulative_time_s: 0.1 * f64::from(u32::try_from(idx).expect("small")),
        segment_time_s: 0.1,
        is_cutting: true,
        cut_kinematics: CutKinematics::Linear,
        feed_rate_mm_min: feed,
        spindle_rpm: rpm,
        flute_count: flutes,
        axial_doc_mm: ap,
        axial_engagement_mm: ap,
        arc_engagement_radians: Some(arc),
        chipload_mm_per_tooth: fz,
        effective_chip_thickness_mm: Some(fz),
        engagement: Engagement::with_radial_woc(ae / D),
        removed_volume_est_mm3: 0.1,
        mrr_mm3_s: 1.0,
        ..SimulationCutSample::test_fixture()
    }
}

fn trace_of(samples: Vec<SimulationCutSample>) -> SimulationCutTrace {
    SimulationCutTrace {
        samples,
        sample_step_mm: 1.0,
        ..SimulationCutTrace::test_fixture()
    }
}

/// The gate context of the stated pocket.
fn with_ctx<R>(f: impl FnOnce(&ToolpathLoadContext<'_>) -> R) -> R {
    let tool = tool_definition();
    let material = oak();
    let machine = machine();
    let ctx = ToolpathLoadContext::of_operation(
        TP,
        &rs_cam_core::compute::catalog::OperationConfig::new_default(OperationType::Pocket),
        &tool,
        &material,
        machine.op_steady_feed(STATED_FEED, None),
        None,
        rs_cam_core::limits::CapTable::empty(),
        None,
    );
    f(&ctx)
}

fn gate_verdict(trace: &SimulationCutTrace) -> PowerVerdict {
    let machine = machine();
    let tolerance = ToleranceBands::default();
    with_ctx(|ctx| {
        power::evaluate(
            ctx,
            &GateEnv {
                sim_trace: Some(trace),
                machine: Some(&machine),
                tolerance: &tolerance,
            },
        )
    })
}

/// The gate power (kW) of a sample at `(ap, ae)` and `feed`.
fn gate_kw(ap: f64, ae: f64, rpm: u32, feed: f64) -> f64 {
    let line = oak().force_line().expect("white oak has a force line");
    power::sample_power_kw(
        &tool_definition(),
        &line,
        &sample(0, ap, ae, rpm, feed),
        feed,
    )
    .expect("a cutting sample with an arc has a power figure")
}

/// The feed (mm/min) at which the gate power of `(ap, ae)` is `kw`. Power
/// is affine in the feed (`PowerTerms`): two points give the line.
fn feed_for_gate_kw(ap: f64, ae: f64, kw: f64) -> f64 {
    let p0 = gate_kw(ap, ae, RPM, 0.0);
    let p1 = gate_kw(ap, ae, RPM, 1000.0);
    assert!(kw > p0, "the edge term alone {p0} kW is under {kw} kW");
    1000.0 * (kw - p0) / (p1 - p0)
}

/// The point of the samples of (a) and (c): a width where the drawn Power
/// edge is the lowest ceiling, and a depth between that edge and
/// the next ceiling, 30 % of the way (non-vacuity of (a): at HEAD the drawn edge alone places
/// the sample outside).
fn point_above_the_power_edge(env: &SafeEnvelope) -> CutPoint {
    let mut table = String::new();
    // From ae = 0.1 D: a narrower sample is a low-engagement or air cut.
    for k in 6..60_u32 {
        let ae = 0.1 * D * f64::from(k) / 6.0;
        let Some(edge) = env.judged_ap_ceiling(LimitKind::Power, ae) else {
            continue;
        };
        let mut next = f64::INFINITY;
        let mut width_ok = true;
        for l in &env.boundary {
            if l.kind == LimitKind::Power {
                continue;
            }
            match l.edge {
                Edge::ApCeiling { .. } | Edge::ApCurve { .. } => {
                    if let Some(c) = env.judged_ap_ceiling(l.kind, ae) {
                        next = next.min(c);
                    }
                }
                Edge::AeCeiling { mm } => width_ok &= ae < mm,
                Edge::AeCurve { .. }
                | Edge::ApFloor { .. }
                | Edge::AeFloor { .. }
                | Edge::PerPoint
                | Edge::Unmodelled { .. } => {}
            }
        }
        table.push_str(&format!("ae {ae:.2}: power {edge:.3}, next {next:.3}\n"));
        if width_ok && next > 1.2 * edge {
            return CutPoint {
                // 30 % of the way: the depth share over the edge (at most
                // 0.3) differs from the gate share of (c), 0.5.
                ap_mm: edge + 0.3 * (next.min(2.0 * edge) - edge),
                ae_mm: ae,
            };
        }
    }
    panic!("no width where the drawn Power edge is the lowest ceiling:\n{table}");
}

#[test]
fn a_sample_within_at_its_emitted_feed_is_not_outside_for_power_a() {
    let op = stated_pocket();
    let env = envelope_of(&op);
    let CutPoint {
        ap_mm: ap,
        ae_mm: ae,
    } = point_above_the_power_edge(&env);
    let available = machine().power_at_rpm(f64::from(RPM));
    // The emitted feed: half the stated feed (the modulator lowered it).
    let feed = 0.5 * STATED_FEED;
    let kw = gate_kw(ap, ae, RPM, feed);
    assert!(
        kw < available,
        "non-vacuity: the gate power {kw} kW at the emitted feed is under {available} kW"
    );
    let trace = trace_of((0..8).map(|i| sample(i, ap, ae, RPM, feed)).collect());

    let verdict = gate_verdict(&trace);
    assert!(
        matches!(verdict, PowerVerdict::Within { .. }),
        "the gate grades the sample within: {verdict:?}"
    );
    let placement =
        with_ctx(|ctx| rs_cam_core::compute::simulate::envelope_placement(&trace, ctx, &env));
    assert!(placement.placed > 0, "non-vacuity: {placement:?}");
    if let Some(worst) = placement.worst.filter(|_| placement.outside > 0) {
        let Placement::Outside { binding, .. } = worst.placement else {
            panic!("an outside count has an outside worst point: {worst:?}");
        };
        assert_ne!(
            binding,
            LimitKind::Power,
            "the gate says within at the emitted feed {feed:.0}, the envelope says power: {placement:?}"
        );
    }
    assert!(
        placement
            .outside_row()
            .is_none_or(|row| !row.contains("spindle power")),
        "no power row: {:?}",
        placement.outside_row()
    );
}

#[test]
fn one_engagement_gives_one_kw_through_every_door_b() {
    let mut op = fixture_op(OperationType::Adaptive3d);
    let OperationConfig::Adaptive3d(c) = &mut op else {
        panic!("the fixture is a 3D Rough");
    };
    c.clearing_strategy = ClearingStrategy::GeodesicAdaptive;
    let env = envelope_of(&op);
    let tool = tool_config();
    let material = oak();
    let machine = machine();
    let mut checked = 0_usize;
    let mut planner_cap_seen = false;
    for j in 0..env.speeds.aes.len() {
        for i in 0..env.speeds.aps.len() {
            let point = env.speeds.at(i, j).expect("a grid point");
            let Some(grid_kw) = point.power_kw else {
                continue;
            };
            let json = serde_json::to_value(point).expect("the point serialises");
            let case = json
                .get("power_case")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_else(|| panic!("the grid point names its power case: {json}"))
                .to_owned();
            let width = json
                .get("power_width_mm")
                .and_then(serde_json::Value::as_f64)
                .unwrap_or_else(|| panic!("the grid point names its power width: {json}"));
            let rpm = json
                .get("rpm")
                .and_then(serde_json::Value::as_f64)
                .expect("an rpm");
            let feed = json
                .get("feed")
                .and_then(serde_json::Value::as_f64)
                .expect("a feed");
            let (ap, ae) = (point.ap_mm, point.ae_mm);
            if case == "planner_cap" {
                planner_cap_seen = true;
                let expect = PlannerWidth::PassCeiling.full_feed_width_mm(ae, D);
                assert!(
                    (width - expect).abs() <= REL * expect,
                    "the planner cap width is the pass ceiling c D = {expect}: {json}"
                );
            }

            // Suggest's door at the grid point's speeds.
            let suggest_at = |rpm: f64| {
                let mut at = op.clone();
                let _ = at.set_depth_per_pass(ap);
                let _ = at.set_stepover(ae);
                at.set_feed_rate(feed);
                at.set_spindle_rpm(None);
                rs_cam_core::feeds::power_at_operating_point(
                    &at,
                    &tool,
                    &material,
                    &machine,
                    Some(CalculatorOperatingPoint {
                        radial_width_mm: ae,
                        axial_depth_mm: ap,
                        feed_rate_mm_min: feed,
                        rpm,
                    }),
                )
                .expect("the grid point has a power figure")
            };
            let suggest = suggest_at(rpm);
            assert!(
                (suggest.required_kw - grid_kw).abs() <= REL * grid_kw,
                "grid {grid_kw} kW vs Suggest {} kW at ({ap}, {ae}): {json}",
                suggest.required_kw
            );
            assert!(
                (suggest.case_ae_mm - width).abs() <= REL * width,
                "grid width {width} vs Suggest case width {}",
                suggest.case_ae_mm
            );

            // The gate grades no air cut: a narrower width has no gate figure.
            if width / D < AIR_CUT_RADIAL_WOC_FRACTION {
                continue;
            }
            // The gate at the case width. The sample RPM is an integer, so
            // Suggest is read again at that RPM.
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "a spindle speed of the grid, finite and positive"
            )]
            let rpm_u = rpm.round() as u32;
            let suggest_u = suggest_at(f64::from(rpm_u));
            let gate = gate_kw(ap, width, rpm_u, feed);
            assert!(
                (gate - suggest_u.required_kw).abs() <= REL * gate,
                "gate {gate} kW vs Suggest {} kW at ({ap}, width {width}, {rpm_u} rpm)",
                suggest_u.required_kw
            );
            checked += 1;
        }
    }
    assert!(checked > 0, "non-vacuity: no grid point has a power figure");
    assert!(
        planner_cap_seen,
        "non-vacuity: no grid point is in the planner cap case"
    );
}

#[test]
fn a_sample_over_the_budget_is_reported_by_both_c() {
    let op = stated_pocket();
    let env = envelope_of(&op);
    let CutPoint {
        ap_mm: ap,
        ae_mm: ae,
    } = point_above_the_power_edge(&env);
    let available = machine().power_at_rpm(f64::from(RPM));
    let feed = feed_for_gate_kw(ap, ae, 1.5 * available);
    let trace = trace_of((0..8).map(|i| sample(i, ap, ae, RPM, feed)).collect());

    let verdict = gate_verdict(&trace);
    assert!(
        matches!(verdict, PowerVerdict::Exceeds { .. }),
        "the gate grades the sample over the budget: {verdict:?}"
    );
    let placement =
        with_ctx(|ctx| rs_cam_core::compute::simulate::envelope_placement(&trace, ctx, &env));
    assert_eq!(placement.outside, placement.placed, "{placement:?}");
    let worst = placement.worst.expect("a worst point");
    let Placement::Outside { binding, margin } = worst.placement else {
        panic!("the worst point is outside: {worst:?}");
    };
    assert_eq!(binding, LimitKind::Power, "{placement:?}");
    let row = placement.outside_row().expect("an outside row");
    assert!(row.contains("spindle power"), "{row}");
    // The share over the budget is the gate's: 1.5 x the rated power.
    let gate_share = match verdict {
        PowerVerdict::Exceeds {
            peak_kw,
            available_kw,
            ..
        } => peak_kw / available_kw - 1.0,
        PowerVerdict::Within { .. } | PowerVerdict::Unmodeled { .. } => {
            panic!("checked above")
        }
    };
    assert!(gate_share > 0.0, "non-vacuity: {gate_share}");
    assert!(
        (-margin - gate_share).abs() <= REL,
        "the envelope share {:.6} is the gate share {gate_share:.6}",
        -margin
    );
}
