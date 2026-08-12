#!/usr/bin/env python3
"""B-4 dispatch-floor census — the same measurement B-1 took, WITHOUT gdb.

R8 of `DISPATCH_DECOUPLING_DESIGN.md` §8: B-1 measured 50 of 64 MCP calls
between 0.94 s and 1.08 s regardless of payload, but every one of those runs
had the GUI as a `gdb --batch` inferior. Checkpoint M-6 requires the floor be
re-measured *without* gdb, before and after the decoupling, with the same
instrument on both sides.

This script is deliberately small: launch one isolated `rs_cam_gui --mcp`,
issue a fixed list of CHEAP calls (no generation, no simulation, no large
payload), and print the wall-clock distribution. A cheap call's wall time is
dispatch latency plus a few microseconds of JSON, so what it measures is the
floor and nothing else.

Never point this at the operator's live GUI or the operator's project file:
it launches its own process and takes a project path that the caller is
expected to have copied into scratch.

Usage:
    python3 floor_census.py --binary target/debug/rs_cam_gui \
        --log-dir /tmp/b4/floor_pre --project /tmp/b4/sample.toml --repeats 5

    # window-hidden Wayland run (the step-4 bar):
    python3 floor_census.py ... --wayland --hidden
"""

import argparse
import json
import os
import statistics
import sys
import time

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "b1"))

from mcp_stdio_client import McpProcess  # noqa: E402

# Cheap, no-argument-ish calls. `generation_status` is the OFF-LOOP door (it
# never touches the frame loop) and is included as the control: whatever floor
# it shows is transport + tokio, not dispatch.
CHEAP_CALLS = [
    ("generation_status", {}),
    ("project_summary", {}),
    ("list_toolpaths", {}),
    ("inspect_stock", {}),
    ("inspect_machine", {}),
    ("inspect_model", {}),
]

# Pure FRAME-DOOR tools: no snapshot fallback, no off-loop shortcut. Under a
# parked frame loop these do not degrade, they HANG — which makes them the
# sharpest probe available. A `wall_s` at the timeout is the pre-fix answer;
# a millisecond reading is the post-fix one.
FRAME_DOOR_CALLS = [
    ("get_operation_schema", {"operation_type": "pocket"}),
    ("list_tools", {}),
]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--binary", required=True)
    ap.add_argument("--log-dir", required=True)
    ap.add_argument("--project", default=None, help="scratch COPY of a project .toml")
    ap.add_argument("--repeats", type=int, default=5)
    ap.add_argument("--timeout", type=float, default=30.0)
    ap.add_argument("--wayland", action="store_true", help="keep WAYLAND_DISPLAY set")
    ap.add_argument(
        "--hidden",
        action="store_true",
        help="probe frame_loop.frames either side of the census, to prove "
        "whether frames were being produced while calls were answered",
    )
    ap.add_argument(
        "--minimize-after",
        type=int,
        default=None,
        help="RS_CAM_MINIMIZE_AFTER_FRAMES — park the window after N frames. "
        "Only parks under Wayland; on X11 redraws are client-driven.",
    )
    ap.add_argument("--settle", type=float, default=3.0)
    args = ap.parse_args()

    os.makedirs(args.log_dir, exist_ok=True)
    extra_env = {}
    if args.minimize_after is not None:
        extra_env["RS_CAM_MINIMIZE_AFTER_FRAMES"] = str(args.minimize_after)
    proc = McpProcess(
        os.path.abspath(args.binary),
        os.getcwd(),
        args.log_dir,
        mode="direct",
        extra_env=extra_env or None,
        keep_wayland=args.wayland,
    )
    proc.start()
    rows = []
    try:
        proc.initialize(timeout=args.timeout)
        if args.project:
            r = proc.call_tool(
                "load_project", {"path": os.path.abspath(args.project)},
                timeout=args.timeout,
            )
            rows.append({"call": "load_project", "wall_s": r["wall_s"], "phase": "setup"})
        time.sleep(args.settle)

        if args.hidden:
            # The caller is responsible for having parked the window (see the
            # README). Record the frame counter before and after so the log
            # proves frames were NOT being produced during the census.
            rows.append(_frames_probe(proc, args.timeout, "before"))

        for i in range(args.repeats):
            for name, a in CHEAP_CALLS + FRAME_DOOR_CALLS:
                r = proc.call_tool(name, a, timeout=args.timeout)
                row = {
                    "call": name,
                    "wall_s": r["wall_s"],
                    "repeat": i,
                    "phase": "census",
                    "served_from": _served_from(r),
                }
                if args.hidden:
                    # Sample the paint-frame counter beside EVERY census call,
                    # off the frame door, so "frames were not being produced
                    # while these answered" is a measurement and not an
                    # inference from two endpoints.
                    row.update(_frame_loop(proc, args.timeout))
                rows.append(row)

        if args.hidden:
            rows.append(_frames_probe(proc, args.timeout, "after"))
    finally:
        proc.kill()

    out = os.path.join(args.log_dir, "floor.jsonl")
    with open(out, "w") as f:
        for row in rows:
            f.write(json.dumps(row) + "\n")

    _report(rows, out)


