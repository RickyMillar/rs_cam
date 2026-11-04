"""P11 step 0: radial chip thinning + helix engagement force model against the vendor rows.

Read-only. Run: python3 -I model.py  (from any directory). Writes results.json next to this file.
The pass rule, the assumptions and the candidates are fixed in PREREGISTRATION.md (written first).
Every constant cites its repo source or is marked Unsourced; no constant is tuned per row.
"""
import glob
import json
import math
import os
import re
import statistics

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", "..", ".."))
OBS = os.path.join(REPO, "crates", "rs_cam_core", "data", "vendor_lut", "observations")

# ---------------------------------------------------------------- constants (cited)
CONST = {
    "curti_ks_n": (0.0768333, "material/force_line.rs:265-266; CREDITS.md:433 (Curti 2021 helix-0 envelope / rho)"),
    "curti_int_n": (0.0060333, "material/force_line.rs:265-266; CREDITS.md:433"),
    "goli_mdf_ks": (31.44, "material/force_line.rs:90"),
    "goli_mdf_int": (3.36, "material/force_line.rs:94"),
    "oak_ks": (58.52, "tool_load/power.rs:901 (white oak, rho 761.6)"),
    "oak_fedge": (4.595, "tool_load/power.rs:901"),
    "helix_deg": (30.0, "Unsourced for vendor tools; rivmap100 tool planning/fixtures/rivmap100/rivmap100.toml:71"),
    "stickout_over_d": (3.0, "Unsourced; least stickout that can cut the printed 3 x D (PREREGISTRATION)"),
    "point_tol": (0.05, "Unsourced pass tolerance for a one-value band (PREREGISTRATION)"),
    "rivmap_chip": (0.131, "research_ae/REPORT.md section 6(a) (rivmap100 Suggest chip)"),
    "rivmap_d": (6.0, "planning/fixtures/rivmap100/rivmap100.toml (D 6, Z 2), research_ae/FINDINGS.md"),
    "pass_fraction": (0.80, "PREREGISTRATION pass rule"),
}
C_WOOD = CONST["curti_int_n"][0] / CONST["curti_ks_n"][0]
C_MDF = CONST["goli_mdf_int"][0] / CONST["goli_mdf_ks"][0]
WOOD_SOLID = {"hardwood", "softwood"}
PLYWOOD = {"plywood_hardwood", "plywood_softwood"}
SHEET_AS_MDF = {"hdf", "particleboard"}


def c_for(material, alt=None):
    """Edge/shear ratio c = F_edge/Ks (mm) and its status. alt picks the sensitivity value."""
    if material in WOOD_SOLID:
        return C_WOOD, "Curti 2021"
    if material == "mdf":
        return C_MDF, "Goli 2018"
    if material in PLYWOOD:
        return ({"zero": 0.0, "mdf": C_MDF}.get(alt, C_WOOD)), "Unsourced (solid-wood c assumed)"
    if material in SHEET_AS_MDF:
        return ({"zero": 0.0, "wood": C_WOOD}.get(alt, C_MDF)), "Unsourced (MDF c assumed)"
    return ({"wood": C_WOOD}.get(alt, 0.0)), "Unsourced (shear only)"


# ---------------------------------------------------------------- geometry
def psi_of(ae, d):
    return math.acos(max(-1.0, min(1.0, 1.0 - 2.0 * ae / d)))  # feeds/force.rs:57 (acos(1 - ae/r))


def peak_chip(fz, ae, d):  # Garr; feeds/geometry.rs:17-31
    p = psi_of(ae, d)
    return fz * (math.sin(p) if p < math.pi / 2 else 1.0)


def mean_chip(fz, ae, d):  # feeds/operating_point.rs:240-242
    p = psi_of(ae, d)
    return fz * (1 - math.cos(p)) / p


_EDGE_CACHE = {}
NZ, NT = 400, 720


