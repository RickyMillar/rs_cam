#!/usr/bin/env python3
"""Independent acceptance table for the wanaka 21 feeds and speeds.

The script computes every expected value from printed source lines and
physics. It does not import or read the engine. The app values below are
copies of MCP read-tool output (get_toolpath_params, get_suggest_rationale,
build b9e67b24e (P20e)). Re-grade a new build: replace the APP dict, run the script.

Output: wanaka_21.toml, and wanaka_21.md = wanaka_21_header.md + the row tables.
"""
import itertools
import math
import pathlib

HERE = pathlib.Path(__file__).resolve().parent
IN = 25.4  # mm per inch (definition)
TOL = 0.01  # 1 % grading tolerance

# ---------------------------------------------------------------- sources
VL = "crates/rs_cam_core/data/vendor_lut/sources/"
SRC = {
    "SPK3": (VL + "amana_spektra_spiral_plunge_v24.txt", 3,
             "CNC Operating Spindle Speed: 18,000 RPM / Depth of Cut: 1 x Tool Diameter"),
    "SPK12": (VL + "amana_spektra_spiral_plunge_v24.txt", 12,
              '46237-K ** 1/16"  70"  .0020"  35"'),
    "SPK17": (VL + "amana_spektra_spiral_plunge_v24.txt", 17,
              '46239-K ** 3/32"  80"  .0023"  40"'),
    "SPK26": (VL + "amana_spektra_spiral_plunge_v24.txt", 26,
              '48118-K 48218-K 6mm  180"  .0050"  90"'),
    "SPK37": (VL + "amana_spektra_spiral_plunge_v24.txt", 37,
              '46106-K 46206-K 1/2"  200"  .0057"  100"'),
    "SPK38": (VL + "amana_spektra_spiral_plunge_v24.txt", 37,
              "1 x D Use recommended feed rate / 2 x D Reduce feed rate by 25% / 3 x D Reduce feed rate by 50% (lines 37-39)"),
    "SPK47": (VL + "amana_spektra_spiral_plunge_v24.txt", 47,
              "To find Feed Rate IPM: RPM x # of flutes x chip load (lines 47-48)"),
    "SPK52": (VL + "amana_spektra_spiral_plunge_v24.txt", 52,
              "To find Ramp Down: Feed Rate IPM / # of flutes (lines 52-53)"),
    "SPK54": (VL + "amana_spektra_spiral_plunge_v24.txt", 54,
              '3 Flute: 46500-K 3/4"  330"  .009"  110"'),
    "ONS46": (VL + "onsrud_hard_wood_cutting_data.txt", 46,
              "60-200  1xD  .005-.007 (first cell; column 1/4 by text position)"),
    "ONS61": (VL + "onsrud_hard_wood_cutting_data.txt", 61,
              "* = 16,000 RPM / ** = 15,000 RPM (lines 61-62; markers only on series 37-80)"),
    "ZRN22": (VL + "amana_zrn_3d_v8.txt", 22,
              'Wood, MDF, Sign-Foam ... 6mm (0.2362"): 250" - 320"  0.007" - 0.009" (2 Flute Flat Bottom)'),
    "AMS3": (VL + "amana_ams159_vgroove_v2.txt", 3,
             "Operating RPM: 18,000 / Depth of Cut: 1 x Tool Diameter"),
    "AMS11": (VL + "amana_ams159_vgroove_v2.txt", 11,
              'Hard Wood  18deg 1F: 50" - 130"  0.003" - 0.007" | 30deg 1F: 50" - 130"  0.003" - 0.007" | 60deg 2F: 90"  0.003"'),
    "SPE23": (VL + "spetool_2d3d_tapered.txt", 23,
              "2.0 | Wood, MDF, Sign-Foam | 18,000 | 0.003 | 108 | 2.0 | 0.8"),
    "SPE14": (VL + "spetool_2d3d_tapered.txt", 14, "Depth of cut: 1xCutting diameter"),
    "IDC165": ("planning/extrapolation_2026-09-24/fetch/G10/sources/wood_idc_feeds_speeds.txt", 165,
               "60deg V-bit 0.25 2 0.216 2.00 0.25 30 40 20 ... 22,000 (Feed 40 in/min, Plunge 20 in/min)"),
    "IDC285": ("planning/extrapolation_2026-09-24/fetch/G10/sources/wood_idc_feeds_speeds.txt", 285,
               "60deg V-bit 0.25 2 0.216 2.00 0.25 30 60 20 ... 22,000 (Feed 60 in/min, Plunge 20 in/min)"),
    "CUR785": ("planning/extrapolation_2026-09-24/fetch/G7/sources/marcon2021_generalized_force_model.txt", 785,
               "Down-milling 0  Kmodel = -3e-6 GA2 + 5e-4 GA + 56e-3"),
    "CUR787": ("planning/extrapolation_2026-09-24/fetch/G7/sources/marcon2021_generalized_force_model.txt", 787,
               "Intmodel = -3e-7 GA2 + 4e-5 A + 47e-4 (down-milling, helix 0)"),
    "CUR672": ("planning/extrapolation_2026-09-24/fetch/G7/sources/marcon2021_generalized_force_model.txt", 672,
               "Fc(GA) = Ks,norm(GA) . h + int,norm(GA) . Ap x rho   (4)"),
    "CUR155": ("planning/extrapolation_2026-09-24/fetch/G7/sources/marcon2021_generalized_force_model.txt", 155,
               "uncut chip thicknesses h roughly varying from 40 um ... to 100 um (lines 155-157)"),
    "FPL": ("planning/extrapolation_2026-09-24/fetch/G7/pdf/fpl_gtr190_wood_handbook_2010.pdf", 6708,
            "Table 5-3a, Oak, white: 'White Green 0.60 ...' / '12% 0.68 105,000 ...' (pdftotext -layout text lines 6707-6708); "
            "footnote line 6866: 'Specific gravity is based on weight when ovendry and volume when green or at 12% moisture content.'"),
    "WSP": ("crates/rs_cam_core/data/wood_species.toml", 378,
            'display_name = "Oak, white" / janka_lbf = 1348.9 / specific_gravity_12 = 0.68 / source_id = "fpl_ch5_2010"'),
    "MACH": ("planning/rivmap_block_job/nz_wanaka_21_operator.toml", 59,
             "spindle Variable min_rpm 6000 max_rpm 24000; power VfdConstantTorque rated_power_kw 1.5 rated_rpm 24000 (lines 59-65); "
             "max_rate_xyz_mm_min [15000, 15000, 2500] (lines 84-88)"),
    "T6CC": ("planning/rivmap_block_job/nz_wanaka_21_operator.toml", 384,
             "[tools.centre_cutting] stated = \"no\", centre_core_diameter_mm = 8.0, blade_protrusion_mm = 1.0 (tool 6, lines 383-386)"),
}

