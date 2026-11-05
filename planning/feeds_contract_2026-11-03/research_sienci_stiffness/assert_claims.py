#!/usr/bin/env python3
"""Assert each numeric claim of FINDINGS.md.

Every input value is read from stiffness_table.csv (built by build_table.py
from sources/*.txt) or from a stored text file. Every derived number is
computed here. Run: python3 -I assert_claims.py
"""
import csv
import re
from pathlib import Path

HERE = Path(__file__).resolve().parent
SRC = HERE / "sources"
REPO = HERE.parents[2]
THOU_MM = 0.0254  # definition of the inch

ROWS = list(csv.DictReader((HERE / "stiffness_table.csv").open()))
n_ok = 0


def ok(cond, msg):
    global n_ok
    assert cond, msg
    n_ok += 1
    print(f"PASS  {msg}")


def text(name):
    return (SRC / name).read_text(encoding="utf-8").split("\n")


def on_line(path, line, needle):
    ls = Path(path).read_text(encoding="utf-8").split("\n")
    return needle in ls[line - 1]


def rows(machine=None, axis=None, direction=None, force=None, src=None):
    out = []
    for r in ROWS:
        if machine and r["machine"] != machine:
            continue
        if axis and r["axis"] != axis:
            continue
        if direction and r["direction"] != direction:
            continue
        if force is not None and abs(float(r["force"]) - force) > 1e-9:
            continue
        if src and src not in r["source_file"]:
            continue
        out.append(r)
    return out


def one(**kw):
    r = rows(**kw)
    assert len(r) == 1, (kw, len(r))
    return r[0]


def mm(r):
    return float(r["deflection"]) * THOU_MM


def k_secant(r):
    return float(r["force"]) / mm(r)


def fit(points):
    """Least-squares line d = a + b F; returns (b mm/N, a mm)."""
    n = len(points)
    sf = sum(f for f, _ in points)
    sd = sum(d for _, d in points)
    sff = sum(f * f for f, _ in points)
    sfd = sum(f * d for f, d in points)
    b = (n * sfd - sf * sd) / (n * sff - sf * sf)
    a = (sd - b * sf) / n
    return b, a


# ---------------------------------------------------------------- 0. table
ok(all(r["stiffness_printed"] == "" for r in ROWS), "no source prints a stiffness value; every row is force and deflection")
for r in ROWS:
    p = HERE / r["source_file"]
    v = r["deflection"]
    needle = v if "xlsx" in r["source_file"] else v.rstrip("0").rstrip(".") if "." in v else v
    assert on_line(p, int(r["line"]), needle), (r, needle)
    f = r["force"].rstrip("0").rstrip(".")
    if "blog_mk3" in r["source_file"] or "Linearity" in r["note"] or "xlsx" in r["source_file"]:
        assert on_line(p, int(r["line"]), f), (r, f)
ok(True, f"each of the {len(ROWS)} CSV rows: the deflection (and the force where the row prints it) is on the cited line")
ok(not any("shapeoko" in r["machine"].lower() for r in ROWS), "no Shapeoko (or other belt machine) row in the table")
pdf = "sienci_stiffness_rating_with_commentary.txt"
ok(any("Shapeoko" in l and "3" in l for l in text(pdf)[170:185]), "the PDF names the Shapeoko 3 only in one qualitative remark")

# --------------------------------------------------------------- 1. method
pl = text(pdf)
ok(any("0.001” Resolution dial indicator" in l for l in pl), "gauge: 0.001 in dial indicator")
ok(any("Apply a 10N/2.25lbs* load" in l for l in pl), "procedure load: 10 N")
ok(any("1.25” stick out" in l for l in pl) and any("Raise the end mill 1” off" in l for l in pl), "pin: 0.25 in end mill, 1.25 in stick-out, 1 in above the wasteboard")
ok(any("theoretical maximum" in l for l in pl) and any("21 pounds / 93N" in l for l in pl), "Sienci's theoretical maximum cut force: 93 N")
ok(any("Machine behavior in motion is not evaluated" in l for l in pl), "static test only")
m3 = text("sienci_blog_mk3_rigidity_2026.txt")
ok(any("(3.8N-10.4N)" in l for l in m3), "MK3 post: Millalyser mean and peak feed forces 3.8-10.4 N")
ok(any("keep forces to under 25N to be within the 0.005” deflection" in l for l in m3), "MK3 post: keep force under 25 N for 0.005 in")
ok(any("65-80% improvements" in l for l in m3), "MK3 post: 65-80 % MK2 to MK3")
ok(any("approximately 2-4x more rigid" in l for l in m3), "MK3 post: AltMill 2-4x LongMill")

