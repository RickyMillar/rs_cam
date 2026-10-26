"""C42: can each lift-over descent of the C35 spiral_pick runs (fix12) be a
low travel over ground that is already cut?

For each descent of fix12/entries_fix12.py's selection (keep-down-link
spans whose entry_plunge samples descend 1 mm or more and remove more than
0.1 mm3: Corne op 1 1076, wanaka200 2016):

1. The lift point is the tool position at the start of the span (the end of
   the previous move, read from program.nc as fix11/audit.py maps it: move i
   is G-code command i + 1 of the op section). The plunge point is the last
   entry_plunge sample of the span; its Z is the level Z.
2. The stock top at that moment is rebuilt from the samples of toolpath 1
   before the span's first sample, as fix9/stockmap.py rebuilds it: a flat
   end mill of R 3 mm lowers each cell whose centre is within R of the
   densified path (step g/2) to the tip Z. Grid g: Corne 0.25 mm, wanaka200
   0.5 mm.
3. A tool centre cell is free when the highest stock top over the disc of R
   round its centre is at most the level Z + 0.5 mm (stay_down_clearance_mm
   of the project). The shortest path for the centre is a Dijkstra on the
   free cells with 16 neighbours (8 plus the knight steps, whose two
   crossed cells must be free too), in a window round the two points with a
   margin of 4 D = 24 mm.
4. The lift point starts the path when its cell is free (the start set is
   the free cells within 2 g of it). The plunge point is in stock (the
   descent removes stock), so the path ends at the reachable free cell
   nearest to it, and a straight side-entry leg of length s runs from there
   to the plunge point. Recorded: whether a path exists, the travel length
   (free path + s), the ratio to the straight distance, s, and the drop.

A low travel at the rapid rate (max_feed_mm_min, high-feedrate mode off in
all four project files) is the upper bound of what a low link could give.

Times: the plunge-feed time now is the sample time of the fed, vertical,
downward moves of the span (fix11/audit.py's 'plunge feed' class), on the
samples' own clock as fix11 reads it. The low travel is its length over the
link feed (link_feed_rate, 500 mm/min in both project files), and over the
cut feed (the feed the adaptive3d keep-down link runs at, path.rs:1576), with
no acceleration, the same basis as the sample clock.

C40 causes: each descent takes the cause of the nearest unused op-1
'C40 entry' line of the D25 build (fix15/<run>/stdout_plain.txt) within
0.5 mm in XY and 1.0 mm in Z, as fix15/c40_join.py joins. A different build:
a guide only.

Run: python3 -I lowtravel.py   (writes lowtravel.json; several minutes)
"""
import bisect, collections, importlib.util, json, math, os, re, sys
import numpy as np
from scipy import ndimage
from scipy.sparse import csr_matrix
from scipy.sparse.csgraph import dijkstra

H = os.path.dirname(os.path.abspath(__file__))
P3 = os.path.dirname(H)
# fix11/audit.py by its path (op_moves, GCODE_EPS); fix11 is not put on
# sys.path, so its make_readme.py cannot shadow this folder's.
_spec = importlib.util.spec_from_file_location("fix11_audit", os.path.join(P3, "fix11", "audit.py"))
A = importlib.util.module_from_spec(_spec); _spec.loader.exec_module(A)

R = 3.0
D = 6.0
CLEAR = 0.5
MARGIN = 4 * D
LINK_FEED = 500.0
_CORNE = dict(op="3D Rough 1", tp="tp_1_3D_Rough_1.json", grid=0.25, zoff=23.0, frame0=False,
              planes=[17.0, 11.0, 5.0, 0.5], cut_feed=1518.75, rapid=4000.0, stepover=1.2, dpp=6.0)
_WANAKA = dict(op="2 Back Rough", tp="tp_1_2_Back_Rough.json", grid=0.5, zoff=25.0, frame0=True,
               planes=[19.54, 14.08, 8.62, 4.0], cut_feed=1000.0, rapid=10000.0, stepover=2.0, dpp=5.46)
