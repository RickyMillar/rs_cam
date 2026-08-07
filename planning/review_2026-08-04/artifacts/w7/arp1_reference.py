#!/usr/bin/env python3
"""ARP-1 analytic reference plate — executable specification and measurement rig.

This is the SPEC's reference implementation, not production code. It exists so
that every closed form in REFERENCE_FIXTURE_SPEC.md is executable, every band
area is checked against numerical integration, and the tessellation rule is
MEASURED rather than assumed, all before a line of Rust is written.

Run:  python3 arp1_reference.py            (measurements + renders)

Outputs land next to this file.
"""

import math
import os
import sys

import numpy as np

OUT = os.path.dirname(os.path.abspath(__file__))

# ---------------------------------------------------------------------------
# Zone definitions.  Each zone is a closed-form height field z(u, v) in its own
# LOCAL frame, plus exact first derivatives (hence the exact unit normal) and
# exact principal curvatures of the surface.
#
# Convention: local origin at the zone centre, datum plane z = 0, material
# below.  `support(u, v)` is the zone's exact plan-view support predicate;
# outside it the zone contributes z = 0 (datum).
# ---------------------------------------------------------------------------


class Zone:
    name = "zone"
    half_extent = 9.0

    def z(self, u, v):
        raise NotImplementedError

    def dz(self, u, v):
        """(dz/du, dz/dv) exact."""
        raise NotImplementedError

    def kappa_max(self, u, v):
        """Max |principal curvature| of the SURFACE at (u, v), 1/mm."""
        raise NotImplementedError

    def support(self, u, v):
        return np.abs(u) <= self.half_extent + 1e-12

    def normal(self, u, v):
        du, dv = self.dz(u, v)
        n = np.stack([-du, -dv, np.ones_like(du)], axis=-1)
        return n / np.linalg.norm(n, axis=-1, keepdims=True)

    def slope_deg(self, u, v):
        du, dv = self.dz(u, v)
        return np.degrees(np.arctan(np.hypot(du, dv)))


class Datum(Zone):
    name = "Z0 Datum"

    def z(self, u, v):
        return np.zeros_like(u)

    def dz(self, u, v):
        return np.zeros_like(u), np.zeros_like(u)

    def kappa_max(self, u, v):
        return np.zeros_like(u)


class SphereCap(Zone):
    """Hemispherical boss (sign=+1) or dimple (sign=-1), radius R, capped at
    polar angle theta_max so the rim slope is bounded.

    Boss:   z = sqrt(R^2 - r^2) - R cos(theta_max)   for r <= R sin(theta_max)
    Dimple: z = -(sqrt(R^2 - r^2) - R cos(theta_max))

    Both principal curvatures are exactly 1/R everywhere on the cap.
    Slope: sin(theta) = r / R, so the 45 deg iso-slope circle is at
    r = R/sqrt(2) and the 75 deg circle at r = R sin(75 deg) -- EXACT.
    """

    def __init__(self, R=8.0, sign=+1, theta_max_deg=85.0):
        self.R = R
        self.sign = sign
        self.theta_max = math.radians(theta_max_deg)
        self.r_max = R * math.sin(self.theta_max)
        self.z_rim = R * math.cos(self.theta_max)
        self.name = f"Z1 Dome R{R:g}" if sign > 0 else f"Z2 Bowl R{R:g}"

    def z(self, u, v):
        r = np.hypot(u, v)
        inside = r <= self.r_max
        rc = np.clip(r, 0.0, self.r_max)
        h = np.sqrt(np.maximum(self.R**2 - rc**2, 0.0)) - self.z_rim
        return np.where(inside, self.sign * h, 0.0)

    def dz(self, u, v):
        r = np.hypot(u, v)
        inside = (r <= self.r_max) & (r > 1e-12)
        rc = np.clip(r, 1e-12, self.r_max)
        dh_dr = -rc / np.sqrt(np.maximum(self.R**2 - rc**2, 1e-30))
        f = np.where(inside, self.sign * dh_dr / rc, 0.0)
        return f * u, f * v

    def kappa_max(self, u, v):
        r = np.hypot(u, v)
        return np.where(r <= self.r_max, 1.0 / self.R, 0.0)

    def support(self, u, v):
        return np.hypot(u, v) <= self.r_max

    # ---- closed-form ground truth -----------------------------------------
    def band_area_mm2(self, t0_deg, t1_deg):
        """Exact 3D surface area of the spherical zone between two slope
        angles: A = 2 pi R^2 (cos t0 - cos t1)."""
        t0, t1 = math.radians(t0_deg), math.radians(min(t1_deg, math.degrees(self.theta_max)))
        if t1 <= t0:
            return 0.0
        return 2.0 * math.pi * self.R**2 * (math.cos(t0) - math.cos(t1))

    def iso_slope_radius(self, deg):
        return self.R * math.sin(math.radians(deg))


