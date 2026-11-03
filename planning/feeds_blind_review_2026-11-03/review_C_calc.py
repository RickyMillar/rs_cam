#!/usr/bin/env python3
"""Blind review group C: chip loads, forces, deflection, power and motion limits.

Run: python3 -I planning/feeds_blind_review_2026-11-03/review_C_calc.py

Every constant names its source. "judgement" marks a value with no source.
The script asserts the claims that review_C.md makes from these numbers.
"""
import math

# ---------------------------------------------------------------- machine
# ~/.config/rs_cam/machines/shapeoko_pro_xxl_scylla.toml and brief 00.
RPM_MIN, RPM_MAX = 6000.0, 24000.0
P_RATED_W, RPM_RATED = 1500.0, 24000.0           # VfdConstantTorque
A_XY, A_Z = 1200.0, 550.0                        # mm/s^2
VMAX_Z = 2500.0                                  # mm/min
JD = 0.1                                         # junction deviation, mm
FEED_CAP = 6000.0                                # rs_cam default cutting cap, brief 00


def power_avail_w(n):
    return P_RATED_W * min(n, RPM_RATED) / RPM_RATED


# ---------------------------------------------------------------- material
# White oak SG 0.68 (FPL Wood Handbook Table 5-3a, brief 00); rho = SG*1000*1.12.
RHO = 0.68 * 1000 * 1.12


# Curti et al. 2021, Eur. J. Wood Wood Prod., doi:10.1007/s00107-021-01667-5,
# Table 5, helix 0 (as quoted in crates/rs_cam_core/src/material/force_line.rs):
# Fc/ap = Ks*h + F_edge, Ks = Ks_n*rho, F_edge = Int_n*rho, Ks_n, Int_n quadratic
# in the grain angle GA (deg). Printed mean-chip range 0.041-0.091 mm.
def quad(a, b, c, ga):
    return a * ga * ga + b * ga + c


def curti_terms(mode, ga):
    if mode == "up":
        return quad(-5e-6, 1e-3, 26e-3, ga), quad(-1e-7, 1e-5, 55e-4, ga)
    return quad(-3e-6, 5e-4, 56e-3, ga), quad(-3e-7, 4e-5, 47e-4, ga)


def curti_envelope():
    ks = max(curti_terms(m, g / 10)[0] for m in ("up", "down") for g in range(0, 901))
    it = max(curti_terms(m, g / 10)[1] for m in ("up", "down") for g in range(0, 901))
    return ks * RHO, it * RHO


def curti_low():
    # The low bound: down-milling along the grain (GA 0).
    ks, it = curti_terms("down", 0.0)
    return ks * RHO, it * RHO


KS_HI, FE_HI = curti_envelope()
KS_LO, FE_LO = curti_low()

# ---------------------------------------------------------------- carbide
E_CARBIDE = 600e3        # N/mm^2 (task brief: carbide E ~ 600 GPa)
SIGMA_ALLOW = 1000.0     # N/mm^2, judgement: fatigue-safe bending stress for
                         # micrograin carbide, well under its rupture strength.
RESULTANT = math.sqrt(2) # judgement: radial force = tangential force (conservative)

# ---------------------------------------------------------------- rules
RUB_FLOOR = 0.025        # mm, Shaw (sources.toml shaw_metal_cutting)


def rctf(ae, d):
    """DAPRA radial chip thinning factor (sources.toml dapra_rctf), ae < d/2."""
    if ae >= d / 2:
        return 1.0
    return d / (2 * math.sqrt(ae * (d - ae)))


def ball_deff(ap, r):
    """IMCO 'Adjustments for ball nose end mills': De = 2 sqrt(R^2 - (R-ap)^2)."""
    ap = min(ap, r)
    return 2 * math.sqrt(r * r - (r - ap) ** 2)


