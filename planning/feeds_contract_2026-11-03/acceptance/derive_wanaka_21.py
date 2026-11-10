#!/usr/bin/env python3
"""Independent acceptance table for the wanaka 21 feeds and speeds.

The script computes every expected value from printed source lines and
physics. It does not import or read the engine. The app values below are
copies of MCP read-tool output (get_toolpath_params, get_suggest_rationale,
build 2d0f7849f (P23c)). Re-grade a new build: replace the APP dict, run the script.

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
    "MACH": ("planning/rivmap_block_job/nz_wanaka_21_operator.toml", 60,
             "spindle Variable min_rpm 6000 max_rpm 24000; power VfdConstantTorque rated_power_kw 1.5 rated_rpm 24000 (lines 60-66); "
             "max_rate_xyz_mm_min [15000, 15000, 2500] (lines 85-89); "
             "spindle_strategy = \"constant_chip\", spindle_rpm_ceiling = 24000 (lines 50-51)"),
    # Repo rules with no vendor source (operator rulings 2026-11-10). The quote is the product source line.
    "SIZEC": ("crates/rs_cam_core/src/feeds/extrapolation/size.rs", 87,
              'const RULE_C: &str = "G1 form C: the generic size law (d / d_row)^0.61 inside 0.5x-2x of the row"; '
              "exponent: crates/rs_cam_core/src/feeds/vendor_lookup.rs line 802 'pub const CHIPLOAD_DIAMETER_EXPONENT: f64 = 0.61;'"),
    "SIZEA": ("crates/rs_cam_core/src/feeds/extrapolation/size.rs", 82,
              'const RULE_A: &str = "G1 form A: log-log interpolation between two adjacent printed sizes of one chart series"'),
    "PLGTXT": ("crates/rs_cam_core/src/feeds/plunge.rs", 49,
               'PLUNGE_BASE_RULE_TEXT = "material base 1000/h (wood), 900/h (plywood, sheet) at 6 mm, x clamp(D/6, 0.25, 3), h = (Janka/600)^0.4 (a port of the Shapeoko reference calculator)"'),
    "PLGBASE": ("crates/rs_cam_core/src/material/mod.rs", 788,
                'FormulaConstant::unsourced("material.plunge_rate_base.solid_wood", 1000.0, PLUNGE_CLAIM); reference_diameter_mm 6.0 (line 810); '
                "diameter_scale_min 0.25 (line 815); diameter_scale_max 3.0 (line 820)"),
    "PLGH": ("crates/rs_cam_core/src/material/mod.rs", 827,
             'material.feed_scale_factor.janka_reference_lbf 600.0 (line 827); material.feed_scale_factor.exponent 0.4 (line 832): "law (janka / 600)^0.4 ported from reference/ (not in the repo)"'),
    "PLGJANKA": ("crates/rs_cam_core/src/material/mod.rs", 153,
                 "WoodSpecies::WhiteOak => 1360.0 (janka_lbf); the project states [job.stock.material.SolidWood] species = \"WhiteOak\" (nz_wanaka_21_operator.toml lines 16-17)"),
    "PLGTIP": ("crates/rs_cam_core/src/tool_load/plunge_stress.rs", 21,
               "const PLUNGE_CAP_PER_MM_TIP_DIAMETER: f64 = 150.0; (comment: FSWizard / GWizard ranges, not stored in the repo)"),
    # The pass ceiling c D (operator ruling Q2 of 2026-11-09, P22_DESIGN section 9).
    "PASSLIM": ("crates/rs_cam_core/src/adaptive/mod.rs", 185,
                "pub fn pass_engagement_limit(stepover, tool_radius) = search::pass_engagement_ceiling(target_engagement_fraction(stepover, tool_radius))"),
    "PASSTOL": ("crates/rs_cam_core/src/adaptive/search.rs", 599,
                "pub(crate) const PASS_ENGAGEMENT_TOLERANCE: f64 = 0.05; (line 599; 'matches the libactp reference') "
                "pass_engagement_ceiling(target_frac) = target_frac * (1.0 + PASS_ENGAGEMENT_TOLERANCE) (lines 603-605)"),
    "PASSSW": ("crates/rs_cam_core/src/adaptive/search.rs", 895,
               "PassLoad::swept_width: ceiling = radial_woc_fraction_from_leading_arc(pass_engagement_ceiling(target_engagement_fraction(stepover, tool_radius))) (lines 895-900)"),
    "PASSARC": ("crates/rs_cam_core/src/ops/adaptive_shared.rs", 10,
                "target_engagement_fraction: alpha = acos(1 - woc / tool_radius); alpha / TAU (lines 10-14). "
                "radial_woc_fraction_from_leading_arc(f_arc) = (1 - cos(TAU * f)) / 2, f clamped to [0, 0.5] (lines 36-39)"),
    "T6CC": ("planning/rivmap_block_job/nz_wanaka_21_operator.toml", 384,
             "[tools.centre_cutting] stated = \"no\", centre_core_diameter_mm = 8.0, blade_protrusion_mm = 1.0 (tool 6, lines 384-387)"),
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


PASS_TOL = 0.05  # PASSTOL


def pass_ceiling_width(stepover, d):
    """The pass ceiling c D: the widest width the planner holds at the full feed.
    alpha/2pi = acos(1 - s/R)/2pi (PASSARC); x (1 + 0.05) (PASSTOL, PASSLIM);
    c = (1 - cos(2pi f)) / 2 (PASSARC, PASSSW); width = c D."""
    r = d / 2.0
    f = math.acos(max(-1.0, min(1.0, 1.0 - min(stepover, d) / r))) / (2 * math.pi)
    f = min(max(f * (1.0 + PASS_TOL), 0.0), 0.5)
    return (1.0 - math.cos(2 * math.pi * f)) / 2.0 * d


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
# From MCP, build 2d0f7849f (P23c), 2026-11-10. sug = get_suggest_rationale,
# shp = get_toolpath_params. chip = vendor_band.score_winner_chip_mm.
# idx 5 and 9 (shipped_power unmodeled): feed, rpm, plunge and ramp are
# basis.rpm.feed_mm_min, .value, .plunge_mm_min and .ramp_feed_mm_min (P23b).
# ae = shipped_power.case_ae_mm. The shp values are the stored params (no apply
# since the read of 2026-11-09), so they still hold the 18000 / 10610 rpm values.
APP = {
    0: dict(sug=dict(feed=8651, rpm=24000, plunge=2162, ramp=8597, preq=0.05789052125364684, pav=1.5, ae=8.0, ap=0.5, chip=0.17912843029568412),
            shp=dict(feed=6488, rpm=18000, plunge=2162, ramp=6447)),
    1: dict(sug=dict(feed=9123, rpm=24000, plunge=2162, ramp=8598, preq=0.9399861902794343, pav=1.5, ae=5.9913484744305885, ap=10.0, chip=0.17912843029568412),
            shp=dict(feed=8475, rpm=18000, plunge=2162, ramp=6448)),
    2: dict(sug=dict(feed=6841, rpm=24000, plunge=2500, ramp=6095, preq=0.13832718775276132, pav=1.5, ae=1.6380285007813598, ap=6.0, chip=0.127),
            shp=dict(feed=5279, rpm=18000, plunge=2285, ramp=4571)),
    3: dict(sug=dict(feed=2499, rpm=19685, plunge=2499, ramp=None, preq=0.15415957357877005, pav=1.2303125, ae=6.0, ap=0.1269494538989078, chip=0.0635,
                     padv=True, section="axial_circle"),
            shp=dict(feed=1000, rpm=18000, plunge=None, ramp=None)),
    4: dict(sug=dict(feed=2499, rpm=19685, plunge=2499, ramp=None, preq=0.15415957357877005, pav=1.2303125, ae=6.0, ap=0.1269494538989078, chip=0.0635,
                     padv=True, section="axial_circle"),
            shp=dict(feed=1000, rpm=18000, plunge=None, ramp=None)),
    # idx 5: shipped_power is unmodeled; feed, rpm and plunge are from basis.rpm (P23b).
    5: dict(sug=dict(feed=7161, rpm=24000, plunge=2500, ramp=None, preq=None, pav=None, ae=None, ap=None, chip=0.14919260416145616),
            shp=dict(feed=3165, rpm=10610, plunge=1347, ramp=None)),
    6: dict(sug=dict(feed=1347, rpm=24000, plunge=240, ramp=None, preq=0.005881127640829388, pav=1.5, ae=2.0, ap=0.7, chip=0.05501007821762709),
            shp=dict(feed=1010, rpm=18000, plunge=240, ramp=None)),
    7: dict(sug=dict(feed=1320, rpm=24000, plunge=240, ramp=None, preq=0.004174473137735276, pav=1.5, ae=2.0, ap=0.5, chip=0.05501007821762709),
            shp=dict(feed=1258, rpm=18000, plunge=240, ramp=None)),
    8: dict(sug=dict(feed=1320, rpm=24000, plunge=240, ramp=1320, preq=0.004174473137735276, pav=1.5, ae=2.0, ap=0.5, chip=0.05501007821762709),
            shp=dict(feed=1258, rpm=18000, plunge=240, ramp=1258)),
    # idx 9: shipped_power is unmodeled; feed, rpm and plunge are from basis.rpm (P23b).
    9: dict(sug=dict(feed=3048, rpm=24000, plunge=381, ramp=None, preq=None, pav=None, ae=None, ap=None, chip=0.127),
            shp=dict(feed=2286, rpm=18000, plunge=381, ramp=None)),
    10: dict(sug=dict(feed=3048, rpm=24000, plunge=1016, ramp=None, preq=0.03762435947088806, pav=1.5, ae=6.35, ap=2.6, chip=0.0635),
             shp=dict(feed=2286, rpm=18000, plunge=762, ramp=None)),
    11: dict(sug=dict(feed=8651, rpm=24000, plunge=2162, ramp=None, preq=0.12790170831923497, pav=1.5, ae=18.0, ap=0.5, chip=0.17912843029568412),
             shp=dict(feed=6488, rpm=18000, plunge=2162, ramp=None)),
    12: dict(sug=dict(feed=8651, rpm=24000, plunge=2162, ramp=8597, preq=0.05789052125364684, pav=1.5, ae=8.0, ap=0.5, chip=0.17912843029568412),
             shp=dict(feed=6488, rpm=18000, plunge=2162, ramp=6447)),
    13: dict(sug=dict(feed=3048, rpm=24000, plunge=1016, ramp=None, preq=0.0020036641138342754, pav=1.5, ae=6.35, ap=0.6, chip=0.0635),
             shp=dict(feed=2286, rpm=18000, plunge=762, ramp=None)),
    14: dict(sug=dict(feed=10397, rpm=24000, plunge=2162, ramp=8597, preq=0.30061307828110684, pav=1.5, ae=3.939509813596315, ap=4.166, chip=0.17912843029568412),
             shp=dict(feed=8060, rpm=18000, plunge=2162, ramp=6448)),
    15: dict(sug=dict(feed=7022, rpm=24000, plunge=2500, ramp=6095, preq=0.12135369231685468, pav=1.5, ae=1.510433184138314, ap=5.532, chip=0.127),
             shp=dict(feed=5429, rpm=18000, plunge=2285, ramp=4571)),
    16: dict(sug=dict(feed=3657, rpm=24000, plunge=240, ramp=None, preq=0.0015221251699678547, pav=1.5, ae=0.8717797887081346, ap=0.2, chip=0.0762),
             shp=dict(feed=2317, rpm=18000, plunge=240, ramp=None)),
    17: dict(sug=dict(feed=7161, rpm=24000, plunge=2500, ramp=7161, preq=0.09568610226602917, pav=1.5, ae=6.0, ap=1.25, chip=0.14919260416145616),
             shp=dict(feed=3165, rpm=10610, plunge=1582, ramp=3165)),
}

# ---------------------------------------------------------------- toolpaths
# Inputs from get_toolpath_params (op kind, stepover, depth per pass,
# trochoid_cap_mult, entry) and the project tools. No engine value.
TP = {
    0: ("Face back to T1", 6, "face", dict(ae=8.0, ap=0.5, entry="ramp")),
    1: ("Back bulk rough 18 mm", 6, "adaptive", dict(ae=5.5, ap=10.0, entry="helix", helix_r=5.4, helix_pitch=2.0)),
    2: ("Back rest 6 mm (cavity, hollow, lakes, pockets)", 0, "adaptive", dict(ae=1.5, ap=6.0, entry="ramp")),
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
    14: ("Front bulk rough 18 mm", 6, "adaptive", dict(ae=3.6, ap=4.166, entry="ramp")),
    15: ("Front 3D rough", 0, "adaptive", dict(ae=1.382, ap=5.532, entry="ramp")),
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

# U1 ruled, operator 2026-11-10: the 18 mm 2F chip is the Spektra 1/2 in 2F row (SPK37)
# scaled by the generic size law (SIZEC). 18 / 12.7 = 1.417 lies inside 0.5x-2x.
SIZE_EXP = 0.61  # SIZEC
assert 0.5 <= 18.0 / (IN / 2) <= 2.0
c_18mm = c_spk12 * (18.0 / (IN / 2)) ** SIZE_EXP

# Repo plunge rule (PLGTXT, PLGBASE, PLGH, PLGJANKA, PLGTIP); no vendor source.
# Operator rulings 2026-11-10 (U3 tools 6, 1, 4; the tool 5 plunge under U2).
JANKA_WHITE_OAK = 1360.0  # PLGJANKA
H_OAK = (JANKA_WHITE_OAK / 600.0) ** 0.4  # PLGH
PLG_BASE_6MM = 1000.0 / H_OAK  # PLGBASE


def plunge_repo_rule(d_mm, tip_d_mm=None):
    """base/h at 6 mm x clamp(D/6, 0.25, 3); the tip cap 150 x tip (ball tips only); the Z rate."""
    v = PLG_BASE_6MM * min(max(d_mm / 6.0, 0.25), 3.0)
    terms = [v, V_Z]
    if tip_d_mm is not None:
        terms.append(150.0 * tip_d_mm)
    return min(terms), v


CHIP = {
    # P2 ruled (b), operator 2026-11-08: the Onsrud 60-200 row stands for tool 0.
    0: {"Onsrud 60-200 1/4 in": (c_ons[0], c_ons[1], 'ONS46 ".005-.007" x 25.4 = %.4f-%.4f mm' % c_ons)},
    6: {"Spektra 1/2 in x size law": (c_18mm, c_18mm,
                    'U1 ruled, operator 2026-11-10. SPK37 1/2 in 2F ".0057"" = %.4f mm x (18 / 12.7)^0.61 (SIZEC) = %.5f mm. Repo size law, no vendor row at 18 mm. '
                    'It lies inside the old bracket %.4f-%.4f mm and under the Freud straight-bit band 0.229-0.330 mm '
                    '(research/large_straight_router_bits_2026-11-10.md line 119); the tool is a straight-flute router bit' % (c_spk12, c_18mm, c_spk12, c_spk19_3f))},
    5: {"Spektra 2F log-log": (c_2mm_log, c_2mm_log,
                    'U2 ruled, operator 2026-11-10. Log-log interpolation (SIZEA) between SPK12 1/16 in ".0020"" = %.4f mm and SPK17 3/32 in ".0023"" = %.4f mm at 2.0 mm = %.5f mm. '
                    'The rows are 2F; the tool is 1F; the operator accepts the 2F chip per tooth for it' % (c_116, c_332, c_2mm_log))},
    1: {"AMS 18/30 deg 1F": (c_v20[0], c_v20[1], 'AMS11 18deg and 30deg 1F both "0.003" - 0.007""; 20deg lies between equal cells: %.4f-%.4f mm' % c_v20)},
    2: {"AMS 60 deg 2F": (c_v60[0], c_v60[1], 'AMS11 60deg 2F: "90"" / (18000 x 2) x 25.4 = %.4f mm; printed "0.003"" = %.4f mm; the cell disagrees with itself' % c_v60)},
    4: {"SpeTool 2.0 mm": (c_spe2, c_spe2, 'SPE23 chipload "0.003" (inch, 2 flutes derived: 108/(18000 x 0.003) = %.2f) x 25.4 = %.4f mm. '
                           'Operator ruling 2026-11-10 ("1, yes"): for the 2 mm tapered ball the row printed at exactly that size wins over a size claim from another chart' % (108 / (18000 * 0.003), c_spe2))},
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
    chip_status = {0: "DERIVED", 6: "DERIVED", 5: "DERIVED", 1: "SOURCED", 2: "SOURCED", 4: "SOURCED"}[tid]
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
        ae_cap = pass_ceiling_width(g["ae"], d)
        la, lb = lift(g["ae"], d), lift(ae_cap, d)
        # P3: the chart chip is a peak chip; the peak occurs at the largest engagement
        # (lead 2026-11-08). Operator ruling Q2 of 2026-11-09 (P22_DESIGN section 9): that
        # width is the pass ceiling c D; trochoid_cap_mult 1.6 is retired as unsourced.
        lifts = {"ceiling ae": lb}
        lift_deriv = ("lift = 1 / (2 sqrt(r(1-r))), r = ae/D. P3 with ruling Q2 (2026-11-09): the width is the pass ceiling c D of the stepover %.3f "
                      "(clearing_strategy geodesic_adaptive): alpha = acos(1 - s/R) x 1.05 (PASSARC, PASSTOL), c = (1 - cos alpha) / 2 = %.5f, c D = %.4f mm, lift %.4f "
                      "(the nominal stepover would give lift %.4f)."
                      % (g["ae"], ae_cap / d, ae_cap, lb, la))
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
        # P4 ruled (a), operator 2026-11-10: no ball lead-angle lift ("finishing feeds are
        # already pretty high generally"). The P4b candidate (lift 1 / sin k) is deleted.
        lifts = {"no lead-angle lift": 1.0}
        lift_deriv = ("P4 ruled (a), operator 2026-11-10: no ball lead-angle lift; feed = chart chip x z x n. "
                      "Information: ball R %.1f at ap %.2f has sin k = sqrt(1-(1-ap/R)^2) = %.4f, so the peak chip is fz x %.4f, below the chart chip." % (R, g["ap"], sk, sk))

    # -------------------------------------------------------- rpm row
    pol_n = {}
    deriv_n = []
    max_lift = max(lifts.values())
    nb, nb_bind = n_b_lateral(chip_hi, z, max_lift)
    pol_n["P1b binding limit"] = nb  # P1 ruled (b), operator 2026-11-08
    chart_rpm = {0: "SPK3 18,000 RPM (Onsrud prints no RPM for 60-200: ONS61)", 6: "SPK3 18,000 RPM (the chart of the scaled row)",
                 5: "SPK3 18,000 RPM", 1: "AMS3 18,000", 2: "AMS3 18,000", 4: "SPE23 18,000"}[tid]
    deriv_n.append("P1 ruled (b), operator 2026-11-08 (the chart rpm: " + chart_rpm + ").")
    deriv_n.append("n_xy = 15000 / (chip_hi %.4f x %d x lift %.4f) = %.0f; spindle max 24000; n_b = %.0f (%s)."
                   % (chip_hi, z, max_lift, V_XY / (chip_hi * z * max_lift), nb, nb_bind))
    deriv_n.append("Power does not bind: P_req and P_avail both scale with n (constant torque below 24000 rpm, MACH), so the ratio holds at every n.")
    app_rpm_val = a["sug"]["rpm"]  # basis.rpm.value on every op (P23b); no fallback to the stored param
    row(idx, "rpm", "DERIVED", nb_bind,
        {k: (v, v) for k, v in pol_n.items()}, " ".join(deriv_n),
        app_val=app_rpm_val, unit="rpm")
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
    feed_status = "DERIVED"  # P4 ruled (a), operator 2026-11-10
    deriv_f = "feed = chip x z x n x lift (SPK47), clamped to XY 15000. " + lift_deriv
    if tid == 1 or tid == 2:
        deriv_f += " Printed IPM: " + ("50\"-130\" = 1270-3302 mm/min at 18000." if tid == 1 else "90\" = 2286 mm/min at 18000.")
    app_feed = a["sug"]["feed"]  # basis.rpm.feed_mm_min where shipped_power is unmodeled (P23b); no fallback
    if tid == 1:
        if n_app == 18000:
            gfeed = {"@app": (min(gfeed["@app"][0], 50 * IN), max(gfeed["@app"][1], 130 * IN))}
    row(idx, "feed", feed_status, "chart chip x lift; XY 15000", cands, deriv_f,
        app_val=app_feed, grade_cands=gfeed, n_app=n_app)
    ROWS[-1]["app_shipped"] = a["shp"]["feed"]
    if idx == 6:
        ROWS[-1]["note"] += (" With the 2F log-log chip %.5f in a slot the feed is %.0f at the app rpm; the app's %d is %+.1f %%: it applies the ae 0.8 lift in a pocket that slots."
                             % (c_2mm_log, c_2mm_log * n_app, app_feed, (app_feed / (c_2mm_log * n_app) - 1) * 100))
    if idx == 1:
        p8475 = p_flat(8475, 18000, ae_cap, g["ap"], d, z)[0]
        ROWS[-1]["note"] += " Shipped 8475 (at 18000 rpm) is the operator value: P at the ceiling width %.2f mm = %.3f kW = %.1f %% of 1.125 kW." % (ae_cap, p8475, p8475 / 1.125 * 100)

    # -------------------------------------------------------- plunge row
    if tid == 0:
        rd = 90 * IN
        pc = {"Z-bound": (min(c_spk6 * N_MAX, V_Z), min(c_spk6 * N_MAX, V_Z))}
        pa = min(c_spk6 * n_app, V_Z)
        row(idx, "plunge", "DERIVED", "Z rate 2500", pc,
            "SPK26 Ramp Down 90\" = %.1f mm/min at 18000 (SPK52: Ramp Down = Feed / flutes = n x chip; the Spektra 6 mm chip 0.127 sets the plunge, P2 rules the side chip only). P1 (b): 0.127 x 24000 = %.0f > Z 2500 -> 2500. At the app rpm: 0.127 x n." % (rd, c_spk6 * N_MAX),
            app_key="plunge", grade_cands={"@app": (pa, pa)}, n_app=n_app,
            note=("Same tool as idx %d, but a different Suggest plunge (%s vs %s): the two ops disagree (check C2)."
                  % (22 - idx, fmt(APP[5]["sug"]["plunge"]), fmt(APP[17]["sug"]["plunge"]))
                  if idx in (5, 17) and APP[5]["sug"]["plunge"] != APP[17]["sug"]["plunge"] else ""))
    elif tid in (6, 5):
        pv, raw = plunge_repo_rule(d)
        row(idx, "plunge", "DERIVED", "repo rule, no vendor source", {"repo plunge rule": (pv, pv)},
            "Repo rule, no vendor source (operator ruling 2026-11-10: %s). "
            "PLGTXT: base 1000 / h at 6 mm x clamp(D/6, 0.25, 3), h = (Janka / 600)^0.4. Janka 1360 (PLGJANKA): h = %.4f; 1000 / h = %.1f; "
            "x clamp(%.1f / 6) = %.4f -> %.1f mm/min; min with the Z rate 2500 (MACH) = %.1f. The rule does not read the RPM."
            % ("U3 tool 6, accepted as the repo rule" if tid == 6 else "U2, the operator keeps the slow plunge; the n x chip candidate is deleted",
               H_OAK, PLG_BASE_6MM, d, min(max(d / 6.0, 0.25), 3.0), raw, pv)
            + (" Tool 6 is not centre-cutting (T6CC); the rule does not read that fact." if tid == 6 else ""),
            app_key="plunge",
            note=("Straight plunge entry at ap 0.5 <= protrusion 1.0: geometrically possible." if idx == 11 else ""))
    elif tid == 2:
        f_lo, f_hi = c_v60[0] * 2 * 18000, c_v60[1] * 2 * 18000
        pc = {"IDC ratio x feed (P1 b)": (c_v60[0] * 2 * N_MAX / 3, c_v60[1] * 2 * N_MAX / 2)}
        row(idx, "plunge", "DERIVED", "cross-vendor ratio", pc,
            "No Amana plunge for this bit. IDC 60deg 2F V-bit prints plunge/feed 20/40 = 0.5 (IDC165) and 20/60 = 0.333 (IDC285); the two printings disagree. "
            "Range = [feed_lo/3, feed_hi/2]: a [%.0f/3, %.0f/2]." % (f_lo, f_hi),
            app_key="plunge", n_app=n_app,
            grade_cands={"@app": (c_v60[0] * 2 * n_app / 3, c_v60[1] * 2 * n_app / 2)})
    else:  # tid 1 (V20), 4 (tapered)
        tip = 2.0 * g["R"] if tid == 4 else None  # the app states a tip cap on the tapered ball only
        pv, raw = plunge_repo_rule(d, tip)
        row(idx, "plunge", "DERIVED", "repo rule, no vendor source", {"repo plunge rule": (pv, pv)},
            "Repo rule, no vendor source (operator ruling 2026-11-10: \"for the no source ones, if they look right, let's go for it\"). "
            "PLGTXT: 1000 / h at 6 mm x clamp(D/6, 0.25, 3); h = %.4f (Janka 1360, PLGJANKA); x clamp(%.3f / 6) = %.4f -> %.1f mm/min; Z rate 2500 (MACH)%s; min = %.1f."
            % (H_OAK, d, min(max(d / 6.0, 0.25), 3.0), raw,
               ("; tip cap 150 x tip %.1f mm = %.0f mm/min (PLGTIP)" % (tip, 150.0 * tip)) if tip is not None else "", pv)
            + (" No repo source prints a plunge for a 20deg 1F V-bit." if tid == 1 else " SpeTool (SPE23) and Amana ZrN print no plunge."),
            app_key="plunge")

    # -------------------------------------------------------- ramp row
    if g.get("entry") in ("ramp", "helix"):  # H1 ruled "yes", operator 2026-11-10: a helix entry feed takes the ramp rule
        def ramp_cands(pns):
            out = {}
            for (pn, n), (cn, cv) in itertools.product(pns.items(), chips.items()):
                key = pn.split(" ")[0] + (("/" + cn.split(" ")[0]) if len(chips) > 1 else "")
                out[key] = (min(cv[0] * z * n, V_XY), min(cv[1] * z * n, V_XY))
            return out
        rc = ramp_cands(pol_n)
        grc = ramp_cands({"@app": n_app})
        st = "DERIVED"
        vert = ""
        if tid == 0:
            vert = " Vertical part: 4572 x sin 3deg = %.0f mm/min < Ramp Down 2286: the side chip binds." % (4572 * math.sin(math.radians(3)))
        if g.get("entry") == "helix":
            hr, hp = g["helix_r"], g["helix_pitch"]
            vert += (" The entry is a helix (r = 0.3 x D = %.2f mm, pitch %.2f mm, slope %.2f deg). H1 ruled \"yes\", operator 2026-11-10: the ramp rule grades a helix entry feed. "
                     "The path radius is less than the tool radius %.1f mm, so the contact is full width."
                     % (hr, hp, math.degrees(math.atan(hp / (2 * math.pi * hr))), d / 2))
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
       'graded_build = "2d0f7849f"',
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