def edge_sums(ap, ae, d, z, helix_deg):
    """Per rotation theta: A = sum sin(phi) dz, B = sum dz over engaged edge; and their vector forms."""
    key = (round(ap, 9), round(ae, 9), round(d, 9), z, helix_deg)
    if key in _EDGE_CACHE:
        return _EDGE_CACHE[key]
    r = d / 2
    psi = psi_of(ae, d)
    dz = ap / NZ
    zs = (np.arange(NZ) + 0.5) * dz
    pitch = 2 * math.pi / z
    th = np.arange(NT) * pitch / NT
    lag = zs * math.tan(math.radians(helix_deg)) / r  # tool/mod.rs:250-255 helix wrap
    A = np.zeros(NT); B = np.zeros(NT)
    Ax = np.zeros(NT); Ay = np.zeros(NT); Bx = np.zeros(NT); By = np.zeros(NT)
    for j in range(z):
        phi = np.mod(th[:, None] + j * pitch - lag[None, :], 2 * math.pi)
        eng = (phi >= 0) & (phi <= psi)
        s = np.where(eng, np.sin(phi), 0.0) * dz
        b = np.where(eng, 1.0, 0.0) * dz
        A += s.sum(1); B += b.sum(1)
        Ax += (s * np.cos(phi)).sum(1); Ay += (s * np.sin(phi)).sum(1)
        Bx += (b * np.cos(phi)).sum(1); By += (b * np.sin(phi)).sum(1)
    out = (A, B, Ax, Ay, Bx, By)
    _EDGE_CACHE[key] = out
    return out


def force(kind, fz, ap, ae, d, z, c, helix):
    """Engaged-edge force per unit Ks (N per (N/mm^2)), so only c matters."""
    if kind != "mean":
        A, B, Ax, Ay, Bx, By = edge_sums(ap, ae, d, z, helix)
    if kind == "peak":
        return float(np.max(fz * A + c * B))
    if kind == "mean":  # exact closed form (the numeric mean agrees to 1 %, checked in main)
        return mean_closed(fz, ap, ae, d, z, c)
    if kind == "vec":
        return float(np.max(np.hypot(fz * Ax + c * Bx, fz * Ay + c * By)))
    raise ValueError(kind)


def mean_closed(fz, ap, ae, d, z, c):
    p = psi_of(ae, d)
    return ap * z / (2 * math.pi) * (fz * (1 - math.cos(p)) + c * p)


def deflection(fz, ap, ae, d, z, c, helix, l_over_d):
    L = l_over_d * d
    a = L - ap / 2  # tool/mod.rs:749-770 load position
    if a <= 0:
        return float("nan")
    return force("vec", fz, ap, ae, d, z, c, helix) * a * a * (3 * L - a)


def solve(fun, target, hi):
    """Smallest fz2 >= 0 with fun(fz2) = target; 0 if fun(0) > target (no positive chip holds it)."""
    if fun(0.0) > target:
        return 0.0
    lo = 0.0
    while fun(hi) < target:
        hi *= 2
        if hi > 100:
            return float("nan")
    for _ in range(60):
        mid = (lo + hi) / 2
        if fun(mid) < target:
            lo = mid
        else:
            hi = mid
    return (lo + hi) / 2


CANDIDATES = ["I1", "I2", "I3", "I4", "I3v", "I5", "C1", "C2"]
FORCE_CANDS = {"I3", "I4", "I3v", "I5", "C1", "C2"}


def predict(fz1, ref, tgt, d, z, c, helix, l_over_d):
    """ref/tgt: dict(ap, ae). Returns {candidate: fz2 or None (not testable)}."""
    out = {}
    out["I1"] = fz1 * peak_chip(1, ref["ae"], d) / peak_chip(1, tgt["ae"], d)
    out["I2"] = fz1 * mean_chip(1, ref["ae"], d) / mean_chip(1, tgt["ae"], d)
    if ref["ap"] is None or tgt["ap"] is None:
        for k in FORCE_CANDS:
            out[k] = None
        return out
    for k, kind in (("I3", "peak"), ("I4", "mean"), ("I3v", "vec")):
        t = force(kind, fz1, ref["ap"], ref["ae"], d, z, c, helix)
        out[k] = solve(lambda f: force(kind, f, tgt["ap"], tgt["ae"], d, z, c, helix), t, 2 * fz1)
    t = deflection(fz1, ref["ap"], ref["ae"], d, z, c, helix, l_over_d)
    out["I5"] = solve(lambda f: deflection(f, tgt["ap"], tgt["ae"], d, z, c, helix, l_over_d), t, 2 * fz1)
    out["C1"] = min(out["I1"], out["I3"])
    out["C2"] = min(out["I1"], out["I4"])
    return out


