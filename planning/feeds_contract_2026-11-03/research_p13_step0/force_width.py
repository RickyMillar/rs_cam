"""The peak resultant and the mean force against the width at the add-door depth (D 6 hardwood).

Run: python3 -I force_width.py  Writes force_width.json. Feed per shipped speeds law on the
generic machine (cap 4000); the peak chip is the band chip at every width (chip first).
Also checks the offset-corner coverage limit of G33 by sampling.
"""
import json
import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import shipped_laws as L  # noqa: E402

d, z, ap = 6.0, 2, 1.5
ks, fe = L.force_line("GenericHardwood")
rows = []
for k in range(1, 21):
    ae = 0.05 * k * d
    rpm, feed = L.speeds(ae, d, z, 18000.0, 0.127, 4000.0, 8000.0)
    fz = feed / (rpm * z)
    mean, peak = L.edge_force(ap, ae, d, z, fz, ks, fe)
    rows.append({"ae_fr": round(0.05 * k, 3), "rpm": rpm, "feed": feed, "fz": fz, "mean_n": mean, "peak_n": peak,
                 "mrr": ae * ap * feed, "peak_per_mrr": peak / (ae * ap * feed)})


def corner_gap(s, r=1.0, n=400):
    """Largest uncovered point between two square offset loops of spacing s (sharp corners)."""
    worst = 0.0
    for i in range(n + 1):
        for j in range(n + 1):
            x, y = r + (s + r) * i / n, r + (s + r) * j / n  # region between the loops near the corner
            if x > r + s + 1e-12 and y > r + s + 1e-12:
                continue  # inside loop 2's path: cut by later loops
            d1 = min(abs(x - r) if y >= r else math.hypot(x - r, y - r), abs(y - r) if x >= r else math.hypot(x - r, y - r))
            ox, oy = r + s, r + s
            dx = abs(x - ox) if y >= oy else math.hypot(x - ox, y - oy)
            dy = abs(y - oy) if x >= ox else math.hypot(x - ox, y - oy)
            d2 = min(dx, dy)
            worst = max(worst, min(d1, d2) - r)
    return worst


lim = 1 + 1 / math.sqrt(2)
gap = {"limit_s_over_r": lim, "limit_s_over_d": lim / 2,
       "gap_below": corner_gap(lim - 0.02), "gap_above": corner_gap(lim + 0.02)}
flutes = []
for zz in (2, 3, 4):
    for app in (1.5, 6.0):
        flutes.append({"z": zz, "ap": app, "peak_n": {str(af): L.edge_force(app, af * d, d, zz, 0.127 * L.lift(af * d, d), ks, fe)[1]
                                                     for af in (0.1, 0.35, 0.5, 1.0)}})
json.dump({"rows": rows, "corner": gap, "flutes": flutes}, open(os.path.join(HERE, "force_width.json"), "w"), indent=1)
for r in rows:
    print(r["ae_fr"], round(r["fz"], 4), round(r["mean_n"], 2), round(r["peak_n"], 2), round(r["mrr"]))
print(gap)
