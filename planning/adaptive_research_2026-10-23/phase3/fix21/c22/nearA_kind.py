"""C22 §16.12 item 1: the op-1 clearing length over 0.471 near A's steps (fix21/load_split.py's
"near A's steps" set: not a turn or slot move, within R + one sim cell of a D26 LargeZ /
LargeZHeld entry), split by the move it lies on:
- 'on a step': the move itself climbs or drops by the path rule A replaced (its start-to-end
  |dz| over max(1.1 DPP, 1 mm), or its slope over 0.3);
- 'after a step': a level move whose previous fed move is such a step;
- 'level': any other move (a pass beside a face or on terrain).
Moves are read from the simulation samples (start = the last sample of the move before)."""
import collections, json, math, re, sys
from pathlib import Path
import numpy as np
H = Path(__file__).resolve().parent; P3 = H.parent.parent
R = 3.0
CFG = {"wanaka200": ("fix16/wanaka200_pick", 1.0, 5.46), "corne": ("fix16/corne_pick", 0.3, 6.0)}
OUT = {}
for fx in ("wanaka200", "corne"):
    ref_run, cell, dpp = CFG[fx]
    ref = []
    for ln in open(P3 / ref_run / "stdout_plain.txt"):
        if "index=0" in ln and "C40 entry kind" in ln and "LargeZ" in ln:
            kv = dict(re.findall(r"(\w+)=(\S+)", ln.split("C40 entry", 1)[1])); ref.append((float(kv["x"]), float(kv["y"])))
    ref = np.array(ref)
    thr = max(1.1 * dpp, 1.0)
    for col, run in (("D26", ref_run), ("D30", f"fix21/a_only/{fx}")):
        S = [s for s in json.load(open(P3 / run / "simulation.json"))["trace"]["samples"] if s["toolpath_id"] == 1]
        first, last = {}, {}
        for s in S:
            first.setdefault(s["move_index"], s); last[s["move_index"]] = s
        ms = sorted(last)
        prev_of = {b: a for a, b in zip(ms, ms[1:])}
        def is_step(m):
            a = prev_of.get(m)
            if a is None: return False
            pa, pb = last[a]["position"], last[m]["position"]
            dz = abs(pb[2] - pa[2]); xy = math.hypot(pb[0] - pa[0], pb[1] - pa[1])
            return last[m]["source_intent"] == "clearing_cut" and (dz > thr or (xy > 1e-6 and dz / xy > 0.3) or (xy <= 1e-6 and dz > 1e-6))
        base = collections.Counter(round(s["feed_rate_mm_min"]) for s in S if s["is_cutting"] and s["source_intent"] == "clearing_cut").most_common(1)[0][0]
        tot = collections.Counter(); prev = None
        for s in S:
            p = s["position"]; ln = math.hypot(p[0]-prev[0], p[1]-prev[1]) if prev else 0.0; prev = p
            if not (s["is_cutting"] and s["source_intent"] == "clearing_cut" and s["engagement"]["radial_woc_fraction"] > 0.471): continue
            if s["feed_rate_mm_min"] < base - 0.5: continue
            if not (len(ref) and np.min(np.hypot(ref[:, 0]-p[0], ref[:, 1]-p[1])) <= R + cell): continue
            m = s["move_index"]
            kind = "on a step" if is_step(m) else ("after a step" if prev_of.get(m) is not None and is_step(prev_of[m]) else "level")
            tot[kind] += ln
        OUT[f"{fx} {col}"] = {k: round(v, 1) for k, v in sorted(tot.items())}
        print(fx, col, OUT[f"{fx} {col}"], "total", round(sum(tot.values()), 1))
json.dump(OUT, open(H / "nearA_kind.json", "w"), indent=1)
