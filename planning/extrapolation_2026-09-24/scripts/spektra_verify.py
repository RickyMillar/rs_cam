#!/usr/bin/env python3
"""G6 Spektra sizes: verify the transcription against the PDF text.

This script does not read the transcriber's typed table as truth. It reads
the Amana Spektra v24 PDF again with `pdftotext -layout` and parses every
chart row itself. Then it checks:

  1. the PDF sha256 against the source manifest value;
  2. every row of `printed_table` in candidate_rows_spektra_all.json against
     the parsed row (all six printed numbers and the tool numbers), and that
     no parsed row is missing from the typed table;
  3. every candidate observation: diameter_mm, flute_count and
     chipload_max_mm_tooth (printed inch x 25.4) against the parsed row, the
     column (Wood/Plywood or MDF/Laminate) against the family, row_kind and
     evidence_grade against the column, rpm_nominal 18000, and that no row
     field carries the Ramp Down value;
  4. every `ramp_down_check` entry against the parsed Ramp Down and Feed;
  5. the chart's own identities on every parsed row:
       Ramp Down = Feed Rate / Z  (printed rule "Feed Rate IPM / # of flutes")
       Feed Rate = 18000 x Z x Chip Load (printed rule "RPM x # of flutes x
       chip load"), reported as a ratio;
  6. the parsed rows against the Spektra rows that the LUT already holds
     (every observations/*.json file, ids ending in "-spektra"), and that no
     candidate id is already in the LUT.

A failure of checks 1-4, or a duplicate id, is a transcription fault (exit
code 1). Checks 5 and 6 report what the chart and the LUT print; a chart
inconsistency is not a transcription fault.

Usage: python3 spektra_verify.py
"""
import glob
import hashlib
import json
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", "..", ".."))
G6 = os.path.normpath(os.path.join(HERE, "..", "fetch", "G6"))
PDF = os.path.join(G6, "pdf", "amana_spektra_v24.pdf")
CAND = os.path.join(G6, "candidate_rows_spektra_all.json")
LUT = os.path.join(ROOT, "crates", "rs_cam_core", "data", "vendor_lut")
SOURCE = "amana_spektra_spiral_plunge_v24"
PDF_SHA = "5b6fef854b2cf6b422e2eb5bbc86e29b4dcab5b19195b7ff228b70f99cf86d3a"
RPM = 18000.0
WOOD = ("softwood", "hardwood", "plywood_softwood", "plywood_hardwood")

ROW = re.compile(
    r'(?P<d>\d+/\d+"|\d+(?:\.\d+)?mm|0\.\d+")\s+'
    r'(?P<v>(?:\d*\.?\d+"\s+){5}\d*\.?\d+")'
)
TOOL = re.compile(r"\d{5}-K")


def num(s):
    return float(s.strip().rstrip('"'))


def dia_mm(tok):
    if tok.endswith("mm"):
        return float(tok[:-2])
    t = tok.rstrip('"')
    if "/" in t:
        a, b = t.split("/")
        return float(a) / float(b) * 25.4
    return float(t) * 25.4


def parse_pdf():
    txt = subprocess.run(["pdftotext", "-layout", PDF, "-"], check=True,
                         capture_output=True, text=True).stdout
    rows, z = [], None
    for line in txt.splitlines():
        if re.search(r"\b2 Flute\b", line):
            z = 2
        if re.search(r"\b3 Flute\b", line):
            z = 3
        m = ROW.search(line)
        if not m or z is None:
            continue
        v = m.group("v").split()
        rows.append({
            "z": z, "dia": m.group("d"), "d_mm": dia_mm(m.group("d")),
            "tools": TOOL.findall(line[: m.start()]),
            "wp": [num(x) for x in v[:3]], "mdf": [num(x) for x in v[3:]],
            "raw": [x.rstrip('"') for x in v],
        })
    return rows


def key(z, d_mm):
    # A 0.05 mm grid: it joins 4.7625 and 3/16 in, and keeps every printed
    # size apart (the closest pair is 1.5 mm and 1/16 in).
    return (z, round(d_mm * 20) / 20)


