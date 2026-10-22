"""Zoom crop of one rough op from the exported G-code: one XY box at high detail.

Fed cutting moves are green (G1) and dark green (G2/G3 arcs), G0 moves are
orange, and entry points are coloured as in links_entries.py. The script
uses the same move mapping as links_entries.py (section line i + 1 is move i).

Usage:
  python3 zoom.py --nc F.nc --op "3D Rough" --moves N --entries entry.json \
      --box X0 X1 Y0 Y1 --out crop.png [--title T] [--zmin Z]
"""
import argparse
import json

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402

from links_entries import COLOURS, arc_xy, parse, section, top_level


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--nc", required=True)
    ap.add_argument("--op", required=True)
    ap.add_argument("--moves", type=int, required=True)
    ap.add_argument("--entries", required=True)
    ap.add_argument("--box", type=float, nargs=4, required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--title", default="")
    a = ap.parse_args()
    x0, x1, y0, y1 = a.box
    cmds = section(open(a.nc).read().splitlines(), a.op)
    if len(cmds) - a.moves not in (1, 2):
        raise SystemExit("mapping not safe")
    mv = parse(cmds, [0.0, 0.0, 0.0])[1:1 + a.moves]

    def inside(p):
        return x0 <= p[0] <= x1 and y0 <= p[1] <= y1

    fig, ax = plt.subplots(figsize=(8, 8 * (y1 - y0) / (x1 - x0)), dpi=110)
    for g, s, e, c in mv:
        if not (inside(s) or inside(e)):
            continue
        if g == 0:
            ax.plot([s[0], e[0]], [s[1], e[1]], color="#ff7f0e", lw=1.0)
        elif c is not None:
            px, py = arc_xy(s, e, c, g, 24)
            ax.plot(px, py, color="#1a6b1a", lw=0.7)
        else:
            ax.plot([s[0], e[0]], [s[1], e[1]], color="#33b833", lw=0.7)
    for sp in top_level(json.load(open(a.entries))["spans"]):
        i0, i1 = sp["start_move"], min(sp["end_move"], len(mv) - 1)
        g0 = [i for i in range(i0, i1 + 1) if mv[i][0] == 0]
        p = mv[g0[-1]][2] if g0 else mv[i0][2]
        if inside(p):
            ax.scatter([p[0]], [p[1]], s=30, color=COLOURS.get(sp["label"], "#000"), zorder=3,
                       edgecolors="black", linewidths=0.4)
    ax.set_xlim(x0, x1)
    ax.set_ylim(y0, y1)
    ax.set_aspect("equal")
    ax.set_title(a.title + "  (G1 light green, arcs dark green, G0 orange; dots = entries)", fontsize=8)
    fig.tight_layout()
    fig.savefig(a.out)
    print(a.out)


if __name__ == "__main__":
    main()
