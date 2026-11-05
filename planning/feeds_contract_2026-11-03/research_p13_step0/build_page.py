"""Inline the data files into page_template.html -> index.html. Run: python3 -I build_page.py"""
import json
import os

HERE = os.path.dirname(os.path.abspath(__file__))
J = lambda n: json.load(open(os.path.join(HERE, n)))  # noqa: E731
res, vp, fw, inv, pc = J("results.json"), J("vendor_points.json"), J("force_width.json"), J("inventory.json"), J("port_check.json")
cells = {k: {kk: vv for kk, vv in v.items() if kk != "grid_hw"} for k, v in res["cells"].items()}
kbars = [
    {"label": "Pocket convention", "k": res["implied_k"]["t0.2"]["pocket_0.25D_0.35D"]["k_frame_n_per_mm"], "kind": "convention"},
    {"label": "Adaptive convention", "k": res["implied_k"]["t0.2"]["adaptive_0.33D_0.25D"]["k_frame_n_per_mm"], "kind": "convention"},
] + [{"label": p["id"].replace("_", " "), "k": p["implied_k_n_per_mm_at_200um"],
      "kind": "hobby" if p["id"].startswith(("sienci", "c3d")) else "industrial"} for p in vp]
data = {
    "inventory": inv, "vendor": vp, "fw": fw["rows"], "cells": cells, "kbars": kbars, "port_n": len(pc),
    "grid": [{"ae_fr": p["ae_fr"], "ap_fr": p["ap_fr"], "mrr": p["mrr"], "out": p["out"]} for p in res["cells"]["hw6"]["grid_hw"]],
    "fronts": [{"label": k, "k": v["k"], "best": v["best"], "front": v["front"]} for k, v in res["frame_e"].items()],
}
tpl = open(os.path.join(HERE, "page_template.html")).read()
open(os.path.join(HERE, "index.html"), "w").write(tpl.replace("__DATA__", json.dumps(data, separators=(",", ":"))))
print("index.html", os.path.getsize(os.path.join(HERE, "index.html")))
