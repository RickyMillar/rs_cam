#!/usr/bin/env python3
"""G8 fetch builder: write sources.json, candidate_rows.json, derate_factors.json
and micro_rules.json for gap group G8 (long tool and small tool loads).

The script hashes every downloaded file, and it checks that each "verbatim"
string occurs in its stored text (whitespace runs collapsed to one space).
It stops with an error if a verbatim string is missing. It does not change
any number that a document prints. Values marked "derived" are computed here
or read across frames; the entry says so.

Usage: python3 g8_build.py
"""
import hashlib
import json
import os
import re
import sys

G = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "fetch", "G8")
G = os.path.normpath(G)
ACCESSED = "2026-09-24"
IN = 25.4


def sha(path):
    h = hashlib.sha256()
    with open(os.path.join(G, path), "rb") as f:
        h.update(f.read())
    return h.hexdigest()


def norm(s):
    return re.sub(r"\s+", " ", s).strip()


_text_cache = {}


def stored(source_id):
    if source_id not in _text_cache:
        src = SOURCES[source_id]
        t = open(os.path.join(G, src["stored_text"]), encoding="utf-8", errors="replace").read()
        _text_cache[source_id] = norm(t)
    return _text_cache[source_id]


def check(source_id, verbatim):
    if norm(verbatim) not in stored(source_id):
        sys.exit(f"verbatim not found in {source_id}: {verbatim!r}")


