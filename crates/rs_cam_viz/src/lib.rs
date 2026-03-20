pub mod app;
pub mod compute;
pub mod state;
pub mod ui;
pub mod render;
pub mod io;

pub fn run() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 900.0])
            .with_title("rs_cam"),
        ..Default::default()
    };

    eframe::run_native(
        "rs_cam",
        options,
        Box::new(|cc| Ok(Box::new(app::RsCamApp::new(cc)))),
    )
}
