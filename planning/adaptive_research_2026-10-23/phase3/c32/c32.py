"""C32: the body clearing_cut samples of '2 Back Rough' (wanaka200) that the
simulation reads over 0.471: exact width (C18 exact_width_at on the densified
sample path), mechanism, level and slope.

Usage: python3 -I c32.py <run_dir> <simart_dir> <tag>
"""
import collections, json, math, os, re, sys, glob, pickle
import numpy as np
from scipy.spatial import cKDTree
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'fix2'))
from producer_split import phases

run, simart, tag = sys.argv[1:4]
R = 3.0; PITCH = 0.05; STEP = 0.1; BOUND = 0.471
CEIL = 0.3626
tpf = sorted(glob.glob(os.path.join(run, 'tp_1_*.json')))[0]
tp = json.load(open(tpf))
phase, slice_arm = phases(tp)
an = tp['debug_trace']['annotations']
# level of each move (final indices), from the same mapping as phases()
ents = sorted([i for i in tp['semantic_trace']['items'] if i['kind'] == 'entry'], key=lambda i: i['move_start'])
gen = {}
for a in an:
    m = re.match(r'Pass (\d+) — ', a['label'])
    if m: gen[int(m.group(1))] = a['move_index']
fin = sorted((gen[e['params']['values']['pass_index']], e['move_start']) for e in ents)
def final_of(g):
    for gi, fi in fin:
        if gi >= g: return fi
    return tp['move_count']
levels = [(float(a['label'].split()[2]), final_of(a['move_index'])) for a in an if a['label'].startswith('Adaptive Z')]
def level_of(m):
    z = None
    for lz, st in levels:
        if m >= st: z = lz
    return z

S = [x for x in json.load(open(os.path.join(simart, 'simulation.json')))['trace']['samples'] if x['toolpath_id'] == 1]
sim_cell = json.load(open(os.path.join(simart, 'summary.json')))['simulation_resolution']['mm']
P = np.array([x['position'] for x in S])
# densified path: points between sample k-1 and k carry seq k
pts = [P[0:1]]; seq = [np.array([0])]
for k in range(1, len(P)):
    a, b = P[k - 1], P[k]
    n = max(1, int(math.ceil(math.hypot(b[0] - a[0], b[1] - a[1]) / STEP)))
    t = (np.arange(1, n + 1) / n)[:, None]
    pts.append(a + t * (b - a)); seq.append(np.full(n, k))
DP = np.vstack(pts); DS = np.concatenate(seq)
B = 9.0
keys = collections.defaultdict(list)
cx = np.floor(DP[:, 0] / B).astype(int); cy = np.floor(DP[:, 1] / B).astype(int)
order = np.lexsort((cy, cx))
for i in order: keys[(cx[i], cy[i])].append(i)
keys = {k: np.array(v) for k, v in keys.items()}
g = np.arange(-R, R + PITCH / 2, PITCH); GX, GY = np.meshgrid(g, g); ins = GX ** 2 + GY ** 2 <= R * R
GX = GX[ins]; GY = GY[ins]

def width(k, zmax, same_level_from=None):
    frm = P[k - 1, :2]; to = P[k, :2]; mid = (frm + to) / 2
    dv = to - frm; l = max(np.hypot(*dv), 1e-9); sx, sy = -dv[1] / l, dv[0] / l
    i0, j0 = int(math.floor(mid[0] / B)), int(math.floor(mid[1] / B))
    cand = [keys[(i, j)] for i in (i0 - 1, i0, i0 + 1) for j in (j0 - 1, j0, j0 + 1) if (i, j) in keys]
    L = np.column_stack([mid[0] + GX, mid[1] + GY])
    if cand:
        c = np.concatenate(cand)
        keep = DP[c, 2] <= zmax
        if same_level_from is not None:
            keep |= DS[c] >= same_level_from
        c = c[(DS[c] <= k - 1) & keep]
        if len(c):
            dist, _ = cKDTree(DP[c, :2]).query(L, distance_upper_bound=R + 1e-12)
            unc = ~(dist <= R)
        else:
            unc = np.ones(len(L), bool)
    else:
        unc = np.ones(len(L), bool)
    if not unc.any(): return 0.0
    u = GX[unc] * sx + GY[unc] * sy
    return (u.max() - u.min()) / (2 * R)

def step_thickness(k, zq, lvl_from):
    """The axial thickness of the stock a higher same-level pass left in the
    disc: for each lattice point that only a higher pass covers, the lowest
    such pass Z within R, minus the sample Z. Returns (median, max) in mm."""
    frm = P[k - 1, :2]; to = P[k, :2]; mid = (frm + to) / 2
    i0, j0 = int(math.floor(mid[0] / B)), int(math.floor(mid[1] / B))
    c = np.concatenate([keys[(i, j)] for i in (i0 - 1, i0, i0 + 1) for j in (j0 - 1, j0, j0 + 1) if (i, j) in keys])
    c = c[DS[c] <= k - 1]
    low = c[DP[c, 2] <= zq + 0.01]
    high = c[(DP[c, 2] > zq + 0.01) & (DS[c] >= lvl_from)]
    L = np.column_stack([mid[0] + GX, mid[1] + GY])[::7]
    cut_low = np.zeros(len(L), bool)
    if len(low):
        d0, _ = cKDTree(DP[low, :2]).query(L, distance_upper_bound=R + 1e-12); cut_low = d0 <= R
    if not len(high): return None
    th = []
    tree = cKDTree(DP[high, :2])
    for q, nb in zip(L[~cut_low], tree.query_ball_point(L[~cut_low], R)):
        if nb: th.append(DP[high[nb], 2].min() - zq)
    if not th: return None
    return float(np.median(th)), float(np.max(th))

level_first_sample = {}
for k, x in enumerate(S):
    lz = level_of(x['move_index'])
    level_first_sample.setdefault(lz, k)
rows = []
prev = None
for k, x in enumerate(S):
    p = x['position']
    ln = math.hypot(p[0] - prev[0], p[1] - prev[1]) if prev is not None else 0.0
    dz = p[2] - prev[2] if prev is not None else 0.0
    prev = p
    m = x['move_index']
    if not x['is_cutting'] or x['source_intent'] != 'clearing_cut': continue
    ph = phase[m] if m < len(phase) else None
    if ph != 'body': continue
    r = x['engagement']['radial_woc_fraction']
    if r <= BOUND or k == 0: continue
    lz = level_of(m)
    strict = width(k, p[2] + 0.01)
    # every earlier pass of the same level counts as cut, at any drape height
    loose = width(k, p[2] + 0.01, level_first_sample[lz])
    # the same reading with only stock at least 0.5 mm thick counted
    thick = width(k, p[2] + 0.5)
    th = step_thickness(k, p[2], level_first_sample[lz]) if strict > BOUND and loose <= BOUND else None
    rows.append(dict(k=k, m=m, lz=lz, z=p[2], kin=x['cut_kinematics'], ln=ln, dz=dz, r=r, strict=strict, loose=loose, thick=thick, th=th))
print(tag, 'sim cell', sim_cell, 'levels', levels, 'body over-0.471 samples', len(rows), 'mm', round(sum(r['ln'] for r in rows), 1))
pickle.dump(dict(rows=rows, levels=levels, sim_cell=sim_cell), open(os.path.join(os.path.dirname(os.path.abspath(__file__)), f'c32_{tag}.pkl'), 'wb'))
