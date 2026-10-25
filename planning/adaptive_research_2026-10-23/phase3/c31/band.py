"""C31: what the perimeter sweep of the 3D slice is for, on a geodesic run.

Per level of '3D Rough 1': the band the perimeter sweep cuts (discs of R round
its cutting samples), and how much of that band the region body alone, and the
body with the waterline cleanup, also cut. A band cell that only the sweep
cuts is stock the research arm leaves if it skips the sweep. Also the XY
distance from each sweep sample to the nearest body sample, and to the
nearest waterline sample. Phases as C30 (`attribute.py`): perimeter = the
level's first entry run, body = from the second entry to the waterline
annotation, waterline = after it.
Usage: python3 -I band.py <run dir>"""
import json, math, re, sys
import numpy as np
run = sys.argv[1]
R = 3.0
PITCH = 0.1
d = json.load(open(run + '/simulation.json'))
S = [x for x in d['trace']['samples'] if x['toolpath_id'] == 1]
del d
t = json.load(open(run + '/tp_1_3D_Rough_1.json'))
ents = sorted([i for i in t['semantic_trace']['items'] if i['kind'] == 'entry'], key=lambda i: i['move_start'])
an = t['debug_trace']['annotations']
gen_of_pass = {}
for a in an:
    m = re.match(r'Pass (\d+) — ', a['label'])
    if m:
        gen_of_pass[int(m.group(1))] = a['move_index']
fin = sorted((gen_of_pass[e['params']['values']['pass_index']], e['move_start']) for e in ents)
def final_of(g):
    for gi, fi in fin:
        if gi >= g:
            return fi
    return t['move_count']
levels = []; wls = []
for a in an:
    if a['label'].startswith('Adaptive Z'):
        levels.append((float(a['label'].split()[2]), final_of(a['move_index'])))
    if a['label'] == 'Waterline cleanup':
        wls.append(final_of(a['move_index']))
n = t['move_count']
level_of = [None] * n; phase = ['?'] * n
for k, (z, st) in enumerate(levels):
    en = levels[k + 1][1] if k + 1 < len(levels) else n
    wl = wls[k]
    le = [e for e in ents if st <= e['move_start'] < wl]
    body = le[1]['move_start']
    for m in range(st, en):
        level_of[m] = z
        phase[m] = 'perimeter' if m < body else ('body' if m < wl else 'waterline')

pts = {}
wl_samples = {}
prev = None
for x in S:
    p = x['position']
    step = math.hypot(p[0] - prev[0], p[1] - prev[1]) if prev else 0.0
    prev = p
    m = x['move_index']
    if (x['is_cutting'] and x['cut_kinematics'] in ('linear', 'arc') and m < n
            and level_of[m] is not None and phase[m] == 'waterline'):
        wl_samples.setdefault(level_of[m], []).append((p[:2], step, x['engagement']['radial_woc_fraction']))
for x in S:
    if not x['is_cutting'] or x['cut_kinematics'] == 'plunge':
        continue
    m = x['move_index']
    if m >= n or level_of[m] is None:
        continue
    pts.setdefault((level_of[m], phase[m]), []).append(x['position'][:2])

r_cells = int(math.ceil(R / PITCH))
yy, xx = np.mgrid[-r_cells:r_cells + 1, -r_cells:r_cells + 1]
disc = (xx * PITCH) ** 2 + (yy * PITCH) ** 2 <= R * R

def stamp(grid, x0, y0, P):
    h, w = grid.shape
    for (x, y) in P:
        i = int(round((y - y0) / PITCH)); j = int(round((x - x0) / PITCH))
        i0, j0 = i - r_cells, j - r_cells
        a0, b0 = max(i0, 0), max(j0, 0)
        a1, b1 = min(i0 + disc.shape[0], h), min(j0 + disc.shape[1], w)
        if a1 <= a0 or b1 <= b0:
            continue
        grid[a0:a1, b0:b1] |= disc[a0 - i0:a1 - i0, b0 - j0:b1 - j0]

