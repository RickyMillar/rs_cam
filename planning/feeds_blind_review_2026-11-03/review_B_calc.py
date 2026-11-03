#!/usr/bin/env python3
"""Reviewer B (V-bits and engraving): chip loads, geometry and limit checks.

Run: python3 -I review_B_calc.py
Every number in review_B.md that is a derivation comes from this script.
The script asserts each derived claim that review_B.md makes.
"""
import math

IN = 25.4  # mm per inch

# ---- Machine (brief 00 / shapeoko_pro_xxl_scylla.toml) ----
RPM_MIN, RPM_MAX = 6000.0, 24000.0
P_RATED_W, RPM_RATED = 1500.0, 24000.0
FEED_CAP = 6000.0          # rs_cam software cutting cap, mm/min
Z_MAX = 2500.0             # mm/min
A_XY, A_Z = 1200.0, 550.0  # mm/s^2


def power_w(n):
    return P_RATED_W * min(n, RPM_RATED) / RPM_RATED


# ---- Sources (vendor values in inch, converted here) ----
AMANA_1F_HW = (0.003 * IN, 0.007 * IN)      # 18/30/45 deg 1-flute, hardwood, 18000 rpm
AMANA_60_ENGR_HW = (0.004 * IN, 0.006 * IN)  # 60 deg engraving, hardwood, 18000 rpm
AMANA_2F_VGROOVE_HW = (0.003 * IN, 0.007 * IN)  # 15/60/90 deg 2-flute, hardwood
AMANA_2F_IPM = (50.0, 125.0)                # same chart, feed column, 18000 rpm
SPETOOL_ENGR_HW = 0.003 * IN                # tip 0.005 / 0.015 in, hardwood, 54 IPM, 18000
SHAW_FLOOR = 0.025                          # mm, rubbing floor (sources.toml shaw_metal_cutting)
VENDOR_RPM = 18000.0

assert abs(AMANA_1F_HW[0] - 0.0762) < 1e-9 and abs(AMANA_1F_HW[1] - 0.1778) < 1e-9
# Amana 2-flute chart: the IPM column and the chip column do not agree.
ipm_chip = [ipm / (VENDOR_RPM * 2) for ipm in AMANA_2F_IPM]  # in/tooth
assert ipm_chip[0] < 0.0015 and ipm_chip[1] < 0.0035  # 0.0014 .. 0.0035 in, under the 0.003-0.007 column
# Spetool row: 54 IPM / 18000 rpm / 0.003 in = 1 flute.
assert abs(54.0 / (VENDOR_RPM * 0.003) - 1.0) < 1e-9


# ---- Geometry ----
def width(d, tip, half_deg):
    return tip + 2.0 * d * math.tan(math.radians(half_deg))


def area(d, tip, half_deg):
    """V section area from the tip up to depth d (mm^2)."""
    return tip * d + d * d * math.tan(math.radians(half_deg))


def chip_normal(fz, half_deg):
    """Max uncut chip thickness normal to a V flank in a full slot."""
    return fz * math.cos(math.radians(half_deg))


def plunge_chip(fz_axial, half_deg):
    """Chip thickness normal to the flank when the V moves straight down."""
    return fz_axial * math.sin(math.radians(half_deg))


def thin_factor(e, d):
    """Radial chip thinning h_max / fz for a side shave of width e on diameter d."""
    r = e / d
    return 1.0 if r >= 0.5 else 2.0 * math.sqrt(r * (1.0 - r))


def accel_dist(f_mm_min, a):
    v = f_mm_min / 60.0
    return v * v / (2.0 * a)


