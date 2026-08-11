# Dispatch decoupling — architecture map, candidates, and a recommendation

TD3 wave **B-3** (research/design only; **no production code changed**). Ledger
row **G-LV.1** re-open path (b) in
`planning/review_2026-08-04/TECH_DEBT_2_CLOSEOUT.md` §4.2; charter in
`planning/review_2026-08-08/TECH_DEBT_3_RESEARCH_AND_FIX_PLAN.md` §3 "B-3".

Date: 2026-08-12. Branch `tech-debt-3`, tree at `045b5b97`. Every source
citation below was read at that revision — including the files B-2 rewrote in
`825524fe` / `afd102bb` / `03216770`, which are read as they now stand, not as
earlier notes describe them.

**No Cargo job was launched by this wave.** The machine-wide slot was held by a
foreign build throughout; everything here is established by reading source —
this repository, `eframe-0.34.3`, `egui-0.34.3` and `winit-0.30.13` from the
vendored registry — and by re-reading B-1's measurements. Where a claim would
have needed a build or a live run, it is marked NOT ESTABLISHED and says why.

---

## 1. Why this is on the table

The operator drives this GUI through Claude agents in production and called
repaint-coupled dispatch **"not ideal for a program with strong AI
integration"**. That is the motivation of record; the rest is corroboration.

- **The incident.** 2026-08-07, release build at `73e2376`, wanaka,
  `simulation_resolution_mm 0.1`: a `generate_all` fixpoint run stalled
  indefinitely between rounds once the window stopped repainting. The lane
  finished, went idle, and `generation_status` answered *"idle: no toolpath
  generation in flight"* — true, and indistinguishable from success.
- **The cost to this programme.** **Two consecutive Lane A waves** (A-1, A-2)
  had rule-3 screenshot obligations blocked by parked frame loops. One park
  lasted **~9.5 h with the window visible**, frames frozen at **87899**; it
  resumed between polls days later and **the unpark trigger was never
  observed**. That is stated as unknown, not guessed.
- **B-1 could not exercise the Wayland half of its own repro** — its matching
  run aborted at `load_project` after 90 s with no response
  (`GLV2_CRASH_CAPTURE.md` §7, uncertainty (b)).
- **The remedy that works today** is to launch with `WAYLAND_DISPLAY` unset, so
  winit picks X11/XWayland where redraws are client-driven.
  `WINIT_UNIX_BACKEND=x11` is **inert** — winit removed it in 0.29
  (`winit-0.30.13/src/changelog/v0.29.md:134`) and B-2 fixed the strings that
  recommended it (`crates/rs_cam_viz/src/bin/main.rs:35-63`).

What shipped at `66c8f16` is **reporting, not removal**: the standing repaint
while `PendingMcpCompute::awaiting_gui() > 0` (`app.rs:795-806`) and the
`FrameLoopBeat` that lets a caller tell a parked loop from an idle lane
(`mcp_bridge.rs:109-288`). Its own commit message says the repaint "cannot
help". This wave asks whether the mechanism can be removed.

---

## 2. The current dispatch architecture, mapped

### 2.1 Ingress — the MCP server thread

`rs_cam_viz::run(mcp_mode)` (`lib.rs:17-33`) calls `eframe::run_native`.
`RsCamApp::new` (`app.rs:51-175`) creates a `std::sync::mpsc::channel`
(`app.rs:75`), clones `cc.egui_ctx` (`:76`), takes a `Send + Sync`
`GenerationControl` off the controller (`:80`, defined `controller.rs:170`),
clones the `McpReadCache` (`:81`), and spawns the **`mcp-server` thread**
(`app.rs:83-119`) running a tokio runtime that serves `rmcp` over stdio. The
`Receiver` stays on the GUI thread as `RsCamApp::mcp_receiver` (`app.rs:36-37`).

`EmbeddedCamServer` (`mcp_server.rs:67-76`) holds `request_tx`, `egui_ctx`,
`generation`, `reads`. It registers **69 tools** (`#[tool_router]`,
`mcp_server.rs:360-1555`).

### 2.2 The four dispatch doors, and the exact tool split

| door | mechanism | tools | code |
|---|---|---|---|
| **FRAME** | `send_request` → mpsc → frame drain → oneshot | **58** | `mcp_server.rs:100-115` |
| **PROGRESS** | `send_with_progress` → same channel, plus a progress mpsc and an optional `timeout_s` | **4** (`generate_toolpath`, `generate_all`, `run_simulation`, `collision_check`) | `mcp_server.rs:254-320` |
| **CHEAP** | `cheap_read` — same channel, but races `BUSY_READ_DEADLINE = 750 ms` and falls back to the published snapshot | **5** (`project_summary`, `list_toolpaths`, `inspect_model`, `inspect_stock`, `inspect_machine`) | `mcp_server.rs:149-238` |
| **OFF-LOOP** | answered on the server thread from `GenerationControl` | **2** (`generation_status`, `cancel_generation`) | `mcp_server.rs:1277,1285`; builders `mcp_bridge.rs:709,794` |

**62 of 69 tools cannot answer without a frame.** 5 more can answer *stale*
(snapshot) but only ever produce a *live* answer from a frame. Only 2 are
genuinely frame-independent today.

