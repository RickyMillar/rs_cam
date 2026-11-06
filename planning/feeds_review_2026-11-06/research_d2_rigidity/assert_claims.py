"""Re-derive every number of the D2 findings from the stored files.

Run: python3 -I planning/feeds_review_2026-11-06/research_d2_rigidity/assert_claims.py
If FINDINGS.md exists next to this script, each claimed literal must also
appear in it.
"""

import importlib.util
import sys

sys.dont_write_bytecode = True
import re
from pathlib import Path

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("build_pairs", HERE / "build_pairs.py")
bp = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bp)

bp.check_classes()
hob, ind, pairs, nopair = bp.build()
st = bp.stats(pairs, hob, nopair)
S = bp.sig3

CLAIMS = []


def claim(name, got, want):
    assert got == want, f"{name}: got {got!r}, want {want!r}"
    CLAIMS.append((name, want))


# counts
claim("hobby rows", st["n_hobby_rows"], 60)
claim("pairs all", st["n_pairs_all"], 214)
claim("pairs primary", st["n_pairs_primary"], 115)
claim("primary hobby rows", st["n_hobby_rows_primary_paired"], 31)
claim("industrial rows used", st["n_industrial_rows_used"], 55)
claim("unpaired hobby rows", st["n_hobby_rows_unpaired"], 3)
claim_lines = sorted(int(re.search(r"\.txt:(\d+)", u).group(1)) for u in st["unpaired"])
assert claim_lines == [129, 153, 177], claim_lines  # the hard-block 1/16 in rows

# pooled primary
P = st["primary"]
for k, v in (("median", 0.368), ("q25", 0.244), ("q75", 0.749), ("min", 0.0947), ("max", 2.34), ("frac_below_1", 0.913)):
    claim(f"primary {k}", P[k], v)

# by hobby source
for src, n, med, q25, q75 in (("sienci_metric", 46, 0.645, 0.521, 0.782), ("c3d_s3", 9, 0.235, 0.12, 0.261),
                              ("c3d_nomad", 14, 1.09, 0.801, 1.83), ("idc", 46, 0.274, 0.185, 0.325)):
    g = st["by_hobby_source"][src]
    claim(f"{src} n", g["n"], n)
    claim(f"{src} median", g["median"], med)
    claim(f"{src} q25", g["q25"], q25)
    claim(f"{src} q75", g["q75"], q75)

for v, n, med in (("amana", 64, 0.578), ("freud", 22, 0.312), ("onsrud", 29, 0.284)):
    claim(f"vs {v} n", st["by_industrial_vendor"][v]["n"], n)
    claim(f"vs {v} median", st["by_industrial_vendor"][v]["median"], med)
for w, n, med in (("softwood", 44, 0.292), ("hardwood", 36, 0.333), ("MDF", 19, 0.625), ("plywood", 16, 0.624)):
    claim(f"{w} n", st["by_wood"][w]["n"], n)
    claim(f"{w} median", st["by_wood"][w]["median"], med)

# spread of the three low sources: 0.645 / 0.235
claim("low-source spread", S(st["by_hobby_source"]["sienci_metric"]["median"] / st["by_hobby_source"]["c3d_s3"]["median"]), 2.74)

# envelope
claim("envelope total", st["envelope_total"], {"below": 41, "inside": 11, "above": 0})
E = st["envelope_by_hobby_source"]
claim("envelope sienci", E["sienci_metric"], {"below": 14, "inside": 8})
claim("envelope s3", E["c3d_s3"], {"below": 5})
claim("envelope nomad", E["c3d_nomad"], {"below": 2, "inside": 3})
claim("envelope idc", E["idc"], {"below": 20})

# Sienci tiers
for t, med in (("reduced", 0.387), ("regular", 0.516), ("full", 0.645)):
    claim(f"tier {t} n", st["sienci_by_tier"][t]["n"], 46)
    claim(f"tier {t} median", st["sienci_by_tier"][t]["median"], med)

# decision rule band from IDC lines 64-65 ("about 20%")
idc_line = bp.line(bp.IDC, 65, "about 20%")
pct = float(re.search(r"(\d+)%", idc_line).group(1))
lo = 1 - pct / 100
claim("band low", S(lo), 0.8)
claim("band high", S(1 / lo), 1.25)
med = P["median"]
a_ok = (not lo <= med <= 1 / lo) and all(g["median"] < 1 for g in st["by_hobby_source"].values()) and P["q75"] < 1
b_ok = lo <= med <= 1 / lo
claim("outcome a holds", a_ok, False)
claim("outcome b holds", b_ok, False)
claim("nomad median inside band", lo <= st["by_hobby_source"]["c3d_nomad"]["median"] <= 1 / lo, True)

# depth rows
D = {re.search(r"\.txt:(\d+)", d["cite"]).group(1) + "@" + d["source"]: d for d in st["depth_rows"]}
for key, sod, chip in (("79@sienci_metric", 0.85, 0.114), ("170@sienci_metric", 0.394, 0.114),
                       ("83@sienci_metric", 1.73, 0.0794), ("174@sienci_metric", 0.819, 0.0794),
                       ("63@c3d_s3", 1.6, 0.0302), ("64@c3d_s3", 0.4, 0.029), ("65@c3d_s3", 1.0, 0.0447),
                       ("66@c3d_s3", 1.2, 0.0398), ("61@c3d_nomad", 0.24, 0.203), ("62@c3d_nomad", 0.112, 0.104)):
    claim(f"depth/D {key}", D[key]["slot_over_D"], sod)
    claim(f"chip {key}", D[key]["chip"], chip)
