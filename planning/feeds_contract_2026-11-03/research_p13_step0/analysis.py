"""P13 step 0: the removal-rate aim (E) and the implied frame stiffness (B) from the shipped laws.

Run: python3 -I analysis.py   (after verify_port.py passes). Writes results.json.
Every law is the port in shipped_laws.py (checked against the CLI at 56 points).
Sweep values (feed caps above 6000, spindle powers above 1.5 kW, frame stiffness) are
sweep values, not sources; the page says so.
"""
import json
import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import shipped_laws as L  # noqa: E402

Z = 2
RPM_FLOOR = 8000.0  # generic_wood_router min_rpm, machine/mod.rs:899
# Cells: vendor row rpm and chip from cli_points.json (Amana Spektra rows, MatchChart)
cli = json.load(open(os.path.join(HERE, "cli_points.json")))


def row_speeds(cid):
    import re
    c = cli[cid]["calculator"]
    rpm0 = c["rpm"]
    for w in c["warnings"]:
        m = re.search(r"rpm_from: ([0-9.]+)", w)
        if m:
            rpm0 = float(m.group(1))
    return rpm0, c["chip_load_mm"], c["vendor_source"]


CELLS = {
    # id: (cli cell for the row, D, flute, stickout, material)
    "hw6": ("fm1_hw6", 6.0, 22.0, 30.0, "GenericHardwood"),
    "hw3": ("hw3", 3.175, 12.0, 20.0, "GenericHardwood"),
    "hw12": ("hw12", 12.0, 30.0, 38.0, "GenericHardwood"),
    "sw6": ("sw6", 6.0, 22.0, 30.0, "GenericSoftwood"),
    "mdf6": ("mdf6", 6.0, 22.0, 30.0, "mdf"),
}
AE_FR = [0.05 * k for k in range(1, 21)]
AP_FR = [0.05 * k for k in range(1, 41)]  # 0.05 D .. 2 D


def grid(cell, cap=4000.0, power_kw=0.8, k_frame=None, target=L.DEFLECTION_TARGET_MM,
         power_case="slot", typed_caps=False, stickout=None):
    cid, d, flute, stick, mat = CELLS[cell]
    stick = stickout or stick
    rpm0, chip, row = row_speeds(cid)
    ks, fe = L.force_line(mat)
    best = None
    pts = []
    for af in AE_FR:
        ae = af * d
        rpm, feed = L.speeds(ae, d, Z, rpm0, chip, cap, RPM_FLOOR)
        fz = feed / (rpm * Z)
        for pf in AP_FR:
            ap = pf * d
            reasons = []
            if ap > 0.8 * flute + 1e-9:
                reasons.append("flute_guard")  # feeds/mod.rs Step 4, 0.8 x flute (Unsourced)
            if ap > 1.0 * d + 1e-9:
                reasons.append("vendor_depth_table")  # peak chip = band max -> 1 x D (geometry.rs)
            w = d if power_case == "slot" else ae
            if L.power_kw(ap, w, d, Z, rpm, feed, ks, fe) > power_kw:
                reasons.append("power")
            if L.tip_deflection_mm(ap, ae, d, Z, fz, ks, fe, d, stick, flute, k_frame=k_frame) > target:
                reasons.append("deflection")
            if typed_caps:
                if ap > 0.25 * d + 1e-9:
                    reasons.append("rigidity_cap")
            mrr = ae * ap * feed
            pts.append({"ae_fr": round(af, 3), "ap_fr": round(pf, 3), "mrr": mrr, "out": reasons})
            if not reasons and (best is None or mrr > best["mrr"] + 1e-9):
                best = {"ae_fr": round(af, 3), "ap_fr": round(pf, 3), "mrr": mrr, "feed": feed, "rpm": rpm}
    conv = next(p for p in pts if abs(p["ae_fr"] - 0.35) < 1e-9 and abs(p["ap_fr"] - 0.25) < 1e-9)
    return {"best": best, "convention": conv, "points": pts, "row": row, "rpm0": rpm0, "chip": chip}


