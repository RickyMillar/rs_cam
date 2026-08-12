# N-2 artifacts — PresentMode A/B under the park rig (G-LV.1)

Rig and raw measurements for TD3 wave **N-2**, executing the Checkpoint N-2
ruling. The deliverable is `../PRESENT_MODE_AB.md`; everything here is its
evidence.

**A measurement wave. No default was changed.** The one production edit is
`RS_CAM_PRESENT_MODE`, a rig-only environment lever in `rs_cam_viz::run` whose
unset value is the shipped `AutoVsync`.

## Never point any of this at the operator's live GUI

`present_mode_ab.py` launches its **own** `rs_cam_gui --mcp` process. Pass a
scratch **copy** of a project file if you pass one at all; never
`planning/airrun_2026-06-01/wanaka.toml`.

## Why this is a new directory and not more files in `b4/`

B-4's `floor_census.py` answers one question — the dispatch floor — and answers
it well; this wave asks a different one and would have had to fork its argument
handling, its report and its park-detection to do it. So `present_mode_ab.py`
**imports** B-4's call lists and B-1's transport rather than copying them, and
the two rigs stay separately runnable. Nothing under `b4/` or `b1/` was edited.

## Files

| file | what it is |
|---|---|
| `present_mode_ab.py` | the A/B instrument. Three phases (`--phase park \| pacing \| idle`), one present mode per run (`--mode`), B-4's `RS_CAM_MINIMIZE_AFTER_FRAMES` as the park lever, `/proc/<pid>/syscall` sampled at 4 Hz throughout because the blocked/unblocked distinction is a syscall fact and everything else is an inference from it. |
| `measurements/<arm>/result.json` | per-call rows with `wall_s`, `served_from`, and the `frames`/`pumps`/`wakeups` counters sampled beside **every** call off the off-loop door. |
| `measurements/<arm>/syscall.jsonl` | the raw syscall samples, labelled by phase (`pre_park`, `parked`, `generate_all`, `screenshot`). |
| `measurements/<arm>/gui_stderr.log` | the GUI's own log, including the requested-mode line and — for the `immediate` arm — wgpu's verbatim list of the surface's supported present modes. |
| `measurements/SUMMARY.txt` | every arm reduced to one block; regenerate from `result.json` alone. |
| `evidence/screenshot_gui_minimised_mailbox.png` | a full 1400×900 GUI render served by a **minimised** window under Mailbox in 3.79 s. The same call under AutoVsync times out at 60 s with 0 bytes. |

## Reading the syscall column

- `7 … 0x1 0xffffffff` — `poll()` on **one** fd with an **infinite** timeout.
  This is the block: the main thread is below winit, inside the Wayland/Mesa
  present, and no `ApplicationHandler` callback can run.
- `281 … 0x400 0xffffffffffffffff` — `epoll_wait` on many fds. This is calloop,
  i.e. the event loop is *waiting* and can be woken. Not a block.
- `202` — `futex`. `running` — the sample landed while the thread was on-CPU.

## Running an arm

```sh
# the A/B itself (one mode per run; the mode is the ONLY variable)
python3 present_mode_ab.py --phase park --mode mailbox --wayland \
    --minimize-after 250 --repeats 3 \
    --binary <worktree>/target/debug/rs_cam_gui \
    --log-dir measurements/park_wayland_mailbox

# the fixpoint bar (load must complete BEFORE the park — raise --minimize-after)
python3 present_mode_ab.py --phase park --mode mailbox --wayland \
    --minimize-after 1500 --generate --sim-resolution 0.6 \
    --project /path/to/SCRATCH_COPY.toml ...

# interactive cost: idle (no traffic) and continuous repaint (lever beyond the window)
python3 present_mode_ab.py --phase idle   --mode mailbox --wayland --window 30 ...
python3 present_mode_ab.py --phase idle   --mode mailbox --wayland --window 30 --minimize-after 200000 ...
python3 present_mode_ab.py --phase pacing --mode mailbox --wayland --window 20 --minimize-after 200000 ...
```

Two traps worth stating, both hit during the wave:

1. **`--minimize-after` is also a continuous-repaint driver** (`app.rs:942-952`
   requests a repaint on every frame while it counts down). That is what makes
   `--phase pacing` and the "busy" idle arms possible — and it means a value
   *smaller* than the frames a phase needs will park the window mid-measurement.
2. **A park must be proved, not assumed.** `--phase park` polls until the frame
   counter is static across reads separated by more than `PARKED_FRAME_LOOP`
   *and* the beat reports `healthy:false`, and records `parked: false` honestly
   when that never happens — which is exactly what the non-FIFO arms report.