# ---------------------------------------------------------------------------
# Sources. "hash_of" names the file the sha256 is taken from: the downloaded
# PDF for a PDF, the raw download for an XML article, the stored text for an
# HTML page (as the task asks).
# ---------------------------------------------------------------------------
S = [
    dict(
        source_id="harvey_undercutting_sf23200",
        source_vendor="harvey",
        source_title="Harvey Tool Speeds & Feeds, Undercutting End Mills - 270 deg (SF_23200), with the Undercutting Guide",
        source_url="https://harveyperformance.widen.net/content/t3xctfqkjz/pdf/SF_23200.pdf?u=d9orjt",
        stored_text="sources/harvey_undercutting_sf23200.txt",
        hash_of="pdf/harvey_SF_23200.pdf",
        coverage_notes="Metal cutting only (aluminium to high-temperature alloys). Page 1 prints Table 1, a chip-load multiplier by neck length multiple (neck length / neck diameter): 3x 120 %, 5x 100 %, 8x 80 %, 12x 65 %, 15x 55 %. The base (100 %) is 5x, not 1x. Tool: lollipop undercutting end mill, 0.062-1.000 in. No wood, no router bit. Re-downloaded 2026-09-24; the hash matches the file a previous run left.",
    ),
    dict(
        source_id="harvey_miniature_long_reach_sf846100",
        source_vendor="harvey",
        source_title="Harvey Tool Speeds & Feeds, Miniature End Mills - Square - Long Reach, Standard Flute (SF_846100)",
        source_url="https://harveyperformance.widen.net/content/9gprk0fq3c/original/SF_846100.pdf?u=d9orjt&download=true",
        stored_text="sources/harvey_miniature_long_reach_sf846100.txt",
        hash_of="pdf/harvey_SF_846100.pdf",
        coverage_notes="Metal only, 0.015-0.500 in. A long-reach chart that prints NO reach or L/D factor. It prints Slotting / Roughing / Finishing chip loads with a radial and axial depth per row (for example slotting axial .07 x Dia for 0.015-0.047 in, .16 x Dia above). Evidence that Harvey handles reach in this series through the depth of cut, not a chip-load factor. Not a wood chart. 0 candidate rows.",
    ),
    dict(
        source_id="helical_machining_guidebook_2016",
        source_vendor="helical",
        source_title="Helical Solutions Machining Guidebook (2016)",
        source_url="https://web.mae.ufl.edu/designlab/Advanced%20Manufacturing/Helical_Machining_Guidebook.pdf",
        stored_text="sources/helical_machining_guidebook_2016.txt",
        hash_of="pdf/helical_guidebook_2016.pdf",
        coverage_notes="Same URL as the LUT manifest entry helical_machining_guidebook_2016 (the manifest has no pdf_sha256; the hash here is the first one recorded). Page 68: 'Tool overhang length decreases rigidity as a third power (L3), but even more importantly, tool diameter increases rigidity by the fourth power (D4).' Page 7: necked tooling 'when reaching >3x dia. depths'. Troubleshooting table: 'Tool Overhang ... reduce overhang from tool holder'. No numeric feed de-rate for overhang. Metal cutting.",
    ),
    dict(
        source_id="onsrud_cnc_production_routing_guide",
        source_vendor="onsrud",
        source_title="LMT Onsrud CNC Production Routing Guide",
        source_url="https://precisionboard.com/wp-content/uploads/2017/08/CNC-Prod-Routing-Guide-05.pdf",
        stored_text="sources/onsrud_cnc_production_routing_guide.txt",
        hash_of="pdf/onsrud_cnc_prod_routing_guide.pdf",
        coverage_notes="Router bits, wood and plastics. The vendor URL http://www.onsrud.com/files/pdf/LMT-Onsrud-CNC-Prod-Routing-Guide.pdf returned an HTML page on 2026-09-24; the precisionboard.com mirror returned a PDF whose hash matches the file a previous run left. Prints: cutting edge length 'not more than three times the diameter in a perfect world'; the collet '80% rule'; 'Use shortest CEL to achieve depth of cut'. Prints NO stickout or overhang feed de-rate.",
    ),
    dict(
        source_id="niagara_hp_endmills_speeds_feeds",
        source_vendor="niagara_cutter",
        source_title="Niagara Cutter Speeds & Feeds, Solid Carbide High Performance End Mills (c 2016)",
        source_url="https://community.carbide3d.com/uploads/default/original/2X/0/0415837bc64b13a4420124800fcf8d2ea07e548e.pdf",
        stored_text="sources/niagara_hp_endmills_speeds_feeds.txt",
        hash_of="pdf/c3d_hp_endmills.pdf",
        coverage_notes="Metal cutting chart; footer '(c) 2016 Niagara Cutter, LLC'. Hosted as a forum upload on community.carbide3d.com (not a vendor URL), so treat the host as a mirror. Prints 'For Long and Extra Carbide Reduce Feed by 50%.' with no L/D definition of 'Long' or 'Extra'. The niagaracutter.com speed/feed pages were not fetched as PDF.",
    ),
    dict(
        source_id="redline_gp_carbide_endmills_p216",
        source_vendor="redline",
        source_title="RedLine Tools General Purpose Carbide Endmills Speeds & Feeds (catalog p216)",
        source_url="https://www.redlinetools.com/customer/docs/skudocs/General-Purpose-Carbide-Endmills-Speeds-Feeds-p216.pdf",
        stored_text="sources/redline_gp_carbide_endmills_p216.txt",
        hash_of="pdf/redline_p216.pdf",
        coverage_notes="Metal cutting. Prints '(4) For extra long endmills, reduce SFM by 25%.' (a speed, not a feed, factor) and 'Use the shortest LOC (Length of Cut) available'. Hash matches the file a previous run left.",
    ),
    dict(
        source_id="redline_endmill_tech_info",
        source_vendor="redline",
        source_title="RedLine Tools Endmills - Technical Information",
        source_url="https://www.redlinetools.com/customer/docs/redlinetoolsendmilltechinfo.pdf",
        stored_text="sources/redline_endmill_tech_info.txt",
        hash_of="pdf/redline_endmill_tech_info.pdf",
        coverage_notes="Metal cutting, 12 pages. Prints three different long-tool rules in one document: 'Reduce feed rates by 20% when using long length tools.' (under several HP/XHP tables), '(4) The use of Long and Extra Long Endmills require a reduction in feed by up to 50%.', and on page 5 (XHP 5-flute) 'Milling Long Reach or with Long Overhang / Reduce speed and chipload by 10%'. The page-5 layout text is garbled; sources/redline_endmill_tech_info_p5_raw.txt is the same page from 'pdftotext -raw' and carries the readable text. No L/D threshold is printed for 'long'.",
    ),
    dict(
        source_id="fullerton_endmill_speeds",
        source_vendor="fullerton",
        source_title="Fullerton Tool End Mill Speed and Feed Recommendations",
        source_url="https://cuttingtoolsales.com/wp-content/uploads/2015/11/FullertonTool_EndMill_Speeds.pdf",
        stored_text="sources/fullerton_endmill_speeds.txt",
        hash_of="pdf/fullerton_endmill_speeds.pdf",
        coverage_notes="Metal cutting; distributor-hosted copy. Prints 'Extra Long End Mills / For extra long end mills the SFM should be reduced by 25%.' (a speed factor) and the shortest-LOC / shortest-gage-line advice.",
    ),
    dict(
        source_id="amana_zrn_3d_profiling_v8",
        source_vendor="amana",
        source_title="Amana ZrN-Coated and Uncoated 2D/3D Carving CNC Solid Carbide Router Bits v8",
        source_url="https://www.amanatool.com/pub/media/productattachments/ZrN-3D-Profiling-Feed-Chip-Load-Chart-v8.pdf",
        stored_text="sources/amana_zrn_3d_profiling_v8.txt",
        hash_of="pdf/amana_zrn_3d_profiling_v8.pdf",
        coverage_notes="Wood router bits. The LUT manifest already lists this source (amana_zrn_3d_profiling_v8, no pdf_sha256, no stored text); the hash here is the first one recorded. Page 2 prints a '3 Flute Extra Long Ball Nose & Flat Bottom' block at 1/4, 3/8 and 1/2 in (tools 46490, 46491, 46493, 46496, 46590, 46593, 46596, 46597) that the LUT does not hold. The same chart prints the standard-length 3 Flute Ball Nose rows (LUT rows amana-zrn-ball-softwood-parallel-9525-3f and -12700-3f; the LUT copies match the chart). The older unversioned chart (LUT source amana_zrn_3d_profiling, sha 52465d1d..., same hash on 2026-09-24) prints the 1/4 in extra-long wood cell as '0.0004\" - 0.006\"'; v8 prints '0.004\" - 0.006\"', which the IPM column (215-320 IPM at 18,000 rpm, 3 flutes) supports. Depth statement: 1 x D; 2 x D reduce feed 25 %; 3 x D reduce 50 %.",
    ),
    dict(
        source_id="amana_spektra_3d_profiling_v6",
        source_vendor="amana",
        source_title="Amana Spektra Extreme Tool Life Coated 2D/3D Carving CNC Solid Carbide Router Bits, Feed and Chip Load Chart v6",
        source_url="https://www.amanatool.com/pub/media/productattachments/Spektra-Coated-3D-Profiling-Feed-Chip-Load-Chart-v6.pdf",
        stored_text="sources/amana_spektra_3d_profiling_v6.txt",
        hash_of="pdf/amana_spektra_3d_profiling_v6.pdf",
        coverage_notes="Wood router bits, Spektra coating. Page 2 prints '3 Flute Extra Long Ball Nose & Flat Bottom' 1/4 in (46490-K) for 'Wood, MDF, Sign-Foam': 0.004-0.006 in/tooth. Not in the LUT manifest.",
    ),
    dict(
        source_id="toolstoday_amana_4649x_specs",
        source_vendor="toolstoday (Amana distributor)",
        source_title="ToolsToday product pages for Amana 46490, 46491, 46496 (extra long) and 46494, 46495 (standard)",
        source_url="https://www.toolstoday.com/v-12487-46491.html",
        stored_text="sources/toolstoday_amana_4649x_specs.txt",
        hash_of="sources/toolstoday_amana_4649x_specs.txt",
        coverage_notes="Catalog geometry, not a chipload source. amanatool.com product pages returned a bot challenge ('Just a moment...'), so the geometry comes from the distributor pages (five URLs, each named in the stored text). The stored text keeps only the title line and the dimension table of each page. Extra-long tools carry a relieved neck D1 and reach L1: 46490 1/4 D, 1/2 CH, D1 .235, L1 1-1/4, OAL 4; 46491 3/8 D, 3/4 CH, D1 .352, L1 1-1/4, OAL 4; 46496 1/2 D, 1-1/4 CH, D1 .470, L1 2, OAL 7. Standard tools: 46494 3/8 D, 2-1/4 CH, OAL 4; 46495 1/2 D, 2-1/4 CH, OAL 4. 46493 (1/2 D, 1 CH, 6 in long, from search titles) was not stored.",
    ),
    dict(
        source_id="precisebits_calibrating_feeds_speeds",
        source_vendor="precisebits",
        source_title="PreciseBits tutorial: Calibrating Feeds and Speeds When Using Carbide Microtools",
        source_url="https://www.precisebits.com/tutorials/calibrating_feeds_n_speeds.htm",
        stored_text="sources/precisebits_calibrating_feeds_speeds.txt",
        hash_of="sources/precisebits_calibrating_feeds_speeds.txt",
        coverage_notes="Micro tools in wood and plastics. A test procedure, not a chart: start feed 'F = 0.03 x D x No. flutes x RPM (3% chipload per flute)' for soft, hard and extreme hardwoods; plunge depth Z = 2 x D (Janka < 1,000), 1 x D (1,000-2,500), 0.5 x D (2,500-5,000); sweet spot = 0.75 x Fmax (the feed where the cut fails). Grade c at best for a number: it is a start point for a test, and the page says so.",
    ),
    dict(
        source_id="precisebits_faq",
        source_vendor="precisebits",
        source_title="PreciseBits FAQ on CNCs, Materials, and Tooling",
        source_url="https://www.precisebits.com/faq.htm",
        stored_text="sources/precisebits_faq.txt",
        hash_of="sources/precisebits_faq.txt",
        coverage_notes="Micro tools. 'Spindle Runout (TIR) - should be less than or equal to 2% of the tool's diameter (absolute maximum).' The 0.80 mm floor applies to a hand-held pencil grinder mounted on a CNC, not to every spindle.",
    ),
    dict(
        source_id="harvey_blog_optimize_miniature_end_mills",
        source_vendor="harvey",
        source_title="Harvey Performance In The Loupe: How to Optimize Miniature End Mill Performance (2020-07-01)",
        source_url="https://www.harveyperformance.com/in-the-loupe/how-to-optimize-results-while-machining-with-miniature-end-mills/",
        stored_text="sources/harvey_blog_optimize_miniature_end_mills.txt",
        hash_of="sources/harvey_blog_optimize_miniature_end_mills.txt",
        coverage_notes="Vendor blog (grade c). Runout 'should not exceed 2% of the tool diameter'; chip thickness vs edge radius (ploughing, no number); 'doubling the length sticking out of the holder will result in 8 times more deflection. Doubling the diameter ... 16 times less deflection.'",
    ),
    dict(
        source_id="harvey_blog_running_parameters_miniature",
        source_vendor="harvey",
        source_title="Harvey Performance In The Loupe: How to Adjust Running Parameters for Miniature Tooling",
        source_url="https://www.harveyperformance.com/in-the-loupe/running-parameters-for-miniature-tooling/",
        stored_text="sources/harvey_blog_running_parameters_miniature.txt",
        hash_of="sources/harvey_blog_running_parameters_miniature.txt",
        coverage_notes="Vendor blog (grade c). 'Runout should be measured at less than .0001\"'; 'ensure the tool is the largest diameter and shortest length of cut possible'. No numeric chip-load rule for small tools.",
    ),
    dict(
        source_id="micromachines_2020_hmin_effective_rake",
        source_vendor="literature (Micromachines 11(10):924, doi 10.3390/mi11100924, PMC7600950)",
        source_title="Experimental Study on the Minimum Undeformed Chip Thickness Based on Effective Rake Angle in Micro Milling",
        source_url="https://www.ebi.ac.uk/europepmc/webservices/rest/PMC7600950/fullTextXML",
        stored_text="sources/micromachines_2020_hmin_effective_rake.txt",
        hash_of="raw/pmc7600950.xml",
        coverage_notes="Micro milling of copper with a 1 mm 2-flute TiCN-coated carbide end mill; measured edge radius rn about 4.4 um; result hmin = 0.17 rn. The introduction cites other metals: 0.43-0.48 rn (KDP crystal), 0.15-0.49 rn (titanium alloy), about 0.3 rn (Al 6061), 14-21 % of rn (Al 6082), and 0.1-0.3 rn (copper). No wood.",
    ),
    dict(
        source_id="materials_2019_mdf_drill_edge_radius",
        source_vendor="literature (Materials 12(3):386, 2019, PMC6384622)",
        source_title="Experimental Study on Drilling MDF with Tools Coated with TiAlN and ZrN",
        source_url="https://www.ebi.ac.uk/europepmc/webservices/rest/PMC6384622/fullTextXML",
        stored_text="sources/materials_2019_mdf_drill_edge_radius.txt",
        hash_of="raw/PMC6384622.xml",
        coverage_notes="HW (cemented carbide) drills in MDF, coated and uncoated: cutting edge radius 5.91-5.95 um (selected to be equal). The only measured edge radius of a carbide tool used in a wood composite that I found.",
    ),
    dict(
        source_id="materials_2021_spruce_saw_edge_radius",
        source_vendor="literature (Materials 14(20), 2021, PMC8539808)",
        source_title="Interaction between Thermal Modification Temperature of Spruce Wood and the Cutting and Fracture Parameters",
        source_url="https://www.ebi.ac.uk/europepmc/webservices/rest/PMC8539808/fullTextXML",
        stored_text="sources/materials_2021_spruce_saw_edge_radius.txt",
        hash_of="raw/PMC8539808.xml",
        coverage_notes="Carbide-tipped circular sawblade for spruce: cutting-edge radius rho0 = 8 um.",
    ),
    dict(
        source_id="materials_2020_hss_planer_knife_edge_radius",
        source_vendor="literature (Materials 13(11), 2020, PMC7288033)",
        source_title="Experimental Studies on Durability of PVD-Based CrCN/CrN-Coated Cutting Blade of Planer Knives Used in the Pine Wood Planing Process",
        source_url="https://www.ebi.ac.uk/europepmc/webservices/rest/PMC7288033/fullTextXML",
        stored_text="sources/materials_2020_hss_planer_knife_edge_radius.txt",
        hash_of="raw/PMC7288033.xml",
        coverage_notes="HSS planer knives (not carbide) in pine: edge radius after sharpening 2.08 um (sigma 1.20) and 2.17 um; at end of life 13.42 um and 9.75 um. Shows the edge radius grows several times with wear.",
    ),
    dict(
        source_id="materials_2025_wpc_pcd_edge_radius",
        source_vendor="literature (Materials, 2025, PMC12565702)",
        source_title="Investigation of Cutting Forces and Temperature in Face Milling of Wood-Plastic Composite Using Radial Basis Function Neural Network",
        source_url="https://www.ebi.ac.uk/europepmc/webservices/rest/PMC12565702/fullTextXML",
        stored_text="sources/materials_2025_wpc_pcd_edge_radius.txt",
        hash_of="raw/PMC12565702.xml",
        coverage_notes="PCD face-milling cutter, 10 mm, 8 teeth, in wood-plastic composite: 'The average edge radius measured about 10 um.' PCD, not carbide.",
    ),
]

