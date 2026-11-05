#!/usr/bin/env python3
"""Build stiffness_table.csv from the text extractions in sources/.

Every value is parsed from a line of a text extraction; the line number is
the 1-based line in that file (header included). Sienci prints no stiffness
value, so `stiffness_printed` is empty on every row. The column
`stiffness_derived_N_per_mm` is DERIVED here as force / deflection (secant
through the origin); it is not printed by any source.

Run: python3 -I build_table.py   (from any directory)
"""
import csv
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
SRC = HERE / "sources"
THOU_MM = 0.0254  # 1 thou = 0.001 in = 0.0254 mm (definition of the inch)

rows = []


def lines_of(name):
    return (SRC / name).read_text(encoding="utf-8").split("\n")


def add(machine, axis, direction, force, defl, defl_unit, defl_mm_printed, src, line, note):
    defl_s = str(defl)
    defl = float(defl_s)
    defl_mm = defl * THOU_MM if defl_unit.startswith("thou") else defl
    k = force / defl_mm if defl_mm > 0 else ""
    rows.append(
        {
            "machine": machine,
            "axis": axis,
            "direction": direction,
            "force": force,
            "force_unit": "N",
            "deflection": defl_s,
            "deflection_unit": defl_unit,
            "deflection_mm_printed": defl_mm_printed,
            "stiffness_printed": "",
            "stiffness_derived_N_per_mm": f"{k:.1f}" if k != "" else "",
            "source_file": f"sources/{src}",
            "line": line,
            "note": note,
        }
    )


# --- A/B: the PDF report and the MK2 blog post (same table layout) ---------
ROW_RE = re.compile(
    r"(\d+[x×]30) Longmill MK2\s*\|?\s*([\d.]+) thou / ([\d.]+)mm\s*\|?\s*([\d.]+) thou / ([\d.]+)mm"
)


def mk2_tables(src, header_res, notes):
    ls = lines_of(src)
    force = None
    note = ""
    for i, l in enumerate(ls, 1):
        for hre, f, n in zip(header_res, (10.0, None), notes):
            m = re.search(hre, l)
            if m:
                force = f if f is not None else float(m.group(1))
                note = n
        m = ROW_RE.search(l)
        if m and force is not None:
            mach = f"LongMill MK2 {m.group(1)}".replace("×", "x")
            add(mach, "X", "mean of +/-", force, m.group(2), "thou", m.group(3), src, i, note)
            add(mach, "Y", "mean of +/-", force, m.group(4), "thou", m.group(5), src, i, note)


mk2_tables(
    "sienci_stiffness_rating_with_commentary.txt",
    [r"\(Tested at 10N\)", r"\(Tested at (25)N\)"],
    [
        "PDF report, load on the inverted 0.25in end mill",
        "PDF prints 25 N; Sienci corrected this table to 50 N on 2026-03-18 (MK2 blog) and the xlsx raw data is at 50 N",
    ],
)
mk2_tables(
    "sienci_blog_mk2_stiffness_2022.txt",
    [r"\(Tested at 10N\)", r"\(Tested at ~~25N~~ (50)N\)"],
    ["MK2 blog", "MK2 blog, 25 N struck through and replaced by 50 N (update 2026-03-18)"],
)

# --- C: the xlsx raw data, sheet "Final Deflection Measurement" -----------
src = "sienci_mk2_deflection_testing_xlsx.txt"
ls = lines_of(src)
sheet = None
mach = axis = None
for i, l in enumerate(ls, 1):
    if l.startswith("=== sheet:"):
        sheet = l
        continue
    if sheet != "=== sheet: Final Deflection Measurement ===":
        continue
    m = re.search(r"A\d+='(\d+x30) \(([^)]*)\)'", l)
    if m:
        mach = f"LongMill MK2 {m.group(1)}"
        pos = m.group(2)
    m = re.search(r"B\d+='([XY])-Axis'", l)
    if m:
        axis = m.group(1)
    mg = re.search(r"C\d+='([\d.]+)'.*G\d+='([\d.]+)'.*H\d+='([\d.]+)'", l)
    if mg:
        add(
            mach,
            axis,
            "mean of +/-, 3 runs",
            float(mg.group(1)),
            mg.group(2),
            "thou",
            mg.group(3),
            src,
            i,
            f"xlsx raw data, unrounded run average; position {pos}",
        )

# --- E: the xlsx sheet "Linearity" (machine not named in the sheet) -------
sheet = None
for i, l in enumerate(ls, 1):
    if l.startswith("=== sheet:"):
        sheet = l
        continue
    if sheet != "=== sheet: Linearity ===":
        continue
    m = re.match(r"A\d+='([\d.]+)'\s+([BCD])\d+='([\d.]+)'", l)
    if m:
        ax = {"B": "Y", "C": "X", "D": "45deg"}[m.group(2)]
        add(
            "LongMill MK2 (size not named in sheet)",
            ax,
            "one direction",
            float(m.group(1)),
            m.group(3),
            "thou (unit not printed in sheet)",
            "",
            src,
            i,
            "xlsx Linearity sheet; deflection unit not printed (the thou column convention of the workbook)",
        )

# --- D: the MK3 blog tables (first occurrence of each table only) ---------
src = "sienci_blog_mk3_rigidity_2026.txt"
ls = lines_of(src)
mach = None
seen = set()
for i, l in enumerate(ls, 1):
    m = re.match(r"\s*\|\s*(LongMill MK3 48×30|AltMill 4×4|LongMill MK2 48×30)\s*\|", l)
    if m:
        mach = m.group(1).replace("×", "x")
        if mach in seen:
            mach = None  # a repeated table; skip it
        else:
            seen.add(mach)
        continue
    if l.strip().startswith("| Comparison"):
        mach = None
    if mach is None:
        continue
    cells = [c.strip() for c in l.split("|")[1:]]
    if len(cells) == 6 and re.fullmatch(r"\d+", cells[0]):
        force = float(cells[0])
        for idx, (ax, d) in zip(range(2, 6), [("X", "+"), ("X", "-"), ("Y", "+"), ("Y", "-")]):
            v = cells[idx]
            if v == "NA":
                continue
            note = "MK3 blog; load from a force gauge pushed against the router by a lead-screw jig"
            if v.startswith("~"):
                note = "MK3 blog; printed with ~ and called extrapolated in the comparison table"
            if mach.startswith("LongMill MK2"):
                note += "; MK2 values copied from the 2022 test"
            add(mach, ax, d, force, v.lstrip("~"), "thou", "", src, i, note)

out = HERE / "stiffness_table.csv"
with out.open("w", newline="") as f:
    w = csv.DictWriter(f, fieldnames=list(rows[0].keys()))
    w.writeheader()
    w.writerows(rows)
print(f"wrote {len(rows)} rows to {out}", file=sys.stderr)
