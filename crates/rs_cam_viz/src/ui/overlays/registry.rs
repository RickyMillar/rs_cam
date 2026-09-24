//! The Overlays registry — one declarative list of every viewport overlay.
//!
//! One list feeds three surfaces, so they cannot disagree:
//!
//! - the viewport dock (`crate::ui::viewport_overlay`) and the All viewport
//!   options catalogue (`super::panel`),
//! - the MCP `set_ui_view` `overlays` map ([`apply_overlays`]),
//! - the completeness sentries (`tests/overlays_registry.rs`).
//!
//! ## The rule the list encodes
//!
//! > Every overlay appears in the list, always. An overlay that cannot draw
//! > is shown disabled, with one line that says why, and — where one exists —
//! > a button that makes it drawable.
//!
//! That generalises the two good precedents already in the app: the
//! `Deviation` mode's "No deviation data — [Re-run simulation]"
//! (`ui/sim_diagnostics.rs`) and the row controls' disabled hover that NAMES
//! the control blocking them (`ui/toolpath_row_controls.rs`). It replaces the
//! `Show generator steps` precedent, which hid a feature until it was
//! already in use.
//!
//! ## Two mechanisms, and why each row records which one it uses
//!
//! A **draw-time** flag rides `ViewportCallback`, which `app/viewport.rs`
//! rebuilds every frame; it takes effect at once. An **upload-time** flag is
//! consumed in `app/gpu_upload.rs`, which runs only when
//! `take_pending_upload()` fires — so an upload-time flag with no trigger is
//! a control that does nothing until an unrelated event happens to fire an
//! upload. That was the tool-profile ghost for its whole life (audit §3.6).
//! Every [`OverlayMechanism::UploadTime`] row therefore names its trigger,
//! and a sentry asserts the name is non-empty.

use std::collections::BTreeMap;

use crate::state::overlays::DockSection;
use crate::state::simulation::StockVizMode;
use crate::state::viewport::ToolpathColorMode;
use crate::state::{AppState, Workspace};

// ── the shape of a row ─────────────────────────────────────────────────────

/// The row's family. The old Overlays panel listed a collapsible group per
/// family; the dock places a row by [`dock_section`], which falls back to
/// the family for a row it does not name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayGroup {
    Geometry,
    Toolpath,
    Regions,
    Analysis,
}

/// The surface a row paints a per-vertex or per-move SCALAR FIELD on.
///
/// Exclusivity is applied per surface and only among scalar fields (UX §6.5).
/// A *territory* overlay — the tier map, the planner islands, a boundary
/// outline — paints a region label over an area, and territories stack by
/// design, so those rows carry [`OverlaySurface::None`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlaySurface {
    /// Not a scalar field: stacks with everything.
    None,
    /// The model mesh. Members: Reach (P5) and the rest heatmap — the one
    /// cross-group exclusion in the panel.
    Model,
    /// The simulated (dexel) stock: Solid / Deviation / By height.
    Stock,
    /// The toolpath move lines: Palette / Engagement / Advance-per-tooth.
    Moves,
}

impl OverlaySurface {
    /// The surfaces exclusivity is enforced on.
    pub const EXCLUSIVE: [Self; 3] = [Self::Model, Self::Stock, Self::Moves];

    pub fn label(self) -> &'static str {
        match self {
            Self::None => "",
            Self::Model => "model",
            Self::Stock => "stock",
            Self::Moves => "moves",
        }
    }
}

/// How the row's flag reaches the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayMechanism {
    /// Read into `ViewportCallback` every frame. Immediate.
    DrawTime,
    /// Consumed in `app/gpu_upload.rs`. The `&'static str` names what fires
    /// the upload — a registry row must never leave this blank.
    UploadTime(&'static str),
}

/// The composite per-frame change detector in `app.rs` that watches every
/// upload-time overlay dial at once. One detector, not one per dial: the
/// three-dial version shipped with two detectors and left the third dial
/// stale for its whole life.
const UPLOAD_DETECTOR: &str = "the composite `overlay_upload_key` detector in app.rs";

/// A button the panel offers beside a disabled row, to make it drawable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayAction {
    RunCollisionCheck,
    /// Open `Toolpath ▸ Plan multi-tool finishing…`.
    OpenPlanner,
    GenerateAll,
    /// Switch on generator-trace capture for the next generation.
    RecordGeneratorTrace,
    /// Switch the selected toolpath's rest analysis on and regenerate it.
    ///
    /// W2 (G-STARTFROM): switching the heatmap on IS the demand for a rest
    /// grid, the same demand a `Rest regions` boundary is. There is no
    /// checkbox to navigate to any more, so the row asks for the work
    /// instead of pointing at an authoring home.
    EnableRestAnalysis,
}

impl OverlayAction {
    pub fn label(self) -> &'static str {
        match self {
            Self::RunCollisionCheck => "Run collision check",
            Self::OpenPlanner => "Plan\u{2026}",
            Self::GenerateAll => "Generate all",
            Self::RecordGeneratorTrace => "Record & re-generate",
            Self::EnableRestAnalysis => "Compute rest",
        }
    }

    /// Does the action start compute work? `Plan…` opens a dialog only, so
    /// it has no `Compute & show` twin.
    pub fn is_compute(self) -> bool {
        !matches!(self, Self::OpenPlanner)
    }

    /// The label of the `Compute & show` twin of the action. The plain
    /// action computes only; the twin also switches the row on when the
    /// data lands, if the target and the preferred mode did not change.
    pub fn show_label(self) -> Option<&'static str> {
        match self {
            Self::RunCollisionCheck => Some("Run check & show"),
            Self::OpenPlanner => None,
            Self::GenerateAll => Some("Generate all & show"),
            Self::RecordGeneratorTrace => Some("Record, re-generate & show"),
            Self::EnableRestAnalysis => Some("Compute & show"),
        }
    }
}

/// Whether the row can draw right now, and if not, why not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Precondition {
    Ready,
    Disabled {
        reason: String,
        compute: Option<OverlayAction>,
    },
}

impl Precondition {
    fn no(reason: &str) -> Self {
        Self::Disabled {
            reason: reason.to_owned(),
            compute: None,
        }
    }

    fn no_with(reason: String, compute: OverlayAction) -> Self {
        Self::Disabled {
            reason,
            compute: Some(compute),
        }
    }

    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready)
    }

    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Ready => None,
            Self::Disabled { reason, .. } => Some(reason.as_str()),
        }
    }

    pub fn compute(&self) -> Option<OverlayAction> {
        match self {
            Self::Ready => None,
            Self::Disabled { compute, .. } => *compute,
        }
    }
}

/// One overlay.
pub struct OverlayRow {
    /// Stable id. It is the MCP wire name, so it does not change with a
    /// label.
    pub id: &'static str,
    pub group: OverlayGroup,
    pub label: &'static str,
    pub surface: OverlaySurface,
    pub mechanism: OverlayMechanism,
    /// The state field this row drives, written as a path
    /// (`"viewport.show_grid"`). `None` means the row has no flag because it
    /// has NO RENDERER yet — it is listed, permanently disabled, and says so.
    /// The completeness sentry maps declared state fields onto this.
    pub flag: Option<&'static str>,
    /// `true` for a member of a per-surface radio group whose "off" has no
    /// meaning (Palette, Solid, and their peers). Switching such a row off
    /// is refused rather than silently ignored.
    pub radio: bool,
    pub get: fn(&AppState) -> bool,
    /// Writes the flag. A `false` on a radio member is a no-op here; the
    /// refusal happens in [`apply_overlays`], which can report it.
    pub set: fn(&mut AppState, bool),
    pub precondition: fn(&AppState) -> Precondition,
    /// The per-workspace default (UX §6.4). `None` = this workspace names no
    /// default, so the flag keeps whatever value it has.
    pub default_for: fn(Workspace) -> Option<bool>,
    pub hover: &'static str,
}

// ── per-workspace default helpers (UX §6.4) ────────────────────────────────

