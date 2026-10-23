# C15 level march, metrics and raster replay.
import math
import numpy as np
import contourpy
from scipy.spatial import cKDTree
from scipy import ndimage

from geom import (R, S, D, CD, C_FRAC, TOL, PASS_STEP, RECT, RX, ISL_C, ISL_R, SEED,
                  law_spacing, front_width, in_stock, hidden)
from fields import XS, YS, H, GX, GY, ZONE


# ── contours ───────────────────────────────────────────────────────────

class Field:
    def __init__(self, F):
        self.F = F
        self.gen = contourpy.contour_generator(x=XS, y=YS, z=np.ma.masked_invalid(F),
                                               name="serial", line_type="Separate",
                                               corner_mask=True)
        Fz = np.where(np.isfinite(F), F, 0.0)
        gy, gx = np.gradient(Fz, H)
        self.gx, self.gy = gx, gy
        self.fmax = float(np.nanmax(F))

    def lines(self, t):
        out = []
        for ln in self.gen.lines(t):
            if len(ln) >= 2:
                out.append(resample(ln, TOL))
        return [l for l in out if len(l) >= 2]

    def grad(self, P):
        j = np.clip(np.round((P[:, 0] - XS[0]) / H).astype(int), 0, len(XS) - 1)
        i = np.clip(np.round((P[:, 1] - YS[0]) / H).astype(int), 0, len(YS) - 1)
        return np.stack([self.gx[i, j], self.gy[i, j]], axis=1)


def resample(P, step):
    seg = np.hypot(*np.diff(P, axis=0).T)
    s = np.concatenate([[0], np.cumsum(seg)])
    if s[-1] <= 0:
        return P[:1]
    n = max(2, int(math.ceil(s[-1] / step)) + 1)
    u = np.linspace(0, s[-1], n)
    return np.stack([np.interp(u, s, P[:, 0]), np.interp(u, s, P[:, 1])], axis=1)


def arclen(P):
    return np.concatenate([[0], np.cumsum(np.hypot(*np.diff(P, axis=0).T))])


def signed_radius(P, field, half=PASS_STEP):
    """Three-point circle over +-half of arc length (the planner's pass step):
    the front's curvature radius at each point, signed + when its centre lies
    on the cleared (lower-field) side. inf for a straight piece."""
    s = arclen(P)
    L = s[-1]
    n = len(P)
    rho = np.full(n, np.inf)
    if L < 2 * TOL:
        return rho
    h = min(half, L / 2)
    sm = np.clip(s - h, 0, L)
    sp = np.clip(s + h, 0, L)
    # at an end, slide the triple inward so it keeps its span
    sm = np.where(sp - s < h, np.maximum(L - 2 * h, 0), sm)
    sp = np.where(s - sm < h, np.minimum(2 * h, L), sp)
    mid = np.where(s - sm < h, h, np.where(sp - s < h, L - h, s))
    a = np.stack([np.interp(sm, s, P[:, 0]), np.interp(sm, s, P[:, 1])], 1)
    b = np.stack([np.interp(mid, s, P[:, 0]), np.interp(mid, s, P[:, 1])], 1)
    c = np.stack([np.interp(sp, s, P[:, 0]), np.interp(sp, s, P[:, 1])], 1)
    ab = b - a
    ac = c - a
    cr = ab[:, 0] * ac[:, 1] - ab[:, 1] * ac[:, 0]
    la = np.hypot(*(b - c).T)
    lb = np.hypot(*ac.T)
    lc = np.hypot(*ab.T)
    with np.errstate(divide="ignore", invalid="ignore"):
        rad = la * lb * lc / (2 * np.abs(cr))
    # centre direction: the normal of the chord a-c toward b's bend
    nrm = np.stack([-(c - a)[:, 1], (c - a)[:, 0]], 1)
    nrm *= np.sign(cr)[:, None]  # points to the centre side
    g = field.grad(b)
    sign = np.where((nrm * g).sum(1) < 0, 1.0, -1.0)
    ok = np.isfinite(rad) & (np.abs(cr) > 1e-12)
    rho[ok] = (sign * rad)[ok]
    return rho


# ── the march ──────────────────────────────────────────────────────────

def law_spacing_v(rho):
    """Vectorised geom.law_spacing."""
    rho = np.asarray(rho, float)
    out = np.full(rho.shape, S)
    pos = np.isfinite(rho) & (rho > 0)
    r = rho[pos]
    sb = r + R - np.sqrt(R * R + r * r + 2 * r * (R - CD))
    out[pos] = np.clip(sb, 0.0, S)
    return out


def front_width_v(g, rho):
    """Vectorised geom.front_width (signed rho; inf: straight)."""
    g = np.asarray(g, float)
    rho = np.asarray(rho, float)
    with np.errstate(divide="ignore", invalid="ignore"):
        w = g + g * (R - g / 2.0) / rho
    return np.where(np.isfinite(rho), w, g)


def check(lines, tree, field):
    """Per point: gap g to the cut set, signed rho, allowed law gap, formula w."""
    out = []
    for P in lines:
        g, _ = tree.query(P)
        rho = signed_radius(P, field)
        out.append((P, g, rho, law_spacing_v(rho), front_width_v(g, rho)))
    return out


def ok_level(lines, tree, field):
    if not lines:
        return False
    for P, g, rho, allow, w in check(lines, tree, field):
        if np.any(g > allow + 1e-9):
            return False
    return True


