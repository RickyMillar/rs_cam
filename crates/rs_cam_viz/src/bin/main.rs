fn main() -> eframe::Result {
    tracing_subscriber::fmt::init();
    rs_cam_viz::run()
}
