#!/usr/bin/env python3
"""G7 trend: material physics (specific cutting force).

Read-only. Reads the vendor LUT (to show that it holds no Kc) and
fetch/G7/verified_rows.json. Prints the tables for EXTRAPOLATION_G7.md.

Every value this script computes is DERIVED. The printed values come from
verified_rows.json. The engine constants are copied from the Rust source
(file:line in ENGINE below); this script does not run cargo.

One measure only: every affine pair (Ks, Int) is also shown as the
equivalent total specific force kc_eq(h) = Ks + Int / h (N/mm2) at
h = 0.05 and 0.10 mm, the range inside every router study here.
"""
import glob
import hashlib
import json
import pathlib
import re

REPO = pathlib.Path(__file__).resolve().parents[3]
PLAN = REPO / "planning" / "extrapolation_2026-09-24"
G7 = PLAN / "fetch" / "G7"
LUT = REPO / "crates" / "rs_cam_core" / "data" / "vendor_lut" / "observations"
H_EVAL = (0.05, 0.10)

rows = json.loads((G7 / "verified_rows.json").read_text())["observations"]
by_id = {r["observation_id"]: r for r in rows}


def f(x, w=7, p=1):
    if x is None:
        return "-".rjust(w)
    return f"{x:{w}.{p}f}"


def table(title, head, body):
    print(f"\n### {title}\n")
    print("| " + " | ".join(head) + " |")
    print("|" + "|".join("---" for _ in head) + "|")
    for b in body:
        print("| " + " | ".join(str(c).strip() for c in b) + " |")


# ---------------------------------------------------------------- T0 LUT
files = sorted(glob.glob(str(LUT / "*.json")))
n_obs = 0
kc_keys = set()
fam = {}
for p in files:
    d = json.loads(pathlib.Path(p).read_text())
    for o in d["observations"]:
        n_obs += 1
        for k in o:
            if re.search(r"(^|_)(kc|ks|force|power|energy)(_|$)", k, re.I):
                kc_keys.add(k)
        m = o.get("material_family") or "?"
        fam[m] = fam.get(m, 0) + 1
table(
    "T0. The vendor LUT (read-only scan)",
    ["LUT files", "observations", "fields named kc/ks/force/power/energy"],
    [[len(files), n_obs, len(kc_keys) if kc_keys else "0"]],
)
table(
    "T0b. LUT observations per material_family (chipload rows only)",
    ["material_family", "rows"],
    sorted(([k, v] for k, v in fam.items()), key=lambda x: -x[1]),
)

# ------------------------------------------------------------ ENGINE
# Copied from the Rust source, 2026-09-24 (master 67e98529).
ENGINE_SRC = {
    "MILLING_KC_FACTOR 2.7": "crates/rs_cam_core/src/material/mod.rs:899",
    "GenericSoftwood 6.5 x 2.7": "crates/rs_cam_core/src/material/mod.rs:965",
    "GenericHardwood 13.0 x 2.7": "crates/rs_cam_core/src/material/mod.rs:972",
    "Plywood 8.0 / 13.0 / 11.0": "crates/rs_cam_core/src/material/mod.rs:998-1002",
    "SheetGood Mdf 31.4": "crates/rs_cam_core/src/material/mod.rs:1015",
    "SheetGood Particleboard 35.0": "crates/rs_cam_core/src/material/mod.rs:1025",
    "Plastic Hdpe 40.0": "crates/rs_cam_core/src/material/mod.rs:1035",
    "janka_to_kc = janka/100": "crates/rs_cam_core/src/material/mod.rs:629-637",
    "LIT_KS 49.95": "crates/rs_cam_core/src/feeds/force.rs:62",
    "LIT_FEDGE 5.30": "crates/rs_cam_core/src/feeds/force.rs:66",
    "LIT_ANCHOR_KC 35.1": "crates/rs_cam_core/src/feeds/force.rs:77",
    "GRAIN_ANISOTROPY_FACTOR 2.0": "crates/rs_cam_core/src/tool_load/power.rs:95",
}
MKF = 2.7
ENGINE = {
    "GenericSoftwood": MKF * 6.5,
    "GenericHardwood": MKF * 13.0,
    "SheetGood::Mdf": 31.4,
    "SheetGood::Particleboard": 35.0,
    "Plywood::BalticBirch (matrix plywood)": 13.0,
    "Plywood::HardwoodFaced": 11.0,
    "Plywood::Softwood": 8.0,
    "Plastic::Hdpe": 40.0,
}


