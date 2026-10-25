# C27: entry rings per level, matched between the arms within 1.5 mm; the
# unmatched ones with the moves before and after each.
import math
from perlevel import fx, per_level
A, _ = per_level("geodesic_local_zigzag"); B, _ = per_level("geodesic_local_zigzag_v2")
def rings(d):
    out = {}
    for i, p in d["rings"]:
        out.setdefault(d["lev"][i], []).append((i, p))
    return out
ra, rb = rings(A), rings(B)
def ctx(d, i):
    g0, a0, b0 = d["moves"][i - 1]
    # the next fed move after the ring
    j = i + 1
    while j < len(d["moves"]) and (d["moves"][j][0] == 0 or fx.xy(*d["moves"][j][1:]) < 1e-6):
        j += 1
    k = i - 1
    while k > 0 and (d["moves"][k][0] == 0 or d["cls"][k] == "rapid"):
        k -= 1
    return d["moves"][k], d["moves"][j] if j < len(d["moves"]) else None, d["cls"][j] if j < len(d["moves"]) else None
for lev in A["levels"]:
    a, b = ra.get(lev, []), rb.get(lev, [])
    ua = [x for x in a if not any(math.dist(x[1][:2], y[1][:2]) < 1.5 for y in b)]
    ub = [y for y in b if not any(math.dist(x[1][:2], y[1][:2]) < 1.5 for x in a)]
    print(f'{lev}: zigzag {len(a)} v2 {len(b)}; zigzag only {len(ua)}, v2 only {len(ub)}')
    for name, d, us in (("zz", A, ua), ("v2", B, ub)):
        for i, p in us:
            prev, nxt, nc = ctx(d, i)
            print(f'   {name} ring ({p[0]:.2f},{p[1]:.2f}) prev fed end ({prev[2][0]:.2f},{prev[2][1]:.2f}) gap {math.dist(prev[2][:2], p[:2]):.1f}; next {nc} to ({nxt[2][0]:.2f},{nxt[2][1]:.2f}) {fx.xy(nxt[1], nxt[2]):.1f} mm')
