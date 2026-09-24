#!/usr/bin/env python3
"""G3 (operation family / pass role) candidate rows.

Writes fetch/G3/candidate_rows.json from the stored chart texts in
fetch/G3/sources/. Every row carries the verbatim line from the stored text;
the script refuses to write if a verbatim line is not in its stored text.

Two row sets:
  1. Amana 2-flute spiral plunge with corner radius (46460 / 46462), wood
     columns. Printed chip loads -> row_kind exact, grade a. The chart names
     no operation and no pass role; the rows copy each printed cell into the
     families the engine routes a bull nose to (the Onsrud 77-100 precedent).
  2. IDC Woodcraft V-bit rows. The PDF prints feed, RPM and flutes, not a
     chip load. The chip load is computed here -> row_kind derived, grade c.
"""
import json
import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[1]
G3 = ROOT / "fetch" / "G3"
SRC = G3 / "sources"

IN = 25.4
ACCESSED = "2026-09-24"

# Engine Janka proxies, as in the LUT example row onsrud-hardwood-77-100-1_8-parallel.
JANKA = {"softwood": 600.0, "hardwood": 1450.0, "mdf": 1100.0}


def check_verbatim(stored: str, verbatim: str) -> None:
    text = (SRC / stored).read_text()
    if verbatim not in text:
        raise SystemExit(f"verbatim not found in {stored}: {verbatim!r}")


rows = []

# ---------------------------------------------------------------- bull nose
CR_SRC = {
    "source_id": "amana_corner_radius_spiral_plunge_2f",
    "source_vendor": "amana",
    "source_title": "Amana Tool 2 Flute Solid Carbide Spiral Plunge with Corner Radius Router Bit (speed chart)",
    "source_url": "https://toolstoday.com/content/ProductFile/Attachments/Solid-Carbide-Spiral-Plunge-w-Corner-Radius.pdf",
    "stored": "amana_corner_radius_spiral_plunge.txt",
}
# (diameter label, diameter in, tool ref, corner radius in, verbatim row)
CR_LINES = [
    ("1/4", 0.25, "46460", 1 / 16,
     '1/4" (0.25)       250" - 320" 0.007" - 0.009" 180" - 250" 0.005" - 0.007" 210" - 290" 0.006" - 0.008" 150" - 210" 0.004" - 0.006" 150" - 210" 0.004" - 0.006"'),
    ("1/2", 0.50, "46462", 1 / 8,
     '1/2" (0.50)       330" - 425" 0.009" - 0.011" 240" - 330" 0.007" - 0.009" 280" - 390" 0.008" - 0.010" 200" - 280" 0.006" - 0.008" 200" - 280" 0.006" - 0.008"'),
]
# Printed column order: Soft Wood, Hard Wood, MDF, Plastics, Aluminum.
CR_BANDS = {
    "1/4": {"softwood": (0.007, 0.009), "hardwood": (0.005, 0.007), "mdf": (0.006, 0.008)},
    "1/2": {"softwood": (0.009, 0.011), "hardwood": (0.007, 0.009), "mdf": (0.008, 0.010)},
}
LABEL = {"softwood": "Soft Wood", "hardwood": "Hard Wood", "mdf": "MDF"}
# Families the engine routes a bull nose to (registry.rs feeds_family / feeds_pass_role):
# DropCutter/RampFinish/RadialFinish/HorizontalFinish -> parallel finish;
# Trace/ProjectCurve/Pencil -> trace finish; Scallop/SpiralFinish/UnifiedFinish -> scallop finish;
# Profile -> contour roughing; Pocket -> pocket roughing; Adaptive/Adaptive3d -> adaptive roughing.
CR_FAMILIES = [
    ("parallel", "finish"),
    ("trace", "finish"),
    ("scallop", "finish"),
    ("contour", "roughing"),
    ("pocket", "roughing"),
    ("adaptive", "roughing"),
]
AP_RULE = ("Depth of Cut: 1 x D Use recommended chip load; 2 x D Reduce chip load by 25%; "
           "3 x D Reduce chip load by 50% (chart footnote)")

