"""C33: the body samples of Corne '3D Rough 1' (D20, zigzag_pick, no perimeter
sweep) that the simulation reads over 0.471, with their exact widths (strict:
stock above the tip; thick: stock at least one planner cell, 0.5 mm, above the
tip), move context, wall distance, and whether D18's shipped sweep covered the
stock the thick reading finds. Writes c33_rows.pkl."""
import collections, json, math, os, pickle, re, sys
import numpy as np
from scipy.spatial import cKDTree
from scipy import ndimage
H = os.path.dirname(os.path.abspath(__file__))
D20 = os.path.join(H, "..", "fix7", "corne", "geodesic_local_zigzag_pick")
D18 = os.path.join(H, "..", "fix5", "corne", "geodesic_local_zigzag_pick")
R, PITCH, STEP, BOUND, THICK, BIN = 3.0, 0.05, 0.1, 0.471, 0.5, 9.0

def load(run):
    tp = json.load(open(os.path.join(run, "tp_1_3D_Rough_1.json")))
    S = [x for x in json.load(open(os.path.join(run, "simulation.json")))["trace"]["samples"] if x["toolpath_id"] == 1]
    an = tp["debug_trace"]["annotations"]
    ents = sorted([i for i in tp["semantic_trace"]["items"] if i["kind"] == "entry"], key=lambda i: i["move_start"])
    gen = {}
    for a in an:
        m = re.match(r"Pass (\d+) — ", a["label"])
        if m: gen[int(m.group(1))] = a["move_index"]
    fin = sorted((gen[e["params"]["values"]["pass_index"]], e["move_start"]) for e in ents)
    def final_of(g):
        for gi, fi in fin:
            if gi >= g: return fi
        return tp["move_count"]
    levels = [(float(a["label"].split()[2]), final_of(a["move_index"])) for a in an if a["label"].startswith("Adaptive Z")]
    wls = [final_of(a["move_index"]) for a in an if a["label"] == "Waterline cleanup"]
    return tp, S, ents, levels, wls

tp, S, ents, levels, wls = load(D20)
n = tp["move_count"]
level = [None] * n; phase = [None] * n
for k, (z, st) in enumerate(levels):
    en = levels[k + 1][1] if k + 1 < len(levels) else n
    for m in range(st, en):
        level[m] = z; phase[m] = "body" if m < wls[k] else "waterline"
P = np.array([x["position"] for x in S])
pts = [P[0:1]]; seq = [np.array([0])]
for k in range(1, len(P)):
    a, b = P[k - 1], P[k]
    m_ = max(1, int(math.ceil(math.hypot(b[0] - a[0], b[1] - a[1]) / STEP)))
    t = (np.arange(1, m_ + 1) / m_)[:, None]
    pts.append(a + t * (b - a)); seq.append(np.full(m_, k))
DP = np.vstack(pts); DS = np.concatenate(seq)
keys = collections.defaultdict(list)
cx = np.floor(DP[:, 0] / BIN).astype(int); cy = np.floor(DP[:, 1] / BIN).astype(int)
for i in range(len(DP)): keys[(cx[i], cy[i])].append(i)
keys = {k: np.array(v) for k, v in keys.items()}
g = np.arange(-R, R + PITCH / 2, PITCH); GX, GY = np.meshgrid(g, g); ins = GX ** 2 + GY ** 2 <= R * R
GX, GY = GX[ins], GY[ins]

def uncut(k, zmax, same_from=None):
    frm, to = P[k - 1, :2], P[k, :2]; mid = (frm + to) / 2
    i0, j0 = int(math.floor(mid[0] / BIN)), int(math.floor(mid[1] / BIN))
    cand = [keys[(i, j)] for i in (i0 - 1, i0, i0 + 1) for j in (j0 - 1, j0, j0 + 1) if (i, j) in keys]
    L = np.column_stack([mid[0] + GX, mid[1] + GY]); unc = np.ones(len(L), bool)
    if cand:
        c = np.concatenate(cand); keep = DP[c, 2] <= zmax
        if same_from is not None: keep |= DS[c] >= same_from
        c = c[(DS[c] <= k - 1) & keep]
        if len(c):
            d, _ = cKDTree(DP[c, :2]).query(L, distance_upper_bound=R + 1e-12); unc = ~(d <= R)
    return L, unc, mid, frm, to

def width(L, unc, mid, frm, to):
    if not unc.any(): return 0.0
    dv = to - frm; l = max(np.hypot(*dv), 1e-9); sx, sy = -dv[1] / l, dv[0] / l
    u = (L[unc, 0] - mid[0]) * sx + (L[unc, 1] - mid[1]) * sy
    return (u.max() - u.min()) / (2 * R)

