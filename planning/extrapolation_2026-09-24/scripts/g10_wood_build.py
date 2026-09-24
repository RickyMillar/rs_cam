#!/usr/bin/env python3
"""G10 fetch, part `wood`: write parts/wood_sources.json and parts/wood_statements.json.

The statements are transcribed by hand below. This script only adds the
hashes, converts the printed feeds and computes the derived fractions, so a
reader can check every derived number against its formula.

value_si convention for this part: a feed is in mm/min (IPM x 25.4); a
fraction is dimensionless; an angle is in degrees.

Usage: python3 g10_wood_build.py
"""
import hashlib
import json
import os

HERE = os.path.dirname(os.path.abspath(__file__))
G10 = os.path.normpath(os.path.join(HERE, "..", "fetch", "G10"))
ACC = "2026-09-25"


def sha(path):
    with open(path, "rb") as f:
        return hashlib.sha256(f.read()).hexdigest()


def src(sid, vendor, title, url, raw_rel, notes, text=True, status=200):
    raw = os.path.join(G10, raw_rel) if raw_rel else None
    txt = f"sources/{sid}.txt" if text else None
    return {
        "source_id": sid,
        "source_vendor": vendor,
        "source_title": title,
        "source_url": url,
        "accessed_on": ACC,
        "stored_text": txt,
        "text_sha256": sha(os.path.join(G10, txt)) if txt else None,
        "raw_sha256": sha(raw) if raw and os.path.exists(raw) else None,
        "http_status": status,
        "coverage_notes": notes,
    }


PRIOR = "Same bytes as the stored copy at {p} (raw sha256 equal); re-stored for G10 because it carries entry guidance."

