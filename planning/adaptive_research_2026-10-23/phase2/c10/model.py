"""C10: a Python model of the C8 lead-in fit test on the star and pocket.

Exact geodesic field (visibility + one wrap round a convex disc obstacle),
the curvature-law levels, contour arcs, the lobe rules, and a port of
`lead_band` / `lead_in_onto` from research_geodesic.rs at 609ce75b.
Every number comes from R, s, the cell, the tolerance and c*D.
"""
import math
import sys
import numpy as np
import contourpy
from matplotlib.path import Path

R = 3.0
S = 2.0
CD = 0.3626 * 2 * R  # c*D, 2.1756 mm
CELL = max(R / 6.0, 0.1)  # planner_cell_size: R / 6 vs tolerance 0.1
REACH = S + (1 + math.sqrt(2)) * CELL
RATIO = (R + 2 * S) / (R + S)
GRID = 0.1  # model grid (finer than the planner lattice)
H2 = lambda d: d * (2 * R - d)


def w_front(rho, delta):
    a = rho - delta + R
    return R - (a * a - R * R - rho * rho) / (2 * rho)


def rho_star():
    return S * (2 * R - S) / (2 * (CD - S))


def law_levels(max_rho):
    out = [R]
    rho = R
    b = R - CD
    while True:
        nxt = -b + math.sqrt(b * b + (rho + R) ** 2 - R * R)
        nxt = min(nxt, rho + S)
        if nxt > max_rho:
            break
        out.append(nxt)
        rho = nxt
    return out


def lead_in_radius(rho, delta):
    m = CD - w_front(rho, delta)
    if m <= 0 or delta <= 0:
        return None, m
    h2 = delta * max(2 * R - delta, 0)
    return R + max(h2 - m * m, 0) / (2 * m), m


def lead_in_radius_pivot(rho, delta):
    st, m = lead_in_radius(rho, delta)
    if st is None:
        return None, m, None
    return st * rho / (rho + st), m, st


# ---- shapes ---------------------------------------------------------------

def seg_dist(px, py, ax, ay, bx, by):
    dx, dy = bx - ax, by - ay
    L2 = dx * dx + dy * dy
    t = np.clip(((px - ax) * dx + (py - ay) * dy) / L2, 0, 1)
    return np.hypot(px - (ax + t * dx), py - (ay + t * dy))


class Star:
    name = "star"
    pts = np.array([(50, 5), (61, 35), (95, 35), (68, 57), (79, 91), (50, 70),
                    (21, 91), (32, 57), (5, 35), (39, 35)], float)
    reflex = pts[1::2]

    def __init__(self):
        self.path = Path(self.pts)

    def boundary_dist(self, x, y):
        d = np.full(np.shape(x), np.inf)
        n = len(self.pts)
        for i in range(n):
            a, b = self.pts[i], self.pts[(i + 1) % n]
            d = np.minimum(d, seg_dist(x, y, a[0], a[1], b[0], b[1]))
        return d

    def inside_part(self, x, y):
        sh = np.shape(x)
        return self.path.contains_points(np.c_[np.ravel(x), np.ravel(y)]).reshape(sh)

    def region(self, x, y):
        return self.inside_part(x, y) & (self.boundary_dist(x, y) >= R)

    discs = property(lambda self: [(v, R) for v in self.reflex])


class Pocket:
    name = "pocket"
    O = np.array([40.0, 30.0])

    def boundary_dist_part(self, x, y):
        # rounded rect (5..75, 5..55, r 8) minus disc (40,30) r 10
        cx = np.clip(x, 13, 67)
        cy = np.clip(y, 13, 47)
        inner = np.minimum.reduce([x - 5, 75 - x, y - 5, 55 - y])
        corner = 8 - np.hypot(x - cx, y - cy)
        in_corner = (np.abs(x - cx) > 0) & (np.abs(y - cy) > 0)
        d = np.where(in_corner, corner, inner)
        di = np.hypot(x - 40, y - 30) - 10
        return np.minimum(d, di)

    def region(self, x, y):
        return self.boundary_dist_part(x, y) >= R

    discs = property(lambda self: [(self.O, 13.0)])


