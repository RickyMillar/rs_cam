//! K7 (feeds contract P9, ruling Q5): the simulation is placed on the K2b
//! envelope; it is not fed back into Suggest.
//!
//! - (a) A measured point outside a ceiling of the envelope gives ONE
//!   `project.sim_outside_envelope` warning per toolpath that names the
//!   binding limit, in the diagnostics and in the export (ruling Q6: a
//!   warning). The placement is the tenth shared decision
//!   (`compute::simulate::envelope_placement`).
//! - (b) Suggest is byte-equal with and without a stored simulation.
//! - (c) No `feeds/` code names `SimulationResult`, `SimulationCutSample`
//!   or `ToolpathLoadVerdict`, except `feeds/engagement.rs`: its
//!   `Engagement::from_sample` is the one sample constructor of K7, and the
//!   gate and the placement call it (Suggest does not).
//!
//! Red: make `EnvelopePlacement::outside_row` return `None`; (a) loses its
//! warning.

#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure; the shared source scan indexes its own char buffer"
)]

mod common;

#[path = "common/source_scan.rs"]
mod source_scan;

use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::floor::Floor;
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::compute::tool_config::{CentreCutting, ToolConfig, ToolFact, ToolId, ToolType};
use rs_cam_core::diagnostics::ids;
use rs_cam_core::feeds::envelope::Placement;
use rs_cam_core::session::{
    ProjectSession, ProjectSessionBuilder, SimulationOptions, SimulationResolution,
};

/// A test-built tool (ruling D12: it states its facts): 6 mm, flute 8 mm,
/// stickout 30 mm, centre cutting. The values are the case, not a real
/// tool.
fn short_flute_tool() -> ToolConfig {
    let mut tool = ToolConfig::test_default(ToolId::from_raw(0), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.set_cutting_length(ToolFact::Stated(8.0));
    tool.set_stickout(ToolFact::Stated(30.0));
    tool.set_centre_cutting(ToolFact::Stated(CentreCutting::Yes));
    tool
}

/// The stock top (mm) of the case.
const STOCK_TOP_Z: f64 = 25.5;
/// The floor: 10.5 mm below the top, more than the 8 mm flute.
const DEEP_FLOOR_Z: f64 = 15.0;

/// One pocket, half 8 mm, cut in ONE level of 10.5 mm (a depth per pass
/// above the depth), generated.
fn deep_pocket_session() -> ProjectSession {
    let mut stock = common::session::stock_over(20.0, STOCK_TOP_Z);
    rs_cam_core::compute::fixture_values::fill_unset_stock(&mut stock);
    let mut builder = ProjectSessionBuilder::new().stock(stock);
    let tool_idx = builder.add_tool(short_flute_tool());
    let tool_id = rs_cam_core::ids::ToolId::from_raw(builder.tools()[tool_idx].id.raw());
    let model = builder.add_model(common::session::polygon_model(
        vec![common::session::square_polygon(8.0)],
        "square",
    ));
    let op = OperationConfig::Pocket(PocketConfig {
        floor: Floor::Z { z: DEEP_FLOOR_Z },
        depth_per_pass: Some(12.0),
        ..PocketConfig::default()
    });
    let cfg = common::session::toolpath_config("deep pocket", op, tool_id, model);
    let _ = builder.add_toolpath(0, cfg).expect("add the pocket");
    let mut session = builder.build();
    common::session::generate(&mut session, 0);
    session
}

fn simulate(session: &mut ProjectSession) {
    let opts = SimulationOptions {
        resolution: SimulationResolution::Fixed(0.5),
        adaptive_feed_modulation: false,
        ..SimulationOptions::default()
    };
    session
        .run_simulation(&opts, &AtomicBool::new(false))
        .expect("the simulation runs");
}

/// What Suggest gives for toolpath 0: the calculator result and the
/// envelope, as text.
fn suggest_text(session: &ProjectSession) -> String {
    let tc = &session.toolpath_configs()[0];
    let tool = session.toolpath_tool(tc).expect("the tool");
    let feeds = session.feeds_result_for_toolpath(tc, tool);
    let envelope = session
        .safe_envelope(tc)
        .expect("a tool")
        .expect("a runnable pairing");
    format!(
        "{feeds:?}\n{}",
        serde_json::to_string(&envelope).expect("the envelope serialises")
    )
}

#[test]
fn a_measured_point_outside_the_envelope_warns_once_with_its_limit() {
    let mut session = deep_pocket_session();
    simulate(&mut session);
    let trace = session
        .simulation_result()
        .and_then(|sim| sim.cut_trace.clone());
    let placement = session
        .envelope_placement_measured(0, trace.as_deref())
        .expect("a fresh run places the toolpath");
    assert!(placement.placed > 0, "non-vacuity: {placement:?}");
    assert!(placement.outside > 0, "the 10.5 mm level crosses a ceiling");
    let worst = placement.worst.expect("a worst point");
    let Placement::Outside { binding, .. } = worst.placement else {
        panic!("the worst point is outside: {worst:?}");
    };
    assert!(
        placement.peak.and_then(|p| p.power_kw).is_some(),
        "the peak carries the gate's power: {placement:?}"
    );

    let diagnostics = session
        .diagnose_toolpath_with_trace(0, trace.as_deref())
        .expect("the diagnostics");
    let rows: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.id.0 == ids::PROJECT_SIM_OUTSIDE_ENVELOPE)
        .collect();
    assert_eq!(rows.len(), 1, "one warning per toolpath: {diagnostics:?}");
    let row = rows.first().expect("one row");
    let judged = worst
        .judged
        .expect("an outside point names its judged value");
    assert!(
        judged.measured > judged.limit,
        "the judged value crosses its ceiling: {judged:?}"
    );
    assert!(
        row.message
            .contains(&format!("{:.2} {}", judged.limit, judged.unit)),
        "the row names the limit value: {}",
        row.message
    );
    assert!(
        row.message.contains(binding.label()),
        "the row names the binding limit {}: {}",
        binding.label(),
        row.message
    );

    let exported = session
        .export_gcode_checked(trace.as_deref())
        .expect("the export writes the program (Q6)");
    assert_eq!(
        exported
            .warnings
            .iter()
            .filter(|w| w.id == ids::PROJECT_SIM_OUTSIDE_ENVELOPE)
            .count(),
        1,
        "the export names it once: {:?}",
        exported.warnings
    );
}

