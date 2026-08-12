#!/usr/bin/env python3
"""B-4b — does the SHIPPED `--mcp` path clear the park, and can you see it?

Checkpoint O (BINDING, 2026-08-13) ruled the flip: a GUI launched with `--mcp`
requests `PresentMode::AutoNoVsync`; a plain interactive launch keeps
`AutoVsync`. N-2 measured that under the **rig lever**
(`RS_CAM_PRESENT_MODE=auto_no_vsync`). This script measures the same bars on
the **shipped path** — the lever is deliberately NOT set, so what is exercised
is `present_mode::decide(mcp_mode = true, env = None)` and nothing else. Those
are different code paths and only one of them ships.

It also checks the half of O-2 that no A/B could check, because the field did
not exist when N-2 ran: that the **negotiated** mode is observable from
`generation_status`. `AutoNoVsync`'s fallback rule always ends at `Fifo` so it
can never fail to configure, which means a surface offering neither `Immediate`
nor `Mailbox` silently restores the G-LV.1 park — and every other signal in the
response looks identical either way.

Five pre-registered bars, printed as PASS/FAIL and exited on:

    O2-observable  frame_loop.present_mode.negotiated is a STRING, and
                   negotiated_known_from says it was observed rather than
                   inferred from the request.
    O1-requested   ... .requested == "AutoNoVsync" and requested_source names
                   the --mcp default. (Proves the ruling is what ran, not the
                   lever.)
    step4-live     with the window MINIMISED: every cheap read answers LIVE
                   (`served_from` absent/"live", never "snapshot") and both
                   PURE FRAME-DOOR calls — which have no fallback and therefore
                   hang rather than degrade — answer under `--live-bar` seconds.
    step5-fixpoint with the window MINIMISED: `generate_all` with fixpoint
                   returns ok, `rounds >= 2`, `errors: []`. This is the
                   2026-08-07 incident's exact shape: the SIMULATE-ROUND HANDOFF
                   is what stranded, so a single-round pass would not test it.
    step7-capture  with the window MINIMISED: `screenshot_gui` writes a real
                   PNG. Under M-4 the call refuses when the window is genuinely
                   not rendering; under Mailbox it must simply work, and a
                   refusal here would be a false negative on a live window.

**The park is proved, never assumed.** `wait_for_park` (imported from N-2's
rig) requires the frame counter to be static across reads separated by more
than `PARKED_FRAME_LOOP` before it will call a window parked. Under the flip
the expected outcome is that it is NOT reached — the minimised window keeps
painting — and that is a PASS for this script, recorded explicitly rather than
read as a failed measurement. `--expect-park` inverts it for the control arm.

**Control arm.** `--present-mode fifo` sets the rig lever back on and
reproduces the pre-flip state on the same binary, same project, same calls.
Run it whenever you want the before to the after.

Never point this at the operator's live GUI or the operator's project file: it
launches its own process and takes a project path the caller has copied into
scratch.

Usage:
    python3 shipped_flip_check.py --binary target/debug/rs_cam_gui \\
        --log-dir .../measurements/shipped_flip --project /tmp/b4b/sample.toml \\
        --minimize-after 250

    # the before, on the same binary:
    python3 shipped_flip_check.py ... --present-mode fifo --expect-park \\
        --log-dir .../measurements/control_fifo
"""

import argparse
import json
import os
import sys
import time

_HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(_HERE, "..", "b1"))
sys.path.insert(0, os.path.join(_HERE, "..", "n2"))

from mcp_stdio_client import McpProcess  # noqa: E402
from floor_census import CHEAP_CALLS, FRAME_DOOR_CALLS  # noqa: E402
from present_mode_ab import (  # noqa: E402
    SyscallSampler,
    body,
    frame_loop,
    read_syscall,
    served_from,
    wait_for_park,
)


def raw_text(r):
    """The reply's text content, WITHOUT the JSON parse `body()` does.

    M-4's refusal is prose, not a JSON object — deliberately: it is addressed to
    whoever reads the failure. `body()` returns `None` for it, which reads
    identically to a call that answered nothing, so a rig checking the refusal
    must look at the text.
    """
    try:
        return r["msg"]["result"]["content"][0]["text"]
    except Exception:  # noqa: BLE001
        return ""


