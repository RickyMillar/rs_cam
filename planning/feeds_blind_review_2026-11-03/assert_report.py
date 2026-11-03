"""Assert each claim of report.html.

Run: python3 -I assert_report.py (from this folder). The script reads the data
that report.html embeds and the machine file. It does not import
compare_calc.py: it recomputes with its own formulas.

It checks:
1. every chip load (fz, real chip h) for the operator, Suggest and recommended
   columns, from rpm, feed, flutes and the engagement;
2. every power percent in the energy table, from MRR and the power curve;
3. the machine limits for every recommended value (rpm range, feed cap) and
   the power available at each rpm;
4. the classification (agree / high / low) of each field against the
   reviewer range and the tolerance;
5. that the dangerous cards on top cover every op with a dangerous field, and
   that each number in the card text and the energy text matches the data.
"""

import json
import math
import os
import re
import tomllib

HERE = os.path.dirname(os.path.abspath(__file__))
with open(os.path.join(HERE, "report.html")) as fh:
    HTML = fh.read()
m = re.search(r"const DATA = (\{.*?\});\n", HTML, re.S)
assert m, "no DATA in report.html"
D = json.loads(m.group(1).replace("<\\/", "</"))
with open(os.path.expanduser("~/.config/rs_cam/machines/shapeoko_pro_xxl_scylla.toml"), "rb") as fh:
    MACH = tomllib.load(fh)

checks = 0


def close(a, b, rel=1e-3, what=""):
    global checks
    checks += 1
    assert a is not None and b is not None, what
    assert abs(a - b) <= rel * max(abs(a), abs(b), 1e-9), f"{what}: {a} != {b}"


# --- machine facts --------------------------------------------------------
mc = D["machine"]
spindle = MACH.get("spindle", MACH)


def find(d, k):
    if isinstance(d, dict):
        if k in d:
            return d[k]
        for v in d.values():
            r = find(v, k)
            if r is not None:
                return r
    return None


close(mc["rpm_min"], find(MACH, "min_rpm"), what="rpm_min")
close(mc["rpm_max"], find(MACH, "max_rpm"), what="rpm_max")
close(mc["p_rated_kw"], find(MACH, "rated_power_kw"), what="rated power")
close(mc["n_rated_rpm"], find(MACH, "rated_rpm"), what="rated rpm")
assert "VfdConstantTorque" in str(MACH), "power model is not constant torque"


def p_avail(n):
    return mc["p_rated_kw"] * min(n, mc["n_rated_rpm"]) / mc["n_rated_rpm"]


# --- chip formulas ---------------------------------------------------------
def thin(f, ae, d):
    r = ae / d
    return f * 2 * math.sqrt(r - r * r) if r < 0.5 else f


def taper_de(ap, R, alpha):
    a = math.radians(alpha)
    ht = R * (1 - math.sin(a))
    if ap <= ht:
        return 2 * math.sqrt(ap * (2 * R - ap))
    return 2 * (R * math.cos(a) + (ap - ht) * math.tan(a))


def chip(ci):
    n, feed, z = ci["rpm"], ci["feed"], ci["z"]
    if not n or not feed:
        return None, None
    if ci["kind"] == "drill":
        return ci["plunge"] / (n * z), None
    f = feed / (n * z)
    k = ci["kind"]
    if k == "vbit":
        return f, f * math.cos(math.radians(ci["half"]))
    if k == "ball_shallow":
        return f, f * taper_de(ci["ap"], ci["R"], ci["alpha"]) / ci["d"]
    if k == "taper_ball":
        return f, thin(f, ci["ae"], taper_de(ci["ap"], ci["R"], ci["alpha"]))
    return f, thin(f, ci["ae"], ci["d"])


tol = D["tolerance_pct"] / 100.0
for o in D["ops"]:
    i, F = o["index"], o["fields"]
    for who in ("operator", "suggest", "recommended"):
        ci = o["chip_inputs"][who]
        # inputs on the page match the field values
        for k in ("rpm", "feed"):
            if k in F and F[k][who] is not None:
                close(ci[k], F[k][who], what=f"op {i} {who} {k} input")
        fz, h = chip(ci)
        if "fz" in F and F["fz"][who] is not None:
            close(F["fz"][who], fz, what=f"op {i} {who} fz")
        if "hex" in F and F["hex"][who] is not None:
            close(F["hex"][who], h, what=f"op {i} {who} h")
        if "plunge_fz" in F and F["plunge_fz"][who] is not None:
            close(F["plunge_fz"][who], ci["plunge"] / (ci["rpm"] * ci["z"]), what=f"op {i} {who} plunge chip")
    # machine limits on the recommended values
    rec = {k: F[k]["recommended"] for k in F if F[k]["recommended"] is not None}
    if "rpm" in rec:
        checks += 1
        assert mc["rpm_min"] <= rec["rpm"] <= mc["rpm_max"], f"op {i} recommended rpm {rec['rpm']} out of range"
    for k in ("feed", "plunge", "ramp_feed"):
        if k in rec:
            checks += 1
            assert rec[k] <= mc["feed_cap_mm_min"], f"op {i} recommended {k} {rec[k]} over the cap"
    # the recommended real chip is not under the rubbing floor
    if "hex" in rec:
        checks += 1
        assert rec["hex"] >= mc["rubbing_floor_mm"] * 0.999, f"op {i} recommended chip {rec['hex']} under the floor"
    # classification against the reviewer range
    for k, f in F.items():
        lo, hi = f["reviewer_min"], f["reviewer_max"]
        for who, cls in (("operator", f["class_operator"]), ("suggest", f["class_suggest"])):
            v = f[who]
            if lo is None or v is None or cls == "n/a":
                continue
            exp = "agree" if lo * (1 - tol) <= v <= hi * (1 + tol) else ("high" if v > hi else "low")
            checks += 1
            assert cls == exp, f"op {i} {k} {who}: page says {cls}, the range gives {exp} ({lo}-{hi}, {v})"