# ---------------------------------------------------------------- machine
N_MAX, N_MIN = 24000.0, 6000.0
P_RATED, N_RATED = 1.5, 24000.0
V_XY, V_Z = 15000.0, 2500.0


def p_avail(n):
    """Constant torque below the rated speed (machine file model)."""
    return P_RATED * min(n, N_RATED) / N_RATED


# ---------------------------------------------------------------- force line
# Curti 2021 Table 5, down-milling, helix 0: vertex of each quadratic.
KS_A, KS_B, KS_C = -3e-6, 5e-4, 56e-3
IN_A, IN_B, IN_C = -3e-7, 4e-5, 47e-4
GA_KS = -KS_B / (2 * KS_A)
KS_N = KS_C - KS_B ** 2 / (4 * KS_A)
GA_IN = -IN_B / (2 * IN_A)
IN_N = IN_C - IN_B ** 2 / (4 * IN_A)
SG12 = 0.68
RHO = SG12 * 1000 * 1.12  # ovendry mass x 1.12 = mass at 12 % MC, volume at 12 %
KS = KS_N * RHO  # N/mm^2
FE = IN_N * RHO  # N/mm of edge


def lift(ae, d):
    """Radial chip thinning: peak chip = fz * 2 sqrt(r(1-r)) for r = ae/d < 1/2."""
    r = ae / d
    if r >= 0.5:
        return 1.0
    return 1.0 / (2.0 * math.sqrt(r * (1.0 - r)))


def psi(ae, d):
    return math.acos(max(-1.0, 1.0 - 2.0 * min(ae, d) / d))


def p_flat(vf, n, ae, ap, d, z):
    """Mean power, flat peripheral cut. Shear: Ks x MRR. Edge: F_edge x ap x
    cutting speed x engaged fraction z psi / 2pi. N mm/min / 6e7 = kW."""
    shear = KS * ae * ap * vf / 6e7
    edge = FE * ap * math.pi * d * n * z * psi(ae, d) / (2 * math.pi) / 6e7
    return shear + edge, shear, edge


def p_vgroove(vf, n, ap, half_deg, z):
    """V-bit slot. Cross-section = ap x R (triangle), R = ap tan(half).
    Edge term per tooth: integral of F_edge x (dr / sin h) x 2 pi r n from 0
    to R = F_edge x pi n R^2 / sin h; times z teeth; engaged half a turn."""
    h = math.radians(half_deg)
    r = ap * math.tan(h)
    shear = KS * ap * r * vf / 6e7
    # integral over r of FE * (dr/sin h) * 2 pi r n, times z teeth, engaged 1/2 rev
    edge = FE * (math.pi * n * r * r / math.sin(h)) * z * 0.5 / 6e7
    return shear + edge, shear, edge


