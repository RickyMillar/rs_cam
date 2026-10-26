"""C22 §12.8 Q2: the cut length over 0.471 (fixture3d_v4 load_share rule: clearing_cut,
cutting, radial_woc_fraction > 0.471, weight = XY step from the previous sample) by level
and by radius from the central seed, D20 against C35."""
import sys, os, math, json, collections, bisect
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import probe as pr
EXACT = 0.471
OUT = {}
for col, d in pr.COLS.items():
    sim = json.load(open(os.path.join(pr.P3, d, "simulation.json")))
    ALL = sim["trace"]["samples"]
    S, tp = pr.load(d)
    ent = sorted([(e["move_start"], e["move_end"], e["params"]["values"]["style"]) for e in tp["semantic_trace"]["items"] if e["kind"] == "entry"])
    pl = []
    for a, b, st in ent:
        if st != "plunge entry": continue
        ss = [s for s in S if a <= s["move_index"] <= b]; pl.append((a, ss[-1]["position"]))
    seeds = [a for a, p in pl if math.hypot(p[0]-pr.SEED[0], p[1]-pr.SEED[1]) < 0.01]
    l4 = next(a for a, p in pl if abs(p[0]-122.0) < 0.01 and abs(p[1]-31.18) < 0.01)
    bounds = seeds + [l4, 10**9]
    prev = None; over = collections.Counter(); cut = collections.Counter(); hi = collections.Counter()
    for sm in ALL:
        p = sm["position"]
        ln = math.hypot(p[0]-prev[0], p[1]-prev[1]) if prev else 0.0
        prev = p
        if sm["toolpath_id"] != 1 or not sm["is_cutting"] or sm["source_intent"] != "clearing_cut": continue
        L = bisect.bisect_right(bounds, sm["move_index"])
        rr = math.hypot(p[0]-pr.SEED[0], p[1]-pr.SEED[1])
        b = "r<90" if rr < 90 else ("90-105" if rr < 105 else "r>=105")
        w = sm["engagement"]["radial_woc_fraction"]
        cut[(L, b)] += ln
        if w > EXACT:
            over[(L, b)] += ln
            if w > 0.6: hi[(L, b)] += ln
    OUT[col] = {f"L{L} {b}": [round(cut[(L, b)], 1), round(over[(L, b)], 1), round(hi[(L, b)], 1)] for (L, b) in sorted(cut)}
    print(col, "total over", round(sum(over.values()), 1))
    for k, v in OUT[col].items(): print("  ", k, "cut / over 0.471 / over 0.6 mm:", v)
json.dump(OUT, open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "over_by_core.json"), "w"), indent=1)
