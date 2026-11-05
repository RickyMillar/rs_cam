"""Check each claim of FINDINGS.md. PASS/FAIL per ID; exit 1 on any FAIL.

Run: python3 -I analysis.py && python3 -I assert_claims.py
Reads results.json (written by analysis.py from git show 7048456f8) and re-reads repo files at REV.
"""
import csv
import io
import json
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", "..", ".."))
REV = "7048456f8"
R = json.load(open(os.path.join(HERE, "results.json")))
F = R["fits"]
FAILS = []


def show(path):
    return subprocess.run(["git", "-C", REPO, "show", f"{REV}:{path}"], capture_output=True, text=True,
                          check=True).stdout


def claim(cid, text, ok):
    print(("PASS" if ok else "FAIL"), cid, text)
    if not ok:
        FAILS.append(cid)


def near(a, b, tol):
    return abs(a - b) <= tol


cells = R["cells"]
flat = [c for c in cells if c["tool"] == "EndMill"]
ball = [c for c in cells if c["tool"] == "BallNose"]
vbit = [c for c in cells if c["tool"] == "VBit"]

# --- (a) the formula-chip cells
claim("A01", "FM1 has 60 formula-chip cells: 32 flat, 20 ball, 8 V-bit",
      R["fm1_formula_cells"] == 60 and (len(flat), len(ball), len(vbit)) == (32, 20, 8))
claim("A02", "flat cells are Trace, RampFinish, RadialFinish, HorizontalFinish x {3.175, 6} x 4 woods",
      sorted({c["op"] for c in flat}) == ["HorizontalFinish", "RadialFinish", "RampFinish", "Trace"]
      and sorted({c["D"] for c in flat}) == [3.175, 6.0])
claim("A03", "the shipped formula and the SFM rpm reproduce every flat and ball cell (chip 1 %, rpm 2)",
      all(abs(c["repro_chip"] / c["chip"] - 1) < 0.01 and abs(c["repro_rpm"] - c["rpm"]) <= 2
          for c in flat + ball))
c6h = [c for c in flat if c["D"] == 6.0 and c["mat"] == "hardwood" and c["op"] == "Trace"][0]
claim("A04", "6 mm hardwood Trace ships chip 0.0459, rpm 10610, feed 973",
      (c6h["chip"], c6h["rpm"], c6h["feed"]) == (0.0459, 10610.0, 973.0))
reg = show("crates/rs_cam_core/src/compute/catalog/registry.rs").split("\n")
claim("A05", "DropCutter routes FlatParallelAsRoughPocket; RampFinish, RadialFinish, HorizontalFinish AsStated",
      "FlatParallelAsRoughPocket" in reg[878] and all("AsStated" in reg[i - 1] for i in (1309, 1424, 1485)))
lut_rows = []
files = [l.split("observations/")[1].split('"')[0] for l in
         show("crates/rs_cam_core/src/feeds/vendor_lut.rs").split("\n") if "include_str!(\"../../data/vendor_lut/observations/" in l]
for f in files:
    lut_rows += json.loads(show("crates/rs_cam_core/data/vendor_lut/observations/" + f))["observations"]
flat_fams = {r["operation_family"] for r in lut_rows if r["tool_family"] == "flat_end"}
ball_fams = {r["operation_family"] for r in lut_rows if r["tool_family"] == "ball_nose"}
vbit_fams = {r["operation_family"] for r in lut_rows if r["tool_family"] == "chamfer_vbit"}
claim("A06", "no embedded flat_end row sits in the trace or parallel family", not (flat_fams & {"trace", "parallel"}))
claim("A07", "no embedded ball_nose row sits in the contour or trace family", not (ball_fams & {"contour", "trace"}))
claim("A08", "every embedded V-bit row sits in the trace family", vbit_fams == {"trace"})
fam = show("crates/rs_cam_core/src/feeds/extrapolation/family.rs")
claim("A09", "family.rs states there is no ball-nose rule", "There is no ball-nose rule" in fam)
fm1 = list(csv.DictReader(io.StringIO(show(
    "crates/rs_cam_core/tests/fixtures/feeds_matrix_2026-09-23/matrix_2026-09-23.csv"))))
dc = [x for x in fm1 if x["tool_type"] == "EndMill" and x["operation"] == "DropCutter"
      and x["diameter_mm"] == "6.0000" and x["material"] == "hardwood"][0]
claim("A10", "FM1 6 mm hardwood DropCutter reads the Spektra row 0.127 at feed 3999",
      dc["lut_observation_id"] == "amana-flat-hardwood-pocket-6000-2f-spektra" and dc["r_chip_load_mm"] == "0.1270"
      and dc["feed_mm_min"] == "3999.0000")

# --- (b) fits
claim("B01", "165 deduplicated wood flat points from 209 series rows (amana 73, onsrud 80, freud 12)",
      R["points"] == 165 and R["series_rows"] == 209
      and R["points_by_vendor"] == {"amana": 73, "onsrud": 80, "freud": 12})
claim("B02", "only the Spektra chart prints more than one flute count",
      R["z_sources"] == ["amana:amana_spektra_spiral_plunge_v24"])
