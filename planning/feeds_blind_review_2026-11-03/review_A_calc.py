#!/usr/bin/env python3
"""Blind reviewer A (roughing and slotting): chip load, torque, power and entry checks.

Run: python3 -I review_A_calc.py
Every number in review_A.md that is a derivation comes from this script.
The script asserts each limit claim that review_A.md makes.

Inputs and their sources:
- Machine: briefs/00_machine_and_material.md and
  ~/.config/rs_cam/machines/shapeoko_pro_xxl_scylla.toml (rpm 6000-24000, VFD constant torque,
  1.5 kW at 24000 rpm, XY accel 1200 mm/s^2, Z max 2500 mm/min, cutting-feed cap 6000 mm/min).
- Specific cutting energy u for hardwood: Oliveira Guedes et al. 2020, Maderas Cienc. Tecnol.
  22(4):477-482, https://www.redalyc.org/journal/485/48565177006/48565177006.pdf
  (6 mm cutter on a 1300 W CNC router, 6 mm x 3 mm cut, 0.70 m/min; Goupia glabra 717 kg/m3:
  261 (saturated) to 513 (anhydrous); Dipteryx alata 914 kg/m3: up to 570). The paper prints
  "kJ/cm3"; check_unit_label() shows that the unit must be J/cm3.
  Design value U_DESIGN = 0.50 J/mm3 (high end, conservative); U_LOW = 0.26 J/mm3 (low end).
- Chip load per tooth (vendor): Onsrud Hard Wood chart p.115, https://www.onsrud.com/images/Hard%20Wood.pdf
  (series 52-200/57-200, the chart's own GOOD roughing choice);
  Freud CNC chart 2017-08-22,
  https://www.freudtools.com/public/assets/freud/downloadables/freudtools-router-bit-feed-and-speed-for-cnc-20170822.pdf
- Hobby chip load and plunge ratio: Shapeoko CNC A to Z (sources.toml key shapeoko_wiki).
- Rubbing floor 0.025 mm: Shaw, Metal Cutting Principles (sources.toml key shaw_metal_cutting).
- Radial chip thinning: DAPRA (sources.toml key dapra_rctf); hex = fz * 2*sqrt(r - r^2), r = ae/D < 0.5.
- Plunge axial chip: Amana Spektra spiral plunge v24, Ramp Down 2F 6 mm wood 90 in/min at 18000 rpm
  (sources.toml key amana_spektra_plunge_v24).
"""

import math

IN = 25.4

# ---------------------------------------------------------------- machine (00 brief)
RPM_MIN, RPM_MAX = 6000.0, 24000.0
P_RATED_W, N_RATED = 1500.0, 24000.0
FEED_CAP = 6000.0  # rs_cam software cap, mm/min
Z_MAX = 2500.0  # mm/min
A_XY = 1200.0  # mm/s^2
T_RATED_NMM = P_RATED_W / (2 * math.pi * N_RATED / 60.0) * 1000.0  # N*mm
MARGIN = 0.80  # judgement: 20 % reserve for engagement spikes, dull edges, hard grain

U_DESIGN = 0.50  # J/mm^3 (= 500 N/mm^2)
U_LOW = 0.26  # J/mm^3

RUB_FLOOR = 0.025  # mm/tooth, Shaw


def check_unit_label():
    """Guedes 2020: 0.70 m/min x 6 mm x 3 mm cut; 1300 W router."""
    mrr_cm3_s = 700.0 / 60.0 * 6.0 * 3.0 / 1000.0
    p_if_j = 513.0 * mrr_cm3_s  # W, if the unit is J/cm3
    p_if_kj = 513.0e3 * mrr_cm3_s  # W, if the unit is kJ/cm3
    assert p_if_j < 1300.0 < p_if_kj, "unit check"
    return mrr_cm3_s, p_if_j, p_if_kj


def p_avail(n):
    return P_RATED_W * min(n, N_RATED) / N_RATED


def thin(ae, d):
    r = ae / d
    if r >= 0.5:
        return 1.0
    return 2.0 * math.sqrt(r - r * r)


def torque_nmm(u, ap, ae, fz, z):
    # energy per rev = u[N/mm^2] * ap*ae*fz*z [mm^3]; torque = energy / (2 pi)
    return u * 1000.0 * ap * ae * fz * z / (2 * math.pi)


def fz_max_torque(u, ap, ae, z, frac=MARGIN):
    return frac * T_RATED_NMM * 2 * math.pi / (u * 1000.0 * ap * ae * z)


def power_w(u, ap, ae, vf):
    return u * ap * ae * vf / 60.0


