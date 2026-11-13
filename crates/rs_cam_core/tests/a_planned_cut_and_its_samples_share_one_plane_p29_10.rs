//! Feeds contract P29-10 10g
//! (`planning/feeds_contract_2026-11-03/phases/P29-10_DESIGN.md` 3.8 and
//! section 16): a planned cut and its samples share one plane.
//!
//! - Claim 1: every planned row of a cut with a plane point sits at the
//!   mark of its cut at the proposal (`SafeEnvelope::marks_at_proposal`):
//!   the body at the proposal point, a slot-width cut (a slot ring, a ramp
//!   or a helix entry) at the tool diameter, an entry at its own depth with
//!   `own_depth` set. One fact, one field: `own_depth_mm` is the depth of
//!   the point (lead ruling R5).
//! - Claim 2: the marks at the stated point place the body at the stated
//!   point; they describe the rows exactly when the stated point is the
//!   proposal point.
//! - Claim 3: on a simulated contour Pocket, the force plane of the body
//!   cut exists, its budget is the machine force budget and a limit of the
//!   cut (`BudgetRole::Limit`); each node is the one force law
//!   (`force_budget::peak_force_n`) at the plane's chip; the slot ring
//!   mark (at the tool diameter) is the widest cut on the plane: no sample
//!   is wider than it by more than one width column of the field.
//! - Claim 4: `CutPoint3::over_budget` is the strict test `force > line`,
//!   and a cut with no line has no sample over it (lead ruling O4).
//!
//! `cargo test -p rs_cam_core -q --test a_planned_cut_and_its_samples_share_one_plane_p29_10`

#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use common::kind_fixtures::{TOOL_D, pocket_session};

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::config::{DressupConfig, DressupEntryStyle};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolFact, ToolId, ToolType};
use rs_cam_core::feeds::SpindleStrategy;
use rs_cam_core::feeds::envelope::{Edge, EnvelopeInput, LimitKind, SafeEnvelope};
use rs_cam_core::feeds::suggest::{NoProject, SuggestContext};
use rs_cam_core::limits::{CutKind, EntryKind};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::tool_load::cut_force::{CutPoint3, budget_line, cut_points};
use rs_cam_core::tool_load::force_field::BudgetRole;
use rs_cam_core::tool_load::{GateEnv, ToleranceBands};

fn tool(kind: ToolType, d: f64) -> ToolConfig {
    let mut t = ToolConfig::test_default(ToolId::from_raw(1), kind);
    t.diameter = d;
    t.set_flute_count(ToolFact::Stated(2));
    t.set_cutting_length(ToolFact::Stated((d * 3.0).max(12.0)));
    t.set_shank_diameter(ToolFact::Stated(d.max(3.0)));
    t.shaft_diameter = d.max(3.0);
    t.set_stickout(ToolFact::Stated(t.cutting_length_planning_mm() + 8.0));
    t
}

/// Every op at its fixture values on a 6 mm end mill, with the registry
/// entry and with a ramp entry.
fn envelopes() -> Vec<(String, SafeEnvelope)> {
    let ramp = DressupConfig {
        entry_style: DressupEntryStyle::Ramp,
        ..Default::default()
    };
    let t = tool(ToolType::EndMill, 6.0);
    let mut out = Vec::new();
    for &op_type in OperationType::ALL {
        let op = rs_cam_core::compute::fixture_values::fixture_op(op_type);
        for dressups in [None, Some(&ramp)] {
            let Ok(env) = SafeEnvelope::for_cut(EnvelopeInput {
                operation: &op,
                tool: &t,
                machine: &MachineProfile::default(),
                material: &Material::SolidWood {
                    species: WoodSpecies::GenericHardwood,
                },
                lut: rs_cam_core::feeds::embedded_vendor_lut(),
                spindle_strategy: SpindleStrategy::default(),
                context: SuggestContext::no_project(NoProject {
                    dressups,
                    ..NoProject::NONE
                }),
                modulation: rs_cam_core::dressup::feed_modulation::RunModulation::Off,
            }) else {
                continue;
            };
            out.push((format!("{op_type:?} ramp={}", dressups.is_some()), env));
        }
    }
    out
}

