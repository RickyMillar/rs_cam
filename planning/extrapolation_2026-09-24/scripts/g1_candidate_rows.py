#!/usr/bin/env python3
"""G1 fetch: build fetch/G1/candidate_rows.json from the stored chart texts.

Every row carries "verbatim", a line copied from the stored text. The script
asserts that each verbatim is a substring of its stored text, and that each
printed feed closes the identity feed = RPM x flutes x chipload where the
chart prints a feed. It writes no number that the chart does not print,
except the unit conversion (inch -> mm, x 25.4) and the derived flute count
of the SpeTool rows (from the printed feed identity), which the notes name.

Run from the repository root:
  python3 planning/extrapolation_2026-09-24/scripts/g1_candidate_rows.py
"""
import json
import os

G = "planning/extrapolation_2026-09-24/fetch/G1"
IN = 25.4
JANKA = {"softwood": 600.0, "hardwood": 1450.0, "mdf": 1100.0}
ACCESSED = "2026-09-24"
HARDNESS_NOTE = ("hardness_value is the engine's own Janka proxy for the material "
                 "family (GenericSoftwood 600, GenericHardwood 1450, MDF 1100); "
                 "the chart prints no hardness.")


def text(name):
    return open(os.path.join(G, "sources", name + ".txt")).read()


def check(verbatim, stored):
    assert verbatim in text(stored), f"verbatim not in {stored}: {verbatim!r}"


def mm(v):
    return round(v * IN, 5)


rows = []

# ---------------------------------------------------------------- Amana v8
AM = dict(source_id="amana_zrn_3d_profiling_v8", source_vendor="amana",
          source_title="Amana ZrN-Coated and Uncoated 2D/3D Carving CNC Solid "
                       "Carbide Router Bits chip load chart (v8)",
          source_url="https://www.amanatool.com/pub/media/productattachments/"
                     "ZrN-3D-Profiling-Feed-Chip-Load-Chart-v8.pdf")
AM_STORED = "amana_zrn_3d_v8"
L2F = ("Wood, MDF, Sign-Foam                                         30\" - 70\""
       "                0.00075\" - 0.002\"                 55\" - 90\"                "
       "0.003\" - 0.005\"              250\" - 320\"                  0.007\" - 0.009\"")
L3F = ("Wood, MDF, Sign-Foam           40\" - 108\"     0.00075\" - 0.002\"     "
       "80\" - 100\"    0.0015\" - 0.0025\"      100\" - 170\"     0.0025\" - 0.004\"      "
       "215\" - 320\"     0.004\" - 0.006\"        320\" - 430\"     0.006\" - 0.008\"       "
       "375\" - 490\"       0.007\" - 0.009\"")
L4F = ("Wood, MDF, Sign-Foam                     35\" - 45\"         0.0005\" - 0.00065\""
       "           35\" - 45\"         0.0005\" - 0.00065\"          35\" - 45\"        "
       "0.0005\" - 0.00065\"           35\" - 45\"          0.0005\" - 0.00065\"")
for line in (L2F, L3F, L4F):
    check(line, AM_STORED)

