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

/// The place of a row in `More` and in the catalogue (R6, the More menu
/// rulings of 2026-10-30). The compiler holds it: every row literal names
/// one. A group of the Scene section is one object of the scene, so the
/// rows that draw or colour one object stand together under its caption.
///
/// The order of [`DockGroup::ALL`] is the draw order of `More` and the
/// catalogue. Inside a group the rows keep their order in [`ROWS`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockGroup {
    /// The view cube row.
    View,
    /// The model: its switch, its colour and the maps that paint it.
    Model,
    /// The stock: the box, the block, the simulated stock and its colour.
    Stock,
    /// The tool: collisions and the tool deflection. It is the home for
    /// a later tool and holder render.
    Tool,
    /// The obstacles of the setup.
    Obstacles,
    /// The alignment pins, the flip axis and the wrong-orientation ghost.
    Pins,
    /// A single row of the Scene section, with no caption.
    Scene,
    /// The toolpath rows.
    Paths,
}

impl DockGroup {
    /// Every group, in the order `More` and the catalogue draw them.
    pub const ALL: [Self; 8] = [
        Self::View,
        Self::Model,
        Self::Stock,
        Self::Tool,
        Self::Obstacles,
        Self::Pins,
        Self::Scene,
        Self::Paths,
    ];

    /// The dock section of the group.
    #[must_use]
    pub fn section(self) -> DockSection {
        match self {
            Self::View => DockSection::View,
            Self::Model | Self::Stock | Self::Tool | Self::Obstacles | Self::Pins | Self::Scene => {
                DockSection::Scene
            }
            Self::Paths => DockSection::Paths,
        }
    }

    /// The caption of an object group, or `None` for a group that the
    /// section title names (View, Paths) or that has no caption (Scene).
    #[must_use]
    pub fn caption(self) -> Option<&'static str> {
        match self {
            Self::Model => Some("Model"),
            Self::Stock => Some("Stock"),
            Self::Tool => Some("Tool"),
            Self::Obstacles => Some("Obstacles"),
            Self::Pins => Some("Pins"),
            Self::View | Self::Scene | Self::Paths => None,
        }
    }
}

/// The surface a row paints a per-vertex or per-move SCALAR FIELD on.
///
/// Exclusivity is applied per surface and only among scalar fields (UX §6.5).
/// A *territory* overlay — the tier map, a boundary outline — paints a region label over an area, and territories stack by
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

/// The reason of a drawing overlay while the drawing itself is off: the
/// three P4.4 rows draw into the polygon buffer, which only `curves` shows.
const CURVES_OFF: &str = "the drawing is off \u{2014} switch Scene \u{25B8} Curves and edges on";

/// The reason of a toolpath row in the Setup workspace (R6, ruling Q3):
/// Setup draws no toolpath moves. The bar's Cutting moves and Rapids are
/// greyed there with it.
const PATHS_NOT_IN_SETUP: &str = "toolpaths draw in Toolpaths and Simulation";

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
    /// The place of the row in `More` and the catalogue (R6).
    pub group: DockGroup,
    pub label: &'static str,
    pub surface: OverlaySurface,
    pub mechanism: OverlayMechanism,
    /// The state field this row drives, written as a path
    /// (`"viewport.show_grid"`). `None` would mean a row with no renderer;
    /// R6 deleted the last two, and a sentry keeps the set empty. The
    /// completeness sentry maps declared state fields onto this.
    pub flag: Option<&'static str>,
    /// `true` for a member of a per-surface radio group whose "off" has no
    /// meaning (Palette, Solid, and their peers). Switching such a row off
    /// is refused rather than silently ignored.
    pub radio: bool,
    /// `true` for a context row (R6, ruling Q5): a special case that the
    /// context rail beside the legend offers as a chip with an eye while
    /// its precondition holds (`super::context_rail`).
    pub context: bool,
    /// The workspaces in which the row can ever apply (R5c). Out of its
    /// scope the row is refused ([`OverlayRow::precondition`]) and `More`
    /// does not list it; the catalogue lists every row.
    pub scope: RowScope,
    pub get: fn(&AppState) -> bool,
    /// Writes the flag. A `false` on a radio member is a no-op here; the
    /// refusal happens in [`apply_overlays`], which can report it.
    pub set: fn(&mut AppState, bool),
    /// The row's own condition. Read it through
    /// [`OverlayRow::precondition`], which checks the scope too.
    requires: fn(&AppState) -> Precondition,
    /// The per-workspace default (UX §6.4). `None` = this workspace names no
    /// default, so the flag keeps whatever value it has.
    pub default_for: fn(Workspace) -> Option<bool>,
    pub hover: &'static str,
}

/// The workspaces in which a row can ever apply (R5c, the More menu
/// review). A row whose renderer draws in one workspace only is out of
/// scope in the others: a greyed row there can never become drawable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowScope {
    /// Every viewport workspace.
    Any,
    /// This workspace only.
    Only(Workspace),
    /// Every viewport workspace but this one.
    Not(Workspace),
}

impl RowScope {
    /// True when the row can apply in `ws`.
    #[must_use]
    pub fn applies(self, ws: Workspace) -> bool {
        match self {
            Self::Any => true,
            Self::Only(only) => ws == only,
            Self::Not(not) => ws != not,
        }
    }

    /// The refusal of a row out of its scope.
    fn reason(self) -> String {
        match self {
            Self::Any => String::new(),
            Self::Only(only) => format!("draws in the {} workspace only", only.label()),
            Self::Not(not) => format!("does not draw in the {} workspace", not.label()),
        }
    }
}

impl OverlayRow {
    /// THE precondition of the row: its own condition, then its scope. The
    /// row's own reason wins, so a precise reason is kept; a row whose own
    /// condition holds but which cannot draw in this workspace is refused
    /// with the scope reason.
    #[must_use]
    pub fn precondition(&self, state: &AppState) -> Precondition {
        let own = (self.requires)(state);
        if own.is_ready() && !self.scope.applies(state.workspace) {
            return Precondition::no(&self.scope.reason());
        }
        own
    }