SOURCES = [
    src("wood_amana_compression_v8", "Amana Tool", "Solid Carbide Compression Spiral Router Bits (speed chart v8)",
        "https://www.amanatool.com/pub/media/productattachments/Solid-Carbide-Compression-Spirals-v8.pdf",
        "pdf/wood_amana_compression_v8.pdf",
        "Prints a 'Ramp Down' IPM column per diameter, flute count and material (Wood, MDF/Laminate, Plywood, Plastic) and the rule 'To find Ramp Down: Feed Rate IPM / # of flutes'. "
        + PRIOR.format(p="fetch/G4/sources/amana_compression_spirals_v8.txt")),
    src("wood_amana_spektra_plunge_v24", "Amana Tool", "Solid Carbide Spektra Spiral Plunge Router Bits, 2 and 3 flute (speed chart v24)",
        "https://www.amanatool.com/pub/media/productattachments/Solid-Carbide-Spektra-Spiral-Plunge-2-3-Flute-v24.pdf",
        "pdf/wood_amana_spektra_plunge_v24.pdf",
        "Up-cut and down-cut flat spirals. 'Ramp Down' column for Wood/Plywood and MDF/Laminate; the same rule. "
        + PRIOR.format(p="fetch/G6/sources/amana_spektra_spiral_plunge_v24.txt (and crates/rs_cam_core/data/vendor_lut/sources/amana_spektra_spiral_plunge_v24.txt)")),
    src("wood_amana_ball_nose_v7", "Amana Tool", "2 Flute Solid Carbide CNC Spiral Ball Nose Router Bits (speed chart v7)",
        "https://www.amanatool.com/pub/media/productattachments/Spiral-Ball-Nose-Speed-Chart-v7.pdf",
        "pdf/wood_amana_ball_nose_v7.pdf",
        "Ball nose. No ramp column; prints only the rule 'To find Ramp Down: Feed Rate IPM / # of flutes'. "
        + PRIOR.format(p="fetch/G6/sources/amana_ball_nose_v7.txt")),
    src("wood_amana_corner_radius_plunge", "Amana Tool (hosted by ToolsToday)", "2 Flute Solid Carbide Spiral Plunge with Corner Radius Router Bit (speed chart)",
        "https://toolstoday.com/content/ProductFile/Attachments/Solid-Carbide-Spiral-Plunge-w-Corner-Radius.pdf",
        "pdf/wood_amana_corner_radius_plunge.pdf",
        "Bull nose (corner radius). No ramp column; prints only the ramp-down rule. Earlier copy: fetch/G3/sources/amana_corner_radius_spiral_plunge.txt."),
    src("wood_amana_insert_vgroove_v16", "Amana Tool", "Insert Carbide V-Groove Router Bits (speed chart v16)",
        "https://www.amanatool.com/pub/media/productattachments/Insert-V-Groove-Speed-Chart-v16.pdf",
        "pdf/wood_amana_insert_vgroove_v16.pdf",
        "V-bits 40 to 160 degree, 1 and 2 flute. 'Ramp Down' column per material (Hardwood, Softwood, Plywood/Chipboard, MDF, Plastic, Foam). The printed 1-flute ramp values are Feed/2, not the printed rule Feed/# of Flutes (see NOTES section 5). "
        + PRIOR.format(p="fetch/G5/sources/amana_insert_v_groove_v16.txt")),
    src("wood_amana_spektra_engraving_v4", "Amana Tool", "Spektra 15/30/45/120 Degree Single Flute Engraving Router Bits (speed chart v4)",
        "https://www.amanatool.com/pub/media/productattachments/Spektra-15-30-45-120-Degree-Engraving-Speed-Chart-v4.pdf",
        "pdf/wood_amana_spektra_engraving_v4.pdf",
        "Engraving V-bits, 1 flute. No ramp column; prints only the ramp-down rule. "
        + PRIOR.format(p="fetch/G5/sources/amana_spektra_engraving_v4.txt")),
    src("wood_vortex_catalog", "Vortex Tool", "Vortex Tool Company catalog (wood, plastic, composite tooling)",
        "https://www.vortextool.com/media/assets/Vortex_Catalog.pdf", "pdf/wood_vortex_catalog.pdf",
        "Prose: down-cut tools must not plunge straight into wood and must ramp; up-cut spirals plunge; veining bits are end cutting and plunge. No ramp angle, no plunge fraction."),
    src("wood_onsrud_routing_guide", "LMT Onsrud (Leitz Metalworking Technology Group)", "CNC Production Routing Guide (05)",
        "https://precisionboard.com/wp-content/uploads/2017/08/CNC-Prod-Routing-Guide-05.pdf", "pdf/wood_onsrud_routing_guide.pdf",
        "Prose only: 'Ramped Plunging' as a heat-reduction method; ramp up and down (oscillation) in plywood, laminated MDF and particleboard; PCD tools 'TIPICALLY CANNOT PLUNGE'. No numbers for entry. "
        + PRIOR.format(p="fetch/G8/sources/onsrud_cnc_production_routing_guide.txt")),
    src("wood_onsrud_pct19", "LMT Onsrud", "Production Cutting Tools catalog PCT-19 (wood, plastic, composite)",
        "https://onsrud.com/images/LMT%20Onsrud%20Product%20Cutting%20Tools%20Catalog%20PCT-19.pdf", "pdf/wood_onsrud_pct19.pdf",
        "Full text of 132 pages. Entry content: the 34-100 potted-fastener tool (composite/aluminium panels) prints Plunge Feed Rate 40 IPM against Feed Rate 80 IPM; the 66-500 composite series prints which point style ramps, plunges or drills. No wood entry data. Earlier excerpt: crates/rs_cam_core/data/vendor_lut/sources/onsrud_pct19_catalog.txt."),
    src("wood_onsrud_plastics_faq2", "LMT Onsrud", "Frequently Asked Questions in the Routing of Plastics, part 2 (article)",
        "https://onsrud.com/articles/Frequently-Asked-Questions-in-the-Routing-of-Plastics-2.asp", "html/wood_onsrud_plastics_faq2.html",
        "Plastics, prose: ramp into cuts, helical ramp with interpolation for holes, reduce plunge feed. No numbers."),
    src("wood_idc_feeds_speeds", "IDC Woodcraft", "CNC Router Bit Feeds & Speeds, Imperial & Metric (PDF, hosted on the Carbide 3D forum)",
        "https://community.carbide3d.com/uploads/short-url/fwPIYiWQNjUx8eEwsA7qmYiLMxv.pdf", "pdf/wood_idc_feeds_speeds.pdf",
        "Benchtop wood chart with a 'Plunge (in/min)' column next to 'Feed (in/min)' for down cut, up cut, ball nose, V-bit, taper ball nose, surfacing, bowl and drilling bits. Earlier copy: fetch/G3/sources/idcwoodcraft_feeds_speeds_pdf.txt."),
    src("wood_whiteside_cnc_brochure", "Whiteside Machine Company", "CNC Brochure 1-14-19",
        "https://cdn.shopify.com/s/files/1/1698/7023/files/CNC_Brochure_1-14-19.pdf?20", "pdf/wood_whiteside_cnc_brochure.pdf",
        "Only entry content: up cut benefit 'Best At Plunge Cuts'. No numbers. Earlier copy of the PDF: fetch/G1/pdf/whiteside_cnc_brochure_1-14-19.pdf."),
    src("wood_toolstoday_calc_video", "ToolsToday (Amana Tool distributor)", "How To Calculate CNC Feeds and Speeds (video page and transcript)",
        "https://toolstoday.com/t-video-how-to-calculate-feeds-and-speeds", "html/wood_toolstoday_calc_video.html",
        "Reads the Amana 'Ramp Down' value as 'ramp-down or plunge rate'. Prints Feed and Ramp Down for five Amana bits (compression, up cut, down cut, 60 degree insert V-groove). Earlier copy: fetch/G4/sources/toolstoday_how_to_calculate_feeds_and_speeds.txt."),
    src("wood_toolstoday_understanding_feeds_speeds", "ToolsToday", "Understanding CNC Feeds and Speeds (learn article)",
        "https://toolstoday.com/learn/understanding-cnc-feeds-and-speeds", "html/wood_toolstoday_understanding_feeds_speeds.html",
        "Prose: ramp or plunge feed about half the main feed rate, as a general starting point."),
    src("wood_axyz_compression_bit_tip", "AXYZ (CNC router builder)", "Technical Tip of the Week: How to Use a Compression Bit",
        "https://www.axyz.com/using-a-compression-bit/", "html/wood_axyz_compression_bit_tip.html",
        "Machine builder, not a tool vendor. Prose: up-cut tools may plunge; down-cut and compression tools should ramp. No numbers."),
    # dead ends
    src("wood_dead_freud_cnc_2017", "Freud", "Router bit feed and speed for CNC (2017-08-22)",
        "https://www.freudtools.com/public/assets/freud/downloadables/freudtools-router-bit-feed-and-speed-for-cnc-20170822.pdf",
        "pdf/wood_freud_cnc_feed_speed_2017.pdf",
        "Dead end for G10: chip load chart and formulas only; no ramp, plunge or helix text (pdftotext grep).", text=False),
    src("wood_dead_vortex_chipload_chart", "Vortex Tool", "Vortex chip load chart (one-page image PDF)",
        "https://www.vortextool.com/media/assets/chipLoadChart.pdf", "pdf/wood_vortex_chipload_chart.pdf",
        "Dead end: an image-only page (Photoshop). Rendered and read: chip loads and RPM formulas only; no plunge or ramp text.", text=False),
    src("wood_dead_amana_product_46350", "Amana Tool", "46350 mortise compression product page",
        "https://www.amanatool.com/46350-cnc-solid-carbide-mortise-compression-spiral-1-4-dia-x-1-inch-x-1-4-shank.html", None,
        "Dead end: HTTP 403 to curl (also with a browser user agent).", text=False, status=403),
    src("wood_dead_amana_tech_info", "Amana Tool", "Router bit technical information page",
        "https://www.amanatool.com/router-bit-technical-information", None,
        "Dead end: HTTP 403. The Wayback copy stored for G4 (fetch/G4/sources/amana_router_bit_technical_information_wayback_20240528.txt) has no ramp or helix text.",
        text=False, status=403),
]

