#!/usr/bin/env python3
"""G2 fetch builder (extrapolation programme, gap group G2: material category).

Run from any directory:  python3 g2_build.py

Inputs:  fetch/G2/pdf/*.pdf and the pdftotext -layout copies next to them
         (pdftotext -layout <x>.pdf <x>.txt).
Outputs: fetch/G2/sources/<source_id>.txt   (stored text, full or excerpt)
         fetch/G2/candidate_rows.json       (LUT observation schema + 2 fields)
         fetch/G2/sources.json              (manifest-style entries)

The script transcribes only cells that the stored text prints. The column of
each value on an Onsrud sheet comes from the character position under the
header line (the same method as g2_column_map.py). The MDF sheet and the
laminated page 119 were also checked against a rendered image (FETCH_NOTES.md).
"""
import hashlib
import json
import os
import re

HERE = os.path.dirname(os.path.abspath(__file__))
G2 = os.path.normpath(os.path.join(HERE, "..", "fetch", "G2"))
PDF = os.path.join(G2, "pdf")
SRC = os.path.join(G2, "sources")
ACCESSED = "2026-09-24"
IN = 25.4

os.makedirs(SRC, exist_ok=True)


def sha(path):
    h = hashlib.sha256()
    with open(os.path.join(PDF, path), "rb") as f:
        h.update(f.read())
    return h.hexdigest()


def text(name):
    with open(os.path.join(PDF, name), encoding="utf-8", errors="replace") as f:
        return f.read()


def frac(s):
    """'1/8' -> 0.125, '1 1/4' or '1-1/4' -> 1.25, '1' -> 1.0 (inch)."""
    s = s.strip().replace("-", " ")
    parts = s.split()
    total = 0.0
    for p in parts:
        if "/" in p:
            a, b = p.split("/")
            total += float(a) / float(b)
        else:
            total += float(p)
    return total


def mm(x_in):
    return round(x_in * IN, 4)


# --------------------------------------------------------------------------
# 1. Stored texts
# --------------------------------------------------------------------------
# (source_id, pdf file, txt file, pages to keep or None for the whole text)
STORE = [
    ("onsrud_hard_wood_cutting_data", "onsrud_hard_wood.pdf", "onsrud_hard_wood.txt", None),
    ("onsrud_soft_wood_cutting_data", "onsrud_soft_wood.pdf", "onsrud_soft_wood.txt", None),
    ("onsrud_mdf_cutting_data", "onsrud_mdf.pdf", "onsrud_mdf.txt", None),
    ("onsrud_hard_plywood_cutting_data", "onsrud_hard_plywood.pdf", "onsrud_hard_plywood.txt", None),
    ("onsrud_soft_plywood_cutting_data", "onsrud_soft_plywood.pdf", "onsrud_soft_plywood.txt", None),
    ("onsrud_laminated_chipboard_cutting_data", "onsrud_laminated_chipboard.pdf", "onsrud_laminated_chipboard.txt", None),
    ("onsrud_laminated_plywood_cutting_data", "onsrud_laminated_plywood.pdf", "onsrud_laminated_plywood.txt", None),
    ("onsrud_pct19_catalog", "onsrud_pct19_catalog.pdf", "onsrud_pct19_catalog.txt", [20, 21, 22, 119]),
    ("amana_pcd_ball_nose_v2", "amana_pcd_ball_nose_v2.pdf", "amana_pcd_ball_nose_v2.txt", None),
    ("amana_spektra_3d_profiling_v6", "amana_spektra_3d_profiling_v6.pdf", "amana_spektra_3d_profiling_v6.txt", None),
    ("amana_ball_nose_v7", "amana_spiral_ball_nose_v7.pdf", "amana_spiral_ball_nose_v7.txt", None),
    ("amana_spiral_ball_nose_v6", "amana_spiral_ball_nose_v6.pdf", "amana_spiral_ball_nose_v6.txt", None),
    ("amana_spiral_ball_nose_2015", "amana_spiral_ball_nose_2015.pdf", "amana_spiral_ball_nose_2015.txt", None),
    ("freud_router_bit_feed_and_speed_for_cnc_20170822", "freud_cnc_20170822.pdf", "freud_cnc_20170822.txt", None),
    ("leitz_lexicon7_05_routing", "leitz_lexicon7_05_routing.pdf", "leitz_lexicon7_05_routing.txt", "correction"),
    ("techno_cnc_chipload_rev2", "techno_cnc_chipload_rev2.pdf", "techno_cnc_chipload_rev2.txt", None),
    ("sienci_feeds_speeds_imperial", "sienci_feeds_speeds_imperial.pdf", "sienci_feeds_speeds_imperial.txt", None),
    ("sorotec_schnittwerte", "sorotec_schnittwerte.pdf", "sorotec_schnittwerte.txt", None),
    ("shopbot_feedsandspeeds_2016", "shopbot_feedsandspeeds_2024.pdf", "shopbot_feedsandspeeds_2024.txt", None),
    ("ansi_a208_1_2016_particleboard", "cpa_ansi_a208_1_2016.pdf", "cpa_ansi_a208_1_2016.txt", None),
    ("wood_handbook_1999_ch10", "drj_wood_handbook_ch10.pdf", "drj_wood_handbook_ch10.txt", None),
    ("weyerhaeuser_mdf_spec", "weyerhaeuser_mdf_spec.pdf", "weyerhaeuser_mdf_spec.txt", None),
]