# D18's shipped perimeter sweep per level: the level's first entry run (C30 phases)
tp18, S18, ents18, lev18, wls18 = load(D18)
sweep = {}
for k, (z, st) in enumerate(lev18):
    le = [e for e in ents18 if st <= e["move_start"] < wls18[k]]
    body = le[1]["move_start"]
    pts18 = np.array([x["position"] for x in S18 if st <= x["move_index"] < body])
    dens = []
    for a, b in zip(pts18, pts18[1:]):
        m_ = max(1, int(math.ceil(math.hypot(b[0] - a[0], b[1] - a[1]) / STEP)))
        dens.append(a + (np.arange(1, m_ + 1) / m_)[:, None] * (b - a))
    sweep[z] = np.vstack(dens)

# the part: cells whose top stands above a Z, their distance and component
pm = np.load(os.path.join(H, "partmap.npz")); top = pm["top"]; x0, y0, G = float(pm["x0"]), float(pm["y0"]), float(pm["g"])
part_cache = {}
def part_at(zc):
    if zc not in part_cache:
        mask = top > zc
        lab, nlab = ndimage.label(mask)
        sizes = ndimage.sum(mask, lab, range(1, nlab + 1))
        jj, ii = np.nonzero(mask)
        part_cache[zc] = (cKDTree(np.column_stack([x0 + ii * G, y0 + jj * G])), lab[jj, ii], sizes)
    return part_cache[zc]

# move-level context
first = {}; last = {}
for k, x in enumerate(S):
    first.setdefault(x["move_index"], k); last[x["move_index"]] = k
intent = {x["move_index"]: x["source_intent"] for x in S}
ent_start = {e["move_start"]: e["params"]["values"]["style"] for e in ents}

level_first = {}
for k, x in enumerate(S): level_first.setdefault(level[x["move_index"]], k)
rows = []
prev = None
for k, x in enumerate(S):
    p = x["position"]
    ln = math.hypot(p[0] - prev[0], p[1] - prev[1]) if prev is not None else 0.0
    prev = p
    m = x["move_index"]
    if k == 0 or not x["is_cutting"] or x["source_intent"] != "clearing_cut" or phase[m] != "body": continue
    r = x["engagement"]["radial_woc_fraction"]
    if r <= BOUND: continue
    L, unc_s, mid, frm, to = uncut(k, p[2] + 0.01)
    strict = width(L, unc_s, mid, frm, to)
    L, unc_t, mid, frm, to = uncut(k, p[2] + THICK)
    thick = width(L, unc_t, mid, frm, to)
    # every earlier pass of the same level counted as cut at any height (C32)
    L, unc_l, mid, frm, to = uncut(k, p[2] + 0.01, level_first[level[m]])
    loose = width(L, unc_l, mid, frm, to)
    swept_d18 = None
    if unc_t.any():
        sw = sweep[level[m]]
        sw = sw[sw[:, 2] <= p[2] + THICK]
        if len(sw):
            d, _ = cKDTree(sw[:, :2]).query(L[unc_t], distance_upper_bound=R + 1e-12)
            swept_d18 = float((d <= R).mean())
        else:
            swept_d18 = 0.0
    zc = math.floor((p[2] + 0.01) / 0.5) * 0.5
    tree, labs, sizes = part_at(zc)
    dw, iw = tree.query([p[0], p[1]])
    comp = int(labs[iw]); big = int(np.argmax(sizes)) + 1
    rows.append(dict(k=k, m=m, lz=level[m], p=p, ln=ln, r=r, kin=x["cut_kinematics"], strict=strict, thick=thick, loose=loose,
                     swept_d18=swept_d18, wall_d=float(dw), island=(comp != big), comp_size=float(sizes[comp - 1]) * G * G))
# move context for each over move
ctx = {}
for m in sorted(set(r["m"] for r in rows)):
    a = P[first[m] - 1] if first[m] > 0 else P[first[m]]; b = P[last[m]]
    # run: from the last entry start at or before m to the next entry start
    starts = [s for s in ent_start if s <= m]; s0 = max(starts) if starts else 0
    nxt = min([s for s in ent_start if s > m], default=n)
    # the next non-clearing move after m
    j = m + 1
    while j < n and intent.get(j) == "clearing_cut": j += 1
    ctx[m] = dict(len=float(math.hypot(b[0] - a[0], b[1] - a[1])), run_start=s0, run_style=ent_start.get(s0), run_end=nxt,
                  moves_from_start=m - s0, moves_to_end=nxt - m, next_noncut=j, next_intent=intent.get(j),
                  a=a.tolist(), b=b.tolist())
pickle.dump(dict(rows=rows, ctx=ctx, levels=levels, wls=wls), open(os.path.join(H, "c33_rows.pkl"), "wb"))
print("body over-0.471 samples", len(rows), "mm", round(sum(r["ln"] for r in rows), 1),
      "thick-over mm", round(sum(r["ln"] for r in rows if r["thick"] > BOUND), 1),
      "strict-over mm", round(sum(r["ln"] for r in rows if r["strict"] > BOUND), 1))
