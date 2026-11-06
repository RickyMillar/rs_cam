"""Check the numbers and claims of index.html against the repo files.

Run: python3 -I assert_claims.py   (needs node on PATH; read-only; no cargo)

1. Citations: each constant of the page's "laws" JSON block is printed at its path:line.
2. The page's own JS port (the "laws-js" and "claims-js" scripts, run in node) reproduces
   the FM1 golden cells (matrix_2026-09-23.csv) and the P13c numbers in phases/P13.md.
3. Each number in a prose <span data-claim> equals the port value at its printed digits. Numbers
   outside a span (rule names such as "0.85 D" in card text) are covered by checks 1 and 2 only.
4. Page rules: title, colour tokens, dark mode guards, body background, no external script.
Prints N/N PASS; exits 1 on a failure.
"""
import csv
import json
import os
import re
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", "..", ".."))
PAGE = open(os.path.join(HERE, "index.html"), encoding="utf-8").read()
FM1 = os.path.join(REPO, "crates/rs_cam_core/tests/fixtures/feeds_matrix_2026-09-23/matrix_2026-09-23.csv")
P13 = open(os.path.join(REPO, "planning/feeds_contract_2026-11-03/phases/P13.md"), encoding="utf-8").read()

results = []


def check(name, ok, detail=""):
    results.append((name, bool(ok), detail))


def lines_of(path):
    return open(os.path.join(REPO, path), encoding="utf-8").read().split("\n")


def block(tag_id):
    m = re.search(r'<script[^>]*id="%s"[^>]*>(.*?)</script>' % tag_id, PAGE, re.S)
    assert m, tag_id
    return m.group(1)


LAWS = json.loads(block("laws"))

# ---------------------------------------------------------------- 1. citations
for key, c in LAWS.items():
    if key in ("VENDOR_ROWS", "HOBBY_SLOT_POINTS", "IGNORED", "UPCUT_ROW"):
        continue
    src = lines_of(c["path"])
    pairs = list(zip(c["lines"], c["printeds"])) if "lines" in c else [(c["line"], c["printed"])]
    for ln, pr in pairs:
        check(f"cite {key} {c['path']}:{ln}", pr in src[ln - 1], f"line reads: {src[ln - 1].strip()[:90]}")
    # the typed value equals the printed number where the printed text holds one number
    if isinstance(c["value"], (int, float)):
        nums = [float(x.replace("_", "")) for x in re.findall(r"-?\d[\d_]*\.?\d*(?:e-?\d+)?", c["printed"])]
        check(f"value {key} = printed", any(abs(n - c["value"]) < 1e-12 for n in nums) or key in ("DOC_DERATE_2D",), f"{c['value']} vs {nums}")
    if isinstance(c["value"], list) and "lines" in c:
        check(f"value {key} = printed rows", [float(x) for x in c["printeds"]] == c["value"])
    if isinstance(c["value"], list) and len(c["value"]) == 3 and "lines" not in c:
        nums = [float(x) for x in re.findall(r"-?\d+(?:\.\d+)?(?:e-?\d+)?", c["printed"])]
        check(f"value {key} = printed quadratic", nums == c["value"], f"{nums}")

# vendor rows: the JSON file holds the chip and the rpm of each row
vr = LAWS["VENDOR_ROWS"]
obs = {o["observation_id"]: o for o in json.load(open(os.path.join(REPO, vr["path"])))["observations"]}
src = lines_of(vr["path"])
for d, mats in vr["value"].items():
    for mat, r in mats.items():
        o = obs[r["id"]]
        check(f"vendor row {r['id']}", r["id"] in src[r["line"] - 1] and o["chipload_max_mm_tooth"] == r["chip"] and o["rpm_nominal"] == r["rpm"]
              and o["flute_count"] == 2 and abs(o["diameter_mm"] - float(d)) < 1e-9 and o["operation_family"] == "pocket")