/// Setup on, Toolpaths on, Simulation off. The setup-geometry family.
fn geometry_default(ws: Workspace) -> Option<bool> {
    match ws {
        Workspace::Setup | Workspace::Toolpaths => Some(true),
        Workspace::Simulation => Some(false),
        Workspace::Readiness => None,
    }
}

/// Setup on, Toolpaths off, Simulation off — the solid stock block.
fn setup_only_default(ws: Workspace) -> Option<bool> {
    match ws {
        Workspace::Setup => Some(true),
        Workspace::Toolpaths | Workspace::Simulation => Some(false),
        Workspace::Readiness => None,
    }
}

/// On in all three viewport workspaces — the alignment pins, which today's
/// code force-shows in Simulation when pins exist so pin-drill review keeps
/// working.
fn always_on_default(ws: Workspace) -> Option<bool> {
    match ws {
        Workspace::Setup | Workspace::Toolpaths | Workspace::Simulation => Some(true),
        Workspace::Readiness => None,
    }
}

/// Setup and Simulation off, Toolpaths on — cutting, rapids, entry markers.
fn moves_default(ws: Workspace) -> Option<bool> {
    match ws {
        Workspace::Toolpaths => Some(true),
        Workspace::Setup | Workspace::Simulation => Some(false),
        Workspace::Readiness => None,
    }
}

/// Toolpaths only — the height planes.
fn toolpaths_only_default(ws: Workspace) -> Option<bool> {
    match ws {
        Workspace::Toolpaths => Some(true),
        Workspace::Setup | Workspace::Simulation => Some(false),
        Workspace::Readiness => None,
    }
}

/// Simulation only — the simulated stock, collisions, the deflection panel.
fn simulation_only_default(ws: Workspace) -> Option<bool> {
    match ws {
        Workspace::Simulation => Some(true),
        Workspace::Setup | Workspace::Toolpaths => Some(false),
        Workspace::Readiness => None,
    }
}

/// Off in every workspace — the rest heatmap's default since the operator
/// ruled that Reach owns the model surface by default. It is named rather
/// than `no_default` so a workspace round trip RESTORES it to off: it is one
/// half of an exclusive pair, and leaving it unmanaged would let it survive
/// as the model's colour source after Reach was applied and then displaced.
fn rest_heatmap_default(ws: Workspace) -> Option<bool> {
    match ws {
        Workspace::Setup | Workspace::Toolpaths | Workspace::Simulation => Some(false),
        Workspace::Readiness => None,
    }
}

/// Simulation defaults the WP27 draw scope off; other workspaces leave the
/// operator's choice unmanaged. A named Simulation default is restored on
/// exit through `displaced`.
fn draw_scope_default(ws: Workspace) -> Option<bool> {
    match ws {
        Workspace::Simulation => Some(false),
        Workspace::Setup | Workspace::Toolpaths | Workspace::Readiness => None,
    }
}

fn no_default(_ws: Workspace) -> Option<bool> {
    None
}

// ── precondition helpers ───────────────────────────────────────────────────

fn always_ready(_state: &AppState) -> Precondition {
    Precondition::Ready
}

fn has_model_mesh(state: &AppState) -> bool {
    state.session.models().iter().any(|m| m.mesh.is_some())
}

fn selected_toolpath(state: &AppState) -> Option<crate::state::toolpath::ToolpathId> {
    match state.selection {
        crate::state::selection::Selection::Toolpath(id) => Some(id),
        _ => None,
    }
}

/// Does at least one toolpath hold a generated result?
pub(crate) fn any_generated(state: &AppState) -> bool {
    state.gui.toolpath_rt.values().any(|rt| rt.result.is_some())
}

/// `(threshold_mm, ramp_top_mm)` of the selected toolpath's rest grid.
///
/// **Any** operation attaches one when its `rest_analysis.enabled` is set and
/// a mesh plus spatial index are present; the pencil `RestDepth` arm and the
/// UnifiedFinish claims pipeline attach their own. The "only the pencil
/// detector" claim the old hover text carried was stale (audit §2c).
///
/// The top is [`rest_ramp_top`], the value at which the heatmap mesh
/// reaches full red. Before the viewport redesign it was the maximum, so
/// the red end of the legend named a larger number than the red end of the
/// mesh (inventory §3, the Rest row).
pub fn rest_grid_info(state: &AppState) -> Option<(f64, f32)> {
    let tp_id = selected_toolpath(state)?;
    state
        .gui
        .toolpath_rt
        .get(&tp_id)
        .and_then(|rt| rt.result.as_ref())
        .and_then(|r| r.annotated.rest_grid.as_ref())
        .map(|grid| (grid.threshold, rest_ramp_top(grid)))
}

/// The rest depth (mm) at which the heatmap mesh reaches full red: the 95th
/// percentile of the trusted cells above the threshold.
///
/// This is the same arithmetic as
/// `rs_cam_core::maps::rest_heatmap_mesh::rest_grid_to_heatmap_mesh`, which
/// does not publish its value. A cell is trusted when its rest depth and
/// its surface Z are both finite. The sentry
/// `the_dock_states_read_live_state_g_vpstate` builds a grid, asks the core
/// mesh for its colours and asserts that this value gives the same colours,
/// so a change on either side fails a test.
pub fn rest_ramp_top(grid: &rs_cam_core::surface::rest_field::RestGrid) -> f32 {
    let threshold = grid.threshold as f32;
    let mut above: Vec<f32> = (0..grid.grid.cell_count())
        .filter_map(|i| {
            let rest = grid.rest.get(i).copied().filter(|v| v.is_finite())?;
            grid.surface_z.get(i).copied().filter(|v| v.is_finite())?;
            (rest > threshold).then_some(rest)
        })
        .collect();
    if above.is_empty() {
        return threshold + 1e-6;
    }
    above.sort_by(f32::total_cmp);
    let last = above.len().saturating_sub(1);
    let index = ((last as f32) * 0.95).round() as usize;
    above
        .get(index)
        .or_else(|| above.last())
        .copied()
        .unwrap_or(threshold)
        .max(threshold + 1e-6)
}

/// The By Area region map of the toolpath selected RIGHT NOW, if its
/// result carries one. The overlay mesh, the order labels, the legend and
/// the row precondition all read this one function.
pub fn selected_area_regions(
    state: &AppState,
) -> Option<&std::sync::Arc<rs_cam_core::adaptive3d::AreaRegionMap>> {
    let tp_id = selected_toolpath(state)?;
    state
        .gui
        .toolpath_rt
        .get(&tp_id)
        .and_then(|rt| rt.result.as_ref())
        .and_then(|r| r.annotated.area_regions.as_ref())
}

/// Does the viewport draw the By Area regions this frame? The flag is on
/// and the row precondition is `Ready`. The callback gate, the order labels
/// and the legend read this one function.
pub fn area_regions_drawn(state: &AppState) -> bool {
    state.viewport.show_area_regions && area_regions_precondition(state).is_ready()
}

/// Can the By Area regions overlay draw, and if not, why not?
fn area_regions_precondition(state: &AppState) -> Precondition {
    use rs_cam_core::compute::catalog::OperationConfig;
    use rs_cam_core::compute::operation_configs::RegionOrdering;
    if state.workspace != Workspace::Toolpaths {
        return Precondition::no("the By Area regions draw in the Toolpaths workspace");
    }
    if selected_area_regions(state).is_some() {
        return Precondition::Ready;
    }
    let Some(tp_id) = selected_toolpath(state) else {
        return Precondition::no("select a 3D Rough with By Area ordering");
    };
    let ordering = state
        .session
        .find_toolpath_config_by_id(tp_id)
        .and_then(|(_, tc)| match &tc.operation {
            OperationConfig::Adaptive3d(cfg) => Some(cfg.region_ordering),
            _ => None,
        });
    match ordering {
        None => Precondition::no("this operation is not a 3D Rough"),
        Some(RegionOrdering::Global) => {
            Precondition::no("this 3D Rough uses Global ordering \u{2014} it detects no regions")
        }
        Some(RegionOrdering::ByArea) => Precondition::no_with(
            "generate this toolpath to see its regions".to_owned(),
            OverlayAction::GenerateAll,
        ),
    }
}

