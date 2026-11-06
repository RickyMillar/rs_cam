"""D2 rigidity research: hobby chip per tooth against industrial chip per tooth.

Reads only stored files in the repo. Every hobby value is parsed from the cited
line of the stored chart text; every industrial value is read from the vendor
LUT observation rows. Writes pairs.csv and stats.json next to this script.

Run: python3 -I planning/feeds_review_2026-11-06/research_d2_rigidity/build_pairs.py
"""

import csv
import json
import math
import re
import statistics
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
G10 = REPO / "planning/extrapolation_2026-09-24/fetch/G10/sources"
G9 = REPO / "planning/extrapolation_2026-09-24/fetch/G9/sources"
LUT = REPO / "crates/rs_cam_core/data/vendor_lut"

INCH_MM = 25.4  # definition of the inch

_CACHE = {}


def rel(p):
    return str(Path(p).relative_to(REPO))


def line(path, n, *tokens):
    """Return line n (1-based) of path; assert that each token is in it."""
    path = Path(path)
    if path not in _CACHE:
        _CACHE[path] = path.read_text(encoding="utf-8").split("\n")
    text = _CACHE[path][n - 1]
    for t in tokens:
        assert t in text, f"{rel(path)}:{n} lacks {t!r}: {text.strip()[:160]}"
    return text


def nums(text):
    """All numbers in a line, commas in thousands removed, inch marks removed."""
    t = text.replace(",", "").replace("″", "").replace('"', " ")
    return [float(x) for x in re.findall(r"(?<![\w/])\.?\d+(?:\.\d+)?(?![\w/])", t)]


