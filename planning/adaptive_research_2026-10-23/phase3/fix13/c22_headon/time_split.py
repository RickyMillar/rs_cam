"""C22 §13.9: Corne simulated time by toolpath and source intent (segment_time_s summed),
C35 (fix12/corne) against D23 spiral_pick (fix13/corne_pick) and D23 spiral_headon (fix13/corne)."""
import json, os, collections
H = os.path.dirname(os.path.abspath(__file__)); P3 = os.path.join(H, "..", "..")
RUNS = {"C35 pick": "fix12/corne", "D23 pick": "fix13/corne_pick", "D23 headon": "fix13/corne"}
OUT = {}
for name, d in RUNS.items():
    sim = json.load(open(os.path.join(P3, d, "simulation.json")))
    t = collections.Counter()
    for s in sim["trace"]["samples"]:
        t[(s["toolpath_id"], s["source_intent"])] += s["segment_time_s"]
    OUT[name] = {f"tp{a} {b}": round(v, 1) for (a, b), v in sorted(t.items())}
    OUT[name]["total"] = round(sum(t.values()), 1)
    print(name, OUT[name])
json.dump(OUT, open(os.path.join(H, "time_split.json"), "w"), indent=1)