IPM = 25.4


def feed_pair(feed, ramp, label):
    return [
        {"quantity": f"{label} ramp/feed fraction", "value": round(ramp / feed, 4), "formula": f"{ramp} / {feed}"},
        {"quantity": "ramp_feed_mm_per_min", "value": round(ramp * IPM, 1), "formula": f"{ramp} IPM x 25.4"},
    ]


def st(sid, source, page, param, fam, sub, flutes, cc, dia, mat, vp, unit, vsi, derived, verb, grade, notes=""):
    return {
        "statement_id": sid, "source_id": source, "source_page": page, "parameter": param,
        "tool_family": fam, "tool_subfamily": sub, "flutes": flutes, "centre_cutting": cc,
        "diameter_mm": dia, "material": mat, "value_printed": vp, "unit": unit, "value_si": vsi,
        "derived": derived, "verbatim": verb, "grade": grade, "notes": notes,
    }


RULE = "To find Ramp Down: Feed Rate IPM / # of flutes"
RULE_NOTE = ("Amana does not say whether 'Ramp Down' is a pure Z plunge feed or the feed along a ramp. "
             "ToolsToday (wood_toolstoday_calc_video) reads it as 'ramp-down or plunge rate'.")


def rule_derived():
    return [{"quantity": "ramp_feed_fraction for z flutes", "value": "1/z (1 flute 1.0, 2 flute 0.5, 3 flute 0.333)",
             "formula": "(Feed / z) / Feed = 1 / z"}]


