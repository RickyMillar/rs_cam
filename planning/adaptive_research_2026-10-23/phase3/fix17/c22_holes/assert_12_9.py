"""C22 §12.9: the three Corne hole floors. Per hole, the op-1 cutting samples of the
simulation within 1.5 mm of the hole centre, by level: D20 (fix7 zigzag_pick) cuts each
hole at the Z 0.5 level; C35 (fix12) and D24 (fix14) cut nothing there below Z 4.0."""
import json, os, math, collections
P3 = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..")
HOLES = [(28.3, -30.0), (80.3, -40.0), (45.6, -80.1)]
SEED = (55.87, -49.76)
def near(d):
    S = [s for s in json.load(open(os.path.join(P3, d, "simulation.json")))["trace"]["samples"] if s["toolpath_id"] == 1]
    out = []
    for h in HOLES:
        ss = [s for s in S if s["is_cutting"] and math.hypot(s["position"][0] - h[0], s["position"][1] - h[1]) < 1.5]
        out.append(collections.Counter(s["source_intent"] for s in ss if s["position"][2] < 1.0))
    return out
d20, c35, d24 = near("fix7/corne/geodesic_local_zigzag_pick"), near("fix12/corne"), near("fix14/corne")
for k, h in enumerate(HOLES):
    r = math.hypot(h[0] - SEED[0], h[1] - SEED[1])
    print(f"hole {h} r {r:.1f}: below Z 1.0 D20 {dict(d20[k])} C35 {dict(c35[k])} D24 {dict(d24[k])}")
    assert d20[k]["clearing_cut"] >= 5
    assert sum(c35[k].values()) == 0 and sum(d24[k].values()) == 0
assert [round(math.hypot(h[0] - SEED[0], h[1] - SEED[1]), 1) for h in HOLES] == [33.9, 26.3, 32.0]
print("all §12.9 claims hold")
