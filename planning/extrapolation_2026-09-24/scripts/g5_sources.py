#!/usr/bin/env python3
"""G5: write fetch/G5/sources.json with the sha256 of every stored document.

pdf_sha256 is the hash of the downloaded PDF for a PDF source. For an HTML
or JS source it is the hash of the stored text file (the task rule); the
raw download's hash is in the text file's header and in coverage_notes.
"""
import hashlib
import json
import pathlib

G5 = pathlib.Path(__file__).resolve().parent.parent / "fetch" / "G5"


def sha(p):
    return hashlib.sha256((G5 / p).read_bytes()).hexdigest()


AM = "https://www.amanatool.com/pub/media/productattachments/"
PB = "https://www.precisebits.com/"
S = []


def add(source_id, vendor, title, url, stored, hashed, notes, kind="pdf"):
    e = dict(
        source_id=source_id, source_vendor=vendor, source_title=title,
        source_url=url, accessed_on="2026-09-24", stored_text=f"sources/{stored}",
        pdf_sha256=None, coverage_notes=notes,
    )
    if kind == "pdf":
        e["pdf_sha256"] = sha(hashed)
    else:
        raw = kind_raw[source_id]
        e["pdf_sha256"] = sha(f"sources/{stored}")
        e["raw_download_sha256"] = sha(f"pdf/{raw}")
        e["coverage_notes"] = (
            f"pdf_sha256 is the sha256 of the stored text ({kind} source); raw_download_sha256 "
            f"is the downloaded file pdf/{raw}. " + notes
        )
    S.append(e)


kind_raw = {
    "precisebits_tapered_ball_2f": "pb_taperedcarve250b2f_asp.html",
    "precisebits_vtip_2500": "pb_2500_scoreengrave_asp.html",
    "precisebits_vtip_scoreengrave": "pb_scoreengrave_asp.html",
    "precisebits_engraving_application": "pb_engravingtools_htm.html",
    "precisebits_feeds_and_speeds": "pb_precisefedsped_asp.html",
    "precisebits_calibrating_feeds": "pb_calibrating_feeds_n_speeds_htm.html",
    "precisebits_calc": "pb_calc.html",
    "precisebits_calcv10_js": "pb_calcv10.js",
    "precisebits_bullnose_2f_125": "pb_2fltbullnose125.html",
    "vectric_tool_database_v11": "vectric_tooldb_v11.html",
    "cnccookbook_vbit_form_milling": "cnccookbook_vbit.html",
    "whiteside_120_vgroove_collection": "whiteside_120-v-groove.html",
    "whiteside_1550_product": "whiteside_1550.html",
}

# --- Amana V-bit charts (PDF) ---
add("amana_insert_v_groove_v16", "amana", "Amana Insert V-Groove Speed Chart v16",
    AM + "Insert-V-Groove-Speed-Chart-v16.pdf", "amana_insert_v_groove_v16.txt",
    "pdf/Insert-V-Groove-Speed-Chart-v16.pdf",
    "Also stored as amana_insert_v_groove_v16_raw.txt (pdftotext -raw, one line per tool; the verbatim source). "
    "Prints per tool number: angle 40-160 deg, flutes 1 or 2, RPM, and Feed/Chip Load per tooth/Ramp for Hardwood, "
    "Softwood, Plywood/Chipboard, MDF, Plastic, Foam. Wood/plywood .0024 in; MDF .0047/.0048 in (about 2x wood). "
    "Prints NO tool diameter and NO depth of cut; keys only on angle and tool number. Column order checked on a "
    "rendered image. RC-1146 and RC-1101 (1 flute, 14,000 RPM, 90 IPM) print .0024 in, which disagrees with their "
    "own feed (90/14000 = .0064). 105 candidate rows. The manifest already lists this chart with no hash; the six "
    "amana_vbit.json rows that cite it do not match it (lut_discrepancies.md).")
