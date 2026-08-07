#!/usr/bin/env python3
"""ARP-1 render gallery — the spec's figures, drawn from the analytic
equations in arp1_reference.py so a verdict is never published without a
picture of the surface it is about."""

import math
import os

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np
from matplotlib.colors import BoundaryNorm, ListedColormap

from arp1_reference import (
    ConeAnnulus,
    Datum,
    MicroRipple,
    Saddle,
    SphereCap,
    StepTerrace,
    UGroove,
    VGroove,
    interp_error_xy_grid,
    interp_error_sphere_polar,
)

OUT = os.path.dirname(os.path.abspath(__file__))
TILE = 24.0
NT = 4
EXTENT = TILE * NT  # 96 mm


def rot(u, v, deg):
    c, s = math.cos(math.radians(deg)), math.sin(math.radians(deg))
    return u * c + v * s, -u * s + v * c


class Comb:
    """A comb of parallel grooves laid across the tile, each groove getting
    its own lane. Purely a placement wrapper around the groove zones."""

    def __init__(self, zones, pitch, name):
        self.zones, self.pitch, self.name = zones, pitch, name

    def z(self, u, v):
        out = np.zeros_like(u)
        n = len(self.zones)
        for k, g in enumerate(self.zones):
            uc = (k - (n - 1) / 2.0) * self.pitch
            out = np.minimum(out, g.z(u - uc, v))
        return out

    def dz(self, u, v):
        du = np.zeros_like(u)
        dv = np.zeros_like(u)
        n = len(self.zones)
        for k, g in enumerate(self.zones):
            uc = (k - (n - 1) / 2.0) * self.pitch
            a, b = g.dz(u - uc, v)
            m = g.support(u - uc, v)
            du = np.where(m, a, du)
            dv = np.where(m, b, dv)
        return du, dv

    def slope_deg(self, u, v):
        a, b = self.dz(u, v)
        return np.degrees(np.arctan(np.hypot(a, b)))


class ConeLadder:
    def __init__(self, degs, name):
        self.name = name
        self.zones = []
        r = 2.0
        for d in degs:
            self.zones.append(ConeAnnulus(d, r, r + 3.0))
            r += 3.5

    def z(self, u, v):
        out = np.zeros_like(u)
        for c in self.zones:
            out = np.minimum(out, c.z(u, v))
        return out

    def slope_deg(self, u, v):
        s = np.zeros_like(u)
        for c in self.zones:
            m = c.support(u, v)
            s = np.where(m, c.theta_deg, s)
        return s


# --- the ARP-1 tile map ----------------------------------------------------
LAYOUT = [
    # (col, row, object, plan rotation deg, label)
    (0, 0, Datum(), 0.0, "Z0 Datum"),
    (1, 0, SphereCap(8.0, +1), 0.0, "Z1 Dome R8"),
    (2, 0, SphereCap(8.0, -1), 0.0, "Z2 Bowl R8"),
    (3, 0, Saddle(8.0), 30.0, "Z3 Saddle rho8"),
    (0, 1, ConeLadder([44.0, 46.0], "cl1"), 0.0, "Z4a Cones 44/46"),
    (1, 1, ConeLadder([74.0, 76.0], "cl2"), 0.0, "Z4b Cones 74/76"),
    (2, 1, Comb([UGroove(r) for r in (0.2, 0.35, 0.5, 0.8, 1.5)], 3.2, "u"), 20.0,
     "Z5 U-groove comb"),
    (3, 1, Comb([VGroove(a, 1.6) for a in (15.0, 30.0, 45.0)], 5.0, "v"), 20.0,
     "Z6 V-groove comb"),
    (0, 2, StepTerrace(), 0.0, "Z9 StepTerrace"),
    (1, 2, MicroRipple(1.2), 30.0, "Z10 Ripple lam1.2"),
    (2, 2, MicroRipple(0.3), 30.0, "Z10 Ripple lam0.3"),
    (3, 2, MicroRipple(2.4), 30.0, "Z10 Ripple lam2.4"),
    (0, 3, SphereCap(3.0, +1), 0.0, "Z1s Dome R3"),
    (1, 3, SphereCap(3.0, -1), 0.0, "Z2s Bowl R3"),
    (2, 3, Saddle(3.0, half_extent=4.5), 0.0, "Z3s Saddle rho3"),
    (3, 3, Datum(), 0.0, "Z0 Datum"),
]


def field(n=1400):
    xs = np.linspace(0, EXTENT, n)
    X, Y = np.meshgrid(xs, xs, indexing="xy")
    Z = np.zeros_like(X)
    S = np.zeros_like(X)
    FEAT = np.zeros(X.shape)
    for k, (ci, rj, obj, phi, label) in enumerate(LAYOUT):
        cx, cy = TILE * (ci + 0.5), TILE * (rj + 0.5)
        inside = (np.abs(X - cx) <= TILE / 2) & (np.abs(Y - cy) <= TILE / 2)
        u, v = rot(X - cx, Y - cy, phi)
        z = obj.z(u, v)
        s = obj.slope_deg(u, v)
        Z = np.where(inside, z, Z)
        S = np.where(inside, s, S)
        FEAT = np.where(inside & ((np.abs(z) > 1e-9) | (s > 1e-9)), 1.0, FEAT)
    return X, Y, Z, S, FEAT


