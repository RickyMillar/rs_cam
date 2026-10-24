"""D-FIX1: top views of the rough ops of one exported G-code program.

Reads the program that `rs_cam_cli project --emit-gcode` writes. An op
section is the lines after a comment that holds the op name, up to the next
op-name comment or the program end. Modal G0/G1 with X/Y/Z words.

Classes (the G-code carries no move intent, so this is a reading of the
words only):
  rapid  - a G0 move.
  entry  - a G1 move with no XY travel that goes down, after a G0 or a
           retract (the plunge into stock). Its end point is marked.
  cut    - every other G1 move with XY travel. A keep-down link is a G1
           move too, so it is drawn as a cut (the G-code does not tell them
           apart; feed words are printed per class in the counts).

Usage:
  python3 -I render_fix1.py --nc F.nc --op "3D Rough 1" --out PREFIX
Writes PREFIX_cuts.png, PREFIX_rapids.png, PREFIX_entries.png and
PREFIX_counts.json.
"""
import argparse
import json
import math
import re

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402
from matplotlib.collections import LineCollection  # noqa: E402

WORD = re.compile(r"([GXYZF])\s*(-?\d+(?:\.\d*)?)")


def sections(lines, ops):
    """Line ranges of each op: from its name comment to the next op comment."""
    marks = []
    for i, line in enumerate(lines):
        if "(" in line or ";" in line:
            for op in ops:
                if op in line:
                    marks.append((i, op))
                    break
    out = {}
    for k, (i, op) in enumerate(marks):
        end = marks[k + 1][0] if k + 1 < len(marks) else len(lines)
        out.setdefault(op, (i, end))
    return out


def parse(lines, start, end, pos):
    moves = []
    g = 0
    x, y, z = pos
    for line in lines[start:end]:
        code = re.sub(r"\(.*?\)|;.*", "", line).upper()
        words = WORD.findall(code)
        if not words:
            continue
        nx, ny, nz = x, y, z
        moved = False
        for w, v in words:
            v = float(v)
            if w == "G" and v in (0.0, 1.0):
                g = int(v)
            elif w == "X":
                nx, moved = v, True
            elif w == "Y":
                ny, moved = v, True
            elif w == "Z":
                nz, moved = v, True
        if moved:
            moves.append((g, (x, y, z), (nx, ny, nz)))
            x, y, z = nx, ny, nz
    return moves, (x, y, z)


def classify(moves):
    out = []
    after_rapid = True
    for g, a, b in moves:
        xy = math.hypot(b[0] - a[0], b[1] - a[1])
        if g == 0:
            kind = "rapid"
            after_rapid = True
        elif xy < 1e-6 and b[2] < a[2]:
            kind = "entry" if after_rapid else "descend"
        elif xy < 1e-6:
            kind = "lift"
            after_rapid = True
        else:
            kind = "cut"
            after_rapid = False
        out.append((kind, a, b))
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--nc", required=True)
    ap.add_argument("--op", required=True, action="append")
    ap.add_argument("--out", required=True)
    args = ap.parse_args()
    lines = open(args.nc).read().splitlines()
    secs = sections(lines, args.op)
    counts = {}
    for op in args.op:
        if op not in secs:
            counts[op] = {"missing": True}
            continue
        s, e = secs[op]
        moves, _ = parse(lines, s, e, (0.0, 0.0, 0.0))
        cls = classify(moves)
        cuts = [(a, b) for k, a, b in cls if k == "cut"]
        rapids = [(a, b) for k, a, b in cls if k == "rapid" and math.hypot(b[0] - a[0], b[1] - a[1]) > 1e-6]
        entries = [b for k, a, b in cls if k == "entry"]
        cut_len = sum(math.hypot(b[0] - a[0], b[1] - a[1]) for a, b in cuts)
        rapid_len = sum(math.hypot(b[0] - a[0], b[1] - a[1]) for a, b in rapids)
        counts[op] = {
            "lines": e - s,
            "cut_moves": len(cuts),
            "cut_xy_mm": round(cut_len, 1),
            "rapid_xy_moves": len(rapids),
            "rapid_xy_mm": round(rapid_len, 1),
            "entries": len(entries),
        }
        tag = re.sub(r"[^A-Za-z0-9]+", "_", op).strip("_")
        allxy = [p for a, b in cuts + rapids for p in (a, b)] or [(0, 0, 0)]
        xs = [p[0] for p in allxy]
        ys = [p[1] for p in allxy]
        lim = (min(xs) - 2, max(xs) + 2, min(ys) - 2, max(ys) + 2)
        zs = sorted({round(b[2], 2) for a, b in cuts})

        def frame(title):
            fig, ax = plt.subplots(figsize=(6, 6 * (lim[3] - lim[2]) / max(lim[1] - lim[0], 1e-9) + 0.6))
            ax.set_aspect("equal")
            ax.set_xlim(lim[0], lim[1])
            ax.set_ylim(lim[2], lim[3])
            ax.set_title(title, fontsize=9)
            ax.tick_params(labelsize=7)
            return fig, ax

        fig, ax = frame(f"{op}: cutting moves by Z ({len(cuts)} moves, {len(zs)} Z values)")
        segs = [[(a[0], a[1]), (b[0], b[1])] for a, b in cuts]
        lc = LineCollection(segs, linewidths=0.5, cmap="viridis")
        lc.set_array([b[2] for a, b in cuts] or [0])
        ax.add_collection(lc)
        cb = fig.colorbar(lc, ax=ax, fraction=0.04)
        cb.set_label("Z (mm)", fontsize=7)
        cb.ax.tick_params(labelsize=7)
        fig.savefig(f"{args.out}_{tag}_cuts.png", dpi=130, bbox_inches="tight")
        plt.close(fig)

        fig, ax = frame(f"{op}: rapids with XY travel ({len(rapids)}, {rapid_len:.0f} mm)")
        ax.add_collection(LineCollection([[(a[0], a[1]), (b[0], b[1])] for a, b in cuts], linewidths=0.3, colors="#dddddd"))
        ax.add_collection(LineCollection([[(a[0], a[1]), (b[0], b[1])] for a, b in rapids], linewidths=0.6, colors="#e07000"))
        fig.savefig(f"{args.out}_{tag}_rapids.png", dpi=130, bbox_inches="tight")
        plt.close(fig)

        fig, ax = frame(f"{op}: entries (plunges after a rapid) = {len(entries)}")
        ax.add_collection(LineCollection([[(a[0], a[1]), (b[0], b[1])] for a, b in cuts], linewidths=0.3, colors="#dddddd"))
        if entries:
            ax.scatter([p[0] for p in entries], [p[1] for p in entries], s=14, facecolors="none", edgecolors="#dd0000", linewidths=0.9)
        fig.savefig(f"{args.out}_{tag}_entries.png", dpi=130, bbox_inches="tight")
        plt.close(fig)
    with open(f"{args.out}_counts.json", "w") as fh:
        json.dump(counts, fh, indent=1)
    print(json.dumps(counts))


if __name__ == "__main__":
    main()
