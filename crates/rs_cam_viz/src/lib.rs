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
    // Title carries the git desc so the running build is identifiable
    // at a glance (e.g. "rs_cam — 3f9a1c2-dirty").
    let title = format!("rs_cam — {}", rs_cam_core::build_info::GIT_DESC);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 900.0])
            .with_title(&title),
        ..Default::default()
    };

    eframe::run_native(
        "rs_cam",
        options,
        Box::new(move |cc| Ok(Box::new(app::RsCamApp::new(cc, mcp_mode)))),
    )
}
