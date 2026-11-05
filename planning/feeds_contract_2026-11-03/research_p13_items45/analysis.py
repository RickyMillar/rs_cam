"""P13 items 4 and 5, step 0: candidate chip laws over the embedded vendor rows.

Run: python3 -I analysis.py   Writes results.json. Reads every repo file at REV with git show.
No constant is typed from memory: each one names its path:line in CONST below, and
assert_claims.py checks each against the file at REV.
"""
import csv
import io
import json
import math
import os
import re
import subprocess

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", "..", ".."))
REV = "7048456f8"
LUT = "crates/rs_cam_core/data/vendor_lut/"
FM1 = "crates/rs_cam_core/tests/fixtures/feeds_matrix_2026-09-23/matrix_2026-09-23.csv"
HARVEY = "planning/extrapolation_2026-09-24/fetch/G10/sources/metal_harvey_sf_809500.txt"


def show(path):
    return subprocess.run(["git", "-C", REPO, "show", f"{REV}:{path}"], capture_output=True,
                          text=True, check=True).stdout


# Constants, each with the file line that prints it (assert_claims.py re-reads each line).
CONST = {
    "k0": (0.024, "crates/rs_cam_core/src/machine/mod.rs", 51, "k0: 0.024"),
    "p": (0.61, "crates/rs_cam_core/src/machine/mod.rs", 52, "p: 0.61"),
    "q": (1.26, "crates/rs_cam_core/src/machine/mod.rs", 53, "q: 1.26"),
    "janka_ref": (600.0, "crates/rs_cam_core/src/material/mod.rs", 822, "600.0"),
    "feed_scale_exp": (0.4, "crates/rs_cam_core/src/material/mod.rs", 827, "0.4"),
    "sfm_wood": (200.0, "crates/rs_cam_core/src/material/mod.rs", 737, "200.0"),
    "sfm_ply": (180.0, "crates/rs_cam_core/src/material/mod.rs", 742, "180.0"),
    "sfm_sheet": (170.0, "crates/rs_cam_core/src/material/mod.rs", 747, "170.0"),
    "rpm_min": (8000.0, "crates/rs_cam_core/src/machine/mod.rs", 936, "min_rpm: 8000.0"),
    "rpm_max": (24000.0, "crates/rs_cam_core/src/machine/mod.rs", 937, "max_rpm: 24000.0"),
    "max_feed": (4000.0, "crates/rs_cam_core/src/machine/mod.rs", 941, "max_feed_mm_min: 4000.0"),
    "cut_cap_default": (6000.0, "crates/rs_cam_core/src/machine/mod.rs", 834,
                        "DEFAULT_CUTTING_FEED_CAP_MM_MIN: f64 = 6000.0"),
    "janka_sw": (600.0, "crates/rs_cam_core/src/material/mod.rs", 137, "GenericSoftwood => 600.0"),
    "janka_hw": (1450.0, "crates/rs_cam_core/src/material/mod.rs", 149, "GenericHardwood => 1450.0"),
    "janka_bb": (1200.0, "crates/rs_cam_core/src/material/mod.rs", 871, "1200.0"),
    "janka_mdf": (1100.0, "crates/rs_cam_core/src/material/mod.rs", 881, "1100.0"),
    "spektra_rpm": (18000.0, LUT + "sources/amana_spektra_spiral_plunge_v24.txt", 3, "18,000 RPM"),
}
for k, (v, path, line, text) in CONST.items():
    got = show(path).split("\n")[line - 1]
    assert text in got, (k, path, line, got)

K0, P, Q = CONST["k0"][0], CONST["p"][0], CONST["q"][0]
JREF, FSE = CONST["janka_ref"][0], CONST["feed_scale_exp"][0]
# The cutting ceiling of MachineProfile::default(): min(travel, default cap) (machine/mod.rs:356-360).
CUT_CAP = min(CONST["max_feed"][0], CONST["cut_cap_default"][0])

# Janka proxies of the engine per material family, printed in the Spektra row notes
# ("GenericSoftwood 600, GenericHardwood 1450, softwood plywood 600, hardwood-faced plywood 1000,
# MDF 1100"), amana_flat_end.json; assert_claims.py checks the sentence.
JPROXY = {"softwood": 600.0, "hardwood": 1450.0, "plywood_softwood": 600.0,
          "plywood_hardwood": 1000.0, "mdf": 1100.0, "hdf": 1300.0, "particleboard": 750.0}
