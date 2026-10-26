"""C38 D24 analysis, step 5: why the band rose (D23 455, C38a 794, D24
999). (a) C22a's replay rounds in op 1's outer strip (region 1 at Z 11, 5,
0.5): groups dropped by the first filter and by the later rounds, from the
D24 log (both plans of the C29 pick are listed; the sums are over the
lines, so they count both plans). (b) op 1's cut in the strip at the strip
levels, C38a against D24, from the samples."""
import json, re
from pathlib import Path
import numpy as np
H = Path(__file__).resolve().parent
P3 = H.parent.parent
exec((H.parent / "d22_why" / "why.py").read_text().split("C_OP1 = ")[0])
pat = re.compile(r"index=(\d)\}.*C22a: replayed the plan after the F-038 group filter z=([0-9.]+) region=(\d+) rounds=(\d+) dropped_first=(\d+) dropped_more=(\d+)")
rows = []
for line in (P3 / "fix14" / "corne" / "stdout_plain.txt").read_text().splitlines():
    m = pat.search(line)
    if m:
        op, z, reg, rounds, first, more = m.groups()
        rows.append((int(op), round(float(z), 2), int(reg), int(rounds), int(first), int(more)))
strip = [r for r in rows if r[0] == 0 and r[2] == 1 and r[1] in (11.0, 5.0, 0.5)]
out = {"op1_strip_lines": len(strip),
       "op1_strip_dropped_first": sum(r[4] for r in strip),
       "op1_strip_dropped_more": sum(r[5] for r in strip),
       "op1_strip_max_rounds": max(r[3] for r in strip)}
for run in ("c38a", "d24"):
    S = np.load(H / f"samples_{run}.npz"); it = list(S["intents"])
    pos = S["pos"].astype(float); w = wall(pos); z = pos[:, 2]
    lv = np.zeros(len(z), bool)
    for v in (11.0, 5.0, 0.5):
        lv |= np.abs(z - v) < 0.05
    m = (S["tp"] == 1) & S["cut"] & (w > 3.0) & (w <= 6.2) & lv & (S["intent"] == it.index("clearing_cut"))
    out[f"{run}_op1_strip_cut_mm"] = round(float(m.sum()) * 0.3, 1)
    out[f"{run}_op1_strip_removed_mm3"] = round(float(S["removed"][m].sum()), 1)
print(json.dumps(out, indent=1))
(H / "c22a_drops.json").write_text(json.dumps(out, indent=1))