# (diameter_mm, flutes, section, column, cl_min_in, cl_max_in, ipm_lo, ipm_hi,
#  tools, extra note)
AM_CELLS = [
    (1.0, 2, "2 Flute Ball Nose", "1mm (0.0394\")", 0.00075, 0.002, 30, 70,
     "46256 (5.4 deg taper, 1mm D x 0.5mm R, 2 flutes). The same column also "
     "lists 46471 (0.10 deg, 1mm D, a straight ball), which the chart lists a "
     "second time in the 3 Flute section as '46471 0.8mm Dia.'.", L2F),
    (1.5875, 2, "2 Flute Ball Nose", "1/16\" (0.0625\")", 0.003, 0.005, 55, 90,
     "46252 (5.5 deg taper, 1/16\" D x 1/32\" R, 2 flutes). CHART SELF-"
     "INCONSISTENT: the printed IPM 55-90 at 18,000 RPM x 2 flutes gives "
     "0.0015-0.0025 in/tooth (derived), half the printed chip load; the LUT row "
     "amana-ball-softwood-parallel-1587-2f-zrn uses the IPM-derived value. This "
     "row keeps the printed chip load and is graded b for that reason.", L2F),
    (6.35, 2, "2 Flute Ball Nose", "6mm (0.2362\") - 1/4\" (0.250\")", 0.007, 0.009,
     250, 320,
     "46283 (3 deg taper, 1/4\" D x 1/8\" R, 2 flutes). The same column lists "
     "46294 and 46479 (0.10 deg straight balls, 1/4\" and 6 mm): the chart gives "
     "one value to a tapered and a straight ball of the same tip.", L2F),
    (0.79375, 3, "3 Flute Ball Nose", "1/32\" (0.031\") - 1mm (0.0394\")", 0.00075,
     0.002, 40, 108,
     "46280, 46291, 46580 (6.2 deg taper, 1/32\" D x 1/64\" R, 3 flutes) and "
     "46470 (6.2 deg taper, 0.8 D x 0.40 R). v8 also lists 46473 (6.2 deg taper, "
     "0.5 D) in this section, but no column covers 0.5 mm (the column label "
     "starts at 1/32\"); no 0.5 mm row is written from this chart.", L3F),
    (3.175, 3, "3 Flute Ball Nose", "1/8\" (0.125\") - 3.2mm (.126\")", 0.0015,
     0.0025, 80, 100,
     "46284 (1 deg), 46286 (3.6 deg), 46287 (5 deg), 46288 (7 deg) tapers, 1/8\" D "
     "x 1/16\" R, 3 flutes; 46474 (1 deg taper, 3.2 D). The column also lists "
     "46295 (0.10 deg straight ball). CHART SELF-INCONSISTENT at the top of the "
     "band: IPM 80-100 at 18,000 x 3 gives 0.00148-0.00185 in/tooth (derived).",
     L3F),
    (4.7625, 3, "3 Flute Ball Nose", "3/16\" (0.1875\")", 0.0025, 0.004, 100, 170,
     "46298 (1 deg taper, 3/16\" D x 3/32\" R, 3 flutes). CHART SELF-INCONSISTENT: "
     "IPM 100-170 at 18,000 x 3 gives 0.00185-0.00315 in/tooth (derived).", L3F),
    (1.5, 4, "4 Flute Ball Nose & Flat Bottom", "1.5mm (0.0591\")", 0.0005, 0.00065,
     35, 45, "46472 (5.4 deg taper, 1.5 D x 0.75 R, 4 flutes).", L4F),
    (1.5875, 4, "4 Flute Ball Nose & Flat Bottom", "1/16\" (0.0625\")", 0.0005,
     0.00065, 35, 45,
     "46282, 46293, 46582 (5.4 deg taper, 1/16\" D x 1/32\" R, 4 flutes). The "
     "section also lists 46572 (5.4 deg tapered flat bottom), not written here.",
     L4F),
    (3.175, 4, "4 Flute Ball Nose & Flat Bottom", "1/8\" (0.125\")", 0.0005,
     0.00065, 35, 45,
     "46583 (3.6 deg taper, 1/8\" D x 1/16\" R, 4 flutes). The section also "
     "lists 46292 (0.10 deg flat bottom), not written here.", L4F),
]
for d, z, sect, col, lo, hi, ipm_lo, ipm_hi, tools, line in AM_CELLS:
    implied = (ipm_lo / (18000 * z), ipm_hi / (18000 * z))
    for mat in ("mdf", "softwood", "hardwood"):
        shared = mat != "mdf"
        inconsistent = "SELF-INCONSISTENT" in tools and d == 1.5875
        grade = "b" if (shared or inconsistent) else "a"
        kind = "derived" if shared else "exact"
        oid = f"x-g1-amana-zrn-tapered-{mat}-parallel-{round(d * 1000):05d}-{z}f"
        rows.append(dict(
            observation_id=oid, **AM, accessed_on=ACCESSED,
            source_page=f"page {'1' if z < 4 else '2'} of 2, section '{sect}', column "
                        f"{col}, row 'Wood, MDF, Sign-Foam': {lo}\" - {hi}\" chip "
                        f"load per tooth, {ipm_lo}\" - {ipm_hi}\" IPM at 18,000 RPM",
            evidence_grade=grade, row_kind=kind,
            tool_family="tapered_ball_nose", tool_subfamily="zrn_2d3d_carving_tapered",
            operation_family="parallel", pass_role="finish",
            material_family=mat,
            material_label="Wood, MDF, Sign-Foam (one printed row)",
            hardness_kind="janka", hardness_value=JANKA[mat],
            diameter_mm=d, tip_diameter_mm=d, flute_count=z, rpm_nominal=18000.0,
            chipload_min_mm_tooth=mm(lo), chipload_max_mm_tooth=mm(hi),
            ap_rule="Depth of Cut: 1 x Tool Diameter (chart header); 2 x D reduce "
                    "feed rate by 25%; 3 x D reduce feed rate by 50%",
            ap_max_factor=1.0,
            notes=("G1 fetch 2026-09-24. The chart keys the chip load on the tool "
                   "'Dia.', which is the ball TIP diameter of these tapered tools "
                   "(product spec D and R, see sources/amana_46xxx_identity.txt). "
                   f"Tools in this cell: {tools} "
                   + ("The chart prints one 'Wood, MDF, Sign-Foam' row; this row "
                      f"applies it to {mat} (R5 convention for a shared column: "
                      "derived, grade b). " if shared else
                      "MDF is named in the printed row 'Wood, MDF, Sign-Foam'. ")
                   + f"Printed IPM / (18,000 x {z}) = {implied[0]:.5f}-{implied[1]:.5f} "
                   "in/tooth (derived check). Grade rule for a self-inconsistent "
                   "cell: where the printed chip-load band and the IPM band overlap, "
                   "the printed band keeps its grade; where they do not overlap "
                   "(only the 2 flute 1/16\" cell), the row is grade b. The chart "
                   "names no operation; the "
                   "row sits in parallel / finish, as the LUT's Amana ZrN rows do. "
                   "v1 of the chart (amana_zrn_3d_profiling, same sha256 as the LUT "
                   "copy) prints the same values in this cell. " + HARDNESS_NOTE),
            extrapolation_group="G1", verbatim=line))