def in_band(x, lo, hi, tol):
    if x is None or x != x:
        return None
    if lo is None or abs(hi - lo) < 1e-12:
        return abs(x - hi) <= tol * hi
    return lo * (1 - 1e-9) <= x <= hi * (1 + 1e-9)


# ---------------------------------------------------------------- rows
def load_rows():
    rows = []
    for f in sorted(glob.glob(os.path.join(OBS, "*.json"))):
        for r in json.load(open(f))["observations"]:
            r = dict(r)
            r["_file"] = os.path.basename(f)
            rows.append(r)
    return rows


IPM_RE = re.compile(r'([\d.]+)"(?:\s*-\s*([\d.]+)")?\s*IPM')


def per_row_table(rows):
    table = []
    for r in rows:
        if r["tool_family"] != "flat_end" or r.get("chipload_max_mm_tooth") is None or not r.get("diameter_mm"):
            continue
        d, z = r["diameter_mm"], r["flute_count"]
        lo, hi = r.get("chipload_min_mm_tooth"), r["chipload_max_mm_tooth"]
        mid = hi if lo is None else (lo + hi) / 2
        printed_ae = r["source_vendor"] in ("garr", "helical")
        ae = r.get("ae_max_mm")
        if ae is not None and r.get("ae_min_mm") is not None and printed_ae:
            ae = (r["ae_min_mm"] + ae) / 2
        if ae is None and printed_ae and r.get("ae_rule", "").startswith("20%"):
            ae = 0.2 * d
        ae_src = "printed" if (printed_ae and ae is not None) else ("repo-authored" if ae is not None else "none (A1 slot)")
        ae_used = ae if ae is not None else d
        ap = r.get("ap_max_mm")
        if ap is None and r.get("ap_max_factor") is not None:
            ap = r["ap_max_factor"] * d
        feed_chip = None
        m = IPM_RE.search(str(r.get("source_page", "")))
        if m and r.get("rpm_nominal"):
            ipm = [float(m.group(1)), float(m.group(2) or m.group(1))]
            feed_chip = [v * 25.4 / (r["rpm_nominal"] * z) for v in ipm]
        p = psi_of(ae_used, d)
        table.append(dict(
            id=r["observation_id"], file=r["_file"], vendor=r["source_vendor"], material=r["material_family"],
            kind=r["row_kind"], grade=r["evidence_grade"], d=d, z=z, chip_min=lo, chip_max=hi, chip_mid=mid,
            ap=ap, ae=ae_used, ae_source=ae_src, rpm=r.get("rpm_nominal"),
            feed_chip=feed_chip,
            feed_chip_matches=(None if feed_chip is None else
                               all(abs(a - b) <= 0.02 * b for a, b in zip(feed_chip, [lo if lo is not None else hi, hi]))),
            psi=p, peak_chip=peak_chip(mid, ae_used, d), mean_chip=mean_chip(mid, ae_used, d),
            rctf=1 / (math.sin(p) if p < math.pi / 2 else 1.0)))
    return table


def by_id(rows):
    return {r["observation_id"]: r for r in rows}


def band(r, scale=1.0):
    lo, hi = r.get("chipload_min_mm_tooth"), r["chipload_max_mm_tooth"]
    return (None if lo is None else lo * scale), hi * scale


def mid_of(r):
    lo, hi = band(r)
    return hi if lo is None else (lo + hi) / 2


def group_w_cases(ids):
    cases = []
    garr = [("242m", "6000"), ("842m", "6000"), ("a3", "6000"), ("142m", "6000")]
    for series, dia in garr:
        ref = ids[f"garr-{series}-alum-slot-{dia}-flat-{'3f' if series == 'a3' else '2f'}"]
        d = ref["diameter_mm"]
        tg = [f"garr-{series}-alum-profile-{dia}-flat-{'3f' if series == 'a3' else '2f'}"]
        if series == "a3":
            tg += ["garr-a3-alum-hem-profile-6000-flat-3f", "garr-a3-alum-finish-6000-flat-3f"]
        for t in tg:
            tr = ids[t]
            if "hem" in t:
                cond = dict(ap=2.0 * d, ae=0.35 * d)
            elif "finish" in t:
                cond = dict(ap=None, ae=0.025 * d)
            else:
                cond = dict(ap=1.0 * d, ae=0.5 * d)
            cases.append(dict(group="W", ref=ref, tgt=tr, refc=dict(ap=0.5 * d, ae=1.0 * d), tgtc=cond,
                              scale=1.0, label=f"Garr {series.upper()} slot -> {t.split('-alum-')[1].split('-6000')[0]}"))
    ref = ids["helical-h45al-6061-trad-rough-12700-3f"]
    tr = ids["helical-h45al-6061-hem-adaptive-12700-3f"]
    d = ref["diameter_mm"]
    cases.append(dict(group="W", ref=ref, tgt=tr, refc=dict(ap=1.0 * d, ae=0.5 * d), tgtc=dict(ap=2.0 * d, ae=0.4 * d),
                      scale=1.0, label="Helical traditional -> HEM"))
    return cases


