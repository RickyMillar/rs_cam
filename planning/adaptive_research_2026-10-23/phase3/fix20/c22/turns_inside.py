"""C22 §16.9 item 3: Corne op-1 planar turns over 30 deg at move junctions (sim samples, the
last sample of each move) inside the hole-filled silhouette (fix9/corne_part.npz), D26 vs D29,
split by: near a D26 large-Z step (within R + one sim cell), and the length of the two moves
at the junction (short: under one stepover, 1.2 mm on Corne)."""
import json, os, math, re, collections
import numpy as np
from scipy import ndimage
H = os.path.dirname(os.path.abspath(__file__)); P3 = os.path.join(H, "..", "..")
part = np.load(os.path.join(P3, "fix9", "corne_part.npz")); top, g, x0, y0 = part["top"], float(part["g"]), float(part["x0"]), float(part["y0"])
sil = ndimage.binary_fill_holes(np.isfinite(top) & (top > 1e-6))
def inside(x, y):
    c, r = int(round((x - x0) / g)), int(round((y - y0) / g))
    return 0 <= r < sil.shape[0] and 0 <= c < sil.shape[1] and bool(sil[r, c])
ref = []
for ln in open(os.path.join(P3, "fix16/corne_pick/stdout_plain.txt")):
    if "index=0" in ln and "C40 entry kind" in ln and "LargeZ" in ln:
        kv = dict(re.findall(r"(\w+)=(\S+)", ln.split("C40 entry", 1)[1])); ref.append((float(kv["x"]), float(kv["y"])))
ref = np.array(ref)
OUT = {}
for col, d in (("D26", "fix16/corne_pick"), ("D29", "fix20/a_only/corne")):
    last = {}
    for s in json.load(open(os.path.join(P3, d, "simulation.json")))["trace"]["samples"]:
        if s["toolpath_id"] == 1: last[s["move_index"]] = s
    ms = sorted(last); c = collections.Counter(); pd = None; plen = None
    for a, b in zip(ms, ms[1:]):
        pa, pb = last[a]["position"], last[b]["position"]
        dx, dy = pb[0]-pa[0], pb[1]-pa[1]; L = math.hypot(dx, dy)
        if L < 1e-6: pd = None; continue
        dd = math.atan2(dy, dx)
        if pd is not None and inside(pa[0], pa[1]) and abs((dd-pd+math.pi) % (2*math.pi) - math.pi) > math.radians(30):
            nr = bool((np.hypot(ref[:, 0]-pa[0], ref[:, 1]-pa[1]) <= 3.3).any())
            short = min(L, plen) < 1.2
            c[("near step" if nr else "away", "short move" if short else "long moves")] += 1
        pd, plen = dd, L
    OUT[col] = {f"{a} / {b}": n for (a, b), n in sorted(c.items())}
    print(col, sum(c.values()), OUT[col])
json.dump(OUT, open(os.path.join(H, "turns_inside.json"), "w"), indent=1)
