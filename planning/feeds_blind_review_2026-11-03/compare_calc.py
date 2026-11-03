#!/usr/bin/env python3
"""Phase 3 compare: reviewer ranges vs rs_cam Suggest vs the operator's values.

Run: python3 -I compare_calc.py   (from any directory)

It reads suggest/suggest_operator.jsonl (Suggest and the operator's current values),
holds the reviewer ranges transcribed from review_A.md, review_B.md, review_C.md,
recomputes every chip load and every power figure, asserts each claim that
compare.md makes, and writes compare.json and compare.md.
"""
import json
import math
import os

HERE = os.path.dirname(os.path.abspath(__file__))
SUGGEST = os.path.join(HERE, "suggest", "suggest_operator.jsonl")

# ---------------------------------------------------------------- constants
# Machine (brief 00, MachineProfile::power_at_rpm): P(n) = 1.5 kW x min(n, 24000) / 24000.
P_RATED_KW = 1.5
N_RATED = 24000.0
T_RATED_NM = P_RATED_KW * 1000 / (2 * math.pi * N_RATED / 60)  # 0.597 N m
RUBBING_FLOOR_MM = 0.025  # Shaw, sources.toml shaw_metal_cutting
TOL = 0.05  # a value within 5 % of a range end counts as inside (rounding of feeds and rpm)

# rs_cam's oak force line (crates/rs_cam_core/src/material/force_line.rs):
# Curti 2021 helix-0 envelope, Ks_n 0.0768333, Int_n 0.0060333 (derived in the source file
# from the printed quadratics), rho = SG 0.68 x 1000 x 1.12 (FPL Table 5-3a, white oak).
RHO_OAK = 0.68 * 1000 * 1.12
KS_N, INT_N = 0.0768333, 0.0060333
KS = KS_N * RHO_OAK          # N/mm^2
F_EDGE = INT_N * RHO_OAK     # N/mm

# Guedes et al. 2020 (Maderas 22(4):477-482), read 2026-11-03 from the redalyc PDF:
# Bosch GOF 1300 CE router, 6 mm cutter, feed 0.70 m/min, cutting speed 3.77 m/s,
# slot 6 mm wide x 3 mm deep, "active power ... required by the router" from a Fluke 435
# on the router leads. Goupia glabra (basic density 717): 261 (saturated) to 513 (anhydrous).
G_D, G_VC, G_F, G_AP, G_AE = 6.0, 3.77, 700.0, 3.0, 6.0
G_N = G_VC * 60000 / (math.pi * G_D)        # rpm, = 12000
G_Z_ASSUMED = 2                               # the paper does not state the flute count
G_FZ = G_F / (G_N * G_Z_ASSUMED)
G_HM = G_FZ * 2 / math.pi                     # mean chip of a full slot (psi = pi)
G_U_HIGH, G_U_LOW = 0.513, 0.261              # J/mm^3 (see the unit check)
U_A = 0.50                                    # reviewer A design value
M_EXPONENTS = (0.3, 0.5)                      # judgement: u ~ h^-m between the two chips


def power_available_kw(n):
    return P_RATED_KW * min(n, N_RATED) / N_RATED


def fz(feed, n, z):
    return feed / (n * z)


def radial_thin(fz_mm, ae, d):
    """DAPRA: hex = fz x 2 sqrt(r - r^2) for r = ae/D < 0.5, else fz."""
    r = ae / d
    return fz_mm * 2 * math.sqrt(r - r * r) if r < 0.5 else fz_mm


def psi(ae, d):
    return math.acos(max(-1.0, min(1.0, 1 - 2 * ae / d)))


def mean_chip(fz_mm, ae, d):
    p = psi(ae, d)
    return fz_mm * (1 - math.cos(p)) / p


def power_models(ap, ae, d, feed, n, z):
    """Spindle power (kW) of one cut under four energy assumptions."""
    mrr_s = ap * ae * feed / 60.0  # mm^3/s
    f = fz(feed, n, z)
    hm = mean_chip(f, ae, d)
    p = psi(ae, d)
    vc = math.pi * d * n  # mm/min
    curti = (KS * ap * ae * feed + F_EDGE * ap * vc * z * p / (2 * math.pi)) / 60e6
    out = {
        "mrr_mm3_s": mrr_s,
        "fz_mm": f,
        "mean_chip_mm": hm,
        "available_kw": power_available_kw(n),
        "A_flat_u050_kw": U_A * mrr_s / 1000,
        "A_flat_u026_kw": 0.26 * mrr_s / 1000,
        "rs_cam_curti_kw": curti,
        "rs_cam_curti_u_J_mm3": curti * 1000 / mrr_s,
    }
    for m in M_EXPONENTS:
        u = U_A * (hm / G_HM) ** (-m)
        out[f"guedes_chip_m{m}_u_J_mm3"] = u
        out[f"guedes_chip_m{m}_kw"] = u * mrr_s / 1000
    for k in list(out):
        if k.endswith("_kw") and k != "available_kw":
            out[k.replace("_kw", "_pct")] = 100 * out[k] / out["available_kw"]
    return out


# ------------------------------------------------------------ energy checks
def energy_checks():
    res = {}
    # (a) unit: kJ/cm^3 or J/cm^3. MRR of the Guedes cut:
    g_mrr_cm3_s = G_F * G_AP * G_AE / 60 / 1000
    res["guedes_mrr_cm3_s"] = g_mrr_cm3_s
    res["guedes_power_if_J_per_cm3_W"] = G_U_HIGH * 1000 * g_mrr_cm3_s / 1000 * 1000 / 1000 * 1000 / 1000
    # simpler, explicit:
    res["guedes_power_if_J_per_cm3_W"] = 513 * g_mrr_cm3_s
    res["guedes_power_if_kJ_per_cm3_W"] = 513e3 * g_mrr_cm3_s
    res["guedes_rpm"] = G_N
    res["guedes_fz_2flute_mm"] = G_FZ
    res["guedes_mean_chip_2flute_mm"] = G_HM
    res["guedes_mean_chip_1flute_mm"] = 2 * G_HM
    # (d) rs_cam's line at the Guedes chip: does it reproduce Guedes?
    res["curti_u_at_guedes_chip_J_mm3"] = (KS + F_EDGE / G_HM) / 1000
    res["curti_u_at_guedes_chip_1flute_J_mm3"] = (KS + F_EDGE / (2 * G_HM)) / 1000
    res["ks_n_mm2"] = KS
    res["f_edge_n_mm"] = F_EDGE
    return res


EC = energy_checks()
assert abs(EC["guedes_rpm"] - 12000) < 5, EC["guedes_rpm"]
assert 100 < EC["guedes_power_if_J_per_cm3_W"] < 115          # J/cm^3 gives about 108 W
assert EC["guedes_power_if_kJ_per_cm3_W"] > 1300 * 50           # kJ/cm^3 gives about 108 kW
assert 0.025 < G_FZ < 0.032 and 0.017 < G_HM < 0.020
# rs_cam's line at Guedes' chip lands inside Guedes' Goupia span 0.261-0.513 J/mm^3.
assert G_U_LOW < EC["curti_u_at_guedes_chip_J_mm3"] < G_U_HIGH, EC["curti_u_at_guedes_chip_J_mm3"]

# ------------------------------------------------------------------ inputs
SUG = {}
with open(SUGGEST) as fh:
    for line in fh:
        d = json.loads(line)
        SUG[d["index"]] = d

# Tool geometry (project [[tools]] tables).
TOOLS = {
    "flat18": dict(d=18.0, z=2, label="18 mm 2F bulk bit, non-centre-cutting (core 8, protrusion 1: estimates)"),
    "flat6": dict(d=6.0, z=2, label="6 mm 2F flat, 30 mm stick-out, centre cutting not confirmed"),
    "dmm2": dict(d=2.0, z=1, label="DMM 2 mm 1F, 8 mm flute, 20 mm stick-out, centre cutting not known"),
    "v20": dict(d=3.175, z=1, half=10.0, label="3.175 mm 20 deg V, 0.3 mm flat tip, 1F (project), 25 mm stick-out"),
    "v60": dict(d=6.35, z=2, half=30.0, label="1/4 in 60 deg V, 2F, 30 mm stick-out"),
    "r1": dict(d=2.0, z=2, R=1.0, alpha=5.7106, label="R1 x 6 x 20 2F tapered ball, 30 mm stick-out"),
}


def taper_ball_de(ap, R=1.0, alpha_deg=5.7106):
    a = math.radians(alpha_deg)
    ht = R * (1 - math.sin(a))
    if ap <= ht:
        return 2 * math.sqrt(ap * (2 * R - ap))
    return 2 * (R * math.cos(a) + (ap - ht) * math.tan(a))


# Reviewer ranges per op. Each field: (min, max, unit, source, note). None = not comparable.
# Values transcribed from the review tables; the source column names the review and table.
F = {}


def rv(lo, hi, unit, src, note=""):
    return dict(lo=lo, hi=hi, unit=unit, src=src, note=note)