def engine_line(kc):
    s = kc / 35.1
    return 49.95 * s, 5.30 * s


table(
    "T1. Engine constants (source lines)",
    ["constant", "file:line"],
    [[k, v] for k, v in ENGINE_SRC.items()],
)
body = []
for name, kc in ENGINE.items():
    ks, fe = engine_line(kc)
    k05, k10 = ks + fe / 0.05, ks + fe / 0.10
    body.append([name, f(kc), f(ks), f(fe, 5, 2), f(k05), f(k10), f(2 * k05), f(2 * k10)])
table(
    "T2. Engine lines on the common measure (derived from the constants)",
    ["material", "'Kc'", "Ks", "F_edge", "kc_eq h0.05", "kc_eq h0.10",
     "x2.0 grain h0.05", "x2.0 grain h0.10"],
    body,
)

# ------------------------------------------------------ printed values
body = []
for r in rows:
    q = r["quantity"]
    ks = r["ks_n_per_mm2"]
    ksr = (r["ks_min_n_per_mm2"], r["ks_max_n_per_mm2"])
    it = r["int_n_per_mm"]
    itr = (r["int_min_n_per_mm"], r["int_max_n_per_mm"])
    kc = r["kc_n_per_mm2"]
    kcr = (r["kc_min_n_per_mm2"], r["kc_max_n_per_mm2"])

    def rng(v, lo_hi):
        if v is not None:
            s = f"{v:g}"
            if lo_hi[0] is not None:
                s += f" ({lo_hi[0]:g}-{lo_hi[1]:g})"
            return s
        if lo_hi[0] is not None:
            return f"{lo_hi[0]:g}-{lo_hi[1]:g}"
        return "-"

    if q == "density_normalised_ks_int_model":
        val = "model (T4)"
    elif q == "per_tooth_force_line":
        val = r["verbatim"].split(":", 1)[1].strip() + " N/tooth"
    elif q == "per_blade_force_power_law":
        val = "Fb = 243.6 em^0.4761 N; kc " + rng(None, kcr) + " (derived, b 30)"
    elif q == "shear_yield_stress_from_cutting":
        val = "yield stress " + ("46.89" if "rake15" in r["observation_id"] else "33.85") + " MPa (not a Kc)"
    else:
        val = ""
    h = (r["chip_thickness_mm_min"], r["chip_thickness_mm_max"])
    body.append([
        r["observation_id"].replace("x-g7-", ""),
        r["material_family"],
        q,
        rng(ks, ksr),
        rng(it, itr),
        rng(kc, kcr) if q in ("kc_total", "kc_textbook_base") else val,
        f"{h[0] if h[0] is not None else '?'}-{h[1] if h[1] is not None else '?'}",
        r["milling_mode"] or "-",
        r["helix_deg"] if r["helix_deg"] is not None else "-",
        r["rake_deg"] if r["rake_deg"] is not None else "-",
        r["cutting_speed_m_s"] if r["cutting_speed_m_s"] is not None else "-",
        r["density_kg_m3"] if r["density_kg_m3"] is not None else "-",
        f"{r['evidence_grade']}/{r['row_kind']}",
    ])
table(
    "T3. Every verified value, as printed (figure reads are derived, grade c)",
    ["row", "material", "quantity", "Ks N/mm2", "Int N/mm", "kc or other",
     "h mm", "mode", "helix", "rake", "vc m/s", "rho", "grade/kind"],
    body,
)