# ---------------------------------------------------------------- app values
# From MCP, build b9e67b24e (P20e), 2026-11-09. sug = get_suggest_rationale,
# shp = get_toolpath_params. chip = vendor_band.score_winner_chip_mm.
APP = {
    0: dict(sug=dict(feed=6488, rpm=18000, plunge=2162, ramp=6447, preq=0.04341691566912402, pav=1.125, ae=8.0, ap=0.5, chip=0.17912843029568412),
            shp=dict(feed=6488, rpm=18000, plunge=2162, ramp=6447)),
    1: dict(sug=dict(feed=6999, rpm=18000, plunge=2162, ramp=6448, preq=0.9849273042542057, pav=1.125, ae=8.8, ap=10.0, chip=0.17912843029568412),
            shp=dict(feed=8475, rpm=18000, plunge=2162, ramp=6448)),
    2: dict(sug=dict(feed=5279, rpm=18000, plunge=2285, ramp=4571, preq=0.14209732235325212, pav=1.125, ae=2.4, ap=6.0, chip=0.127),
            shp=dict(feed=5279, rpm=18000, plunge=2285, ramp=4571)),
    3: dict(sug=dict(feed=2286, rpm=18000, plunge=2286, ramp=None, preq=0.14098888469360185, pav=1.125, ae=6.0, ap=0.127, chip=0.0635,
                     padv=True, section="axial_circle"),
            shp=dict(feed=1000, rpm=18000, plunge=None, ramp=None)),
    4: dict(sug=dict(feed=2286, rpm=18000, plunge=2286, ramp=None, preq=0.14098888469360185, pav=1.125, ae=6.0, ap=0.127, chip=0.0635,
                     padv=True, section="axial_circle"),
            shp=dict(feed=1000, rpm=18000, plunge=None, ramp=None)),
    # idx 5: Suggest prints no feed or rpm (shipped_power unmodeled); the
    # Suggest chip is score_winner 0.14919; plunge 1347 from basis.plunge.
    5: dict(sug=dict(feed=None, rpm=None, plunge=1347, ramp=None, preq=None, pav=None, ae=None, ap=None, chip=0.14919260416145616),
            shp=dict(feed=3165, rpm=10610, plunge=1347, ramp=None)),
    6: dict(sug=dict(feed=1010, rpm=18000, plunge=240, ramp=None, preq=0.004410504385733152, pav=1.125, ae=2.0, ap=0.7, chip=0.05501007821762709),
            shp=dict(feed=1010, rpm=18000, plunge=240, ramp=None)),
    7: dict(sug=dict(feed=990, rpm=18000, plunge=240, ramp=None, preq=0.0031308548533014575, pav=1.125, ae=2.0, ap=0.5, chip=0.05501007821762709),
            shp=dict(feed=1258, rpm=18000, plunge=240, ramp=None)),
    8: dict(sug=dict(feed=990, rpm=18000, plunge=240, ramp=990, preq=0.0031308548533014575, pav=1.125, ae=2.0, ap=0.5, chip=0.05501007821762709),
            shp=dict(feed=1258, rpm=18000, plunge=240, ramp=1258)),
    # idx 9: Suggest prints no feed or rpm (unmodeled); chip = score_winner 0.127.
    9: dict(sug=dict(feed=None, rpm=None, plunge=381, ramp=None, preq=None, pav=None, ae=None, ap=None, chip=0.127),
            shp=dict(feed=2286, rpm=18000, plunge=381, ramp=None)),
    10: dict(sug=dict(feed=2286, rpm=18000, plunge=762, ramp=None, preq=0.02821826960316605, pav=1.125, ae=6.35, ap=2.6, chip=0.0635),
             shp=dict(feed=2286, rpm=18000, plunge=762, ramp=None)),
    11: dict(sug=dict(feed=6488, rpm=18000, plunge=2162, ramp=None, preq=0.09592408687942625, pav=1.125, ae=18.0, ap=0.5, chip=0.17912843029568412),
             shp=dict(feed=6488, rpm=18000, plunge=2162, ramp=None)),
    12: dict(sug=dict(feed=6488, rpm=18000, plunge=2162, ramp=6447, preq=0.04341691566912402, pav=1.125, ae=8.0, ap=0.5, chip=0.17912843029568412),
             shp=dict(feed=6488, rpm=18000, plunge=2162, ramp=6447)),
    13: dict(sug=dict(feed=2286, rpm=18000, plunge=762, ramp=None, preq=0.0015027480853757064, pav=1.125, ae=6.35, ap=0.6, chip=0.0635),
             shp=dict(feed=2286, rpm=18000, plunge=762, ramp=None)),
    14: dict(sug=dict(feed=8060, rpm=18000, plunge=2162, ramp=6448, preq=0.3129324141800096, pav=1.125, ae=5.76, ap=4.166, chip=0.17912843029568412),
             shp=dict(feed=8060, rpm=18000, plunge=2162, ramp=6448)),
    15: dict(sug=dict(feed=5429, rpm=18000, plunge=2285, ramp=4571, preq=0.12446544548149027, pav=1.125, ae=2.2112, ap=5.532, chip=0.127),
             shp=dict(feed=5429, rpm=18000, plunge=2285, ramp=4571)),
    16: dict(sug=dict(feed=2317, rpm=18000, plunge=240, ramp=None, preq=0.0010691975045593193, pav=1.125, ae=0.8717797887081346, ap=0.2, chip=0.06437017664182963),
             shp=dict(feed=2317, rpm=18000, plunge=240, ramp=None)),
    17: dict(sug=dict(feed=3165, rpm=10610, plunge=1347, ramp=3165, preq=0.042295681141273724, pav=0.663125, ae=6.0, ap=1.25, chip=0.14919260416145616),
             shp=dict(feed=3165, rpm=10610, plunge=1582, ramp=3165)),
}

# ---------------------------------------------------------------- toolpaths
# Inputs from get_toolpath_params (op kind, stepover, depth per pass,
# trochoid_cap_mult, entry) and the project tools. No engine value.
TP = {
    0: ("Face back to T1", 6, "face", dict(ae=8.0, ap=0.5, entry="ramp")),
    1: ("Back bulk rough 18 mm", 6, "adaptive", dict(ae=5.5, cap=1.6, ap=10.0, entry="ramp")),
    2: ("Back rest 6 mm (cavity, hollow, lakes, pockets)", 0, "adaptive", dict(ae=1.5, cap=1.6, ap=6.0, entry="ramp")),
    3: ("Pin drill, flip pins", 0, "drill", dict()),
    4: ("Drill light holes 6 mm end mill", 0, "drill", dict()),
    5: ("River channel 6 mm, pass 7 of 7", 0, "slot", dict(ae=6.0, ap=None, entry="plunge")),
    6: ("Lake pocket 2 mm, 11 lakes to Z 5.88", 5, "pocket", dict(ae=0.8, ap=0.7, entry="plunge")),
    7: ("Lake pocket 2 mm, 1 lake to Z 3.99", 5, "pocket", dict(ae=1.0, ap=0.5, entry="plunge")),
    8: ("ANTENNA_D5.00 pocket 2 mm", 5, "pocket", dict(ae=1.0, ap=0.5, entry="ramp")),
    9: ("V rivers and lakes", 1, "vtrace", dict(ap=None, entry="plunge")),
    10: ("Day strip slit V60", 2, "vtrace", dict(ap=2.6, entry="plunge")),
    11: ("Face clamp pads 6 mm", 6, "pocket", dict(ae=8.0, ap=0.5, entry="plunge")),
    12: ("Face front to T", 6, "face", dict(ae=8.0, ap=0.5, entry="ramp")),
    13: ("Engrave day labels V60", 2, "vtrace", dict(ap=0.6, entry="plunge")),
    14: ("Front bulk rough 18 mm", 6, "adaptive", dict(ae=3.6, cap=1.6, ap=4.166, entry="ramp")),
    15: ("Front 3D rough", 0, "adaptive", dict(ae=1.382, cap=1.6, ap=5.532, entry="ramp")),
    16: ("Front scallop finish", 4, "ball", dict(ae=0.8717797887081346, ap=0.2, R=1.0, entry="plunge")),
    17: ("Outline profile with tabs", 0, "slot", dict(ae=6.0, ap=1.25, entry="ramp")),
}
TOOLS = {0: (6.0, 2), 1: (3.175, 1), 2: (6.35, 2), 4: (2.0, 2), 5: (2.0, 1), 6: (18.0, 2)}