/// `Some(setup_index)` when the planner holds a `Ready` preview.
fn ready_preview_setup(state: &AppState) -> Option<usize> {
    state
        .multitool_planner
        .as_ref()
        .filter(|p| p.ready_preview().is_some())
        .map(|p| p.setup_index)
}

/// The failure text of the reach walk for the toolpath selected RIGHT NOW,
/// if that walk failed.
fn reach_map_failure(state: &AppState) -> Option<&str> {
    use crate::state::runtime::ReachStatus;
    let id = selected_toolpath(state)?;
    if state.gui.reach_overlay.toolpath != Some(id) {
        return None;
    }
    match &state.gui.reach_overlay.status {
        ReachStatus::Failed(message) => Some(message.as_str()),
        _ => None,
    }
}

/// Is a reach map held, or on its way, for the toolpath selected RIGHT NOW?
///
/// `Computing` counts as drawable. The controller schedules the walk from the
/// SELECTION, not from the overlay flag, so a cold map always arrives; making
/// the row wait for it would grey the checkbox for the seconds the walk takes
/// and refuse an MCP request outright. The draw gate in `app/viewport.rs`
/// still requires a `Ready` map, so nothing wrong is painted meanwhile — the
/// plain model draws.
fn reach_map_wanted(state: &AppState) -> bool {
    use crate::state::runtime::ReachStatus;
    match selected_toolpath(state) {
        Some(id) => {
            state.gui.reach_overlay.toolpath == Some(id)
                && matches!(
                    state.gui.reach_overlay.status,
                    ReachStatus::Ready(_) | ReachStatus::Computing
                )
        }
        None => false,
    }
}

/// The active setup's session record, for the fixture-family preconditions.
fn active_setup(state: &AppState) -> Option<&rs_cam_core::session::SetupData> {
    let index = state.active_setup_index()?;
    state.session.list_setups().get(index)
}

fn in_simulation(state: &AppState) -> bool {
    state.workspace == Workspace::Simulation
}

// ── the list ───────────────────────────────────────────────────────────────

