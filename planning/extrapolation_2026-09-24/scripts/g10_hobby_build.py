#!/usr/bin/env python3
"""G10 fetch, part `hobby`: build parts/hobby_sources.json and
parts/hobby_statements.json from the stored texts.

The script reads the stored texts under fetch/G10/sources/, computes the
text and raw hashes, parses the tabular sources (Sienci chart, Carbide 3D
charts) row by row, and adds the prose statements listed below. Every
verbatim string is asserted against the stored text before the files are
written. Every computed number goes into `derived` with its formula.

Usage: python3 g10_hobby_build.py
"""
import hashlib
import json
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
G10 = os.path.normpath(os.path.join(HERE, "..", "fetch", "G10"))
IN2MM = 25.4
ACC = "2026-09-25"


def sha(path):
    return hashlib.sha256(open(path, "rb").read()).hexdigest()


def norm(s):
    return re.sub(r"\s+", " ", s).strip()


# ---------------------------------------------------------------- sources
SOURCES = [
    # (source_id, vendor, title, url, raw file (relative to G10) or None, http, coverage)
    ("precisebits_fret_plane", "precisebits",
     "PreciseBits 3-flute Fingerboard Planing and Radiusing Bits (MM3I8), product page",
     "https://www.precisebits.com/products/carbidebits/fret-plane.asp",
     "html/precisebits_fret_plane.html", 200,
     "3-flute corner-radius (bull-nose) cutter, 3.0 mm and 0.125 in, r 0.64 mm. Prints a plunge rate per Janka class "
     "(75 / 50 / 40 in/min). No ramp or helix guidance. G6 holds an earlier copy "
     "(fetch/G6/sources/precisebits_fret_plane.txt, text sha256 9777923942c6...); the raw HTML differs by page chrome only."),
    ("precisebits_vtip_2500", "precisebits",
     "PreciseBits V-Groove Router Bits (EM2E4), 1/4 in. shank, 2-flute V-tip, product page",
     "https://www.precisebits.com/products/carbidebits/2500_scoreengrave.asp",
     "html/precisebits_vtip_2500.html", 200,
     "2-flute V-tip, 0.015 in tip, 30/45/60/90 deg. Prints 'Plunge rate = 40 IPM to 250 IPM' for all materials. "
     "No ramp or helix guidance. G5 holds an earlier copy (fetch/G5/sources/precisebits_vtip_2500.txt)."),
    ("precisebits_vtip_scoreengrave", "precisebits",
     "PreciseBits 2-flute Micro-engraving, PCB Trace Isolation Bits (EM2E8), product page",
     "https://www.precisebits.com/products/carbidebits/scoreengrave.asp",
     "html/precisebits_vtip_scoreengrave.html", 200,
     "2-flute V point, 0.005 in tip, 1/8 in shank. Prints 'Plunge rate = 5 IPM to 25 IPM' and a plunge-style tip. "
     "G5 holds an earlier copy (fetch/G5/sources/precisebits_vtip_scoreengrave.txt)."),
    ("precisebits_tapered_ball_2f", "precisebits",
     "PreciseBits Carbide 2-flute Tapered Ball-nose 3D Carving Tools, product page",
     "https://www.precisebits.com/products/carbidebits/taperedcarve250b2f.asp",
     "html/precisebits_tapered_ball_2f.html", 200,
     "Tapered ball nose, 2 flutes. Prints 'Plunge rate = depends on speed (RPM)' and no number. No ramp guidance. "
     "G5 holds an earlier copy (fetch/G5/sources/precisebits_tapered_ball_2f.txt)."),
    ("precisebits_point_styles", "precisebits",
     "PreciseBits reference: Cutting Tool Tip Styles",
     "https://www.precisebits.com/reference/point_styles.htm",
     "html/precisebits_point_styles.html", 200,
     "Tip-style table. The ball end is a 'center cutting plunge point'; the radical fish-tail has a 'minimum 20 deg angle' "
     "for plunge cuts. No feed or angle number for entry moves."),
    ("carbide3d_s3_feeds_250", "carbide3d",
     "Carbide 3D Shapeoko 3 Feeds & Speeds chart, #201 .25 in square / #202 .25 in ball (S3_feeds_250.pdf)",
     "https://web.archive.org/web/20211118030340/https://docs.carbide3d.com/support/supportfiles/S3_feeds_250.pdf",
     "pdf/carbide3d_s3_feeds_250.pdf", 200,
     "One table: DOC, RPM, router dial, FEED (ipm), PLUNGE (ipm) per material, one row shared by #201 (3F square) and #202 "
     "(2F ball); the header naming the tools is an image (see EXTRAPOLATION_G9_machine_class.md). Wood rows: Bamboo, Pine, "
     "Mahogany, Plywood, MDF. No ramp or helix guidance. The PDF bytes are the G9 copy (fetch/G9/pdf/S3_feeds_250.pdf); "
     "web.archive.org timed out on 2026-09-25 (two tries); the sha256 equals the G9 record."),
    ("carbide3d_nomad883_feeds_125", "carbide3d",
     "Carbide 3D Nomad 883 Feeds & Speeds chart, #101 / #102 .125 in (Nomad883_feeds_125.pdf)",
     "https://web.archive.org/web/20221027042155/https://docs.carbide3d.com/support/supportfiles/Nomad883_feeds_125.pdf",
     "pdf/carbide3d_nomad883_feeds_125.pdf", 200,
     "Benchtop mill chart (10 krpm class spindle): DOC, RPM, FEED (ipm), PLUNGE (ipm) per material for #101 / #102 at "
     ".125 in (header labels #101 Square, #102 Ball per G9). Wood rows: Bamboo, Pine, Mahogany, Plywood, MDF. G9 excluded it "
     "from the LUT as a different machine class; here it is a second witness of the plunge/feed ratio only. Bytes copied from "
     "fetch/G9/pdf/Nomad883_feeds_125.pdf; the live docs.carbide3d.com URL returned an HTML page (38 134 B) on 2026-09-25."),
    ("carbide3d_community_ramping_cc_pro", "carbide3d",
     "Carbide 3D Community: 'Ramping in Carbide Create Pro' (post by robgrz, Carbide 3D staff, 2022-09-29)",
     "https://community.carbide3d.com/t/ramping-in-carbide-create-pro/50824",
     "html/carbide3d_community_ramping_cc_pro.html", 200,
     "The Carbide Create developer (staff flag true in the Discourse JSON) states the CC Pro ramp feed rule: 1/3 of the "
     "feed rate or 100% of the plunge rate, whichever is higher; and that pockets prefer a helix. Grade c: a CAM default, "
     "not tool data. No ramp angle default printed in this thread."),
    ("carbide3d_community_ramp_entry_angle", "carbide3d",
     "Carbide 3D Community: 'Ramp Entry Angle' (user thread, 2024-03-27)",
     "https://community.carbide3d.com/t/ramp-entry-angle/75578",
     "html/carbide3d_community_ramp_entry_angle.html", 200,
     "A user reports the Carbide Create ramp entry default as 20 degrees; a forum regular (not staff) says a centre-cutting "
     "tool can ramp at any angle and a solid-centre side-cutting tool must not ramp or plunge. Weak witness (grade c, user posts)."),
    ("sienci_feeds_speeds_metric", "sienci",
     "Sienci Labs Feeds & Speeds chart, metric (FeedsSpeedsMetric.pdf, InDesign 2022-11-08), 4 pages",
     "https://raw.githubusercontent.com/Sienci-Labs/Resources/main/_downloads/FeedsSpeedsMetric.pdf",
     "pdf/sienci_feeds_speeds_metric.pdf", 200,
     "Machine vendor chart for its own tool range on the LongMill (Makita router). Columns: Feed Rate (mm/min), Plunge Rate "
     "(mm/min), Stepover, Stepdown slotting / pocketing, RPM, dial. Pages: Softwood/Soft Plywood/MDF; Hardwood/Hard Plywood; "
     "Plastics; Aluminum. Rows for V-bits, tapered ball, ball, flat UC/DC, single flute, corncob, surfacing, round groove. "
     "Plunge is printed per row, so the plunge/feed fraction per tool family is derivable. No ramp or helix column."),
    ("sienci_lm_feeds_and_speeds", "sienci",
     "Sienci Labs Resources: LongMill Feeds & Speeds (Markdown source of resources.sienci.com/view/lm-feeds-and-speeds/)",
     "https://raw.githubusercontent.com/Sienci-Labs/Resources/main/longmill/lm-the-basics/lm-feeds-and-speeds.md",
     "html/sienci_lm_feeds_and_speeds.md", 200,
     "Prose: plunge rates of 100 to 300 mm/min for most materials, for 2-flute 1/8 in carbide end mills. Raw file is Markdown "
     "with HTML tables; converted with g5_html_to_text.py (header line says raw_md_sha256)."),
    ("vectric_v12_profile_toolpath", "vectric",
     "Vectric VCarve Pro V12 help: 2D Profile Toolpath (Ramps section)",
     "https://docs.vectric.com/docs/V12.0/VCarvePro/ENU/Help/form/uiProfileMachineForm/index.html",
     "html/vectric_v12_profile_toolpath.html", 200,
     "CAM behaviour: all ramp moves run at the tool's plunge rate; ramp by Distance or Angle; the Angle option is for cutters "
     "that cannot plunge and have a manufacturer entry angle; Spiral computes its own angle. No default angle printed."),
    ("vectric_tool_database_v11", "vectric",
     "Vectric VCarve Pro V11 help: Tool Database",
     "https://docs.vectric.com/docs/V11.0/VCarvePro/ENU/Help/form/Tool%20Database/index.html",
     "html/vectric_tool_database_v11.html", 200,
     "Defines Plunge Rate as the rate for vertical moves and for ramping moves. G5 holds an earlier copy "
     "(fetch/G5/sources/vectric_tool_database_v11.txt)."),
    ("fusion_adaptive_roughing_reference", "autodesk",
     "Autodesk Fusion CAM help: 3D Adaptive Roughing reference (Linking: ramp parameters)",
     "https://help.autodesk.com/cloudhelp/ENU/Fusion-CAM/files/GUID09E44604-DAD8-47D6-ADC6-C100869DE724.htm",
     "html/fusion_adaptive_roughing_reference.html", 200,
     "Defines Ramping Angle, Ramp Taper Angle, Maximum Ramp Stepdown, Ramp Clearance Height, Helical Ramp Diameter, Minimum "
     "Ramp Diameter, Ramp Feedrate and Plunge Feedrate. Prints no default values. The Helical Ramp Diameter text says a value "
     "bigger than the tool diameter can leave a boss; the figure captions compare 1.8 x Dia (boss) with 0.8 x Dia."),
    ("cnccookbook_helical_ramp_angle", "cnccookbook",
     "CNCCookbook: Helical Interpolation Ramp Angle Calculator: Best Ways to Enter a Cut",
     "https://www.cnccookbook.com/helical-interpolation-ramp-angle-calculator-best-ways-to-enter-a-cut/",
     "html/cnccookbook_helical_ramp_angle.html", 200,
     "Software vendor article (G-Wizard), metal example. Prints a plunge rule of thumb (slot feed / flutes), common ramp "
     "angles 1.5-2.5 deg, and cites OSG 10-20 deg. Grade c. Literature substitute: no handbook text was reachable."),
    ("shopbot_user_guide_2015", "shopbot",
     "ShopBot User Guide SBG-00142 (2015-03-17)",
     "https://shopbottools.com/wp-content/uploads/2024/01/SBG-00142-User-Guide-20150317.pdf",
     "pdf/shopbot_user_guide_2015.pdf", 200,
     "Machine vendor guide. For most woods: XY move speed 1.7 in/s start, Z plunge move speed about 0.5 in/s. 'Ramping' in "
     "this guide means acceleration, not an entry move. No tool-specific entry guidance."),
    # dead ends (no stored text)
    ("shopbot_feeds_speeds_charts_2016", "shopbot",
     "ShopBot Feeds and Speeds Charts (2016-07-21)",
     "https://shopbottools.com/wp-content/uploads/2024/01/FeedsandSpeeds.pdf",
     None, 200,
     "Dead end for G10: 12 pages; the wood tables print feed, RPM and chip load only. pdftotext finds no 'plunge', 'ramp' "
     "or 'helix'. Not stored."),
    ("carbide3d_carbide_create_v5_manual", "carbide3d",
     "Carbide Create Simplified 2D CAD/CAM User Manual (v5, 2021)",
     "https://guides.carbide3d.com/files/pdf/carbide-create-v5.pdf",
     None, 200,
     "Dead end: 36 pages; pdftotext finds no 'plunge', 'ramp' or 'helix' (the v5 manual predates CC Pro ramping, 2022-09)."),
    ("carbide3d_create_tool_library", "carbide3d",
     "Carbide Create V8 tool library (ccpro.db)",
     "local install, see EXTRAPOLATION_G9_machine_class.md section 0",
     None, None,
     "Dead end, inherited from G9: the tool library that would hold CC default plunge rates is encrypted; not read."),
    ("onefinity_feeds", "onefinity",
     "Onefinity CNC feeds and speeds",
     "https://forum.onefinitycnc.com/t/feeds-and-speeds-guide-for-beginners/3780",
     None, None,
     "Dead end: search finds only forum threads and a user-made calculator; no vendor-published plunge or ramp guidance."),
    ("literature_wood_entry", "literature",
     "Handbooks and papers on ramp / helical entry in wood",
     "https://api.crossref.org/works?query=... (see hobby_NOTES.md section 4)",
     None, None,
     "Dead end: Machinery's Handbook and Koch 'Wood Machining Processes' are not online; Crossref and Semantic Scholar "
     "searches find no wood study of ramp, helix or plunge entry. Sandvik is covered by the metal part."),
]