CLASS = {"softwood": "softwood", "hardwood": "hardwood", "mdf": "mdf", "hdf": "mdf",
         "particleboard": "mdf", "plywood_softwood": "plywood", "plywood_hardwood": "plywood"}
CLASSES = ["softwood", "hardwood", "mdf", "plywood"]
FM1_CLASS = {"softwood": "softwood", "hardwood": "hardwood", "mdf": "mdf",
             "plywood_hardwood": "plywood"}
FM1_JANKA = {"softwood": CONST["janka_sw"][0], "hardwood": CONST["janka_hw"][0],
             "mdf": CONST["janka_mdf"][0], "plywood_hardwood": CONST["janka_bb"][0]}
FM1_SFM = {"softwood": CONST["sfm_wood"][0], "hardwood": CONST["sfm_wood"][0],
           "mdf": CONST["sfm_sheet"][0], "plywood_hardwood": CONST["sfm_ply"][0]}

INDUSTRIAL_IDS = []


def embedded_files():
    src = show("crates/rs_cam_core/src/feeds/vendor_lut.rs")
    return re.findall(r'include_str!\("\.\./\.\./data/vendor_lut/observations/([^"]+)"\)', src)


def mid(r):
    lo, hi = r.get("chipload_min_mm_tooth"), r.get("chipload_max_mm_tooth")
    if lo is not None and hi is not None:
        return (lo + hi) / 2.0
    return lo if lo is not None else (hi if hi is not None else 0.0)


def is_series_row(r):
    # The filter of feeds/extrapolation/size.rs is_series_row: a diameter, a chip, grade a or b,
    # row kind not fallback.
    d = r.get("diameter_mm")
    return (d is not None and d > 0 and mid(r) > 0 and r.get("evidence_grade") in ("a", "b")
            and r.get("row_kind") != "fallback")


def load_rows():
    files = embedded_files()
    rows = []
    for f in files:
        for r in json.loads(show(LUT + "observations/" + f))["observations"]:
            r["_file"] = f
            rows.append(r)
    ind = json.loads(show(LUT + "industrial_only/freud_solid_carbide_industrial.json"))["observations"]
    return files, rows, ind


def wood_flat(rows):
    out = [r for r in rows if r["tool_family"] == "flat_end" and r["material_family"] in CLASS]
    return out


def dedupe(rows):
    """One point per (class, printed cell). A copy of one cell under a second operation family,
    pass role or material family of the same class is not a second witness (EXTRAPOLATION_G1 1.1)."""
    seen = {}
    for r in rows:
        key = (CLASS[r["material_family"]], r["source_id"], r.get("tool_subfamily"),
               round(r["diameter_mm"], 4), r.get("flute_count"),
               r.get("chipload_min_mm_tooth"), r.get("chipload_max_mm_tooth"))
        if key not in seen:
            seen[key] = r
    pts = []
    for key, r in seen.items():
        pts.append({"id": r["observation_id"], "vendor": r["source_vendor"], "source": r["source_id"],
                    "cls": key[0], "mat": r["material_family"], "sub": r.get("tool_subfamily"), "D": r["diameter_mm"],
                    "Z": r["flute_count"], "chip": mid(r), "J": JPROXY[r["material_family"]],
                    "op": r["operation_family"], "role": r["pass_role"], "grade": r["evidence_grade"]})
    return sorted(pts, key=lambda p: p["id"])


def design(pts, model):
    cols = []
    for p in pts:
        row = []
        lnD, lnZ = math.log(p["D"]), math.log(p["Z"])
        if model == "per_class":
            for c in CLASSES:
                row += [1.0 if p["cls"] == c else 0.0, lnD if p["cls"] == c else 0.0]
        elif model == "class_D":
            row = [1.0 if p["cls"] == c else 0.0 for c in CLASSES] + [lnD]
        elif model == "class_D_Z":
            row = [1.0 if p["cls"] == c else 0.0 for c in CLASSES] + [lnD, lnZ]
        elif model == "janka_D_Z":
            row = [1.0, lnD, lnZ, math.log(p["J"] / JREF)]
        elif model == "class_D_Z_series":
            ss = sorted({(q["source"], q["sub"]) for q in pts})
            row = ([1.0 if p["cls"] == c else 0.0 for c in CLASSES] + [lnD, lnZ]
                   + [1.0 if (p["source"], p["sub"]) == k else 0.0 for k in ss[1:]])
        elif model == "class_D_Z_vendor":
            vs = sorted({q["vendor"] for q in pts})
            row = ([1.0 if p["cls"] == c else 0.0 for c in CLASSES] + [lnD, lnZ]
                   + [1.0 if p["vendor"] == v else 0.0 for v in vs[1:]])
        cols.append(row)
    return np.array(cols)


