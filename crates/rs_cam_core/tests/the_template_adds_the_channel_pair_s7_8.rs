//! Step 7.8 (`planning/adaptive_research_2026-10-23/integration/
//! step78_design.md`; operator ruling 2026-10-28, midday): the template
//! "Geodesic body + edges" adds the D47 channel pair in ONE command.
//!
//! 1. The draft and the command give op 1 "Geodesic body" (Geodesic
//!    Adaptive, rest analysis on) and, directly after it, op 2 "Geodesic
//!    edges" (Contour Parallel, rest machining from previous ops, the
//!    rest regions of op 1 with Center containment and offset 0, op 1's
//!    tool). The pair generates: the edge op is not refused (D53, D44).
//! 2. `edges: false` adds op 1 alone, with its rest analysis off.
//! 3. An edge tool larger than op 1's refuses with the D53 sentence, and
//!    the command adds nothing.
//! 4. With no fitting tool, both ops are added with no tool (R2).
//!
//! ```text
//! cargo test -p rs_cam_core -q --test the_template_adds_the_channel_pair_s7_8
//! ```

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use common::meshes::height_field;
use common::session::{mesh_model, stock_over};
use common::tools::endmill_tool_config;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{BoundaryContainment, BoundarySource, StockSource};
use rs_cam_core::compute::operation_configs::ClearingStrategy;
use rs_cam_core::ids::ToolId;
use rs_cam_core::session::{
    AddGeodesicBodyAndEdgesArgs, Command, GEODESIC_BODY_NAME, GEODESIC_EDGES_NAME,
    GeodesicPairRequest, ProjectSession, ProjectSessionBuilder, SessionError, SimulationOptions,
    SimulationResolution, ToolChoice,
};
use std::sync::atomic::AtomicBool;

// The narrow channel of the D53 sentry: one stepped pocket.
const TOP_Z: f64 = 10.0;
const FLOOR_Z: f64 = 4.0;
const MESH_HALF: f64 = 40.0;
const MESH_STEP: f64 = 0.5;
const CHANNEL_HALF_LENGTH: f64 = 30.0;
const CHANNEL_HALF_WIDTH: f64 = 4.8;
const WIDE_HALF_X: f64 = 34.0;
const WIDE_HALF_Y: f64 = 24.0;
const WIDE_FLOOR_Z: f64 = 7.0;
const SOURCE_DIAMETER: f64 = 6.0;
const LARGER_DIAMETER: f64 = 8.0;
/// The simulation cell of the D50 sentry, mm: the mesh step of the
/// channel, so a cell holds one height sample.
const SIM_CELL: f64 = MESH_STEP;

/// The channel model and stock, with the tools `diameters` in list order.
fn session(diameters: &[f64]) -> ProjectSession {
    let mesh = height_field(MESH_HALF, MESH_STEP, |x, y| {
        if x.abs() <= CHANNEL_HALF_LENGTH && y.abs() <= CHANNEL_HALF_WIDTH {
            FLOOR_Z
        } else if x.abs() <= WIDE_HALF_X && y.abs() <= WIDE_HALF_Y {
            WIDE_FLOOR_Z
        } else {
            TOP_Z
        }
    });
    let mut stock = stock_over(MESH_HALF, TOP_Z);
    rs_cam_core::compute::fixture_values::fill_unset_stock(&mut stock);
    let mut builder = ProjectSessionBuilder::new().stock(stock);
    for &d in diameters {
        let _ = builder.add_tool(endmill_tool_config(d));
    }
    let _ = builder.add_model(mesh_model(mesh, "channel"));
    builder.build()
}

fn add(
    s: &mut ProjectSession,
    request: GeodesicPairRequest,
) -> Result<rs_cam_core::session::Effects, SessionError> {
    let draft = s.draft_geodesic_pair(request).expect("the pair drafts");
    s.apply(Command::AddGeodesicBodyAndEdges(
        AddGeodesicBodyAndEdgesArgs {
            setup_index: 0,
            body: Box::new(draft.body.config),
            edges: draft.edges.map(|e| Box::new(e.config)),
        },
    ))
}