def v_arc(r_mm):
    """Max steady feed on an arc of radius r at the XY acceleration (mm/min)."""
    return math.sqrt(A_XY * r_mm) * 60.0


def point(name, d, z, n, fz, ap, ae, u=U_DESIGN):
    vf = fz * z * n
    hex_ = fz * thin(ae, d)
    t = torque_nmm(u, ap, ae, fz, z)
    p = power_w(u, ap, ae, vf)
    row = dict(name=name, d=d, n=n, fz=fz, hex=hex_, vf=vf, ap=ap, ae=ae,
               torque_pct=100 * t / T_RATED_NMM, p_w=p, p_avail=p_avail(n),
               p_low=power_w(U_LOW, ap, ae, vf))
    assert RPM_MIN <= n <= RPM_MAX, name
    return row


def show(rows):
    print(f"{'point':44s} {'rpm':>6s} {'fz':>6s} {'hex':>6s} {'vf':>6s} {'ap':>5s} {'ae':>5s} "
          f"{'T%':>5s} {'P(u.5)':>7s} {'P(u.26)':>7s} {'Pav':>6s} cap?")
    for r in rows:
        cap = "OVER CAP" if r["vf"] > FEED_CAP + 0.5 else ""
        print(f"{r['name']:44s} {r['n']:6.0f} {r['fz']:6.3f} {r['hex']:6.3f} {r['vf']:6.0f} "
              f"{r['ap']:5.2f} {r['ae']:5.2f} {r['torque_pct']:5.0f} {r['p_w']:7.0f} {r['p_low']:7.0f} "
              f"{r['p_avail']:6.0f} {cap}")


# ---------------------------------------------------------------- entry geometry (18 mm, no centre cut)
def ramp_max_deg(protrusion, core):
    return math.degrees(math.atan(protrusion / core))


def helix_descent_under_core(r_helix, core_r, pitch, steps=2000):
    """Max descent while a fixed floor point stays under the non-cutting core during a helix.
    The tool centre runs on a circle of radius r_helix; a point P at radius rho is under the core
    while |C(phi) - P| < core_r. Descent = pitch * (angular span / 2 pi). Worst case over rho."""
    worst = 0.0
    for i in range(1, steps):
        rho = (r_helix + core_r) * i / steps
        # |C-P|^2 = rh^2 + rho^2 - 2 rh rho cos(d) < core_r^2
        c = (r_helix ** 2 + rho ** 2 - core_r ** 2) / (2 * r_helix * rho)
        if c >= 1.0:
            continue
        span = 2 * math.pi if c <= -1.0 else 2 * math.acos(c)
        worst = max(worst, pitch * span / (2 * math.pi))
    return worst


# ---------------------------------------------------------------- deflection (judgement inputs)
E_CARBIDE = 600e3  # N/mm^2, judgement (carbide 550-650 GPa)
E_STEEL = 210e3  # N/mm^2, judgement
FLUTE_EFF = 0.8  # effective diameter of a 2-flute section, judgement


def cantilever_defl(f_n, lever, d_eff, e):
    i = math.pi * d_eff ** 4 / 64.0
    return f_n * lever ** 3 / (3 * e * i)


def bend_stress(f_n, lever, d):
    return 32 * f_n * lever / (math.pi * d ** 3)