class Bars:
    """Pre-registered bars, evaluated once each, printed in order."""

    def __init__(self):
        self.rows = []

    def record(self, name, ok, detail):
        self.rows.append({"bar": name, "pass": bool(ok), "detail": detail})
        print(f"  [{'PASS' if ok else 'FAIL'}] {name}: {detail}")
        return ok

    def all_passed(self):
        return all(r["pass"] for r in self.rows)


def launch(args):
    env = {
        # Everything wgpu says about the surface, so the negotiated mode is in
        # the stderr log too and the in-process capture can be cross-checked
        # against wgpu's own words rather than trusted on its own.
        "RUST_LOG": "info,wgpu_core=info,wgpu_hal=info",
    }
    if args.present_mode:
        # Control arm only. Absent — the default — the shipped `--mcp` rule
        # decides, which is the entire point of this script.
        env["RS_CAM_PRESENT_MODE"] = args.present_mode
    if args.minimize_after is not None:
        env["RS_CAM_MINIMIZE_AFTER_FRAMES"] = str(args.minimize_after)
    proc = McpProcess(
        os.path.abspath(args.binary),
        os.path.abspath(args.workdir),
        args.log_dir,
        mode="direct",
        extra_env=env,
        keep_wayland=not args.x11,
    )
    proc.start()
    return proc


def check_present_mode_block(proc, args, bars, rows):
    """O-2 and O-1, off the OFF-LOOP door so a parked loop cannot hide them."""
    fl = frame_loop(proc, timeout=30.0)
    pm = (fl or {}).get("present_mode") or {}
    rows.append({"call": "present_mode", "phase": "o2", "block": pm, "wall_s": 0.0})

    negotiated = pm.get("negotiated")
    known_from = pm.get("negotiated_known_from") or ""
    bars.record(
        "O2-observable",
        isinstance(negotiated, str) and "NOT OBSERVED" not in known_from,
        f"negotiated={negotiated!r} known_from={known_from!r}",
    )

    expect_requested = "AutoNoVsync" if not args.present_mode else None
    if expect_requested:
        bars.record(
            "O1-requested",
            pm.get("requested") == expect_requested
            and "--mcp default" in (pm.get("requested_source") or ""),
            f"requested={pm.get('requested')!r} source={pm.get('requested_source')!r}",
        )
    else:
        print(
            f"  [ -- ] O1-requested: skipped, control arm forced "
            f"RS_CAM_PRESENT_MODE={args.present_mode}"
        )
    if pm.get("park_hazard"):
        print(f"  NOTE park_hazard reported: {pm['park_hazard'][:120]}...")
    return pm


def census(proc, args, bars, rows):
    """step4-live: cheap reads live, frame-door reads answered not hung."""
    snapshots = 0
    slowest_door = 0.0
    door_timeouts = 0
    for i in range(args.repeats):
        for name, a in CHEAP_CALLS + FRAME_DOOR_CALLS:
            t0 = time.time()
            r = proc.call_tool(name, a, timeout=args.timeout)
            fl = frame_loop(proc)
            row = {
                "call": name,
                "repeat": i,
                "phase": "census",
                "wall_s": r["wall_s"],
                "served_from": served_from(r),
                "timed_out": r.get("__transport__") == "timeout",
                "frames": fl.get("frames"),
                "pumps": fl.get("pumps"),
                "wakeups": fl.get("wakeups"),
                "healthy": fl.get("healthy"),
                "syscall": read_syscall(proc._gui_pid),
                "t": t0,
            }
            rows.append(row)
            if row["served_from"] == "snapshot":
                snapshots += 1
            if (name, a) in FRAME_DOOR_CALLS or name in {c[0] for c in FRAME_DOOR_CALLS}:
                slowest_door = max(slowest_door, r["wall_s"])
                door_timeouts += 1 if row["timed_out"] else 0

    bars.record(
        "step4-live",
        snapshots == 0 and door_timeouts == 0 and slowest_door < args.live_bar,
        f"snapshot-served reads={snapshots} frame-door timeouts={door_timeouts} "
        f"slowest frame-door={slowest_door:.4f}s (bar {args.live_bar}s)",
    )