#[test]
fn suggest_is_byte_equal_with_and_without_a_simulation() {
    let mut session = deep_pocket_session();
    let before = suggest_text(&session);
    simulate(&mut session);
    assert!(
        session.simulation_result().is_some(),
        "non-vacuity: a stored simulation"
    );
    let after = suggest_text(&session);
    assert_eq!(before, after, "Suggest reads no simulation (Q5)");
}

#[test]
fn feeds_code_names_no_simulation_result() {
    let feeds = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/feeds");
    let needles = [
        "SimulationResult",
        "SimulationCutSample",
        "ToolpathLoadVerdict",
    ];
    let mut anchor = false;
    let mut hits = Vec::new();
    for path in source_scan::rust_files(&feeds) {
        let text = source_scan::strip_comments(
            &std::fs::read_to_string(&path).expect("a readable source file"),
        );
        let is_engagement = path.ends_with("engagement.rs");
        for needle in needles {
            if text.contains(needle) {
                if is_engagement && needle == "SimulationCutSample" {
                    anchor = true;
                } else {
                    hits.push(format!("{}: {needle}", path.display()));
                }
            }
        }
    }
    assert!(
        anchor,
        "non-vacuity: the scan reads feeds/engagement.rs and finds its sample constructor"
    );
    assert!(hits.is_empty(), "feeds code reads the simulation: {hits:?}");
}

/// Q5: no Suggest path builds an engagement from a simulation sample. Only
/// the gate and the placement (outside `feeds/`) call
/// `Engagement::from_sample`; inside `feeds/` only its definition in
/// `engagement.rs` names it.
#[test]
fn no_suggest_path_calls_the_sample_constructor() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut defined = false;
    let mut called_outside_feeds = false;
    let mut hits = Vec::new();
    for path in source_scan::rust_files(&src) {
        let text = source_scan::strip_comments(
            &std::fs::read_to_string(&path).expect("a readable source file"),
        );
        if !text.contains("from_sample(") {
            continue;
        }
        let in_feeds = path.components().any(|c| c.as_os_str() == "feeds");
        if path.ends_with("feeds/engagement.rs") {
            defined = text.contains("pub fn from_sample(");
        } else if in_feeds {
            hits.push(path.display().to_string());
        } else {
            called_outside_feeds = true;
        }
    }
    assert!(
        defined && called_outside_feeds,
        "non-vacuity: the scan finds the definition and a caller (the gate)"
    );
    assert!(
        hits.is_empty(),
        "a feeds path builds from a sample: {hits:?}"
    );
}

