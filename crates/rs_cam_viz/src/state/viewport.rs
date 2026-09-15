use super::toolpath::ToolpathId;
use std::collections::HashMap;

/// Per-SpanKind visibility toggles for the 3D toolpath renderer. A `false`
/// flag hides cut segments whose innermost span kind matches; rapids and
/// segments with no matching kind are unaffected.
///
/// The filter is applied at GPU upload time and only in the `Palette` colour
/// mode — the Engagement and Advance-per-tooth builders take no filter — so
/// the Overlays panel disables these four rows outside Palette and says why
/// (P6, audit §3.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpanKindFilter {
    pub show_entry: bool,
    pub show_lead_out: bool,
    pub show_link_bridge: bool,
    pub show_dressup: bool,
}

impl Default for SpanKindFilter {
    fn default() -> Self {
        Self {
            show_entry: true,
            show_lead_out: true,
            show_link_bridge: true,
            show_dressup: true,
        }
    }
}

impl SpanKindFilter {
    /// True iff every kind is visible — lets the renderer skip the per-move
    /// classify cost in the common case.
    pub fn all_visible(&self) -> bool {
        self.show_entry && self.show_lead_out && self.show_link_bridge && self.show_dressup
    }
}

/// Per-toolpath move-type visibility. Defaults to both-visible.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolpathMoveVisibility {
    pub show_cutting: bool,
    pub show_rapids: bool,
}

impl Default for ToolpathMoveVisibility {
    fn default() -> Self {
        Self {
            show_cutting: true,
            show_rapids: true,
        }
    }
}