for dlabel, d_in, ref, r_in, verbatim in CR_LINES:
    check_verbatim(CR_SRC["stored"], verbatim)
    for mat, (lo, hi) in CR_BANDS[dlabel].items():
        cell = f'{lo:.3f}" - {hi:.3f}"'
        if cell not in verbatim:
            raise SystemExit(f"cell {cell} not in verbatim for {dlabel} {mat}")
        for fam, role in CR_FAMILIES:
            rows.append({
                "observation_id": f"x-g3-amana-cr-{ref}-{mat}-{fam}",
                "source_id": CR_SRC["source_id"],
                "source_vendor": CR_SRC["source_vendor"],
                "source_title": CR_SRC["source_title"],
                "source_url": CR_SRC["source_url"],
                "accessed_on": ACCESSED,
                "source_page": (f"single page; row {dlabel}\" (tool ref {ref}), "
                                f"column {LABEL[mat]} Chip Load Per Tooth: {lo:.3f}\"-{hi:.3f}\""),
                "evidence_grade": "a",
                "row_kind": "exact",
                "tool_family": "bull_nose",
                "tool_subfamily": "corner_radius",
                "operation_family": fam,
                "pass_role": role,
                "material_family": mat,
                "material_label": (f"{LABEL[mat]} (Amana chart column); tool {ref}, "
                                   f"{r_in:.4g} in corner radius x {d_in} in diameter, 2 flute up-cut spiral plunge"),
                "hardness_kind": "janka",
                "hardness_value": JANKA[mat],
                "diameter_mm": round(d_in * IN, 3),
                "flute_count": 2,
                "rpm_nominal": 18000.0,
                "chipload_min_mm_tooth": round(lo * IN, 4),
                "chipload_max_mm_tooth": round(hi * IN, 4),
                "ap_rule": AP_RULE,
                "ap_max_factor": 1.0,
                "machine_assumption": "CNC Operating Spindle Speed: 18,000 RPM (chart header)",
                "notes": (
                    "Printed cell, transcribed 2026-09-24 from fetch/G3/sources/amana_corner_radius_spiral_plunge.txt. "
                    "The sheet names no operation and no pass role; its only condition is Depth of Cut 1 x D at 18,000 RPM. "
                    f"This row copies the printed band into the {fam} family ({role}) because the engine routes a bull nose there; "
                    "the family choice is a mapping, not a printed statement (same convention as the Onsrud 77-100 rows). "
                    f"Corner radius {r_in:.4g} in = D/4; the schema has no corner-radius field. "
                    "hardness_value is the engine's Janka proxy for the material family; the sheet prints no hardness. "
                    "The ZrN (46460-Z) and Spektra (46460-K) charts print the same values at 1/4 in (and the Spektra chart at 1/2 in). "
                    "Derived observation, not printed: these bands equal the Amana 2 flute spiral ball nose v7 bands at 1/4 in and 1/2 in "
                    "for softwood, hardwood and MDF."
                ),
                "extrapolation_group": "G3",
                "verbatim": verbatim,
            })

