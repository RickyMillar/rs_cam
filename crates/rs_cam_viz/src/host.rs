//! The event-loop host — rs_cam's own `winit::ApplicationHandler`, wrapped
//! around eframe's.
//!
//! **Why this exists (G-LV.1, `planning/review_2026-08-08/DISPATCH_DECOUPLING_DESIGN.md`).**
//! Every MCP request and every `generate_all` round handoff is dispatched from
//! a *frame*, and on Wayland a hidden, occluded or screen-locked surface never
//! receives a compositor frame callback — so winit refuses to emit
//! `RedrawRequested` (`winit-0.30.13
//! src/platform_impl/linux/wayland/event_loop/mod.rs:486-489`) and the frame
//! loop parks forever. The **event loop** is not parked: `AboutToWait` and
//! `UserEvent` are dispatched unconditionally every iteration (`mod.rs:515`,
//! `:356`). Dispatch is frame-coupled by where we put the drain, not by the
//! platform.
//!
//! Owning the loop is the precondition for un-coupling it. `eframe` 0.34.3
//! exposes exactly one route — [`eframe::create_native`] returning a public
//! [`eframe::EframeWinitApplication`] that we can wrap — because
//! `NativeOptions::event_loop_builder` hands out the *builder*, which has no
//! `create_proxy`, and every other hook is per-frame, which is precisely what
//! a parked loop does not have.
//!
//! **MEASURED CORRECTION, B-4 step 4 (2026-08-12).** The paragraph above is
//! the design's model, read from winit's source, and on this machine it is
//! **not what strands the process**. Instrumented under a real park (GNOME
//! Wayland, window minimised, `wgpu` on `PresentMode::AutoVsync` = FIFO), the
//! main thread stops calling `about_to_wait` **488 ms after the minimise** and
//! never calls it again. `/proc/<pid>/syscall` reads `7 … 0x1 0xffffffff` —
//! `poll()` on **one** fd with an **infinite** timeout, which is not calloop
//! (calloop polls an epoll fd) but the Wayland/Mesa WSI waiting for a buffer
//! release that a minimised surface never gets. Seven `EventLoopProxy::send_event`
//! pings arrived during that window and produced zero `user_event` and zero
//! `about_to_wait` callbacks.
//!
//! So the loop is not merely declining to *paint*: the main thread is blocked
//! **inside the present**, below winit, and no `ApplicationHandler` callback
//! can run at all. An off-frame pump that lives on the main thread cannot
//! rescue that, because under this failure mode there is no main thread to
//! pump on. Everything in this module is still correct and still necessary —
//! it decouples dispatch from the *paint* — but it is not sufficient against
//! a present-blocked park, and B-4 stopped at the design's own step-4 gate
//! rather than build steps 5-7 on top of an unmet bar. The evidence is in
//! `planning/review_2026-08-08/artifacts/b4/`.
//!
//! This module is the wrapper only. What it does with the wakeup is the
//! subject of later steps in that design's migration order.

use std::cell::RefCell;
use std::rc::Rc;

use crate::app::RsCamApp;

/// The `cumulative_pass_nr` that marks a `UserEvent::RequestRepaint` as
/// rs_cam's own bare wakeup rather than a repaint request from egui.
///
/// eframe's `UserEvent` is a **closed enum** — `RequestRepaint` and a
/// feature-gated accesskit variant — so a wakeup cannot carry an MCP payload
/// and must borrow an existing shape. `u64::MAX` is a pass number no real
/// egui pass will ever reach, and [`RsCamHost::user_event`] swallows it so
/// eframe never sees it.
pub(crate) const MCP_WAKE_PASS_NR: u64 = u64::MAX;

/// The one `RsCamApp`, shared between the host (which pumps it off-frame) and
/// the [`RsCamAppProxy`] that eframe owns and paints through.
///
/// `Rc`/`RefCell` and not `Arc`/`Mutex` on purpose. The controller is
/// main-thread-owned; a cross-thread lock would re-serialise exactly what
/// A/M12 unpicked (a frame render blocking on a long `narrate_toolpath` and
/// vice versa), would let a handler mutate `session` mid-frame so a panel
/// renders half of one state and half of another, and would make the lock
/// order between egui `Context` internals and the controller unstateable.
/// Nothing here ever leaves the main thread, so none of that is needed and no
/// `Send` bound is claimed.
pub(crate) type AppCell = Rc<RefCell<RsCamApp>>;

/// Bootstrap slot. `RsCamApp` is built inside eframe's `AppCreator`, which
/// runs on `resumed` — i.e. *after* `run_app` has started — so the host cannot
/// be handed the app at construction time. The creator drops it in here and
/// the host picks it up on its first `about_to_wait`. Before that there is
/// nothing to pump, which is correct rather than merely tolerable.
pub(crate) type AppLatch = Rc<RefCell<Option<AppCell>>>;

/// What eframe actually owns: a handle onto the shared app.
///
/// **`RsCamApp` implements exactly one `eframe::App` method — `ui`.** If it
/// ever grows another (`logic`, `save`, `on_exit`, `raw_input_hook`), that
/// method must be forwarded here too or it will silently stop being called.
/// `logic` is already forwarded so the common case is covered.
///
/// The `&self` methods (`clear_color`, `persist_egui_memory`,
/// `auto_save_interval`) are deliberately **not** forwarded: eframe can call
/// them while `ui`'s borrow is live, and a `try_borrow` that lost would have
/// to substitute a default silently — a wrong colour presented as the app's
/// choice. `RsCamApp` overrides none of them, so the defaults it gets through
/// this proxy are the defaults it gets today.
pub(crate) struct RsCamAppProxy {
    app: AppCell,
}

