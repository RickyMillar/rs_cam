"""P11b L9: check each claim of index.html against the raw CLI output.

Run: python3 -I assert_claims.py   (after run_suggest.py and build_page.py)

The script recomputes every claim from `suggest_raw.jsonl` (the CLI records) and
`../results.json` (the step 0 cases and constants), not from `results.json`, and checks
that `results.json` and the page text state the same numbers.
"""
import json
import os
import re

HERE = os.path.dirname(os.path.abspath(__file__))
R0 = json.load(open(os.path.join(HERE, "..", "results.json")))
R = json.load(open(os.path.join(HERE, "results.json")))
PAGE = open(os.path.join(HERE, "index.html")).read()
RAW = {}
for line in open(os.path.join(HERE, "suggest_raw.jsonl")):
    rec = json.loads(line)
    RAW[rec["toolpath_name"]] = rec

TOL = R0["constants"]["point_tol"]["value"]
PASS = R0["constants"]["pass_fraction"]["value"]
checked = []


def claim(cid, cond, msg):
    assert cond, f"{cid}: {msg}"
    checked.append(cid)


def page_claim(cid):
    m = re.search(r'data-claim="' + cid + r'">(.*?)</p>', PAGE, re.S)
    assert m, f"{cid}: not on the page"
    return m.group(1)


def chip_of(key):
    rec = RAW.get(key)
    if rec is None or "profile" not in rec or "calculator" not in rec["profile"]:
        return None
    c = rec["profile"]["calculator"]
    return c["feed_rate_mm_min"] / (c["rpm"] * rec["tool"]["flute_count"]) if c["rpm"] > 0 else None


def in_band(x, lo, hi):
    if x is None:
        return None
    if lo is None or abs(hi - lo) < 1e-12:
        return abs(x - hi) <= TOL * hi
    return lo * (1 - 1e-9) <= x <= hi * (1 + 1e-9)


# Recompute the target pass per case from the raw records and the step 0 bands.
groups = {"W": [0, 0], "D": [0, 0]}
ref = [0, 0]
for ci, case in enumerate(R0["cases"]):
    lo, hi = case["tgt_band"]
    p = in_band(chip_of(f"c{ci}-target"), lo, hi)
    out = R["cases"][ci]
    assert out["passes"] == p, f"case {ci}: results.json {out['passes']} vs recomputed {p}"
    if p is not None:
        groups[case["group"]][0] += int(p)
        groups[case["group"]][1] += 1
    if case["group"] == "D":
        s = 0.75 if case["k"] == 2 else 0.50
        rp = in_band(chip_of(f"c{ci}-reference"), None if lo is None else lo / s, hi / s)
        if rp is not None:
            ref[0] += int(rp)
            ref[1] += 1

claim("A01", R["n_cases"] == len(R0["cases"])
      and f"{len(R0['cases'])} cases" in page_claim("A01")
      and f"group W {groups['W'][1]} testable" in page_claim("A01")
      and f"group D {groups['D'][1]} testable" in page_claim("A01"),
      "the case count is the step 0 case count")
for cid, g in (("A02", "W"), ("A03", "D")):
    passed, n = groups[g]
    claim(cid, R["groups"][g]["passed"] == passed and R["groups"][g]["n"] == n
          and f"{passed} of {n} cases" in page_claim(cid), f"group {g} {passed} / {n}")
verdict = "PASS" if all(groups[g][1] and groups[g][0] / groups[g][1] >= PASS for g in groups) else "FAIL"
claim("A04", R["verdict"] == verdict and f"The verdict is {verdict}" in page_claim("A04"),
      f"the verdict is {verdict}")
claim("A05", f"{ref[0]} of {ref[1]} cases" in page_claim("A05"), f"reference {ref[0]} / {ref[1]}")

# A case with no CLI record is a case whose target depth is above the Pocket range.
missing = [ci for ci, _ in enumerate(R0["cases"]) if f"c{ci}-target" not in RAW]
too_deep = [ci for ci, c in enumerate(R0["cases"])
            if c["tgt_cond"]["ap"] is not None and c["tgt_cond"]["ap"] > 50.0]
claim("A06", missing == too_deep and f"{len(missing)} cases are not testable" in page_claim("A06"),
      f"cases with no CLI record {missing} vs too deep {too_deep}")
refused = [k for k, rec in RAW.items() if k.endswith("-target")
           and str(rec.get("error", "")).startswith("Suggest refused")]
claim("A07", f"{len(refused)} cases are not testable" in page_claim("A07"),
      f"{len(refused)} refused target cases")

# A08: the machine figures are the CLI output's.
kw, caps = set(), set()
for k, rec in RAW.items():
    c = (rec.get("profile") or {}).get("calculator")
    if not c:
        continue
    kw.add(c["available_power_kw"])
    for text in c["warnings"]:
        m = re.search(r"feed_ceiling_mm_min: ([0-9.]+)", text)
        if m:
            caps.add(float(m.group(1)))
target_kw = {(RAW.get(f"c{ci}-target") or {}).get("profile", {}).get("calculator", {}).get(
    "available_power_kw") for ci, _ in enumerate(R0["cases"])} - {None}
claim("A08", sorted(target_kw) == R["machine"]["available_kw"]
      and set(R["machine"]["feed_ceiling_mm_min"]) <= caps
      and all(f"{x:g}" in page_claim("A08") for x in R["machine"]["available_kw"]),
      "the machine figures are the CLI output's")

# The per-family table (section 2): the target counts recomputed per (group, vendor).
fam = {}
for ci, case in enumerate(R0["cases"]):
    lo, hi = case["tgt_band"]
    p = in_band(chip_of(f"c{ci}-target"), lo, hi)
    if p is None:
        continue
    k = f'{case["group"]}:{case["vendor"]}'
    a = fam.setdefault(k, [0, 0])
    a[0] += int(p)
    a[1] += 1
for k, (passed, n) in fam.items():
    t = R["per_family"][k]["target"]
    vendor = k.split(":")[1]
    row = f"<td>{k.split(':')[0]}</td><td>{vendor}</td><td>{passed} / {n}</td>"
    claim(f"T-{k}", t["passed"] == passed and t["n"] == n and row in PAGE,
          f"family {k}: {passed} / {n}")
# The unbound column: no feed cap, no RPM descent, no power limit in the record.
unb = {}
for ci, case in enumerate(R0["cases"]):
    rec = RAW.get(f"c{ci}-target")
    c = (rec or {}).get("profile", {}).get("calculator")
    if not c:
        continue
    w = c["warnings"]
    if c["power_limited"] or any(x.startswith(("FeedRateClamped", "RpmLoweredForFeedCeiling"))
                                 for x in w):
        continue
    lo, hi = case["tgt_band"]
    p = in_band(chip_of(f"c{ci}-target"), lo, hi)
    if p is None:
        continue
    a = unb.setdefault(f'{case["group"]}:{case["vendor"]}', [0, 0])
    a[0] += int(p)
    a[1] += 1
for k, v in R["per_family"].items():
    passed, n = unb.get(k, [0, 0])
    u = v["target_unbound"]
    claim(f"U-{k}", u["passed"] == passed and u["n"] == n, f"unbound {k}: {passed} / {n}")

print(f"{len(checked)} claims checked: {', '.join(checked)}")
