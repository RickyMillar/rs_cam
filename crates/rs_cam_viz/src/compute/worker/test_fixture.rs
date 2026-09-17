//! One builder for a test [`ComputeRequest`].
//!
//! WP11b: a `ComputeRequest` is the handle `ProjectSession::start` produced,
//! so a test cannot write one field by field any more — which is the point
//! (tracker row N12: one input assembly). Every worker test that used to
//! build a request literal and then patch a field describes the same fixture
//! here instead, as a [`RequestSpec`], and this module turns it into a
//! session, a toolpath config and one `start` call.
//!
//! The stock hangs BELOW `z = 0`, because a 2D operation cuts at negative Z
//! and the resolver takes `heights.top_z` from the stock top. The request
//! literals this replaces passed `HeightContext::simple`, whose
//! `stock_top_z` is `0.0`, so a board topped at `z = 0` is what keeps the
//! converted assertions measuring the same geometry.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits, StockConfig};
use rs_cam_core::geo::BoundingBox3;
use rs_cam_core::mesh::TriangleMesh;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{
    GenerateToolpathArgs, Job, JobHandle, LoadedModel, ProjectSessionBuilder,
    ToolpathComputeResult, ToolpathConfig,
};
use rs_cam_core::trace::toolpath_spans::AnnotatedToolpath;

use super::{ComputeRequest, VizExtras};
use crate::state::job::ToolConfig;
use crate::state::toolpath::{
    BoundaryConfig, BoundarySource, DressupConfig, OperationConfig, RestAnalysisConfig,
    StockSource, ToolpathId,
};

/// The stable id of the source toolpath a `DerivedRestRegions` fixture adds.
const REST_SOURCE_ID: ToolpathId = ToolpathId(9000);

