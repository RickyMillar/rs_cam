#!/usr/bin/env python3
"""N-2 present-mode A/B — does a non-FIFO present mode unpark a minimised GUI?

Checkpoint N-2 (BINDING, 2026-08-13) authorised a *measurement* wave: B-4
proved that under a minimised Wayland park the main thread blocks in `poll()`
on ONE fd with an INFINITE timeout, below winit, attributed to the Wayland/Mesa
WSI waiting on a FIFO buffer release a minimised surface never gives. This
script varies exactly one thing — `RS_CAM_PRESENT_MODE`, the rig-only lever
added to `rs_cam_viz::run` — and measures the consequence.

It **extends** B-4's rig rather than replacing it: the transport is B-1's
`mcp_stdio_client.McpProcess`, the park lever is B-4's
`RS_CAM_MINIMIZE_AFTER_FRAMES`, and the cheap/frame-door call lists are
imported from B-4's `floor_census` so both waves measure the same population.

Never point this at the operator's live GUI or the operator's project file: it
launches its own process and takes a project path the caller has copied into
scratch.

Three phases, one per question:

    --phase park     the A/B itself. Park the window, then ask: does
                     `about_to_wait` still run (`pumps` climbing)? are proxy
                     pings answered (`wakeups` vs `pumps`)? do PURE FRAME-DOOR
                     calls answer or hang? does `generate_all` round-trip?
                     `/proc/<pid>/syscall` is sampled throughout, because the
                     blocked/unblocked distinction is a syscall fact and
                     everything else is an inference from it.

    --phase pacing   the interactive cost. `RS_CAM_MINIMIZE_AFTER_FRAMES=<big>`
                     is ALSO a continuous-repaint driver (`app.rs:942-952`
                     requests a repaint on every frame while it counts down),
                     so it doubles as the closest headless proxy for an
                     interactive drag. Frame timestamps are reconstructed from
                     `frame_loop.last_frame_age_s` sampled off the OFF-LOOP
                     door, so the instrument never itself asks for a frame.

    --phase idle     idle CPU. No MCP traffic at all during the window; utime +
                     stime read from `/proc/<pid>/stat` at both ends.

Usage:
    python3 present_mode_ab.py --phase park --mode mailbox --wayland \
        --binary <path>/rs_cam_gui --log-dir .../measurements/park_mailbox \
        --project /tmp/n2/sample.toml --minimize-after 250
"""

import argparse
import json
import os
import statistics
import sys
import threading
import time

_HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(_HERE, "..", "b1"))
sys.path.insert(0, os.path.join(_HERE, "..", "b4"))

from mcp_stdio_client import McpProcess  # noqa: E402
from floor_census import CHEAP_CALLS, FRAME_DOOR_CALLS  # noqa: E402

CLK_TCK = os.sysconf("SC_CLK_TCK")


# --------------------------------------------------------------- /proc probes
def read_syscall(pid):
    """`/proc/<pid>/syscall` for the MAIN thread.

    B-4's signature of the present block is `7 <ptr> 0x1 0xffffffff`: syscall 7
    (`poll`) on ONE fd with an infinite timeout. calloop's own poll is an
    `epoll_wait`/`ppoll` over many fds with a bounded timeout, so the arity and
    the timeout word are what separate "blocked in the WSI" from "waiting in
    the event loop, ready to be woken".
    """
    try:
        with open(f"/proc/{pid}/syscall") as f:
            return f.read().strip()
    except OSError as e:  # noqa: BLE001
        return f"<unreadable: {e}>"


def read_wchan(pid):
    try:
        with open(f"/proc/{pid}/wchan") as f:
            return f.read().strip()
    except OSError:  # noqa: BLE001
        return ""


