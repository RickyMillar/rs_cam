#!/usr/bin/env python3
"""G5 (engaged geometry) candidate rows, transcribed from stored chart texts.

Run from any directory:  python3 g5_candidate_rows.py
Writes fetch/G5/candidate_rows.json.

Rules:
- Every number comes from a stored text under fetch/G5/sources/.
- The script finds the "verbatim" line in that text and stops when the line
  is not there, so a row cannot cite a line that the document does not print.
- Inch values convert to mm with x 25.4, rounded to 4 decimals. That is the
  only arithmetic on a chipload.
- Column positions in the layout text were checked against a rendered image
  of each PDF page on 2026-09-24 (see FETCH_NOTES.md).
"""
import json
import pathlib
import re

HERE = pathlib.Path(__file__).resolve().parent
G5 = HERE.parent / "fetch" / "G5"
SRC = G5 / "sources"
IN = 25.4


def mm(inch):
    return round(inch * IN, 4)


def find_line(stored, pattern):
    """Return the one stripped line of `stored` that matches `pattern`."""
    text = (SRC / stored).read_text()
    hits = [ln.strip() for ln in text.splitlines() if re.search(pattern, ln)]
    if len(hits) != 1:
        raise SystemExit(f"{stored}: pattern {pattern!r} matched {len(hits)} lines")
    return hits[0]


rows = []

# ---------------------------------------------------------------------------
# 1. Amana Insert V-Groove Speed Chart v16 (grade a where the mapping is 1:1).
#    Source text: amana_insert_v_groove_v16_raw.txt (pdftotext -raw, one line
#    per tool). Columns: Hardwood, Softwood, Plywood/Chipboard, MDF, Plastic,
#    Foam; each column is Feed Rate IPM, Chip Load Per Tooth, Ramp Down.
#    The chart prints no tool diameter and no depth of cut.
# ---------------------------------------------------------------------------
INSERT = dict(
    source_id="amana_insert_v_groove_v16",
    source_vendor="amana",
    source_title="Amana Insert V-Groove Speed Chart v16",
    source_url="https://www.amanatool.com/pub/media/productattachments/Insert-V-Groove-Speed-Chart-v16.pdf",
    accessed_on="2026-09-24",
)
line_re = re.compile(
    r'^(RC-\d+(?:/-M)?) (\d+)° (\d) ([\d,]+) '
    r'((?:\d+" \.\d+" \d+" ){5}\d+" \.\d+" \d+")$'
)
raw_lines = (SRC / "amana_insert_v_groove_v16_raw.txt").read_text().splitlines()
groups = {}
for ln in raw_lines:
    m = line_re.match(ln.strip())
    if not m:
        continue
    tool, angle, flutes, rpm, rest = m.groups()
    vals = re.findall(r'(\d+)" (\.\d+)" (\d+)"', rest)
    assert len(vals) == 6, ln
    cols = dict(zip(["hardwood", "softwood", "plywood", "mdf", "plastic", "foam"], vals))
    for mat in ["hardwood", "softwood", "plywood", "mdf"]:
        feed, chip, ramp = cols[mat]
        key = (int(angle), int(flutes), int(rpm.replace(",", "")), mat, chip)
        groups.setdefault(key, []).append((tool, int(feed), int(ramp), ln.strip()))

