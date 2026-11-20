"""P44 instrument: the wood that stands over the target in each of the seven places.

Input: a replay of gapmap.py (`gapmap.py replay <nc> <out.npz>`: one stock snapshot per op,
0.2 mm nodes, G-code frame). For each place (a window in the G-code frame) and each of two
moments (after the last 3D rough "Back rest 6 mm", and at the end of the setup):
  over    = nodes with stock - target > 0.3 mm
  fits    = of them, the nodes with stock - reach6h > 0.3 mm. reach6h is the lowest surface
            that a 6 mm flat tool makes over the target (gapmap.reach: the grey opening of
            the target by the 3 mm disc), with one change: a hole of the model (nodes with no
            triangle, enclosed by the model: the through openings in the control relief and
            under the slit) reads as far under the part, as the product reads it (the
            bounding-box floor), not as the stock top. So "fits" is wood that a 6 mm flat
            tool can cut and not go under the target; "over - fits" is the inside-corner
            radius and each floor narrower than the tool.
  tallest = the largest stock - reach6 in the place, and its node
  last    = the fits area by the last op that lowered the node (or "none": only the stock top)
usage: places.py <out.npz> [--csv file] [--png file]
"""
import sys, os, json
H = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(H, "..", "p37"))
from gapmap import *

# G-code frame windows (x0, x1, y0, y1). Place 6 is the board pocket less places 1 to 4.
PLACES = [
    ("1 antenna slot",   (243.5, 291.5, 54.0, 63.0)),
    ("2 control relief", (174.8, 190.2, 51.2, 65.8)),
    ("3 slit slot",      (48.7, 346.3, 91.5, 98.5)),
    ("4 ESP relief",     (138.5, 159.5, 39.0, 64.6)),
    ("5 step shelf",     (42.7, 352.0, 101.0, 118.5)),
    ("6 board pocket",   (46.7, 348.3, 41.0, 104.0)),
    ("7 cavity",         (43.0, 352.0, 118.5, 427.0)),
]


def masks():
    J, I = np.mgrid[0:NY, 0:NX]
    X, Y = I * CELL, J * CELL
    out = []; used = np.zeros((NY, NX), bool)
    T = target()
    for name, (x0, x1, y0, y1) in PLACES:
        m = (X >= x0 - 1e-9) & (X <= x1 + 1e-9) & (Y >= y0 - 1e-9) & (Y <= y1 + 1e-9)
        if name.startswith("5"):
            # the lead's window starts at Y 101; the board pocket floor (target under 12) in
            # Y 101..104 stays with place 6
            m &= (Y >= 104.0) | (T > 12.0)
        if name.startswith("6"):
            m &= ~used
        else:
            used |= m
        out.append((name, m))
    out.append(("other (outside the seven)", ~np.any([m for _, m in out], axis=0)))
    return out


def reach6h(T):
    rp = os.path.join(H, "reach6h.npy")
    if os.path.exists(rp):
        return np.load(rp)
    ok = np.isfinite(T)
    lab, n = ndimage.label(~ok)
    edge = set(np.unique(np.concatenate([lab[0], lab[-1], lab[:, 0], lab[:, -1]])))
    hole = (~ok) & ~np.isin(lab, list(edge))
    Th = np.where(hole, -50.0, T)          # an enclosed hole: far under the part
    R = reach(Th, 3.0)
    np.save(rp, R)
    return R


def measure(npz):
    d = np.load(npz); names = [str(n) for n in d["names"]]
    T = target(); ok = np.isfinite(T)
    R6 = reach6h(T)
    k2 = [i for i, n in enumerate(names) if n.startswith("Back rest")][0]
    # the last op that lowered each node
    last = np.full((NY, NX), -1, dtype=np.int16); prev = np.full((NY, NX), STOCK_TOP, dtype=np.float32)
    snaps = {}
    for k in range(len(names)):
        s = d[f"s{k}"]
        last[s < prev - 1e-4] = k; prev = s; snaps[k] = s
    rows = []
    for when, k in (("after id 2", k2), ("end of setup", len(names) - 1)):
        S = snaps[k].astype(float)
        gT = np.where(ok, S - T, 0.0); g6 = np.where(ok, S - R6, 0.0)
        lastk = np.where(last <= k, last, -2)
        if when == "after id 2":
            lastk = np.full((NY, NX), -1, dtype=np.int16); pv = np.full((NY, NX), STOCK_TOP, dtype=np.float32)
            for q in range(k + 1):
                lastk[snaps[q] < pv - 1e-4] = q; pv = snaps[q]
        for name, m in masks():
            over = (gT > BAR) & m; fits = (g6 > BAR) & m
            r = dict(when=when, place=name, over_mm2=over.sum() * CELL * CELL, fits_mm2=fits.sum() * CELL * CELL,
                     corner_mm2=(over & ~fits).sum() * CELL * CELL, tallest_mm=0.0, at=None, last={})
            if fits.any():
                g = np.where(fits, g6, -1); j, i = np.unravel_index(np.argmax(g), g.shape)
                r["tallest_mm"] = float(g[j, i]); r["at"] = (round(i * CELL, 1), round(j * CELL, 1), round(float(S[j, i]), 2), round(float(T[j, i]), 2))
                for q in np.unique(lastk[fits]):
                    r["last"]["none" if q < 0 else names[q][:28]] = round(float((fits & (lastk == q)).sum() * CELL * CELL), 1)
            rows.append(r)
    return rows, names


if __name__ == "__main__":
    rows, names = measure(sys.argv[1])
    for when in ("after id 2", "end of setup"):
        print(f"== {when}: wood more than {BAR} mm over the target, by place (mm2)")
        print(f"{'place':28s} {'over':>8s} {'fits':>8s} {'corner':>8s} {'tallest':>8s}  at (x, y, stock z, target z) ; fits area by the last op that cut the node")
        tot = [0.0, 0.0, 0.0]
        for r in rows:
            if r["when"] != when: continue
            tot[0] += r["over_mm2"]; tot[1] += r["fits_mm2"]; tot[2] += r["corner_mm2"]
            print(f"{r['place']:28s} {r['over_mm2']:8.1f} {r['fits_mm2']:8.1f} {r['corner_mm2']:8.1f} {r['tallest_mm']:8.2f}  {r['at']} ; {r['last']}")
        print(f"{'total':28s} {tot[0]:8.1f} {tot[1]:8.1f} {tot[2]:8.1f}")
    if "--json" in sys.argv:
        json.dump(rows, open(sys.argv[sys.argv.index("--json") + 1], "w"), indent=1)