# ---------------------------------------------------------------- source class
# Each source gets a class and a cited reason (file:line and the words).
CLASS = {
    "sienci_metric": ("hobby", rel(G10 / "sienci_feeds_speeds_metric.txt") + ":8",
                      "chart column 'Makita Speed Dial Setting' (a trim router); Sienci LongMill page names the Makita RT0701",
                      (G10 / "sienci_feeds_speeds_metric.txt", 8, "Makita Speed")),
    "c3d_s3": ("hobby", rel(G9 / "community_2402_post1_announcement.txt") + ":3",
               "post names the Shapeoko3; chart columns 'Carbide Compact /Makita' and 'Dewalt' router dials",
               (G9 / "community_2402_post1_announcement.txt", 3, "Shapeoko3")),
    "c3d_nomad": ("hobby", rel(G9 / "community_2402_post1_announcement.txt") + ":7",
                  "post names the Nomad 883 (desktop machine)",
                  (G9 / "community_2402_post1_announcement.txt", 7, "Nomad 883")),
    "idc": ("hobby", rel(G10 / "wood_idc_feeds_speeds.txt") + ":19-20",
            "'average accepted values for benchtop CNC routers'",
            (G10 / "wood_idc_feeds_speeds.txt", 20, "benchtop CNC routers")),
    # industrial, keyed by LUT source_id
    "amana_spektra_spiral_plunge_v24": ("industrial", "crates/rs_cam_core/data/vendor_lut/sources/amana_spektra_spiral_plunge_v24.txt:1",
                                        "chart header 'INDUSTRIAL'",
                                        (LUT / "sources/amana_spektra_spiral_plunge_v24.txt", 1, "INDUSTRIAL")),
    "amana_ball_nose_v7": ("industrial", "crates/rs_cam_core/data/vendor_lut/sources/amana_ball_nose_v7.txt:1",
                           "chart header 'INDUSTRIAL'",
                           (LUT / "sources/amana_ball_nose_v7.txt", 1, "INDUSTRIAL")),
    "amana_zrn_3d_profiling_v8": ("industrial", "crates/rs_cam_core/data/vendor_lut/sources/amana_zrn_3d_v8.txt:1",
                                  "chart header 'INDUSTRIAL'",
                                  (LUT / "sources/amana_zrn_3d_v8.txt", 1, "INDUSTRIAL")),
    "amana_compression_spirals_v8": ("industrial", rel(G10 / "wood_amana_compression_v8.txt") + ":4",
                                     "chart header 'INDUSTRIAL'",
                                     (G10 / "wood_amana_compression_v8.txt", 4, "INDUSTRIAL")),
    "amana_spoilboard_v8": ("industrial", "crates/rs_cam_core/data/vendor_lut/source_manifest.json (no stored text)",
                            "Amana chart; no stored text of this chart in the repo; the stored Amana charts all print 'INDUSTRIAL' on line 1",
                            None),
    "freud_router_bit_feed_and_speed_for_cnc_20170822": (
        "industrial", "crates/rs_cam_core/data/vendor_lut/source_manifest.json:369",
        "the chart names no machine class; the manifest note says 'calibrated for industrial 10-15 kW CNC spindles'",
        (LUT / "source_manifest.json", 369, "industrial 10")),
    "onsrud": ("industrial", rel(G10 / "wood_onsrud_routing_guide.txt") + ":94",
               "Onsrud routing guide: 'router bits in the industrial market place'",
               (G10 / "wood_onsrud_routing_guide.txt", 94, "industrial market place")),
}
ONSRUD_IDS = {
    "onsrud_hard_wood_cutting_data", "onsrud_soft_wood_cutting_data", "onsrud_mdf_cutting_data",
    "onsrud_hard_plywood_cutting_data", "onsrud_soft_plywood_cutting_data",
}
EXCLUDED_SOURCES = {
    # source: (reason, file:line, token)
    "sienci_lm_feeds_and_speeds.txt": ("hobby; prints only the dial-to-rpm table, no feed rows",
                                       G10 / "sienci_lm_feeds_and_speeds.txt", 60, "Speed (RPM)"),
    "shopbot_user_guide_2015.txt": ("reprints the Onsrud chip ranges, not an independent chart",
                                    G10 / "shopbot_user_guide_2015.txt", 1961, "abstracted from the Onsrud"),
    "vectric_tool_database_v11.txt": ("software help; prints no chip or feed values",
                                      G10 / "vectric_tool_database_v11.txt", 835, "will vary"),
    "precisebits_tapered_ball_2f.txt": ("prints no feed: 'Do the Sweetspot Test'",
                                        G10 / "precisebits_tapered_ball_2f.txt", 344, "Sweetspot Test"),
    "precisebits_vtip_scoreengrave.txt": ("prints no feed: 'Do the Sweetspot Test'",
                                          G10 / "precisebits_vtip_scoreengrave.txt", 390, "Sweetspot Test"),
    "precisebits_vtip_2500.txt": ("prints no feed: 'Depends on the material being cut'",
                                  G10 / "precisebits_vtip_2500.txt", 360, "Depends on the material"),
    "wood_toolstoday_calc_video.txt": ("retailer narrative; one desktop example, used as prose evidence only",
                                       G10 / "wood_toolstoday_calc_video.txt", 81, "tabletop CNC"),
    "wood_toolstoday_understanding_feeds_speeds.txt": ("retailer narrative; one desktop example, used as prose evidence only",
                                                       G10 / "wood_toolstoday_understanding_feeds_speeds.txt", 179, "desktop CNC"),
    "idcwoodcraft_chipload_csv_2026-05-30 (LUT)": ("community CSV, grade c, not a tool maker",
                                                   LUT / "source_manifest.json", 377, "Community-aggregated"),
    "spetool_2d3d_tapered_router_bit_chart (LUT)": ("chart names no machine class; tapered only",
                                                    LUT / "sources/spetool_2d3d_tapered.txt", 1, "SpeTool"),
}

WOOD_GROUP = {"softwood": "softwood", "hardwood": "hardwood", "mdf": "MDF",
              "plywood_softwood": "plywood", "plywood_hardwood": "plywood"}

# ------------------------------------------------------------------ hobby rows

SIENCI = G10 / "sienci_feeds_speeds_metric.txt"
SIENCI_SOFT = ("softwood", "plywood_softwood", "mdf")  # block title line 5
SIENCI_HARD = ("hardwood", "plywood_hardwood")          # block title line 94
SIENCI_TIERS = {  # tier label line -> tier name; checked below
    "reduced": [(SIENCI, 27, "Reduced Speed"), (SIENCI, 118, "Reduced Speed")],
    "regular": [(SIENCI, 55, "Regular Speed"), (SIENCI, 146, "Regular Speed")],
    "full": [(SIENCI, 74, "Full Speed - Roughing"), (SIENCI, 165, "Full Speed - Roughing")],
}
# (line, label token, kind, diameter token, flute rule, tier, block, duplicate line with equal numbers)
SIENCI_ROWS = []
for block, woods, base in (("soft", SIENCI_SOFT, {"reduced": 26, "regular": 50, "full": 79}),
                           ("hard", SIENCI_HARD, {"reduced": 117, "regular": 141, "full": 170})):
    pass  # explicit table below is easier to audit

