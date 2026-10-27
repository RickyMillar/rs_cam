import sys, collections
def load(p):
    rows = []
    for line in open(p):
        if line.startswith("#"): continue
        f = line.split()
        rows.append((f[1], f[2], f[3], " ".join(f[4:])))
    return rows
def horizontal(rows):
    out = []; prev = None
    for x, y, z, rest in rows:
        if prev is None or (x, y) != prev:
            out.append((x, y, z, rest))
        prev = (x, y)
    return out
def kinds(rows):
    return collections.Counter(r[3].split()[-1] + ("/" + r[3].split()[0]) for r in rows)
for case in sys.argv[2:]:
    old = load(f"{sys.argv[1]}/old/{case}.txt"); new = load(f"{sys.argv[1]}/new/{case}.txt")
    ho, hn = horizontal(old), horizontal(new)
    same = ho == hn
    d = kinds(old); d.subtract(kinds(new)); d = {k: v for k, v in d.items() if v}
    print(f"{case}: moves {len(old)} -> {len(new)}; XY moves {len(ho)} -> {len(hn)}, identical: {same}; removed by kind {d}")
