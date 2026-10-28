//! W2b (D69 §3): the session fills the wall step, and a real rest Waterline
//! after a 3D Rough widens its step on a vertical wall.
//!
//! Pre-registration:
//! `planning/adaptive_research_2026-10-23/integration/W2_PREREG.md`; the
//! session-level sentry is the lead's condition of 2026-10-28. The session
//! fills `ExecutionContext::material` (the stock material) and
//! `rest_axial_step_mm` (the smallest axial step of the earlier enabled
//! roughing ops of the setup, on an op that cuts the remaining stock).
//!
//! The fixture: a square boss with vertical walls (20 x 20 x 15 mm). The
//! 3D Rough offsets in Z alone (it leaves no radial stock on a vertical
//! wall), so toolpath 0, a 3D Rough at the registry default of 3 mm per
//! pass, roughs a boss [`SKIN`] wider on each side: it leaves a radial
//! skin of [`SKIN`] on the walls. Toolpath 1 is a helical rest Waterline
//! of the real boss at 0.5 mm per level.
//!
//! 1. Control: in a stock whose material has no force line (particleboard,
//!    `Material::force_line` refuses), the deflection check fails, and the
//!    Waterline reaches every level of its ladder (the W1 output).
//! 2. In hard maple the Waterline keeps fewer rings, and its simulated peak
//!    axial engagement is at most the rough's step.

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use std::sync::atomic::AtomicBool;

use common::session::{mesh_model, pinned_heights, stock_over, toolpath_config};
use common::tools::endmill_tool_config;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::StockSource;
use rs_cam_core::compute::operation_configs::WaterlineConfig;
use rs_cam_core::geo::P3;
use rs_cam_core::material::{Material, SheetGoodKind, WoodSpecies};
use rs_cam_core::mesh::TriangleMesh;
use rs_cam_core::session::{ProjectSession, ProjectSessionBuilder, SimulationOptions};
use rs_cam_core::toolpath::Toolpath;

const BOSS_HALF: f64 = 10.0;
/// The radial skin (mm) the rough leaves on each wall.
const SKIN: f64 = 0.5;
const BOSS_TOP: f64 = 15.0;
const LEVEL_TOP: f64 = 14.5;
/// Above the web the rough leaves (its 0.5 mm axial leave on the model
/// bottom): W4 keeps the outer ring of a through part off the levels at or
/// below the web, and this file pins the wall step, not the web.
const LEVEL_BOTTOM: f64 = 1.0;
const Z_STEP: f64 = 0.5;

/// A closed box of half width `h`: the boss.
fn boss(h: f64) -> TriangleMesh {
    let (z0, z1) = (0.0, BOSS_TOP);
    let v = vec![
        P3::new(-h, -h, z1),
        P3::new(h, -h, z1),
        P3::new(h, h, z1),
        P3::new(-h, h, z1),
        P3::new(-h, -h, z0),
        P3::new(h, -h, z0),
        P3::new(h, h, z0),
        P3::new(-h, h, z0),
    ];
    let t = vec![
        [0, 1, 2],
        [0, 2, 3],
        [4, 6, 5],
        [4, 7, 6],
        [0, 4, 5],
        [0, 5, 1],
        [1, 5, 6],
        [1, 6, 2],
        [2, 6, 7],
        [2, 7, 3],
        [3, 7, 4],
        [3, 4, 0],
    ];
    TriangleMesh::from_raw(v, t)
}