S = []
# Amana compression v8
S += [
    st("g10-wood-amana-comp-rule", "wood_amana_compression_v8", "p1, Simple Machining Calculations", "ramp_feed_fraction",
       "compression", "solid carbide compression spiral", None, None, None, {"class": "any", "label": "all chart materials"},
       RULE, "IPM", None, rule_derived(), RULE, "b", RULE_NOTE + " The chart gives compression bits a ramp-down feed; it does not forbid plunge or ramp."),
    st("g10-wood-amana-comp-1f-half-wood", "wood_amana_compression_v8", "p1, 1 Flute table, 1/2\" row, Wood", "ramp_feed",
       "compression", "1 flute compression", 1, None, 12.7, {"class": "wood", "label": "Wood"},
       "140\" feed, 140\" ramp down", "IPM", round(140 * IPM, 1), feed_pair(140, 140, "1 flute"),
       '1/2" 140" .0077" 140" 280" .0153" 280"', "a", "Printed ramp = feed for 1 flute; this agrees with Feed / # of flutes."),
    st("g10-wood-amana-comp-2f-quarter-wood", "wood_amana_compression_v8", "p1, 2 Flute table, 1/4\" row, Wood", "ramp_feed",
       "compression", "2 flute compression", 2, None, 6.35, {"class": "wood", "label": "Wood"},
       "110\" feed, 55\" ramp down", "IPM", round(55 * IPM, 1), feed_pair(110, 55, "2 flute"),
       '1/4" 110" .0031" 55" 220" .0061" 110"', "a"),
    st("g10-wood-amana-comp-2f-half-mdf", "wood_amana_compression_v8", "p1, 2 Flute table, 1/2\" row, MDF/Laminate", "ramp_feed",
       "compression", "2 flute compression", 2, None, 12.7, {"class": "mdf", "label": "MDF/Laminate"},
       "400\" feed, 200\" ramp down", "IPM", round(200 * IPM, 1), feed_pair(400, 200, "2 flute"),
       '1/2" 280" .0077" 140" 400" .0111" 200" 280" .0077" 140"', "a", "Same row also prints Wood 280/140 and Plywood 280/140."),
    st("g10-wood-amana-comp-3f-half-wood", "wood_amana_compression_v8", "p1, 3 Flute table, 1/2\" row, Wood", "ramp_feed",
       "compression", "3 flute compression", 3, None, 12.7, {"class": "wood", "label": "Wood"},
       "350\" feed, 117\" ramp down", "IPM", round(117 * IPM, 1), feed_pair(350, 117, "3 flute"),
       '1/2" 350" .0065" 117" 450" .0080" 150"', "a", "117 = 350/3 rounded."),
]
# Amana Spektra v24
S += [
    st("g10-wood-amana-spektra-rule", "wood_amana_spektra_plunge_v24", "p1, Simple Machining Calculations", "ramp_feed_fraction",
       "flat_end_mill", "up-cut and down-cut spiral plunge", None, None, None, {"class": "any", "label": "Wood/Plywood, MDF/Laminate"},
       "Feed Rate IPM / # of flutes", "IPM", None, rule_derived(), "Feed Rate IPM / # of flutes", "b",
       RULE_NOTE + " The two-column layout puts 'To find Ramp Down:' on the line above the formula, with table cells between them."),
    st("g10-wood-amana-spektra-2f-quarter", "wood_amana_spektra_plunge_v24", "p1, 2 Flute table, 46102-K / 46202-K 1/4\" row", "ramp_feed",
       "flat_end_mill", "2 flute up-cut / down-cut spiral", 2, None, 6.35, {"class": "wood", "label": "Wood/Plywood"},
       "180\" feed, 90\" ramp down", "IPM", round(90 * IPM, 1), feed_pair(180, 90, "2 flute"),
       '46102-K 46202-K 1/4" 180" .0050" 90" 215" .0060" 107.5"', "a",
       "Same row, MDF/Laminate: 215/107.5 (fraction 0.5). The same numbers apply to the up-cut and the down-cut part number."),
    st("g10-wood-amana-spektra-3f-quarter", "wood_amana_spektra_plunge_v24", "p1, 3 Flute table, 46002-K / 46052-K 1/4\" row", "ramp_feed",
       "flat_end_mill", "3 flute up-cut / down-cut spiral", 3, None, 6.35, {"class": "wood", "label": "Wood/Plywood"},
       "270\" feed, 90\" ramp down", "IPM", round(90 * IPM, 1), feed_pair(270, 90, "3 flute"),
       '46002-K 46052-K 1/4" 270" .0050" 90" 325" .0060" 109"', "a"),
    st("g10-wood-amana-spektra-3f-3-4", "wood_amana_spektra_plunge_v24", "p1, 3 Flute table, 46500-K 3/4\" row", "ramp_feed",
       "flat_end_mill", "3 flute down-cut spiral", 3, None, 19.05, {"class": "wood", "label": "Wood/Plywood"},
       "330\" feed, 110\" ramp down", "IPM", round(110 * IPM, 1), feed_pair(330, 110, "3 flute"),
       '46500-K 3/4" 330" .009" 110" 360" .010" 120"', "a"),
]
# Amana ball nose, corner radius, engraving: rule only
S += [
    st("g10-wood-amana-ballnose-rule", "wood_amana_ball_nose_v7", "p1, Simple Machining Calculations", "ramp_feed_fraction",
       "ball_nose", "2 flute spiral ball nose", 2, None, None, {"class": "any", "label": "Softwood, Hardwood, MDF and others"},
       "Feed Rate IPM / # of flutes", "IPM", None,
       [{"quantity": "ramp_feed_fraction (2 flute)", "value": 0.5, "formula": "(Feed / 2) / Feed"}],
       "To find Ramp Down: Feed Rate IPM / # of flutes", "b", RULE_NOTE + " No ramp column is printed for the ball nose."),
    st("g10-wood-amana-bullnose-rule", "wood_amana_corner_radius_plunge", "p1, Simple Machining Calculations", "ramp_feed_fraction",
       "bull_nose", "2 flute spiral plunge with corner radius", 2, None, [6.35, 12.7], {"class": "any", "label": "Soft Wood, Hard Wood, MDF, Plastics, Aluminum"},
       "Feed Rate IPM / # of flutes", "IPM", None,
       [{"quantity": "ramp_feed_fraction (2 flute)", "value": 0.5, "formula": "(Feed / 2) / Feed"}],
       "To find Ramp Down: Feed Rate IPM / # of flutes", "b", RULE_NOTE + " No ramp column is printed."),
    st("g10-wood-amana-engraving-rule", "wood_amana_spektra_engraving_v4", "p1, Simple Machining Calculations", "ramp_feed_fraction",
       "v_bit", "single flute engraving 15/30/45/120 degree", 1, None, None, {"class": "any", "label": "Soft Wood, Hard Wood, plastics, Solid Surface"},
       "Feed Rate IPM / # of flutes", "IPM", None,
       [{"quantity": "ramp_feed_fraction (1 flute)", "value": 1.0, "formula": "(Feed / 1) / Feed"}],
       "To find Ramp Down: Feed Rate IPM / # of flutes", "b",
       RULE_NOTE + " The insert V-groove chart prints Feed/2 for 1-flute V-bits (g10-wood-amana-vgroove-1f-60); the rule and the printed values disagree."),
]
# Amana insert V-groove
S += [
    st("g10-wood-amana-vgroove-rule", "wood_amana_insert_vgroove_v16", "p1, footer Simple Machining Calculations", "ramp_feed_fraction",
       "v_bit", "insert carbide V-groove", None, None, None, {"class": "any", "label": "all chart materials"},
       "Feed Rate IPM / # of Flutes)", "IPM", None, rule_derived(),
       "To ﬁnd Ramp Down: Feed Rate IPM / # of Flutes)", "b",
       "The text copy carries the ligature 'fi' as U+FB01. The printed 1-flute rows below 120 degree do not follow this rule; see NOTES section 5."),
    st("g10-wood-amana-vgroove-1f-60", "wood_amana_insert_vgroove_v16", "p1, RC-1108 row (60 deg, 1 flute, 18,000 RPM)", "ramp_feed",
       "v_bit", "insert V-groove 60 deg", 1, None, None, {"class": "hardwood", "label": "Hardwood (Softwood and Plywood/Chipboard print the same)"},
       "40\" feed, 20\" ramp down", "IPM", round(20 * IPM, 1), feed_pair(40, 20, "1 flute V-bit"),
       'RC-1108 60° 1 18,000 40" .0024" 20" 40" .0024" 20"', "a",
       "Fraction 0.5 at 1 flute. The chart rule gives Feed/1 = 40. Every 1-flute row from 40 to 110 degree prints Feed/2; MDF prints 90/45."),
    st("g10-wood-amana-vgroove-1f-120", "wood_amana_insert_vgroove_v16", "p1, RC-1146 row (120 deg, 1 flute, 14,000 RPM)", "ramp_feed",
       "v_bit", "insert V-groove 120 deg", 1, None, None, {"class": "hardwood", "label": "Hardwood"},
       "90\" feed, 90\" ramp down", "IPM", round(90 * IPM, 1), feed_pair(90, 90, "1 flute V-bit"),
       'RC-1146 120° 1 14,000 90" .0024" 90" 90" .0024" 90"', "a", "Fraction 1.0; this row follows the rule. RC-1101 (150 deg, 1 flute) prints the same."),
    st("g10-wood-amana-vgroove-2f-120", "wood_amana_insert_vgroove_v16", "p1, RC-1104 row (120 deg, 2 flute, 18,000 RPM)", "ramp_feed",
       "v_bit", "insert V-groove 120 deg", 2, None, None, {"class": "hardwood", "label": "Hardwood"},
       "90\" feed, 45\" ramp down", "IPM", round(45 * IPM, 1), feed_pair(90, 45, "2 flute V-bit"),
       'RC-1104 120° 2 18,000 90" .0024" 45" 90" .0024" 45"', "a", "Fraction 0.5; every 2-flute row prints Feed/2."),
]
# ToolsToday
S += [
    st("g10-wood-tt-compression-mdf", "wood_toolstoday_calc_video", "transcript, 46172-K example", "plunge_feed",
       "compression", "2 flute Spektra compression (up-down)", 2, None, 9.525, {"class": "mdf", "label": "MDF"},
       "260 IPM feed, ramp-down or plunge rate 130 IPM", "IPM", round(130 * IPM, 1), feed_pair(260, 130, "2 flute"),
       "feed rate is 260 inches per minute, producing a chip load of 0.0072 with a ramp-down or plunge rate of 130 inches per minute",
       "b", "Distributor reads the Amana chart. The phrase shows that the distributor treats 'Ramp Down' as the plunge rate."),
    st("g10-wood-tt-46172-740", "wood_toolstoday_calc_video", "description, running parameters, 46172-K", "ramp_feed",
       "compression", "2 flute Spektra compression", 2, None, 9.525, {"class": "other", "label": "melamine / laminate (video demo)"},
       "Feed 740, Ramp Down 370 IPM", "IPM", round(370 * IPM, 1), feed_pair(740, 370, "2 flute"),
       "Feed Rate (IPM): 740 Speed (RPM): 18,000 Chip Load (Per Tooth): 0.020\" Ramp Down: 370 IPM", "b",
       "Operator values above the chart; the ramp stays at Feed/2."),
    st("g10-wood-tt-rc1108", "wood_toolstoday_calc_video", "description, running parameters, RC-1108", "ramp_feed",
       "v_bit", "60 deg insert V-groove, 1 flute", 1, None, None, {"class": "other", "label": "melamine / laminate (video demo)"},
       "Feed 30, Ramp Down 15 IPM", "IPM", round(15 * IPM, 1), feed_pair(30, 15, "1 flute V-bit"),
       "Feed Rate (IPM): 30 Speed (RPM): 18,000 Chip Load (Per Tooth): 0.002\" Ramp Down: 15 IPM", "b",
       "1-flute V-bit at Feed/2; this agrees with the printed chart rows, not with the rule Feed/# of flutes."),
    st("g10-wood-tt-half", "wood_toolstoday_understanding_feeds_speeds", "section 'Ramp and Plunge Feed Rates'", "plunge_feed_fraction",
       "any", None, None, None, None, {"class": "wood", "label": "not stated (router bit retailer)"},
       "about half the main feed rate", "fraction", 0.5, [],
       "many CNC users reduce ramp or plunge feed to about half the main feed rate", "c",
       "Applies to ramp and plunge alike. 'The right number depends on the bit, material, depth of cut, and toolpath strategy.'"),
]
# Vortex
S += [
    st("g10-wood-vortex-downcut-noplunge", "wood_vortex_catalog", "p12-13, Downcut Spirals", "no_plunge",
       "flat_end_mill", "down-cut spiral", None, None, None, {"class": "wood", "label": "wood"},
       "CANNOT be used to plunge straight into wood", None, None, [],
       "Downcut tools CANNOT be used to plunge straight into wood and should be ramped into the part.", "c"),
    st("g10-wood-vortex-1300-noplunge", "wood_vortex_catalog", "Series 1300 two flute downcut finishing spirals", "no_plunge",
       "flat_end_mill", "down-cut spiral", 2, None, None, {"class": "wood", "label": "wood tooling"},
       "Never plunge straight down with downcut tooling", None, None, [],
       "Never plunge straight down with downcut tooling as this may cause fire or breakage.", "c"),
    st("g10-wood-vortex-upcut-plunge", "wood_vortex_catalog", "p12, Upcut Spirals", "entry_general",
       "flat_end_mill", "up-cut spiral", None, True, None, {"class": "wood", "label": "wood"},
       "straight plunge/drill", None, None, [],
       "Upcut spirals straight plunge/drill and have good end cutting geometry.", "c"),
    st("g10-wood-vortex-veining-plunge", "wood_vortex_catalog", "Series 3700 solid carbide veining bits", "entry_general",
       "ball_nose", "veining bit (radius end)", None, True, None, {"class": "any", "label": "wide variety of materials"},
       "end cutting so they can plunge", None, None, [],
       "so they can plunge and be used to groove material while", "c"),
]
# Onsrud
S += [
    st("g10-wood-onsrud-guide-ramped-plunge", "wood_onsrud_routing_guide", "p (CNC Feed & Speeds), Additional methods of heat reduction", "entry_general",
       "any", None, None, None, None, {"class": "wood", "label": "wood (page lists typical feed rates in wood)"},
       "Ramped Plunging into the workpiece", None, None, [],
       "- Ramped Plunging into the workpiece", "c", "The same list names '- Higher Plunge Speeds' and 'Get in and start making chips'."),
    st("g10-wood-onsrud-guide-oscillate", "wood_onsrud_routing_guide", "Oscillating to improve tool life", "entry_general",
       "any", None, None, None, None, {"class": "plywood", "label": "plywood, laminated MDF, particleboard"},
       "ramp the tool up and down through each tangent of the part", None, None, [],
       "is to ramp the tool up and down through each", "c", "Oscillation for glue-line wear; not an entry rule."),
    st("g10-wood-onsrud-guide-pcd", "wood_onsrud_routing_guide", "p4, PCD Diamond cons", "no_plunge",
       "other", "PCD diamond tipped", None, False, None, {"class": "any", "label": "not stated"},
       "TIPICALLY CANNOT PLUNGE", None, None, [], "TIPICALLY CANNOT PLUNGE", "c", "Spelling as printed. The same page lists solid carbide 'BEST PLUNGING CAPABIITIES'."),
    st("g10-wood-onsrud-pct19-potted", "wood_onsrud_pct19", "p31, 34-100 Series Potted Fastener Tools Technical Data", "plunge_feed_fraction",
       "other", "potted fastener tool", None, None, None, {"class": "other", "label": "composite honeycomb panels (aluminium: pre-drill)"},
       "RPM 10,000; Plunge Feed Rate 40 IPM; Feed Rate 80 IPM", "IPM", round(40 * IPM, 1),
       [{"quantity": "plunge/feed fraction", "value": 0.5, "formula": "40 / 80"}],
       "10,000 40 IPM 80 IPM", "c", "Only printed plunge feed in the Onsrud catalog; not a wood tool."),
    st("g10-wood-onsrud-pct19-burr", "wood_onsrud_pct19", "66-500 Series point styles", "entry_general",
       "other", "diamond-coated composite router (burr / end mill / drill point)", None, None, [3.175, 6.35], {"class": "other", "label": "carbon fiber laminates"},
       "Burr end for ramping and helical interpolation; End mill point for plunging and helical interpolation", None, None, [],
       "Burr end for ramping and helical interpolation.", "c",
       "Implies that the burr end is not for plunging. The next line prints '■ End mill point for plunging and helical interpolation.'"),
    st("g10-wood-onsrud-faq2-helical", "wood_onsrud_plastics_faq2", "How Can Chip Wrap, Crazing, or Keyhole Slots Be Prevented When Plunging?", "entry_general",
       "any", None, None, None, None, {"class": "other", "label": "plastics"},
       "ramp into transverse cuts and use a helical ramp with interpolation for holes", None, None, [],
       "A better approach is to ramp into transverse cuts and use a helical ramp with interpolation for holes.", "c",
       "No helix diameter or pitch is printed."),
    st("g10-wood-onsrud-faq2-plungefeed", "wood_onsrud_plastics_faq2", "Crazing", "plunge_feed",
       "any", "flat-bottom cutter", None, None, None, {"class": "other", "label": "plastics"},
       "reducing plunge feed rate can reduce stress", None, None, [],
       "If ramping is not possible, increasing spindle speed or reducing plunge feed rate can reduce stress", "c"),
]
# IDC Woodcraft (plunge column)
IDCMAT = {"class": "wood", "label": "soft, medium and moderately hard wood (benchtop CNC)"}