def fig_slope_map():
    X, Y, Z, S, F = field()
    bands = [0, 45, 75, 90.001]
    cmap = ListedColormap(["#4c8fd4", "#e8c34a", "#c0392b"])
    fig, ax = plt.subplots(1, 2, figsize=(15, 7.4))
    im0 = ax[0].pcolormesh(X, Y, Z, cmap="viridis", shading="auto")
    ax[0].set_title("ARP-1 height z(x,y) — exact, closed form")
    fig.colorbar(im0, ax=ax[0], label="z (mm)")
    im1 = ax[1].pcolormesh(X, Y, S, cmap=cmap, norm=BoundaryNorm(bands, 3), shading="auto")
    ax[1].set_title("ARP-1 slope band from the EXACT normal\n"
                    "blue Shallow <45°  ·  yellow MidSteep 45–75°  ·  red VerySteep >75°")
    fig.colorbar(im1, ax=ax[1], ticks=[22, 60, 82], label="band")
    # Outline every feature's support: a 44-deg cone is coloured EXACTLY like
    # the flat datum in a three-band map, so the run-off pair is invisible
    # without this overlay.  Recorded in the spec as a gate-design constraint.
    for a in ax:
        a.contour(X, Y, F, levels=[0.5], colors="k", linewidths=0.6)
    for a in ax:
        a.set_aspect("equal")
        a.set_xlabel("x (mm)")
        a.set_ylabel("y (mm)")
        for k in range(1, NT):
            a.axhline(TILE * k, color="w", lw=0.5, alpha=0.5)
            a.axvline(TILE * k, color="w", lw=0.5, alpha=0.5)
        for ci, rj, obj, phi, label in LAYOUT:
            a.text(TILE * ci + 0.6, TILE * rj + 0.9, label, fontsize=6.5,
                   color="w", family="monospace")
    fig.tight_layout()
    p = os.path.join(OUT, "arp1_zone_and_slope_map.png")
    fig.savefig(p, dpi=125)
    plt.close(fig)
    return p


def fig_cross_sections():
    fig, ax = plt.subplots(2, 2, figsize=(13, 8))
    # dome / bowl meridian with the exact band boundaries marked
    cap = SphereCap(8.0, +1)
    r = np.linspace(-cap.r_max, cap.r_max, 4000)
    ax[0][0].plot(r, cap.z(r, np.zeros_like(r)), lw=1.6, label="Dome R8")
    b = SphereCap(8.0, -1)
    ax[0][0].plot(r, b.z(r, np.zeros_like(r)), lw=1.6, label="Bowl R8")
    for d, c in ((45, "#e8c34a"), (75, "#c0392b")):
        for sgn in (-1, 1):
            ax[0][0].axvline(sgn * cap.iso_slope_radius(d), color=c, ls="--", lw=1)
    ax[0][0].set_title("Z1/Z2 sphere caps — dashed = EXACT 45°/75° band circles")
    ax[0][0].legend(fontsize=8)

    # U groove comb with the exact reach floors for a D1 ball
    us = np.linspace(-8, 8, 6000)
    comb = Comb([UGroove(rr) for rr in (0.2, 0.35, 0.5, 0.8, 1.5)], 3.2, "u")
    ax[0][1].plot(us, comb.z(us, np.zeros_like(us)), lw=1.4, color="k")
    for k, rr in enumerate((0.2, 0.35, 0.5, 0.8, 1.5)):
        uc = (k - 2) * 3.2
        g = UGroove(rr)
        f = g.reach_floor_mm(0.5)
        zb = -(g.R - g.z_rim) + f
        ax[0][1].plot([uc - g.u_max, uc + g.u_max], [zb, zb], color="#c0392b", lw=2)
        ax[0][1].text(uc, zb + 0.12, f"R{rr:g}\n{f*1000:.0f}µm", fontsize=6.5,
                      ha="center", color="#c0392b")
    ax[0][1].set_title("Z5 U-groove comb — red = EXACT D1.0-ball reach floor")

    # V groove comb
    combv = Comb([VGroove(a, 1.6) for a in (15.0, 30.0, 45.0)], 5.0, "v")
    ax[1][0].plot(us, combv.z(us, np.zeros_like(us)), lw=1.4, color="k")
    for k, a in enumerate((15.0, 30.0, 45.0)):
        uc = (k - 1) * 5.0
        g = VGroove(a, 1.6)
        f = g.reach_floor_mm(0.5)
        zb = -g.w / math.tan(g.alpha) + f
        ax[1][0].plot([uc - 1.6, uc + 1.6], [zb, zb], color="#c0392b", lw=2)
        ax[1][0].text(uc, zb + 0.25, f"α{a:g}°\n{f*1000:.0f}µm", fontsize=6.5,
                      ha="center", color="#c0392b")
    ax[1][0].set_title("Z6 V-groove comb — never enterable, floor is CLOSED FORM")

    # ripples
    for lam in (0.3, 1.2, 2.4):
        rp = MicroRipple(lam)
        ax[1][1].plot(us, rp.z(us, np.zeros_like(us)), lw=1.1,
                      label=f"λ={lam:g}, trough R={rp.trough_radius_mm():.3f}")
    ax[1][1].axhline(0, color="k", lw=0.4)
    ax[1][1].set_title("Z10 micro-ripple comb — constant 32.1° max slope,\n"
                       "trough radius swept ACROSS the D1.0 ball radius 0.5 mm")
    ax[1][1].legend(fontsize=7)
    for row in ax:
        for a in row:
            a.set_xlabel("u (mm)")
            a.set_ylabel("z (mm)")
            a.grid(alpha=0.25)
    fig.tight_layout()
    p = os.path.join(OUT, "arp1_cross_sections.png")
    fig.savefig(p, dpi=125)
    plt.close(fig)
    return p