# Explicit table: line numbers in sienci_feeds_speeds_metric.txt.
# kind, label, D token, single_flute, tier, block, line, dup_line
_S = [
    # soft block
    ("flat_end", '1/4" Flat End Mill UC', '1/4"', False, "reduced", "soft", 26, 32),
    ("ball_nose", '1/4" Ball End Mill', '1/4"', False, "reduced", "soft", 33, None),
    ("flat_end", '1/8" Flat End Mill UC', '1/8"', False, "reduced", "soft", 35, 36),
    ("ball_nose", '1/8" Ball End Mill', '1/8"', False, "reduced", "soft", 37, None),
    ("flat_end", '1/16" Flat End Mill UC', '1/16"', False, "reduced", "soft", 38, 39),
    ("facing_bit", "22mm Surfacing Bit", "22mm", False, "reduced", "soft", 41, None),
    ("flat_end", '1/4" Flat End Mill UC', '1/4"', False, "regular", "soft", 50, 51),
    ("ball_nose", '1/4" Ball End Mill', '1/4"', False, "regular", "soft", 52, None),
    ("flat_end", '1/8" Flat End Mill UC', '1/8"', False, "regular", "soft", 54, 60),
    ("ball_nose", '1/8" Ball End Mill', '1/8"', False, "regular", "soft", 61, None),
    ("flat_end", '1/16" Flat End Mill UC', '1/16"', False, "regular", "soft", 62, 63),
    ("facing_bit", "22mm Surfacing Bit", "22mm", False, "regular", "soft", 65, None),
    ("flat_end", '1/4" Flat End Mill UC', '1/4"', False, "full", "soft", 79, 80),
    ("ball_nose", '1/4" Ball End Mill', '1/4"', False, "full", "soft", 81, None),
    ("flat_end", '1/8" Flat End Mill UC', '1/8"', False, "full", "soft", 83, 84),
    ("ball_nose", '1/8" Ball End Mill', '1/8"', False, "full", "soft", 85, None),
    ("flat_end", '1/16" Flat End Mill UC', '1/16"', False, "full", "soft", 86, 87),
    ("facing_bit", "22mm Surfacing Bit", "22mm", False, "full", "soft", 89, None),
    ("flat_end", '1/4" Single Flute Flat End Mill UC', '1/4"', True, "full", "soft", 73, None),
    ("flat_end", '1/8" Single Flute Flat End Mill UC', '1/8"', True, "full", "soft", 82, None),
    # hard block
    ("flat_end", '1/4" Flat End Mill UC', '1/4"', False, "reduced", "hard", 117, 123),
    ("ball_nose", '1/4" Ball End Mill', '1/4"', False, "reduced", "hard", 124, None),
    ("flat_end", '1/8" Flat End Mill UC', '1/8"', False, "reduced", "hard", 126, 127),
    ("ball_nose", '1/8" Ball End Mill', '1/8"', False, "reduced", "hard", 128, None),
    ("flat_end", '1/16" Flat End Mill UC', '1/16"', False, "reduced", "hard", 129, 130),
    ("facing_bit", "22mm Surfacing Bit", "22mm", False, "reduced", "hard", 132, None),
    ("flat_end", '1/4" Flat End Mill UC', '1/4"', False, "regular", "hard", 141, 142),
    ("ball_nose", '1/4" Ball End Mill', '1/4"', False, "regular", "hard", 143, None),
    ("flat_end", '1/8" Flat End Mill UC', '1/8"', False, "regular", "hard", 145, 151),
    ("ball_nose", '1/8" Ball End Mill', '1/8"', False, "regular", "hard", 152, None),
    ("flat_end", '1/16" Flat End Mill UC', '1/16"', False, "regular", "hard", 153, 154),
    ("facing_bit", "22mm Surfacing Bit", "22mm", False, "regular", "hard", 156, None),
    ("flat_end", '1/4" Flat End Mill UC', '1/4"', False, "full", "hard", 170, 171),
    ("ball_nose", '1/4" Ball End Mill', '1/4"', False, "full", "hard", 172, None),
    ("flat_end", '1/8" Flat End Mill UC', '1/8"', False, "full", "hard", 174, 175),
    ("ball_nose", '1/8" Ball End Mill', '1/8"', False, "full", "hard", 176, None),
    ("flat_end", '1/16" Flat End Mill UC', '1/16"', False, "full", "hard", 177, 178),
    ("facing_bit", "22mm Surfacing Bit", "22mm", False, "full", "hard", 180, None),
    ("flat_end", '1/4" Single Flute Flat End Mill UC', '1/4"', True, "full", "hard", 164, None),
    ("flat_end", '1/8" Single Flute Flat End Mill UC', '1/8"', True, "full", "hard", 173, None),
]
# Tier ranges: a row belongs to the tier whose label lies in the same table section.
TIER_RANGES = {("soft", "reduced"): (24, 45), ("soft", "regular"): (48, 69), ("soft", "full"): (72, 93),
               ("hard", "reduced"): (115, 136), ("hard", "regular"): (139, 160), ("hard", "full"): (163, 184)}


