#!/usr/bin/env python3
"""Arithmetic of entry_rules_2026-11-11.md. Each assert is one number the note states.

Run: python3 planning/feeds_contract_2026-11-03/research/sources_entry_rules/check_entry_rules.py
Inputs are code constants (named) or values printed in a stored source (named).
"""
from math import atan, asin, tan, sin, pi, degrees, radians

def close(a, b, tol):
    assert abs(a - b) <= tol, (a, b)

# --- code constants (crates/rs_cam_core/src/compute/config.rs, operation_configs.rs)
HELIX_RADIUS_OVER_D = 0.3
DRESSUP_HELIX_PITCH_MM = 1.0
ADAPTIVE3D_HELIX_PITCH_MM = 2.0

def helix_angle_deg(pitch, radius):
    """tan(theta) = pitch / (2 pi r): Tungaloy formula with (Dh - Dc) = 2 r."""
    return degrees(atan(pitch / (2 * pi * radius)))

print("1. Helix angle of today's code, r = 0.3 D")
print("   D mm | pitch 1 mm | pitch 2 mm")
rows = {}
for d in (0.5, 1.0, 3.175, 6.0, 12.0, 18.0):
    a1 = helix_angle_deg(DRESSUP_HELIX_PITCH_MM, HELIX_RADIUS_OVER_D * d)
    a2 = helix_angle_deg(ADAPTIVE3D_HELIX_PITCH_MM, HELIX_RADIUS_OVER_D * d)
    rows[d] = (a1, a2)
    print(f"   {d:5} | {a1:6.2f} deg | {a2:6.2f} deg")
close(rows[0.5][0], 46.70, 0.01); close(rows[0.5][1], 64.77, 0.01)
close(rows[1.0][0], 27.95, 0.01); close(rows[1.0][1], 46.70, 0.01)
close(rows[3.175][0], 9.49, 0.01); close(rows[3.175][1], 18.48, 0.01)
close(rows[6.0][0], 5.05, 0.01); close(rows[6.0][1], 10.03, 0.01)
close(rows[12.0][0], 2.53, 0.01); close(rows[12.0][1], 5.05, 0.01)
close(rows[18.0][0], 1.69, 0.01); close(rows[18.0][1], 3.37, 0.01)

print("2. Pitch that holds one helix angle, r = 0.3 D: pitch = 2 pi r tan(theta)")
def pitch_over_d(theta_deg, r_over_d=HELIX_RADIUS_OVER_D):
    return 2 * pi * r_over_d * tan(radians(theta_deg))
for th in (3, 5, 10, 20):
    print(f"   {th:2} deg: pitch = {pitch_over_d(th):.4f} x D")
close(pitch_over_d(3), 0.0988, 0.0001)
close(pitch_over_d(5), 0.1649, 0.0001)
close(pitch_over_d(10), 0.3324, 0.0001)
close(pitch_over_d(20), 0.6861, 0.0001)
close(pitch_over_d(5) * 0.5, 0.082, 0.001)   # 0.5 mm tool at 5 deg
close(pitch_over_d(5) * 18, 2.97, 0.01)      # 18 mm tool at 5 deg

print("3. Helix frames: hole diameter = 2 r + D; path diameter = 2 r")
close(2 * HELIX_RADIUS_OVER_D, 0.6, 1e-12)          # code path diameter / D
close(2 * HELIX_RADIUS_OVER_D + 1, 1.6, 1e-12)      # code hole diameter / D
close(0.95 / 2, 0.475, 1e-12)                       # Fusion default (snippet), flat tool: r / D
close(0.95 + 1, 1.95, 1e-12)                        # its hole / D
close(1.00 / 2, 0.5, 1e-12)                         # FreeCAD HelixMaxDiameterPercent = 100
close((1.10 - 1) / 2, 0.05, 1e-12)                  # Harvey ">110-120 %" read as the hole: r / D
close((1.20 - 1) / 2, 0.10, 1e-12)
# IMCO: hole = 2 D - 2 rc  ->  path = D - 2 rc  ->  r = D/2 - rc  (the flat-bottom cap, ruling Q6)