MAT_OUT = {
    "hardwood": [("hardwood", "exact", "a")],
    "softwood": [("softwood", "exact", "a")],
    "mdf": [("mdf", "exact", "a")],
    # The chart prints one "Plywood/Chipboard" column. It does not say which
    # plywood. The LUT precedent for a shared column (Spektra "Wood/Plywood")
    # is row_kind derived, grade b. The same rule applies here.
    "plywood": [("plywood_hardwood", "derived", "b"), ("plywood_softwood", "derived", "b")],
}
for (angle, flutes, rpm, mat, chip), tools in sorted(groups.items()):
    chip_in = float(chip)
    tool_ids = [t[0] for t in tools]
    feeds = sorted({t[1] for t in tools})
    # Derived check: feed / (rpm x flutes) against the printed chip load.
    implied = sorted({round(f / (rpm * flutes), 5) for f in feeds})
    consistent = all(abs(x - chip_in) / chip_in <= 0.25 for x in implied)
    for fam, kind, grade in MAT_OUT[mat]:
        g = grade if consistent else "b"
        note = (
            f"Printed cell: chip load {chip}\" per tooth at {rpm} RPM, {flutes} flute(s), "
            f"{angle} deg, tools {', '.join(tool_ids)}; printed feed {feeds} IPM. "
            "The chart prints no tool diameter and no depth of cut; diameter_mm is absent "
            "on purpose. The chart keys only on the included angle and the tool number. "
            f"Derived check (not printed): feed/(RPM x flutes) = {implied} in/tooth"
            + ("." if consistent else
               f", which does NOT agree with the printed {chip}\" (more than 25 % apart); "
               "grade lowered to b until the chart is read again.")
        )
        if mat == "plywood":
            note += " The printed column is 'Plywood/Chipboard'; the split into hardwood and softwood plywood is this row's mapping, not the chart's."
        rows.append(dict(
            observation_id=f"x-g5-amana-insert-{fam}-{angle}deg-{flutes}f-{rpm}rpm",
            **INSERT,
            source_page="single page; row(s) " + ", ".join(tool_ids) + f"; column {mat}",
            evidence_grade=g,
            row_kind=kind,
            tool_family="chamfer_vbit",
            tool_subfamily="insert_vgroove",
            included_angle_deg=float(angle),
            operation_family="trace",
            pass_role="finish",
            material_family=fam,
            material_label={"plywood": "Plywood/Chipboard", "mdf": "MDF",
                            "hardwood": "Hardwood", "softwood": "Softwood"}[mat],
            flute_count=flutes,
            rpm_nominal=float(rpm),
            chipload_max_mm_tooth=mm(chip_in),
            machine_assumption=f"chart RPM {rpm}; single printed value (no band)",
            notes=note,
            extrapolation_group="G5",
            verbatim=tools[0][3],
        ))