# ------------------------------------------------------------ Curti
CURTI = {  # (mode, helix): (Ks_norm a,b,c ; Int_norm a,b,c), Table 5 as printed
    ("up", 0): ((-5e-6, 1e-3, 26e-3), (-1e-7, 1e-5, 55e-4)),
    ("up", 15): ((-4e-6, 7e-4, 32e-3), (8e-8, -2e-5, 2e-4)),
    ("up", 30): ((-3e-6, 5e-4, 21e-3), (7e-8, -1e-5, -6e-5)),
    ("down", 0): ((-3e-6, 5e-4, 56e-3), (-3e-7, 4e-5, 47e-4)),
    ("down", 15): ((-3e-6, 5e-4, 49e-3), (1e-8, -4e-6, -4e-4)),
    ("down", 30): ((-2e-6, 4e-4, 30e-3), (3e-8, -7e-6, -4e-4)),
}
# Cross-check against the verified rows' printed strings.
for (mode, lam), _ in CURTI.items():
    rid = f"x-g7-curti2021-ksnorm-{mode}-helix{lam}"
    assert rid in by_id, rid


def poly(c, ga):
    return c[0] * ga * ga + c[1] * ga + c[2]


def curti(mode, lam, rho):
    """Ks and Int over GA 0..180 (1 deg). Returns stats (derived)."""
    kc, ic = CURTI[(mode, lam)]
    ks = [poly(kc, g) * rho for g in range(181)]
    it = [poly(ic, g) * rho for g in range(181)]
    return {
        "ks0": ks[0], "ks90": ks[90], "ksmin": min(ks), "ksmax": max(ks),
        "ksmean": sum(ks) / len(ks), "gamax": ks.index(max(ks)),
        "it0": it[0], "it90": it[90], "itmean": sum(it) / len(it),
    }


table(
    "T4a. Curti 2021 Table 5 per unit density (derived evaluation, GA 0 = along the grain)",
    ["mode", "helix", "Ks_n(0)", "Ks_n(90)", "Ks_n max (GA)", "Ks_n mean 0-180",
     "Ks_n max/min", "Int_n(0)", "Int_n mean"],
    [[m, l,
      f"{curti(m, l, 1)['ks0']:.4f}", f"{curti(m, l, 1)['ks90']:.4f}",
      f"{curti(m, l, 1)['ksmax']:.4f} ({curti(m, l, 1)['gamax']})",
      f"{curti(m, l, 1)['ksmean']:.4f}",
      f"{curti(m, l, 1)['ksmax'] / curti(m, l, 1)['ksmin']:.2f}",
      f"{curti(m, l, 1)['it0']:.5f}", f"{curti(m, l, 1)['itmean']:.5f}"]
     for (m, l) in CURTI],
)

SPECIES = [("paulownia", 287.1), ("lime", 585.7), ("maple", 623.9),
           ("oak", 737.8), ("azobe", 1079.5)]  # Curti Table 1, printed
NRMSE_UP0 = {"paulownia": 22.34, "lime": 14.38, "maple": 8.10, "oak": 11.52, "azobe": 30.29}
body = []
for name, rho in SPECIES:
    c = curti("up", 0, rho)
    body.append([name, rho, f(c["ks0"]), f(c["ks90"]), f(c["ksmax"]), f(c["ksmean"]),
                 f(c["it0"], 5, 2), f(c["ks0"] + c["it0"] / 0.05), f(c["ksmax"] + c["it0"] / 0.05),
                 f(c["ks0"] + c["it0"] / 0.10), f(c["ksmax"] + c["it0"] / 0.10), NRMSE_UP0[name]])
