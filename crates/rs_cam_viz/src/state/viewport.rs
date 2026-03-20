/// Viewport rendering state.
pub struct ViewportState {
    pub show_grid: bool,
    pub show_stock: bool,
    pub render_mode: RenderMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderMode {
    Shaded,
    Wireframe,
}

impl ViewportState {
    pub fn new() -> Self {
        Self {
            show_grid: true,
            show_stock: true,
            render_mode: RenderMode::Shaded,
        }
    }
}