# ------------------------------------------------------------------ V-bits (IDC, derived)
IDC_SRC = {
    "source_id": "idcwoodcraft_feeds_speeds_pdf",
    "source_vendor": "idcwoodcraft",
    "source_title": "CNC Router Bit Feeds & Speeds, Imperial (Inch) & Metric, provided by IDC Woodcraft (PDF, Carbide 3D forum mirror)",
    "source_url": "https://community.carbide3d.com/uploads/short-url/fwPIYiWQNjUx8eEwsA7qmYiLMxv.pdf",
    "stored": "idcwoodcraft_feeds_speeds_pdf.txt",
}
# (angle, cut dia in, flutes, feed ipm, rpm, stepover, final stepover in, verbatim)
# Column order of the V-BITS table: Cut Dia, # Flutes, Flute Length, Overall Length, Shank Dia,
# Side Angle, Feed (in/min), Plunge (in/min), Depth Per Pass, Step Over, Final Pass Stepover, Spindle (rpm), Router Dial.
IDC_VBITS = [
    (30, 0.25, 1, 35, 27000, "20%", 0.005,
     "30° V-bit 0.25         1      0.750       2.00      0.25      15        35         20       0.025 20%          0.005      27,000        5      NOW"),
    (60, 0.25, 2, 60, 22000, "20%", 0.005,
     "60° V-bit 0.25         2      0.216       2.00      0.25      30        60         20       0.05     20%       0.005      22,000        4      NOW"),
    (90, 0.25, 2, 45, 17000, "20%", 0.005,
     "90° V-bit 0.25         2      0.125       2.00      0.25      45        45         25        0.1     20%       0.005      17,000        3      NOW"),
]
IDC_FAMILIES = [("trace", "finish", "final (V-carve) pass"),
                ("pocket", "roughing", "clear pass"),
                ("adaptive", "roughing", "clear pass")]
for angle, d_in, z, feed, rpm, clear_so, final_so, verbatim in IDC_VBITS:
    check_verbatim(IDC_SRC["stored"], verbatim)
    cl_in = feed / (rpm * z)
    cl_mm = round(cl_in * IN, 4)
    for mat in ("softwood", "hardwood"):
        for fam, role, what in IDC_FAMILIES:
            rows.append({
                "observation_id": f"x-g3-idc-vbit-{angle}deg-{mat}-{fam}",
                "source_id": IDC_SRC["source_id"],
                "source_vendor": IDC_SRC["source_vendor"],
                "source_title": IDC_SRC["source_title"],
                "source_url": IDC_SRC["source_url"],
                "accessed_on": ACCESSED,
                "source_page": "FEEDS & SPEEDS - V-BITS table (imperial section)",
                "evidence_grade": "c",
                "row_kind": "derived",
                "tool_family": "chamfer_vbit",
                "tool_subfamily": "idc_vbit",
                "included_angle_deg": float(angle),
                "operation_family": fam,
                "pass_role": role,
                "material_family": mat,
                "material_label": "soft, medium and moderately hard wood (IDC preamble; no category column)",
                "hardness_kind": "janka",
                "hardness_value": JANKA[mat],
                "diameter_mm": round(d_in * IN, 3),
                "flute_count": z,
                "rpm_nominal": float(rpm),
                "chipload_min_mm_tooth": cl_mm,
                "chipload_max_mm_tooth": cl_mm,
                "ae_rule": f"Step Over {clear_so} (the starter-set table calls it Clear Pass Stepover); Final Pass Stepover {final_so} in (printed)",
                "machine_assumption": "benchtop CNC router (IDC preamble: 'average accepted values for benchtop CNC routers')",
                "notes": (
                    f"DERIVED, not printed: chip load = feed / (RPM x flutes) = {feed} / ({rpm} x {z}) = {cl_in:.6f} in = {cl_mm} mm. "
                    "The table prints ONE feed and ONE RPM per V-bit and two stepovers: a Clear Pass Stepover and a Final Pass Stepover. "
                    f"This row stands for the {what}; the clear pass and the final pass carry the same feed, so the same chip load. "
                    "The starter-set table in the same PDF prints different feeds for the 60 deg (40 in/min) and 90 deg (50 in/min) bits; this row uses the V-BITS section, which the metric section repeats (1524 and 1143 mm/min). "
                    "The table names no wood category; this row copies the value to softwood and hardwood. "
                    "Community retailer table for benchtop machines (grade c); the URL is a Carbide 3D forum upload, not idcwoodcraft.com."
                ),
                "extrapolation_group": "G3",
                "verbatim": verbatim,
            })

out = G3 / "candidate_rows.json"
out.write_text(json.dumps({"observations": rows}, indent=1, ensure_ascii=False) + "\n")
ids = [r["observation_id"] for r in rows]
assert len(ids) == len(set(ids)), "duplicate observation_id"
print(f"wrote {len(rows)} rows to {out}")