def idc(sid, page, fam, sub, flutes, dia_in, feed, plunge, verb, notes=""):
    return st(sid, "wood_idc_feeds_speeds", page, "plunge_feed", fam, sub, flutes, None,
              round(dia_in * 25.4, 3) if dia_in else None, IDCMAT,
              f"Feed {feed} in/min, Plunge {plunge} in/min", "in/min", round(plunge * IPM, 1),
              [{"quantity": "plunge/feed fraction", "value": round(plunge / feed, 4), "formula": f"{plunge} / {feed}"},
               {"quantity": "plunge_feed_mm_per_min", "value": round(plunge * IPM, 1), "formula": f"{plunge} x 25.4"}],
              verb, "a", notes)


S += [
    idc("g10-wood-idc-down-eighth", "Section 2, DOWN CUT ENDMILL, 1/8\" row", "flat_end_mill", "down cut", 2, 0.125, 50, 15,
        '1/8" 0.125 2 0.750 2.00 0.125 50 15 0.125 40% 22,000'),
    idc("g10-wood-idc-down-quarter", "Section 2, DOWN CUT ENDMILL, 1/4\" row", "flat_end_mill", "down cut", 2, 0.25, 70, 30,
        '1/4" 0.25 2 1.000 3.00 0.25 70 30 0.25 40% 19,000'),
    idc("g10-wood-idc-up-eighth", "Section 2, UP CUT ENDMILL, 1/8\" row", "flat_end_mill", "up cut", 2, 0.125, 50, 25,
        '1/8" 0.125 2 0.750 2.00 0.125 50 25 0.125 40% 22,000', "Up cut 1/8\" plunges at 25 against down cut 15 at the same feed."),
    idc("g10-wood-idc-up-quarter", "Section 2, UP CUT ENDMILL, 1/4\" row", "flat_end_mill", "up cut", 2, 0.25, 80, 40,
        '1/4" 0.25 2 1.00 3.00 0.25 80 40 0.250 40% 19,000'),
    idc("g10-wood-idc-ball-eighth", "Section 2, BALLNOSE, 1/8\" row", "ball_nose", "ball nose", 2, 0.125, 60, 15,
        "1/8\" Ballnose 0.125 2 0.500 2.00 0.125 60 15 0.05 8%"),
    idc("g10-wood-idc-ball-quarter", "Section 2, BALLNOSE, 1/4\" row", "ball_nose", "ball nose", 2, 0.25, 70, 30,
        "1/4\" Ballnose 0.25 2 1.00 3.00 0.25 70 30 0.12 8%"),
    idc("g10-wood-idc-v30", "Section 2, V-BIT, 30 deg row", "v_bit", "30 deg V-bit (side angle 15)", 1, 0.25, 35, 20,
        "30° V-bit 0.25 1 0.750 2.00 0.25 15 35 20 0.025"),
    idc("g10-wood-idc-v60", "Section 2, V-BIT, 60 deg row", "v_bit", "60 deg V-bit (side angle 30)", 2, 0.25, 60, 20,
        "60° V-bit 0.25 2 0.216 2.00 0.25 30 60 20 0.05", "The starter-set table on p3 prints Feed 40 for the same bit."),
    idc("g10-wood-idc-v90", "Section 2, V-BIT, 90 deg row", "v_bit", "90 deg V-bit (side angle 45)", 2, 0.25, 45, 25,
        "90° V-bit 0.25 2 0.125 2.00 0.25 45 45 25 0.1"),
    idc("g10-wood-idc-v120", "Section 2, V-BIT, 120 deg row", "v_bit", "120 deg V-bit (side angle 60)", 2, 1.0, 80, 30,
        "120° V-bit 1.0 2 0.288 2.00 0.25 60 80 30 0.19"),
    idc("g10-wood-idc-taperball", "Section 2, TAPER BALLNOSE CARVING BIT, first row", "tapered_ball_nose", "taper ball nose, tip radius 0.015, 10 deg", 2, 0.25, 60, 25,
        "0.250 2 0.750 2.00 0.25 0.015 10 60 25 0.25 5-8%", "Cut Dia 0.250 is the shank-end diameter; the tip radius is 0.015 in. The 0.045 tip-radius row prints the same feeds."),
    idc("g10-wood-idc-surfacing-1", "Section 2, SURFACING BIT, 1\" row", "flat_end_mill", "4 flute surfacing bit", 4, 1.0, 150, 7,
        "1“ Surfacing 1.0 4 0.250 2.00 0.25 150 7 0.125 70%", "Fraction 0.047; a surfacing bit barely plunges."),
    idc("g10-wood-idc-bowl", "Section 2, BOWL BIT, row", "bull_nose", "bowl bit (radiused endmill)", 2, 1.0, 80, 15,
        "Bowl 1.00 2 .75 2.00 0.25 80 15 0.250 40%"),
    st("g10-wood-idc-spiral-drill", "wood_idc_feeds_speeds", "p3, 1/8\" DRILLING ENDMILL table and note", "entry_general",
       "flat_end_mill", "1/8 in up-cut endmill used for spiral drilling", 2, True, 3.175, IDCMAT,
       "Drilling: Feed 50, Plunge 60*; Conventional: Feed 50, Plunge 25", "in/min", None,
       [{"quantity": "spiral-drilling plunge/feed", "value": 1.2, "formula": "60 / 50"},
        {"quantity": "conventional plunge/feed", "value": 0.5, "formula": "25 / 50"}],
       "* The plunge value is for using the spiral drilling technique.", "b",
       "Spiral drilling is a helical entry; the chart gives no helix diameter or pitch. The metric table prints 1270/635 and 1270/638 mm/min."),
]
# Whiteside, AXYZ
S += [
    st("g10-wood-whiteside-upcut", "wood_whiteside_cnc_brochure", "p2, Up Cut Benefits", "entry_general",
       "flat_end_mill", "up cut spiral", None, None, None, {"class": "wood", "label": "not stated"},
       "Best At Plunge Cuts", None, None, [], "Best At Plunge Cuts", "c", "The only Whiteside entry statement found."),
    st("g10-wood-axyz-compression-ramp", "wood_axyz_compression_bit_tip", "How do I use this tool correctly?", "entry_general",
       "compression", "compression and down-cut spiral", None, None, None, {"class": "plywood", "label": "double side laminates, melamine, plywood"},
       "best to ramp into the material", None, None, [],
       "However, for downward spirals and/or compression tools, it is best to ramp into the material.", "c",
       "Advice, not a prohibition. The page also says 'a direct plunge into the material will be fine for your standard upwards tool'. Machine builder, not tool vendor."),
]


def main():
    parts = os.path.join(G10, "parts")
    os.makedirs(parts, exist_ok=True)
    with open(os.path.join(parts, "wood_sources.json"), "w") as f:
        json.dump(SOURCES, f, indent=2, ensure_ascii=False)
        f.write("\n")
    with open(os.path.join(parts, "wood_statements.json"), "w") as f:
        json.dump(S, f, indent=2, ensure_ascii=False)
        f.write("\n")
    print(len(SOURCES), "sources,", len(S), "statements")


if __name__ == "__main__":
    main()