# The columns. spiral_pick (fix12): the lift-over descents of the C42
# brief (keep-down-link spans, the entries_fix12 rule). contour_parallel
# (fix6, the operator's daily strategy): every entry span, both styles,
# each tagged with the rule and with whether it holds a rapid.
FIX = {
    "corne spiral_pick": dict(_CORNE, fixture="corne", run="fix12/corne", c40="fix15/corne_pick/stdout_plain.txt", all_spans=False),
    "wanaka200 spiral_pick": dict(_WANAKA, fixture="wanaka200", run="fix12/wanaka200", c40="fix15/wanaka200_pick/stdout_plain.txt", all_spans=False),
    "corne contour_parallel": dict(_CORNE, fixture="corne", run="fix6/corne/contour_parallel", c40=None, all_spans=True),
    "wanaka200 contour_parallel": dict(_WANAKA, fixture="wanaka200", run="fix6/wanaka200/contour_parallel", c40=None, all_spans=True),
}
C40 = re.compile(r"generate_toolpath\{index=(\d)\}: \S+: C40 entry kind=(\w+) cause=(\S+) join=(\S+) x=(\S+) y=(\S+) z=(\S+) drop_mm=(\S+)")


class Stock:
    """The stock top, lowered in order by the samples (fix9/stockmap.py)."""

    def __init__(self, lo, hi, g):
        self.lo, self.g = lo, g
        self.nx = int(round((hi[0] - lo[0]) / g)); self.ny = int(round((hi[1] - lo[1]) / g))
        self.top = np.full((self.ny, self.nx), float(hi[2]))
        rr = int(math.ceil(R / g)) + 1
        self.offs = [(di, dj) for dj in range(-rr, rr + 1) for di in range(-rr, rr + 1)]

    def stamp(self, P):
        """Lower the cells under the polyline P (k x 3), densified at g/2."""
        g, lo = self.g, self.lo
        if len(P) == 1:
            Dn = P
        else:
            seg = np.diff(P, axis=0); L = np.hypot(seg[:, 0], seg[:, 1])
            n = np.maximum(1, np.ceil(L / (g / 2)).astype(int))
            idx = np.repeat(np.arange(len(seg)), n)
            f = (np.arange(n.sum()) - np.repeat(np.cumsum(n) - n, n) + 1) / np.repeat(n, n)
            Dn = np.vstack([P[:1], P[idx] + f[:, None] * seg[idx]])
        ci = np.floor((Dn[:, 0] - lo[0]) / g).astype(int); cj = np.floor((Dn[:, 1] - lo[1]) / g).astype(int)
        for di, dj in self.offs:
            I, J = ci + di, cj + dj
            cx = lo[0] + (I + 0.5) * g; cy = lo[1] + (J + 0.5) * g
            m = ((cx - Dn[:, 0]) ** 2 + (cy - Dn[:, 1]) ** 2 <= R * R) & (I >= 0) & (I < self.nx) & (J >= 0) & (J < self.ny)
            if m.any():
                np.minimum.at(self.top, (J[m], I[m]), Dn[m, 2])

    def cell(self, x, y):
        return int(math.floor((x - self.lo[0]) / self.g)), int(math.floor((y - self.lo[1]) / self.g))


def disc_footprint(g):
    k = int(math.ceil(R / g)); yy, xx = np.mgrid[-k:k + 1, -k:k + 1]
    return (xx * g) ** 2 + (yy * g) ** 2 <= R * R


STEPS = [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1),
         (2, 1), (2, -1), (-2, 1), (-2, -1), (1, 2), (1, -2), (-1, 2), (-1, -2)]


