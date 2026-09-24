#!/usr/bin/env python3
"""G10 fetch, part `metal`: write parts/metal_sources.json and parts/metal_statements.json.

The script holds the transcribed statements in one place. It computes the
text and raw hashes from the stored files, so a re-run after a re-fetch
keeps the hashes true. It asserts that every verbatim occurs in its stored
text (whitespace collapsed), the same test as g10_check_statements.py.

Usage: python3 g10_metal_build.py
"""
import hashlib
import json
import os
import re

HERE = os.path.dirname(os.path.abspath(__file__))
G10 = os.path.normpath(os.path.join(HERE, "..", "fetch", "G10"))
ACC = "2026-09-25"


def sha(path):
    with open(path, "rb") as f:
        return hashlib.sha256(f.read()).hexdigest()


def norm(s):
    return re.sub(r"\s+", " ", s).strip()


# source_id, vendor, title, url, raw file (relative to G10) or None, http_status, coverage
SOURCES = [
    ("metal_harvey_ramping_success", "Harvey Performance Company (Harvey Tool / Helical Solutions)",
     "Ramping to Success (In The Loupe blog, 2017-05-10, updated 2023-10-12)",
     "https://www.harveyperformance.com/in-the-loupe/ramping-success/",
     "html/metal_harvey_ramping_success.html", 200,
     "Prose. Prints suggested starting ramp angles by material class (soft/non-ferrous 3-10 deg, hard/ferrous 1-3 deg). "
     "No flute-count, diameter or centre-cutting split. A reader comment (not vendor) gives a 1 deg habit; not used."),
    ("metal_harvey_tool_entry", "Harvey Performance Company (Harvey Tool / Helical Solutions)",
     "Most Common Methods of Tool Entry (In The Loupe blog)",
     "https://www.harveyperformance.com/in-the-loupe/types-tool-entry/",
     "html/metal_harvey_tool_entry.html", 200,
     "Prose. Pre-drill 5-10% over D; helix diameter >110-120% of D; ramp angles as the ramping post; straight plunge needs a "
     "centre-cutting tool; straight and roll-in entry feed -50%. A vendor reply in the comments says helical entry also suits non-ferrous."),
    ("metal_helical_guidebook_2016", "Helical Solutions, LLC (Harvey Performance Company)",
     "Helical Machining Guidebook (c) 2016, 70 pages (mirror on web.mae.ufl.edu)",
     "https://web.mae.ufl.edu/designlab/Advanced%20Manufacturing/Helical_Machining_Guidebook.pdf",
     "pdf/metal_helical_guidebook_2016.pdf", 200,
     "Pages 4-7: Types of Tool Entry and Ramping. Same numbers as the blog posts, plus the sentence that centre-cutting and "
     "non-centre-cutting end mills can both ramp but the angle of descent varies with the end style. The PDF carries hidden "
     "earlier text layers (see NOTES section 5). The stored text has a -layout section and a -raw section. No per-tool ramp table."),
    ("metal_harvey_sf_18700", "Harvey Tool", "Speeds & Feeds: Chamfer Cutters - Pointed & Flat End, 2 Flutes, Type I (SF_18700)",
     "https://harveyperformance.widen.net/content/olyhfjr4yu/pdf/SF_18700.pdf?u=d9orjt",
     "pdf/metal_harvey_sf_18700.pdf", 200,
     "One-page chart, metals (aluminium alloys to hardened steel). Prints the vertical-plunge chip-load reduction for a chamfer cutter."),
    ("metal_harvey_sf_25000", "Harvey Tool", "Speeds & Feeds: Engraving Cutters - Pointed, 1 Flute, Pointed Tip (SF_25000)",
     "https://harveyperformance.widen.net/content/tfcdc7pipl/pdf/SF_25000.pdf?u=d9orjt",
     "pdf/metal_harvey_sf_25000.pdf", 200,
     "One-page chart. Prints the vertical-plunge chip-load reduction (50%) for a pointed (V) engraving cutter and prefers ramping."),
    ("metal_harvey_sf_809500", "Harvey Tool", "Speeds & Feeds: End Mills for Wood - Square Upcut, 3x Length of Cut (SF_809500)",
     "https://harveyperformance.widen.net/content/mrt84hhfmn/pdf/SF_809500.pdf?u=d9orjt",
     "pdf/metal_harvey_sf_809500.pdf", 200,
     "A wood chart (softer, harder, engineered woods incl. MDF, phenolic). It prints NO ramp, plunge or helix entry value. "
     "Kept as evidence of absence. The downcut chart SF_809100, the plastics chart SF_827700 and the aluminium chart SF_968700 "
     "were probed and also print no entry value (not stored)."),
    ("metal_harvey_helical_interp_tv", "Harvey Performance Company",
     "In the Loupe TV: Understanding Helical Interpolation (video page)",
     "https://www.harveyperformance.com/in-the-loupe/in-the-loupe-tv-helical-interpolation/",
     "html/metal_harvey_helical_interp_tv.html", 200,
     "Video page. The page text prints no number. A reader comment reports an on-screen caption '0.8 cutter diameter' for "
     "step-over. That is a third-party report of a video caption; no statement is taken from it."),
    ("metal_sgs_catalog_2021", "KYOCERA SGS Precision Tools",
     "2021 SGS Kyocera catalogue, excerpt pp. 15-121 (PDF of 20 pages: catalogue pages 16-35)",
     "https://asia.kyocera.com/products/cuttingtools/wp-content/uploads/2021/01/2021-SGS-Kyocera-Catalog-12-31-20-web_compressed-15-121.pdf",
     "pdf/metal_sgs_catalog_2021.pdf", 200,
     "End Mill Matrix (catalogue pp. 22-23): a Maximum Recommended Ramp Angle and a Center Cutting column for 26 series, "
     "incl. 5 router series. Entry Methods (p. 24): ramp feed against ramp angle, plunge feed = 25% of slotting feed. "
     "Z-Carb-HPR chart (p. 34): 'Do not plunge'. The stored text has a -layout section and a -raw section."),
    ("metal_imco_helical_ramp", "IMCO Carbide Tool",
     "Technical Resources: Helical Ramp to Create an Entry Hole (catalogue pp. 130-131)",
     "https://cms.imcousa.com/wp-content/uploads/2021/01/Helical-Ramp-PDF.pdf",
     "pdf/metal_imco_helical_ramp.pdf", 200,
     "The strongest helix source: bore diameter = 2D - 2r; helical ramp angle and feed per tool series; one step for <= 7 "
     "flutes, two steps for > 7 flutes; the expanded hole 3xD or 3.75xD for 7-13 flute tools. Material per series is not printed here."),
    ("metal_garr_technical", "GARR TOOL", "Technical Helps (catalogue technical section, 61 pages, printed pp. 283-)",
     "https://www.garrtool.com/wp-content/uploads/2018/11/TECHNICAL.pdf",
     "pdf/metal_garr_technical.pdf", 200,
     "General Purpose Milling Guide (printed p. 290 inch, p. 291 metric) incl. aluminium and 'Fiberglass, Plastics, G10' rows: "
     "plunge feed -50%. Troubleshooting (PDF p. 3): decrease ramp angle for chipping. No ramp angle or helix number. "
     "The server returns 404 to a bare 'Mozilla/5.0' agent; a full browser user-agent string gets the file."),
    ("metal_sandvik_ramping", "Sandvik Coromant", "Ramping: two axis linear and circular (knowledge page)",
     "https://www.sandvik.coromant.com/en-gb/knowledge/milling/milling-holes-cavities-pockets/ramping",
     "html/metal_sandvik_ramping.html", 200,
     "Indexable-cutter context (insert sizes 16/22). Linear ramp feed 75% of normal; max hole in one spiral = 2 x D3; the "
     "core and pip rules; pitch <= max ap. The ramp-angle-against-diameter graph is an image and is not transcribed. "
     "A bare 'Mozilla/5.0' agent gets an 8 kB shell; a full browser user-agent string gets the rendered page."),
    ("metal_kennametal_ramping_blog", "Kennametal", "Buying Guide: The Importance of Ramping in Milling (blog)",
     "https://www.kennametal.com/us/en/resources/blog/metal-cutting/importance-of-ramping-in-milling.html",
     "html/metal_kennametal_ramping_blog.html", 200,
     "Prose only. It prints no ramp angle, helix diameter or feed fraction; it says to choose the ramp angle from tool "
     "geometry, material and depth of cut, and to ask Kennametal."),
]

