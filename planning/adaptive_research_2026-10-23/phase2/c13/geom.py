"""C13: exact and lattice width checks of the growing-bite lead-in with a
reserved band margin, on the star arms (seed-visible, pivot = seed).

Every value comes from R, s, the planner cell and the ceiling formula
(search.rs `PassLoad::swept_width`, ops/adaptive_shared.rs). No tuned
number. Run from the c13 folder with the c10 folder as argv[1].
"""
import math
import sys

import numpy as np
from matplotlib.path import Path
from scipy.spatial import cKDTree

R = 3.0
S = 2.0
D = 2 * R
CELL = max(R / 6.0, 0.1)  # planner_cell_size (path.rs:43)
STEP_LEN = 3 * CELL  # pass_step_len (path.rs:49)
SUB = 4  # material_grid.rs:32


def ceiling_fraction(s=S, r=R):
    """`PassLoad::swept_width` ceiling: radial_woc(1.05 x alpha/2pi)."""
    alpha = math.acos(max(-1.0, min(1.0, 1 - min(s, 2 * r) / r))) / (2 * math.pi)
    f = min(max(alpha * 1.05, 0.0), 0.5)
    return (1 - math.cos(2 * math.pi * f)) / 2


C = ceiling_fraction()  # 0.362644...
CD = C * D  # 2.175868 mm (C10 used 0.3626 x 6 = 2.1756)
CD_C10 = 0.3626 * D
SLIVER_C = C + CELL / D

STAR = np.array([(50, 5), (61, 35), (95, 35), (68, 57), (79, 91), (50, 70),
                 (21, 91), (32, 57), (5, 35), (39, 35)], float)
STAR_PATH = Path(STAR)
SEED = np.array([50.0, 50.6])  # the C10 model's EDT seed (model.edt_seed)
TIPS = STAR[0::2]


def boundary_dist(x, y):
    x = np.asarray(x, float)
    y = np.asarray(y, float)
    d = np.full(np.shape(x), np.inf)
    n = len(STAR)
    for i in range(n):
        a, b = STAR[i], STAR[(i + 1) % n]
        dx, dy = b - a
        L2 = dx * dx + dy * dy
        t = np.clip(((x - a[0]) * dx + (y - a[1]) * dy) / L2, 0, 1)
        d = np.minimum(d, np.hypot(x - (a[0] + t * dx), y - (a[1] + t * dy)))
    return d


def inside_part(x, y):
    x = np.asarray(x, float)
    y = np.asarray(y, float)
    sh = np.shape(x)
    return STAR_PATH.contains_points(np.c_[np.ravel(x), np.ravel(y)]).reshape(sh)


def in_region(x, y):
    """Tool-centre region: inside the part and at least R from the wall."""
    return inside_part(x, y) & (boundary_dist(x, y) >= R)


def polar(r, th):
    return SEED + r * np.array([math.cos(th), math.sin(th)])


def arm_axis(k):
    t = TIPS[k]
    return math.atan2(t[1] - SEED[1], t[0] - SEED[0])


def arc_range(r, th0, cap=math.radians(150)):
    """Angle range (t0 < t1) of the region piece of the circle r about the
    seed that holds the arm axis th0: walk out, then bisect to 1e-12."""
    if not in_region(*polar(r, th0)):
        return None
    ends = []
    for sgn in (-1, 1):
        step = 0.002
        lo = 0.0
        hi = None
        t = step
        while t <= cap:
            if not in_region(*polar(r, th0 + sgn * t)):
                hi = t
                break
            lo = t
            t += step
        if hi is None:
            ends.append(th0 + sgn * cap)
            continue
        for _ in range(60):
            mid = (lo + hi) / 2
            if in_region(*polar(r, th0 + sgn * mid)):
                lo = mid
            else:
                hi = mid
        ends.append(th0 + sgn * lo)
    return ends[0], ends[1]


# ---- the C10 width formula and the growing-bite law ------------------------

def w_front(rho, delta):
    """C5 circular-front swept width of a pass at rho, delta outside the last."""
    a = rho - delta + R
    return R - (a * a - R * R - rho * rho) / (2 * rho)


def h_of(d):
    return math.sqrt(max(R * R - (R - d) ** 2, 0.0))


def tilt(b, delta, rho, cd):
    """EntryLaw tilt at bite b with h = h(delta) and the circular excess
    e = w(rho, delta) - delta (C10 growing_bite_span)."""
    h = h_of(delta)
    e = w_front(rho, delta) - delta
    arg = (cd - e - b - R) / math.hypot(h, R)
    if arg <= -1:
        return None
    return math.atan2(R, h) + math.asin(min(arg, 1.0))


