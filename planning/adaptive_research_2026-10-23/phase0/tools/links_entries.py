"""Links view and entries view of one rough op, from the EXPORTED G-code.

The script reads the op section of the .nc file (the lines after the op-name
comment, up to the next op comment or M5). It maps move index i of the op to
command line (i + 1) of the section: the post adds one G0 to the clearance
plane before the op, and one more G0 at the end of the program (surplus 1,
or 2 for the last op). Evidence (rivmap100,
2026-10-23): the cut trace puts move 32 on the long +X cut at Z -2.45,
which is section line 33. Corne 2026-10-23: the nested plunge spans at
moves 2 and 4 are section lines 3 and 5 (the two G1 plunges). The script
stops when the surplus is not 1 or 2. As a check it
prints the share of moves inside link_bridge spans that are G0 moves.

inspect_spans returns nested Entry spans: a short span inside a longer one
(on rivmap100 exactly half of all Entry spans). The script keeps only the
top-level spans (not inside a longer Entry span) for the counts and the
points, and prints the nested count apart.

An entry span starts one move before its link. The script puts the entry
point at the end of the last G0 move inside the span (the point where the
tool goes down). A span with no G0 move (a keep-down link) gets the end of
its first move.

Usage:
  python3 links_entries.py --nc F.nc --op "3D Rough" --moves 17649 \
      --entries entry.json [--links link_bridge.json] --out PREFIX

Writes PREFIX_links.png (top-down XY: G0 moves in orange, the fed moves
inside "keep-down link" entry spans in blue, cutting moves faint grey) and PREFIX_entries.png (entry
start points coloured by the inspect_spans label).
"""
import argparse
import json
import math
import re

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402

WORD = re.compile(r"([GXYZIJF])(-?\d+(?:\.\d+)?)")
COLOURS = {
    "helix entry": "#d62728",
    "ramp entry": "#2ca02c",
    "plunge entry": "#1f77b4",
    "keep-down link": "#9467bd",
}


def top_level(spans):
    """Entry spans that are not inside a longer Entry span."""
    out = []
    for s in spans:
        n = s["end_move"] - s["start_move"]
        inside = any(o is not s and o["start_move"] <= s["start_move"] and s["end_move"] <= o["end_move"]
                     and (o["end_move"] - o["start_move"]) > n for o in spans)
        if not inside:
            out.append(s)
    return out


def section(lines, op):
    start = None
    for k, ln in enumerate(lines):
        if ln.strip() == f"({op})":
            start = k + 1
            break
    if start is None:
        raise SystemExit(f"no section ({op})")
    out = []
    for ln in lines[start:]:
        s = ln.strip()
        if s.startswith("("):
            if out and not s.startswith("(feeds"):
                break
            continue
        if s[:1] in "MTS":
            if out:
                break
            continue
        if s.startswith("G0") or s.startswith("G1") or s.startswith("G2") or s.startswith("G3") or s[:1] in "XYZ":
            out.append(s)
    return out


def parse(cmds, pos):
    """Return a list of (gmode, start xyz, end xyz, arc centre or None, cw)."""
    moves = []
    g = 0
    for s in cmds:
        w = dict((a, float(b)) for a, b in WORD.findall(s))
        if "G" in w:
            g = int(w["G"])
        end = [w.get("X", pos[0]), w.get("Y", pos[1]), w.get("Z", pos[2])]
        ctr = None
        if g in (2, 3):
            ctr = (pos[0] + w.get("I", 0.0), pos[1] + w.get("J", 0.0))
        moves.append((g, tuple(pos), tuple(end), ctr))
        pos = end
    return moves


def arc_xy(a, b, c, g, n=12):
    r = math.hypot(a[0] - c[0], a[1] - c[1])
    t0 = math.atan2(a[1] - c[1], a[0] - c[0])
    t1 = math.atan2(b[1] - c[1], b[0] - c[0])
    if g == 3 and t1 <= t0:
        t1 += 2 * math.pi
    if g == 2 and t1 >= t0:
        t1 -= 2 * math.pi
    if abs(t1 - t0) < 1e-9:
        t1 = t0 + (2 * math.pi if g == 3 else -2 * math.pi)
    ts = [t0 + (t1 - t0) * k / n for k in range(n + 1)]
    return [c[0] + r * math.cos(t) for t in ts], [c[1] + r * math.sin(t) for t in ts]


