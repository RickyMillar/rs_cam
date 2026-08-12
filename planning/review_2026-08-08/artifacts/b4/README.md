# B-4 artifacts — dispatch decoupling (G-LV.1)

Rig and raw measurements for TD3 wave **B-4**, implementing
`../DISPATCH_DECOUPLING_DESIGN.md` under the Checkpoint M ruling (M-A).

## Never point any of this at the operator's live GUI

`floor_census.py` launches its **own** `rs_cam_gui --mcp` process. Pass a
scratch **copy** of a project file if you pass one at all; never
`planning/airrun_2026-06-01/wanaka.toml`.

## Files

| file | what it is |
|---|---|
| `floor_census.py` | the R8 / Checkpoint M-6 instrument: one isolated GUI, a fixed list of **cheap** MCP calls, wall-clock distribution per call. Reuses B-1's `../b1/mcp_stdio_client.py` transport so both sides of the before/after are literally the same client. **No gdb** — that is the entire point. |
| `measurements/` | raw `floor.jsonl` rows, one per call. |

## The measurement M-6 asked for

B-1 measured 50 of 64 calls at **0.94–1.08 s regardless of payload**
(`../GLV2_CRASH_CAPTURE.md` §5) with the GUI running as a `gdb --batch`
inferior. R8 pre-registered: re-measure without gdb, before and after, same
instrument, and book a surviving ~900 ms as a **separate** finding.

Run it as:

```sh
python3 floor_census.py --binary target/debug/rs_cam_gui \
    --log-dir planning/review_2026-08-08/artifacts/b4/measurements/pre_x11 \
    --repeats 8
```

`--wayland` keeps `WAYLAND_DISPLAY` set (the default unsets it, so winit picks
X11/XWayland — the documented remedy). `--hidden` adds a `frame_loop.frames`
probe either side of the census, so a run can **prove** whether frames were
being produced while the calls were answered; it does not itself park
anything — see below for how the window is parked.

## Parking a Wayland window, deterministically

Two routes were checked against winit 0.30.13 and only one exists:

- `ViewportBuilder::with_visible(false)` / `Window::set_visible(false)` —
  **dead end.** `winit .../wayland/window/mod.rs:253-255` is a literal no-op
  with the comment "Not possible on Wayland", and `is_visible()` returns
  `None` (`:258-260`).
- `ViewportCommand::Minimized(true)` → `Window::set_minimized(true)` —
  **works**, and is one-way: `.../wayland/window/mod.rs:436-444` refuses to
  un-minimize ("Unminimizing is ignored on Wayland"). A minimized surface
  stops receiving compositor frame callbacks, which is precisely the
  `FrameCallbackState::Requested` gate G-LV.1 dies on.

That one-way-ness is why the repro lever is a first-class, documented
environment variable rather than throwaway scaffolding: `RS_CAM_MINIMIZE_AFTER_FRAMES=<n>`
minimizes the window after `n` frames. It is how G-LV.1 is reproduced without
hiding or locking the operator's actual desktop, and the reason the 2026-08-07
incident's unpark trigger was never observed is that on Wayland there may not
be one the client can reach.

---

## Wave B-4b — the shipped flip, and the two bars it unblocked

`shipped_flip_check.py` is B-4b's instrument, executing Checkpoint O
(2026-08-13) and `../DISPATCH_DECOUPLING_DESIGN.md` §8 steps 5 and 7.

**Why a second script rather than reusing N-2's.** N-2 measured the present-mode
A/B through the **rig lever** (`RS_CAM_PRESENT_MODE=auto_no_vsync`). The flip
O-1 actually ruled is a `--mcp` **launch default**, and it lives on a different
code path — `present_mode::decide(mcp_mode = true, env = None)`. Only one of the
two ships. This script therefore sets **no** present-mode variable at all, and
its `O1-requested` bar exists to prove that: it fails if the mode came from the
lever. Everything else is imported — B-1's transport, B-4's park lever and call
lists, N-2's syscall sampler and park proof.

| bar | what it pins |
|---|---|
| `O2-observable` | `frame_loop.present_mode.negotiated` is a real observed string (O-2) |
| `O1-requested` | ...and the request was `AutoNoVsync` from the `--mcp` default, not the lever |
| `park-state` | the park is **proved** per run, and the expected outcome is stated up front (`--expect-park` for the arms that should park) |
| `step4-live` | every cheap read live, both pure frame-door calls answered |
| `step5-fixpoint` | `generate_all` fixpoint with `rounds >= 2` — the simulate-round handoff, which is what 2026-08-07 actually died on |
| `step7-capture` / `step7-refusal` | a rendering window captures; a genuinely parked one **refuses** (M-4) |

### `wait_for_idle` is part of the instrument, not a workaround

Loading the fixture leaves toolpaths stale with `auto_regen` set, so the GUI
submits them itself about half a second later. Issuing `generate_all` on top of
a running `adaptive3d` re-submits the same toolpath and the in-flight job comes
back as `"Back Rough: generation cancelled"`, `rounds: 1`, `generated: 0`. That
is **not** a park effect: it reproduced identically on a window that was never
minimised. The settle is measured and reported in the row.

### Arms

```sh
# the shipped path, Wayland, window minimised (the after)
python3 shipped_flip_check.py --binary target/debug/rs_cam_gui \
    --log-dir measurements/shipped_flip_wayland --project /tmp/b4b/cascade.toml \
    --repeats 2 --minimize-after 250 --sim-resolution 1.0 --generate-timeout 600

# M-4's refusal, which only a LOOP-ALIVE park can exercise (see below)
python3 shipped_flip_check.py --binary target/debug/rs_cam_gui \
    --log-dir measurements/step7_x11_park --x11 --expect-park --expect-refusal \
    --repeats 1 --minimize-after 150 --park-timeout 40 --timeout 30
```

### Why the refusal arm is X11 and not Wayland

M-4's refusal is delivered from the off-frame pump, which runs from the event
loop's `about_to_wait`. Under a **present-blocked** Wayland park the main thread
is below winit and that callback does not run either, so nothing can deliver a
refusal — that state is addressed by not being in it (O-1's flip). The state the
refusal is *for* is a window that has stopped painting while its event loop
still runs, and a minimised **X11/XWayland** window is exactly that: `frames`
static at 153, `healthy: false`, and all eight census calls still answered live
at 0.6 ms.

Incidentally measured and worth knowing: the XWayland surface negotiates
`AutoNoVsync` to **`Immediate`**, where the Wayland surface negotiates it to
`Mailbox`. Two different answers to the same request on the same machine, which
is the clearest possible argument for O-2 reporting the negotiated mode rather
than the requested one.

### `measurements/` from this wave

| directory | arm |
|---|---|
| `shipped_flip_wayland/` | the after: shipped `--mcp` path, Wayland, minimised. All six bars pass. Includes `screenshot_gui.png` — 788,881 bytes rendered off a minimised window. |
| `step7_x11_park/` | M-4 refusal, **green**: answered in 2.01 s naming the mechanism |
| `step7_x11_park_before/` | the same arm on the binary one commit earlier, **red**: 120.00 s, no reply, 0 bytes |