/// Viewport rendering state.
///
/// Every `show_*: bool` here is one row of the Overlays registry
/// (`crate::ui::overlays::registry`), and the completeness sentry
/// `overlays_registry.rs` fails if a field is added without a row. Add the
/// row in the same commit as the field.
pub struct ViewportState {
    pub show_grid: bool,
    /// The imported model surface — the plain (STL) draw loop AND the
    /// enriched (STEP) one.
    ///
    /// This replaces `RenderMode::Wireframe`, which drew nothing: the crate
    /// has no wireframe pipeline, so "Wireframe" hid an STL model and left a
    /// STEP model untouched (audit §3.1, fix §6.2). One flag, both loops,
    /// one honest meaning.
    pub show_model: bool,
    /// The stock wireframe box.
    pub show_stock: bool,
    /// The opaque stock block. Split from `show_stock` (audit §3.2 — one
    /// flag drove three consumers with two different preconditions).
    pub show_stock_solid: bool,
    /// The XYZ axis triad at the stock origin. Split from `show_stock`.
    pub show_origin_axes: bool,
    /// The magenta datum crosshair — the XY zero the export writes.
    /// Split out of the five-way `show_fixtures` coupling (audit §3.3).
    pub show_datum: bool,
    /// Fixture clearance boxes only. The four overlays that used to ride
    /// this flag now have their own.
    pub show_fixtures: bool,
    pub show_keep_outs: bool,
    pub show_alignment_pins: bool,
    pub show_flip_axis: bool,
    pub show_polygons: bool,
    /// The camera-axis triad painted in the viewport corner by egui.
    pub show_orientation_gizmo: bool,
    pub show_cutting: bool,
    pub show_rapids: bool,
    /// Cyan markers where each entry / ramp / helix starts. Before P6 these
    /// drew whenever a toolpath was selected, ignoring `show_cutting`, the
    /// per-toolpath entry and the scrub move limit (audit §6.10).
    pub show_entry_markers: bool,
    /// The five Z planes: clearance, retract, feed, top, bottom.
    pub show_height_planes: bool,
    pub show_collisions: bool,
    /// Render a ghost of the cutter silhouette stacked along the selected
    /// toolpath — visualizes swept material before running a simulation.
    ///
    /// Consumed at GPU-upload time, so it needs an upload trigger; the
    /// composite `overlay_upload_key` detector in `app.rs` is that trigger.
    /// Without one the checkbox did nothing until an unrelated event
    /// happened to fire an upload (audit §3.6, fix §6.1).
    pub show_tool_profile_preview: bool,
    /// Rest-depth heatmap overlay. **Any** operation attaches a `rest_grid`
    /// when its `rest_analysis.enabled` is set and a mesh plus spatial index
    /// are present (`compute::execute::attach_generic_rest_analysis`); the
    /// pencil `RestDepth` arm and the UnifiedFinish claims pipeline are two
    /// further writers. The older "only the pencil detector" claim was
    /// stale (audit §2c, fix §6.4/6.5).
    ///
    /// Defaults **off** since P6. It shares the model surface with
    /// `show_reach_map`, and the Overlays panel makes that surface exclusive
    /// (UX §6.5), so exactly one of the two may be on. The reach map takes
    /// the default because the operator asked for it as the always-on answer
    /// ("show the reach map when ANY finishing op is selected"); the rest
    /// heatmap is the diagnostic they switch on to inspect rest regions.
    /// Switching it on clears Reach, and the panel says so on both rows.
    pub show_rest_heatmap: bool,
    /// Multi-tool tier-map preview overlay (Phase U). Its own flag, its own
    /// upload key and its own GPU slot alongside the rest heatmap rather than
    /// multiplexed onto it: a selected toolpath's rest grid and a plan
    /// preview are different questions and can be wanted at once.
    ///
    /// Defaults **off**. The planner switches it on when a preview lands and
    /// off again when the operator vetoes it, so a default-on flag would just
    /// be a checkbox the planner keeps overwriting. It is a TERRITORY
    /// overlay, so it stacks with whatever colours the model surface.
    pub show_tier_preview: bool,
    /// Per-tool reach-map overlay (P5). The model is drawn in the reach
    /// colours of the SELECTED toolpath's cutter: green where the cutter can
    /// form the surface to the operation's tolerance, red where it cannot.
    ///
    /// Defaults **on in the Toolpaths workspace**, which is where it draws:
    /// the operator's ask for P5 was "show the reach map when ANY finishing
    /// op is selected". It is self-gating, so on every selection that carries
    /// no reach question the derived
    /// `ViewportCallback::show_reach_overlay` gate is false and the plain
    /// model draws.
    ///
    /// It shares the model surface with `show_rest_heatmap`, and the Overlays
    /// panel makes that surface exclusive (UX §6.5), so switching this on
    /// clears the rest heatmap and the reverse. The rest heatmap is the side
    /// that defaults off.
    pub show_reach_map: bool,
    /// The dexel stock as cut. Before P6 no control could hide it — the
    /// derived gate read the workspace and `has_results()` only, while a
    /// comment claimed `show_stock` covered it (audit §2b, fix §6.8).
    pub show_sim_stock: bool,
    /// The tool-deflection panel egui paints over the viewport. Before P6
    /// it had no switch of any kind.
    pub show_tool_deflection: bool,
    /// Draw every generated toolpath of the active setup, rather than the
    /// selected one alone (WP27, plan §32).
    ///
    /// Defaults **off**. The operator ruled that a project with many
    /// toolpaths must not pay for all of them on every frame: the renderer
    /// issues a pipeline, a bind group, a vertex buffer and a draw per
    /// resident toolpath per frame, and the upload pass holds a GPU buffer
    /// for each one. The selected toolpath is what the operator reads, so it
    /// is what the viewport draws.
    ///
    /// Consumed at GPU-upload time, so it needs an upload trigger; the
    /// composite `overlay_upload_key` detector in `app.rs` is that trigger,
    /// and it carries the derived selection beside this flag.
    ///
    /// The Simulation workspace names `Some(true)`, because playback reviews
    /// every toolpath in the program. The Toolpaths workspace names NO
    /// default, so an operator override survives a round trip.
    pub show_all_toolpaths: bool,
    /// Color mode for toolpath lines.
    pub toolpath_color_mode: ToolpathColorMode,
    /// Per-toolpath move-type visibility overlay — AND'd with the global
    /// `show_cutting` / `show_rapids` flags. Missing entries default to visible.
    ///
    /// Per-object, so it stays on the operation row (eye / C / R)
    /// and is deliberately NOT an Overlays registry row: the panel is
    /// per-scene (UX §6.7).
    pub toolpath_move_visibility: HashMap<ToolpathId, ToolpathMoveVisibility>,
    /// SpanKind visibility filter for the 3D renderer. When a kind is hidden,
    /// cut segments with that innermost span kind are dropped at upload time.
    pub span_kind_filter: SpanKindFilter,
}

