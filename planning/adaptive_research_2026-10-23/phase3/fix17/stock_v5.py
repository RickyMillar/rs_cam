"""C18/fix17 (copy of fix9/stockmap.py; the v5 columns; the part rasters of fix9): the stock after the first rough op, rebuilt from the closing
simulation's samples, and the part target it is read against.

simulation.json (`--sim-artifact`) holds the cut trace (one sample per
0.3 mm or 1.0 mm of path, with the tool tip position), not the simulated
stock. So the stock top after op 1 is rebuilt here: a flat end mill of
radius R lowers every cell whose centre lies within R of a tool tip to the
tip Z (path densified to STEP mm), from the stock top of the sim's stock
box. Frames: the sim's own frame (Corne: the model frame; wanaka200 setup 0,
face bottom up: x = xs - 20, y = 225 - ys, z_model = 7 - zs).

The target is the floor the tool can reach plus the axial leave: the
morphological closing of the part heightmap by a disc of R (the drop-cutter
height at each centre, then the lowest such height over the centres that
cover a cell), plus the leave. Writes stock_<fixture>_<column>.npz.
"""
import json, math, os, sys
import numpy as np
from scipy import ndimage

H = os.path.dirname(os.path.abspath(__file__))
P3 = os.path.join(H, "..")
R = 3.0
FIX = {
    "corne": {"grid": 0.25, "leave": 0.5, "part": "corne_part.npz",
              "cols": {"contour_parallel": "fix6/corne/contour_parallel", "zigzag_pick D20": "fix7/corne/geodesic_local_zigzag_pick",
                       "spiral_pick C35": "fix12/corne", "spiral_pick D24": "fix14/corne", "spiral_pick D25": "fix15/corne_pick"}},
    "wanaka200": {"grid": 0.5, "leave": 4.0, "part": "wanaka_part.npz",
                  "cols": {"contour_parallel": "fix6/wanaka200/contour_parallel", "zigzag_pick D20": "fix7/wanaka200/geodesic_local_zigzag_pick",
                           "spiral_pick C35": "fix12/wanaka200", "spiral_pick D24": "fix14/wanaka200", "spiral_pick D25": "fix15/wanaka200_pick"}},
}


def part_surface_sim(fx, xs, ys):
    """The part surface Z in the sim frame at sim-frame cell centres (no part: the stock bottom, 0)."""
    m = np.load(os.path.join(H, "..", "fix9", FIX[fx]["part"]))
    top, x0, y0, g = m["top"], float(m["x0"]), float(m["y0"]), float(m["g"])
    if fx == "corne":
        X, Y = xs, ys
    else:
        X, Y = xs - 20.0, 225.0 - ys
    i = np.round((X - x0) / g).astype(int); j = np.round((Y - y0) / g).astype(int)
    ok = (i >= 0) & (i < top.shape[1]) & (j >= 0) & (j < top.shape[0])
    s = np.zeros(xs.shape)
    t = np.full(xs.shape, -np.inf); t[ok] = top[j[ok], i[ok]]
    has = np.isfinite(t)
    if fx == "corne":
        s[has] = t[has]
    else:
        s[has] = 7.0 - t[has]   # the terrain seen from below, in the flipped setup
    return s


def build(fx, col):
    cfg = FIX[fx]; g = cfg["grid"]
    sim = json.load(open(os.path.join(P3, cfg["cols"][col], "simulation.json")))
    lo, hi = sim["stock_bbox_min"], sim["stock_bbox_max"]
    if fx == "wanaka200":   # the setup frame of the flipped stock: x from 0, z from 0
        lo, hi = [0.0, 0.0, 0.0], [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]]
    nx = int(round((hi[0] - lo[0]) / g)); ny = int(round((hi[1] - lo[1]) / g))
    xs = lo[0] + (np.arange(nx) + 0.5) * g; ys = lo[1] + (np.arange(ny) + 0.5) * g
    top = np.full((ny, nx), float(hi[2]))
    P = np.array([s["position"] for s in sim["trace"]["samples"] if s["toolpath_id"] == 1])
    step = g / 2
    seg = np.diff(P, axis=0); L = np.hypot(seg[:, 0], seg[:, 1])
    n = np.maximum(1, np.ceil(L / step).astype(int))
    idx = np.repeat(np.arange(len(seg)), n)
    f = (np.arange(n.sum()) - np.repeat(np.cumsum(n) - n, n) + 1) / np.repeat(n, n)
    D = np.vstack([P[:1], P[idx] + f[:, None] * seg[idx]])
    ci = np.floor((D[:, 0] - lo[0]) / g).astype(int); cj = np.floor((D[:, 1] - lo[1]) / g).astype(int)
    rr = int(math.ceil(R / g)) + 1
    for dj in range(-rr, rr + 1):
        for di in range(-rr, rr + 1):
            I, J = ci + di, cj + dj
            cx = lo[0] + (I + 0.5) * g; cy = lo[1] + (J + 0.5) * g
            m = ((cx - D[:, 0]) ** 2 + (cy - D[:, 1]) ** 2 <= R * R) & (I >= 0) & (I < nx) & (J >= 0) & (J < ny)
            if m.any():
                np.minimum.at(top, (J[m], I[m]), D[m, 2])
    XX, YY = np.meshgrid(xs, ys)
    surf = part_surface_sim(fx, XX, YY)
    k = int(math.ceil(R / g)); yy, xx = np.mgrid[-k:k + 1, -k:k + 1]
    disc = (xx * g) ** 2 + (yy * g) ** 2 <= R * R
    reach = ndimage.grey_erosion(ndimage.grey_dilation(surf, footprint=disc, mode="nearest"), footprint=disc, mode="nearest")
    target = reach + cfg["leave"]
    np.savez_compressed(os.path.join(H, f"stock_{fx}_{col.replace(' ', '_')}.npz"), top=top, surf=surf, target=target,
                        xs=xs, ys=ys, g=g, leave=cfg["leave"], stock_top=float(hi[2]))
    above = top - target
    cell = g * g
    print(f"{fx} {col}: grid {top.shape}, samples {len(P)}, area > 0.8 mm above target {np.sum(above > 0.8) * cell:.1f} mm2, "
          f"max {above.max():.2f} mm")


if __name__ == "__main__":
    for fx in (sys.argv[1:] or FIX):
        for col in FIX[fx]["cols"]:
            build(fx, col)