// ── Cone tools (lead decision (i), 2026-11-05) ──────────────────────────
//
// A tapered ball or a V-bit cuts wider than its tip as the cone engages, so
// the placement compares a measured width with the engaged diameter at the
// sample depth (`Engagement::d_eng_mm`), not the nominal tip diameter. P16
// (D12) deleted the slot rule and its cone exception.

/// The envelope of `op` with `tool` in generic hardwood on the shipped
/// generic router.
fn envelope_of(
    op: &OperationConfig,
    tool: &ToolConfig,
) -> rs_cam_core::feeds::envelope::SafeEnvelope {
    rs_cam_core::feeds::envelope::SafeEnvelope::for_cut(
        rs_cam_core::feeds::envelope::EnvelopeInput {
            operation: op,
            tool,
            machine: &rs_cam_core::machine::MachineProfile::generic_wood_router(),
            material: &rs_cam_core::material::Material::SolidWood {
                species: rs_cam_core::material::WoodSpecies::GenericHardwood,
            },
            lut: rs_cam_core::feeds::embedded_vendor_lut(),
            spindle_strategy: rs_cam_core::feeds::SpindleStrategy::MatchChart,
            context: rs_cam_core::feeds::suggest::SuggestContext::default(),
        },
    )
    .expect("a runnable pairing")
}

/// Place one steady-state cutting sample of `tool` at depth `ap` and arc
/// `psi` against `envelope`, through the tenth shared decision.
fn place_one(
    tool: &ToolConfig,
    kind: rs_cam_core::compute::catalog::OperationType,
    envelope: &rs_cam_core::feeds::envelope::SafeEnvelope,
    ap: f64,
    psi: f64,
) -> rs_cam_core::compute::simulate::EnvelopePlacement {
    use rs_cam_core::stock::simulation_cut::{SimulationCutSample, SimulationCutTrace};
    let id = rs_cam_core::ids::ToolpathId::placeholder();
    let sample = SimulationCutSample {
        toolpath_id: id,
        is_cutting: true,
        spindle_rpm: 16_000,
        flute_count: 2,
        feed_rate_mm_min: 1000.0,
        axial_doc_mm: ap,
        axial_engagement_mm: ap,
        arc_engagement_radians: Some(psi),
        engagement: rs_cam_core::stock::simulation_cut::Engagement {
            radial_woc_fraction: 0.5,
            ..Default::default()
        },
        ..SimulationCutSample::test_fixture()
    };
    let trace = SimulationCutTrace {
        samples: vec![sample],
        ..SimulationCutTrace::test_fixture()
    };
    let tool_def = rs_cam_core::compute::build_cutter(tool).expect("the tool builds");
    let material = rs_cam_core::material::Material::SolidWood {
        species: rs_cam_core::material::WoodSpecies::GenericHardwood,
    };
    let ctx = rs_cam_core::tool_load::ToolpathLoadContext {
        toolpath_id: id,
        tool: &tool_def,
        material: &material,
        operation_family: rs_cam_core::feeds::vendor_lut::LutOperationFamily::Pocket,
        pass_role: rs_cam_core::feeds::vendor_lut::LutPassRole::Finish,
        operation_feed_rate_mm_min: 1000.0,
        steady_feed: rs_cam_core::machine::SteadyFeedMmMin::op_feed_only(1000.0),
        operation_kind: kind,
        spans: None,
        drill_op: None,
        commanded_thinning_lift: None,
        radial_stock_to_leave_mm: None,
    };
    rs_cam_core::compute::simulate::envelope_placement(&trace, &ctx, envelope)
}

fn binding_of(p: &rs_cam_core::compute::simulate::EnvelopePlacement) -> Option<LimitKind> {
    p.worst.and_then(|w| match w.placement {
        Placement::Outside { binding, .. } => Some(binding),
        Placement::Inside { .. } => None,
    })
}

use rs_cam_core::feeds::envelope::LimitKind;