def fit(pts, model, weight):
    X = design(pts, model)
    y = np.array([math.log(p["chip"]) for p in pts])
    if weight == "vendor":
        n = {}
        for p in pts:
            n[p["vendor"]] = n.get(p["vendor"], 0) + 1
        w = np.array([1.0 / n[p["vendor"]] for p in pts])
    else:
        w = np.ones(len(pts))
    sw = np.sqrt(w)
    beta, *_ = np.linalg.lstsq(X * sw[:, None], y * sw, rcond=None)
    return beta


def predict(beta, model, D, Z, cls, J):
    lnD, lnZ = math.log(D), math.log(Z)
    if model == "per_class":
        i = CLASSES.index(cls)
        return math.exp(beta[2 * i] + beta[2 * i + 1] * lnD)
    if model == "class_D":
        return math.exp(beta[CLASSES.index(cls)] + beta[4] * lnD)
    if model == "class_D_Z":
        return math.exp(beta[CLASSES.index(cls)] + beta[4] * lnD + beta[5] * lnZ)
    if model == "janka_D_Z":
        return math.exp(beta[0] + beta[1] * lnD + beta[2] * lnZ + beta[3] * math.log(J / JREF))
    raise ValueError(model)


def shipped_formula(D, J):
    feed_scale = (J / JREF) ** FSE
    return K0 * D ** P * (1.0 / feed_scale) ** Q


def r2(pts, f):
    y = np.array([math.log(p["chip"]) for p in pts])
    yh = np.array([math.log(f(p)) for p in pts])
    ss = float(((y - yh) ** 2).sum())
    st = float(((y - y.mean()) ** 2).sum())
    return 1.0 - ss / st, math.exp(math.sqrt(ss / len(pts)))


def per_vendor(pts, f):
    out = {}
    for v in sorted({p["vendor"] for p in pts}):
        rs = [p["chip"] / f(p) for p in pts if p["vendor"] == v]
        rs.sort()
        out[v] = {"n": len(rs), "median_ratio": float(np.median(rs)), "min": rs[0], "max": rs[-1]}
    return out


MODELS = ["per_class", "class_D", "class_D_Z", "janka_D_Z"]


