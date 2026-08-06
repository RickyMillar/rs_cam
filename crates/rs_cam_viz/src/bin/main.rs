fn main() -> eframe::Result {
    let mcp_mode = std::env::args().any(|arg| arg == "--mcp");

    if mcp_mode {
        // MCP mode: stdout is the MCP transport, so redirect tracing to stderr.
        tracing_subscriber::fmt()
            .with_writer(std::io::stderr)
            .with_ansi(false)
            .init();
    } else {
        tracing_subscriber::fmt::init();
    }

    install_panic_hook();
    if mcp_mode {
        warn_if_wayland_can_park_the_frame_loop();
    }
    rs_cam_viz::run(mcp_mode)
}

/// G-LV.1 (live, 2026-08-07): on Wayland, hiding or locking this window
/// stalls every MCP call and every `generate_all` round handoff, silently.
///
/// The GUI dispatches MCP work from `RsCamApp::update`, which only runs on a
/// repaint. A repaint request reaches `Window::request_redraw`, and winit's
/// Wayland backend will not emit `RedrawRequested` while the surface is
/// waiting on a compositor frame callback — which a hidden, occluded or
/// screen-locked surface never receives. The loop parks and `request_repaint`
/// cannot unpark it. X11 has no such gate: its redraws are client-driven.
///
/// This is a warning and not a default because forcing the backend would
/// change how the GUI renders for every interactive session too. The
/// escape hatches (`generation_status`, `cancel_generation`) now report the
/// parked state when it happens; this says it before it does.
fn warn_if_wayland_can_park_the_frame_loop() {
    // winit picks Wayland when WAYLAND_DISPLAY is set unless WINIT_UNIX_BACKEND
    // overrides it, so "not overridden away from wayland" is the condition.
    let backend_forced_elsewhere =
        std::env::var("WINIT_UNIX_BACKEND").is_ok_and(|backend| backend != "wayland");
    if std::env::var_os("WAYLAND_DISPLAY").is_none() || backend_forced_elsewhere {
        return;
    }
    tracing::warn!(
        "MCP mode on Wayland: keep this window VISIBLE. A hidden, occluded or \
         screen-locked window receives no compositor frame callbacks, so the GUI \
         stops repainting — and every MCP request plus every generate_all round \
         handoff is dispatched from a repaint. They will stall indefinitely with \
         the compute lane reporting idle. `generation_status` reports this as \
         `frame_loop.healthy: false`. To remove the hazard entirely, relaunch with \
         WINIT_UNIX_BACKEND=x11, where redraws are client-driven."
    );
}

fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_else(|| "unknown".into());

        let message = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_owned()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "unknown panic".to_owned()
        };

        tracing::error!("rs_cam crashed due to internal error: {message} (at {location})");

        // Delegate to default hook for stderr backtrace
        default_hook(info);
    }));
}
