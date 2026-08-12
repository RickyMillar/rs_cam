#![deny(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

pub mod app;
pub mod compute;
pub mod controller;
pub mod error;
mod host;
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

    // G-LV.1 / Checkpoint M-1: rs_cam owns the winit event loop.
    //
    // This replaces `eframe::run_native`, and it is the programme's one
    // architectural change. `create_native` is eframe 0.34.3's supported hook
    // for exactly this (`eframe/src/lib.rs:328`, documented from `:281`); the
    // returned `EframeWinitApplication` is a public `ApplicationHandler` we
    // wrap rather than replace, so nothing about how eframe drives egui, wgpu
    // or the window moves. What moves is who gets called first, which is what
    // lets a later step in the migration drain MCP work from `about_to_wait`
    // — an event-loop callback the Wayland frame-callback gate does not
    // touch.
    //
    // Two divergences from `run_native`, both stated rather than discovered:
    //
    // 1. `run_native` with `run_and_return: true` (the default,
    //    `eframe/src/epi.rs:488`) returns `WinitAppWrapper::return_result`,
    //    eframe's own internal error. `create_native` does not expose that
    //    field, so an internal eframe error is logged by eframe and this
    //    function returns `Ok`. Startup errors still surface — they come back
    //    from `build()` below.
    // 2. `create_wgpu` hard-codes `run_and_return: true`
    //    (`eframe/src/native/run.rs:465`), so exit still asks the loop to
    //    exit rather than calling `std::process::exit(0)`. Quit semantics are
    //    unchanged.
    let event_loop = winit::event_loop::EventLoop::<eframe::UserEvent>::with_user_event().build()?;

    // Bootstrap latch — see `host::AppLatch`. The creator closure below runs
    // on `resumed`, inside `run_app`, so this is the only way to hand the
    // host a reference to an app that does not exist yet.
    let latch: host::AppLatch = std::rc::Rc::new(std::cell::RefCell::new(None));
    let creator_latch = std::rc::Rc::clone(&latch);

    let eframe_app = eframe::create_native(
        "rs_cam",
        options,
        Box::new(move |cc| {
            let app = std::rc::Rc::new(std::cell::RefCell::new(app::RsCamApp::new(cc, mcp_mode)));
            match creator_latch.try_borrow_mut() {
                Ok(mut slot) => *slot = Some(std::rc::Rc::clone(&app)),
                // Unreachable: the latch is touched here and in
                // `RsCamHost::adopt_app`, never nested. Reported rather than
                // unwrapped, because the consequence is "the off-frame pump
                // never starts" and that must not be silent.
                Err(_) => tracing::error!(
                    "event-loop host could not adopt the app: off-frame MCP dispatch is DISABLED \
                     for this session; the GUI still runs frame-coupled"
                ),
            }
            Ok(Box::new(host::RsCamAppProxy::new(app)) as Box<dyn eframe::App>)
        }),
        &event_loop,
    );

    let mut host = host::RsCamHost::new(eframe_app, latch);
    event_loop.run_app(&mut host)?;
    Ok(())
}
