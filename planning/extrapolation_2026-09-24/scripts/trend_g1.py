#!/usr/bin/env python3
"""G1 Phase 2 trend (read-only on the LUT). DERIVED numbers only.

Question: how does the printed wood chipload change with the tool diameter,
per series, and what do the charts print below 1.5 mm?

Inputs (read-only):
  crates/rs_cam_core/data/vendor_lut/observations/*.json   (the LUT)
  planning/extrapolation_2026-09-24/fetch/G1/verified_rows.json

Series key (as lut_axes.py and CHIPLOAD_LITERATURE_VERDICT section 4.0):
  (source_id, tool_family, tool_subfamily, material_family, flute_count,
   pass_role). An ungrouped fit mixes series that differ 3x at one diameter.

Filter for a "printed" point:
  - wood material family, diameter and chipload_max present;
  - evidence_grade a or b (grade c rows are presets, community values or
    repo-authored bands; they are listed apart in section G);
  - row_kind is not "fallback".
Dedupe: the LUT copies one chart cell into several operation rows
(parallel, scallop, pocket, adaptive). Inside a series, one diameter keeps
one point: the distinct (min, max) pair. Two distinct pairs at one diameter
are a conflict; the script prints it and uses the mean.

Fit: ordinary least squares of ln(chipload) on ln(diameter), on the band
mid (min+max)/2, and also on min and on max where both limits exist. A
series with one printed value per size fits on that value (mid = max).
A series with 2 sizes gives a two-point slope with no residual.

Usage: python3 trend_g1.py
"""
import glob
import json
import math
import statistics as st
from collections import defaultdict
from pathlib import Path

HERE = Path(__file__).resolve().parent
PROG = HERE.parent
ROOT = PROG.parents[1]
OBS = ROOT / "crates/rs_cam_core/data/vendor_lut/observations"
VERIFIED = PROG / "fetch/G1/verified_rows.json"

WOOD = {"softwood", "hardwood", "mdf", "hdf", "particleboard",
        "plywood_hardwood", "plywood_softwood"}
SHIPPED_P = 0.61  # vendor_lookup::CHIPLOAD_DIAMETER_EXPONENT
MICRO_D = 1.5     # support::MICRO_TOOL_DIAMETER_MM

# CHIPLOAD_LITERATURE_VERDICT.md section 4.1 (2026-08-04), per family.
PRIOR = [
    ("Onsrud (own charts, both woods)", "50 fits", "0.20-0.66", 0.37, "1.6-50.8"),
    ("Onsrud (LUT subset)", "9 fits", "0.29-0.49", 0.375, "3.2-19.1"),
    ("Amana Spektra spiral plunge", "2 fits", "0.586-0.619", 0.60, "0.79-12.7"),
    ("Amana ZrN 2D/3D carving", "2 fits", "0.488-0.924", 0.71, "3.2-6.4"),
    ("Freud", "3 fits", "1.05-1.25", 1.09, "3.2-12.7"),
    ("IDC Woodcraft (grade c)", "1 fit", "0.230", 0.23, "3.2-12.7"),
    ("Whiteside Fusion360 (grade c)", "-", "0.0", 0.0, "3.2-6.4"),
]

# The six LUT ball_nose rows that the G1 fetch found to be tapered tools
# (fetch/G1/lut_discrepancies.md D1), and the D2 row (value from the IPM).
D1_ROWS = {
    "amana-ball-softwood-parallel-1000-2f-zrn", "amana-ball-mdf-parallel-1000-2f-zrn",
    "amana-ball-softwood-parallel-0794-3f-zrn", "amana-ball-mdf-parallel-0794-3f-zrn",
    "amana-ball-softwood-parallel-1500-4f-zrn", "amana-ball-softwood-parallel-1587-2f-zrn",
}
D2_ROW = "amana-ball-softwood-parallel-1587-2f-zrn"


