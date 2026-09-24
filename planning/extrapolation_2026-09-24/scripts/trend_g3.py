#!/usr/bin/env python3
"""G3 Phase 2 trend (read-only on the LUT). DERIVED numbers only.

Question: when one tool is printed (or copied) in two or more pass roles or
operation families, what is the ratio between the values? Does "a finish
figure is the rough figure times k" hold, and does k transfer across tool
families? For the bull nose: does a flat-end row of the same vendor and
diameter predict the printed bull row?

Inputs (read-only):
  crates/rs_cam_core/data/vendor_lut/observations/*.json
  planning/extrapolation_2026-09-24/fetch/G3/verified_rows.json
  planning/extrapolation_2026-09-24/inventory_cells.csv

Definitions (all DERIVED by this script):
  - "tool": (source_id, tool_family, tool_subfamily, diameter_mm, flutes,
    material_family, included_angle_deg). One vendor article in one material.
  - "value": the band midpoint when min and max exist, else the one value.
  - "printed cell": one chart cell. Rows that copy one cell into several
    operation families share one printed cell. Key: the verbatim line and
    material when a row has one, else (source, subfamily, diameter, flutes,
    material, min, max).
  - pair kind: exact/exact, derived/exact, derived/derived, from row_kind
    (fallback counts as derived).

Usage: python3 trend_g3.py
"""
import csv
import glob
import json
import statistics as st
from collections import Counter, defaultdict
from itertools import combinations
from pathlib import Path

HERE = Path(__file__).resolve().parent
PROG = HERE.parent
ROOT = PROG.parents[1]
OBS = ROOT / "crates/rs_cam_core/data/vendor_lut/observations"
VERIFIED = PROG / "fetch/G3/verified_rows.json"
CELLS = PROG / "inventory_cells.csv"

WOOD = {"softwood", "hardwood", "mdf", "hdf", "particleboard",
        "plywood_hardwood", "plywood_softwood"}
ROLE_ORDER = {"roughing": 0, "semi_finish": 1, "finish": 2}


def load():
    rows = []
    for f in sorted(glob.glob(str(OBS / "*.json"))):
        for o in json.load(open(f))["observations"]:
            o = dict(o)
            o["_origin"] = "lut"
            rows.append(o)
    for o in json.load(open(VERIFIED))["observations"]:
        o = dict(o)
        o["_origin"] = "g3"
        rows.append(o)
    return [o for o in rows if o.get("material_family") in WOOD]


def value(o):
    lo, hi = o.get("chipload_min_mm_tooth"), o.get("chipload_max_mm_tooth")
    if lo is not None and hi is not None:
        return (lo + hi) / 2.0
    return hi if hi is not None else lo


def kind(o):
    return "exact" if o.get("row_kind") == "exact" else "derived"


def tool_key(o):
    return (o["source_id"], o["tool_family"], o.get("tool_subfamily"),
            o.get("diameter_mm"), o.get("flute_count"), o["material_family"],
            o.get("included_angle_deg"))


def cell_key(o):
    if o.get("verbatim"):
        return (o["source_id"], o["verbatim"], o["material_family"])
    return (o["source_id"], o.get("tool_subfamily"), o.get("diameter_mm"),
            o.get("flute_count"), o["material_family"],
            o.get("chipload_min_mm_tooth"), o.get("chipload_max_mm_tooth"))


def fam(o):
    return f"{o['operation_family']}/{o['pass_role']}"


def summ(xs):
    if not xs:
        return "n=0"
    return (f"n={len(xs)} median {st.median(xs):.2f} "
            f"range {min(xs):.2f}-{max(xs):.2f}")


def band(o):
    lo, hi = o.get("chipload_min_mm_tooth"), o.get("chipload_max_mm_tooth")
    if lo is None:
        return f"-{hi:.4f}" if hi is not None else "none"
    return f"{lo:.4f}-{hi:.4f}"


def section(t):
    print()
    print("## " + t)