stored = {}
for sid, pdf, txt, pages in STORE:
    body = text(txt)
    head = f"# stored text for {sid}\n# made from {pdf} with pdftotext -layout on {ACCESSED}\n"
    if pages == "correction":
        pp = body.split("\f")
        keep = [i for i, p in enumerate(pp, 1) if "Correction factor" in p]
        head += (
            "# EXCERPT: only the pdftotext pages that print a 'Correction factor' "
            f"statement (pages {keep} of {len(pp)}); pdf_sha256 covers the whole PDF.\n"
        )
        body = "\f".join(f"### pdftotext page {i}\n" + pp[i - 1] for i in keep)
    elif pages:
        pp = body.split("\f")
        head += (
            f"# EXCERPT: pdftotext pages {pages} of {len(pp)} only; "
            "pdf_sha256 covers the whole PDF.\n"
        )
        body = "\f".join(f"### pdftotext page {i}\n" + pp[i - 1] for i in pages)
    out = os.path.join(SRC, f"{sid}.txt")
    with open(out, "w", encoding="utf-8") as f:
        f.write(head + body)
    stored[sid] = (f"planning/extrapolation_2026-09-24/fetch/G2/sources/{sid}.txt", sha(pdf))

# --------------------------------------------------------------------------
# 2. Candidate rows
# --------------------------------------------------------------------------
rows = []

# Engine Janka proxy per family, the same convention as onsrud_tapered_ball.json
# (material::effective_janka_lbf). The sheets print no hardness.
PROXY = {
    "hardwood": 1450.0,
    "softwood": 600.0,
    "mdf": 1100.0,
    "plywood_hardwood": 1000.0,
    "plywood_softwood": 600.0,
    "particleboard": 750.0,
}

