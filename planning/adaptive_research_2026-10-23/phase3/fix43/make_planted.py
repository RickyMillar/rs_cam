"""D62 planted check: a CP-only job where a CP rest op on remaining stock meets a band that the prior op left taller than one
DPP above the rest op's own anchor. From fix26/jobs/k_contour_parallel.toml (Corne CP, k_cp_fix6job):
- op 1 (CP, 6 mm): boundary offset 6.0 -> 0.0 (centre on the silhouette, so the tool reaches 3 mm out and the band 3-9 mm
  outside the wall stays at the stock top, 23);
- a new op 5 "3D Rough 1 (edges, CP rest)": a copy of op 1 with stock_source from_remaining_stock and the boundary offset 6.0
  (the shipped silhouette + 6 mm), placed after op 1;
- the copy op (3) and the waterline (4) disabled.
tomllib diff printed."""
import re, tomllib
from pathlib import Path
P3 = Path(__file__).resolve().parent.parent; H = Path(__file__).resolve().parent
src = P3 / "fix26/jobs/k_contour_parallel.toml"; txt = src.read_text()
i1 = txt.index("[[setups.toolpaths]]\nid = 1\n"); i3 = txt.index("[[setups.toolpaths]]\nid = 3\n")
op1 = txt[i1:i3]
b_old = 'source = "model_silhouette"\ncontainment = "center"\noffset = 6.0\n'
assert op1.count(b_old) == 1
op1_new = op1.replace(b_old, b_old.replace("offset = 6.0", "offset = 0.0"))
op5 = op1.replace("id = 1\n", "id = 5\n", 1).replace('name = "3D Rough 1"', 'name = "3D Rough 1 (edges, CP rest)"', 1)
assert op5.count('stock_source = "fresh"') == 1
op5 = op5.replace('stock_source = "fresh"', 'stock_source = "from_remaining_stock"')
rest = txt[i3:]
for tid in (3, 4):
    j = rest.index(f"[[setups.toolpaths]]\nid = {tid}\n"); k = rest.index("enabled = true\n", j); assert k < j + 200
    rest = rest[:k] + "enabled = false\n" + rest[k + len("enabled = true\n"):]
out = txt[:i1] + op1_new + op5 + rest
(H / "jobs/corne_cp_planted_band.toml").write_text(out)
def flat(d, p=""):
    if isinstance(d, dict):
        for k, v in d.items(): yield from flat(v, f"{p}.{k}" if p else k)
    elif isinstance(d, list):
        for i, v in enumerate(d): yield from flat(v, f"{p}[{i}]")
    else: yield p, d
o, n = dict(flat(tomllib.loads(txt))), dict(flat(tomllib.loads(out)))
ops = tomllib.loads(out)["setups"][0]["toolpaths"]
print([(t["id"], t["name"], t["enabled"], t["stock_source"], t["boundary"]["offset"]) for t in ops])
for k in sorted(set(o) | set(n)):
    if o.get(k, "<absent>") != n.get(k, "<absent>") and not k.startswith("setups[0].toolpaths[1]."): print(f"  {k}: {o.get(k, '<absent>')!r} -> {n.get(k, '<absent>')!r}")
print("  setups[0].toolpaths[1]: the new op 5 (copy of op 1; stock_source from_remaining_stock, boundary offset 6.0); old toolpaths[1..2] shift to [2..3]")