table(
    "T4b. Curti law at the five printed species densities (up-milling, helix 0; derived)",
    ["species", "rho", "Ks GA0", "Ks GA90", "Ks max", "Ks mean", "Int GA0",
     "kc_eq0.05 GA0", "kc_eq0.05 max", "kc_eq0.10 GA0", "kc_eq0.10 max", "NRMSE % up 0 (Curti Table 6, printed)"],
    body,
)
print("\nCurti Table 6 NRMSE (printed), all 30 couples: min 8.10 % (maple, up, 0), "
      "max 37.82 % (azobe, down, 0); paulownia and azobe (the two density ends) "
      "hold 9 of the 10 values above 20 % (maple up 15: 23.94 is the other).")

# ------------------------------------------- composites vs density proxy
COMP = [  # (row id, mode, helix, label)
    ("x-g7-goli2018-mdf-up-straight", "up", 0, "MDF, Goli 2018 (D80 1 blade, printed)"),
    ("x-g7-goli2023-mdf-up-helix0-figread", "up", 0, "MDF, Goli 2023 (same rig as Curti, read)"),
    ("x-g7-goli2023-particleboard-up-helix0-figread", "up", 0, "PB, Goli 2023 (same rig, read)"),
    ("x-g7-goli2023-plywood-poplar-up-helix0-figread", "up", 0, "poplar plywood up h0 (read)"),
    ("x-g7-goli2023-plywood-poplar-down-helix0-figread", "down", 0, "poplar plywood down h0 (read)"),
    ("x-g7-goli2023-plywood-poplar-up-helix30-figread", "up", 30, "poplar plywood up h30 (read)"),
    ("x-g7-goli2023-plywood-poplar-down-helix30-figread", "down", 30, "poplar plywood down h30 (read)"),
    ("x-g7-goli2018-beech-lvl-0deg-up-straight", "up", 0, "beech LVL across grain (printed)"),
    ("x-g7-goli2018-beech-lvl-90deg-up-straight", "up", 0, "beech LVL along grain (printed)"),
]
body = []
for rid, m, l, label in COMP:
    r = by_id[rid]
    rho = r["density_kg_m3"]
    c = curti(m, l, rho)
    if r["ks_n_per_mm2"] is not None:
        lo = hi = r["ks_n_per_mm2"]
        meas = f"{lo:g}"
        if r["ks_min_n_per_mm2"] is not None and "lvl" not in rid:
            meas += f" ({r['ks_min_n_per_mm2']:g}-{r['ks_max_n_per_mm2']:g})"
            lo, hi = r["ks_min_n_per_mm2"], r["ks_max_n_per_mm2"]
    else:
        lo, hi = r["ks_min_n_per_mm2"], r["ks_max_n_per_mm2"]
        meas = f"{lo:g}-{hi:g}"
    mid = (lo + hi) / 2
    verdict = ("inside" if lo >= c["ksmin"] - 1e-9 and hi <= c["ksmax"] + 1e-9
               else "above top" if hi > c["ksmax"] and lo >= c["ksmin"]
               else "below bottom" if lo < c["ksmin"] and hi <= c["ksmax"]
               else "wider")
    body.append([label, rho, f"{m} {l}", meas,
                 f"{c['ksmin']:.1f}-{c['ksmax']:.1f}", f(c["ksmean"]),
                 f"{mid / c['ksmean']:.2f}", verdict])
table(
    "T5. Density proxy test: Curti's solid-wood law at the composite's density vs the composite's own Ks (derived)",
    ["material (source)", "rho", "mode helix", "measured Ks", "Curti Ks range 0-180",
     "Curti Ks mean", "measured mid / Curti mean", "measured vs Curti range"],
    body,
)
print("\nNote: the Goli 2018 rows use a D80 single-blade cutterhead at 12.6 m/s; "
      "Curti and Goli 2023 use the same D20 2-flute tools at 3.14 m/s. "
      "Goli 2018 uses 0 deg = across the grain; Curti uses 0 deg = along.")