R_FACE = {
    "rpm": rv(16000, 18000, "rpm", "review_A 01"),
    "feed": rv(4480, 6000, "mm/min", "review_A 01"),
    "fz": rv(0.14, 0.167, "mm/tooth", "review_A 01", "ae 0.8 D: no thinning, hex = fz"),
    "hex": rv(0.14, 0.167, "mm", "review_A 01"),
    "plunge": rv(None, None, "mm/min", "review_A 01", "no straight plunge into wood; start rows in air"),
    "stepover": rv(12.0, 14.4, "mm", "review_A 01"),
    "depth_per_pass": rv(0.5, 1.0, "mm", "review_A 01", "1.0 only if the board is cupped"),
    "ramp_angle": rv(2.0, 3.0, "deg", "review_A entry rule", "2 deg until the core and protrusion are measured"),
}
REV = {
    0: R_FACE,
    12: R_FACE,
    1: {
        "rpm": rv(20000, 24000, "rpm", "review_A 02"),
        "feed": rv(3200, 4416, "mm/min", "review_A 02 options A", "at ae 3.6; at ae 2.7 the range is 4000-6000"),
        "fz": rv(0.080, 0.092, "mm/tooth", "review_A 02 options A", "at ae 3.6 (80 % torque at u 0.50)"),
        "hex": rv(0.064, 0.074, "mm", "review_A 02 options A"),
        "plunge": rv(None, None, "mm/min", "review_A 02", "no straight plunge (non-centre-cutting)"),
        "stepover": rv(2.7, 3.6, "mm", "review_A 02", "3.6 only at <= 4400 mm/min"),
        "depth_per_pass": rv(9.0, 9.0, "mm", "review_A 02"),
        "ramp_angle": rv(2.0, 3.0, "deg", "review_A entry rule"),
    },
    2: {
        "rpm": rv(18000, 20000, "rpm", "review_A 03"),
        "feed": rv(2250, 4000, "mm/min", "review_A 03"),
        "fz": rv(0.0625, 0.10, "mm/tooth", "review_A 03"),
        "hex": rv(0.05, 0.08, "mm", "review_A 03"),
        "plunge": rv(720, 1080, "mm/min", "review_A 03", "only if centre cutting is confirmed"),
        "stepover": rv(1.2, 1.8, "mm", "review_A 03"),
        "depth_per_pass": rv(1.2, 1.2, "mm", "review_A 03", "set by the V20 budget"),
        "ramp_angle": rv(3.0, 3.0, "deg", "review_A 03 (judgement)"),
    },
    3: {
        "rpm": rv(16000, 18000, "rpm", "review_A 04"),
        "feed": rv(720, 1080, "mm/min", "review_A 04", "the drill feed is the plunge"),
        "fz": rv(0.02, 0.03, "mm/tooth axial", "review_A 04"),
        "plunge": rv(720, 1080, "mm/min", "review_A 04"),
        "depth_per_pass": rv(3.0, 6.0, "mm (peck)", "review_A 04", "6, 6, 4, 4, 3 ... with a full retract (judgement)"),
    },
    4: {
        "rpm": rv(18000, 18000, "rpm", "review_A 05"),
        "feed": rv(720, 1080, "mm/min", "review_A 05"),
        "fz": rv(0.02, 0.03, "mm/tooth axial", "review_A 05"),
        "plunge": rv(720, 1080, "mm/min", "review_A 05"),
        "depth_per_pass": rv(4.0, 8.0, "mm (peck)", "review_A 05"),
        "dwell": rv(0.0, 0.1, "s", "review_A 05"),
    },
    5: {
        "rpm": rv(18000, 18000, "rpm", "review_A 06"),
        "feed": rv(1260, 1620, "mm/min", "review_A 06", "one pass up to 10 mm; levels of <= 5 mm allow 2160-2880"),
        "fz": rv(0.035, 0.045, "mm/tooth", "review_A 06", "full slot, no thinning"),
        "hex": rv(0.035, 0.045, "mm", "review_A 06"),
        "plunge": rv(None, 720, "mm/min", "review_A 06", "<= 720 at 18000 (0.02 mm/tooth axial)"),
        "plunge_fz": rv(None, 0.02, "mm/tooth axial", "review_A 06"),
    },
    11: {
        "rpm": rv(18000, 20000, "rpm", "review_A 11"),
        "feed": rv(3240, 4000, "mm/min", "review_A 11"),
        "fz": rv(0.09, 0.10, "mm/tooth", "review_A 11"),
        "hex": rv(0.088, 0.098, "mm", "review_A 11"),
        "stepover": rv(2.4, 3.0, "mm", "review_A 11"),
        "depth_per_pass": rv(0.5, 0.5, "mm", "review_A 11"),
        "ramp_angle": rv(3.0, 3.0, "deg", "review_A 11 (judgement)"),
    },
    14: {
        "rpm": rv(18000, 18000, "rpm", "review_A 13"),
        "feed": rv(4500, 6000, "mm/min", "review_A 13"),
        "fz": rv(0.125, 0.167, "mm/tooth", "review_A 13"),
        "hex": rv(0.10, 0.133, "mm", "review_A 13"),
        "plunge": rv(None, None, "mm/min", "review_A 13", "no straight plunge"),
        "stepover": rv(3.6, 3.6, "mm", "review_A 13"),
        "depth_per_pass": rv(4.17, 4.17, "mm", "review_A 13"),
        "ramp_angle": rv(2.0, 3.0, "deg", "review_A entry rule"),
    },
    15: {
        "rpm": rv(18000, 20000, "rpm", "review_A 14"),
        "feed": rv(2160, 3800, "mm/min", "review_A 14"),
        "fz": rv(0.060, 0.095, "mm/tooth", "review_A 14"),
        "hex": rv(0.051, 0.080, "mm", "review_A 14"),
        "plunge": rv(720, 720, "mm/min", "review_A 14", "if centre cutting is confirmed"),
        "stepover": rv(1.38, 1.8, "mm", "review_A 14"),
        "depth_per_pass": rv(2.77, 5.53, "mm", "review_A 14"),
        "ramp_angle": rv(3.0, 3.0, "deg", "review_A 14 (judgement)"),
    },
    18: {
        "rpm": rv(18000, 20000, "rpm", "review_A 17"),
        "feed": rv(2880, 4000, "mm/min", "review_A 17", "at 1.5 mm levels"),
        "fz": rv(0.08, 0.10, "mm/tooth", "review_A 17", "full slot, no thinning"),
        "hex": rv(0.08, 0.10, "mm", "review_A 17"),
        "depth_per_pass": rv(1.5, 3.0, "mm", "review_A 17", "3.0 only at 2160-2700 mm/min"),
        "ramp_angle": rv(3.0, 3.0, "deg", "review_A 17 (judgement)"),
        "ramp_feed": rv(2880, 4000, "mm/min", "review_A 17", "ramp at the cutting feed"),
    },
    9: {
        "rpm": rv(16000, 18000, "rpm", "review_B op 9"),
        "feed": rv(630, 1026, "mm/min", "review_B op 9", "at 18000 rpm, 1 flute"),
        "fz": rv(0.035, 0.057, "mm/tooth", "review_B op 9", "per tooth; the same at every point of the V edge"),
        "hex": rv(0.034, 0.056, "mm", "review_B op 9", "flank chip h = fz cos 10 deg"),
        "plunge": rv(300, 600, "mm/min", "review_B op 9", "into the 4 mm web"),
    },
    10: {
        "rpm": rv(16000, 18000, "rpm", "review_B op 10"),
        "feed": rv(1800, 2736, "mm/min", "review_B op 10", "at 18000 rpm"),
        "fz": rv(0.050, 0.076, "mm/tooth", "review_B op 10"),
        "hex": rv(0.043, 0.066, "mm", "review_B op 10", "flank chip h = fz cos 30 deg"),
        "plunge": rv(600, 900, "mm/min", "review_B op 10"),
        "depth_per_pass": rv(2.598, 3.67, "mm", "review_B op 10", "first-pass depth of options A-D"),
    },
    13: {
        "rpm": rv(16000, 18000, "rpm", "review_B op 13"),
        "feed": rv(1400, 2200, "mm/min", "review_B op 13"),
        "fz": rv(0.040, 0.060, "mm/tooth", "review_B op 13"),
        "hex": rv(0.035, 0.052, "mm", "review_B op 13", "fz cos 30 deg"),
        "plunge": rv(600, 900, "mm/min", "review_B op 13"),
        "depth_per_pass": rv(0.6, 0.6, "mm", "review_B op 13"),
    },
    6: None,  # filled below (same as 7)
    16: {
        "rpm": rv(20000, 24000, "rpm", "review_C 15"),
        "feed": rv(1570, 2620, "mm/min", "review_C 15", "at 24000 rpm"),
        "fz": rv(0.033, 0.055, "mm/tooth", "review_C 15", "h x RCTF 1.093 at ae 0.66 on De 2.21"),
        "hex": rv(0.030, 0.050, "mm", "review_C 15", "h_max at De 2.21 (taper at ap 2.0)"),
        "plunge": rv(500, 800, "mm/min", "review_C 15"),
        "stepover": rv(0.66, 1.0, "mm", "review_C 15"),
        "depth_per_pass": rv(1.0, 2.0, "mm", "review_C 15"),
        "ramp_angle": rv(None, 5.0, "deg", "review_C 15"),
    },
    17: {
        "rpm": rv(18000, 20000, "rpm", "review_C 16"),
        "feed": rv(1500, 2200, "mm/min", "review_C 16"),
        "fz": rv(0.040, 0.052, "mm/tooth", "review_C 16", "h / 0.866 (ball at 0.5 mm stock, De 1.73)"),
        "hex": rv(0.035, 0.045, "mm", "review_C 16", "h_max at De 1.73"),
        "stepover": rv(0.87, 0.87, "mm", "review_C 16", "scallop 0.1 mm on flats"),
    },
}
R_DMM = {
    "rpm": rv(18000, 24000, "rpm", "review_C 07", "24000 preferred"),
    "feed": rv(720, 1080, "mm/min", "review_C 07", "at 24000 rpm"),
    "fz": rv(0.030, 0.045, "mm/tooth", "review_C 07"),
    "hex": rv(0.030, 0.045, "mm", "review_C 07", "h_max; ae 1.0 = 50 % D, no thinning"),
    "plunge": rv(250, 400, "mm/min", "review_C 07", "only if centre cutting is confirmed"),
    "stepover": rv(1.0, 1.0, "mm", "review_C 07"),
    "depth_per_pass": rv(0.5, 1.0, "mm", "review_C 07"),
    "ramp_angle": rv(None, 3.0, "deg", "review_C 07"),
}
REV[6] = R_DMM
REV[7] = R_DMM
REV[8] = dict(R_DMM, feed=rv(720, 960, "mm/min", "review_C 08"), fz=rv(0.030, 0.040, "mm/tooth", "review_C 08"),
              hex=rv(0.030, 0.040, "mm", "review_C 08"))

