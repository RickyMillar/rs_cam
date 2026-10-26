"""C43 (fix24): synthetic checks of area_oracle.walk. A straight pass along the
heading, a lift, then a parallel pass at stepover e on stock of height t over
the tip (z 10, d 6.0, R 3). The second pass is read 10 to 30 mm from its start;
the median L and w are kept. Writes synthetic.json."""
import json, sys
from pathlib import Path
import numpy as np
H = Path(__file__).resolve().parent; sys.path.insert(0, str(H))
from area_oracle import walk, R, D  # noqa: E402
CASES = [(2.0, 6.0, 0.0), (1.2, 6.0, 0.0), (2.0, 0.5, 0.0), (1.2, 3.0, 0.0), (2.0, 6.0, 0.5), (1.2, 1.0, 0.5), (2.0, 9.0, 0.5)]
DPP = 6.0


def smp(x, y, z, cut=True):
    return {"position": [x, y, z], "is_cutting": cut, "source_intent": "clearing_cut", "engagement": {"radial_woc_fraction": 0.0},
            "axial_engagement_mm": 0.0, "move_index": 0, "removed_volume_est_mm3": 0.0}


def case(e, t, ang):
    z = 10.0; top = z + t; c, s = np.cos(ang), np.sin(ang)
    P = lambda x, y: (x * c - y * s, x * s + y * c)
    xs = np.arange(0, 40.01, 0.3)
    S = [smp(*P(x, 0), z) for x in xs] + [smp(*P(40, 0), top + 1, False), smp(*P(0, e), top + 1, False)] + [smp(*P(x, e), z) for x in xs]
    rows = walk(S, [-30, -30, 0], [50, 50, top], DPP, read_all=True)
    r = [q for q in rows if q[0] > len(S) - len(xs) and 10 < np.hypot(q[2], q[3]) < 30]
    return {"e": e, "t": t, "ang": ang, "n": len(r), "L": round(float(np.median([q[11] for q in r])), 4), "L_expect": round(e * t / (D * DPP), 4),
            "w": round(float(np.median([q[8] for q in r])), 4), "w_expect": round(e / D, 4)}


if __name__ == "__main__":
    out = {"cases": [case(*k) for k in CASES], "R": R, "dpp": DPP}
    print(json.dumps(out)); json.dump(out, open(H / "synthetic.json", "w"), indent=1)
