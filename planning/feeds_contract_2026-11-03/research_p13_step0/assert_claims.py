"""Check every numeric or factual claim of index.html (and the findings text). PASS/FAIL per ID.

Run: python3 -I assert_claims.py   Exit 1 on any FAIL. Reads git show 36fd50c54:<path>, the data
files of this folder, and recomputes with shipped_laws.py.
"""
import csv
import json
import math
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", "..", ".."))
sys.path.insert(0, HERE)
import shipped_laws as L  # noqa: E402

REV = "36fd50c54"
J = lambda n: json.load(open(os.path.join(HERE, n)))  # noqa: E731
res, vp, fw, inv, pc, cli = (J("results.json"), J("vendor_points.json"), J("force_width.json"),
                             J("inventory.json"), J("port_check.json"), J("cli_points.json"))
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


def readf(rel):
    return open(os.path.join(REPO, rel), encoding="utf-8", errors="replace").read()


# C01: the binary's code is the code at 36fd50c54 (later commits touched planning/ only)
diff = subprocess.run(["git", "-C", REPO, "diff", "--quiet", REV, "9ca624b13", "--", "crates", "Cargo.toml",
                       "Cargo.lock"]).returncode
claim("C01", "9ca624b13 changes nothing under crates/ against 36fd50c54", diff == 0)

# C02: the port matches the shipped code
claim("C02", f"port checks {len(pc)} >= 50, all within tolerance, include rpm, feed, power, deflection, FM1 force",
      len(pc) >= 50 and all(c["ok"] for c in pc)
      and {"rpm", "feed_mm_min", "power_kw_slot_case", "force_n_peak"} <= {c["what"] for c in pc}
      and any(c["what"].startswith("deflection_edge") for c in pc))

# C03: FM1 D 6 hardwood Pocket force 15.3253, power 0.0600, feed 3999 of the 4000 cap
fm1 = list(csv.DictReader(open(os.path.join(REPO, "crates/rs_cam_core/tests/fixtures/feeds_matrix_2026-09-23/matrix_2026-09-23.csv"))))
row = next(r for r in fm1 if r["tool_type"] == "EndMill" and r["diameter_mm"] == "6.0000" and r["operation"] == "Pocket"
           and r["material"] == "hardwood")
gm = show("crates/rs_cam_core/src/machine/mod.rs")
claim("C03", "FM1 D6 hardwood Pocket: force_n 15.3253, power_kw 0.0600, feed 3999 under max_feed 4000",
      row["force_n"] == "15.3253" and row["power_kw"] == "0.0600" and row["feed_mm_min"] == "3999.0000"
      and "max_feed_mm_min: 4000.0," in gm)

# C04: Pocket power case is the slot at ae = D
sp = cli["fm1_hw6"]["shipped_power"]
claim("C04", "Pocket shipped power case is slot at case_ae_mm = D", sp["case"] == "slot" and sp["case_ae_mm"] == 6.0
      and "A slot (ae = D): `FirstPass::Slot` or `Radial::Slot`." in show("crates/rs_cam_core/src/feeds/engagement.rs"))

# C05: 7.5 % of 0.8 kW
claim("C05", "FM1 point power is 7.5 % of 0.8 kW", near(float(row["power_kw"]) / 0.8, 0.075, 1e-6)
      and "power: PowerModel::ConstantPower { power_kw: 0.8 }," in gm)

# C06: every vendor point inside 0.8 kW and 200 um tool; max power 0.548; max tool deflection 17.2 um
claim("C06", "9 vendor points, all inside 0.8 kW and 200 um; max power 0.548 kW; max tool deflection 17.2 um",
      len(vp) == 9 and all(p["inside_generic_0p8kw"] and p["inside_tool_200um"] for p in vp)
      and near(max(p["power_kw"] for p in vp), 0.548, 0.0005)
      and near(max(p["tool_deflection_mm"] for p in vp) * 1000, 17.2, 0.05)
      and all(p["inside_makita_0p71kw"] for p in vp if p["id"].startswith("c3d"))
      and "power: PowerModel::ConstantPower { power_kw: 0.71 }," in gm)

# C07: implied k ranges (hobby 71-182, industrial 309-569) at 200 um
hk = [p["implied_k_n_per_mm_at_200um"] for p in vp if p["id"].startswith(("sienci", "c3d"))]
ik = [p["implied_k_n_per_mm_at_200um"] for p in vp if not p["id"].startswith(("sienci", "c3d"))]
claim("C07", "implied k: hobby charts 71-182 N/mm, industrial charts 309-569 N/mm",
      round(min(hk)) == 71 and round(max(hk)) == 182 and round(min(ik)) == 309 and round(max(ik)) == 569)