    /// True when `More` lists the row in `ws` (R5c): the row can apply
    /// there.
    #[must_use]
    pub fn listed_in(&self, ws: Workspace) -> bool {
        self.scope.applies(ws)
    }
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

/// Off in every viewport workspace: a row whose surface shows it at need
/// (the height planes with the Heights tab).
fn always_off_default(ws: Workspace) -> Option<bool> {
    match ws {
        Workspace::Setup | Workspace::Toolpaths | Workspace::Simulation => Some(false),
        Workspace::Readiness => None,
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
    state.session().models().iter().any(|m| m.mesh.is_some())
}

#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "projection: only the named variant(s) carry this (named: Selection::Toolpath)"
)]
fn selected_toolpath(state: &AppState) -> Option<crate::state::toolpath::ToolpathId> {
    match *state.focus() {
        crate::state::selection::Selection::Toolpath(id) => Some(id),
        _ => None,
    }
}

/// Does at least one toolpath hold a generated result?
pub(crate) fn any_generated(state: &AppState) -> bool {
    state
        .gui
        .toolpath_rt
        .values()
        .any(|rt| rt.run.result().is_some())
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
        .and_then(|rt| rt.run.result())
        .and_then(|r| r.annotated.produced.rest_grid.as_ref())
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
        .and_then(|rt| rt.run.result())
        .and_then(|r| r.annotated.produced.area_regions.as_ref())
}

/// Does the viewport draw the By Area regions this frame? The flag is on
/// and the row precondition is `Ready`. The callback gate, the order labels
/// and the legend read this one function.
pub fn area_regions_drawn(state: &AppState) -> bool {
    state.viewport.show_area_regions && area_regions_precondition(state).is_ready()
}

/// Can the By Area regions overlay draw, and if not, why not?
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "projection: only the named variant(s) carry this (named: OperationConfig::Adaptive3d)"
)]
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
        return Precondition::no("select a 3D Adaptive with By Area ordering");
    };
    let ordering = state
        .session()
        .find_toolpath_config_by_id(tp_id)
        .and_then(|(_, tc)| match &tc.operation {
            OperationConfig::Adaptive3d(cfg) => Some(cfg.region_ordering),
            _ => None,
        });
    match ordering {
        None => Precondition::no("this operation is not a 3D Adaptive"),
        Some(RegionOrdering::Global) => {
            Precondition::no("this 3D Adaptive uses Global ordering \u{2014} it detects no regions")
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
        ReachStatus::Idle | ReachStatus::Computing | ReachStatus::Ready(_) => None,
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
    state.session().list_setups().get(index)
}

fn in_simulation(state: &AppState) -> bool {
    state.workspace == Workspace::Simulation
}

// ── the model look (FOLLOW_UPS §1) ─────────────────────────────────────────

/// Why the model look cannot change now, or `None`. The rules of the old
/// `model_edges` row: the Readiness workspace renders no viewport, the
/// simulated stock replaces the model in Simulation, the look needs the
/// model on and a 3D model. The dock control prints the reason; the MCP
/// reply carries it.
#[must_use]
pub fn display_mode_refusal(state: &AppState) -> Option<&'static str> {
    if state.workspace == Workspace::Readiness {
        Some("the Readiness workspace renders no viewport")
    } else if in_simulation(state) {
        Some("replaced by the simulated stock here")
    } else if !state.viewport.show_model {
        Some("the model is off")
    } else if !has_model_mesh(state) {
        Some("this project carries no 3D model")
    } else {
        None
    }
}

/// THE write of the model look: the dock control and `set_ui_view
/// display_mode` both call it. `Err` is the refusal; the look then keeps
/// its value.
pub fn set_display_mode(
    state: &mut AppState,
    mode: rs_cam_core::settings::DisplayMode,
) -> Result<(), &'static str> {
    if let Some(reason) = display_mode_refusal(state) {
        return Err(reason);
    }
    state.viewport.display_mode = mode;
    Ok(())
}

// ── the stock look (DOCK_DECLUTTER, answer 2) ──────────────────────────────

/// The stock look the two stock rows give: `Solid` when the solid block is
/// on, else `Box` when the box is on, else `Off`.
#[must_use]
pub fn stock_display(state: &AppState) -> crate::state::overlays::StockDisplay {
    use crate::state::overlays::StockDisplay;
    if state.viewport.show_stock_solid {
        StockDisplay::Solid
    } else if state.viewport.show_stock {
        StockDisplay::Box
    } else {
        StockDisplay::Off
    }
}

/// The row a stock look switches on, or `None` for `Off`.
fn stock_display_row(mode: crate::state::overlays::StockDisplay) -> Option<&'static OverlayRow> {
    use crate::state::overlays::StockDisplay;
    match mode {
        StockDisplay::Off => None,
        StockDisplay::Box => row("stock_box"),
        StockDisplay::Solid => row("stock_solid"),
    }
}

/// Why `mode` cannot apply now, or `None`: the precondition of the row it
/// switches on (`Off` always applies, as an "off" of a row does).
#[must_use]
pub fn stock_display_refusal(
    state: &AppState,
    mode: crate::state::overlays::StockDisplay,
) -> Option<String> {
    let target = stock_display_row(mode)?;
    target.precondition(state).reason().map(str::to_owned)
}

/// THE write of the dock's Stock control: one of `Off`, `Box`, `Solid`,
/// through the two rows' own setter, so the refusals and the workspace
/// defaults stay the rows'. `Err` is the refusal; nothing changes then.
pub fn set_stock_display(
    state: &mut AppState,
    mode: crate::state::overlays::StockDisplay,
) -> Result<(), String> {
    if let Some(reason) = stock_display_refusal(state, mode) {
        return Err(reason);
    }
    let target = stock_display_row(mode).map(|r| r.id);
    for id in ["stock_box", "stock_solid"] {
        if let Some(r) = row(id) {
            set_overlay(state, r, Some(id) == target);
        }
    }
    Ok(())
}