# ---- V20, 0.3 mm flat, 3.175 body, 1 flute (brief 09) ----
V20 = dict(tip=0.3, half=10.0, body=3.175, z=1)
d_nom, d_max = 6.418, 7.65
w_nom = width(d_nom, V20["tip"], V20["half"])
assert abs(w_nom - 2.563) < 0.01
A20 = area(d_nom, V20["tip"], V20["half"])
assert abs(A20 - 9.19) < 0.01
# Depth relative to the nominal (body) diameter: 2 x D -> chart derate 25 % (Amana), 30 % (Spetool).
assert 2.0 < d_nom / V20["body"] < 2.1
# Chip thickness normal to the flank hardly depends on D_eff: cos(10 deg) = 0.985.
assert abs(chip_normal(1.0, 10.0) - 0.985) < 0.001
# Tip strength: bending stress at height z above the tip ~ z^2 / D(z)^3 for a uniform edge load.
# Maximum at z = 2a/b (a = tip flat, b = 2 tan(half)), where D = 3a.
a, b = V20["tip"], 2 * math.tan(math.radians(V20["half"]))
zs = [i / 1000 for i in range(1, 7650)]
zcrit = max(zs, key=lambda z: z * z / (a + b * z) ** 3)
assert abs(zcrit - 2 * a / b) < 0.002 and abs(zcrit - 1.70) < 0.01
assert abs(width(zcrit, a, V20["half"]) - 3 * a) < 0.002  # D_crit = 0.9 mm

# Effective diameters and surface speeds at 18000 rpm.
for d in (0.0, 2.0, 3.0, 5.0, d_nom):
    w = width(d, V20["tip"], V20["half"])
    vc = math.pi * w * 18000 / 1000.0  # m/min
    if d == 0.0:
        assert abs(vc - 16.96) < 0.05
    if d == d_nom:
        assert abs(vc - 144.9) < 0.5

# Chip load options (mm/tooth) and feeds at 18000 rpm, 1 flute.
v20_fz = {"conservative": 0.035, "centre": 0.045, "productive": 0.057}
assert abs(AMANA_1F_HW[0] * 0.75 - 0.0572) < 0.001  # chart low end after the 2 x D derate
for k, fz in v20_fz.items():
    assert fz >= SHAW_FLOOR
    f18 = 18000 * fz
    assert f18 <= FEED_CAP
v20_feed = {k: round(18000 * fz) for k, fz in v20_fz.items()}
assert v20_feed == {"conservative": 630, "centre": 810, "productive": 1026}
# Margin against a 50 % feed loss on Z-following paths (grblHAL planner slows at 0.25 mm vertices).
assert v20_fz["conservative"] * 0.5 < SHAW_FLOOR < v20_fz["productive"] * 0.5 + 0.01
# Z-following: slope at which Z hits its 2500 mm/min cap.
for k, f in v20_feed.items():
    slope = Z_MAX / f
    assert slope > 2.4  # > 67 deg at 1026 mm/min
# Power: specific energy needed to reach the available power at 18000 rpm, centre feed.
mrr = A20 * v20_feed["productive"] / 60.0  # mm^3/s
u_break = power_w(18000) / mrr  # J/mm^3
assert mrr < 160 and u_break > 7.0

# Pass split options for the V20 (depth below the web top).
def eq_area_split(total_d, tip, half, n):
    tot = area(total_d, tip, half)
    out = []
    for i in range(1, n + 1):
        target = tot * i / n
        lo, hi = 0.0, total_d
        for _ in range(60):
            mid = (lo + hi) / 2
            lo, hi = (mid, hi) if area(mid, tip, half) < target else (lo, mid)
        out.append(round(hi, 2))
    return out

v20_eq2 = eq_area_split(d_nom, 0.3, 10.0, 2)
assert v20_eq2 == [4.32, 6.42]
p1 = area(3.21, 0.3, 10.0)
assert abs(p1 - 2.78) < 0.01 and abs((A20 - p1) - 6.41) < 0.01
# Finish-pass option: rough to d_nom - 1.0, finish 1.0 mm. Wall shave per side = 1.0 tan 10.
shave = 1.0 * math.tan(math.radians(10.0))
assert abs(shave - 0.176) < 0.001
tf = thin_factor(shave, w_nom)
assert 0.45 < tf < 0.55  # chip thins to about half on the upper flanks

# Plunge chip for the V20 (axial feed, flank chip = fz_ax sin 10 deg).
for fz_plunge in (300.0, 1000.0):
    h = plunge_chip(fz_plunge / 18000.0, 10.0)
    assert h < SHAW_FLOOR
need_ax = SHAW_FLOOR / math.sin(math.radians(10.0))
assert 18000 * need_ax > Z_MAX  # no Z feed reaches the floor at 18000 rpm, 1 flute
assert 6000 * need_ax < Z_MAX   # at 6000 rpm, 864 mm/min reaches it
assert abs(6000 * need_ax - 864) < 1

