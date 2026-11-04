//! D47 (G-RESTSTALE, `planning/adaptive_research_2026-10-23/DEFECTS.md`):
//! the export load gate judges the toolpaths the program emits.
//!
//! The D45 runs refused every G-code export with "cached simulation is
//! stale". The stale row was the enabled operation that the run skipped
//! (`--skip 4`, or an operation in a setup that `--setup` left out). A
//! skipped operation has no result, so the simulation does not carve it and
//! the program does not emit it. The per-operation freshness rule reads "no
//! result" as stale, and the gate refused the whole export on that row.
//!
//! The session here is two enabled 3D roughs on a pocket. Op 0 leaves
//! `LEAVE` on the floor and op 1 cuts it, so each op cuts its own material.
//! The rule does not depend on the strategy or on a rest channel, so any two
//! ops show it:
//! 1. generate op 0, simulate with op 1 skipped, export with NO override:
//!    the export succeeds and emits op 0 only;
//! 2. the same session with nothing skipped exports with no override too;
//! 3. the gate is not weaker: an emitted operation edited after the
//!    simulation still raises stale rows, and the rows name that operation
//!    only.
//!
//! K6 (feeds contract P2, ruling Q6): no row refuses an export now. A stale
//! row is an export warning (`load.*.unmodeled`); a skipped enabled op is
//! left out and named (`export.op_left_out`). Each claim above holds on the
//! warnings.

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use common::meshes::height_field;
use common::session::{mesh_model, stock_over, toolpath_config};
use common::tools::endmill_tool_config;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::{Adaptive3dConfig, ClearingStrategy};
use rs_cam_core::gcode::{EmitError, ExportedGcode};
use rs_cam_core::ids::{ToolId, ToolpathId};
use rs_cam_core::session::{
    AddToolpathArgs, Command, ProjectSession, ProjectSessionBuilder, ReplaceToolpathConfigArgs,
    SimulationOptions, SimulationResolution,
};
use std::sync::atomic::AtomicBool;

// A square pocket in a flat top.
const TOP_Z: f64 = 10.0;
const FLOOR_Z: f64 = 4.0;
const MESH_HALF: f64 = 40.0;
const MESH_STEP: f64 = 0.5;
const POCKET_HALF: f64 = 24.0;
const TOOL_DIAMETER: f64 = 6.0;
const SIM_CELL: f64 = 0.5;
/// Op 0's floor allowance: one `depth_per_pass`, so op 1 has one level to cut.
const LEAVE: f64 = 3.0;

fn rough(stock_to_leave_axial: f64) -> OperationConfig {
    OperationConfig::Adaptive3d(Adaptive3dConfig {
        stepover: Some(2.0),
        depth_per_pass: Some(LEAVE),
        stock_to_leave_axial,
        clearing_strategy: ClearingStrategy::ContourParallel,
        ..Adaptive3dConfig::default()
    })
}

/// Op 0 the rough the program emits, op 1 an enabled rough a run may skip:
/// it cuts the floor allowance op 0 leaves.
fn session() -> ProjectSession {
    let mesh = height_field(MESH_HALF, MESH_STEP, |x, y| {
        if x.abs() <= POCKET_HALF && y.abs() <= POCKET_HALF {
            FLOOR_Z
        } else {
            TOP_Z
        }
    });
    let mut stock = stock_over(MESH_HALF, TOP_Z);
    rs_cam_core::compute::fixture_values::fill_unset_stock(&mut stock);
    let mut builder = ProjectSessionBuilder::new().stock(stock);
    let tool_idx = builder.add_tool(endmill_tool_config(TOOL_DIAMETER));
    let tool_id = ToolId::from_raw(builder.tools()[tool_idx].id.raw());
    let model_id = builder.add_model(mesh_model(mesh, "pocket"));
    let op0 = toolpath_config("rough", rough(LEAVE), tool_id, model_id);
    let _ = builder.add_toolpath(0, op0).expect("add the rough");
    let mut session = builder.build();
    let op1 = toolpath_config("skipped", rough(0.0), tool_id, model_id);
    let _ = session
        .apply(Command::AddToolpath(AddToolpathArgs {
            setup_index: 0,
            config: Box::new(op1),
        }))
        .expect("add the second op");
    session
}

