"""D24: the 'C22a: held link over standing stock' lines of a pick run,
counted for the kept plan only. Each plan of a region writes one 'C22a:
replayed the plan after the F-038 group filter' line before its held-link
lines; the region ends at its 'C29 pick (3D score)' line (kept=A|B)."""
import collections, re, sys
for run in sys.argv[1:]:
    ev = [l for l in open(f"{run}/stdout_plain.txt") if re.search(r"C22a: |C29 pick|C31 region opening|C38 G-b", l)]
    blocks, cur = [], None
    for l in ev:
        if "C31 region opening" in l or ("C38 G-b" in l and cur is None):
            cur = {"plans": [], "kept": None, "op": re.search(r"index=(\d)", l).group(1)}; blocks.append(cur)
        elif "C22a: replayed" in l:
            cur["plans"].append([])
        elif "held link over standing stock" in l:
            cur["plans"][-1].append(l)
        elif "C29 pick" in l:
            cur["kept"] = re.search(r'kept="(\w)"', l).group(1); cur = None
    kept = []; allc = 0
    for b in blocks:
        allc += sum(len(p) for p in b["plans"])
        if b["plans"]:
            i = 0 if b["kept"] in (None, "A") else 1
            kept += b["plans"][i] if i < len(b["plans"]) else []
    zs = collections.Counter(re.search(r"z=([\d.]+)", l).group(1)[:6] for l in kept)
    st = [float(re.search(r"stock_above_mm=([\d.]+)", l).group(1)) for l in kept]
    print(run, "all lines", allc, "kept plan", len(kept), "by z", dict(zs), "stock_above max", max(st) if st else None, "median", sorted(st)[len(st)//2] if st else None)
    for l in kept[:40]:
        print("   ", re.sub(r"^.*clearing: ", "", l.strip()))