def build_sources():
    out = []
    for sid, vendor, title, url, raw, http, cov in SOURCES:
        txt = os.path.join(G10, "sources", sid + ".txt")
        stored = os.path.exists(txt) and raw is not None
        out.append({
            "source_id": sid,
            "source_vendor": vendor,
            "source_title": title,
            "source_url": url,
            "accessed_on": ACC,
            "stored_text": f"sources/{sid}.txt" if stored else None,
            "text_sha256": sha(txt) if stored else None,
            "raw_sha256": sha(os.path.join(G10, raw)) if raw else None,
            "http_status": http,
            "coverage_notes": cov,
        })
    return out


# ------------------------------------------------------------- statements
def st(sid, page, param, family, verbatim, grade, value_printed, unit, value_si,
       sub=None, flutes=None, cc=None, dia=None, mat="any", label=None, derived=None, notes=""):
    return {
        "source_id": sid, "source_page": page, "parameter": param, "tool_family": family,
        "tool_subfamily": sub, "flutes": flutes, "centre_cutting": cc, "diameter_mm": dia,
        "material": {"class": mat, "label": label}, "value_printed": value_printed, "unit": unit,
        "value_si": value_si, "derived": derived or [], "verbatim": verbatim, "grade": grade, "notes": notes,
    }


