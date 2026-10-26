"""C38 §15.7: how much of D30 op 1's over-0.471 length (fix21/load_split.py
classes) lies on plunge departures, the moves §15.6 fix 1 feeds. A
departure move: a feed move after a vertical plunge at the plunge rate 750
(no other XY move between) whose start lies within R = 3 mm of the plunge
in XY (the readout's `departing` rule reads a step's start; a step whose
start is within R makes its whole window a departure record). Writes
other_departures.json. Read only."""
import collections, importlib.util, json, math, re, runpy, sys
from pathlib import Path
import numpy as np
H = Path(__file__).resolve().parent; P3 = H.parent.parent; FIX = P3 / "fix21"
R = 3.0
spec = importlib.util.spec_from_file_location("audit", P3 / "fix11" / "audit.py"); A = importlib.util.module_from_spec(spec); spec.loader.exec_module(A)
ops = A.op_moves(str(FIX / "a_only" / "corne" / "program.nc"), "3D Rough 1")
# Departure moves (op-1 move index k = program op line k + 1).
dep = set(); plunge = None
for k in range(len(ops) - 1):
    m = ops[k + 1]; a, b = m["a"], m["b"]
    if m["g"] == 0:
        continue
    if abs((m["f"] or 0) - 750.0) < 0.5 and a[:2] == b[:2] and b[2] < a[2]:
        plunge = b[:2]; continue
    if plunge is None:
        continue
    if math.hypot(a[0] - plunge[0], a[1] - plunge[1]) < R and math.hypot(b[0] - a[0], b[1] - a[1]) > 1e-6:
        dep.add(k)
    else:
        plunge = None
# The load_split classes of D30, with the departure moves split out.
sys.argv = ["feed_check.py", "fix21/a_only/corne", "3D Rough 1", "fix21/a_only/corne"]
g = runpy.run_path(str(FIX / "feed_check.py"))
S, turns = g["S"], set(g["moves"])
ref = []
for ln in open(P3 / "fix16" / "corne_pick" / "stdout_plain.txt"):
    if "index=0" in ln and "C40 entry kind" in ln and "LargeZ" in ln:
        kv = dict(re.findall(r"(\w+)=(\S+)", ln.split("C40 entry", 1)[1])); ref.append((float(kv["x"]), float(kv["y"])))
ref = np.array(ref); cell = 0.3
base = collections.Counter(round(s["feed_rate_mm_min"]) for s in S if s["is_cutting"] and s["source_intent"] == "clearing_cut").most_common(1)[0][0]
prev = None; tot = collections.Counter(); on_dep = collections.Counter()
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
    if k in dep:
        on_dep[c] += ln
out = {"departure_moves": len(dep), "base_feed": base,
       "class_mm": {k: round(v, 1) for k, v in sorted(tot.items())},
       "on_departure_moves_mm": {k: round(v, 1) for k, v in sorted(on_dep.items())}}
(H / "other_departures.json").write_text(json.dumps(out, indent=1))
print(json.dumps(out, indent=1))