Every enqueue path goes through `wake_gui` (`mcp_server.rs:129-132`): record
`sent` on the `FrameLoopBeat`, then `egui_ctx.request_repaint()`.

### 2.3 The drain — once per repaint, strictly sequential

`impl eframe::App for RsCamApp` (`app.rs:464`) → `fn ui` (`:468`) →
`draw_frame` (`:475`). In frame order:

| # | what | line |
|---|---|---|
| 1 | screenshot events from the previous frame; `complete_mcp_gui_screenshot` | `app.rs:487-497` |
| 2 | `controller.drain_compute_results()` | `app.rs:506` |
| 3 | **`drain_mcp_requests(ctx)`** | `app.rs:509` |
| 4 | `pump_mcp_gui_screenshot(ctx)` | `app.rs:515` |
| 5 | **100 ms MCP heartbeat** `request_repaint_after(100 ms)` | `app.rs:534-537` |
| 6 | `take_pending_upload()` → `upload_gpu_data(frame)` | `app.rs:562-564` |
| 7 | all UI panels | `app.rs:566-680` |
| 8 | **`handle_events(ctx)`** — the `AppEvent` queue | `app.rs:681` |
| 9 | `process_auto_regen()` | `app.rs:784` |
| 10 | standing repaint while `awaiting_gui() > 0` | `app.rs:795-806` |
| 11 | `end_mcp_frame()` — closes the `in_frame` bracket | `app.rs:831` |

`drain_mcp_requests` (`app/mcp.rs:42-78`) opens the frame bracket
(`frame_loop().frame_begin()`, `:60`), `try_recv`s the whole channel into a
`Vec` (`:63-70`), then runs **every** request to completion inline
(`:72-75`) — one thread, one frame, no yielding. It closes by publishing the
cheap-read snapshot (`:77`, rate-limited to 500 ms at `app/mcp.rs:112-131`).

`handle_mcp_request` (`app/mcp.rs:133-…`) matches **67 `McpRequestKind`
variants** (`mcp_bridge.rs:342-708`) and resolves each oneshot.

### 2.4 Controller access — and the one thing that actually needs a frame

A field census of the entire 6,146-line `app/mcp.rs` (`grep -oP 'self\.\K[a-z_]+'`):

```
169 controller     4 mcp_reads     2 mcp_receiver     1 mcp_reads_published_at
```

plus its own `mcp_*` helper methods. **`camera`, `viewport_rect`,
`pending_checkpoint_load`, `last_hover_face` and every other `RsCamApp` field
are never touched by any MCP handler.**

`ctx: &egui::Context` reaches exactly **one** handler:
`McpRequestKind::ScreenshotGui` (`app/mcp.rs:732` → `mcp_screenshot_gui`,
`:4068-4133`), plus its per-frame pump (`:4135-4151`) and completion
(`:4153-…`, driven from `app.rs:487-497`).

**Correcting a premise.** `screenshot_simulation` and `screenshot_toolpath` are
**not frame-coupled at all**. They rasterise on the CPU through
`rs_cam_core::fingerprint::render_stock_composite` / `render_mesh_composite`
(`app/mcp.rs:3939,3941`) and `render_toolpath_composite` (`:4025`), or emit
standalone HTML (`:3965,4045`). They never touch the wgpu viewport. The
"screenshots need a frame" intuition holds for `screenshot_gui` **only**.

### 2.5 Egress

`McpResponse { result: Result<String, String> }` (`mcp_bridge.rs:858`) over a
`tokio::sync::oneshot`. Every GUI-side resolution is `let _ = tx.send(..)`, so
a timed-out caller dropping its receiver is a silent no-op
(`mcp_server.rs:240-253` documents the invariant). `format_result`
(`mcp_server.rs:347-357`) renders errors compactly (Checkpoint L-4).

### 2.6 The second frame dependency: deferred completions

Four tools do not answer in the drain at all. They arm `PendingMcpCompute`
(`mcp_bridge.rs:894-932`) and are resolved by a **later** frame:

- generation → `drain_compute_results` (`controller/events/compute.rs:722`) →
  `notify_mcp_toolpath_complete` (`:1505`);
- `generate_all`'s fixpoint ladder — `mcp_start_generate_all` and
  `settle_generate_all_round` / `resume_generate_all_after_simulation`
  (`controller/events/compute.rs:1250-1500`), **all of it `impl AppController`**;
- `screenshot_gui` → `pump_mcp_gui_screenshot` → the next-frame
  `egui::Event::Screenshot`.

This is the path the live incident actually died on: the round handoff needs a
frame that nothing was requesting. `app.rs:795-806` exists solely to request it,
and cannot make it happen.

### 2.7 Where the six GUI-only frame-coupled drivers sit

From `66c8f16`'s audit, recorded verbatim at
`planning/review_2026-08-04/ORCHESTRATION_LOG.md:1255`:

| driver | site | position in §2.3 |
|---|---|---|
| `process_auto_regen` 500 ms stale debounce | `controller.rs:230` | step 9 — poll-based; nothing schedules the frame that fires it |
| `pending_upload` set during `handle_events` | `app.rs:681` runs after the consume at `:562` | steps 6 vs 8 — a pure UI toggle waits a frame |
| lane-`Idle` drain race, GUI-initiated | `drain_compute_results` | step 2 — the same window `app.rs:795-806` covers for MCP only |
| `RunSimulationWith` re-push behind `pending_inspect_toolpath` | `controller/events/toolpath.rs:384` | step 8 |
| `pending_toolpath_tab` | `state/runtime.rs:139` | step 7 |
| `scrub_drag_active` can stick `true` if the timeline is not drawn | `app/simulation.rs:98` | step 7 — a liveness bug, not a repaint bug |