def inch_frac_mm(tok):
    """'1/4"' -> 6.35; '22mm' -> 22.0. Parsed from the label token."""
    if tok.endswith("mm"):
        return float(tok[:-2])
    a, b = tok.rstrip('"').split("/")
    return float(a) / float(b) * INCH_MM


def sienci_rows():
    line(SIENCI, 5, "Softwood, Soft Plywood, MDF")
    line(SIENCI, 94, "Hardwood, Hard Plywood")
    line(SIENCI, 8, "Feed Rate", "Plunge Rate", "Stepover", "Stepdown - Slotting", "Stepdown - Pocketing", "Router Speed")
    for tier, checks in SIENCI_TIERS.items():
        for p, n, tok in checks:
            line(p, n, tok)
    flute_line = line(REPO / "CREDITS.md", 952, "2 flutes are assumed")
    assumed_z = int(re.search(r"(\d+) flutes are assumed", flute_line).group(1))
    out = []
    for kind, label, dtok, single, tier, block, n, dup in _S:
        lo, hi = TIER_RANGES[(block, tier)]
        assert lo <= n <= hi, (n, tier, block)
        text = line(SIENCI, n, label)
        v = nums(text.split(label, 1)[1])
        feed, plunge, stepover, slot, pocket, rpm, dial = v
        if dup is not None:
            dl = line(SIENCI, dup)
            assert nums(dl.split("End Mill", 1)[1].split(" ", 2)[-1])[-7:] == v, (n, dup)
        if single:
            z, znote = 1, "1 flute printed in the tool name ('Single Flute')"
        else:
            z, znote = assumed_z, f"chart prints no flute count; {assumed_z} flutes assumed per CREDITS.md:952"
        D = inch_frac_mm(dtok)
        woods = SIENCI_SOFT if block == "soft" else SIENCI_HARD
        out.append(dict(source="sienci_metric", cite=f"{rel(SIENCI)}:{n}" + (f" (=:{dup})" if dup else ""),
                        tool=kind, label=label, D=D, Z=z, znote=znote, woods=woods, tier=tier,
                        feed_mm_min=feed, rpm=rpm, chip=feed / (rpm * z),
                        slot_mm=slot, pocket_mm=pocket, depth_note="Stepdown - Slotting / Stepdown - Pocketing"))
    return out


C3D_S3 = G10 / "carbide3d_s3_feeds_250.txt"
C3D_NOMAD = G10 / "carbide3d_nomad883_feeds_125.txt"
POST = G9 / "community_2402_post1_announcement.txt"