def main():
    fails = []
    got = hashlib.sha256(open(PDF, "rb").read()).hexdigest()
    if got != PDF_SHA:
        fails.append(f"pdf sha256 {got} != {PDF_SHA}")
    parsed = parse_pdf()
    by_key = {}
    for r in parsed:
        by_key.setdefault(key(r["z"], r["d_mm"]), []).append(r)
    # One diameter and flute count must print one set of values.
    for k, rs in by_key.items():
        if len({tuple(r["raw"]) for r in rs}) != 1:
            fails.append(f"chart prints two value sets at {k}")
    print(f"parsed {len(parsed)} chart lines, {len(by_key)} (Z, diameter) keys")

    cand = json.load(open(CAND))

    # Check 2: the typed table.
    typed_tools = set()
    typed_keys = set()
    for t in cand["printed_table"]:
        s = json.dumps(t)
        z = t.get("flutes", t.get("z", t.get("flute_count")))
        dtok = t.get("diameter", t.get("dia", t.get("diam")))
        k = key(z, dia_mm(dtok))
        typed_keys.add(k)
        if k not in by_key:
            fails.append(f"typed row {z}F {dtok} not in the PDF")
            continue
        ref = by_key[k][0]
        vals = [float(x) for x in re.findall(r'"(\d*\.?\d+)"', s)]
        # every printed number of the parsed row must occur in the typed row
        for x in ref["wp"] + ref["mdf"]:
            if not any(abs(x - y) < 1e-9 for y in vals):
                fails.append(f"typed row {z}F {dtok}: printed {x} missing")
        typed_tools.update(TOOL.findall(s))
    for k in by_key:
        if k not in typed_keys:
            fails.append(f"PDF row {k} missing from the typed table")
    pdf_tools = {t for r in parsed for t in r["tools"]}
    if pdf_tools - typed_tools:
        fails.append(f"tool numbers not typed: {sorted(pdf_tools - typed_tools)}")
    if typed_tools - pdf_tools:
        fails.append(f"typed tool numbers not in the PDF: {sorted(typed_tools - pdf_tools)}")

    # Check 3: the candidate observations.
    for o in cand["observations"]:
        oid = o["observation_id"]
        k = key(o["flute_count"], o["diameter_mm"])
        if k not in by_key:
            fails.append(f"{oid}: no chart row at {k}")
            continue
        r = by_key[k][0]
        col = "mdf" if o["material_family"] == "mdf" else "wp"
        if o["material_family"] not in WOOD + ("mdf",):
            fails.append(f"{oid}: family {o['material_family']}")
        want = round(r[col][1] * 25.4, 4)
        if abs(o["chipload_max_mm_tooth"] - want) > 5e-5:
            fails.append(f"{oid}: chip {o['chipload_max_mm_tooth']} != {want}")
        if "chipload_min_mm_tooth" in o:
            fails.append(f"{oid}: carries a chipload_min")
        exp = ("exact", "a") if col == "mdf" else ("derived", "b")
        if (o["row_kind"], o["evidence_grade"]) != exp:
            fails.append(f"{oid}: {o['row_kind']}/{o['evidence_grade']} != {exp}")
        if o["rpm_nominal"] != RPM:
            fails.append(f"{oid}: rpm {o['rpm_nominal']}")
        if (o["tool_subfamily"], o["operation_family"], o["pass_role"]) != (
                "spektra_spiral_plunge", "pocket", "roughing"):
            fails.append(f"{oid}: filed wrong")
        if (o["material_label"] == "MDF/Laminate") != (col == "mdf"):
            fails.append(f"{oid}: label {o['material_label']}")
        # The Ramp Down figure is a check, not a row field.
        for kk, vv in o.items():
            if isinstance(vv, (int, float)) and kk not in ("diameter_mm", "flute_count") \
                    and abs(vv - r[col][2]) < 1e-9 and r[col][2] not in (o["rpm_nominal"],):
                fails.append(f"{oid}: field {kk} carries the Ramp Down {vv}")
        if "ramp" in json.dumps({k2: v2 for k2, v2 in o.items() if k2 not in ("notes",)}).lower():
            fails.append(f"{oid}: a field names the ramp")
        if f'{r["raw"][0 if col == "wp" else 3]}"' not in o["source_page"]:
            fails.append(f"{oid}: source_page does not print the feed")
    ids = [o["observation_id"] for o in cand["observations"]]
    if len(ids) != len(set(ids)):
        fails.append("duplicate observation ids")

    # Check 4: the ramp-down check list.
    for e in cand.get("ramp_down_check", []):
        s = json.dumps(e)
        z = e.get("flutes", e.get("z", e.get("flute_count")))
        d = e.get("diameter_mm", e.get("d_mm", e.get("diam_mm")))
        col = "mdf" if "mdf" in s.lower() else "wp"
        r = by_key.get(key(z, d))
        if not r:
            fails.append(f"ramp check {z}F {d}: no chart row")
            continue
        vals = []
        for v in e.values():
            try:
                vals.append(float(v))
            except (TypeError, ValueError):
                pass
        for x in (r[0][col][0], r[0][col][2]):
            if not any(abs(x - y) < 1e-9 for y in vals):
                fails.append(f"ramp check {z}F {d} {col}: printed {x} missing")

    # Check 5: the chart's own identities.
    print("\nchart identities (every parsed line, one per key):")
    print("Z  dia     col  feed   ramp   feed/Z   ramp-feed/Z  feed/(18000*Z*chip)")
    ident = []
    for k, rs in sorted(by_key.items()):
        r = rs[0]
        for col in ("wp", "mdf"):
            f, c, rd = r[col]
            fz = f / r["z"]
            ratio = f / (RPM * r["z"] * c)
            flag = ""
            if abs(rd - fz) > 0.5:
                flag += " RAMP"
            if abs(ratio - 1) > 0.05:
                flag += " RPM"
            ident.append((r["z"], r["dia"], col, f, rd, fz, ratio, flag))
            print(f"{r['z']}  {r['dia']:<7} {col:<4} {f:<6g} {rd:<6g} {fz:<8.2f} "
                  f"{rd - fz:+.2f}        {ratio:.3f}{flag}")

    # Check 6: the LUT rows.
    sp, all_ids = [], set()
    for f in sorted(glob.glob(os.path.join(LUT, "observations", "*.json"))):
        d = json.load(open(f))
        for o in d["observations"] if isinstance(d, dict) else d:
            all_ids.add(o["observation_id"])
            if o.get("source_id") == SOURCE and o["observation_id"].endswith("-spektra"):
                sp.append((os.path.basename(f), o))
    print(f"\nLUT: {len(sp)} Spektra rows (ids ending -spektra)")
    lut_cells = {}
    for fname, o in sp:
        k = key(o["flute_count"], o["diameter_mm"])
        lut_cells.setdefault(k, set()).add((o["material_family"], o["operation_family"]))
        r = by_key.get(k)
        col = "mdf" if o["material_family"] == "mdf" else "wp"
        if not r:
            print(f"  LUT row with no chart line: {o['observation_id']}")
            continue
        want = round(r[0][col][1] * 25.4, 4)
        if abs(o["chipload_max_mm_tooth"] - want) > 5e-5:
            print(f"  LUT DISCREPANCY {fname} {o['observation_id']}: "
                  f"{o['chipload_max_mm_tooth']} vs printed {want}")
        if (o.get("row_kind"), o.get("evidence_grade")) != (
                ("exact", "a") if col == "mdf" else ("derived", "b")):
            print(f"  LUT GRADE {fname} {o['observation_id']}: "
                  f"{o.get('row_kind')}/{o.get('evidence_grade')}")
    for o in cand["observations"]:
        if o["observation_id"] in all_ids:
            fails.append(f"candidate id already in the LUT: {o['observation_id']}")
    print("  pocket families per chart key (LUT | candidate):")
    cand_cells = {}
    for o in cand["observations"]:
        cand_cells.setdefault(key(o["flute_count"], o["diameter_mm"]), set()).add(o["material_family"])
    for k in sorted(by_key):
        lp = sorted(m for m, op in lut_cells.get(k, ()) if op == "pocket")
        cp = sorted(cand_cells.get(k, ()))
        print(f"    {k[0]}F {by_key[k][0]['dia']:<7} LUT {lp} | candidate {cp}")

    print(f"\ncandidate observations: {len(cand['observations'])}")
    print(f"transcription failures: {len(fails)}")
    for f in fails:
        print("  FAIL", f)
    sys.exit(1 if fails else 0)


if __name__ == "__main__":
    main()