# ---------------------------------------------------------------------------
# 2. LMT Onsrud Cutting Data Recommendations, five wood sheets, 37-series
#    engraving and V-bottom tools. Heading: "Recommended Chip Load per Tooth
#    by Cutting Diameter (in)". Column positions checked on the rendered page.
#    Tool geometry (angle, flutes) from the Onsrud PCT-19 catalogue p. 20-22
#    (stored text onsrud_pct19_catalog_p20-22.txt).
# ---------------------------------------------------------------------------
SHEETS = [
    ("onsrud_hard_wood_cutting_data", "hardwood", "Hard Wood", "https://www.onsrud.com/images/Hard%20Wood.pdf"),
    ("onsrud_soft_wood_cutting_data", "softwood", "Soft Wood", "https://www.onsrud.com/images/Soft%20Wood.pdf"),
    ("onsrud_mdf_cutting_data", "mdf", "MDF", "https://www.onsrud.com/images/MDF.pdf"),
    ("onsrud_hard_plywood_cutting_data", "plywood_hardwood", "Hard Plywood", "https://www.onsrud.com/images/Hard%20Plywood.pdf"),
    ("onsrud_soft_plywood_cutting_data", "plywood_softwood", "Soft Plywood", "https://www.onsrud.com/images/Soft%20Plywood.pdf"),
]
# (series pattern, subfamily, angle or None, flutes, [(diam_in label, diam_in, lo, hi, rpm or None)], cut rule)
ONS = [
    (r"^\s*37-00/37-20\s", "onsrud_37_00_60deg", 60.0, 1,
     [("1/4", 0.25, 0.004, 0.006, None)]),
    (r"^\s*37-00/37-20\s", "onsrud_37_20_30deg", 30.0, 1,
     [("1/4", 0.25, 0.004, 0.006, None)]),
    (r"^\s*37-50\s", "onsrud_37_50_v_bottom_sc", 90.0, 2,
     [("3/16", 0.1875, 0.003, 0.006, None), ("1/4", 0.25, 0.003, 0.006, None),
      ("3/8", 0.375, 0.003, 0.006, None)]),
    (r"^\s*37-60\s", "onsrud_37_60_v_bottom_ct", 90.0, 2,
     [("3/8", 0.375, 0.004, 0.006, None), ("1/2", 0.5, 0.004, 0.006, None),
      ("3/4", 0.75, 0.006, 0.008, None), ("1", 1.0, 0.008, 0.010, None)]),
    (r"^\s*37-80\s", "onsrud_37_80_lettering_ct", None, 2,
     [("1", 1.0, 0.004, 0.006, None), ("1 1/4", 1.25, 0.004, 0.006, 16000.0),
      ("2", 2.0, 0.004, 0.006, 15000.0)]),
]
GEOM_NOTE = {
    "onsrud_37_00_60deg": "Catalogue PCT-19 p.20: 37-00 series, single flute, solid carbide, 60 deg, 1/4 in shank, tips 0.005-0.090 in. The chart prints the 37-00/37-20 value in the 1/4 column: the shank/body size, not the tip.",
    "onsrud_37_20_30deg": "Catalogue PCT-19 p.20: 37-20 series, single flute, solid carbide, 30 deg, 1/4 in shank, tips 0.005-0.090 in. The chart prints the 37-00/37-20 value in the 1/4 column: the shank/body size, not the tip.",
    "onsrud_37_50_v_bottom_sc": "Catalogue PCT-19 p.21: 37-50 series, two flute V bottom, solid carbide, cutting DIA 3/16, 1/4, 3/8 in; 'Designed for V grooving or beveling 90°.'",
    "onsrud_37_60_v_bottom_ct": "Catalogue PCT-19 p.21: 37-60 series, two flute V bottom, carbide tipped, cutting DIA 1/2, 3/4, 1 in; 'Designed for V grooving or beveling 90°.' The chart also prints a 3/8 column for 37-60 that the catalogue does not list.",
    "onsrud_37_80_lettering_ct": "Catalogue PCT-19 p.22: 37-80 series, two flute carbide tipped lettering bits: 37-82 1 in 60 deg, 37-87 1-1/2 in 90 deg, 37-92 2 in 120 deg, 37-97 2 in 140 deg. The chart prints 1, 1 1/4 and 2 in columns; the catalogue lists no 1 1/4 in tool. included_angle_deg is absent because one chart cell covers tools of several angles.",
}
for stored, fam, label, url in SHEETS:
    for pat, sub, angle, flutes, cells in ONS:
        verb = find_line(stored + ".txt", pat)
        cut = re.split(r"\s{2,}", verb)[1]
        for dlabel, d_in, lo, hi, rpm in cells:
            # The printed range must be on the line (Hard Plywood prints ".004 -.006").
            want = re.compile(rf"\.{int(round(lo * 1000)):03d}\s?-\s?\.{int(round(hi * 1000)):03d}")
            assert want.search(verb), (stored, verb, lo, hi)
            row = dict(
                observation_id=f"x-g5-onsrud-{sub.replace('onsrud_', '').replace('_', '-')}-{fam}-{dlabel.replace(' ', '-').replace('/', '_')}in",
                source_id=stored,
                source_vendor="onsrud",
                source_title=f"LMT Onsrud {label} Cutting Data Recommendations",
                source_url=url,
                accessed_on="2026-09-24",
                source_page=f"{label} sheet, 'Recommended Chip Load per Tooth by Cutting Diameter (in)', series row {verb.split()[0]}, column {dlabel}",
                evidence_grade="a",
                row_kind="exact",
                tool_family="chamfer_vbit",
                tool_subfamily=sub,
                operation_family="trace",
                pass_role="finish",
                material_family=fam,
                material_label=f"{label} (Onsrud sheet)",
                diameter_mm=mm(d_in),
                flute_count=flutes,
                chipload_min_mm_tooth=mm(lo),
                chipload_max_mm_tooth=mm(hi),
                ap_rule=f"Cut: {cut} (printed); sheet header: 1 x D use recommended chip load; 2 x D reduce 25 %; 3 x D reduce 50 %",
                notes=(
                    f"Printed cell {lo:.3f}-{hi:.3f} in/tooth in the {dlabel} in 'Cutting Diameter' column. "
                    "diameter_mm is the printed column heading (the tool's cutting diameter, i.e. the top of the V), "
                    "not an engaged width. " + GEOM_NOTE[sub]
                    + (f" Footnote on the cell: {rpm:.0f} RPM." if rpm else " The sheet prints no RPM for this cell.")
                    + " hardness_value is absent: the sheet prints no hardness."
                ),
                extrapolation_group="G5",
                verbatim=verb,
            )
            if angle is not None:
                row["included_angle_deg"] = angle
            if rpm is not None:
                row["rpm_nominal"] = rpm
            if cut in ("1/2 CED", "1/2 x D"):
                row["ap_max_factor"] = 0.5
            rows.append(row)