GROUP = {0: "A", 1: "A", 2: "A", 3: "A", 4: "A", 5: "A", 11: "A", 12: "A", 14: "A", 15: "A", 18: "A",
         9: "B", 10: "B", 13: "B", 6: "C", 7: "C", 8: "C", 16: "C", 17: "C"}
TOOLKEY = {0: "flat18", 1: "flat18", 12: "flat18", 14: "flat18", 2: "flat6", 3: "flat6", 4: "flat6",
           5: "flat6", 11: "flat6", 15: "flat6", 18: "flat6", 6: "dmm2", 7: "dmm2", 8: "dmm2",
           9: "v20", 10: "v60", 13: "v60", 16: "r1", 17: "r1"}

# Values the jsonl does not hold (project file, read only).
EXTRA_OP = {  # operator values from nz_wanaka_21_operator.toml
    0: dict(ramp_angle=3.0), 12: dict(ramp_angle=3.0), 1: dict(ramp_angle=3.0), 14: dict(ramp_angle=3.0),
    2: dict(ramp_angle=3.0), 15: dict(ramp_angle=3.0), 16: dict(ramp_angle=3.0), 18: dict(ramp_angle=3.0, ramp_feed=275.333),
    11: dict(ramp_angle=None), 8: dict(ramp_angle=3.0), 6: dict(ramp_angle=None), 7: dict(ramp_angle=None),
    3: dict(depth_per_pass=6.0), 4: dict(depth_per_pass=6.0, dwell=0.5),
}
# Suggest never writes the entry style or the angle (feeds/CLAUDE.md, Q11), the peck or the dwell.
EXTRA_SUG = {k: dict(v) for k, v in EXTRA_OP.items()}
EXTRA_SUG[18]["ramp_feed"] = 5999.0  # rationale: ramp feed 5999 mm/min
# Correction 2026-11-03 (rootcause/04): Suggest DOES write the drill peck: 15 mm for ops 3 and 4
# (feeds/suggest/apply.rs, white oak 5 x D / 2). The suggest CLI does not print the peck.
EXTRA_SUG[3]["depth_per_pass"] = 15.0
EXTRA_SUG[4]["depth_per_pass"] = 15.0

# Planned engagement for the chip computation (ae, ap), from the briefs and the project.
ENG_OP = {0: (14.4, 0.5), 12: (14.4, 0.5), 1: (3.6, 9.0), 14: (3.6, 4.1667), 2: (1.2, 1.2), 15: (1.382, 5.532),
          5: (6.0, 10.02), 11: (2.4, 0.5), 18: (6.0, 1.5), 6: (1.0, 0.5), 7: (1.0, 0.5), 8: (1.0, 0.5),
          16: (0.66, 2.0), 17: (0.8718, 0.5)}

# Manual verdicts (judgement, with the reason) for each field that is not plain "agree -> safe".
# key: (op, field, "op"|"sug") -> (risk, reason)
RISK = {}


def setr(op, field, who, risk, reason):
    RISK[(op, field, who)] = (risk, reason)


# ---- op 0 / 12 face
for o in (0, 12):
    for w in ("op", "sug"):
        setr(o, "fz", w, "safe", "0.185 mm is 11 % over the reviewer band but under the Freud carbide-tipped 5/8 in band 0.203-0.305; power about 3 % (rs_cam line)")
        setr(o, "hex", w, "safe", "as fz (no thinning at ae 0.8 D)")
        setr(o, "ramp_angle", w, "risky", "3 deg is safe only if the protrusion is >= 0.42 mm (8 mm core); both values are estimates")
        setr(o, "rpm", w, "safe", "within 1.4 % of the range")
    setr(o, "stepover", "sug", "risky", "time: 5.487 mm makes 2.6 x the rows of 14.4 mm; no limit asks for it")
    setr(o, "depth_per_pass", "sug", "risky", "time: two 0.25 mm passes for a 0.5 mm face; a 0.25 mm skim can miss a cupped board")
# ---- op 1 back bulk
for w in ("op", "sug"):
    setr(1, "rpm", w, "risky", "16219 rpm gives 1.01 kW; the 2026-11-02 sim found a 1.18 kW peak at this rpm")
    setr(1, "feed", w, "dangerous", "power over the curve: sim peak 1.18 kW > 1.01 kW (rs_cam line); see the energy check")
    setr(1, "fz", w, "dangerous", "as feed")
    setr(1, "hex", w, "dangerous", "as feed")
    setr(1, "ramp_angle", w, "risky", "3 deg vs atan(p/core) with estimated core and protrusion")
setr(1, "depth_per_pass", "sug", "dangerous", "14.047 mm: 1.6 x the planned 9 mm level, 70 % of the flute; nominal power 52 % on the rs_cam line, 109-144 % on the chip-corrected Guedes line")
setr(1, "stepover", "sug", "safe", "3.16 mm is inside 2.7-3.6")
# ---- op 2 back rest
for w in ("op", "sug"):
    setr(2, "feed", w, "dangerous", "the 14.5 mm full-slot entry (rs_cam's own entry-load critical, 2026-11-02) at fz 0.131: 69 % power on the rs_cam line, 194-304 % on the Guedes lines, bending stress about 525 MPa")
    setr(2, "fz", w, "dangerous", "as feed")
    setr(2, "hex", w, "risky", "hex 0.105 at the planned 1.2 x 1.2 is 31 % over the band; the planned cut itself is light (about 6 % power)")
    setr(2, "plunge", w, "risky", "2360 mm/min with centre cutting not confirmed; the adaptive ramps (3 deg), so a straight plunge is rare")
setr(2, "depth_per_pass", "sug", "risky", "4.702 mm breaks the V20 budget (a terrace step must not take the V past its cone); finish and geometry, not load")
setr(2, "stepover", "sug", "risky", "time: 0.94 mm adds rings; no limit asks for it")
# ---- op 3 pin drill
setr(3, "fz", "op", "safe", "0.0347 mm axial, 16 % over the reviewer band, under the Amana ramp-down 0.0635")
setr(3, "feed", "op", "safe", "as fz")
setr(3, "plunge", "op", "safe", "as fz")
setr(3, "rpm", "op", "safe", "the chip, not the rpm, matters for a plunge; 10610 rpm gives 0.80 kW available")
setr(3, "rpm", "sug", "safe", "as operator")
for f in ("fz", "feed", "plunge"):
    setr(3, f, "sug", "risky", "0.0655 mm/tooth axial (2.2 x the band) for a 30 mm hole with a 25 mm flute: chips pack below 25 mm")
setr(3, "depth_per_pass", "op", "dangerous", "peck 6 mm to 30 mm: the hole (30 mm) is deeper than the flute (25 mm) and equals the stick-out (30 mm); collet nut at the stock top")
setr(3, "depth_per_pass", "sug", "dangerous", "Suggest writes a 15 mm peck (rootcause/04); the 30 mm hole stays deeper than the 25 mm flute")
# ---- op 4 light holes
for w in ("op", "sug"):
    for f in ("fz", "feed", "plunge"):
        setr(4, f, w, "safe", "0.0655 mm/tooth axial: 2.2 x the reviewer band but 3 % over the printed Amana ramp-down 0.0635; 8 mm = 1.33 D")
    setr(4, "rpm", w, "safe", "the chip, not the rpm, matters")
    setr(4, "dwell", w, "safe", "no effect: the peck cycle emits no G4 dwell (rootcause/04, gcode/mod.rs:515)")