def _body(r):
    """The tool's JSON payload, or `None` when the call did not answer."""
    try:
        return json.loads(r["msg"]["result"]["content"][0]["text"])
    except Exception:  # noqa: BLE001
        return None


def _served_from(r):
    """`snapshot` means the frame loop lost the 750 ms race — NOT a live read."""
    if "__transport__" in r:
        return r["__transport__"]
    body = _body(r)
    if body is None:
        return "unparsed"
    if not isinstance(body, dict):
        # `list_toolpaths` answers with a bare array when it is live.
        return "live"
    return body.get("served_from", "live")


def _frame_loop(proc, timeout):
    """`frame_loop` off the OFF-LOOP door, so the probe cannot itself unpark."""
    r = proc.call_tool("generation_status", {}, timeout=timeout)
    body = _body(r) or {}
    fl = body.get("frame_loop", {}) if isinstance(body, dict) else {}
    return {
        "frames": fl.get("frames"),
        "pumps": fl.get("pumps"),
        "wakeups": fl.get("wakeups"),
        "healthy": fl.get("healthy"),
        "last_frame_age_s": fl.get("last_frame_age_s"),
    }


def _frames_probe(proc, timeout, label):
    row = {"call": "frames_probe", "phase": label, "wall_s": 0.0}
    row.update(_frame_loop(proc, timeout))
    return row


def _report(rows, out):
    census = [r for r in rows if r.get("phase") == "census"]
    print(f"wrote {out} ({len(rows)} rows)")
    for name, _ in CHEAP_CALLS + FRAME_DOOR_CALLS:
        vals = [r["wall_s"] for r in census if r["call"] == name]
        if not vals:
            continue
        stale = sum(1 for r in census
                    if r["call"] == name and r.get("served_from") == "snapshot")
        print(
            f"  {name:20s} n={len(vals):2d} "
            f"min={min(vals):.4f} med={statistics.median(vals):.4f} "
            f"max={max(vals):.4f}" + (f"  SNAPSHOT×{stale}" if stale else "")
        )
    allv = [r["wall_s"] for r in census]
    if allv:
        print(
            f"  {'ALL':20s} n={len(allv):2d} min={min(allv):.4f} "
            f"med={statistics.median(allv):.4f} max={max(allv):.4f}"
        )
    for r in rows:
        if r.get("call") == "frames_probe":
            print(
                f"  frames({r['phase']}): {r['frames']} pumps={r['pumps']} "
                f"wakeups={r['wakeups']} "
                f"healthy={r['healthy']} last_frame_age_s={r['last_frame_age_s']}"
            )
    # The park proof: a census during which `frames` never moved is a census
    # answered without a single paint. A census where it moved is not evidence
    # of anything and must be re-run.
    seen = [r["frames"] for r in census if r.get("frames") is not None]
    if seen:
        pumps = [r["pumps"] for r in census if r.get("pumps") is not None]
        print(
            f"  PARK PROOF: frames {min(seen)}..{max(seen)} "
            f"({'STATIC — no paint during the census' if min(seen) == max(seen) else 'MOVED — window was drawing, re-run'})"
            + (f", pumps {min(pumps)}..{max(pumps)}" if pumps else "")
        )


if __name__ == "__main__":
    main()