DEAD_ENDS = [
    ("metal_dead_kennametal_msc_interp", "Kennametal", "Milling technical information: interpolation (MSC mirror)",
     "https://www1.mscdirect.com/images/solutions/kennametal/millingTechInfoInterpolation.pdf", 403,
     "403 Forbidden (Akamai). Not retried with other agents."),
    ("metal_dead_kennametal_sem_2023", "Kennametal", "2023 Solid Carbide End Milling Master Catalog (productivity.com mirror)",
     "https://productivity.com/wp-content/uploads/2022/08/Kennametal-2023-Solid-Carbide-End-Milling-Inch-Master-Catalog-Interactive.pdf", 404,
     "404 (HTML error page)."),
    ("metal_dead_kennametal_sem_2018", "Kennametal", "2018 Vol. 2 Solid Milling Master Catalog (productivity.com mirror)",
     "https://productivity.com/wp-content/uploads/2020/07/Kennametal-2018-Vol.2-Solid-Milling-Kennametal-Master-Interactive.pdf", 404,
     "404 (HTML error page)."),
    ("metal_dead_kennametal_harvi_guide", "Kennametal (reseller page)", "Kennametal End Mill HARVI Application Guide",
     "https://www.carbidedepot.com/ken-application-em-harvi-frac.htm", 403, "403 Forbidden."),
    ("metal_dead_kennametal_product_pages", "Kennametal", "HARVI product pages p.4046274, p.5350644 and HARVI I TE / III family pages",
     "https://www.kennametal.com/us/en/products/p.4046274.html", 200,
     "200, but the 'Ramping' attribute is 'Blank'; the pages say only 'Centre cutting for plunging and ramping operations'. No number."),
    ("metal_dead_kennametal_helical_calc", "Kennametal", "Helical Milling Interpolation Calculator",
     "https://www.kennametal.com/us/en/resources/engineering-calculators/end-milling/helical-interpolation.html", 200,
     "200. A calculator: it computes the ramp angle from inputs and prints no recommended limit."),
    ("metal_dead_helical_sf_index", "Helical Solutions", "Speeds and Feeds index",
     "https://www.helicaltool.com/resources/speeds-feeds", 200,
     "200, but the PDF list is rendered by JavaScript; curl gets no PDF links. Not pursued (Harvey S&F PDFs cover the family)."),
    ("metal_dead_lakeshore_sf", "Lakeshore Carbide", "Speeds and feeds page",
     "https://www.lakeshorecarbide.com/speedsfeeds.aspx", 200,
     "200. No ramp, plunge or helix text in the page; links only to a 2013 catalogue PDF (not fetched)."),
]

S = []  # statements


def st(sid, src, page, param, fam, sub, flutes, cc, dia, mat_printed, mat_norm, vp, unit, vsi, verbatim, grade, notes,
       derived=None):
    S.append({
        "statement_id": "g10-metal-" + sid, "source_id": src, "source_page": page, "parameter": param,
        "tool_family": fam, "tool_subfamily": sub, "flutes": flutes, "centre_cutting": cc, "diameter_mm": dia,
        "material": {"printed": mat_printed, "normalised": mat_norm}, "value_printed": vp, "unit": unit,
        "value_si": vsi, "derived": derived or [], "verbatim": verbatim, "grade": grade, "notes": notes,
    })


