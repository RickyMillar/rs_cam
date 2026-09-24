#!/usr/bin/env python3
"""G6 (drill): write fetch/G6/candidate_rows.json from the stored texts.

Every number that enters a row is typed here next to the verbatim text
that carries it. The script does the unit arithmetic only:
  - in -> mm: x 25.4
  - feed speed vf (m/min) at spindle speed n (1/min) with Z teeth:
    feed per tooth (mm) = vf * 1000 / (n * Z)   [derived]
  - Amana "Ramp Down" (IPM) at the chart's 18,000 RPM with z flutes:
    axial advance per tooth (in) = ramp / (18000 * z)  [derived]
The script asserts that each verbatim string occurs in its stored text.

Usage: python3 g6_candidate_rows.py   (run from any directory)
"""
import json
import os

HERE = os.path.dirname(os.path.abspath(__file__))
G6 = os.path.join(HERE, "..", "fetch", "G6")
SRC = os.path.join(G6, "sources")
ACCESSED = "2026-09-24"
IN = 25.4

SOURCES = {
    "onsrud_drill_cutting_data": (
        "onsrud",
        "LMT Onsrud Drill Cutting Data Recommendations (catalogue page 124)",
        "https://www.onsrud.com/images/Drill.pdf",
    ),
    "leitz_lexicon7_06_drilling": (
        "leitz",
        "Leitz Lexicon Edition 7, chapter 6 Drilling (Version 2, 03/2026)",
        "https://www.leitz.org/fileadmin/Downloads/Lexicon/EN/Leitz_Lexicon_Edition_7_-_06_Drilling.pdf",
    ),
    "cmt_311_71_72_hwm_dowel_drill": (
        "cmt",
        "CMT Solid Carbide Dowel Drills - LONG LIFE SHARPENING 311.71/72HWM (product page)",
        "https://www.cmtorangetools.com/eu-en/industrial-boring-bits/solid-carbide-dower-drill-hwm",
    ),
    "amana_spektra_spiral_plunge_v24": (
        "amana",
        "Amana Solid Carbide Spektra Spiral Plunge 2/3 Flute Chart v24",
        "https://www.amanatool.com/pub/media/productattachments/Solid-Carbide-Spektra-Spiral-Plunge-2-3-Flute-v24.pdf",
    ),
    "precisebits_fret_plane": (
        "precisebits",
        "PreciseBits Hardwood Fingerboard Planing and Radiusing 3-flute up-cut end mills (product page)",
        "https://www.precisebits.com/products/carbidebits/fret-plane.asp",
    ),
}

_text_cache = {}


def check_verbatim(source_id, verbatim):
    path = os.path.join(SRC, source_id + ".txt")
    if path not in _text_cache:
        _text_cache[path] = open(path, encoding="utf-8").read()
    text = _text_cache[path]
    for part in verbatim.split(" ... "):
        if part not in text:
            raise SystemExit(f"verbatim not found in {source_id}: {part!r}")


def base(oid, source_id, page, grade, kind, tool_family, subfamily,
         op_family, role, material, label, diameter, flutes, verbatim, notes):
    vendor, title, url = SOURCES[source_id]
    check_verbatim(source_id, verbatim)
    return {
        "observation_id": oid,
        "source_id": source_id,
        "source_vendor": vendor,
        "source_title": title,
        "source_url": url,
        "accessed_on": ACCESSED,
        "source_page": page,
        "evidence_grade": grade,
        "row_kind": kind,
        "tool_family": tool_family,
        "tool_subfamily": subfamily,
        "operation_family": op_family,
        "pass_role": role,
        "material_family": material,
        "material_label": label,
        "diameter_mm": diameter,
        "flute_count": flutes,
        "extrapolation_group": "G6",
        "verbatim": verbatim,
        "notes": notes,
    }


rows = []

# ---------------------------------------------------------------- Onsrud 72-000
ONSRUD_LINE = ("72-000*        Wood                          .009-.011"
               "                                   .011-.013       .013-.015"
               "                                     .015-.017")