def c3d_rows():
    out = []
    s3_post = line(POST, 5, "3 flute", '.25"', "100% engagement")
    s3_z = int(re.search(r"(\d+) flute", s3_post).group(1))
    s3_D = float(re.search(r'(\.\d+)"', s3_post).group(1)) * INCH_MM
    nm_post = line(POST, 7, "2 flute", '.125"', "100% engagement")
    nm_z = int(re.search(r"(\d+) flute", nm_post).group(1))
    nm_D = float(re.search(r'(\.\d+)"', nm_post).group(1)) * INCH_MM
    line(C3D_S3, 7, "DOC", "RPM")
    line(C3D_S3, 9, "FEED", "PLUNGE")
    line(C3D_S3, 61, "WOOD")
    line(C3D_NOMAD, 6, "DOC", "RPM", "FEED", "PLUNGE")
    line(C3D_NOMAD, 59, "WOOD")
    s3 = {"Pine": (63, ("softwood",)), "Mahogany": (64, ("hardwood",)),
          "Plywood": (65, ("plywood_softwood", "plywood_hardwood")), "MDF": (66, ("mdf",))}
    nm = {"Pine": (61, ("softwood",)), "Mahogany": (62, ("hardwood",)),
          "Plywood": (63, ("plywood_softwood", "plywood_hardwood")), "MDF": (64, ("mdf",))}
    for mat, (n, woods) in s3.items():
        v = nums(line(C3D_S3, n, mat))
        doc_in, rpm, _d1, _d2, feed_ipm, _pl = v
        out.append(dict(source="c3d_s3", cite=f"{rel(C3D_S3)}:{n}", tool="flat_end",
                        label=f"S3 .25 in end mill, {mat}", D=s3_D, Z=s3_z,
                        znote=f"{s3_z} flutes from {rel(POST)}:5 (not on the chart)",
                        woods=woods, tier="chart", feed_mm_min=feed_ipm * INCH_MM, rpm=rpm,
                        chip=feed_ipm * INCH_MM / (rpm * s3_z), slot_mm=doc_in * INCH_MM, pocket_mm=None,
                        depth_note="DOC at 100% engagement (slot) per post line 5"))
    for mat, (n, woods) in nm.items():
        v = nums(line(C3D_NOMAD, n, mat))
        doc_in, rpm, feed_ipm, _pl = v
        out.append(dict(source="c3d_nomad", cite=f"{rel(C3D_NOMAD)}:{n}", tool="flat_end",
                        label=f"Nomad .125 in end mill, {mat}", D=nm_D, Z=nm_z,
                        znote=f"{nm_z} flutes from {rel(POST)}:7 (not on the chart)",
                        woods=woods, tier="chart", feed_mm_min=feed_ipm * INCH_MM, rpm=rpm,
                        chip=feed_ipm * INCH_MM / (rpm * nm_z), slot_mm=doc_in * INCH_MM, pocket_mm=None,
                        depth_note="DOC at 100% engagement (slot) per post line 7"))
    return out


IDC = G10 / "wood_idc_feeds_speeds.txt"


def idc_rows():
    """Imperial section 2 (the bits are inch bits; the metric section is inconsistent)."""
    line(IDC, 190, "inches")
    line(IDC, 203, "Dia.", "# Flutes")  # down cut header
    line(IDC, 202, "Feed", "Plunge", "Spindle")
    out = []
    table = [("flat_end", "down cut", n) for n in (207, 210, 213, 216, 219)] + \
            [("flat_end", "up cut", n) for n in (233, 236, 239, 242, 245)] + \
            [("ball_nose", "ballnose", n) for n in (261, 263)]
    line(IDC, 200, "DOWN CUT ENDMILL")
    line(IDC, 228, "UP CUT ENDMILL")
    line(IDC, 254, "BALLNOSE")
    for kind, sect, n in table:
        text = line(IDC, n)
        size = text.split()[0]
        v = nums(text.replace(size, "", 1).replace("Ballnose", "").replace("40%", "").replace("8%", ""))
        D_in, z, flen, oal, shank, feed_ipm, plunge, depth_in, rpm = v[:9]
        z = int(z)
        D = D_in * INCH_MM
        assert abs(D - inch_frac_mm(size)) < 0.05, (n, size, D)
        out.append(dict(source="idc", cite=f"{rel(IDC)}:{n}", tool=kind, label=f"IDC {size} {sect}",
                        D=inch_frac_mm(size), Z=z, znote=f"{z} flutes printed in the row",
                        woods=("softwood", "hardwood"), tier="chart", feed_mm_min=feed_ipm * INCH_MM, rpm=rpm,
                        chip=feed_ipm * INCH_MM / (rpm * z), slot_mm=None, pocket_mm=depth_in * INCH_MM,
                        depth_note="Depth Per Pass at the printed stepover (40% flat, 8% ball)"))
    return out


def hobby_rows():
    return sienci_rows() + c3d_rows() + idc_rows()


# ------------------------------------------------------------- industrial rows

def industrial_class(source_id):
    if source_id in ONSRUD_IDS:
        return CLASS["onsrud"]
    return CLASS.get(source_id)


