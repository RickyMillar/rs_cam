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
    /// Color each segment by per-sample effective chip thickness vs the
    /// matched LUT row's chipload window. Blue = under-engaged, green =
    /// within bounds, orange/red = approaching or exceeding cl_max,
    /// grey = no envelope or no sample.
    Chipload,
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
