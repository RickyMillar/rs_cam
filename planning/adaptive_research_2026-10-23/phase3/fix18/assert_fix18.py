"""C42: assert every number of README.md (quotes.json lists them).
Run: python3 -I assert_fix18.py   (about 2 minutes)

1. lowtravel.run again for both fixtures from the raw files (not
   lowtravel.json); make_readme.facts on that fresh run gives each quoted
   value, and each quoted text stands in README.md.
2. The selection: spiral_pick equals fix12/entries_fix12.json's count;
   contour_parallel equals the op's entry spans in its tp file.
3. The frames: each sample of a vertical G-code move lies at the G-code XY
   plus the stated offset.
4. The stock rebuild stamped in one call over the samples before a descent
   equals the incremental rebuild (two calls), cell for cell.
5. The paths, for 8 descents per fixture, by other means: the 8-connected
   components of the free mask (scipy.ndimage.label) give the same answer
   on whether a path exists, and the nearest free cell of the lift's
   component lies at the side leg's distance from the plunge point.
6. Every path: travel = free path + side leg, travel >= straight distance.
"""
import json, math, os, sys
import numpy as np
from scipy import ndimage
from scipy.spatial import cKDTree

H = os.path.dirname(os.path.abspath(__file__))
P3 = os.path.dirname(H)
sys.path.insert(0, H)
import lowtravel as L  # noqa: E402
import make_readme as MR  # noqa: E402

FAIL = []


def check(ok, msg):
    if not ok:
        FAIL.append(msg)


# 1
fresh = {fx: L.run(fx) for fx in L.FIX}
F = MR.facts(fresh)
quotes = json.load(open(os.path.join(H, "quotes.json")))
readme = open(os.path.join(H, "README.md")).read()
for k, qv in quotes.items():
    if k not in F:
        check(False, f"{k}: no fact"); continue
    check(abs(F[k] - qv["value"]) <= 1e-9 * max(1.0, abs(qv["value"])), f"{k}: {F[k]} vs {qv['value']}")
    check(format(F[k], qv["fmt"]) == qv["text"], f"{k} text")
    check(qv["text"] in readme, f"{k}: '{qv['text']}' not in README")

# 2
E = json.load(open(os.path.join(P3, "fix12", "entries_fix12.json")))
for fx, col in (("corne spiral_pick", "corne spiral_pick C35"), ("wanaka200 spiral_pick", "wanaka200 spiral_pick C35")):
    check(fresh[fx]["selected"] == E[col]["descent_1mm_removing_stock"]["keep-down link"], f"{fx} selection")
for fx in ("corne contour_parallel", "wanaka200 contour_parallel"):
    cfg = L.FIX[fx]
    tp = json.load(open(os.path.join(P3, cfg["run"], cfg["tp"])))
    n = sum(1 for e in tp["semantic_trace"]["items"] if e["kind"] == "entry")
    check(fresh[fx]["selected"] == n, f"{fx} selection {fresh[fx]['selected']} vs {n} entry spans")