def fig_tessellation():
    fig, ax = plt.subplots(1, 2, figsize=(13, 5.2))
    cases = [
        ("Dome R8 (κ=0.125)", SphereCap(8.0, +1), (-5.14, 5.14, -5.14, 5.14)),
        ("Saddle ρ8 (κ=0.125)", Saddle(8.0), (-9, 9, -9, 9)),
        ("U groove R1.5 (κ=0.667)", UGroove(1.5), (-1, 1, -1, 1)),
        ("Ripple λ0.3 (κ=13.16)", MicroRipple(0.3), (-1, 1, -1, 1)),
    ]
    for label, zone, (ul, uh, vl, vh) in cases:
        steps, p99 = [], []
        base = (uh - ul) / 8.0
        for k in range(5):
            s = base / 2**k
            _, e99, _ = interp_error_xy_grid(zone, s, ul, uh, vl, vh)
            steps.append(s)
            p99.append(e99)
        ax[0].loglog(steps, p99, "o-", ms=4, lw=1.2, label=label)
    ref = np.array([1e-2, 1e0])
    ax[0].loglog(ref, 300 * ref**2, "k--", lw=1, label="slope 2 reference")
    ax[0].set_xlabel("XY lattice step s (mm)")
    ax[0].set_ylabel("interpolation error p99 (µm)")
    ax[0].set_title("Tessellation error is MEASURED, and it is s²")
    ax[0].legend(fontsize=7)
    ax[0].grid(which="both", alpha=0.25)

    cap = SphereCap(8.0, +1, 85.0)
    degs = np.linspace(0, 85, 300)
    rr = cap.R * np.sin(np.radians(degs))
    d2 = cap.R**2 / np.maximum((cap.R**2 - rr * rr) ** 1.5, 1e-30)
    ax[1].semilogy(degs, 2 * np.sqrt(0.001 / d2), lw=2,
                   label="uniform XY lattice")
    ax[1].axhline(math.sqrt(8 * 0.001 * cap.R), color="#c0392b", lw=2,
                  label="uniform arc-length (polar) mesh")
    for d, c in ((45, "#e8c34a"), (75, "#c0392b")):
        ax[1].axvline(d, color=c, ls="--", lw=1)
    ax[1].set_xlabel("surface slope θ (deg)")
    ax[1].set_ylabel("step needed for a 1 µm sag (mm)")
    ax[1].set_title("Why a global XY grid is rejected:\n"
                    "its cost DIVERGES on steep ground, arc-length's does not")
    ax[1].legend(fontsize=8)
    ax[1].grid(which="both", alpha=0.25)
    fig.tight_layout()
    p = os.path.join(OUT, "arp1_tessellation_rule.png")
    fig.savefig(p, dpi=125)
    plt.close(fig)
    return p


def fig_aliasing():
    fig, ax = plt.subplots(figsize=(7.6, 5.2))
    cells = np.array([0.5, 0.25, 0.1, 0.05, 0.02, 0.01])
    for d, c in ((20, "#4c8fd4"), (45, "#e8c34a"), (75, "#c0392b"), (85, "#7d3c98")):
        ax.loglog(cells, cells * math.tan(math.radians(d)) * 1000, "o-", color=c,
                  ms=4, label=f"θ = {d}°")
    ax.axhline(10, color="k", ls="--", lw=1.4)
    ax.text(0.011, 11.5, "the prior campaign's ±10 µm bin", fontsize=8)
    ax.axvline(0.25, color="k", ls=":", lw=1.4)
    ax.text(0.26, 3.2, "its 0.25 mm grid", fontsize=8, rotation=90)
    ax.set_xlabel("simulation cell (mm)")
    ax.set_ylabel("phase-alias bound  cell·tan θ  (µm)")
    ax.set_title("A COLUMNS bin below cell·tan θ cannot separate two arms")
    ax.legend(fontsize=8)
    ax.grid(which="both", alpha=0.25)
    fig.tight_layout()
    p = os.path.join(OUT, "arp1_alias_bound.png")
    fig.savefig(p, dpi=125)
    plt.close(fig)
    return p


if __name__ == "__main__":
    for f in (fig_slope_map, fig_cross_sections, fig_tessellation, fig_aliasing):
        print("wrote", f())