def edt_seed(shape):
    xs = np.arange(0, 100, 0.05)
    X, Y = np.meshgrid(xs, xs)
    if shape.name == "star":
        d = np.where(shape.inside_part(X, Y), shape.boundary_dist(X, Y), -1)
    else:
        d = np.where(shape.boundary_dist_part(X, Y) > 0, shape.boundary_dist_part(X, Y), -1)
    i = np.argmax(d)
    return np.array([X.flat[i], Y.flat[i]]), d.flat[i]


# ---- geodesic field: seed visibility, else one wrap round a disc ----------

def field(shape, seed, X, Y):
    sx, sy = seed
    d = np.hypot(X - sx, Y - sy)
    px = np.full(X.shape, sx)
    py = np.full(X.shape, sy)
    label = np.zeros(X.shape, int)
    blocked = np.zeros(X.shape, bool)
    best = np.full(X.shape, np.inf)
    for k, (O, r) in enumerate(shape.discs):
        ox, oy = O
        hit = seg_dist(ox, oy, sx, sy, X, Y) < r - 1e-9
        blocked |= hit
        vs = np.array([sx - ox, sy - oy])
        ds = np.hypot(*vs)
        vx, vy = X - ox, Y - oy
        dx = np.hypot(vx, vy)
        cosang = np.clip((vs[0] * vx + vs[1] * vy) / (ds * dx + 1e-300), -1, 1)
        ang = np.arccos(cosang)
        gam = ang - math.acos(r / ds) - np.arccos(np.clip(r / dx, -1, 1))
        tx = np.sqrt(np.maximum(dx * dx - r * r, 0))
        dw = math.sqrt(ds * ds - r * r) + r * np.maximum(gam, 0) + tx
        side = np.sign(vs[0] * vy - vs[1] * vx)  # which way round
        # tangent point from x on the side toward the seed
        phi = np.arctan2(vy, vx) - side * np.arccos(np.clip(r / dx, -1, 1))
        tpx, tpy = ox + r * np.cos(phi), oy + r * np.sin(phi)
        take = hit & (dw < best)
        best = np.where(take, dw, best)
        px = np.where(take, tpx, px)
        py = np.where(take, tpy, py)
        label = np.where(take, 1 + 2 * k + (side > 0), label)
    d = np.where(blocked, best, d)
    return d, px, py, label


class Field:
    def __init__(self, shape):
        self.shape = shape
        self.seed, self.clear_part = edt_seed(shape)
        xs = np.arange(-2, 102, GRID)
        self.xs = xs
        X, Y = np.meshgrid(xs, xs)
        self.inside = shape.region(X, Y)
        d, px, py, lab = field(shape, self.seed, X, Y)
        self.d = d
        self.px, self.py, self.lab = px, py, lab
        dm = np.where(self.inside, d, np.nan)
        self.gen = contourpy.contour_generator(xs, xs, dm, name="serial", corner_mask=True)
        self.dmax = np.nanmax(dm)

    def idx(self, p):
        i = int(round((p[1] - self.xs[0]) / GRID))
        j = int(round((p[0] - self.xs[0]) / GRID))
        i = min(max(i, 0), len(self.xs) - 1)
        j = min(max(j, 0), len(self.xs) - 1)
        return i, j

    def pivot(self, p):
        i, j = self.idx(p)
        return np.array([self.px[i, j], self.py[i, j]])

    def label(self, p):
        i, j = self.idx(p)
        return self.lab[i, j]

    def inside_pt(self, p):
        return bool(self.shape.region(np.array([p[0]]), np.array([p[1]]))[0])

    def seg_inside(self, a, b):
        n = max(2, int(math.ceil(math.hypot(*(b - a)) / 0.05)) + 1)
        t = np.linspace(0, 1, n)
        x = a[0] + t * (b[0] - a[0])
        y = a[1] + t * (b[1] - a[1])
        return bool(self.shape.region(x, y).all())


# ---- polylines ---------------------------------------------------------------

def plen(p):
    return float(np.sum(np.hypot(*np.diff(p, axis=0).T))) if len(p) > 1 else 0.0


def subdivide(p, step):
    out = [p[0]]
    for a, b in zip(p[:-1], p[1:]):
        n = max(1, int(math.ceil(math.hypot(*(b - a)) / step)))
        for k in range(1, n + 1):
            out.append(a + (b - a) * k / n)
    return np.array(out)