sf = F["shipped_formula"]["per_vendor"]
claim("B03", "printed chip / shipped formula: median 2.17 Amana, 4.95 Freud, 5.20 Onsrud",
      near(sf["amana"]["median_ratio"], 2.17, 0.01) and near(sf["freud"]["median_ratio"], 4.95, 0.01)
      and near(sf["onsrud"]["median_ratio"], 5.20, 0.01))
cd = F["class_D/point"]
claim("B04", "pooled law a_class D^b: b 0.717, R2 0.555, rms factor 1.58",
      near(cd["beta"][4], 0.717, 0.001) and near(cd["r2"], 0.555, 0.001) and near(cd["rms_factor"], 1.58, 0.01))
lo = cd["lovo"]
claim("B05", "pooled law, leave one vendor out: Amana 0.48, Onsrud 2.09, Freud 1.55 (median printed/law)",
      near(lo["amana"]["median_ratio"], 0.48, 0.01) and near(lo["onsrud"]["median_ratio"], 2.09, 0.01)
      and near(lo["freud"]["median_ratio"], 1.55, 0.01))
cz = F["class_D_Z/point"]
claim("B06", "pooled Z exponent -1.10 without a vendor term; +0.06 with a vendor term",
      near(cz["beta"][5], -1.10, 0.01) and near(F["class_D_Z_vendor/point"]["beta"][5], 0.057, 0.005))
p3 = F["paired_3f_over_2f"]["amana"]
claim("B07", "Spektra 3F / 2F at the same D and material: median 1.00, range 0.625-1.25 (n 20)",
      p3["n"] == 20 and near(p3["median"], 1.0, 1e-9) and near(p3["min"], 0.625, 1e-6) and near(p3["max"], 1.25, 1e-6))
jk = F["janka_D_Z/point"]
claim("B08", "pooled Janka exponent -0.002 (the shipped formula composes -0.504)", near(jk["beta"][3], -0.002, 0.001))
claim("B09", "hardwood / softwood inside one series: Onsrud 1.00 (n 11), Freud 0.86 (n 3)",
      near(F["paired_hard_over_soft"]["onsrud"]["median"], 1.0, 1e-9) and F["paired_hard_over_soft"]["onsrud"]["n"] == 11
      and near(F["paired_hard_over_soft"]["freud"]["median"], 0.864, 0.001))
claim("B10", "MDF / softwood inside one series: Amana 1.25, Freud 1.16, Onsrud 0.85",
      near(F["paired_mdf_over_soft"]["amana"]["median"], 1.25, 0.005)
      and near(F["paired_mdf_over_soft"]["freud"]["median"], 1.163, 0.001)
      and near(F["paired_mdf_over_soft"]["onsrud"]["median"], 0.845, 0.001))
cv = F["class_D_Z_vendor/point"]
claim("B11", "a vendor offset lifts R2 0.601 -> 0.801; Freud x2.05 and Onsrud x2.20 over Amana",
      near(cz["r2"], 0.601, 0.001) and near(cv["r2"], 0.801, 0.001)
      and near(cv["vendor_offsets_vs_first"]["freud"], 2.05, 0.01) and near(cv["vendor_offsets_vs_first"]["onsrud"], 2.20, 0.01))
cs = F["class_D_Z_series/point"]
claim("B12", "series offsets (37): R2 0.953, rms 1.16, D slope 0.560, Z slope 0.026",
      cs["n_series"] == 37 and near(cs["r2"], 0.953, 0.001) and near(cs["rms_factor"], 1.16, 0.005)
      and near(cs["D_slope"], 0.560, 0.001) and near(cs["Z_slope"], 0.026, 0.001))
off = cs["class_offsets"]
import math  # noqa: E402
claim("B13", "series law: softwood, hardwood, plywood within 2 %; MDF x1.38 over softwood",
      max(off[0], off[1], off[3]) - min(off[0], off[1], off[3]) < 0.02 and near(math.exp(off[2] - off[0]), 1.38, 0.005))
ps = F["per_vendor_slope"]
claim("B14", "own D slope per vendor: Amana 0.53, Onsrud 0.19 (series mixed), Freud 1.16",
      near(ps["amana"]["D_slope"], 0.53, 0.01) and near(ps["onsrud"]["D_slope"], 0.19, 0.01)
      and near(ps["freud"]["D_slope"], 1.16, 0.01))

# --- (c) industrial rows
claim("C01", "the 4 industrial_only rows are in the embedded freud_solid_carbide.json with the same values",
      all(x["in_embedded"] and x["same_values"] and x["embedded_file"] == "freud_solid_carbide.json"
          for x in R["industrial_vs_embedded"]) and len(R["industrial_vs_embedded"]) == 4)
readme = show("crates/rs_cam_core/data/vendor_lut/industrial_only/README.md")
claim("C02", "the industrial_only README still says the rows are not loaded", "**not loaded**" in readme)
log = subprocess.run(["git", "-C", REPO, "log", "--format=%h %ad", "--date=short", "-S",
                      "freud-solid-carbide-half-hardwood", REV, "--",
                      "crates/rs_cam_core/data/vendor_lut/observations/freud_solid_carbide.json"],
                     capture_output=True, text=True, check=True).stdout.split()