SOURCES = {s["source_id"]: s for s in S}
for s in S:
    s["accessed_on"] = ACCESSED
    s["pdf_sha256"] = sha(s["hash_of"])
    s["hashed_file"] = s.pop("hash_of")

# ---------------------------------------------------------------------------
# Candidate rows: printed wood chiploads for extra-long router bits.
# ---------------------------------------------------------------------------
V8_LINE = 'Wood, MDF, Sign-Foam 215" - 320" 0.004" - 0.006" 270" - 370" 0.005" - 0.007" 320" - 430" 0.006" - 0.008"'
V6_LINE = 'Wood, MDF, Sign-Foam 215" - 320" 0.004" - 0.006"'
GEOM = {
    6.35: "46490 (ZrN) / 46490-K (Spektra): 1/4 D, 1/2 CH, relieved neck D1 .235 in to L1 1-1/4 in, 1/4 shank, 4 in OAL",
    9.525: "46491: 3/8 D, 3/4 CH, relieved neck D1 .352 in to L1 1-1/4 in, 3/8 shank, 4 in OAL (the standard 46494 is also 4 in OAL, with 2-1/4 CH)",
    12.7: "46493: 1/2 D, 1 CH, 6 in OAL (search title only); 46496: 1/2 D, 1-1/4 CH, relieved neck D1 .470 in to L1 2 in, 7 in OAL (the standard 46495 is 4 in OAL, 2-1/4 CH)",
}
STD = {
    6.35: "no standard 3-flute 1/4 in column; the 6 mm standard 3-flute ball column prints 0.004-0.006 in",
    9.525: "standard 3-flute ball 3/8 in prints 0.006-0.008 in (LUT amana-zrn-ball-softwood-parallel-9525-3f)",
    12.7: "standard 3-flute ball 1/2 in prints 0.007-0.009 in (LUT amana-zrn-ball-softwood-parallel-12700-3f)",
}

