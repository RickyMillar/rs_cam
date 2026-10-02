#!/usr/bin/env python3
"""Tier trial (PLAN.md) — read every run JSON, apply the pre-registered pass
rule against A1 under the planner dressup policy (Amendment 1), write
results.csv, and assert each claim RESULTS.md and the report page make.

Run from the repo root: python3 planning/tier_trial_2026-10-01/analyse.py
"""
import csv
import glob
import json
import os

HERE = os.path.dirname(os.path.abspath(__file__))
RUNS = os.path.join(HERE, "runs")

# Run file -> (label, group). Group: ref, r1, r2, tier, ladder, blob, frag,
# style, arc, info.
LABELS = {
    "A1_planner": ("R1 iso h0.03 (reference)", "ref"),
    "A1": ("R1 iso h0.03, fixture arcs 0.05", "arc"),
    "A1_arcoff": ("R1 iso h0.03, arcs off", "arc"),
    "A1_arc020": ("R1 iso h0.03, arcs 0.02", "arc"),
    "A1_arc030": ("R1 iso h0.03, arcs 0.03", "arc"),
    "A2_p": ("R1 iso h0.02", "r1"),
    "A3_p": ("R1 iso h0.05", "r1"),
    "A4_p": ("R1 iso h0.08", "r1"),
    "A5_p": ("R2 iso h0.03", "r2"),
    "A6_p": ("R2 iso h0.02", "r2"),
    "T1_p": ("Tiers R2>R1, tol 0.05", "tier"),
    "T2_p": ("Tiers R2>R1, tol 0.10", "tier"),
    "T3": ("Tiers R2>R1, tol 0.15", "tier"),
    "T4_p": ("Tiers R2>R1, tol 0.25", "tier"),
    "T5_p": ("Tiers R2>R1, GUI default (Unified), BEFORE G-FLUTETOP", "info"),
    "T5_flutetop": ("Tiers R2>R1, GUI default (Unified), after G-FLUTETOP", "tier"),
    "T9_p": ("Tiers R2>R1, tol 0.15, R2 h0.05", "tier"),
    "T6_p": ("Tiers R3.2>R2>R1, tol 0.10 (hyp. ball)", "ladder"),
    "T7_p": ("Tiers R3.2>R2>R1, tol 0.15 (hyp. ball)", "ladder"),
    "T8_p": ("Tiers R3.2>R1, tol 0.15 (hyp. ball)", "ladder"),
    "B1_c5": ("Blob: R3 flats/sea, R1 range, close 5 mm", "blob"),
    "B1_c10": ("Blob: R3 flats/sea, R1 range, close 10 mm", "blob"),
    "P1_p": ("R2 whole + R1 pencil", "frag"),
    "P2_p": ("Tiers R2>R1 tol 0.15, R2 skips R1 islands", "frag"),
    "P3_p": ("R2 whole + R1 tier-1 islands", "frag"),
    "F1_p": ("R2 semi h0.08 + R1 iso h0.03", "frag"),
    "C1_p": ("R1 parallel raster, s 0.49", "style"),
    "R45_p": ("R1 parallel raster, s 0.34 (cusp held to 45°)", "style"),
    "R60_p": ("R1 parallel raster, s 0.24 (cusp held to 60°)", "style"),
    "R65_p": ("R1 parallel raster, s 0.21 (cusp held to 65°)", "style"),
    "R70_p": ("R1 parallel raster, s 0.17 (cusp held to 70°)", "style"),
    "RS45_p": ("R1 raster <45° + iso >45° (split)", "style"),
    "C2_p": ("R1 scallop, not iso (cascade)", "style"),
}


def load(stem):
    with open(os.path.join(RUNS, stem + ".json")) as f:
        return json.load(f)


def row(stem):
    d = load(stem)
    s = d["windows"]["pooled"]
    b = d["board"]
    fin = [o for o in d["ops"] if o["role"] == "finish"]
    return {
        "run": stem,
        "label": LABELS[stem][0],
        "group": LABELS[stem][1],
        "finish_h": d["time"]["finish_s"] / 3600.0,
        "p50": s["p50"],
        "p90": s["p90"],
        "p99": s["p99"],
        "pct_gt_006": 100.0 * s["frac_dev_gt_0_06"],
        "pct_gouge": 100.0 * s["frac_dev_lt_m0_02"],
        "min_dev": s["min"],
        "board_gt_030_mm2": b["area_dev_gt_0_30_mm2"],
        "entries": sum(o["counts"]["entry_runs"] for o in fin),
        "cutting_m": sum(o["cutting_mm"] for o in fin) / 1000.0,
        "hypothetical_tool": any(o.get("hypothetical_tool") for o in fin),
    }


