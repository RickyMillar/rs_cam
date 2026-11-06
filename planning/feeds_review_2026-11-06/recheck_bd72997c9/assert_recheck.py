"""One assert per number of ../RECHECK_bd72997c9.md (feeds review recheck at HEAD bd72997c9).

Run: python3 -I assert_recheck.py   (read-only; no cargo)

1. Each claim re-derives from a stored engine output in this folder (cells.json,
   power_cells.json, runs/*.jsonl, gate_mcp.json, postsim/*_art/out.nc), from the old review
   outputs in ../matrix and ../runs (the "was" values), or from a printed source line.
2. The shown text of each claim is in RECHECK_bd72997c9.md.
Prints N/N PASS; exits 1 on a failure.
"""
import json
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
OLD = os.path.abspath(os.path.join(HERE, ".."))
REPO = os.path.abspath(os.path.join(HERE, "..", "..", ".."))
sys.path.insert(0, OLD)
import reference  # noqa: E402

DOC = open(os.path.join(OLD, "RECHECK_bd72997c9.md"), encoding="utf-8").read()
IN = 25.4
_c = {}


def js(p):
    if p not in _c:
        _c[p] = json.load(open(p))
    return _c[p]


def new(cid):
    return js(os.path.join(HERE, "cells.json"))[cid]["rec"]


def old(cid):
    return js(os.path.join(OLD, "matrix", "cells.json"))[cid]["rec"]


def calc(rec, k):
    return rec["profile"]["calculator"][k]


def prop(rec, k):
    return (rec["profile"]["envelope"]["proposal"].get("value") or {}).get(k)


def band(rec):
    return [(m["vendor"], m["chip_mid_mm"]) for m in rec["profile"]["vendor_band"]["members"]]


def jsonl(path, name):
    for line in open(path):
        r = json.loads(line)
        if r["toolpath_name"].startswith(name):
            return r
    raise KeyError(name)


def fix_new(name, f="nz_wanaka_21"):
    return jsonl(os.path.join(HERE, "runs", f"suggest_{f}.jsonl"), name)


def fix_old(name, f="nz_wanaka_21"):
    return jsonl(os.path.join(OLD, "runs", f"suggest_{f}.jsonl"), name)


def gate(cid, k, which="new"):
    p = os.path.join(HERE, "gate_mcp.json") if which == "new" else os.path.join(OLD, "matrix", "gate_mcp.json")
    return js(p)[cid][k]


def gcode(cid):
    return open(os.path.join(HERE, "postsim", f"{cid}_art", "out.nc")).read().split("\n")


def fact(k, f):
    return reference.FACTS[k][3][f]


def line_has(path, n, needle):
    return needle in open(os.path.join(REPO, path), encoding="utf-8").read().split("\n")[n - 1]