setr(4, "depth_per_pass", "sug", "risky", "Suggest writes a 15 mm peck: one 13 mm plunge per hole, chips pack (rootcause/04)")
# ---- op 5 river channel
setr(5, "rpm", "op", "safe", "10610 rpm: the chip 0.039 mm is in the band; only the feed (time) is lower")
setr(5, "feed", "op", "safe", "the chip is in the band; time only")
setr(5, "plunge", "op", "risky", "straight descents up to 10 mm at 0.0347 mm/tooth axial with centre cutting not confirmed")
setr(5, "plunge_fz", "op", "risky", "as plunge")
for f in ("rpm", "feed", "fz", "hex"):
    setr(5, f, "sug", "dangerous", "full slot up to 10 mm at fz 0.382: peak tooth torque about 0.81 N m > the 0.597 N m rated; mean power 95 % on the rs_cam line, 283 % on the Guedes line; bending about 875 MPa")
setr(5, "plunge", "sug", "dangerous", "2999 mm/min straight descents at 7843 rpm = 0.191 mm/tooth axial, 3 x the Amana ramp-down 0.0635")
setr(5, "plunge_fz", "sug", "dangerous", "as plunge")
# ---- ops 6, 7, 8 DMM
for o in (6, 7, 8):
    for w in ("op", "sug"):
        setr(o, "fz", w, "risky", "tool life and breakage margin of a 2 mm 1F at 10 x D stick-out; the chip is inside the undrated vendor band 0.038-0.089 but over the x0.70 projection derate")
        setr(o, "hex", w, "risky", "as fz")
        setr(o, "feed", w, "risky", "as fz")
    setr(o, "stepover", "sug", "risky", "time: 0.47 mm doubles the rings")
    setr(o, "depth_per_pass", "sug", "risky", "time: 0.335 mm adds levels")
for o in (6, 7):
    for w in ("op", "sug"):
        setr(o, "ramp_angle", w, "dangerous", "no entry dressup: a straight 0.5 mm plunge per level into fresh oak; centre cutting of the DMM is not known")
for w in ("op", "sug"):
    setr(8, "ramp_angle", w, "risky", "all 10 levels ramp at 3 deg (rootcause/05: the rough-score '9 plunges' was a counter error); centre cutting not known")
# ---- op 9 V20
for w in ("op", "sug"):
    for f in ("feed", "fz", "hex"):
        setr(9, f, w, "risky", "tip breakage and line shift: 2.2 x the reviewer band; inside the Amana 18 deg 1F hardwood band after the 2 x D derate (0.057-0.133)")
# ---- op 10 V60 slit
for f in ("rpm", "feed", "fz", "hex"):
    setr(10, f, "op", "safe", "the flank chip 0.035 mm is over the 0.025 mm floor; time only")
setr(10, "depth_per_pass", "sug", "risky", "time: 0.371 mm makes 14 passes for 5.196 mm")
setr(10, "plunge", "sug", "safe", "914 is 1.6 % over 900")
# ---- op 11 clamp pads
for w in ("op", "sug"):
    for f in ("feed", "fz", "hex"):
        setr(11, f, w, "safe", "inside the Onsrud 1/4 in hardwood band 0.127-0.178; the cut is 0.5 mm deep; force about 8 N")
setr(11, "stepover", "sug", "risky", "time: 1.637 mm adds rings")
setr(11, "depth_per_pass", "sug", "safe", "1.169 mm exceeds the 0.5 mm pad depth, so the op still cuts one 0.5 mm pass")
setr(11, "ramp_angle", "op", "safe", "no entry dressup: a straight 0.5 mm plunge, centre cutting not confirmed; 0.5 mm only")
setr(11, "ramp_angle", "sug", "safe", "as operator")
# ---- op 13 engrave
for f in ("rpm", "feed"):
    setr(13, f, "op", "safe", "the chip 0.040 mm is in the band; time only")
for f in ("feed", "fz", "hex"):
    setr(13, f, "sug", "safe", "0.076 mm is the Amana tipped 60 deg point; 27 % over the band; a 0.6 mm cut has a small force")
setr(13, "depth_per_pass", "sug", "risky", "time: two passes of 0.3 mm and a second plunge per stroke (burn spots)")
# ---- op 14 front bulk
for w in ("op", "sug"):
    setr(14, "rpm", w, "safe", "10 % under 18000; power is not binding at 4.17 x 3.6")
    setr(14, "fz", w, "safe", "11 % over the band; 17-74 % power at 4.17 x 3.6 under all four energy lines")
    setr(14, "hex", w, "safe", "as fz")
    setr(14, "ramp_angle", w, "risky", "as op 1: core and protrusion are estimates")
setr(14, "depth_per_pass", "sug", "dangerous", "14.047 mm = one level for the front relief: 3.4 x the planned 4.17 mm; power over the curve on the chip-corrected Guedes line; the old sim had 175 steep descents for this tool")
setr(14, "stepover", "sug", "safe", "3.16 mm, 12 % under 3.6")
# ---- op 15 front rough
for f in ("feed", "fz", "hex"):
    setr(15, f, "sug", "risky", "19-31 % over the band at 30 mm stick-out (tool life); the 4.62 mm entry critical gets a thicker chip")
setr(15, "stepover", "sug", "risky", "time: 0.94 mm adds passes")
setr(15, "plunge", "op", "risky", "2360 mm/min, centre cutting not confirmed; the adaptive ramps (3 deg), so a straight plunge is rare")
setr(15, "plunge", "sug", "risky", "as operator")
# ---- op 16 R1 rest
for w in ("op", "sug"):
    setr(16, "rpm", w, "safe", "18000 rpm, 10 % under the range; the chip matters")
    for f in ("feed", "fz", "hex"):
        setr(16, f, w, "risky", "tool life: h 0.077 mm, 1.5 x the reviewer top; force about 30 N, deflection about 20 um: not breakage")
    setr(16, "plunge", w, "risky", "burning: 240 mm/min at 18000 rpm, 2F = 0.0067 mm/tooth axial; the entries are 3 deg ramps at the cut feed, so the plunge rate is rarely used")
setr(16, "depth_per_pass", "sug", "risky", "2.643 mm, 32 % over 2.0: more chip packing in a buried 2 mm tool")
# ---- op 17 scallop
for w in ("op", "sug"):
    for f in ("feed", "fz", "hex"):
        setr(17, f, w, "risky", "finish and tool life: 2.2 x the band; the path cannot hold 4100-4700 mm/min (Z rate, curves), so the real chip varies widely")
setr(17, "rpm", "op", "safe", "20611 is 3 % over 20000")
# ---- op 18 outline
for f in ("rpm", "feed", "fz", "hex"):
    setr(18, f, "op", "risky", "burning and time: fz 0.039 mm (51 % under the band) in a 25 mm deep full slot, 17 levels")
setr(18, "ramp_feed", "op", "risky", "burning: the ramp at 275 mm/min (0.013 mm/tooth at 10610 rpm)")
for f in ("feed", "fz", "hex"):
    setr(18, f, "sug", "risky", "tool life and edge quality: fz 0.382 mm, 3.8 x the band, 2.1 x the Onsrud 1/4 in roughing top 0.178; force about 29 N per tooth at 1.087 mm: not breakage")
setr(18, "rpm", "sug", "risky", "7843 rpm only serves the 0.382 mm chip at the 6000 cap")
setr(18, "depth_per_pass", "sug", "risky", "time: 1.087 mm makes 23 levels for 25 mm")
setr(18, "ramp_feed", "sug", "safe", "5999 mm/min = the cut feed")