fn generate(session: &mut ProjectSession, index: usize) {
    let cancel = AtomicBool::new(false);
    if let Err(e) = session.generate_toolpath(index, &cancel) {
        panic!("generating toolpath {index}: {e}");
    }
}

fn simulate(session: &mut ProjectSession, skip_ids: Vec<ToolpathId>) {
    let cancel = AtomicBool::new(false);
    session
        .run_simulation(
            &SimulationOptions {
                resolution: SimulationResolution::Fixed(SIM_CELL),
                skip_ids,
                ..SimulationOptions::default()
            },
            &cancel,
        )
        .expect("simulate");
}

fn export(session: &ProjectSession) -> Result<ExportedGcode, EmitError> {
    let trace = session
        .simulation_result()
        .and_then(|s| s.cut_trace.as_deref());
    session.export_gcode_checked(trace)
}

/// The stale load rows of an export: an unmodelled row whose clause says
/// the simulation is stale.
fn stale_rows(exported: &ExportedGcode) -> Vec<String> {
    exported
        .warnings
        .iter()
        .filter(|w| w.id.ends_with(".unmodeled") && w.detail.contains("stale"))
        .map(|w| w.detail.clone())
        .collect()
}

#[test]
fn a_skipped_op_does_not_stale_the_export() {
    // 1. Op 1 is skipped: it has no result and the program does not hold it.
    let mut skipped = session();
    generate(&mut skipped, 0);
    assert!(skipped.result_at(1).is_none(), "the skipped op generated");
    let skip_id = skipped.toolpath_configs()[1].id;
    simulate(&mut skipped, vec![skip_id]);
    let exported = match export(&skipped) {
        Ok(exported) => exported,
        Err(refusal) => panic!("the export with a skipped op refused:\n{refusal}"),
    };
    assert!(
        stale_rows(&exported).is_empty(),
        "the skipped op staled the export: {:?}",
        exported.warnings
    );
    assert!(
        exported.warnings.iter().any(|w| {
            w.id == rs_cam_core::diagnostics::ids::EXPORT_OP_LEFT_OUT && w.toolpath == "skipped"
        }),
        "the export names the op it leaves out: {:?}",
        exported.warnings
    );
    let gcode = exported.gcode;
    assert!(gcode.contains("rough"), "the program does not hold op 0");
    // K6: the header warning names the skipped op; no other line does.
    assert!(
        !gcode
            .lines()
            .any(|line| line.contains("skipped") && !line.contains("WARNING")),
        "the program holds the skipped op"
    );

    // 2. Nothing skipped: both ops export with no override.
    let mut all = session();
    for index in 0..2 {
        generate(&mut all, index);
    }
    simulate(&mut all, Vec::new());
    match export(&all) {
        Ok(exported) => assert!(
            stale_rows(&exported).is_empty(),
            "nothing skipped, nothing stale: {:?}",
            exported.warnings
        ),
        Err(refusal) => panic!("the export with nothing skipped refused:\n{refusal}"),
    }

    // 3. An emitted op edited after the simulation still reads as stale.
    // The edit drops the stored simulation, so the export is handed the
    // trace of the run before the edit, as the GUI holds its own copy.
    let old_trace = skipped
        .simulation_result()
        .and_then(|s| s.cut_trace.clone())
        .expect("the cut trace of the run");
    let mut config = skipped.toolpath_configs()[0].clone();
    if let OperationConfig::Adaptive3d(params) = &mut config.operation {
        params.stepover = params.stepover.map(|s| s * 0.5);
    }
    let _ = skipped
        .apply(Command::ReplaceToolpathConfig(ReplaceToolpathConfigArgs {
            index: 0,
            config: Box::new(config),
        }))
        .expect("edit op 0");
    generate(&mut skipped, 0);
    let edited = format!("toolpath {}:", skipped.toolpath_configs()[0].id);
    let exported = skipped
        .export_gcode_checked(Some(&old_trace))
        .expect("no finding refuses the export");
    let rows = stale_rows(&exported);
    assert!(
        !rows.is_empty() && rows.iter().all(|row| row.contains(&edited)),
        "the stale rows do not name the edited op only: {rows:?}"
    );
}
