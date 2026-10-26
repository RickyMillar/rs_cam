"""D29 (copy of fix16/c40_join.py; the fix20 columns and fix20/entries_d29.json): the C40 entry causes. (1) Per op, the 'C40 entry summary' line and
the per-entry lines counted by kind and cause; Other = cause None. (2) The
join with entries.py (fix15/entries_d25.json): each op-1 descent of 1 mm or
more that removes more than 0.1 mm3 (the §12.5 / §15.3 selection, both span
styles) takes the cause of the nearest unused op-1 'C40 entry' line within
0.5 mm in XY (and 1.0 mm in Z where the row has an end plane). Writes
c40_join.json."""
import collections, json, math, re, sys
from pathlib import Path
H = Path(__file__).resolve().parent
E = json.loads((H / "entries_d30.json").read_text())
LINE = re.compile(r"generate_toolpath\{index=(\d)\}: \S+: C40 entry kind=(\w+) cause=(\S+) join=(\S+) x=(\S+) y=(\S+) z=(\S+) drop_mm=(\S+)")
out = {}
for run, col in (("a_only/corne", "corne D30 a_only"), ("a_only/wanaka200", "wanaka200 D30 a_only")):
    ents = []
    summ = []
    for ln in open(H / run / "stdout_plain.txt"):
        m = LINE.search(ln)
        if m:
            op, kind, cause, join, x, y, z, drop = m.groups()
            ents.append({"op": int(op), "kind": kind, "cause": cause, "join": join, "x": float(x), "y": float(y), "z": float(z), "drop": float(drop)})
        elif "C40 entry summary" in ln:
            summ.append(re.sub(r"^.*C40 entry summary", "C40 entry summary", ln.strip()))
    r = {"summary_lines": summ}
    for op in sorted({e["op"] for e in ents}):
        es = [e for e in ents if e["op"] == op]
        other = [e for e in es if e["cause"] == "None"]
        r[f"op{op}"] = {"entries": len(es), "by_kind_cause": dict(collections.Counter(f'{e["kind"]} {e["cause"]}' for e in es).most_common()),
                        "other": len(other), "other_share_pct": round(100 * len(other) / len(es), 2),
                        "largez_share_pct": round(100 * sum(e["cause"] in ("Some(LargeZ)", "Some(LargeZHeld)") for e in es) / len(es), 2),
                        "other_first20": [{k: e[k] for k in ("kind", "x", "y", "z", "drop", "join")} for e in other[:20]]}
    # join with entries.py, op 1 (op index 0)
    rows = [w for w in E[col]["rows"] if w["descent_mm"] >= 1.0 and w["removed_mm3"] > 0.1]
    pool = [e for e in ents if e["op"] == 0]
    used = set(); causes = collections.Counter(); un = 0
    for w in rows:
        best, bd = None, 0.5
        for i, e in enumerate(pool):
            if i in used:
                continue
            d = math.hypot(e["x"] - w["x"], e["y"] - w["y"])
            if d <= bd and (w["end_plane"] is None or abs(e["z"] - w["end_plane"]) <= 1.0):
                best, bd = i, d
        if best is None:
            un += 1
        else:
            used.add(best); causes[pool[best]["cause"]] += 1
    n = len(rows)
    r["join_op1"] = {"descents": n, "matched": n - un, "unmatched": un,
                     "by_cause": dict(causes.most_common()),
                     "share_pct": {k: round(100 * v / n, 1) for k, v in causes.most_common()},
                     "largez_share_pct": round(100 * (causes["Some(LargeZ)"] + causes["Some(LargeZHeld)"]) / n, 1) if n else None,
                     "other_share_pct": round(100 * causes["None"] / n, 2) if n else None}
    out[run] = r
    print("==", run)
    for l in summ:
        print("  ", l[:420])
    for op in [k for k in r if k.startswith("op")]:
        v = r[op]; print(f"  {op}: entries {v['entries']}, Other {v['other']} = {v['other_share_pct']} %, LargeZ(+Held) {v['largez_share_pct']} %")
    print("  join op1:", r["join_op1"])
(H / "c40_join.json").write_text(json.dumps(out, indent=1))