def march(field, cut_pts, t0=0.0, iters=30, max_levels=200):
    """Choose levels from t0 so that every point of each new curve lies at most
    the C5 law gap (from its own signed curvature radius) from the cut set.
    cut_pts: the centre paths already cut (or the exact cut edge)."""
    levels = []
    curves = []
    pts = [cut_pts]
    t = t0
    for _ in range(max_levels):
        tree = cKDTree(np.concatenate(pts))
        # remaining region within the law cap of the cut set: done
        rem = np.isfinite(field.F) & (field.F > t + 1e-9)
        if not rem.any():
            break
        rp = np.stack([GX[rem], GY[rem]], 1)
        dmax = tree.query(rp)[0].max()
        if dmax <= S:  # what is left lies within one pass of a cut
            break
        lo, hi = t, None
        step = (field.fmax - t) / 1024.0
        # grow by doubling until the level fails or leaves the field
        while True:
            cand = lo + step
            if cand >= field.fmax:
                hi = field.fmax
                break
            if not ok_level(field.lines(cand), tree, field):
                hi = cand
                break
            lo = cand
            step *= 2.0
        for _ in range(iters):
            mid = 0.5 * (lo + hi)
            if ok_level(field.lines(mid), tree, field):
                lo = mid
            else:
                hi = mid
        if lo <= t + 1e-9:
            # no level fits: stop and report (should not happen)
            break
        t = lo
        ls = field.lines(t)
        levels.append(t)
        curves.append(ls)
        pts.extend(ls)
    return levels, curves


def fixed_levels(field, levels):
    return [field.lines(t) for t in levels]


# ── raster replay (material space, cell = tolerance) ───────────────────

MX = np.arange(RECT[0], RECT[1] + H / 2, H)
MY = np.arange(RECT[2], RECT[3] + H / 2, H)
MGX, MGY = np.meshgrid(MX, MY)
STOCK0 = in_stock(MGX, MGY)
_rc = int(math.ceil(R / H)) + 1
_off = np.arange(-_rc, _rc + 1)
_OX, _OY = np.meshgrid(_off * H, _off * H)


class Replay:
    def __init__(self):
        self.stock = STOCK0.copy()
        self.clear_disc(SEED, 2 * R)  # the starter pocket: centre radius R

    def clear_disc(self, c, r):
        m = np.hypot(MGX - c[0], MGY - c[1]) <= r
        self.stock &= ~m

    def _window(self, x, y):
        j0 = int(round((x - MX[0]) / H))
        i0 = int(round((y - MY[0]) / H))
        ii = i0 + _off
        jj = j0 + _off
        vi = (ii >= 0) & (ii < len(MY))
        vj = (jj >= 0) & (jj < len(MX))
        ii, jj = ii[vi], jj[vj]
        dx = MX[jj][None, :] - x
        dy = MY[ii][:, None] - y
        inside = dx * dx + dy * dy <= R * R + 1e-12
        return ii, jj, dx, dy, inside

    def cut(self, P):
        """Walk a centre path. Return, per sample: the swept width read before
        the disc is cut (perpendicular extent of standing cells in the disc,
        the planner's reading convention), and the area the sample cut (mm^2)."""
        n = len(P)
        if n < 2:
            return np.zeros(n), np.zeros(n)
        tang = np.gradient(P, axis=0)
        tl = np.hypot(*tang.T)
        tang = tang / np.where(tl > 0, tl, 1)[:, None]
        w = np.zeros(n)
        a = np.zeros(n)
        for k in range(n):
            x, y = P[k]
            ii, jj, dx, dy, inside = self._window(x, y)
            sub = self.stock[np.ix_(ii, jj)]
            mat = sub & inside
            if mat.any():
                nx, ny = -tang[k, 1], tang[k, 0]
                nn = (dx * nx + dy * ny)[mat]
                w[k] = nn.max() - nn.min()
                a[k] = mat.sum() * H * H
            sub[inside] = False
            self.stock[np.ix_(ii, jj)] = sub
        return w, a


def pocket_boundary_distance(x, y):
    """Distance from a material point inside the pocket to its boundary (the
    rounded outer wall, signed-distance form, or the island)."""
    cx, cy = (RECT[0] + RECT[1]) / 2, (RECT[2] + RECT[3]) / 2
    hx, hy = (RECT[1] - RECT[0]) / 2 - RX, (RECT[3] - RECT[2]) / 2 - RX
    qx = np.abs(x - cx) - hx
    qy = np.abs(y - cy) - hy
    sd = np.hypot(np.maximum(qx, 0), np.maximum(qy, 0)) + np.minimum(np.maximum(qx, qy), 0) - RX
    isl = np.hypot(x - ISL_C[0], y - ISL_C[1]) - ISL_R
    return np.minimum(-sd, isl)


def zone_footprint():
    """Material cells within R of a zone point (tool-centre zone dilated)."""
    k = int(round(R / H))
    yy, xx = np.mgrid[-k:k + 1, -k:k + 1]
    disc = xx * xx + yy * yy <= k * k
    dil = ndimage.binary_dilation(ZONE, structure=disc)
    fp = np.zeros_like(STOCK0)
    oi = int(round((YS[0] - MY[0]) / H))
    oj = int(round((XS[0] - MX[0]) / H))
    fp[oi:oi + dil.shape[0], oj:oj + dil.shape[1]] = dil
    return fp & STOCK0


SCALLOP = R - math.sqrt(R * R - (S / 2) ** 2)  # wall cusp between passes s apart


def coverage(stock, footprint):
    res = stock & footprint
    if not res.any():
        return 0.0, 0.0
    bd = pocket_boundary_distance(MGX[res], MGY[res])
    interior = bd > SCALLOP + H
    return float(interior.sum() * H * H), float(bd.max())