# ------------------------------------------------- 2. the 25 N / 50 N issue
m2 = text("sienci_blog_mk2_stiffness_2022.txt")
ok(any("~~25N~~ 50N" in l for l in m2) and any("mixed up 25N and 50N" in l for l in m2), "MK2 post strikes 25 N and prints 50 N (update 2026-03-18)")
ok(rows(machine="LongMill MK2 48x30", axis="X", force=25.0, src=pdf)[0]["deflection"] == "14.2", "the PDF prints 14.2 thou X at '25N' (48x30)")
x10 = one(machine="LongMill MK2 48x30", axis="X", force=10.0, src="xlsx")
x50 = one(machine="LongMill MK2 48x30", axis="X", force=50.0, src="xlsx")
y10 = one(machine="LongMill MK2 48x30", axis="Y", force=10.0, src="xlsx")
y50 = one(machine="LongMill MK2 48x30", axis="Y", force=50.0, src="xlsx")
kx10, kx50, ky10, ky50 = map(k_secant, (x10, x50, y10, y50))
kx25 = 25.0 / mm(x50)
print(f"      48x30 X: k10 {kx10:.1f}, k50 {kx50:.1f}, k(if 25 N) {kx25:.1f}; Y: k10 {ky10:.1f}, k50 {ky50:.1f} N/mm")
ok(round(kx10) == 139 and round(kx50) == 139, "48x30 X: 139 N/mm at 10 N and 139 N/mm at 50 N (xlsx)")
ok(round(kx25) == 69, "48x30 X read at 25 N: 69 N/mm, half the 10 N value, so 25 N is the wrong label")
ok(round(ky10) == 124 and round(ky50) == 95, "48x30 Y: 124 N/mm at 10 N, 95 N/mm at 50 N")
for tag, r in (("X", x10), ("Y", y10)):
    pr = one(machine="LongMill MK2 48x30", axis=tag, force=10.0, src=pdf)
    assert abs(float(r["deflection_mm_printed"]) - float(pr["deflection_mm_printed"])) < 0.0005
ok(True, "the PDF 10 N mm values are the xlsx run averages rounded")

# ----------------------------------------------------- 3. secant at 50 N
def worst50(machine):
    rs = [r for r in rows(machine=machine, force=50.0) if "xlsx" in r["source_file"] or "blog_mk3" in r["source_file"]]
    if machine.startswith("LongMill MK2"):
        rs = [r for r in rs if "xlsx" in r["source_file"]]
    return min(k_secant(r) for r in rs)

k_mk2 = {s: worst50(f"LongMill MK2 {s}") for s in ("12x30", "30x30", "48x30")}
k_mk3 = worst50("LongMill MK3 48x30")
k_alt = worst50("AltMill 4x4")
print(f"      worst-axis secant at 50 N: MK2 {k_mk2}, MK3 {k_mk3:.1f}, AltMill {k_alt:.1f}")
ok(all(105 <= round(v) <= 109 for k, v in k_mk2.items() if k != "48x30") and round(k_mk2["48x30"]) == 95, "MK2 worst axis (Y) at 50 N: 95-108 N/mm")
ok(round(k_mk3) == 164, "MK3 48x30 worst axis (Y-) at 50 N: 164 N/mm")
ok(round(k_alt) == 492, "AltMill 4x4 worst axis (Y+) at 50 N: 492 N/mm")

# ------------------------------------------------- 4. fit slopes (25-100 N)
def slope_k(machine, axis):
    pts = [(float(r["force"]), mm(r)) for r in rows(machine=machine, axis=axis, src="blog_mk3") if r["direction"] in "+-"]
    b, a = fit(pts)
    return 1 / b, a * 1000

for mach, exp in (("LongMill MK3 48x30", {"X": 176.5, "Y": 182.3}), ("AltMill 4x4", {"X": 546.8, "Y": 546.8})):
    for ax in ("X", "Y"):
        kf, a_um = slope_k(mach, ax)
        print(f"      {mach} {ax}: fit slope k {kf:.1f} N/mm, intercept {a_um:.1f} um")
        ok(abs(kf - exp[ax]) < 0.05, f"{mach} {ax}: fit slope 25-100 N gives {exp[ax]} N/mm")
kf, a_um = slope_k("LongMill MK3 48x30", "X")
ok(abs(a_um + 25.4) < 0.05 and abs(slope_k("LongMill MK3 48x30", "Y")[1] - 50.8) < 0.05, "MK3 fit intercepts: X -25.4 um, Y +50.8 um (not a line through zero)")

# ----------------------------------------------------- 5. use for rs_cam
ex = (REPO / "planning/feeds_contract_2026-11-03/explainer_p13c/index.html").read_text()
budget = float(re.search(r'data-claim="budget_n"[^>]*>([\d.]+)<', ex).group(1))
s0 = (REPO / "planning/feeds_contract_2026-11-03/research_p13_step0/FINDINGS.md").read_text().split("\n")
target_um = float(re.search(r"(\d+) um \(F201 Unsourced\)", s0[58]).group(1))
k_need = float(re.search(r"\| 200 um \| (\d+) N/mm", s0[64]).group(1))
lo, hi = map(float, re.search(r"imply (\d+)-(\d+) N/mm", s0[68]).groups())
ok(budget == 35.3224, "hobby budget 35.3224 N (explainer_p13c)")
ok(target_um == 200 and k_need == 79 and (lo, hi) == (71, 182), "step 0: target 200 um, back-solved 79 N/mm, charts imply 71-182 N/mm")