ONSRUD_FOOT = "* Gang drills run at 4,500 RPM and 150 IPM"
for d_mm, lo, hi in [(3, .009, .011), (5, .011, .013), (6, .013, .015), (8, .015, .017)]:
    r = base(
        f"x-g6-onsrud-72000-wood-{d_mm}mm", "onsrud_drill_cutting_data",
        "Drill Cutting Data Recommendations, catalogue page 124, series 72-000*, "
        f"material Wood, {d_mm} mm column: {lo:.3f}-{hi:.3f} in/tooth".replace("0.", "."),
        "b", "derived", "brad_point_drill", "72_000_series", "drill", "roughing",
        "hardwood",
        "Wood (Onsrud drill sheet prints one 'Wood' row); series 72-000 solid carbide boring bit",
        float(d_mm), 2,
        ONSRUD_LINE + " ... " + ONSRUD_FOOT,
        "The chip load band is printed; the family assignment is not (row_kind derived, "
        "grade b, by the R5 rule for a shared column). One 'Wood' row for all wood. The sheet splits no "
        "species or category; the catalogue icons for 72-000 are SW HW CW LW "
        "(onsrud_pct19_catalog.txt page 90). material_family hardwood is a placeholder "
        "for the reconciler: the R5 precedent (Spektra Wood/Plywood column) encodes a "
        "shared column as derived grade b per family. flute_count 2 is printed in the "
        "72-000 product tables (catalogue page 90). CONFOUND that must travel with the "
        "number: the row is footnoted 'Gang drills run at 4,500 RPM and 150 IPM' - a "
        "rigid multi-spindle borer with short brad-point bits, not a router collet. "
        "150/(4500 x 2) = 0.0167 in/tooth sits in the printed 8 mm band. The tool is a "
        "brad-point / through-hole boring bit, NOT a router end mill: this row must not "
        "serve an EndMill plunge. The sheet prints no peck or hole-depth guidance.",
    )
    r["chipload_min_mm_tooth"] = round(lo * IN, 4)
    r["chipload_max_mm_tooth"] = round(hi * IN, 4)
    r["rpm_nominal"] = 4500.0
    r["machine_assumption"] = "gang drill (multi-spindle borer), 4,500 RPM and 150 IPM per the footnote"
    rows.append(r)