def vendor_of(source_id):
    for v in ("amana", "onsrud", "freud"):
        if source_id.startswith(v):
            return v
    return source_id


# The research reads the vendor rows at the commit it states (FINDINGS.md: cb40af153), so later
# row edits (feeds contract P16) do not move its numbers.
DATA_REV = "cb40af153"


def rev_text(f):
    import subprocess
    return subprocess.run(["git", "-C", str(REPO), "show", f"{DATA_REV}:{rel(f)}"],
                          capture_output=True, text=True, check=True).stdout


def load_industrial():
    files = sorted((LUT / "observations").glob("*.json")) + [LUT / "industrial_only/freud_solid_carbide_industrial.json"]
    seen = {}
    for f in files:
        for r in json.loads(rev_text(f))["observations"]:
            if r["material_family"] not in WOOD_GROUP:
                continue
            if r["tool_family"] not in ("flat_end", "ball_nose", "facing_bit"):
                continue
            if r["evidence_grade"] not in ("a", "b") or r["row_kind"] == "fallback":
                continue
            if industrial_class(r["source_id"]) is None:
                continue
            if r.get("diameter_mm") is None or r.get("chipload_max_mm_tooth") is None:
                continue
            cmin = r.get("chipload_min_mm_tooth")
            cmax = r["chipload_max_mm_tooth"]
            cmin = cmax if cmin is None else cmin
            key = (r["source_id"], r.get("tool_subfamily"), r["tool_family"], r["material_family"],
                   r["diameter_mm"], r.get("flute_count"), cmin, cmax)
            if key in seen:
                seen[key]["n_copies"] += 1
                continue
            seen[key] = dict(obs=r["observation_id"], file=rel(f), source_id=r["source_id"],
                             vendor=vendor_of(r["source_id"]), sub=r.get("tool_subfamily"),
                             tool=r["tool_family"], wood=r["material_family"], D=r["diameter_mm"],
                             Z=r.get("flute_count"), cmin=cmin, cmax=cmax, mid=(cmin + cmax) / 2,
                             grade=r["evidence_grade"], kind=r["row_kind"], n_copies=1,
                             z_printed=not r["source_id"].startswith("freud"))
    return list(seen.values())


D_TOL = 0.02  # relative; picks the same printed inch size, not the next metric size


def flutes_match(h, i):
    if not i["z_printed"]:
        return True  # Freud prints no flute count; its chip is per tooth for any Z
    return h["Z"] == i["Z"]


def build():
    hob = hobby_rows()
    ind = load_industrial()
    pairs = []
    nopair = []
    for h in hob:
        found = 0
        for wood in h["woods"]:
            cands = [i for i in ind if i["tool"] == h["tool"] and i["wood"] == wood
                     and abs(i["D"] - h["D"]) / h["D"] <= D_TOL]
            matched = [i for i in cands if flutes_match(h, i)]
            if not matched:
                continue
            env_lo = min(i["cmin"] for i in matched)
            env_hi = max(i["cmax"] for i in matched)
            pos = "below" if h["chip"] < env_lo else ("above" if h["chip"] > env_hi else "inside")
            for i in matched:
                found += 1
                pairs.append(dict(h=h, i=i, wood=wood, ratio=h["chip"] / i["mid"],
                                  env_lo=env_lo, env_hi=env_hi, env_pos=pos))
        if not found:
            nopair.append(h)
    return hob, ind, pairs, nopair


# -------------------------------------------------------------------- stats

def sig3(x):
    if x == 0:
        return 0.0
    return float(f"{x:.3g}")


def summary(ratios):
    r = sorted(ratios)
    q = statistics.quantiles(r, n=4, method="inclusive") if len(r) >= 2 else [r[0]] * 3
    return dict(n=len(r), median=sig3(statistics.median(r)), q25=sig3(q[0]), q75=sig3(q[2]),
                min=sig3(r[0]), max=sig3(r[-1]), frac_below_1=sig3(sum(x < 1 for x in r) / len(r)))


def is_primary(p):
    h = p["h"]
    return h["tier"] in ("full", "chart") and h["Z"] != 1