print("4. Vertical rate of a ramp at the full cut feed: F sin(theta). Amana rule: Ramp Down = F / Z")
for z in (1, 2, 3):
    print(f"   Z = {z}: F sin(theta) <= F / Z  up to theta = {degrees(asin(1 / z)):.2f} deg")
close(degrees(asin(1 / 2)), 30.0, 1e-9)
close(degrees(asin(1 / 3)), 19.47, 0.01)
for th, frac in ((3, 0.052), (5, 0.087), (10, 0.174), (12, 0.208), (20, 0.342), (27, 0.454)):
    close(sin(radians(th)), frac, 0.0005)

print("5. Ramp length for 6 mm of depth: L = depth / tan(theta)")
for th, length in ((3, 114.5), (5, 68.6), (10, 34.0), (12, 28.2), (20, 16.5), (27, 11.8)):
    close(6 / tan(radians(th)), length, 0.05)
    print(f"   {th:2} deg: {6 / tan(radians(th)):6.1f} mm")

print("6. IMCO: straight-line ramp angle = helical ramp angle x 5 (M233 chart footnote)")
assert [5 * a for a in (3.0, 2.5, 2.0)] == [15.0, 12.5, 10.0]
# IMCO drop table: drop per inch = tan(angle)
for th, drop in ((1, 0.0175), (2.5, 0.0438), (5, 0.0875), (10, 0.1750)):
    close(tan(radians(th)), drop, 0.0015)

print("7. Shaper table: the printed angle is not atan(plunge speed / auto speed)")
for name, plunge, auto, printed in (("hardwood", 250, 250, 20), ("softwood", 350, 250, 27),
                                    ("acrylic", 125, 250, 10), ("aluminium", 50, 100, 4)):
    a = degrees(atan(plunge / auto))
    print(f"   {name}: atan({plunge}/{auto}) = {a:.1f} deg, printed ~{printed} deg")
    assert abs(a - printed) > 15
close(degrees(atan(250 / 250)), 45.0, 1e-9)

print("8. Tungaloy worked example: hole 50, cutter 10, pitch 5")
close((50 - 10) * pi, 125.7, 0.05)
close(degrees(atan(5.0 / 125.7)), 2.3, 0.05)

print("9. Today's three ramp angles against the printed wood band (Easel 5 and 20, Shaper 20 and 27)")
assert 3.0 < 5 <= 10.0 <= 12.0 <= 20 <= 27
print("10. Ramp feed = min(F, vertical rate / tan(theta)) (RAMP_PLAN rule; rulings Q1, Q2), as a share of F")
def ramp_share(vertical_share, theta_deg):
    return min(1.0, vertical_share / tan(radians(theta_deg)))
close(degrees(atan(1 / 2)), 26.57, 0.01)      # Z = 2: the cut feed holds up to here
close(degrees(atan(1 / 3)), 18.43, 0.01)      # Z = 3
close(degrees(atan(0.5)), 26.57, 0.01)        # ball, tapered ball (0.50 F)
close(degrees(atan(1 / 3)), 18.43, 0.01)      # 60 deg V-bit (0.33 F)
assert ramp_share(1 / 2, 20) == 1.0 and ramp_share(1 / 2, 5) == 1.0
close(ramp_share(1 / 3, 20), 0.916, 0.001)    # 3 flutes or V-bit at 20 deg
assert ramp_share(1 / 3, 5) == 1.0 and ramp_share(1 / 3, 12) == 1.0
close(ramp_share(1 / 2, 27), 0.981, 0.001)    # 2 flutes at Shaper's 27 deg

print("11. Lower bounds of the helix radius, r / D")
close(0.10 / 2, 0.05, 1e-12)    # FreeCAD HelixMinDiameterPercent = 10 (a diameter)
close((1 / 8) / 2, 0.0625, 1e-12)  # FreeCAD Adaptive.cpp: toolDiameter / 8 (a diameter)
close(0.25 / 2, 0.125, 1e-12)   # Fusion default expression (snippet): tool_diameter * 0.25
assert 0.125 < HELIX_RADIUS_OVER_D < 0.475
print("all asserts pass")