/// Every overlay, in panel order.
pub const ROWS: &[OverlayRow] = &[
    // ── Geometry ──────────────────────────────────────────────────────────
    OverlayRow {
        id: "grid",
        group: OverlayGroup::Geometry,
        label: "Grid",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::DrawTime,
        flag: Some("viewport.show_grid"),
        radio: false,
        get: |s| s.viewport.show_grid,
        set: |s, on| s.viewport.show_grid = on,
        precondition: always_ready,
        default_for: geometry_default,
        hover: "The ground grid.",
    },
    OverlayRow {
        id: "model",
        group: OverlayGroup::Geometry,
        label: "Model",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::DrawTime,
        flag: Some("viewport.show_model"),
        radio: false,
        get: |s| s.viewport.show_model,
        set: |s, on| s.viewport.show_model = on,
        precondition: |s| {
            if in_simulation(s) {
                // One of the TWO gates that stay hard (UX §6.4). The dexel
                // stock replaces the model here, so a checkbox would be dead.
                Precondition::no("replaced by the simulated stock here")
            } else if !has_model_mesh(s) {
                Precondition::no("this project carries no 3D model")
            } else {
                Precondition::Ready
            }
        },
        default_for: no_default,
        hover: "The imported STL / STEP surface. Applies to both draw loops.",
    },
    OverlayRow {
        id: "stock_box",
        group: OverlayGroup::Geometry,
        label: "Stock \u{2014} box",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::DrawTime,
        flag: Some("viewport.show_stock"),
        radio: false,
        get: |s| s.viewport.show_stock,
        set: |s, on| s.viewport.show_stock = on,
        precondition: always_ready,
        default_for: geometry_default,
        hover: "The stock wireframe box. It is derived from the stock \
                config, so it draws on a 2D job too \u{2014} before P6 the gate \
                required a MESH and the checkbox did nothing on an SVG or DXF \
                project.",
    },
    OverlayRow {
        id: "stock_solid",
        group: OverlayGroup::Geometry,
        label: "Stock \u{2014} solid",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::DrawTime,
        flag: Some("viewport.show_stock_solid"),
        radio: false,
        get: |s| s.viewport.show_stock_solid,
        set: |s, on| s.viewport.show_stock_solid = on,
        precondition: always_ready,
        default_for: setup_only_default,
        hover: "The opaque stock block. Split from the stock box \u{2014} one \
                checkbox used to remove the box, the block and the axes together.",
    },
    OverlayRow {
        id: "origin_axes",
        group: OverlayGroup::Geometry,
        label: "Origin axes",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::DrawTime,
        flag: Some("viewport.show_origin_axes"),
        radio: false,
        get: |s| s.viewport.show_origin_axes,
        set: |s, on| s.viewport.show_origin_axes = on,
        precondition: always_ready,
        default_for: geometry_default,
        hover: "The XYZ triad at the stock origin. Distinct from the datum \
                crosshair, which is where the G-code zeroes.",
    },
    OverlayRow {
        id: "datum",
        group: OverlayGroup::Geometry,
        label: "Datum crosshair",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::UploadTime(UPLOAD_DETECTOR),
        flag: Some("viewport.show_datum"),
        radio: false,
        get: |s| s.viewport.show_datum,
        set: |s, on| s.viewport.show_datum = on,
        precondition: |s| {
            use rs_cam_core::session::XYDatum;
            match active_setup(s) {
                None => Precondition::no("no setup in this project"),
                Some(sd) => match sd.datum.xy_method {
                    XYDatum::CornerProbe(_) | XYDatum::CenterOfStock => Precondition::Ready,
                    _ => Precondition::no("no datum set on this setup"),
                },
            }
        },
        default_for: geometry_default,
        hover: "The XY zero the export writes (magenta crosshair).",
    },
    OverlayRow {
        id: "fixtures",
        group: OverlayGroup::Geometry,
        label: "Fixtures",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::UploadTime(UPLOAD_DETECTOR),
        flag: Some("viewport.show_fixtures"),
        radio: false,
        get: |s| s.viewport.show_fixtures,
        set: |s, on| s.viewport.show_fixtures = on,
        precondition: |s| match active_setup(s) {
            Some(sd) if sd.fixtures.iter().any(|f| f.enabled) => Precondition::Ready,
            Some(_) => Precondition::no("no fixture in this setup"),
            None => Precondition::no("no setup in this project"),
        },
        default_for: geometry_default,
        hover: "Fixture clearance boxes. Before P6 this one checkbox also \
                drew keep-outs, pins, the flip axis and the datum.",
    },
    OverlayRow {
        id: "keep_outs",
        group: OverlayGroup::Geometry,
        label: "Keep-out zones",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::UploadTime(UPLOAD_DETECTOR),
        flag: Some("viewport.show_keep_outs"),
        radio: false,
        get: |s| s.viewport.show_keep_outs,
        set: |s, on| s.viewport.show_keep_outs = on,
        precondition: |s| match active_setup(s) {
            Some(sd) if sd.keep_out_zones.iter().any(|k| k.enabled) => Precondition::Ready,
            Some(_) => Precondition::no("no keep-out zone in this setup"),
            None => Precondition::no("no setup in this project"),
        },
        default_for: geometry_default,
        hover: "Red wireframe forbidden volumes.",
    },
    OverlayRow {
        id: "alignment_pins",
        group: OverlayGroup::Geometry,
        label: "Alignment pins",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::UploadTime(UPLOAD_DETECTOR),
        flag: Some("viewport.show_alignment_pins"),
        radio: false,
        get: |s| s.viewport.show_alignment_pins,
        set: |s, on| s.viewport.show_alignment_pins = on,
        precondition: |s| {
            if s.session.stock_config().alignment_pins.is_empty() {
                Precondition::no("no alignment pin on this stock")
            } else {
                Precondition::Ready
            }
        },
        default_for: always_on_default,
        hover: "Green circles at the pin centres. Authored in the Stock panel.",
    },
    OverlayRow {
        id: "flip_axis",
        group: OverlayGroup::Geometry,
        label: "Flip axis",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::UploadTime(UPLOAD_DETECTOR),
        flag: Some("viewport.show_flip_axis"),
        radio: false,
        get: |s| s.viewport.show_flip_axis,
        set: |s, on| s.viewport.show_flip_axis = on,
        precondition: |s| {
            if s.session.stock_config().flip_axis.is_some() {
                Precondition::Ready
            } else {
                Precondition::no("no flip axis set on this stock")
            }
        },
        default_for: geometry_default,
        hover: "The dashed centreline a two-sided job flips about.",
    },
    OverlayRow {
        id: "curves",
        group: OverlayGroup::Geometry,
        label: "Curves (DXF/SVG)",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::DrawTime,
        flag: Some("viewport.show_polygons"),
        radio: false,
        get: |s| s.viewport.show_polygons,
        set: |s, on| s.viewport.show_polygons = on,
        precondition: |s| {
            if s.session.models().iter().any(|m| m.polygons.is_some()) {
                Precondition::Ready
            } else {
                Precondition::no("this project carries no 2D curves")
            }
        },
        default_for: no_default,
        hover: "Imported 2D polygons.",
    },
    OverlayRow {
        id: "orientation_gizmo",
        group: OverlayGroup::Geometry,
        label: "Orientation gizmo",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::DrawTime,
        flag: Some("viewport.show_orientation_gizmo"),
        radio: false,
        get: |s| s.viewport.show_orientation_gizmo,
        set: |s, on| s.viewport.show_orientation_gizmo = on,
        precondition: always_ready,
        default_for: no_default,
        hover: "The camera-axis triad in the viewport corner.",
    },
    // ── Toolpath ──────────────────────────────────────────────────────────
    OverlayRow {
        id: "all_toolpaths",
        group: OverlayGroup::Toolpath,
        label: "All toolpaths",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::UploadTime(UPLOAD_DETECTOR),
        flag: Some("viewport.show_all_toolpaths"),
        radio: false,
        get: |s| s.viewport.show_all_toolpaths,
        set: |s, on| s.viewport.show_all_toolpaths = on,
        precondition: |s| {
            if any_generated(s) {
                Precondition::Ready
            } else {
                Precondition::no_with(
                    "no toolpath generated yet".to_owned(),
                    OverlayAction::GenerateAll,
                )
            }
        },
        default_for: draw_scope_default,
        hover: "Draw every generated toolpath. Off draws the selected one only.",
    },
    OverlayRow {
        id: "cutting_moves",
        group: OverlayGroup::Toolpath,
        label: "Cutting moves",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::DrawTime,
        flag: Some("viewport.show_cutting"),
        radio: false,
        get: |s| s.viewport.show_cutting,
        set: |s, on| s.viewport.show_cutting = on,
        precondition: |s| {
            if any_generated(s) {
                Precondition::Ready
            } else {
                Precondition::no_with(
                    "no toolpath generated yet".to_owned(),
                    OverlayAction::GenerateAll,
                )
            }
        },
        default_for: moves_default,
        hover: "Fed moves, in each toolpath's colour. Per-toolpath: each row's C.",
    },
    OverlayRow {
        id: "rapids",
        group: OverlayGroup::Toolpath,
        label: "Rapids",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::DrawTime,
        flag: Some("viewport.show_rapids"),
        radio: false,
        get: |s| s.viewport.show_rapids,
        set: |s, on| s.viewport.show_rapids = on,
        precondition: |s| {
            if any_generated(s) {
                Precondition::Ready
            } else {
                Precondition::no_with(
                    "no toolpath generated yet".to_owned(),
                    OverlayAction::GenerateAll,
                )
            }
        },
        default_for: moves_default,
        hover: "Rapid moves, a darker toolpath colour. Per-toolpath: each row's R.",
    },
    OverlayRow {
        id: "entry_markers",
        group: OverlayGroup::Toolpath,
        label: "Entry markers",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::DrawTime,
        flag: Some("viewport.show_entry_markers"),
        radio: false,
        get: |s| s.viewport.show_entry_markers,
        set: |s, on| s.viewport.show_entry_markers = on,
        precondition: |s| {
            if selected_toolpath(s).is_some() {
                Precondition::Ready
            } else {
                Precondition::no("select a toolpath to see its entry moves")
            }
        },
        default_for: moves_default,
        hover: "Cyan markers where each entry / ramp / helix starts. \
                Geometric only \u{2014} it shows WHERE an entry is, never how hard.",
    },
    OverlayRow {
        id: "height_planes",
        group: OverlayGroup::Toolpath,
        label: "Height planes",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::DrawTime,
        flag: Some("viewport.show_height_planes"),
        radio: false,
        get: |s| s.viewport.show_height_planes,
        set: |s, on| s.viewport.show_height_planes = on,
        precondition: |s| {
            if selected_toolpath(s).is_some() {
                Precondition::Ready
            } else {
                Precondition::no("select a toolpath to see its Z planes")
            }
        },
        default_for: toolpaths_only_default,
        hover: "Clearance, retract, feed, top and bottom Z.",
    },
    OverlayRow {
        id: "tool_profile_ghost",
        group: OverlayGroup::Toolpath,
        label: "Tool-profile ghost",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::UploadTime(UPLOAD_DETECTOR),
        flag: Some("viewport.show_tool_profile_preview"),
        radio: false,
        get: |s| s.viewport.show_tool_profile_preview,
        set: |s, on| s.viewport.show_tool_profile_preview = on,
        precondition: |s| {
            if selected_toolpath(s).is_some() {
                Precondition::Ready
            } else {
                Precondition::no("select a toolpath to see its cutter ghost")
            }
        },
        default_for: no_default,
        hover: "The swept cutter silhouette along the selected toolpath.",
    },
    OverlayRow {
        id: "span_entry",
        group: OverlayGroup::Toolpath,
        label: "Spans: Entry",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::UploadTime(UPLOAD_DETECTOR),
        flag: Some("viewport.span_kind_filter.show_entry"),
        radio: false,
        get: |s| s.viewport.span_kind_filter.show_entry,
        set: |s, on| s.viewport.span_kind_filter.show_entry = on,
        precondition: span_filter_precondition,
        default_for: no_default,
        hover: "Plunge / ramp / helix lead-in segments.",
    },
    OverlayRow {
        id: "span_lead_out",
        group: OverlayGroup::Toolpath,
        label: "Spans: LeadOut",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::UploadTime(UPLOAD_DETECTOR),
        flag: Some("viewport.span_kind_filter.show_lead_out"),
        radio: false,
        get: |s| s.viewport.span_kind_filter.show_lead_out,
        set: |s, on| s.viewport.span_kind_filter.show_lead_out = on,
        precondition: span_filter_precondition,
        default_for: no_default,
        hover: "Lead-out / retract transition segments.",
    },
    OverlayRow {
        id: "span_link_bridge",
        group: OverlayGroup::Toolpath,
        label: "Spans: LinkBridge",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::UploadTime(UPLOAD_DETECTOR),
        flag: Some("viewport.span_kind_filter.show_link_bridge"),
        radio: false,
        get: |s| s.viewport.span_kind_filter.show_link_bridge,
        set: |s, on| s.viewport.span_kind_filter.show_link_bridge = on,
        precondition: span_filter_precondition,
        default_for: no_default,
        hover: "Linker bridges inserted between regions.",
    },
    OverlayRow {
        id: "span_dressup",
        group: OverlayGroup::Toolpath,
        label: "Spans: DressupArtifact",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::UploadTime(UPLOAD_DETECTOR),
        flag: Some("viewport.span_kind_filter.show_dressup"),
        radio: false,
        get: |s| s.viewport.span_kind_filter.show_dressup,
        set: |s, on| s.viewport.span_kind_filter.show_dressup = on,
        precondition: span_filter_precondition,
        default_for: no_default,
        hover: "Dogbone overcuts and other dressup-introduced bridge segments. \
                Arc-fit replacements are ordinary cutting geometry and are not \
                filtered here.",
    },
    // ── Regions ───────────────────────────────────────────────────────────
    OverlayRow {
        id: "rest_heatmap",
        group: OverlayGroup::Regions,
        label: "Rest heatmap",
        surface: OverlaySurface::Model,
        mechanism: OverlayMechanism::DrawTime,
        flag: Some("viewport.show_rest_heatmap"),
        radio: false,
        get: |s| s.viewport.show_rest_heatmap,
        set: |s, on| s.viewport.show_rest_heatmap = on,
        precondition: |s| {
            if s.workspace != Workspace::Toolpaths {
                Precondition::no("the rest heatmap draws in the Toolpaths workspace")
            } else if rest_grid_info(s).is_some() {
                Precondition::Ready
            } else if selected_toolpath(s).is_some() {
                Precondition::no_with(
                    "this toolpath has no rest grid yet".to_owned(),
                    OverlayAction::EnableRestAnalysis,
                )
            } else {
                Precondition::no("select a toolpath that carries a rest grid")
            }
        },
        // Off in every workspace. It shares the model surface with Reach,
        // which the operator asked to be the always-on answer, so this row
        // is the diagnostic they switch on — and switching it on clears
        // Reach.
        default_for: rest_heatmap_default,
        hover: "How much material this operation leaves. ANY operation \
                attaches a rest grid once something demands one. Off by \
                default: switching it on clears Inspect \u{25B8} Model colour: \
                Reach, which shares the model surface.",
    },
    OverlayRow {
        id: "tier_map",
        group: OverlayGroup::Regions,
        label: "Tier map",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::DrawTime,
        flag: Some("viewport.show_tier_preview"),
        radio: false,
        get: |s| s.viewport.show_tier_preview,
        set: |s, on| s.viewport.show_tier_preview = on,
        precondition: |s| {
            if s.workspace != Workspace::Toolpaths {
                return Precondition::no("the tier map draws in the Toolpaths workspace");
            }
            match ready_preview_setup(s) {
                None => Precondition::no_with(
                    "run a preview from Toolpath \u{25B8} Plan multi-tool finishing\u{2026}"
                        .to_owned(),
                    OverlayAction::OpenPlanner,
                ),
                // The OTHER gate that stays hard (UX §6.4). The tier map
                // lives in its own setup's emission frame, so in another
                // setup's display frame it draws at a wrong offset — an
                // operator saw it floating beside the flipped back setup.
                // Before P6 the checkbox read ON here while nothing drew.
                Some(setup_index) if s.active_setup_index() != Some(setup_index) => {
                    Precondition::no(&format!(
                        "previewed on setup {} \u{2014} switch setup to see it",
                        setup_index + 1
                    ))
                }
                Some(_) => Precondition::Ready,
            }
        },
        default_for: no_default,
        hover: "One colour per tool tier over the territory that tier owns, \
                with each fine tier's overlap band in a lighter tint.",
    },
    OverlayRow {
        id: "area_regions",
        group: OverlayGroup::Regions,
        label: "By Area regions",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::DrawTime,
        flag: Some("viewport.show_area_regions"),
        radio: false,
        get: |s| s.viewport.show_area_regions,
        set: |s, on| s.viewport.show_area_regions = on,
        precondition: area_regions_precondition,
        default_for: no_default,
        hover: "The regions that the selected 3D Rough detected with By Area \
                ordering. Each region has one colour and one order number. \
                The thin box is the filter the planner cuts the region by. \
                The planner detects the regions once, before the first level. \
                One colour over the full part means one region: By Area then \
                cuts as Global does.",
    },
    OverlayRow {
        id: "planner_islands",
        group: OverlayGroup::Regions,
        label: "Planner islands",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::DrawTime,
        flag: None,
        radio: false,
        get: |_s| false,
        set: |_s, _on| {},
        // Not a fake row and not a second home: the islands ARE the
        // territory the tier map paints (`gpu_upload` passes
        // `preview.islands` into the tier mesh), so a flag of their own
        // would be a duplicate writer of one state. There is no separate
        // island-outline renderer, and the row says so.
        precondition: |_s| {
            Precondition::no(
                "drawn by Inspect \u{25B8} Tier map \u{2014} no separate island outline renderer yet",
            )
        },
        default_for: no_default,
        hover: "Per-tier island polygons. The numeric table lives in the \
                multi-tool planner dialog.",
    },
    OverlayRow {
        id: "derived_rest_regions",
        group: OverlayGroup::Regions,
        label: "Derived rest regions",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::DrawTime,
        flag: None,
        radio: false,
        get: |_s| false,
        set: |_s, _on| {},
        precondition: |_s| Precondition::no("not drawn yet (no renderer)"),
        default_for: no_default,
        hover: "The islands a rest pass would cut. Authored under \
                Geometry \u{25B8} Machining Boundary \u{25B8} Source \u{25B8} Rest Regions; \
                the render work is ledgered, not built.",
    },
    OverlayRow {
        id: "boundary_outline",
        group: OverlayGroup::Regions,
        label: "Boundary outline",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::DrawTime,
        flag: None,
        radio: false,
        get: |_s| false,
        set: |_s, _on| {},
        precondition: |_s| Precondition::no("not drawn yet (no renderer)"),
        default_for: no_default,
        hover: "The clip polygon in use. Authored under \
                Geometry \u{25B8} Machining Boundary; the render work is ledgered, \
                not built.",
    },
    // ── Analysis ──────────────────────────────────────────────────────────
    OverlayRow {
        id: "reach_map",
        group: OverlayGroup::Analysis,
        label: "Model colour: Reach",
        surface: OverlaySurface::Model,
        mechanism: OverlayMechanism::DrawTime,
        flag: Some("viewport.show_reach_map"),
        radio: false,
        get: |s| s.viewport.show_reach_map,
        set: |s, on| s.viewport.show_reach_map = on,
        precondition: |s| {
            if s.workspace != Workspace::Toolpaths {
                Precondition::no("the reach map draws in the Toolpaths workspace")
            } else if !s.viewport.show_model {
                Precondition::no(
                    "the reach map IS the model, re-coloured \u{2014} switch Scene \u{25B8} Model on",
                )
            } else if reach_map_wanted(s) {
                Precondition::Ready
            } else if let Some(message) = reach_map_failure(s) {
                // Before the viewport redesign a failed walk fell to the
                // reason below, so the operator read "select a finishing
                // operation" beside a finishing operation (inventory §2.3,
                // item 2). The MCP refusal carries the same words.
                Precondition::no(&format!("the reach walk failed \u{2014} {message}"))
            } else {
                // The controller answers "is this operation one a reach map
                // speaks about" by whether it scheduled a walk at all
                // (`OperationType::supports_reach_map`, plus a mesh and a
                // tool), so this one reason covers both "nothing selected"
                // and "this selection carries no reach question".
                Precondition::no("select a finishing operation")
            }
        },
        // ON in Toolpaths. The operator's ask for P5 was "show the reach map
        // when ANY finishing op is selected", so it is the model surface's
        // default colour source and the rest heatmap is the side that
        // defaults off.
        default_for: toolpaths_only_default,
        hover: "Green where this cutter forms the surface inside the \
                operation's tolerance, red where it cannot, neutral where not \
                measured. Clears Inspect \u{25B8} Rest heatmap \u{2014} both colour \
                the model.",
    },
    OverlayRow {
        id: "stock_colour_solid",
        group: OverlayGroup::Analysis,
        label: "Stock colour: Solid",
        surface: OverlaySurface::Stock,
        mechanism: OverlayMechanism::UploadTime(UPLOAD_DETECTOR),
        flag: Some("simulation.stock_viz_mode.solid"),
        radio: true,
        get: |s| matches!(s.simulation.stock_viz_mode, StockVizMode::Solid),
        set: |s, on| {
            if on {
                s.simulation.stock_viz_mode = StockVizMode::Solid;
            }
        },
        precondition: stock_colour_precondition,
        default_for: no_default,
        hover: "Default wood-tone gradient. No analysis colouring.",
    },
    OverlayRow {
        id: "stock_colour_deviation",
        group: OverlayGroup::Analysis,
        label: "Stock colour: Deviation",
        surface: OverlaySurface::Stock,
        mechanism: OverlayMechanism::UploadTime(UPLOAD_DETECTOR),
        flag: Some("simulation.stock_viz_mode.deviation"),
        radio: true,
        get: |s| matches!(s.simulation.stock_viz_mode, StockVizMode::Deviation),
        set: |s, on| {
            if on {
                s.simulation.stock_viz_mode = StockVizMode::Deviation;
            }
        },
        precondition: |s| {
            let base = stock_colour_precondition(s);
            if !base.is_ready() {
                return base;
            }
            if s.simulation.playback.display_deviations.is_none() {
                Precondition::no("no deviation data \u{2014} run a simulation")
            } else {
                Precondition::Ready
            }
        },
        default_for: no_default,
        hover: "Blue = material remaining, green = on target, red = over-cut.",
    },
    OverlayRow {
        id: "stock_colour_by_height",
        group: OverlayGroup::Analysis,
        label: "Stock colour: By height",
        surface: OverlaySurface::Stock,
        mechanism: OverlayMechanism::UploadTime(UPLOAD_DETECTOR),
        flag: Some("simulation.stock_viz_mode.by_height"),
        radio: true,
        get: |s| matches!(s.simulation.stock_viz_mode, StockVizMode::ByHeight),
        set: |s, on| {
            if on {
                s.simulation.stock_viz_mode = StockVizMode::ByHeight;
            }
        },
        precondition: stock_colour_precondition,
        default_for: no_default,
        hover: "Colour by Z height: low = blue, high = red.",
    },
    OverlayRow {
        id: "move_colour_palette",
        group: OverlayGroup::Analysis,
        label: "Move colour: Palette",
        surface: OverlaySurface::Moves,
        mechanism: OverlayMechanism::UploadTime(UPLOAD_DETECTOR),
        flag: Some("viewport.toolpath_color_mode.normal"),
        radio: true,
        get: |s| matches!(s.viewport.toolpath_color_mode, ToolpathColorMode::Normal),
        set: |s, on| {
            if on {
                s.viewport.toolpath_color_mode = ToolpathColorMode::Normal;
            }
        },
        precondition: move_colour_precondition,
        default_for: no_default,
        hover: "Per-toolpath palette colour with Z-depth blending. The only \
                mode the span filter applies in.",
    },
    OverlayRow {
        id: "move_colour_engagement",
        group: OverlayGroup::Analysis,
        label: "Move colour: Engagement",
        surface: OverlaySurface::Moves,
        mechanism: OverlayMechanism::UploadTime(UPLOAD_DETECTOR),
        flag: Some("viewport.toolpath_color_mode.engagement"),
        radio: true,
        get: |s| {
            matches!(
                s.viewport.toolpath_color_mode,
                ToolpathColorMode::Engagement
            )
        },
        set: |s, on| {
            if on {
                s.viewport.toolpath_color_mode = ToolpathColorMode::Engagement;
            }
        },
        precondition: move_colour_precondition,
        default_for: no_default,
        hover: "Colour cutting moves by feed rate: green \u{2192} yellow \u{2192} red \
                for light \u{2192} heavy load.",
    },
    OverlayRow {
        id: "move_colour_advance_per_tooth",
        group: OverlayGroup::Analysis,
        label: "Move colour: Advance / tooth",
        surface: OverlaySurface::Moves,
        mechanism: OverlayMechanism::UploadTime(UPLOAD_DETECTOR),
        flag: Some("viewport.toolpath_color_mode.advance_per_tooth"),
        radio: true,
        get: |s| {
            matches!(
                s.viewport.toolpath_color_mode,
                ToolpathColorMode::AdvancePerTooth
            )
        },
        set: |s, on| {
            if on {
                s.viewport.toolpath_color_mode = ToolpathColorMode::AdvancePerTooth;
            }
        },
        precondition: |s| {
            let base = move_colour_precondition(s);
            if !base.is_ready() {
                return base;
            }
            if s.simulation.has_results() {
                Precondition::Ready
            } else {
                Precondition::no("advance per tooth needs a simulation")
            }
        },
        default_for: no_default,
        hover: "Achieved advance per tooth (effective feed \u{00F7} (RPM \u{00D7} flutes)) \
                against the matched vendor band \u{2014} the same quantity the \
                tool-load gate observes.",
    },
    OverlayRow {
        id: "simulated_stock",
        group: OverlayGroup::Analysis,
        label: "Simulated stock",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::DrawTime,
        flag: Some("viewport.show_sim_stock"),
        radio: false,
        get: |s| s.viewport.show_sim_stock,
        set: |s, on| s.viewport.show_sim_stock = on,
        precondition: |s| {
            if s.workspace != Workspace::Simulation {
                Precondition::no("the simulated stock draws in the Simulation workspace")
            } else if s.simulation.has_results() {
                Precondition::Ready
            } else {
                Precondition::no("run a simulation")
            }
        },
        default_for: simulation_only_default,
        hover: "The dexel stock as cut. Its opacity slider is below.",
    },
    OverlayRow {
        id: "collisions",
        group: OverlayGroup::Analysis,
        label: "Collisions",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::DrawTime,
        flag: Some("viewport.show_collisions"),
        radio: false,
        get: |s| s.viewport.show_collisions,
        set: |s, on| s.viewport.show_collisions = on,
        precondition: |s| {
            let checks = &s.simulation.checks;
            if checks.collision_report.is_some() || !checks.rapid_collisions.is_empty() {
                Precondition::Ready
            } else {
                Precondition::no_with(
                    "no collision check has run".to_owned(),
                    OverlayAction::RunCollisionCheck,
                )
            }
        },
        default_for: simulation_only_default,
        hover: "Holder and shank strike points, graded yellow \u{2192} red by \
                local density.",
    },
    OverlayRow {
        id: "tool_deflection",
        group: OverlayGroup::Analysis,
        label: "Tool deflection",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::DrawTime,
        flag: Some("viewport.show_tool_deflection"),
        radio: false,
        get: |s| s.viewport.show_tool_deflection,
        set: |s, on| s.viewport.show_tool_deflection = on,
        precondition: |s| {
            if s.workspace != Workspace::Simulation {
                Precondition::no("the deflection panel draws in the Simulation workspace")
            } else if s.simulation.has_results() {
                Precondition::Ready
            } else {
                Precondition::no("run a simulation")
            }
        },
        default_for: simulation_only_default,
        hover: "Deflection in \u{00B5}m, drawn as a bent cutter. 200\u{00D7} \
                exaggerated; direction is illustrative.",
    },
    OverlayRow {
        id: "generator_steps",
        group: OverlayGroup::Analysis,
        label: "Generator steps",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::DrawTime,
        flag: Some("simulation.debug.enabled"),
        radio: false,
        get: |s| s.simulation.debug.enabled,
        set: |s, on| s.simulation.debug.enabled = on,
        precondition: |s| {
            if any_trace_recorded(s) {
                Precondition::Ready
            } else {
                Precondition::no_with(
                    "switch on Record generator trace and regenerate".to_owned(),
                    OverlayAction::RecordGeneratorTrace,
                )
            }
        },
        default_for: no_default,
        hover: "A semantic timeline band plus a per-toolpath outline of \
                generator steps.",
    },
    OverlayRow {
        id: "active_step_highlight",
        group: OverlayGroup::Analysis,
        label: "Highlight active step",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::DrawTime,
        flag: Some("simulation.debug.highlight_active_item"),
        radio: false,
        get: |s| s.simulation.debug.highlight_active_item,
        set: |s, on| s.simulation.debug.highlight_active_item = on,
        precondition: |s| {
            if !any_trace_recorded(s) {
                Precondition::no_with(
                    "switch on Record generator trace and regenerate".to_owned(),
                    OverlayAction::RecordGeneratorTrace,
                )
            } else if !s.simulation.debug.enabled {
                Precondition::no("switch Generator steps on first")
            } else {
                Precondition::Ready
            }
        },
        default_for: no_default,
        hover: "Box the generator step playback is inside, in the 3D viewport.",
    },
];

