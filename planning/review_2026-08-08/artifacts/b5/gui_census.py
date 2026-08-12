#!/usr/bin/env python3
"""TD3 wave B-5 — GUI-mode results-parity census driver.

Launches an ISOLATED `rs_cam_gui --mcp` (never the operator's live GUI),
loads a project COPY, runs generate_all + run_simulation, and dumps every
read whose content the G-RESULTS ledger row claims diverges from the CLI:

  * get_toolpath_diagnostics(index)   -- unified Diagnostic list
  * get_tool_load_report()            -- gates + drill_gates + spans-derived
  * narrate_toolpath(index)           -- agent-readable narration
  * inspect_spans(index)              -- span presence
  * get_toolpath_params(index)        -- runtime block (claims_reference)
  * get_diagnostics()                 -- project triage

Everything is written as JSON under --log-dir so the diff can be done
offline against the CLI's `project` output.

Reuses B-1's dependency-free stdio client.
"""

import argparse
import json
import os
import sys
import time

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "b1"))

from mcp_stdio_client import McpProcess, RssSampler  # noqa: E402


def tool_text(resp):
    """Pull the text payload out of an MCP tools/call response."""
    msg = resp.get("msg")
    if msg is None:
        return None
    res = msg.get("result")
    if res is None:
        return json.dumps(msg)
    content = res.get("content") or []
    parts = [c.get("text", "") for c in content if c.get("type") == "text"]
    return "\n".join(parts)


def maybe_json(text):
    if text is None:
        return None
    try:
        return json.loads(text)
    except Exception:  # noqa: BLE001
        return text


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--binary", default="target/release/rs_cam_gui")
    ap.add_argument("--workdir", default=os.getcwd())
    ap.add_argument("--project", required=True)
    ap.add_argument("--log-dir", required=True)
    ap.add_argument("--resolution", type=float, default=0.35)
    ap.add_argument("--gen-timeout", type=float, default=5400.0)
    ap.add_argument("--sim-timeout", type=float, default=2400.0)
    ap.add_argument(
        "--generate-indices",
        default=None,
        help="Comma-separated toolpath indices to generate one at a time "
        "instead of generate_all. Cheap mode for a post-fix wire check on a "
        "debug binary — the row SHAPE is what is under test, not the numbers.",
    )
    args = ap.parse_args()

    os.makedirs(args.log_dir, exist_ok=True)
    out = {}

    proc = McpProcess(
        os.path.abspath(args.binary), args.workdir, args.log_dir, mode="direct"
    )
    sampler = RssSampler(
        lambda: proc._gui_pid,  # noqa: SLF001
        os.path.join(args.log_dir, "rss.csv"),
        on_floor=lambda: proc.kill(),
    )
    proc.start()
    sampler.start()
    try:
        out["initialize"] = proc.initialize(timeout=120).get("msg", {}).get("result", {})

        sampler.mark("load_project")
        r = proc.call_tool("load_project", {"path": os.path.abspath(args.project)},
                           timeout=600)
        out["load_project"] = tool_text(r)
        print("load_project:", out["load_project"], flush=True)

        r = proc.call_tool("list_toolpaths", {}, timeout=120)
        tps = maybe_json(tool_text(r))
        out["list_toolpaths"] = tps
        print("list_toolpaths ok", flush=True)

        sampler.mark("generate_all")
        t0 = time.time()
        if args.generate_indices:
            gen = []
            for raw in args.generate_indices.split(","):
                idx = int(raw.strip())
                gr = proc.call_tool(
                    "generate_toolpath", {"index": idx}, timeout=args.gen_timeout
                )
                gen.append({"index": idx, "reply": maybe_json(tool_text(gr))})
                print("generated toolpath", idx, flush=True)
            out["generate_all"] = {"per_index": gen}
        else:
            r = proc.call_tool(
                "generate_all",
                {"fixpoint": True, "simulation_resolution_mm": args.resolution},
                timeout=args.gen_timeout,
            )
            out["generate_all"] = maybe_json(tool_text(r))
        out["generate_all_wall_s"] = time.time() - t0
        print("generate_all done in %.0fs" % out["generate_all_wall_s"], flush=True)

        sampler.mark("run_simulation")
        t0 = time.time()
        r = proc.call_tool("run_simulation", {"resolution": args.resolution},
                           timeout=args.sim_timeout)
        out["run_simulation"] = maybe_json(tool_text(r))
        out["run_simulation_wall_s"] = time.time() - t0
        print("run_simulation done in %.0fs" % out["run_simulation_wall_s"], flush=True)

        # ---- the reads under census ------------------------------------
        sampler.mark("reads")
        out["tool_load_report"] = maybe_json(
            tool_text(proc.call_tool("get_tool_load_report", {}, timeout=900))
        )
        out["project_diagnostics"] = maybe_json(
            tool_text(proc.call_tool("get_diagnostics", {}, timeout=900))
        )

        n = 0
        if isinstance(tps, dict):
            n = len(tps.get("toolpaths", []))
        elif isinstance(tps, list):
            n = len(tps)
        out["toolpath_count"] = n

        per = []
        for i in range(n):
            row = {"index": i}
            row["diagnostics"] = maybe_json(
                tool_text(proc.call_tool("get_toolpath_diagnostics", {"index": i},
                                         timeout=600))
            )
            row["narration"] = tool_text(
                proc.call_tool("narrate_toolpath", {"index": i}, timeout=900)
            )
            row["spans"] = maybe_json(
                tool_text(proc.call_tool("inspect_spans", {"index": i}, timeout=600))
            )
            row["params"] = maybe_json(
                tool_text(proc.call_tool("get_toolpath_params", {"index": i},
                                         timeout=600))
            )
            per.append(row)
            print("read toolpath", i, flush=True)
        out["per_toolpath"] = per
    finally:
        sampler.stop_flag.set()
        out["peak_rss_kb"] = sampler.peak_rss_kb
        path = os.path.join(args.log_dir, "gui_census.json")
        with open(path, "w") as f:
            json.dump(out, f, indent=1, default=str)
        print("wrote", path, flush=True)
        proc.kill()


if __name__ == "__main__":
    main()