# ---------------------------------------------------------------- Leitz Lexicon
# (oid suffix, pdf page, printed page, heading, Z, diameter text, material,
#  vf m/min, n 1/min, verbatim, extra note)
LEITZ = [
    ("hs-twist-z2-softwood", 35, 33, "6.4.1 Twist drills, HS solid, Z 2 / V 2", 2,
     "table D 3-12 mm; the plot legend prints three bands: Ø 3 … 8, Ø 8 … 12, Ø 12 … 16",
     "softwood", 1.2, 2500,
     "HS solid, Z 2 / V 2 ... Softwood ... Hardwood = 0.7 ... RPM: n = 1500 - 4000 min-1",
     "The red example line meets the plot inside the overlapping bands; the page does "
     "not print which diameter band the example belongs to. Hardwood correction 0.7 is printed."),
    ("hw-twist-z2-heel-softwood", 37, 35, "6.4.1 Twist drills, HW solid, Z 2 / V2, with heel", 2,
     "table D 6-16 mm (GL 105/130/150)",
     "softwood", 1.5, 4500,
     "HW solid, Z 2 / V2, with heel ... Hardwood = 0.8 ... Laminated veneer lumber = 1.1",
     "Two plotted bands: Hirnholz (end grain) and Quer zur Faser (across the grain); the page "
     "does not print which band the red example reads. Printed corrections: hardwood 0.8, "
     "laminated veneer lumber 1.1."),
    ("hw-marathon-z2-softwood-d-le6", 38, 36, "6.4.1 Twist drills, HW solid, Z 2 / V 2, Marathon", 2,
     "Diameter: D ≤ 6 mm", "softwood", 3.0, 4500,
     "HW solid, Z 2 / V 2, Marathon ... For drilling very deep holes without interim clearance strokes",
     "Printed corrections: hardwood 0.8, laminated veneer lumber 1.2. The tool is sold for "
     "very deep holes without interim clearance strokes."),
    ("hw-marathon-z2-softwood-d6-12", 39, 37, "6.4.1 Twist drills, HW solid, Z 2 / V 2, Marathon (continued)", 2,
     "Diameter: D = 6 - 12 mm", "softwood", 4.5, 4500,
     "D = 6 - 12 mm ... D > 12 mm ... Hardwood = 0.8",
     "Z 2 is printed on the facing page (PDF page 38) for this tool. Printed corrections: "
     "hardwood 0.8, laminated veneer lumber 1.2."),
    ("hw-marathon-z2-softwood-d-gt12", 39, 37, "6.4.1 Twist drills, HW solid, Z 2 / V 2, Marathon (continued)", 2,
     "Diameter: D > 12 mm", "softwood", 3.5, 4500,
     "D > 12 mm",
     "Z 2 is printed on the facing page (PDF page 38). Printed corrections: hardwood 0.8, "
     "laminated veneer lumber 1.2."),
    ("hw-vpoint-z2-softwood", 41, 39, "6.4.1 Twist drills, HW solid, Z 2, V-point", 2,
     "table D 7-12 mm", "softwood", 1.2, 4500,
     "HW solid, Z 2, V-point ... Hardwood = 0.8 ... Laminated veneer lumber = 1.1",
     "Two plotted bands (Hirnholz, Quer zur Faser); the band of the red example is not printed."),
    ("hw-vpoint-marathon-z2-softwood-d6-12", 42, 40, "6.4.1 Twist drills, HW solid, Z 2, V-point, Marathon", 2,
     "Diameter: D = 6 - 12 mm", "softwood", 3.5, 4500,
     "HW solid, Z 2, V-point, Marathon ... For drilling very deep holes without interim clearance strokes at high feed speed",
     "Operation printed as 'Drilling, through hole'. Printed corrections: hardwood 0.8, "
     "laminated veneer lumber 1.2."),
    ("hs-levin-z1-solidwood", 43, 41, "6.4.2 Levin type drills, HS solid, Z 1", 1,
     "table D 5-12 mm", "softwood", 1.5, 4500,
     "HS solid, Z 1 ... Suitable for depths up to approx. 4 x D without interim ... Drilling depth > 4 x D = 0.8",
     "Printed material is 'Solid wood' (no softwood/hardwood split); softwood is a placeholder. "
     "Printed correction: drilling depth > 4 x D = 0.8."),
    ("hw-levin-z1-solidwood", 44, 42, "6.4.2 Levin type drills, HW, Z 1 / V 1", 1,
     "table D 12-16 mm", "softwood", 1.5, 4500,
     "HW, Z 1 / V 1 ... Suitable for depths up to 75 mm without interim clearance ... Drilling depth > 4 x D = 0.8",
     "Printed material is 'Solid wood'; softwood is a placeholder. Printed correction: "
     "drilling depth > 4 x D = 0.8."),
    ("dowel-excellent-z2-chipboard-coated", 12, 10, "6.1.3 Dowel drills - Excellent, shank 10 mm, HW solid", 2,
     "table D 3-10 mm (GL 57.5 and 70)", "particleboard", 2.0, 4500,
     "Shank 10 mm, HW solid ... Chipboard plastic coated ... MDF, solid wood = 0.7 ... Chipboard, uncoated = 1.3 ... RPM: n = 3000 - 12000 min-1",
     "Printed base material is 'Chipboard plastic coated'. The same red example (2 m/min at "
     "4500 1/min) is printed on all six dowel-drill diagrams (PDF pages 6, 7, 9, 10, 11, 12)."),
]
for sfx, pdfp, prp, head, z, dtext, mat, vf, n, verb, extra in LEITZ:
    fz = vf * 1000.0 / (n * z)
    r = base(
        f"x-g6-leitz-{sfx}", "leitz_lexicon7_06_drilling",
        f"PDF page {pdfp} (printed page {prp}), {head}; diagram 'Feed speed vf depending on the "
        f"spindle RPM n', red worked example {str(vf).rstrip('0').rstrip('.').replace('.', ',')} m/min at {n} 1/min; {dtext}",
        "b", "derived", ("brad_point_drill" if "dowel" in sfx else "levin_drill" if "levin" in sfx else "twist_drill"),
        "leitz_" + sfx.split("-")[0] + "_" + sfx.split("-")[1], "drill", "roughing",
        mat, ("Chipboard PLASTIC COATED (Leitz base material; not raw particleboard, see the x1.3 uncoated row)" if mat == "particleboard" else "Softwood / solid wood (Leitz diagram base material; 'Solid wood' pages have no softwood/hardwood split, softwood is the placeholder)"),
        None, z, verb,
        f"DERIVED chip load: the page prints feed speed vf = {vf} m/min at n = {n} 1/min as the "
        f"red worked example (text layer, see leitz_lexicon7_06_drilling_red_spans.txt) and Z = {z} "
        f"in the tool heading. vf*1000/(n*Z) = {fz:.4f} mm/tooth. The page prints no chip load. "
        "chipload_min = chipload_max = the one example value; the plotted band edges are "
        "graphics, not text, and are not transcribed. diameter_mm is null because the diagram "
        f"covers a diameter range ({dtext}). {extra} The tool is a wood drill, NOT a router end mill.",
    )
    r["chipload_min_mm_tooth"] = round(fz, 4)
    r["chipload_max_mm_tooth"] = round(fz, 4)
    r["rpm_nominal"] = float(n)
    r["vf_m_min_printed"] = vf
    rows.append(r)

