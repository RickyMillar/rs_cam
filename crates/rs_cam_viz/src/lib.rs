#![deny(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

pub mod app;
pub mod compute;
pub mod controller;
pub mod error;
pub mod interaction;
pub mod io;
#[cfg(feature = "mcp")]
pub mod mcp_bridge;
#[cfg(feature = "mcp")]
pub mod mcp_server;
pub mod render;
pub mod state;
pub mod ui;

pub fn run(mcp_mode: bool) -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 900.0])
            .with_title("rs_cam"),
        ..Default::default()
    };

    eframe::run_native(
        "rs_cam",
        options,
        Box::new(move |cc| Ok(Box::new(app::RsCamApp::new(cc, mcp_mode)))),
    )
}