/// A board of `2·half` mm square and `height` mm thick whose TOP face is at
/// `z = 0`.
pub(super) fn board(half: f64, height: f64) -> StockConfig {
    StockConfig {
        x: 2.0 * half + 4.0,
        y: 2.0 * half + 4.0,
        z: height,
        origin_x: -half - 2.0,
        origin_y: -half - 2.0,
        origin_z: -height,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

/// A stock configuration with exactly these bounds.
///
/// The 3D fixtures need the board TOP above the mesh, or a roughing pass has
/// nothing to remove and the empty-generation gate refuses. They state their
/// original bounds here rather than taking [`board`].
pub(super) fn stock_between(min: rs_cam_core::geo::P3, max: rs_cam_core::geo::P3) -> StockConfig {
    StockConfig {
        x: max.x - min.x,
        y: max.y - min.y,
        z: max.z - min.z,
        origin_x: min.x,
        origin_y: min.y,
        origin_z: min.z,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

/// The bounding box of a stock configuration.
pub(super) fn stock_bbox(stock: &StockConfig) -> BoundingBox3 {
    BoundingBox3 {
        min: rs_cam_core::geo::P3::new(stock.origin_x, stock.origin_y, stock.origin_z),
        max: rs_cam_core::geo::P3::new(
            stock.origin_x + stock.x,
            stock.origin_y + stock.y,
            stock.origin_z + stock.z,
        ),
    }
}

/// The retract plane the submit step resolved for this job.
///
/// Read off the handle's own snapshot, because the heights are private to
/// the bundle. A test that wants to know where a rapid goes asks the job,
/// not a second resolution of its own.
pub(super) fn retract_z(req: &ComputeRequest) -> f64 {
    let snapshot = req.handle.request_snapshot();
    snapshot["heights"]["retract_z"]
        .as_f64()
        .unwrap_or_else(|| panic!("the handle snapshot must carry a retract_z: {snapshot}"))
}

/// Every dressup off.
///
/// The nearest thing to the raw generator output the tests that used to call
/// `generate_via_core` were reading: that door applied no dressup at all,
/// and `DressupConfig::default()` turns three of them on.
pub(super) fn no_dressups() -> DressupConfig {
    DressupConfig {
        link_moves: None,
        feed_optimization: false,
        optimize_rapid_order: false,
        ..DressupConfig::default()
    }
}

/// What one test fixture varies.
pub(super) struct RequestSpec {
    /// The toolpath's stable id. The lane keys its queue by it, so tests
    /// that submit two jobs for one toolpath pass the same value.
    pub id: usize,
    pub name: String,
    pub operation: OperationConfig,
    pub tool: ToolConfig,
    /// Tools the operation REFERS to — a Rest `prev_tool_id`, a Pencil or
    /// rest-analysis `reference_tool_id`. The session must hold them for
    /// the resolver to find them.
    pub extra_tools: Vec<ToolConfig>,
    pub polygons: Option<Vec<Polygon2>>,
    /// The model's drill targets — the `Drill` family's hole source.
    pub drill_targets: Vec<rs_cam_core::io::dxf_input::DrillTarget>,
    pub mesh: Option<TriangleMesh>,
    pub stock: StockConfig,
    pub dressups: DressupConfig,
    pub stock_source: StockSource,
    pub boundary: BoundaryConfig,
    pub rest_analysis: RestAnalysisConfig,
    pub debug_options: rs_cam_core::trace::debug_trace::ToolpathDebugOptions,
    /// Rest regions a SOURCE toolpath publishes.
    ///
    /// `Some` adds a second toolpath to the session, seeds its cached
    /// result with an annotated toolpath carrying these regions, and points
    /// `boundary.source` at it. A `DerivedRestRegions` boundary resolves
    /// from a cached result, so there is no other way to state one.
    pub rest_source_regions: Option<Vec<Polygon2>>,
}

impl RequestSpec {
    /// A 40 x 40 mm square polygon over a 15 mm board, one end mill, no
    /// dressups, no boundary.
    pub(super) fn new(id: usize, name: &str, operation: OperationConfig, tool: ToolConfig) -> Self {
        Self {
            id,
            name: name.to_owned(),
            operation,
            tool,
            extra_tools: Vec::new(),
            polygons: Some(vec![Polygon2::rectangle(-20.0, -20.0, 20.0, 20.0)]),
            drill_targets: Vec::new(),
            mesh: None,
            stock: board(25.0, 15.0),
            dressups: no_dressups(),
            stock_source: StockSource::Fresh,
            boundary: BoundaryConfig::default(),
            rest_analysis: RestAnalysisConfig::default(),
            debug_options: rs_cam_core::trace::debug_trace::ToolpathDebugOptions::default(),
            rest_source_regions: None,
        }
    }

    /// Record a debug and a semantic trace.
    pub(super) fn with_debug_trace(mut self) -> Self {
        self.debug_options.enabled = true;
        self
    }

    /// Cut a mesh instead of polygons.
    pub(super) fn with_mesh(mut self, mesh: TriangleMesh) -> Self {
        self.mesh = Some(mesh);
        self.polygons = None;
        self
    }
}

/// An annotated toolpath that carries `regions` and nothing else.
///
/// The rest-regions boundary reads its source's cached result, so a fixture
/// for it has to publish one.
fn rest_source_result(regions: Vec<Polygon2>) -> ToolpathComputeResult {
    let mut annotated = AnnotatedToolpath::new(rs_cam_core::toolpath::Toolpath::new());
    annotated.rest_regions = Some(Arc::new(regions));
    ToolpathComputeResult {
        op_data: rs_cam_core::ops::drill_op::OpData::Toolpath(Arc::new(annotated)),
        stats: rs_cam_core::compute::toolpath_stats::ToolpathStats::default(),
        debug_trace: None,
        semantic_trace: None,
    }
}

/// One toolpath config.
fn toolpath_config(
    id: ToolpathId,
    name: &str,
    operation: OperationConfig,
    tool_id: usize,
    model_id: usize,
    spec: &RequestSpec,
    boundary: BoundaryConfig,
) -> ToolpathConfig {
    ToolpathConfig {
        id,
        name: name.to_owned(),
        enabled: true,
        operation,
        dressups: spec.dressups.clone(),
        heights: Default::default(),
        tool_id,
        model_id,
        pre_gcode: None,
        post_gcode: None,
        boundary,
        boundary_inherit: false,
        stock_source: spec.stock_source,
        coolant: Default::default(),
        face_selection: None,
        debug_options: spec.debug_options,
        feeds_provenance: Default::default(),
        rest_analysis: spec.rest_analysis.clone(),
        planner_origin: None,
    }
}

/// Build one `ComputeRequest` through the production submit door.
///
/// Panics with the refusal when `start` refuses: a fixture that cannot
/// submit is a broken fixture, and the message names what is missing.
// The spec is read, not consumed: the builder clones each field and
// `toolpath_config` borrows the spec afterwards. A reference here would move
// the lint onto `build_request` and every one of its callers.
#[allow(clippy::needless_pass_by_value)]
pub(super) fn request(spec: RequestSpec) -> ComputeRequest {
    let mut builder = ProjectSessionBuilder::new()
        .stock(spec.stock.clone())
        .tool(spec.tool.clone());
    for tool in &spec.extra_tools {
        builder = builder.tool(tool.clone());
    }
    let kind = if spec.mesh.is_some() {
        ModelKind::Stl
    } else {
        ModelKind::Svg
    };
    builder = builder.model(LoadedModel {
        id: 0,
        path: std::path::PathBuf::from("fixture"),
        name: "Fixture".to_owned(),
        kind: Some(kind),
        mesh: spec.mesh.clone().map(Arc::new),
        polygons: spec.polygons.clone().map(Arc::new),
        drill_targets: Arc::new(spec.drill_targets.clone()),
        layers: Arc::new(Vec::new()),
        enriched_mesh: None,
        units: Some(ModelUnits::Millimeters),
        winding_report: None,
        load_error: None,
    });

    // The source toolpath first, so the operation under test is the one
    // whose boundary names it.
    let mut boundary = spec.boundary.clone();
    let mut index = 0;
    if let Some(regions) = spec.rest_source_regions.clone() {
        builder = builder
            .toolpath(toolpath_config(
                REST_SOURCE_ID,
                "Rest source",
                OperationConfig::new_default(crate::state::toolpath::OperationType::Pocket),
                spec.tool.id.0,
                0,
                &spec,
                BoundaryConfig::default(),
            ))
            .result(0, rest_source_result(regions));
        boundary.source = BoundarySource::DerivedRestRegions {
            source_toolpath_id: REST_SOURCE_ID,
        };
        index = 1;
    }

    let id = ToolpathId(spec.id);
    builder = builder.toolpath(toolpath_config(
        id,
        &spec.name,
        spec.operation.clone(),
        spec.tool.id.0,
        0,
        &spec,
        boundary,
    ));

    let mut session = builder.build();
    let cancel = Arc::new(AtomicBool::new(false));
    let JobHandle::GenerateToolpath(handle) = session
        .start(
            Job::GenerateToolpath(GenerateToolpathArgs { index }),
            &cancel,
        )
        .unwrap_or_else(|error| panic!("the fixture must submit: {error}"))
    else {
        panic!("the generate_toolpath row answers its own handle variant");
    };

    ComputeRequest {
        handle: *handle,
        viz: VizExtras {
            toolpath_id: id,
            cancel,
        },
    }
}
