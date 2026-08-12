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
pub mod present_mode;
pub mod render;
pub mod state;
pub mod ui;

/// A wakeup for the GUI event loop that survives a parked frame loop.
///
/// **Why this is not `egui::Context::request_repaint`.** Under a park, the
/// second and every subsequent `request_repaint()` is silently dropped before
/// it reaches eframe. egui only invokes the repaint callback when the new
/// delay is *strictly lower* than the lowest already requested
/// (`egui-0.34.3 src/context.rs:155-168`), and `repaint_delay` is reset per
/// pass — so with no pass running it stays at `Duration::ZERO`, `0 < 0` is
/// false, and nothing is ever sent. The call is made and not delivered, which
/// is exactly why a sentry asserting the *call* stays green straight through
/// the hole.
///
/// The concrete implementation is an `EventLoopProxy::send_event`, which
/// writes to a calloop channel registered as an event source and wakes the
/// poll directly — no compositor, no dedup, no frame. It is kept behind this
/// alias rather than named as a winit type so the MCP server stays
/// window-system agnostic, and so the escape-hatch sentries can drive it with
/// a counter instead of a real event loop.
pub type GuiWaker = std::sync::Arc<dyn Fn() + Send + Sync>;

pub fn run(mcp_mode: bool) -> eframe::Result {
    // Title carries the git desc so the running build is identifiable
    // at a glance (e.g. "rs_cam — 3f9a1c2-dirty").
    let title = format!("rs_cam — {}", rs_cam_core::build_info::GIT_DESC);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 900.0])
            .with_title(&title),
        wgpu_options: egui_wgpu::WgpuConfiguration {
            // Checkpoint O-1: `--mcp` asks for AutoNoVsync, a plain launch keeps
            // AutoVsync. See `present_mode` for the ruling, the measurement it
            // rests on, and the negotiated-vs-requested distinction that makes
            // this observable rather than assumed.
            present_mode: present_mode::decide_and_record(mcp_mode),
            ..Default::default()
        },
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
    let event_loop =
        winit::event_loop::EventLoop::<eframe::UserEvent>::with_user_event().build()?;

    // G-LV.1 / C5: the wakeup that survives a park. `EventLoopProxy` is `Send`
    // but NOT `Sync` — eframe wraps its own in an `Arc<Mutex<_>>` for exactly
    // this reason (`eframe/src/native/wgpu_integration.rs:251-268`) — so the
    // mutex here is around a proxy handle, not around any app state, and it is
    // held for the duration of one channel write.
    let proxy = std::sync::Mutex::new(event_loop.create_proxy());
    let waker: GuiWaker = std::sync::Arc::new(move || {
        // `send_event` writes to a calloop channel registered as an event
        // source; the write wakes the poll directly. No compositor frame
        // callback is involved, which is the entire point.
        //
        // The payload must be inert to eframe, so the host swallows this
        // sentinel rather than forwarding it: an unswallowed `RequestRepaint`
        // would make eframe set `ControlFlow::Poll` and busy-spin. (Forwarding
        // would in fact also be safe — `run.rs:329-332` classifies a
        // mismatched pass number as outdated and returns `EventResult::Wait` —
        // but swallowing is explicit, and explicit is what a wakeup path
        // nobody looks at again should be.)
        if let Ok(proxy) = proxy.lock() {
            let _ = proxy.send_event(eframe::UserEvent::RequestRepaint {
                viewport_id: egui::ViewportId::ROOT,
                when: std::time::Instant::now(),
                cumulative_pass_nr: host::MCP_WAKE_PASS_NR,
            });
        }
    });

    // Bootstrap latch — see `host::AppLatch`. The creator closure below runs
    // on `resumed`, inside `run_app`, so this is the only way to hand the
    // host a reference to an app that does not exist yet.
    let latch: host::AppLatch = std::rc::Rc::new(std::cell::RefCell::new(None));
    let creator_latch = std::rc::Rc::clone(&latch);

    let eframe_app = eframe::create_native(
        "rs_cam",
        options,
        Box::new(move |cc| {
            let app = std::rc::Rc::new(std::cell::RefCell::new(app::RsCamApp::new(
                cc,
                mcp_mode,
                Some(std::sync::Arc::clone(&waker)),
            )));
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