WOOD_C = "Grade c for wood: the document names no wood."

# --- Harvey: Ramping to Success
st("harvey-rs-ramp-soft", "metal_harvey_ramping_success", "web page", "ramp_angle_recommended", "flat_end_mill",
   "end mill (general)", None, None, None, "Soft/Non-Ferrous Materials", "non_ferrous", "3° – 10°", "deg", None,
   "Suggested Starting Ramp Angles: Soft/Non-Ferrous Materials: 3° – 10°", "c",
   "A starting range, not a maximum. No split by flutes, diameter or centre cutting. " + WOOD_C,
   [{"quantity": "range_min_deg", "value": 3, "formula": "printed lower bound"},
    {"quantity": "range_max_deg", "value": 10, "formula": "printed upper bound"}])
st("harvey-rs-ramp-hard", "metal_harvey_ramping_success", "web page", "ramp_angle_recommended", "flat_end_mill",
   "end mill (general)", None, None, None, "Hard/Ferrous Materials", "ferrous", "1° – 3°", "deg", None,
   "Hard/Ferrous Materials 1° – 3°", "c", "A starting range. " + WOOD_C)
st("harvey-rs-circular", "metal_harvey_ramping_success", "web page", "entry_general", "flat_end_mill",
   "end mill (general)", None, None, None, "not printed", "any", "Circular Ramping (Helical Interpolation) ... recommended method",
   None, None, "This is the recommended method, as it ensures the longest tool life.", "c",
   "The sentence follows the Circular Ramping definition: helical entry is preferred over linear ramp. " + WOOD_C)

# --- Harvey: Most Common Methods of Tool Entry
st("harvey-te-predrill", "metal_harvey_tool_entry", "web page", "entry_general", "flat_end_mill", "end mill (general)",
   None, None, None, "not printed", "any", "5-10% larger than the end mill diameter", "fraction of D", None,
   "Pre-drilling a hole to full pocket depth (and 5-10% larger than the end mill diameter) is the safest practice", "c",
   "Pre-drill entry, not a toolpath entry. " + WOOD_C,
   [{"quantity": "predrill_diameter_over_D", "value": [1.05, 1.10], "formula": "1 + 0.05 .. 1 + 0.10"}])
st("harvey-te-helix-dia", "metal_harvey_tool_entry", "web page", "helix_diameter_min_frac", "flat_end_mill",
   "end mill (general); corner radius end mills advised", None, None, None, "ferrous materials (a vendor reply adds non-ferrous)",
   "any", "greater than 110-120% of the cutter diameter", "fraction of D", 1.1,
   "use a programmed helix diameter of greater than 110-120% of the cutter diameter", "c",
   "AMBIGUOUS: 'helix diameter' is not defined. If it is the bore (hole) diameter, the tool-centre path diameter is "
   "0.1-0.2 x D (derived). If it is the path diameter, the bore is 2.1-2.2 x D. See NOTES section 5. value_si = lower bound 1.1. " + WOOD_C,
   [{"quantity": "path_diameter_over_D_if_bore", "value": [0.1, 0.2], "formula": "helix_dia/D - 1"},
    {"quantity": "bore_over_D_if_path", "value": [2.1, 2.2], "formula": "helix_dia/D + 1"}])
st("harvey-te-helix-nonferrous", "metal_harvey_tool_entry", "web page (vendor reply in comments)", "entry_general",
   "flat_end_mill", "end mill (general)", None, None, None, "non-ferrous material", "non_ferrous",
   "Helical Interpolation is also recommended in non-ferrous material.", None, None,
   "Helical Interpolation is also recommended in non-ferrous material.", "c",
   "Harvey's own reply to a reader question. " + WOOD_C)
st("harvey-te-ramp-soft", "metal_harvey_tool_entry", "web page", "ramp_angle_recommended", "flat_end_mill",
   "end mill (general); corner radius advised", None, None, None, "Soft/Non-Ferrous Materials", "non_ferrous", "3°-10°",
   "deg", None, "Soft/Non-Ferrous Materials: 3°-10°", "c",
   "Same numbers as g10-metal-harvey-rs-ramp-soft (same publisher). The page also says 'A strong core is key'. " + WOOD_C)
st("harvey-te-plunge-cc", "metal_harvey_tool_entry", "web page", "no_plunge", "flat_end_mill", "non-centre-cutting end mill",
   None, False, None, "not printed", "any", "The tool must be center cutting", None, None,
   "The tool must be center cutting, as end milling incorporates a flat entry point making chip evacuation extremely difficult.",
   "c", "Straight plunge: a non-centre-cutting end mill must not plunge. The page calls straight plunge 'often problematic' "
   "and points to drills for plunging. " + WOOD_C)
st("harvey-te-straight-entry", "metal_harvey_tool_entry", "web page", "entry_general", "flat_end_mill", "end mill (general)",
   None, None, None, "not printed", "any", "reduced by at least 50%", "fraction of feed", 0.5,
   "Until the cutter is fully engaged, the feed rate upon entry is recommended to be reduced by at least 50% during this operation.",
   "c", "Straight (side) entry, not plunge. value_si = the feed multiplier at most 0.5. " + WOOD_C,
   [{"quantity": "entry_feed_multiplier_max", "value": 0.5, "formula": "1 - 0.50"}])
