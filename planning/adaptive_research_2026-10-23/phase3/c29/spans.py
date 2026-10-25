# C29: each top-level entry span of Corne "3D Rough 1" (D17, phase3/fix4):
# its kind (plunge entry / keep-down link), its level, and the XY distance
# from the tool position before the span to the position after it.
import importlib.util, os, sys, json, collections, math
HERE = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location("fx", os.path.join(HERE, "..", "fix2", "fixture3d_v2.py"))
fx = importlib.util.module_from_spec(spec); spec.loader.exec_module(fx)
BASE = os.path.join(HERE, "..", "fix4", "corne")
def spans(arm):
    d = fx.load_op(os.path.join(BASE, arm), "tp_1_*.json", "3D Rough 1", os.path.join(BASE, arm))
    items = d["tp"]["semantic_trace"]["items"]
    ents = fx.top_level([it for it in items if it["kind"] == "entry"])
    gm, lev = d["moves"], d["lev"]
    out = []
    for e in sorted(ents, key=lambda s: s["move_start"]):
        lo, hi = e["move_start"], min(e["move_end"], len(gm) - 1)
        before = gm[lo - 1][2] if lo > 0 else gm[lo][1]
        after = gm[hi][2]
        kind = "keep" if "keep-down link" in e["label"] else "plunge"
        L = lev[hi] or lev[lo]
        out.append((L, kind, math.hypot(after[0] - before[0], after[1] - before[1]), before, after, e["label"]))
    return d, out
if __name__ == "__main__":
    for arm in sys.argv[1:]:
        d, out = spans(arm)
        per = collections.defaultdict(lambda: collections.Counter())
        for L, k, dist, *_ in out:
            per[L][k] += 1
            per[L][k + (">48" if dist > 48 else "<=48")] += 1
        print(arm)
        for L in d["levels"]:
            print("  ", L, dict(per[L]))