rows = []


def add_row(src_id, verbatim, d_in, d_mm, lo_in, hi_in, family, mat, page, subfamily, tools):
    check(src_id, verbatim)
    src = SOURCES[src_id]
    fam_op = ("parallel", "finish") if family == "ball_nose" else ("pocket", "roughing")
    oid = f"x-g8-{src_id.replace('_', '-')}-xl-{family.replace('_', '')}-{mat}-{int(round(d_mm * 1000))}-3f"
    rows.append({
        "observation_id": oid,
        "source_id": src_id,
        "source_vendor": src["source_vendor"],
        "source_title": src["source_title"],
        "source_url": src["source_url"],
        "accessed_on": ACCESSED,
        "source_page": page,
        "evidence_grade": "a",
        "row_kind": "exact",
        "tool_family": family,
        "tool_subfamily": subfamily,
        "operation_family": fam_op[0],
        "pass_role": fam_op[1],
        "material_family": mat,
        "material_label": "wood/MDF/sign-foam (chart row); extra-long tool",
        "hardness_kind": "janka",
        "hardness_value": 600.0 if mat == "softwood" else 1100.0,
        "diameter_mm": d_mm,
        "flute_count": 3,
        "rpm_nominal": 18000.0,
        "chipload_min_mm_tooth": round(lo_in * IN, 4),
        "chipload_max_mm_tooth": round(hi_in * IN, 4),
        "ap_rule": "1xD use recommended feed rate; 2xD reduce 25%; 3xD reduce 50%",
        "machine_assumption": f"chart at 18000 rpm, 1xD DOC; CPT range {lo_in}-{hi_in} in = {round(lo_in * IN, 4)}-{round(hi_in * IN, 4)} mm",
        "ap_max_factor": 1.0,
        "notes": (
            f"Printed row for the chart's '3 Flute Extra Long Ball Nose & Flat Bottom' block ({tools}); one printed cell serves both ball nose and flat bottom. "
            f"Tool geometry (derived from toolstoday_amana_4649x_specs, not printed on the chart): {GEOM[d_mm]}. "
            f"Same-chart standard-length comparison (derived, for G8 only): {STD[d_mm]}. "
            "The chart prints no stickout or L/D; 'extra long' is a catalog class with a relieved neck and a short flute, so a ratio against the standard row is not a pure stickout effect. "
            "Material split follows the LUT convention for this chart (softwood and MDF only; the chart does not split hardwood). "
            "hardness_value is the engine's own Janka proxy for the family, as in the LUT rows of this chart."
        ),
        "extrapolation_group": "G8",
        "verbatim": verbatim,
    })