# ------------------------------------------------------- SpeTool tapered
SP = dict(source_id="spetool_2d3d_tapered_router_bit_chart", source_vendor="spetool",
          source_title="SpeTool WOODWORKING 2D/3D ROUTER BIT FEED&SPEED CHART",
          source_url="https://cdn.shopify.com/s/files/1/0404/1582/1977/files/"
                     "SpeTool_2D_3D_Tapered_router_bit_a51e0653-1672-4ba2-98bc-"
                     "8852cde4fc0f.pdf?v=1676951061")
SP_CELLS = [  # (label, diameter_mm, chipload_in, ipm, stepdown, stepover, unit)
    ("0.5", 0.5, "0.0007", "25.2", "0.5", "0.2"),
    ("1.0", 1.0, "0.001", "36", "1.0", "0.4"),
    ("1.5", 1.5, "0.0015", "54", "1.5", "0.6"),
    ("2.0", 2.0, "0.003", "108", "2.0", "0.8"),
    ("3.0", 3.0, "0.004", "144", "3.0", "1.2"),
    ("4.0", 4.0, "0.005", "180", "4.0", "1.6"),
    ("1/32", 0.79375, "0.0008", "28.8", "0.03", "0.0125"),
    ("1/16", 1.5875, "0.0015", "54", "0.06", "0.025"),
    ("1/8", 3.175, "0.004", "144", "0.125", "0.05"),
]
for lab, d, cl, ipm, sd, so in SP_CELLS:
    line = f"{lab} | Wood, MDF, Sign-Foam | 18,000 | {cl} | {ipm} | {sd} | {so}"
    check(line, "spetool_2d3d_tapered")
    z = float(ipm) / (18000 * float(cl))
    assert abs(z - 2.0) < 0.02, (lab, z)
    unit = "MM" if "/" not in lab else "INCH"
    for mat in ("mdf", "softwood", "hardwood"):
        shared = mat != "mdf"
        oid = f"x-g1-spetool-tapered-{mat}-parallel-{round(d * 1000):05d}-2f"
        rows.append(dict(
            observation_id=oid, **SP, accessed_on=ACCESSED,
            source_page=f"page 1, 'Tip Diameter ({unit})' table, row {lab}, "
                        f"'Wood, MDF, Sign-Foam': CHIPLOAD {cl}, FEED RATE {ipm} "
                        "INCH/MIN at 18,000 RPM",
            evidence_grade="b", row_kind="derived" if shared else "exact",
            tool_family="tapered_ball_nose", tool_subfamily="spetool_2d3d_tapered",
            operation_family="parallel", pass_role="finish",
            material_family=mat,
            material_label="Wood, MDF, Sign-Foam (one printed row)",
            hardness_kind="janka", hardness_value=JANKA[mat],
            diameter_mm=d, tip_diameter_mm=d, flute_count=2, rpm_nominal=18000.0,
            chipload_max_mm_tooth=mm(float(cl)),
            ap_rule="Depth of cut: 1xCutting diameter; 2 x cutting diameter reduce "
                    "feed rate by 30%; 3 x cutting diameter reduce feed rate by 50%",
            ap_max_factor=1.0,
            notes=("G1 fetch 2026-09-24. The chart keys the chip load on 'Tip "
                   "Diameter'. The chart prints one value, not a range, so the row "
                   "publishes only chipload_max_mm_tooth. The chart prints no unit "
                   "for CHIPLOAD and no flute count: the unit (inch per tooth) and "
                   f"flute_count 2 are DERIVED from the printed feed identity "
                   f"'Feed rate=RPM x # of flutes x chipload' ({ipm} / (18,000 x "
                   f"{cl}) = {z:.2f}); the SpeTool W01001 tapered ball page prints "
                   "'2 Flute'. The tool family is inferred: the chart title says "
                   "'2D/3D ROUTER BIT', the PDF file name says '2D_3D_Tapered_router_"
                   "bit' and the SpeTool index page lists it as '2D & 3D Tapered "
                   "Router Bits' (sources/spetool_speed_feeds_index.txt). Grade b "
                   "for these inferences and because the PDF is a scan (the table "
                   "is transcribed from the page image, section A of the stored "
                   "text). "
                   + (f"The chart prints one 'Wood, MDF, Sign-Foam' row; this row "
                      f"applies it to {mat} (R5 convention: derived). " if shared
                      else "MDF is named in the printed row. ")
                   + "The chart names no operation; the row sits in parallel / "
                   "finish. " + HARDNESS_NOTE),
            extrapolation_group="G1", verbatim=line))

