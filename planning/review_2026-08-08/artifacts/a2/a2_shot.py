#!/usr/bin/env python3
"""A-2 / F-HEATMAP — before/after heat-map capture.

Drives an ISOLATED rs_cam_gui --mcp instance (never the operator's).
Reuses B-1's dependency-free stdio client.
"""
import json, os, sys, time
sys.path.insert(0, "/home/ricky/personal_repos/rs_cam/planning/review_2026-08-08/artifacts/b1")
from mcp_stdio_client import McpProcess  # noqa: E402

binary, tag, project, outdir = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4]
logdir = os.path.join(outdir, f"log_{tag}")
os.makedirs(outdir, exist_ok=True)
p = McpProcess(binary, "/home/ricky/personal_repos/rs_cam", logdir, mode="direct")
p.start()

def call(name, args=None, timeout=180.0, label=None):
    t0 = time.time()
    r = p.call_tool(name, args or {}, timeout=timeout)
    dt = time.time() - t0
    txt = ""
    try:
        txt = r["result"]["content"][0]["text"]
    except Exception:
        txt = json.dumps(r)[:400]
    print(f"[{tag}] {label or name}  {dt:.1f}s  -> {txt[:260]}", flush=True)
    return txt

try:
    p.initialize()
    call("load_project", {"path": project}, timeout=300, label="load_project")
    call("generate_all", {"fixpoint": True, "simulation_resolution_mm": 0.4},
         timeout=2400, label="generate_all")
    call("run_simulation", {"resolution": 0.4}, timeout=2400, label="run_simulation")
    call("get_diagnostics", {}, timeout=300, label="get_diagnostics")
    # The viewport heat-map only renders where TOOLPATHS are drawn; the
    # Simulation workspace shows machined stock instead. `screenshot_toolpath`
    # is a separate fixed-palette offscreen renderer and is NOT a heat-map
    # surface — verified on the first before-run, which came back all-green.
    call("set_ui_view", {"workspace": "toolpaths"}, timeout=120, label="set_ui_view(toolpaths)")
    tpv_png = os.path.join(outdir, f"heatmap_{tag}_viewport.png")
    call("screenshot_gui", {"path": tpv_png}, timeout=300, label="screenshot_gui(toolpaths)")
    call("set_ui_view", {"workspace": "simulation"}, timeout=120, label="set_ui_view(simulation)")
    gui_png = os.path.join(outdir, f"heatmap_{tag}_gui.png")
    tp_png = os.path.join(outdir, f"heatmap_{tag}_toolpath.png")
    call("screenshot_gui", {"path": gui_png}, timeout=300, label="screenshot_gui")
    call("screenshot_toolpath", {"path": tp_png, "index": 1, "include_rapids": False},
         timeout=300, label="screenshot_toolpath")
finally:
    p.kill()
    time.sleep(1)
print(f"[{tag}] done", flush=True)
