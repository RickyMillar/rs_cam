//! Q8 (feeds contract P8, operator ruling 2026-11-03): Project Curve enters
//! with a straight plunge (`EntryContract::StraightPlunge`).
//!
//! Suggest writes the plunge rate of a Project Curve as
//! `min(G10 rule, G6 axial chip x rpm x Z, F)` where a G6 claim covers the
//! tool (report 02 F-c1), and the generator descends at that rate at each
//! chain start. Before P8 the plunge was the G10 rule alone (F / Z): report
//! 02 (c) measured 2999 mm/min where the G6 vertical rate was 1028 mm/min.
//! P20b supersedes lead decision Q8a ("Project Curve only"): the one
//! vertical-rate limit (`feeds::vertical::VerticalRateLimit`) gives the G6
//! bound to every op, so a Pocket with the same tool carries it too, and
//! the `PlungeFeedRule::AxialChip` variant is gone.
//!
//! The tool: the op 5 tool of report 02, a 6 mm 2F flat end mill, in white
//! oak. Every expected value comes from the result under test: the G10
//! fraction of the claim, the G6 chip of `FeedsResult::ramp`, the shipped
//! feed and RPM.
//!
//! A `No(..)` tool generates, takes no ramp (K4: P7 C stopped the
//! G-CENTRECUT fallback ramp on a `StraightPlunge` contract) and names the
//! deep plunge. An `Unknown` tool stops with the D12 reason (ruling Q10: a
//! missing input stops generation); it gets no ramp either.
//!
//! Red: drop the axial chip from `VerticalRateLimit::new`. The shipped
//! plunge is then the G10 rule.

#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use std::sync::atomic::AtomicBool;

use common::op_fixtures::{CaseSpec, Fixture, Frame, Prior, case_session};
use rs_cam_core::compute::OperationError;
use rs_cam_core::compute::catalog::{EntryContract, OperationConfig, OperationType};
use rs_cam_core::compute::config::StockSource;
use rs_cam_core::compute::not_set::NotSetKind;
use rs_cam_core::compute::tool_config::{
    CentreCutting, NonCentreCuttingGeometry, ToolConfig, ToolFact, ToolId, ToolType,
};
use rs_cam_core::diagnostics::ids;
use rs_cam_core::feeds::plunge::PlungeBinding;
use rs_cam_core::feeds::suggest::{
    NoProject, SuggestContext, SuggestForOperationInput, SuggestedParams, suggest_for_operation,
};
use rs_cam_core::feeds::{RampBasis, SpindleStrategy, embedded_vendor_lut};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::session::SessionError;
use rs_cam_core::toolpath::{MoveIntent, MoveType};

/// Report 02 op 5: a 6 mm 2F flat end mill.
fn op5_tool(centre: ToolFact<CentreCutting>) -> ToolConfig {
    let mut t = ToolConfig::test_default(ToolId::from_raw(1), ToolType::EndMill);
    t.diameter = 6.0;
    t.set_flute_count(ToolFact::Stated(2));
    t.set_cutting_length(ToolFact::Stated(24.0));
    t.set_shank_diameter(ToolFact::Stated(6.0));
    t.set_stickout(ToolFact::Stated(32.0));
    t.set_centre_cutting(centre);
    t
}

fn oak() -> Material {
    Material::SolidWood {
        species: WoodSpecies::WhiteOak,
    }
}

/// A Project Curve 1 mm into the surface.
fn project_curve() -> OperationConfig {
    let mut op = OperationConfig::new_default(OperationType::ProjectCurve);
    if let OperationConfig::ProjectCurve(cfg) = &mut op {
        cfg.depth = Some(1.0);
    }
    op
}

fn suggested(op: &OperationConfig, tool: &ToolConfig) -> SuggestedParams {
    suggest_for_operation(SuggestForOperationInput {
        operation: op,
        tool,
        machine: &MachineProfile::generic_wood_router(),
        material: &oak(),
        lut: embedded_vendor_lut(),
        spindle_strategy: SpindleStrategy::MatchChart,
        context: SuggestContext::no_project(NoProject::NONE),
    })
    .unwrap_or_else(|e| panic!("Suggest serves {:?}: {e}", op.op_type()))
}