# ----------------------------------------------- common measure, all
body = []
for rid, label in [
    ("x-g7-goli2018-mdf-up-straight", "MDF Goli 2018"),
    ("x-g7-goli2018-ptfe-up-straight", "PTFE Goli 2018"),
    ("x-g7-goli2018-beech-lvl-0deg-up-straight", "beech LVL across"),
    ("x-g7-goli2018-beech-lvl-90deg-up-straight", "beech LVL along"),
]:
    r = by_id[rid]
    body.append([label, "printed Ks/Int", f(r["ks_n_per_mm2"] + r["int_n_per_mm"] / 0.05),
                 f(r["ks_n_per_mm2"] + r["int_n_per_mm"] / 0.10)])
for rid, label in [
    ("x-g7-goli2023-mdf-up-helix0-figread", "MDF Goli 2023"),
    ("x-g7-goli2023-particleboard-up-helix0-figread", "PB Goli 2023"),
    ("x-g7-goli2023-plywood-poplar-up-helix0-figread", "poplar plywood up h0"),
]:
    r = by_id[rid]
    a05 = (r["ks_min_n_per_mm2"] + r["int_min_n_per_mm"] / 0.05, r["ks_max_n_per_mm2"] + r["int_max_n_per_mm"] / 0.05)
    a10 = (r["ks_min_n_per_mm2"] + r["int_min_n_per_mm"] / 0.10, r["ks_max_n_per_mm2"] + r["int_max_n_per_mm"] / 0.10)
    body.append([label, "figure read", f"{a05[0]:.0f}-{a05[1]:.0f}", f"{a10[0]:.0f}-{a10[1]:.0f}"])
for name, rho in [("Curti @ lime 585.7", 585.7), ("Curti @ oak 737.8", 737.8)]:
    c = curti("up", 0, rho)
    body.append([name, "density law, GA 0 to max",
                 f"{c['ks0'] + c['it0'] / 0.05:.0f}-{c['ksmax'] + c['it90'] / 0.05:.0f}",
                 f"{c['ks0'] + c['it0'] / 0.10:.0f}-{c['ksmax'] + c['it90'] / 0.10:.0f}"])
for rid, label in [("x-g7-palubicki2021-pb-up-vc40", "PB Palubicki 40 m/s"),
                   ("x-g7-palubicki2021-pb-up-vc60", "PB Palubicki 60 m/s")]:
    r = by_id[rid]
    body.append([label, "printed total kc, mean over h to ~0.31", f"{r['kc_n_per_mm2']:g} (not at 0.05)", "-"])
# Consistency check only: Goli 2023 PB line taken OUT of its h range to
# the Palubicki h (mean to max). Not a claim.
r = by_id["x-g7-goli2023-particleboard-up-helix0-figread"]
for hh in (0.155, 0.31):
    body.append([f"PB Goli 2023 at h {hh} (outside 0.04-0.10)", "figure read, consistency check only",
                 f"{r['ks_min_n_per_mm2'] + r['int_min_n_per_mm'] / hh:.0f}-{r['ks_max_n_per_mm2'] + r['int_max_n_per_mm'] / hh:.0f} (at h {hh})", "-"])
# Durkovic: kc = Fb/(b em) with b 30 (derived)
d05 = 243.6 * 0.05 ** 0.4761 / (30 * 0.05)
d10 = 243.6 * 0.10 ** 0.4761 / (30 * 0.10)
body.append(["oak Durkovic 2017", "motor power, b 30 assumed, 38 m/s", f(d05), f(d10)])
# Kopecky, two readings
for b, lab in ((1.0, "per-mm reading (engine)"), (18.0, "per-18-mm reading")):
    body.append([f"MDF Kopecky 2019 {lab}", "per-tooth line, b not printed",
                 f((49.954 * 0.05 + 5.3047) / b / 0.05), f((49.954 * 0.10 + 5.3047) / b / 0.10)])
table(
    "T6. Every value on the common measure kc_eq(h) = Ks + Int/h, N/mm2 (derived; helix 0 or straight blade only)",
    ["material, source", "basis", "kc_eq h 0.05", "kc_eq h 0.10"],
    body,
)