def load():
    rows = []
    for f in sorted(glob.glob(str(OBS / "*.json"))):
        d = json.load(open(f))
        for r in (d["observations"] if isinstance(d, dict) else d):
            r = dict(r)
            r["_origin"] = "LUT"
            rows.append(r)
    for r in json.loads(VERIFIED.read_text())["observations"]:
        r = dict(r)
        r["_origin"] = "G1"
        rows.append(r)
    return rows


def frame(r):
    """The diameter the series keys on."""
    fam, src = r["tool_family"], r["source_id"]
    if fam == "tapered_ball_nose":
        if src.startswith("whiteside"):
            return "preset D (not tip, D4)"
        return "tip"
    if r["observation_id"] in D1_ROWS:
        return "tip (tapered tool, D1)"
    if fam == "chamfer_vbit":
        return "body D"
    return "cutting D"


def is_printed(r):
    return (r["material_family"] in WOOD and r.get("diameter_mm")
            and r.get("chipload_max_mm_tooth")
            and r["evidence_grade"] in ("a", "b") and r["row_kind"] != "fallback")


def fit(xs, ys):
    """OLS of ln y on ln x: (slope, r2); r2 None when y is constant."""
    lx = [math.log(x) for x in xs]
    ly = [math.log(y) for y in ys]
    mx, my = st.mean(lx), st.mean(ly)
    sxx = sum((a - mx) ** 2 for a in lx)
    sxy = sum((a - mx) * (b - my) for a, b in zip(lx, ly))
    syy = sum((b - my) ** 2 for b in ly)
    slope = sxy / sxx
    r2 = None if syy == 0 else (sxy * sxy) / (sxx * syy)
    return slope, r2


def build_series(rows):
    series = defaultdict(lambda: defaultdict(set))
    meta = defaultdict(lambda: {"ids": [], "kinds": set(), "grades": set(),
                                "frames": set(), "origin": set()})
    for r in rows:
        if not is_printed(r):
            continue
        k = (r["source_id"], r["tool_family"], r.get("tool_subfamily"),
             r["material_family"], r.get("flute_count"), r["pass_role"])
        lo = r.get("chipload_min_mm_tooth")
        hi = r["chipload_max_mm_tooth"]
        series[k][round(r["diameter_mm"], 4)].add((lo, hi))
        m = meta[k]
        m["ids"].append(r["observation_id"])
        m["kinds"].add(r["row_kind"])
        m["grades"].add(r["evidence_grade"])
        m["frames"].add(frame(r))
        m["origin"].add(r["_origin"])
    return series, meta


def points(cells):
    """Per diameter: (d, min or None, max, mid, conflict)."""
    out = []
    for d in sorted(cells):
        pairs = cells[d]
        conflict = len(pairs) > 1
        los = [p[0] for p in pairs if p[0]]
        his = [p[1] for p in pairs]
        lo = st.mean(los) if len(los) == len(pairs) else None
        hi = st.mean(his)
        mid = (lo + hi) / 2 if lo else hi
        out.append((d, lo, hi, mid, conflict))
    return out


def short(s, n):
    s = str(s)
    return s if len(s) <= n else s[: n - 1] + "~"