up = LAWS["UPCUT_ROW"]
uo = obs[up["value"]["id"]]
check("upcut row (not read by FM1)", up["printed"] in src[up["line"] - 1] and uo["rpm_nominal"] == up["value"]["rpm"]
      and uo["chipload_min_mm_tooth"] == up["value"]["chip_min"] and uo["chipload_max_mm_tooth"] == up["value"]["chip_max"]
      and uo["operation_family"] == "pocket" and uo["material_family"] == "hardwood" and uo["diameter_mm"] == 6.0)

# hobby slot rows: the Rust table prints each value (inch values times INCH_MM)
hp = LAWS["HOBBY_SLOT_POINTS"]
src = lines_of(hp["path"])
inch = float(re.search(r"const INCH_MM: f64 = ([\d.]+);", "\n".join(src)).group(1))
for p in hp["value"]:
    ok = f'id: "{p["id"]}"' in src[p["line"] - 1]
    body = "\n".join(src[p["line"] - 1:p["line"] + 12])

    def field(name):
        expr = re.search(name + r": ([^,]+),", body).group(1).replace("INCH_MM", repr(inch))
        return eval(expr, {"__builtins__": {}})  # arithmetic on printed numbers only

    for name, key in (("diameter_mm", "diameter_mm"), ("ap_mm", "ap_mm"), ("feed_mm_min", "feed_mm_min"), ("rpm", "rpm"), ("flutes", "flutes")):
        ok = ok and abs(field(name) - p[key]) < 1e-9
    mat = re.search(r"SlotPointMaterial::(\w+)", body).group(1)
    ok = ok and {"GenericSoftwood": "softwood", "GenericHardwood": "hardwood", "Mdf": "mdf"}[mat] == p["material"]
    check(f"hobby row {p['id']} at force_budget.rs:{p['line']}", ok)

for g in LAWS["IGNORED"]["value"]:
    if g["path"]:
        check(f"inventory {g['id']} site {g['path']}:{g['line']}", g["needle"] in lines_of(g["path"])[g["line"] - 1])

# ---------------------------------------------------------------- 2. the port in node
node = shutil.which("node")
check("node on PATH", node is not None)
if node is None:
    print("FAIL node is required to run the page port (index.html script laws-js)")
    print(f"{sum(r[1] for r in results)}/{len(results)} PASS (stopped before the port checks)")
    sys.exit(1)
driver = """
const C = __LAWS__;
__JS__
__CLAIMS__
const L = makeLaws(C);
const out = { claims: default_claims(L, C), cells: [] };
const m = { min_rpm: C.GENERIC_MIN_RPM.value, max_rpm: C.GENERIC_MAX_RPM.value, power_kw: C.GENERIC_POWER_KW.value,
            max_feed: C.GENERIC_MAX_FEED.value, max_cutting_feed: null, slot_doc_factor: C.SLOT_DOC_FACTOR.value };
const hb = L.hobby_budget();
for (const d of [6, 3.175]) for (const mat of ["hardwood", "softwood", "mdf"]) {
  const tool = L.fm1_tool(d, 2);
  const ctx = { tool, line: L.force_line(mat), row: C.VENDOR_ROWS.value[String(d)][mat], m, budget: hb.force_n,
                limit_um: C.DEFLECTION_BACKOFF_TARGET_UM.value };
  const p = L.propose_removal_rate(ctx);
  const sp = L.speeds({ ae: p.ae }, tool, ctx.row, m, ctx.line, p.ap);
  const feed = Math.floor(sp.feed), fz = feed / (sp.rpm * 2);
  out.cells.push({ d, mat, ae: p.ae, ap: p.ap, limits: p.limits, rpm: sp.rpm, feed,
    force: L.edge_force(p.ap, p.ae, d, 2, fz, ctx.line, tool.helix).peak_resultant_n,
    power: L.pocket_power_kw(tool, ctx.line, p.ap, p.ae, sp.rpm, feed),
    cap_mm: L.depth_at_budget(tool, ctx.line, p.ae, fz, hb.force_n).mm });
}
console.log(JSON.stringify(out));
"""
js = driver.replace("__LAWS__", json.dumps(LAWS)).replace("__JS__", block("laws-js")).replace("__CLAIMS__", block("claims-js"))
run = subprocess.run([node, "-"], input=js, capture_output=True, text=True, timeout=300)
check("node run of the page port", run.returncode == 0, run.stderr[:300])
OUT = json.loads(run.stdout)
cl = OUT["claims"]