for d_in_s, d_mm, lo, hi in (("1/4", 6.35, 0.004, 0.006), ("3/8", 9.525, 0.005, 0.007), ("1/2", 12.7, 0.006, 0.008)):
    for family in ("ball_nose", "flat_end"):
        for mat in ("softwood", "mdf"):
            add_row(
                "amana_zrn_3d_profiling_v8", V8_LINE, d_in_s, d_mm, lo, hi, family, mat,
                f"page 2, 3 Flute Extra Long Ball Nose & Flat Bottom, {d_in_s}\" column, Wood/MDF/Sign-Foam row",
                "zrn_extra_long", "46490, 46491, 46493, 46496, 46590, 46593, 46596, 46597",
            )
for mat in ("softwood", "mdf"):
    add_row(
        "amana_spektra_3d_profiling_v6", V6_LINE, "1/4", 6.35, 0.004, 0.006, "ball_nose", mat,
        "page 2, 3 Flute Extra Long Ball Nose & Flat Bottom, 1/4\" column, Wood/MDF/Sign-Foam row",
        "spektra_extra_long", "46490-K",
    )

# ---------------------------------------------------------------------------
# De-rate factors and limits on the length axis (not LUT rows).
# ---------------------------------------------------------------------------
F = []


def factor(src_id, verbatim, **kw):
    check(src_id, verbatim)
    e = {"source_id": src_id, "pdf_sha256": SOURCES[src_id]["pdf_sha256"], "verbatim": verbatim}
    e.update(kw)
    F.append(e)


HARVEY_FRAME = (
    "Axis = neck length / NECK diameter of a lollipop undercutting end mill; base 100 % is 5x (3x is a boost to 120 %). "
    "The repo's long_tool_load_share uses stickout / CUTTER diameter with base 1.0 up to 4x. A comparison such as 'Harvey 8x = 80 % vs repo 6x = 0.75' is derived and crosses two frames; it is not a match. Metals only."
)
for mult, pct in ((3, 120), (5, 100), (8, 80), (12, 65), (15, 55)):
    factor(
        "harvey_undercutting_sf23200", f"{mult}x {pct}%",
        kind="chipload_multiplier", axis="neck_length_over_neck_diameter", axis_value=mult,
        factor=pct / 100.0, applies_to="chip load (IPT)", material_scope="metals (aluminium to high-temperature alloys)",
        tool_scope="Harvey undercutting end mills 270 deg, 0.062-1.000 in", evidence_grade="a", row_kind="exact",
        frame_note=HARVEY_FRAME,
    )
factor("harvey_undercutting_sf23200", "Adjust Chip Load to account for neck length to cutter diameter ratio. (see Table 2)",
       kind="method_statement", evidence_grade="a", row_kind="exact",
       frame_note="The step text says 'cutter diameter'; the worked example divides by neck diameter (.500 / .076). The printed table is 'Table 1' (the step says Table 2). Record both as printed.")
