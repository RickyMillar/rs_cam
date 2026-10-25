"""C30's producer split, for any rough op: the XY length of cutting samples
whose intent is clearing_cut (any kinematics) that the simulation reads over
0.471, per producer.

Phases per move (from phase3/c30/attribute.py, generalised to any toolpath
file and id):
- a level starts at the final move of its "Adaptive Z" annotation, the
  waterline at the final move of its "Waterline cleanup" annotation (an
  annotation's generation move maps to the first entry span whose pass
  index is at or after it);
- agent-slice arms (debug spans "agent2d_region"): the perimeter sweep runs
  from the level start to the level's second entry, then the body to the
  waterline;
- other arms (contour_parallel has no perimeter sweep): the body runs from
  the level start to the waterline;
- the waterline runs to the next level.
"""
import collections
import glob
import json
import math
import os
import re


def phases(tp):
    ents = sorted([i for i in tp["semantic_trace"]["items"] if i["kind"] == "entry"], key=lambda i: i["move_start"])
    an = tp["debug_trace"]["annotations"]
    gen_of_pass = {}
    for a in an:
        m = re.match(r"Pass (\d+) — ", a["label"])
        if m:
            gen_of_pass[int(m.group(1))] = a["move_index"]
    fin = sorted((gen_of_pass[e["params"]["values"]["pass_index"]], e["move_start"]) for e in ents)
    n = tp["move_count"]

    def final_of(g):
        for gi, fi in fin:
            if gi >= g:
                return fi
        return n
    levels = [(float(a["label"].split()[2]), final_of(a["move_index"])) for a in an if a["label"].startswith("Adaptive Z")]
    wls = [final_of(a["move_index"]) for a in an if a["label"] == "Waterline cleanup"]
    slice_arm = any(s.get("kind") == "agent2d_region" for s in tp["debug_trace"]["spans"])
    phase = [None] * n
    for k, (z, st) in enumerate(levels):
        en = levels[k + 1][1] if k + 1 < len(levels) else n
        wl = wls[k]
        body = st
        if slice_arm:
            le = [e for e in ents if st <= e["move_start"] < wl]
            body = le[1]["move_start"] if len(le) > 1 else st
        for m in range(st, en):
            phase[m] = "perimeter" if m < body else ("body" if m < wl else "waterline")
    return phase, slice_arm


def split(run_dir, simart_dir, tp_glob, bound=0.471):
    tpf = sorted(glob.glob(os.path.join(run_dir, tp_glob)))[0]
    tp = json.load(open(tpf))
    tpid = int(re.search(r"tp_(\d+)_", os.path.basename(tpf)).group(1))
    phase, slice_arm = phases(tp)
    out = collections.Counter()
    prev = None
    for sm in json.load(open(os.path.join(simart_dir, "simulation.json")))["trace"]["samples"]:
        p = sm["position"]
        ln = math.hypot(p[0] - prev[0], p[1] - prev[1]) if prev else 0.0
        prev = p
        if sm["toolpath_id"] != tpid or not sm["is_cutting"] or sm["source_intent"] != "clearing_cut":
            continue
        if sm["engagement"]["radial_woc_fraction"] > bound:
            ph = phase[sm["move_index"]] if sm["move_index"] < len(phase) else None
            out[ph or "unplaced"] += ln
    return {"perimeter": round(out["perimeter"], 1) if slice_arm else None,
            "body": round(out["body"], 1), "waterline": round(out["waterline"], 1),
            "unplaced": round(out["unplaced"], 1)}