SHEETS = [
    # (source_id, txt, material_family, label, url, title, header line index 0-based of the V table)
    ("onsrud_hard_wood_cutting_data", "onsrud_hard_wood.txt", "hardwood", "hard wood (Onsrud sheet)",
     "https://www.onsrud.com/images/Hard%20Wood.pdf", "LMT Onsrud Hard Wood Cutting Data Recommendations", 0),
    ("onsrud_soft_wood_cutting_data", "onsrud_soft_wood.txt", "softwood", "soft wood (Onsrud sheet)",
     "https://www.onsrud.com/images/Soft%20Wood.pdf", "LMT Onsrud Soft Wood Cutting Data Recommendations", 0),
    ("onsrud_mdf_cutting_data", "onsrud_mdf.txt", "mdf", "MDF (Onsrud sheet)",
     "https://www.onsrud.com/images/MDF.pdf", "LMT Onsrud MDF Cutting Data Recommendations", 0),
    ("onsrud_hard_plywood_cutting_data", "onsrud_hard_plywood.txt", "plywood_hardwood", "hard plywood (Onsrud sheet)",
     "https://www.onsrud.com/images/Hard%20Plywood.pdf", "LMT Onsrud Hard Plywood Cutting Data Recommendations", 0),
    ("onsrud_soft_plywood_cutting_data", "onsrud_soft_plywood.txt", "plywood_softwood", "soft plywood (Onsrud sheet)",
     "https://www.onsrud.com/images/Soft%20Plywood.pdf", "LMT Onsrud Soft Plywood Cutting Data Recommendations", 0),
    ("onsrud_laminated_chipboard_cutting_data", "onsrud_laminated_chipboard.txt", "particleboard",
     "laminated chipboard (Onsrud sheet; coated particleboard)",
     "https://www.onsrud.com/images/Laminated%20Chipboard.pdf",
     "LMT Onsrud Laminated Chipboard Cutting Data Recommendations", 0),
    ("onsrud_laminated_plywood_cutting_data", "onsrud_laminated_plywood.txt", "plywood_hardwood",
     "laminated plywood (Onsrud sheet; family is a best-fit mapping)",
     "https://www.onsrud.com/images/Laminated%20Plywood.pdf",
     "LMT Onsrud Laminated Plywood Cutting Data Recommendations", 1),
]

SERIES = {
    "37-00/37-20": dict(sub="onsrud_37_00_37_20_engraving", flutes=1),
    "37-50": dict(sub="onsrud_37_50_v_bottom_solid_carbide", flutes=2, angle=90.0),
    "37-60": dict(sub="onsrud_37_60_v_bottom_carbide_tipped", flutes=2, angle=90.0),
    "37-80": dict(sub="onsrud_37_80_lettering_carbide_tipped", flutes=2),
}
CATALOG = {  # PCT-19 pages 20-22 (stored excerpt): printed cutting diameters (in)
    "37-50": {0.1875: "37-50", 0.25: "37-51", 0.375: "37-52"},
    "37-60": {0.5: "37-61", 0.75: "37-62", 1.0: "37-63"},
    "37-80": {1.0: ("37-82", 60.0), 1.5: ("37-87", 90.0), 2.0: ("37-92 120 deg / 37-97 140 deg", None)},
}
RPM_MARK = {"*": 16000.0, "**": 15000.0}


def page_no(body):
    m = re.findall(r"www\.onsrud\.com\s+(\d{3})|^\s*(\d{3})\s+www\.onsrud\.com", body, re.M)
    for a, b in m:
        return a or b
    return "?"