def read_cpu_ticks(pid):
    """(utime, stime) in ticks for the whole process, from `/proc/<pid>/stat`."""
    try:
        with open(f"/proc/{pid}/stat") as f:
            raw = f.read()
    except OSError:
        return None
    # The comm field can contain spaces and parentheses; split after the last ')'.
    tail = raw[raw.rfind(")") + 2:].split()
    if len(tail) < 13:
        return None
    return int(tail[11]), int(tail[12])


def read_threads(pid):
    try:
        return sorted(int(t) for t in os.listdir(f"/proc/{pid}/task"))
    except OSError:
        return []


class SyscallSampler(threading.Thread):
    """Samples the main thread's syscall while an RPC may be blocked."""

    def __init__(self, pid, out_path, period=0.25):
        super().__init__(daemon=True)
        self.pid = pid
        self.out_path = out_path
        self.period = period
        self.rows = []
        self._stopper = threading.Event()
        self._label = "start"

    def mark(self, label):
        self._label = label

    def run(self):
        while not self._stopper.wait(self.period):
            self.rows.append(
                {
                    "t": time.time(),
                    "label": self._label,
                    # /proc/<pid>/syscall with pid == tgid is the MAIN thread,
                    # which is the one that owns the event loop and the present.
                    "syscall": read_syscall(self.pid),
                    "wchan": read_wchan(self.pid),
                }
            )

    def halt(self):
        self._stopper.set()
        self.join(timeout=3.0)
        with open(self.out_path, "w") as f:
            for r in self.rows:
                f.write(json.dumps(r) + "\n")

    def summary(self, label=None):
        rows = [r for r in self.rows if label is None or r["label"] == label]
        counts = {}
        for r in rows:
            counts[r["syscall"]] = counts.get(r["syscall"], 0) + 1
        return sorted(counts.items(), key=lambda kv: -kv[1])


# ------------------------------------------------------------------- helpers
def body(r):
    try:
        return json.loads(r["msg"]["result"]["content"][0]["text"])
    except Exception:  # noqa: BLE001
        return None


def frame_loop(proc, timeout=20.0):
    """Read `frame_loop` off the OFF-LOOP door so the probe cannot unpark."""
    t0 = time.time()
    r = proc.call_tool("generation_status", {}, timeout=timeout)
    b = body(r) or {}
    fl = b.get("frame_loop", {}) if isinstance(b, dict) else {}
    fl = dict(fl)
    fl["_sampled_at"] = time.time()
    fl["_probe_wall_s"] = time.time() - t0
    return fl


def served_from(r):
    if "__transport__" in r:
        return r["__transport__"]
    b = body(r)
    if b is None:
        return "unparsed"
    if not isinstance(b, dict):
        return "live"
    return b.get("served_from", "live")


def launch(args, extra_env=None):
    env = {"RS_CAM_PRESENT_MODE": args.mode}
    # wgpu logs the mode it NEGOTIATED for the two Auto* rules at info level
    # (`wgpu-core-29.0.3/src/device/resource.rs:4996`). An explicit mode the
    # surface does not support is a hard error there, not a fallback — so a
    # clean launch on an explicit mode IS the negotiation record.
    env["RUST_LOG"] = "info,wgpu_core=info,wgpu_hal=info"
    if extra_env:
        env.update(extra_env)
    proc = McpProcess(
        os.path.abspath(args.binary),
        os.path.abspath(args.workdir),
        args.log_dir,
        mode="direct",
        extra_env=env,
        keep_wayland=args.wayland,
    )
    proc.start()
    return proc


def wait_for_park(proc, sampler, timeout, poll=1.0):
    """Poll until the frame counter stops moving AND the beat calls it parked.

    Returns (parked: bool, trail: list). A park is *proved* per measurement —
    two static reads separated by more than `PARKED_FRAME_LOOP` — never
    inferred from the fact that we asked for one.
    """
    trail = []
    deadline = time.time() + timeout
    last = None
    static_since = None
    while time.time() < deadline:
        fl = frame_loop(proc)
        trail.append(fl)
        frames = fl.get("frames")
        if frames is not None and frames == last:
            if static_since is None:
                static_since = time.time()
            elif time.time() - static_since >= 3.0 and fl.get("healthy") is False:
                return True, trail
        else:
            static_since = None
        last = frames
        time.sleep(poll)
    return False, trail