# ---------------------------------------------------------------- tools
class TaperedBall:
    """R1 x 6 x 20 2F tapered ball (endmill.com.au table: R1, 6 mm shank, 20 mm, angle 5.7)."""

    def __init__(self, stickout):
        self.r = 1.0
        self.alpha = math.atan((6.0 / 2 - 1.0) / 20.0)   # 5.71 deg half angle
        self.h_t = self.r * (1 - math.sin(self.alpha))   # tangent height 0.90 mm
        self.flute = 20.0
        self.shank = 6.0
        self.L = stickout
        self.z = 2
        self.k_core = 0.8   # judgement: 2-flute equivalent bending diameter / D

    def dia(self, h):
        if h <= self.h_t:
            return 2 * math.sqrt(max(h * (2 * self.r - h), 0.0))
        return 2 * (self.r * math.cos(self.alpha) + (h - self.h_t) * math.tan(self.alpha))

    def bend_dia(self, s):
        if s >= self.flute:
            return self.shank
        return max(self.k_core * self.dia(s), 1e-3)

    def edge_len(self, ap):
        """Length of cutting edge from the tip to height ap."""
        if ap <= self.h_t:
            return self.r * math.acos(1 - ap / self.r)
        theta_t = math.pi / 2 - self.alpha
        return self.r * theta_t + (ap - self.h_t) / math.cos(self.alpha)


class SingleFlute2:
    """DMM DSP1f-D2.0-L08: 2.0 mm, 1 flute, 8 mm flute; shank ESTIMATE."""

    def __init__(self, stickout=20.0, shank=3.175, k_core=0.7):
        self.d = 2.0
        self.flute = 8.0
        self.shank = shank
        self.L = stickout
        self.z = 1
        self.k_core = k_core   # judgement: 1-flute equivalent bending diameter / D

    def bend_dia(self, s):
        return self.k_core * self.d if s < self.flute else self.shank


def deflection(tool, F, x_load, n=4000):
    """Cantilever clamped at tool.L, point load F at x_load from the tip.
    delta = integral_{x_load}^{L} F (s - x_load)^2 / (E I(s)) ds."""
    tot = 0.0
    ds = (tool.L - x_load) / n
    for i in range(n):
        s = x_load + (i + 0.5) * ds
        d = tool.bend_dia(s)
        inertia = math.pi * d ** 4 / 64
        tot += (s - x_load) ** 2 / inertia * ds
    return F * tot / E_CARBIDE


def max_stress(tool, F, x_load, n=4000):
    best = (0.0, 0.0)
    ds = (tool.L - x_load) / n
    for i in range(n):
        s = x_load + (i + 0.5) * ds
        d = tool.bend_dia(s)
        sig = F * (s - x_load) / (math.pi * d ** 3 / 32)
        if sig > best[0]:
            best = (sig, s)
    return best


def peak_force(edge, h, hi=True):
    ks, fe = (KS_HI, FE_HI) if hi else (KS_LO, FE_LO)
    return edge * (ks * h + fe)


def power_w(n, z, ap_edge, ae, fz, deff, hi=True):
    """P = n z/60 * (Ks ap ae fz + F_edge ap R psi), psi the engagement angle."""
    ks, fe = (KS_HI, FE_HI) if hi else (KS_LO, FE_LO)
    r = deff / 2
    psi = math.acos(max(-1.0, 1 - ae / r)) if ae < deff else math.pi
    e_tooth = ks * ap_edge * ae * fz + fe * ap_edge * r * psi   # N*mm per tooth pass
    return n * z / 60 * e_tooth / 1000


def corner_speed_mm_min(turn_deg):
    """grbl junction speed for a direction change of turn_deg."""
    theta = math.radians(180 - turn_deg)
    s = math.sin(theta / 2)
    if s >= 1:
        return float("inf")
    return math.sqrt(A_XY * JD * s / (1 - s)) * 60


def curve_speed_mm_min(radius):
    return math.sqrt(A_XY * radius) * 60


def accel_dist(feed):
    v = feed / 60
    return v * v / (2 * A_XY)


# ================================================================ output
def line(*a):
    print(*a)


line(f"rho oak = {RHO:.1f} kg/m3")
line(f"Curti envelope: Ks = {KS_HI:.2f} N/mm2, F_edge = {FE_HI:.3f} N/mm")
line(f"Curti low (down, GA 0): Ks = {KS_LO:.2f} N/mm2, F_edge = {FE_LO:.3f} N/mm")
line(f"constant torque = {P_RATED_W / (2 * math.pi * RPM_RATED / 60):.3f} N m")

# ---------------- DMM 2 mm 1F
line("\n== DMM 2 mm 1F (briefs 07, 08)")
dmm = SingleFlute2()
dmm_thin = SingleFlute2(shank=2.0)
v24 = math.pi * 2.0 * 24000 / 1000
line(f"surface speed at 24000 rpm: {v24:.0f} m/min")
for fz in (0.025, 0.03, 0.045, 0.06):
    line(f"fz {fz:.3f}: feed at 24000 = {24000 * fz:.0f}, at 18000 = {18000 * fz:.0f} mm/min")