A635O, A635H = "A_flat_6.350_2f_whiteoak", "A_flat_6.350_2f_hardwood"
CLAIMS = [
    # head
    ("head", lambda: open(os.path.join(HERE, "runs", "HEAD.txt")).read().strip()[:9], "{}"),
    # D1
    ("d1_face5_new", lambda: calc(fix_new("Face 5", "rivmap100"), "chip_load_mm"), "{:.3f}"),
    ("d1_face5_old", lambda: calc(fix_old("Face 5", "rivmap100"), "chip_load_mm"), "{:.4f}"),
    ("d1_spektra", lambda: fact("spektra_2f_6mm", "wood_chip_in") * IN, "{:.3f}"),
    ("d1_oak6_old", lambda: calc(old("A_flat_6.000_2f_whiteoak"), "chip_load_mm"), "{:.4f}"),
    # D2
    ("d2_row_new", lambda: calc(new(A635O), "vendor_source"), "{}"),
    ("d2_chip_new", lambda: calc(new(A635O), "chip_load_mm"), "{:.3f}"),
    ("d2_rpm_new", lambda: new(A635O)["suggested"]["spindle_rpm"], "{:d}"),
    ("d2_row_old", lambda: calc(old(A635O), "vendor_source"), "{}"),
    ("d2_chip_old", lambda: calc(old(A635O), "chip_load_mm"), "{:.4f}"),
    ("d2_rpm_old", lambda: old(A635O)["suggested"]["spindle_rpm"], "{:d}"),
    ("d2_band_hi", lambda: max(c for _, c in band(new(A635O))), "{:.4f}"),
    ("d2_band_mid", lambda: sorted(c for _, c in band(new(A635O)))[1], "{:.4f}"),
    # D3
    ("d3_ap", lambda: prop(new("A_flat_3.175_2f_hardwood"), "ap_mm"), "{:.3f}"),
    ("d3_ae", lambda: prop(new("A_flat_3.175_2f_hardwood"), "ae_mm"), "{:.2f}"),
    ("d3_chip", lambda: calc(new("A_flat_3.175_2f_hardwood"), "chip_load_mm"), "{:.4f}"),
    ("d3_budget", lambda: gate("P_r37_pick", "depth_budget_mm"), "{:.2f}"),
    ("d3_fz", lambda: gate("P_r37_pick", "depth_fz_mm"), "{:.4f}"),
    ("d3_ratio", lambda: gate("P_r37_pick", "achieved_median_ratio"), "{:.2f}"),
    ("d3_touched", lambda: gate("P_r37_pick", "modulation_moves_touched"), "{:d}"),
    ("d3_emit", lambda: [int(m) for m in re.findall(r"F(\d+)", "\n".join(gcode("P_r37_pick")))][1], "{:d}"),
    ("d3_sienci_peak", lambda: gate("P_r37_sienci", "depth_peak_mm"), "{:.1f}"),
    ("d3_sienci_budget", lambda: gate("P_r37_sienci", "depth_budget_mm"), "{:.2f}"),
    ("d3_sienci_old_budget", lambda: gate("P_r37_sienci", "depth_budget_mm", "old"), "{:.2f}"),
    ("d3_sienci_old_fz", lambda: gate("P_r37_sienci", "depth_fz_mm", "old"), "{:.4f}"),
    # D4
    ("d4_ap", lambda: prop(new(A635H), "ap_mm"), "{:.3f}"),
    ("d4_sienci", lambda: fact("sienci_hw_quarter_full", "slot"), "{:.1f}"),
    ("d4_c3d", lambda: fact("c3d_mahogany", "doc_in") * IN, "{:.2f}"),
    # D5
    ("d5_ae", lambda: prop(new("A_flat_6.350_2f_plywood"), "ae_mm"), "{:.2f}"),
    ("d5_ae_ratio", lambda: prop(new("A_flat_6.350_2f_plywood"), "ae_mm") / 6.35, "{:.4f}"),
    ("d5_ap_old", lambda: prop(old("A_flat_6.350_2f_plywood"), "ap_mm"), "{:.2f}"),
    ("d5_row_new", lambda: calc(new("A_flat_6.350_2f_plywood"), "vendor_source"), "{}"),
    ("d5_unmodeled", lambda: new("A_flat_6.350_2f_plywood")["profile"]["shipped_power"]["unmodeled"], "{}"),
    # D6
    ("d6_budget", lambda: gate("P_thin_005", "depth_budget_mm"), "{:.3f}"),
    ("d6_fz", lambda: gate("P_thin_005", "depth_fz_mm"), "{:.4f}"),
    ("d6_old_budget", lambda: gate("P_thin_005", "depth_budget_mm", "old"), "{:.2f}"),
    ("d6_old_fz", lambda: gate("P_thin_005", "depth_fz_mm", "old"), "{:.4f}"),
    ("d6_fb_share", lambda: gate("P_thin_005", "binding_force_budget_share"), "{:.3f}"),
    ("d6_first_ring", lambda: int(re.search(r"F(\d+)", gcode("P_thin_005")[12]).group(1)), "{:d}"),
    ("d6_ae", lambda: gate("P_thin_005", "depth_ae_mm"), "{:.1f}"),
    ("gui_build_s", lambda: (lambda m: int(m.group(1)) * 60 + float(m.group(2)))(re.search(r"(\d+):([\d.]+) total", open(os.path.join(HERE, "runs", "gui_build.txt")).read())), "{:.0f}"),
    ("d6_lift", lambda: gate("P_thin_005", "thinning_lift"), "{:.2f}"),
    # D7
    ("d7_kind", lambda: fix_new("River channel")["lineage"]["cut"]["axial"]["kind"], "{}"),
    ("d7_offset", lambda: fix_new("River channel")["lineage"]["cut"]["axial"]["value"], "{:.1f}"),
    ("d7_power", lambda: fix_new("River channel")["profile"]["shipped_power"]["unmodeled"], "{}"),
    # D8
    ("d8_sw_old", lambda: old("A_flat_12.700_2f_softwood")["suggested"]["feed_rate"], "{:.0f}"),
    ("d8_hw_old", lambda: old("A_flat_12.700_2f_hardwood")["suggested"]["feed_rate"], "{:.0f}"),
    ("d8_sw_new", lambda: new("A_flat_12.700_2f_softwood")["suggested"]["feed_rate"], "{:.0f}"),
    ("d8_floor_feed", lambda: 0.025 * 18000 * 2, "{:.0f}"),
    ("d8_sw_kw", lambda: calc(new("A_flat_12.700_2f_softwood"), "power_kw"), "{:.3f}"),
    ("d8_hw_kw", lambda: calc(new("A_flat_12.700_2f_hardwood"), "power_kw"), "{:.3f}"),
    # D9
    ("d9_code", lambda: "TravelRate" if line_has("crates/rs_cam_core/src/machine/mod.rs", 924, "CuttingCeilingSource::TravelRate") else None, "{}"),
    ("d9_3f_rpm_old", lambda: old("A_flat_6.350_3f_softwood")["suggested"]["spindle_rpm"], "{:d}"),
    ("d9_3f_feed_new", lambda: new("A_flat_6.350_3f_softwood")["suggested"]["feed_rate"], "{:.0f}"),
    # D10
    ("d10_new", lambda: calc(fix_new("Lake pocket 2 mm, 8"), "chip_load_mm"), "{:.4f}"),
    ("d10_old", lambda: calc(fix_old("Lake pocket 2 mm, 8"), "chip_load_mm"), "{:.4f}"),
    ("d10_lo", lambda: fact("spektra_2f_sixteenth", "wood_chip_in") * IN, "{:.4f}"),
    ("d10_hi", lambda: fact("spektra_2f_3_32", "wood_chip_in") * IN, "{:.4f}"),
    # D11
    ("d11_feed", lambda: fix_new("Day strip slit V60")["suggested"]["feed_rate"], "{:.0f}"),
    ("d11_old", lambda: fix_old("Day strip slit V60")["suggested"]["feed_rate"], "{:.0f}"),
    ("d11_printed", lambda: 90 * IN, "{:.0f}"),
    # D12
    ("d12_slotdet", lambda: sum(1 for k, v in js(os.path.join(HERE, "cells.json")).items() if not k.startswith("_") and "profile" in v["rec"] and any("SlottingDetected" in w for w in calc(v["rec"], "warnings"))), "{:d}"),
    ("d12_refused", lambda: sum(1 for k, v in js(os.path.join(HERE, "cells.json")).items() if not k.startswith("_") and "profile" not in v["rec"]), "{:d}"),
    ("d12_cells", lambda: sum(1 for k in js(os.path.join(HERE, "cells.json")) if not k.startswith("_")), "{:d}"),
    # agrees
    ("ag_drill", lambda: fix_new("Pin drill")["suggested"]["plunge_rate"], "{:.0f}"),
    ("ag_face18", lambda: calc(fix_new("Face back"), "chip_load_mm"), "{:.4f}"),
    ("ag_face18_feed", lambda: fix_new("Face back")["suggested"]["feed_rate"], "{:.0f}"),
    ("ag_vriv", lambda: fix_new("V rivers")["suggested"]["feed_rate"], "{:.0f}"),
    ("ag_bull05", lambda: new("D_bull_ap0.5")["suggested"]["feed_rate"], "{:.0f}"),
    ("ag_bull3", lambda: new("D_bull_ap3.0")["suggested"]["feed_rate"], "{:.0f}"),
    ("ag_rigid", lambda: prop(new("E_rigid_hardwood"), "ap_mm"), "{:.2f}"),
    # D14
    ("d14_plunge", lambda: gate("P_thin_005", "plunge_rate_mm_min"), "{:.0f}"),
    ("d14_chip", lambda: gate("P_thin_005", "plunge_rate_mm_min") / (gate("P_thin_005", "spindle_rpm") * 2), "{:.4f}"),
    ("d14_line", lambda: gcode("P_thin_005")[11], "{}"),
    ("d14_base", lambda: new("A_flat_6.000_2f_hardwood")["suggested"]["plunge_rate"] / (new("A_flat_6.000_2f_hardwood")["suggested"]["spindle_rpm"] * 2), "{:.4f}"),
    ("d14_achieved", lambda: gate("P_thin_005", "plunge_worst_achieved_z_rate_mm_min"), "{:.0f}"),
    # D15
    ("d15_feed", lambda: new("C_width_0.05")["suggested"]["feed_rate"], "{:.0f}"),
    ("d15_rpm", lambda: new("C_width_0.05")["suggested"]["spindle_rpm"], "{:d}"),
    ("d15_old", lambda: old("C_width_0.05")["suggested"]["feed_rate"], "{:.0f}"),
    ("d15_f9999", lambda: sum(1 for l in gcode("P_thin_005") if l.startswith("G1") and "F9999" in l), "{:d}"),
    # D16
    ("d16_row", lambda: calc(new("A_flat_12.700_2f_mdf"), "vendor_source"), "{}"),
    ("d16_chip", lambda: calc(new("A_flat_12.700_2f_mdf"), "chip_load_mm"), "{:.4f}"),
    ("d16_flat", lambda: max(c for _, c in band(new("A_flat_12.700_2f_mdf"))), "{:.4f}"),
    ("d16_bull_row", lambda: calc(new("D_bull_ap0.5"), "vendor_source"), "{}"),
    # D17
    ("d17_ply_kw", lambda: calc(new("A_flat_6.350_2f_plywood"), "power_kw"), "{:.1f}"),
]


def main():
    ok = 0
    for cid, fn, fmt in CLAIMS:
        try:
            v = fn()
            shown = fmt.format(int(round(v)) if fmt == "{:d}" and isinstance(v, float) else v)
            good = shown in DOC
        except Exception as e:  # noqa: BLE001
            shown, good = f"ERROR {type(e).__name__}: {e}", False
        print(("PASS " if good else "FAIL ") + f"{cid}: {shown}")
        ok += good
    print(f"{ok}/{len(CLAIMS)} PASS")
    sys.exit(0 if ok == len(CLAIMS) else 1)


if __name__ == "__main__":
    main()