impl RsCamAppProxy {
    pub(crate) fn new(app: AppCell) -> Self {
        Self { app }
    }
}

impl eframe::App for RsCamAppProxy {
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if let Ok(mut app) = self.app.try_borrow_mut() {
            app.logic(ctx, frame);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        // `try_borrow_mut`, never `borrow_mut`: the workspace denies `panic`,
        // and this satisfies it structurally rather than by argument. A lost
        // race means a frame is already in progress on this same thread, which
        // cannot happen through this path today — but if it ever does, losing
        // a frame is recoverable and a panic is not.
        if let Ok(mut app) = self.app.try_borrow_mut() {
            app.ui(ui, frame);
        }
    }
}

/// How often the event loop re-wakes while MCP work or a compute lane is
/// outstanding.
///
/// Deliberately slow. This is a **safety net for completions nobody
/// announces** (see `RsCamApp::needs_pump_tick`), not the dispatch path —
/// enqueues arrive through the `GuiWaker` in sub-millisecond time and do not
/// wait on this. A generation that finishes 250 ms before anyone notices is
/// invisible next to a generation that takes minutes; a general-purpose clock
/// fast enough to matter would be the 100 ms heartbeat again, and that
/// heartbeat is the thing masking six known bugs.
const PUMP_TICK: std::time::Duration = std::time::Duration::from_millis(250);

/// Tighten the loop's `ControlFlow` to at most one [`PUMP_TICK`] away.
///
/// **Only ever tightens.** eframe sets the control flow from its own redraw
/// scheduling in the same callback, and overriding a sooner wakeup with a
/// later one would delay a paint eframe had already asked for. `Poll` is left
/// alone for the same reason: it is already sooner than anything this could
/// ask for.
fn arm_pump_tick(event_loop: &winit::event_loop::ActiveEventLoop) {
    use winit::event_loop::ControlFlow;

    let deadline = std::time::Instant::now() + PUMP_TICK;
    match event_loop.control_flow() {
        ControlFlow::Wait => event_loop.set_control_flow(ControlFlow::WaitUntil(deadline)),
        ControlFlow::WaitUntil(existing) if existing > deadline => {
            event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
        }
        ControlFlow::Poll | ControlFlow::WaitUntil(_) => {}
    }
}

/// rs_cam's `ApplicationHandler`, wrapping eframe's.
///
/// Every callback forwards to eframe **first** and takes the app borrow only
/// after it returns, never across an inner call. That ordering is load-bearing:
/// eframe can paint synchronously from inside `check_redraw_requests`
/// (`eframe .../native/run.rs:221-224`), which re-enters [`RsCamAppProxy::ui`]
/// and takes the same borrow.
pub(crate) struct RsCamHost<'a> {
    inner: eframe::EframeWinitApplication<'a>,
    latch: AppLatch,
    app: Option<AppCell>,
}

impl<'a> RsCamHost<'a> {
    pub(crate) fn new(inner: eframe::EframeWinitApplication<'a>, latch: AppLatch) -> Self {
        Self {
            inner,
            latch,
            app: None,
        }
    }

    /// Pick the app up out of the bootstrap latch, once.
    fn adopt_app(&mut self) {
        if self.app.is_some() {
            return;
        }
        if let Ok(mut slot) = self.latch.try_borrow_mut() {
            self.app = slot.take();
        }
    }
}

impl winit::application::ApplicationHandler<eframe::UserEvent> for RsCamHost<'_> {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        self.inner.resumed(event_loop);
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        self.inner.window_event(event_loop, window_id, event);
    }

    fn new_events(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        cause: winit::event::StartCause,
    ) {
        self.inner.new_events(event_loop, cause);
    }

    fn user_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        event: eframe::UserEvent,
    ) {
        // Swallow rs_cam's own wakeup. Its job was done the moment it woke the
        // poll: `about_to_wait` runs at the end of this iteration and pumps.
        // Forwarding it would have eframe set `ControlFlow::Poll` and spin.
        if matches!(
            event,
            eframe::UserEvent::RequestRepaint {
                cumulative_pass_nr: MCP_WAKE_PASS_NR,
                ..
            }
        ) {
            return;
        }
        self.inner.user_event(event_loop, event);
    }

    fn device_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        self.inner.device_event(event_loop, device_id, event);
    }

    fn about_to_wait(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        // eframe first, always. It can paint synchronously from inside
        // `check_redraw_requests`, which re-enters `RsCamAppProxy::ui` and
        // takes the borrow this method is about to take.
        self.inner.about_to_wait(event_loop);
        self.adopt_app();

        // The off-frame dispatch. `AboutToWait` is dispatched on every
        // event-loop iteration regardless of frame-callback state, so this
        // runs on a window the compositor has stopped drawing.
        //
        // A lost `try_borrow_mut` means a frame is running on this same
        // thread and is about to run the identical dispatch at its top, so
        // losing the race costs a wakeup and no work.
        if let Some(app) = self.app.as_ref()
            && let Ok(mut app) = app.try_borrow_mut()
        {
            app.off_frame_pump();
            if app.needs_pump_tick() {
                arm_pump_tick(event_loop);
            }
        }
    }

    fn suspended(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        self.inner.suspended(event_loop);
    }

    fn exiting(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        self.inner.exiting(event_loop);
    }

    fn memory_warning(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        self.inner.memory_warning(event_loop);
    }
}