claim("idc 219 depth/D", D["219@idc"]["pocket_over_D"], 1.0)
claim("idc 219 chip", D["219@idc"]["chip"], 0.0468)
nomad = [d["slot_over_D"] for d in st["depth_rows"] if d["source"] == "c3d_nomad"]
claim("nomad depth min", min(nomad), 0.112)
claim("nomad depth max", max(nomad), 0.272)

claim("spearman n", st["depth_ratio_spearman"]["n"], 29)
claim("spearman rho", st["depth_ratio_spearman"]["rho"], -0.406)

W = st["wood_witnesses"]
claim("sienci wood chip", W["sienci_1_4_full"]["chip_hard_over_soft"], 1.0)
claim("sienci wood depth", W["sienci_1_4_full"]["depth_hard_over_soft"], 0.463)
claim("s3 wood chip", W["c3d_s3_pine_mahogany"]["chip_hard_over_soft"], 0.96)
claim("s3 wood depth", W["c3d_s3_pine_mahogany"]["depth_hard_over_soft"], 0.25)
claim("nomad wood chip", W["c3d_nomad_pine_mahogany"]["chip_hard_over_soft"], 0.51)
claim("nomad wood depth", W["c3d_nomad_pine_mahogany"]["depth_hard_over_soft"], 0.467)

# industrial depth condition, printed
bp.line(bp.LUT / "sources/amana_spektra_spiral_plunge_v24.txt", 3, "1 x")
bp.line(bp.LUT / "sources/onsrud_soft_wood_cutting_data.txt", 11, "1 x D Use recommended chip load")
bp.line(bp.REPO / "planning/extrapolation_2026-09-24/fetch/G4/sources/freud_router_bit_feed_and_speed_for_cnc_20170822.txt",
        57, "based on cut depth equal to bit diameter")

# IDC metric/imperial rpm inconsistency
claim("idc imperial 1/4 up rpm", int(bp.nums(bp.line(bp.IDC, 245, "1/4"))[-2]), 19000)
claim("idc metric 1/4 up rpm", int(re.search(r"(\d+),(\d{3})", bp.line(bp.IDC, 462, "1/4")).group(0).replace(",", "")), 15000)

# ToolsToday prose witnesses
tv = bp.G10 / "wood_toolstoday_calc_video.txt"
chart = float(re.search(r"starting point of (\d+) inches", bp.line(tv, 79, "Amana")).group(1))
desk = float(re.search(r"let's say (\d+) inches", bp.line(tv, 81, "tabletop CNC")).group(1))
claim("toolstoday video ratio", S(desk / chart), 0.462)
tg = bp.G10 / "wood_toolstoday_understanding_feeds_speeds.txt"
chart2 = float(re.search(r"feed rate of (\d+) IPM", bp.line(tg, 176, "MDF")).group(1))
desk2 = float(re.search(r"such as (\d+) IPM", bp.line(tg, 179, "desktop CNC")).group(1))
claim("toolstoday guide ratio", S(desk2 / chart2), 0.3)

# force budget rows named in CREDITS
bp.line(bp.REPO / "CREDITS.md", 939, "HOBBY_SLOT_POINTS")
bp.line(bp.REPO / "CREDITS.md", 948, "line 79")
bp.line(bp.REPO / "CREDITS.md", 951, "line 170")
bp.line(bp.REPO / "CREDITS.md", 954, "line 63")

# Sienci 3-flute what-if: chips scale by 2/3
claim("sienci 3F factor", S(2 / 3), 0.667)

# Section 6 (lead analysis, 2026-11-06): a hobby chart states a feed at its spindle speed.
import csv as _csv, math as _m, statistics as _st
_rows = [r for r in _csv.DictReader(open(HERE / "pairs.csv")) if r["primary"] == "yes"]
def _pear(a, b):
    ma, mb = _st.mean(a), _st.mean(b)
    return sum((x - ma) * (y - mb) for x, y in zip(a, b)) / _m.sqrt(sum((x - ma) ** 2 for x in a) * sum((y - mb) ** 2 for y in b))
_lr = [_m.log(float(r["ratio_hobby_over_mid"])) for r in _rows]
_rz = [float(r["hobby_rpm"]) * int(r["flutes"]) for r in _rows]
_lf = [_m.log(float(r["hobby_feed_mm_min"])) for r in _rows]
claim("s6 corr log ratio vs log rpmZ", S(_pear(_lr, [_m.log(x) for x in _rz])), -0.714)
claim("s6 corr log ratio vs log feed", S(_pear(_lr, _lf)), 0.396)
claim("s6 rpmZ min", int(min(_rz)), 8000)
claim("s6 rpmZ max", int(max(_rz)), 63000)
claim("s6 rpmZ span", S(max(_rz) / min(_rz)), 7.88)
_by = {}
for r in _rows:
    _by.setdefault(r["hobby_source"], []).append(float(r["hobby_feed_mm_min"]))
_med = {k: _st.median(v) for k, v in _by.items()}
claim("s6 median feed sienci", int(_med["sienci_metric"]), 2720)
claim("s6 median feed idc", int(_med["idc"]), 1778)
claim("s6 source median feed ratio", S(max(_med.values()) / min(_med.values())), 1.53)

findings = Path(sys.argv[1]) if len(sys.argv) > 1 else HERE / "FINDINGS.md"
if findings.exists():
    text = findings.read_text()
    for name, v in CLAIMS:
        if isinstance(v, (int, float)) and not isinstance(v, bool):
            assert str(v) in text, f"FINDINGS.md lacks {name} = {v}"
print(f"{len(CLAIMS)} claims pass" + (" (FINDINGS.md checked)" if findings.exists() else " (FINDINGS.md absent)"))