for sid, txt, fam, label, url, title, tab in SHEETS:
    body = text(txt)
    lines = body.split("\n")
    hdrs = [i for i, l in enumerate(lines) if re.search(r"Series\s+Cut\s", l)]
    h = hdrs[tab]
    end = hdrs[tab + 1] if tab + 1 < len(hdrs) else len(lines)
    hdr = lines[h].expandtabs(8)
    cols = [(m.group(1), (m.start() + m.end()) / 2)
            for m in re.finditer(r"(\d+(?:[ -]\d+/\d+|/\d+)?)", hdr) if m.start() >= 15]
    pg = page_no(body)
    for li in range(h + 1, end):
        raw = lines[li]
        row = raw.expandtabs(8)
        m = re.match(r"\s*(37-00/37-20|37-50|37-60|37-80)\s+(1/2 x D|1/2 CED|Varies)\s", row)
        if not m:
            continue
        series, cut = m.group(1), m.group(2)
        spec = SERIES[series]
        for v in re.finditer(r"\.(\d{3}) ?-\s?\.(\d{3})(\*{0,2})", row):
            c = (v.start() + v.end()) / 2
            col, pos = min(cols, key=lambda k: abs(k[1] - c))
            d_in = frac(col)
            lo, hi, mark = int(v.group(1)) / 1000, int(v.group(2)) / 1000, v.group(3)
            notes = [
                f"Printed cell, transcribed {ACCESSED} from the stored text (G2 fetch). "
                f"The value sits in the '{col}' column of the chip-load table "
                f"(raw pdftotext -layout lines {h + 1} and {li + 1}; the stored copy adds 2 header lines, so {h + 3} and {li + 3}); the column comes "
                "from the character position under the header (scripts/g2_column_map.py).",
                "The sheet prints no hardness; hardness_value is the engine's Janka proxy for the "
                "family (same convention as onsrud_tapered_ball.json).",
                "The sheet names no operation. operation_family trace / pass_role finish copies the "
                "existing chamfer_vbit rows; the reconciler decides the families.",
            ]
            o = {
                "observation_id": "",
                "source_id": sid,
                "source_vendor": "onsrud",
                "source_title": title,
                "source_url": url,
                "accessed_on": ACCESSED,
                "source_page": f"{title}, page {pg}, series {series}, {col} in column",
                "evidence_grade": "a",
                "row_kind": "exact",
                "tool_family": "chamfer_vbit",
                "tool_subfamily": spec["sub"],
                "operation_family": "trace",
                "pass_role": "finish",
                "material_family": fam,
                "material_label": label,
                "hardness_kind": "janka",
                "hardness_value": PROXY[fam],
                "flute_count": spec["flutes"],
                "chipload_min_mm_tooth": mm(lo),
                "chipload_max_mm_tooth": mm(hi),
                "ap_rule": f"printed Cut column: '{cut}'",
            }
            if cut in ("1/2 x D", "1/2 CED"):
                o["ap_max_factor"] = 0.5  # printed: half the cutting edge diameter
            if series == "37-00/37-20":
                notes.insert(1,
                    "FRAME WARNING: every 37-0x and 37-2x part has a 1/4 in shank and a 0.005-0.040 in tip "
                    "(PCT-19 page 20: 37-00 60 deg, 37-20 30 deg, 1 flute). The '1/4' column is therefore the "
                    "shank, not a cutting diameter. diameter_mm, tip_diameter_mm and included_angle_deg are "
                    "left out on purpose; the chart prints one band for all tips and both angles.")
                o["source_page"] += " (the column is the shank diameter, see notes)"
                oid = f"x-g2-onsrud-{fam}-37-00-37-20"
            else:
                o["diameter_mm"] = mm(d_in)
                angle = spec.get("angle")
                cat = CATALOG[series].get(d_in)
                if series == "37-80":
                    if cat:
                        angle = cat[1]
                        notes.insert(1, f"PCT-19 page 22 lists part {cat[0]} at this cutting diameter"
                                     + (f" ({angle:.0f} deg)." if angle else "; the angle is not unique, so included_angle_deg is left out."))
                    else:
                        notes.insert(1, f"PCT-19 page 22 lists no 37-80 part at {col} in "
                                     "(parts: 1 in 60 deg, 1-1/2 in 90 deg, 2 in 120 deg, 2 in 140 deg). "
                                     "Transcribed as printed; included_angle_deg is left out.")
                else:
                    if cat:
                        notes.insert(1, f"PCT-19 pages 20-21 list part {cat} at this cutting diameter "
                                     "(90 deg V bottom, 2 flutes).")
                    else:
                        notes.insert(1, f"PCT-19 pages 20-21 list no {series} part at {col} in. Transcribed "
                                     "as printed; the reconciler decides whether to use it.")
                        if sid == "onsrud_laminated_chipboard_cutting_data":
                            notes.insert(2, "The rendered page 119 confirms this cell position. The other "
                                         "sheets print 37-60 at 3/8, 1/2, 3/4 and 1 in; this table prints "
                                         "3/8, 9/16 and 7/8 in. Probable layout error in the source; not corrected here.")
                if angle:
                    o["included_angle_deg"] = angle
                if mark:
                    o["rpm_nominal"] = RPM_MARK[mark]
                    notes.insert(1, f"The cell carries '{mark}'; the sheet footnote reads "
                                 f"'{mark} = {int(RPM_MARK[mark]):,} RPM'.")
                oid = f"x-g2-onsrud-{fam}-{series}-{int(round(mm(d_in) * 1000))}"
            if sid == "onsrud_laminated_plywood_cutting_data":
                notes.append("Laminated Plywood maps to plywood_hardwood as a best fit; the LUT has no "
                             "laminated family. The PDF is the same catalog page 119 as Laminated Chipboard; "
                             "this row reads the second table on the page.")
                oid = oid.replace(f"x-g2-onsrud-{fam}", "x-g2-onsrud-lamply")
            if sid == "onsrud_laminated_chipboard_cutting_data":
                notes.append("Laminated Chipboard maps to particleboard (coated); the reconciler decides.")
                oid = oid.replace(f"x-g2-onsrud-{fam}", "x-g2-onsrud-lamchip")
            o["observation_id"] = oid.replace("/", "-")
            o["notes"] = " ".join(notes)
            o["extrapolation_group"] = "G2"
            o["verbatim"] = raw.rstrip()
            rows.append(o)

