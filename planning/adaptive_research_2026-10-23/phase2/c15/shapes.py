# C15 (e)/(f): the two development shapes in tool-centre space, on a grid at
# the tolerance. Sources:
#   fixtures/demo_pocket.svg (geom.py) and fixtures/demo_star.svg:
#     polygon 50,5 61,35 95,35 68,57 79,91 50,70 21,91 32,57 5,35 39,35
#     (world = SVG coordinates, as the harness View maps them).
#   seeds: the harness summaries (renders4/*_geodesic_curv.txt, "field piece 0"):
#     pocket (18.5, 18.5), star (49.5, 50.5); seed radius R.
#   entry disc: GeodesicField::on_lattice(.., seed, r = tool_radius, ..) in
#     research_geodesic.rs geodesic_segments; the spiral starter ends on that
#     lap (path.rs emit_spiral_starter_pocket).
import math
import numpy as np

import geom
from geom import R, TOL

H = TOL
STAR = np.array([[50, 5], [61, 35], [95, 35], [68, 57], [79, 91], [50, 70], [21, 91], [32, 57],
                 [5, 35], [39, 35]], float)


def poly_inside(P, X, Y):
    """Even-odd point-in-polygon, vectorised."""
    inside = np.zeros(np.broadcast(X, Y).shape, bool)
    n = len(P)
    for i in range(n):
        x1, y1 = P[i]
        x2, y2 = P[(i + 1) % n]
        cond = (y1 > Y) != (y2 > Y)
        with np.errstate(divide="ignore", invalid="ignore"):
            xc = x1 + (Y - y1) * (x2 - x1) / (y2 - y1)
        inside ^= cond & (X < xc)
    return inside


def poly_dist(P, X, Y):
    """Distance to the polygon's edges, vectorised."""
    best = np.full(np.broadcast(X, Y).shape, np.inf)
    n = len(P)
    for i in range(n):
        a = P[i]
        b = P[(i + 1) % n]
        d = b - a
        L2 = d @ d
        t = np.clip(((X - a[0]) * d[0] + (Y - a[1]) * d[1]) / L2, 0, 1)
        best = np.minimum(best, np.hypot(X - (a[0] + t * d[0]), Y - (a[1] + t * d[1])))
    return best


class Shape:
    def __init__(self, name):
        self.name = name
        if name == "pocket":
            self.seed = geom.SEED.copy()
            lo_x, hi_x, lo_y, hi_y = geom.OM
            self.mat_box = geom.RECT
        else:
            self.seed = np.array([49.5, 50.5])
            lo_x, hi_x = STAR[:, 0].min(), STAR[:, 0].max()
            lo_y, hi_y = STAR[:, 1].min(), STAR[:, 1].max()
            self.mat_box = (lo_x, hi_x, lo_y, hi_y)
        # two cells past the region on each side, so that a loop next to the
        # wall stays closed in the contouring
        self.xs = np.arange(lo_x - 2 * H, hi_x + 2.5 * H, H)
        self.ys = np.arange(lo_y - 2 * H, hi_y + 2.5 * H, H)
        self.GX, self.GY = np.meshgrid(self.xs, self.ys)
        self.omega = self.in_omega(self.GX, self.GY)
        self.disc = np.hypot(self.GX - self.seed[0], self.GY - self.seed[1]) <= R
        mb = self.mat_box
        self.mx = np.arange(mb[0], mb[1] + H / 2, H)
        self.my = np.arange(mb[2], mb[3] + H / 2, H)
        MGX, MGY = np.meshgrid(self.mx, self.my)
        self.MGX, self.MGY = MGX, MGY
        self.stock0 = self.in_stock(MGX, MGY)

    # tool-centre region
    def in_omega(self, X, Y):
        if self.name == "pocket":
            return geom.in_omega(X, Y)
        return poly_inside(STAR, X, Y) & (poly_dist(STAR, X, Y) >= R - 1e-12)

    def in_stock(self, X, Y):
        if self.name == "pocket":
            return geom.in_stock(X, Y)
        return poly_inside(STAR, X, Y)

    def wall_dist(self, X, Y):
        """Tool-centre distance to the nearest wall or island (to the region's
        boundary): the material-boundary distance minus R."""
        return self.boundary_dist(X, Y) - R

    def boundary_dist(self, X, Y):
        """Distance from a point inside the part's pocket to its boundary."""
        if self.name == "pocket":
            from march import pocket_boundary_distance
            return pocket_boundary_distance(X, Y)
        return poly_dist(STAR, X, Y)

    def entry_dist(self, X, Y):
        """Geodesic distance from the entry disc (radius R about the seed)."""
        if self.name == "pocket":
            return geom.geo(self.seed[0], self.seed[1], X, Y, far_only=False) - R
        # the star's tool-centre region is checked visible from the seed
        # (visible_from_seed); the distance is then straight
        return np.hypot(X - self.seed[0], Y - self.seed[1]) - R

    def visible_from_seed(self):
        """Fraction of region nodes (every 7th) whose segment to the seed stays
        inside, sampled at the grid spacing."""
        X, Y = self.GX[self.omega][::7], self.GY[self.omega][::7]  # every 7th node
        ok = np.ones(len(X), bool)
        L = np.hypot(X - self.seed[0], Y - self.seed[1])
        n = int(math.ceil(L.max() / H))
        for k in range(1, n):
            t = k / n
            ok &= self.in_omega(self.seed[0] + t * (X - self.seed[0]), self.seed[1] + t * (Y - self.seed[1]))
        return float(ok.mean())

    def outline(self):
        """Material outline polylines and island circles for drawing."""
        if self.name == "pocket":
            return dict(rect=(geom.RECT, geom.RX), circles=[(geom.ISL_C, geom.ISL_R)])
        return dict(poly=STAR, circles=[])

    def harness_view(self):
        """The harness View::over(outline): min_x, max_y, scale."""
        if self.name == "pocket":
            return 5.0, 55.0, (900 - 48) / 70.0
        w = STAR[:, 0].max() - STAR[:, 0].min()
        hgt = STAR[:, 1].max() - STAR[:, 1].min()
        return float(STAR[:, 0].min()), float(STAR[:, 1].max()), (900 - 48) / max(w, hgt)