st("harvey-te-rollin", "metal_harvey_tool_entry", "web page", "entry_general", "flat_end_mill", "end mill (general)",
   None, None, None, "not printed", "any", "reduced by 50%", "fraction of feed", 0.5,
   "The feed rate in this scenario should be reduced by 50%.", "c",
   "Roll-in (arc) entry into the cut. " + WOOD_C,
   [{"quantity": "entry_feed_multiplier", "value": 0.5, "formula": "1 - 0.50"}])

# --- Helical Machining Guidebook 2016
st("helical-gb-ramp-soft", "metal_helical_guidebook_2016", "p. 4 (visible layer)", "ramp_angle_recommended",
   "flat_end_mill", "end mill (general)", None, None, None, "Soft/Non-Ferrous Materials", "non_ferrous", "3º – 10º", "deg",
   None, "Soft/Non-Ferrous Materials: 3º – 10º", "c",
   "The PDF prints the degree as U+00BA. Hidden layers print 'Non-Ferrous Materials: 3º – 10º'. " + WOOD_C)
st("helical-gb-ramp-hard", "metal_helical_guidebook_2016", "p. 4 (visible layer)", "ramp_angle_recommended",
   "flat_end_mill", "end mill (general)", None, None, None, "Hard/Ferrous Materials", "ferrous", "1º – 3º", "deg", None,
   "Hard/Ferrous Materials: 1º – 3º", "c", WOOD_C)
st("helical-gb-helix-dia", "metal_helical_guidebook_2016", "p. 4", "helix_diameter_min_frac", "flat_end_mill",
   "end mill (general); corner radius advised", None, None, None, "ferrous materials", "ferrous", ">110-120% of tool diameter",
   "fraction of D", 1.1, "We recommend a programmed helix diameter >110-120% of tool diameter.", "c",
   "Same rule and same ambiguity as g10-metal-harvey-te-helix-dia. " + WOOD_C,
   [{"quantity": "path_diameter_over_D_if_bore", "value": [0.1, 0.2], "formula": "helix_dia/D - 1"}])
st("helical-gb-cc-descent", "metal_helical_guidebook_2016", "p. 6", "entry_general", "flat_end_mill",
   "centre-cutting and non-centre-cutting end mills", None, None, None, "not printed", "any",
   "the angle of descent will vary depending on end style", None, None,
   "End mills with either center cutting or non-center cutting geometry can be used for both forms of ramping, however the angle of descent will vary depending on end style.",
   "c", "Shape of the rule: the max ramp angle depends on the end style (centre cutting or not). No number. " + WOOD_C)
st("helical-gb-plunge-cc", "metal_helical_guidebook_2016", "p. 4-5", "no_plunge", "flat_end_mill",
   "non-centre-cutting end mill", None, False, None, "not printed", "any", "The tool must be center cutting.", None, None,
   "The least preferred method and one that can easily break a tool. The tool must be center cutting.", "c",
   "Straight plunge. The page adds 'Drill bits are intended for straight plunging'. " + WOOD_C)

# --- Harvey S&F charts
st("harvey-sf18700-plunge", "metal_harvey_sf_18700", "p. 1 (notes)", "plunge_feed_fraction", "chamfer_v",
   "Chamfer Cutters - Pointed & Flat End, 2 Flutes, Type I", 2, None, [0.381, 25.4],
   "chart materials: aluminium alloys, magnesium, zinc, copper alloys, steels to 45 Rc and more", "any_metal",
   "reduce Chip Loads to 40%-50% depending on finish", "fraction of chip load", 0.4,
   "For vertical plunging, reduce Chip Loads to 40%-50% depending on finish", "c",
   "'reduce ... to 40%-50%' reads as: the plunge chip load = 0.40-0.50 x the posted side-milling chip load. value_si = the "
   "lower bound. diameter_mm = the chart's effective-diameter span 0.015-1.000 in (derived). Grade a for the printed "
   "metals; grade c for wood. " + WOOD_C,
   [{"quantity": "plunge_chipload_multiplier", "value": [0.4, 0.5], "formula": "printed 40%-50% / 100"},
    {"quantity": "diameter_mm", "value": [0.381, 25.4], "formula": "0.015 in x 25.4 .. 1.000 in x 25.4"}])
st("harvey-sf25000-plunge", "metal_harvey_sf_25000", "p. 1 (notes)", "plunge_feed_fraction", "chamfer_v",
   "Engraving Cutters - Pointed, 1 Flute, Pointed Tip", 1, None, None, "chart materials (metals)", "any_metal",
   "reduce posted chip loads by 5 0%", "fraction of chip load", 0.5,
   "For VERTICAL plunge milling to depth, reduce posted chip loads by 5 0%", "c",
   "The PDF text prints '5 0%' (a kerning gap); the value is 50%. The plunge chip load = 0.5 x the posted horizontal chip "
   "load. Grade a for the printed metals; grade c for wood. " + WOOD_C,
   [{"quantity": "plunge_chipload_multiplier", "value": 0.5, "formula": "1 - 0.50"}])
st("harvey-sf25000-ramp-pref", "metal_harvey_sf_25000", "p. 1 (notes)", "entry_general", "chamfer_v",
   "Engraving Cutters - Pointed, 1 Flute, Pointed Tip", 1, None, None, "chart materials (metals)", "any_metal",
   "ramping is preferred to maintain tip integrity", None, None, "(ramping is preferred to maintain tip integrity).", "c",
   "A V / pointed tool: ramp, do not plunge where possible. No angle printed. " + WOOD_C)