/// The span filter is applied at GPU upload and only in the `Palette` arm —
/// the Engagement and Advance-per-tooth builders take no filter argument, so
/// outside Palette these four rows are inert. Before P6 the submenu gave no
/// sign of that (audit §3.5, fix §6.9).
fn span_filter_precondition(state: &AppState) -> Precondition {
    if !matches!(
        state.viewport.toolpath_color_mode,
        ToolpathColorMode::Normal
    ) {
        Precondition::no("applies in Palette move colour only")
    } else if any_generated(state) {
        Precondition::Ready
    } else {
        Precondition::no_with(
            "no toolpath generated yet".to_owned(),
            OverlayAction::GenerateAll,
        )
    }
}

fn stock_colour_precondition(state: &AppState) -> Precondition {
    if state.simulation.has_results() {
        Precondition::Ready
    } else {
        Precondition::no("run a simulation")
    }
}

fn move_colour_precondition(state: &AppState) -> Precondition {
    if any_generated(state) {
        Precondition::Ready
    } else {
        Precondition::no_with(
            "no toolpath generated yet".to_owned(),
            OverlayAction::GenerateAll,
        )
    }
}

fn any_trace_recorded(state: &AppState) -> bool {
    state
        .gui
        .toolpath_rt
        .values()
        .any(|rt| rt.debug_trace.is_some() || rt.semantic_trace.is_some())
}

