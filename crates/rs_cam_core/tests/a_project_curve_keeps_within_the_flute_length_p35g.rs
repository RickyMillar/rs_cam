//! P35 part G: a Project Curve keeps each sample within the flute length
//! of its tool (`ProjectCurveConfig::keep_within_flute_length`).
//!
//! Operator ruling 2026-11-16 ("Shallower V there"): where the V-bit is
//! deeper in the wood than its cone, the op lifts the tool so that it is
//! never deeper than its cutting length.
//!
//! The fixture: a V-bit (60 degrees, 6.35 mm, cutting length 5.0 mm), a
//! curve along Y 0 from X -40 to X 40 with its full depth at Z 8, in a
//! board with its top at Z 20. A pocket before it cuts a floor at Z 12
//! over X -45 to X 20. Under the floor the wood over the tip is 4 mm
//! (within the flute); under the full height it is 12 mm.
//!
//! # The claims
//!
//! - (a) Switch off (the path of before): the simulation finds flute
//!   overreach on the op. Switch on: it finds none; under the full height
//!   the tip is at `20 - L + 0.1`; away from the step the path is the path
//!   with the switch off; no sample goes down. The op reports the count,
//!   the largest lift and its position, as one warning.
//! - (b) Switch on with a Fresh stock source refuses, and the text names
//!   the remaining stock.
//! - (c) With `depth_per_pass` and `lateral_offset`: no flute overreach,
//!   and no fed move is below the lifted full-depth path.
//! - (d) No key and `false` give the same moves; no file gains the key.
//!
//! Red proof: before P35 G the parameter door refuses
//! `keep_within_flute_length` ((a), (b), (c) fail at the write).
//!
//! ```text
//! cargo test -p rs_cam_core -q --test a_project_curve_keeps_within_the_flute_length_p35g
//! ```

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::print_stderr,
    reason = "test code: a panic is how a test reports a failure; the measured figures are the record of the step"
)]

mod common;

use std::sync::atomic::AtomicBool;

use common::meshes::height_field;
use common::op_fixtures::{run_prior, simulate};
use common::session::{mesh_model, pinned_heights, polygon_model, toolpath_config};
use common::tools::endmill_tool_config;
use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::StockSource;
use rs_cam_core::compute::operation_configs::{PocketConfig, ProjectCurveConfig};
use rs_cam_core::compute::tool_config::{CentreCutting, ToolConfig, ToolFact, ToolId, ToolType};
use rs_cam_core::diagnostics::Severity;
use rs_cam_core::geo::P2;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{Command, ProjectSession, ProjectSessionBuilder, SetToolpathParamArgs};
use rs_cam_core::toolpath::{MoveType, Toolpath};
use serde_json::json;

const STOCK_TOP: f64 = 20.0;
const FLOOR_Z: f64 = 12.0;
const FULL_Z: f64 = 8.0;
/// The stated cutting length of the V-bit (mm).
const FLUTE: f64 = 5.0;
/// The margin of the lift (mm): the clearance of the air-cut filter.
const MARGIN: f64 = 0.1;
/// The pocket ends at X 20: the full height starts there.
const STEP_X: f64 = 20.0;
/// The reach of the rule from a sample: the envelope radius of the V-bit
/// (6.35 / 2) plus the sample spacing of the op (0.5).
const REACH: f64 = 6.35 / 2.0 + 0.5;