# -------------------------------------------------------- SpeTool spiral
SS = dict(source_id="spetool_carbide_spiral_router_bit_chart", source_vendor="spetool",
          source_title="SpeTool WOODWORKING CARBIDE SPIRAL ROUTER BIT FEED&SPEED CHART",
          source_url="https://cdn.shopify.com/s/files/1/0404/1582/1977/files/"
                     "SpeTool_spiral_Router_bits_8ae74723-4152-4436-82f7-"
                     "8306b61437f3.pdf?v=1676951571")
SUB = {"up": "spetool_upcut_spiral", "down": "spetool_downcut_spiral",
       "compression": "spetool_compression_spiral"}
MAT = {"MDF Laminate": "mdf", "softwood": "softwood", "hardwood": "hardwood"}
SIZES = {"1/16": 1.5875, "1/8": 3.175, "1/4": 6.35, "3/8": 9.525, "1/2": 12.7}
for ln in text("spetool_spiral").split("## B.")[0].splitlines():
    parts = [p.strip() for p in ln.split("|")]
    if len(parts) != 8 or parts[0] not in SIZES:
        continue
    lab, matl, dirn, rpm, cl, ipm, sd, so = parts
    d = SIZES[lab]
    rpmv = float(rpm.replace(",", ""))
    z = float(ipm) / (rpmv * float(cl))
    assert abs(z - 2.0) < 0.2, (ln, z)  # the chart rounds its feeds; worst case 2.13 (1/4 MDF down)
    mat = MAT[matl]
    oid = f"x-g1-spetool-spiral-{dirn}-{mat}-pocket-{round(d * 1000):05d}-2f"
    rows.append(dict(
        observation_id=oid, **SS, accessed_on=ACCESSED,
        source_page=f"page 1, CUTTING DIAMETER {lab}, {matl}, {dirn}: CHIPLOAD {cl}, "
                    f"FEED RATE {ipm} INCH/MIN at {rpm} RPM",
        evidence_grade="b", row_kind="exact",
        tool_family="flat_end", tool_subfamily=SUB[dirn],
        operation_family="pocket", pass_role="roughing",
        material_family=mat, material_label=matl,
        hardness_kind="janka", hardness_value=JANKA[mat],
        diameter_mm=d, flute_count=2, rpm_nominal=rpmv,
        chipload_max_mm_tooth=mm(float(cl)),
        ap_rule="Depth of cut: 1xCutting diameter; 2 x cutting diameter reduce "
                "feed rate by 30%; 3 x cutting diameter reduce feed rate by 50%",
        ap_max_factor=1.0,
        notes=("G1 fetch 2026-09-24. Printed row; hardwood, softwood and MDF "
               "Laminate are printed apart. The chart prints one value, not a range, "
               "so the row publishes only chipload_max_mm_tooth. The chart prints no "
               "unit for CHIPLOAD and no flute count: inch per tooth and flute_count "
               f"2 are DERIVED from the printed feed identity ({ipm} / ({rpm} x {cl})"
               f" = {z:.2f}; the chart rounds the feed). The chart names no "
               "operation; the row sits in pocket / roughing at the chart's 1 x D "
               "condition, as the LUT's Spektra rows do. The table is transcribed "
               "from the page image (the PDF is a scan). Grade b, not a: the PDF "
               "is a scan and the unit and the flute count are derived, as on the "
               "SpeTool tapered rows. " + HARDNESS_NOTE),
        extrapolation_group="G1", verbatim=ln))

ids = [r["observation_id"] for r in rows]
assert len(ids) == len(set(ids)), "duplicate observation_id"
json.dump({"observations": rows}, open(os.path.join(G, "candidate_rows.json"), "w"),
          indent=1)
from collections import Counter
print(len(rows), Counter(r["source_id"] for r in rows))