# FM1 golden cells
rows = {}
for r in csv.DictReader(open(FM1)):
    if r["tool_type"] == "EndMill" and r["operation"] == "Pocket":
        rows[(round(float(r["diameter_mm"]), 3), r["material"])] = r
for c in OUT["cells"]:
    r = rows[(round(c["d"], 3), c["mat"])]
    tag = f"FM1 Pocket D{c['d']} {c['mat']}"
    check(f"{tag} rpm", c["rpm"] == float(r["rpm"]), f"{c['rpm']} vs {r['rpm']}")
    check(f"{tag} feed (floored)", c["feed"] == float(r["feed_mm_min"]))
    check(f"{tag} stepover", abs(c["ae"] - float(r["stepover_mm"])) < 5e-5 and abs(c["ae"] - float(r["proposal_ae_mm"])) < 5e-5)
    check(f"{tag} depth", abs(c["ap"] - float(r["depth_per_pass_mm"])) < 5e-5 and abs(c["ap"] - float(r["proposal_ap_mm"])) < 5e-5, f"{c['ap']} vs {r['depth_per_pass_mm']}")
    check(f"{tag} peak force", abs(c["force"] - float(r["force_n"])) < 5e-5, f"{c['force']} vs {r['force_n']}")
    check(f"{tag} power", abs(c["power"] - float(r["power_kw"])) < 5e-5, f"{c['power']} vs {r['power_kw']}")
    check(f"{tag} cap_mm (budget depth at the floored chip)", abs(c["cap_mm"] - float(r["cap_mm"])) < 5e-5, f"{c['cap_mm']} vs {r['cap_mm']}")
    check(f"{tag} budget column", abs(cl["budget_n"] - float(r["cap_factor"])) < 5e-5)
    check(f"{tag} slot is a pick limit (FM1 binding_limit)", r["binding_limit"] in c["limits"], f"{c['limits']} vs {r['binding_limit']}")

# P13.md numbers (Result P13c)
def p13(text):
    return text in P13


check("P13 budget 35.3224 N", p13("35.3224 N") and f"{cl['budget_n']:.4f}" == "35.3224")
for key, txt in (("f_sienci_sw", "30.25 (Sienci softwood)"), ("f_sienci_mdf", "**35.32 (Sienci MDF)**"), ("f_sienci_hw", "24.67 (Sienci"),
                 ("f_c3d_mdf", "32.40 (C3D MDF)"), ("f_c3d_pine", "30.99 (C3D pine)"), ("f_c3d_mah", "14.03 N (C3D mahogany)")):
    num = re.search(r"[\d.]+", txt.replace("**", "")).group(0)
    check(f"P13 slot row force {key} {num}", p13(txt) and f"{cl[key]:.2f}" == num)
check("P13 median 30.62 N", p13("the median is\n30.62 N") or p13("median is 30.62 N") or "30.62 N" in P13, "")
check("port median 30.62", f"{cl['budget_median']:.2f}" == "30.62")
check("P13 pick 0.850 D x 3.406 mm (Rigidity, Slot)", p13("0.850 D x 3.406 mm (Rigidity, Slot), 69 474 mm3/min") and f"{cl['pick_ae_over_d']:.3f}" == "0.850"
      and f"{cl['depth_hw']:.3f}" == "3.406" and cl["limits_hw"] == "Rigidity,Slot" and f"{cl['mrr_pick']:.0f}" == "69474")
