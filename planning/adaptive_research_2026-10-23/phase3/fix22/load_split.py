"""D31 (copy of fix21/load_split.py; caps from fix22): op 1's clearing length over 0.471 (C22 §16.9 measure: XY length of
clearing_cut samples whose radial_woc_fraction is over 0.471), split into
- turn moves: the moves of the D30 "C38 capped turn" lines (fix21/feed_check.py match);
- slot moves: other moves whose sample feed (the planner feed, before modulation)
  is below the op's base clearing feed (the most common clearing sample feed):
  the planner lowered them, and the only other lowered steps are the slots;
- near A's steps: the rest, within R + one sim cell of a D26 LargeZ / LargeZHeld
  entry (fix20/c22/parta_place.py's reference);
- other.
Usage: load_split.py <fixture: corne|wanaka200> <run dir>..."""
import collections, json, math, re, runpy, sys
from pathlib import Path
import numpy as np
H = Path(__file__).resolve().parent; P3 = H.parent
fx = sys.argv[1]; R = 3.0
ref_run, cell, op = {"corne": ("fix16/corne_pick", 0.3, "3D Rough 1"), "wanaka200": ("fix16/wanaka200_pick", 1.0, "2 Back Rough")}[fx]
caps_run = f"fix22/cut_feed/{fx}"
ref = []
for ln in open(P3 / ref_run / "stdout_plain.txt"):
    if "index=0" in ln and "C40 entry kind" in ln and "LargeZ" in ln:
        kv = dict(re.findall(r"(\w+)=(\S+)", ln.split("C40 entry", 1)[1])); ref.append((float(kv["x"]), float(kv["y"])))
ref = np.array(ref)
out = {}
for run in sys.argv[2:]:
    argv = sys.argv; sys.argv = ["feed_check.py", run, op, caps_run]
    g = runpy.run_path(str(P3 / "fix21" / "feed_check.py")); sys.argv = argv
    S, turns = g["S"], set(g["moves"])
    base = collections.Counter(round(s["feed_rate_mm_min"]) for s in S if s["is_cutting"] and s["source_intent"] == "clearing_cut").most_common(1)[0][0]
    prev = None; tot = collections.Counter()
    for s in S:
        p = s["position"]; ln = math.hypot(p[0] - prev[0], p[1] - prev[1]) if prev else 0.0; prev = p
        if not (s["is_cutting"] and s["source_intent"] == "clearing_cut" and s["engagement"]["radial_woc_fraction"] > 0.471):
            continue
        k = s["move_index"]
        if k in turns:
            c = "turn moves"
        elif s["feed_rate_mm_min"] < base - 0.5:
            c = "slot moves"
        elif len(ref) and np.min(np.hypot(ref[:, 0] - p[0], ref[:, 1] - p[1])) <= R + cell:
            c = "near A's steps"
        else:
            c = "other"
        tot[c] += ln
    r = {"base_feed": base, "over_0.471_mm": round(sum(tot.values()), 1), **{k: round(v, 1) for k, v in sorted(tot.items())}}
    out[run] = r; print(run, json.dumps(r))
json.dump(out, open(H / f"load_split_{fx}.json", "w"), indent=1)
