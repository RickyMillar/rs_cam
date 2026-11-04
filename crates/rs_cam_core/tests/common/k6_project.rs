//! The K6 project (feeds contract P2): one project with each finding that
//! refused a job before P2. The core sentry
//! (`a_job_with_each_finding_exports_and_names_it_k6`), the viz twin and the
//! CLI test include this file by `#[path]`, so it reads only `rs_cam_core`.
//!
//! The stock top is Z 0 and the stock bottom Z -12. The ops:
//!
//! 0. `Inverted`: a Pocket whose clearance plane is below its retract plane
//!    (`geom.clearance_z_below_retract_z`).
//! 1. `Gouge`: a Face on a plate model at Z 0, 1 mm deep, so it cuts into
//!    the model (`safety.cuts_into_model`).
//! 2. `Plunge`: a Pocket too small for a ramp, cut with a tool that does not
//!    cut at its centre. The entry plunges 3.5 mm into stock
//!    (`safety.non_centre_cutting_plunge`), and the simulation shows the
//!    strike (`safety.non_centre_cutting_descent`).
//! 3. `Left out`: an enabled Pocket that the test does not generate
//!    (`export.op_left_out`).
//!
//! The load rows (an exceedance, a drill `Critical`, a stale simulation)
//! come from a hand-built report: [`load_report`].

#![allow(
    dead_code,
    reason = "each test binary that includes this file uses only part of it"
)]
#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::tool_config::ToolFact;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{
    BoundaryConfig, DressupConfig, DressupEntryStyle, HeightMode, HeightsConfig, StockSource,
};
use rs_cam_core::compute::operation_configs::{FaceConfig, PocketConfig};
use rs_cam_core::compute::tool_config::{
    CentreCutting, NonCentreCuttingGeometry, ToolConfig, ToolId, ToolType,
};
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::geo::P2;
use rs_cam_core::ids::{ModelId, ToolpathId};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{
    LoadedModel, ProjectSession, ProjectSessionBuilder, SimulationOptions, SimulationResolution,
    ToolpathConfig,
};
use rs_cam_core::trace::debug_trace::ToolpathDebugOptions;

/// The half width of the stock and of the plate model (mm).
pub const HALF: f64 = 30.0;
/// The stock thickness (mm). The stock top is Z 0.
pub const THICKNESS: f64 = 12.0;
/// The index of each op.
pub const INVERTED: usize = 0;
pub const GOUGE: usize = 1;
pub const PLUNGE: usize = 2;
pub const LEFT_OUT: usize = 3;

/// The toolpath names, in index order.
pub const NAMES: [&str; 4] = ["Inverted", "Gouge", "Plunge", "Left out"];

fn square(cx: f64, cy: f64, half: f64) -> Polygon2 {
    Polygon2::new(vec![
        P2::new(cx - half, cy - half),
        P2::new(cx + half, cy - half),
        P2::new(cx + half, cy + half),
        P2::new(cx - half, cy + half),
    ])
}

fn model(
    name: &str,
    mesh: Option<rs_cam_core::mesh::TriangleMesh>,
    polygons: Vec<Polygon2>,
) -> LoadedModel {
    let ext = if mesh.is_some() { "stl" } else { "svg" };
    LoadedModel {
        id: ModelId::from_raw(0),
        name: name.to_owned(),
        mesh: mesh.map(Arc::new),
        items: Arc::new(rs_cam_core::selection::ItemSet::from_polygons(polygons)),
        path: PathBuf::from(format!("synthetic://{name}.{ext}")),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
        import_options: Default::default(),
        import_report: None,
        drawing: Default::default(),
    }
}

/// A 6 mm two-flute end mill. `non_centre_cutting` gives it a 2.4 mm core
/// and blades 0.5 mm below the centre.
fn end_mill(non_centre_cutting: bool) -> ToolConfig {
    let mut tool = ToolConfig::test_default(ToolId::from_raw(0), ToolType::EndMill);
    tool.name = if non_centre_cutting {
        "6 mm bulk"
    } else {
        "6 mm"
    }
    .to_owned();
    tool.diameter = 6.0;
    tool.set_cutting_length(ToolFact::Stated(20.0));
    tool.set_stickout(ToolFact::Stated(30.0));
    tool.set_shank_diameter(ToolFact::Stated(6.0));
    tool.set_flute_count(ToolFact::Stated(2));
    if non_centre_cutting {
        tool.set_centre_cutting(ToolFact::Stated(CentreCutting::No(
            NonCentreCuttingGeometry {
                centre_core_diameter_mm: 2.4,
                blade_protrusion_mm: 0.5,
            },
        )));
    }
    tool
}