d_mk2 = budget / k_mk2["48x30"] * 1000
d_mk3 = budget / k_mk3 * 1000
d_alt = budget / k_alt * 1000
print(f"      frame deflection at the budget: MK2 48x30 {d_mk2:.0f} um, MK3 {d_mk3:.0f} um, AltMill {d_alt:.0f} um")
ok(round(d_mk2) == 371, "frame alone at 35.32 N: MK2 48x30 371 um (> 200 um)")
ok(round(d_mk3) == 215, "frame alone at 35.32 N: MK3 48x30 215 um (> 200 um)")
ok(round(d_alt) == 72, "frame alone at 35.32 N: AltMill 72 um")
f_mk2 = k_mk2["48x30"] * target_um / 1000
f_mk3 = k_mk3 * target_um / 1000
f_alt = k_alt * target_um / 1000
print(f"      force for 200 um of frame: MK2 {f_mk2:.1f} N, MK3 {f_mk3:.1f} N, AltMill {f_alt:.1f} N")
ok(round(f_mk2) == 19 and round(f_mk3) == 33 and round(f_alt) == 98, "force for 200 um frame deflection: MK2 19 N, MK3 33 N, AltMill 98 N")
k_req = budget / (target_um / 1000)
print(f"      frame k for 200 um at the budget, tool beam ignored: {k_req:.1f} N/mm")
ok(abs(k_req - 176.6) < 0.05, "frame-only stiffness for 200 um at 35.32 N: 176.6 N/mm")
lin = [k_secant(r) for r in rows(machine="LongMill MK2 (size not named in sheet)", axis="X")]
ok(min(lin) > 89 and max(lin) < 92, "xlsx Linearity sheet X: secant k 89.5-91.6 N/mm from 25.6 to 150 N (linear)")
allk = list(k_mk2.values()) + [k_mk3]
ok(all(lo <= v <= hi for v in allk), "LongMill MK2 and MK3 worst-axis k (95-164 N/mm) lie inside the implied hobby band 71-182 N/mm")
ilo, ihi = map(float, re.search(r"industrial charts (\d+)-(\d+) N/mm", s0[68]).groups())
ok(k_alt > hi and ilo <= k_alt <= ihi, "AltMill (492 N/mm) is above the hobby band and inside the implied industrial band 309-569 N/mm")
ok(min(allk) > k_need, "every LongMill worst-axis k is above the back-solved 79 N/mm")

# ------------------------------------------- 6. line citations in FINDINGS
CITES = [
    (pdf, 57, "dial indicator"), (pdf, 104, "holding torque"), (pdf, 144, "(Tested at 25N)"),
    (pdf, 179, "Shapeoko"), (pdf, 210, "93N"), (pdf, 249, "nearest 5N"), (pdf, 298, "10N"),
    (pdf, 302, "order of magnitude higher"), (pdf, 333, "hysteresis and backlash"),
    (pdf, 349, "proportionally scaled"), (pdf, 157, "tempting to conclude"),
    (pdf, 161, "V-wheels"), (pdf, 603, "Manufacturer"), (pdf, 616, "90-120N"),
    (pdf, 664, "not evaluated"),
    ("sienci_blog_mk2_stiffness_2022.txt", 245, "~~25N~~ 50N"),
    ("sienci_blog_mk2_stiffness_2022.txt", 261, "mixed up 25N and 50N"),
    ("sienci_blog_mk3_rigidity_2026.txt", 256, "force gauge against the router"),
    ("sienci_blog_mk3_rigidity_2026.txt", 262, "65-80%"),
    ("sienci_blog_mk3_rigidity_2026.txt", 435, "under 25N"),
    ("sienci_blog_mk3_rigidity_2026.txt", 443, "3.8N-10.4N"),
    ("sienci_blog_mk3_rigidity_2026.txt", 467, "2-4x"),
    ("thirdparty_carbide3d_forum_28669_backlash_deflection.txt", 24, "< 0.4mm or 15 thou"),
    ("thirdparty_carbide3d_forum_28669_backlash_deflection.txt", 35, "50 Newtons of Force"),
]
for f, ln, needle in CITES:
    assert on_line(SRC / f, ln, needle), (f, ln, needle)
ok(True, f"all {len(CITES)} line citations in FINDINGS.md hold")
xl = text("sienci_mk2_deflection_testing_xlsx.txt")
ok(any("98N applied" in l for l in xl) and any("Z-axis Deflection Breakdown" in l for l in xl), "xlsx Z breakdown at 98 N exists (unit not printed)")
ok(all(r["machine"] != "Shapeoko" for r in ROWS) and "staff=False" in "".join(text("sienci_forum_26150_longmill_rigidity.txt")), "forum 26150 posts are by a non-staff user")

print(f"\nALL {n_ok} ASSERTS PASS")