**The trap that audit recorded, and which now becomes a sequencing
constraint:** all six are masked by the 100 ms heartbeat (`app.rs:534-537`), so
**none reproduces under MCP-driven testing**. Any design that deletes the
heartbeat therefore **unmasks all six**. See §8, migration step 6.

---

## 3. The platform mechanism, re-verified at this revision — and one new fact

### 3.1 The gate is exactly where the ledger says

`winit-0.30.13/src/platform_impl/linux/wayland/event_loop/mod.rs:486-489`,
inside `single_iteration`:

```rust
if window.frame_callback_state() == FrameCallbackState::Requested {
    return None;                       // ← no WindowEvent::RedrawRequested
}
```

A hidden, occluded or screen-locked surface never receives the frame callback,
so this returns `None` forever.

### 3.2 What that gate does **not** cover — the opening the design uses

In the same `single_iteration`, after the redraw block:

- `mod.rs:515`: `callback(Event::AboutToWait, &self.window_target);` —
  **dispatched unconditionally, every iteration, with no reference to frame
  callback state.**
- `mod.rs:356`: `callback(Event::UserEvent(user_event), …)` — likewise.
- `poll_events_with_timeout` (`mod.rs:257-322`) honours
  `ControlFlow::WaitUntil` through the calloop poll timeout — a timer wakeup,
  not a compositor event.
- `EventLoopProxy::send_event` (`wayland/event_loop/proxy.rs:25-27`) writes to
  a **calloop channel** registered as an event source (`mod.rs:127-133`), whose
  handler sets `dispatched_events = true`. The write wakes the poll directly.

So: **on a parked Wayland surface the event loop still runs. Only the frame
does not.** Everything in §2 is frame-coupled by *our* choice of where to put
the drain, not by the platform.

### 3.3 eframe's own rescue is dead here — confirmed

`WinitAppWrapper::check_redraw_requests` (`eframe/src/native/run.rs:186-243`)
paints invisible windows directly, but only when `is_invisible_or_minimized`
(`:205`) — which reads winit's `is_visible()`/`is_minimized()`, both `None` on
Wayland. The `else` branch (`:207-211`) instead sets `ControlFlow::Poll` and
calls `window.request_redraw()` — straight back into §3.1's gate.

### 3.4 NEW: `ctx.request_repaint()` goes **silent** after the first one

Not previously recorded, and it changes what "the repaint is necessary and not
sufficient" (`mcp_server.rs:120-128`) actually means.

`egui-0.34.3/src/context.rs:155-168`:

```rust
// We save some CPU time by only calling the callback if we need to.
// If the new delay is greater or equal to the previous lowest,
// it means we have already called the callback, and don't need to do it again.
if delay < viewport.repaint.repaint_delay {
    viewport.repaint.repaint_delay = delay;
    if let Some(callback) = &self.request_repaint_callback { (callback)(…) }
}
```

`repaint_delay` is reset per pass. **Under a park no pass runs**, so after the
first `wake_gui()` it is already `Duration::ZERO`, `0 < 0` is false, and every
subsequent `ctx.request_repaint()` **never invokes eframe's callback** — so it
never reaches `EventLoopProxy::send_event`
(`eframe/src/native/wgpu_integration.rs:261`) and never wakes the loop at all.

Consequence for the design: a dedicated proxy owned by us is **not** a
re-spelling of `request_repaint`. It is the only wakeup that survives a park,
because it has no dedup. And the sentry
`every_mcp_enqueue_requests_a_repaint` asserts the call, not the delivery — so
it is green today and would stay green through this hole.

---

## 4. Design candidates

### (a) `EventLoopProxy` user events

**What it buys.** A `Send` handle whose `send_event` wakes the loop through
calloop with no compositor involvement (§3.2), and with no egui-side dedup
(§3.4).

**What it does not buy.** `eframe::UserEvent`
(`eframe/src/native/winit_integration.rs:52-69`, re-exported at
`epi.rs:14`) is a **closed enum**: `RequestRepaint { viewport_id, when,
cumulative_pass_nr }` and (feature-gated) `AccessKitActionRequest`. **We cannot
add a variant**, so a user event cannot carry an MCP payload. Its only value is
as a *wakeup ping*. And eframe's own `user_event` handler (`run.rs:290-335`)
converts `RequestRepaint` into `EventResult::RepaintAt` → `request_redraw()` →
§3.1's gate. **A user event alone therefore does not run a frame or a drain.**

**How eframe 0.34.3 lets us hold one — the load-bearing API.** `NativeOptions`
offers only `event_loop_builder: Option<EventLoopBuilderHook>`
(`eframe/src/epi.rs:34,384-391`), which is `Box<dyn FnOnce(&mut
EventLoopBuilder<UserEvent>)>` — it hands you the **builder**, which has no
`create_proxy`. There is no `App` hook either: `raw_input_hook` and
`persist_egui_memory` (`epi.rs:269`, `epi_integration.rs:275`) are per-frame
callbacks, and per-frame is exactly what we do not have.