def shortest(free, starts, start_d, g):
    """Distances (mm) from the lift point over the free cells, 16 neighbours.
    A virtual source node joins each start cell at its distance from the
    lift point, so a path is never shorter than the straight line."""
    ny, nx = free.shape
    ids = np.arange(ny * nx).reshape(ny, nx)
    rows, cols, w = [], [], []
    for dx, dy in STEPS:
        ys0, ys1 = max(0, -dy), min(ny, ny - dy); xs0, xs1 = max(0, -dx), min(nx, nx - dx)
        a = free[ys0:ys1, xs0:xs1] & free[ys0 + dy:ys1 + dy, xs0 + dx:xs1 + dx]
        if abs(dx) == 2 or abs(dy) == 2:   # a knight step crosses two cells
            sx, sy = (int(math.copysign(1, dx)), 0) if abs(dx) == 2 else (0, int(math.copysign(1, dy)))
            c1 = free[ys0 + sy:ys1 + sy, xs0 + sx:xs1 + sx]
            c2 = free[ys0 + (dy - sy):ys1 + (dy - sy), xs0 + (dx - sx):xs1 + (dx - sx)]
            a = a & c1 & c2
        src = ids[ys0:ys1, xs0:xs1][a]; dst = ids[ys0 + dy:ys1 + dy, xs0 + dx:xs1 + dx][a]
        rows.append(src); cols.append(dst); w.append(np.full(src.shape, g * math.hypot(dx, dy)))
    N = ny * nx
    rows.append(np.full(len(starts), N)); cols.append(starts); w.append(start_d)
    G = csr_matrix((np.concatenate(w), (np.concatenate(rows), np.concatenate(cols))), shape=(N + 1, N + 1))
    dist = dijkstra(G, directed=True, indices=N)
    return dist[:N].reshape(ny, nx)


def side_leg(top, XX, YY, a, b, zl, g):
    """C42b: the tool disc swept along the side leg a -> b (XY), sampled as
    adaptive3d/clearing.rs:872-888 `corridor_ceiling_z` samples a corridor:
    n = ceil(len / (g / 2)) steps, both ends included, the highest stock
    top over the disc of R (cells whose centre is within R) at each step.
    Returns (ceiling Z, radial engagement): the engagement is how far the
    swept disc reaches into the stock that stands above the level Z +
    the stay-down clearance, R - (the least distance from a step to such a
    cell centre), at least 0."""
    ln = math.hypot(b[0] - a[0], b[1] - a[1])
    n = max(1, math.ceil(ln / (g / 2)))
    high = top > zl + CLEAR + 1e-9
    hx, hy = XX[high], YY[high]
    ceil, dmin = -math.inf, math.inf
    for k in range(n + 1):
        t = k / n
        x, y = a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t
        d2 = (XX - x) ** 2 + (YY - y) ** 2
        m = d2 <= R * R
        if m.any():
            ceil = max(ceil, float(top[m].max()))
        if len(hx):
            dmin = min(dmin, float(np.sqrt(((hx - x) ** 2 + (hy - y) ** 2).min())))
    return ceil, max(0.0, R - dmin)


def level_of(z, planes):
    below = [p for p in planes if p <= z + 0.35]
    return max(below) if below else min(planes)