# C08: back-solve of the convention
k = res["implied_k"]
claim("C08", "Pocket convention k 79 / 162 / 344 N/mm at 200/100/50 um; adaptive 101; vendor slot 322; stickout 20-60 mm: 78-90",
      round(k["t0.2"]["pocket_0.25D_0.35D"]["k_frame_n_per_mm"]) == 79
      and round(k["t0.1"]["pocket_0.25D_0.35D"]["k_frame_n_per_mm"]) == 162
      and round(k["t0.05"]["pocket_0.25D_0.35D"]["k_frame_n_per_mm"]) == 344
      and round(k["t0.2"]["adaptive_0.33D_0.25D"]["k_frame_n_per_mm"]) == 101
      and round(k["t0.2"]["vendor_slot_1D"]["k_frame_n_per_mm"]) == 322
      and round(k["stickout20_t0.2"]["k_frame_n_per_mm"]) == 78 and round(k["stickout60_t0.2"]["k_frame_n_per_mm"]) == 90)
# recompute one point independently
ks, fe = L.force_line("GenericHardwood")
_, pk = L.edge_force(1.5, 2.1, 6.0, 2, 3999.823 / (15022 * 2), ks, fe)
claim("C08b", "recomputed peak at the FM1 point equals 15.33 N", near(pk, 15.33, 0.01))

# C09: force nearly flat with width at ap 1.5
r = {x["ae_fr"]: x for x in fw["rows"]}
claim("C09", "peak 15.33 N at 0.35 D, 15.92 N at D (+3.9 %); MRR ratio 2.86",
      near(r[0.35]["peak_n"], 15.33, 0.005) and near(r[1.0]["peak_n"], 15.92, 0.005)
      and near(r[1.0]["peak_n"] / r[0.35]["peak_n"] - 1, 0.039, 0.001) and near(r[1.0]["mrr"] / r[0.35]["mrr"], 2.86, 0.01))

# C10: with a frame spring the optimum is full width
fe_ = res["frame_e"]
claim("C10", "frame spring k_conv/x2/x4/x10: optimum ae = D; k_conv gives ap 0.2 D",
      all(v["best"]["ae_fr"] == 1.0 for v in fe_.values()) and fe_["k_conv"]["best"]["ap_fr"] == 0.2
      and round(fe_["k_conv"]["k"]) == 79)

# C11: computed-only optimum ae = D for all cells; per-pass power hw12 0.95 D
c = res["cells"]
claim("C11", "computed-limit optimum ae = D in all 5 cells; hw12 ap 0.85 D; per-pass power hw12 ae 0.95 D",
      all(v["computed_only"]["ae_fr"] == 1.0 for v in c.values()) and c["hw12"]["computed_only"]["ap_fr"] == 0.85
      and c["hw12"]["per_pass_power"]["ae_fr"] == 0.95
      and all(v["per_pass_power"]["ae_fr"] == 1.0 for kk, v in c.items() if kk != "hw12"))

# C12: sweeps
claim("C12", "20 cap x power sweeps, all optimum ae = D", len(res["sweeps"]) == 20
      and all(v["ae_fr"] == 1.0 for v in res["sweeps"].values()))

# C13: convention vs optimum on hw6
claim("C13", "hw6: convention 12 599, computed optimum 144 000 (11.4x), typed-cap optimum 36 000 at (D, 0.25 D) (2.86x)",
      round(c["hw6"]["convention"]["mrr"]) == 12599 and round(c["hw6"]["computed_only"]["mrr"]) == 144000
      and near(c["hw6"]["computed_only"]["mrr"] / c["hw6"]["convention"]["mrr"], 11.4, 0.05)
      and round(c["hw6"]["with_typed_caps"]["mrr"]) == 36000 and c["hw6"]["with_typed_caps"]["ae_fr"] == 1.0
      and c["hw6"]["with_typed_caps"]["ap_fr"] == 0.25
      and near(c["hw6"]["with_typed_caps"]["mrr"] / c["hw6"]["convention"]["mrr"], 2.86, 0.01))

# C14: corner coverage bound
cg = fw["corner"]
claim("C14", "offset corner coverage: no gap below s = 0.854 D, a gap above",
      near(cg["limit_s_over_d"], 0.854, 0.0005) and cg["gap_below"] <= 1e-9 and cg["gap_above"] > 0)

# C15: the P11c sentry exists at HEAD
claim("C15", "the P11c inverted sentry is at HEAD", "fn no_computed_limit_bounds_the_pocket_width_below_d_p11c" in
      show("crates/rs_cam_core/tests/the_pocket_width_has_no_computed_limit_below_d_p11c.rs"))