The real hook is **`eframe::create_native`** (`eframe/src/lib.rs:328`, documented
from `:281`,
re-exported with `EframeWinitApplication` at `lib.rs:198` and
`EframePumpStatus` at `:202`, both gated on
`any(feature = "glow", feature = "wgpu_no_default_features")`; this workspace
enables `eframe/wgpu`, which implies `wgpu_no_default_features`
— `eframe-0.34.3/Cargo.toml` features `wgpu = ["wgpu_no_default_features", …]`).
The caller builds the `EventLoop<UserEvent>` and drives it:

```rust
let event_loop = EventLoop::<eframe::UserEvent>::with_user_event().build()?;
let mut app = eframe::create_native(name, options, creator, &event_loop);
event_loop.run_app(&mut app)?;
```

`EframeWinitApplication` is a public `ApplicationHandler<UserEvent>`
(`run.rs:479-524`) forwarding every callback to `WinitAppWrapper`, so it can be
**wrapped** by an outer handler of ours. `run_and_return` is `true` in both
`NativeOptions::default()` (`epi.rs:488`) and hard-coded in `create_wgpu`
(`run.rs:465`), so exit semantics do not move.

**Verdict: FEASIBLE and necessary, but not sufficient alone.** It is the
wakeup, not the dispatch. Ownership of the event loop (`create_native`) is a
**precondition** — there is no other route to a proxy in this eframe version.

### (a′) Synthesise `WindowEvent::RedrawRequested` — REJECTED

From an outer handler we could call
`inner.window_event(el, window_id, WindowEvent::RedrawRequested)`, which
`WinitAppWrapper::window_event` (`run.rs:362-364`) turns straight into
`run_ui_and_paint`. Technically available, and it would run full frames on a
parked surface.

Rejected: it forces wgpu to acquire and present a swapchain image for a surface
the compositor is not consuming. Under FIFO present that can block the **main
thread** inside `get_current_texture`, converting a stalled dispatcher into a
hung process — strictly worse than today, and worse in a way the escape hatches
cannot report. **NOT MEASURED** (no build this wave); listed as design
conservatism, and it is why the recommendation refuses to force a render rather
than trying one.

### (b) Timer-driven drain

**Shape.** No new thread. Once the outer handler exists (a), it can tighten
`ControlFlow` to `WaitUntil(now + tick)` in `about_to_wait`, so the loop wakes
on a timer even with nothing pending.

**Feasibility.** `poll_events_with_timeout` honours `WaitUntil` through the
calloop timeout (§3.2), and the "reduce spurious wake-ups" short-circuit
(`mod.rs:311-318`) is skipped whenever a timeout is set, so the iteration —
and `about_to_wait` — really runs. **It cannot park**, because nothing about
it consults `FrameCallbackState`.

**Why it should not be the primary mechanism.** A timer is a floor on latency
(one tick) and a floor on idle CPU. This is exactly what `app.rs:534-537`
already is, one layer down, and that heartbeat is the thing masking six known
bugs (§2.7). **Recommended role: a slow safety net (e.g. 250 ms) that only arms
while `awaiting_gui() > 0`, not a general clock.**

**A separate thread is not viable** and should be named as such: the drain
needs `&mut` access to state eframe owns on the main thread (§5), so any
"drain thread" reduces to a cross-thread lock over the controller, which is
§7's rejected shape.

### (c) Partial extraction — grow the MCP-readable snapshot

**Shape.** Extend `McpReadSnapshot` (`mcp_bridge.rs:28-38`) beyond its five
payloads and route more tools through `cheap_read`, so a park degrades reads to
"stale but answered" instead of "hung".

**Feasibility: high, and it is the only candidate that needs no event-loop
change.** The publish site already exists (`app/mcp.rs:112-131`), rate-limited
to 500 ms, and the fallback envelope already distinguishes snapshot from live
(`mcp_server.rs:204-237`).

**But its ceiling is low, and honestly stated it is a mitigation, not a fix:**

- It covers **no-argument reads only**. Of the 62 frame-door tools, the
  parameterised reads (`narrate_toolpath`, `get_cut_trace`,
  `get_toolpath_diagnostics`, `inspect_spans`, `get_generation_debug_trace`,
  `get_toolpath_params`, …) cannot be pre-rendered — the argument space is
  unbounded, and B-1 measured single responses in the megabytes
  (`GLV2_CRASH_CAPTURE.md` §5).
- It covers **no mutation**, and **no deferred completion**. The actual G-LV.1
  incident was a `generate_all` round handoff (§2.6). A snapshot cannot advance
  a fixpoint.
- It makes every covered read **stale by up to 500 ms plus the park duration**
  and marks it so, which is correct behaviour and still not an answer.

**Verdict: FEASIBLE, keep it, insufficient as the whole answer.** It is the
right NO-GO consolation prize (§9).

### Composition

(a) is the wakeup, (b) is the safety net, (c) is the degradation path. None is
a dispatcher. **The dispatcher is a fourth piece that (a) makes possible:** an
outer `ApplicationHandler` that owns a handle to the app and drains from
`about_to_wait`. That is the recommendation.

---

## 5. Recommendation — "host wrapper + off-frame pump"