# --- SGS / Kyocera
st("sgs-ramp-feed-1-2deg", "metal_sgs_catalog_2021", "catalogue p. 24 (PDF p. 9), Entry Methods", "ramp_feed_fraction",
   "flat_end_mill", "SGS solid carbide end mills (general)", None, None, None, "not printed (all materials)", "any",
   "Use slotting speeds and feeds for ramp angles of 1° to 2°.", "fraction of slotting feed", 1.0,
   "Use slotting speeds and feeds for ramp angles of 1° to 2°.", "c",
   "Ramp feed = 1.0 x slotting feed for 1-2 deg. Read from the -raw section. " + WOOD_C,
   [{"quantity": "ramp_feed_over_slot_feed", "value": 1.0, "formula": "printed 'slotting ... feeds' = 1.0 x slot feed"}])
st("sgs-ramp-feed-6deg", "metal_sgs_catalog_2021", "catalogue p. 24 (PDF p. 9), Entry Methods", "ramp_feed_fraction",
   "flat_end_mill", "SGS solid carbide end mills (general)", None, None, None, "not printed (all materials)", "any",
   "Reduce feed to 25% when ramp angles approach 6°.", "fraction of slotting feed", 0.25,
   "Reduce feed to 25% when ramp angles approach 6°.", "c",
   "With the 1-2 deg line this gives two points of ramp feed against angle: 1.0 at 1-2 deg, 0.25 at about 6 deg. "
   "The document prints no curve between them. The base of '25%' is read as the slotting feed of the line before. " + WOOD_C)
st("sgs-ramp-general", "metal_sgs_catalog_2021", "catalogue p. 24 (PDF p. 9), Entry Methods", "entry_general",
   "flat_end_mill", "SGS solid carbide end mills (general)", None, None, None, "difficult to machine materials", "any",
   "General purpose tools ... will require lower ramp angles and reduced feed.", None, None,
   "General purpose tools and/or difficult to machine materials will require lower ramp angles and reduced feed.", "c",
   "Also: 'High ramp angles require reduced feed. Lower ramp angles will allow higher feed rates and extend tool life.' " + WOOD_C)
st("sgs-plunge-feed", "metal_sgs_catalog_2021", "catalogue p. 24 (PDF p. 9), Entry Methods", "plunge_feed_fraction",
   "flat_end_mill", "SGS solid carbide end mills (general)", None, True, None, "non-ferrous and short-chipping materials",
   "non_ferrous", "25% slotting feeds", "fraction of slotting feed", 0.25,
   "Plunge only in non-ferrous and short-chipping materials using slotting speeds and 25% slotting feeds.", "c",
   "Plunge feed = 0.25 x slotting feed, at slotting speed. Plunge is limited to non-ferrous / short-chipping materials. " + WOOD_C)
st("sgs-matrix-footnote", "metal_sgs_catalog_2021", "catalogue p. 23 (PDF p. 8), End Mill Matrix footnote ***",
   "entry_general", "flat_end_mill", "SGS series", None, None, None, "most materials", "any",
   "shown is general recommendation for most materials, lower ramp angles are required for materials with lower machinability",
   None, None, "shown is general recommendation for most materials, lower", "c",
   "Qualifies every 'Maximum Recommended Ramp Angle' in the matrix. " + WOOD_C)

MATRIX = "catalogue pp. 22-23 (PDF pp. 7-8), End Mill Matrix, column 'Maximum Recommended Ramp Angle ***'"
MX_NOTE = ("Row read across two pages: the series name is on p. 22, the values on p. 23, by row order (see NOTES "
           "section 5). The material fit is colour-coded and is not in the text. ")
st("sgs-mx-z5", "metal_sgs_catalog_2021", MATRIX, "ramp_angle_max", "flat_end_mill",
   "Z-Carb-HPR Z5 (5 flute rougher; S, R end styles)", 5, False, [6, 25], "not printed per row (colour-coded)", "any_metal",
   "7", "deg", 7, "1 to 3 – S, R By Request SR, WF, CH No 7 37 Unequal", "c",
   MX_NOTE + "Non-centre-cutting: 7 deg. CONFLICT: the Z5 chart (p. 34) says 'ramp up to 5 degrees'. " + WOOD_C)
st("sgs-mx-77", "metal_sgs_catalog_2021", MATRIX, "ramp_angle_max", "flat_end_mill",
   "H-Carb 77 (7 flute high efficiency)", 7, False, [6, 25], "not printed per row (colour-coded)", "any_metal", "1", "deg",
   1, "2.5 to 4 – S, R SR No 1 37 Unequal", "c", MX_NOTE + "Non-centre-cutting, 7 flutes: 1 deg. " + WOOD_C)
st("sgs-mx-66", "metal_sgs_catalog_2021", MATRIX, "ramp_angle_max", "flat_end_mill",
   "Multi-Carb 66 (multi-flute finisher)", None, False, [6, 25], "not printed per row (colour-coded)", "any_metal", "1",
   "deg", 1, "1.5 to 3.25 – S, R By Request SR No 1 35 Equal Ti-Namite-A", "c",
   MX_NOTE + "Non-centre-cutting multi-flute finisher: 1 deg. " + WOOD_C)
st("sgs-mx-56b", "metal_sgs_catalog_2021", MATRIX, "ramp_angle_max", "ball_nose",
   "Turbo-Carb 56B (2 flute contouring long reach ball end)", 2, True, [1, 20], "not printed per row (colour-coded)",
   "any_metal", "25", "deg", 25, "1 2 to 2.25 B By Request SR Yes 25 30 Equal Ti-Namite-A", "c",
   MX_NOTE + "The only ball-only row: 25 deg, centre cutting. " + WOOD_C)
st("sgs-mx-47", "metal_sgs_catalog_2021", MATRIX, "ramp_angle_max", "flat_end_mill",
   "S-Carb 2 Flute, series 47 (S, B end styles)", 2, True, [3, 25], "not printed per row (colour-coded)", "any_metal", "90",
   "deg", 90, "1 to 3 3 to 9 S, B By Request SR Yes 90 35 Equal Ti-Namite-B", "c",
   MX_NOTE + "S-Carb is the SGS non-ferrous line (the name, not the matrix, says so). " + WOOD_C)