claim("C03", "the rows entered the embedded file in 51edd494d (2026-06-01)", log[:2] == ["51edd494d", "2026-06-01"])
ioos = [e["ratio"] for e in cd["industrial_oos"]]
claim("C04", "pooled law (fitted without them) gives the industrial rows 1.24-1.99 x too low",
      near(min(ioos), 1.24, 0.01) and near(max(ioos), 1.99, 0.01))
an = [e["ratio"] for e in F["anchored_series_law_industrial_oos"]]
claim("C05", "anchored series law (Freud own row x (D/D_row)^0.56): 1.11-1.76 x too low",
      near(min(an), 1.11, 0.01) and near(max(an), 1.76, 0.01))

claim("C06", "no FM1 cell reads a Freud row", not any("freud" in x["lut_observation_id"] for x in fm1))
fr = show("planning/extrapolation_2026-09-24/fetch/G4/sources/freud_router_bit_feed_and_speed_for_cnc_20170822.txt")
froles = {(r["material_family"], r["pass_role"]) for r in lut_rows if r["source_vendor"] == "freud"
          and r["material_family"] in ("hardwood", "softwood", "mdf", "plywood_hardwood")}
claim("C07", "Freud prints one solid-carbide chart at cut depth = D with no rough/finish split; the LUT labels "
      "hardwood, softwood, plywood finish and MDF roughing",
      "based on cut depth equal to bit diameter" in fr and "Roughing" not in fr and "Finishing" not in fr
      and froles == {("hardwood", "finish"), ("softwood", "finish"), ("plywood_hardwood", "finish"), ("mdf", "roughing")})

# --- (d) finish relation
hv = R["harvey"]
claim("D01", "Harvey 809500 Finishing / Slot-Rough median 0.763 / 0.773 / 0.775 / 0.771 (four rows)",
      [round(hv["rows"][k]["ratio_median"], 3) for k in ("softer", "harder", "engineered", "phenolic")]
      == [0.763, 0.773, 0.775, 0.771])
claim("D02", "Harvey ratio range over 15 diameters and 4 rows: 0.667-0.833",
      near(min(v["ratio_min"] for v in hv["rows"].values()), 0.667, 0.001)
      and near(max(v["ratio_max"] for v in hv["rows"].values()), 0.833, 0.001))
claim("D03", "Garr lift at ae 0.1 D is 1.667; x the printed 3 x D table 0.5 gives 0.833",
      near(hv["model"]["garr_lift"], 1.6667, 1e-3) and near(hv["model"]["product"], 0.8333, 1e-3))
claim("D05", "no-thinning reading at (0.1 D, 3 x D) is 0.5; Harvey 0.763-0.775 is 1.53-1.55 x above it",
      all(near(hv["rows"][k]["ratio_median"] / 0.5, 1.54, 0.015) for k in ("softer", "harder", "engineered")))
hvt = show("planning/extrapolation_2026-09-24/fetch/G10/sources/metal_harvey_sf_809500.txt").split("\n")
claim("D04", "Harvey prints Finishing at .1 x Dia radial, 3 x Dia axial (line 17)",
      "Finishing" in hvt[16] and ".1 x Dia" in hvt[16] and "3 x Dia" in hvt[16])

# --- (e) moves
mv = R["moves"]
claim("E01", "every candidate moves all 32 flat cells by more than 5 %",
      all(v["gt5pct"] == 32 for v in mv.values()))
m = mv["anchored_series_law|feed_sfm_rpm"]
claim("E02", "anchored law at the SFM rpm: feed x1.77-x3.28, 28 of 32 cells at 2x or more, 12 at the cap",
      near(m["ratio_min"], 1.77, 0.01) and near(m["ratio_max"], 3.28, 0.01) and m["ge2x"] == 28 and m["at_cap"] == 12)
m = mv["anchored_series_law|feed_vendor_rpm"]
claim("E03", "anchored law at 18 000 rpm: 20 of 32 cells at the 4000 cap", m["at_cap"] == 20)
l6 = c6h["laws"]
claim("E04", "6 mm hardwood Trace: pooled law chip 0.170 (1.33 x the Spektra row 0.127); anchored 0.127",
      near(l6["class_D/point"]["chip"], 0.1695, 0.0005) and near(l6["anchored_series_law"]["chip"], 0.127, 1e-9)
      and near(l6["class_D/point"]["chip"] / 0.127, 1.33, 0.01))
claim("E05", "6 mm hardwood Trace, anchored law: feed 2694 at 10610 rpm, 4000 (cap) at 18000 rpm",
      l6["anchored_series_law"]["feed_sfm_rpm"] == 2694 and l6["anchored_series_law"]["feed_vendor_rpm"] == 4000)

print(f"{'FAIL' if FAILS else 'ALL PASS'}: {len(FAILS)} failed")
sys.exit(1 if FAILS else 0)
