use super::toolpath::ToolpathId;

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
    /// When set, only this toolpath is visible (isolation mode, toggle with I).
    pub isolate_toolpath: Option<ToolpathId>,
    /// Color mode for toolpath lines.
    pub toolpath_color_mode: ToolpathColorMode,
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
            isolate_toolpath: None,
            toolpath_color_mode: ToolpathColorMode::Normal,
        }
    }
}

impl Default for ViewportState {
    fn default() -> Self {
        Self::new()
    }
}