# C16: every inventory site holds its needle at the stated line; status counts
ok = True
for g in inv:
    if g["site"] == "no site":
        continue
    path, ln = g["site"].rsplit(":", 1)
    lines = show(path).splitlines()
    if g["needle"] not in lines[int(ln) - 1]:
        ok = False
        print("   site miss", g["id"], g["site"])
from collections import Counter  # noqa: E402
cnt = Counter(g["status"] for g in inv)
claim("C16", "37 inventory rows (8 a, 2 b, 15 c, 12 d); each site has its needle at the line at 36fd50c54",
      ok and len(inv) == 37 and cnt == Counter({"a": 8, "b": 2, "c": 15, "d": 12}))

# C17: no printed machine stiffness in the source captures (no N/um, N/µm, lbf/in, um/N)
pat = re.compile(r"N\s*/\s*(?:u|µ|μ)m|lbf?\s*/\s*in\b|(?:u|µ|μ)m\s*/\s*N\b")
hits = []
for root in ("planning/extrapolation_2026-09-24/fetch", "crates/rs_cam_core/data/vendor_lut/sources"):
    for dp, _, fns in os.walk(os.path.join(REPO, root)):
        for fn in fns:
            if fn.endswith(".txt"):
                t = open(os.path.join(dp, fn), encoding="utf-8", errors="replace").read()
                hits += [fn for _ in pat.finditer(t)]
metric = [h for h in hits if "wood_handbook" not in h]
claim("C17", f"no N/um or um/N in the printed captures; the {len(hits)} lb/in hits are all Wood Handbook ch10 stress units (lb/in2)",
      not metric and all("wood_handbook" in h for h in hits))

# C18: four typed constants are outside F201 (no registry row name names them)
src_names = set()
for f in subprocess.run(["git", "-C", REPO, "ls-tree", "-r", "--name-only", REV, "crates/rs_cam_core/src"],
                        capture_output=True, text=True, check=True).stdout.split():
    if f.endswith(".rs"):
        t = show(f)
        src_names |= set(re.findall(r"FormulaConstant::(?:unsourced|cited)\(\s*\"([^\"]+)\"", t))
bad = [n for n in src_names if re.search(r"helix|youngs|equivalent_diameter|cutting_feed_cap", n, re.I)]
tc = show("crates/rs_cam_core/src/compute/tool_config.rs")
claim("C18", f"no F201 row ({len(src_names)} names) names helix, Young's modulus, the bending fraction or the default cap",
      len(src_names) > 100 and not bad and "fn default_helix_deg() -> f64 {\n    30.0" in tc
      and "CARBIDE_YOUNGS_MODULUS_N_PER_MM2: f64 = 600_000.0" in tc
      and "ENDMILL_EQUIVALENT_DIAMETER_FRACTION: f64 = 0.80" in show("crates/rs_cam_core/src/feeds/predict.rs")
      and "DEFAULT_CUTTING_FEED_CAP_MM_MIN: f64 = 6000.0" in gm)

# C19: Adams belt tier quote; convention and full-width MRR in cm3/min for D6 softwood
ad = readf("planning/extrapolation_2026-09-24/fetch/G6/sources/adams_bits_feeds_and_speeds_guide.txt")
claim("C19", "Adams prints the belt tier 4,000 x 4 x 4 = 64 cm3/min at 100 % width; D6 softwood convention 12.6, full width 0.25 D 36",
      "Belt driven – 4,000 x 4 x 4 = 64 cm³/min" in ad and "The width of cut assumed in the calculator is 100%" in ad
      and near(c["sw6"]["convention"]["mrr"] / 1000, 12.6, 0.05) and near(c["sw6"]["with_typed_caps"]["mrr"] / 1000, 36.0, 0.05))

# C20: printed hobby slot depths 0.39 D (Sienci hardwood) and 0.40 D (Carbide 3D mahogany)
v = {p["id"]: p for p in vp}
claim("C20", "Sienci hardwood slot 2.5 mm on 1/4 in = 0.39 D; Carbide 3D mahogany .1 in = 0.40 D",
      near(v["sienci_quarter_hw"]["ap_over_d"], 0.39, 0.005) and near(v["c3d_s3_mahogany"]["ap_over_d"], 0.40, 0.001))