class Saddle(Zone):
    """z = (u^2 - v^2) / (2 rho).  Principal curvatures +-1/rho at the origin
    (opposite signs -- a scalar mean curvature reads ZERO here, which is the
    mechanism this zone claims)."""

    def __init__(self, rho=8.0, half_extent=9.0):
        self.rho = rho
        self.half_extent = half_extent
        self.name = f"Z3 Saddle rho{rho:g}"

    def z(self, u, v):
        return np.where(self.support(u, v), (u * u - v * v) / (2.0 * self.rho), 0.0)

    def dz(self, u, v):
        s = self.support(u, v)
        return np.where(s, u / self.rho, 0.0), np.where(s, -v / self.rho, 0.0)

    def kappa_max(self, u, v):
        return np.where(self.support(u, v), 1.0 / self.rho, 0.0)

    def support(self, u, v):
        return (np.abs(u) <= self.half_extent) & (np.abs(v) <= self.half_extent)


class ConeAnnulus(Zone):
    """Right circular cone frustum: z = -tan(theta) * (r - r0) for
    r0 <= r <= r1.  Slope is EXACTLY theta everywhere; one principal curvature
    is 0 (the ruling) and the other is cos(theta)/r.  Placed in pairs
    straddling a shipped band boundary (44/46, 74/76 deg)."""

    def __init__(self, theta_deg, r0, r1):
        self.theta = math.radians(theta_deg)
        self.theta_deg = theta_deg
        self.r0, self.r1 = r0, r1
        self.name = f"Z4 Cone {theta_deg:g}deg"

    def z(self, u, v):
        r = np.hypot(u, v)
        inside = (r >= self.r0) & (r <= self.r1)
        return np.where(inside, -math.tan(self.theta) * (r - self.r0), 0.0)

    def dz(self, u, v):
        r = np.maximum(np.hypot(u, v), 1e-12)
        inside = (r >= self.r0) & (r <= self.r1)
        f = np.where(inside, -math.tan(self.theta) / r, 0.0)
        return f * u, f * v

    def kappa_max(self, u, v):
        r = np.maximum(np.hypot(u, v), 1e-12)
        inside = (r >= self.r0) & (r <= self.r1)
        return np.where(inside, math.cos(self.theta) / r, 0.0)

    def support(self, u, v):
        r = np.hypot(u, v)
        return (r >= self.r0) & (r <= self.r1)

    def slant_area_mm2(self):
        """Exact 3D area of the frustum: pi (r1^2 - r0^2) / cos(theta)."""
        return math.pi * (self.r1**2 - self.r0**2) / math.cos(self.theta)