def main():
    files, rows, ind = load_rows()
    wf = wood_flat(rows)
    series = [r for r in wf if is_series_row(r)]
    pts = dedupe(series)
    ind_ids = [r["observation_id"] for r in ind]
    emb_by_id = {r["observation_id"]: r for r in rows}
    ind_compare = []
    for r in ind:
        e = emb_by_id.get(r["observation_id"])
        same = e is not None and all(e.get(k) == r.get(k) for k in
                                     ("diameter_mm", "flute_count", "chipload_min_mm_tooth",
                                      "chipload_max_mm_tooth", "material_family"))
        ind_compare.append({"id": r["observation_id"], "in_embedded": e is not None,
                            "same_values": same, "embedded_file": e["_file"] if e else None})
    res = {"rev": REV, "embedded_files": len(files), "rows_total": len(rows),
           "wood_flat_rows": len(wf), "series_rows": len(series), "points": len(pts),
           "points_by_vendor": {}, "points_by_class": {}, "industrial_vs_embedded": ind_compare,
           "cut_cap": CUT_CAP}
    for p in pts:
        res["points_by_vendor"][p["vendor"]] = res["points_by_vendor"].get(p["vendor"], 0) + 1
        res["points_by_class"][p["cls"]] = res["points_by_class"].get(p["cls"], 0) + 1
    # Z separation: which vendors print more than one flute count for one class.
    zsep = {}
    for p in pts:
        zsep.setdefault((p["vendor"], p["source"]), set()).add(p["Z"])
    res["z_sources"] = sorted(f"{v}:{s}" for (v, s), z in zsep.items() if len(z) > 1)
    res["points_list"] = pts

    # Fits: in-sample (all points), and out of sample with the four industrial ids removed.
    pts_noind = [p for p in pts if p["id"] not in ind_ids]
    fits = {}
    f0 = lambda p: shipped_formula(p["D"], p["J"])  # noqa: E731
    fits["shipped_formula"] = {"r2": r2(pts, f0)[0], "rms_factor": r2(pts, f0)[1],
                               "per_vendor": per_vendor(pts, f0)}
    for model in MODELS:
        for weight in ("point", "vendor"):
            b = fit(pts, model, weight)
            f = lambda p, b=b, m=model: predict(b, m, p["D"], p["Z"], p["cls"], p["J"])  # noqa: E731
            r, rms = r2(pts, f)
            entry = {"beta": [float(x) for x in b], "r2": r, "rms_factor": rms,
                     "per_vendor": per_vendor(pts, f)}
            # leave one vendor out
            lovo = {}
            for v in ("amana", "onsrud", "freud"):
                train = [p for p in pts if p["vendor"] != v]
                test = [p for p in pts if p["vendor"] == v]
                if model == "class_D_Z" or model == "janka_D_Z":
                    if len({p["Z"] for p in train}) < 2:
                        lovo[v] = None
                        continue
                bt = fit(train, model, weight)
                rs = sorted(p["chip"] / predict(bt, model, p["D"], p["Z"], p["cls"], p["J"])
                            for p in test)
                lovo[v] = {"n": len(rs), "median_ratio": float(np.median(rs)), "min": rs[0],
                           "max": rs[-1],
                           "median_abs_factor": float(np.exp(np.median(np.abs(np.log(rs)))))}
            entry["lovo"] = lovo
            # industrial rows out of sample
            bo = fit(pts_noind, model, weight)
            entry["industrial_oos"] = [
                {"id": r["observation_id"], "printed_mid": mid(r),
                 "law": predict(bo, model, r["diameter_mm"], r["flute_count"],
                                CLASS[r["material_family"]], JPROXY[r["material_family"]])}
                for r in ind]
            for e in entry["industrial_oos"]:
                e["ratio"] = e["printed_mid"] / e["law"]
            fits[f"{model}/{weight}"] = entry
    # Vendor term diagnostic: how much does a vendor offset add?
    bv = fit(pts, "class_D_Z_vendor", "point")
    Xv = design(pts, "class_D_Z_vendor")
    y = np.array([math.log(p["chip"]) for p in pts])
    ssv = float(((y - Xv @ bv) ** 2).sum())
    st = float(((y - y.mean()) ** 2).sum())
    vs = sorted({q["vendor"] for q in pts})
    fits["class_D_Z_vendor/point"] = {
        "r2": 1 - ssv / st, "rms_factor": math.exp(math.sqrt(ssv / len(pts))),
        "beta": [float(x) for x in bv], "vendor_order": vs,
        "vendor_offsets_vs_first": {v: math.exp(float(bv[6 + i])) for i, v in enumerate(vs[1:])}}
    # Series term diagnostic: one offset per chart series (source, subfamily), common D and Z slope.
    bs = fit(pts, "class_D_Z_series", "point")
    Xs = design(pts, "class_D_Z_series")
    sss = float(((y - Xs @ bs) ** 2).sum())
    nser = len({(p["source"], p["sub"]) for p in pts})
    fits["class_D_Z_series/point"] = {
        "r2": 1 - sss / st, "rms_factor": math.exp(math.sqrt(sss / len(pts))), "n_series": nser,
        "class_offsets": [float(x) for x in bs[:4]], "D_slope": float(bs[4]), "Z_slope": float(bs[5])}
    # Paired ratios inside one chart: hardwood / softwood and 3F / 2F at the same series and D.
    def paired(a_sel, b_sel, keyf):
        A = {keyf(p): p["chip"] for p in pts if a_sel(p)}
        B = {keyf(p): p["chip"] for p in pts if b_sel(p)}
        out = {}
        for k in sorted(set(A) & set(B)):
            out.setdefault(k[0], []).append(A[k] / B[k])
        return {v: {"n": len(r), "median": float(np.median(r)), "min": min(r), "max": max(r)}
                for v, r in out.items()}
    fits["paired_hard_over_soft"] = paired(
        lambda p: p["cls"] == "hardwood", lambda p: p["cls"] == "softwood",
        lambda p: (p["vendor"], p["sub"], round(p["D"], 3), p["Z"]))
    fits["paired_mdf_over_soft"] = paired(
        lambda p: p["cls"] == "mdf", lambda p: p["cls"] == "softwood",
        lambda p: (p["vendor"], p["sub"], round(p["D"], 3), p["Z"]))
    fits["paired_ply_over_soft"] = paired(
        lambda p: p["cls"] == "plywood", lambda p: p["cls"] == "softwood",
        lambda p: (p["vendor"], p["sub"], round(p["D"], 3), p["Z"]))
    fits["paired_3f_over_2f"] = paired(
        lambda p: p["Z"] == 3, lambda p: p["Z"] == 2,
        lambda p: (p["vendor"], p["cls"], round(p["D"], 3)))
    # Candidate B out of sample on the industrial rows: anchor at the largest Freud row of the same
    # material family below 12.7 mm (industrial ids removed), scale by the series slope.
    bser = fits["class_D_Z_series/point"]["D_slope"]
    anch = []
    for r in ind:
        cand = [p for p in pts_noind if p["vendor"] == "freud" and p["mat"] == r["material_family"]]
        if not cand:
            anch.append({"id": r["observation_id"], "anchor": None})
            continue
        a = max(cand, key=lambda p: p["D"])
        law = a["chip"] * (r["diameter_mm"] / a["D"]) ** bser
        anch.append({"id": r["observation_id"], "anchor": a["id"], "anchor_D": a["D"],
                     "printed_mid": mid(r), "law": law, "ratio": mid(r) / law})
    fits["anchored_series_law_industrial_oos"] = anch
    # Per-vendor own slope (class offsets, one D slope per vendor), the size exponent per vendor.
    slopes = {}
    for v in ("amana", "onsrud", "freud"):
        sub = [p for p in pts if p["vendor"] == v]
        b = fit(sub, "class_D", "point") if len({p["cls"] for p in sub}) == 4 else None
        X = np.array([[1.0 if p["cls"] == c else 0.0 for c in CLASSES if any(q["cls"] == c for q in sub)]
                      + [math.log(p["D"])] + ([math.log(p["Z"])] if len({q["Z"] for q in sub}) > 1 else [])
                      for p in sub])
        yy = np.array([math.log(p["chip"]) for p in sub])
        bb, *_ = np.linalg.lstsq(X, yy, rcond=None)
        ncls = len([c for c in CLASSES if any(q["cls"] == c for q in sub)])
        slopes[v] = {"n": len(sub), "D_slope": float(bb[ncls]),
                     "Z_slope": float(bb[ncls + 1]) if len(bb) > ncls + 1 else None,
                     "D_range": [min(p["D"] for p in sub), max(p["D"] for p in sub)]}
        del b
    fits["per_vendor_slope"] = slopes
    res["fits"] = fits

    # FM1 formula-chip cells.
    fm1 = list(csv.DictReader(io.StringIO(show(FM1))))
    cells = [x for x in fm1 if x["chipload_source"].startswith("FormulaFallback")]
    res["fm1_formula_cells"] = len(cells)
    out = []
    for x in cells:
        D, Z, mat = float(x["diameter_mm"]), int(x["flutes"]), x["material"]
        c = {"tool": x["tool_type"], "D": D, "Z": Z, "op": x["operation"], "mat": mat,
             "rpm": float(x["rpm"]), "feed": float(x["feed_mm_min"]), "chip": float(x["r_chip_load_mm"]),
             "warnings": x["feeds_warnings"]}
        # Reproduce the shipped chip and the SFM rpm.
        c["repro_chip"] = shipped_formula(D, FM1_JANKA[mat])
        rpm_sfm = FM1_SFM[mat] * 1000.0 / (math.pi * D)
        c["repro_rpm"] = min(max(rpm_sfm, CONST["rpm_min"][0]), CONST["rpm_max"][0])
        if x["tool_type"] == "EndMill":
            laws = {}
            for model in MODELS:
                for weight in ("point", "vendor"):
                    b = fits[f"{model}/{weight}"]["beta"]
                    ch = predict(b, model, D, Z, FM1_CLASS[mat], FM1_JANKA[mat])
                    laws[f"{model}/{weight}"] = {
                        "chip": ch,
                        "feed_sfm_rpm": math.floor(min(c["rpm"] * ch * Z, CUT_CAP)),
                        "feed_vendor_rpm": math.floor(min(CONST["spektra_rpm"][0] * ch * Z, CUT_CAP))}
            # Reference: the (Pocket, Roughing) row of the same tool and material, read at x1.00 by
            # the FM1 Pocket cell of the same D and material (the DropCutter route does this today).
            pk = [y for y in fm1 if y["tool_type"] == "EndMill" and y["operation"] == "Pocket"
                  and y["diameter_mm"] == x["diameter_mm"] and y["material"] == mat][0]
            ch = float(pk["r_chip_load_mm"])
            laws["roughing_row_x1"] = {"chip": ch, "row": pk["lut_observation_id"],
                                       "feed_sfm_rpm": math.floor(min(c["rpm"] * ch * Z, CUT_CAP)),
                                       "feed_vendor_rpm": math.floor(min(CONST["spektra_rpm"][0] * ch * Z, CUT_CAP)),
                                       "pocket_cell_rpm": float(pk["rpm"]),
                                       "pocket_cell_feed": float(pk["feed_mm_min"])}
            # Candidate B: the within-series law anchored at the tool's own roughing row,
            # chip = row x (D / D_row)^b_series; at FM1 the row is at the cell's D, so the factor is 1.
            b_ser = fits["class_D_Z_series/point"]["D_slope"]
            d_row = float(pk["diameter_mm"])
            chb = ch * (D / d_row) ** b_ser
            laws["anchored_series_law"] = {"chip": chb, "b": b_ser,
                                           "feed_sfm_rpm": math.floor(min(c["rpm"] * chb * Z, CUT_CAP)),
                                           "feed_vendor_rpm": math.floor(min(CONST["spektra_rpm"][0] * chb * Z, CUT_CAP))}
            c["laws"] = laws
        out.append(c)
    res["cells"] = out
    # Moves per candidate (flat cells), against the shipped feed.
    moves = {}
    flat = [c for c in out if c["tool"] == "EndMill"]
    for k in flat[0]["laws"]:
        for rpmk in ("feed_sfm_rpm", "feed_vendor_rpm"):
            rs = [c["laws"][k][rpmk] / c["feed"] for c in flat]
            moves[f"{k}|{rpmk}"] = {"n": len(rs), "gt5pct": sum(abs(r - 1) > 0.05 for r in rs),
                                     "ge2x": sum(r >= 2.0 for r in rs),
                                     "at_cap": sum(c["laws"][k][rpmk] >= CUT_CAP - 1 for c in flat),
                                     "ratio_min": min(rs), "ratio_max": max(rs)}
    res["moves"] = moves

    # Harvey 809500: Finishing / Slot-Rough per diameter and material row.
    hv = show(HARVEY).split("\n")
    dia = [float(t) for t in hv[11].split()[1:16]]
    pairs = {"softer": (13, 17), "harder": (20, 24), "engineered": (26, 28), "phenolic": (30, 32)}
    harvey = {}
    for name, (ls, lf) in pairs.items():
        s = [float(t) for t in re.findall(r"\.\d{4}", hv[ls - 1])][:15]
        f = [float(t) for t in re.findall(r"\.\d{4}", hv[lf - 1])][:15]
        rat = [b / a for a, b in zip(s, f)]
        harvey[name] = {"slot_ipt": s, "finish_ipt": f, "ratio_min": min(rat), "ratio_max": max(rat),
                        "ratio_median": float(np.median(rat))}
    # Model reading of the finishing row: Garr peak-chip lift at ae = 0.1 D times the printed
    # 3 x D depth table (0.5, Onsrud/Freud/Amana "3 x D reduce 50%").
    ae = 0.1
    psi = math.acos(1 - 2 * ae)
    harvey_model = {"ae_over_D": ae, "garr_lift": 1 / math.sin(psi), "depth_table_3xD": 0.5,
                    "product": 0.5 / math.sin(psi)}
    res["harvey"] = {"diameters_in": dia, "rows": harvey, "model": harvey_model}
    with open(os.path.join(HERE, "results.json"), "w") as fh:
        json.dump(res, fh, indent=1)


if __name__ == "__main__":
    main()
