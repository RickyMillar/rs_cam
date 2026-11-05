"""Aggregates over the FM1 matrix (engine answer, generic machine). Usage: python3 -I fm1_stats.py"""
import csv, json, os, collections
REPO = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
P = os.path.join(REPO, "crates/rs_cam_core/tests/fixtures/feeds_matrix_2026-09-23/matrix_2026-09-23.csv")
rows = [r for r in csv.DictReader(open(P))]
ok = [r for r in rows if r["status"] == "ok"]
f = lambda r, k: float(r[k]) if r[k] not in ("", None) else None
out = {"cells": len(rows), "ok": len(ok)}
# 1. hardwood chip above softwood chip, same tool/diameter/flutes/op
key = lambda r: (r["tool_type"], r["diameter_mm"], r["flutes"], r["operation"])
by = collections.defaultdict(dict)
for r in ok:
    by[key(r)][r["material"]] = r
pairs = [(k, v["hardwood"], v["softwood"]) for k, v in by.items() if "hardwood" in v and "softwood" in v]
hw_gt = [(k, f(h, "r_chip_load_mm"), f(s, "r_chip_load_mm"), h["lut_observation_id"], s["lut_observation_id"])
         for k, h, s in pairs if f(h, "r_chip_load_mm") and f(s, "r_chip_load_mm") and f(h, "r_chip_load_mm") > f(s, "r_chip_load_mm") * (1 + 1e-9)]
out["hw_sw_pairs"] = len(pairs)
out["hw_chip_above_sw"] = len(hw_gt)
out["hw_chip_above_sw_examples"] = hw_gt[:8]
# 2. Freud rows in use
fr = [r for r in ok if r["lut_observation_id"].startswith("freud-")]
out["freud_cells"] = len(fr)
out["freud_rows"] = sorted(collections.Counter(r["lut_observation_id"] for r in fr).items())
# 3. depth over D, and depth over 2 D, among Pocket-family roughing with a force line
deep = [r for r in ok if f(r, "depth_per_pass_mm") and f(r, "diameter_mm") and f(r, "depth_per_pass_mm") > 2.0 * f(r, "diameter_mm") + 1e-9]
out["depth_over_2D"] = len(deep)
out["depth_over_2D_examples"] = [(r["tool_type"], r["diameter_mm"], r["operation"], r["material"], r["depth_per_pass_mm"], r["stepover_mm"], r["binding_limit"]) for r in deep[:12]]
# 4. plywood: no force line
ply = [r for r in ok if r["material"] == "plywood"] if any(r["material"] == "plywood" for r in ok) else []
out["materials"] = sorted(collections.Counter(r["material"] for r in rows).items())
out["no_force_line_cells"] = sum(1 for r in ok if not r["force_line"])
# 5. formula-chip cells
out["formula_chip_cells"] = sum(1 for r in ok if r["support_arm"] and r["support_arm"] != "VendorBacked")
out["support_arms"] = sorted(collections.Counter(r["support_arm"] for r in ok).items())
# 6. binding limits of the proposal
out["binding"] = sorted(collections.Counter(r["binding_limit"] for r in ok).items())
json.dump(out, open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "fm1_stats.json"), "w"), indent=1)
print(json.dumps(out, indent=1)[:5000])