def group_d_cases(rows, ae_factor=1.0):
    cases = []
    for r in rows:
        if r["tool_family"] != "flat_end" or not r.get("diameter_mm") or r.get("chipload_max_mm_tooth") is None:
            continue
        rule = r.get("ap_rule", "").lower()
        if not (("2xd" in rule) and ("25%" in rule) and ("50%" in rule)):
            continue
        d = r["diameter_mm"]
        for k, s in ((2, 0.75), (3, 0.50)):
            cases.append(dict(group="D", ref=r, tgt=r, refc=dict(ap=d, ae=ae_factor * d),
                              tgtc=dict(ap=k * d, ae=ae_factor * d), scale=s, k=k,
                              label=f"{r['observation_id']} 1xD -> {k}xD"))
    return cases


def run_cases(cases, helix, l_over_d, tol, c_alt=None):
    res = []
    for cs in cases:
        ref, tgt = cs["ref"], cs["tgt"]
        d, z = ref["diameter_mm"], ref["flute_count"]
        c, c_status = c_for(ref["material_family"], c_alt)
        fz1 = mid_of(ref)
        lo, hi = band(tgt, cs["scale"])
        tmid = hi if lo is None else (lo + hi) / 2
        pred = predict(fz1, cs["refc"], cs["tgtc"], d, z, c, helix, l_over_d)
        # band ends for the weaker "intersects" view (A4)
        rl, rh = band(ref)
        rl = rh if rl is None else rl
        passes, ratio_ok, inter = {}, {}, {}
        printed_ratio = tmid / fz1
        for k in CANDIDATES:
            v = pred[k]
            passes[k] = in_band(v, lo, hi, tol)
            ratio_ok[k] = None if v is None else abs(v / fz1 - printed_ratio) <= 0.10
            if v is None:
                inter[k] = None
            else:
                plo, phi_ = v / fz1 * rl, v / fz1 * rh
                blo = hi * (1 - tol) if (lo is None or abs(hi - lo) < 1e-12) else lo
                bhi = hi * (1 + tol) if (lo is None or abs(hi - lo) < 1e-12) else hi
                inter[k] = (plo <= bhi + 1e-12) and (phi_ >= blo - 1e-12)
        res.append(dict(label=cs["label"], group=cs["group"], k=cs.get("k"), id=ref["observation_id"],
                        tgt_id=tgt["observation_id"], vendor=ref["source_vendor"], material=ref["material_family"],
                        grade=ref["evidence_grade"], kind=ref["row_kind"], d=d, z=z, c=c, c_status=c_status,
                        ref_cond=cs["refc"], tgt_cond=cs["tgtc"], fz1=fz1, tgt_band=[lo, hi],
                        printed_ratio=printed_ratio, pred=pred,
                        pred_ratio={k: (None if pred[k] is None else pred[k] / fz1) for k in CANDIDATES},
                        passes=passes, ratio_ok=ratio_ok, intersects=inter))
    return res


def fractions(res, field="passes"):
    out = {}
    for g in ("W", "D"):
        sub = [r for r in res if r["group"] == g]
        out[g] = {}
        for k in CANDIDATES:
            vals = [r[field][k] for r in sub if r[field][k] is not None]
            out[g][k] = dict(n=len(vals), passed=sum(vals), frac=(sum(vals) / len(vals) if vals else None))
    return out


def verdict_of(fr):
    pf = CONST["pass_fraction"][0]
    winners = [k for k in CANDIDATES
               if fr["W"][k]["frac"] is not None and fr["D"][k]["frac"] is not None
               and fr["W"][k]["frac"] >= pf and fr["D"][k]["frac"] >= pf]
    return winners