# Recommended values (one line why). Units as the field.
REC = {
    0: dict(rpm=(18000, "review_A prod; the cap binds, torque 32 %"), feed=(6000, "fz 0.167 at the cap"),
            stepover=(14.4, "no rule binds at 0.5 mm"), depth_per_pass=(0.5, "1.0 if the board is cupped > 0.4 mm"),
            ramp_angle=(2.0, "until the core and protrusion are measured"), plunge=(None, "start rows in air")),
    1: dict(rpm=(24000, "only if the bit is rated for 24000 rpm; else 18000 with feed 4500"), feed=(6000, "with stepover 2.7: fz 0.125, hex 0.089"),
            stepover=(2.7, "review_A option B; rough-score: 52 m cut path vs 40 m, 8.6 min vs 6.6 min at the feed"),
            depth_per_pass=(9.0, "the cavity level"), ramp_angle=(2.0, "until the bit is measured"), plunge=(None, "no straight plunge")),
    2: dict(rpm=(18000, "review_A"), feed=(3600, "fz 0.10, hex 0.08 (review_A prod chip at 18000)"), stepover=(1.2, "keep"),
            depth_per_pass=(1.2, "the V20 budget"), plunge=(1080, "after the centre cut is confirmed"), ramp_angle=(3.0, "keep"),
            note="Fix the path at the 14.5 mm entry and the 13 mm links; no feed makes them safe."),
    3: dict(rpm=(18000, "review_A"), feed=(720, "fz 0.02 axial for a deep hole"), plunge=(720, "as feed"),
            depth_per_pass=(6.0, "then shorter pecks; and a tool with a flute >= 33 mm, or drill the oak to 23 mm only")),
    4: dict(rpm=(18000, "review_A"), feed=(1080, "fz 0.03 axial"), plunge=(1080, "as feed"), depth_per_pass=(8.0, "one plunge or 4 + 4"),
            dwell=(0.0, "no burn spot")),
    5: dict(rpm=(18000, "review_A"), feed=(1620, "fz 0.045 for the 10 mm worst point"), plunge=(720, "0.02 mm/tooth axial"),
            note="Better: depth levels <= 5 mm at 2880 mm/min (Project Curve has no depth per pass)."),
    6: dict(rpm=(24000, "review_C"), feed=(1080, "fz 0.045"), plunge=(300, "only if centre cutting is confirmed"), stepover=(1.0, "keep"),
            depth_per_pass=(0.5, "1.0 after the shank is measured"), ramp_angle=(3.0, "add a ramp entry, or start each level in the op 2 opening")),
    8: dict(rpm=(24000, "review_C"), feed=(960, "fz 0.040, near-slot"), plunge=(300, "only if centre cutting is confirmed"), stepover=(1.0, "keep"),
            depth_per_pass=(0.5, "1.0 after the shank is measured"), ramp_angle=(3.0, "and make every level ramp, not only the first")),
    9: dict(rpm=(18000, "review_B"), feed=(1026, "fz 0.057, the derated vendor low end"), plunge=(381, "keep (inside 300-600)"),
            note="Count the flutes first: 2 flutes halve the chip."),
    10: dict(rpm=(18000, "review_B"), feed=(2300, "fz 0.064, centre"), plunge=(750, "centre of 600-900"),
             depth_per_pass=(3.67, "equal-area 2 passes; rough-score generated 2 passes")),
    11: dict(rpm=(18000, "keep"), feed=(4720, "keep: fz 0.131 is in the Onsrud 1/4 in band, the cut is light"), stepover=(2.4, "keep"),
             depth_per_pass=(0.5, "keep")),
    13: dict(rpm=(18000, "review_B"), feed=(2000, "fz 0.056"), plunge=(750, "centre"), depth_per_pass=(0.6, "one pass")),
    14: dict(rpm=(18000, "review_A"), feed=(6000, "fz 0.167, torque 67 % at u 0.50"), stepover=(3.6, "keep"),
             depth_per_pass=(4.1667, "keep"), ramp_angle=(2.0, "until measured"), plunge=(None, "no straight plunge")),
    15: dict(rpm=(20102, "keep (operator agrees with the band)"), feed=(3900, "keep: hex 0.082"), stepover=(1.382, "keep"),
             depth_per_pass=(5.532, "keep; 2.77 if the finish matters more than time"), plunge=(720, "if centre cutting is confirmed"),
             ramp_angle=(3.0, "keep")),
    16: dict(rpm=(24000, "review_C"), feed=(2620, "h 0.050 at De 2.21"), plunge=(500, "review_C"), stepover=(0.66, "keep"),
             depth_per_pass=(2.0, "keep"), ramp_angle=(3.0, "keep; the helix pitch is not used with the ramp style")),
    17: dict(rpm=(20000, "review_C prod"), feed=(2000, "fz 0.050, h 0.043; the path reaches this feed"), stepover=(0.87, "scallop 0.1")),
    18: dict(rpm=(18000, "review_A"), feed=(2880, "fz 0.08 at 1.5 mm levels"), depth_per_pass=(1.5, "keep"),
             ramp_angle=(3.0, "keep"), ramp_feed=(2880, "ramp at the cutting feed")),
}
REC[7] = REC[6]
REC[12] = REC[0]


# ------------------------------------------------------------- per field
def classify(v, r):
    if v is None or r is None:
        return "n/a"
    lo, hi = r["lo"], r["hi"]
    if lo is None and hi is None:
        return "n/a"
    if lo is not None and v < lo * (1 - TOL):
        return "low"
    if hi is not None and v > hi * (1 + TOL):
        return "high"
    return "agree"


def chips(op, vals):
    """fz and hex (thinned or effective chip) for one set of values."""
    t = TOOLS[TOOLKEY[op]]
    n, feed = vals.get("rpm"), vals.get("feed")
    if not n or not feed:
        return None, None, ""
    z = t["z"]
    if op in (3, 4):
        f = vals["plunge"] / (n * z)
        return f, None, "axial chip = plunge / (rpm x z)"
    f = feed / (n * z)
    if TOOLKEY[op] == "v20" or TOOLKEY[op] == "v60":
        h = f * math.cos(math.radians(t["half"]))
        return f, h, f"flank chip = fz x cos {t['half']:.0f} deg; the same at every point of the edge"
    ae, ap = ENG_OP[op]
    if op in (1, 14, 2, 15, 6, 7, 8, 11) and vals.get("stepover"):
        ae = vals["stepover"]
    if op in (1, 14, 2, 15, 16) and vals.get("depth_per_pass"):
        ap = vals["depth_per_pass"]
    if TOOLKEY[op] == "r1":
        de = taper_ball_de(ap)
        if op == 17:
            h = f * de / t["d"]  # ball at ap < R: h = fz x sin(kappa) = fz x De/D; ae 0.87 = 50 % De, no radial thinning
            return f, h, f"ball at ap {ap} mm: De {de:.2f} mm, h = fz x De/D"
        h = radial_thin(f, ae, de)
        return f, h, f"taper at ap {ap} mm: De {de:.2f} mm, radial thinning at ae {ae} on De"
    h = radial_thin(f, ae, t["d"])
    return f, h, f"radial thinning at ae {ae} / D {t['d']}"


def values(op, who):
    d = SUG[op]["current" if who == "op" else "suggested"]
    out = dict(rpm=d["spindle_rpm"], feed=d["feed_rate"], plunge=d["plunge_rate"], stepover=d["stepover"],
               depth_per_pass=d["depth_per_pass"])
    extra = (EXTRA_OP if who == "op" else EXTRA_SUG).get(op, {})
    out.update(extra)
    if op in (3, 4):
        out["plunge"] = out["feed"]
    if op in (3, 4) and out.get("depth_per_pass") is None:
        out["depth_per_pass"] = extra.get("depth_per_pass")
    if op == 5:
        out["plunge_fz"] = out["plunge"] / (out["rpm"] * 2)
    return out


UNITS = dict(rpm="rpm", feed="mm/min", plunge="mm/min", stepover="mm", depth_per_pass="mm", ramp_angle="deg",
             fz="mm/tooth", hex="mm", dwell="s", ramp_feed="mm/min", plunge_fz="mm/tooth axial")
ORDER = ["rpm", "feed", "fz", "hex", "plunge", "plunge_fz", "stepover", "depth_per_pass", "ramp_angle", "ramp_feed", "dwell"]

OPS = []
for op in range(19):
    s = SUG[op]
    vo, vs = values(op, "op"), values(op, "sug")
    fo, ho, chip_basis = chips(op, vo)
    fs, hs, chip_basis_s = chips(op, vs)
    vo["fz"], vo["hex"], vs["fz"], vs["hex"] = fo, ho, fs, hs
    rev = REV[op]
    rec = REC.get(op, {})
    rec_vals = {k: v[0] for k, v in rec.items() if k != "note"}
    # derived recommended chip
    if rec_vals.get("rpm") and rec_vals.get("feed"):
        rv_ = dict(vo)
        rv_.update({k: v for k, v in rec_vals.items() if v is not None})
        if op in (3, 4):
            rv_["plunge"] = rec_vals.get("plunge", rv_["plunge"])
        rf, rh, _ = chips(op, rv_)
        rec_vals["fz"], rec_vals["hex"] = rf, rh
        if op == 5:
            rec_vals["plunge_fz"] = rec_vals["plunge"] / (rec_vals["rpm"] * 2)
    fields = {}
    for f in ORDER:
        r = rev.get(f)
        if r is None and vo.get(f) is None and vs.get(f) is None:
            continue
        if r is None and f not in ("fz", "hex"):
            continue
        co, cs = classify(vo.get(f), r), classify(vs.get(f), r)
        ro = RISK.get((op, f, "op"), ("safe", "inside the reviewer range") if co == "agree" else ("safe", "outside the range; no limit at risk") if co != "n/a" else ("n/a", "not comparable"))
        rs = RISK.get((op, f, "sug"), ("safe", "inside the reviewer range") if cs == "agree" else ("safe", "outside the range; no limit at risk") if cs != "n/a" else ("n/a", "not comparable"))
        rec_v = rec_vals.get(f)
        rec_why = rec.get(f, (None, ""))[1] if f in rec else ("derived from the recommended rpm and feed" if f in ("fz", "hex", "plunge_fz") else "")
        basis = (r or {}).get("note", "")
        if f in ("fz", "hex"):
            basis = (basis + "; " if basis else "") + chip_basis
        fields[f] = dict(
            reviewer_min=(r or {}).get("lo"), reviewer_max=(r or {}).get("hi"),
            suggest=vs.get(f), operator=vo.get(f), recommended=rec_v, unit=UNITS[f],
            class_suggest=cs, class_operator=co, risk_suggest=rs[0], risk_operator=ro[0],
            risk_suggest_why=rs[1], risk_operator_why=ro[1], recommended_why=rec_why,
            basis=basis, source=(r or {}).get("src", "compare_calc.py"),
        )
    OPS.append(dict(index=op, name=s["toolpath_name"], tool=s["tool"]["name"], tool_key=TOOLKEY[op],
                    tool_note=TOOLS[TOOLKEY[op]]["label"], group=GROUP[op], fields=fields,
                    note=rec.get("note", ""),
                    suggest_vendor_row=s["profile"]["calculator"]["vendor_source"],
                    suggest_support=s["profile"]["basis"]["support"]["headline"]))

