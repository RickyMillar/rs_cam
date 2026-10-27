"""D49: a copy of fix33/area_oracle.py (writes into fix34). D47: a copy of fix31/area_oracle.py (writes fix33/area_*.npz). D45 copy of fix27/area_oracle.py (writes fix31/area_*.npz). D37: a copy of fix25/area_oracle.py ( one change: the screen bound is AREA_SCREEN, default 0.471; D37 reads with 0). D34 copy of fix24/area_oracle.py. C43 (C18): the exact load oracle, the depth-weighted version of C18's
exact_width_at (tests/adaptive3d_geodesic_arm_c12.rs on adaptive-instruments).

The definition (lead's ruling 2026-10-27, agreed with C22 §18.6): the
codebase's own chip cross-section, applied per lateral strip of the disc:
  A = sum over the lateral bins of mrr_cross_section_mm2(h(u), bin width)
  L = A / mrr_cross_section_mm2(d, D)
  mrr_cross_section_mm2: tool/mod.rs:508, ap * ae (a V-bit halves it,
  tool/vbit.rs:149; the factor cancels), the cross-section that the power
  gate multiplies by the feed (tool_load/power.rs:287).
  h(u): the deepest H - (z + p(r)) of the uncut disc points in the strip at
  lateral offset u; a point is uncut when that is > 0.01 mm (the 0.01 of
  the C12 oracle; p(r) the cutter's height above its tip at radius r,
  height_at_radius, 0 for the flat end mill of every fixture).
  The bin width is the lattice pitch (0.05 mm); d the op's Depth/Pass;
  bound 0.471 (the radial bound; at a full pass h = d over the width and
  L = ae / D, the width fraction of today).
Info column (not the definition): L_box = mrr_cross_section_mm2(ap, ae) /
mrr_cross_section_mm2(d, D), ap the deepest uncut height in the whole disc,
ae its lateral extent: the box that the sim screen reads
(radial_woc_fraction * axial_engagement_mm / d).

The geometry, as exact_width_at but in 3D:
- H is a height field on a lattice of pitch 0.05 mm (EXACT_PITCH_MM = sim
  cell / 10 of the C12 test) over the stock box; it starts at the stock top.
  A point outside the box has no stock.
- Each sample of op 1 (toolpath 1), in order, is read first, then its step
  (previous sample to it) is stamped: the flat end mill of radius R at the
  tip Z sets H = min(H, z) in its disc, at points every 0.3 mm or less along
  the step (Z linear along the step). So a read sees every earlier step in
  full and the current move up to the previous sample ('from'), as in
  exact_width_at.
- The read: the disc of radius R at the step midpoint, tip Z at the step
  midpoint; u along the left normal of the XY step; bins of one pitch.
  Also reported: the exact width w = (max u - min u) / D over the uncut
  points (exact_width_at's quantity, with the same 0.01 rule in 3D) and the
  largest uncut height hmax.
Which samples are read: the screen (README section 1): clearing_cut, cutting,
XY step > 1e-6, and either the sim radial > 0.471 (the old count) or the box
form radial * axial_engagement_mm / d > 0.471 (a deep cut the old count
missed). The sim box is the same quantity as L, read by the simulation
(radial_woc_fraction * D as ae, axial_engagement_mm as ap).

Usage: area_oracle.py <run dir under phase3> <corne|wanaka200> [toolpath id, default 1] [Depth/Pass, default op 1's]
Writes fix24/area_<run>[_tp<id>].npz (one row per read sample)."""
import json, math, sys, time
from pathlib import Path
import numpy as np

H0 = Path(__file__).resolve().parent; P3 = H0.parent
R = 3.0; D = 2 * R; PITCH = 0.05; UNCUT = 0.01; BOUND = 0.471; STAMP_STEP = 0.3
# D37: the screen bound, AREA_SCREEN in the environment (default BOUND). 0 reads every clearing cutting sample.
import os; SCREEN = float(os.environ.get("AREA_SCREEN", BOUND))
DPP = {"corne": 6.0, "wanaka200": 5.46}   # op 1 depth_per_pass in the project TOMLs (README section 2)


def mrr_cross_section(ap, ae):
    """MillingCutter::mrr_cross_section_mm2 of the flat end mill (tool/mod.rs:508)."""
    return ap * ae


def tip_profile(r):
    """The cutter's height above its tip at radius r (height_at_radius): 0 for a flat end mill."""
    return np.zeros_like(r)


