/// Viewport rendering state.
pub struct ViewportState {
    pub show_grid: bool,
    pub show_stock: bool,
    pub show_wireframe: bool,
}

impl ViewportState {
    pub fn new() -> Self {
        Self {
            show_grid: true,
            show_stock: true,
            show_wireframe: false,
        }
    }
}