# 3, 4, 5
for fx, cfg in L.FIX.items():
    rd = os.path.join(P3, cfg["run"])
    tp = json.load(open(os.path.join(rd, cfg["tp"])))
    sim = json.load(open(os.path.join(rd, "simulation.json")))
    lo, hi = sim["stock_bbox_min"], sim["stock_bbox_max"]
    if cfg["frame0"]:
        lo, hi = [0.0, 0.0, 0.0], [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]]
    S = [(s["move_index"], s["position"]) for s in sim["trace"]["samples"] if s["toolpath_id"] == 1]
    del sim
    mv = L.A.op_moves(os.path.join(rd, "program.nc"), cfg["op"])[1:tp["move_count"] + 1]
    off = (lo[0], lo[1]) if not cfg["frame0"] else (0.0, 0.0)
    nv = bad = 0
    for m, p in S:
        q = mv[m]
        if math.hypot(q["b"][0] - q["a"][0], q["b"][1] - q["a"][1]) < L.A.GCODE_EPS:
            nv += 1
            if math.hypot(p[0] - (q["a"][0] + off[0]), p[1] - (q["a"][1] + off[1])) > 2e-3:
                bad += 1
    check(nv > 1000 and bad == 0, f"{fx} frame: {bad} of {nv} vertical samples off")
    P = np.array([p for _, p in S])
    rows = fresh[fx]["rows"]
    pick = [rows[i] for i in np.linspace(0, len(rows) - 1, 8).astype(int)]
    foot = L.disc_footprint(cfg["grid"])
    for r in pick:
        k0 = r["first_sample"]
        one = L.Stock(lo, hi, cfg["grid"]); one.stamp(P[:k0])
        two = L.Stock(lo, hi, cfg["grid"]); h = k0 // 2
        two.stamp(P[:h]); two.stamp(P[max(h - 1, 0):k0])
        check(np.array_equal(one.top, two.top), f"{fx} stamp at {k0}")
        if r["lift_above"]:
            check(r["lift"][2] > r["level_z"] + L.CLEAR, f"{fx} lift_above at {k0}")
            continue
        lift, pl, zl, g = r["lift"], r["plunge"], r["level_z"], cfg["grid"]
        i0, j0 = one.cell(min(lift[0], pl[0]) - L.MARGIN, min(lift[1], pl[1]) - L.MARGIN)
        i1, j1 = one.cell(max(lift[0], pl[0]) + L.MARGIN, max(lift[1], pl[1]) + L.MARGIN)
        i0, j0 = max(i0, 0), max(j0, 0); i1, j1 = min(i1, one.nx - 1), min(j1, one.ny - 1)
        free = ndimage.grey_dilation(one.top[j0:j1 + 1, i0:i1 + 1], footprint=foot, mode="constant", cval=-np.inf) <= zl + L.CLEAR + 1e-9
        lab, _ = ndimage.label(free, structure=np.ones((3, 3)))
        xs = one.lo[0] + (np.arange(i0, i1 + 1) + 0.5) * g; ys = one.lo[1] + (np.arange(j0, j1 + 1) + 0.5) * g
        XX, YY = np.meshgrid(xs, ys)
        near = free & ((XX - lift[0]) ** 2 + (YY - lift[1]) ** 2 <= (2 * g) ** 2)
        comps = set(lab[near].tolist()) - {0}
        check(bool(comps) == r["path"], f"{fx} path existence at {k0}")
        if comps and r["path"]:
            inc = np.isin(lab, list(comps))
            s = np.hypot(XX - pl[0], YY - pl[1])[inc].min()
            check(abs(s - r["side_mm"]) <= 2e-3, f"{fx} side leg at {k0}: {s} vs {r['side_mm']}")
            # C42b by other means: the leg sampled 4 times finer, with a
            # KD tree over the cell centres. The finer reading can only be
            # higher; the engagement moves by at most a quarter step (g / 4).
            top = one.top[j0:j1 + 1, i0:i1 + 1]
            pts = np.c_[XX.ravel(), YY.ravel()]
            tree = cKDTree(pts)
            hi_ = (top > zl + L.CLEAR + 1e-9).ravel()
            htree = cKDTree(pts[hi_]) if hi_.any() else None
            a_, b_ = r["leg_from"], pl
            ln = math.hypot(b_[0] - a_[0], b_[1] - a_[1]); n = max(1, math.ceil(ln / (g / 8)))
            ceil_f, dmin = -math.inf, math.inf
            for t in np.linspace(0, 1, n + 1):
                x, y = a_[0] + (b_[0] - a_[0]) * t, a_[1] + (b_[1] - a_[1]) * t
                idx = tree.query_ball_point([x, y], L.R + 1e-12)
                if idx:
                    ceil_f = max(ceil_f, float(top.ravel()[idx].max()))
                if htree is not None:
                    dmin = min(dmin, htree.query([x, y])[0])
            ae_f = max(0.0, L.R - dmin)
            check(ceil_f >= r["leg_ceiling_z"] - 1e-9, f"{fx} leg ceiling at {k0}")
            check((ceil_f - zl <= L.FIX[fx]["dpp"] + L.CLEAR + 1e-9) == r["leg_ap_ok"], f"{fx} leg ap flag at {k0}: {ceil_f - zl} vs {r['leg_ap_mm']}")
            check(-1e-9 <= ae_f - r["leg_ae_mm"] <= g / 4 + 1e-9, f"{fx} leg ae at {k0}: {ae_f} vs {r['leg_ae_mm']}")

# 6
for fx in L.FIX:
    for r in fresh[fx]["rows"]:
        if r["path"]:
            check(abs(r["travel_mm"] - r["free_path_mm"] - r["side_mm"]) < 1e-9, "travel sum")
            cfg = L.FIX[fx]
            for key, feed in (("low_link_s", L.LINK_FEED), ("low_cut_s", cfg["cut_feed"]), ("low_rapid_s", cfg["rapid"])):
                check(abs(r[key] - r["travel_mm"] / feed * 60.0) < 1e-9, f"{fx} {key}")
            check(r["travel_mm"] >= r["straight_mm"] - 2e-3, f"{fx} travel < straight at {r['first_sample']}")
            check(r["leg_ap_ok"] == (r["leg_ap_mm"] <= cfg["dpp"] + L.CLEAR + 1e-9) and r["leg_ae_ok"] == (r["leg_ae_mm"] <= cfg["stepover"] + 1e-9), f"{fx} leg flags")
            check(0.0 <= r["leg_ae_mm"] <= L.R, f"{fx} leg ae range")

if FAIL:
    print(f"FAIL ({len(FAIL)})")
    for m in FAIL[:40]:
        print(" -", m)
    sys.exit(1)
print(f"OK: {len(quotes)} quotes; selection, frames, stamp, 32 path and swept-leg checks, every path's sum, bound, times and leg flags")