#[test]
fn a_tapered_ball_wider_than_its_tip_is_judged_at_its_engaged_diameter() {
    use rs_cam_core::tool::MillingCutter;
    // The rivmap tapered ball (fixture values from its project file): tip
    // D 2, half-angle 5.7 deg, shank 6.
    let mut tool = ToolConfig::test_default(ToolId::from_raw(1), ToolType::TaperedBallNose);
    tool.diameter = 2.0;
    tool.set_taper_half_angle(ToolFact::Stated(5.7));
    tool.set_shank_diameter(ToolFact::Stated(6.0));
    tool.set_cutting_length(ToolFact::Stated(20.0));
    let op = OperationConfig::Scallop(rs_cam_core::compute::operation_configs::ScallopConfig {
        feed_rate: Some(1000.0),
        plunge_rate: Some(300.0),
        spindle_rpm: Some(16_000),
        ..Default::default()
    });
    let envelope = envelope_of(&op, &tool);
    let tip = match envelope.limit(LimitKind::Diameter).map(|l| &l.edge) {
        Some(rs_cam_core::feeds::envelope::Edge::AeCeiling { mm }) => *mm,
        other => panic!("non-vacuity: the envelope has a width ceiling: {other:?}"),
    };
    let ap = 3.0;
    let tool_def = rs_cam_core::compute::build_cutter(&tool).expect("the tool builds");
    let d_eng = 2.0 * tool_def.engagement_radius(ap);
    assert!(d_eng > tip, "the cone is wider than the tip at this depth");
    let ae = 0.5 * (tip + d_eng);
    let psi = rs_cam_core::feeds::force::immersion_angle(ae, d_eng / 2.0);
    let placement = place_one(
        &tool,
        rs_cam_core::compute::catalog::OperationType::Scallop,
        &envelope,
        ap,
        psi,
    );
    assert_eq!(placement.placed, 1, "non-vacuity: {placement:?}");
    assert_ne!(
        binding_of(&placement),
        Some(LimitKind::Diameter),
        "a width {ae} mm under the engaged diameter {d_eng} mm is not past the tool: {placement:?}"
    );
}

/// Lead decision (iii), 2026-11-05: with a fresh trace, the simulation row
/// supersedes the pre-simulation `project.outside_envelope` row of the
/// same toolpath (the one reducer, as the flute pair does).
#[test]
fn the_simulation_row_supersedes_the_pre_simulation_row() {
    let mut session = deep_pocket_session();
    let before = session
        .diagnose_toolpath_with_trace(0, None)
        .expect("the diagnostics");
    assert!(
        before
            .iter()
            .any(|d| d.id.0 == ids::PROJECT_OUTSIDE_ENVELOPE),
        "non-vacuity: the stated cut is outside before the simulation: {before:?}"
    );
    simulate(&mut session);
    let trace = session
        .simulation_result()
        .and_then(|sim| sim.cut_trace.clone());
    let after = session
        .diagnose_toolpath_with_trace(0, trace.as_deref())
        .expect("the diagnostics");
    assert!(
        after
            .iter()
            .any(|d| d.id.0 == ids::PROJECT_SIM_OUTSIDE_ENVELOPE),
        "the simulation row stands: {after:?}"
    );
    assert!(
        !after
            .iter()
            .any(|d| d.id.0 == ids::PROJECT_OUTSIDE_ENVELOPE),
        "the pre-simulation row is superseded: {after:?}"
    );
}

/// Lead decision (b), 2026-11-05: with a fresh trace the K8 lineage
/// records the measured peak, the case "measured" and the binding limit
/// kind; with no trace it records none.
#[test]
fn the_lineage_records_the_measured_peak_and_its_limit() {
    let mut session = deep_pocket_session();
    let unmeasured = session.lineage_measured(0, None).expect("a result");
    assert!(unmeasured.measured.is_none(), "no trace, no measurement");
    simulate(&mut session);
    let trace = session
        .simulation_result()
        .and_then(|sim| sim.cut_trace.clone());
    let placement = session
        .envelope_placement_measured(0, trace.as_deref())
        .expect("a fresh run places the toolpath");
    let lineage = session
        .lineage_measured(0, trace.as_deref())
        .expect("a result");
    let measured = lineage.measured.as_ref().expect("the lineage records it");
    assert_eq!(measured.case, "measured");
    assert_eq!(measured.peak, placement.peak, "the peak of the placement");
    let binding = placement.worst.and_then(|w| match w.placement {
        Placement::Outside { binding, .. } => Some(binding),
        Placement::Inside { .. } => None,
    });
    assert!(binding.is_some(), "non-vacuity: the case crosses a limit");
    assert_eq!(measured.binding, binding, "the binding limit kind");
    let json = lineage.to_json();
    assert_eq!(
        json["measured"]["case"], "measured",
        "the JSON (MCP, CLI) names it: {json}"
    );
}
