#!/usr/bin/env python3
"""W0 artifact — the decisive evidence figure for `planner_sim_dexel_parity_agent_search`.

Three panels, all from measured runs recorded in `parity_bisect_evidence.txt`
and `parity_agent_search_run1.txt`:

  A  Directional counters either side of the bisected first-bad commit.
     `sim_higher` = the simulator's stock top is higher = the PLANNER removed
     more than replaying its own emitted toolpath does. The sign REVERSES at
     `fa27b08` on both strategies — the signature of an emitter transform the
     planner's stamp does not mirror.

  B  The gated metric vs the defect. `ContourParallel`'s interior count FALLS
     across the same commit while its directional signature explodes 95x, so
     the number the test gates on is a poor detector of the mechanism.

  C  Fixture curvature contrast (cut-only probes at HEAD). Flat fixtures are
     near-clean; curved fixtures diverge. That is what a sampling mismatch
     between an exact drop-cutter drape and a nearest-cell heightmap lift
     looks like, and it is not what a broken stamping kernel would look like.

Usage: python3 render_parity_evidence.py <out.svg>
"""

import sys

# ── measured data (see parity_bisect_evidence.txt) ─────────────────────
PARENT = "7a95614 (parent)"
BAD = "fa27b08 (first bad)"

# (strategy, planner_higher, sim_higher) at parent and at fa27b08
DIRECTIONAL = [
    ("AgentSearch", (778, 562), (443, 2404)),
    ("ContourParallel", (502, 12), (113, 1144)),
]
# interior divergent cells (the gated number), parent -> fa27b08, bar = 792
INTERIOR = [("AgentSearch", 664, 945), ("ContourParallel", 459, 406)]
BAR = 792
# cut-only probes at HEAD: (label, interior divergent, total cells)
CUTONLY = [
    ("ContourParallel flat", 0, 11664),
    ("AgentSearch flat", 21, 11664),
    ("AgentSearch hemisphere", 479, 7921),
    ("ContourParallel hemisphere", 614, 7921),
]

W, H = 900, 1000
CB = "#1f6fb2"   # planner_higher / before
CR = "#c0392b"   # sim_higher / after
CG = "#888"


def bars(x0, y0, w, h, series, maxv, colors, labels, fmt="{:d}"):
    """Grouped horizontal-baseline bar block. series = list of lists."""
    out = []
    n = sum(len(s) for s in series)
    gap = 16
    bw = (w - gap * (len(series) - 1)) / n
    i = 0
    gi = 0
    for group in series:
        for j, v in enumerate(group):
            x = x0 + i * bw + gi * gap
            bh = (v / maxv) * h if maxv else 0
            out.append(f'<rect x="{x:.1f}" y="{y0 + h - bh:.1f}" width="{bw - 3:.1f}" '
                       f'height="{bh:.1f}" fill="{colors[j]}"/>')
            out.append(f'<text x="{x + bw/2:.1f}" y="{y0 + h - bh - 5:.1f}" '
                       f'text-anchor="middle" style="font:10px monospace">'
                       f'{fmt.format(v)}</text>')
            i += 1
        gi += 1
    out.append(f'<line x1="{x0}" y1="{y0+h}" x2="{x0+w}" y2="{y0+h}" stroke="#333"/>')
    for k, lb in enumerate(labels):
        cx = x0 + (k + 0.5) * (w / len(labels))
        out.append(f'<text x="{cx:.1f}" y="{y0+h+16:.1f}" text-anchor="middle" '
                   f'style="font:11px monospace">{lb}</text>')
    return out


b = [f'<rect width="{W}" height="{H}" fill="#fff"/>']
b.append('<text x="24" y="30" style="font:16px monospace;font-weight:bold">'
         'W0 / test 3 — planner_sim_dexel_parity_agent_search: measured evidence</text>')
b.append('<text x="24" y="50" style="font:11px monospace;fill:#555">'
         'hemisphere r=20, flat endmill r=3.175, cell 0.5292 mm, tol = 1 cell. '
         'Bisected first bad commit: fa27b08 (drape / gouge guard).</text>')

# ── Panel A ────────────────────────────────────────────────────────────
b.append('<text x="24" y="92" style="font:13px monospace;font-weight:bold">'
         'A — directional counters reverse at the first bad commit</text>')