fn config(
    name: &str,
    op: OperationConfig,
    tool_id: rs_cam_core::ids::ToolId,
    model_id: ModelId,
) -> ToolpathConfig {
    let op_type = op.op_type();
    let mut op = op;
    rs_cam_core::compute::fixture_values::fill_unset(&mut op);
    let mut dressups = DressupConfig::for_op(op_type);
    rs_cam_core::compute::fixture_values::fill_unset_dressups(&mut dressups);
    ToolpathConfig {
        shoe: Default::default(),
        feeds_lock: Default::default(),
        id: ToolpathId::from_raw(0),
        name: name.to_owned(),
        enabled: true,
        operation: op,
        dressups,
        heights: HeightsConfig::default(),
        tool_id: Some(tool_id),
        model_id,
        pre_gcode: None,
        post_gcode: None,
        boundary: BoundaryConfig::default(),
        boundary_inherit: true,
        stock_source: StockSource::default(),
        coolant: CoolantMode::Off,
        debug_options: ToolpathDebugOptions::default(),
        feeds_provenance: rs_cam_core::feeds::FeedsProvenance::default(),
        rest_analysis: rs_cam_core::compute::config::RestAnalysisConfig::default(),
        planner_origin: None,
    }
}

fn pocket() -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        depth_per_pass: Some(6.0),
        feed_rate: Some(1000.0),
        ..PocketConfig::default()
    })
}

/// The project, with no result and no simulation.
pub fn project() -> ProjectSession {
    let mut stock = StockConfig {
        x: 2.0 * HALF + 4.0,
        y: 2.0 * HALF + 4.0,
        z: Some(THICKNESS),
        origin_x: -HALF - 2.0,
        origin_y: -HALF - 2.0,
        origin_z: -THICKNESS,
        auto_from_model: false,
        ..StockConfig::default()
    };
    rs_cam_core::compute::fixture_values::fill_unset_stock(&mut stock);
    let mut builder = ProjectSessionBuilder::new().stock(stock);
    let mill = builder.add_tool(end_mill(false));
    let tool_id = |builder: &ProjectSessionBuilder, index: usize| {
        builder
            .tools()
            .get(index)
            .map(|tool| ToolId::from_raw(tool.id.raw()))
            .expect("the builder holds the tool it added")
    };
    let mill = tool_id(&builder, mill);
    let bulk = builder.add_tool(end_mill(true));
    let bulk = tool_id(&builder, bulk);
    let big = builder.add_model(model("big square", None, vec![square(0.0, 0.0, 10.0)]));
    // 3.3 mm half width: the 6 mm tool has 0.3 mm to each side, so no ramp
    // fits and the entry plunges. Away from the big square, so the plunge
    // goes into wood that no earlier pocket cleared.
    let small = builder.add_model(model("small square", None, vec![square(20.0, 20.0, 3.3)]));
    let plate = builder.add_model(model(
        "plate",
        Some(rs_cam_core::mesh::make_test_flat(2.0 * HALF)),
        Vec::new(),
    ));

    let mut inverted = config(NAMES[INVERTED], pocket(), mill, big);
    inverted.heights.retract_z = HeightMode::Manual(10.0);
    inverted.heights.clearance_z = HeightMode::Manual(5.0);
    let gouge = config(
        NAMES[GOUGE],
        OperationConfig::Face(FaceConfig {
            depth: 1.0,
            depth_per_pass: Some(1.0),
            feed_rate: Some(1000.0),
            ..FaceConfig::default()
        }),
        mill,
        plate,
    );
    let mut plunge = config(NAMES[PLUNGE], pocket(), bulk, small);
    plunge.dressups.entry_style = DressupEntryStyle::None;
    let left_out = config(NAMES[LEFT_OUT], pocket(), mill, big);
    for cfg in [inverted, gouge, plunge, left_out] {
        let _ = builder
            .add_toolpath(0, cfg)
            .expect("a fresh session takes the op");
    }
    let mut session = builder.build();
    // Each pocket cuts every item of its drawing.
    for index in [INVERTED, PLUNGE, LEFT_OUT] {
        let _ = session
            .apply(rs_cam_core::session::Command::EditSelection(
                rs_cam_core::session::EditSelectionArgs {
                    index,
                    target: rs_cam_core::session::SelectionTarget::Op,
                    edit: rs_cam_core::selection::SelectionEdit::Replace(Box::new(
                        rs_cam_core::selection::Selection::drawing(),
                    )),
                },
            ))
            .expect("the op takes the selection");
    }
    session
}

/// [`project`] with ops 0 to 2 generated and the project simulated. Op 3
/// has no result.
pub fn generated_and_simulated() -> ProjectSession {
    let mut session = project();
    let cancel = AtomicBool::new(false);
    for index in [INVERTED, GOUGE, PLUNGE] {
        session
            .generate_toolpath(index, &cancel)
            .expect("each op with its inputs generates");
    }
    let opts = SimulationOptions {
        resolution: SimulationResolution::Fixed(0.5),
        skip_ids: session
            .toolpath_configs()
            .get(LEFT_OUT)
            .map(|tc| vec![tc.id])
            .unwrap_or_default(),
        use_predicted_feed_in_gates: false,
        adaptive_feed_modulation: false,
        modulation_strategy:
            rs_cam_core::dressup::feed_modulation::ModulationStrategy::ConstrainedMax,
        modulation_feed_scale: 1.0,
    };
    session
        .run_simulation(&opts, &cancel)
        .expect("the simulation completes");
    session
}