class UGroove(Zone):
    """One straight circular groove of radius R, axis along +v, rim at z = 0:
    z = -sqrt(R^2 - u^2) for |u| <= R sin(phi_max).

    Ground truth this zone owns -- the EXACT tool reach floor.  A ball of
    radius rho inside a concave circular cylinder of radius R:
      rho <= R : the ball is tangent at the bottom, tip reaches z = -R,
                 residual 0.
      rho >  R : the ball rides the two rim corners at (+-R, 0); its centre
                 sits at zc = sqrt(rho^2 - R^2), tip at zc - rho, so the
                 residual at the centreline is exactly
                     R + sqrt(rho^2 - R^2) - rho.
    """

    def __init__(self, R, phi_max_deg=85.0):
        self.R = R
        self.u_max = R * math.sin(math.radians(phi_max_deg))
        self.z_rim = R * math.cos(math.radians(phi_max_deg))
        self.name = f"Z5 UGroove R{R:g}"

    def z(self, u, v):
        inside = np.abs(u) <= self.u_max
        uc = np.clip(u, -self.u_max, self.u_max)
        return np.where(inside, -(np.sqrt(np.maximum(self.R**2 - uc**2, 0.0)) - self.z_rim), 0.0)

    def dz(self, u, v):
        inside = np.abs(u) <= self.u_max
        uc = np.clip(u, -self.u_max, self.u_max)
        d = uc / np.sqrt(np.maximum(self.R**2 - uc**2, 1e-30))
        return np.where(inside, d, 0.0), np.zeros_like(u)

    def kappa_max(self, u, v):
        return np.where(np.abs(u) <= self.u_max, 1.0 / self.R, 0.0)

    def support(self, u, v):
        return np.abs(u) <= self.u_max

    def reach_floor_mm(self, ball_radius):
        rho, R = ball_radius, self.R
        if rho <= R:
            return 0.0
        return R + math.sqrt(rho**2 - R**2) - rho


class VGroove(Zone):
    """Symmetric V groove, half-angle alpha measured from the vertical axis:
    z = -(w - |u|) / tan(alpha) ... written as z = -(w - |u|) * cot(alpha)
    for |u| <= w, apex at z = -w cot(alpha).

    Ground truth: a ball of radius rho seats with its CENTRE at
    rho / sin(alpha) above the apex, so its tip is
        rho (1 - sin alpha) / sin alpha
    above the apex -- ALWAYS positive.  A V groove is never fully enterable,
    which is what makes it the honest 'this residual is geometry, not
    algorithm' control.
    """

    def __init__(self, alpha_deg, half_width):
        self.alpha = math.radians(alpha_deg)
        self.alpha_deg = alpha_deg
        self.w = half_width
        self.name = f"Z6 VGroove {alpha_deg:g}deg"

    def z(self, u, v):
        inside = np.abs(u) <= self.w
        return np.where(inside, -(self.w - np.abs(u)) / math.tan(self.alpha), 0.0)

    def dz(self, u, v):
        inside = np.abs(u) <= self.w
        d = np.sign(u) / math.tan(self.alpha)
        return np.where(inside, d, 0.0), np.zeros_like(u)

    def kappa_max(self, u, v):
        return np.zeros_like(u)  # ruled planar flanks; the apex is a crease

    def support(self, u, v):
        return np.abs(u) <= self.w

    def reach_floor_mm(self, ball_radius):
        return ball_radius * (1.0 - math.sin(self.alpha)) / math.sin(self.alpha)

    def slope_deg_flank(self):
        return 90.0 - self.alpha_deg


