#!/usr/bin/env python3
"""A-8i / Checkpoint P-4 — capture the optimizer results card.

Drives an **isolated** `rs_cam_gui --mcp` instance (never the operator's) over
the B-1 stdio rig, opens the per-toolpath Optimize modal on a small pocket
fixture, and writes a full-window PNG.

    python3 p4_screenshot.py --log-dir /tmp/a8i/run \\
        --project planning/review_2026-08-08/artifacts/a8i/a8i_pocket.toml \\
        --binary target/debug/rs_cam_gui \\
        --out planning/review_2026-08-08/artifacts/a8i/optimize_card.png

`--skip-optimize` captures the same card without running a search — the two
stamp blocks are populated on **every** outcome including refusals, which is
the case worth proving on its own.
"""

import argparse
import json
import os
import sys
import time

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "b1"))
from mcp_stdio_client import McpProcess  # noqa: E402


def text_of(resp):
    """Pull the tool text out of an rmcp tools/call reply."""
    try:
        return resp["result"]["content"][0]["text"]
    except (KeyError, IndexError, TypeError):
        return json.dumps(resp)[:2000]


def call(gui, name, args=None, timeout=120.0, quiet=False):
    t0 = time.time()
    resp = gui.call_tool(name, args or {}, timeout=timeout)
    body = text_of(resp)
    if not quiet:
        print(f"[{time.time() - t0:6.2f}s] {name} -> {body[:400]}", flush=True)
    return body


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--log-dir", required=True)
    ap.add_argument("--project", required=True)
    ap.add_argument("--binary", default="target/debug/rs_cam_gui")
    ap.add_argument("--out", required=True)
    ap.add_argument("--toolpath-index", type=int, default=0)
    ap.add_argument("--resolution", type=float, default=0.5)
    ap.add_argument("--optimize-timeout", type=float, default=1800.0)
    ap.add_argument("--skip-optimize", action="store_true")
    ap.add_argument("--width", type=int, default=1600)
    ap.add_argument("--height", type=int, default=1000)
    ap.add_argument("--shots", type=float, nargs="+",
                    default=[8.0, 20.0, 40.0, 60.0],
                    help="sleep intervals (s) between successive captures")
    args = ap.parse_args()

    os.makedirs(args.log_dir, exist_ok=True)
    out = os.path.abspath(args.out)
    os.makedirs(os.path.dirname(out), exist_ok=True)

    gui = McpProcess(
        binary=os.path.abspath(args.binary),
        workdir=os.getcwd(),
        log_dir=args.log_dir,
        mode="direct",
    )
    gui.start()
    try:
        gui.initialize(timeout=120.0)
        print("initialized", flush=True)

        call(gui, "load_project", {"path": os.path.abspath(args.project)})
        call(gui, "list_toolpaths", {})
        call(gui, "generate_toolpath", {"index": args.toolpath_index},
             timeout=600.0)
        call(gui, "run_simulation", {"resolution": args.resolution}, timeout=900.0)

        # NOTE: the MCP `optimize_toolpath` tool returns the outcome as JSON
        # and does NOT stash it in `AppState::optimize_modal`
        # (`app/mcp.rs::mcp_optimize_toolpath`). The card is populated only by
        # `AppEvent::OpenOptimizeModal`, which `set_ui_view` pushes — and the
        # controller then runs the search on the frame loop. So `set_ui_view`
        # returns fast and the *screenshot* is what waits behind the search.
        if not args.skip_optimize:
            call(gui, "optimize_toolpath", {"index": args.toolpath_index},
                 timeout=args.optimize_timeout, quiet=True)

        call(gui, "set_ui_view",
             {"modal": "optimize_modal", "toolpath_index": args.toolpath_index})

        # `OpenOptimizeModal` opens the card in `Loading` and hands the search
        # to the Optimize worker lane (`controller/events/mod.rs:241`), so a
        # single shot right after `set_ui_view` captures the spinner. There is
        # no MCP read for that lane's state, so capture a short series and
        # pick the frame that carries the result. Stated rather than hidden:
        # this is polling, not synchronisation.
        stem, ext = os.path.splitext(out)
        elapsed = 0.0
        for step in args.shots:
            time.sleep(step)
            elapsed += step
            shot = f"{stem}_t{int(elapsed)}s{ext}"
            call(gui, "screenshot_gui",
                 {"path": shot, "width": args.width, "height": args.height},
                 timeout=600.0)
            print(f"  -> {shot} exists={os.path.exists(shot)} "
                  f"bytes={os.path.getsize(shot) if os.path.exists(shot) else 0}",
                  flush=True)
    finally:
        gui.kill()


if __name__ == "__main__":
    main()
