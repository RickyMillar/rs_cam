fn main() -> eframe::Result {
    let mcp_mode = std::env::args().any(|arg| arg == "--mcp");

    init_tracing(mcp_mode);
    install_panic_hook();
    if mcp_mode {
        warn_if_wayland_can_park_the_frame_loop();
    }
    rs_cam_viz::run(mcp_mode)
}

/// Install the global tracing subscriber.
///
/// **The interactive path is byte-for-byte what it was**: `fmt::init()`, whose
/// filtering is `RUST_LOG` if set and `INFO` otherwise
/// (`tracing-subscriber-0.3.23/src/fmt/mod.rs:1216-1234`).
///
/// The `--mcp` path adds one thing to that same arrangement — a layer that
/// reads the negotiated `wgpu::PresentMode` off wgpu-core's own log line
/// (Checkpoint O-2; see `rs_cam_viz::present_mode::capture_layer`). It is built
/// by hand rather than through `fmt::init()` because the two layers need
/// **different** filters: the format layer keeps the user's level, and the
/// capture layer needs `TRACE` on exactly one target. The `Targets` filter and
/// default level here are the same ones `fmt::init()` would have installed, so
/// nothing an operator sees changes.
///
/// It is not installed for interactive launches, and deliberately: the flip it
/// observes is `--mcp`-only, and enabling a `TRACE` target raises the global
/// `log` max level, which re-checks every `log` record in the process.
fn init_tracing(mcp_mode: bool) {
    use tracing_subscriber::Layer as _;
    use tracing_subscriber::layer::SubscriberExt as _;
    use tracing_subscriber::util::SubscriberInitExt as _;

    if !mcp_mode {
        tracing_subscriber::fmt::init();
        return;
    }

    // MCP mode: stdout is the MCP transport, so tracing goes to stderr.
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .with_filter(user_log_filter());
    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(rs_cam_viz::present_mode::capture_layer())
        .init();
}

/// `RUST_LOG` if it parses, `INFO` otherwise — `fmt::init()`'s own rule
/// (`tracing-subscriber-0.3.23/src/fmt/mod.rs:1216-1234`), reproduced here
/// because the layered build cannot call it.
///
/// An unparseable `RUST_LOG` yields an empty filter, exactly as upstream does.
/// Upstream also prints a note about it; this cannot, because the subscriber it
/// would print through is the one being built.
fn user_log_filter() -> tracing_subscriber::filter::Targets {
    use std::str::FromStr as _;
    use tracing_subscriber::filter::{LevelFilter, Targets};

    match std::env::var("RUST_LOG") {
        Ok(var) => Targets::from_str(&var).unwrap_or_default(),
        Err(_) => Targets::new().with_default(LevelFilter::INFO),
    }
}

/// G-LV.1 (live, 2026-08-07): on Wayland, hiding or locking this window used to
/// stall every MCP call and every `generate_all` round handoff, silently.
///
/// **What changed, and exactly how far it goes (Checkpoint O, 2026-08-13).**
/// The mechanism was measured to travel with FIFO, not with Wayland: under
/// `PresentMode::AutoVsync` (= `Fifo` on the measured surface) a minimised
/// window blocked the main thread inside the present in **405/405** syscall
/// samples; under `AutoNoVsync` (→ `Mailbox`) in **0/355**, and the minimised
/// window kept painting. `--mcp` launches now request `AutoNoVsync`
/// (`rs_cam_viz::present_mode`). So this warning is no longer a description of
/// the shipped agent path — it is a description of what happens **if the
/// negotiation falls back to FIFO anyway**, which `AutoNoVsync`'s rule permits
/// on a surface offering neither `Immediate` nor `Mailbox`.
///
/// **The caveat, stated because it is load-bearing:** this fixes the *minimise*
/// reproduction; incident-state coverage (occluded / visible-frozen) is pending
/// N-3 at close-out. The 2026-08-07 incident was occluded or merely unfocused,
/// and one park was explicitly "visible". Those states are not measured.
///
/// The dispatch half is separate and unchanged: MCP work is dispatched from a
/// repaint *and* from the event loop's `about_to_wait` (`host.rs`), so on a
/// window that stops painting but leaves the loop alive — which is what X11
/// does — calls still answer. N-2 measured 240/240 live at 0.2 ms there. Note
/// that a minimised **X11** window also stops painting; what X11 does not do is
/// strand dispatch. The older claim that "X11 redraws are client-driven" was
/// imprecise about which half survives.
///
/// The escape hatches (`generation_status`, `cancel_generation`) report the
/// parked state when it happens, and `frame_loop.present_mode.negotiated`
/// reports whether the hazardous mode is in force; this says it before either.
fn warn_if_wayland_can_park_the_frame_loop() {
    // winit picks Wayland whenever WAYLAND_DISPLAY is set. That is the whole
    // condition.
    //
    // This used to also suppress the warning when `WINIT_UNIX_BACKEND` was
    // set to anything but "wayland" — which made the warning **silently
    // wrong**: winit removed that variable in 0.29 ("in favor of standard
    // WAYLAND_DISPLAY and DISPLAY variables", winit-0.30.13
    // src/changelog/v0.29.md:134) and this workspace is on 0.30.13, so
    // setting it changed nothing except whether the user was warned. Wave
    // B-1 measured that exact trap on 2026-08-08: a run with
    // WINIT_UNIX_BACKEND=x11 set came up on Wayland anyway (sctk_adwaita in
    // the log) and its first MCP call never returned, killed at 162.6 s;
    // unsetting WAYLAND_DISPLAY instead, the same call answered in 0.515 s.
    if std::env::var_os("WAYLAND_DISPLAY").is_none() {
        return;
    }
    tracing::warn!(
        "MCP mode on Wayland. This launch requests PresentMode::AutoNoVsync, which on a \
         surface offering Mailbox or Immediate removes the measured park: a minimised window \
         kept painting and every MCP call answered live. CHECK WHICH MODE YOU GOT — \
         `generation_status` reports it as `frame_loop.present_mode.negotiated`, and the \
         startup log prints it. If it says Fifo or FifoRelaxed, the hazard is back: a hidden, \
         occluded or screen-locked window can block the main thread inside the present, and \
         every MCP request plus every generate_all round handoff stalls indefinitely with the \
         compute lane reporting idle (`frame_loop.healthy: false`). The certain remedy is still \
         to relaunch with WAYLAND_DISPLAY unset so winit picks X11/XWayland, where a window \
         that stops painting does not strand dispatch. Setting WINIT_UNIX_BACKEND does NOT \
         work — winit removed that variable in 0.29 and this build is on 0.30. Coverage \
         caveat: the flip is measured against a MINIMISED window; occluded and \
         visible-but-frozen states are not yet measured (pending N-3)."
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
