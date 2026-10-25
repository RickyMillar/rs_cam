"""C32 report: over length by mechanism, level and slope (reads c32_<tag>.pkl)."""
import pickle, sys, collections, os
import numpy as np
B = 0.471
for tag in sys.argv[1:]:
    D = pickle.load(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), f'c32_{tag}.pkl'), 'rb'))
    rows = D['rows']; screen = 0.3626 + (0.5 + D['sim_cell'] + 0.1) / 6
    def mech(r):
        if r['strict'] <= B: return '0 sim-only (exact <= 0.471)'
        if r['loose'] <= B: return 'A step: stock a higher same-level pass left'
        return 'B fresh: no same-level pass cut it at any height'
    def slope_bin(r):
        s = abs(r['dz']) / r['ln'] if r['ln'] > 1e-9 else float('inf')
        for lim, name in ((0.0, 'flat'), (0.1, '<=0.1'), (0.3, '0.1-0.3'), (1.0, '0.3-1'), (float('inf'), '>1')):
            if s <= lim + 1e-9: return name
    def drape_bin(r):
        h = r['z'] - r['lz']
        for lim, name in ((0.05, 'at level'), (1.0, '<=1 mm'), (3.0, '1-3 mm'), (float('inf'), '>3 mm')):
            if h <= lim: return name
    tot = sum(r['ln'] for r in rows)
    print(f'\n== {tag}: body clearing_cut over 0.471: {tot:.1f} mm, {len(rows)} samples; screen {screen:.4f}')
    for kin in ('linear+arc', 'helix'):
        sel = [r for r in rows if (r['kin'] in ('linear', 'arc')) == (kin == 'linear+arc')]
        print(f'  {kin}: {sum(r["ln"] for r in sel):.1f} mm')
    by = collections.defaultdict(float)
    for r in rows: by[mech(r)] += r['ln']
    print('  by mechanism:', {k: round(v, 1) for k, v in sorted(by.items())})
    print('  sim > screen:', round(sum(r['ln'] for r in rows if r['r'] > screen), 1), 'of which exact > 0.471:', round(sum(r['ln'] for r in rows if r['r'] > screen and r['strict'] > B), 1))
    for key, f in (('level', lambda r: r['lz']), ('kinematics', lambda r: 'linear+arc' if r['kin'] in ('linear', 'arc') else r['kin']), ('path slope |dz|/dxy', slope_bin), ('drape above level', drape_bin)):
        t = collections.defaultdict(lambda: collections.defaultdict(float))
        for r in rows: t[f(r)][mech(r)[0]] += r['ln']
        print(f'  by {key} (mm; 0 sim-only / A step / B fresh):')
        for k in sorted(t, key=str):
            v = t[k]; print(f'     {str(k):10s} 0 {v["0"]:7.1f}   A {v["A"]:7.1f}   B {v["B"]:7.1f}')
    for mname in ('A', 'B'):
        sel = [r for r in rows if mech(r)[0] == mname]
        if sel:
            st = np.array([r['strict'] for r in sel]); lo = np.array([r['loose'] for r in sel]); sm = np.array([r['r'] for r in sel])
            print(f'  {mname}: sim median {np.median(sm):.3f}; exact median {np.median(st):.3f} (max {st.max():.3f}); same-level-loose median {np.median(lo):.3f}')