def stats(pairs, hob, nopair):
    prim = [p for p in pairs if is_primary(p)]
    out = {"primary": summary([p["ratio"] for p in prim])}
    for key, fn in (("by_hobby_source", lambda p: p["h"]["source"]),
                    ("by_industrial_vendor", lambda p: p["i"]["vendor"]),
                    ("by_wood", lambda p: WOOD_GROUP[p["wood"]]),
                    ("by_tool", lambda p: p["h"]["tool"])):
        g = {}
        for p in prim:
            g.setdefault(fn(p), []).append(p["ratio"])
        out[key] = {k: summary(v) for k, v in sorted(g.items())}
    g = {}
    for p in prim:
        g.setdefault((p["h"]["source"], p["i"]["vendor"]), []).append(p["ratio"])
    out["by_source_x_vendor"] = {f"{a}|{b}": summary(v) for (a, b), v in sorted(g.items())}
    # Sienci tier sensitivity (2-flute rows)
    g = {}
    for p in pairs:
        if p["h"]["source"] == "sienci_metric" and p["h"]["Z"] != 1:
            g.setdefault(p["h"]["tier"], []).append(p["ratio"])
    out["sienci_by_tier"] = {k: summary(v) for k, v in sorted(g.items())}
    # envelope position, one count per (hobby row, wood) in the primary set
    env = {}
    seen = set()
    for p in prim:
        k = (p["h"]["cite"], p["wood"])
        if k in seen:
            continue
        seen.add(k)
        env.setdefault(p["h"]["source"], {}).setdefault(p["env_pos"], 0)
        env[p["h"]["source"]][p["env_pos"]] += 1
    out["envelope_by_hobby_source"] = env
    out["envelope_total"] = {pos: sum(e.get(pos, 0) for e in env.values()) for pos in ("below", "inside", "above")}
    out["n_pairs_all"] = len(pairs)
    out["n_pairs_primary"] = len(prim)
    out["n_hobby_rows"] = len(hob)
    out["n_hobby_rows_primary_paired"] = len({p["h"]["cite"] for p in prim})
    out["n_hobby_rows_unpaired"] = len(nopair)
    out["unpaired"] = [f'{h["cite"]} {h["label"]} D={h["D"]:.3f} Z={h["Z"]}' for h in nopair]
    out["n_industrial_rows_used"] = len({p["i"]["obs"] for p in prim})
    # depth: one value per primary hobby row
    dep = []
    seen = set()
    for p in prim:
        h = p["h"]
        if h["cite"] in seen:
            continue
        seen.add(h["cite"])
        dep.append(dict(cite=h["cite"], label=h["label"], source=h["source"], D=h["D"],
                        slot_over_D=None if h["slot_mm"] is None else sig3(h["slot_mm"] / h["D"]),
                        pocket_over_D=None if h["pocket_mm"] is None else sig3(h["pocket_mm"] / h["D"]),
                        chip=sig3(h["chip"])))
    out["depth_rows"] = dep
    # depth versus chip ratio: one point per primary hobby row (facing excluded: a 22 mm face cut)
    g, d = {}, {}
    for p in prim:
        h = p["h"]
        if h["tool"] == "facing_bit":
            continue
        g.setdefault(h["cite"], []).append(p["ratio"])
        dep_mm = h["slot_mm"] if h["slot_mm"] is not None else h["pocket_mm"]
        d[h["cite"]] = dep_mm / h["D"]
    keys = sorted(g)
    xs = [d[k] for k in keys]
    ys = [statistics.median(g[k]) for k in keys]
    out["depth_ratio_spearman"] = dict(n=len(keys), rho=sig3(statistics.correlation(ranks(xs), ranks(ys))))
    # within-source wood witnesses: same tool, soft block against hard block
    hb = {h["cite"]: h for h in hob}
    wit = {}
    for name, a, b in (("sienci_1_4_full", f"{rel(SIENCI)}:79 (=:80)", f"{rel(SIENCI)}:170 (=:171)"),
                       ("c3d_s3_pine_mahogany", f"{rel(C3D_S3)}:63", f"{rel(C3D_S3)}:64"),
                       ("c3d_nomad_pine_mahogany", f"{rel(C3D_NOMAD)}:61", f"{rel(C3D_NOMAD)}:62")):
        ha, hbb = hb[a], hb[b]
        wit[name] = dict(chip_hard_over_soft=sig3(hbb["chip"] / ha["chip"]),
                         depth_hard_over_soft=sig3(hbb["slot_mm"] / ha["slot_mm"]))
    out["wood_witnesses"] = wit
    return out