def growing_span(rho, delta, cd=CD_C10, n=4000):
    """C10's span: integral of db / tan(beta(b)), beta at the larger bite."""
    if cd - w_front(rho, delta) <= 0:
        return None
    tot = 0.0
    bs = np.linspace(0, delta, n + 1)
    for b0, b1 in zip(bs[:-1], bs[1:]):
        be = tilt(b1, delta, rho, cd)
        if be is None or be <= 0:
            return None
        tot += (b1 - b0) / math.tan(be)
    return tot


def reserve_delta(rho, L, cd=CD_C10):
    """C10 reserve.need: the largest delta <= s whose span is <= L."""
    lo, hi = 0.5, S
    g = growing_span(rho, hi, cd)
    if g is not None and g <= L:
        return hi
    for _ in range(60):
        mid = (lo + hi) / 2
        g = growing_span(rho, mid, cd, n=1500)
        if g is not None and g <= L:
            lo = mid
        else:
            hi = mid
    return lo


def ramp_polar(rho, delta, th_start, sgn, cd=CD_C10, n=4000):
    """The ramp in the pivot frame: r = rho - delta + b, the tilt against
    the circle about the seed is beta(b), so d(theta) = db / (r tan beta).
    Returns arrays (theta, r) from bite 0 to bite delta."""
    bs = np.linspace(0, delta, n + 1)
    th = [th_start]
    for b0, b1 in zip(bs[:-1], bs[1:]):
        be = tilt(b1, delta, rho, cd)
        r = rho - delta + 0.5 * (b0 + b1)
        th.append(th[-1] + sgn * (b1 - b0) / (r * math.tan(be)))
    return np.array(th), rho - delta + bs


def resample_polyline(P, pitch):
    seg = np.hypot(*np.diff(P, axis=0).T)
    s = np.concatenate([[0], np.cumsum(seg)])
    n = max(2, int(math.ceil(s[-1] / pitch)) + 1)
    t = np.linspace(0, s[-1], n)
    return np.c_[np.interp(t, s, P[:, 0]), np.interp(t, s, P[:, 1])]


def circle_pts(r, t0, t1, pitch):
    n = max(2, int(math.ceil(abs(t1 - t0) * r / pitch)) + 1)
    th = np.linspace(t0, t1, n)
    return SEED + r * np.c_[np.cos(th), np.sin(th)]


# ---- the exact stock and the exact swept width ------------------------------

EPS = 1e-5  # a point within R + EPS of a cut centre is cut (KD pitch 0.01:
#             a point sample over-reads the distance by <= 0.01^2/(8R) = 4e-6)


class Stock:
    """Stock = part minus the discs swept along prior circular arcs about the
    seed (exact distance) and along extra polylines (dense KD samples)."""

    def __init__(self, arcs, th0, walls=True):
        """walls=False: the law check. The prior arcs run on as full circles
        and the part has no wall, so no wall corner is left (the stock is
        only what lies outside the last circle swept)."""
        self.arcs = arcs if walls else [(r, -math.inf, math.inf) for r, _, _ in arcs]
        self.th0 = th0
        self.walls = walls
        self.kd = None

    def add_path(self, P):
        dense = resample_polyline(P, 0.01)
        pts = dense if self.kd is None else np.vstack([self.kd.data, dense])
        self.kd = cKDTree(pts)

    def is_stock(self, Q):
        Q = np.atleast_2d(Q)
        ok = inside_part(Q[:, 0], Q[:, 1]) if self.walls else np.ones(len(Q), bool)
        v = Q - SEED
        rr = np.hypot(v[:, 0], v[:, 1])
        psi = self.th0 + (np.arctan2(v[:, 1], v[:, 0]) - self.th0 + math.pi) % (2 * math.pi) - math.pi
        for r, t0, t1 in self.arcs:
            if not math.isfinite(t0):
                ok &= np.abs(rr - r) > R + EPS
                continue
            inr = (psi >= t0) & (psi <= t1)
            e0 = polar(r, t0)
            e1 = polar(r, t1)
            d_end = np.minimum(np.hypot(*(Q - e0).T), np.hypot(*(Q - e1).T))
            d = np.where(inr, np.abs(rr - r), d_end)
            ok &= d > R + EPS
        if self.kd is not None:
            d, _ = self.kd.query(Q)
            ok &= d > R + EPS
        return ok


NPHI = 3000  # leading-arc samples: pitch pi R / 3000 = 0.0031 mm on the rim