// ── lookup and application ─────────────────────────────────────────────────

/// The row with this id.
pub fn row(id: &str) -> Option<&'static OverlayRow> {
    ROWS.iter().find(|r| r.id == id)
}

/// Every row, in list order. The dock, the catalogue and the dock sentry
/// iterate this list.
pub fn rows() -> &'static [OverlayRow] {
    ROWS
}

/// The dock section that is the ONE home of `row` (viewport redesign,
/// MOCKUPS §1).
///
/// One home per row keeps the rule "one state, one writer". The function is
/// total: a row that the named arms do not list falls to the section of its
/// group, so a new row always has a home in the dock. The catalogue lists
/// every row in addition.
pub fn dock_section(row: &OverlayRow) -> DockSection {
    match row.id {
        "orientation_gizmo" => DockSection::View,
        "derived_rest_regions" | "boundary_outline" | "simulated_stock" => DockSection::Scene,
        _ => match (row.group, row.surface) {
            (_, OverlaySurface::Moves) | (OverlayGroup::Toolpath, _) => DockSection::Paths,
            (OverlayGroup::Geometry, _) => DockSection::Scene,
            (OverlayGroup::Regions | OverlayGroup::Analysis, _) => DockSection::Inspect,
        },
    }
}

/// The rows whose dock home is `section`, in list order.
pub fn rows_in_section(section: DockSection) -> impl Iterator<Item = &'static OverlayRow> {
    ROWS.iter().filter(move |r| dock_section(r) == section)
}