# ---- Amana PCD ball nose v2 (DRB-432 1/4 in, DRB-433 3/8 in) -------------
pcd = text("amana_pcd_ball_nose_v2.txt").split("\n")
for raw in pcd:
    m = re.match(r"\s*(MDF|Chipboard Without Coating)\s+(\d/\d)\" \(([\d.]+)mm\)\s+(\d)\s+"
                 r"\.(\d+)\" \(([\d.]+)mm\)\s+\.(\d+)\" \(([\d.]+)mm\)\s+([\d,]+)\s+([\d,]+)", raw)
    if not m:
        continue
    matl, dfrac, dmm_p, z, lo_s, lo_mm, hi_s, hi_mm, rlo, rhi = m.groups()
    fam = "mdf" if matl == "MDF" else "particleboard"
    lo, hi = float("." + lo_s), float("." + hi_s)
    tool = "DRB-432" if dfrac == "1/4" else "DRB-433"
    rows.append({
        "observation_id": f"x-g2-amana-pcd-ball-{fam}-{int(round(float(dmm_p) * 1000))}",
        "source_id": "amana_pcd_ball_nose_v2",
        "source_vendor": "amana",
        "source_title": "Amana Polycrystalline Diamond (PCD) Ball Nose Router Bit Speed Chart v2",
        "source_url": "https://www.amanatool.com/pub/media/productattachments/PCD-Ball-Nose-Speed-Chart-v2.pdf",
        "accessed_on": ACCESSED,
        "source_page": f"single page, Tool No. {tool}, row '{matl}'",
        "evidence_grade": "a",
        "row_kind": "exact",
        "tool_family": "ball_nose",
        "tool_subfamily": "pcd",
        "operation_family": "pocket",
        "pass_role": "roughing",
        "material_family": fam,
        "material_label": "MDF (Amana PCD chart)" if fam == "mdf" else "chipboard without coating (Amana PCD chart)",
        "hardness_kind": "janka",
        "hardness_value": PROXY[fam],
        "diameter_mm": float(dmm_p),
        "flute_count": int(z),
        "rpm_min": float(rlo.replace(",", "")),
        "rpm_max": float(rhi.replace(",", "")),
        "chipload_min_mm_tooth": mm(lo),
        "chipload_max_mm_tooth": mm(hi),
        "ap_rule": "Depth of Cut: 1 x D use recommended feed rate; 2 x D reduce feed rate by 25%; 3 x D reduce feed rate by 50%",
        "ap_max_factor": 1.0,
        "notes": (f"Printed cell, transcribed {ACCESSED} (G2 fetch). The chart prints inch and mm; the mm "
                  f"fields here are inch x 25.4 (the chart's own mm column reads {lo_mm}-{hi_mm} mm). "
                  "PCD (diamond) tool, not solid carbide: the reconciler decides whether it may anchor a "
                  "carbide ball-nose query. The same table prints a 'Wood' row (no softwood / hardwood split), "
                  "which is not transcribed because it has no LUT family; it is the within-chart ratio anchor "
                  "(see FETCH_NOTES.md). pocket / roughing copies the printed amana-ball-*-v7 rows (a 1 x D chart). "
                  "hardness_value is the engine proxy; the chart prints no hardness."),
        "extrapolation_group": "G2",
        "verbatim": raw.rstrip(),
    })