def main():
    rows = load()
    series, meta = build_series(rows)
    multi = {k: v for k, v in series.items() if len(v) >= 2}
    lut_n = sum(1 for r in rows if r["_origin"] == "LUT")
    g1_n = sum(1 for r in rows if r["_origin"] == "G1")
    printed = sum(1 for r in rows if is_printed(r))
    print("# trend_g1.py output (DERIVED; every slope and ratio is computed, no vendor prints it)")
    print(f"\nInputs: {lut_n} LUT rows, {g1_n} verified G1 rows; {printed} printed wood points "
          f"(grade a/b, not fallback) before the per-diameter dedupe.")
    print(f"Series with >= 2 diameters: {len(multi)} "
          f"(LUT only {sum(1 for k in multi if meta[k]['origin'] == {'LUT'})}, "
          f"G1 only {sum(1 for k in multi if meta[k]['origin'] == {'G1'})}).")

    # Identify copies: series whose point set equals another series of the
    # same source / family / subfamily / flutes / role (a shared chart cell
    # applied to several materials).
    sig = {}
    first = {}
    # The series whose rows are all printed (row_kind exact) is the canonical
    # one; the derived shared-column copies name it (the MDF column of the
    # Spektra chart is exact, the wood columns are derived b).
    def canon_order(k):
        return (0 if meta[k]["kinds"] == {"exact"} else 1, k)
    for k in sorted(multi, key=canon_order):
        pts = tuple((d, lo, hi) for d, lo, hi, _, _ in points(multi[k]))
        s = (k[0], k[1], k[2], k[4], k[5], pts)
        if s in first:
            sig[k] = first[s]
        else:
            first[s] = k
            sig[k] = None

    # ---- A. per-point table
    print("\n## A. Chipload against diameter, every series with >= 2 sizes (mm/tooth)")
    print("Frame = the diameter the series keys on. 'copy of' = the same printed cell as the named")
    print("material (a shared chart row); it is not a second witness.")
    fits = {}
    for fam in ["tapered_ball_nose", "ball_nose", "chamfer_vbit", "bull_nose", "flat_end"]:
        ks = sorted(k for k in multi if k[1] == fam)
        if not ks:
            continue
        print(f"\n### {fam}: {len(ks)} series")
        print("| source | subfamily | material | fl | role | grade/kind | frame | d mm | min | max | mid |")
        print("|---|---|---|---|---|---|---|---|---|---|---|")
        for k in ks:
            m = meta[k]
            gk = "+".join(sorted(m["grades"])) + "/" + "+".join(sorted(m["kinds"]))
            mat = k[3] + (f" (copy of {sig[k][3]})" if sig[k] else "")
            for i, (d, lo, hi, mid, c) in enumerate(points(multi[k])):
                head = (f"| {short(k[0], 34)} | {short(k[2], 24)} | {mat} | {k[4]} | {k[5]} | {gk} | "
                        f"{'; '.join(sorted(m['frames']))} " if i == 0 else "| | | | | | | ")
                print(f"{head}| {d:g} | {lo if lo else '-'} | {hi:g} | {mid:.4f}"
                      f"{' (conflict)' if c else ''} |")

    # ---- B. slopes per series
    print("\n## B. Log-log slope per series (on mid; min and max where both limits are printed)")
    print("| family | source | subfamily | material | fl | role | n | span | slope mid | r2 | slope min | slope max | note |")
    print("|---|---|---|---|---|---|---|---|---|---|---|---|---|")
    for k in sorted(multi, key=lambda k: (k[1], k[0], str(k[2]), k[3], str(k[4]), k[5])):
        pts = points(multi[k])
        ds = [p[0] for p in pts]
        mids = [p[3] for p in pts]
        n = len(pts)
        span = max(ds) / min(ds)
        s_mid, r2 = fit(ds, mids)
        both = all(p[1] for p in pts)
        s_lo = fit(ds, [p[1] for p in pts])[0] if both else None
        s_hi = fit(ds, [p[2] for p in pts])[0]
        note = []
        if n == 2:
            note.append("2 sizes: no residual")
        if r2 is None:
            note.append("constant")
        if sig[k]:
            note.append(f"copy of {sig[k][3]}")
        if any(i in D1_ROWS for i in meta[k]["ids"]):
            note.append("D1 tapered cells filed as ball")
        if D2_ROW in meta[k]["ids"]:
            note.append("D2: 1.5875 value from IPM")
        if any(p[4] for p in pts):
            note.append("conflict at one size")
        fits[k] = dict(n=n, span=span, slope=s_mid, r2=r2, dmin=min(ds), dmax=max(ds),
                       copy=bool(sig[k]), mids=list(zip(ds, mids)))
        r2s = "n/a" if r2 is None else ("-" if n == 2 else f"{r2:.2f}")
        print(f"| {k[1]} | {short(k[0], 30)} | {short(k[2], 20)} | {k[3]} | {k[4]} | {k[5]} | {n} | "
              f"{span:.1f}x | {s_mid:+.2f} | {r2s} | {'-' if s_lo is None else f'{s_lo:+.2f}'} | "
              f"{s_hi:+.2f} | {'; '.join(note)} |")

    # ---- C. per family
    print("\n## C. Slopes grouped per tool family (distinct printed series only; copies left out)")
    print("| family | series >=3 sizes | median | range | series 2 sizes | median | range | widest span |")
    print("|---|---|---|---|---|---|---|---|")
    for fam in ["flat_end", "ball_nose", "tapered_ball_nose", "chamfer_vbit", "bull_nose"]:
        ks = [k for k in fits if k[1] == fam and not fits[k]["copy"]]
        f3 = [fits[k]["slope"] for k in ks if fits[k]["n"] >= 3]
        f2 = [fits[k]["slope"] for k in ks if fits[k]["n"] == 2]
        wide = max((fits[k]["span"] for k in ks), default=0)

        def fmt(v):
            return ("-", "-") if not v else (f"{st.median(v):+.2f}", f"{min(v):+.2f} to {max(v):+.2f}")
        a, b = fmt(f3)
        c, d = fmt(f2)
        print(f"| {fam} | {len(f3)} | {a} | {b} | {len(f2)} | {c} | {d} | {wide:.1f}x |")

    print("\n### C2. Slopes per source and family (distinct series, >= 3 sizes, then 2 sizes)")
    print("| family | source | n series | slopes (>=3 sizes) | slopes (2 sizes) | frame |")
    print("|---|---|---|---|---|---|")
    bysrc = defaultdict(list)
    for k in fits:
        if not fits[k]["copy"]:
            bysrc[(k[1], k[0])].append(k)
    for (fam, src), ks in sorted(bysrc.items()):
        s3 = sorted(fits[k]["slope"] for k in ks if fits[k]["n"] >= 3)
        s2 = sorted(fits[k]["slope"] for k in ks if fits[k]["n"] == 2)
        fr = "; ".join(sorted(set().union(*(meta[k]["frames"] for k in ks))))
        print(f"| {fam} | {src} | {len(ks)} | {', '.join(f'{s:+.2f}' for s in s3) or '-'} | "
              f"{', '.join(f'{s:+.2f}' for s in s2) or '-'} | {fr} |")

    # ---- D. against the shipped 0.61 and the prior exponents
    print(f"\n## D. Against the shipped exponent {SHIPPED_P} and CHIPLOAD_LITERATURE_VERDICT section 4.1")
    print("\n### D1. Prior per-family exponents (section 4.1, 2026-08-04, quoted)")
    print("| family | fits | range | median | span mm |")
    print("|---|---|---|---|---|")
    for p in PRIOR:
        print(f"| {p[0]} | {p[1]} | {p[2]} | {p[3]} | {p[4]} |")
    print(f"\n### D2. The shipped law applied across each distinct series with >= 3 sizes")
    print(f"Ratio = printed mid at the smallest size / (printed mid at the largest size x (dmin/dmax)^{SHIPPED_P}).")
    print("Under 1 = the law gives MORE than the chart prints at the small end (permissive).")
    print("| family | source | subfamily | material | fl | dmin-dmax | slope | ratio at dmin |")
    print("|---|---|---|---|---|---|---|---|")
    ratios = defaultdict(list)
    for k in sorted(fits, key=lambda k: (k[1], k[0])):
        f = fits[k]
        if f["copy"] or f["n"] < 3:
            continue
        mids = dict(f["mids"])
        pred = mids[f["dmax"]] * (f["dmin"] / f["dmax"]) ** SHIPPED_P
        ratio = mids[f["dmin"]] / pred
        ratios[k[1]].append(ratio)
        print(f"| {k[1]} | {short(k[0], 30)} | {short(k[2], 20)} | {k[3]} | {k[4]} | "
              f"{f['dmin']:g}-{f['dmax']:g} | {f['slope']:+.2f} | {ratio:.2f} |")
    print("\n| family | n | median ratio | range |")
    print("|---|---|---|---|")
    for fam, v in ratios.items():
        print(f"| {fam} | {len(v)} | {st.median(v):.2f} | {min(v):.2f}-{max(v):.2f} |")
    above = sum(1 for k in fits if not fits[k]["copy"] and fits[k]["n"] >= 3 and fits[k]["slope"] > SHIPPED_P)
    below = sum(1 for k in fits if not fits[k]["copy"] and fits[k]["n"] >= 3 and fits[k]["slope"] <= SHIPPED_P)
    print(f"\nDistinct series with >= 3 sizes: slope above {SHIPPED_P}: {above}; at or below: {below}.")

    # ---- E. micro sizes
    print(f"\n## E. What the charts print at the micro sizes (d < 2.0 mm; the size rule acts below {MICRO_D} mm)")
    print("Every distinct printed cell (grade a/b, wood). % of D = chipload / diameter x 100 (derived).")
    print("'law from 1/8\" Onsrud' = the Onsrud 77-100 1/8\" 3-flute hardwood band 0.0762-0.127 mm")
    print(f"scaled by (d/3.175)^{SHIPPED_P}: what the shipped law gives on the tapered row the lookup finds today.")
    print("| family | source | subfamily | fl | d mm | frame | min | max | % of D (min-max) | law from 1/8\" Onsrud (mm) | law % of D | printed mid / law mid | materials |")
    print("|---|---|---|---|---|---|---|---|---|---|---|---|---|")
    cells = defaultdict(set)
    for r in rows:
        if not is_printed(r) or r["diameter_mm"] >= 2.0:
            continue
        fam = r["tool_family"]
        if r["observation_id"] in D1_ROWS:
            fam = "ball_nose (D1: tapered)"
        key = (fam, r["source_id"], r.get("tool_subfamily"), r.get("flute_count"),
               round(r["diameter_mm"], 4), frame(r),
               r.get("chipload_min_mm_tooth"), r["chipload_max_mm_tooth"])
        cells[key].add(r["material_family"])
    for key in sorted(cells, key=lambda k: (k[0], k[4], k[1], str(k[2]))):
        fam, src, sub, fl, d, fr, lo, hi = key
        law_lo = 0.0762 * (d / 3.175) ** SHIPPED_P
        law_hi = 0.127 * (d / 3.175) ** SHIPPED_P
        mid = (lo + hi) / 2 if lo else hi
        pct = f"{100 * lo / d:.1f}-{100 * hi / d:.1f}" if lo else f"{100 * hi / d:.1f}"
        tag = " *" if d < MICRO_D else ""
        print(f"| {fam} | {short(src, 32)} | {short(sub, 20)} | {fl} | {d:g}{tag} | {fr} | {lo if lo else '-'} | {hi:g} | {pct} | "
              f"{law_lo:.4f}-{law_hi:.4f} | {100 * law_lo / d:.1f}-{100 * law_hi / d:.1f} | "
              f"{mid / ((law_lo + law_hi) / 2):.2f} | {', '.join(sorted(cells[key]))} |")
    print("(* = under 1.5 mm, where support::micro_extrapolation_refusal acts.)")

    # ---- F. tapered frame note
    print("\n## F. Diameter frame per tapered-ball series")
    for k in sorted(fits):
        if k[1] == "tapered_ball_nose" or any(i in D1_ROWS for i in meta[k]["ids"]):
            print(f"- {k[0]} {k[2]} {k[3]} f{k[4]} {k[5]}: {'; '.join(sorted(meta[k]['frames']))}")
    print("- onsrud 77_100_series: 1/8\" is 3 flutes and 1/4\" is 2 flutes, so no series forms under the")
    print("  flute grouping. A cross-flute two-point slope (mid) is derived below for reference only.")
    on = {}
    for r in rows:
        if r["source_id"] == "onsrud_hard_wood_cutting_data" and r.get("tool_subfamily") == "77_100_series":
            on[r["diameter_mm"]] = (r["chipload_min_mm_tooth"] + r["chipload_max_mm_tooth"]) / 2
    if len(on) == 2:
        (d1, c1), (d2, c2) = sorted(on.items())
        print(f"  hardwood 77-100 {d1}->{d2}: mid {c1:.4f}->{c2:.4f}, slope {math.log(c2 / c1) / math.log(d2 / d1):+.2f}"
              " (3F against 2F; not a series)")

    # ---- H. local slopes and one sensitivity, tapered series (ratios only)
    print("\n## H. Local slopes between adjacent printed sizes, tapered-ball series (distinct, mid)")
    print("| source | fl | size pair mm | mid pair mm/tooth | local slope |")
    print("|---|---|---|---|---|")
    for k in sorted(fits):
        if k[1] != "tapered_ball_nose" or fits[k]["copy"]:
            continue
        m = fits[k]["mids"]
        for (d1, c1), (d2, c2) in zip(m, m[1:]):
            print(f"| {short(k[0], 34)} | {k[4]} | {d1:g}-{d2:g} | {c1:.4f}-{c2:.4f} | "
                  f"{math.log(c2 / c1) / math.log(d2 / d1):+.2f} |")
    for k in sorted(fits):
        if k[0] == "amana_zrn_3d_profiling_v8" and k[1] == "tapered_ball_nose" and k[4] == 2 and not fits[k]["copy"]:
            m = [p for p in fits[k]["mids"] if abs(p[0] - 1.5875) > 1e-3]
            (d1, c1), (d2, c2) = m[0], m[-1]
            print(f"\nSensitivity: Amana 2F tapered without the self-inconsistent 1/16\" cell: "
                  f"{d1:g}->{d2:g} mm, mid {c1:.4f}->{c2:.4f}, two-point slope "
                  f"{math.log(c2 / c1) / math.log(d2 / d1):+.2f} (with the cell, 3-point fit {fits[k]['slope']:+.2f}).")

    # ---- G. grade c series, listed apart
    print("\n## G. Grade c series with >= 2 sizes (presets, community, repo-authored; not fitted above)")
    gc = defaultdict(dict)
    for r in rows:
        if (r["material_family"] in WOOD and r.get("diameter_mm") and r.get("chipload_max_mm_tooth")
                and (r["evidence_grade"] == "c" or r["row_kind"] == "fallback")):
            k = (r["source_id"], r["tool_family"], r.get("tool_subfamily"), r["material_family"],
                 r.get("flute_count"), r["pass_role"])
            lo = r.get("chipload_min_mm_tooth")
            gc[k][r["diameter_mm"]] = ((lo + r["chipload_max_mm_tooth"]) / 2) if lo else r["chipload_max_mm_tooth"]
    print("| family | source | subfamily | material | fl | role | d -> mid | slope |")
    print("|---|---|---|---|---|---|---|---|")
    for k, v in sorted(gc.items(), key=lambda kv: (kv[0][1], kv[0][0])):
        if len(v) < 2:
            continue
        ds = sorted(v)
        s, _ = fit(ds, [v[d] for d in ds])
        print(f"| {k[1]} | {short(k[0], 30)} | {short(k[2], 20)} | {k[3]} | {k[4]} | {k[5]} | "
              f"{', '.join(f'{d:g}->{v[d]:.4f}' for d in ds)} | {s:+.2f} |")


if __name__ == "__main__":
    main()