fn diameter(env: &SafeEnvelope) -> Option<f64> {
    match env.limit(LimitKind::Diameter).map(|l| &l.edge) {
        Some(Edge::AeCeiling { mm }) => Some(*mm),
        Some(_) | None => None,
    }
}

const EPS: f64 = 1e-9;

#[test]
fn every_planned_row_sits_at_the_mark_of_its_cut_p29_10() {
    let envs = envelopes();
    assert!(
        envs.len() > 20,
        "non-vacuity: the op fixtures build envelopes"
    );
    let mut slot_marks = 0_usize;
    let mut own_depth_rows = 0_usize;
    for (name, env) in &envs {
        let marks = env
            .marks_at_proposal()
            .unwrap_or_else(|| panic!("{name}: every envelope places its cuts"));
        assert!(
            marks.describes_rows(),
            "{name}: the proposal marks describe the rows"
        );
        let p = marks.point();
        for row in env.rows().iter().filter(|r| r.is_planned()) {
            let cut = row.cut().expect("a planned row has a cut");
            match (marks.of_cut(cut), row.plane_point()) {
                (Some(mark), Some(at)) => assert_eq!(mark.at, at, "{name}: {cut:?}"),
                (None, None) => {}
                (mark, at) => panic!("{name}: {cut:?}: the mark {mark:?} and the row {at:?}"),
            }
        }
        for mark in marks.marks() {
            if mark.cut == CutKind::Body && !mark.at.own_depth {
                assert!(
                    (mark.at.ap_mm - p.ap_mm).abs() < EPS,
                    "{name}: the body is at the proposal depth"
                );
            }
            let slot_width = matches!(
                mark.cut,
                CutKind::SlotRing | CutKind::Entry(EntryKind::Ramp | EntryKind::Helix)
            );
            if slot_width && let Some(d) = diameter(env) {
                slot_marks += 1;
                assert!(
                    (mark.at.ae_mm - d).abs() < EPS,
                    "{name}: {:?} declares the slot width: {} against D {d}",
                    mark.cut,
                    mark.at.ae_mm
                );
            }
        }
        for row in env.rows().iter().filter(|r| r.is_planned()) {
            if let Some(own) = row.own_depth_mm() {
                own_depth_rows += 1;
                let at = row.plane_point().expect("an own depth is a plane depth");
                assert!(at.own_depth && (at.ap_mm - own).abs() < EPS, "{name}");
            }
        }
    }
    assert!(slot_marks > 0, "non-vacuity: a slot-width mark");
    assert!(own_depth_rows > 0, "non-vacuity: an own-depth row");
}

#[test]
fn the_stated_marks_place_the_cut_the_op_holds_p29_10() {
    let mut stated_seen = 0_usize;
    for (name, env) in envelopes() {
        let Some(stated) = env.stated else {
            assert!(env.marks_at_stated().is_none(), "{name}");
            continue;
        };
        stated_seen += 1;
        let marks = env.marks_at_stated().expect("a stated point has marks");
        let p = marks.point();
        assert!((p.ap_mm - stated.ap_mm).abs() < EPS && (p.ae_mm - stated.ae_mm).abs() < EPS);
        let proposal = env.marks_at_proposal().expect("proposal marks").point();
        let same =
            (p.ap_mm - proposal.ap_mm).abs() < 1e-6 && (p.ae_mm - proposal.ae_mm).abs() < 1e-6;
        assert_eq!(marks.describes_rows(), same, "{name}");
        if let Some(body) = marks.of_cut(CutKind::Body)
            && !body.at.own_depth
        {
            assert!((body.at.ap_mm - stated.ap_mm).abs() < EPS, "{name}");
        }
    }
    assert!(stated_seen > 5, "non-vacuity: stated points");
}