# ---- Amana Spektra coated 2D/3D carving v6: ball nose, 'Wood, MDF, Sign-Foam' --
sp = text("amana_spektra_3d_profiling_v6.txt").split("\n")
SPEK = [
    # (block marker, flutes, [(size label, diameter in)], row regex)
    ("2 Flute Ball Nose", 2, [("1/4", 0.25)]),
    ("3 Flute Ball Nose", 3, [("1/32", 0.03125), ("1/8", 0.125)]),
    ("4 Flute Ball Nose", 4, [("1/16", 0.0625), ("1/8", 0.125)]),
    ("3 Flute Extra Long", 3, [("1/4", 0.25)]),
]
for marker, z, sizes in SPEK:
    start = next(i for i, l in enumerate(sp) if marker in l)
    li = next(i for i in range(start, len(sp)) if "Wood, MDF, Sign-Foam" in sp[i])
    raw = sp[li]
    cls = re.findall(r"(0?\.\d+)\" - (0?\.\d+)\"", raw)
    for (lab, d_in), (lo_s, hi_s) in zip(sizes, cls):
        lo, hi = float(lo_s), float(hi_s)
        sub = "spektra_3d_carving" if "Extra" not in marker else "spektra_3d_carving_extra_long"
        rows.append({
            "observation_id": f"x-g2-amana-spektra3d-ball-mdf-{int(round(mm(d_in) * 1000))}-{z}f",
            "source_id": "amana_spektra_3d_profiling_v6",
            "source_vendor": "amana",
            "source_title": "Amana Spektra Extreme Tool Life Coated 2D/3D Carving CNC Solid Carbide Router Bits, Feed and Chip Load Chart v6",
            "source_url": "https://www.amanatool.com/pub/media/productattachments/Spektra-Coated-3D-Profiling-Feed-Chip-Load-Chart-v6.pdf",
            "accessed_on": ACCESSED,
            "source_page": f"block '{marker}', {lab} in column, row 'Wood, MDF, Sign-Foam'",
            "evidence_grade": "a",
            "row_kind": "exact",
            "tool_family": "ball_nose",
            "tool_subfamily": sub,
            "operation_family": "pocket",
            "pass_role": "roughing",
            "material_family": "mdf",
            "material_label": "Wood, MDF, Sign-Foam (one printed row)",
            "hardness_kind": "janka",
            "hardness_value": PROXY["mdf"],
            "diameter_mm": mm(d_in),
            "flute_count": z,
            "rpm_nominal": 18000.0,
            "chipload_min_mm_tooth": mm(lo),
            "chipload_max_mm_tooth": mm(hi),
            "ap_rule": "Depth of Cut: 1 x D use recommended chip load; 2 x D reduce chip load by 25%; 3 x D reduce chip load by 50%",
            "ap_max_factor": 1.0,
            "notes": (f"Printed cell, transcribed {ACCESSED} (G2 fetch). The chart prints ONE row for "
                      "'Wood, MDF, Sign-Foam': the vendor gives MDF and wood the same band on this tool. Only "
                      "the mdf row is emitted; 'Wood' names no LUT family. "
                      + ("The block title is '4 Flute Ball Nose & Flat Bottom' (one band for both shapes). " if z == 4 else "")
                      + ("The block title is '3 Flute Extra Long Ball Nose & Flat Bottom' (one band for both shapes). " if "Extra" in marker else "")
                      + "NOT an independent witness: the stored ZrN 3D profiling chart (LUT source amana_zrn_3d_profiling) "
                      "prints the same 'Wood, MDF, Sign-Foam' values for the matching ZrN tools. LUT precedent: the ZrN rows "
                      "emit that one printed row as both mdf and softwood rows (amana-ball-*-zrn); this fetch emits mdf only. "
                      + "Operating RPM 18,000 as printed. pocket / roughing copies the printed amana-ball-*-v7 rows "
                      "(the chart states a 1 x D depth rule); the reconciler decides the families."),
            "extrapolation_group": "G2",
            "verbatim": raw.rstrip(),
        })