def ipm(v):
    return round(v * IN2MM, 1)


MANUAL = [
    # PreciseBits fret plane: plunge per Janka class, bull nose 3F
    st("precisebits_fret_plane", "product page, feeds block", "plunge_feed", "bull_nose",
       "Softwood (Janka < 1,500) = 75 inches/minute", "a", "75 inches/minute", "in/min", ipm(75),
       sub="MM3I8 fingerboard planing, r 0.64 mm", flutes=3, dia=[3.0, 3.175], mat="softwood",
       label="Softwood (Janka < 1,500)",
       derived=[{"quantity": "plunge_mm_min", "value": ipm(75), "formula": "75 in/min x 25.4"}],
       notes="Janka threshold 1,500 here; the PreciseBits calibration tutorial uses 1,000 for softwood."),
    st("precisebits_fret_plane", "product page, feeds block", "plunge_feed", "bull_nose",
       "Medium hardness hardwood(1,500 < Janka < 2,500) = 50 inches/minute", "a", "50 inches/minute", "in/min", ipm(50),
       sub="MM3I8 fingerboard planing, r 0.64 mm", flutes=3, dia=[3.0, 3.175], mat="hardwood",
       label="Medium hardness hardwood (1,500 < Janka < 2,500)",
       derived=[{"quantity": "plunge_mm_min", "value": ipm(50), "formula": "50 in/min x 25.4"}]),
    st("precisebits_fret_plane", "product page, feeds block", "plunge_feed", "bull_nose",
       "High hardness hardwoodwood (Janka > 2,500) = 40 inches/minute", "a", "40 inches/minute", "in/min", ipm(40),
       sub="MM3I8 fingerboard planing, r 0.64 mm", flutes=3, dia=[3.0, 3.175], mat="hardwood",
       label="High hardness hardwood (Janka > 2,500)",
       derived=[{"quantity": "plunge_mm_min", "value": ipm(40), "formula": "40 in/min x 25.4"}],
       notes="The page prints the typo 'hardwoodwood'."),
    # PreciseBits V-tips
    st("precisebits_vtip_2500", "product page, feeds block", "plunge_feed", "v_bit",
       "Plunge rate = 40 IPM to 250 IPM (again depending on material being cut)", "b", "40 IPM to 250 IPM", "in/min",
       [ipm(40), ipm(250)], sub="EM2E4 V-groove, 0.015 in tip, 30/45/60/90 deg", flutes=2, cc=True, dia=6.35,
       mat="any", label="depending on material being cut (page lists HDU, hardwood, softwood, MDF, plastic)",
       derived=[{"quantity": "plunge_mm_min_range", "value": [ipm(40), ipm(250)], "formula": "IPM x 25.4"}],
       notes="One range for all materials (shared column); diameter_mm is the 1/4 in shank."),
    st("precisebits_vtip_scoreengrave", "product page, feeds block", "plunge_feed", "v_bit",
       "Plunge rate = 5 IPM to 25 IPM (again depending on material being cut)", "b", "5 IPM to 25 IPM", "in/min",
       [ipm(5), ipm(25)], sub="EM2E8 micro-engraving, 0.005 in tip", flutes=2, cc=True, dia=3.175,
       mat="any", label="depending on material being cut (page lists softwood, MDF, PCB, plastics)",
       derived=[{"quantity": "plunge_mm_min_range", "value": [ipm(5), ipm(25)], "formula": "IPM x 25.4"}],
       notes="Micro V-tip. diameter_mm is the 1/8 in shank."),
    st("precisebits_vtip_scoreengrave", "product page, description", "entry_general", "v_bit",
       "The plunge style tip geometry insures burr-free material removal at all plunge depths and cutting widths.",
       "c", "plunge style tip geometry", None, None, sub="EM2E8 micro-engraving", flutes=2, cc=True,
       notes="The vendor designs this V-tip to plunge; no statement that a V-bit must not plunge."),
    st("precisebits_tapered_ball_2f", "product page, feeds block", "plunge_feed", "tapered_ball_nose",
       "Plunge rate = depends on speed (RPM)", "c", "depends on speed (RPM)", None, None, flutes=2,
       notes="No number printed. The 3/4-flute tapered pages (G1 copies) print the same sentence."),
    st("precisebits_point_styles", "tip-style table, Ball end", "entry_general", "ball_nose",
       "Two opposing flutes meet slightly offset, creating a center cutting plunge point.", "c",
       "center cutting plunge point", None, None, cc=True,
       notes="Ball-end tools are centre cutting and made to plunge."),
    st("precisebits_point_styles", "tip-style table, radical fish-tail", "entry_general", "flat_end_mill",
       "The radical fish-tail point style has a minimum 20°angle that provides effective plunge cuts", "c",
       "minimum 20°angle", "deg", 20.0, sub="radical fish-tail tip", cc=True,
       notes="The 20 deg is the tip (end-gash) angle, not a ramp angle. Recorded to prevent a misreading."),
    # Carbide 3D / Carbide Create
    st("carbide3d_community_ramping_cc_pro", "post 1, robgrz (staff), 2022-09-29", "ramp_feed_fraction", "any",
       "If ramping is enabled, the feedrate is 1/3 of the feerate, or 100% of the plunge rate, whichever is higher.",
       "c", "1/3 of the feerate, or 100% of the plunge rate, whichever is higher", "fraction of feed", 1 / 3,
       derived=[{"quantity": "ramp_feed", "value": "max(F/3, F_plunge)",
                 "formula": "ramp_feed = max(feed_rate / 3, plunge_rate), as printed"}],
       notes="Carbide Create Pro default (CAM behaviour). Typo 'feerate' is in the post."),
    st("carbide3d_community_ramping_cc_pro", "post 3, robgrz (staff), 2022-09-29", "entry_general", "any",
       "Winston and I always prefer a helical plunge rather than a ramp when pocketing so we went with that", "c",
       "helical plunge rather than a ramp when pocketing", None, None,
       notes="CC Pro pockets spiral (helix) when far enough from the edge, else zig-zag ramp."),
    st("carbide3d_community_ramp_entry_angle", "post 1, user machinemoney, 2024-03-27", "ramp_angle_recommended", "any",
       "I notice the default is 20 degrees", "c", "20 degrees", "deg", 20.0,
       notes="A user reports the Carbide Create ramp entry default. Not a vendor document; no staff confirmation in the thread."),
    st("carbide3d_community_ramp_entry_angle", "post 2, user Tod1d (not staff), 2024-03-27", "no_ramp", "any",
       "If it’s a solid center, side cutting tool, you don’t want to ramp or plunge at all.", "c",
       "don't want to ramp or plunge at all", None, None, cc=False,
       notes="Forum regular, not staff. Same post: 'If it’s a center cutting tool, you can ramp at any angle.'"),
    # Sienci prose
    st("sienci_lm_feeds_and_speeds", "Feeds & Speeds page", "plunge_feed", "flat_end_mill",
       "we usually recommended lower plunge rates (100mm/min to 300mm/min) for most materials", "c",
       "100mm/min to 300mm/min", "mm/min", [100, 300], flutes=2, dia=3.175, mat="any", label="most materials",
       notes="Tested tool: 2-flute 1/8 in carbide end mill. The same vendor's PDF chart prints much higher plunge values "
             "(about half the feed); the page and the chart disagree."),
    # Vectric
    st("vectric_v12_profile_toolpath", "Ramp section", "ramp_feed", "any",
       "All ramp moves are performed at the plunge rate selected for the current tool.", "c",
       "at the plunge rate", None, None,
       derived=[{"quantity": "ramp_feed", "value": "F_plunge", "formula": "ramp_feed = plunge_rate, as printed"}],
       notes="CAM behaviour (VCarve / Aspire)."),
    st("vectric_v12_profile_toolpath", "Ramp section, Zig Zag", "entry_general", "any",
       "The Angle option is typically used for cutters that cannot plunge vertically but have an entry angle specified by the manufacturer.",
       "c", "entry angle specified by the manufacturer", None, None, cc=False),
    st("vectric_tool_database_v11", "Tool Database, Plunge Rate", "ramp_feed", "any",
       "The cutting rate at which the cutter is moved vertically into the material or during ramping moves.", "c",
       "plunge rate used during ramping moves", None, None,
       derived=[{"quantity": "ramp_feed", "value": "F_plunge", "formula": "ramp_feed = plunge_rate, as printed"}]),
    # Fusion
    st("fusion_adaptive_roughing_reference", "Linking, Helical Ramp Diameter", "helix_diameter_max_frac", "any",
       "If the value is bigger than the diameter of the tool it can leave a boss standing in the center of the helix.",
       "c", "bigger than the diameter of the tool", "x D", 1.0,
       derived=[{"quantity": "helix_path_diameter_max", "value": "1.0 x D",
                 "formula": "helix path diameter <= tool diameter (no boss), read from the printed sentence"}],
       notes="Fusion's Helical Ramp Diameter is the helix path diameter. In rs_cam terms helix radius <= 0.5 x D."),
    st("fusion_adaptive_roughing_reference", "Linking, Helical Ramp Diameter, figure captions", "helix_diameter_max_frac", "any",
       "Value of 1.8 x the Dia | | Value of 0.8 x the Dia", "c", "1.8 x the Dia / 0.8 x the Dia", "x D", 0.8,
       derived=[{"quantity": "helix_radius_frac", "value": 0.4, "formula": "0.8 x D / 2"}],
       notes="Two figure captions: 1.8 x Dia is the bad case (boss), 0.8 x Dia the good case. Not stated as a default."),
    st("fusion_adaptive_roughing_reference", "Linking, Minimum Ramp Diameter", "helix_diameter_min_frac", "any",
       "Smaller diameters can reduce the chip evacuation, create jerking machine motion and can cause tool breakage.", "c",
       "no number", None, None, notes="No minimum value printed."),
    st("fusion_adaptive_roughing_reference", "Feed & Speed", "ramp_feed", "any",
       "Ramp Feedrate - Feed used when doing helical ramps into stock", "c", "separate field", None, None,
       notes="Fusion keeps ramp feed separate from plunge feed ('Plunge Feedrate - Feed used when plunging into stock'). No default printed."),
    # CNCCookbook (literature substitute)
    st("cnccookbook_helical_ramp_angle", "section 'Why not plunge?'", "plunge_feed_fraction", "flat_end_mill",
       "A good rule of thumb is to divide the normal feedrate if slotting to the same depth by the number of flutes.",
       "c", "slotting feedrate / number of flutes", "fraction of slot feed", None, mat="any",
       derived=[{"quantity": "plunge_feed", "value": "F_slot / Z", "formula": "plunge = slot feed / flutes; 2F -> 0.5, 3F -> 0.333"}],
       notes="General / metal context."),
    st("cnccookbook_helical_ramp_angle", "section 'Why not plunge?'", "no_plunge", "flat_end_mill",
       "you need a center cutting endmill", "c", "center cutting endmill required", None, None, cc=False),
    st("cnccookbook_helical_ramp_angle", "ramping section", "ramp_angle_recommended", "flat_end_mill",
       "I see recommendations like 1.5 degrees or 2.5 degrees all the time.", "c", "1.5 degrees or 2.5 degrees", "deg",
       [1.5, 2.5], mat="other", label="metal (general)"),
    st("cnccookbook_helical_ramp_angle", "ramping section", "ramp_angle_max", "flat_end_mill",
       "OSG talks about 10 to 20 degrees in tough materials for some of their endmills.", "c", "10 to 20 degrees", "deg",
       [10, 20], mat="other", label="tough materials (metal)", notes="Second-hand citation of OSG; OSG document not fetched."),
    st("cnccookbook_helical_ramp_angle", "ramping section", "entry_general", "flat_end_mill",
       "A 4 flute non-centercutting endmill will have a limit as will an indexable tool.", "c", "limit", None, None,
       flutes=4, cc=False, notes="Also: 'If the tool can plunge a hole, there won't be such a limitation.'"),
    # ShopBot
    st("shopbot_user_guide_2015", "p. ~40, Suggested Feed Rate Starting Point", "plunge_feed", "any",
       "A good starting Move Speed for your Z plunge will be about .5\"/sec.", "c", "about .5\"/sec", "in/s",
       round(0.5 * IN2MM * 60, 1), mat="wood", label="most woods",
       derived=[{"quantity": "plunge_mm_min", "value": 762.0, "formula": "0.5 in/s x 25.4 x 60"},
                {"quantity": "plunge_feed_fraction", "value": round(0.5 / 1.7, 3),
                 "formula": "0.5 in/s / 1.7 in/s (the XY start speed 'For working in most woods' on the same page)"}],
       notes="Machine setup default, no tool."),
]