Five changes. The ordering constraint between them is in §8.

### C1 — own the event loop (`lib.rs`, ~40 lines)

Replace `eframe::run_native` (`lib.rs:28-32`) with the `create_native` form
above. `create_proxy()` on the loop yields the `Send` proxy for the MCP thread.
No fork, no patch, public API, both cfg-gates satisfied by `eframe/wgpu`.

### C2 — share the app through a main-thread cell, not the controller

The naive move — `Rc<RefCell<AppController>>` — would rewrite the ~169
`self.controller.…` sites in `app/mcp.rs` alone and every one in `app/*.rs`,
because those helpers take `&mut self` and a `RefMut` cannot be held across
them. **Share `RsCamApp` instead:**

```rust
struct AppCell(Rc<RefCell<RsCamApp>>);          // host side + proxy side
struct RsCamAppProxy(Rc<RefCell<RsCamApp>>);    // what eframe owns

impl eframe::App for RsCamAppProxy {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        if let Ok(mut app) = self.0.try_borrow_mut() { app.ui(ui, frame); }
    }
}
```

Diff: **one ~15-line proxy type. Zero changes to any existing `self.controller`
site.** `eframe::App` requires neither `Send` nor `Sync`, and `AppCreator`'s
closure runs on the main thread, so an `Rc` captured there is sound.

*Bootstrap.* `RsCamApp` is built inside the `AppCreator` closure, which eframe
runs on `resumed` — after `run_app` starts. The host takes a
`Rc<RefCell<Option<Rc<RefCell<RsCamApp>>>>>` latch, the closure fills it, the
host picks it up on its first `about_to_wait`. Before that there is nothing to
pump, which is correct.

*Re-entrancy, and why it cannot panic.* Both borrow sites use
**`try_borrow_mut`**, never `borrow_mut` — so the `panic` lint is satisfied
structurally rather than by argument. The host's rule is: **forward to the
inner eframe handler first, take the borrow only after it returns, never hold
it across an inner call.** That matters because eframe can paint synchronously
from inside `check_redraw_requests` (`run.rs:221-224`). A lost `try_borrow_mut`
means a frame is already running, and the frame runs the same pump at its top
(C3) — so a lost race costs nothing.

### C3 — the off-frame pump

Extract from `draw_frame` the steps that need no `eframe::Frame`:

```rust
impl RsCamApp {
    pub(crate) fn off_frame_pump(&mut self) {           // needs no `&mut Frame`
        self.controller.drain_compute_results();        // app.rs:506
        self.drain_mcp_requests(&self.egui_ctx);        // app.rs:509
        self.pump_internal_events();                    // see C4
        self.pump_mcp_gui_screenshot(&self.egui_ctx);   // app.rs:515 (arms only)
        self.controller.process_auto_regen();           // app.rs:784
        self.end_mcp_frame();
    }
}
```

`RsCamApp` gains one field, `egui_ctx: egui::Context` — it already receives
`cc.egui_ctx.clone()` at `app.rs:76`. `egui::Context` is `Clone + Send + Sync`.

The pump is called from **two** places: `RsCamHost::about_to_wait` (off-frame)
and the top of `draw_frame` (unchanged in-frame order). It is idempotent and
near-free when the channel is empty, so the non-MCP path does not regress.

Because §2.4 shows no MCP handler touches `camera`, `viewport_rect` or
`eframe::Frame`, **no handler moves and no `McpRequestKind` arm changes.** That
is the whole reason this is a 5-file change rather than a rewrite.

### C4 — split the `AppEvent` queue by class, compiler-checked

The one genuinely invasive piece, and the one that makes generation work
off-frame. `RsCamApp::handle_events` (`app/input.rs:8-427`) claims ~40 variants
that need `RsCamApp` — camera (`:40,41,42,69`), `SwitchWorkspace` (`:72`),
`SimStep*`/`SimJumpTo*` which set `self.pending_checkpoint_load`
(`:98,106,114,125`), export/file-dialog arms — and delegates **everything else**
to `self.controller.handle_internal_event(other)` (`:426`), which is
`impl AppController` and therefore pump-safe. That "everything else" includes
`GenerateToolpath`, `GenerateAll`, `RunSimulation`, `RunCollisionCheck` — i.e.
the events MCP actually pushes for work (`app/mcp.rs:3832,3885,3905`, and the
ladder at `controller/events/compute.rs:1250-1360`).

`pump_internal_events` drains the queue, dispatches the internal class
immediately, and **re-queues the app class in order** for the next frame.
Classification lives in **one exhaustive `AppEvent::needs_app(&self) -> bool`
with no `_` arm**, so a new variant fails to compile until it is classified —
"miss nothing or don't compile", and it removes the drift hazard of keeping a
second list in sync with `handle_events`.

MCP pushes only eight app-class events: `SwitchWorkspace` from
`AddAlignmentPin` (`app/mcp.rs:271`), `SetSetupFace` (`:295`),
`SetToolpathParam` (`:398`), `RunSimulation` (`:637`) and `SetUiView` (`:4217`);
and `SimJumpToMove`/`Start`/`End` (`:4398,4409,4421`). All eight are
presentation, not computation — see §6.