st("sgs-mx-43", "metal_sgs_catalog_2021", MATRIX, "ramp_angle_max", "flat_end_mill",
   "S-Carb 3 Flute, series 43 (S, R, B end styles)", 3, True, [3, 25], "not printed per row (colour-coded)", "any_metal",
   "90", "deg", 90, "1 to 7 2.25 to 8.5 S, R, B By Request SR Yes 90 38 Equal Ti-Namite-B", "c", MX_NOTE + WOOD_C)
st("sgs-mx-series7", "metal_sgs_catalog_2021", MATRIX, "ramp_angle_max", "flat_end_mill",
   "Series 7 (4 flute variable geometry long length; S, B)", 4, True, [3, 25], "not printed per row (colour-coded)",
   "any_metal", "1", "deg", 1, "2.25 to 8.25 – S, B By Request SR Yes 1 38 Unequal Ti-Namite-A", "c",
   MX_NOTE + "Centre cutting but long length: 1 deg. So centre cutting alone does not set the limit; length does too. " + WOOD_C)
st("sgs-mx-ccr", "metal_sgs_catalog_2021", MATRIX, "ramp_angle_max", "flat_end_mill",
   "CCR series 20-CCR (composite cutting router; end style varies)", None, None, [2, 12],
   "not printed per row (colour-coded)", "composite", "Based upon end style / 5 (for end cut styles)", "deg", 5,
   "Based upon 5 (for end cut Di-Namite 2.75 to 4 – S Standard SR 15 Equal 2 to 12 end style styles)", "c",
   MX_NOTE + "Centre cutting 'Based upon end style'; max ramp 5 deg for end-cut styles. 31-CCR (6-12 mm) prints the same. "
   "Router for composites (CCR). " + WOOD_C)
st("sgs-mx-compression", "metal_sgs_catalog_2021", MATRIX, "ramp_angle_max", "flat_end_mill",
   "Compression Router, series 25", None, True, [6, 12], "not printed per row (colour-coded)", "composite",
   "5", "deg", 5, "2.75 to 4 – S By Request SR Yes 5 30 Equal", "c",
   MX_NOTE + "Compression router: 5 deg, centre cutting. Nearest tool to a wood compression bit in this set. " + WOOD_C)
st("sgs-mx-upcut", "metal_sgs_catalog_2021", MATRIX, "ramp_angle_max", "flat_end_mill", "Up Cut Router, series 21", None,
   True, [3, 12], "not printed per row (colour-coded)", "composite", "90", "deg", 90,
   "2.5 to 4.25 – S By Request SR Yes 90 35 Equal various optional", "c",
   MX_NOTE + "Up-cut router: 90 deg (plunge allowed). " + WOOD_C)
st("sgs-mx-downcut", "metal_sgs_catalog_2021", MATRIX, "no_ramp", "flat_end_mill", "Down Cut Router, series 22", None,
   True, [3, 12], "not printed per row (colour-coded)", "composite", "–", "deg", None,
   "2.5 to 4.25 – S By Request SR Yes – 35 Equal various optional", "c",
   MX_NOTE + "Down-cut router: the ramp-angle cell is a dash. The document does not say if the dash means 'do not ramp' or "
   "'no data'. Parameter no_ramp is a tentative reading; a verifier must decide. " + WOOD_C)
st("sgs-z5-ramp5", "metal_sgs_catalog_2021", "catalogue p. 34 (PDF p. 19), Z-Carb-HPR Z5 chart notes", "ramp_angle_max",
   "flat_end_mill", "Z-Carb-HPR Z5 (5 flute rougher square end)", 5, False, None, "chart materials (steels ... hardened steels)",
   "steel", "ramp up to 5 degrees", "deg", 5, "ramp up to 5 degrees using slotting speed and feed rates. Do not plunge.", "c",
   "Ramp feed at up to 5 deg = slotting feed (1.0 x). Conflicts with the matrix value 7 for Z5. flutes = 5 from 'ipm = Fz x 5 x rpm' "
   "and the index line '5 Flute Rougher Square End'. " + WOOD_C,
   [{"quantity": "ramp_feed_over_slot_feed", "value": 1.0, "formula": "printed 'using slotting speed and feed rates'"}])
st("sgs-z5-noplunge", "metal_sgs_catalog_2021", "catalogue p. 34 (PDF p. 19), Z-Carb-HPR Z5 chart notes", "no_plunge",
   "flat_end_mill", "Z-Carb-HPR Z5 (5 flute rougher square end)", 5, False, None, "chart materials (steels ... hardened steels)",
   "steel", "Do not plunge.", None, None, "Do not plunge.", "c",
   "A non-centre-cutting rougher (matrix: Center Cutting 'No'). " + WOOD_C)

# --- IMCO
st("imco-bore-2d-2r", "metal_imco_helical_ramp", "p. 130, Step 1", "helix_diameter_max_frac", "flat_end_mill",
   "IMCO end mills (square and corner radius)", None, True, None, "not printed", "any",
   "(tool diameter x 2) - (corner radius x 2)", "bore diameter", 2.0,
   "The diameter of the starting hole will be: (tool diameter x 2) - (corner radius x 2)", "c",
   "The bore of the helical entry hole = 2D - 2r. For r = 0 the bore is 2.0 x D and the tool-centre path diameter is 1.0 x D, "
   "so the flat end just passes the hole centre: the largest bore that leaves no core. value_si = 2.0 (bore/D at r = 0). " + WOOD_C,
   [{"quantity": "bore_over_D", "value": "2 - 2r/D", "formula": "(2D - 2r)/D"},
    {"quantity": "path_diameter_over_D", "value": "1 - 2r/D", "formula": "bore - D, divided by D"},
    {"quantity": "path_diameter_over_D_at_r0", "value": 1.0, "formula": "1 - 0"}])