# Derived MDF and solid-wood rows from the dowel-drill correction factor.
for mat, label, fac, ftxt in [("mdf", "MDF", 0.7, "MDF, solid wood = 0.7"),
                              ("softwood", "solid wood (softwood placeholder, as on the Levin rows)", 0.7, "MDF, solid wood = 0.7"),
                              ("particleboard", "chipboard uncoated", 1.3, "Chipboard, uncoated = 1.3")]:
    vf = 2.0 * fac
    fz = vf * 1000.0 / (4500 * 2)
    r = base(
        f"x-g6-leitz-dowel-excellent-z2-{mat}-by-factor", "leitz_lexicon7_06_drilling",
        "PDF page 12 (printed page 10), 6.1.3 Dowel drills - Excellent; red example 2 m/min at "
        f"4500 1/min times the printed correction '{ftxt}'",
        "b", "derived", "brad_point_drill", "leitz_dowel_excellent", "drill", "roughing",
        mat, f"{label} via the printed Leitz correction factor {fac} on chipboard plastic coated",
        None, 2,
        "Chipboard plastic coated ... " + ftxt,
        f"DERIVED twice: vf = 2 m/min x {fac} = {vf:.1f} m/min; {vf:.1f}*1000/(4500*2) = {fz:.4f} "
        "mm/tooth. Both inputs are printed; the product is not. Leitz prints one factor for "
        "'MDF, solid wood' and does not split softwood from hardwood on this page.",
    )
    r["chipload_min_mm_tooth"] = round(fz, 4)
    r["chipload_max_mm_tooth"] = round(fz, 4)
    r["rpm_nominal"] = 4500.0
    rows.append(r)

# ---------------------------------------------------------------- CMT 311.71/72
for mat in ["particleboard", "mdf"]:
    lo, hi = 1.0 * 1000 / (6000 * 2), 4.0 * 1000 / (6000 * 2)
    r = base(
        f"x-g6-cmt-311hwm-{mat}", "cmt_311_71_72_hwm_dowel_drill",
        "product page 311.71/72HWM, 'Technical details'",
        "b", "derived", "brad_point_drill", "cmt_311_71_72_hwm", "drill", "roughing",
        mat, "chipboard, MDF, HDF and laminates (CMT page prints one range for all)",
        None, 2,
        "Recommended feed speed 1÷ 4m/minute – RPM 6000. ... Ideal for chipboard, MDF, HDF and laminates. ... 2 cutting edges [Z2]",
        f"DERIVED chip load: 1-4 m/min at 6000 RPM, Z2 -> {lo:.4f}-{hi:.4f} mm/tooth. Diameters "
        "listed on the page: 5, 6, 7, 8, 10 mm (diameter_mm null: one range for all). Vendor "
        "product-page prose, not a chart: grade b. The page also says 'No center-point or "
        "spurs' and '2 curved ground spurs [V2]' - it contradicts itself on the point form.",
    )
    r["chipload_min_mm_tooth"] = round(lo, 4)
    r["chipload_max_mm_tooth"] = round(hi, 4)
    r["rpm_nominal"] = 6000.0
    rows.append(r)