rows = []
for ap, ae, label in ((0.5, 2.0, "slot ap0.5"), (0.5, 1.0, "50% ap0.5"), (1.0, 2.0, "slot ap1.0"),
                      (1.0, 1.0, "50% ap1.0"), (2.0, 2.0, "slot ap2.0"), (2.0, 0.5, "25% ap2.0"),
                      (5.0, 2.0, "slot ap5.0 (one-pass antenna, not advised)")):
    for fz in (0.03, 0.045):
        h = fz / rctf(ae, 2.0)
        F = peak_force(ap, h) * RESULTANT
        xl = ap / 2
        d1 = deflection(dmm, F, xl)
        d2 = deflection(dmm_thin, F, xl)
        sig, at = max_stress(dmm, F, xl)
        p = power_w(24000, 1, ap, ae, fz, 2.0)
        rows.append((label, fz, h, F, d1, d2, sig, p))
        line(f"{label:42s} fz {fz:.3f} hmax {h:.3f} F {F:5.1f} N  defl(3.175 shank) {d1 * 1000:5.1f} um"
             f"  defl(2 mm shank) {d2 * 1000:6.1f} um  sigma {sig:5.0f} MPa @ {at:.1f} mm  P {p:5.1f} W")

# claims
slot05 = [r for r in rows if r[0] == "slot ap0.5" and r[1] == 0.045][0]
assert slot05[4] < 0.015, "DMM slot ap0.5 deflection under 15 um"
slot20 = [r for r in rows if r[0] == "slot ap2.0" and r[1] == 0.045][0]
assert slot20[4] < 0.060 and slot20[6] < SIGMA_ALLOW, "DMM slot ap 2.0: under 60 um, stress OK"
assert all(r[7] < 0.1 * power_avail_w(24000) for r in rows), "power under 10 % of available"
assert all(r[5] > 2.5 * r[4] for r in rows), "a 2 mm shank makes deflection > 2.5x"
line(f"rctf(1.0, 2.0) = {rctf(1.0, 2.0):.3f} (50 % rings: no thinning)")

# ramps
line("ramp geometry, DMM:")
for rh in (0.6, 0.8, 1.0):
    for pitch in (0.25, 0.5, 1.0):
        ang = math.degrees(math.atan(pitch / (2 * math.pi * rh)))
        line(f"  helix radius {rh:.1f} pitch {pitch:.2f}: {ang:4.1f} deg")
line(f"  linear ramp 3 deg for 0.5 mm: length {0.5 / math.tan(math.radians(3)):.1f} mm;"
     f" 1.0 mm: {1.0 / math.tan(math.radians(3)):.1f} mm")
assert abs(math.degrees(math.atan(1.0 / (2 * math.pi * 1.0))) - 9.04) < 0.05

# acceleration on lake rings
for feed in (720, 1080):
    line(f"feed {feed}: accel distance {accel_dist(feed):.3f} mm, min radius at full feed "
         f"{(feed / 60) ** 2 / A_XY:.3f} mm, 90 deg corner speed {corner_speed_mm_min(90):.0f} mm/min")
assert corner_speed_mm_min(90) > 1000

# ---------------- R1 tapered ball
line("\n== R1 x 6 x 20 tapered ball (briefs 15, 16)")
tb30, tb35 = TaperedBall(30.0), TaperedBall(35.0)
line(f"half angle {math.degrees(tb30.alpha):.2f} deg, tangent height {tb30.h_t:.3f} mm")
for h in (0.1, 0.25, 0.5, 0.9, 1.0, 1.5, 2.0, 2.5, 3.0, 5.4):
    line(f"  h {h:4.2f}: D {tb30.dia(h):.3f} mm, edge length {tb30.edge_len(h):.2f} mm")
assert abs(tb30.dia(0.5) - 1.732) < 0.002 and abs(tb30.dia(2.0) - 2.211) < 0.003