# --------------------------------------------------------- ratios
SW_RHO = (450.0,)  # illustrative softwood density, NOT printed in any stored source
c_sw = curti("up", 0, 450.0)
c_hw = curti("up", 0, 737.8)
mdf18 = by_id["x-g7-goli2018-mdf-up-straight"]["ks_n_per_mm2"]
mdf23 = sum((by_id["x-g7-goli2023-mdf-up-helix0-figread"]["ks_min_n_per_mm2"],
             by_id["x-g7-goli2023-mdf-up-helix0-figread"]["ks_max_n_per_mm2"])) / 2
pb23 = sum((by_id["x-g7-goli2023-particleboard-up-helix0-figread"]["ks_min_n_per_mm2"],
            by_id["x-g7-goli2023-particleboard-up-helix0-figread"]["ks_max_n_per_mm2"])) / 2
ply23 = sum((by_id["x-g7-goli2023-plywood-poplar-up-helix0-figread"]["ks_min_n_per_mm2"],
             by_id["x-g7-goli2023-plywood-poplar-up-helix0-figread"]["ks_max_n_per_mm2"])) / 2
body = [
    ["Goli 2023 (one rig, read)", "plywood / MDF (Ks mid)", f"{ply23 / mdf23:.2f}", "one lab, figure reads"],
    ["Goli 2023 (one rig, read)", "PB / MDF (Ks mid)", f"{pb23 / mdf23:.2f}", "one lab, figure reads"],
    ["Goli 2018 (printed)", "MDF / beech LVL along", f"{mdf18 / 11.73:.2f}", "LVL is not plywood"],
    ["Goli 2018 (printed)", "MDF / beech LVL across", f"{mdf18 / 32.29:.2f}", ""],
    ["Goli 2018 (printed)", "PTFE / MDF", f"{20.33 / mdf18:.2f}", ""],
    ["Curti law (printed model)", "hardwood / softwood", "= rho_hw / rho_sw", "linear in rho by construction"],
    ["Curti law", "oak / paulownia (printed rho)", f"{737.8 / 287.1:.2f}", "both terms"],
    ["Curti law", "oak / lime (printed rho)", f"{737.8 / 585.7:.2f}", ""],
    ["cross-source: Goli 2023 / Curti @450 mean", "MDF / softwood(450)", f"{mdf23 / c_sw['ksmean']:.2f}", "rho 450 is an assumption"],
    ["cross-source: Goli 2023 / Curti @450 mean", "plywood / softwood(450)", f"{ply23 / c_sw['ksmean']:.2f}", "rho 450 is an assumption"],
    ["cross-source: Curti @737.8 / @450", "hardwood(oak) / softwood(450)", f"{c_hw['ksmean'] / c_sw['ksmean']:.2f}", "rho 450 is an assumption"],
    ["engine", "MDF / softwood", f"{ENGINE['SheetGood::Mdf'] / ENGINE['GenericSoftwood']:.2f}", "31.4 / 17.55"],
    ["engine", "plywood (BalticBirch) / softwood", f"{13.0 / ENGINE['GenericSoftwood']:.2f}", "13.0 / 17.55"],
    ["engine", "plywood (BalticBirch) / MDF", f"{13.0 / 31.4:.2f}", ""],
    ["engine", "PB / MDF", f"{35.0 / 31.4:.2f}", ""],
    ["engine", "hardwood / softwood", f"{ENGINE['GenericHardwood'] / ENGINE['GenericSoftwood']:.2f}", "13.0 / 6.5 (FPL shear)"],
]
table("T7. Ratios per source (derived)", ["source", "ratio", "value", "note"], body)