# ---------------------------------------------------------------- Amana Ramp Down
# (flutes, diameter label, diameter mm, wood feed, wood cl, wood ramp,
#  mdf feed, mdf cl, mdf ramp, verbatim line fragment)
AMANA = [
    (2, "1/8", 3.175, "145\"     .0040\"     72.5\"     180\"       .0050\"      90\"", 72.5, 90.0, .0040, .0050),
    (2, "6mm", 6.0, "180\"     .0050\"     90\"       215\"       .0060\"     107.5\"", 90.0, 107.5, .0050, .0060),
    (3, "1/8", 3.175, "215\"     .0040\"     72\"       270\"      .0050\"       90\"", 72.0, 90.0, .0040, .0050),
    (3, "6mm", 6.0, "270\"     .0050\"     90\"       325\"      .0060\"      109\"", 90.0, 109.0, .0050, .0060),
]
for z, dlab, dmm, frag, ramp_w, ramp_m, cl_w, cl_m in AMANA:
    for mat, ramp, cl, col in [("plywood_hardwood", ramp_w, cl_w, "Wood/Plywood"),
                               ("mdf", ramp_m, cl_m, "MDF/Laminate")]:
        adv = ramp / (18000 * z)
        r = base(
            f"x-g6-amana-spektra-{z}f-{dlab.replace('/', '_')}-{mat}-rampdown",
            "amana_spektra_spiral_plunge_v24",
            f"Spektra Spiral Plunge chart v24, {z} Flute, {dlab} row, {col} 'Ramp Down' column: {ramp} IPM",
            "b", "derived", "flat_end", f"spektra_{z}f", "drill", "roughing",
            mat, f"{col} column (Amana Spektra v24)", dmm, z,
            frag + " ... To find Ramp Down: ... Feed Rate IPM / # of flutes ... CNC Operating Spindle Speed: 18,000 RPM",
            f"The chart prints 'Ramp Down' = {ramp} IPM (= Feed Rate / flutes). Amana does not print "
            "the word 'plunge' for this column; 'Ramp Down' is transcribed as printed. DERIVED: at the "
            f"chart's 18,000 RPM the axial advance per tooth is {ramp}/(18000 x {z}) = {adv:.5f} in "
            f"= {adv * IN:.4f} mm, i.e. the printed side chip load {cl} in divided by {z} flutes. "
            "chipload_* here carry that derived axial advance per tooth, not the side chip load. "
            "The side chip load of this row is already in the LUT (amana_flat_end.json). The "
            "Wood/Plywood column is one column for all wood and plywood (R5 precedent: derived per "
            "family); plywood_hardwood is a placeholder. This is the one printed figure that sits "
            "on the refused EndMill Drill cells (3.175 and 6.0 mm).",
        )
        r["chipload_min_mm_tooth"] = None
        r["chipload_max_mm_tooth"] = round(adv * IN, 4)
        r["rpm_nominal"] = 18000.0
        r["ramp_down_ipm"] = ramp
        rows.append(r)

# ---------------------------------------------------------------- PreciseBits
PB = [
    ("softwood", 600.0, 75, "Softwood (Janka < 1,500) = 75 inches/minute", "janka-lt1500"),
    ("hardwood", 1450.0, 75, "Softwood (Janka < 1,500) = 75 inches/minute", "janka-lt1500"),
    ("hardwood", 2000.0, 50, "Medium hardness hardwood(1,500 < Janka < 2,500) = 50 inches/minute", "janka-1500-2500"),
    ("hardwood", 2600.0, 40, "High hardness hardwoodwood (Janka > 2,500) = 40 inches/minute", "janka-gt2500"),
]
for mat, jv, ipm, band, tag in PB:
    r = base(
        f"x-g6-precisebits-fretplane-{mat}-{tag}-plunge", "precisebits_fret_plane",
        "product page 'Application Data', Plunge rate (based on wood hardness)",
        "b", "exact", "bull_nose", "precisebits_fret_plane_3f", "drill", "finish",
        mat, "wood by Janka (PreciseBits fingerboard page)", 3.175, 3,
        "Plunge rate (based on wood hardness): ... " + band + " ... Tip Radius (R) = 0.0250 inches (0.64mm)",
        f"Printed plunge FEED RATE {ipm} IPM = {ipm * IN:.0f} mm/min for one tool: a 3-flute "
        "radiused (bull-nose) end mill, D 0.125 in / 3.0 mm, tip radius 0.64 mm. No RPM is "
        "printed ('for bits this small, use your maximum RPM'), so no plunge chip load follows "
        "without an assumed RPM; chipload fields are null. The page bands by Janka, not by "
        f"softwood/hardwood: this row serves the printed band '{band}'. hardness_value {jv} is the "
        "engine's Janka proxy (GenericSoftwood 600, GenericHardwood 1450) or a point inside the "
        "printed band (2000 for 1,500-2,500; 2600 for > 2,500). The engine's generic hardwood "
        "(1450) falls in the < 1,500 band, so it reads 75 in/min, not 50. Vendor page for one "
        "application (fingerboard planing): grade b.",
    )
    r["chipload_min_mm_tooth"] = None
    r["chipload_max_mm_tooth"] = None
    r["plunge_feed_mm_min_printed"] = round(ipm * IN, 1)
    r["hardness_kind"] = "janka"
    r["hardness_value"] = jv
    rows.append(r)