/// Switch off every row on one surface. This is the `Off` choice of the
/// model colour. A radio surface has no `Off`, so the call does nothing
/// there.
pub fn clear_surface(state: &mut AppState, surface: OverlaySurface) {
    if surface == OverlaySurface::None {
        return;
    }
    for row in ROWS {
        if row.surface == surface && !row.radio && (row.get)(state) {
            set_overlay(state, row, false);
        }
    }
}

/// Put each exclusive surface back to at most one row on.
///
/// Every dock and MCP write goes through [`set_overlay`], which keeps this
/// rule. One writer does not: the inspector's `Show reach map` checkbox
/// writes `show_reach_map` straight into the state (inventory §2.3, item 6;
/// MOCKUPS §12, Q4), so it can leave Reach and the rest heatmap both on,
/// and the renderer then draws both. The dock calls this function before it
/// draws, and the frame loop draws the dock before it builds the viewport
/// callback, so no frame draws two colour sources on one surface.
///
/// When two rows are on, the LAST one in [`ROWS`] stays on. On the model
/// surface that is Reach, which is the row the direct writer writes: a
/// switch through [`set_overlay`] would already have cleared the other.
/// The radio surfaces are enums and can never hold two, so the rule has
/// work on the model surface only.
pub fn settle_exclusive_surfaces(state: &mut AppState) {
    for surface in OverlaySurface::EXCLUSIVE {
        let on: Vec<&'static OverlayRow> = ROWS
            .iter()
            .filter(|row| row.surface == surface && (row.get)(state))
            .collect();
        if let Some((keep, rest)) = on.split_last() {
            for row in rest {
                (row.set)(state, false);
            }
            if !rest.is_empty() {
                tracing::debug!(
                    kept = keep.id,
                    "two colour sources were on one surface; a direct writer bypassed set_overlay"
                );
            }
        }
    }
}

/// Switch one row on or off, enforcing per-surface exclusivity.
///
/// Enabling a scalar-field row clears every other row on the same surface —
/// so switching Reach on clears the rest heatmap, and the panel prints the
/// reason on both. Territory rows carry [`OverlaySurface::None`] and stack.
pub fn set_overlay(state: &mut AppState, row: &OverlayRow, on: bool) {
    (row.set)(state, on);
    if !on || row.surface == OverlaySurface::None {
        return;
    }
    for peer in ROWS {
        if peer.id != row.id && peer.surface == row.surface && (peer.get)(state) {
            (peer.set)(state, false);
        }
    }
}

/// What [`apply_overlays`] did.
///
/// **Test door.** Stays `pub` for two reasons: it is the return type of
/// `pub fn apply_overlays`, and `crates/rs_cam_viz/tests/overlays_registry.rs`
/// binds that return at fifteen sites. The test never writes the type name,
/// so the S29 instrument, which counts name occurrences, read this row as
/// own-file-only (S29, tech debt 2026-09-16).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ApplyReport {
    /// id → the value now in force.
    pub applied: BTreeMap<String, bool>,
    /// id → why it was not applied. The string is the registry's own reason,
    /// so the panel and the MCP reply can never disagree.
    pub refused: BTreeMap<String, String>,
}

/// Apply an MCP `overlays` map. Nothing is silently dropped: every key lands
/// in exactly one of `applied` or `refused`.
///
/// Switching a row OFF needs no precondition — hiding something that cannot
/// draw is harmless and idempotent. Switching one ON does.
pub fn apply_overlays(state: &mut AppState, requested: &BTreeMap<String, bool>) -> ApplyReport {
    let mut report = ApplyReport::default();
    if requested.is_empty() {
        return report;
    }
    if state.workspace == Workspace::Readiness {
        for id in requested.keys() {
            report.refused.insert(
                id.clone(),
                "the Readiness workspace renders no viewport".to_owned(),
            );
        }
        return report;
    }
    for (id, want) in requested {
        let Some(row) = row(id.as_str()) else {
            report.refused.insert(
                id.clone(),
                format!("unknown overlay id '{id}' \u{2014} see All viewport options"),
            );
            continue;
        };
        if *want {
            let pre = (row.precondition)(state);
            if let Some(reason) = pre.reason() {
                report.refused.insert(id.clone(), reason.to_owned());
                continue;
            }
            set_overlay(state, row, true);
        } else {
            if row.radio {
                report.refused.insert(
                    id.clone(),
                    format!(
                        "'{id}' is one of the {} colour choices \u{2014} switch another \
                         one on instead of switching this off",
                        row.surface.label()
                    ),
                );
                continue;
            }
            set_overlay(state, row, false);
        }
        report.applied.insert(id.clone(), (row.get)(state));
    }
    report
}

/// Switch workspace and apply that workspace's overlay defaults, in that
/// order.
///
/// **Call this synchronously, never through the event queue, when overlay
/// writes follow in the same call.** `UiCommand::SwitchWorkspace` lands later
/// in the frame, so an MCP request that pushed the event and then wrote
/// overlays would have its writes clobbered by the arriving defaults, and
/// every precondition it evaluated would have answered about the OLD
/// workspace.
pub fn switch_workspace(state: &mut AppState, target: Workspace) {
    if state.workspace == target && state.overlays.defaults_applied_for == Some(target) {
        return;
    }
    state.workspace = target;
    apply_workspace_defaults(state, target);
}