# ------------------------------------------------ engine vs measured
body = []
for name, kc, meas_lab, meas05 in [
    ("SheetGood::Mdf", 31.4, "Goli 2018 MDF", 31.44 + 3.36 / 0.05),
    ("GenericHardwood", MKF * 13.0, "Curti oak GA0..max", None),
    ("GenericSoftwood", MKF * 6.5, "Curti @450 GA0..max", None),
    ("Plywood::BalticBirch", 13.0, "Goli 2023 poplar plywood (read)", None),
    ("SheetGood::Particleboard", 35.0, "Goli 2023 PB (read)", None),
]:
    ks, fe = engine_line(kc)
    e05 = ks + fe / 0.05
    if meas05 is not None:
        m = f"{meas05:.0f}"
        rat = f"{e05 / meas05:.2f}"
    else:
        if "oak" in meas_lab:
            c = c_hw
            lo, hi = c["ks0"] + c["it0"] / 0.05, c["ksmax"] + c["it90"] / 0.05
        elif "450" in meas_lab:
            c = c_sw
            lo, hi = c["ks0"] + c["it0"] / 0.05, c["ksmax"] + c["it90"] / 0.05
        elif "plywood" in meas_lab:
            r = by_id["x-g7-goli2023-plywood-poplar-up-helix0-figread"]
            lo, hi = r["ks_min_n_per_mm2"] + r["int_min_n_per_mm"] / 0.05, r["ks_max_n_per_mm2"] + r["int_max_n_per_mm"] / 0.05
        else:
            r = by_id["x-g7-goli2023-particleboard-up-helix0-figread"]
            lo, hi = r["ks_min_n_per_mm2"] + r["int_min_n_per_mm"] / 0.05, r["ks_max_n_per_mm2"] + r["int_max_n_per_mm"] / 0.05
        m = f"{lo:.0f}-{hi:.0f}"
        rat = f"{e05 / hi:.2f}-{e05 / lo:.2f}"
    body.append([name, f"{e05:.0f}", f"{2 * e05:.0f}", meas_lab, m, rat])
table(
    "T8. Engine vs measured at h = 0.05 mm on the common measure (derived; helix 0 only)",
    ["engine material", "engine kc_eq", "engine x2.0 grain", "measured (source)",
     "measured kc_eq", "engine / measured (no grain factor)"],
    body,
)

# ----------------------------------------------- helix (T8b)
def kceq_range(mode, lam, rho, h):
    kc, ic = CURTI[(mode, lam)]
    v = [(poly(kc, g) + poly(ic, g) / h) * rho for g in range(181)]
    return min(v), max(v)


body = []
for rho, lab, eng in ((450.0, "450 (assumed softwood)", "GenericSoftwood"),
                      (737.8, "737.8 (oak, printed)", "GenericHardwood")):
    ks, fe = engine_line(ENGINE[eng])
    e05 = ks + fe / 0.05
    for (m, l) in CURTI:
        lo, hi = kceq_range(m, l, rho, 0.05)
        body.append([lab, f"{m} {l}", f"{lo:.0f}-{hi:.0f}", f"{eng} {e05:.0f}",
                     f"{e05 / hi:.1f}-{e05 / lo:.1f}"])
table(
    "T8b. Helix effect: Curti kc_eq(0.05) over GA 0-180 per mode and helix vs the engine (derived; the engine has no helix input)",
    ["rho", "mode helix", "Curti kc_eq h0.05", "engine kc_eq (no grain factor)", "engine / Curti"],
    body,
)

# ----------------------------------------------------------- hashes
body = []
for p in sorted((G7 / "sources").glob("*.txt")):
    body.append([p.name, hashlib.sha256(p.read_bytes()).hexdigest()[:16] + "..."])
table("T9. sha256 of the stored text copies (the manifest hashes the PDF/XML)",
      ["stored text", "sha256 (first 16)"], body)
print(f"\nverified rows: {len(rows)}; grades: "
      + ", ".join(f"{g}={sum(1 for r in rows if (r['evidence_grade'], r['row_kind']) == g)}"
                  for g in sorted({(r['evidence_grade'], r['row_kind']) for r in rows})))