# ---------------------------------------------------------------- depth statements
DEPTH = [
    ("leitz_lexicon7_06_drilling", "PDF page 36 (printed page 34), 6.4.1 Twist drills, HW, Z 2 / V 2, with heel",
     "When drilling holes with a depth greater ... than 4 x D interim clearance stroke is ... recommended!",
     "total hole depth above which a clearance stroke (peck retract) is recommended: 4 x D; base material chipboard plastic coated"),
    ("leitz_lexicon7_06_drilling", "PDF page 43 (printed page 41), 6.4.2 Levin type drills, HS solid, Z 1",
     "Suitable for depths up to approx. 4 x D without interim ... clearance strokes.",
     "single-flute Levin drill, softwood and hardwood: up to 4 x D without clearance strokes; feed correction 0.8 beyond 4 x D"),
    ("leitz_lexicon7_06_drilling", "PDF pages 43-44, Levin type drills, correction factor for vf",
     "Drilling depth > 4 x D = 0.8",
     "feed correction for holes deeper than 4 x D"),
    ("leitz_lexicon7_06_drilling", "PDF page 44 (printed page 42), 6.4.2 Levin type drills, HW, Z 1 / V 1",
     "Suitable for depths up to 75 mm without interim clearance",
     "D 12-16 mm Levin drill: up to 75 mm without clearance strokes (4.7-6.3 x D, derived)"),
    ("leitz_lexicon7_06_drilling", "PDF page 13 (printed page 11), 6.1.4 Boring pins, HW solid, Z 1/1",
     "pre-drilling screw holes in plastic coated and veneered furniture parts. Infeed depth in ... hardwood and glulam maximum 2 x D.",
     "boring pin (D 3 mm named): infeed depth in hardwood and glulam at most 2 x D"),
    ("leitz_lexicon7_06_drilling", "PDF page 13 (printed page 11), boring pins, Note",
     "When using the bore pins in hardwood and glulam, the potential bore depth is ... restricted. Interim chip removal (return stroke) then is obligatory.",
     "boring pins in hardwood and glulam: a return stroke is obligatory"),
    ("leitz_lexicon7_06_drilling", "chapter introduction, twist drills",
     "Solid tungsten carbide Z 2/V 2 design suitable for drilling deep holes in solid wood ... without interim clearance strokes and for high feed speeds.",
     "some HW twist drills are sold for deep holes in solid wood with no clearance stroke"),
    ("leitz_lexicon7_06_drilling", "chapter 6 troubleshooting table (row 'Chips and workpiece become hot')",
     "Tool too long at the reversal point       Reduce RPM or increase acceleration",
     "printed dwell warning: heat wear when the bit dwells at the reversal point"),
]
depth_statements = []
for sid, page, verb, meaning in DEPTH:
    check_verbatim(sid, verb)
    depth_statements.append({
        "source_id": sid, "source_page": page, "extrapolation_group": "G6",
        "verbatim": verb, "meaning": meaning,
        "note": "Printed vendor depth guidance. It is a total-hole-depth regime (4 x D before a "
                "clearance stroke) or a maximum infeed (2 x D, boring pins, hardwood), not a "
                "per-peck depth for a router end mill.",
    })

out = {"observations": rows, "depth_statements": depth_statements}
with open(os.path.join(G6, "candidate_rows.json"), "w", encoding="utf-8") as fh:
    json.dump(out, fh, indent=2, ensure_ascii=False)
    fh.write("\n")
print(len(rows), "rows,", len(depth_statements), "depth statements")