def passes(r, ref):
    """PLAN.md pass rule, margins as pre-registered."""
    checks = {
        "p99": r["p99"] <= ref["p99"] + 0.010,
        "gt006": r["pct_gt_006"] <= ref["pct_gt_006"] + 0.5,
        "gouge": r["pct_gouge"] <= ref["pct_gouge"] + 0.05,
        "board": r["board_gt_030_mm2"] <= 1.1 * ref["board_gt_030_mm2"] + 100.0,
    }
    return all(checks.values()), checks


def main():
    stems = sorted(
        os.path.basename(p)[:-5]
        for p in glob.glob(os.path.join(RUNS, "*.json"))
    )
    rows = {s: row(s) for s in stems if s in LABELS}
    ref = rows["A1_planner"]
    out = []
    for s, r in rows.items():
        ok, checks = passes(r, ref)
        r["pass"] = ok
        r["fails"] = ",".join(k for k, v in checks.items() if not v)
        r["speedup_vs_ref"] = ref["finish_h"] / r["finish_h"]
        out.append(r)
    out.sort(key=lambda r: r["finish_h"])
    with open(os.path.join(HERE, "results.csv"), "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(out[0].keys()))
        w.writeheader()
        for r in out:
            w.writerow({k: (round(v, 4) if isinstance(v, float) else v) for k, v in r.items()})
    for r in out:
        print(
            f"{r['run']:12} {r['finish_h']:6.2f} h  p99 {r['p99']:.3f}  >0.06 {r['pct_gt_006']:5.1f}%  "
            f"gouge {r['pct_gouge']:4.2f}%  board>0.3 {r['board_gt_030_mm2']:6.0f}  "
            f"entries {r['entries']:6}  {'PASS' if r['pass'] else 'fail: ' + r['fails']}"
        )

    # ── Claims (each one RESULTS.md / the page states) ──────────────────
    R = rows
    # Determinism: A1 twice, identical metrics.
    a, b = load("A1"), load("A1_rep")
    assert a["windows"]["pooled"] == b["windows"]["pooled"], "A1 not deterministic"
    # Arc fit at 0.05 gouges; 0.015 does not.
    assert R["A1"]["pct_gouge"] > 5.0 and R["A1_planner"]["pct_gouge"] < 0.2
    assert R["A1_arcoff"]["pct_gouge"] < 0.2
    assert R["A1_arcoff"]["finish_h"] - R["A1_planner"]["finish_h"] > 1.5
    assert R["A1_planner"]["finish_h"] - R["A1"]["finish_h"] > 2.0
    # The arc gouge grows past 0.02.
    assert R["A1_arc020"]["pct_gouge"] < 0.2 < R["A1_arc030"]["pct_gouge"]
    # No R2/R1 tier arm beats the reference on time AND passes.
    for s in ["T1_p", "T2_p", "T3", "T4_p", "T9_p", "P2_p", "P3_p"]:
        assert not (R[s]["pass"] and R[s]["finish_h"] < ref["finish_h"]), s
    # Tight tiers are much slower; fragmentation (entries) is the cause.
    assert R["T1_p"]["finish_h"] > 1.5 * ref["finish_h"]
    assert R["T1_p"]["entries"] > 10 * ref["entries"]
    # The blob arms: faster than the reference, p99 within margin.
    for s in ["B1_c5", "B1_c10"]:
        assert R[s]["finish_h"] < 0.86 * ref["finish_h"], s
        assert R[s]["p99"] <= ref["p99"] + 0.010, s
    # Path style dominates: the raster and the cascade are 2.5x+ faster.
    for s in ["C1_p", "C2_p", "R45_p"]:
        assert R[s]["speedup_vs_ref"] > 2.5, s
    # The slope split fragments.
    assert R["RS45_p"]["entries"] > 10000
    # G-FLUTETOP: the pre-fix GUI-default tier run gouged to the stock floor.
    assert R["T5_p"]["min_dev"] < -20.0
    if "T5_flutetop" in R:
        assert R["T5_flutetop"]["min_dev"] > -1.0, "G-FLUTETOP re-run still gouges"
    print("all claims hold")


if __name__ == "__main__":
    main()
