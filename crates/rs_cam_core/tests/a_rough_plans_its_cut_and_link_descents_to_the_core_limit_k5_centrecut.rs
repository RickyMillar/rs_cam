//! K5 CentreCut, the plan stage (feeds contract P7 commit C): a 3D Rough
//! with a tool that does not cut at its centre lays every cut and link
//! descent at the tool's limit slope.
//!
//! The case is op 1 of nz-wanaka-21 ("Back bulk rough 18 mm"): the 18 mm 2F
//! end mill with `centre_cutting = { stated = "no",
//! centre_core_diameter_mm = 8.0, blade_protrusion_mm = 1.0 }` (operator
//! file lines 237-258), Geodesic Adaptive, stepover 3.16, depth per pass 9,
//! ramp 3 deg, a stay-down link (operator file lines 604-640). Before
//! commit C only the entries read the limit (`effective_entry_style`,
//! `emit_ramp`); the cut path and the link path draped down a slope as
//! steep as the surface, so the simulation gate
//! (`SimulationResult::non_centre_cutting_descents`) caught them.
//!
//! The fixture is built the wanaka way: a surface at the stock top with a
//! pocket sunk below it. An entry in the pocket ramps out over the surface
//! at the stock top: the G-RAMPTERRAIN clip held each leg there and the leg
//! end dropped to the level, steeper than the limit; and an entry that the
//! drape lifted onto the surface left the drop to the first cut segment.
//! Measured on wanaka op 1 at deb03a1a9: 31 entry ramp strikes and 6 cut
//! strikes. The plate of the G-CENTRECUT sentry and a smooth bump are the
//! no-regression cases.
//!
//! A `StraightPlunge` op (Trace) with the same tool takes no ramp (K4: the
//! contract forbids it; before commit C the dressup fallback ramp added
//! one and hid the plunges). Each descent deeper than the protrusion gives
//! the G-CENTRECUT warning at generation (`safety.non_centre_cutting_plunge`)
//! and a CentreCut row in the lineage.
//!
//! Red: remove the plan clamp (`dressup::entry_descent::replan_descent_at_limit`
//! not called, and `adaptive3d::path::clamp_descents_to_limit` returning its
//! input). The pocket case then has core strikes.

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::{
    Adaptive3dConfig, Adaptive3dEntryStyle, ClearingStrategy,
};
use rs_cam_core::compute::tool_config::{
    CentreCutting, NonCentreCuttingGeometry, ToolConfig, ToolFact, ToolId, ToolType,
};
use rs_cam_core::diagnostics::ids;
use rs_cam_core::mesh::{TriangleMesh, make_test_flat};
use rs_cam_core::session::{ProjectSession, SimulationOptions, SimulationResolution};

/// Half the stock width (mm), as the G-CENTRECUT sentry.
const HALF: f64 = 30.0;
/// The stock top (mm), as the G-CENTRECUT sentry.
const STOCK_TOP_Z: f64 = 12.0;

/// Tool 6 of nz-wanaka-21 (operator file lines 237-258): diameter 18.0,
/// flute 20 (estimated), shank 6 (estimated), stickout 25 (estimated),
/// 2 flutes (estimated), centre core 8, blade protrusion 1 (stated).
fn bulk_18(centre: bool) -> ToolConfig {
    let basis = "bare number before K3 (2026-11)";
    let mut tool = ToolConfig::test_default(ToolId::from_raw(0), ToolType::EndMill);
    tool.name = "18 mm 2F".to_owned();
    tool.diameter = 18.0;
    tool.set_cutting_length(ToolFact::estimated(20.0, basis));
    tool.set_shank_diameter(ToolFact::estimated(6.0, basis));
    tool.set_stickout(ToolFact::estimated(25.0, basis));
    tool.set_flute_count(ToolFact::estimated(2, basis));
    tool.set_centre_cutting(ToolFact::Stated(if centre {
        CentreCutting::Yes
    } else {
        CentreCutting::No(NonCentreCuttingGeometry {
            centre_core_diameter_mm: 8.0,
            blade_protrusion_mm: 1.0,
        })
    }));
    tool
}

/// Op 1 (operator file lines 625-640): stepover 3.16, depth per pass 9,
/// leave 0, feed 4200, ramp 3 deg, Geodesic Adaptive; the stay-down link
/// at its default reach.
fn op1() -> OperationConfig {
    OperationConfig::Adaptive3d(Adaptive3dConfig {
        stepover: Some(3.16),
        depth_per_pass: Some(9.0),
        stock_to_leave_axial: 0.0,
        feed_rate: Some(4200.0),
        entry_style: Adaptive3dEntryStyle::Ramp,
        ramp_angle_deg: 3.0,
        clearing_strategy: ClearingStrategy::GeodesicAdaptive,
        max_stay_down_distance_mm: None,
        ..Adaptive3dConfig::default()
    })
}

/// The rough of `mesh` with `tool`, generated and simulated.
fn rough(mesh: TriangleMesh, tool: ToolConfig) -> ProjectSession {
    let mut session = common::session::single_op_session_with(
        common::session::stock_over(HALF, STOCK_TOP_Z),
        tool,
        common::session::mesh_model(mesh, "k5_centrecut"),
        "Back bulk rough 18 mm",
        op1(),
        |tc| tc.dressups.arc_fitting = None,
    );
    common::session::generate(&mut session, 0);
    let opts = SimulationOptions {
        resolution: SimulationResolution::Fixed(0.5),
        adaptive_feed_modulation: false,
        ..SimulationOptions::default()
    };
    session
        .run_simulation(&opts, &AtomicBool::new(false))
        .expect("the simulation runs");
    session
}