# ---------------------------------------------------------------------------
# 3. Amana 2 Flute Solid Carbide V-Groove Engraving 15/60/90 deg chart.
#    The 15/60 chart prints the same cells; this one is the superset.
#    Header: "Chip Load Per Tooth IPR**" with footnote "IPR** Inches per
#    revolution". The printed feed (50-125 IPM at 18,000 RPM) divided by RPM
#    gives 0.0028-0.0069 in/rev, i.e. 0.0014-0.0035 in/tooth on 2 flutes.
#    So the printed 0.003-0.007 is per revolution by the chart's own feed,
#    and per tooth by its column heading. Grade b until someone rules.
# ---------------------------------------------------------------------------
TWOF = "amana_15_60_90_vgroove_engraving_2f.txt"
for fam, label in [("softwood", "Soft Wood"), ("hardwood", "Hard Wood")]:
    verb = find_line(TWOF, rf"^\s*{label}\s")
    for angle in (15, 60, 90):
        rows.append(dict(
            observation_id=f"x-g5-amana-vgroove-engrave-2f-{fam}-{angle}deg",
            source_id="amana_15_60_90_vgroove_engraving_2f",
            source_vendor="amana",
            source_title="Amana 2 Flute Solid Carbide V-Groove Engraving 15, 60 & 90 Degree Router Bits speed chart",
            source_url="https://www.amanatool.com/pub/media/productattachments/15-60-90_Degree-V-Groove-Engraving-Speed-Chart.pdf",
            accessed_on="2026-09-24",
            source_page=f"single page; row {label}; column {angle} deg",
            evidence_grade="b",
            row_kind="exact",
            tool_family="chamfer_vbit",
            tool_subfamily="amana_2f_vgroove_engraving",
            included_angle_deg=float(angle),
            operation_family="trace",
            pass_role="finish",
            material_family=fam,
            material_label=label,
            flute_count=2,
            rpm_nominal=18000.0,
            chipload_min_mm_tooth=mm(0.003),
            chipload_max_mm_tooth=mm(0.007),
            ap_rule="Depth of Cut: 1 x Tool Diameter (printed); 2 x D reduce feed rate 25 %; 3 x D reduce 50 %",
            ap_max_factor=1.0,
            notes=(
                "Printed cell 0.003\"-0.007\" under 'Chip Load Per Tooth IPR**', footnote 'IPR** Inches per revolution', "
                "feed 50\"-125\" IPM at 18,000 RPM, 2 flutes. Derived check (not printed): 50/18000 = 0.0028 and "
                "125/18000 = 0.0069 in/rev, so the printed band matches per revolution; per tooth on 2 flutes it "
                "would be 0.0014-0.0035 in. Grade b because the chart contradicts itself on the unit. "
                "The chart prints no tool diameter (tool reference numbers only); diameter_mm is absent. "
                "The 15/60 chart (15-60-Degree-V-Groove-Engraving-Speed-Chart.pdf) prints the same cells for 15 and 60 deg."
            ),
            extrapolation_group="G5",
            verbatim=verb,
        ))

out = G5 / "candidate_rows.json"
out.write_text(json.dumps({"observations": rows}, indent=2, ensure_ascii=False) + "\n")
ids = [r["observation_id"] for r in rows]
assert len(ids) == len(set(ids)), "duplicate observation_id"
by_src = {}
for r in rows:
    by_src[r["source_id"]] = by_src.get(r["source_id"], 0) + 1
print(len(rows), "rows")
for k, v in sorted(by_src.items()):
    print(f"  {k}: {v}")
