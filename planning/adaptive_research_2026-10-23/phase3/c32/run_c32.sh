#!/usr/bin/env bash
# C32: wanaka200 '2 Back Rough' body samples over 0.471: exact width, mechanism, level, slope.
set -euo pipefail
cd "$(dirname "$0")"
P=..
python3 -I c32.py $P/fix2/simart/wanaka200/geodesic_local_zigzag $P/fix2/simart/wanaka200/geodesic_local_zigzag zigzag
python3 -I c32.py $P/fix6/wanaka200/contour_parallel $P/fix6/wanaka200/contour_parallel cp
python3 -I report32.py zigzag cp
python3 -I - <<'PY'
import pickle, collections, numpy as np
for tag in ('zigzag', 'cp'):
    rows = pickle.load(open(f'c32_{tag}.pkl', 'rb'))['rows']
    A = [r for r in rows if r['th']]
    th = np.array([r['th'][0] for r in A]); w = np.array([r['ln'] for r in A])
    print(f'\n{tag}: step (A) layer thickness, length-weighted bins (mm of cut):')
    for lo, hi in ((0, 0.25), (0.25, 0.5), (0.5, 1), (1, 2), (2, 99)):
        sel = (th > lo) & (th <= hi); print(f'   ({lo}, {hi}] mm: {w[sel].sum():.1f}')
    t = collections.defaultdict(float)
    for r in rows:
        if r['thick'] > 0.471: t[(r['lz'], '0' if r['strict'] <= 0.471 else ('A' if r['loose'] <= 0.471 else 'B'))] += r['ln']
    print(f'{tag}: exact over 0.471 with only stock >= 0.5 mm (one planner cell) thick counted:', round(sum(t.values()), 1), {k: round(v, 1) for k, v in sorted(t.items())})
PY