*Residual risk, stated:* cross-class ordering can change (an app-class event
pushed before an internal one now executes after it). Within each class,
ordering is preserved. **A cheaper variant is offered at Checkpoint M
(question M-3): don't split — leave `handle_events` frame-only.** That keeps
ordering byte-identical but leaves generation frame-coupled, i.e. it does not
fix the incident.

### C5 — a wakeup that survives a park

`EmbeddedCamServer` gains the proxy. `wake_gui` (`mcp_server.rs:129-132`)
becomes: record `sent`; `ctx.request_repaint()` (unchanged, so the visible-
window path and the `every_mcp_enqueue_requests_a_repaint` sentry are
untouched); **and** `proxy.send_event(…)`.

The ping must be inert to eframe. Send `UserEvent::RequestRepaint` with
`cumulative_pass_nr: u64::MAX` and have `RsCamHost::user_event` **swallow**
that sentinel instead of forwarding — otherwise eframe would set
`ControlFlow::Poll` and busy-spin. (Forwarding is also safe: `run.rs:329-332`
classifies a mismatched pass number as outdated and returns
`EventResult::Wait`. Swallowing is preferred because it is explicit.)

Delivery guarantee: calloop channel → `dispatched_events = true` →
`single_iteration` → `about_to_wait` → pump. **No compositor, no dedup, no
frame.**

### What this predicts, and what it deletes

- The `generate_all` round handoff (§2.6) advances without a frame — the
  incident's exact failure.
- 62 frame-door tools become event-loop-door tools. The remaining genuine
  frame dependency is **one tool** (§6).
- The 100 ms heartbeat (`app.rs:534-537`) becomes deletable. Its own comment
  says it exists because "the cross-thread `request_repaint()` does not
  reliably wake a sleeping winit loop" — precisely §3.4. Deleting it removes a
  permanent 10 Hz wakeup from every MCP session **and unmasks the six drivers**
  (§2.7). Sequencing in §8.
- `FrameLoopBeat`, the standing repaint and the snapshot fallback **all stay**.
  They stop being the only defence and become the honest report of a window
  that is not rendering — which is still a true and useful thing to say.

---

## 6. The honest boundary — what still needs a rendered frame

| call | needs a frame? | contract under the new architecture |
|---|---|---|
| `screenshot_simulation`, `screenshot_toolpath` | **No.** CPU compositors / HTML (`app/mcp.rs:3939,3941,3965,4025,4045`) | answer from the pump, unchanged output |
| `screenshot_gui` | **Yes.** `ViewportCommand::Screenshot` → `egui::Event::Screenshot` one to two frames later (`app/mcp.rs:4068-4151`, `app.rs:487-497`) | see below |
| `set_ui_view` | **No to apply, yes to see.** Mutates workspace/selection and pushes `SwitchWorkspace` (`app/mcp.rs:4197-4240`) | applies from the pump; response gains `visible_on_next_frame` |
| `sim_scrub_toolpath`, `sim_jump_to_*` | **No to apply, yes to see.** State only (`app/mcp.rs:4398-4470`); `pending_checkpoint_load` is a flag the next frame consumes | same |
| everything else (64 tools) | No | pump |

**`screenshot_gui` on a window that is not rendering: refuse, with the
mechanism named.** Do not block (today it hangs — the pending slot is armed and
the pump never fires). Do not wake-then-render (§4 (a′): risks a hung main
thread). The pump arms the capture, then reads `FrameLoopBeat::is_parked()`
(`mcp_bridge.rs:255-262`, threshold `PARKED_FRAME_LOOP = 2 s` at `:78`); if
parked it resolves the oneshot immediately with a typed refusal that names the
compositor gate and the `WAYLAND_DISPLAY`-unset remedy, and clears the slot.
A visible window is unaffected. This is the one place the new architecture is
**deliberately less capable than a wish** and says so.

**`visible_on_next_frame` is a promise about pixels, not about state.** These
calls return `applied: true` and, when `frame_loop.healthy` is false,
`visible_on_next_frame: false` with the same remedy text. An agent that then
calls `screenshot_gui` gets the refusal above rather than a stale image
presented as current — which is the failure mode worth engineering against.

---

## 7. `Controller` ownership — respected, not worked around

The ledger records `AppController` as "main-thread-owned and not `Send`-shared".
Two precise statements, because they are not the same claim:

1. **What is certainly true, and what binds the design.** `AppController` is
   owned by value by `RsCamApp` (`app.rs:17`), which eframe owns behind
   `Box<dyn App>` inside `WinitAppWrapper` (`run.rs:78-82`) with **no
   accessor**. Nothing outside the event loop can reach it. That is an
   ownership fact, independent of any auto-trait, and it is why §4 (b)'s
   "separate drain thread" is not viable.
2. **What I could NOT establish without a build.** Whether `AppController` is
   literally `!Send`. Reading for it: there is **no `Rc` or `RefCell` anywhere
   in `crates/rs_cam_viz/src` outside tests**, and no `TextureHandle`/`wgpu`
   handle on `AppState` (`state/mod.rs:29-…`) or the controller
   (`controller.rs:64-79`). `ThreadedComputeBackend` (`compute/worker.rs:488-496`)
   holds `Arc<LaneQueue<_>>`, `JoinHandle`s and an
   `mpsc::Receiver<ComputeMessage>` — `Send` but `!Sync`. So `AppController`
   is plausibly `Send + !Sync`. **NOT ESTABLISHED** — it would take a
   `fn assert_send<T: Send>()` and a compile.

