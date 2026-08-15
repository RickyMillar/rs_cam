#!/usr/bin/env python3
"""G-EXPL-HIDDEN — capture the `NoSafeImprovement` optimizer card.

Same rig as A-8i's `p4_screenshot.py` (isolated `rs_cam_gui --mcp` over the
B-1 stdio client), with two differences that matter for THIS wave:

1. A-8i's refusal fixture produces `OutcomeKind::Skipped`, which is a
   DIFFERENT modal branch (`draw_refusal_section`) from the one this wave
   fixed. It cannot witness the fix. This driver uses
   `gexpl_deflection_pocket.toml` instead, whose long-stickout HSS tool in
   hard maple is meant to trip the pre-flight `DeflectionSetupLocked`
   refusal — a `NoSafeImprovement` whose narrative has an EMPTY headline
   and puts the whole prescription on `explanation`.
2. It PRINTS the `optimize_toolpath` outcome before capturing, so the run
   states on the record which outcome kind and reason it actually got.
   A capture of the wrong branch would prove nothing, and this is how the
   run tells you that before you read the PNG.

    python3 gexpl_screenshot.py --log-dir /tmp/gexpl/run \\
        --project planning/review_2026-08-08/artifacts/gexpl/gexpl_deflection_pocket.toml \\
        --out planning/review_2026-08-08/artifacts/gexpl/optimize_card.png
"""

import argparse
import json
import os
import sys
import time

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "b1"))
from mcp_stdio_client import McpProcess  # noqa: E402


def text_of(resp):
    try:
        return resp["result"]["content"][0]["text"]
    except (KeyError, IndexError, TypeError):
        return json.dumps(resp)[:2000]


def call(gui, name, args=None, timeout=120.0, head=400):
    t0 = time.time()
    resp = gui.call_tool(name, args or {}, timeout=timeout)
    body = text_of(resp)
    print(f"[{time.time() - t0:6.2f}s] {name} -> {body[:head]}", flush=True)
    return body


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--log-dir", required=True)
    ap.add_argument("--project", required=True)
    ap.add_argument("--binary", default="target/debug/rs_cam_gui")
    ap.add_argument("--out", required=True)
    ap.add_argument("--toolpath-index", type=int, default=0)
    ap.add_argument("--resolution", type=float, default=1.0)
    ap.add_argument("--optimize-timeout", type=float, default=1800.0)
    ap.add_argument("--width", type=int, default=1600)
    ap.add_argument("--height", type=int, default=1000)
    ap.add_argument("--shots", type=float, nargs="+", default=[10.0, 25.0, 45.0])
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
        call(gui, "generate_toolpath", {"index": args.toolpath_index}, timeout=1800.0)
        call(gui, "run_simulation", {"resolution": args.resolution}, timeout=1800.0)

        # Printed in full (head=4000): this is the run stating which branch
        # the screenshot below is about to capture.
        call(gui, "optimize_toolpath", {"index": args.toolpath_index},
             timeout=args.optimize_timeout, head=4000)

        # The MCP tool does NOT stash the outcome in `AppState::optimize_modal`
        # — only `AppEvent::OpenOptimizeModal` does, which `set_ui_view` pushes
        # and the controller then runs on the frame loop. So the card is
        # populated by a SECOND search here; on a pre-flight refusal that costs
        # no sims. Polling, not synchronisation — stated rather than hidden.
        call(gui, "set_ui_view",
             {"modal": "optimize_modal", "toolpath_index": args.toolpath_index})

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