/// The rough, then the rest Waterline, in a stock of `material`.
fn session(material: Material) -> (ProjectSession, f64) {
    let mut stock = stock_over(BOSS_HALF + 8.0, BOSS_TOP + 2.0);
    stock.material = material;
    let mut builder = ProjectSessionBuilder::new().stock(stock);
    // A short stickout: a stiff tool, so the deflection check passes.
    let mut tool = endmill_tool_config(6.0);
    tool.stickout = 20.0;
    let tool_idx = builder.add_tool(tool);
    let tool_id = rs_cam_core::ids::ToolId::from_raw(builder.tools()[tool_idx].id.raw());
    let model_id = builder.add_model(mesh_model(boss(BOSS_HALF), "w2b_boss"));
    let wider = mesh_model(boss(BOSS_HALF + SKIN), "w2b_boss_with_skin");
    let rough_model = builder.add_model(wider);

    let rough = toolpath_config(
        "rough",
        OperationConfig::new_default(OperationType::Adaptive3d),
        tool_id,
        rough_model,
    );
    let rough_step = rough
        .operation
        .depth_per_pass()
        .expect("the 3D Rough states a step");
    let _ = builder.add_toolpath(0, rough).expect("add the rough");

    let mut waterline = toolpath_config(
        "waterline",
        OperationConfig::Waterline(WaterlineConfig {
            z_step: Z_STEP,
            sampling: 0.5,
            ..WaterlineConfig::default()
        }),
        tool_id,
        model_id,
    );
    waterline.heights = pinned_heights(LEVEL_TOP, LEVEL_BOTTOM);
    // The op output: no dressup merges or fits the laps, so each level the
    // helix reaches stays a vertex at its own Z.
    waterline.dressups = common::op_fixtures::no_dressups();
    waterline.stock_source = StockSource::FromRemainingStock;
    let _ = builder
        .add_toolpath(0, waterline)
        .expect("add the waterline");
    (builder.build(), rough_step)
}

fn simulate(session: &mut ProjectSession) {
    let cancel = AtomicBool::new(false);
    let _ = session
        .run_simulation(&SimulationOptions::default(), &cancel)
        .expect("the session simulates");
}

/// Generate the rough, simulate, generate the Waterline, simulate.
fn run(material: Material) -> (ProjectSession, f64) {
    let (mut s, rough_step) = session(material);
    let cancel = AtomicBool::new(false);
    let _ = s
        .generate_toolpath(0, &cancel)
        .expect("the rough generates");
    simulate(&mut s);
    let _ = s
        .generate_toolpath(1, &cancel)
        .expect("the waterline generates");
    simulate(&mut s);
    (s, rough_step)
}

/// The ladder levels that the toolpath reaches exactly (a flat lap or the
/// end of a descent).
fn levels_reached(tp: &Toolpath) -> usize {
    rs_cam_core::ops::waterline::waterline_z_levels(LEVEL_TOP, LEVEL_BOTTOM, Z_STEP)
        .iter()
        .filter(|&&z| {
            tp.moves
                .iter()
                .any(|m| m.move_type.is_cutting() && (m.target.z - z).abs() < 1e-9)
        })
        .count()
}

#[test]
fn a_rest_waterline_after_a_rough_keeps_fewer_rings_w2b() {
    let ladder = rs_cam_core::ops::waterline::waterline_z_levels(LEVEL_TOP, LEVEL_BOTTOM, Z_STEP);
    let (w1, _) = run(Material::SheetGood {
        kind: SheetGoodKind::Particleboard,
    });
    let w1_levels = levels_reached(w1.result_at(1).expect("a result").toolpath());
    assert_eq!(
        w1_levels,
        ladder.len(),
        "control: with no force line the helix reaches every level"
    );

    let (w2, rough_step) = run(Material::SolidWood {
        species: WoodSpecies::HardMaple,
    });
    let w2_levels = levels_reached(w2.result_at(1).expect("a result").toolpath());
    assert!(
        w2_levels < w1_levels,
        "the wall step keeps fewer rings: {w2_levels} of {w1_levels}"
    );

    let id = w2.toolpath_configs()[1].id;
    let summary = w2
        .simulation_result()
        .and_then(|sim| sim.cut_trace.as_deref())
        .and_then(|trace| {
            trace
                .toolpath_summaries
                .iter()
                .find(|s| s.toolpath_id == id)
        })
        .expect("the waterline has a simulated summary")
        .clone();
    assert!(
        summary.peak_axial_doc_mm <= rough_step + 1e-6,
        "peak axial {:.3} mm is above the rough step {rough_step} mm",
        summary.peak_axial_doc_mm
    );
}
