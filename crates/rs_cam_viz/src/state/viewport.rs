use super::toolpath::ToolpathId;
use std::collections::HashMap;

/// Per-SpanKind visibility toggles for the 3D toolpath renderer. A `false`
/// flag hides cut segments whose innermost span kind matches; rapids and
/// segments with no matching kind are unaffected.
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
pub struct ViewportState {
    pub show_grid: bool,
    pub show_stock: bool,
    pub show_fixtures: bool,
    pub show_polygons: bool,
    pub render_mode: RenderMode,
    pub show_cutting: bool,
    pub show_rapids: bool,
    pub show_collisions: bool,
    /// Render a ghost of the cutter silhouette stacked along the selected
    /// toolpath — visualizes swept material before running a simulation.
    pub show_tool_profile_preview: bool,
    /// Rest-depth heatmap overlay (pencil detector #4). Only ever visible
    /// when the selected toolpath actually carries a `rest_grid`, so
    /// defaulting to `true` doesn't clutter unrelated workspaces — see the
    /// derived gate on `ViewportCallback::show_rest_heatmap`.
    pub show_rest_heatmap: bool,
    /// Multi-tool tier-map preview overlay (Phase U). Its own flag, its own
    /// upload key and its own GPU slot alongside the rest heatmap rather than
    /// multiplexed onto it: a selected toolpath's rest grid and a plan
    /// preview are different questions and can be wanted at once.
    ///
    /// Defaults **off**, unlike `show_rest_heatmap`. The rest overlay is
    /// self-gating (nothing draws unless the selection carries a `rest_grid`);
    /// this one is switched on by the planner when a preview lands and off
    /// again when the operator vetoes it, so a default-on flag would just be
    /// a checkbox the planner keeps overwriting.
    pub show_tier_preview: bool,
    /// Per-tool reach-map overlay (P5). The model is drawn in the reach
    /// colours of the SELECTED toolpath's cutter: green where the cutter can
    /// form the surface to the operation's tolerance, red where it cannot.
    ///
    /// Defaults **on**, like `show_rest_heatmap` and unlike
    /// `show_tier_preview`, because it is self-gating in the same way. The
    /// map exists only for a reach-capable operation
    /// (`OperationType::supports_reach_map`) with a mesh and a tool, so on
    /// every other selection the derived
    /// `ViewportCallback::show_reach_overlay` gate is false and the plain
    /// model draws.
    ///
    /// This flag replaces the plain model draw rather than draping over it —
    /// see the draw pass in `render/mod.rs` — so the two cannot z-fight.
    pub show_reach_map: bool,
    /// When set, only this toolpath is visible (isolation mode, toggle with I).
    pub isolate_toolpath: Option<ToolpathId>,
    /// Color mode for toolpath lines.
    pub toolpath_color_mode: ToolpathColorMode,
    /// Per-toolpath move-type visibility overlay — AND'd with the global
    /// `show_cutting` / `show_rapids` flags. Missing entries default to visible.
    pub toolpath_move_visibility: HashMap<ToolpathId, ToolpathMoveVisibility>,
    /// SpanKind visibility filter for the 3D renderer. When a kind is hidden,
    /// cut segments with that innermost span kind are dropped at upload time.
    pub span_kind_filter: SpanKindFilter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderMode {
    Shaded,
    Wireframe,
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
    pub fn new() -> Self {
        Self {
            show_grid: true,
            show_stock: true,
            show_fixtures: true,
            show_polygons: true,
            render_mode: RenderMode::Shaded,
            show_cutting: true,
            show_rapids: true,
            show_collisions: true,
            show_tool_profile_preview: false,
            show_rest_heatmap: true,
            show_tier_preview: false,
            show_reach_map: true,
            isolate_toolpath: None,
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