/// The dome: a smooth bump in the middle of the plate, 8 mm high under the
/// 12 mm stock top, `8 exp(-(r / 10)^2)`. Its flanks are steeper than the
/// limit of the 18 mm bit (`atan(1 / 8)`).
fn dome() -> TriangleMesh {
    common::meshes::height_field(HALF, 1.0, |x, y| 8.0 * (-(x * x + y * y) / 100.0).exp())
}

/// The wanaka shape: a surface at the stock top (Z 12) with a 40 mm square
/// pocket sunk to Z 2, the walls one grid cell wide.
fn pocket_block() -> TriangleMesh {
    common::meshes::height_field(HALF, 1.0, |x, y| {
        if x.abs() < 20.0 && y.abs() < 20.0 {
            2.0
        } else {
            STOCK_TOP_Z
        }
    })
}

#[track_caller]
fn assert_inside_the_limit(session: &ProjectSession, case: &str) {
    let result = session.result_at(0).expect("a result");
    assert!(
        result.toolpath().moves.len() > 20,
        "{case}: the rough cuts ({} moves)",
        result.toolpath().moves.len()
    );
    let sim = session.simulation_result().expect("a simulation");
    assert!(
        sim.non_centre_cutting_descents.is_empty(),
        "{case}: {} core strikes, first {:?}",
        sim.non_centre_cutting_descents.len(),
        sim.non_centre_cutting_descents.first()
    );
    let trace = sim.cut_trace.clone();
    let exported = session
        .export_gcode_checked(trace.as_deref())
        .expect("the export writes the program (Q6)");
    assert!(
        !exported.warnings.iter().any(|w| {
            w.id == ids::SAFETY_NON_CENTRE_CUTTING_DESCENT
                || w.id == ids::SAFETY_NON_CENTRE_CUTTING_PLUNGE
        }),
        "{case}: no CentreCut warning: {:?}",
        exported.warnings
    );
}

#[test]
fn the_plate_rough_keeps_its_limit() {
    let session = rough(make_test_flat(2.0 * HALF), bulk_18(false));
    assert_inside_the_limit(&session, "plate");
}

#[test]
fn the_pocket_rough_lays_its_entries_and_cuts_at_the_limit() {
    let session = rough(pocket_block(), bulk_18(false));
    assert_inside_the_limit(&session, "pocket");
}

#[test]
fn the_bump_rough_keeps_its_limit() {
    let session = rough(dome(), bulk_18(false));
    assert_inside_the_limit(&session, "bump");
}

/// A centre-cutting tool keeps the drape: its toolpath is the toolpath of
/// the rough before commit C (the clamp reads only a `No` fact).
#[test]
fn a_centre_cutting_tool_keeps_the_surface_slope() {
    let yes = rough(pocket_block(), bulk_18(true));
    let no = rough(pocket_block(), bulk_18(false));
    let steepest = |session: &ProjectSession| {
        session
            .result_at(0)
            .expect("a result")
            .toolpath()
            .moves
            .windows(2)
            .filter(|w| {
                matches!(
                    w[1].move_type,
                    rs_cam_core::toolpath::MoveType::Linear { .. }
                )
            })
            .map(|w| {
                let (a, b) = (w[0].target, w[1].target);
                let dxy = (b.x - a.x).hypot(b.y - a.y);
                if dxy > 1e-6 { (a.z - b.z) / dxy } else { 0.0 }
            })
            .fold(0.0_f64, f64::max)
    };
    assert!(
        steepest(&yes) > steepest(&no),
        "the Yes tool keeps its steep descents; the No tool is held to the limit"
    );
}

/// K5 CentreCut on a `StraightPlunge` op: a Trace with the 18 mm `No` tool
/// keeps its plunges (no fallback ramp), generates and exports (Q6), and
/// names each plunge deeper than the protrusion.
#[test]
fn a_straight_plunge_op_with_a_no_tool_warns_on_each_deep_plunge() {
    use rs_cam_core::compute::catalog::OperationType;
    use rs_cam_core::compute::config::StockSource;
    use rs_cam_core::compute::feasibility::PhysicalRule;
    use rs_cam_core::toolpath::MoveIntent;
    let (mut session, index) = common::op_fixtures::case_session(common::op_fixtures::CaseSpec {
        name: "trace",
        // A trace 3 mm deep: deeper than the 1 mm protrusion (the fixture
        // depth, 1 mm, is not).
        op: {
            let mut op = OperationConfig::new_default(OperationType::Trace);
            if let OperationConfig::Trace(cfg) = &mut op {
                cfg.depth = Some(3.0);
            }
            op
        },
        prior: common::op_fixtures::Prior::None,
        stock_source: StockSource::Fresh,
        frame: common::op_fixtures::Frame::TopUp,
        fixture: common::op_fixtures::Fixture::Standard,
        tool: Some(bulk_18(false)),
    });
    common::session::generate(&mut session, index);
    let result = session.result_at(index).expect("a result");
    let moves = &result.toolpath().moves;
    assert!(!moves.is_empty(), "the trace generates (Q6)");
    assert!(
        !moves
            .iter()
            .any(|m| matches!(m.intent, MoveIntent::EntryRamp | MoveIntent::EntryHelix)),
        "a StraightPlunge op takes no ramp (K4)"
    );
    let plunge = result
        .stats
        .non_centre_cutting_plunge
        .expect("the G-CENTRECUT record of the deep plunge");
    assert!(plunge.depth_mm > plunge.max_plunge_depth_mm);
    let row = result
        .lineage
        .rules
        .iter()
        .find(|r| r.rule() == PhysicalRule::CentreCut)
        .expect("the CentreCut row in the lineage");
    assert!(
        row.text().contains("deeper than the 1 mm"),
        "{}",
        row.text()
    );
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
        "one G-CENTRECUT row, not doubled by the rule row: {:?}",
        exported.warnings
    );
}