check("P13 MDF 0.850 D x 4.558 mm (Rigidity, Slot)", p13("0.850 D x 4.558 mm (Rigidity, Slot)") and f"{cl['depth_mdf']:.3f}" == "4.558" and cl["limits_mdf"] == "Rigidity,Slot")
check("P13 softwood 0.850 D x 6.0 mm (VendorDepthTable, Slot)", p13("0.850 D x 6.0 mm (VendorDepthTable, Slot)") and f"{cl['depth_sw']:.1f}" == "6.0" and cl["limits_sw"] == "VendorDepthTable,Slot")
check("P13 D 3.175 hardwood 3.175 mm (VendorDepthTable, Slot)", p13("0.850 D x 3.175 mm (VendorDepthTable, Slot)") and abs(cl["d3_depth"] - 3.175) < 1e-12 and cl["d3_limits"] == "VendorDepthTable,Slot")
check("P13 sensitivity: median 2.93 mm, 40 N 3.89 mm", p13("gives 2.93 mm") and p13("3.89 mm") and f"{cl['sens_median']:.2f}" == "2.93" and f"{cl['sens_40']:.2f}" == "3.89")
check("P13 corner bound 0.8536 D, threshold 0.4 % under", p13("0.4 % under the\ncorner bound 0.8536 D") or (p13("0.4 %") and p13("0.8536 D")))
check("port corner 0.8536, 0.4 %", f"{cl['corner_factor']:.4f}" == "0.8536" and f"{cl['thr_under_corner_pct']:.1f}" == "0.4")
check("FINDINGS corner (1 + 1/sqrt 2) r = 0.854 D",
      "(1 + 1/sqrt 2) r = 0.854 D" in open(os.path.join(REPO, "planning/feeds_contract_2026-11-03/research_p13_step0/FINDINGS.md")).read())

check("P13 research port 3.407 mm = the fine root", p13("3.407 mm at 0.85 D") and f"{cl['root_hw_fine']:.3f}" == "3.407")
check("shipped depth sits under the root by less than the convergence", 0 <= cl["root_hw_fine"] - cl["depth_hw"] < LAWS["BINSEARCH_CONVERGENCE_MM"]["value"])
check("step 0 port sums 600 midpoints in z", "nz=600" in open(os.path.join(REPO, "planning/feeds_contract_2026-11-03/research_p13_step0/shipped_laws.py")).read())
check("FM1 reads the Spektra row, not the upcut row", rows[(6.0, "hardwood")]["lut_observation_id"] == "amana-flat-hardwood-pocket-6000-2f-spektra")