b.append('<text x="24" y="110" style="font:10px monospace;fill:#555">'
         'blue = planner_higher (simulator removed more) | '
         'red = sim_higher (PLANNER removed more than its own emitted path)</text>')
y = 130
for si, (name, pre, post) in enumerate(DIRECTIONAL):
    yy = y + si * 150
    b.append(f'<text x="24" y="{yy+10}" style="font:11px monospace;font-weight:bold">'
             f'{name}</text>')
    b += bars(120, yy, 660, 100,
              [list(pre), list(post)], 2500, [CB, CR],
              [PARENT, BAD])
b.append(f'<text x="120" y="{y+300}" style="font:11px monospace;fill:#333">'
         f'ContourParallel\'s sim_higher goes 12 -&gt; 1144 (95x). '
         f'One commit; both strategies; same direction.</text>')

# ── Panel B ────────────────────────────────────────────────────────────
yb = 460
b.append(f'<text x="24" y="{yb}" style="font:13px monospace;font-weight:bold">'
         f'B — the gated number is a poor detector of that mechanism</text>')
b.append(f'<text x="24" y="{yb+18}" style="font:10px monospace;fill:#555">'
         f'interior divergent cells; the test fails above {BAR} '
         f'(= total/10, a whole-grid tenth compared against an interior count)</text>')
maxv = 1000
y0 = yb + 30
b += bars(120, y0, 660, 110,
          [[INTERIOR[0][1], INTERIOR[0][2]], [INTERIOR[1][1], INTERIOR[1][2]]],
          maxv, [CG, CR], [INTERIOR[0][0], INTERIOR[1][0]])
ybar = y0 + 110 - (BAR / maxv) * 110
b.append(f'<line x1="120" y1="{ybar:.1f}" x2="780" y2="{ybar:.1f}" stroke="#c00" '
         f'stroke-dasharray="6 4"/>')
b.append(f'<text x="784" y="{ybar+4:.1f}" style="font:10px monospace;fill:#c00">'
         f'bar {BAR}</text>')
b.append(f'<text x="120" y="{y0+150}" style="font:11px monospace;fill:#333">'
         f'grey = parent, red = fa27b08. ContourParallel\'s interior count FALLS '
         f'(459 -&gt; 406) across the commit that</text>')
b.append(f'<text x="120" y="{y0+166}" style="font:11px monospace;fill:#333">'
         f'broke it 95x harder by the directional measure — so it stays green. '
         f'It is not clean; it is under-detected.</text>')

# ── Panel C ────────────────────────────────────────────────────────────
yc = 720
b.append(f'<text x="24" y="{yc}" style="font:13px monospace;font-weight:bold">'
         f'C — flat fixtures are clean, curved fixtures diverge (cut-only probes, HEAD)</text>')
b.append(f'<text x="24" y="{yc+18}" style="font:10px monospace;fill:#555">'
         f'interior divergent cells, Cut segments only (entries/pecks excluded)</text>')
y0 = yc + 34
mx = 700
for i, (lb, v, tot) in enumerate(CUTONLY):
    yy = y0 + i * 34
    wpx = (v / mx) * 480 if mx else 0
    col = CG if v < 50 else CR
    b.append(f'<text x="24" y="{yy+13}" style="font:11px monospace">{lb}</text>')
    b.append(f'<rect x="250" y="{yy}" width="{max(wpx,1.5):.1f}" height="17" fill="{col}"/>')
    b.append(f'<text x="{250+max(wpx,1.5)+6:.1f}" y="{yy+13}" '
             f'style="font:11px monospace">{v} / {tot}</text>')
b.append(f'<text x="24" y="{y0+150}" style="font:11px monospace;fill:#333">'
         f'On a flat mesh the nearest-cell heightmap lift and the exact drop-cutter '
         f'drape agree, so parity is near-perfect.</text>')
b.append(f'<text x="24" y="{y0+166}" style="font:11px monospace;fill:#333">'
         f'Curvature is what separates them. A broken stamping kernel would '
         f'diverge on flat stock too; this does not.</text>')

svg = (f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" '
       f'viewBox="0 0 {W} {H}">\n' + "\n".join(b) + "\n</svg>\n")

out = sys.argv[1] if len(sys.argv) > 1 else "parity_evidence.svg"
with open(out, "w") as f:
    f.write(svg)
print(f"wrote {out}")