add("amana_ams159_vgroove_v2", "amana", "Amana 18/30/45/60/90 Degree V-Groove Speed Chart v2 (AMS-159)",
    AM + "AMS-159-18-30-45-60-90-Degree-V-Groove-Speed-Chart-v2.pdf", "amana_ams159_vgroove_v2.txt",
    "pdf/AMS-159-18-30-45-60-90-Degree-V-Groove-Speed-Chart-v2.pdf",
    "Header 'Operating RPM: 18,000 / Depth of Cut: 1 x Tool Diameter'. One chip load per angle for all wood/plastic "
    "rows: 18/30/45 deg 1 flute 0.003-0.007 in; 60/90 deg 2 flute 0.003 in. No diameter, no engaged width. "
    "LUT rows from it match the chart (no candidates). The chart prints 'IPR* Inches per revolution / IPM** Inches per "
    "minute' with the asterisks swapped against its column headings.")
add("amana_spektra_engraving_v4", "amana", "Amana Spektra 15/30/45/120 Degree Engraving Speed Chart v4",
    AM + "Spektra-15-30-45-120-Degree-Engraving-Speed-Chart-v4.pdf", "amana_spektra_engraving_v4.txt",
    "pdf/Spektra-15-30-45-120-Degree-Engraving-Speed-Chart-v4.pdf",
    "Header '18,000 RPM / Depth of Cut: 1 x Tool Diameter'. Prints a tip width per angle (15 deg 0.005 in, 30 deg "
    "0.005-0.030 in, 45 deg 0.042 in, 120 deg 0.015 in) and one chip load band per angle (0.003-0.007 in; 120 deg "
    "0.002-0.006 in), the same for every material. No tool diameter, no depth-dependent value. LUT rows match (no "
    "candidates).")
add("amana_15_60_90_vgroove_engraving_2f", "amana",
    "Amana 2 Flute Solid Carbide V-Groove Engraving 15, 60 & 90 Degree Router Bits speed chart",
    AM + "15-60-90_Degree-V-Groove-Engraving-Speed-Chart.pdf", "amana_15_60_90_vgroove_engraving_2f.txt",
    "pdf/15-60-90_Degree-V-Groove-Engraving-Speed-Chart.pdf",
    "New to the repo. 2 flute, 18,000 RPM, 'Depth of Cut: 1 x Tool Diameter'. Soft Wood, Hard Wood (and plastics, "
    "solid surface): feed 50-125 IPM, 'Chip Load Per Tooth IPR**' 0.003-0.007 in for 15, 60 and 90 deg. The printed "
    "feed gives 0.0028-0.0069 in per REVOLUTION, i.e. half the printed band per tooth on 2 flutes: the unit is "
    "ambiguous, rows are grade b. No diameter. 6 candidate rows.")
add("amana_15_60_vgroove_engraving_2f", "amana",
    "Amana 2 Flute Solid Carbide V-Groove Engraving 15 & 60 Degree Router Bits speed chart",
    AM + "15-60-Degree-V-Groove-Engraving-Speed-Chart.pdf", "amana_15_60_vgroove_engraving_2f.txt",
    "pdf/15-60-Degree-V-Groove-Engraving-Speed-Chart.pdf",
    "Subset of the 15/60/90 chart with the same cells for 15 and 60 deg. Kept as a second copy; no rows of its own.")

# --- Onsrud (PDF; hashes equal the manifest's) ---
for sid, name, fname in [
    ("onsrud_hard_wood_cutting_data", "Hard Wood", "Hard_Wood"),
    ("onsrud_soft_wood_cutting_data", "Soft Wood", "Soft_Wood"),
    ("onsrud_mdf_cutting_data", "MDF", "MDF"),
    ("onsrud_hard_plywood_cutting_data", "Hard Plywood", "Hard_Plywood"),
    ("onsrud_soft_plywood_cutting_data", "Soft Plywood", "Soft_Plywood"),
]:
    add(sid, "onsrud", f"LMT Onsrud {name} Cutting Data Recommendations",
        "https://www.onsrud.com/images/" + name.replace(" ", "%20") + ".pdf",
        f"{sid}.txt", f"pdf/onsrud_{fname}.pdf",
        "Same file as the manifest entry (sha256 identical, text identical to the LUT copy). G5 use: the 37-series "
        "rows (37-00/37-20 engraving, 37-50 and 37-60 90 deg V bottom, 37-80 lettering bits) under 'Recommended Chip "
        "Load per Tooth by Cutting Diameter (in)', with a 'Cut' column of '1/2 CED' / '1/2 x D' (37-50, 37-60) or "
        "'Varies' (37-00/37-20, 37-80). The chip load is keyed on the cutting diameter (the top of the V) and rises "
        "with it for 37-60 (.004-.006 at 3/8 and 1/2 in, .006-.008 at 3/4, .008-.010 at 1 in). Columns checked on a "
        "rendered image of every sheet. These rows are not in the LUT. 12 candidate rows.")
