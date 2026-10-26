"""C22 §12.8 Q1: the lift-over descents (entries.py's rule) per level, inside and
outside the inscribed disc of radius 97.4 mm about the central seed, D20 against C35."""
import sys, os, math, json, bisect, collections
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import probe as pr
R_CORE = 97.4
OUT = {}
for col, d in pr.COLS.items():
    S, tp = pr.load(d)
    ent = sorted([(e["move_start"], e["move_end"], e["params"]["values"]["style"]) for e in tp["semantic_trace"]["items"] if e["kind"] == "entry"])
    plunges = []
    for a, b, st in ent:
        if st != "plunge entry": continue
        ss = [s for s in S if a <= s["move_index"] <= b]
        plunges.append((a, ss[-1]["position"]))
    seeds = [a for a, p in plunges if math.hypot(p[0]-pr.SEED[0], p[1]-pr.SEED[1]) < 0.01]
    l4 = next(a for a, p in plunges if abs(p[0]-122.0) < 0.01 and abs(p[1]-31.18) < 0.01)
    bounds = seeds + [l4, 10**9]
    D = pr.descents(S, ent)
    rows = collections.Counter()
    for m, p in D:
        L = bisect.bisect_right(bounds, m)
        inside = math.hypot(p[0]-pr.SEED[0], p[1]-pr.SEED[1]) < R_CORE
        rows[(L, inside)] += 1
    OUT[col] = {f"L{L} {'inside' if i else 'outside'}": n for (L, i), n in sorted(rows.items())}
    print(col, len(D), OUT[col])
json.dump(OUT, open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "descents_by_core.json"), "w"), indent=1)