def nearest_on(poly, p):
    a = poly[:-1]
    b = poly[1:]
    d = b - a
    L2 = np.sum(d * d, axis=1)
    t = np.where(L2 > 0, np.clip(np.sum((p - a) * d, axis=1) / np.where(L2 > 0, L2, 1), 0, 1), 0)
    q = a + d * t[:, None]
    dist = np.hypot(*(q - p).T)
    i = int(np.argmin(dist))
    return q[i], float(dist[i])


def resample(p, m):
    s = np.concatenate([[0], np.cumsum(np.hypot(*np.diff(p, axis=0).T))])
    t = np.linspace(0, s[-1], m)
    return np.c_[np.interp(t, s, p[:, 0]), np.interp(t, s, p[:, 1])]


def aligned_to(a, b):
    st = np.hypot(*(a[0] - b[0])) + np.hypot(*(a[-1] - b[-1]))
    cr = np.hypot(*(a[0] - b[-1])) + np.hypot(*(a[-1] - b[0]))
    return b[::-1].copy() if cr < st else b.copy()


# ---- chains --------------------------------------------------------------------

class Chain:
    def __init__(self, level, pts, closed, labels):
        self.level, self.pts, self.closed = level, pts, closed
        self.labels = labels

    def within(self, other, reach):
        q = other.pts
        for p in self.pts[:: max(1, len(self.pts) // 200)]:
            if nearest_on(q, p)[1] <= reach:
                return True
        return False


def chains_of(F, levels):
    out = []
    for k, rho in enumerate(levels):
        if k == 0:
            continue
        for line in F.gen.lines(rho):
            if len(line) < 2:
                continue
            closed = math.hypot(*(line[0] - line[-1])) <= math.sqrt(2) * CELL + 1e-9
            labs = np.array([F.label(p) for p in line])
            # split where the branch label changes (Split meeting rule)
            cuts = [0] + [i for i in range(1, len(line)) if labs[i] != labs[i - 1]] + [len(line)]
            if len(cuts) > 2:
                closed = False
            for a, b in zip(cuts[:-1], cuts[1:]):
                piece = line[a:b]
                if len(piece) >= 2:
                    out.append(Chain(k, piece, closed, frozenset(labs[a:b].tolist())))
    return out


def core_levels(F, levels):
    k = 0
    for lv in range(1, len(levels)):
        ls = [l for l in F.gen.lines(levels[lv]) if len(l) >= 2]
        if len(ls) == 1 and math.hypot(*(ls[0][0] - ls[0][-1])) <= math.sqrt(2) * CELL:
            k = lv
        else:
            break
    return k


# ---- the C8 port ------------------------------------------------------------

def lead_in_onto(F, prev, p, t):
    foot, delta = nearest_on(prev, p)
    info = dict(delta=delta)
    if delta <= 0:
        return None, info, "delta<=0"
    n = (foot - p) / delta
    pv = F.pivot(p)
    rho = math.hypot(*(p - pv))
    info["rho"] = rho
    radius, m, st = lead_in_radius_pivot(rho, delta)
    info["margin"] = m
    info["rho_min"] = st
    if radius is None:
        return None, info, "no margin"
    info["radius"] = radius
    if radius > rho - delta / 2:
        return None, info, "radius > rho - delta/2"
    centre = p + radius * n
    at = lambda th: centre - radius * (n * math.cos(th) + t * math.sin(th))
    dth = CELL / radius
    steps = int(math.floor((math.pi / 2) / dth))
    for i in range(1, steps + 1):
        if nearest_on(prev, at(i * dth))[1] <= CELL:
            pts = np.array([at(j * dth) for j in range(i, -1, -1)])
            info["theta"] = i * dth
            return (radius, pts), info, None
    return None, info, "start not reached by a right angle"


def point_at(sub, at):
    walked = 0.0
    for i in range(len(sub) - 1):
        u, v = sub[i], sub[i + 1]
        L = math.hypot(*(v - u))
        if L > 0 and walked + L >= at:
            f = min(max((at - walked) / L, 0), 1)
            return u + f * (v - u), i, (v - u) / L
        walked += L
    return None


def lead_band(F, a, b):
    sub_a = subdivide(a, CELL)
    sub_b = subdivide(b, CELL)
    total = plen(sub_b)
    rec = dict(total=total)
    r = point_at(sub_b, total / 2)
    if r is None:
        return None, rec, "point_at"
    mid, _, t_mid = r
    res, info, why = lead_in_onto(F, a, mid, t_mid)
    rec.update({k + "_mid": v for k, v in info.items()})
    if res is None:
        return None, rec, why
    span = math.hypot(*(res[1][0] - mid))
    rec["span"] = span
    at = max(total / 2, span)
    if at >= total:
        return None, rec, "span >= arc length"
    p, seg, t = point_at(sub_b, at)
    res, info, why = lead_in_onto(F, a, p, t)
    if res is None:
        return None, rec, "at P: " + why
    radius, lead = res
    start = lead[0]
    iq = int(np.argmin(np.hypot(*(sub_a - start).T)))
    link = np.vstack([sub_a[: iq + 1], start])
    fwd = np.vstack([p, sub_b[seg + 1:]])
    bwd = np.vstack([p, sub_b[: seg + 1][::-1]])
    for poly in (link, lead, fwd, bwd):
        for u, v in zip(poly[:-1], poly[1:]):
            if not F.seg_inside(u, v):
                return None, rec, "region"
    rec["radius"] = radius
    return radius, rec, None


def growing_bite_span(rho, delta, exact_h=False, n=4000, excess_at_delta=True):
    """Shortest run (mm) over which the bite grows 0 -> delta with every
    point at most c*D: tilt beta(b) from b + R(1-cos b) + h sin b + e <= cD,
    e = w(rho, delta) - delta the circular excess (the C8 test form; at b
    with excess_at_delta=False). None when m <= 0."""
    if CD - w_front(rho, delta) <= 0:
        return None
    hs = math.sqrt(H2(delta))
    tot = 0.0
    bs = np.linspace(0, delta, n + 1)
    for b0, b1 in zip(bs[:-1], bs[1:]):
        b = b1  # the larger bite of the step binds (the tilt falls with b)
        h = math.sqrt(H2(b)) if exact_h else hs
        bb = delta if excess_at_delta else b
        e = (w_front(rho, bb) - bb) if math.isfinite(rho) else 0.0
        rhoh = math.hypot(h, R)
        arg = (CD - e - b - R) / rhoh
        if arg <= -1:
            return None
        beta = math.atan2(R, h) + math.asin(min(arg, 1.0))
        if beta <= 0:
            return None
        tot += (b1 - b0) / math.tan(beta)
    return tot


def run(shape):
    F = Field(shape)
    levels = law_levels(F.dmax)
    K = core_levels(F, levels)
    ch = chains_of(F, levels)
    ch = [c for c in ch if c.level >= K + 1]
    parents = []
    for c in ch:
        parents.append([j for j, p in enumerate(ch) if p.level + 1 == c.level and c.within(p, REACH)])
    children = [[] for _ in ch]
    for j, ps in enumerate(parents):
        for i in ps:
            children[i].append(j)
    nxt = [None] * len(ch)
    has_prev = [False] * len(ch)
    stops = {}
    for i, a in enumerate(ch):
        kids = children[i]
        if not kids:
            continue
        why = None
        if len(kids) != 1:
            why = "split children"
        else:
            j = kids[0]
            b = ch[j]
            if a.closed or b.closed:
                why = "split closed"
            elif len(parents[j]) != 1:
                why = "split parents"
            elif a.labels != b.labels:
                why = "branch"
            else:
                la, lb = plen(a.pts), plen(b.pts)
                if max(la, lb) > RATIO * min(la, lb):
                    why = "ratio"
                else:
                    bp = aligned_to(a.pts, b.pts)
                    m = int(max(la, lb) / CELL) + 2
                    gap = np.max(np.hypot(*(resample(a.pts, m) - resample(bp, m)).T))
                    if gap > REACH:
                        why = "gap"
        if why is None:
            nxt[i] = kids[0]
            has_prev[kids[0]] = True
        else:
            stops[why] = stops.get(why, 0) + 1
    units = []
    for i in range(len(ch)):
        if has_prev[i]:
            continue
        u = [i]
        while nxt[u[-1]] is not None:
            u.append(nxt[u[-1]])
        units.append(u)
    lobes = [u for u in units if len(u) >= 2]
    print(f"== {shape.name}: seed {F.seed.round(2)} clearance(part) {F.clear_part:.2f}, "
          f"levels {len(levels)-1}, core K {K}, chains {len(ch)}, lobes {len(lobes)} / "
          f"{sum(len(u) for u in lobes)} arcs, plain {len(units)-len(lobes)}, stops {stops}")
    print("   level rho:", " ".join(f"{k}:{r:.2f}" for k, r in enumerate(levels)))
    # closed wraps after the core
    closed = [c for c in ch if c.closed]
    for c in closed:
        p = c.pts[0]
        rho = math.hypot(*(p - F.pivot(p)))
        print(f"   closed wrap level {c.level}: rho {rho:.2f}, margin {CD - w_front(rho, S if c.level>0 else S):+.4f}"
              f" (law delta {levels[c.level]-levels[c.level-1]:.3f}, margin at law delta "
              f"{CD - w_front(rho, levels[c.level]-levels[c.level-1]):+.4f})")
    rows = []
    for li, u in enumerate(lobes):
        for orient in (0, 1):
            first = ch[u[0]].pts
            p0 = first[::-1].copy() if orient == 0 else first.copy()
            arcs = [p0]
            for j in u[1:]:
                arcs.append(aligned_to(arcs[-1], ch[j].pts))
            far = False
            for k in range(len(arcs) - 1):
                a, b = arcs[k], arcs[k + 1]
                if far:
                    a, b = a[::-1], b[::-1]
                radius, rec, why = lead_band(F, a, b)
                # growing-bite need at mid
                mid = point_at(subdivide(b, CELL), plen(subdivide(b, CELL)) / 2)[0]
                rho = math.hypot(*(mid - F.pivot(mid)))
                foot, delta = nearest_on(a, mid)
                g = growing_bite_span(rho, delta)
                gx = growing_bite_span(rho, delta, exact_h=True)
                rows.append(dict(lobe=li, orient=orient, band=k, level=ch[u[k + 1]].level,
                                 why=why or "LEAD-IN", rho=rho, delta=delta,
                                 total=rec["total"], span=rec.get("span"),
                                 radius=rec.get("radius_mid"), margin=rec.get("margin_mid"),
                                 grow=g, grow_exact=gx))
                if radius is None:
                    far = not far
    return F, rows


def fmt(v, f="{:.2f}"):
    return "-" if v is None else f.format(v)


if __name__ == "__main__":
    print(f"cD {CD:.4f}  cell {CELL}  reach {REACH:.3f}  ratio {RATIO:.3f}  rho* {rho_star():.2f}")
    print(f"growing-bite span, straight edge, h(s) bound: {growing_bite_span(math.inf, S):.2f} mm (EntryLaw doc: 7.7)")
    for rho in (23.5, 25, 30, 35, 40, 45, 60):
        rl, m, st = lead_in_radius_pivot(rho, S)
        print(f"  rho {rho:5.1f}: margin {m:.4f}  rho_L {fmt(rl)}  growing-bite span {fmt(growing_bite_span(rho, S))}"
              f" (h(b): {fmt(growing_bite_span(rho, S, True))})")
    for shape in (Star(), Pocket()):
        F, rows = run(shape)
        print(" lobe o band lvl  rho    delta  margin   rho_L   arc    span   grow  grow_h(b)  result")
        for r in rows:
            print(f" {r['lobe']:4d} {r['orient']} {r['band']:4d} {r['level']:3d} {r['rho']:6.2f} {r['delta']:6.3f}"
                  f" {fmt(r['margin'], '{:+.4f}'):>8} {fmt(r['radius']):>7} {r['total']:6.2f} {fmt(r['span']):>6}"
                  f" {fmt(r['grow']):>6} {fmt(r['grow_exact']):>6}  {r['why']}")
        for o in (0, 1):
            rs = [r for r in rows if r["orient"] == o]
            from collections import Counter
            print(f"   orient {o}: {len(rs)} bands, reasons {dict(Counter(r['why'] for r in rs))}")
            gfit = sum(1 for r in rs if r['grow'] is not None and r['grow'] < r['total'])
            gfit2 = sum(1 for r in rs if r['grow_exact'] is not None and r['grow_exact'] < r['total'])
            beyond = sum(1 for r in rs if r['margin'] is not None and r['margin'] > 0)
            print(f"   orient {o}: margin>0 {beyond}; growing-bite fits (span < arc) {gfit} (h(b): {gfit2})")