def wait_for_idle(proc, args, rows, timeout):
    """Let the GUI's OWN auto-regeneration finish before measuring dispatch.

    **Measured 2026-08-13, and it is not a park effect.** Loading this project
    leaves toolpaths marked stale with `auto_regen` set, so
    `Controller::process_auto_regen` submits them on its own about half a second
    later — and an `adaptive3d` on a 220k-triangle terrain runs for a while.
    Issuing `generate_all` on top of that re-submits the same toolpath and the
    in-flight job comes back as `"Back Rough: generation cancelled"`, `rounds: 1`,
    `generated: 0`. Reproduced identically on a window that was NEVER minimised
    (`nomin/result.json`), which is what rules the park out: the rig was racing
    the GUI, not the compositor.

    So the wait is part of the instrument, not a workaround. Whatever it waits
    for is reported in the row, so a run that spent 40 s here says so.
    """
    t0 = time.time()
    last = None
    while time.time() - t0 < timeout:
        st = frame_loop(proc, timeout=20.0)
        # `frame_loop()` returns the frame_loop block; the lane flag lives beside
        # it, so ask the door itself.
        r = proc.call_tool("generation_status", {}, timeout=20.0)
        b = body(r)
        busy = b.get("busy") if isinstance(b, dict) else None
        last = {"busy": busy, "frames": (st or {}).get("frames")}
        if busy is False:
            break
        time.sleep(1.0)
    waited = time.time() - t0
    rows.append({"call": "wait_for_idle", "phase": "step5_setup", "wall_s": waited,
                 "final": last})
    print(f"  pre-generate settle: lane idle after {waited:.1f}s ({last})")


def step5_fixpoint(proc, args, bars, rows):
    """The 2026-08-07 incident's own shape: the simulate-round handoff."""
    t0 = time.time()
    r = proc.call_tool(
        "generate_all",
        {
            "fixpoint": True,
            "simulation_resolution_mm": args.sim_resolution,
            "timeout_s": int(args.generate_timeout),
        },
        timeout=args.generate_timeout + 120.0,
    )
    b = body(r)
    fl = frame_loop(proc)
    wall = time.time() - t0
    reply = b if isinstance(b, dict) else {"raw": str(b)[:2000]}
    rows.append(
        {
            "call": "generate_all",
            "phase": "step5_bar",
            "wall_s": wall,
            "timed_out": r.get("__transport__") == "timeout",
            "reply": reply,
            "frames": fl.get("frames"),
            "pumps": fl.get("pumps"),
            "wakeups": fl.get("wakeups"),
            "healthy": fl.get("healthy"),
        }
    )
    rounds = reply.get("rounds")
    bars.record(
        "step5-fixpoint",
        bool(reply.get("ok"))
        and isinstance(rounds, int)
        and rounds >= 2
        and reply.get("errors") == [],
        f"ok={reply.get('ok')} generated={reply.get('generated')} rounds={rounds} "
        f"simulations={reply.get('simulations')} errors={reply.get('errors')} "
        f"awaiting_prior_stock={reply.get('awaiting_prior_stock')} in {wall:.1f}s",
    )