def sienci_rows(text):
    """Parse the two wood pages of the Sienci metric chart."""
    rows = []
    page = None
    em_count = 0
    row_re = re.compile(
        r"(?P<name>(?:\d+, )?(?:\d+, )?\d+ Degree V-bit - [A-Za-z/ -]+?|1/[48]\" - D[0-9/.m\"]+ Tapered Ball End Mill - (?:Coarse|Fine)"
        r"|[0-9/]+\" [A-Za-z ]*?(?:Ball End Mill(?: - Finishing)?|Flat End Mill (?:UC|DC)|Corncob End Mill|Round Groove Bit)"
        r"|22mm Surfacing Bit)\s+(?P<feed>\d+)\s+(?P<plunge>\d+)\s+(?P<so>[\d.]+)\s+(?P<sds>N/A|[\d.]+)\s+(?P<sdp>[\d.]+)\s+(?P<rpm>\d+)\s+(?P<dial>\d)")
    for ln in text.splitlines():
        if "Softwood, Soft Plywood, MDF - Metric Units" in ln:
            page, em_count = "soft", 0
            continue
        if "Hardwood, Hard Plywood - Metric Units" in ln:
            page, em_count = "hard", 0
            continue
        if "Plastics (" in ln:
            page = None
        if page is None:
            continue
        if ln.strip() == "End Mills":
            em_count += 1
            continue
        m = row_re.search(ln)
        if not m:
            continue
        tier = ["Carving/Detailing - Regular Speed", "Reduced Speed - Finishing, or mild cutting",
                "Regular Speed", "Full Speed - Roughing or aggressive cutting"][em_count]
        rows.append((page, tier, m))
    return rows