def run(fx):
    cfg = FIX[fx]; g = cfg["grid"]
    rd = os.path.join(P3, cfg["run"])
    tp = json.load(open(os.path.join(rd, cfg["tp"])))
    ents = [e for e in tp["semantic_trace"]["items"] if e["kind"] == "entry"]
    spans = sorted([(e["move_start"], e["move_end"], e["params"]["values"]["style"]) for e in ents])
    params = {(e["move_start"], e["move_end"]): e["params"]["values"] for e in ents}
    sim = json.load(open(os.path.join(rd, "simulation.json")))
    lo, hi = sim["stock_bbox_min"], sim["stock_bbox_max"]
    if cfg["frame0"]:
        lo, hi = [0.0, 0.0, 0.0], [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]]
    S = [(s["move_index"], s["position"], s["source_intent"], s["removed_volume_est_mm3"], s["segment_time_s"])
         for s in sim["trace"]["samples"] if s["toolpath_id"] == 1]
    del sim
    # The G-code frame: the sim frame less the stock box corner on Corne
    # (the post's origin), the same XY on wanaka200 (its sim frame is the
    # setup frame from 0). Z: the sim frame less the stock top.
    xyoff = (lo[0], lo[1]) if not cfg["frame0"] else (0.0, 0.0)
    allm = A.op_moves(os.path.join(rd, "program.nc"), cfg["op"])
    mv = allm[1:tp["move_count"] + 1]
    # 1. the selection of fix12/entries_fix12.py (entry_plunge samples per span)
    byspan = collections.defaultdict(list)
    first = {}
    starts_ = [x[0] for x in spans]

    def span_of(m):
        """The first span in sorted order that holds move m (entries_fix12's
        `next(...)`), found by bisection over the starts."""
        i = bisect.bisect_right(starts_, m)
        hits = [x for x in spans[max(0, i - 8):i] if x[0] <= m <= x[1]]
        return hits[0] if hits else None

    for k, (m, p, it, vol, dt) in enumerate(S):
        sp = span_of(m)
        if sp is not None and sp not in first:
            first[sp] = k
        if it == "entry_plunge":
            byspan[sp].append(k)
    def rule(ks):
        z = [S[k][1][2] for k in ks]
        return bool(ks) and max(z) - z[-1] >= 1.0 and sum(S[k][3] for k in ks) > 0.1

    sel = []
    if cfg["all_spans"]:
        for sp in spans:
            if sp in first:
                sel.append((first[sp], sp, byspan.get(sp, [])))
    else:
        for sp, ks in byspan.items():
            if sp is not None and sp[2] == "keep-down link" and rule(ks):
                sel.append((first[sp], sp, ks))
    sel.sort()
    # C40 pool (op index 0 of the D25 build)
    pool = []
    for ln in (open(os.path.join(P3, cfg["c40"])) if cfg["c40"] else []):
        mm = C40.search(ln)
        if mm and mm.group(1) == "0":
            pool.append(dict(kind=mm.group(2), cause=mm.group(3), x=float(mm.group(5)), y=float(mm.group(6)), z=float(mm.group(7))))
    used = set()
    st = Stock(lo, hi, g)
    foot = disc_footprint(g)
    P = np.array([s[1] for s in S])
    done = 0
    rows = []
    for k0, sp, ks in sel:
        if k0 > done:
            st.stamp(P[max(done - 1, 0):k0])
            done = k0
        a = mv[sp[0]]["a"]
        lift = (a[0] + xyoff[0], a[1] + xyoff[1], a[2] + cfg["zoff"])
        pv = params[(sp[0], sp[1])]
        if ks:
            pl = S[ks[-1]][1]
            # The level Z: the first sample after the last entry_plunge sample
            # that is neither entry_plunge nor retract (the cut the descent
            # serves). The last plunge sample stands about 0.5 mm above it.
            kz = next((k for k in range(ks[-1] + 1, len(S)) if S[k][2] not in ("entry_plunge", "retract")), ks[-1])
            zl = S[kz][1][2]
            drop = max(S[k][1][2] for k in ks) - zl
        else:
            # No fed descent in the span: the span's own entry point.
            pl = (pv["entry_x"], pv["entry_y"], pv["entry_z"])
            zl, drop = pv["entry_z"], 0.0
        param_off = math.hypot(pv["entry_x"] - pl[0], pv["entry_y"] - pl[1])
        straight = math.hypot(pl[0] - lift[0], pl[1] - lift[1])
        # plunge-feed time now: fed, vertical, downward moves of the span
        pf = 0.0
        span_t = 0.0
        for k in range(k0, len(S)):
            m = S[k][0]
            if m > sp[1]:
                break
            span_t += S[k][4]
            q = mv[m]
            if q["g"] != 0 and math.hypot(q["b"][0] - q["a"][0], q["b"][1] - q["a"][1]) < A.GCODE_EPS and q["b"][2] - q["a"][2] < -A.GCODE_EPS:
                pf += S[k][4]
        has_rapid = any(mv[m]["g"] == 0 for m in range(sp[0], sp[1] + 1))
        row = dict(first_sample=k0, style=sp[2], into_stock=rule(ks), retract=has_rapid, param_off_mm=round(param_off, 3), lift=[round(c, 3) for c in lift], plunge=[round(c, 3) for c in pl], level_z=round(zl, 3), level=level_of(zl, cfg["planes"]),
                   drop_mm=round(drop, 3), straight_mm=round(straight, 3), plunge_feed_s=pf, span_s=span_t,
                   lift_above=lift[2] > zl + CLEAR + 1e-6)
        # C40 cause
        best, bd = None, 0.5
        for i, e in enumerate(pool):
            if i in used:
                continue
            d = math.hypot(e["x"] - pl[0], e["y"] - pl[1])
            if d <= bd and abs(e["z"] - zl) <= 1.0:
                best, bd = i, d
        if best is not None:
            used.add(best)
        row["cause"] = pool[best]["cause"] if best is not None else ("unmatched" if pool else "n/a")
        # window
        x0 = min(lift[0], pl[0]) - MARGIN; x1 = max(lift[0], pl[0]) + MARGIN
        y0 = min(lift[1], pl[1]) - MARGIN; y1 = max(lift[1], pl[1]) + MARGIN
        i0, j0 = st.cell(x0, y0); i1, j1 = st.cell(x1, y1)
        i0, j0 = max(i0, 0), max(j0, 0); i1, j1 = min(i1, st.nx - 1), min(j1, st.ny - 1)
        top = st.top[j0:j1 + 1, i0:i1 + 1]
        high = ndimage.grey_dilation(top, footprint=foot, mode="constant", cval=-np.inf)
        free = high <= zl + CLEAR + 1e-9
        ny, nx = free.shape
        xs = st.lo[0] + (np.arange(i0, i1 + 1) + 0.5) * g; ys = st.lo[1] + (np.arange(j0, j1 + 1) + 0.5) * g
        XX, YY = np.meshgrid(xs, ys)
        near_lift = (XX - lift[0]) ** 2 + (YY - lift[1]) ** 2 <= (2 * g) ** 2
        starts = np.flatnonzero((near_lift & free).ravel())
        row["lift_free"] = bool(len(starts))
        dpl = np.hypot(XX - pl[0], YY - pl[1])
        row["plunge_free"] = bool(free[np.unravel_index(np.argmin(dpl), dpl.shape)])
        row["path"] = False
        if len(starts) and not row["lift_above"]:
            start_d = np.hypot(XX - lift[0], YY - lift[1]).ravel()[starts]
            dist = shortest(free, starts, start_d, g)
            ok = np.isfinite(dist)
            if ok.any():
                # the reachable free cell nearest the plunge point; ties: the shorter path
                key = np.where(ok, dpl, np.inf)
                s = key.min()
                cand = ok & (key <= s + 1e-9)
                c = np.unravel_index(np.argmin(np.where(cand, dist, np.inf)), dist.shape)
                travel = float(dist[c]) + float(s)
                row.update(path=True, side_mm=float(s), side_le_stepover=bool(s <= cfg["stepover"] + 1e-9), free_path_mm=float(dist[c]), travel_mm=travel,
                           ratio=travel / straight if straight > 1e-9 else None,
                           low_link_s=travel / LINK_FEED * 60.0, low_cut_s=travel / cfg["cut_feed"] * 60.0,
                           low_rapid_s=travel / cfg["rapid"] * 60.0)
                cxy = (float(XX[c]), float(YY[c]))
                lc, ae = side_leg(top, XX, YY, cxy, pl, zl, g)
                ap = lc - zl
                row.update(leg_from=[round(cxy[0], 4), round(cxy[1], 4)], leg_ceiling_z=lc, leg_ap_mm=ap,
                           leg_ap_ok=bool(ap <= cfg["dpp"] + CLEAR + 1e-9), leg_ae_mm=ae,
                           leg_ae_ok=bool(ae <= cfg["stepover"] + 1e-9))
        rows.append(row)
        if len(rows) % 200 == 0:
            print(fx, len(rows), "of", len(sel), flush=True)
    # The op time on the samples' clock (the clock of span_s).
    return dict(selected=len(sel), c40_pool=len(pool), runtime_s=sum(x[4] for x in S), rows=rows)


if __name__ == "__main__":
    path = os.path.join(H, "lowtravel.json")
    out = json.load(open(path)) if sys.argv[1:] and os.path.exists(path) else {}
    for fx in (sys.argv[1:] or FIX):
        out[fx] = run(fx)
    json.dump(out, open(path, "w"), indent=1)