def step7_capture(proc, args, bars, rows):
    """M-4's other side: on a window that IS rendering, the call just works."""
    target = os.path.abspath(args.screenshot)
    if os.path.exists(target):
        os.remove(target)
    t0 = time.time()
    r = proc.call_tool("screenshot_gui", {"path": target}, timeout=args.timeout * 4)
    b = body(r)
    size = os.path.getsize(target) if os.path.exists(target) else 0
    wall = time.time() - t0
    rows.append(
        {
            "call": "screenshot_gui",
            "phase": "step7_bar",
            "wall_s": wall,
            "timed_out": r.get("__transport__") == "timeout",
            "reply": b if isinstance(b, dict) else str(b)[:800],
            "bytes_on_disk": size,
        }
    )
    reply_text = raw_text(r) or str(b)
    rows[-1]["reply_text"] = reply_text[:1200]
    if args.expect_refusal:
        # M-4's own case, and the only state that can exercise it live: a window
        # that has stopped painting while its event loop still runs. A minimised
        # X11 window is exactly that (N-2: `frames` static, `pumps` climbing).
        # Under a FIFO-blocked Wayland park nothing runs at all, so the refusal
        # cannot be delivered there and this arm would measure the old hang.
        bars.record(
            "step7-refusal",
            "REFUSED" in reply_text
            and size == 0
            and r.get("__transport__") != "timeout"
            and wall < args.timeout,
            f"answered in {wall:.2f}s, {size} bytes on disk, "
            f"refused={'REFUSED' in reply_text}: {reply_text[:220]}",
        )
        return
    bars.record(
        "step7-capture",
        size > 0 and r.get("__transport__") != "timeout",
        f"{size} bytes in {wall:.2f}s reply={str(b)[:160]}",
    )


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--binary", required=True)
    ap.add_argument("--log-dir", required=True)
    ap.add_argument("--workdir", default=".")
    ap.add_argument("--project", default=None, help="scratch COPY of a project .toml")
    ap.add_argument("--repeats", type=int, default=3)
    ap.add_argument("--timeout", type=float, default=15.0)
    ap.add_argument("--live-bar", type=float, default=1.0)
    ap.add_argument("--settle", type=float, default=3.0)
    ap.add_argument("--park-timeout", type=float, default=25.0)
    ap.add_argument("--minimize-after", type=int, default=250)
    ap.add_argument("--sim-resolution", type=float, default=1.0)
    ap.add_argument("--generate-timeout", type=float, default=300.0)
    ap.add_argument("--screenshot", default=None)
    ap.add_argument(
        "--present-mode",
        default=None,
        help="CONTROL ARM ONLY: force RS_CAM_PRESENT_MODE. Leave unset to "
        "exercise the shipped --mcp rule, which is what this script is for.",
    )
    ap.add_argument(
        "--x11",
        action="store_true",
        help="unset WAYLAND_DISPLAY so winit picks X11/XWayland. A minimised "
        "X11 window stops painting but leaves the event loop running, which is "
        "the ONE state that can exercise M-4's refusal live.",
    )
    ap.add_argument(
        "--expect-refusal",
        action="store_true",
        help="step 7 must REFUSE rather than capture (M-4). Pair with --x11 "
        "--expect-park.",
    )
    ap.add_argument(
        "--expect-park",
        action="store_true",
        help="the control arm expects the window to park; the shipped path "
        "expects it NOT to.",
    )
    args = ap.parse_args()

    os.makedirs(args.log_dir, exist_ok=True)
    if args.screenshot is None:
        args.screenshot = os.path.join(args.log_dir, "screenshot_gui.png")

    bars = Bars()
    rows = []
    proc = launch(args)
    sampler = SyscallSampler(
        proc._gui_pid, os.path.join(args.log_dir, "syscall.jsonl")
    )
    sampler.start()
    try:
        proc.initialize(timeout=30.0)
        if args.project:
            # Before the park: a load is a frame-door call and would be the
            # first casualty, which would tell us nothing new.
            r = proc.call_tool(
                "load_project", {"path": os.path.abspath(args.project)}, timeout=180.0
            )
            rows.append(
                {"call": "load_project", "phase": "setup", "wall_s": r["wall_s"]}
            )
            print(f"  load_project {r['wall_s']:.3f}s")

        check_present_mode_block(proc, args, bars, rows)

        sampler.mark("pre_park")
        time.sleep(args.settle)
        parked, trail = wait_for_park(proc, sampler, args.park_timeout)
        sampler.mark("parked" if parked else "no_park")
        rows.append(
            {"call": "park_wait", "phase": "park", "parked": parked, "trail": trail,
             "wall_s": 0.0}
        )
        bars.record(
            "park-state",
            parked == args.expect_park,
            f"park {'reached' if parked else 'NOT reached'}; expected "
            f"{'park' if args.expect_park else 'no park'}. "
            f"frames={trail[-1].get('frames') if trail else None} "
            f"healthy={trail[-1].get('healthy') if trail else None}",
        )

        census(proc, args, bars, rows)
        if args.project:
            wait_for_idle(proc, args, rows, timeout=args.generate_timeout)
            step5_fixpoint(proc, args, bars, rows)
        step7_capture(proc, args, bars, rows)
    finally:
        sampler.halt()
        out = os.path.join(args.log_dir, "result.json")
        with open(out, "w") as f:
            json.dump({"args": vars(args), "bars": bars.rows, "rows": rows}, f, indent=1)
        print(f"  wrote {out}")
        proc.kill()

    print("\nALL BARS PASSED" if bars.all_passed() else "\nAT LEAST ONE BAR FAILED")
    return 0 if bars.all_passed() else 1


if __name__ == "__main__":
    sys.exit(main())