def sienci_family(name):
    if "V-bit" in name:
        return "v_bit", name, None, None
    if "Tapered Ball" in name:
        tip = 1.5875 if "D1/16" in name else 0.5
        return "tapered_ball_nose", name, None, tip
    frac = re.match(r"([0-9/]+)\"", name)
    d = None
    if frac:
        n, dd = frac.group(1).split("/") if "/" in frac.group(1) else (frac.group(1), "1")
        d = round(float(n) / float(dd) * IN2MM, 3)
    if "Surfacing" in name:
        return "flat_end_mill", "surfacing bit", None, 22.0
    if "Round Groove" in name:
        return "ball_nose", "round groove (core box) bit", None, d
    if "Ball End Mill" in name:
        return "ball_nose", name, None, d
    if "Corncob" in name:
        return "flat_end_mill", "corncob (chip-breaker) end mill", None, d
    fl = 1 if "Single Flute" in name else None
    sub = ("upcut" if name.endswith("UC") else "downcut") + (", single flute" if fl else "")
    return "flat_end_mill", sub, fl, d


def build_sienci(text):
    out = []
    for i, (page, tier, m) in enumerate(sienci_rows(text)):
        name = m.group("name").strip()
        feed, plunge = int(m.group("feed")), int(m.group("plunge"))
        fam, sub, fl, d = sienci_family(name)
        mat = "wood" if page == "soft" else "hardwood"
        label = "Softwood, Soft Plywood, MDF" if page == "soft" else "Hardwood, Hard Plywood"
        verb = f"{name} {feed} {plunge} {m.group('so')}"
        out.append(st("sienci_feeds_speeds_metric", f"p.{1 if page == 'soft' else 2}, {tier}", "plunge_feed", fam, verb, "a",
                      f"{plunge}", "mm/min", float(plunge), sub=f"{name} ({sub})" if sub != name else name, flutes=fl,
                      cc=True if fam in ("v_bit", "tapered_ball_nose", "ball_nose") else None, dia=d, mat=mat, label=label,
                      derived=[{"quantity": "plunge_feed_fraction", "value": round(plunge / feed, 3),
                                "formula": f"plunge / feed = {plunge} / {feed}"}],
                      notes=f"Row: feed {feed} mm/min, plunge {plunge} mm/min, RPM {m.group('rpm')}. Section: {tier}."))
    return out