/// The shipped feed, RPM and plunge of `s`.
fn shipped(s: &SuggestedParams) -> (f64, f64, f64) {
    let feed = s.operation.feed_rate().expect("Suggest writes the feed");
    let rpm = f64::from(s.operation.spindle_rpm().expect("Suggest writes the rpm"));
    let plunge = s
        .operation
        .plunge_rate()
        .expect("Suggest writes the plunge");
    (feed, rpm, plunge)
}

/// The G6 axial chip (mm/tooth) and Z of the result.
fn g6_chip(s: &SuggestedParams) -> (f64, u32) {
    match &s.feeds_result.ramp {
        RampBasis::Sourced {
            axial_chip_mm,
            drill,
            ..
        } => (*axial_chip_mm, drill.flutes),
        RampBasis::NoChip { reason } => panic!("a G6 claim covers the 6 mm flat: {reason:?}"),
    }
}

#[test]
fn the_project_curve_row_is_a_straight_plunge() {
    match OperationType::ProjectCurve.registry_entry().entry {
        EntryContract::StraightPlunge { .. } => {}
        EntryContract::Dressup { .. }
        | EntryContract::Planner { .. }
        | EntryContract::DescentIsCut => panic!("Project Curve is a straight plunge (Q8)"),
    }
}

#[test]
fn suggest_writes_the_axial_chip_plunge_on_a_project_curve() {
    let tool = op5_tool(ToolFact::Stated(CentreCutting::Yes));
    let s = suggested(&project_curve(), &tool);
    let (feed, rpm, plunge) = shipped(&s);
    let fraction = s
        .feeds_result
        .plunge
        .claim()
        .expect("the G10 flat claim covers the 6 mm 2F flat")
        .fraction;
    let (chip, z) = g6_chip(&s);
    let rule = fraction * feed;
    let axial = chip * rpm * f64::from(z);
    assert!(
        axial < rule,
        "non-vacuity: the axial chip binds ({axial} vs the G10 rule {rule})"
    );
    let want = rule.min(axial).min(feed).floor();
    assert_eq!(plunge, want, "feed {feed}, rpm {rpm}");
    let bound = s
        .feeds_result
        .plunge
        .vertical_limit()
        .axial()
        .expect("the result carries the bound");
    assert_eq!(bound.axial_chip_mm(), chip);
    // P18 (review D14): the plunge reads the feed before the radial
    // chip-thinning lift. A Project Curve cuts the full width, so the lift
    // is 1 and the feed before the lift is the shipped feed.
    assert_eq!(s.feeds_result.derates.radial_chip_thinning, 1.0);
    let unlifted = s.feeds_result.unlifted_feed();
    assert_eq!(
        rs_cam_core::feeds::plunge::resolve_plunge_at(&s.feeds_result.plunge, unlifted, rpm),
        Some((want, PlungeBinding::AxialChip))
    );
    let (headline, _) = s.feeds_result.plunge.card_text_at(unlifted, rpm);
    assert!(headline.contains("set by the G6 axial chip"), "{headline}");
}

#[test]
fn a_pocket_with_the_same_tool_carries_the_bound_p20b() {
    let tool = op5_tool(ToolFact::Stated(CentreCutting::Yes));
    let s = suggested(&OperationConfig::new_default(OperationType::Pocket), &tool);
    let (chip, _) = g6_chip(&s);
    let bound = s
        .feeds_result
        .plunge
        .vertical_limit()
        .axial()
        .expect("P20b: every op carries the G6 bound");
    assert_eq!(bound.axial_chip_mm(), chip);
}