# ---------------------------------------------------------------- chips
# Each chip candidate: name -> (lo, hi, derivation)
c_spk6 = 0.0050 * IN
c_ons = (0.005 * IN, 0.007 * IN)
c_zrn6 = (0.007 * IN, 0.009 * IN)
c_spk12 = 0.0057 * IN
c_spk19_3f = 0.009 * IN
c_116 = 0.0020 * IN
c_332 = 0.0023 * IN
slope = math.log(c_332 / c_116) / math.log((3 / 32) / (1 / 16))
c_2mm_log = c_116 * (2.0 / (IN / 16)) ** slope
c_2mm_lin = c_116 + (2.0 - IN / 16) / (IN * 3 / 32 - IN / 16) * (c_332 - c_116)
c_v20 = (0.003 * IN, 0.007 * IN)
c_v60 = (90 * IN / (18000 * 2), 0.003 * IN)
c_spe2 = 0.003 * IN

CHIP = {
    # P2 ruled (b), operator 2026-11-08: the Onsrud 60-200 row stands for tool 0.
    0: {"Onsrud 60-200 1/4 in": (c_ons[0], c_ons[1], 'ONS46 ".005-.007" x 25.4 = %.4f-%.4f mm' % c_ons)},
    6: {"bracket": (c_spk12, c_spk19_3f,
                    'UNSOURCED: no 18 mm 2F row. Bracket: SPK37 1/2 in 2F ".0057"" = %.4f mm (below), SPK54 3/4 in 3F ".009"" = %.4f mm (above)' % (c_spk12, c_spk19_3f))},
    5: {"bracket": (c_116, c_332,
                    'UNSOURCED for 1F. 2F bracket: SPK12 1/16 in ".0020"" = %.4f mm, SPK17 3/32 in ".0023"" = %.4f mm; '
                    'log-log at 2.0 mm = %.5f, linear = %.5f' % (c_116, c_332, c_2mm_log, c_2mm_lin))},
    1: {"AMS 18/30 deg 1F": (c_v20[0], c_v20[1], 'AMS11 18deg and 30deg 1F both "0.003" - 0.007""; 20deg lies between equal cells: %.4f-%.4f mm' % c_v20)},
    2: {"AMS 60 deg 2F": (c_v60[0], c_v60[1], 'AMS11 60deg 2F: "90"" / (18000 x 2) x 25.4 = %.4f mm; printed "0.003"" = %.4f mm; the cell disagrees with itself' % c_v60)},
    4: {"SpeTool 2.0 mm": (c_spe2, c_spe2, 'SPE23 chipload "0.003" (inch, 2 flutes derived: 108/(18000 x 0.003) = %.2f) x 25.4 = %.4f mm' % (108 / (18000 * 0.003), c_spe2))},
}

# ---------------------------------------------------------------- grading


def within(v, lo, hi):
    return lo * (1 - TOL) <= v <= hi * (1 + TOL)


def grade(app, cands, status, override=None):
    """cands: dict label -> (lo, hi). Return (grade, matched labels)."""
    if override:
        return override, []
    if app is None:
        return ("FAIL" if cands else "PASS"), []
    if not cands:
        return "NEEDS-RULING", []
    hit = [k for k, (lo, hi) in cands.items() if within(app, lo, hi)]
    if status in ("SOURCED", "DERIVED"):
        return ("PASS" if hit else "FAIL"), hit
    if status == "UNSOURCED":
        return ("NEEDS-RULING" if hit else "FAIL"), hit
    # POLICY
    if len(hit) == len(cands):
        return "PASS", hit
    return ("NEEDS-RULING" if hit else "FAIL"), hit


ROWS = []


def cand_text(cands):
    parts = []
    for k, (lo, hi) in cands.items():
        parts.append("%s: %s" % (k, fmt(lo) if abs(hi - lo) < 1e-12 else "%s-%s" % (fmt(lo), fmt(hi))))
    return "; ".join(parts) if parts else "none"


def row(idx, qty, status, binding, cands, deriv, app_key=None, app_val=None,
        override=None, note="", unit="mm/min", expected_text=None,
        grade_cands=None, n_app=None):
    """grade_cands: the candidates recomputed at the app's own rpm. The rpm
    row alone carries P1; feed, plunge and ramp rows are graded at n_app."""
    a = APP[idx]
    sug = a["sug"].get(app_key) if app_key else app_val
    shp = a["shp"].get(app_key) if app_key and app_key in a["shp"] else None
    gc = cands if grade_cands is None else grade_cands
    g, hit = grade(sug, gc, status, override)
    if expected_text is None:
        expected_text = cand_text(cands)
        if grade_cands is not None:
            expected_text += " // graded at app rpm %.0f: %s" % (n_app, cand_text(grade_cands))
    ROWS.append(dict(idx=idx, toolpath=TP[idx][0], quantity=qty, unit=unit, status=status,
                     binding=binding, expected=expected_text,
                     candidates={k: [round(lo, 6), round(hi, 6)] for k, (lo, hi) in cands.items()},
                     derivation=deriv, app_suggest=sug, app_shipped=shp, grade=g,
                     matched=hit, note=note,
                     grade_candidates={k: [round(lo, 6), round(hi, 6)] for k, (lo, hi) in gc.items()},
                     n_app=n_app))


def fmt(x):
    if x is None:
        return "-"
    if abs(x) >= 100:
        return "%.0f" % x
    if abs(x) >= 1:
        return "%.3f" % x
    return "%.5f" % x


def n_b_lateral(chip_hi, z, lft):
    """Constant chip up to the binding limit: spindle max or XY axis rate."""
    n_xy = V_XY / (chip_hi * z * lft)
    n = min(N_MAX, n_xy)
    return n, ("spindle max 24000" if n == N_MAX else "XY rate 15000")