add("onsrud_pct19_catalog", "onsrud", "LMT Onsrud Production Cutting Tools Catalog PCT-19 (pages 20-22)",
    "https://onsrud.com/images/LMT%20Onsrud%20Product%20Cutting%20Tools%20Catalog%20PCT-19.pdf",
    "onsrud_pct19_catalog_p20-22.txt", "pdf/onsrud_PCT-19.pdf",
    "132-page catalogue; stored text is PDF pages 20-22 only. Gives the 37-series geometry the cutting-data sheets "
    "omit: 37-00 60 deg and 37-20 30 deg single flute, 1/4 in shank, tips 0.005-0.090 in; 37-50 (3/16, 1/4, 3/8 in) "
    "and 37-60 (1/2, 3/4, 1 in) two flute 'Designed for V grooving or beveling 90°'; 37-80 two flute lettering bits "
    "37-82 1 in 60 deg, 37-87 1-1/2 in 90 deg, 37-92 2 in 120 deg, 37-97 2 in 140 deg. No chip load on these pages.")

# --- PreciseBits (HTML/JS: hash of the stored text) ---
add("precisebits_tapered_ball_2f", "precisebits", "PreciseBits 2, 3-flute Carbide Tapered Ball-Nose Carving tools (CM204, CM304)",
    PB + "products/carbidebits/taperedcarve250b2f.asp", "precisebits_tapered_ball_2f.txt", "",
    "Prints 'Stepdown (pass depth, depth per pass) recommended - 1X tip dia. / maximum - 2X tip dia.' and stepover "
    "0.08 x tip (finish), 0.40 x tip (roughing). Feedrate: 'Depends on the material being cut. Do the Sweetspot Test'. "
    "NO chip load, and no chip load keyed on tip or engaged diameter.", kind="html")
add("precisebits_vtip_2500", "precisebits", "PreciseBits V-Groove Router Bits EM2E4 (1/4 in shank, 2-flute, V-tip)",
    PB + "products/carbidebits/2500_scoreengrave.asp", "precisebits_vtip_2500.txt", "",
    "0.015 in tip, 0.250 in max cutting diameter, 30/45/60/90 deg. Prints stepdown by material (Hardwood 0.118 in "
    "(3.0mm), Softwood 0.250 in (6.4mm), MDF '0.188 in. (3.0mm)' as printed, Plastic 0.118 in) and max DOC per angle. "
    "Feedrate by Sweetspot Test only. NO chip load.", kind="html")
add("precisebits_vtip_scoreengrave", "precisebits", "PreciseBits 2-flute Micro-engraving V-tip bits EM2E8 (0.125 in shank)",
    PB + "products/carbidebits/scoreengrave.asp", "precisebits_vtip_scoreengrave.txt", "",
    "0.005 in tip. Prints stepdown by material (Hardwood 0.020 in, Softwood 0.030 in, MDF 0.020 in) and the "
    "'Calculating engraving width and plunge depth' tutorial link. NO chip load.", kind="html")
add("precisebits_engraving_application", "precisebits", "PreciseBits Tools for Engraving and 3D Carving",
    PB + "applications/engravingtools.htm", "precisebits_engraving_application.txt", "",
    "Application overview; V-tip families by letter height. 'High flute volume ... support high chiploads'. No "
    "figure.", kind="html")
add("precisebits_feeds_and_speeds", "precisebits", "PreciseBits Feeds and Speeds for Woods, Thermoplastics, Composites",
    PB + "reference/precisefedsped.asp", "precisebits_feeds_and_speeds.txt", "",
    "States (Aug. 31, 2009) that a database 'will publish ... as soon as we can'. No figure.", kind="html")
