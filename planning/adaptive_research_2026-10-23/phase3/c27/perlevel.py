# C27: per level of Corne "3D Rough 1", zigzag against zigzag_v2: entries
# (plunge rings), cut XY mm, keep-down mm, from the D-FIX2 / D16b runs.
import importlib.util, os, sys, json, collections, math
HERE = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location("fx", os.path.join(HERE, "..", "fix2", "fixture3d_v2.py"))
fx = importlib.util.module_from_spec(spec); spec.loader.exec_module(fx)
BASE = os.path.join(HERE, "..", "fix2", "corne")
# The shared loader reads each move's intent from the run's simulation.json
# (its fourth argument). The C27 runs: zigzag at 31935b89 (D-FIX2; its
# sim-artifact rerun under fix2/simart has a byte-equal program.nc) and
# zigzag_v2 at 4cceca914 (D16b, run with --sim-artifact in place).
SIMART = {
    "geodesic_local_zigzag": os.path.join(HERE, "..", "fix2", "simart", "corne", "geodesic_local_zigzag"),
    "geodesic_local_zigzag_v2": os.path.join(BASE, "geodesic_local_zigzag_v2"),
}
def per_level(arm):
    d = fx.load_op(os.path.join(BASE, arm), "tp_1_*.json", "3D Rough 1", SIMART[arm])
    rows = collections.OrderedDict((l, [0, 0.0, 0.0, 0]) for l in d["levels"])
    for (g, a, b), c, l in zip(d["moves"], d["cls"], d["lev"]):
        if l is None: continue
        if c == "cut": rows[l][1] += fx.xy(a, b)
        if c == "link": rows[l][2] += fx.xy(a, b)
        if c == "cut" and fx.xy(a, b) > 1e-6: rows[l][3] += 1
    for i, p in d["rings"]:
        rows[d["lev"][i]][0] += 1
    return d, rows
if __name__ == "__main__":
    out = {}
    for arm in ("geodesic_local_zigzag", "geodesic_local_zigzag_v2"):
        d, rows = per_level(arm)
        out[arm] = rows
        print(arm, "entries", d["entries_plunge"], "keep-down spans", d["entries_keepdown"], "moves", len(d["moves"]))
    a, b = out["geodesic_local_zigzag"], out["geodesic_local_zigzag_v2"]
    print(f'{"level":10s} {"ent zz":>6s} {"ent v2":>6s} {"cut zz":>8s} {"cut v2":>8s} {"kd zz":>7s} {"kd v2":>7s}')
    for l in a:
        x, y = a[l], b.get(l, [0, 0, 0, 0])
        print(f'{l:10s} {x[0]:6d} {y[0]:6d} {x[1]:8.0f} {y[1]:8.0f} {x[2]:7.0f} {y[2]:7.0f}')