# ---------------------------------------------------------------- rows
for idx, (name, tid, kind, g) in TP.items():
    d, z = TOOLS[tid]
    a = APP[idx]
    chips = CHIP[tid]
    chip_status = {0: "DERIVED", 6: "UNSOURCED", 5: "UNSOURCED", 1: "SOURCED", 2: "SOURCED", 4: "SOURCED"}[tid]
    chip_bind = "chart"
    deriv_chip = " | ".join(v[2] for v in chips.values())
    if tid == 0:
        deriv_chip += " | P2 ruled (b), operator 2026-11-08: the Onsrud 60-200 row stands for the unbranded tool. Onsrud column read by text position (I)."
    ap = g.get("ap")
    depth_note = ""
    if ap is not None:
        depth_note = "ap/D = %.3f <= 1: the 1 x D chart condition holds (SPK38/AMS3/SPE14); no feed derate." % (ap / d)

    # -------------------------------------------------------- drills
    if kind == "drill":
        rd = 90 * IN  # Ramp Down 90" (SPK26)
        axial = rd / (18000 * z)
        row(idx, "axial chip per tooth", "SOURCED", "chart", {"SPK26": (axial, axial)},
            'SPK26 Ramp Down "90"" x 25.4 = %.1f mm/min; / (18000 x 2) = %.5f mm (SPK3 18,000 RPM; SPK52 Ramp Down = Feed / flutes)' % (rd, axial),
            app_key="chip", unit="mm", note="App chip = vendor_band.score_winner_chip_mm.")
        nb = min(N_MAX, V_Z / (axial * z))
        row(idx, "rpm", "DERIVED", "Z rate", {"Z-bound": (nb, nb)},
            "P1 ruled (b), operator 2026-11-08. Constant axial chip; vz = %.4f x 2 x n <= 2500 -> n <= %.0f; < 24000, so the Z rate binds." % (axial, nb),
            app_key="rpm", unit="rpm")
        na = a["sug"]["rpm"]
        gd = {"@app": (min(axial * 2 * na, V_Z), min(axial * 2 * na, V_Z))}
        row(idx, "feed (drill, vertical)", "DERIVED", "Z rate", {"Z-bound": (V_Z, V_Z)},
            "P1 (b): %.5f x 2 x %.0f = %.0f = Z rate 2500 (SPK26 Ramp Down 90\" = %.1f mm/min at 18000). Graded at the app rpm: axial chip x 2 x n." % (axial, nb, axial * 2 * nb, rd),
            app_key="feed", note="Shipped 1000 mm/min is an operator value (feeds_lock spindle).", grade_cands=gd, n_app=na)
        row(idx, "plunge", "DERIVED", "Z rate", {"Z-bound": (V_Z, V_Z)},
            "A drill cycle has one feed; this row repeats the feed row.", app_key="plunge", grade_cands=gd, n_app=na)
        row(idx, "ramp feed", "N/A", "none", {}, "A drill cycle has no ramp.", app_key="ramp", override="N/A")
        # Drill power ruled ADVISORY, operator 2026-11-08 (P20d Q1): no source for an end
        # cut, so the figure may show only as advisory on the axial section; no gate acts.
        adv_ok = a["sug"].get("padv") is True and a["sug"].get("section") == "axial_circle"
        row(idx, "power required vs available", "DERIVED", "advisory only", {},
            "Curti 2021 models peripheral milling only (CUR672). No repo source gives an axial drilling force for wood. "
            "Ruling 2026-11-08: the app may print the axial-section figure only as advisory (shipped_power.advisory = true, section = axial_circle).",
            app_key="preq", unit="kW", override=("PASS" if adv_ok else "FAIL"),
            note="Graded on the label: advisory = %s, section = %s." % (a["sug"].get("padv"), a["sug"].get("section")))
        continue

    # -------------------------------------------------------- chip row
    row(idx, "chip per tooth (target peak)", chip_status, chip_bind,
        {k: (v[0], v[1]) for k, v in chips.items()}, deriv_chip + (" " + depth_note if depth_note else ""),
        app_key="chip", unit="mm",
        note="App = vendor_band.score_winner_chip_mm (the target before the radial lift).")
    chip_hi = max(v[1] for v in chips.values())

    # -------------------------------------------------------- lifts
    lifts = {}
    lift_deriv = ""
    if kind == "adaptive":
        ae_cap = g["ae"] * g["cap"]
        la, lb = lift(g["ae"], d), lift(ae_cap, d)
        # P3 ruled (b), lead 2026-11-08 (operator: "whichever is most correct"): the chart
        # chip is a peak chip; the peak occurs at the largest engagement, the cap width.
        lifts = {"cap ae": lb}
        lift_deriv = ("lift = 1 / (2 sqrt(r(1-r))), r = ae/D. P3 ruled (b): ae %.3f x %.1f = %.4f, r %.4f, lift %.4f (the nominal ae %.3f would give lift %.4f)."
                      % (g["ae"], g["cap"], ae_cap, ae_cap / d, lb, g["ae"], la))
    elif kind == "face":
        la = lift(g["ae"], d)
        lifts = {"face ae": la}
        lift_deriv = "Face: max ae = stepover %.1f, r %.4f, lift %.4f." % (g["ae"], g["ae"] / d, la)
    elif kind in ("pocket", "slot"):
        lifts = {"slot": 1.0}
        lift_deriv = "The op cuts a full slot (pocket first ring, profile, centre-line curve): r >= 0.5, lift 1."
    elif kind == "vtrace":
        lifts = {"slot": 1.0}
        lift_deriv = "V trace is a slot; the V chart is for the same geometry: lift 1."
    elif kind == "ball":
        R = g["R"]
        sk = math.sqrt(1 - (1 - g["ap"] / R) ** 2)
        lifts = {"P4a no lead-angle lift": 1.0, "P4b lead-angle lift": 1 / sk}
        lift_deriv = ("Ball R %.1f at ap %.2f: sin k = sqrt(1-(1-ap/R)^2) = %.4f; peak chip = fz sin k. P4b lift = 1/%.4f = %.4f. "
                      "The chart chip is at 1 x D (SPE14), where the equator cuts (sin k = 1)." % (R, g["ap"], sk, sk, 1 / sk))

    # -------------------------------------------------------- rpm row
    pol_n = {}
    deriv_n = []
    max_lift = max(lifts.values())
    nb, nb_bind = n_b_lateral(chip_hi, z, max_lift)
    pol_n["P1b binding limit"] = nb  # P1 ruled (b), operator 2026-11-08
    chart_rpm = {0: "SPK3 18,000 RPM (Onsrud prints no RPM for 60-200: ONS61)", 6: "SPK3 18,000 RPM (nearest printed chart)",
                 5: "SPK3 18,000 RPM", 1: "AMS3 18,000", 2: "AMS3 18,000", 4: "SPE23 18,000"}[tid]
    deriv_n.append("P1 ruled (b), operator 2026-11-08 (the chart rpm: " + chart_rpm + ").")
    deriv_n.append("n_xy = 15000 / (chip_hi %.4f x %d x lift %.4f) = %.0f; spindle max 24000; n_b = %.0f (%s)."
                   % (chip_hi, z, max_lift, V_XY / (chip_hi * z * max_lift), nb, nb_bind))
    deriv_n.append("Power does not bind: P_req and P_avail both scale with n (constant torque below 24000 rpm, MACH), so the ratio holds at every n.")
    app_rpm_val = a["sug"]["rpm"] if a["sug"]["rpm"] is not None else a["shp"]["rpm"]
    row(idx, "rpm", "DERIVED", nb_bind,
        {k: (v, v) for k, v in pol_n.items()}, " ".join(deriv_n),
        app_val=app_rpm_val, unit="rpm",
        note=("Suggest prints no rpm; graded on the shipped param." if a["sug"]["rpm"] is None else ""))
    ROWS[-1]["app_shipped"] = a["shp"]["rpm"]

    # -------------------------------------------------------- feed row
    n_app = app_rpm_val

    def feed_cands(pns):
        out = {}
        for (pn, n), (cn, cv), (ln, lv) in itertools.product(pns.items(), chips.items(), lifts.items()):
            lo = min(cv[0] * z * n * lv, V_XY)
            hi = min(cv[1] * z * n * lv, V_XY)
            key = "/".join(x for x in (pn.split(" ")[0], cn.split(" ")[0] if len(chips) > 1 else "", ln.split(" ")[0] if len(lifts) > 1 else "") if x)
            out[key] = (lo, hi)
        return out
    cands = feed_cands(pol_n)
    gfeed = feed_cands({"@app": n_app})
    feed_status = "POLICY" if kind == "ball" else "DERIVED"  # P4 open
    if tid in (6, 5):
        feed_status = "UNSOURCED"
    deriv_f = "feed = chip x z x n x lift (SPK47), clamped to XY 15000. " + lift_deriv
    if tid == 1 or tid == 2:
        deriv_f += " Printed IPM: " + ("50\"-130\" = 1270-3302 mm/min at 18000." if tid == 1 else "90\" = 2286 mm/min at 18000.")
    app_feed = a["sug"]["feed"] if a["sug"]["feed"] is not None else a["shp"]["feed"]
    if tid == 1:
        if n_app == 18000:
            gfeed = {"@app": (min(gfeed["@app"][0], 50 * IN), max(gfeed["@app"][1], 130 * IN))}
    row(idx, "feed", feed_status, "chart chip x lift; XY 15000", cands, deriv_f,
        app_val=app_feed, grade_cands=gfeed, n_app=n_app,
        note=("Suggest prints no feed; graded on the shipped param." if a["sug"]["feed"] is None else ""))
    ROWS[-1]["app_shipped"] = a["shp"]["feed"]
    if idx == 6:
        ROWS[-1]["note"] += (" With the 2F log-log chip %.5f in a slot the feed is %.0f; the app's 1010 is %+.1f %%: it applies the ae 0.8 lift in a pocket that slots."
                             % (c_2mm_log, c_2mm_log * 18000, (1010 / (c_2mm_log * 18000) - 1) * 100))
    if idx == 1:
        p8475 = p_flat(8475, 18000, g["ae"] * g["cap"], g["ap"], d, z)[0]
        ROWS[-1]["note"] += " Shipped 8475 is the operator value: P at ae 8.8 = %.3f kW = %.1f %% of 1.125 kW." % (p8475, p8475 / 1.125 * 100)

    # -------------------------------------------------------- plunge row
    if tid == 0:
        rd = 90 * IN
        pc = {"Z-bound": (min(c_spk6 * N_MAX, V_Z), min(c_spk6 * N_MAX, V_Z))}
        pa = min(c_spk6 * n_app, V_Z)
        row(idx, "plunge", "DERIVED", "Z rate 2500", pc,
            "SPK26 Ramp Down 90\" = %.1f mm/min at 18000 (SPK52: Ramp Down = Feed / flutes = n x chip; the Spektra 6 mm chip 0.127 sets the plunge, P2 rules the side chip only). P1 (b): 0.127 x 24000 = %.0f > Z 2500 -> 2500. At the app rpm: 0.127 x n." % (rd, c_spk6 * N_MAX),
            app_key="plunge", grade_cands={"@app": (pa, pa)}, n_app=n_app,
            note=("Same tool and rpm as idx 17, but a different plunge (1347 vs 1582): the two ops disagree." if idx in (5, 17) else ""))
    elif tid == 6:
        row(idx, "plunge", "UNSOURCED", "Z rate 2500", {"<= Z rate": (0.0, V_Z)},
            "UNSOURCED. Tool 6 is not centre-cutting (T6CC). The Spektra Ramp Down rule (SPK52) is for spiral plunge bits; it does not transfer. Bound: Z rate 2500 (MACH). "
            "Geometry (I: field meaning): a straight plunge is possible only to the blade protrusion 1.0 mm; a 3 deg ramp sinks 8 x tan 3 = %.2f mm over the 8 mm core, < 1.0 mm. Needed: a plunge rate for this tool from its maker." % (8 * math.tan(math.radians(3))),
            app_key="plunge",
            note=("Straight plunge entry at ap 0.5 <= protrusion 1.0: geometrically possible." if idx == 11 else ""))
    elif tid == 5:
        pc = {"chip x 24000 (P1 b)": (c_116 * N_MAX, c_332 * N_MAX)}
        row(idx, "plunge", "UNSOURCED", "chart (2F rule)", pc,
            "UNSOURCED for 1F. SPK12/SPK17 print Ramp Down 35\" and 40\" (= %.0f and %.0f mm/min) at 1/16 and 3/32 in. "
            "SPK47 + SPK52 give Ramp Down = n x chip, which does not depend on the flute count; so a 1F tool with the 2F chip plunges at the same rate. Every candidate is >= %.0f mm/min." % (35 * IN, 40 * IN, 35 * IN),
            app_key="plunge", n_app=n_app,
            grade_cands={"@app": (35 * IN, 40 * IN) if n_app == 18000 else (c_116 * n_app, c_332 * n_app)},
            note="The app ships 240 from an unsourced repo rule (conservative: 3.7x slower than the lowest candidate).")
    elif tid == 2:
        f_lo, f_hi = c_v60[0] * 2 * 18000, c_v60[1] * 2 * 18000
        pc = {"IDC ratio x feed (P1 b)": (c_v60[0] * 2 * N_MAX / 3, c_v60[1] * 2 * N_MAX / 2)}
        row(idx, "plunge", "DERIVED", "cross-vendor ratio", pc,
            "No Amana plunge for this bit. IDC 60deg 2F V-bit prints plunge/feed 20/40 = 0.5 (IDC165) and 20/60 = 0.333 (IDC285); the two printings disagree. "
            "Range = [feed_lo/3, feed_hi/2]: a [%.0f/3, %.0f/2]." % (f_lo, f_hi),
            app_key="plunge", n_app=n_app,
            grade_cands={"@app": (c_v60[0] * 2 * n_app / 3, c_v60[1] * 2 * n_app / 2)})
    else:  # tid 1 (V20), 4 (tapered)
        row(idx, "plunge", "UNSOURCED", "none", {},
            ("No repo source prints a plunge for a 20deg 1F V-bit. Needed: a vendor plunge or Ramp Down for this bit." if tid == 1 else
             "SpeTool (SPE23) and Amana ZrN print no plunge. Needed: a vendor plunge for a 2 mm tapered ball."),
            app_key="plunge", note="The app says 'no source' too.")

    # -------------------------------------------------------- ramp row
    if g.get("entry") == "ramp":
        def ramp_cands(pns):
            out = {}
            for (pn, n), (cn, cv) in itertools.product(pns.items(), chips.items()):
                key = pn.split(" ")[0] + (("/" + cn.split(" ")[0]) if len(chips) > 1 else "")
                out[key] = (min(cv[0] * z * n, V_XY), min(cv[1] * z * n, V_XY))
            return out
        rc = ramp_cands(pol_n)
        grc = ramp_cands({"@app": n_app})
        st = "UNSOURCED" if tid in (6, 5) else "DERIVED"
        vert = ""
        if tid == 0:
            vert = " Vertical part: 4572 x sin 3deg = %.0f mm/min < Ramp Down 2286: the side chip binds." % (4572 * math.sin(math.radians(3)))
        row(idx, "ramp feed", st, "slot chip (full-width ramp)", rc,
            "A ramp cuts a full slot: ramp feed = chip x z x n (lift 1)." + vert, app_key="ramp",
            grade_cands=grc, n_app=n_app)
    else:
        row(idx, "ramp feed", "N/A", "none", {}, "The entry is a straight plunge; no ramp.", app_key="ramp", override="N/A")

    # -------------------------------------------------------- power row
    s = a["sug"]
    if s["preq"] is None:
        row(idx, "power required vs available", "UNSOURCED", "none", {},
            "The depth per pass comes from the simulated surface; no fixed ap exists to evaluate. The app declares 'unmodeled (DepthAfterSimulation)'.",
            app_key="preq", unit="kW", override="PASS")
        continue
    if kind == "vtrace":
        p, sh, ed = p_vgroove(s["feed"], s["rpm"], s["ap"], 30.0, z)
        deriv_p = ("V-groove at the app point: vf %d, n %d, ap %.2f, R = ap tan 30 = %.4f; shear Ks x ap x R x vf = %.5f; edge F_edge x pi n R^2 / sin 30 x z / 2 = %.5f; "
                   "total %.5f kW. The app uses a rectangle ae %.2f x ap." % (s["feed"], s["rpm"], s["ap"], s["ap"] * math.tan(math.radians(30)), sh, ed, p, s["ae"]))
        note = "App overstates the V cross-section (rectangle, not triangle): conservative."
    elif kind == "ball":
        R = g["R"]
        deng = 2 * math.sqrt(R * R - (R - s["ap"]) ** 2)
        p, sh, ed = p_flat(s["feed"], s["rpm"], s["ae"], s["ap"], deng, z)
        deriv_p = ("Upper bound: rectangle ae x ap on the engaged diameter 2 sqrt(R^2-(R-ap)^2) = %.3f mm; shear %.6f + edge %.6f = %.6f kW." % (deng, sh, ed, p))
        note = "Bound only; the scallop cross-section is smaller than the rectangle."
    else:
        p, sh, ed = p_flat(s["feed"], s["rpm"], s["ae"], s["ap"], d, z)
        deriv_p = ("At the app point: vf %d, n %d, ae %.4f, ap %.3f, D %.1f, z %d; psi = acos(1 - 2ae/D) = %.4f rad; shear = Ks ae ap vf / 6e7 = %.5f; "
                   "edge = F_edge ap pi D n z psi / 2pi / 6e7 = %.5f; total %.5f kW." % (s["feed"], s["rpm"], s["ae"], s["ap"], d, z, psi(s["ae"], d), sh, ed, p))
        note = ""
    pav = p_avail(s["rpm"])
    deriv_p += " P_avail = 1.5 x %d / 24000 = %.4f kW (MACH); ratio %.3f. Spindle efficiency: no source; the ratio assumes the rated shaft power." % (s["rpm"], pav, p / pav)
    if kind == "ball":
        cands = {"<= bound": (0.0, p)}
    else:
        cands = {"model": (p, p)}
    row(idx, "power required vs available", "DERIVED", "power (ratio %.3f)" % (p / pav), cands, deriv_p,
        app_key="preq", unit="kW", note=note)
    ROWS[-1]["ratio"] = round(p / pav, 4)
    ROWS[-1]["app_available_kw"] = s["pav"]