line("rest rough options (contour_parallel z-levels):")
rest = []
for ap, ae in ((2.0, 0.66), (1.5, 0.8), (1.0, 1.0), (0.5, 0.87), (1.0, 2.0)):
    deff = tb30.dia(ap)
    th = rctf(ae, deff)
    for h_t in (0.03, 0.05):
        fz = h_t * th
        F = peak_force(tb30.edge_len(ap), h_t) * RESULTANT
        xl = ap * 0.5
        d30 = deflection(tb30, F, xl)
        d35 = deflection(tb35, F, xl)
        sig, at = max_stress(tb35, F, xl)
        p = power_w(24000, 2, tb30.edge_len(ap), ae, fz, deff)
        feed = 24000 * 2 * fz
        rest.append((ap, ae, h_t, fz, feed, F, d30, d35, sig, p))
        line(f"  ap {ap:.1f} ae {ae:.2f} De {deff:.2f} rctf {th:.3f} h {h_t:.3f} fz {fz:.4f} "
             f"feed@24k {feed:5.0f} F {F:5.1f} N defl30 {d30 * 1000:5.1f} um defl35 {d35 * 1000:5.1f} um"
             f" sigma {sig:4.0f} MPa @ {at:.1f}  P {p:5.1f} W")
assert all(r[4] <= FEED_CAP for r in rest)
r2 = [r for r in rest if r[0] == 2.0 and r[2] == 0.03][0]
assert r2[6] < 0.040, "R1 ap2 ae0.66 h0.03 under 40 um at 30 mm"

line("deep bite (finish meets standing stock up to 5.4 mm, full width):")
for h_t in (0.02, 0.03, 0.04):
    ap = 5.4
    F = peak_force(tb30.edge_len(ap), h_t) * RESULTANT
    d30 = deflection(tb30, F, ap * 0.5)
    d35 = deflection(tb35, F, ap * 0.5)
    sig, at = max_stress(tb35, F, ap * 0.5)
    line(f"  h {h_t:.2f}: F {F:5.1f} N defl30 {d30 * 1000:5.1f} um defl35 {d35 * 1000:5.1f} um "
         f"sigma {sig:4.0f} MPa @ {at:.1f} mm")
    if h_t == 0.04:
        deep = (F, d30, d35, sig)
assert deep[3] < SIGMA_ALLOW, "deep bite: no breakage by bending"
assert deep[1] < 0.040 and deep[2] < 0.040, "deep bite: the taper keeps deflection under 40 um"

line("scallop finish:")
r = 1.0
stepover = 2 * math.sqrt(2 * r * 0.1 - 0.1 ** 2)
deff = tb30.dia(0.5)
axial = 2.0 / deff                          # IMCO MMPTadj = D*MMPT/De
line(f"  flat stepover for 0.1 mm scallop {stepover:.3f} mm; De(0.5) {deff:.3f};"
     f" IMCO axial factor {axial:.3f}; ae/De {stepover / deff:.2f} rctf {rctf(stepover, deff):.3f}")
for h_t in (0.03, 0.04):
    fz = h_t * axial
    for n in (16000, 18000, 20000, 24000):
        line(f"  h {h_t:.2f} fz {fz:.4f} rpm {n}: feed {n * 2 * fz:5.0f} mm/min")
for feed in (1000, 1500, 2000, 2500, 3000):
    for n in (18000, 24000):
        h = feed / (n * 2) / axial
        line(f"  real feed {feed} at {n}: chip {h:.4f} mm {'UNDER floor' if h < RUB_FLOOR else ''}")
line("  motion limits:")
for turn in (10, 20, 30, 45, 90):
    line(f"    junction speed, {turn:3d} deg turn: {corner_speed_mm_min(turn):6.0f} mm/min")
for rad in (0.5, 1, 2, 5, 10):
    line(f"    curve radius {rad:4.1f} mm: {curve_speed_mm_min(rad):5.0f} mm/min")
for slope in (30, 45, 60, 75, 90):
    cap = VMAX_Z / math.sin(math.radians(slope))
    line(f"    slope {slope} deg: Z rate caps the path feed at {cap:5.0f} mm/min")
for feed in (2000, 3000):
    line(f"    accel distance to {feed}: {accel_dist(feed):.2f} mm (XY)")
assert curve_speed_mm_min(1) < 2100 and curve_speed_mm_min(5) > 4600
assert abs(stepover - 0.872) < 0.001

# tip speed
for n in (18000, 24000):
    line(f"  surface speed at De 1.73 mm, {n} rpm: {math.pi * deff * n / 1000:.0f} m/min")

# ramp of the rest op
rh = 0.3 * 2.0
ang = math.degrees(math.atan(2.0 / (2 * math.pi * rh)))
line(f"R1 rest helix: radius {rh:.2f} mm (0.3 x D), pitch 2.0 mm -> {ang:.1f} deg (not 3 deg)")
assert ang > 25
line("\nall asserts passed")