# C21: each vendor quote's numbers are in the cited source lines
srcchecks = {
    "planning/extrapolation_2026-09-24/fetch/G10/sources/carbide3d_s3_feeds_250.txt": ["Mahogany             .1″    18950", "MDF                  .3″    17000", "Pine                 .4″    21000"],
    "planning/extrapolation_2026-09-24/fetch/G9/sources/community_2402_post1_announcement.txt": ["3 flute .25\" endmill, 100% engagement"],
    "planning/extrapolation_2026-09-24/fetch/G10/sources/sienci_feeds_speeds_metric.txt": ["Stepdown - Slotting"],
    "planning/extrapolation_2026-09-24/fetch/G10/sources/metal_harvey_sf_809500.txt": ["Slot - Rough   .0006   .0013   .0020   .0026   .0033   .0039    .0053    .0079    .0106", "Starting RPM should be between 10,000 and 20,000"],
    "crates/rs_cam_core/data/vendor_lut/sources/amana_spektra_spiral_plunge_v24.txt": ["INDUSTRIAL"],
}
okq = all(s in readf(p) for p, ss in srcchecks.items() for s in ss)
sl = readf("planning/extrapolation_2026-09-24/fetch/G10/sources/sienci_feeds_speeds_metric.txt").split("\n")
okq = okq and re.search(r"2740\s+1370\s+2\.85\s+2\.5\s+5\.6\s+12000", sl[169]) is not None
okq = okq and re.search(r"2740\s+1370\s+2\.85\s+5\.4\s+12\.1\s+12000", sl[78]) is not None
claim("C21", "the vendor numbers used are printed in the cited captures (Sienci lines 79 and 170)", okq)

# C22: force-line coefficients equal the captured Curti (helix 0) and Goli tables
cu = readf("planning/extrapolation_2026-09-24/fetch/G7/sources/marcon2021_generalized_force_model.txt")
go = readf("planning/extrapolation_2026-09-24/fetch/G7/sources/pmc6315737_round_shape_ks.txt")
fl = show("crates/rs_cam_core/src/material/force_line.rs")
claim("C22", "Curti helix-0 quadratics and Goli 31.44/3.36 are in the captures and in force_line.rs; envelope 0.0768333/0.0060333",
      "Kmodel = −5 ⋅ 10−6 GA2 + 1⋅10−3 GA + 26⋅10−3" in cu and "Kmodel = −3⋅10−6 GA2 + 5 ⋅ 10−4 GA + 56⋅10−3" in cu
      and "Intmodel = −1⋅10−7 GA2 + 1⋅10−5 GA + 55⋅10−4" in cu and "Intmodel = −3⋅10−7 GA2 + 4⋅10−5 A + 47⋅10−4" in cu
      and "Ks [N mm−2] | 31.44 (2.68)" in go and "Int [N mm−1] | 3.36 (0.27)" in go
      and "GrainQuadratic::new(-5e-6, 1e-3, 26e-3)" in fl and "pub const GOLI_2018_MDF_KS_N_PER_MM2: f64 = 31.44;" in fl
      and near(L.curti_envelope()[0], 0.0768333, 1e-7) and near(L.curti_envelope()[1], 0.0060333, 1e-7))

# C23: Curti helix-30 rows give Ks_n 0.050 (0.65x helix 0) and an edge term near zero
h30 = {"up_ks": (-3e-6, 5e-4, 21e-3), "down_ks": (-2e-6, 4e-4, 30e-3), "up_int": (7e-8, -1e-5, -6e-5), "down_int": (3e-8, -7e-6, -4e-4)}
printed = ["Kmodel = −3⋅10−6 GA2 + 5⋅10−4 GA + 21⋅10−3", "Kmodel = −2⋅10−6 GA2 + 4⋅10−4 GA + 30⋅10−3",
           "Intmodel = 7⋅10−8 GA2 − 1⋅10−5 GA − 6⋅10−5", "Intmodel = 3⋅10−8 GA2 − 7⋅10−6 GA − 4⋅10−4"]
ks30 = max(L._qmax(h30["up_ks"]), L._qmax(h30["down_ks"]))
int30 = max(L._qmax(h30["up_int"]), L._qmax(h30["down_int"]))
claim("C23", f"helix-30 envelope Ks_n {ks30:.4f} = {ks30 / L.curti_envelope()[0]:.2f} x helix 0; Int_n {int30:.5f} (< 0.1 x helix 0)",
      all(s in cu for s in printed) and near(ks30, 0.050, 0.0005) and near(ks30 / L.curti_envelope()[0], 0.65, 0.01)
      and int30 < 0.1 * L.curti_envelope()[1] and "(Table 5, helix 0)" in fl)

# C24: the page shows every claim ID it cites, and each cited ID is checked here
page = open(os.path.join(HERE, "index.html"), encoding="utf-8").read()
cited = set(re.findall(r"\bC\d\d\b", page))
checked = {f"C{n:02d}" for n in range(1, 25)}
claim("C24", f"every claim ID on the page ({len(cited)}) is checked here", cited <= checked and len(cited) >= 15)

print(f"\n{len(FAILS)} FAIL" if FAILS else "\nall PASS")
sys.exit(1 if FAILS else 0)