# ---- V60, sharp, 6.35 body, 2 flutes (briefs 10 and 12) ----
V60 = dict(tip=0.0, half=30.0, body=6.35, z=2)
cone = V60["body"] / 2 / math.tan(math.radians(30.0))
assert abs(cone - 5.50) < 0.01
assert 5.27 < cone and 5.27 / V60["body"] < 1.0  # sim peak bite is inside the cone and under 1 x D
A_p1 = area(2.598, 0, 30.0)
A_tot = area(5.196, 0, 30.0)
assert abs(A_p1 - 3.90) < 0.01 and abs(A_tot - 15.59) < 0.01
assert abs((A_tot - A_p1) / A_p1 - 3.0) < 0.01  # pass 2 removes 3 x pass 1
v60_eq2 = eq_area_split(5.196, 0, 30.0, 2)
assert v60_eq2 == [3.67, 5.2]
v60_eq3 = eq_area_split(5.196, 0, 30.0, 3)
assert v60_eq3 == [3.0, 4.24, 5.2]
# Light-exit option 3.4 / 4.8 / 5.196: last pass area.
last = A_tot - area(4.8, 0, 30.0)
assert abs(last - 2.29) < 0.01
# Front-face line width: the tip passes Z0 by 0.366 mm.
assert abs(width(0.366, 0, 30.0) - 0.4226) < 0.001
v60_fz = {"conservative": 0.05, "centre": 0.064, "productive": 0.076}
v60_feed = {k: round(18000 * 2 * fz) for k, fz in v60_fz.items()}
assert v60_feed == {"conservative": 1800, "centre": 2304, "productive": 2736}
for f in v60_feed.values():
    assert f < FEED_CAP
mrr60 = (A_tot - A_p1) * v60_feed["productive"] / 60.0
assert mrr60 < 540 and power_w(18000) / mrr60 > 2.0
# Flank chip in pass 2: wall shave 2.598 tan 30 = 1.5 mm per side on D up to 6 mm.
assert abs(thin_factor(2.598 * math.tan(math.radians(30)), 6.0) - 0.866) < 0.01
# Plunge (Amana: ramp down = feed / flutes).
assert v60_feed["conservative"] / 2 == 900

# Engraving at 0.6 mm (brief 12)
w06 = width(0.6, 0, 30.0)
assert abs(w06 - 0.693) < 0.001
assert chip_normal(0.05, 30.0) > SHAW_FLOOR
eng_feed = (1200.0, 2000.0)
for f in eng_feed:
    assert accel_dist(f, A_XY) < 0.5   # feed is reached inside 0.5 mm
assert abs(accel_dist(1500, A_XY) - 0.26) < 0.01
# Plunge 0.6 mm at 750 mm/min.
assert abs(0.6 / 750 * 60 - 0.048) < 1e-3
assert abs(accel_dist(750, A_Z) - 0.142) < 0.001

# Power curve sanity
assert power_w(16000) == 1000.0 and power_w(18000) == 1125.0

print("V20: width at 6.42 mm = %.2f mm, area = %.2f mm^2, critical section z = %.2f mm (D = %.2f mm)"
      % (w_nom, A20, zcrit, width(zcrit, a, 10.0)))
print("V20 feeds at 18000 rpm, 1 flute:", v20_feed, "u_break = %.1f J/mm^3" % u_break)
print("V20 eq-area 2-pass:", v20_eq2, "1 mm finish shave thin factor = %.2f" % tf)
print("V20 plunge: fz_axial for floor = %.3f mm -> %.0f mm/min at 18000 rpm" % (need_ax, 18000 * need_ax))
print("V60 slit: areas p1 %.2f, total %.2f; eq2 %s eq3 %s; feeds %s; u_break %.1f J/mm^3"
      % (A_p1, A_tot, v60_eq2, v60_eq3, v60_feed, power_w(18000) / mrr60))
print("Engrave: width %.3f mm; accel dist 1500 mm/min = %.2f mm" % (w06, accel_dist(1500, A_XY)))
print("ALL ASSERTIONS PASS")
