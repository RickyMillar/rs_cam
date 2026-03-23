fn main() -> eframe::Result {
    tracing_subscriber::fmt::init();
    install_panic_hook();
    rs_cam_viz::run()
}

fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_else(|| "unknown".into());

        let message = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "unknown panic".to_string()
        };

        tracing::error!("rs_cam crashed due to internal error: {message} (at {location})");

        // Delegate to default hook for stderr backtrace
        default_hook(info);
    }));
}
