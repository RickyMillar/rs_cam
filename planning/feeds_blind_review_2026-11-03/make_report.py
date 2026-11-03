"""Phase 4: write report.html from compare.json, the briefs and the Suggest jsonl.

Run: python3 -I make_report.py (from this folder). It imports compare_calc.py,
which writes compare.md and compare.json again (deterministic) and asserts its
own claims. The page renders only the data embedded here. assert_report.py
reads that data back from report.html and recomputes every chip load, every
power figure and every machine-limit check with its own formulas.
"""

import json
import os
import re
import sys
import tomllib

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import compare_calc as cc  # noqa: E402

MACHINE = os.path.expanduser("~/.config/rs_cam/machines/shapeoko_pro_xxl_scylla.toml")

with open(os.path.join(HERE, "compare.json")) as fh:
    C = json.load(fh)
with open(os.path.join(HERE, "briefs", "INDEX.md")) as fh:
    INDEX = fh.read()


def brief_for(op):
    for line in INDEX.splitlines():
        m = re.match(r"\|\s*`(\d\d_[^`]+\.md)`\s*\|\s*([\d, ]+)\s*\|", line)
        if m and op in [int(x) for x in m.group(2).split(",")]:
            return m.group(1)
    return None


def brief_summary(fname):
    with open(os.path.join(HERE, "briefs", fname)) as fh:
        text = fh.read()
    out = []
    for key in ("What it cuts", "Material", "Entry"):
        m = re.search(r"^- " + key + r"[^:]*:\s*(.+(?:\n  .+)*)", text, re.M)
        if m:
            out.append(re.sub(r"\s+", " ", m.group(1)).strip())
    return out


# Machine facts (the file the reviewers read).
with open(MACHINE, "rb") as fh:
    mach = tomllib.load(fh)


def find(d, *keys):
    for k in keys:
        if isinstance(d, dict) and k in d:
            return d[k]
    for v in (d.values() if isinstance(d, dict) else []):
        if isinstance(v, dict):
            r = find(v, *keys)
            if r is not None:
                return r
    return None


machine = dict(
    rpm_min=find(mach, "min_rpm"),
    rpm_max=find(mach, "max_rpm"),
    feed_cap_mm_min=6000,
    feed_cap_note="rs_cam software cutting-feed cap (brief 00), not a machine fact; the machine max_feed is 15000",
    p_rated_kw=find(mach, "rated_power_kw"),
    n_rated_rpm=find(mach, "rated_rpm"),
    power_rule="P_avail = P_rated x min(n, n_rated) / n_rated (the constant-torque line of the machine power curve)",
    rubbing_floor_mm=C["rubbing_floor_mm"],
)

assert machine["p_rated_kw"] == cc.P_RATED_KW and machine["n_rated_rpm"] == cc.N_RATED, machine

# Chip inputs per op and per column, so the assert script can recompute.
SUGL = {}
with open(os.path.join(HERE, "suggest", "suggest_operator.jsonl")) as fh:
    for line in fh:
        d = json.loads(line)
        SUGL[d["index"]] = d


def chip_inputs(op, vals):
    t = cc.TOOLS[cc.TOOLKEY[op]]
    ae, ap = cc.ENG_OP.get(op, (None, None))
    if op in (1, 14, 2, 15, 6, 7, 8, 11) and vals.get("stepover"):
        ae = vals["stepover"]
    if op in (1, 14, 2, 15, 16) and vals.get("depth_per_pass"):
        ap = vals["depth_per_pass"]
    if op in (3, 4):
        kind = "drill"
    elif cc.TOOLKEY[op] in ("v20", "v60"):
        kind = "vbit"
    elif cc.TOOLKEY[op] == "r1":
        kind = "ball_shallow" if op == 17 else "taper_ball"
    else:
        kind = "flat"
    return dict(kind=kind, d=t["d"], z=t["z"], half=t.get("half"), R=t.get("R"), alpha=t.get("alpha"),
                ae=ae, ap=ap, rpm=vals.get("rpm"), feed=vals.get("feed"), plunge=vals.get("plunge"))