# --------------------------------------------------------------------- phases
def phase_park(args, proc, sampler, out):
    rows = []
    proc.initialize(timeout=30.0)
    if args.project:
        # Load BEFORE the park: a load is a frame-door call and would be the
        # first casualty, which would tell us nothing new.
        r = proc.call_tool(
            "load_project", {"path": os.path.abspath(args.project)}, timeout=180.0
        )
        rows.append({"call": "load_project", "wall_s": r["wall_s"], "phase": "setup",
                     "served_from": served_from(r)})
        print(f"  load_project {r['wall_s']:.3f}s ({served_from(r)})")

    sampler.mark("pre_park")
    time.sleep(args.settle)
    parked, trail = wait_for_park(proc, sampler, args.park_timeout)
    rows.append({"call": "park_wait", "phase": "park", "parked": parked,
                 "trail": trail, "wall_s": 0.0})
    print(f"  PARK: {'reached' if parked else 'NOT REACHED within timeout'}"
          f" — frames={trail[-1].get('frames') if trail else None}"
          f" healthy={trail[-1].get('healthy') if trail else None}")
    sampler.mark("parked")

    before = frame_loop(proc)
    for i in range(args.repeats):
        for name, a in CHEAP_CALLS + FRAME_DOOR_CALLS:
            t0 = time.time()
            r = proc.call_tool(name, a, timeout=args.timeout)
            fl = frame_loop(proc)
            rows.append({
                "call": name, "repeat": i, "phase": "census",
                "wall_s": r["wall_s"], "served_from": served_from(r),
                "timed_out": r.get("__transport__") == "timeout",
                "frames": fl.get("frames"), "pumps": fl.get("pumps"),
                "wakeups": fl.get("wakeups"), "healthy": fl.get("healthy"),
                "syscall": read_syscall(proc._gui_pid),
                "t": t0,
            })
    after = frame_loop(proc)
    rows.append({"call": "frames_probe", "phase": "before", "wall_s": 0.0, **before})
    rows.append({"call": "frames_probe", "phase": "after", "wall_s": 0.0, **after})

    # The Lane A capture obligation: a GUI screenshot is the ONE call B-3's
    # §6 lists as genuinely needing a rendered frame, and A-1/A-2 are blocked
    # on it. Under a park that still paints, it should simply work.
    if args.screenshot:
        sampler.mark("screenshot")
        t0 = time.time()
        r = proc.call_tool("screenshot_gui", {"path": os.path.abspath(args.screenshot)},
                           timeout=args.timeout * 4)
        b = body(r)
        rows.append({
            "call": "screenshot_gui", "phase": "lane_a_bar",
            "wall_s": time.time() - t0,
            "timed_out": r.get("__transport__") == "timeout",
            "reply": b if isinstance(b, dict) else str(b)[:800],
            "bytes_on_disk": (os.path.getsize(args.screenshot)
                              if os.path.exists(args.screenshot) else 0),
        })
        print(f"  screenshot_gui: wall={time.time() - t0:.3f}s "
              f"bytes={rows[-1]['bytes_on_disk']} reply={str(b)[:160]}")

    # The step-5 bar B-4 could not reach: a full generate_all round-trip on a
    # window that is not painting. It needs MULTI-ROUND handoffs, not just one
    # dispatch, so it is the strongest single statement this A/B can make.
    if args.generate:
        sampler.mark("generate_all")
        t0 = time.time()
        r = proc.call_tool(
            "generate_all",
            {"fixpoint": True, "simulation_resolution_mm": args.sim_resolution,
             "timeout_s": int(args.generate_timeout)},
            timeout=args.generate_timeout + 120.0,
        )
        b = body(r)
        fl = frame_loop(proc)
        rows.append({
            "call": "generate_all", "phase": "step5_bar",
            "wall_s": time.time() - t0,
            "timed_out": r.get("__transport__") == "timeout",
            "reply": b if isinstance(b, dict) else str(b)[:2000],
            "frames": fl.get("frames"), "pumps": fl.get("pumps"),
            "wakeups": fl.get("wakeups"), "healthy": fl.get("healthy"),
        })
        status = (b or {}).get("status") if isinstance(b, dict) else None
        print(f"  generate_all: wall={time.time() - t0:.1f}s status={status} "
              f"generated={(b or {}).get('generated') if isinstance(b, dict) else '-'} "
              f"rounds={(b or {}).get('rounds') if isinstance(b, dict) else '-'}")
    return rows


