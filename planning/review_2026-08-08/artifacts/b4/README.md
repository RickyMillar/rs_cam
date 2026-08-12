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