def build_carbide(sid, text, dia, tools, has_dial):
    out = []
    in_wood = False
    for ln in text.splitlines():
        if ln.strip() == "WOOD":
            in_wood = True
            continue
        if ln.strip() == "OTHER":
            in_wood = False
        if not in_wood:
            continue
        if has_dial:
            m = re.match(r"(\w+)\s+(\.\d+″)\s+(\d+)\s+([\d.]+)\s+([\d.]+)\s+(\d+)\s+(\d+)\s*$", ln)
        else:
            m = re.match(r"(\w+)\s+(\.\d+″)\s+(\d+)\s+(\d+)\s+(\d+)\s*$", ln)
        if not m:
            continue
        g = m.groups()
        name, doc, rpm = g[0], g[1], g[2]
        feed, plunge = int(g[-2]), int(g[-1])
        mat = {"Pine": "softwood", "Plywood": "plywood", "MDF": "mdf", "Mahogany": "hardwood"}.get(name, "other")
        verb = norm(ln)
        out.append(st(sid, "p.1, WOOD", "plunge_feed", "flat_end_mill", verb, "b", f"{plunge}", "in/min", ipm(plunge),
                      sub=tools, dia=dia, mat=mat, label=name,
                      derived=[{"quantity": "plunge_mm_min", "value": ipm(plunge), "formula": f"{plunge} in/min x 25.4"},
                               {"quantity": "plunge_feed_fraction", "value": round(plunge / feed, 3),
                                "formula": f"PLUNGE / FEED = {plunge} / {feed}"}],
                      notes=f"One row shared by the square and the ball cutter (grade b). DOC {doc}, RPM {rpm}, FEED {feed} ipm."))
    return out