def phase_pacing(args, proc, sampler, out):
    """Frame-interval distribution under a CONTINUOUS-repaint workload.

    `RS_CAM_MINIMIZE_AFTER_FRAMES=<big>` requests a repaint every frame while
    it counts down (`app.rs:942-952`), which is the only headless way to hold
    the app in the state an interactive drag puts it in. The window is NOT
    parked during this phase — the countdown must not reach zero, so pass a
    count larger than `frames_per_s * window_s`.
    """
    rows = []
    proc.initialize(timeout=30.0)
    time.sleep(args.settle)
    pid = proc._gui_pid
    t_end = time.time() + args.window
    c0 = read_cpu_ticks(pid)
    samples = []
    while time.time() < t_end:
        fl = frame_loop(proc, timeout=10.0)
        age = fl.get("last_frame_age_s")
        if age is not None:
            samples.append((fl["_sampled_at"] - age, fl.get("frames")))
    c1 = read_cpu_ticks(pid)

    # Reconstruct distinct frame start times. Identical `frames` counters are
    # the same frame observed twice; a new counter value with a new derived
    # timestamp is a new frame.
    seen = {}
    for t, n in samples:
        if n is None:
            continue
        seen.setdefault(n, t)
    ordered = [seen[n] for n in sorted(seen)]
    intervals = [b - a for a, b in zip(ordered, ordered[1:]) if 0 < b - a < 2.0]
    rows.append({
        "phase": "pacing", "mode": args.mode, "window_s": args.window,
        "samples": len(samples), "frames_observed": len(ordered),
        "cpu_ticks": (None if not (c0 and c1) else
                      [c1[0] - c0[0], c1[1] - c0[1]]),
        "clk_tck": CLK_TCK,
        "intervals_ms": [round(x * 1000, 4) for x in intervals],
    })
    if intervals:
        ms = sorted(x * 1000 for x in intervals)
        print(f"  frames_observed={len(ordered)} over {args.window}s "
              f"(~{len(ordered) / args.window:.1f}/s)")
        print(f"  interval ms: min={ms[0]:.2f} p50={statistics.median(ms):.2f} "
              f"p95={ms[int(0.95 * (len(ms) - 1))]:.2f} max={ms[-1]:.2f}")
    if c0 and c1:
        cpu = (c1[0] - c0[0] + c1[1] - c0[1]) / CLK_TCK
        print(f"  CPU during pacing window: {cpu:.2f} s over {args.window} s "
              f"= {100 * cpu / args.window:.1f}% of one core")
    return rows