factor("harvey_undercutting_sf23200", "Neck Length Ratio = (Neck Length / Neck Diameter)",
       kind="axis_definition", evidence_grade="a", row_kind="exact", frame_note=HARVEY_FRAME)

factor("niagara_hp_endmills_speeds_feeds", "For Long and Extra Carbide Reduce Feed by 50%.",
       kind="feed_multiplier", axis="catalog_length_class", axis_value="Long, Extra (undefined L/D)", factor=0.5,
       applies_to="feed", material_scope="metals", evidence_grade="b", row_kind="exact",
       frame_note="Printed under 'PROFILING CUTTING DEPTH RECOMMENDATIONS AND ADJUSTMENTS'. The chart does not define Long or Extra as an L/D. Host is a forum mirror.")
factor("redline_gp_carbide_endmills_p216", "(4) For extra long endmills, reduce SFM by 25%.",
       kind="speed_multiplier", axis="catalog_length_class", axis_value="extra long (undefined L/D)", factor=0.75,
       applies_to="surface speed (SFM), not feed per tooth", material_scope="metals", evidence_grade="b", row_kind="exact",
       frame_note="A speed factor. At a fixed chip load it lowers the feed rate by the same 25 %, but the chip load per tooth is unchanged.")
factor("redline_endmill_tech_info", "Reduce feed rates by 20% when using long length tools.",
       kind="feed_multiplier", axis="catalog_length_class", axis_value="long length (undefined L/D)", factor=0.8,
       applies_to="feed rate", material_scope="metals", evidence_grade="b", row_kind="exact",
       frame_note="Footnote under several HP/XHP tables in the same document.")
factor("redline_endmill_tech_info", "(4) The use of Long and Extra Long Endmills require a reduction in feed by up to 50%.",
       kind="feed_multiplier_ceiling", axis="catalog_length_class", axis_value="Long, Extra Long (undefined L/D)", factor=0.5,
       applies_to="feed", material_scope="metals", evidence_grade="b", row_kind="exact",
       frame_note="'up to 50%': a ceiling on the reduction, not a value.")
check("redline_endmill_tech_info", "Reduce speed and chipload")
F.append({
    "source_id": "redline_endmill_tech_info", "pdf_sha256": SOURCES["redline_endmill_tech_info"]["pdf_sha256"],
    "verbatim": "Milling Long Reach or with Long Overhang / Reduce speed and chipload / by 10%",
    "verbatim_file": "sources/redline_endmill_tech_info_p5_raw.txt (page 5, pdftotext -raw)",
    "kind": "chipload_and_speed_multiplier", "axis": "catalog_length_class", "axis_value": "long reach or long overhang (undefined L/D)",
    "factor": 0.9, "applies_to": "speed and chip load", "material_scope": "metals (XHP 5-flute)", "evidence_grade": "b", "row_kind": "exact",
    "frame_note": "Page 5 is a diagram page; the layout text is garbled, the raw text is readable. Within one vendor document the long-tool rule varies from 10 % to 'up to 50 %'.",
})
factor("fullerton_endmill_speeds", "For extra long end mills the SFM should be reduced by 25%.",
       kind="speed_multiplier", axis="catalog_length_class", axis_value="extra long (undefined L/D)", factor=0.75,
       applies_to="surface speed (SFM)", material_scope="metals", evidence_grade="b", row_kind="exact",
       frame_note="Same wording family as RedLine p216. A speed factor, not a chip-load factor.")
factor("helical_machining_guidebook_2016", "Tool overhang length decreases rigidity as a third power (L3), but even more importantly, tool diameter increases rigidity by the",
       kind="physics_rule", axis="overhang_and_diameter", evidence_grade="b", row_kind="exact",
       frame_note="Rigidity ~ D^4 / L^3 (the sentence ends 'fourth power (D4).' on the next layout line). This is the cantilever law that feeds::force / tool_load deflection already uses; it is a second witness for the model, not a factor.")
factor("helical_machining_guidebook_2016", "essential to look at necked-down tooling when reaching >3x dia. depths.",
       kind="limit_statement", axis="reach_depth_over_diameter", axis_value=3, evidence_grade="b", row_kind="exact",
       frame_note="A tooling choice threshold at 3x diameter, with no feed factor.")
factor("harvey_blog_optimize_miniature_end_mills", "In theory, doubling the length sticking out of the holder will result in 8 times more deflection. Doubling the diameter of an end mill it will result in 16 times less deflection.",
       kind="physics_rule", axis="stickout_and_diameter", evidence_grade="c", row_kind="exact",
       frame_note="Same L^3 / D^4 law as Helical, from a vendor blog.")
factor("onsrud_cnc_production_routing_guide", "largest diameter possible, but the cutting edge length should be as short as possible and not more than three times the diameter",
       kind="limit_statement", axis="cutting_edge_length_over_diameter", axis_value=3, evidence_grade="b", row_kind="exact",
       frame_note="Router-bit vendor, wood. The sentence continues 'in a perfect world.' A selection rule for flute length, not a stickout de-rate and not a feed factor.")
factor("onsrud_cnc_production_routing_guide", "It is very critical that the 80% rule (at least 80% of the collet filled with tool shank) be followed",
       kind="limit_statement", axis="collet_grip", evidence_grade="b", row_kind="exact",
       frame_note="A holding rule. It bounds how far a short-shank tool can be pulled out for reach.")