**This matters, and it argues *for* the recommendation rather than against
it.** If the controller is in fact `Send`, then `Arc<Mutex<AppController>>`
would *compile* — and would still be the wrong answer, for reasons the type
system does not express:

- it re-serialises exactly what A/M12 unpicked — a frame render would block on
  a long `narrate_toolpath` and vice versa (`mcp_server.rs:56-65` names this as
  the original defect);
- a handler could mutate `session` **mid-frame**, so a panel would render half
  of one state and half of another, and `pending_upload` could be observed
  between set and consume (`controller.rs:147-152`);
- egui re-entrancy makes the lock order between `Context` internals and the
  controller unstateable.

**The recommendation needs no `Send` and no `unsafe`.** `Rc<RefCell<RsCamApp>>`
never leaves the main thread; the only cross-thread objects remain the ones
that are already `Send + Sync` by construction — the mpsc `Sender`,
`egui::Context`, `GenerationControl` (`controller.rs:166-172`), `McpReadCache`,
and the new `EventLoopProxy`. The workspace `unsafe_code` deny stands
untouched.

---

## 8. Risk register and migration order for B-4

### Risks

| # | risk | why it is bounded | check B-4 must run |
|---|---|---|---|
| R1 | `RefCell` double-borrow | both sites are `try_borrow_mut`; the host never holds a borrow across an inner call | a unit sentry that calls the host's `about_to_wait` re-entrantly and asserts no panic and no lost work |
| R2 | Cross-class `AppEvent` reordering (C4) | within-class order preserved; only 8 MCP push sites are app-class, all presentational (§6) | `wizard_e2e` (11) + `viz` lib (238) green; a sentry pinning the order of one mixed sequence |
| R3 | Deleting the 100 ms heartbeat **unmasks all six drivers** (§2.7) | they are pre-existing, listed, and none is MCP-reachable | **must not ship alone** — see migration step 6 |
| R4 | `create_native` diverges from `run_native` (exit path, `run_and_return`) | `run_and_return: true` in both (`epi.rs:488`, `run.rs:465`) | a manual launch + clean quit, non-MCP and MCP |
| R5 | The 12 `mcp_escape_hatches` sentries | none touches the event loop; they drive `EmbeddedCamServer` against a stalled fake GUI (`tests/mcp_escape_hatches.rs:97,107`) | all 12 green at every step; `every_mcp_enqueue_requests_a_repaint` must stay green **and** gain a sibling asserting the **proxy send**, because §3.4 shows the repaint call can be made and not delivered |
| R6 | Fingerprints move | nothing in scope touches generation, geometry or feeds — the same argument `66c8f16` made | the standard fingerprint suite unchanged; a moved fingerprint is a STOP under §0 rule 2 |
| R7 | Wayland present-blocking | avoided by construction — §4 (a′) is rejected and §6 refuses rather than renders | none; do not "just try it" |
| R8 | The ~1 s floor does not improve | see below | pre/post census with the same instrument |

### R8, stated carefully, because it is the wave's headline number

B-1 measured **50 of 64 calls between 0.94 s and 1.08 s regardless of payload**
(`GLV2_CRASH_CAPTURE.md` §5). Two things about that number:

- **It is not a harness artefact.** I checked: `rpc()` timestamps the response
  in the **reader thread** (`artifacts/b1/mcp_stdio_client.py:208-219`,
  `"wall_s": ts - t0`), so the loop's own `queue.get(timeout=min(remaining, 1.0))`
  cannot contaminate it.
- **Its mechanism is NOT ESTABLISHED, and it is not the heartbeat.** The app's
  own designed floor is **100 ms** (`app.rs:534-537`), so ~900 ms is
  unattributed. Two calls in the same census answered in **0.133 s** and
  **0.172 s**, which proves it is not a hard floor. The census also ran with
  the GUI as a **`gdb --batch` inferior** (`artifacts/b1/README.md:16-29`) —
  an uncontrolled variable that was never subtracted.

**Pre-registered bar for B-4:** re-measure the floor **without gdb**, on the
same fixture, before and after. Predicted post value is
`calloop wakeup + handler time`, i.e. sub-millisecond dispatch. **If the
unattributed 900 ms survives the decoupling, the decoupling did not cause it
and the remaining latency is a separate finding — do not fold it into this
row's claim.**

### Migration order

1. **C1 + C2 only.** Own the loop, insert the proxy type, host forwards every
   callback and does nothing else. Behaviourally a no-op. Gate: full viz suite,
   12 escape hatches, a manual launch/quit. **Ship this alone**; it is the
   piece that can break startup, and it should break in isolation if it breaks.
2. **C3.** `off_frame_pump` extracted and called from *both* sites. Still
   behaviourally a no-op (the pump only ever runs inside a frame until step 4).
3. **C5.** Proxy into `wake_gui`; host swallows the sentinel. First observable
   change: enqueues wake the loop. Add the delivery sentry (R5).
4. **Host drains from `about_to_wait`.** First real decoupling. Bar: on
   Wayland, with the window hidden, `list_toolpaths` and `narrate_toolpath`
   answer **live** (not `served_from: "snapshot"`). This is B-1's unexercised
   Wayland run, now runnable.