# ---------------------------------------------------------------- output


def toml_val(v):
    if v is None:
        return '"-"'
    if isinstance(v, bool):
        return "true" if v else "false"
    if isinstance(v, (int, float)):
        return repr(round(float(v), 6))
    if isinstance(v, list):
        return "[" + ", ".join(toml_val(x) for x in v) + "]"
    if isinstance(v, dict):
        return "{ " + ", ".join('"%s" = %s' % (k, toml_val(x)) for k, x in v.items()) + " }"
    return '"' + str(v).replace("\\", "\\\\").replace('"', '\\"') + '"'


out = ["# Wanaka 21 feeds and speeds acceptance table (machine-readable twin of wanaka_21.md).",
       "# Generated by derive_wanaka_21.py. Change only with an operator ruling.",
       "",
       "[meta]",
       'project = "planning/rivmap_block_job/nz_wanaka_21_operator.toml"',
       'graded_build = "b9e67b24e"',
       "tolerance_rel = %s" % TOL,
       "force_ks_n_mm2 = %s" % round(KS, 6),
       "force_f_edge_n_mm = %s" % round(FE, 6),
       "rho_kg_m3 = %s" % round(RHO, 3),
       "", "[sources]"]
for k, (f, ln, q) in SRC.items():
    out.append('%s = { file = %s, line = %d, quote = %s }' % (k, toml_val(f), ln, toml_val(q)))