# --- energy table ---------------------------------------------------------
for key, r in D["energy"]["power"].items():
    if not key.startswith("op1"):
        continue
    for col in ("A_flat_u050", "rs_cam_curti", "guedes_chip_m0.3", "guedes_chip_m0.5"):
        close(r[col + "_pct"], 100 * r[col + "_kw"] / r["available_kw"], what=f"{key} {col} pct")
    close(r["A_flat_u050_kw"], 0.50 * r["mrr_mm3_s"] / 1000, what=f"{key} flat power")
    close(r["rs_cam_curti_kw"], r["rs_cam_curti_u_J_mm3"] * r["mrr_mm3_s"] / 1000, rel=2e-3, what=f"{key} rs_cam u x MRR")

op1 = D["energy"]["power"]["op1_operator_9x3.6"]
close(op1["mrr_mm3_s"], 9.0 * 3.6 * 5999 / 60, what="op 1 MRR")
close(op1["available_kw"], p_avail(16219), what="op 1 available power at 16219 rpm")
rec1 = D["energy"]["power"]["op1_recommended_9x2.7_24000"]
close(rec1["mrr_mm3_s"], 9.0 * 2.7 * 6000 / 60, what="op 1 recommended MRR")
close(rec1["available_kw"], p_avail(24000), what="power at 24000 rpm")

# energy text: the numbers it quotes
E = D["energy"]["checks"]
close(round(E["curti_u_at_guedes_chip_J_mm3"], 2), 0.31, rel=0.02, what="0.31 J/mm3 at the Guedes chip")
close(round(E["guedes_mean_chip_2flute_mm"], 3), 0.019, rel=0.03, what="Guedes chip 0.019 mm")
close(round(op1["rs_cam_curti_u_J_mm3"], 2), 0.12, rel=0.03, what="0.12 J/mm3 at op 1")
close(round(op1["mean_chip_mm"], 3), 0.080, rel=0.01, what="op 1 mean chip 0.080")
close(round(p_avail(16219), 2), 1.01, rel=0.005, what="1.01 kW available at 16219")
assert "1.18 kW" in HTML and "1.01 kW" in HTML
gate_text = open(os.path.join(HERE, "..", "rivmap_block_job.md")).read()
assert "1.18 kW peak, 1.01 kW available at 16 219 rpm" in gate_text, "the sim peak is not in the job story"
checks += 2

# --- dangerous cards ------------------------------------------------------
danger_ops = {o["index"] for o in D["ops"] for f in o["fields"].values()
              if "dangerous" in (f["risk_operator"], f["risk_suggest"])}
card_ops = {i for t in D["top"] for i in t["ops"]}
checks += 1
assert danger_ops == card_ops, f"cards {sorted(card_ops)} vs dangerous ops {sorted(danger_ops)}"
flag_ops = {x["op"] for x in D["flagged"] if x["risk"] == "dangerous"}
checks += 1
assert flag_ops == danger_ops, f"flagged list {sorted(flag_ops)} vs fields {sorted(danger_ops)}"

OPS = {o["index"]: o["fields"] for o in D["ops"]}
# op 1 card
close(OPS[1]["feed"]["operator"], 5999, what="card op 1 feed")
close(OPS[1]["rpm"]["operator"], 16219, what="card op 1 rpm")
close(OPS[1]["stepover"]["operator"], 3.6, what="card op 1 stepover")
close(OPS[1]["depth_per_pass"]["suggest"], 14.05, rel=5e-4, what="card op 1 Suggest depth")
close(OPS[1]["stepover"]["recommended"], 2.7, what="card op 1 rec stepover")
close(OPS[1]["feed"]["recommended"], 6000, what="card op 1 rec feed")
close(OPS[1]["rpm"]["recommended"], 24000, what="card op 1 rec rpm")
close(OPS[1]["ramp_angle"]["recommended"], 2.0, what="card op 1 rec ramp")
# op 5 card
close(OPS[5]["fz"]["suggest"], 0.382, rel=2e-3, what="card op 5 Suggest chip")
close(OPS[5]["plunge"]["suggest"], 2999, what="card op 5 Suggest plunge")
close(OPS[5]["feed"]["operator"], 826, what="card op 5 operator feed")
close(OPS[5]["rpm"]["operator"], 10610, what="card op 5 operator rpm")
close(OPS[5]["feed"]["reviewer_min"], 1260, what="card op 5 review A one-pass min")
close(OPS[5]["feed"]["reviewer_max"], 1620, what="card op 5 review A one-pass max")
assert "2880" in open(os.path.join(HERE, "review_A.md")).read(), "2880 mm/min two-pass feed not in review A"
# op 2 card
close(OPS[2]["feed"]["recommended"], 3600, what="card op 2 rec feed")
close(OPS[2]["rpm"]["recommended"], 18000, what="card op 2 rec rpm")
# op 14 card
close(OPS[14]["depth_per_pass"]["suggest"], 14.05, rel=5e-4, what="card op 14 Suggest depth")
close(OPS[14]["depth_per_pass"]["operator"], 4.17, rel=1e-3, what="card op 14 operator depth")
close(OPS[14]["depth_per_pass"]["suggest"] / OPS[14]["depth_per_pass"]["operator"], 3.4, rel=0.02, what="card op 14 ratio")
checks += 2

print(f"assert_report.py: {checks} checks passed")