def nearest(A, B):
    """For each point of A, the XY distance to the nearest point of B (grid hash)."""
    if not B:
        return np.array([])
    cell = 2.0
    h = {}
    for q in B:
        h.setdefault((int(q[0] // cell), int(q[1] // cell)), []).append(q)
    out = []
    for p in A:
        ci, cj = int(p[0] // cell), int(p[1] // cell)
        best = math.inf
        for ring in range(0, 40):
            for di in range(-ring, ring + 1):
                for dj in range(-ring, ring + 1):
                    if max(abs(di), abs(dj)) != ring:
                        continue
                    for q in h.get((ci + di, cj + dj), ()):
                        best = min(best, math.hypot(p[0] - q[0], p[1] - q[1]))
            if best <= ring * cell:
                break
        out.append(best)
    return np.array(out)

print('run', run)
for z, _ in levels:
    P = pts.get((z, 'perimeter'), []); B = pts.get((z, 'body'), []); W = pts.get((z, 'waterline'), [])
    if not P:
        print(f'Z {z}: no perimeter samples'); continue
    allp = np.array(P + B + W)
    x0 = allp[:, 0].min() - R - 1; y0 = allp[:, 1].min() - R - 1
    w = int((allp[:, 0].max() + R + 1 - x0) / PITCH) + 1; h = int((allp[:, 1].max() + R + 1 - y0) / PITCH) + 1
    gp = np.zeros((h, w), bool); gb = np.zeros((h, w), bool); gw = np.zeros((h, w), bool)
    stamp(gp, x0, y0, P); stamp(gb, x0, y0, B); stamp(gw, x0, y0, W)
    cell_a = PITCH * PITCH
    plen = sum(math.hypot(P[i][0] - P[i - 1][0], P[i][1] - P[i - 1][1]) for i in range(1, len(P)))
    only_sweep = gp & ~gb
    only_sweep_w = gp & ~gb & ~gw
    dpb = nearest(P, B); dpw = nearest(P, W)
    q = lambda a: (f'median {np.median(a):.2f}, p95 {np.percentile(a, 95):.2f}, max {a.max():.2f}' if len(a) else 'none')
    print(f'Z {z}: sweep {len(P)} samples, {plen:.1f} mm; sweep band {gp.sum() * cell_a:.0f} mm2')
    print(f'  band cut by the sweep alone (not the body): {only_sweep.sum() * cell_a:.1f} mm2 = mean width {only_sweep.sum() * cell_a / plen:.3f} mm over the sweep length')
    print(f'  band cut by the sweep alone (not the body, not the waterline): {only_sweep_w.sum() * cell_a:.1f} mm2 = mean width {only_sweep_w.sum() * cell_a / plen:.3f} mm')
    print(f'  sweep sample to nearest body sample (mm): {q(dpb)}; share within {PITCH*3:.1f} mm: {np.mean(dpb <= PITCH*3) if len(dpb) else 0:.3f}')
    print(f'  sweep sample to nearest waterline sample (mm): {q(dpw)}')
    # The waterline samples by their distance to the sweep: at a planned
    # region's wall the waterline runs about R + 0.25 outside the sweep;
    # far from every sweep sample it runs where no region was planned
    # (an F-038-skipped region, or a contour with no region).
    ws = wl_samples.get(z, [])
    if ws:
        dws = nearest([w[0] for w in ws], P)
        near = dws <= R + 0.25 + 1.0
        L = np.array([w[1] for w in ws]); rad = np.array([w[2] for w in ws])
        over = rad > 0.471
        print(f'  waterline linear/arc: {L.sum():.1f} mm, over 0.471: {L[over].sum():.1f} mm; '
              f'at a planned region (<= R + 1.25 mm from the sweep): {L[near].sum():.1f} mm, over {L[near & over].sum():.1f} mm; '
              f'elsewhere: {L[~near].sum():.1f} mm, over {L[~near & over].sum():.1f} mm')