def draw_cut(ax, moves, colour, lw, alpha):
    xs, ys = [], []
    for g, a, b, c in moves:
        if g == 0:
            continue
        if c is not None:
            px, py = arc_xy(a, b, c, g)
        else:
            px, py = [a[0], b[0]], [a[1], b[1]]
        xs += px + [None]
        ys += py + [None]
    ax.plot(xs, ys, color=colour, lw=lw, alpha=alpha)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--nc", required=True)
    ap.add_argument("--op", required=True)
    ap.add_argument("--moves", type=int, required=True)
    ap.add_argument("--entries", required=True)
    ap.add_argument("--links")
    ap.add_argument("--out", required=True)
    ap.add_argument("--title", default="")
    a = ap.parse_args()

    lines = open(a.nc).read().splitlines()
    cmds = section(lines, a.op)
    surplus = len(cmds) - a.moves
    if surplus not in (1, 2):
        raise SystemExit(f"surplus {surplus} (cmds {len(cmds)}, moves {a.moves}); mapping not safe")
    offset = 1
    # Start position: the last G0 before the section, or the first command.
    moves = parse(cmds, [0.0, 0.0, 0.0])
    op_moves = moves[offset:offset + a.moves]

    ent_all = json.load(open(a.entries))["spans"]
    ent = top_level(ent_all)
    nested = {}
    for sp in ent_all:
        if not any(sp is t for t in ent):
            nested[sp["label"]] = nested.get(sp["label"], 0) + 1
    links = json.load(open(a.links))["spans"] if a.links else []
    keep = [sp for sp in ent if sp["label"] == "keep-down link"]

    n_rapid = sum(1 for m in op_moves if m[0] == 0)
    # Retract round trips: runs of consecutive G0 moves.
    runs = 0
    prev = False
    for m in op_moves:
        cur = m[0] == 0
        if cur and not prev:
            runs += 1
        prev = cur

    # Links view.
    fig, ax = plt.subplots(figsize=(10, 10), dpi=100)
    draw_cut(ax, op_moves, "#bbbbbb", 0.3, 0.6)
    xs, ys = [], []
    for g, s, e, c in op_moves:
        if g == 0 and (abs(s[0] - e[0]) > 1e-6 or abs(s[1] - e[1]) > 1e-6):
            xs += [s[0], e[0], None]
            ys += [s[1], e[1], None]
    ax.plot(xs, ys, color="#ff7f0e", lw=0.9, label=f"G0 moves with XY travel (runs of G0: {runs})")
    lx, ly = [], []
    for sp in keep:
        for i in range(sp["start_move"], min(sp["end_move"], len(op_moves))):
            g, s, e, c = op_moves[i]
            if g != 0:
                lx += [s[0], e[0], None]
                ly += [s[1], e[1], None]
    if keep:
        ax.plot(lx, ly, color="#1f3fbf", lw=1.0, label=f"fed moves inside keep-down link entry spans ({len(keep)})")
    ax.set_aspect("equal")
    ax.set_title(f"{a.title} links: G0 (orange), keep-down links (blue), cut (grey)")
    ax.legend(loc="upper right", fontsize=8)
    fig.tight_layout()
    fig.savefig(a.out + "_links.png")
    plt.close(fig)

    # Entries view.
    fig, ax = plt.subplots(figsize=(10, 10), dpi=100)
    draw_cut(ax, op_moves, "#cccccc", 0.3, 0.6)
    counts = {}
    for sp in ent:
        lab = sp["label"]
        counts[lab] = counts.get(lab, 0) + 1
    for lab, n in sorted(counts.items()):
        pts = []
        for sp in ent:
            if sp["label"] != lab:
                continue
            i0, i1 = sp["start_move"], min(sp["end_move"], len(op_moves) - 1)
            g0 = [i for i in range(i0, i1 + 1) if op_moves[i][0] == 0]
            pts.append(op_moves[g0[-1]][2] if g0 else op_moves[i0][2])
        ax.scatter([p[0] for p in pts], [p[1] for p in pts], s=14,
                   color=COLOURS.get(lab, "#000000"), label=f"{lab}: {n}", zorder=3)
    nested_spans = [sp for sp in ent_all if not any(sp is t for t in ent)]
    for lab in sorted(nested):
        pts = [op_moves[min(sp["start_move"], len(op_moves) - 1)][2] for sp in nested_spans if sp["label"] == lab]
        ax.scatter([p[0] for p in pts], [p[1] for p in pts], s=10, marker="x", linewidths=0.8,
                   color=COLOURS.get(lab, "#000000"), label=f"nested {lab}: {nested[lab]}", zorder=4)
    ax.set_aspect("equal")
    ax.set_title(f"{a.title} entries (dots: top-level Entry spans; x: nested Entry spans)")
    ax.legend(loc="upper right", fontsize=8)
    fig.tight_layout()
    fig.savefig(a.out + "_entries.png")
    plt.close(fig)

    in_link = [op_moves[i][0] == 0 for sp in links
               for i in range(sp["start_move"], min(sp["end_move"], len(op_moves)))]
    g0_share = sum(in_link) / len(in_link) if in_link else None
    print(json.dumps({"surplus": surplus, "link_moves_g0_share": g0_share, "cmds": len(cmds), "g0_moves": n_rapid,
                      "g0_runs": runs, "entries": counts, "nested_entries": nested, "link_bridges": len(links)}))


if __name__ == "__main__":
    main()