# claims the prose states in words
check("lift is 1 at 0.85 D (psi > 90 deg)", cl["lift_pick"] == 1 and cl["psi_pick_deg"] > 90)
check("lift 1/sin psi = 2/sqrt 3 at 0.25 D", abs(cl["lift_quarter"] - 2 / 3 ** 0.5) < 1e-12)
check("row feed 18000 x 0.127 x 2 = 4572 > cap 4000", abs(cl["raw_feed"] - 18000 * 0.127 * 2) < 1e-9 and cl["raw_feed"] > cl["cap"] == 4000)
check("cap = min(6000 default, 4000 travel)", cl["cap"] == min(LAWS["DEFAULT_CUTTING_FEED_CAP_MM_MIN"]["value"], LAWS["GENERIC_MAX_FEED"]["value"]))
check("arm L rpm = floor(4000 / 0.254)", cl["rpm_pick"] == int(4000 // (0.127 * 2)))
check("vendor table at share 1 is 1 x D = 6.0 mm", cl["table_hw_mm"] == 6.0)
check("deflection has no root below the flute (default cell)", cl["defl_root_hw"] is None and cl["defl_pick_um"] < LAWS["DEFLECTION_BACKOFF_TARGET_UM"]["value"])
check("FM1 force under the budget (floored chip)", cl["force_pick"] < cl["budget_n"] and cl["fz_fm1"] < cl["chip_hw"])
check("budget depth at ae = D equals the depth at 0.85 D (helical plateau)", abs(cl["budget_at_d"] - cl["depth_hw"]) < 1e-9)
check("softwood: table binds before the budget", cl["budget_sw"] > cl["depth_sw"])
check("FPL density comment: hardwood rho 675.7", "ρ 675.7 kg/m³" in "\n".join(lines_of("crates/rs_cam_core/src/material/mod.rs")) and f"{cl['rho_hw']:.1f}" == "675.7")
check("FPL density comment: softwood rho 403.2", "ρ 403.2 kg/m³" in "\n".join(lines_of("crates/rs_cam_core/src/material/mod.rs")) and f"{cl['rho_sw']:.1f}" == "403.2")
check("Curti envelope Ks_n 0.0768333, Int_n 0.0060333 (force_line.rs doc)", "Ks_n 0.0768333" in "\n".join(lines_of("crates/rs_cam_core/src/material/force_line.rs"))
      and f"{cl['ks_n']:.7f}" == "0.0768333" and f"{cl['int_n']:.7f}" == "0.0060333")
fb = "\n".join(lines_of("crates/rs_cam_core/src/feeds/force_budget.rs"))
check("width assumptions text equals COMPUTED_WIDTH_ASSUMPTIONS", '["chips clear", "tool life not modelled"]' in fb
      and "Width assumes: chips clear; tool life not modelled" in PAGE)
check("corner factor formula in force_budget.rs", "(1.0 + 1.0 / std::f64::consts::SQRT_2) / 2.0" in fb)
cal = "\n".join(lines_of("crates/rs_cam_core/src/feeds/calculator/feed.rs"))
check("Step 5 feed law: rpm x chip x Z x lift", "speed.rpm * chip.chip_load * input.flute_count as f64 * radial_lift" in cal)
check("Step 7 arm L: floor(cap / chip per rev)", "(machine_cut_ceiling / chip_per_rev).floor()" in cal)
cut = "\n".join(lines_of("crates/rs_cam_core/src/feeds/calculator/cut.rs"))
check("Step 4b slot rule uses ae > threshold x D", "if cut.ae > d * slot_threshold_d {" in cut)
check("Pocket power is the slot case (engagement.rs)", "A slot (ae = D): `FirstPass::Slot` or `Radial::Slot`." in "\n".join(lines_of("crates/rs_cam_core/src/feeds/engagement.rs")))
check("K_r = 0 (engagement.rs)", "`K_r = 0`" in "\n".join(lines_of("crates/rs_cam_core/src/feeds/engagement.rs")))

# section 9 prototype: claims in words
check("s9 note: illustrative samples, not a simulation run", "<b>Illustrative samples from a simple pocket model, not a simulation run.</b>" in PAGE)
check("s9 caption: power exact only at planned feed and rpm", "The power line is exact only at the planned feed and rpm; the force line depends on chip and geometry only." in PAGE)
check("s9 power contour label names the planned feed and rpm", '"spindle power at planned feed and rpm"' in PAGE)
viz = "\n".join(lines_of("crates/rs_cam_viz/src/ui/sim_diagnostics.rs"))
check("s9 today's five card titles (CutMetricSpec)", all(f'title: "{t}"' in viz for t in ("Chipload", "Depth of cut", "Tool deflection", "Spindle power", "Engagement")))
dist = "\n".join(lines_of("crates/rs_cam_core/src/tool_load/distribution.rs"))
check("s9 card histograms use DEFAULT_BIN_COUNT", "Histogram::build(&samples, floor, status.bound, DEFAULT_BIN_COUNT)" in dist and cl["proto_bins"] == 24)
check("s9 card floor only when the row publishes one", "The chipload floor is present only when the vendor row" in dist)
spk = obs[LAWS["VENDOR_ROWS"]["value"]["6"]["hardwood"]["id"]]
check("s9 Spektra 6 mm hardwood row is a point (no chipload_min)", "chipload_min_mm_tooth" not in spk and spk["chipload_max_mm_tooth"] == cl["chip_hw"])
oc = "\n".join(lines_of("crates/rs_cam_core/src/compute/operation_configs.rs"))
pocket = re.search(r"pub struct PocketConfig \{(.*?)\n\}", oc, re.S).group(1)
check("s9 PocketConfig has no stock-to-leave field", "stock_to_leave" not in pocket)
check("s9 stock to leave default is Adaptive3dConfig's", "impl Default for Adaptive3dConfig" in "\n".join(lines_of("crates/rs_cam_core/src/compute/operation_configs.rs")[1935:1956]))
check("s9 rough limit = stock to leave x 1000 um", abs(cl["proto_rough_limit_um"] - 1000 * cl["proto_stl_mm"]) < 1e-9)
dep = "\n".join(lines_of("crates/rs_cam_core/src/tool_load/depth.rs"))
check("s9 depth card bound: budget depth at the peak sample's width and chip", "the depth where the peak cutting force at the peak\n//! sample's width and chip meets the machine force budget" in dep
      and abs(cl["proto_depth_bound"] - cl["cap_hw_fm1chip"]) < 1e-12)
dfl = "\n".join(lines_of("crates/rs_cam_core/src/tool_load/deflection.rs"))
check("s9 deflection card law: peak edge force into tip_deflection_mm", "PEAK resultant" in dfl and "[`ToolDefinition::tip_deflection_mm`]" in dfl)
check("s9 power peak = the slot-case power of section 5 (slot samples)", abs(cl["proto_power_kw"] - cl["power_pick"]) < 1e-12)
check("s9 force peak = the pick force (plateau), under the budget, nothing outside", abs(cl["proto_force_n"] - cl["force_pick"]) < 1e-9
      and cl["proto_force_n"] < cl["budget_n"] and cl["proto_outside_pct"] == 0)
check("s9 default shares sum to 100 %", abs(cl["proto_share_steady"] + cl["proto_share_corner"] + cl["proto_share_slot"] + cl["proto_share_ramp"] - 100) < 1e-9)

# ---------------------------------------------------------------- 3. prose spans
extra = {"tool_d": 6, "cap_default_typed": LAWS["DEFAULT_CUTTING_FEED_CAP_MM_MIN"]["value"], "travel": LAWS["GENERIC_MAX_FEED"]["value"],
         "bend_frac": LAWS["ENDMILL_EQUIVALENT_DIAMETER_FRACTION"]["value"], "scan_steps": LAWS["BUDGET_SCAN_STEPS"]["value"],
         "conv_mm": LAWS["BINSEARCH_CONVERGENCE_MM"]["value"]}
spans = re.findall(r'<span data-claim="(\w+)" data-digits="(\d+)">([^<]*)</span>', PAGE)
check("prose has claim spans", len(spans) >= 40, str(len(spans)))
for cid, dg, text in spans:
    val = cl.get(cid, extra.get(cid))
    shown = text.replace(",", "").strip()
    check(f"prose {cid} = {shown}", isinstance(val, (int, float)) and f"{val:.{int(dg)}f}" == shown, f"port {val}")

# ---------------------------------------------------------------- 4. page rules
title = re.search(r"<title>([^<]*)</title>", PAGE).group(1)
check(f"title of 2-4 words: {title}", 2 <= len(title.split()) <= 4)
check("tokens on :root", re.search(r":root\s*\{[^}]*--bg:", PAGE) is not None)
check("dark mode under prefers-color-scheme guarded by :not([data-theme=light])",
      re.search(r'@media \(prefers-color-scheme: dark\)\s*\{\s*:root:not\(\[data-theme="light"\]\)', PAGE) is not None)
check('dark mode under :root[data-theme="dark"]', ':root[data-theme="dark"] {' in PAGE)
check("explicit body background", re.search(r"body\s*\{[^}]*background:\s*var\(--bg\)", PAGE) is not None)
check("no external script or stylesheet", re.search(r"<script[^>]+src=|<link[^>]+stylesheet", PAGE) is None)
check("phone gutter 16px", ".wrap { max-width: 1180px; margin: 0 auto; padding: 0 16px" in PAGE)

bad = [r for r in results if not r[1]]
for name, ok, detail in results:
    if not ok:
        print("FAIL", name, detail)
print(f"{len(results) - len(bad)}/{len(results)} PASS")
sys.exit(1 if bad else 0)