// ── the list ───────────────────────────────────────────────────────────────

/// Every overlay, in panel order.
pub const ROWS: &[OverlayRow] = &[
    // ── Geometry ──────────────────────────────────────────────────────────
    OverlayRow {
        id: "grid",
        group: DockGroup::Scene,
        label: "Grid",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::DrawTime,
        flag: Some("viewport.show_grid"),
        radio: false,
        context: false,
        scope: RowScope::Any,
        get: |s| s.viewport.show_grid,
        set: |s, on| s.viewport.show_grid = on,
        requires: always_ready,
        default_for: geometry_default,
        hover: "The ground grid.",
    },
    OverlayRow {
        id: "model",
        group: DockGroup::Model,
        label: "Model",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::DrawTime,
        flag: Some("viewport.show_model"),
        radio: false,
        context: false,
        scope: RowScope::Not(Workspace::Simulation),
        get: |s| s.viewport.show_model,
        set: |s, on| s.viewport.show_model = on,
        requires: |s| {
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
        hover: "The imported STL or STEP surface.",
    },
    OverlayRow {
        id: "stock_box",
        group: DockGroup::Stock,
        label: "Stock \u{2014} box",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::DrawTime,
        flag: Some("viewport.show_stock"),
        radio: false,
        context: false,
        scope: RowScope::Any,
        get: |s| s.viewport.show_stock,
        set: |s, on| s.viewport.show_stock = on,
        requires: always_ready,
        default_for: geometry_default,
        hover: "The stock as a wireframe box, from the stock settings. It \
                draws on a 2D job too.",
    },
    OverlayRow {
        id: "stock_solid",
        group: DockGroup::Stock,
        label: "Stock \u{2014} solid",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::DrawTime,
        flag: Some("viewport.show_stock_solid"),
        radio: false,
        context: false,
        scope: RowScope::Any,
        get: |s| s.viewport.show_stock_solid,
        set: |s, on| s.viewport.show_stock_solid = on,
        requires: always_ready,
        default_for: setup_only_default,
        hover: "The stock as an opaque block.",
    },
    OverlayRow {
        id: "origin_axes",
        group: DockGroup::Scene,
        label: "Origin axes",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::DrawTime,
        flag: Some("viewport.show_origin_axes"),
        radio: false,
        context: false,
        scope: RowScope::Any,
        get: |s| s.viewport.show_origin_axes,
        set: |s, on| s.viewport.show_origin_axes = on,
        requires: always_ready,
        default_for: geometry_default,
        hover: "The XYZ triad at the stock origin. Distinct from the datum \
                crosshair, which is where the G-code zeroes.",
    },
    OverlayRow {
        id: "datum",
        group: DockGroup::Scene,
        label: "Datum crosshair",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::UploadTime(UPLOAD_DETECTOR),
        flag: Some("viewport.show_datum"),
        radio: false,
        context: false,
        scope: RowScope::Any,
        get: |s| s.viewport.show_datum,
        set: |s, on| s.viewport.show_datum = on,
        requires: |s| {
            use rs_cam_core::session::XYDatum;
            match active_setup(s) {
                None => Precondition::no("no setup in this project"),
                Some(sd) => match sd.datum.xy_method {
                    XYDatum::CornerProbe(_) | XYDatum::CenterOfStock => Precondition::Ready,
                    XYDatum::AlignmentPins | XYDatum::Manual => {
                        Precondition::no("no datum set on this setup")
                    }
                },
            }
        },
        default_for: geometry_default,
        hover: "The XY zero the export writes (magenta crosshair).",
    },
    OverlayRow {
        id: "obstacles",
        group: DockGroup::Obstacles,
        label: "Obstacles",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::UploadTime(UPLOAD_DETECTOR),
        flag: Some("viewport.show_obstacles"),
        radio: false,
        context: false,
        scope: RowScope::Any,
        get: |s| s.viewport.show_obstacles,
        set: |s, on| s.viewport.show_obstacles = on,
        requires: |s| match active_setup(s) {
            Some(sd) if sd.obstacles.iter().any(|o| o.enabled) => Precondition::Ready,
            Some(_) => Precondition::no("no obstacle in this setup"),
            None => Precondition::no("no setup in this project"),
        },
        default_for: geometry_default,
        // K2: one row for one concept (K1 ruling). The old "fixtures" and
        // "keep_outs" ids are gone; a saved preference under them is
        // ignored (no legacy, operator ruling 2026-09-16).
        hover: "Obstacles of this setup: fixtures and keep-out zones (one \
                concept). The solid line is the obstacle, the dim line is its \
                clearance, and the flat rectangle is the obstacle plane (it \
                shows while the setup or one of its obstacles is selected).",
    },
    OverlayRow {
        id: "alignment_pins",
        group: DockGroup::Pins,
        label: "Alignment pins",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::UploadTime(UPLOAD_DETECTOR),
        flag: Some("viewport.show_alignment_pins"),
        radio: false,
        context: false,
        scope: RowScope::Any,
        get: |s| s.viewport.show_alignment_pins,
        set: |s, on| s.viewport.show_alignment_pins = on,
        requires: |s| {
            if s.session().stock_config().alignment_pins.is_empty() {
                Precondition::no("no alignment pin on this stock")
            } else {
                Precondition::Ready
            }
        },
        default_for: always_on_default,
        hover: "A numbered circle at each pin centre. Green: every check passes. \
                Amber: a wall is thinner than 2 mm. Red: the pin does not seat \
                after the flip, or the set does not key. Edit the pins in the \
                Stock panel; a click on a pin marks its row there.",
    },
    OverlayRow {
        id: "flip_axis",
        group: DockGroup::Pins,
        label: "Flip axis",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::UploadTime(UPLOAD_DETECTOR),
        flag: Some("viewport.show_flip_axis"),
        radio: false,
        context: false,
        scope: RowScope::Any,
        get: |s| s.viewport.show_flip_axis,
        set: |s, on| s.viewport.show_flip_axis = on,
        // P3: the axis comes from the setups through the one core
        // selector, the same call the viewport draws from.
        requires: |s| {
            let axis = s
                .session()
                .stock_config()
                .pin_flip_map(
                    s.session()
                        .list_setups()
                        .iter()
                        .map(|sd| (sd.face_up, sd.z_rotation)),
                )
                .and_then(|map| map.axis_segment());
            if axis.is_some() {
                Precondition::Ready
            } else {
                Precondition::no("no flipped Bottom setup in this project")
            }
        },
        default_for: geometry_default,
        hover: "The dashed centreline a two-sided job flips about, from the flipped setup.",
    },
    OverlayRow {
        id: "pin_ghost",
        group: DockGroup::Pins,
        label: "Wrong-orientation ghost",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::UploadTime(UPLOAD_DETECTOR),
        flag: Some("viewport.show_pin_ghost"),
        radio: false,
        context: true,
        scope: RowScope::Any,
        get: |s| s.viewport.show_pin_ghost,
        set: |s, on| s.viewport.show_pin_ghost = on,
        // P7: the ghost shows why a set does not key. A keyed set has no
        // seating to show, so the row is refused with that reason; the
        // flag stays on, and the ghost returns when an edit un-keys the set.
        requires: |s| {
            if s.session().stock_config().alignment_pins.is_empty() {
                return Precondition::no("no alignment pin on this stock");
            }
            if s.session().pin_set_report().flip.keyed() {
                Precondition::no("the pins key: no turn of the part seats it on the dowels")
            } else {
                Precondition::Ready
            }
        },
        default_for: geometry_default,
        hover: "The pin holes at the nearest wrong seating, drawn when the pins \
                do not key the part's orientation.",
    },
    OverlayRow {
        id: "curves",
        group: DockGroup::Scene,
        label: "Curves and edges",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::DrawTime,
        flag: Some("viewport.show_polygons"),
        radio: false,
        context: false,
        scope: RowScope::Any,
        get: |s| s.viewport.show_polygons,
        set: |s, on| s.viewport.show_polygons = on,
        requires: |s| {
            // The draw gate: `show_polygons` gates the 2D curves and, on a
            // STEP model, the edge item lines. The highlight and the hover
            // ring draw without it.
            if s.session()
                .models()
                .iter()
                .any(|m| m.items.has_drawing() || m.items.has_model_edges())
            {
                Precondition::Ready
            } else {
                Precondition::no("this project carries no 2D curves and no STEP edges")
            }
        },
        default_for: no_default,
        hover: "The imported 2D curves, and the edge lines of a STEP model. \
                The selection highlight and the hover ring show without it.",
    },
    OverlayRow {
        id: "selection_outline",
        group: DockGroup::Scene,
        label: "Selection outline",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::DrawTime,
        flag: Some("viewport.show_selection_outline"),
        radio: false,
        context: false,
        scope: RowScope::Only(Workspace::Toolpaths),
        get: |s| s.viewport.show_selection_outline,
        set: |s, on| s.viewport.show_selection_outline = on,
        requires: |s| {
            if in_simulation(s) {
                Precondition::no("replaced by the simulated stock here")
            } else if !s.viewport.show_model {
                Precondition::no("the model is off")
            } else if !has_model_mesh(s) {
                Precondition::no("this project carries no 3D model")
            } else if s.viewport.display_mode == rs_cam_core::settings::DisplayMode::Wireframe {
                Precondition::no("the wireframe draws no faces")
            } else {
                Precondition::Ready
            }
        },
        default_for: toolpaths_only_default,
        hover: "An ink line on the silhouette and the folds of each model, and \
                an accent line just outside each selected face.",
    },
    OverlayRow {
        id: "selection_states",
        group: DockGroup::Scene,
        label: "Drawing states",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::UploadTime(UPLOAD_DETECTOR),
        flag: Some("viewport.show_selection_states"),
        radio: false,
        context: false,
        scope: RowScope::Any,
        get: |s| s.viewport.show_selection_states,
        set: |s, on| s.viewport.show_selection_states = on,
        requires: |s| {
            if !s.viewport.show_polygons {
                return Precondition::no(CURVES_OFF);
            }
            match crate::state::selection_highlight::highlight_refusal(s) {
                Some(reason) => Precondition::no(reason),
                None => Precondition::Ready,
            }
        },
        default_for: toolpaths_only_default,
        hover: "Colours the drawing items by their selection state: green is \
                cut, blue is an island (cut round), amber is skipped (the operation \
                cannot cut its kind), dark grey is removed by hand, grey is not \
                selected. A red ring or cross marks a pick that names no item.",
    },
    OverlayRow {
        id: "open_path_ends",
        group: DockGroup::Scene,
        label: "Open path ends",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::UploadTime(UPLOAD_DETECTOR),
        flag: Some("viewport.show_open_path_ends"),
        radio: false,
        context: false,
        scope: RowScope::Any,
        get: |s| s.viewport.show_open_path_ends,
        set: |s, on| s.viewport.show_open_path_ends = on,
        requires: |s| {
            if !s.viewport.show_polygons {
                return Precondition::no(CURVES_OFF);
            }
            let has_path = crate::state::selection_highlight::highlight_toolpath(s)
                .and_then(|tc| s.session().models().iter().find(|m| m.id == tc.model_id))
                .is_some_and(|m| {
                    m.items
                        .iter()
                        .any(|i| i.kind() == rs_cam_core::selection::ItemKind::Path)
                });
            if has_path {
                Precondition::Ready
            } else {
                Precondition::no("the drawing of the selected toolpath has no open path")
            }
        },
        default_for: always_on_default,
        hover: "A green triangle at the start of each open path, pointing \
                along it, and a red square at its end.",
    },
    OverlayRow {
        id: "orientation_gizmo",
        group: DockGroup::View,
        label: "View cube",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::DrawTime,
        flag: Some("viewport.show_orientation_gizmo"),
        radio: false,
        context: false,
        scope: RowScope::Any,
        get: |s| s.viewport.show_orientation_gizmo,
        set: |s, on| s.viewport.show_orientation_gizmo = on,
        requires: always_ready,
        default_for: no_default,
        hover: "The view cube in the viewport corner, with the projection toggle \
                under it. Click a face for that view; drag the cube to orbit.",
    },
    // ── Toolpath ──────────────────────────────────────────────────────────
    OverlayRow {
        id: "all_toolpaths",
        group: DockGroup::Paths,
        label: "All toolpaths",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::UploadTime(UPLOAD_DETECTOR),
        flag: Some("viewport.show_all_toolpaths"),
        radio: false,
        context: false,
        scope: RowScope::Not(Workspace::Setup),
        get: |s| s.viewport.show_all_toolpaths,
        set: |s, on| s.viewport.show_all_toolpaths = on,
        requires: |s| {
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
        group: DockGroup::Paths,
        label: "Cutting moves",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::DrawTime,
        flag: Some("viewport.show_cutting"),
        radio: false,
        context: false,
        scope: RowScope::Not(Workspace::Setup),
        get: |s| s.viewport.show_cutting,
        set: |s, on| s.viewport.show_cutting = on,
        requires: |s| {
            if s.workspace == Workspace::Setup {
                Precondition::no(PATHS_NOT_IN_SETUP)
            } else if any_generated(s) {
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
        group: DockGroup::Paths,
        label: "Rapids",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::DrawTime,
        flag: Some("viewport.show_rapids"),
        radio: false,
        context: false,
        scope: RowScope::Not(Workspace::Setup),
        get: |s| s.viewport.show_rapids,
        set: |s, on| s.viewport.show_rapids = on,
        requires: |s| {
            if s.workspace == Workspace::Setup {
                Precondition::no(PATHS_NOT_IN_SETUP)
            } else if any_generated(s) {
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
        group: DockGroup::Paths,
        label: "Entry markers",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::DrawTime,
        flag: Some("viewport.show_entry_markers"),
        radio: false,
        context: false,
        scope: RowScope::Not(Workspace::Setup),
        get: |s| s.viewport.show_entry_markers,
        set: |s, on| s.viewport.show_entry_markers = on,
        requires: |s| {
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
        group: DockGroup::Paths,
        label: "Height planes",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::DrawTime,
        flag: Some("viewport.show_height_planes"),
        radio: false,
        context: false,
        scope: RowScope::Not(Workspace::Setup),
        get: |s| s.viewport.show_height_planes,
        set: |s, on| s.viewport.show_height_planes = on,
        requires: |s| {
            if selected_toolpath(s).is_some() {
                Precondition::Ready
            } else {
                Precondition::no("select a toolpath to see its Z planes")
            }
        },
        // Live look round 3: off in every workspace. The planes draw
        // while the Heights tab is open (`viewport::height_planes_drawn`);
        // the row pins them. `get` and the MCP echo report the stored flag
        // only, never the tab.
        default_for: always_off_default,
        hover: "Clearance, retract, feed, top and bottom Z. Shown while the Heights tab is \
                open; switch on to keep them.",
    },
    OverlayRow {
        id: "tool_profile_ghost",
        group: DockGroup::Paths,
        label: "Tool-profile ghost",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::UploadTime(UPLOAD_DETECTOR),
        flag: Some("viewport.show_tool_profile_preview"),
        radio: false,
        context: false,
        scope: RowScope::Not(Workspace::Setup),
        get: |s| s.viewport.show_tool_profile_preview,
        set: |s, on| s.viewport.show_tool_profile_preview = on,
        requires: |s| {
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
        group: DockGroup::Paths,
        label: "Spans: Entry",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::UploadTime(UPLOAD_DETECTOR),
        flag: Some("viewport.span_kind_filter.show_entry"),
        radio: false,
        context: false,
        scope: RowScope::Not(Workspace::Setup),
        get: |s| s.viewport.span_kind_filter.show_entry,
        set: |s, on| s.viewport.span_kind_filter.show_entry = on,
        requires: span_filter_precondition,
        default_for: no_default,
        hover: "Plunge / ramp / helix lead-in segments.",
    },
    OverlayRow {
        id: "span_lead_out",
        group: DockGroup::Paths,
        label: "Spans: Lead-out",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::UploadTime(UPLOAD_DETECTOR),
        flag: Some("viewport.span_kind_filter.show_lead_out"),
        radio: false,
        context: false,
        scope: RowScope::Not(Workspace::Setup),
        get: |s| s.viewport.span_kind_filter.show_lead_out,
        set: |s, on| s.viewport.span_kind_filter.show_lead_out = on,
        requires: span_filter_precondition,
        default_for: no_default,
        hover: "Lead-out / retract transition segments.",
    },
    OverlayRow {
        id: "span_link_bridge",
        group: DockGroup::Paths,
        label: "Spans: Link bridge",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::UploadTime(UPLOAD_DETECTOR),
        flag: Some("viewport.span_kind_filter.show_link_bridge"),
        radio: false,
        context: false,
        scope: RowScope::Not(Workspace::Setup),
        get: |s| s.viewport.span_kind_filter.show_link_bridge,
        set: |s, on| s.viewport.span_kind_filter.show_link_bridge = on,
        requires: span_filter_precondition,
        default_for: no_default,
        hover: "Linker bridges inserted between regions.",
    },
    OverlayRow {
        id: "span_dressup",
        group: DockGroup::Paths,
        label: "Spans: Dressup",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::UploadTime(UPLOAD_DETECTOR),
        flag: Some("viewport.span_kind_filter.show_dressup"),
        radio: false,
        context: false,
        scope: RowScope::Not(Workspace::Setup),
        get: |s| s.viewport.span_kind_filter.show_dressup,
        set: |s, on| s.viewport.span_kind_filter.show_dressup = on,
        requires: span_filter_precondition,
        default_for: no_default,
        hover: "Dogbone overcuts and other dressup-introduced bridge segments. \
                Arc-fit replacements are ordinary cutting geometry and are not \
                filtered here.",
    },
    // ── Regions ───────────────────────────────────────────────────────────
    OverlayRow {
        id: "rest_heatmap",
        group: DockGroup::Model,
        label: "Model colour: Rest",
        surface: OverlaySurface::Model,
        mechanism: OverlayMechanism::DrawTime,
        flag: Some("viewport.show_rest_heatmap"),
        radio: false,
        context: true,
        scope: RowScope::Only(Workspace::Toolpaths),
        get: |s| s.viewport.show_rest_heatmap,
        set: |s, on| s.viewport.show_rest_heatmap = on,
        requires: |s| {
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
                default: switching it on clears Scene \u{25B8} Model \u{25B8} \
                Reach, which shares the model surface.",
    },
    OverlayRow {
        id: "tier_map",
        group: DockGroup::Model,
        label: "Tier map",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::DrawTime,
        flag: Some("viewport.show_tier_preview"),
        radio: false,
        context: true,
        scope: RowScope::Only(Workspace::Toolpaths),
        get: |s| s.viewport.show_tier_preview,
        set: |s, on| s.viewport.show_tier_preview = on,
        requires: |s| {
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
        group: DockGroup::Model,
        label: "By Area regions",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::DrawTime,
        flag: Some("viewport.show_area_regions"),
        radio: false,
        context: true,
        scope: RowScope::Only(Workspace::Toolpaths),
        get: |s| s.viewport.show_area_regions,
        set: |s, on| s.viewport.show_area_regions = on,
        requires: area_regions_precondition,
        default_for: no_default,
        hover: "The regions that the selected 3D Adaptive detected with By Area \
                ordering. Each region has one colour and one order number. \
                The thin box is the filter the planner cuts the region by. \
                The planner detects the regions once, before the first level. \
                One colour over the full part means one region: By Area then \
                cuts as Global does.",
    },
    OverlayRow {
        id: "boundary_outline",
        group: DockGroup::Scene,
        label: "Boundary outline",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::UploadTime(UPLOAD_DETECTOR),
        flag: Some("viewport.show_boundary_outline"),
        radio: false,
        context: false,
        scope: RowScope::Any,
        get: |s| s.viewport.show_boundary_outline,
        set: |s, on| s.viewport.show_boundary_outline = on,
        // P4.4: the renderer draws the regions of a `Selection` boundary
        // (its `Cut` and `Island` rings). Another source has no rings
        // before generation, and the row says so.
        requires: |s| {
            if !s.viewport.show_polygons {
                return Precondition::no(CURVES_OFF);
            }
            let Some(tc) = crate::state::selection_highlight::highlight_toolpath(s) else {
                return Precondition::no("select a toolpath");
            };
            if !tc.boundary.enabled {
                return Precondition::no("the machining boundary of this toolpath is off");
            }
            if crate::state::selection_highlight::boundary_report(s).is_some() {
                Precondition::Ready
            } else {
                Precondition::no(
                    "the boundary source is not Selection \u{2014} only a selection \
                     boundary draws here",
                )
            }
        },
        default_for: no_default,
        hover: "The machining boundary of the selected toolpath, when its \
                source is Selection: the regions inside which the tool may go. \
                Authored under Geometry \u{25B8} Machining Boundary.",
    },
    // ── Analysis ──────────────────────────────────────────────────────────
    OverlayRow {
        id: "reach_map",
        group: DockGroup::Model,
        label: "Model colour: Reach",
        surface: OverlaySurface::Model,
        mechanism: OverlayMechanism::DrawTime,
        flag: Some("viewport.show_reach_map"),
        radio: false,
        context: true,
        scope: RowScope::Only(Workspace::Toolpaths),
        get: |s| s.viewport.show_reach_map,
        set: |s, on| s.viewport.show_reach_map = on,
        requires: |s| {
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
                measured. Clears Scene \u{25B8} Model \u{25B8} Rest \u{2014} both colour \
                the model.",
    },
    OverlayRow {
        id: "simulated_stock",
        group: DockGroup::Stock,
        label: "Simulated stock",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::DrawTime,
        flag: Some("viewport.show_sim_stock"),
        radio: false,
        context: false,
        scope: RowScope::Only(Workspace::Simulation),
        get: |s| s.viewport.show_sim_stock,
        set: |s, on| s.viewport.show_sim_stock = on,
        requires: |s| {
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
        id: "stock_colour_solid",
        group: DockGroup::Stock,
        label: "Stock colour: Solid",
        surface: OverlaySurface::Stock,
        mechanism: OverlayMechanism::UploadTime(UPLOAD_DETECTOR),
        flag: Some("simulation.stock_viz_mode.solid"),
        radio: true,
        context: false,
        scope: RowScope::Only(Workspace::Simulation),
        get: |s| matches!(s.simulation.stock_viz_mode, StockVizMode::Solid),
        set: |s, on| {
            if on {
                s.simulation.stock_viz_mode = StockVizMode::Solid;
            }
        },
        requires: stock_colour_precondition,
        default_for: no_default,
        hover: "Default wood-tone gradient. No analysis colouring.",
    },
    OverlayRow {
        id: "stock_colour_deviation",
        group: DockGroup::Stock,
        label: "Stock colour: Deviation",
        surface: OverlaySurface::Stock,
        mechanism: OverlayMechanism::UploadTime(UPLOAD_DETECTOR),
        flag: Some("simulation.stock_viz_mode.deviation"),
        radio: true,
        context: true,
        scope: RowScope::Only(Workspace::Simulation),
        get: |s| matches!(s.simulation.stock_viz_mode, StockVizMode::Deviation),
        set: |s, on| {
            if on {
                s.simulation.stock_viz_mode = StockVizMode::Deviation;
            }
        },
        requires: |s| {
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
        group: DockGroup::Stock,
        label: "Stock colour: By height",
        surface: OverlaySurface::Stock,
        mechanism: OverlayMechanism::UploadTime(UPLOAD_DETECTOR),
        flag: Some("simulation.stock_viz_mode.by_height"),
        radio: true,
        context: false,
        scope: RowScope::Only(Workspace::Simulation),
        get: |s| matches!(s.simulation.stock_viz_mode, StockVizMode::ByHeight),
        set: |s, on| {
            if on {
                s.simulation.stock_viz_mode = StockVizMode::ByHeight;
            }
        },
        requires: stock_colour_precondition,
        default_for: no_default,
        hover: "Colour by Z height: low = blue, high = red.",
    },
    OverlayRow {
        id: "move_colour_palette",
        group: DockGroup::Paths,
        label: "Move colour: Palette",
        surface: OverlaySurface::Moves,
        mechanism: OverlayMechanism::UploadTime(UPLOAD_DETECTOR),
        flag: Some("viewport.toolpath_color_mode.normal"),
        radio: true,
        context: false,
        scope: RowScope::Not(Workspace::Setup),
        get: |s| matches!(s.viewport.toolpath_color_mode, ToolpathColorMode::Normal),
        set: |s, on| {
            if on {
                s.viewport.toolpath_color_mode = ToolpathColorMode::Normal;
            }
        },
        requires: move_colour_precondition,
        default_for: no_default,
        hover: "Per-toolpath palette colour with Z-depth blending. The only \
                mode the span filter applies in.",
    },
    OverlayRow {
        id: "move_colour_engagement",
        group: DockGroup::Paths,
        label: "Move colour: Engagement",
        surface: OverlaySurface::Moves,
        mechanism: OverlayMechanism::UploadTime(UPLOAD_DETECTOR),
        flag: Some("viewport.toolpath_color_mode.engagement"),
        radio: true,
        context: false,
        scope: RowScope::Not(Workspace::Setup),
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
        requires: move_colour_precondition,
        default_for: no_default,
        hover: "Colour cutting moves by feed rate: green \u{2192} yellow \u{2192} red \
                for light \u{2192} heavy load.",
    },
    OverlayRow {
        id: "move_colour_advance_per_tooth",
        group: DockGroup::Paths,
        label: "Move colour: Advance / tooth",
        surface: OverlaySurface::Moves,
        mechanism: OverlayMechanism::UploadTime(UPLOAD_DETECTOR),
        flag: Some("viewport.toolpath_color_mode.advance_per_tooth"),
        radio: true,
        context: false,
        scope: RowScope::Not(Workspace::Setup),
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
        requires: |s| {
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
        id: "collisions",
        group: DockGroup::Tool,
        label: "Collisions",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::DrawTime,
        flag: Some("viewport.show_collisions"),
        radio: false,
        context: false,
        scope: RowScope::Any,
        get: |s| s.viewport.show_collisions,
        set: |s, on| s.viewport.show_collisions = on,
        requires: |s| {
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
        group: DockGroup::Tool,
        label: "Tool deflection",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::DrawTime,
        flag: Some("viewport.show_tool_deflection"),
        radio: false,
        context: true,
        scope: RowScope::Only(Workspace::Simulation),
        get: |s| s.viewport.show_tool_deflection,
        set: |s, on| s.viewport.show_tool_deflection = on,
        requires: |s| {
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
        group: DockGroup::Paths,
        label: "Generator steps",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::DrawTime,
        flag: Some("simulation.debug.enabled"),
        radio: false,
        context: true,
        scope: RowScope::Only(Workspace::Simulation),
        get: |s| s.simulation.debug.enabled,
        set: |s, on| s.simulation.debug.enabled = on,
        requires: |s| {
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
        group: DockGroup::Paths,
        label: "Highlight active step",
        surface: OverlaySurface::None,
        mechanism: OverlayMechanism::DrawTime,
        flag: Some("simulation.debug.highlight_active_item"),
        radio: false,
        context: false,
        scope: RowScope::Only(Workspace::Simulation),
        get: |s| s.simulation.debug.highlight_active_item,
        set: |s, on| s.simulation.debug.highlight_active_item = on,
        requires: |s| {
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

/// The dock section that is the ONE home of `row`: the section of its
/// group (R6). One home per row keeps the rule "one state, one writer".
/// The catalogue lists every row in addition.
pub fn dock_section(row: &OverlayRow) -> DockSection {
    row.group.section()
}

/// The rows whose dock home is `section`, group by group in
/// [`DockGroup::ALL`] order, each group in list order.
pub fn rows_in_section(section: DockSection) -> impl Iterator<Item = &'static OverlayRow> {
    DockGroup::ALL
        .into_iter()
        .filter(move |group| group.section() == section)
        .flat_map(rows_in_group)
}

/// The rows of `group`, in list order.
pub fn rows_in_group(group: DockGroup) -> impl Iterator<Item = &'static OverlayRow> {
    ROWS.iter().filter(move |r| r.group == group)
}

/// The plain choice of a radio surface: its first member in list order
/// (Solid on the stock, Palette on the moves). `None` for a surface with
/// no radio, where "plain" is no row on (the model drawn plain).
#[must_use]
pub fn plain_choice(surface: OverlaySurface) -> Option<&'static OverlayRow> {
    if surface == OverlaySurface::None {
        return None;
    }
    ROWS.iter().find(|r| r.surface == surface && r.radio)
}

/// Switch `row` to its other state, through [`set_overlay`] (R6, the
/// context rail). On goes on. A radio member goes off to the plain choice
/// of its surface; any other row goes off. `More` and the context rail
/// write through this one setter, so the two give one state.
pub fn toggle_overlay(state: &mut AppState, row: &OverlayRow) {
    if !(row.get)(state) {
        set_overlay(state, row, true);
    } else if row.radio {
        if let Some(plain) = plain_choice(row.surface).filter(|p| p.id != row.id) {
            set_overlay(state, plain, true);
        }
    } else {
        set_overlay(state, row, false);
    }
}

/// The context rows whose precondition holds now, in list order: the
/// chips of the context rail (R6, ruling Q5).
pub fn context_rows_ready(state: &AppState) -> Vec<&'static OverlayRow> {
    ROWS.iter()
        .filter(|r| r.context && r.precondition(state).is_ready())
        .collect()
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
/// Every dock, MCP and inspector write goes through [`set_overlay`], which
/// keeps this rule. Since R3 (2026-10-30) the inspector's Reach row asks,
/// and `draw_toolpath_selection` calls [`set_overlay`]; the old `Show reach
/// map` checkbox, the one direct writer, is gone. This function stays as a
/// guard for a state that a load, a fixture or a future direct write puts
/// on screen with two rows on: the dock calls it before it draws, and the
/// frame loop draws the dock before it builds the viewport callback, so no
/// frame draws two colour sources on one surface.
///
/// When two rows are on, the LAST one in [`ROWS`] stays on (on the model
/// surface, Reach). The radio surfaces are enums and can never hold two,
/// so the rule has work on the model surface only.
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
            let pre = row.precondition(state);
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

/// The overlays whose default File ▸ Preferences ▸ Display can set
/// (`[display.overlays]` of the settings file), in the order the window
/// lists them.
///
/// Each is a plain flag: one surface of its own (`OverlaySurface::None`), no
/// radio group, a renderer. A value from the file replaces the row's
/// per-workspace default in every viewport workspace. A scalar-field
/// surface (the colour rows) and the analyses that need a compute are not
/// here: their defaults belong to the workspace table, and a colour has its
/// own preference (`[display] toolpath_colour_mode`).
pub const PREFERENCE_OVERLAYS: &[&str] = &[
    "grid",
    "model",
    "stock_box",
    "origin_axes",
    "datum",
    "obstacles",
    "alignment_pins",
    "flip_axis",
    "pin_ghost",
    "curves",
    "orientation_gizmo",
    "cutting_moves",
    "rapids",
    "entry_markers",
    "height_planes",
    "tool_profile_ghost",
];

/// Whether File ▸ Preferences can set the default of `row`.
#[must_use]
pub fn is_preference_overlay(row: &OverlayRow) -> bool {
    PREFERENCE_OVERLAYS.contains(&row.id)
        && !row.radio
        && row.surface == OverlaySurface::None
        && row.flag.is_some()
}

/// The default of `row` in `workspace`: the settings file's value when it
/// names one (`[display.overlays]`, File ▸ Preferences ▸ Display), else the
/// row's own per-workspace default.
///
/// THE one answer for every reader: [`apply_workspace_defaults`], the
/// "changed" count of the dock ([`non_default_count`]) and the catalogue's
/// Changed filter. Two readers with two answers would make the count lie.
/// A workspace without a viewport (Readiness) names no default, so the
/// file's value does not reach it either.
#[must_use]
pub fn effective_default(state: &AppState, row: &OverlayRow, workspace: Workspace) -> Option<bool> {
    let row_default = (row.default_for)(workspace);
    if workspace == Workspace::Readiness || !is_preference_overlay(row) {
        return row_default;
    }
    state
        .app_settings
        .display
        .overlays
        .get(row.id)
        .copied()
        .or(row_default)
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
    // Round 4: entering Toolpaths with a toolpath focus engages its pick
    // mode; leaving Toolpaths ends it (`state::pick::engage_pick_for_focus`).
    crate::state::pick::engage_pick_for_focus(state);
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
        let Some(default) = effective_default(state, row, target) else {
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
            Some(row) if row.precondition(state).is_ready() => {
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
        .filter(|row| match effective_default(state, row, state.workspace) {
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
    /// bar to one tip radius (round 7, `state::runtime::reach_display_ramp`)
    /// and a legend built from the bar alone could not draw it (P5.2).
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
        state.session().toolpath_configs().iter().map(|tc| {
            let rt = state.gui.toolpath_rt.get(&tc.id);
            (
                tc.id,
                rt.is_none_or(|r| r.visible),
                rt.is_some_and(|r| r.run.result().is_some()),
            )
        }),
    );
    state
        .session()
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
        row(id).is_some_and(|r| (r.get)(state) && r.precondition(state).is_ready())
    };
    let moves_drawn = state.viewport.show_cutting && any_toolpath_drawn(state);
    if on("rest_heatmap")
        && let Some((threshold, top)) = rest_grid_info(state)
    {
        out.push(Legend::RestHeatmap(threshold, top));
    }
    // The same identity check as the draw gate: a map held for an earlier
    // selection is never drawn, so it gives no legend.
    // Round 7: the legend takes the ramp of the drawn colours (the
    // surface), which is tool-relative, not the map's own ramp.
    if on("reach_map")
        && state.gui.reach_overlay.toolpath == selected_toolpath(state)
        && state.gui.reach_overlay.ready_map().is_some()
        && let Some(surface) = state.gui.reach_overlay.surface.as_ref()
    {
        out.push(Legend::Reach(surface.ramp));
    }
    if on("tier_map")
        && let Some(planner) = state.multitool_planner.as_ref()
        && let Some(preview) = planner.ready_preview()
    {
        out.push(Legend::TierMap(preview.map.tier_count));
    }
    // F4 (render review 2026-10-09): the deviation colours are drawn only
    // on the final composite mesh, so the legend reads the same gate.
    if on("stock_colour_deviation")
        && sim_stock_drawn(state)
        && state.simulation.playback.deviations_fit_display()
    {
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