def main():
    sources = build_sources()
    texts = {}
    for s in sources:
        if s["stored_text"]:
            texts[s["source_id"]] = open(os.path.join(G10, s["stored_text"]), encoding="utf-8").read()
    stmts = list(MANUAL)
    stmts += build_sienci(texts["sienci_feeds_speeds_metric"])
    stmts += build_carbide("carbide3d_s3_feeds_250", texts["carbide3d_s3_feeds_250"], 6.35,
                           "#201 .25 in 3F square / #202 .25 in 2F ball (Shapeoko 3)", True)
    stmts += build_carbide("carbide3d_nomad883_feeds_125", texts["carbide3d_nomad883_feeds_125"], 3.175,
                           "#101 / #102 .125 in square / ball (Nomad 883)", False)
    bad = 0
    for i, s in enumerate(stmts):
        s["statement_id"] = f"g10-hobby-{i + 1:03d}"
        if norm(s["verbatim"]) not in norm(texts[s["source_id"]]):
            print("verbatim missing:", s["statement_id"], s["verbatim"][:80])
            bad += 1
    ordered = [{"statement_id": s["statement_id"], **{k: v for k, v in s.items() if k != "statement_id"}} for s in stmts]
    json.dump(sources, open(os.path.join(G10, "parts", "hobby_sources.json"), "w"), indent=1, ensure_ascii=False)
    json.dump(ordered, open(os.path.join(G10, "parts", "hobby_statements.json"), "w"), indent=1, ensure_ascii=False)
    print(f"{len(sources)} sources, {len(stmts)} statements, {bad} verbatim failures")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
