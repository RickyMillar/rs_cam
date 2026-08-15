#!/usr/bin/env python3
"""G-REGEN-RACE: does `generate_all` report a failure for work it superseded?

TD3 tail wave G-REGEN-RACE. B-4b measured the defect as a failing step-5 bar
and recorded it (`ORCHESTRATION_LOG.md`, B-4b, "Step 6 — DEFERRED"): a
`generate_all` issued while the GUI's own `process_auto_regen` still had an
`adaptive3d` in flight returned `"Back Rough: generation cancelled"`,
`rounds: 1`, `generated: 0`. Its `nomin/result.json` — the never-minimised
arm that ruled the compositor out — lived in session scratch and is gone, so
this is the repro rebuilt from the mechanism rather than recovered from that
file. B-4b's own `wait_for_idle` docstring in `../b4/shipped_flip_check.py`
states the same mechanism; that rig waits the race out, this one fires into
it on purpose.

The mechanism, and where each step lives:

  1. `load_project` marks EVERY toolpath `auto_regen = true` and
     `stale_since = Some(loaded_at)` (`controller/io.rs:180-182`).
  2. 500 ms later `process_auto_regen` submits them
     (`controller.rs::process_auto_regen`); one becomes the toolpath lane's
     ACTIVE job.
  3. The agent's `generate_all` submits that same toolpath again. The lane's
     rule is resubmit-cancels-and-requeues
     (`compute/worker.rs::submit_toolpath`): the in-flight job is cancelled
     and a replacement is queued.
  4. Pre-fix the abandoned job's `ComputeError::Cancelled` drained as the
     toolpath's OUTCOME, so `generate_all` counted a failure for the job it
     had itself replaced — while the replacement went on to succeed
     unobserved.

Step 2 is PROVED, not assumed: the rig polls `generation_status` and refuses
to record a verdict unless the lane is genuinely busy with a toolpath at the
moment `generate_all` is issued. Nothing here depends on the window being
minimised, occluded or anything else — the rig never touches the window.

Never point this at the operator's live GUI or at
`planning/airrun_2026-06-01/wanaka.toml`: it launches its own process and
takes a project path the caller has already copied into scratch
(`make_fixture.py`, next to this file, makes that copy).

Usage:
    python3 regen_race_check.py --binary target/debug/rs_cam_gui \\
        --project /tmp/.../gregen_back_rough.toml \\
        --log-dir .../artifacts/gregen/red
"""

import argparse
import json
import os
import sys
import time

_HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(_HERE, "..", "b1"))

from mcp_stdio_client import McpProcess  # noqa: E402


def text_of(reply):
    try:
        return reply["msg"]["result"]["content"][0]["text"]
    except Exception:  # noqa: BLE001
        return ""


def body(reply):
    try:
        return json.loads(text_of(reply))
    except Exception:  # noqa: BLE001
        return None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--binary", required=True)
    ap.add_argument("--project", required=True, help="scratch COPY of a project .toml")
    ap.add_argument("--log-dir", required=True)
    ap.add_argument("--workdir", default=".")
    ap.add_argument("--arm-timeout", type=float, default=120.0)
    ap.add_argument("--generate-timeout", type=float, default=1200.0)
    ap.add_argument(
        "--label", default="", help="free-text arm label recorded in the result"
    )
    args = ap.parse_args()

    os.makedirs(args.log_dir, exist_ok=True)
    out = {
        "meta": {
            "wave": "G-REGEN-RACE",
            "label": args.label,
            "binary": os.path.abspath(args.binary),
            "project": os.path.abspath(args.project),
            "when": time.strftime("%Y-%m-%dT%H:%M:%S"),
            "window": "never minimised, never touched",
        },
        "steps": [],
        "verdict": None,
    }

    proc = McpProcess(
        os.path.abspath(args.binary),
        os.path.abspath(args.workdir),
        args.log_dir,
        mode="direct",
        extra_env={"RUST_LOG": "info"},
        keep_wayland=True,
    )
    proc.start()
    try:
        proc.initialize(timeout=60.0)
        t0 = time.time()
        r = proc.call_tool(
            "load_project", {"path": os.path.abspath(args.project)}, timeout=600.0
        )
        out["steps"].append(
            {
                "call": "load_project",
                "wall_s": r.get("wall_s"),
                "text": text_of(r)[:400],
            }
        )
        print(f"  load_project {r.get('wall_s')}s")

        # --- ARM: prove the GUI's OWN sweep has a job on the lane. -------
        armed, trail = None, []
        deadline = time.time() + args.arm_timeout
        while time.time() < deadline:
            st = body(proc.call_tool("generation_status", {}, timeout=30.0))
            if isinstance(st, dict):
                trail.append(
                    {k: st.get(k) for k in ("busy", "lane_state", "job",
                                            "toolpath_id", "elapsed_s")}
                )
                if st.get("busy"):
                    armed = trail[-1]
                    break
            time.sleep(0.2)
        out["steps"].append(
            {"call": "arm", "armed": armed, "trail": trail[-8:],
             "wall_s": time.time() - t0}
        )
        print(f"  armed: {armed}")
        if armed is None:
            out["verdict"] = {
                "armed": False,
                "note": "the auto-regen sweep never put a job on the lane, so "
                        "this run measures nothing",
            }
            return out, proc

        # --- The agent's call, landing on a busy lane. -------------------
        t1 = time.time()
        r = proc.call_tool(
            "generate_all",
            {"fixpoint": False, "timeout_s": int(args.generate_timeout)},
            timeout=args.generate_timeout + 120.0,
        )
        ga = body(r)
        out["steps"].append(
            {"call": "generate_all", "wall_s": time.time() - t1, "reply": ga,
             "text": text_of(r)[:2000]}
        )

        # --- What actually exists on the other side. ---------------------
        tps = body(proc.call_tool("list_toolpaths", {}, timeout=180.0))
        out["steps"].append({"call": "list_toolpaths", "reply": tps})

        errors = (ga or {}).get("errors") or []
        phantom = [e for e in errors if "cancel" in json.dumps(e).lower()]
        out["verdict"] = {
            "armed": True,
            "generated": (ga or {}).get("generated"),
            "failed": (ga or {}).get("failed"),
            "rounds": (ga or {}).get("rounds"),
            "errors": errors,
            "phantom_cancellations": phantom,
            "red": bool(phantom),
        }
        return out, proc
    finally:
        pass


if __name__ == "__main__":
    log_dir = sys.argv[sys.argv.index("--log-dir") + 1]
    process = None
    try:
        result, process = main()
    finally:
        if process is not None:
            os.makedirs(log_dir, exist_ok=True)
            path = os.path.join(log_dir, "result.json")
            with open(path, "w") as fh:
                json.dump(result, fh, indent=1)
            print(json.dumps(result.get("verdict"), indent=1))
            print(f"wrote {path}")
            process.kill()
