//! `ProjectSession` wiring — models, stock, the 18-field `ToolpathConfig`,
//! and the one-operation session the end-to-end sentries all build.
//!
//! `standing_material_channel_am9.rs` became the de-facto template for this
//! by accident: later waves copied its `mesh_model` / `toolpath` / `stock_over`
//! / `generate` quartet because it was the most recent working example. This
//! module is that template, made deliberate.
//!
//! # The `ToolpathConfig` problem this exists to solve
//!
//! [`rs_cam_core::session::ToolpathConfig`] has 18 fields and no `Default`,
//! so ~30 test files spell out a full struct literal. Every field added to it
//! breaks all of them at once — exactly the audit `CLAUDE.md` asks for under
//! "if GUI state adds a field, audit test initializers". [`toolpath_config`]
//! is the single place that literal now lives; callers override with Rust's
//! own struct-update syntax, which is inherently field-addition-proof:
//!
//! ```ignore
//! ToolpathConfig {
//!     heights: pinned_heights(0.0, -9.0),
//!     ..toolpath_config("Unified Finish", op, tool_id, model_id)
//! }
//! ```
//!
//! or, when the config is being handed straight to a session builder, via the
//! closure argument of [`single_op_session_with`].
//!
//! Reference consumers: `standing_material_channel_am9.rs` (all of it);
//! `unified_finish_tapered_end_to_end_m21.rs` and
//! `pencil_tip_float_channel_d1.rs` carry the same shapes locally.

#![allow(dead_code)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{
    BoundaryConfig, DressupConfig, HeightMode, HeightsConfig, StockSource,
};
use rs_cam_core::compute::tool_config::ToolConfig;
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::geo::P2;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::mesh::TriangleMesh;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{LoadedModel, ProjectSession, ProjectSessionBuilder, ToolpathConfig};
use rs_cam_core::trace::debug_trace::ToolpathDebugOptions;

// ── Models ──────────────────────────────────────────────────────────────