fn board() -> StockConfig {
    StockConfig {
        x: 100.0,
        y: 40.0,
        z: Some(STOCK_TOP),
        origin_x: -50.0,
        origin_y: -20.0,
        origin_z: 0.0,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

fn vbit() -> ToolConfig {
    let mut t = ToolConfig::test_default(ToolId::from_raw(0), ToolType::VBit);
    t.name = "V-bit 60 deg".to_owned();
    t.diameter = 6.35;
    t.shaft_diameter = 6.35;
    t.included_angle = 60.0;
    t.set_cutting_length(ToolFact::Stated(FLUTE));
    t.set_shank_diameter(ToolFact::Stated(6.35));
    t.set_stickout(ToolFact::Stated(30.0));
    t.set_flute_count(ToolFact::Stated(2));
    t.set_centre_cutting(ToolFact::Stated(CentreCutting::Yes));
    t.set_v_tip_diameter(ToolFact::Stated(0.0));
    t
}

/// The session (the pocket is toolpath 0, the curve op toolpath 1). Each
/// parameter goes through `SetToolpathParam`.
fn session(
    source: StockSource,
    params: Vec<(&'static str, serde_json::Value)>,
) -> Result<ProjectSession, String> {
    let mut builder = ProjectSessionBuilder::new().stock(board());
    let mill = builder.add_tool(endmill_tool_config(6.0));
    let mill_id = rs_cam_core::ids::ToolId::from_raw(builder.tools()[mill].id.raw());
    let v = builder.add_tool(vbit());
    let v_id = rs_cam_core::ids::ToolId::from_raw(builder.tools()[v].id.raw());
    let pocket_model = builder.add_model(polygon_model(
        vec![Polygon2::new(vec![
            P2::new(-45.0, -10.0),
            P2::new(STEP_X, -10.0),
            P2::new(STEP_X, 10.0),
            P2::new(-45.0, 10.0),
        ])],
        "p35g_pocket",
    ));
    let surface = builder.add_model(mesh_model(
        height_field(60.0, 5.0, |_, _| FULL_Z + 1.0),
        "p35g_plate",
    ));
    let curve = builder.add_model(polygon_model(
        vec![Polygon2::open_path(vec![
            P2::new(-40.0, 0.0),
            P2::new(40.0, 0.0),
        ])],
        "p35g_curve",
    ));
    let _ = builder
        .add_toolpath(
            0,
            toolpath_config(
                "pocket",
                OperationConfig::Pocket(PocketConfig {
                    stepover: Some(2.0),
                    floor: rs_cam_core::compute::floor::Floor::top(STOCK_TOP - FLOOR_Z),
                    depth_per_pass: Some(4.0),
                    ..PocketConfig::default()
                }),
                mill_id,
                pocket_model,
            ),
        )
        .expect("add the pocket");
    let mut op = toolpath_config(
        "v groove",
        OperationConfig::ProjectCurve(ProjectCurveConfig {
            depth: Some(1.0),
            surface_model_id: Some(surface),
            ..ProjectCurveConfig::default()
        }),
        v_id,
        curve,
    );
    op.heights = pinned_heights(STOCK_TOP, 0.0);
    op.stock_source = source;
    let _ = builder.add_toolpath(0, op).expect("add the curve op");
    let mut session = builder.build();
    for (param, value) in params {
        let _ = session
            .apply(Command::SetToolpathParam(SetToolpathParamArgs {
                index: 1,
                param: param.to_owned(),
                value,
            }))
            .map_err(|e| format!("`{param}`: {e}"))?;
    }
    Ok(session)
}

/// What one run gives: the stored path, the flute overreach records of the
/// simulation for the op (the wood over the tip of each, and the flute
/// length the simulation read), and the session.
struct Run {
    session: ProjectSession,
    tp: Toolpath,
    overreach: Vec<f64>,
    flute_length: Option<f64>,
}

fn run(session: Result<ProjectSession, String>) -> Result<Run, String> {
    let mut session = session?;
    run_prior(&mut session);
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(1, &cancel)
        .map_err(|e| e.to_string())?;
    let tp = session
        .result_at(1)
        .expect("a result")
        .annotated()
        .toolpath
        .clone();
    simulate(&mut session);
    let id = session.toolpath_configs()[1].id;
    let sim = session.simulation_result().expect("a simulation");
    let records: Vec<_> = sim
        .flute_overreach
        .iter()
        .filter(|r| r.toolpath_id == id)
        .collect();
    let overreach = records
        .iter()
        .map(|r| r.overreach.material_over_tip_mm)
        .collect();
    let flute_length = records.first().map(|r| r.overreach.flute_length_mm);
    Ok(Run {
        session,
        tp,
        overreach,
        flute_length,
    })
}

/// The fed moves of `tp`: X and Z, in order.
fn fed(tp: &Toolpath) -> Vec<(f64, f64)> {
    tp.moves
        .iter()
        .filter(|m| m.move_type != MoveType::Rapid)
        .map(|m| (m.target.x, m.target.z))
        .collect()
}

/// (a)
#[test]
fn the_switch_lifts_the_tool_out_of_the_flute_overreach_p35g() {
    let off = run(session(StockSource::FromRemainingStock, vec![])).expect("the op generates");
    assert!(
        !off.overreach.is_empty(),
        "non-vacuity: with the switch off the simulation finds flute overreach"
    );
    let flute = off.flute_length.expect("the simulation states the flute");
    assert!((flute - FLUTE).abs() < 1e-9, "the flute is {flute} mm");
    eprintln!(
        "switch off: {} moves with flute overreach, the worst {:.2} mm of wood over the tip",
        off.overreach.len(),
        off.overreach.iter().copied().fold(0.0_f64, f64::max)
    );

    let on = run(session(
        StockSource::FromRemainingStock,
        vec![("keep_within_flute_length", json!(true))],
    ))
    .expect("the op generates with the switch");
    assert!(
        on.overreach.is_empty(),
        "switch on: the simulation finds flute overreach: {:?}",
        on.overreach
    );

    // The path, sample by sample. The two runs have the same samples.
    let (before, after) = (fed(&off.tp), fed(&on.tp));
    assert_eq!(before.len(), after.len(), "the same fed moves");
    let lifted_z = STOCK_TOP - FLUTE + MARGIN;
    let (mut lifted, mut equal) = (0, 0);
    for ((x0, z0), (x1, z1)) in before.iter().zip(&after) {
        assert!((x0 - x1).abs() < 1e-9, "the same XY");
        assert!(*z1 >= *z0 - 1e-9, "no sample goes down: {z0} to {z1}");
        if *x0 > STEP_X + REACH {
            // Under the full height: the plunge to the first point and the
            // cut are at the lifted Z, or above it on the way down.
            assert!(
                *z1 >= lifted_z - 1e-6,
                "at X {x0} the tip is at Z {z1}; the bound is {lifted_z}"
            );
            if (*z1 - lifted_z).abs() < 1e-6 {
                lifted += 1;
            }
        } else if *x0 < STEP_X - REACH - 0.5 {
            // Away from the step (one grid cell more): not touched.
            assert!(
                (z0 - z1).abs() < 1e-12,
                "at X {x0} the path moved: Z {z0} to {z1}"
            );
            equal += 1;
        }
    }
    assert!(
        lifted > 20 && equal > 60,
        "{lifted} lifted, {equal} equal samples"
    );

    // The report: one note, one warning.
    let result = on.session.result_at(1).expect("a result");
    let finding = result
        .stats
        .tab_placement
        .clone()
        .expect("the op reports the lift");
    let text = finding.message();
    eprintln!("{text}");
    let largest = lifted_z - FULL_Z;
    assert!(
        text.contains("Project Curve")
            && text.contains(&format!("{largest:.3} mm"))
            && text.contains("flute"),
        "{text}"
    );
    let diagnostics = on
        .session
        .diagnose_toolpath_with_trace(1, None)
        .expect("diagnose");
    let warning = diagnostics
        .iter()
        .find(|d| d.id.as_str() == "geom.lifted_within_flute")
        .expect("the lift is a diagnostic");
    assert_eq!(warning.severity, Severity::Caution);
    assert!(
        !diagnostics
            .iter()
            .any(|d| d.id.as_str() == "geom.profile_loop_split"
                || d.id.as_str() == "geom.tab_placement"),
        "a lift is no split and no tab remark"
    );
    // With the switch off the op reports nothing.
    assert!(
        off.session
            .result_at(1)
            .expect("a result")
            .stats
            .tab_placement
            .is_none()
    );
}

/// (b)
#[test]
fn the_switch_refuses_a_fresh_stock_source_p35g() {
    let text = run(session(
        StockSource::Fresh,
        vec![("keep_within_flute_length", json!(true))],
    ))
    .err()
    .expect("the switch needs the remaining stock");
    assert!(
        text.contains("keep_within_flute_length") && text.contains("remaining stock"),
        "{text}"
    );
}

/// (c)
#[test]
fn the_ladder_and_the_lateral_offset_keep_within_the_flute_p35g() {
    let plain = run(session(
        StockSource::FromRemainingStock,
        vec![
            ("keep_within_flute_length", json!(true)),
            ("lateral_offset", json!(0.6)),
        ],
    ))
    .expect("the one-pass op generates");
    let laddered = run(session(
        StockSource::FromRemainingStock,
        vec![
            ("keep_within_flute_length", json!(true)),
            ("lateral_offset", json!(0.6)),
            ("depth_per_pass", json!(2.0)),
        ],
    ))
    .expect("the laddered op generates");
    for r in [&plain, &laddered] {
        assert!(r.overreach.is_empty(), "flute overreach: {:?}", r.overreach);
        for m in r.tp.moves.iter().filter(|m| m.move_type != MoveType::Rapid) {
            assert!((m.target.y - 0.6).abs() < 1e-9, "the path is at Y +0.6");
        }
    }
    // The lowest fed Z at each X of the full-depth pass bounds the ladder.
    let floor_of = |x: f64| {
        fed(&plain.tp)
            .iter()
            .filter(|(px, _)| (px - x).abs() < 1e-6)
            .map(|(_, z)| *z)
            .fold(f64::INFINITY, f64::min)
    };
    let passes = fed(&laddered.tp);
    assert!(
        passes.len() > fed(&plain.tp).len(),
        "non-vacuity: the ladder adds passes"
    );
    for (x, z) in &passes {
        let floor = floor_of(*x);
        assert!(
            floor.is_infinite() || *z >= floor - 1e-6,
            "at X {x} a pass is at Z {z}, below the lifted full depth {floor}"
        );
    }
}

/// (d)
#[test]
fn no_key_and_false_cut_the_same_path_p35g() {
    let moves = |params| {
        format!(
            "{:?}",
            run(session(StockSource::FromRemainingStock, params))
                .expect("generates")
                .tp
                .moves
        )
    };
    assert_eq!(
        moves(vec![]),
        moves(vec![("keep_within_flute_length", json!(false))])
    );
    let text = toml::to_string(&ProjectCurveConfig::default()).expect("serialises");
    assert!(!text.contains("keep_within_flute_length"), "{text}");
}