class MicroRipple(Zone):
    """z = A sin(2 pi u / lam).  Sweeps the tool-bridging threshold: a ball of
    radius rho bridges relief whose CONCAVE curvature radius is smaller than
    rho.  The trough curvature radius is lam^2 / (4 pi^2 A), so the exact
    bridging threshold is rho > lam^2 / (4 pi^2 A)."""

    def __init__(self, lam, amp_ratio=0.10, half_extent=9.0):
        self.lam = lam
        self.A = lam * amp_ratio
        self.half_extent = half_extent
        self.name = f"Z10 Ripple lam{lam:g}"

    def z(self, u, v):
        return np.where(self.support(u, v), self.A * np.sin(2 * np.pi * u / self.lam), 0.0)

    def dz(self, u, v):
        s = self.support(u, v)
        d = self.A * (2 * np.pi / self.lam) * np.cos(2 * np.pi * u / self.lam)
        return np.where(s, d, 0.0), np.zeros_like(u)

    def kappa_max(self, u, v):
        return np.where(self.support(u, v), self.A * (2 * np.pi / self.lam) ** 2, 0.0)

    def support(self, u, v):
        return (np.abs(u) <= self.half_extent) & (np.abs(v) <= self.half_extent)

    def trough_radius_mm(self):
        return self.lam**2 / (4 * math.pi**2 * self.A)

    def max_slope_deg(self):
        return math.degrees(math.atan(self.A * 2 * math.pi / self.lam))


class StepTerrace(Zone):
    """Flat terraces at z = 0, -1, -2, -3 with vertical risers -- the exact
    Z-level / waterline fixture.  Per-level projected area is exact."""

    def __init__(self, n=4, drop=1.0, half_extent=9.0):
        self.n, self.drop, self.half_extent = n, drop, half_extent
        self.name = "Z9 StepTerrace"

    def _level(self, u):
        k = np.floor((u + self.half_extent) / (2 * self.half_extent / self.n))
        return np.clip(k, 0, self.n - 1)

    def z(self, u, v):
        return np.where(self.support(u, v), -self.drop * self._level(u), 0.0)

    def dz(self, u, v):
        return np.zeros_like(u), np.zeros_like(u)

    def kappa_max(self, u, v):
        return np.zeros_like(u)

    def support(self, u, v):
        return (np.abs(u) <= self.half_extent) & (np.abs(v) <= self.half_extent)


# ---------------------------------------------------------------------------
# Tessellation-error measurement.
#
# Two parametrisations are compared, because choosing between them IS the
# tessellation rule:
#
#   (a) XY-GRID  -- triangulate a uniform (u, v) lattice of step s and read the
#       piecewise-linear interpolant.  This is what every height-field fixture
#       in the repo does today, including the incumbent TIN.
#   (b) ARC-LENGTH -- triangulate uniformly in the surface's own parameter, so
#       the CHORD length on the surface is constant.  For a sphere that is a
#       uniform polar-angle mesh; for a groove, uniform in the wrap angle.
#
# The error is evaluated at dense sample points and reported as p50/p99/max of
# |z_linear - z_exact| in micrometres.
# ---------------------------------------------------------------------------