/// Wrap a mesh as a `LoadedModel` at id 0 with a `synthetic://` path — no
/// file ever touches disk, and the path makes that obvious in any dump.
pub fn mesh_model(mesh: TriangleMesh, name: &str) -> LoadedModel {
    LoadedModel {
        id: 0,
        name: name.to_owned(),
        mesh: Some(Arc::new(mesh)),
        items: Default::default(),
        path: PathBuf::from(format!("synthetic://{name}.stl")),
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

/// Wrap 2D polygons as a `LoadedModel` — the input shape the 2.5D families
/// (pocket, profile, adaptive) take.
pub fn polygon_model(polygons: Vec<Polygon2>, name: &str) -> LoadedModel {
    LoadedModel {
        id: 0,
        name: name.to_owned(),
        mesh: None,
        items: Arc::new(rs_cam_core::selection::ItemSet::from_polygons(polygons)),
        path: PathBuf::from(format!("synthetic://{name}.svg")),
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

/// [`polygon_model`] with one `Point` item per hole after the polygons, on
/// layer `holes`: the hole source of a `Drill` op (and of the extras of a
/// Pin Drill with a drawing selection) since geometry selection P2.3c.
pub fn polygon_model_with_points(
    polygons: Vec<Polygon2>,
    holes: &[[f64; 2]],
    name: &str,
) -> LoadedModel {
    let holes: Vec<TestHole> = holes
        .iter()
        .map(|&[x, y]| TestHole {
            x,
            y,
            layer: "holes".to_owned(),
            diameter: None,
        })
        .collect();
    LoadedModel {
        items: Arc::new(drawing_items(polygons, &holes)),
        ..polygon_model(Vec::new(), name)
    }
}

/// The drill target list that the import gave before geometry selection
/// P2.3c, as `(x, y, layer, diameter)` (`None` for a point). It is the
/// deleted bridge `LoadedModel::drill_targets_pre_selection`, rebuilt on
/// the same inputs and the same production functions, so the P2.1 and
/// P2.2 sentries keep their frozen evidence (the hole items of a drill op
/// against the targets of the old import):
///
/// - SVG: `svg_input::circle_like_ring` over every ring of the nested
///   outlines (exteriors, then the holes of each closed polygon), on the
///   layer `circles`;
/// - any other kind: each `Point` and `Circle` item in import order, with
///   its layer.
pub fn old_drill_targets(m: &LoadedModel) -> Vec<(f64, f64, String, Option<f64>)> {
    use rs_cam_core::compute::stock_config::ModelKind;
    use rs_cam_core::selection::Shape;
    if m.kind == Some(ModelKind::Svg) {
        return m
            .outlines_pre_selection()
            .map(|outlines| {
                outlines
                    .iter()
                    .filter(|poly| poly.closed)
                    .flat_map(|poly| {
                        std::iter::once(poly.exterior.as_slice())
                            .chain(poly.holes.iter().map(Vec::as_slice))
                    })
                    .filter_map(rs_cam_core::io::svg_input::circle_like_ring)
                    .map(|(c, d)| (c.x, c.y, "circles".to_owned(), Some(d)))
                    .collect()
            })
            .unwrap_or_default();
    }
    m.items
        .iter()
        .filter_map(|item| {
            let layer = item.layer().unwrap_or_default().to_owned();
            match item.shape() {
                Shape::Point(p) => Some((p.x, p.y, layer, None)),
                Shape::Circle {
                    centre, diameter, ..
                } => Some((centre.x, centre.y, layer, Some(*diameter))),
                Shape::Loop(_) | Shape::Path(_) | Shape::Face(_) => None,
            }
        })
        .collect()
}

/// One hole of a test drawing: a point (`diameter: None`) or a circle.
#[derive(Debug, Clone)]
pub struct TestHole {
    pub x: f64,
    pub y: f64,
    pub layer: String,
    pub diameter: Option<f64>,
}

/// The items of a drawing: the polygons first (`ItemSet::from_polygons`),
/// then one item per hole in order. A hole with no diameter is a `Point`
/// item; a hole with a diameter is a `Circle` item with the ring the DXF
/// door makes for a CIRCLE, as a CIRCLE entity gives one item since P2.1.
pub fn drawing_items(
    polygons: Vec<Polygon2>,
    holes: &[TestHole],
) -> rs_cam_core::selection::ItemSet {
    use rs_cam_core::selection::{Item, ItemSet, Shape, SourceRef};
    let mut items: Vec<Item> = ItemSet::from_polygons(polygons).iter().cloned().collect();
    items.extend(holes.iter().map(|h| {
        let shape = match h.diameter {
            None => Shape::Point(P2::new(h.x, h.y)),
            Some(diameter) => Shape::circle(P2::new(h.x, h.y), diameter),
        };
        Item::new(
            SourceRef::ToolpathModel,
            Some(h.layer.as_str().into()),
            shape,
        )
        .expect("a hole gives a valid item")
    }));
    ItemSet::new(items)
}

/// An axis-aligned square of half-extent `half`, wound CCW from `(-h, -h)`.
pub fn square_polygon(half: f64) -> Polygon2 {
    Polygon2::new(vec![
        P2::new(-half, -half),
        P2::new(half, -half),
        P2::new(half, half),
        P2::new(-half, half),
    ])
}

// ── Stock ───────────────────────────────────────────────────────────────

/// Stock that covers a `2·half` square model with 2 mm of margin per side and
/// its origin on the world floor (`origin_z = 0`), i.e. the stock a surface
/// fixture built around `z ∈ [0, height]` sits in.
pub fn stock_over(half: f64, height: f64) -> StockConfig {
    StockConfig {
        x: 2.0 * half + 4.0,
        y: 2.0 * half + 4.0,
        z: height,
        origin_x: -half - 2.0,
        origin_y: -half - 2.0,
        origin_z: 0.0,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

/// Stock hanging BELOW `z = 0` — the frame a model whose top face is `z = 0`
/// and whose features cut downwards needs (see the `project_2d_stock_z_frame`
/// note: 2D ops cut at negative Z, so `origin_z` must be negative).
pub fn stock_under(half: f64, height: f64) -> StockConfig {
    StockConfig {
        origin_z: -height,
        ..stock_over(half, height)
    }
}

// ── Toolpaths ───────────────────────────────────────────────────────────

/// The 18-field `ToolpathConfig` literal, once.
///
/// Everything not named here is the type's own default; `dressups` follows
/// the operation's registry role (`DressupConfig::for_op`) rather than being
/// hand-rolled, because several ops strip or require specific dressups and a
/// blanket default silently changes what is being tested.
pub fn toolpath_config(
    name: &str,
    op: OperationConfig,
    tool_id: usize,
    model_id: usize,
) -> ToolpathConfig {
    let op_type = op.op_type();
    ToolpathConfig {
        shoe: Default::default(),
        id: ToolpathId(0),
        name: name.to_owned(),
        enabled: true,
        operation: op,
        dressups: DressupConfig::for_op(op_type),
        heights: HeightsConfig::default(),
        tool_id,
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

/// Explicit top/bottom Z instead of `Auto`.
///
/// Surface ops carry no depth dial, so `bottom_z: Auto` resolves to
/// `top_z - op_depth` and collapses a waterline band to zero height. Any
/// surface fixture that wants a Z range must pin it.
pub fn pinned_heights(top_z: f64, bottom_z: f64) -> HeightsConfig {
    HeightsConfig {
        top_z: HeightMode::Manual(top_z),
        bottom_z: HeightMode::Manual(bottom_z),
        ..HeightsConfig::default()
    }
}

// ── Sessions ────────────────────────────────────────────────────────────

/// One stock, one tool, one model, one toolpath — the shape nearly every
/// end-to-end sentry wants, wired through [`ProjectSessionBuilder`]'s
/// allocating methods, not by poking fields.
///
/// Those methods allocate each id exactly as the session's own setters do,
/// and `add_model` fits the stock when `StockConfig::auto_from_model` is
/// set. The stock is written FIRST, so the fit reads the stock this fixture
/// was given.
///
/// The toolpath is added but NOT generated; call [`generate`] when the test is
/// ready to pay for it.
pub fn single_op_session(
    stock: StockConfig,
    tool: ToolConfig,
    model: LoadedModel,
    name: &str,
    op: OperationConfig,
) -> ProjectSession {
    single_op_session_with(stock, tool, model, name, op, |_| {})
}

/// [`single_op_session`] with a hook to adjust the `ToolpathConfig` before it
/// is added — heights, dressups, boundary, rest analysis.
pub fn single_op_session_with(
    stock: StockConfig,
    tool: ToolConfig,
    model: LoadedModel,
    name: &str,
    op: OperationConfig,
    tweak: impl FnOnce(&mut ToolpathConfig),
) -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new().stock(stock);
    let tool_idx = builder.add_tool(tool);
    let tool_id = builder.tools()[tool_idx].id.0;
    let model_id = builder.add_model(model);
    let mut cfg = toolpath_config(name, op, tool_id, model_id);
    tweak(&mut cfg);
    let _ = builder
        .add_toolpath(0, cfg)
        .expect("add toolpath to a fresh session");
    builder.build()
}

/// Generate one toolpath through the production entry point the GUI worker
/// and the CLI share. Panics with the generator's own error on failure.
#[track_caller]
pub fn generate(session: &mut ProjectSession, index: usize) {
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(index, &cancel)
        .expect("generation must succeed");
}

/// Pick the BREP faces `faces` of the toolpath model into the op selection
/// of toolpath `index` (geometry selection P2.6: a face pick is a
/// `MatchKey` pick, sent through `edit_toolpath_selection` as the
/// viewport sends it). `Err` is the refusal of a face that has no Face
/// item (not flat), from `ProjectSession::face_pick_key`.
pub fn pick_faces(
    session: &mut ProjectSession,
    index: usize,
    faces: &[u16],
) -> Result<(), rs_cam_core::selection::SelectionRefusal> {
    pick_faces_in(
        session,
        index,
        rs_cam_core::session::SelectionTarget::Op,
        faces,
    )
}

/// [`pick_faces`] into the selection `target` of toolpath `index` (P4.1:
/// the boundary selection of an op with no selection slot).
pub fn pick_faces_in(
    session: &mut ProjectSession,
    index: usize,
    target: rs_cam_core::session::SelectionTarget,
    faces: &[u16],
) -> Result<(), rs_cam_core::selection::SelectionRefusal> {
    for face in faces {
        let tc = session.toolpath_configs()[index].clone();
        let key = session.face_pick_key(
            &tc,
            tc.model_id,
            rs_cam_core::geometry::enriched_mesh::FaceGroupId(*face),
        )?;
        let _ = session
            .apply(rs_cam_core::session::Command::EditSelection(
                rs_cam_core::session::EditSelectionArgs {
                    index,
                    target,
                    edit: rs_cam_core::selection::SelectionEdit::AddPick(key),
                },
            ))
            .expect("the target holds a selection");
    }
    Ok(())
}

// ── Generation ──────────────────────────────────────────────────────────

/// Generate every enabled toolpath of `session` in plan order, and return
/// the failures.
///
/// The walk is `session::generation_plan::plan` over the whole project, the
/// same plan the GUI, the MCP server and the CLI drive. A `Step::Simulate`
/// runs the session simulation at the project cell
/// (`ProjectSession::simulation_resolution_mm`), so a rest toolpath reads a
/// prior stock at the cell its snapshot check asks for. A failed step does not stop the
/// walk: the caller decides whether a failure is red.
pub fn generate_all_in_plan_order(
    session: &mut ProjectSession,
    cancel: &AtomicBool,
) -> Vec<(ToolpathId, rs_cam_core::session::SessionError)> {
    use rs_cam_core::session::SimulationOptions;
    use rs_cam_core::session::generation_plan::{Scope, Step, plan};

    let mut failures = Vec::new();
    for step in plan(session, Scope::Project) {
        match step {
            Step::Simulate { upto, .. } => {
                let options = SimulationOptions {
                    resolution: session.simulation_resolution_mm(),
                    ..SimulationOptions::default()
                };
                if let Err(error) = session.run_simulation(&options, cancel) {
                    failures.push((upto, error));
                }
            }
            Step::Generate { toolpath, index } => {
                if let Err(error) = session.generate_toolpath(index, cancel) {
                    failures.push((toolpath, error));
                }
            }
        }
    }
    failures
}

/// Run the multi-tool planner through its command row (F040) and return
/// the outcome that `Effects::planned` carries.
///
/// The planner is the `PlanMultitoolFinishing` row; no public session
/// method writes the ladder outside `ProjectSession::apply`.
pub fn plan_multitool(
    session: &mut ProjectSession,
    spec: &rs_cam_core::session::MultitoolPlanSpec,
) -> Result<rs_cam_core::session::MultitoolPlanOutcome, rs_cam_core::session::SessionError> {
    let effects = session.apply(rs_cam_core::session::Command::PlanMultitoolFinishing(
        rs_cam_core::session::PlanMultitoolFinishingArgs {
            spec: Box::new(spec.clone()),
        },
    ))?;
    Ok(*effects
        .planned
        .expect("the PlanMultitoolFinishing row reports its outcome"))
}
