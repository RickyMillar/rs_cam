//! `ProjectSession` wiring — models, stock, the 17-field `ToolpathConfig`,
//! and the one-operation session the end-to-end sentries all build.
//!
//! `standing_material_channel_am9.rs` became the de-facto template for this
//! by accident: later waves copied its `mesh_model` / `toolpath` / `stock_over`
//! / `generate` quartet because it was the most recent working example. This
//! module is that template, made deliberate.
//!
//! # The `ToolpathConfig` problem this exists to solve
//!
//! [`rs_cam_core::session::ToolpathConfig`] has 17 fields and no `Default`,
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
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::geo::P2;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::mesh::TriangleMesh;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{LoadedModel, ProjectSession, ToolpathConfig};

// ── Models ──────────────────────────────────────────────────────────────

/// Wrap a mesh as a `LoadedModel` at id 0 with a `synthetic://` path — no
/// file ever touches disk, and the path makes that obvious in any dump.
pub fn mesh_model(mesh: TriangleMesh, name: &str) -> LoadedModel {
    LoadedModel {
        id: 0,
        name: name.to_owned(),
        mesh: Some(Arc::new(mesh)),
        polygons: None,
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        path: PathBuf::from(format!("synthetic://{name}.stl")),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    }
}

/// Wrap 2D polygons as a `LoadedModel` — the input shape the 2.5D families
/// (pocket, profile, adaptive) take.
pub fn polygon_model(polygons: Vec<Polygon2>, name: &str) -> LoadedModel {
    LoadedModel {
        id: 0,
        name: name.to_owned(),
        mesh: None,
        polygons: Some(Arc::new(polygons)),
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        path: PathBuf::from(format!("synthetic://{name}.svg")),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    }
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

/// The 17-field `ToolpathConfig` literal, once.
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
        face_selection: None,
        debug_options: ToolpathDebugOptions::default(),
        feeds_provenance: rs_cam_core::feeds::FeedsProvenance::default(),
        rest_analysis: rs_cam_core::compute::config::RestAnalysisConfig::default(),
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
/// end-to-end sentry wants, wired through the REAL production entry points
/// (`ProjectSession::add_*`), not by poking fields.
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
    let mut session = ProjectSession::new_empty();
    session.set_stock_config(stock);
    let tool_idx = session.add_tool(tool);
    let tool_id = session.tools()[tool_idx].id.0;
    let model_id = session.add_model(model);
    let mut cfg = toolpath_config(name, op, tool_id, model_id);
    tweak(&mut cfg);
    session
        .add_toolpath(0, cfg)
        .expect("add toolpath to a fresh session");
    session
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