factor("onsrud_cnc_production_routing_guide", "Use shortest CEL to achieve depth of cut",
       kind="limit_statement", axis="cutting_edge_length", evidence_grade="b", row_kind="exact",
       frame_note="Troubleshooting remedy for TOOL BREAKAGE / Excessive Cutting Edge Length.")
factor("amana_zrn_3d_profiling_v8", "3 x D Reduce feed rate by 50%",
       kind="doc_rule_not_stickout", axis="axial_depth_over_diameter", axis_value=3, factor=0.5, evidence_grade="a", row_kind="exact",
       frame_note="Depth of cut, not stickout. The LUT already carries it in ap_rule ('2xD reduce 25%; 3xD reduce 50%'). Searches for an Amana or Onsrud stickout de-rate returned only this depth rule.")

# Derived same-chart comparison (Amana v8 extra long vs standard, wood).
DERIVED = [
    {"diameter_in": "3/8", "standard": [0.006, 0.008], "extra_long": [0.005, 0.007]},
    {"diameter_in": "1/2", "standard": [0.007, 0.009], "extra_long": [0.006, 0.008]},
    {"diameter_in": "1/4 (standard read from the 6 mm column)", "standard": [0.004, 0.006], "extra_long": [0.004, 0.006]},
]
for d in DERIVED:
    d["ratio_min"] = round(d["extra_long"][0] / d["standard"][0], 3)
    d["ratio_max"] = round(d["extra_long"][1] / d["standard"][1], 3)
F.append({
    "source_id": "amana_zrn_3d_profiling_v8", "pdf_sha256": SOURCES["amana_zrn_3d_profiling_v8"]["pdf_sha256"],
    "kind": "derived_ratio", "row_kind": "derived", "evidence_grade": "c",
    "verbatim": V8_LINE,
    "comparison": DERIVED,
    "frame_note": (
        "DERIVED. Ratio of the printed extra-long wood cell to the printed standard 3-flute ball wood cell in the same chart. "
        "Ratios of the maxima: 0.875 (3/8), 0.889 (1/2), 1.0 (1/4 against 6 mm). The extra-long tools have a relieved neck and a short flute; "
        "at 3/8 in both tools are 4 in OAL, so the ratio is not a stickout effect. It is the only wood-router evidence found that a vendor prints "
        "a lower chip load for a long-reach tool. It is NOT a source for the repo's 0.88 share (that share acts on the load target, not the chip load)."
    ),
})

# ---------------------------------------------------------------------------
# Micro-tool rules.
# ---------------------------------------------------------------------------
M = []


def micro(src_id, verbatim, **kw):
    check(src_id, verbatim)
    e = {"source_id": src_id, "pdf_sha256": SOURCES[src_id]["pdf_sha256"], "verbatim": verbatim}
    e.update(kw)
    M.append(e)


micro("micromachines_2020_hmin_effective_rake", "The minimum undeformed chip thickness of copper is determined as 0.17 rn based on the effective rake angle curve.",
      quantity="hmin_over_edge_radius", value=0.17, material_scope="copper", evidence_grade="b", row_kind="exact")
micro("micromachines_2020_hmin_effective_rake", "the tool cutting edge radius rn was measured to be about 4.4 μm",
      quantity="edge_radius_um", value=4.4, tool_scope="1 mm 2-flute TiCN-coated ultrafine carbide micro end mill (NS Tool MSE 230)", evidence_grade="b", row_kind="exact")
for vb, q, v, scope in (
    ("Chen et al. [15] reported the minimum undeformed chip thickness to be 0.43–0.48 rn", "hmin_over_edge_radius", [0.43, 0.48], "KDP crystal (cited)"),
    ("Rezaei et al. [16] found that the minimum undeformed chip thickness varies between 0.15 rn and 0.49 rn", "hmin_over_edge_radius", [0.15, 0.49], "titanium alloy (cited)"),
    ("the minimum undeformed chip thickness is about 0.3 rn in micro milling of aluminum alloy 6061", "hmin_over_edge_radius", 0.3, "Al 6061 (cited)"),
    ("which is about 14–21% of tool cutting edge radius", "hmin_over_edge_radius", [0.14, 0.21], "Al 6082-T2 (cited)"),
    ("the minimum undeformed chip thickness of copper material is about he/rn = 0.1–0.3", "hmin_over_edge_radius", [0.1, 0.3], "copper (literature summary)"),
):
    micro("micromachines_2020_hmin_effective_rake", vb, quantity=q, value=v, material_scope=scope, evidence_grade="b", row_kind="exact",
          note="Cited by the paper, not measured in it. Metals and crystals; no wood value exists in this literature.")
micro("materials_2019_mdf_drill_edge_radius", "The variation of the the cutting edge radius εr of drills used varied between 5.91 and 5.95 μm.",
      quantity="edge_radius_um", value=[5.91, 5.95], tool_scope="HW carbide drills (coated and uncoated) used in MDF", evidence_grade="b", row_kind="exact")
micro("materials_2021_spruce_saw_edge_radius", "the cutting-edge radius was ρ0 = 8 μm",
      quantity="edge_radius_um", value=8.0, tool_scope="carbide-tipped circular sawblade, spruce", evidence_grade="b", row_kind="exact")