out.append("")
for r in ROWS:
    out.append("[[row]]")
    for k in ("idx", "toolpath", "quantity", "unit", "status", "binding", "expected", "candidates",
              "grade_candidates", "n_app", "derivation", "app_suggest", "app_shipped", "grade", "matched", "note", "ratio"):
        if k in r:
            out.append("%s = %s" % (k, toml_val(r[k])))
    out.append("")
(HERE / "wanaka_21.toml").write_text("\n".join(out))

md = []
for idx in TP:
    rs = [r for r in ROWS if r["idx"] == idx]
    md.append("### idx %d: %s (tool %d)\n" % (idx, TP[idx][0], TP[idx][1]))
    md.append("| Quantity | Expected | Status | Binding | App Suggest | App shipped | Grade |")
    md.append("|---|---|---|---|---|---|---|")
    for r in rs:
        m = (" (matches: %s)" % ", ".join(r["matched"])) if r["grade"] == "NEEDS-RULING" and r["matched"] else ""
        md.append("| %s | %s | %s | %s | %s | %s | **%s**%s |" % (
            r["quantity"], r["expected"], r["status"], r["binding"], fmt(r["app_suggest"]),
            fmt(r["app_shipped"]), r["grade"], m))
    md.append("")
    for r in rs:
        md.append("- *%s*: %s%s" % (r["quantity"], r["derivation"], (" Note: " + r["note"]) if r["note"] else ""))
    md.append("")