def exact_width(stock, p, v):
    """Perpendicular extent (mm) of the stock on the leading half of the
    cutter rim at p heading v: the width the disc cuts as it moves (the
    limit of the simulator's fresh-cell extent as the step goes to 0). The
    two extreme stock angles are bisected to 1e-10 rad."""
    v = v / np.hypot(*v)
    n = np.array([-v[1], v[0]])
    phi = np.linspace(-math.pi / 2, math.pi / 2, NPHI + 1)
    Q = p + R * (np.cos(phi)[:, None] * v + np.sin(phi)[:, None] * n)
    m = stock.is_stock(Q)
    if not m.any():
        return 0.0, None
    idx = np.flatnonzero(m)

    def q(ph):
        return p + R * (math.cos(ph) * v + math.sin(ph) * n)

    def refine(i_in, i_out):
        a, b = phi[i_in], phi[i_out]
        for _ in range(40):
            mid = (a + b) / 2
            if stock.is_stock(q(mid))[0]:
                a = mid
            else:
                b = mid
        return a

    lo_i, hi_i = idx[0], idx[-1]
    plo = refine(lo_i, lo_i - 1) if lo_i > 0 else phi[0]
    phi_hi = refine(hi_i, hi_i + 1) if hi_i < NPHI else phi[-1]
    return R * (math.sin(phi_hi) - math.sin(plo)), (plo, phi_hi)


def path_widths(stock, P, H=None):
    """Exact width at every sample of the polyline P, heading H[i] when
    given (the ramp: the direction of the law step that leaves the node),
    else by central difference (exact on a circle sampled evenly)."""
    out = np.zeros(len(P))
    for i in range(len(P)):
        if H is not None:
            h = H[i]
        else:
            h = P[min(i + 1, len(P) - 1)] - P[max(i - 1, 0)]
        out[i] = exact_width(stock, P[i], h)[0]
    return out


def ramp_samples(native, pitch):
    """Nodes of the integrated ramp about `pitch` apart, each with the
    heading of the law step that leaves it (the last: the step into it)."""
    seg = np.hypot(*np.diff(native, axis=0).T)
    s = np.concatenate([[0], np.cumsum(seg)])
    if pitch <= 0:
        idx = np.arange(len(native))
    else:
        idx = np.unique(np.searchsorted(s, np.arange(0, s[-1], pitch)))
        idx = np.unique(np.append(idx, len(native) - 1))
    H = np.diff(native, axis=0)
    H = np.vstack([H, H[-1:]])
    return idx, native[idx], H[idx]


# ---- the planner lattice (material_grid.rs, search.rs), ported -------------

AIR, MAT, CLR, FRG = 0, 1, 2, 3
_off = (np.arange(SUB) + 0.5) / SUB - 0.5  # sub_offset