def phase_idle(args, proc, sampler, out):
    """Idle CPU with NO MCP traffic in the window at all."""
    rows = []
    proc.initialize(timeout=30.0)
    time.sleep(args.settle)
    pid = proc._gui_pid
    fl0 = frame_loop(proc)
    c0 = read_cpu_ticks(pid)
    t0 = time.time()
    time.sleep(args.window)
    c1 = read_cpu_ticks(pid)
    elapsed = time.time() - t0
    fl1 = frame_loop(proc)
    cpu = None if not (c0 and c1) else (c1[0] - c0[0] + c1[1] - c0[1]) / CLK_TCK
    frames = (fl1.get("frames") or 0) - (fl0.get("frames") or 0)
    rows.append({
        "phase": "idle", "mode": args.mode, "window_s": elapsed,
        "cpu_s": cpu, "cpu_pct_of_core": None if cpu is None else 100 * cpu / elapsed,
        "frames_in_window": frames, "fps": frames / elapsed,
        "threads": len(read_threads(pid)),
        "utime_stime_ticks": None if not (c0 and c1) else
        [c1[0] - c0[0], c1[1] - c0[1]],
    })
    print(f"  idle {elapsed:.1f}s: cpu={cpu:.2f}s "
          f"({100 * cpu / elapsed:.2f}% of one core), frames={frames} "
          f"({frames / elapsed:.1f}/s)")
    return rows


PHASES = {"park": phase_park, "pacing": phase_pacing, "idle": phase_idle}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--phase", choices=sorted(PHASES), required=True)
    ap.add_argument("--mode", default="auto_vsync",
                    help="RS_CAM_PRESENT_MODE value (the ONE variable)")
    ap.add_argument("--binary", required=True)
    ap.add_argument("--log-dir", required=True)
    ap.add_argument("--workdir", default=os.getcwd())
    ap.add_argument("--project", default=None, help="scratch COPY of a .toml")
    ap.add_argument("--wayland", action="store_true",
                    help="keep WAYLAND_DISPLAY set (the park platform)")
    ap.add_argument("--minimize-after", type=int, default=None)
    ap.add_argument("--repeats", type=int, default=3)
    ap.add_argument("--timeout", type=float, default=15.0)
    ap.add_argument("--settle", type=float, default=4.0)
    ap.add_argument("--park-timeout", type=float, default=90.0)
    ap.add_argument("--window", type=float, default=20.0)
    ap.add_argument("--generate", action="store_true")
    ap.add_argument("--screenshot", default=None,
                    help="after the park, ask for a GUI screenshot at this path")
    ap.add_argument("--generate-timeout", type=float, default=600.0)
    ap.add_argument("--sim-resolution", type=float, default=0.6)
    args = ap.parse_args()

    os.makedirs(args.log_dir, exist_ok=True)
    extra = {}
    if args.minimize_after is not None:
        extra["RS_CAM_MINIMIZE_AFTER_FRAMES"] = str(args.minimize_after)
    print(f"=== phase={args.phase} mode={args.mode} "
          f"{'wayland' if args.wayland else 'x11/xwayland'} "
          f"minimize_after={args.minimize_after} ===")

    proc = launch(args, extra)
    sampler = SyscallSampler(proc._gui_pid,
                             os.path.join(args.log_dir, "syscall.jsonl"))
    sampler.start()
    rows = []
    try:
        rows = PHASES[args.phase](args, proc, sampler, args.log_dir)
    finally:
        sampler.halt()
        proc.kill()

    meta = {
        "phase": args.phase, "mode_requested": args.mode,
        "platform": "wayland" if args.wayland else "x11/xwayland",
        "minimize_after": args.minimize_after,
        "binary": os.path.abspath(args.binary),
        "when": time.strftime("%Y-%m-%dT%H:%M:%S"),
    }
    out = os.path.join(args.log_dir, "result.json")
    with open(out, "w") as f:
        json.dump({"meta": meta, "rows": rows}, f, indent=1)

    # The syscall census is the decisive evidence for --phase park.
    print("  syscall census (main thread):")
    for label in ("pre_park", "parked", "generate_all"):
        s = sampler.summary(label)
        if s:
            print(f"    [{label}] " + "; ".join(f"{k} ×{v}" for k, v in s[:4]))
    print(f"  wrote {out}")


if __name__ == "__main__":
    main()
