"""C38 D22 analysis, step 6: the F-038 group filter in plan_one_outline, per
op and level, from the C38a run log (the debug line "F-038: dropped short cut
groups inside 2D adaptive output"; the C29 pick plans each region twice, so
each line appears twice: the first of each pair is counted)."""
import re
from pathlib import Path
H = Path(__file__).resolve().parent
log = (H.parent / "corne_c38a" / "stdout_plain.txt").read_text().splitlines()
pat = re.compile(r"index=(\d)\}.*dropped short cut groups.*z=([0-9.]+) region=(\d+) dropped_groups=(\d+) group_total=(\d+)")
seen, rows = set(), []
for line in log:
    m = pat.search(line)
    if not m:
        continue
    key = m.groups()
    if key in seen:
        continue
    seen.add(key)
    rows.append((int(key[0]), round(float(key[1]), 2), int(key[2]), int(key[3]), int(key[4])))
copy = [r for r in rows if r[0] == 1]
for r in rows:
    print(r)
dropped = sum(r[3] for r in copy); total = sum(r[4] for r in copy)
print("copy op region 1: dropped", dropped, "of", total)
assert all(r[2] == 1 for r in copy) and len(copy) == 6
assert (dropped, total) == (509, 610), (dropped, total)
print("ASSERTS OK")