def walk(S, lo, hi, d, read_all=False, read_tp=1):
    """The oracle on a sample list S (simulation.json sample records, trace
    order). lo / hi: the stock box; d: Depth/Pass. Every step stamps; a step
    joins two consecutive samples of the same toolpath (the first sample of a
    toolpath stamps alone). Only the samples of toolpath read_tp are read.
    read_all reads every step with XY travel (the synthetic checks); else
    the screen picks the steps."""
    # lattice points inside the box only (a point outside the box holds no stock)
    nx = int(math.floor((hi[0] - lo[0]) / PITCH + 1e-9)) + 1; ny = int(math.floor((hi[1] - lo[1]) / PITCH + 1e-9)) + 1
    Hf = np.full((ny, nx), hi[2], np.float32)
    k = int(round(R / PITCH)) + 1   # one lattice cell more than R: the exact disc round an off-lattice centre fits

    def window(x, y):
        ci = int(round((x - lo[0]) / PITCH)); cj = int(round((y - lo[1]) / PITCH))
        i0, i1, j0, j1 = ci - k, ci + k + 1, cj - k, cj + k + 1
        a0, a1, b0, b1 = max(i0, 0), min(i1, nx), max(j0, 0), min(j1, ny)
        if a0 >= a1 or b0 >= b1:
            return None
        g = (slice(b0, b1), slice(a0, a1))
        X = lo[0] + np.arange(a0, a1) * PITCH; Y = lo[1] + np.arange(b0, b1) * PITCH
        # the lattice points of the window within R of (x, y), exactly (as exact_width_at's stamp)
        return g, ((X[None, :] - x) ** 2 + (Y[:, None] - y) ** 2 <= R * R), X, Y

    def stamp(x, y, z):
        w = window(x, y)
        if w is None:
            return
        g, dsk, _, _ = w; sub = Hf[g]; msk = dsk & (sub > z)
        sub[msk] = z

    rows = []
    prev = None
    for n, s in enumerate(S):
        p = s["position"]
        if prev is not None and prev.get("toolpath_id", 1) != s.get("toolpath_id", 1):
            prev = None
        if prev is not None:
            q = prev["position"]; dx, dy = p[0] - q[0], p[1] - q[1]; L = math.hypot(dx, dy)
            e = s["engagement"]["radial_woc_fraction"]
            screen = s.get("toolpath_id", 1) == read_tp and (read_all and L > 1e-6 or s["is_cutting"] and s["source_intent"] == "clearing_cut" and L > 1e-6 and \
                (e > SCREEN or e * s["axial_engagement_mm"] / d > SCREEN))
            if screen:
                mx, my, mz = 0.5 * (p[0] + q[0]), 0.5 * (p[1] + q[1]), 0.5 * (p[2] + q[2])
                w = window(mx, my)
                if w is not None:
                    g, dsk, xs, ys = w
                    X, Y = np.meshgrid(xs, ys)
                    # float64: NumPy 2 keeps float32 - float in float32
                    ht = Hf[g].astype(np.float64) - (mz + tip_profile(np.hypot(X - mx, Y - my)))
                    sel = dsk & (ht > UNCUT)
                    sx, sy = -dy / L, dx / L
                    if sel.any():
                        u = (X[sel] - mx) * sx + (Y[sel] - my) * sy; h = ht[sel]
                        b = np.rint((u + R) / PITCH).astype(int)   # one bin per lattice offset (floor merges float-noise neighbours)
                        hb = np.zeros(int(round(2 * R / PITCH)) + 2); np.maximum.at(hb, b, h)
                        # A: the cross-section function per strip of one pitch
                        A = float(mrr_cross_section(hb, PITCH).sum()); w_ = float((u.max() - u.min()) / D); hm = float(h.max())
                    else:
                        A = w_ = hm = 0.0
                else:
                    A = w_ = hm = 0.0
                full = mrr_cross_section(d, D)
                rows.append((n, s["move_index"], p[0], p[1], p[2], L, e, s["axial_engagement_mm"], w_, hm, A, A / full,
                             s["removed_volume_est_mm3"], s.get("feed_rate_mm_min", 0.0), mrr_cross_section(hm, w_ * D) / full))
            # stamp the step prev -> s
            m_ = max(1, int(math.ceil(L / STAMP_STEP)))
            for t in range(1, m_ + 1):
                f = t / m_
                stamp(q[0] + f * dx, q[1] + f * dy, q[2] + f * (p[2] - q[2]))
        else:
            stamp(p[0], p[1], p[2])
        prev = s
    return rows


def run(rel, fx, tp=1, dpp=None):
    """tp: the toolpath id to read (op 1 = 1). Every sample before the end of
    toolpath tp, in trace order, stamps; so a later op (a rest op) is read on
    the stock that the earlier toolpaths of the trace leave. dpp: that op's
    Depth/Pass (default: op 1's)."""
    sim = json.load(open(P3 / rel / "simulation.json"))
    T = sim["trace"]["samples"]; last = max(i for i, s in enumerate(T) if s["toolpath_id"] == tp)
    S = T[:last + 1]
    lo, hi = sim["stock_bbox_min"], sim["stock_bbox_max"]
    if fx == "wanaka200":   # the samples are in the setup frame of the flipped stock (fix9/stockmap.py): x, y, z from 0
        lo, hi = [0.0, 0.0, 0.0], [hi[k] - lo[k] for k in range(3)]
    d = DPP[fx] if dpp is None else dpp; t0 = time.time()
    rows = walk(S, lo, hi, d, read_tp=tp)
    cols = "sample move x y z step_mm sim_radial sim_axial exact_w hmax area_mm2 L removed_mm3 feed L_box".split()
    arr = np.array(rows, float).reshape(-1, len(cols))
    tag = rel.replace("/", "_") + ("" if tp == 1 else f"_tp{tp}")
    # the op's base clearing feed: the most common feed of its clearing_cut cutting samples (load_split.py rule)
    fs = [round(s["feed_rate_mm_min"]) for s in S if s["toolpath_id"] == tp and s["is_cutting"] and s["source_intent"] == "clearing_cut"]
    base = max(set(fs), key=fs.count)
    np.savez_compressed(H0 / f"area_{tag}.npz", data=arr, cols=np.array(cols), dpp=d, n_samples=sum(1 for s in S if s["toolpath_id"] == tp), base_feed=base, toolpath=tp)
    print(f"{rel} tp {tp}: {sum(1 for s in S if s['toolpath_id'] == tp)} samples, {len(rows)} read, {time.time() - t0:.0f} s")


if __name__ == "__main__":
    run(sys.argv[1], sys.argv[2], int(sys.argv[3]) if len(sys.argv) > 3 else 1, float(sys.argv[4]) if len(sys.argv) > 4 else None)