class Grid:
    def __init__(self, cell=CELL, phase=(0.0, 0.0)):
        x_min, y_min = STAR.min(axis=0)
        x_max, y_max = STAR.max(axis=0)
        self.cs = cell
        self.cols = int(math.ceil((x_max - x_min + 2 * cell) / cell)) + 1
        self.rows = int(math.ceil((y_max - y_min + 2 * cell) / cell)) + 1
        self.ox = x_min - cell + phase[0]
        self.oy = y_min - cell + phase[1]
        xs = self.ox + np.arange(self.cols) * cell
        ys = self.oy + np.arange(self.rows) * cell
        X, Y = np.meshgrid(xs, ys)
        self.X, self.Y = X, Y
        self.state = np.where(inside_part(X, Y), MAT, AIR).astype(np.int8)
        # keep_fringe: sub-point (i, j) inside the polygon holds stock
        bits = np.zeros(X.shape, np.int32)
        for j in range(SUB):
            for i in range(SUB):
                inn = inside_part(X + _off[i] * cell, Y + _off[j] * cell)
                bits |= np.where(inn, 1 << (j * SUB + i), 0)
        self.bits = np.where(self.state != AIR, bits, 0)

    def _window(self, xmin, xmax, ymin, ymax):
        c0 = max(int(math.floor((xmin - self.ox) / self.cs)), 0)
        c1 = min(int(math.ceil((xmax - self.ox) / self.cs)), self.cols - 1)
        r0 = max(int(math.floor((ymin - self.oy) / self.cs)), 0)
        r1 = min(int(math.ceil((ymax - self.oy) / self.cs)), self.rows - 1)
        return slice(r0, r1 + 1), slice(c0, c1 + 1)

    def clear_capsule(self, a, b, r=R):
        reach = r + 0.5 * math.sqrt(2) * self.cs
        sl = self._window(min(a[0], b[0]) - reach, max(a[0], b[0]) + reach,
                          min(a[1], b[1]) - reach, max(a[1], b[1]) + reach)
        X, Y = self.X[sl], self.Y[sl]
        st = self.state[sl]
        bits = self.bits[sl]
        ux, uy = b[0] - a[0], b[1] - a[1]
        L2 = ux * ux + uy * uy

        def dsq(x, y):
            t = np.clip(((x - a[0]) * ux + (y - a[1]) * uy) / L2, 0, 1) if L2 > 1e-20 else 0.0
            return (x - a[0] - t * ux) ** 2 + (y - a[1] - t * uy) ** 2

        d = dsq(X, Y)
        live = ((st == MAT) | (st == FRG)) & (d <= reach * reach)
        centre_cut = d <= r * r
        nb = bits.copy()
        for j in range(SUB):
            for i in range(SUB):
                hit = dsq(X + _off[i] * self.cs, Y + _off[j] * self.cs) <= r * r
                nb = np.where(hit, nb & ~(1 << (j * SUB + i)), nb)
        now = np.where((st == FRG) | centre_cut, np.where(nb == 0, CLR, FRG), MAT)
        self.state[sl] = np.where(live, now, st)
        self.bits[sl] = np.where(live, nb, bits)

    def swept_width_on(self, cx, cy, angle, slivers, swept=None, r=R):
        sl = self._window(cx - r, cx + r, cy - r, cy + r)
        dx = self.X[sl] - cx
        dy = self.Y[sl] - cy
        st = self.state[sl]
        bits = self.bits[sl]
        sx, sy = -math.sin(angle), math.cos(angle)

        def not_swept(px, py):
            if swept is None:
                return np.ones(np.shape(px), bool)
            a, b = swept
            ux, uy = b[0] - a[0], b[1] - a[1]
            L2 = ux * ux + uy * uy
            x, y = cx + px, cy + py
            t = np.clip(((x - a[0]) * ux + (y - a[1]) * uy) / L2, 0, 1) if L2 > 1e-20 else 0.0
            return (x - a[0] - t * ux) ** 2 + (y - a[1] - t * uy) ** 2 > r * r

        us = []
        m = (st == MAT) & (dx * dx + dy * dy <= r * r) & not_swept(dx, dy)
        us.append((dx * sx + dy * sy)[m])
        if slivers:
            f = st == FRG
            for j in range(SUB):
                for i in range(SUB):
                    px = dx + _off[i] * self.cs
                    py = dy + _off[j] * self.cs
                    mm = f & ((bits >> (j * SUB + i)) & 1).astype(bool) & (px * px + py * py <= r * r) & not_swept(px, py)
                    us.append((px * sx + py * sy)[mm])
        u = np.concatenate(us)
        if u.size == 0:
            return 0.0
        return max((u.max() - u.min()) / (2 * r), 1e-6)

    def measure_step(self, a, b, r=R):
        """search.rs measure_step under PassLoad::swept_width (fraction of D)."""
        angle = math.atan2(b[1] - a[1], b[0] - a[0])
        reading = self.swept_width_on(b[0], b[1], angle, False)
        if reading > C:
            return reading
        sl = self.swept_width_on(b[0], b[1], angle, True)
        if sl > SLIVER_C:
            return sl
        n = max(int(math.ceil(math.hypot(b[0] - a[0], b[1] - a[1]) / self.cs)), 1)
        worst = reading
        prev = a
        for k in range(1, n):
            t = k / n
            at = (a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1]))
            rr = self.swept_width_on(at[0], at[1], angle, False, (a, prev))
            if rr <= C:
                s2 = self.swept_width_on(at[0], at[1], angle, True, (a, prev))
                if s2 > SLIVER_C:
                    rr = s2
            if rr > C:
                return rr
            worst = max(worst, rr)
            prev = at
        return worst

    def stamp_path(self, P):
        self.clear_capsule(P[0], P[0])  # cut_start: the disc at the first point
        for a, b in zip(P[:-1], P[1:]):
            self.clear_capsule(a, b)

    def read_path(self, P):
        """The planned readings (fraction of D): each step read, then stamped
        (the plan grid of the cascade walk, research_geodesic.rs:5652)."""
        self.clear_capsule(P[0], P[0])
        out = []
        for a, b in zip(P[:-1], P[1:]):
            out.append(self.measure_step(a, b))
            self.clear_capsule(a, b)
        return np.array(out)