st("imco-flute-steps", "metal_imco_helical_ramp", "p. 130", "entry_general", "flat_end_mill", "IMCO end mills", None, None,
   None, "not printed", "any", "seven or fewer flutes only require one step; ... more than seven flutes require two steps",
   None, None, "Tools with seven or fewer flutes only require one step; tools with more than seven flutes require two steps.",
   "c", "Flute count sets the entry procedure: > 7 flutes must open the hole in a second step. " + WOOD_C)
st("imco-iptc-angle", "metal_imco_helical_ramp", "p. 130, Step 1 table", "helix_ramp_angle", "flat_end_mill",
   "IPT/C 7, 9, 11, 13 (7-13 flute)", [7, 13], True, None, "not printed", "any", "0.5°", "deg", 0.5,
   "IPT/C 13 Same as chart IPT or MMPT x 1.6 IPT or MMPT x 1.25 0.5°", "c",
   "Same 0.5 deg for IPT/C 7, 9, 11 and 13. Flute counts from p. 131 '7-, 9-, 11- and 13-flute tools'. " + WOOD_C)
st("imco-iptc-feed", "metal_imco_helical_ramp", "p. 130, Step 1 table", "ramp_feed_fraction", "flat_end_mill",
   "IPT/C 7, 9, 11, 13 (7-13 flute)", [7, 13], True, None, "not printed", "any",
   "IPT or MMPT x 1.6 (high-pressure coolant); IPT or MMPT x 1.25 (standard flood coolant)", "fraction of chart IPT", 1.25,
   "IPT/C 7 Same as chart IPT or MMPT x 1.6 IPT or MMPT x 1.25 0.5°", "c",
   "Helical ramp feed per tooth = 1.6 x (HP coolant) or 1.25 x (flood) the chart IPT, at 0.5 deg. The chart IPT is the "
   "series chart value (the page does not say which column; Step 2 cites the Peripheral-HEM values). A shallow ramp "
   "with a feed ABOVE 1.0 x. value_si = the flood value. " + WOOD_C)
st("imco-aptc-angle", "metal_imco_helical_ramp", "p. 130, Step 1 table", "helix_ramp_angle", "flat_end_mill",
   "APT/C 5", None, True, None, "not printed", "any", "3°", "deg", 3.0,
   "APT/C 5 Same as chart IPT or MMPT x 1.6 IPT or MMPT x 1.25 3°", "c", "Feed as the IPT/C rows. " + WOOD_C)
st("imco-m5-angle", "metal_imco_helical_ramp", "p. 130, Step 1 table", "helix_ramp_angle", "flat_end_mill",
   "M525, M527, M503, M726, M706, M806, M924, M904, M905, E14, E13, E12, M104", None, True, None, "not printed", "any",
   "1° - 2.5°", "deg", None, "M525 Slotting speed in chart Slotting feed in chart Slotting feed in chart 1° - 2.5°", "c",
   "Speed and feed = the slotting values of the series chart (ramp feed fraction 1.0 of slot feed). " + WOOD_C,
   [{"quantity": "ramp_feed_over_slot_feed", "value": 1.0, "formula": "printed 'Slotting feed in chart'"}])
st("imco-m2-angle", "metal_imco_helical_ramp", "p. 130, Step 1 table", "helix_ramp_angle", "flat_end_mill",
   "M223, M233, M203, M202", None, True, None, "not printed in this document", "aluminium",
   "3° - 5°", "deg", None, "M223 Slotting speed in chart Slotting feed in chart Slotting feed in chart 3° - 5°", "c",
   "The steepest row. The document does not name the material. A reseller page (cuttingtooldepot.com, not stored) calls "
   "M203/M223/M233 aluminium end mills; treat the aluminium label as unverified. Feed = slotting feed (1.0 x). " + WOOD_C,
   [{"quantity": "ramp_feed_over_slot_feed", "value": 1.0, "formula": "printed 'Slotting feed in chart'"}])
st("imco-expand", "metal_imco_helical_ramp", "p. 131, Step 2", "entry_general", "flat_end_mill",
   "IPT/C 7, 9 (3xD); IPT/C 11, 13 (3.75xD)", [7, 13], True, None, "not printed", "any", "3xD; 3.75 x D", "bore diameter",
   None, "IPT/C 11 3.75 x D IPT or MMPT x .75 RDOC x .5", "c",
   "For 7-13 flute tools the entry hole is opened to 3xD or 3.75xD before full HEM cutting: feed x .75, step-over RDOC x .5 "
   "(method A), or a second helix at x 1.6 feed and 0.5 deg (method B). " + WOOD_C)

# --- Garr
st("garr-plunge-50", "metal_garr_technical", "PDF p. 8 (printed p. 290), General Purpose Milling Guide", "plunge_feed_fraction",
   "flat_end_mill", "GARR general purpose solid carbide end mills", None, None, [1.5875, 25.4],
   "chart rows incl. Aluminum, Magnesium, Copper, Brass, 'Fiberglass, Plastics, G10'", "any",
   "drop feed by approximately 50%", "fraction of feed", 0.5, "When plunging into a solid, drop feed by approximately 50%.",
   "c", "Plunge chip load = about 0.5 x the chart chip load per tooth. The chart has a plastics row (Fiberglass, Plastics, "
   "G10), so the rule covers plastics; it names no wood. The metric page (PDF p. 9) prints the same line. diameter_mm = the "
   "chart span 1/16 in to 1 in (derived). " + WOOD_C,
   [{"quantity": "plunge_feed_multiplier", "value": 0.5, "formula": "1 - 0.50"},
    {"quantity": "diameter_mm", "value": [1.5875, 25.4], "formula": "1/16 in x 25.4 .. 1 in x 25.4"}])