from collections import Counter
st_c = Counter(r["status"] for r in ROWS)
gr_c = Counter(r["grade"] for r in ROWS)
summ = ["## Summary (generated by derive_wanaka_21.py)", "",
        "%d rows (%d toolpaths x 6 quantities)." % (len(ROWS), len(TP)), "",
        "- Status: " + ", ".join("%s %d" % (k, st_c[k]) for k in ("SOURCED", "DERIVED", "POLICY", "UNSOURCED", "N/A")),
        "- Grade: " + ", ".join("%s %d" % (k, gr_c[k]) for k in ("PASS", "NEEDS-RULING", "FAIL", "N/A")), "",
        "FAIL rows:", ""]
for r in ROWS:
    if r["grade"] == "FAIL":
        summ.append("- idx %d %s: app %s; expected %s." % (r["idx"], r["quantity"], fmt(r["app_suggest"]), r["expected"]))
summ += ["", "## Rows", ""]
(HERE / "wanaka_21.md").write_text((HERE / "wanaka_21_header.md").read_text() + "\n".join(summ) + "\n" + "\n".join(md))

# ---------------------------------------------------------------- summary
from collections import Counter
print("Ks %.4f N/mm2 (vertex GA %.2f, Ks_n %.7f); F_edge %.4f N/mm (vertex GA %.2f, int_n %.7f); rho %.1f"
      % (KS, GA_KS, KS_N, FE, GA_IN, IN_N, RHO))
print("rows", len(ROWS))
print("status", Counter(r["status"] for r in ROWS))
print("grade", Counter(r["grade"] for r in ROWS))
for r in ROWS:
    if r["grade"] == "FAIL":
        print("FAIL idx %d %s: app %s vs %s" % (r["idx"], r["quantity"], fmt(r["app_suggest"]), r["expected"]))
# self-checks of claims the page makes
assert abs(KS_N - 0.0768333) < 1e-6 and abs(IN_N - 0.0060333) < 1e-6
assert abs(RHO - 761.6) < 1e-9
assert abs(90 * IN / (18000 * 2) - 0.0635) < 1e-12
assert abs(lift(5.5, 18) - 1.0854) < 1e-3