# ------------------------------------------------------------ power check
POWER = {
    "op1_operator_9x3.6": power_models(9.0, 3.6, 18.0, 5999, 16219, 2),
    "op1_suggest_14.047x3.16": power_models(14.047, 3.16, 18.0, 5999, 16219, 2),
    "op1_recommended_9x2.7_24000": power_models(9.0, 2.7, 18.0, 6000, 24000, 2),
    "op1_A_cons_9x3.6_20000": power_models(9.0, 3.6, 18.0, 3200, 20000, 2),
    "op2_operator_planned_1.2x1.2": power_models(1.2, 1.2, 6.0, 4720.856, 18000, 2),
    "op2_operator_entry_14.5_slot": power_models(14.5, 6.0, 6.0, 4720.856, 18000, 2),
    "op2_recommended_entry_14.5_slot": power_models(14.5, 6.0, 6.0, 3600, 18000, 2),
    "op14_operator_4.17x3.6": power_models(4.1667, 3.6, 18.0, 5999, 16219, 2),
    "op14_suggest_14.047x3.16": power_models(14.047, 3.16, 18.0, 5999, 16219, 2),
    "op14_recommended_4.17x3.6_18000": power_models(4.1667, 3.6, 18.0, 6000, 18000, 2),
    "op5_suggest_10_slot": power_models(10.02, 6.0, 6.0, 5999, 7843, 2),
    "op5_operator_10_slot": power_models(10.02, 6.0, 6.0, 826, 10610, 2),
    "op5_recommended_10_slot": power_models(10.02, 6.0, 6.0, 1620, 18000, 2),
}
# Cross-check with rs_cam's own shipped_power for the Suggest geometry of op 1 (0.531 kW).
assert abs(POWER["op1_suggest_14.047x3.16"]["rs_cam_curti_kw"] - SUG[1]["profile"]["shipped_power"]["required_kw"]) < 0.01
assert abs(power_models(0.25, 5.487, 18.0, 5999, 16219, 2)["rs_cam_curti_kw"] - SUG[0]["profile"]["shipped_power"]["required_kw"]) < 0.002
# Operator op 1: rs_cam line 37 %, flat u 0.50 160 %, chip-corrected 77-103 %.
p1 = POWER["op1_operator_9x3.6"]
assert 35 < p1["rs_cam_curti_pct"] < 40 and 155 < p1["A_flat_u050_pct"] < 165
assert 70 < p1["guedes_chip_m0.5_pct"] < 85 and 95 < p1["guedes_chip_m0.3_pct"] < 110
# The 2026-11-02 sim peak 1.18 kW on the rs_cam line is 3.1 x the nominal rs_cam figure.
SIM_PEAK_KW = 1.18
assert 3.0 < SIM_PEAK_KW / p1["rs_cam_curti_kw"] < 3.3
# Op 2 entry and op 5 Suggest: dangerous on the Guedes lines.
assert POWER["op2_operator_entry_14.5_slot"]["guedes_chip_m0.3_pct"] > 150
assert POWER["op5_suggest_10_slot"]["rs_cam_curti_pct"] > 90 and POWER["op5_suggest_10_slot"]["guedes_chip_m0.3_pct"] > 250
# Op 14 operator values: under 80 % on every line.
assert max(POWER["op14_operator_4.17x3.6"][k] for k in ("A_flat_u050_pct", "rs_cam_curti_pct", "guedes_chip_m0.3_pct")) < 80


# Force and bending (judgement inputs as review_C: resultant 1.41 Fc, core 0.8 D, lever = stick-out - ap/2).
def tooth_force_n(fz_mm, ap):
    return (KS * fz_mm + F_EDGE) * ap


def bending_mpa(fc, stick, ap, d):
    m = 1.41 * fc * (stick - ap / 2)
    return m / (math.pi * (0.8 * d) ** 3 / 32)


FORCE = {}
for key, (f_, ap_, st_, d_) in {
    "op5_suggest": (5999 / (7843 * 2), 10.02, 30.0, 6.0),
    "op5_recommended": (1620 / (18000 * 2), 10.02, 30.0, 6.0),
    "op2_entry_operator": (4720.856 / (18000 * 2), 14.5, 30.0, 6.0),
    "op18_suggest": (5999 / (7843 * 2), 1.087, 30.0, 6.0),
}.items():
    fc = tooth_force_n(f_, ap_)
    FORCE[key] = dict(fz_mm=f_, ap_mm=ap_, tooth_force_n=fc, tooth_torque_nm=fc * d_ / 2 / 1000,
                      bending_mpa=bending_mpa(fc, st_, ap_, d_), rated_torque_nm=T_RATED_NM)
assert FORCE["op5_suggest"]["tooth_torque_nm"] > T_RATED_NM
assert 800 < FORCE["op5_suggest"]["bending_mpa"] < 950
assert 450 < FORCE["op2_entry_operator"]["bending_mpa"] < 600
assert FORCE["op18_suggest"]["bending_mpa"] < 150

# ------------------------------------------------------------ checks on chips used in the text
c = {o["index"]: o["fields"] for o in OPS}
assert abs(c[1]["fz"]["operator"] - 0.18494) < 1e-3 and abs(c[1]["hex"]["operator"] - 0.14796) < 1e-3
assert abs(c[5]["fz"]["suggest"] - 0.38245) < 1e-3
assert abs(c[9]["fz"]["operator"] - 0.127) < 1e-6
assert abs(c[10]["fz"]["operator"] - 808 / (10026 * 2)) < 1e-9
assert abs(c[16]["hex"]["operator"] - 0.0767) < 5e-4
assert abs(c[17]["hex"]["operator"] - 4700 / (20611 * 2) * 0.866) < 1e-3
assert abs(c[6]["fz"]["operator"] - 1258 / 18000) < 1e-9
assert c[18]["fz"]["operator"] < RUBBING_FLOOR_MM * 2

# ------------------------------------------------------------ top-level lists
RANK = {"dangerous": 0, "risky": 1}
items = []
for o in OPS:
    for f, v in o["fields"].items():
        for who, key in (("operator", "risk_operator"), ("suggest", "risk_suggest")):
            if v[key] in RANK:
                items.append(dict(op=o["index"], name=o["name"], field=f, who=who, risk=v[key],
                                  value=v[who], why=v[key + "_why"]))
items.sort(key=lambda x: (RANK[x["risk"]], x["op"], x["who"], x["field"]))

REVIEW_ERRATA = [
    dict(review="A", item="Guedes unit", verdict="correct",
         detail="kJ/cm^3 gives 108 kW on a 1300 W router; J/cm^3 gives 108 W. The printed label is wrong; 0.513 J/mm^3 is the value."),
    dict(review="A", item="Guedes u applied at a 4 x larger chip", verdict="wrong (overstates)",
         detail="Guedes cut at 12000 rpm (3.77 m/s on 6 mm), 700 mm/min: fz 0.029 (2F assumed), mean chip 0.019 mm. Op 1 has a mean chip of 0.080 mm. rs_cam's line gives 0.31 J/mm^3 at Guedes' chip (inside Guedes' 0.26-0.51) and 0.116 J/mm^3 at op 1's chip."),
    dict(review="A", item="Guedes power basis", verdict="upper bound",
         detail="Guedes reads 'active power ... required by the router' with a Fluke 435 on the router leads: electrical input, motor losses included. The paper does not subtract a no-load power. Its 108 W total is also small for a 1300 W universal motor (judgement), so the integration window may differ from the volume. Treat 0.51 J/mm^3 as an electrical upper bound at a 0.02 mm chip."),
    dict(review="A", item="op 1 rpm", verdict="minor",
         detail="Review A rates the planned cut at 18000 and 24000 rpm (144 %, 108 %). The project runs 16219 rpm (1.01 kW): 160 % at u 0.50."),
    dict(review="A", item="helix pitch limit (ops 1, 14)", verdict="not applicable",
         detail="The entry style is ramp; rough_3d.rs uses helix_pitch only for the Helix style. The ramp angle (3 deg) is the value that matters."),
    dict(review="B", item="V20 flute count doubt", verdict="resolved for the data, not for the bit",
         detail="The project and map.toml v20 say 1 flute. The library 20 deg entries with 2 flutes are other bits (5.5 mm and 6.35 mm bodies). Count the flutes of the real bit."),
    dict(review="B", item="op 9 MRR", verdict="minor arithmetic",
         detail="9.19 mm^2 x 1026 mm/min / 60 = 157 mm^3/s, not 153. u_break = 1125 / 157 = 7.16 J/mm^3, the printed 7.2. No effect."),
    dict(review="C", item="op 16 helix 27.9 deg and op 8 helix 9 deg", verdict="not applicable",
         detail="Op 16 uses entry_style ramp (3 deg); op 8 uses the ramp dressup. The helix pitch is not emitted. The real op 8 issue is different: 9 of 10 entries are straight plunges (rough-score)."),
    dict(review="C", item="chip floor at 24000 rpm for op 17", verdict="correct",
         detail="0.025 x 2 x 24000 / 0.866 = 1386 mm/min real feed is needed; review C quotes 1300. Same conclusion."),
]