add("precisebits_calibrating_feeds", "precisebits", "PreciseBits Finding the Sweet Spot When Machining with Carbide Microtools",
    PB + "tutorials/calibrating_feeds_n_speeds.htm", "precisebits_calibrating_feeds.txt", "",
    "Test procedure, not a recommendation: plunge Z = 2 x D softwood, 1 x D hardwood; start feed 'F = 0.03 x D x No. "
    "flutes x RPM (3% chipload per flute)' for woods; sweet spot = 0.75 x Fmax. D is 'the diameter of the bit being "
    "tested' (straight bits). No V-bit or tapered rule.", kind="html")
add("precisebits_calc", "precisebits", "PreciseBits Feed and Speed Calculators (page)",
    PB + "calc", "precisebits_calc.txt", "",
    "Prints the V width formula '(TAN(Half Angle) x 2 x Cutting Depth) + (Runout + Tip Diameter) = Cutting Width' "
    "and a chip-thinning calculator (Tool Diameter, Radial Width). Marked BETA.", kind="html")
add("precisebits_calcv10_js", "precisebits", "PreciseBits calculator script calcv10.js (excerpts)",
    PB + "scripts/calcv10.js", "precisebits_calcv10_excerpt.txt", "",
    "Code, not a chart. Chip thinning: corrected = chip x D / (2 sqrt(D a - a^2)) - runout (radial only; the same "
    "algebra as feeds::geometry::radial_chip_thinning_factor). Janka: new feed = (J_tested / J_new) x tested feed "
    "(linear in Janka; the repo scales by a square root; note for G2h). No V-bit depth rule.", kind="js")
add("precisebits_bullnose_2f_125", "precisebits", "PreciseBits 2-flute Carbide Corner-radius (Bull-nose) End-mills MM208",
    PB + "products/carbidebits/2fltbullnose125.asp", "precisebits_bullnose_2f_125.txt", "",
    "1/16 in (r 0.011 in) and 1/8 in (r 0.023 in) bull-nose. 'smooth bottom surface at higher chiploads than ball-nose "
    "cutters'. NO chip load and no effective-diameter rule.", kind="html")

# --- Others ---
add("whiteside_120_vgroove_collection", "whiteside", "Whiteside 120 V-Groove Collection",
    "https://www.whitesiderouterbits.com/collections/120-v-groove", "whiteside_120_vgroove_collection.txt", "",
    "Prints 'Recommended RPM: 14,000-16,000 (max 18,000)' and sizes 3/4 in CD and 1-1/2 in CD. NO chip load and no "
    "12 mm size. The two LUT rows that cite this page with chip load bands are not on it (lut_discrepancies.md).", kind="html")
add("whiteside_1550_product", "whiteside", "Whiteside 1550 60 deg V-Groove Bit",
    "https://www.whitesiderouterbits.com/products/1550", "whiteside_1550_product.txt", "",
    "Prints 'Recommended RPM: 16,000-20,000 (max 24,000)', 1/2 in CD, 7/16 in point length. No chip load.", kind="html")
add("vectric_tool_database_v11", "vectric", "Vectric VCarve Pro V11 Help: Tool Database",
    "https://docs.vectric.com/docs/V11.0/VCarvePro/ENU/Help/form/Tool%20Database/index.html",
    "vectric_tool_database_v11.txt", "",
    "Grade c context. 'Chip Load ... calculated ... based on the entered values for the Number of Flutes, Spindle Speed "
    "and Feed Rate'; the V-bit 'Diameter' is the tool's own diameter. No engaged-width rule, no figure.", kind="html")
add("cnccookbook_vbit_form_milling", "cnccookbook", "CNCCookbook: Form Milling, V-Bits, Dovetails, Roundover Mills",
    "https://www.cnccookbook.com/v-carve-chamfer-dovetail-corner-rounder-round-over-bit/",
    "cnccookbook_vbit_form_milling.txt", "",
    "Grade c (a software vendor's blog). States 'the \"diameter\" of your cutter is, well, not the diameter in the cut!' "
    "and 'The effective diameter in an 0.010\" depth of cut is only 0.020\"' (90 deg V); it uses the effective "
    "diameter for surface speed/RPM in G-Wizard. No vendor figure.", kind="html")

(G5 / "sources.json").write_text(json.dumps(S, indent=2, ensure_ascii=False) + "\n")
for e in S:
    assert (G5 / e["stored_text"]).exists(), e["stored_text"]
print(len(S), "sources")