ops = []
for o in C["ops"]:
    i = o["index"]
    f = o["fields"]
    cols = {}
    for who in ("operator", "suggest", "recommended"):
        vals = {k: f[k][who] for k in ("rpm", "feed", "plunge", "stepover", "depth_per_pass") if k in f}
        if who == "recommended":
            base = {k: f[k]["operator"] for k in ("rpm", "feed", "plunge", "stepover", "depth_per_pass") if k in f}
            base.update({k: v for k, v in vals.items() if v is not None})
            vals = base
        cols[who] = chip_inputs(i, vals)
    bf = brief_for(i)
    ops.append(dict(o, brief=bf, brief_summary=brief_summary(bf) if bf else [], chip_inputs=cols))

# The six dangerous groups, in the order of compare.md.
TOP = [
    dict(ops=[1], who="operator and Suggest", title="Back bulk rough: power over the curve",
         text="9 x 3.6 mm at 5999 mm/min and 16219 rpm. The 2026-11-02 simulation measured a 1.18 kW peak against 1.01 kW available, on rs_cam's own force line. Suggest also sets the depth per pass to 14.05 mm.",
         action="Use 9 x 2.7 mm at 6000 mm/min and 24000 rpm, ramp 2 deg, if the bit is rated for 24000 rpm. Else use 18000 rpm and 4500 mm/min."),
    dict(ops=[5], who="Suggest only", title="River channel: do not apply Suggest",
         text="Suggest gives 0.382 mm/tooth in a full slot up to 10 mm deep and a 2999 mm/min plunge. It reads a finishing row and sees no radial engagement on a Project Curve.",
         action="Keep the operator values (826 mm/min at 10610 rpm). Review A allows 1260-1620 mm/min in one pass, 2880 mm/min in two passes of 5 mm."),
    dict(ops=[2], who="operator and Suggest", title="Back rest 6 mm: the 14.5 mm entry",
         text="The entry and the 13 mm link crossings cut full width. rs_cam's own entry-load gate flagged it on 2026-11-02. No feed makes this safe.",
         action="Change the path (entry and links). Then lower the feed to about 3600 mm/min at 18000 rpm."),
    dict(ops=[3], who="operator and Suggest", title="Pin drill: the hole is deeper than the flute",
         text="The hole is 30 mm deep. The flute is 25 mm and the stick-out is 30 mm, so the collet nut reaches the stock top and chips pack on the shank.",
         action="Use a tool with a flute of 33 mm or more, or drill the oak to 23 mm only. Suggest writes a 15 mm peck; keep 6 mm or less."),
    dict(ops=[6, 7], who="operator and Suggest", title="DMM 2 mm: straight plunges",
         text="Each level plunges 0.5 mm straight into oak (entry style none). Nobody knows if the DMM cuts at the centre. Op 8 ramps every level and is not in this item.",
         action="Check the end of the bit with a loupe. Until then, start each level in an opening that op 2 cut, or ramp."),
    dict(ops=[14], who="Suggest only", title="Front bulk rough: Suggest depth 14.05 mm",
         text="Suggest makes one 14.05 mm level for the front relief, 3.4 x the planned 4.17 mm.",
         action="Keep the operator depth (4.17 mm)."),
]

DATA = dict(date=C["date"], machine=machine, classification_rule=C["classification_rule"],
            chip_formulas=C["chip_formulas"], tolerance_pct=C["tolerance_pct"], energy=C["energy"],
            ops=ops, flagged=C["flagged"], top=TOP, errata=C["review_errata"], gate_checks=C["gate_checks"],
            needs_go_ahead=C["needs_operator_go_ahead"], unresolved=C["unresolved"])

with open(os.path.join(HERE, "report_template.html")) as fh:
    tpl = fh.read()
payload = json.dumps(DATA, separators=(",", ":")).replace("</", "<\\/")
html = tpl.replace("/*__DATA__*/null", payload)
assert "/*__DATA__*/" not in html
with open(os.path.join(HERE, "report.html"), "w") as fh:
    fh.write(html)
print("report.html", len(html), "bytes,", len(ops), "ops,", len(C["flagged"]), "flagged fields")