def subdivide_at(P, step):
    """research_geodesic `subdivide`: each segment cut into ceil(len/step)."""
    out = [P[0]]
    for a, b in zip(P[:-1], P[1:]):
        n = max(1, int(math.ceil(math.hypot(*(b - a)) / step)))
        for k in range(1, n + 1):
            out.append(a + (b - a) * k / n)
    return np.array(out)


# ---- the exact tilt law (C13) -----------------------------------------------

def width_no_wall(b, beta, rho, delta):
    """Exact width (mm) of a cutter at bite b (centre at rho - delta + b about
    the seed) heading beta outward from the circle's tangent, when the stock
    is everything outside the circle A = rho - delta + R (the last pass).
    Rim point at angle phi from the heading: |q|^2 = r^2 + R^2 + 2rR sin(phi
    + beta), so it is stock where sin(phi + beta) > kappa."""
    r = rho - delta + b
    A = rho - delta + R
    kappa = (A * A - r * r - R * R) / (2 * r * R)
    if kappa >= 1:
        return 0.0
    al = math.asin(max(kappa, -1.0))
    lo = max(al - beta, -math.pi / 2)
    hi = min(math.pi / 2, math.pi - al - beta)
    return max(R * (math.sin(hi) - math.sin(lo)), 0.0)


def tilt_exact(b, rho, delta, cd=CD):
    """The largest tilt (rad, below a right angle) whose exact width is cd."""
    top = math.pi / 2 - 1e-9
    if width_no_wall(b, top, rho, delta) <= cd:
        return top
    if width_no_wall(b, 0.0, rho, delta) > cd:
        return None
    lo, hi = 0.0, top
    for _ in range(60):
        mid = (lo + hi) / 2
        if width_no_wall(b, mid, rho, delta) <= cd:
            lo = mid
        else:
            hi = mid
    return lo


def ramp_polar_exact(rho, delta, th_start, sgn, cd=CD, n=4000):
    """The ramp under the exact tilt law (tilt read at the larger bite of
    each step, as C10 does): returns (theta, r) from bite 0 to delta, or
    None when the full bite has no tilt (m <= 0)."""
    bs = np.linspace(0, delta, n + 1)
    th = [th_start]
    for b0, b1 in zip(bs[:-1], bs[1:]):
        be = tilt_exact(b1, rho, delta, cd)
        if be is None or be <= 0:
            return None
        r = rho - delta + 0.5 * (b0 + b1)
        th.append(th[-1] + sgn * (b1 - b0) / (r * math.tan(be)))
    return np.array(th), rho - delta + bs


def span_exact(rho, delta, cd=CD, n=4000):
    """The exact-law span as C10 measures one: the run along the front,
    the integral of db / tan(beta)."""
    out = 0.0
    bs = np.linspace(0, delta, n + 1)
    for b0, b1 in zip(bs[:-1], bs[1:]):
        be = tilt_exact(b1, rho, delta, cd)
        if be is None or be <= 0:
            return None
        out += (b1 - b0) / math.tan(be)
    return out


def tilt_min(b, rho, delta, cd=CD):
    """C13 law: the smaller of the C10 tilt (EntryLaw form plus the circular
    excess) and the exact tilt (`tilt_exact`), both against cd. The C10
    tilt is the gentler one at small bite (it keeps the ramp off the wall
    it starts at); the exact one binds near full bite, where the C10 bound
    reads up to 0.0064 mm under the true width (C13 law check)."""
    a = tilt(b, delta, rho, cd)
    e = tilt_exact(b, rho, delta, cd)
    if a is None or e is None:
        return None
    return min(a, e)


def ramp_polar_min(rho, delta, th_start, sgn, cd=CD, n=4000):
    """The ramp under `tilt_min` (tilt read at the larger bite of each step):
    (theta, r) from bite 0 to delta, or None when no tilt holds."""
    bs = np.linspace(0, delta, n + 1)
    th = [th_start]
    for b0, b1 in zip(bs[:-1], bs[1:]):
        be = tilt_min(b1, rho, delta, cd)
        if be is None or be <= 0:
            return None
        r = rho - delta + 0.5 * (b0 + b1)
        th.append(th[-1] + sgn * (b1 - b0) / (r * math.tan(be)))
    return np.array(th), rho - delta + bs


def span_min(rho, delta, cd=CD, n=2000):
    """The `tilt_min` span as C10 measures one: integral of db / tan(beta)."""
    out = 0.0
    bs = np.linspace(0, delta, n + 1)
    for b0, b1 in zip(bs[:-1], bs[1:]):
        be = tilt_min(b1, rho, delta, cd)
        if be is None or be <= 0:
            return None
        out += (b1 - b0) / math.tan(be)
    return out