def main():
    print("== constants")
    print(f"rated torque {T_RATED_NMM:.1f} N*mm; margin {MARGIN}")
    mrr, pj, pkj = check_unit_label()
    print(f"Guedes unit check: MRR {mrr:.3f} cm3/s -> {pj:.0f} W if J/cm3, {pkj/1000:.0f} kW if kJ/cm3")

    print("\n== vendor chip loads (mm/tooth)")
    vend = {
        "Onsrud 52-200 7/32in": (.004, .006), "Onsrud 52-200 1/4in": (.005, .007),
        "Onsrud 52-200 5/8in": (.008, .010), "Onsrud 52-200 3/4in": (.009, .011),
        "Freud SC 1/4in hardwood": (.008, .011),
        "Freud CT 5/8in hardwood": (.008, .012), "Freud CT 3/4in hardwood": (.009, .013),
        "Shapeoko A-Z 1/4in hard wood": (0.025 / IN, 0.05 / IN),
        "Amana Spektra 6mm 2F wood (single)": (.005, .005),
    }
    for k, (a, b) in vend.items():
        print(f"{k:38s} {a*IN:.3f}-{b*IN:.3f}")

    print("\n== chip thinning factors")
    for ae, d in [(3.6, 18), (2.7, 18), (1.8, 18), (1.2, 6), (1.382, 6), (2.4, 6), (1.8, 6),
                  (0.3, 6), (0.5, 6), (14.4, 18)]:
        print(f"ae {ae:5.2f} D {d:2.0f}: r {ae/d:.3f} factor {thin(ae, d):.3f}")

    rows = []
    # 01 face 18 mm: ap 0.5, ae 14.4
    rows += [point("01 face cons", 18, 2, 16000, 0.14, 0.5, 14.4),
             point("01 face prod (cap)", 18, 2, 18000, 6000 / 36000, 0.5, 14.4)]
    # 02 back bulk
    rows += [point("02 PLANNED 9x3.6 @cap 18000", 18, 2, 18000, 6000 / 36000, 9, 3.6),
             point("02 PLANNED 9x3.6 @cap 24000", 18, 2, 24000, 6000 / 48000, 9, 3.6),
             point("02 A 9x3.6 cons", 18, 2, 20000, 0.08, 9, 3.6),
             point("02 A 9x3.6 prod", 18, 2, 24000, 0.092, 9, 3.6),
             point("02 B 9x2.7 cons", 18, 2, 20000, 0.10, 9, 2.7),
             point("02 B 9x2.7 prod (cap)", 18, 2, 24000, 0.125, 9, 2.7),
             point("02 C 4.5x3.6 prod (cap)", 18, 2, 18000, 6000 / 36000, 4.5, 3.6),
             point("02 D 9x1.8 prod (cap)", 18, 2, 24000, 0.125, 9, 1.8),
             point("02 level2 5.5x3.6 prod (cap)", 18, 2, 20000, 0.15, 5.5, 3.6)]
    # 03 back rest 6 mm
    rows += [point("03 1.2x1.2 cons", 6, 2, 18000, 0.0625, 1.2, 1.2),
             point("03 1.2x1.2 prod", 6, 2, 20000, 0.10, 1.2, 1.2),
             point("03 entry 14.5 full slot @ cons fz", 6, 2, 18000, 0.0625, 14.5, 6),
             point("03 entry 14.5 full slot @ fz limit", 6, 2, 18000,
                   fz_max_torque(U_DESIGN, 14.5, 6, 2), 14.5, 6)]
    # 04/05 plunges: axial chip; power from MRR = pi r^2 vf
    # 06 river channel
    fz10 = fz_max_torque(U_DESIGN, 10.02, 6, 2)
    rows += [point("06 one pass 10.02 slot cons", 6, 2, 18000, 0.035, 10.02, 6),
             point("06 one pass 10.02 slot prod", 6, 2, 18000, 0.045, 10.02, 6),
             point("06 two levels 5.0 slot cons", 6, 2, 18000, 0.06, 5.0, 6),
             point("06 two levels 5.0 slot prod", 6, 2, 18000, 0.08, 5.0, 6),
             point("06 shallow 2.5 slot prod", 6, 2, 18000, 0.08, 2.5, 6)]
    # 11 clamp pads
    rows += [point("11 pads cons", 6, 2, 18000, 0.09, 0.5, 2.4),
             point("11 pads prod", 6, 2, 20000, 0.10, 0.5, 2.4)]
    # 13 front bulk 18
    rows += [point("13 4.17x3.6 cons", 18, 2, 18000, 0.125, 4.1667, 3.6),
             point("13 4.17x3.6 prod (cap)", 18, 2, 18000, 6000 / 36000, 4.1667, 3.6)]
    # 14 front rough 6
    rows += [point("14 5.53x1.38 cons", 6, 2, 18000, 0.06, 5.532, 1.382),
             point("14 5.53x1.38 prod", 6, 2, 20000, 0.095, 5.532, 1.382),
             point("14 alt 2.77x1.8 cons", 6, 2, 18000, 0.06, 2.766, 1.8),
             point("14 alt 2.77x1.8 prod", 6, 2, 20000, 0.085, 2.766, 1.8),
             point("14 entry 4.62 slot @ cons", 6, 2, 18000, 0.06, 4.62, 6),
             point("14 entry 4.62 slot @ prod", 6, 2, 20000, 0.095, 4.62, 6)]
    # 17 outline
    rows += [point("17 1.5 slot cons", 6, 2, 18000, 0.08, 1.5, 6),
             point("17 1.5 slot prod", 6, 2, 20000, 0.10, 1.5, 6),
             point("17 3.0 slot cons", 6, 2, 18000, 0.06, 3.0, 6),
             point("17 3.0 slot prod", 6, 2, 18000, 0.075, 3.0, 6),
             point("17 6.0 slot (1xD) cons", 6, 2, 18000, 0.045, 6.0, 6),
             point("17 finish 25 deep ae 0.3", 6, 2, 18000, 0.115, 25.0, 0.3)]
    print("\n== operating points (u design 0.50 J/mm3; T% = share of rated torque)")
    show(rows)

    by = {r["name"]: r for r in rows}
    # ---- claims asserted (review_A.md quotes these)
    # C1: planned 02 at the 6000 cap needs more than the rated torque (u=0.5) at any rpm in range.
    assert by["02 PLANNED 9x3.6 @cap 24000"]["torque_pct"] > 100
    assert by["02 PLANNED 9x3.6 @cap 18000"]["p_w"] > by["02 PLANNED 9x3.6 @cap 18000"]["p_avail"]
    # C1b: with the low u it fits under the rated power at 24000 but not with the 80 % margin at 18000.
    assert by["02 PLANNED 9x3.6 @cap 24000"]["p_low"] < 1500
    # C2: max fz at 9 x 3.6 under the 80 % torque margin, and the feed it gives at 24000.
    fz93 = fz_max_torque(U_DESIGN, 9, 3.6, 2)
    print(f"\n02: fz max (80 % torque, u .5) at 9x3.6 = {fz93:.4f} mm; vf at 24000 = {fz93*2*24000:.0f}; "
          f"at 18000 = {fz93*2*18000:.0f}")
    print(f"02: fz max at 100 % torque = {fz_max_torque(U_DESIGN, 9, 3.6, 2, 1.0):.4f}; "
          f"u that allows the cap at 24000 (100 %): "
          f"{U_DESIGN * 100 / by['02 PLANNED 9x3.6 @cap 24000']['torque_pct']:.3f} J/mm3")
    assert 0.09 < fz93 < 0.095
    # C3: the productive pairs stay at or under ~81 % of rated torque.
    for k in ["02 A 9x3.6 prod", "02 B 9x2.7 prod (cap)", "02 C 4.5x3.6 prod (cap)",
              "02 D 9x1.8 prod (cap)", "13 4.17x3.6 prod (cap)"]:
        assert by[k]["torque_pct"] <= 82, k
    # C4: MRR comparison (mm^3/min)
    for k in ["02 A 9x3.6 prod", "02 B 9x2.7 prod (cap)", "02 C 4.5x3.6 prod (cap)",
              "02 D 9x1.8 prod (cap)"]:
        r = by[k]
        print(f"{k:30s} MRR {r['ap']*r['ae']*r['vf']/1000:.0f} cm3/min")
    # C5: river channel one pass fz limit
    print(f"06: fz max one pass 10.02 mm slot (80 %) = {fz10:.4f}; at 100 % = "
          f"{fz_max_torque(U_DESIGN, 10.02, 6, 2, 1.0):.4f}")
    assert 0.045 < fz10 < 0.055
    assert by["06 one pass 10.02 slot prod"]["torque_pct"] <= 80
    # C6: 14.5 mm entry bite limits fz near the rubbing floor
    fz145 = fz_max_torque(U_DESIGN, 14.5, 6, 2)
    print(f"03: fz max at 14.5 mm full slot (80 %) = {fz145:.4f}; at the cons fz the torque is "
          f"{by['03 entry 14.5 full slot @ cons fz']['torque_pct']:.0f} %")
    assert fz145 < 0.04 and by["03 entry 14.5 full slot @ cons fz"]["torque_pct"] > 100
    # C7: chip above rubbing floor at every chosen (non-limit) point
    for r in rows:
        if "fz limit" in r["name"] or "PLANNED" in r["name"]:
            continue
        assert r["hex"] >= RUB_FLOOR, r["name"]
        assert r["vf"] <= FEED_CAP + 0.5, r["name"]
    # depth rule (Freud/Onsrud): 2xD -> -25 %, 3xD -> -50 %, applied to the 06 one-pass case
    print("06: depth ratio", round(10.02 / 6, 2), "-> Freud/Onsrud reduction about",
          f"{25 * (10.02/6 - 1):.0f} % (linear between 1xD and 2xD, judgement)")

    print("\n== arc feed limits from XY acceleration (steady arc, v^2/R <= a)")
    for rr in [1, 2, 3, 5, 8.33, 10]:
        print(f"R {rr:5.2f} mm -> {v_arc(rr):6.0f} mm/min")
    assert abs(v_arc(8.333) - 6000) < 5
    print(f"row-end stop distance at 6000 mm/min: {(100.0**2)/(2*A_XY):.2f} mm")

    print("\n== 18 mm non-centre-cutting entry")
    for p, c in [(1.0, 8.0), (0.5, 8.0), (1.0, 10.0), (0.5, 10.0), (0.42, 8.0)]:
        print(f"protrusion {p} core {c}: ramp max {ramp_max_deg(p, c):.2f} deg")
    assert abs(ramp_max_deg(1, 8) - 7.125) < 0.01
    assert ramp_max_deg(0.5, 10) < 3.0  # 3 deg is NOT safe for every plausible estimate
    p_need_3deg = 8.0 * math.tan(math.radians(3))
    print(f"3 deg is safe only if protrusion >= {p_need_3deg:.2f} mm (core 8) / "
          f"{10*math.tan(math.radians(3)):.2f} mm (core 10)")
    for pitch in [1.0, 2.0]:
        w = helix_descent_under_core(0.3 * 18, 4.0, pitch)
        print(f"helix r 5.4, core r 4, pitch {pitch}: max descent under core {w:.3f} mm")
    w2 = helix_descent_under_core(5.4, 4.0, 2.0)
    w2b = helix_descent_under_core(5.4, 5.0, 2.0)
    print(f"helix pitch 2, core r 5 (10 mm core): {w2b:.3f} mm")
    assert w2 < 1.0
    for core_r, prot in [(4.0, 1.0), (4.0, 0.5), (5.0, 0.5)]:
        unit = helix_descent_under_core(5.4, core_r, 1.0)
        print(f"core r {core_r} protrusion {prot}: helix pitch max {prot/unit:.2f} mm/rev")
    print(f"helix angle at r 5.4 pitch 2: {math.degrees(math.atan(2/(2*math.pi*5.4))):.2f} deg")
    print(f"ramp leading-edge depth over one D at 3 deg: {18*math.tan(math.radians(3)):.2f} mm")

    print("\n== shank / tool deflection (judgement inputs)")
    # 18 mm: torque at 80 % -> tangential force at r 9; assume resultant 1.4 x tangential (judgement)
    ft = MARGIN * T_RATED_NMM / 9.0
    f = 1.4 * ft
    lever = 25.0 - 4.5
    s = bend_stress(f, lever, 6.0)
    print(f"18 mm: Ft {ft:.0f} N, F {f:.0f} N, lever {lever} mm, steel 6 mm shank stress {s:.0f} MPa")
    # 6 mm: worst cases
    for label, tpct, lever6 in [("03 entry 14.5 at 80 % torque", 80, 30 - 7.25),
                                ("06 one pass 10 at 80 %", 80, 30 - 5.0),
                                ("17 6.0 slot cons", by["17 6.0 slot (1xD) cons"]["torque_pct"], 30 - 3.0),
                                ("14 prod", by["14 5.53x1.38 prod"]["torque_pct"], 30 - 2.77)]:
        ft6 = tpct / 100 * T_RATED_NMM / 3.0
        f6 = 1.4 * ft6
        dfl = cantilever_defl(f6, lever6, FLUTE_EFF * 6, E_CARBIDE)
        st = bend_stress(f6, lever6, FLUTE_EFF * 6)
        print(f"6 mm {label:30s}: Ft {ft6:5.0f} N F {f6:5.0f} N defl {dfl:.3f} mm stress {st:4.0f} MPa")

    print("\n== plunges (6 mm 2F, assumed centre cutting)")
    amana_fz_ax = 90 * IN / (18000 * 2)
    print(f"Amana ramp-down 2F 6 mm wood: 90 in/min at 18000 -> axial chip {amana_fz_ax:.4f} mm/tooth")
    for n, fza in [(18000, 0.02), (18000, 0.03), (16000, 0.025)]:
        vz = fza * 2 * n
        mrr = math.pi * 9.0 * vz / 60.0
        print(f"plunge rpm {n} axial fz {fza}: {vz:.0f} mm/min (Z cap {Z_MAX:.0f}); "
              f"power {U_DESIGN*mrr:.0f} W of {p_avail(n):.0f}")
        assert vz <= Z_MAX
    print("Shapeoko A-Z plunge 30-40 % of feed: of 2160-3600 ->",
          f"{0.3*2160:.0f}-{0.4*3600:.0f} mm/min")
    dwell_revs = 0.5 * 18000 / 60
    print(f"dwell 0.5 s at 18000 rpm = {dwell_revs:.0f} rev with no feed")

    print("\nall asserts passed")


if __name__ == "__main__":
    main()