#[test]
fn a_slot_ring_and_its_samples_share_one_plane_p29_10() {
    let session = pocket_session(false);
    let tc = session.toolpath_config_at(0).expect("the op");
    let env = session
        .safe_envelope(
            tc,
            rs_cam_core::dressup::feed_modulation::RunModulation::Off,
        )
        .expect("a tool")
        .expect("the Pocket envelope");
    let d = diameter(&env).expect("a diameter");
    assert!((d - TOOL_D).abs() < EPS, "the fixture tool");
    let ring = env
        .marks_at_proposal()
        .and_then(|m| m.of_cut(CutKind::SlotRing))
        .expect("a contour Pocket declares its slot ring");
    assert!((ring.at.ae_mm - d).abs() < EPS);

    let parts = session.load_context_parts(0).expect("a load context");
    let ctx = parts.ctx();
    let trace = session
        .simulation_result()
        .and_then(|s| s.cut_trace.as_deref())
        .expect("a cut trace");
    let tolerance = ToleranceBands::default();
    let gate_env = GateEnv {
        sim_trace: Some(trace),
        machine: Some(session.machine()),
        tolerance: &tolerance,
    };
    let samples = cut_points(&ctx, &gate_env);
    assert!(!samples.is_empty(), "non-vacuity: samples on the plane");
    let line = budget_line(&ctx, &gate_env);
    assert!(
        line.is_some(),
        "a roughing Pocket has the force budget line"
    );

    let top = samples
        .iter()
        .map(|s| s.ap_mm)
        .fold(0.0_f64, f64::max)
        .max(1.0);
    let plane = session
        .force_plane(0, d, top)
        .expect("a Pocket with a force line has a force plane");
    assert_eq!(plane.role, BudgetRole::Limit);
    assert!((plane.budget_n - session.machine().rigidity.force_budget_n).abs() < EPS);
    assert_eq!(
        Some(plane.budget_n),
        line,
        "the field and the samples read one budget"
    );
    // One law: a node is `peak_force_n` at the plane's chip.
    let (c, r) = (plane.field.aes.len() / 2, plane.field.aps.len() / 2);
    let node = plane.field.at(c, r);
    let law = rs_cam_core::feeds::force_budget::peak_force_n(
        ctx.tool(),
        ctx.material(),
        plane.field.aps[r],
        plane.field.aes[c],
        plane.fz_mm,
    )
    .expect("the law answers");
    assert!(
        (node - law).abs() < 1e-9 * law.max(1.0),
        "{node} against {law}"
    );
    // The slot ring mark is the widest cut on the plane: no sample is wider
    // than it by more than one width column of the field. (On this
    // fixture at its simulation cell the widest sample reads 5 mm against
    // the 6 mm slot: the measured arc of a full slot reads short.)
    let column = d / (plane.field.aes.len() - 1) as f64;
    let widest = samples.iter().map(|s| s.ae_mm).fold(0.0_f64, f64::max);
    assert!(
        widest <= ring.at.ae_mm + column,
        "the widest sample {widest} is wider than the slot ring mark {}",
        ring.at.ae_mm
    );
    assert!(
        widest > 0.5 * ring.at.ae_mm,
        "non-vacuity: slot-like samples"
    );
}

#[test]
fn over_budget_is_the_strict_test_against_the_line_p29_10() {
    let at = |force: Option<f64>| CutPoint3 {
        ae_mm: 1.0,
        ap_mm: 1.0,
        force_n: force,
        weight_s: 1.0,
        move_index: 0,
        sample_index: 0,
    };
    assert!(at(Some(10.1)).over_budget(Some(10.0)));
    assert!(!at(Some(10.0)).over_budget(Some(10.0)), "strict");
    assert!(!at(Some(9.0)).over_budget(Some(10.0)));
    assert!(
        !at(Some(50.0)).over_budget(None),
        "no line, no sample over a limit"
    );
    assert!(!at(None).over_budget(Some(10.0)));
    assert!(!at(Some(f64::NAN)).over_budget(Some(10.0)));
    assert!(at(Some(12.0)).over_budget_words(Some(10.0)).is_some());
    assert!(at(Some(9.0)).over_budget_words(Some(10.0)).is_none());
}
