"""Assert the arithmetic of 01_op1_op14_bulk_18mm.md (run: python3 -I check_numbers.py).

Inputs are printed in the repo:
- Ks 58.5 N/mm^2, F_edge 4.59 N/mm: white oak line (compare.md energy check (d),
  crates/rs_cam_core/src/material/force_line.rs).
- Power model: crates/rs_cam_core/src/tool_load/power.rs module doc.
- 0.1448 mm/tooth, 12.7 mm, 18000 rpm: Amana row amana-flat-hardwood-pocket-12700-2f-spektra
  (crates/rs_cam_core/data/vendor_lut/observations/amana_flat_end.json).
- Size exponent 0.61, hardness exponent 0.5: feeds/vendor_lookup.rs:537, :573.
- Janka 1450 (generic hardwood proxy of the row), 1360 (WhiteOak, material/mod.rs:153).
- Machine: nz_wanaka_21_operator.toml (1.5 kW at 24000 rpm, constant torque; aggressiveness 0.85;
  rigidity adaptive_doc_factor 2.0), cutting cap 6000 (machine/mod.rs:628).
"""
import math

D, Z = 18.0, 2
KS, FE = 58.5, 4.59

# (c) the chip
fz = 0.1448 * (D / 12.7) ** 0.61 * (1450 / 1360) ** 0.5
assert abs(fz - 0.18496) < 1e-4, fz

# (b) the RPM descent at the 6000 cap
assert 18000 * fz * Z > 6000
rpm = math.floor(6000 / (fz * Z))
assert rpm == 16219, rpm
avail = 1.5 * rpm / 24000
assert abs(avail - 1.0137) < 1e-3
feed = 5999.0


def power_kw(ap, ae, feed=feed, rpm=rpm):
    """Two-term power (tool_load/power.rs): A(Ks MRR + F_edge ap Vc z psi / 2pi) / 6e7."""
    psi = math.acos(1 - 2 * ae / D) if ae < D else math.pi
    vc = math.pi * D * rpm
    return (KS * ap * ae * feed + FE * ap * vc * Z * psi / (2 * math.pi)) / 60e6


# Suggest's own figures are reproduced
assert abs(power_kw(16.0, 3.6) - 0.6688) < 2e-3  # calculator power_kw 0.6688
assert abs(power_kw(14.047, 3.16) - 0.5313) < 2e-3  # shipped_power 0.5313
nominal_9 = power_kw(9.0, 3.6)
assert abs(nominal_9 - 0.376) < 2e-3

# The 1.18 kW sim peak needs near-slot engagement
ae_at_peak = next(a / 10 for a in range(36, 181) if power_kw(9.0, a / 10) >= 1.18)
assert 14.0 <= ae_at_peak <= 14.3, ae_at_peak
slot_depth = next(a / 10 for a in range(10, 200) if power_kw(a / 10, 18.0) >= 1.18)
assert abs(slot_depth - 6.8) < 0.05, slot_depth
assert power_kw(9.0, 18.0) > 1.5

# (a) the depth chain
ap_req = 2.0 * D  # rigidity adaptive_doc_factor 2.0 x 1.5 / 1.5
assert ap_req == 36.0
ap_guard = 0.8 * 20.0
assert ap_guard == 16.0
scale = 14.047 / 16.0
assert abs(scale - 3.16 / 3.6) < 2e-3  # one common scale on both levers

# (e) the face: 0.7 D x 0.35 D, depth snap on a 0.5 mm total
assert abs(0.7 * D - 12.6) < 1e-9 and abs(0.35 * D - 6.3) < 1e-9
s_face = 5.487 / 6.3
assert 0.5 * s_face < 0.5  # any scale below 1 asks for a step under the total
assert 0.5 / math.ceil(0.5 / (0.5 * s_face)) == 0.25

print("all assertions hold")
print(f"fz {fz:.5f}  rpm {rpm}  avail {avail:.4f} kW")
print(f"P 9x3.6 {nominal_9:.3f}  P 14.047x3.16 {power_kw(14.047, 3.16):.3f}  P 16x3.6 {power_kw(16, 3.6):.3f}")
print(f"ae for 1.18 kW at ap 9: {ae_at_peak} mm; full-slot depth for 1.18 kW: {slot_depth} mm")