fn strategy(op: &OperationConfig) -> ClearingStrategy {
    match op {
        OperationConfig::Adaptive3d(cfg) => cfg.clearing_strategy,
        other => panic!("a 3D Rough, not {}", other.label()),
    }
}

#[test]
fn the_template_adds_the_channel_pair_s7_8() {
    // 1. The pair, its values, and a generation.
    let mut s = session(&[SOURCE_DIAMETER]);
    let effects = add(&mut s, GeodesicPairRequest::new(0, None)).expect("the pair adds");
    assert_eq!(effects.created, Some(0), "op 1 is the created index");
    let configs = s.toolpath_configs();
    assert_eq!(configs.len(), 2);
    let (body, edge) = (&configs[0], &configs[1]);
    assert_eq!(s.list_setups()[0].toolpath_indices(), &[0, 1]);
    assert_eq!(body.name, GEODESIC_BODY_NAME);
    assert_eq!(
        strategy(&body.operation),
        ClearingStrategy::GeodesicAdaptive
    );
    assert!(
        body.rest_analysis.enabled,
        "D44: the edge op demands the regions"
    );
    assert_eq!(edge.name, GEODESIC_EDGES_NAME);
    assert_eq!(strategy(&edge.operation), ClearingStrategy::ContourParallel);
    assert_eq!(edge.stock_source, StockSource::FromRemainingStock);
    assert_eq!(edge.tool_id, body.tool_id, "D-T3: op 1's tool");
    assert!(edge.boundary.enabled);
    assert_eq!(
        edge.boundary.source,
        BoundarySource::DerivedRestRegions {
            source_toolpath_id: body.id
        }
    );
    assert_eq!(edge.boundary.containment, BoundaryContainment::Center);
    assert!(edge.boundary.offset.abs() < f64::EPSILON, "offset 0");
    assert_eq!(s.larger_rest_consumer(1, body.id), None, "D53 allows it");
    let cancel = AtomicBool::new(false);
    s.generate_toolpath(0, &cancel).expect("the body generates");
    // The edge op reads the remaining stock: the plan simulates op 1 first.
    let _ = s
        .run_simulation(
            &SimulationOptions {
                resolution: SimulationResolution::Fixed(SIM_CELL),
                ..SimulationOptions::default()
            },
            &cancel,
        )
        .expect("simulate the body");
    s.generate_toolpath(1, &cancel)
        .expect("the edge op generates");
    assert!(s.result_at(1).is_some(), "the edge op holds a result");

    // 2. The body alone.
    let mut s = session(&[SOURCE_DIAMETER]);
    let mut request = GeodesicPairRequest::new(0, None);
    request.edges = false;
    let _ = add(&mut s, request).expect("the body adds");
    assert_eq!(s.toolpath_configs().len(), 1);
    assert!(
        !s.toolpath_configs()[0].rest_analysis.enabled,
        "no op demands the regions"
    );

    // 3. A larger edge tool refuses and adds nothing.
    let mut s = session(&[SOURCE_DIAMETER, LARGER_DIAMETER]);
    let larger = ToolId::from_raw(s.tools()[1].id.raw());
    let mut request = GeodesicPairRequest::new(0, None);
    request.tool = ToolChoice::Exact(ToolId::from_raw(s.tools()[0].id.raw()));
    request.edge_tool = Some(larger);
    let Err(SessionError::OperationFailed(text)) = add(&mut s, request) else {
        panic!("a larger edge tool must refuse");
    };
    assert!(
        text.contains("with Center containment")
            && text.contains(&format!("{:.3} mm", LARGER_DIAMETER / 2.0))
            && text.contains("no larger than"),
        "the D53 sentence: {text}"
    );
    assert!(s.toolpath_configs().is_empty(), "the command added nothing");

    // 4. No tool fits: both ops have no tool.
    let mut s = session(&[]);
    let _ = add(&mut s, GeodesicPairRequest::new(0, None)).expect("the pair adds");
    assert_eq!(s.toolpath_configs().len(), 2);
    assert!(s.toolpath_configs().iter().all(|tc| tc.tool_id.is_none()));
}