micro("materials_2020_hss_planer_knife_edge_radius", "rr(av) = 2.08 µm for unmodified knives",
      quantity="edge_radius_um_new", value=2.08, tool_scope="HSS planer knives after sharpening (not carbide)", evidence_grade="b", row_kind="exact")
micro("materials_2020_hss_planer_knife_edge_radius", "unmodified knives (rap(av) = 13.42 µm)",
      quantity="edge_radius_um_worn", value=13.42, tool_scope="HSS planer knives at end of life (not carbide)", evidence_grade="b", row_kind="exact")
micro("materials_2025_wpc_pcd_edge_radius", "The average edge radius measured about 10 μm.",
      quantity="edge_radius_um", value=10.0, tool_scope="PCD face-milling cutter, wood-plastic composite (not carbide)", evidence_grade="b", row_kind="exact")
micro("precisebits_calibrating_feeds_speeds", "Hardwoods like birch, cherry, maple or rosewood: F = 0.03 x D x No. flutes x RPM (3% chipload per flute)",
      quantity="start_chipload_over_diameter", value=0.03, material_scope="softwood, hardwood and extreme hardwood (same 3 % in each line)",
      evidence_grade="c", row_kind="exact", note="A START point for a feed test, 'just below the sweetspot'. Not a recommendation or a limit.")
micro("precisebits_calibrating_feeds_speeds", "Multiply Fmax by 0.75 to get the sweet spot for these cutting conditions.",
      quantity="sweet_spot_over_failure_feed", value=0.75, evidence_grade="c", row_kind="exact")
micro("precisebits_calibrating_feeds_speeds", "Hardwoods like birch, cherry, maple or rosewood (1,000 < Janka < 2,500): Z = 1 x D",
      quantity="test_plunge_depth_over_diameter", value=1.0, material_scope="hardwood; softwood (Janka < 1,000) Z = 2 x D; extreme hardwood Z = 0.5 x D",
      evidence_grade="c", row_kind="exact")
micro("precisebits_faq", "Spindle Runout (TIR) - should be less than or equal to 2% of the tool's diameter (absolute maximum).",
      quantity="runout_over_diameter_max", value=0.02, evidence_grade="b", row_kind="exact")
micro("harvey_blog_optimize_miniature_end_mills", "The runout of an operation should not exceed 2% of the tool diameter.",
      quantity="runout_over_diameter_max", value=0.02, evidence_grade="c", row_kind="exact", note="Second vendor, same 2 % rule.")
micro("harvey_blog_running_parameters_miniature", "Runout should be measured at less than .0001”.",
      quantity="runout_in_max", value=0.0001, evidence_grade="c", row_kind="exact", note="Context: breakage at the shank transition of a miniature tool.")
micro("precisebits_faq", "If you are fitting a shop router to your CNC, we recommend that you use a variable speed plunge-style router.",
      quantity="context", evidence_grade="c", row_kind="exact", note="PreciseBits addresses hobby router spindles as well as CNC spindles.")

M.append({
    "kind": "derived_estimate", "row_kind": "derived", "evidence_grade": "c",
    "quantity": "hmin_um_wood_carbide",
    "value": [round(0.14 * 5.91, 2), round(0.49 * 8.0, 2)],
    "rule": "hmin = (0.14 to 0.49) x rn, with rn = 5.91 to 8 um (carbide drill in MDF, carbide saw tooth in spruce)",
    "note": (
        "DERIVED and cross-material: the hmin/rn ratios are from metals and crystals, the edge radii are from carbide wood tools that are not router bits. "
        "Result about 0.8 to 3.9 um per tooth for a sharp edge; a worn edge (the HSS knives grew about 6x) moves it up by the same factor. "
        "Compare: a 1 mm tool at PreciseBits' 3 % start point takes 30 um per tooth, and 2 % runout on the same tool is 20 um. "
        "On this estimate the edge-radius floor is not the binding micro-tool limit in wood; runout and breakage are. This is an inference, not a printed value."
    ),
})

# ---------------------------------------------------------------------------
out_sources = []
for s in S:
    out_sources.append({k: s[k] for k in (
        "source_id", "source_vendor", "source_title", "source_url", "accessed_on",
        "stored_text", "pdf_sha256", "hashed_file", "coverage_notes")})

with open(os.path.join(G, "sources.json"), "w") as f:
    json.dump(out_sources, f, indent=2, ensure_ascii=False)
    f.write("\n")
with open(os.path.join(G, "candidate_rows.json"), "w") as f:
    json.dump({"observations": rows}, f, indent=2, ensure_ascii=False)
    f.write("\n")
with open(os.path.join(G, "derate_factors.json"), "w") as f:
    json.dump({"extrapolation_group": "G8", "entries": F}, f, indent=2, ensure_ascii=False)
    f.write("\n")
with open(os.path.join(G, "micro_rules.json"), "w") as f:
    json.dump({"extrapolation_group": "G8", "entries": M}, f, indent=2, ensure_ascii=False)
    f.write("\n")

ids = [r["observation_id"] for r in rows]
assert len(ids) == len(set(ids)), "duplicate observation_id"
print(f"sources {len(out_sources)}, candidate rows {len(rows)}, factors {len(F)}, micro {len(M)}")
for s in out_sources:
    n = sum(1 for r in rows if r["source_id"] == s["source_id"])
    print(f"  {s['source_id']}: rows {n}, sha {s['pdf_sha256'][:16]}")
