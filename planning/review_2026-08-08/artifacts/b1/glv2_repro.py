#!/usr/bin/env python3
"""TD3 B-1: G-LV.2 crash capture + MCP read-size census.

Sequence (census BEFORE the fatal call, so a crash loses nothing):

  1. launch `rs_cam_gui --mcp` (optionally under gdb), initialize MCP
  2. load_project <scratch copy of wanaka>
  3. generate_all  fixpoint=true  simulation_resolution_mm=0.1
  4. run_simulation resolution=0.1              (the authoritative trace)
  5. census every parameterised read, recording response bytes + wall time
  6. id-vs-index probes on the `toolpath_id` filter
  7. LAST: unfiltered get_cut_trace  <- the G-LV.2 trigger

Usage:
  glv2_repro.py --log-dir DIR --project FILE [--mode direct|gdb]
                [--skip-generate] [--phase all|census|fatal]
"""

import argparse
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from mcp_stdio_client import McpProcess, RssSampler  # noqa: E402

REPO = "/home/ricky/personal_repos/rs_cam"
BIN = os.path.join(REPO, "target/release/rs_cam_gui")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--log-dir", required=True)
    ap.add_argument("--project", required=True)
    ap.add_argument("--mode", default="direct", choices=["direct", "gdb"])
    ap.add_argument("--binary", default=BIN)
    ap.add_argument("--gen-timeout", type=int, default=2400)
    ap.add_argument("--sim-timeout", type=int, default=1800)
    ap.add_argument("--resolution", type=float, default=0.1)
    ap.add_argument("--skip-generate", action="store_true")
    ap.add_argument("--no-fatal", action="store_true")
    ap.add_argument("--wayland", action="store_true",
                    help="keep WAYLAND_DISPLAY set (match the incident's backend)")
    ap.add_argument("--short-census", action="store_true",
                    help="skip the per-index sweeps; go straight to the "
                         "get_cut_trace size census + the fatal call")
    args = ap.parse_args()

    os.makedirs(args.log_dir, exist_ok=True)
    jsonl = os.path.join(args.log_dir, "calls.jsonl")
    rsslog = os.path.join(args.log_dir, "rss.csv")

    proc = McpProcess(args.binary, REPO, args.log_dir, mode=args.mode,
                      keep_wayland=args.wayland)

    def rec(**kw):
        kw["t"] = time.time()
        with open(jsonl, "a") as f:
            f.write(json.dumps(kw) + "\n")
        return kw

    def call(name, a=None, timeout=180.0, note=""):
        sampler.mark(f"call:{name}:{json.dumps(a or {})}")
        r = proc.call_tool(name, a, timeout=timeout)
        entry = {
            "call": name,
            "args": a or {},
            "note": note,
            "wall_s": round(r.get("wall_s", -1), 3),
            "line_bytes": r.get("line_bytes"),
            "transport": r.get("__transport__"),
            "peak_rss_kb_so_far": sampler.peak_rss_kb,
        }
        if r.get("__transport__") in ("broken_pipe_on_send", "stream_closed",
                                      "process_died_while_waiting"):
            entry["fatal_transport"] = True
        msg = r.get("msg")
        if msg is not None:
            res = msg.get("result") or {}
            content = res.get("content") or []
            text = ""
            for c in content:
                if c.get("type") == "text":
                    text += c.get("text", "")
            entry["content_bytes"] = len(text.encode())
            entry["is_error"] = bool(res.get("isError")) or ("error" in msg)
            entry["preview"] = text[:400]
            entry["tail"] = text[-200:] if len(text) > 600 else ""
            entry["full_text"] = None
            # keep whole payloads for the small structural reads
            if len(text) < 200_000:
                entry["full_text"] = text
            # Section accounting: for anything large, record how many bytes
            # each top-level key costs and how long each array is. This is
            # what Checkpoint L needs to pick per-array page sizes.
            if len(text) > 50_000:
                try:
                    obj = json.loads(text)
                    if isinstance(obj, dict):
                        sec = {}
                        for k, v in obj.items():
                            sec[k] = {
                                "bytes_compact": len(
                                    json.dumps(v, separators=(",", ":")).encode()
                                ),
                                "bytes_pretty": len(
                                    json.dumps(v, indent=2).encode()
                                ),
                                "len": len(v) if isinstance(v, (list, dict)) else None,
                            }
                        entry["sections"] = sec
                        entry["bytes_compact_total"] = len(
                            json.dumps(obj, separators=(",", ":")).encode()
                        )
                except Exception as e:  # noqa: BLE001
                    entry["sections_error"] = repr(e)
        rec(**entry)
        if entry.get("fatal_transport") and not getattr(call, "_in_fatal", False):
            print(f"!! transport lost during {name}; aborting remaining plan",
                  flush=True)
        short = entry.get("content_bytes")
        print(
            f"[{time.strftime('%H:%M:%S')}] {name} {json.dumps(a or {})[:80]} "
            f"-> line={entry['line_bytes']} content={short} "
            f"wall={entry['wall_s']}s transport={entry['transport']}",
            flush=True,
        )
        return entry

    sampler = RssSampler(lambda: proc._gui_pid, rsslog, period=0.5,
                         on_floor=proc.kill)
    sampler.start()

    print(f"launching {args.binary} mode={args.mode}", flush=True)
    proc.start()
    time.sleep(1.0)
    print(f"gui pid = {proc._gui_pid}", flush=True)
    rec(event="launched", pid=proc._gui_pid, mode=args.mode, binary=args.binary)

    ini = proc.initialize(timeout=90)
    rec(event="initialize", ok=("msg" in ini), wall_s=ini.get("wall_s"))
    if "msg" not in ini:
        print("initialize FAILED", ini, flush=True)
        return 1

    tl = proc.rpc("tools/list", {}, timeout=60)
    n_tools = 0
    if "msg" in tl:
        tools = (tl["msg"].get("result") or {}).get("tools", [])
        n_tools = len(tools)
        with open(os.path.join(args.log_dir, "tools_list.json"), "w") as f:
            json.dump(tools, f, indent=2)
    rec(event="tools_list", count=n_tools, line_bytes=tl.get("line_bytes"))
    print(f"tools registered: {n_tools}", flush=True)

    # ---------------------------------------------------------------- load
    lp = call("load_project", {"path": args.project}, timeout=90)
    if lp.get("transport") or lp["wall_s"] > 45:
        # On Wayland an occluded/hidden surface gets no frame callbacks and
        # nothing dispatched from a repaint advances (G-LV.1). Bail fast
        # rather than burn a 12-minute slot on a parked loop.
        rec(event="dispatch_stalled_at_load_project", wall_s=lp["wall_s"],
            transport=lp.get("transport"))
        print("DISPATCH STALLED at load_project — aborting", flush=True)
        proc.kill()
        return 3
    lt = call("list_toolpaths", {}, timeout=120)
    tps = []
    try:
        j = json.loads(lt.get("full_text") or "{}")
        tps = j.get("data") or j.get("toolpaths") or j.get("items") or []
    except Exception as e:  # noqa: BLE001
        rec(event="list_toolpaths_parse_failed", err=repr(e))
    rec(event="toolpaths", n=len(tps), raw=tps)
    print(json.dumps(tps)[:2000], flush=True)

    # ------------------------------------------------------------ generate
    if not args.skip_generate:
        sampler.mark("generate_all:begin")
        g = call(
            "generate_all",
            {"fixpoint": True, "simulation_resolution_mm": args.resolution,
             "timeout_s": args.gen_timeout},
            timeout=args.gen_timeout + 300,
            note="fixpoint generate at census resolution",
        )
        # If it returned status:running, poll generation_status.
        deadline = time.time() + args.gen_timeout
        while time.time() < deadline:
            st = call("generation_status", {}, timeout=60)
            txt = st.get("full_text") or ""
            if '"busy": false' in txt or '"busy":false' in txt:
                break
            if proc.dead():
                rec(event="died_during_generate")
                break
            time.sleep(30)
        sampler.mark("generate_all:end")

    sampler.mark("run_simulation:begin")
    call("run_simulation", {"resolution": args.resolution},
         timeout=args.sim_timeout, note="authoritative full-project trace")
    sampler.mark("run_simulation:end")

    if proc.dead():
        rec(event="died_before_census")
        print("PROCESS DIED BEFORE CENSUS", flush=True)
        return 2

    # ------------------------------------------------------------- census
    call("get_diagnostics", {}, timeout=300, note="scale reference")
    call("get_project_diagnostics", {}, timeout=300, note="scale reference")
    call("get_tool_load_report", {}, timeout=300, note="scale reference")

    n = len(tps)
    ids = []
    for i, tp in enumerate(tps):
        tid = tp.get("id") if isinstance(tp, dict) else None
        ids.append(tid)
    rec(event="index_id_map", pairs=list(enumerate(ids)))

    if not args.short_census:
        for i in range(n):
            call("narrate_toolpath", {"index": i}, timeout=600, note="scale ref")
        for i in range(n):
            call("get_toolpath_diagnostics", {"index": i}, timeout=600)
        for i in range(n):
            call("inspect_spans", {"index": i}, timeout=600)
        for i in range(n):
            call("get_generation_debug_trace", {"index": i}, timeout=600)

    # get_cut_trace filtered by every id AND every index value seen -- this is
    # simultaneously the size census and the id-vs-index evidence.
    probe_values = sorted({v for v in ids if v is not None} | set(range(n)))
    if args.short_census:
        probe_values = [v for v in probe_values if v in set(ids)]
    for v in probe_values:
        call("get_cut_trace", {"toolpath_id": v}, timeout=900,
             note="filtered; id-vs-index probe")

    # Drill-sample stream: the one array the tool description already warns
    # "can be verbose".
    for v in [t.get("id") for t in tps
              if isinstance(t, dict) and "drill" in (t.get("operation_label") or "").lower()]:
        call("get_cut_trace", {"toolpath_id": v, "include_drill_samples": True},
             timeout=900, note="drill sample stream")

    if not args.short_census:
        # filtered-but-large variants
        for v in probe_values[:3]:
            call("get_cut_trace",
                 {"toolpath_id": v, "max_hotspots": 10000, "max_issues": 100000},
                 timeout=900, note="filtered, caps raised")
        call("get_cut_trace", {"span_kind": "depth_pass", "max_issues": 50},
             timeout=900, note="span-filtered, no toolpath filter")

    rec(event="census_complete")
    print("CENSUS COMPLETE", flush=True)

    if args.no_fatal:
        proc.kill()
        return 0

    # -------------------------------------------------------------- fatal
    sampler.mark("FATAL:get_cut_trace unfiltered")
    print(">>> firing unfiltered get_cut_trace", flush=True)
    f = call("get_cut_trace", {}, timeout=1200, note="G-LV.2 TRIGGER")
    time.sleep(3)
    alive = not proc.dead()
    rec(event="post_fatal", alive=alive, peak_rss_kb=sampler.peak_rss_kb)
    print(f"after unfiltered call: process alive = {alive}", flush=True)
    if not alive:
        rc = proc.wait_exit(timeout=120)
        rec(event="exit", returncode=rc)
        print(f"wrapper exit code {rc}", flush=True)
    else:
        # transport closed but process alive? probe with a cheap read.
        p = call("list_toolpaths", {}, timeout=60, note="post-fatal liveness probe")
        rec(event="post_fatal_probe", transport=p.get("transport"))

    sampler.stop_flag.set()
    time.sleep(1)
    rec(event="done", peak_rss_kb=sampler.peak_rss_kb,
        safety_tripped=sampler.tripped)
    proc.kill()
    return 0


if __name__ == "__main__":
    sys.exit(main())