def main():
    rows = load_rows()
    ids = by_id(rows)
    helix, lod, tol = CONST["helix_deg"][0], CONST["stickout_over_d"][0], CONST["point_tol"][0]

    # self-check: numeric mean force equals the closed form
    for (ap, ae, d, z) in [(6, 6, 6, 2), (12, 2.1, 6, 3), (6, 1.2, 6, 2)]:
        A, B = edge_sums(ap, ae, d, z, helix)[:2]
        num = float(np.mean(0.05 * A + C_WOOD * B))
        cf = mean_closed(0.05, ap, ae, d, z, C_WOOD)
        assert abs(num - cf) / cf < 0.01, (num, cf)

    cases = group_w_cases(ids) + group_d_cases(rows)
    primary = run_cases(cases, helix, lod, tol)
    fr = fractions(primary)
    fr_ratio = fractions(primary, "ratio_ok")
    fr_inter = fractions(primary, "intersects")
    winners = verdict_of(fr)

    # breakdowns of group D by vendor, material, grade
    def breakdown(field):
        out = {}
        for r in primary:
            if r["group"] != "D":
                continue
            key = r[field]
            o = out.setdefault(key, {k: [0, 0] for k in CANDIDATES})
            for k in CANDIDATES:
                if r["passes"][k] is not None:
                    o[k][0] += int(r["passes"][k]); o[k][1] += 1
        return out

    # sensitivities (one at a time)
    sens = []

    def add_sens(name, value, res):
        f = fractions(res)
        sens.append(dict(name=name, value=value,
                         W={k: f["W"][k]["frac"] for k in CANDIDATES},
                         D={k: f["D"][k]["frac"] for k in CANDIDATES},
                         winners=verdict_of(f)))
    for h in (0.0, 15.0, 45.0):
        add_sens("helix_deg", h, run_cases(cases, h, lod, tol))
    for l in (4.5, 6.0, 7.5):
        add_sens("stickout_over_d", l, run_cases(cases, helix, l, tol))
    for t in (0.02, 0.10):
        add_sens("point_tol", t, run_cases(cases, helix, lod, t))
    for a in (0.5, 0.2):
        add_sens("A1_ae_over_d", a, run_cases(group_w_cases(ids) + group_d_cases(rows, a), helix, lod, tol))
    for alt in ("zero", "wood", "mdf"):
        add_sens("c_alt_for_unsourced", alt, run_cases(cases, helix, lod, tol, alt))

    # ---------------------------------------------------------- the lead's hand check
    fz, d, z = CONST["rivmap_chip"][0], CONST["rivmap_d"][0], 2
    ks, fe = CONST["oak_ks"][0], CONST["oak_fedge"][0]
    c_oak = fe / ks
    one_tooth = 1 - ((fz + c_oak) / 2 - c_oak) / fz  # ap (Ks fz + Fe) held, ap doubles
    slot1, slot2 = dict(ap=d, ae=d), dict(ap=2 * d, ae=d)
    p = predict(fz, slot1, slot2, d, z, c_oak, helix, lod)
    hand = dict(fz=fz, d=d, z=z, c=c_oak, one_tooth_cut=one_tooth,
                cut={k: 1 - p[k] / fz for k in ("I3", "I4", "I3v", "I5")},
                pure_shear_cut=0.5, vendor_cut=0.25)
    hand["confirmed"] = 0.75 <= one_tooth <= 0.85
    # over every group D row at 2xD (wood rows only, c > 0)
    cuts = {k: [] for k in ("I3", "I4", "I3v")}
    force_ratio_vendor = {"2": [], "3": []}
    for r in primary:
        if r["group"] != "D":
            continue
        if r["k"] == 2 and r["material"] in (WOOD_SOLID | PLYWOOD | {"mdf"} | SHEET_AS_MDF):
            for k in cuts:
                cuts[k].append(1 - r["pred"][k] / r["fz1"])
        # the force growth the vendor step implies (peak engaged-edge force, I3 form)
        s = 0.75 if r["k"] == 2 else 0.5
        f1 = force("peak", r["fz1"], r["ref_cond"]["ap"], r["ref_cond"]["ae"], r["d"], r["z"], r["c"], helix)
        f2 = force("peak", s * r["fz1"], r["tgt_cond"]["ap"], r["tgt_cond"]["ae"], r["d"], r["z"], r["c"], helix)
        force_ratio_vendor[str(r["k"])].append(f2 / f1)
    hand["rows_cut_2xD"] = {k: dict(n=len(v), min=min(v), median=statistics.median(v), max=max(v)) for k, v in cuts.items()}
    hand["vendor_force_ratio"] = {k: dict(n=len(v), min=min(v), median=statistics.median(v), max=max(v))
                                  for k, v in force_ratio_vendor.items()}

    # ---------------------------------------------------------- post hoc (labelled; not in the verdict)
    post_hoc = {}
    # PH1: stickout L/D at which I5 gives the printed 25 % step (slot, wood rows, median row)
    def l_for_quarter(r):
        def cut(l):
            v = predict(r["fz1"], r["ref_cond"], r["tgt_cond"], r["d"], r["z"], r["c"], helix, l)["I5"]
            return 1 - v / r["fz1"]
        ls = [x / 2 for x in range(5, 61)]
        best = min(ls, key=lambda l: abs(cut(l) - 0.25))
        return best, cut(best)
    sample = [r for r in primary if r["group"] == "D" and r["k"] == 2 and r["material"] == "hardwood"][:6]
    post_hoc["PH1_stickout_for_25pct"] = [dict(id=r["id"], l_over_d=l, cut=c) for r in sample for (l, c) in [l_for_quarter(r)]]
    post_hoc["PH1_c0_shear_only"] = None
    # PH2: Garr chip in proportion to 1 / (edge time in cut, psi / 2 pi)
    ph2 = []
    for r in primary:
        if r["group"] != "W":
            continue
        dref = psi_of(r["ref_cond"]["ae"], r["d"])
        dt = psi_of(r["tgt_cond"]["ae"], r["d"])
        v = r["fz1"] * dref / dt
        lo, hi = r["tgt_band"]
        ph2.append(dict(label=r["label"], pred=v, band=[lo, hi], passes=in_band(v, lo, hi, tol)))
    post_hoc["PH2_inverse_time_in_cut"] = ph2

    table = per_row_table(rows)
    fc0 = [t for t in table if t["feed_chip"] is not None]

    results = dict(
        constants={k: dict(value=v[0], source=v[1]) for k, v in CONST.items()},
        c_values=dict(wood=C_WOOD, mdf=C_MDF),
        n_rows_total=len(rows), n_flat_rows=len(table),
        n_printed_ae_rows=sum(1 for t in table if t["ae_source"] == "printed"),
        n_repo_ae_rows=sum(1 for t in table if t["ae_source"] == "repo-authored"),
        fc0=dict(n=len(fc0), matches=sum(1 for t in fc0 if t["feed_chip_matches"]),
                 max_rel_dev=max(abs(t["feed_chip"][1] - t["chip_max"]) / t["chip_max"] for t in fc0)),
        cases=primary,
        fractions=fr, fractions_ratio=fr_ratio, fractions_intersects=fr_inter,
        winners=winners, verdict=("PASS" if winners else "FAIL"),
        breakdown_vendor=breakdown("vendor"), breakdown_material=breakdown("material"),
        breakdown_grade=breakdown("grade"),
        sensitivity=sens, hand_check=hand, post_hoc=post_hoc, per_row=table,
    )
    with open(os.path.join(HERE, "results.json"), "w") as f:
        json.dump(results, f, indent=1, default=float)

    print("verdict", results["verdict"], "winners", winners)
    for g in ("W", "D"):
        print(g, {k: (fr[g][k]["passed"], fr[g][k]["n"], round(fr[g][k]["frac"], 3) if fr[g][k]["frac"] is not None else None) for k in CANDIDATES})
    for g in ("W", "D"):
        print(g, "ratio", {k: round(fr_ratio[g][k]["frac"], 3) if fr_ratio[g][k]["frac"] is not None else None for k in CANDIDATES})
    print("hand", json.dumps(hand, default=float)[:900])
    for s in sens:
        print(s["name"], s["value"], s["winners"], {k: round(v, 2) if v is not None else None for k, v in s["D"].items()})
    for r in primary:
        if r["group"] == "W":
            print(r["label"], round(r["fz1"], 4), r["tgt_band"], {k: (round(v, 4) if v is not None else None) for k, v in r["pred"].items()})


if __name__ == "__main__":
    main()