def k_for(cell, ap_fr, ae_fr, target, stickout=None, fz=None, rpm_feed=None):
    """The frame stiffness (N/mm) at which tool + frame tip deflection = target at (ap, ae)."""
    cid, d, flute, stick, mat = CELLS[cell]
    stick = stickout or stick
    rpm0, chip, _ = row_speeds(cid)
    ks, fe = L.force_line(mat)
    ae, ap = ae_fr * d, ap_fr * d
    if fz is None:
        rpm, feed = L.speeds(ae, d, Z, rpm0, chip, 4000.0, RPM_FLOOR)
        fz = feed / (rpm * Z)
    _, peak = L.edge_force(ap, ae, d, Z, fz, ks, fe)
    c_tool = L.tip_compliance_mm_per_n(d, d, stick, flute, ap)
    room = target - peak * c_tool
    k = peak / room if room > 0 else None
    return {"peak_n": peak, "c_tool_mm_per_n": c_tool, "tool_deflection_mm": peak * c_tool, "k_frame_n_per_mm": k,
            "fz": fz}


def main():
    out = {"cells": {}, "sweeps": {}, "implied_k": {}, "frame_e": {}}
    for cell in CELLS:
        r0 = grid(cell)
        r1 = grid(cell, typed_caps=True)
        r2 = grid(cell, power_case="nominal")
        out["cells"][cell] = {
            "row": r0["row"], "rpm0": r0["rpm0"], "chip": r0["chip"],
            "computed_only": r0["best"], "with_typed_caps": r1["best"], "per_pass_power": r2["best"],
            "convention": {k: r0["convention"][k] for k in ("mrr", "out")},
            "grid_hw": r0["points"] if cell == "hw6" else None,
        }
    # cap and power sweeps on hw6 (computed limits only, per-pass power)
    for cap in (4000.0, 6000.0, 10000.0, 20000.0, 1e9):
        for pw in (0.8, 1.5, 3.0, 6.0):
            b = grid("hw6", cap=cap, power_kw=pw, power_case="nominal")["best"]
            out["sweeps"][f"cap{int(min(cap, 1e9))}_p{pw}"] = b
    # implied frame stiffness: today's conventions and the vendor's own slot
    for target in (0.05, 0.10, 0.20):
        out["implied_k"][f"t{target}"] = {
            "pocket_0.25D_0.35D": k_for("hw6", 0.25, 0.35, target),
            "adaptive_0.33D_0.25D": k_for("hw6", 2.0 / 6.0, 0.25, target),
            "vendor_slot_1D": k_for("hw6", 1.0, 1.0, target, fz=0.127),
        }
    for st in (20.0, 30.0, 45.0, 60.0):
        out["implied_k"][f"stickout{int(st)}_t0.2"] = k_for("hw6", 0.25, 0.35, 0.20, stickout=st)
    # does a frame spring give a sub-D MRR optimum? (hw6, computed limits, per-pass power)
    k_conv = out["implied_k"]["t0.2"]["pocket_0.25D_0.35D"]["k_frame_n_per_mm"]
    for label, k in (("k_conv", k_conv), ("k_x2", 2 * k_conv), ("k_x4", 4 * k_conv), ("k_x10", 10 * k_conv)):
        g = grid("hw6", k_frame=k, power_case="nominal")
        out["frame_e"][label] = {"k": k, "best": g["best"],
                                 "grid": [{"ae_fr": p["ae_fr"], "ap_fr": p["ap_fr"], "mrr": p["mrr"],
                                           "ok": not p["out"]} for p in g["points"]]}
        # the deepest admitted ap per width, and the MRR there
        front = []
        for af in AE_FR:
            col = [p for p in g["points"] if abs(p["ae_fr"] - round(af, 3)) < 1e-9 and not p["out"]]
            if col:
                top = max(col, key=lambda p: p["ap_fr"])
                front.append({"ae_fr": top["ae_fr"], "ap_fr": top["ap_fr"], "mrr": top["mrr"]})
        out["frame_e"][label]["front"] = front
    json.dump(out, open(os.path.join(HERE, "results.json"), "w"), indent=1)
    for c, v in out["cells"].items():
        print(c, "computed", v["computed_only"], "\n   typed", v["with_typed_caps"], "\n   perpass", v["per_pass_power"])
    for k, v in out["sweeps"].items():
        print(k, v and (v["ae_fr"], v["ap_fr"], round(v["mrr"])))
    print(json.dumps(out["implied_k"], indent=0)[:3000])
    for k, v in out["frame_e"].items():
        print(k, round(v["k"], 1), v["best"], [(f["ae_fr"], f["ap_fr"], round(f["mrr"])) for f in v["front"]])


if __name__ == "__main__":
    main()