def interp_error_xy_grid(zone, step, u_lo, u_hi, v_lo, v_hi, n_probe=400):
    """Max/p50/p99 deviation of the piecewise-linear interpolant of a uniform
    (u, v) lattice from the exact surface, evaluated on a dense probe grid."""
    nu = max(2, int(round((u_hi - u_lo) / step)) + 1)
    nv = max(2, int(round((v_hi - v_lo) / step)) + 1)
    us = np.linspace(u_lo, u_hi, nu)
    vs = np.linspace(v_lo, v_hi, nv)
    UU, VV = np.meshgrid(us, vs, indexing="ij")
    ZZ = zone.z(UU, VV)

    pu = np.linspace(u_lo, u_hi, n_probe)
    pv = np.linspace(v_lo, v_hi, n_probe)
    PU, PV = np.meshgrid(pu, pv, indexing="ij")
    exact = zone.z(PU, PV)

    # locate each probe in its cell, then in one of the two triangles
    fi = np.clip(((PU - u_lo) / (u_hi - u_lo) * (nu - 1)), 0, nu - 1 - 1e-9)
    fj = np.clip(((PV - v_lo) / (v_hi - v_lo) * (nv - 1)), 0, nv - 1 - 1e-9)
    i0 = fi.astype(int)
    j0 = fj.astype(int)
    a = fi - i0
    b = fj - j0
    z00 = ZZ[i0, j0]
    z10 = ZZ[i0 + 1, j0]
    z01 = ZZ[i0, j0 + 1]
    z11 = ZZ[i0 + 1, j0 + 1]
    # split each quad on the (0,0)-(1,1) diagonal
    lower = a + b <= 1.0
    lin = np.where(
        lower,
        z00 + a * (z10 - z00) + b * (z01 - z00),
        z11 + (1 - a) * (z01 - z11) + (1 - b) * (z10 - z11),
    )
    err = np.abs(lin - exact)
    ok = np.isfinite(err)
    e = np.sort(err[ok].ravel())
    return e[len(e) // 2] * 1000, e[int(0.99 * len(e))] * 1000, e[-1] * 1000


def interp_error_sphere_polar(cap, chord_mm, theta_max_deg, n_probe=4000):
    """Error of a mesh built uniformly in POLAR ANGLE on a spherical cap,
    measured along a meridian (the worst direction).  chord_mm is the surface
    arc step; d_theta = chord / R."""
    R = cap.R
    dth = chord_mm / R
    th_max = math.radians(theta_max_deg)
    n = max(2, int(math.ceil(th_max / dth)) + 1)
    ths = np.linspace(0.0, th_max, n)
    # meridian polyline in (r, z)
    rs = R * np.sin(ths)
    zs = cap.sign * (R * np.cos(ths) - cap.z_rim)
    pr = np.linspace(0.0, R * math.sin(th_max), n_probe)
    lin = np.interp(pr, rs, zs)
    exact = cap.z(pr, np.zeros_like(pr))
    err = np.abs(lin - exact)
    e = np.sort(err)
    return e[len(e) // 2] * 1000, e[int(0.99 * len(e))] * 1000, e[-1] * 1000


def fit_exponent(steps, errs):
    """Least-squares slope of log(err) vs log(step)."""
    x = np.log(np.array(steps))
    y = np.log(np.array(errs))
    A = np.vstack([x, np.ones_like(x)]).T
    m, c = np.linalg.lstsq(A, y, rcond=None)[0]
    return m


def main():
    lines = []

    def emit(s=""):
        print(s)
        lines.append(s)

    emit("# ARP-1 reference measurements")
    emit()
    emit(f"numpy {np.__version__}")
    emit()

    # -- 1. exact band areas, checked against numerical integration ----------
    emit("## 1. Exact slope-band areas (spherical cap, R = 8, capped at 85 deg)")
    emit()
    cap = SphereCap(R=8.0, sign=+1, theta_max_deg=85.0)
    emit("| band | closed form 2*pi*R^2*(cos t0 - cos t1) mm2 | numeric integral mm2 | rel err |")
    emit("|---|---|---|---|")
    for t0, t1, label in [(0, 45, "Shallow 0-45"), (45, 75, "MidSteep 45-75"), (75, 85, "VerySteep 75-85")]:
        exact = cap.band_area_mm2(t0, t1)
        # numeric: A = int 2 pi R sin(t) * R dt
        th = np.linspace(math.radians(t0), math.radians(min(t1, 85)), 200001)
        num = np.trapezoid(2 * math.pi * cap.R**2 * np.sin(th), th)
        emit(f"| {label} | {exact:.4f} | {num:.4f} | {abs(exact-num)/exact:.2e} |")
    emit(f"| **total cap** | {cap.band_area_mm2(0,85):.4f} | | |")
    emit()
    emit(f"45 deg iso-slope circle at r = {cap.iso_slope_radius(45):.6f} mm; "
         f"75 deg at r = {cap.iso_slope_radius(75):.6f} mm (EXACT).")
    emit()

    # -- 2. exact reach floors ----------------------------------------------
    emit("## 2. Exact tool-reach floors (what no algorithm can remove)")
    emit()
    for rho, label in [(0.5, "Ball D1.0 (rho=0.5)"), (1.5, "Ball D3.0 (rho=1.5)")]:
        emit(f"### {label}")
        emit()
        emit("| feature | exact residual at centreline mm | enterable? |")
        emit("|---|---|---|")
        for R in [0.2, 0.35, 0.5, 0.8, 1.5, 3.0]:
            g = UGroove(R)
            f = g.reach_floor_mm(rho)
            emit(f"| U groove R={R:g} | {f:.6f} | {'YES' if f == 0.0 else 'no'} |")
        for a in [15.0, 30.0, 45.0]:
            g = VGroove(a, 3.0)
            emit(f"| V groove half-angle {a:g} deg (flank {g.slope_deg_flank():g} deg) "
                 f"| {g.reach_floor_mm(rho):.6f} | never |")
        emit()

    # -- 3. ripple bridging threshold ---------------------------------------
    emit("## 3. Micro-ripple bridging threshold (ball radius vs trough radius)")
    emit()
    emit("| lambda mm | amplitude mm | trough radius mm | max slope deg | D1.0 ball bridges? |")
    emit("|---|---|---|---|---|")
    for lam in [0.3, 0.6, 1.2, 2.4, 4.8]:
        r = MicroRipple(lam)
        tr = r.trough_radius_mm()
        emit(f"| {lam:g} | {r.A:.4f} | {tr:.4f} | {r.max_slope_deg():.2f} | "
             f"{'YES (geometry, not defect)' if 0.5 > tr else 'no -- reaches bottom'} |")
    emit()

    # -- 4. THE TESSELLATION MEASUREMENT ------------------------------------
    emit("## 4. Tessellation error, MEASURED")
    emit()
    emit("Bound under test: for a piecewise-linear interpolant of a surface of")
    emit("max principal curvature kappa sampled at chord length L, the sag is")
    emit("`eps <= kappa L^2 / 8` along an edge and `<= kappa L^2 / 4` across the")
    emit("diagonal of a square cell.  Pre-registered prediction: **err ~ s^2**,")
    emit("i.e. a log-log slope of 2.00, and halving the step divides the error")
    emit("by 4.0.")
    emit()

    cases = [
        ("Dome R=8, scored 0-40 deg (r <= 5.14)", SphereCap(8.0, +1), (-5.14, 5.14, -5.14, 5.14)),
        ("Bowl R=8, scored 0-40 deg", SphereCap(8.0, -1), (-5.14, 5.14, -5.14, 5.14)),
        ("Saddle rho=8", Saddle(8.0), (-9.0, 9.0, -9.0, 9.0)),
        ("U groove R=1.5, |u| <= 1.0", UGroove(1.5), (-1.0, 1.0, -1.0, 1.0)),
        ("U groove R=0.35, |u| <= 0.25", UGroove(0.35), (-0.25, 0.25, -0.25, 0.25)),
        ("Ripple lambda=1.2", MicroRipple(1.2), (-4.0, 4.0, -4.0, 4.0)),
        ("Ripple lambda=0.3", MicroRipple(0.3), (-1.0, 1.0, -1.0, 1.0)),
    ]
    for label, zone, (ul, uh, vl, vh) in cases:
        emit(f"### {label}")
        emit()
        kap = float(np.max(np.atleast_1d(zone.kappa_max(np.array([0.0]), np.array([0.0])))))
        emit(f"kappa (analytic) = {kap:.4f} /mm")
        emit()
        emit("| XY step s mm | p50 um | p99 um | max um | bound kappa*s^2/4 um |")
        emit("|---|---|---|---|---|")
        steps, p99s = [], []
        base = (uh - ul) / 8.0
        for k in range(5):
            s = base / (2**k)
            p50, p99, mx = interp_error_xy_grid(zone, s, ul, uh, vl, vh)
            emit(f"| {s:.5f} | {p50:.4f} | {p99:.4f} | {mx:.4f} | {kap*s*s/4*1000:.4f} |")
            steps.append(s)
            p99s.append(max(p99, 1e-12))
        m = fit_exponent(steps, p99s)
        ratio = p99s[-2] / p99s[-1] if p99s[-1] > 0 else float("nan")
        emit()
        emit(f"**log-log slope = {m:.3f}** (predicted 2.00); last halving divided p99 by {ratio:.2f} (predicted 4.00)")
        emit()

    # -- 5. XY grid vs arc-length parametrisation on a STEEP cap ------------
    emit("## 5. Why a global XY grid is rejected: the steep-rim measurement")
    emit()
    emit("A height field's apparent second derivative on a sphere is")
    emit("`d2z/dr2 = -R^2 / (R^2 - r^2)^{3/2}`, which DIVERGES at the rim even")
    emit("though the surface curvature stays 1/R.  A uniform XY lattice must")
    emit("therefore be refined without bound to hold a fixed sag on steep")
    emit("ground; a uniform POLAR-ANGLE mesh holds it at constant cost.")
    emit()
    cap = SphereCap(8.0, +1, theta_max_deg=85.0)
    emit("| slope theta deg | height-field d2z/dr2 /mm | XY step for 1 um sag mm | arc step for 1 um sag mm |")
    emit("|---|---|---|---|")
    for deg in [0, 30, 45, 60, 75, 85]:
        r = cap.R * math.sin(math.radians(deg))
        d2 = cap.R**2 / max((cap.R**2 - r * r) ** 1.5, 1e-30)
        s_xy = 2 * math.sqrt(0.001 / d2)
        s_arc = math.sqrt(8 * 0.001 * cap.R)
        emit(f"| {deg} | {d2:.4f} | {s_xy:.5f} | {s_arc:.5f} |")
    emit()
    emit("| arc chord mm | meridian p50 um | p99 um | max um |")
    emit("|---|---|---|---|")
    chords, p99s = [], []
    for c in [0.8, 0.4, 0.2, 0.1, 0.05]:
        p50, p99, mx = interp_error_sphere_polar(cap, c, 85.0)
        emit(f"| {c:.3f} | {p50:.4f} | {p99:.4f} | {mx:.4f} |")
        chords.append(c)
        p99s.append(max(p99, 1e-12))
    emit()
    emit(f"**log-log slope = {fit_exponent(chords, p99s):.3f}** (predicted 2.00), "
         f"and the error is SLOPE-INDEPENDENT: the same chord holds 0 deg and 85 deg.")
    emit()

    # -- 6. the aliasing bound ----------------------------------------------
    emit("## 6. Grid-aliasing bound for a COLUMNS-class instrument")
    emit()
    emit("A column samples the surface at a FIXED lateral position.  Two arms")
    emit("whose surfaces differ in phase relative to that lattice by up to one")
    emit("cell read a Z difference of up to `cell * tan(theta)` from geometry")
    emit("alone.  A bin narrower than this cannot separate arms on that slope.")
    emit()
    emit("| sim cell mm | Shallow (45 deg) um | MidSteep (75 deg) um | VerySteep (85 deg) um |")
    emit("|---|---|---|---|")
    for cell in [0.5, 0.25, 0.1, 0.05, 0.02, 0.01]:
        row = [cell * math.tan(math.radians(d)) * 1000 for d in (45, 75, 85)]
        emit(f"| {cell:g} | {row[0]:.0f} | {row[1]:.0f} | {row[2]:.0f} |")
    emit()
    emit("The prior campaign's +-10 um bin at a 0.25 mm cell is below this bound")
    emit("by a factor of 25 on flat ground and 93 at 75 deg.  That is the")
    emit("arithmetic behind 'sub-repeatability and grid-aliased'.")
    emit()

    with open(os.path.join(OUT, "arp1_measurements.md"), "w") as f:
        f.write("\n".join(lines) + "\n")
    print(f"\nwrote {os.path.join(OUT, 'arp1_measurements.md')}")


if __name__ == "__main__":
    main()