/// Apply `target`'s per-workspace defaults, restoring whatever the previous
/// workspace displaced first.
///
/// This generalises the ad-hoc three-flag save/restore that
/// `UiCommand::SwitchWorkspace` used to carry (audit §3.4). Two properties
/// matter. A row the target workspace names no default for is untouched, so
/// an operator override survives a round trip through a workspace that does
/// not care about it. And the write goes through [`set_overlay`], so a
/// default can never break per-surface exclusivity.
pub(crate) fn apply_workspace_defaults(state: &mut AppState, target: Workspace) {
    let displaced = std::mem::take(&mut state.overlays.displaced);
    for (id, value) in displaced {
        if let Some(row) = row(id) {
            set_overlay(state, row, value);
        }
    }
    let mut newly_displaced = Vec::new();
    for row in ROWS {
        let Some(default) = (row.default_for)(target) else {
            continue;
        };
        let current = (row.get)(state);
        if current != default {
            newly_displaced.push((row.id, current));
            set_overlay(state, row, default);
        }
    }
    state.overlays.displaced = newly_displaced;
    state.overlays.defaults_applied_for = Some(target);
}

/// Step the active scalar-field choice on one surface, skipping any member
/// whose precondition is not `Ready` (UX §6.8's `,` / `.` shortcuts).
///
/// A radio surface — the stock and the move colours — always has exactly one
/// member on, so the cycle runs through the members only. The model surface
/// has a genuine "none" state (no colour source: the plain model), so `none`
/// is one stop of its cycle.
pub fn cycle_surface(state: &mut AppState, surface: OverlaySurface) {
    if surface == OverlaySurface::None {
        return;
    }
    let members: Vec<&'static OverlayRow> = ROWS.iter().filter(|r| r.surface == surface).collect();
    if members.is_empty() {
        return;
    }
    let has_none_state = !members.iter().any(|r| r.radio);
    let current = members.iter().position(|r| (r.get)(state));
    // Stops are the members, plus a trailing "none" stop where one exists.
    let stops = members.len() + usize::from(has_none_state);
    let start = current.unwrap_or(members.len());
    for step in 1..=stops {
        let next = (start + step) % stops;
        match members.get(next) {
            Some(row) if (row.precondition)(state).is_ready() => {
                set_overlay(state, row, true);
                return;
            }
            Some(_) => {}
            // The "none" stop: clear every member on this surface.
            None => {
                for row in &members {
                    if (row.get)(state) {
                        (row.set)(state, false);
                    }
                }
                return;
            }
        }
    }
}

/// Which surface the `,` and `.` shortcuts step (UX §6.8). The keys live in
/// `app/input.rs`; the surfaces live here, beside [`cycle_surface`].
pub const COMMA_SURFACE: OverlaySurface = OverlaySurface::Model;
pub const PERIOD_SURFACE: OverlaySurface = OverlaySurface::Stock;

/// How many overlays sit away from their workspace default. The dock's
/// catalogue button carries the count, so a closed catalogue still says that
/// something is switched on.
pub fn non_default_count(state: &AppState) -> usize {
    ROWS.iter()
        .filter(|row| match (row.default_for)(state.workspace) {
            Some(default) => (row.get)(state) != default,
            None => false,
        })
        .count()
}

// ── legends ────────────────────────────────────────────────────────────────

/// One active scalar-field legend. The panel builds each strip from the
/// SAME colour function the mesh or the line buffer uses, so a legend cannot
/// drift out of sync with what is drawn — the rule the shipped rest legend
/// already followed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Legend {
    /// `(threshold_mm, ramp_top_mm)`; colours from
    /// `rest_heatmap_mesh::rest_ramp_color`. The top is [`rest_ramp_top`],
    /// the 95th percentile at which the mesh reaches full red.
    RestHeatmap(f64, f32),
    /// The reach ramp's stops; colours from `reach_map::reach_color`. The
    /// whole ramp, not just the bar, because the ramp is LOG-scaled from the
    /// bar to the deepest gap and a legend built from the bar alone could not
    /// draw it (P5.2).
    Reach(rs_cam_core::maps::reach_map::ReachRamp),
    /// Colours from `render::sim_render::deviation_colors`.
    Deviation,
    /// Colours from `rs_cam_core::export::ribbon::height_gradient_colors`.
    ByHeight,
    /// Colours from `render::toolpath_render::engagement_color`.
    Engagement,
    /// Colours from `render::toolpath_render::advance_per_tooth_segment_color`.
    AdvancePerTooth,
    /// `tier_count`; colours from `rest_heatmap_mesh::tier_fill_color`.
    TierMap(usize),
}

/// Does the viewport draw at least one toolpath? The same rule as the GPU
/// upload and the pick: `toolpaths_to_draw`.
pub fn any_toolpath_drawn(state: &AppState) -> bool {
    !drawn_toolpaths(state).is_empty()
}

/// The toolpaths the viewport draws, in config order, with their 0-based
/// config index (the palette index). The same rule as the GPU upload and
/// the pick: `toolpaths_to_draw`.
pub fn drawn_toolpaths(state: &AppState) -> Vec<(usize, crate::state::toolpath::ToolpathId)> {
    let drawn = crate::state::viewport::toolpaths_to_draw(
        crate::state::viewport::ToolpathDrawFilter::from_state(state),
        state.session.toolpath_configs().iter().map(|tc| {
            let rt = state.gui.toolpath_rt.get(&tc.id);
            (
                tc.id,
                rt.is_none_or(|r| r.visible),
                rt.is_some_and(|r| r.result.is_some()),
            )
        }),
    );
    state
        .session
        .toolpath_configs()
        .iter()
        .enumerate()
        .filter(|(_, tc)| drawn.contains(&tc.id))
        .map(|(index, tc)| (index, tc.id))
        .collect()
}

/// Does the viewport draw the simulated stock now? The same gate as
/// `ViewportCallback::show_sim_mesh` in `app/viewport.rs`.
pub fn sim_stock_drawn(state: &AppState) -> bool {
    state.viewport.show_sim_stock
        && state.workspace == Workspace::Simulation
        && state.simulation.has_results()
}

/// The legends to draw, given what is ACTUALLY on screen.
///
/// A row contributes when its flag is on, its precondition is `Ready`, and
/// the renderer draws its surface now. The flag alone is not enough: a stock
/// colour legend beside no simulated stock, or a move colour legend beside
/// no drawn move, names a colour that the operator cannot see.
pub fn active_legends(state: &AppState) -> Vec<Legend> {
    let mut out = Vec::new();
    let on = |id: &str| -> bool {
        row(id).is_some_and(|r| (r.get)(state) && (r.precondition)(state).is_ready())
    };
    let moves_drawn = state.viewport.show_cutting && any_toolpath_drawn(state);
    if on("rest_heatmap")
        && let Some((threshold, top)) = rest_grid_info(state)
    {
        out.push(Legend::RestHeatmap(threshold, top));
    }
    // The same identity check as the draw gate: a map held for an earlier
    // selection is never drawn, so it gives no legend.
    if on("reach_map")
        && state.gui.reach_overlay.toolpath == selected_toolpath(state)
        && let Some(map) = state.gui.reach_overlay.ready_map()
    {
        out.push(Legend::Reach(map.ramp()));
    }
    if on("tier_map")
        && let Some(planner) = state.multitool_planner.as_ref()
        && let Some(preview) = planner.ready_preview()
    {
        out.push(Legend::TierMap(preview.map.tier_count));
    }
    if on("stock_colour_deviation") && sim_stock_drawn(state) {
        out.push(Legend::Deviation);
    }
    if on("stock_colour_by_height") && sim_stock_drawn(state) {
        out.push(Legend::ByHeight);
    }
    if on("move_colour_engagement") && moves_drawn {
        out.push(Legend::Engagement);
    }
    if on("move_colour_advance_per_tooth") && moves_drawn {
        out.push(Legend::AdvancePerTooth);
    }
    out
}