#[test]
fn the_chain_starts_descend_at_the_shipped_plunge() {
    let tool = op5_tool(ToolFact::Stated(CentreCutting::Yes));
    let s = suggested(&project_curve(), &tool);
    let (_, _, plunge) = shipped(&s);
    let (mut session, index) = case_session(CaseSpec {
        name: "project curve",
        op: s.operation,
        prior: Prior::None,
        stock_source: StockSource::Fresh,
        frame: Frame::TopUp,
        fixture: Fixture::Standard,
        tool: Some(tool),
    });
    common::session::generate(&mut session, index);
    let result = session.result_at(index).expect("a result");
    let descents: Vec<f64> = result
        .toolpath()
        .moves
        .iter()
        .filter(|m| m.intent == MoveIntent::EntryPlunge)
        .filter_map(|m| match m.move_type {
            MoveType::Linear { feed_rate } => Some(feed_rate),
            MoveType::Rapid | MoveType::ArcCW { .. } | MoveType::ArcCCW { .. } => None,
        })
        .collect();
    assert!(
        !descents.is_empty(),
        "non-vacuity: the chains start with a plunge"
    );
    for f in descents {
        assert_eq!(f, plunge, "each chain start descends at the shipped plunge");
    }
}

#[test]
fn a_no_tool_generates_a_straight_plunge_and_warns() {
    let tool = op5_tool(ToolFact::Stated(CentreCutting::No(
        NonCentreCuttingGeometry {
            centre_core_diameter_mm: 2.0,
            blade_protrusion_mm: 0.2,
        },
    )));
    let (mut session, index) = case_session(CaseSpec {
        name: "project curve",
        op: project_curve(),
        prior: Prior::None,
        stock_source: StockSource::Fresh,
        frame: Frame::TopUp,
        fixture: Fixture::Standard,
        tool: Some(tool),
    });
    common::session::generate(&mut session, index);
    let result = session.result_at(index).expect("a result (Q6: no refusal)");
    let moves = &result.toolpath().moves;
    assert!(!moves.is_empty(), "the op generates");
    assert!(
        !moves
            .iter()
            .any(|m| matches!(m.intent, MoveIntent::EntryRamp | MoveIntent::EntryHelix)),
        "a StraightPlunge op takes no ramp (K4)"
    );
    assert!(
        moves.iter().any(|m| m.intent == MoveIntent::EntryPlunge),
        "the chain starts plunge"
    );
    let record = result
        .stats
        .non_centre_cutting_plunge
        .expect("the G-CENTRECUT record of a plunge deeper than the protrusion");
    assert!(record.depth_mm > record.max_plunge_depth_mm);
    let exported = session
        .export_gcode_checked(None)
        .expect("the export writes the program (Q6)");
    assert_eq!(
        exported
            .warnings
            .iter()
            .filter(|w| w.id == ids::SAFETY_NON_CENTRE_CUTTING_PLUNGE)
            .count(),
        1,
        "one warning: {:?}",
        exported.warnings
    );
}

#[test]
fn an_unknown_tool_stops_with_the_d12_reason() {
    let tool = op5_tool(ToolFact::Unknown);
    let (mut session, index) = case_session(CaseSpec {
        name: "project curve",
        op: project_curve(),
        prior: Prior::None,
        stock_source: StockSource::Fresh,
        frame: Frame::TopUp,
        fixture: Fixture::Standard,
        tool: Some(tool),
    });
    match session.generate_toolpath(index, &AtomicBool::new(false)) {
        Err(SessionError::Operation(OperationError::NotSet(not_set))) => {
            assert_eq!(not_set.kind, NotSetKind::ToolFact, "{not_set:?}");
            assert!(not_set.fields.contains(&"centre_cutting"), "{not_set:?}");
        }
        other => panic!("D12: an Unknown centre cutting stops generation: {other:?}"),
    }
    assert!(
        session.result_at(index).is_none_or(|r| !r
            .toolpath()
            .moves
            .iter()
            .any(|m| matches!(m.intent, MoveIntent::EntryRamp | MoveIntent::EntryHelix))),
        "no ramp"
    );
}