def main():
    rows = [o for o in load() if value(o)]

    # ------------------------------------------------------------------
    section("1. Tools printed or copied in 2+ operation families or pass roles")
    by_tool = defaultdict(list)
    for o in rows:
        by_tool[tool_key(o)].append(o)
    multi = {k: v for k, v in by_tool.items() if len({fam(o) for o in v}) >= 2}
    print("tool_family        source                              mat              d_mm   fl  families (value mm/tooth, kind)")
    flat_multi = Counter()
    for k in sorted(multi, key=lambda k: (k[1], k[0], k[5], k[3] or 0)):
        v = multi[k]
        if k[1] == "flat_end":
            flat_multi[(k[0], tuple(sorted({fam(o) for o in v})), tuple(sorted({kind(o) for o in v})))] += 1
            continue
        fams = sorted({(fam(o), round(value(o), 4), kind(o)) for o in v})
        s = "; ".join(f"{f} {x} {kd[0]}" for f, x, kd in fams)
        ang = f" {k[6]:.0f}deg" if k[6] else ""
        print(f"{k[1]:<18} {k[0][:35]:<35} {k[5]:<16} {k[3]!s:<6} {k[4]!s:<3} {s}{ang}")
    for (src, fams, kinds), n in sorted(flat_multi.items()):
        print(f"flat_end           {src[:35]:<35} {n} tools, families {', '.join(fams)}, kinds {'/'.join(kinds)}")
    print(f"tools: {len(multi)}; rows: {sum(len(v) for v in multi.values())}; "
          f"printed cells: {len({cell_key(o) for v in multi.values() for o in v})}")

    # ------------------------------------------------------------------
    section("2. Same-tool ratio, pass role (finish or semi_finish / roughing), per pair kind")
    print("A ratio is (later role value) / (earlier role value) inside one tool.")
    print("Counted once per tool and role pair (copies into families collapse).")
    ratios = defaultdict(list)       # (tool_family, pair, kind) -> [ratio]
    ratio_mat = defaultdict(list)    # (tool_family, material, pair, kind)
    for k, v in multi.items():
        per_role = defaultdict(set)
        for o in v:
            per_role[o["pass_role"]].add((round(value(o), 5), kind(o)))
        roles = sorted(per_role, key=lambda r: ROLE_ORDER.get(r, 9))
        for a, b in combinations(roles, 2):
            for va, ka in per_role[a]:
                for vb, kb in per_role[b]:
                    pk = "/".join(sorted([ka, kb]))
                    r = vb / va
                    ratios[(k[1], f"{b}/{a}", pk)].append(r)
                    ratio_mat[(k[1], k[5], f"{b}/{a}", pk)].append(r)
    print("tool_family        pair                   kinds            stats")
    for key in sorted(ratios):
        print(f"{key[0]:<18} {key[1]:<22} {key[2]:<16} {summ(ratios[key])}")
    print()
    print("per material:")
    for key in sorted(ratio_mat):
        print(f"  {key[0]:<18} {key[1]:<16} {key[2]:<22} {key[3]:<16} {summ(ratio_mat[key])}")

    # ------------------------------------------------------------------
    section("3. Same-tool ratio, operation family (same pass role), per pair kind")
    fr = defaultdict(list)
    for k, v in multi.items():
        per = defaultdict(set)
        for o in v:
            per[(o["pass_role"], o["operation_family"])].add((round(value(o), 5), kind(o)))
        for (ra, fa), (rb, fb) in combinations(sorted(per), 2):
            if ra != rb:
                continue
            for va, ka in per[(ra, fa)]:
                for vb, kb in per[(rb, fb)]:
                    fr[(k[1], f"{fb}/{fa} ({ra})", "/".join(sorted([ka, kb])))].append(vb / va)
    for key in sorted(fr):
        print(f"{key[0]:<18} {key[1]:<32} {key[2]:<16} {summ(fr[key])}")

    # ------------------------------------------------------------------
    section("4. The repo's derived finish rows against the printed roughing rows (Amana ball v7, ZrN taper)")
    print("The finish rows are row_kind derived / grade c. The chart prints no role.")
    print("The ratio is the repo's own transfer factor, not a vendor number.")
    ball = [o for o in rows if o["source_id"] == "amana_ball_nose_v7"]
    rough = {(o["material_family"], o["diameter_mm"]): o for o in ball
             if o["pass_role"] == "roughing" and kind(o) == "exact"}
    rs = []
    print("material  fin_d  finish_row (band, kind/grade)            rough_d rough band      ratio_mid")
    for o in sorted(ball, key=lambda o: (o["material_family"], o["diameter_mm"])):
        if o["pass_role"] == "roughing":
            continue
        d = o["diameter_mm"]
        # the nearest printed roughing size (6.0 finish rows sit beside the 6.35 printed row)
        cands = [(abs(dd - d), dd) for (m, dd) in rough if m == o["material_family"]]
        if not cands:
            continue
        dd = min(cands)[1]
        r = rough[(o["material_family"], dd)]
        ratio = value(o) / value(r)
        rs.append(ratio)
        print(f"{o['material_family']:<9} {d:<6} {o['observation_id'][:30]:<30} {band(o)} {o['row_kind'][0]}/{o['evidence_grade']}  "
              f"{dd:<7} {band(r)} {ratio:.2f}")
    print(f"repo factor finish/rough (ball v7): {summ(rs)}")
    tap = [o for o in rows if o["source_id"] == "amana_zrn_3d_profiling" and o["tool_family"] == "tapered_ball_nose"]
    print("ZrN taper rows (all derived/c):", "; ".join(
        f"{o['material_family']} {o['diameter_mm']} {fam(o)} {band(o)}" for o in sorted(tap, key=lambda o: (o['material_family'], o['diameter_mm'], fam(o)))))

    # ------------------------------------------------------------------
    section("5. Cross-tool contrast in one sheet: a finish series against a roughing series (DIFFERENT tools)")
    print("Same source, tool family, diameter and material; different subfamily; both rows exact.")
    print("Different tools with different flute forms: NOT a per-tool role ratio, NOT transferable.")
    groups = defaultdict(list)
    for o in rows:
        if kind(o) == "exact":
            groups[(o["source_id"], o["tool_family"], o.get("diameter_mm"), o["material_family"])].append(o)
    xr = defaultdict(list)
    xp = defaultdict(list)
    for k, v in sorted(groups.items(), key=lambda kv: (kv[0][1], kv[0][0], kv[0][3], kv[0][2] or 0)):
        fin = {(o.get("tool_subfamily"), round(value(o), 4)) for o in v if o["pass_role"] == "finish"}
        rou = {(o.get("tool_subfamily"), round(value(o), 4)) for o in v if o["pass_role"] == "roughing"}
        for sf, vf in sorted(fin):
            for sr, vr in sorted(rou):
                if sf == sr:
                    continue
                xr[(k[1], k[0])].append(vf / vr)
                xp[(k[1], sf, sr)].append((vf / vr, k[3], k[2]))
    print("per finish series / roughing series (all sheets, materials, diameters):")
    for k in sorted(xp):
        rr = [x[0] for x in xp[k]]
        mats = ",".join(sorted({x[1] for x in xp[k]}))
        ds = ",".join(str(d) for d in sorted({x[2] for x in xp[k]}))
        print(f"  {k[0]:<9} finish {k[1][:26]:<26} / roughing {k[2][:16]:<16} {summ(rr)}  [{mats}; d {ds}]")
    print("per sheet:")
    for k in sorted(xr):
        print(f"  {k[0]:<9} {k[1]:<34} {summ(xr[k])}")
    allr = [r for v in xr.values() for r in v]
    print(f"all cross-tool finish/roughing pairs: {summ(allr)}")

    # ------------------------------------------------------------------
    section("6. Bull nose: every row, and the same-vendor flat-end row at the same diameter")
    bull = [o for o in rows if o["tool_family"] == "bull_nose"]
    seen = set()
    print("origin source                               mat       d_mm  fam(s)                     band           kind/grade")
    bcells = defaultdict(list)
    for o in bull:
        bcells[cell_key(o)].append(o)
    for ck, v in sorted(bcells.items(), key=lambda kv: (kv[1][0]["source_id"], kv[1][0]["material_family"], kv[1][0]["diameter_mm"])):
        o = v[0]
        fams = ",".join(sorted({o2["operation_family"] for o2 in v}))
        print(f"{o['_origin']:<6} {o['source_id'][:36]:<36} {o['material_family']:<9} {o['diameter_mm']:<5} {fams:<26} {band(o):<14} {o['row_kind']}/{o['evidence_grade']}")
    print()
    print("Printed Amana bull cells against Amana flat-end rows (same material, |d - d_bull| <= 0.35 mm):")
    print("bull_d mat       bull_band       flat source / subfamily                 flat_d flat_band      kind  mid/value  max/max")
    flat = [o for o in rows if o["tool_family"] == "flat_end" and o["source_vendor"] == "amana"]
    pr_mid = defaultdict(list)
    for ck, v in sorted(bcells.items(), key=lambda kv: (kv[1][0]["diameter_mm"], kv[1][0]["material_family"])):
        o = v[0]
        if o["_origin"] != "g3":
            continue
        seenf = set()
        for f in sorted(flat, key=lambda f: (f["source_id"], f.get("tool_subfamily") or "", f["diameter_mm"])):
            if f["material_family"] != o["material_family"] or abs(f["diameter_mm"] - o["diameter_mm"]) > 0.35:
                continue
            if f.get("flute_count") != 2:
                continue
            fk = (f["source_id"], f.get("tool_subfamily"), f["diameter_mm"], band(f))
            if fk in seenf:
                continue
            seenf.add(fk)
            rmid = value(o) / value(f)
            rmax = o["chipload_max_mm_tooth"] / f["chipload_max_mm_tooth"]
            tag = f"{f['source_id'][:22]}/{(f.get('tool_subfamily') or '')[:16]}"
            pr_mid[(f"{f['row_kind']}/{f['evidence_grade']}", o["diameter_mm"])].append(rmid)
            print(f"{o['diameter_mm']:<6} {o['material_family']:<9} {band(o):<15} {tag:<40} {f['diameter_mm']:<6} {band(f):<14} {f['row_kind'][0]}/{f['evidence_grade']}  {rmid:9.2f} {rmax:8.2f}")
    for k in sorted(pr_mid):
        print(f"bull/flat mid ratio, flat row {k[0]}, d={k[1]}: {summ(pr_mid[k])}")
    print()
    print("Printed Amana bull cells against Amana ball v7 (same material, same diameter, exact):")
    for ck, v in sorted(bcells.items(), key=lambda kv: (kv[1][0]["diameter_mm"], kv[1][0]["material_family"])):
        o = v[0]
        if o["_origin"] != "g3":
            continue
        m = [b for b in rows if b["source_id"] == "amana_ball_nose_v7" and b["material_family"] == o["material_family"]
             and b["diameter_mm"] == o["diameter_mm"] and kind(b) == "exact"]
        if m:
            print(f"  {o['diameter_mm']} {o['material_family']:<9} bull {band(o)} ball {band(m[0])} ratio {value(o) / value(m[0]):.2f}")
        else:
            print(f"  {o['diameter_mm']} {o['material_family']:<9} bull {band(o)} ball: no LUT row at this size (the chart prints it; see FETCH_NOTES)")
    print()
    print("Derived LUT bull rows (onsrud-bull-*) against the printed Amana bull cell of the same material at 6.35 mm:")
    for o in [o for o in bull if o["_origin"] == "lut"]:
        m = [p for p in bull if p["_origin"] == "g3" and p["material_family"] == o["material_family"] and p["diameter_mm"] == 6.35]
        if m:
            print(f"  {o['observation_id']:<44} {band(o)}  printed {band(m[0])}  derived/printed mid {value(o) / value(m[0]):.2f}")
        else:
            print(f"  {o['observation_id']:<44} {band(o)}  printed: no Amana column for {o['material_family']}")

    # ------------------------------------------------------------------
    section("7. Machine-class contrast (G9, NOT a G3 ratio): IDC benchtop derived rows against industrial printed rows")
    idcv = [o for o in rows if o["source_id"] == "idcwoodcraft_feeds_speeds_pdf"]
    vg = [o for o in rows if o["tool_family"] == "chamfer_vbit" and o["_origin"] == "lut" and kind(o) == "exact"
          and o["material_family"] in ("softwood", "hardwood") and o.get("diameter_mm") == 6.35
          and o.get("chipload_min_mm_tooth") is not None]
    seen = set()
    for o in idcv:
        ck = (o.get("included_angle_deg"), o["material_family"])
        if ck in seen:
            continue
        seen.add(ck)
        gs = [g for g in vg if g["material_family"] == o["material_family"]]
        mids = [value(o) / value(g) for g in gs]
        edges = [value(o) / g["chipload_max_mm_tooth"] for g in gs] + [value(o) / g["chipload_min_mm_tooth"] for g in gs]
        print(f"  IDC {o.get('included_angle_deg'):>4.0f} deg {o['flute_count']}f {o['material_family']:<9} {value(o):.4f} mm "
              f"against {len(gs)} Amana 6.35 mm V-groove/engraving trace rows: IDC/mid {summ(mids)}; "
              f"IDC/band edge {min(edges):.2f}-{max(edges):.2f}")
    idcb = [o for o in rows if o["source_id"].startswith("idcwoodcraft_chipload") and o["tool_family"] == "ball_nose"]
    for o in idcb:
        m = [b for b in rows if b["source_id"] == "amana_ball_nose_v7" and b["material_family"] == o["material_family"]
             and b["diameter_mm"] == o["diameter_mm"] and kind(b) == "exact"]
        if m:
            print(f"  IDC ball {o['diameter_mm']} {o['material_family']} {band(o)} (parallel/finish, derived c) / Amana v7 {band(m[0])} (roughing copy, exact) = {value(o) / value(m[0]):.2f}")

    # ------------------------------------------------------------------
    section("8. The G3 cells (inventory_cells.csv)")
    cells = [r for r in csv.DictReader(open(CELLS)) if r["group"] == "G3"]
    print(f"total {len(cells)}")
    for axis in ("detail", "tool_type", "operation", "material", "diameter_mm"):
        c = Counter(r[axis] for r in cells)
        print(f"  {axis}: " + ", ".join(f"{k} {v}" for k, v in sorted(c.items(), key=lambda kv: -kv[1])))
    c = Counter((r["tool_type"], r["material"]) for r in cells)
    print("  tool x material: " + ", ".join(f"{k[0]}/{k[1]} {v}" for k, v in sorted(c.items())))


if __name__ == "__main__":
    main()