st("garr-chipping-ramp", "metal_garr_technical", "PDF p. 3, Troubleshooting: End Mills, Chipping", "entry_general",
   "flat_end_mill", "end mill (general)", None, None, None, "not printed", "any",
   "Decrease ramp angle or slow down approach", None, None, "Decrease ramp angle or slow down approach", "c",
   "A remedy for edge chipping. No number. " + WOOD_C)

# --- Sandvik
st("sandvik-linear-75", "metal_sandvik_ramping", "web page, Two axes ramping - linear", "ramp_feed_fraction",
   "indexable", "indexable milling cutters (insert context)", None, False, None, "not printed", "any_metal",
   "Reduce feed to 75% of normal", "fraction of feed", 0.75, "Reduce feed to 75% of normal", "c",
   "Linear ramping (full slot, ae = Dc). Keep the lower feed for one cutter diameter after the ramp. Indexable context. " + WOOD_C)
st("sandvik-max-hole", "metal_sandvik_ramping", "web page, Two axes ramping - circular", "helix_diameter_max_frac",
   "indexable", "cutters that are not centre cutters", None, False, None, "not printed", "any_metal", "2 x D3",
   "bore diameter", 2.0, "The maximum hole diameter, Dm, which can be produced in one continuous spiral, is 2 x D3", "c",
   "Above 2 x D3 a core stays ('Cutter diameter is too small and will leave a core in the middle'). At 2 x D3 a pip stays "
   "in a blind hole; feed to centre to remove it. D3 is the cutter diameter symbol in the figure (not defined in the text). "
   "value_si = 2.0 (Dm/D3). " + WOOD_C,
   [{"quantity": "path_diameter_over_D3", "value": 1.0, "formula": "(Dm - D3)/D3 at Dm = 2 x D3"}])
st("sandvik-core", "metal_sandvik_ramping", "web page, Two axes ramping - circular", "helix_diameter_max_frac",
   "indexable", "cutters that are not centre cutters", None, False, None, "not printed", "any_metal",
   "will leave a core in the middle", None, None,
   "Cutter diameter is too small and will leave a core in the middle – like trepanning.", "c",
   "The 'no core left' rule in words. " + WOOD_C)
st("sandvik-pitch", "metal_sandvik_ramping", "web page, 2. Pitch (P)", "helix_pitch", "indexable",
   "indexable milling cutters", None, None, None, "not printed", "any_metal", "never larger then the maximum ap", None, None,
   "The pitch can never be larger then the maximum ap for the cutter concept, and depends on the hole diameter, the cutter diameter and the ramp angle.",
   "c", "Helix pitch cap = the tool's maximum axial depth of cut. ('then' is the page's spelling.) " + WOOD_C)
st("sandvik-prefer-circular", "metal_sandvik_ramping", "web page", "entry_general", "indexable", "milling cutters", None,
   None, None, "not printed", "any_metal", "Circular ramping is always preferred to linear ramping", None, None,
   "Circular ramping is always preferred to linear ramping (full slotting), because helical interpolation is a much smoother process as the radial cut is reduced.",
   "c", WOOD_C)
st("sandvik-slot-30", "metal_sandvik_ramping", "web page, Two axes ramping - linear", "entry_general", "indexable",
   "milling cutters", None, None, None, "not printed", "any_metal", "narrow slots less than 30 mm wide", "mm", 30,
   "Linear ramping should be limited to narrow slots less than 30 mm wide, if access for circular ramping is limited", "c",
   WOOD_C)

# --- Kennametal
st("kennametal-angle-basis", "metal_kennametal_ramping_blog", "web page", "entry_general", "flat_end_mill",
   "milling cutters (general)", None, None, None, "not printed", "any_metal",
   "chosen based on the geometry of the tool, the material being machined, and the depth of cut", None, None,
   "The ramp angle should be chosen based on the geometry of the tool, the material being machined, and the depth of cut.",
   "c", "No number printed. The page recommends to contact a Kennametal expert. " + WOOD_C)


def main():
    srcs = []
    texts = {}
    for sid, vendor, title, url, raw, status, cov in SOURCES:
        tpath = os.path.join(G10, "sources", sid + ".txt")
        srcs.append({
            "source_id": sid, "source_vendor": vendor, "source_title": title, "source_url": url, "accessed_on": ACC,
            "stored_text": "sources/%s.txt" % sid, "text_sha256": sha(tpath), "raw_sha256": sha(os.path.join(G10, raw)),
            "http_status": status, "coverage_notes": cov,
        })
        texts[sid] = norm(open(tpath, encoding="utf-8", errors="replace").read())
    for sid, vendor, title, url, status, cov in DEAD_ENDS:
        srcs.append({
            "source_id": sid, "source_vendor": vendor, "source_title": title, "source_url": url, "accessed_on": ACC,
            "stored_text": None, "text_sha256": None, "raw_sha256": None, "http_status": status, "coverage_notes": cov,
        })
    bad = 0
    ids = set()
    for s in S:
        assert s["statement_id"] not in ids, s["statement_id"]
        ids.add(s["statement_id"])
        if norm(s["verbatim"]) not in texts[s["source_id"]]:
            print("verbatim missing:", s["statement_id"], repr(s["verbatim"][:70]))
            bad += 1
    json.dump(srcs, open(os.path.join(G10, "parts", "metal_sources.json"), "w"), indent=1, ensure_ascii=False)
    json.dump(S, open(os.path.join(G10, "parts", "metal_statements.json"), "w"), indent=1, ensure_ascii=False)
    print(f"{len(srcs)} sources, {len(S)} statements, {bad} verbatim failures")


if __name__ == "__main__":
    main()