GATE = [
    dict(op=5, id=11, sets=[], result="generated, 20549 moves, 7484 mm cut, 168 entries, all straight plunges (entry_plunge 168)",
         meaning="the river channel descends straight at every path start; the plunge rate governs 168 descents up to 10 mm"),
    dict(op=1, id=1, sets=[], result="generated, 7513 moves, 39712 mm cut, 44 entries: 42 ramps, 2 straight plunges",
         meaning="2 straight plunges remain for a non-centre-cutting tool; a sim must show whether they land in air"),
    dict(op=1, id=1, sets=["1.stepover=2.7", "1.helix_pitch=1.3", "1.ramp_angle_deg=2.0", "1.spindle_rpm=24000", "1.feed_rate=6000"],
         result="generated, 11989 moves, 51807 mm cut, 44 entries: 42 ramps, 2 plunges",
         meaning="+30 % path; at 6000 mm/min 8.6 min vs 6.6 min (base at 5999); 9 x 3.6 at 4400 would take 9.0 min. No power figure without a sim"),
    dict(op=10, id=16, sets=[], result="generated, 13 moves, 634 mm cut, 2 entries (plunge)", meaning="baseline"),
    dict(op=10, id=16, sets=["10.depth_per_pass=3.67", "10.feed_rate=1800", "10.spindle_rpm=18000"],
         result="generated, 13 moves, 634 mm cut, 2 entries", meaning="the equal-area split still gives 2 passes"),
    dict(op=8, id=14, sets=[], result="generated, 640 moves, 10 entry runs; rough-score labels 9 of them plunge",
         meaning="CORRECTED (rootcause/05): all 10 levels ramp. Levels 2-10 start the ramp with a short feed through the open pocket, and rough-score names a run by its first move"),
]
NEEDS_GO_AHEAD = [
    "Op 1 at 9 x 2.7, 24000 rpm, 6000 mm/min: a simulation for the peak power (the 2026-11-02 peak was 1.18 kW at 9 x 3.6).",
    "Op 1: where the 2 straight plunges land (air or wood) for the non-centre-cutting bit.",
    "Op 2: the 14.5 mm entry and 13 mm links after a path fix (op 2 is from remaining stock; its generation needs the ancestor simulation).",
    "Op 16 (R1 rest) and op 15 (front rough): from remaining stock; any check needs the ancestor simulation.",
    "Op 17: the real-feed distribution of the scallop path, to pick the rpm (review C).",
]
UNRESOLVED = [
    "The specific cutting energy of white oak at chips of 0.05-0.15 mm on this spindle: no measurement; the rs_cam line is extrapolated past its 0.04-0.10 mm printed chip range at both ends.",
    "Whether Guedes' energy is net of the no-load power (the paper does not say) and the Guedes flute count.",
    "The rated rpm, type and real core / protrusion of the 18 mm bit.",
    "The centre cutting of the 6 mm flat and the DMM 2 mm; the DMM shank diameter.",
    "The V20 flute count on the real bit.",
    "Peak (not nominal) power of ops 1 and 14 at the recommended values: needs a simulation.",
]

OUT = dict(
    generated_by="compare_calc.py", date="2026-11-03", tolerance_pct=TOL * 100,
    rubbing_floor_mm=RUBBING_FLOOR_MM,
    classification_rule="agree: within the reviewer range +-5 %; high / low: outside it. Risk: safe, risky (tool life, finish, burning, time), dangerous (breakage, stall, non-centre-cutting plunge, power over the curve).",
    chip_formulas=dict(
        fz="feed / (rpm x flutes)", radial_thinning="hex = fz x 2 sqrt(r - r^2), r = ae/D < 0.5 (DAPRA)",
        vbit="h = fz x cos(half angle) (flank chip, the same along the edge)",
        ball_shallow="h = fz x De/D with De = 2 sqrt(ap (2R - ap)) for ap < R",
        taper_ball="De = 2 (R cos a + (ap - R(1 - sin a)) tan a) above the ball; radial thinning on De",
        drill="axial chip = plunge / (rpm x flutes)"),
    energy=dict(checks=EC, models=dict(
        A_flat="P = u x MRR, u 0.50 (review A) and 0.26",
        guedes_chip="u = 0.50 x (h_m / h_guedes)^-m, h_guedes = 0.0186 mm (2F assumed), m 0.3 and 0.5 (judgement: no wood source for m)",
        rs_cam_curti="P = (Ks MRR + F_edge ap Vc z psi / 2 pi) / 60e6, Ks 58.5 N/mm^2, F_edge 4.60 N/mm (force_line.rs, Curti 2021 envelope, rho 761.6)"),
        power=POWER, force=FORCE, sim_peak_op1_kw=SIM_PEAK_KW, sim_peak_source="planning/rivmap_block_job.md 2026-11-02 evening"),
    ops=OPS, flagged=items, review_errata=REVIEW_ERRATA, gate_checks=GATE,
    needs_operator_go_ahead=NEEDS_GO_AHEAD, unresolved=UNRESOLVED,
)
with open(os.path.join(HERE, "compare.json"), "w") as fh:
    json.dump(OUT, fh, indent=1)


# ------------------------------------------------------------- markdown
def fmt(v, unit=""):
    if v is None:
        return "-"
    if isinstance(v, float):
        if unit in ("mm/tooth", "mm", "mm/tooth axial") and abs(v) < 1:
            return f"{v:.4f}".rstrip("0").rstrip(".")
        if abs(v) >= 100:
            return f"{v:.0f}"
        return f"{v:.3g}" if abs(v) < 10 else f"{v:.2f}".rstrip("0").rstrip(".")
    return str(v)


def rng(f):
    lo, hi = f["reviewer_min"], f["reviewer_max"]
    if lo is None and hi is None:
        return "none (see note)"
    if lo is None:
        return f"<= {fmt(hi, f['unit'])}"
    if lo == hi:
        return fmt(lo, f["unit"])
    return f"{fmt(lo, f['unit'])}..{fmt(hi, f['unit'])}"


md = []
md.append("# Phase 3: reviewer ranges vs Suggest vs the operator (nz-wanaka-21)\n")
md.append("Generated by `compare_calc.py` (`python3 -I compare_calc.py`). The script recomputes every chip load and power "
          "figure below and asserts the claims in this file. The same data is in `compare.json`.\n")
md.append("Inputs: `review_A.md`, `review_B.md`, `review_C.md`, `suggest/suggest_operator.jsonl` (Suggest and the operator values), "
          "the operator project (read only), the rs_cam force line (`crates/rs_cam_core/src/material/force_line.rs`), and "
          "Guedes et al. 2020 (read from the redalyc PDF on 2026-11-03).\n")
md.append("Rules: \"agree\" = inside the reviewer range +-5 %. \"high\" / \"low\" = outside it. Risk of the value that would ship: "
          "safe; risky (tool life, finish, burning, time); dangerous (breakage, stall, plunge of a non-centre-cutting tool, power over "
          "the curve). Chip formulas: fz = feed / (rpm x z); hex = fz x 2 sqrt(r - r^2) for r = ae/D < 0.5; V-bit flank chip fz x cos(half "
          "angle); ball at ap < R: h = fz x De/D; drills: plunge / (rpm x z).\n")

md.append("## Dangerous items\n")
md.append("| Op | Field | Who | Value | Why |\n|---|---|---|---|---|")
for it in items:
    if it["risk"] == "dangerous":
        md.append(f"| {it['op']} {it['name']} | {it['field']} | {it['who']} | {fmt(it['value'], UNITS.get(it['field'], ''))} | {it['why']} |")
md.append("")
md.append("Summary of the dangerous items:\n")
md.append("1. **Op 1 back bulk (operator and Suggest): power over the curve.** The 2026-11-02 sim measured a 1.18 kW peak against "
          "1.01 kW at 16219 rpm on rs_cam's own force line. Suggest also raises the depth to 14.047 mm.")
md.append("2. **Op 5 river channel, Suggest: fz 0.382 mm in a full slot up to 10 mm deep, plunge 2999 mm/min.** Suggest reads an Onsrud "
          "finishing row and has no radial engagement for a Project Curve (`NoRadialEngagement`). The operator's 826 mm/min at 10610 rpm "
          "(fz 0.039) agrees with review A. Do not apply Suggest to op 5.")
md.append("3. **Op 2 back rest: the 14.5 mm entry (operator and Suggest feed).** A path problem; rs_cam's own entry-load gate flagged it.")
md.append("4. **Op 3 pin drill: the 30 mm hole is deeper than the 25 mm flute and equals the 30 mm stick-out.** Not a feed problem.")
md.append("5. **Ops 6, 7 DMM 2 mm: straight 0.5 mm plunges per level with centre cutting not known.** Op 8 ramps every level "
          "(corrected: rootcause/05).")