# ---- Amana spiral ball nose v7: printed sizes the LUT does not hold ------------
v7 = text("amana_spiral_ball_nose_v7.txt").split("\n")
V7_SIZES = [("1/16", 0.0625), ("1/8", 0.125), ("1/4", 0.25), ("3/8", 0.375), ("1/2", 0.5), ("5/8", 0.625), ("3/4", 0.75)]
HELD = {0.125, 0.25}  # the LUT holds the printed 1/8 and 1/4 rows (amana-ball-*-v7)
for raw in v7:
    m = re.match(r"\s*(Softwood|Hardwood|MDF)\s", raw)
    if not m:
        continue
    fam = {"Softwood": "softwood", "Hardwood": "hardwood", "MDF": "mdf"}[m.group(1)]
    cls = re.findall(r"(0\.\d+)\" - (0\.\d+)\"", raw)
    assert len(cls) == 7, raw
    for (lab, d_in), (lo_s, hi_s) in zip(V7_SIZES, cls):
        if d_in in HELD:
            continue
        rows.append({
            "observation_id": f"x-g2-amana-ball-v7-{fam}-{int(round(mm(d_in) * 1000))}-2f",
            "source_id": "amana_ball_nose_v7",
            "source_vendor": "amana",
            "source_title": "Amana Spiral Ball Nose Speed Chart v7",
            "source_url": "https://www.amanatool.com/pub/media/productattachments/Spiral-Ball-Nose-Speed-Chart-v7.pdf",
            "accessed_on": ACCESSED,
            "source_page": f"single page, row '{m.group(1)}', {lab} in column",
            "evidence_grade": "a",
            "row_kind": "exact",
            "tool_family": "ball_nose",
            "tool_subfamily": "solid_carbide",
            "operation_family": "pocket",
            "pass_role": "roughing",
            "material_family": fam,
            "material_label": f"{m.group(1)} (Amana ball nose chart v7)",
            "hardness_kind": "janka",
            "hardness_value": PROXY[fam],
            "diameter_mm": mm(d_in),
            "flute_count": 2,
            "rpm_nominal": 18000.0,
            "chipload_min_mm_tooth": mm(float(lo_s)),
            "chipload_max_mm_tooth": mm(float(hi_s)),
            "ap_rule": "Depth of Cut: 1 x D use recommended feed rate; 2 x D reduce feed rate by 25%; 3 x D reduce feed rate by 50%",
            "ap_max_factor": 1.0,
            "notes": (f"Printed cell, transcribed {ACCESSED} (G2 fetch) from the chart the LUT already cites. "
                      "The LUT holds only the 1/8 and 1/4 in columns (amana-ball-*-v7); these are the other "
                      "printed sizes, so the chart's MDF / softwood / hardwood ratio can be read at 7 sizes. "
                      "The stored text of this download is byte-identical to the LUT copy "
                      "(same pdf_sha256). The v6 and 2015 editions print the same wood and MDF values."),
            "extrapolation_group": "G2",
            "verbatim": raw.rstrip(),
        })

ids = [r["observation_id"] for r in rows]
assert len(ids) == len(set(ids)), "duplicate observation_id"
with open(os.path.join(G2, "candidate_rows.json"), "w", encoding="utf-8") as f:
    json.dump({"observations": rows}, f, indent=1, ensure_ascii=False)
    f.write("\n")

count = {}
for r in rows:
    count[r["source_id"]] = count.get(r["source_id"], 0) + 1
print(len(rows), "rows")
for k, v in sorted(count.items()):
    print(f"  {v:3d}  {k}")

with open(os.path.join(G2, ".stored_index.json"), "w") as f:
    json.dump({"stored": stored, "rows_per_source": count}, f, indent=1)