/// How toolpath cutting moves are colored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToolpathColorMode {
    /// Standard palette color with Z-depth blend (per toolpath).
    #[default]
    Normal,
    /// Color by feed rate: green = nominal, yellow = reduced, red = heavily loaded.
    Engagement,
    /// Color each segment by the **achieved advance per tooth**
    /// (`effective_feed / (rpm × flutes)` — the quantity the chipload
    /// gate observes) against the matched vendor row's band. Blue =
    /// under-engaged, green = within band, orange/red = approaching or
    /// above the band ceiling, grey = no band or no sample.
    ///
    /// Named `Chipload` until 2026-08-08, when it also *measured*
    /// something else: an arc-mean chip thickness compared to an
    /// advance-per-tooth band (F-HEATMAP). The variant is GUI-session
    /// state only — it is not serialised into any project file — so the
    /// rename carries no wire compatibility.
    AdvancePerTooth,
}

impl ViewportState {
    /// The Toolpaths column of the per-workspace default table (UX §6.4).
    ///
    /// The app opens in `Workspace::Toolpaths`, so the constructed state must
    /// equal that column — otherwise the first frame contradicts the table
    /// and the workspace-default sentry passes vacuously.
    pub fn new() -> Self {
        Self {
            show_grid: true,
            show_model: true,
            show_stock: true,
            show_stock_solid: false,
            show_origin_axes: true,
            show_datum: true,
            show_fixtures: true,
            show_keep_outs: true,
            show_alignment_pins: true,
            show_flip_axis: true,
            show_polygons: true,
            show_orientation_gizmo: true,
            show_cutting: true,
            show_rapids: true,
            show_entry_markers: true,
            show_height_planes: true,
            show_collisions: false,
            show_tool_profile_preview: false,
            show_rest_heatmap: false,
            show_tier_preview: false,
            show_reach_map: true,
            show_sim_stock: false,
            show_tool_deflection: false,
            show_all_toolpaths: false,
            toolpath_color_mode: ToolpathColorMode::Normal,
            toolpath_move_visibility: HashMap::new(),
            span_kind_filter: SpanKindFilter::default(),
        }
    }
}

impl Default for ViewportState {
    fn default() -> Self {
        Self::new()
    }
}

/// The two dials that decide which toolpaths the viewport draws (WP27).
///
/// One value, built in one place, so the GPU upload and the click pick cannot
/// answer the question differently. A click that selects geometry the viewport
/// does not draw is the drift a second predicate produces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ToolpathDrawFilter {
    /// The selected toolpath, when the selection names one.
    pub selected: Option<ToolpathId>,
    /// Draw every generated toolpath rather than the selected one alone.
    pub show_all: bool,
}

impl ToolpathDrawFilter {
    /// Read the two dials, each from the one place it lives.
    pub fn from_state(state: &super::AppState) -> Self {
        Self {
            selected: match state.selection {
                super::selection::Selection::Toolpath(id) => Some(id),
                _ => None,
            },
            show_all: state.viewport.show_all_toolpaths,
        }
    }
}

/// Which toolpaths the viewport draws, in the order the caller supplied
/// (WP27, plan §32).
///
/// `rows` is `(id, visible, has_result)` in config order. The caller keeps its
/// own setup filter and its own palette index, so a hidden neighbour never
/// moves a toolpath's colour.
///
/// With nothing selected the answer is empty. That is the operator ruling:
/// the model and the stock still draw, and the operations list is the picker.
pub fn toolpaths_to_draw<I>(filter: ToolpathDrawFilter, rows: I) -> Vec<ToolpathId>
where
    I: IntoIterator<Item = (ToolpathId, bool, bool)>,
{
    rows.into_iter()
        .filter(|&(id, visible, has_result)| {
            visible && has_result && (filter.show_all || filter.selected == Some(id))
        })
        .map(|(id, _, _)| id)
        .collect()
}