5. **C4.** Internal-event class off-frame. Bar: with the window hidden, a
   `generate_all` fixpoint completes — the incident, reproduced and fixed, with
   the pre-fix hang captured first.
6. **The six drivers + heartbeat deletion, together, last.** Each with its own
   before/after per the B-4 charter, and each reproduced in a **`--mcp`-less**
   session (the audit's trap: the heartbeat masks all six). **Never delete the
   heartbeat before the six are fixed.**
7. **`screenshot_gui` refusal + `visible_on_next_frame`** (§6), with a sentry
   for the refusal path.

Docs owed at the end: CLAUDE.md's MCP section (the workflow currently tells
agents the window must be visible), plan §0 rule 10, and the startup warning
text at `bin/main.rs:52-62`.

---

## 9. The NO-GO shape, stated plainly

The plan says: *"if the design is unconvincing, the documented x11 remedy
stands and the row stays open — say so rather than forcing it."* Taking that
seriously:

**The design's weakest joint is C4** (event-class split), and its most
invasive is **C1** (owning eframe's event loop). If the operator judges either
unacceptable, the fallback is not a lesser version of this — it is:

- **keep `run_native`**, keep `FrameLoopBeat` and the standing repaint as the
  reporting layer, keep **`WAYLAND_DISPLAY` unset** as the operating
  instruction, and **leave G-LV.1 open**; and, optionally,
- **land candidate (c) alone** — grow `McpReadSnapshot` so more no-argument
  reads degrade to stale-but-answered. That is a genuine improvement to agent
  experience under a park, it needs no event-loop change, and it **does not
  fix the incident**: it cannot advance a fixpoint, cannot serve a
  parameterised read, and cannot apply a mutation.

Stated as a bar rather than a feeling: **if steps 1–5 do not produce a live
`narrate_toolpath` on a hidden Wayland window and a completing hidden-window
`generate_all`, the change should be reverted rather than kept as partial
progress** — a half-decoupled dispatcher is a third state for an agent to
reason about, and G-LV.1's whole lesson is that unreportable states are the
expensive kind.

---

## 10. Checkpoint M package

### Options

| | option | cost | risk | what it fixes |
|---|---|---|---|---|
| **M-A** | **Recommended.** C1+C2+C3+C4+C5, migrated in the 7 steps of §8 | ~5 files, ~250 net lines, no handler rewritten; 7 gated steps | R1–R8; the real ones are R2 (event ordering) and R3 (unmasking six known bugs) | dispatch **and** deferred completion off the paint path; 68 of 69 tools frame-independent; deletes the 10 Hz heartbeat |
| **M-B** | C1+C2+C3+C5 **without** C4 | ~4 files, ~180 lines; ordering byte-identical | R1, R4–R8 only | reads and simple mutations decouple; **generation does not** — the incident stands |
| **M-C** | Candidate (c) alone — grow the snapshot | ~1 file, ~80 lines | very low | reads degrade to stale-but-answered; nothing else. **Mitigation, not fix** |
| **M-D** | NO-GO. Keep the reporting layer + `WAYLAND_DISPLAY` unset; row stays open | zero | zero | nothing; the operating instruction remains "keep the window visible or run XWayland" |

**Recommendation: M-A**, migrated in order, with an explicit stop after step 4
if the hidden-window live-read bar is not met.

### Questions for the operator

- **M-1 — Do we own eframe's event loop?** `eframe::create_native` +
  `run_app` replaces `run_native` (`lib.rs:28`). This is the programme's one
  architectural change and every other piece depends on it (§4 (a): there is no
  other route to an `EventLoopProxy` in eframe 0.34.3). Yes / no.
- **M-2 — Which option: M-A, M-B, M-C or M-D?**
- **M-3 — If M-A: accept C4's cross-class `AppEvent` reordering?** Only 8 MCP
  push sites are app-class and all are presentational (§6), classification is
  compiler-enforced, but ordering between the two classes is not preserved.
  Accept / require a stricter scheme / take M-B instead.
- **M-4 — `screenshot_gui` on a non-rendering window: refuse (recommended),
  or keep blocking?** §6. Refusal names the mechanism and the remedy; it is
  the one place we become *less* capable than the wish, and it changes an
  observable contract.
- **M-5 — May the 100 ms heartbeat be deleted?** Only in the same wave as the
  six-driver fixes (§8 step 6, R3). If the answer is "not yet", the heartbeat
  stays and the idle-CPU win is deferred — the decoupling still works.
- **M-6 — Ruling on the ~1 s floor.** It is unattributed, it is not the
  heartbeat, and it was measured under `gdb` (R8). Confirm B-4 must re-measure
  without gdb before and after, and that a surviving 900 ms is booked as a
  **separate** finding rather than a failure of this row.

### What this wave could not establish

- **Whether `AppController` is literally `!Send`** (§7). Needs one compile. The
  design does not depend on the answer; the ledger's wording might.
- **The mechanism of the ~1 s dispatch floor** (R8). Needs a live run.
- **Whether painting a parked Wayland surface blocks in wgpu** (§4 (a′)). Not
  measured; candidate rejected on conservatism, and nothing in the
  recommendation depends on it being true.
- **The unpark trigger** for the ~9.5 h park that blocked A-1/A-2. Never
  observed. Recorded as unknown.