md.append("6. **Op 14 front bulk, Suggest only: depth per pass 14.047 mm** (operator 4.1667 is safe).\n")

md.append("## Energy check (ops 1, 2, 14)\n")
md.append(f"(a) Unit. Guedes' cut removes {EC['guedes_mrr_cm3_s']:.3f} cm^3/s. At 513 J/cm^3 that is "
          f"{EC['guedes_power_if_J_per_cm3_W']:.0f} W; at 513 kJ/cm^3 it is {EC['guedes_power_if_kJ_per_cm3_W']/1000:.0f} kW on a "
          "1300 W router. Review A is correct: the label must be J/cm^3, so u = 0.26-0.51 J/mm^3.\n")
md.append(f"(b) Chip size. Guedes ran 3.77 m/s on a 6 mm cutter = {EC['guedes_rpm']:.0f} rpm at 700 mm/min. With 2 flutes "
          f"(the paper does not say) fz = {G_FZ:.4f} mm and the full-slot mean chip is {G_HM:.4f} mm. Op 1 has a mean chip of "
          f"{POWER['op1_operator_9x3.6']['mean_chip_mm']:.4f} mm, 4.3 x larger. Two chip laws: (1) rs_cam's affine line "
          "u = Ks + F_edge / h_m (Curti 2021, printed); (2) a power law u ~ h^-m with m = 0.3 or 0.5 (judgement; no wood source for m "
          "was found).\n")
md.append(f"(c) Net or total. Guedes measured the \"active power ... required by the router\" with a Fluke 435 on the router leads. That "
          "is electrical input power, motor losses included. The paper says inactive (not-cutting) time is not counted; it does not say "
          "that it subtracts the no-load power. So 0.51 J/mm^3 is an upper bound for shaft energy at a 0.019 mm chip.\n")
md.append(f"(d) rs_cam's line. Ks = {KS:.1f} N/mm^2, F_edge = {F_EDGE:.2f} N/mm for white oak. At Guedes' chip it gives "
          f"{EC['curti_u_at_guedes_chip_J_mm3']:.2f} J/mm^3, inside Guedes' 0.26-0.51. So the two sources agree; review A's "
          "error is to use the small-chip value at a 4 x larger chip. This agreement depends on the 2-flute assumption: with 1 flute "
          f"the Guedes chip doubles and the line gives {EC['curti_u_at_guedes_chip_1flute_J_mm3']:.2f} J/mm^3, under Guedes' range "
          "(then the electrical losses in (c) must explain the gap). The script reproduces Suggest's `shipped_power` for op 0 and op 1 "
          "from this line (asserted).\n")
md.append("| Cut | MRR mm^3/s | P avail kW | A u0.50 | A u0.26 | Guedes m0.3 | Guedes m0.5 | rs_cam line |\n|---|---|---|---|---|---|---|---|")
for k, p in POWER.items():
    md.append(f"| {k} | {p['mrr_mm3_s']:.0f} | {p['available_kw']:.3f} | {p['A_flat_u050_pct']:.0f} % | {p['A_flat_u026_pct']:.0f} % | "
              f"{p['guedes_chip_m0.3_pct']:.0f} % | {p['guedes_chip_m0.5_pct']:.0f} % | {p['rs_cam_curti_pct']:.0f} % |")
md.append("")
md.append(f"Verdict. **Op 1 (operator):** nominal {p1['rs_cam_curti_pct']:.0f} % (rs_cam line) to {p1['A_flat_u050_pct']:.0f} % "
          f"(flat u 0.50); chip-corrected Guedes {p1['guedes_chip_m0.5_pct']:.0f}-{p1['guedes_chip_m0.3_pct']:.0f} %. The peak binds, "
          f"not the nominal: the sim peak 1.18 kW on the rs_cam line is {SIM_PEAK_KW / p1['rs_cam_curti_kw']:.1f} x the nominal and "
          "over the 1.01 kW curve. Thus op 1 is dangerous on the rs_cam line itself; the Guedes lines only make it worse. "
          "**Op 2:** the planned 1.2 x 1.2 cut is light on every line; the 14.5 mm slot entry is 69 % (rs_cam line) to 304 % (flat "
          "u 0.50) and binds. **Op 14 (operator 4.17 x 3.6):** under 80 % on every line; safe. Suggest's 14.047 mm level is 52 % on the "
          "rs_cam line and 109-219 % on the Guedes lines: dangerous.\n")
md.append("Force check (Curti line, judgement inputs as review C: resultant 1.41 Fc, core 0.8 D):\n")
md.append("| Case | fz mm | ap mm | Fc per tooth N | tooth torque N m (rated 0.597) | bending MPa |\n|---|---|---|---|---|---|")
for k, f_ in FORCE.items():
    md.append(f"| {k} | {f_['fz_mm']:.4f} | {f_['ap_mm']} | {f_['tooth_force_n']:.0f} | {f_['tooth_torque_nm']:.2f} | {f_['bending_mpa']:.0f} |")
md.append("")

md.append("## Other checks on the reviews\n")
for e in REVIEW_ERRATA:
    md.append(f"- Review {e['review']}, {e['item']}: **{e['verdict']}**. {e['detail']}")
md.append("- Checked and correct: A's thinning factors (ops 2, 11, 15, 18 finish), A's depth derate at 1.67 D (-17 %), A's u_break "
          "0.46 J/mm^3 for op 1 at 24000 rpm, B's plunge floor 0.144 mm/rev for the V20, B's slit width 0.42 mm at 0.366 mm, B's "
          "plunge chip 0.010 mm for op 13, C's tapered-ball De values and the RCTF 1.093 at ae 0.66 on De 2.21.\n")

md.append("## Gate checks (rough-score, --no-sim)\n")
md.append("Each run: `systemd-run --user --scope -q -p MemoryMax=12G -p MemorySwapMax=0 timeout 900 target/release/rs_cam_cli "
          "rough-score scratch/wanaka21_scratch.toml --toolpath <id> --no-sim [--set ...]`; outputs in `scratch/gate/`. Each run took "
          "4-32 s and under 0.51 GB. Without a simulation the diagnostic holds geometry and entry counts only, no power or chip gate.\n")
md.append("| Op (id) | Overrides | Result | Meaning |\n|---|---|---|---|")
for g in GATE:
    md.append(f"| {g['op']} ({g['id']}) | {', '.join(g['sets']) or 'none'} | {g['result']} | {g['meaning']} |")
md.append("\nNeeds operator go-ahead (simulation):\n")
for n_ in NEEDS_GO_AHEAD:
    md.append(f"- {n_}")
md.append("")

md.append("## Per-op tables\n")
md.append("Columns: reviewer range (cons..prod), Suggest, operator, class (Suggest / operator), risk (Suggest / operator), "
          "recommended. \"hex\" is the real chip: thinned for a flat end mill, the flank chip for a V-bit, the chip at De for a ball.\n")
for o in OPS:
    md.append(f"### Op {o['index']} {o['name']} (group {o['group']})\n")
    md.append(f"Tool: {o['tool_note']}. Suggest row: `{o['suggest_vendor_row']}` ({o['suggest_support']}).\n")
    md.append("| Field | Unit | Reviewer | Suggest | Operator | Class S / O | Risk S / O | Recommended | Why |\n|---|---|---|---|---|---|---|---|---|")
    for f, v in o["fields"].items():
        md.append(f"| {f} | {v['unit']} | {rng(v)} | {fmt(v['suggest'], v['unit'])} | {fmt(v['operator'], v['unit'])} | "
                  f"{v['class_suggest']} / {v['class_operator']} | {v['risk_suggest']} / {v['risk_operator']} | "
                  f"{fmt(v['recommended'], v['unit'])} | {v['recommended_why']} |")
    notes = []
    for f, v in o["fields"].items():
        for who, k in (("S", "risk_suggest"), ("O", "risk_operator")):
            if v[k] in ("risky", "dangerous"):
                notes.append(f"{f} ({who}, {v[k]}): {v[k + '_why']}")
    md.append("")
    if o["note"]:
        md.append(f"Note: {o['note']}\n")
    if notes:
        md.append("Risks:\n")
        seen = set()
        for n_ in notes:
            if n_ not in seen:
                md.append(f"- {n_}")
                seen.add(n_)
        md.append("")

md.append("## Not resolved\n")
for u in UNRESOLVED:
    md.append(f"- {u}")
md.append("")
with open(os.path.join(HERE, "compare.md"), "w") as fh:
    fh.write("\n".join(md))

print("ok:", len(OPS), "ops,", sum(1 for i in items if i["risk"] == "dangerous"), "dangerous,",
      sum(1 for i in items if i["risk"] == "risky"), "risky")
for k, p in POWER.items():
    print(f"{k:36s} rs_cam {p['rs_cam_curti_pct']:6.0f} %  m0.5 {p['guedes_chip_m0.5_pct']:6.0f} %  m0.3 {p['guedes_chip_m0.3_pct']:6.0f} %  u0.5 {p['A_flat_u050_pct']:6.0f} %")
for k, f_ in FORCE.items():
    print(k, {kk: round(vv, 3) for kk, vv in f_.items()})
print({k: round(v, 4) for k, v in EC.items()})