def ranks(v):
    order = sorted(range(len(v)), key=lambda i: v[i])
    r = [0.0] * len(v)
    i = 0
    while i < len(order):
        j = i
        while j + 1 < len(order) and v[order[j + 1]] == v[order[i]]:
            j += 1
        for t in range(i, j + 1):
            r[order[t]] = (i + j) / 2
        i = j + 1
    return r


def write_csv(pairs, path):
    cols = ["tool", "diameter_mm", "flutes", "wood", "wood_group", "hobby_source", "hobby_cite", "hobby_label",
            "hobby_tier", "hobby_feed_mm_min", "hobby_rpm", "hobby_chip_mm", "hobby_slot_depth_mm",
            "hobby_pocket_depth_mm", "industrial_vendor", "industrial_source_id", "industrial_observation_id",
            "industrial_file", "industrial_subfamily", "industrial_flutes", "industrial_grade",
            "industrial_chip_min", "industrial_chip_mid", "industrial_chip_max", "ratio_hobby_over_mid",
            "envelope_lo", "envelope_hi", "hobby_vs_envelope", "primary", "notes"]
    with open(path, "w", newline="") as fh:
        w = csv.writer(fh)
        w.writerow(cols)
        for p in pairs:
            h, i = p["h"], p["i"]
            notes = h["znote"]
            if not i["z_printed"]:
                notes += "; industrial chart prints no flute count (Freud)"
            if i["kind"] != "exact":
                notes += f"; industrial row_kind={i['kind']}"
            if i["n_copies"] > 1:
                notes += f"; {i['n_copies']} LUT copies of this cell"
            if h["source"] == "idc":
                notes += "; IDC prints one wood class ('soft, medium and moderately hard wood', line 20-21)"
            if h["source"].startswith("c3d") and "Plywood" in h["label"]:
                notes += "; chart prints 'Plywood' with no soft/hard split"
            if i["source_id"] == "amana_spoilboard_v8":
                notes += "; no stored text of this Amana chart"
            w.writerow([h["tool"], f'{h["D"]:.4f}', h["Z"], p["wood"], WOOD_GROUP[p["wood"]], h["source"],
                        h["cite"], h["label"], h["tier"], f'{h["feed_mm_min"]:.1f}', f'{h["rpm"]:.0f}',
                        f'{h["chip"]:.4f}', "" if h["slot_mm"] is None else f'{h["slot_mm"]:.2f}',
                        "" if h["pocket_mm"] is None else f'{h["pocket_mm"]:.2f}', i["vendor"], i["source_id"],
                        i["obs"], i["file"], i["sub"], i["Z"], i["grade"], f'{i["cmin"]:.4f}', f'{i["mid"]:.4f}',
                        f'{i["cmax"]:.4f}', f'{p["ratio"]:.3f}', f'{p["env_lo"]:.4f}', f'{p["env_hi"]:.4f}',
                        p["env_pos"], "yes" if is_primary(p) else "no", notes])


def check_classes():
    for k, (cls, cite, why, chk) in CLASS.items():
        if chk:
            line(*chk)
    for k, (why, p, n, tok) in EXCLUDED_SOURCES.items():
        line(p, n, tok)


def main():
    check_classes()
    hob, ind, pairs, nopair = build()
    write_csv(pairs, HERE / "pairs.csv")
    st = stats(pairs, hob, nopair)
    st["classes"] = {k: {"class": v[0], "cite": v[1], "reason": v[2]} for k, v in CLASS.items()}
    st["excluded_sources"] = {k: {"reason": v[0], "cite": f"{rel(v[1])}:{v[2]}"} for k, v in EXCLUDED_SOURCES.items()}
    (HERE / "stats.json").write_text(json.dumps(st, indent=1))
    print(json.dumps({k: st[k] for k in ("primary", "by_hobby_source", "by_industrial_vendor", "by_wood", "by_tool",
                                         "by_source_x_vendor", "sienci_by_tier", "envelope_by_hobby_source",
                                         "envelope_total", "n_pairs_all", "n_pairs_primary", "n_hobby_rows",
                                         "n_hobby_rows_primary_paired", "n_hobby_rows_unpaired", "unpaired",
                                         "n_industrial_rows_used", "depth_ratio_spearman", "wood_witnesses")}, indent=1))
    for d in st["depth_rows"]:
        print(d)


if __name__ == "__main__":
    main()
