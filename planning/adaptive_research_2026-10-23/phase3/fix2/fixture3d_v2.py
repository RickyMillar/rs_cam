"""D-FIX2: build fixture3d_v2.html, one Z level per image in the 2D colours.

Sources per run directory (`rs_cam_cli project --emit-gcode program.nc`):
  program.nc        the exported G-code (geometry of every move)
  tp_<id>_*.json    the per-toolpath record; its `semantic_trace.items`
                    give move ranges: `depth_level` ("Global Z ..."),
                    `entry` ("Pass N plunge entry", "Pass N keep-down link",
                    helix / ramp), `cleanup` ("Waterline cleanup")
  summary.json      the CLI totals (runtime, cutting and rapid distance,
                    collisions, verdict)
  stderr.txt        the CLI findings (Safety, Act on)

Move mapping (as phase0/tools/links_entries.py): move i of an op is
command line i + 1 of the op's section; the post adds one G0 before the op
and one more at the end of the program, so the surplus is 1 or 2. The
script stops when it is not.

Classes, per move of one op:
  entry ring   the point where a top-level entry span goes down: the end of
               the last G0 in the span; a span with no G0 (a keep-down
               link) has no ring.
  link         a fed move (not G0) inside a top-level keep-down-link span:
               drawn orange (keep-down travel at depth).
  rapid        a G0 move with XY travel: grey dashed.
  cut          every other fed move with XY travel at the level: blue.
Turns (C22) carry no tag in the trace; they are fed moves and draw blue.

The page shows ONE op per fixture in the images (the first rough op) and
three levels that every arm cuts: the first, the middle and the last.
The frame and scale are the same for every arm of a fixture.

Writes the page and `fixture3d_v2_numbers.json` (every number on the page),
which `assert_fixture3d_v2.py` re-derives from the raw files.
"""
import html
import json
import math
import os
import re
import sys
import time
import glob

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402

WORD = re.compile(r"([GXYZIJF])(-?\d+(?:\.\d+)?)")
BLUE, ORANGE, GREY, RED = "#1f5fbf", "#e07000", "#888888", "#d00000"

RS = "/home/ricky/personal_repos/rs_cam"
RES = f"{RS}/planning/adaptive_research_2026-10-23"
BIN = f"{RS}/.claude/worktrees/fixture-cdc9814e/target/release/rs_cam_cli"


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
        if s.startswith(("G0", "G1", "G2", "G3")) or s[:1] in "XYZ":
            out.append(s)
    return out


def parse(cmds):
    moves = []
    g = 0
    pos = [0.0, 0.0, 0.0]
    for s in cmds:
        w = dict((a, float(b)) for a, b in WORD.findall(s))
        if "G" in w:
            g = int(w["G"])
        end = [w.get("X", pos[0]), w.get("Y", pos[1]), w.get("Z", pos[2])]
        moves.append((g, tuple(pos), tuple(end)))
        pos = end
    return moves


def top_level(spans):
    out = []
    for s in spans:
        n = s["move_end"] - s["move_start"]
        inside = any(o is not s and o["move_start"] <= s["move_start"] and s["move_end"] <= o["move_end"]
                     and (o["move_end"] - o["move_start"]) > n for o in spans)
        if not inside:
            out.append(s)
    return out


def load_op(run_dir, tp_glob, op_comment):
    tpf = sorted(glob.glob(os.path.join(run_dir, tp_glob)))
    if len(tpf) != 1:
        raise SystemExit(f"{run_dir}: {tp_glob} matches {tpf}")
    tp = json.load(open(tpf[0]))
    lines = open(os.path.join(run_dir, "program.nc")).read().splitlines()
    cmds = section(lines, op_comment)
    n = tp["move_count"]
    surplus = len(cmds) - n
    if surplus not in (1, 2):
        raise SystemExit(f"{run_dir} {op_comment}: {len(cmds)} commands for {n} moves (surplus {surplus})")
    gm = parse(cmds)[1:1 + n]  # move i = command line i + 1
    items = tp["semantic_trace"]["items"]
    # The depth_level items carry no move range (move_start / move_end are
    # null in every run), so a move takes its level from Z: a fed move is at
    # the highest level plane at or below its Z (a draped move stays with its
    # level); a G0 takes the level of the next fed move.
    planes = sorted({float(it["label"].split()[-1]) for it in items if it["kind"] == "depth_level"}, reverse=True)
    names = {z: f"Z {z:.2f}" for z in planes}
    entries = top_level([it for it in items if it["kind"] == "entry"])
    cls = ["cut"] * n
    rings = []
    for e in entries:
        lo, hi = e["move_start"], min(e["move_end"], n - 1)
        link = "keep-down link" in e["label"]
        last_g0 = None
        for i in range(lo, hi + 1):
            if gm[i][0] == 0:
                last_g0 = i
            elif link:
                cls[i] = "link"
        if not link and last_g0 is not None:
            rings.append((last_g0, gm[last_g0][2]))
    for i, (g, a, b) in enumerate(gm):
        if g == 0:
            cls[i] = "rapid"
    # The G-code Z is in the setup's work frame, not the model frame of the
    # "Global Z" labels. The frame offset is the one that puts the most flat
    # fed XY moves on the label planes; the top plane must hold flat moves,
    # else the script stops. (Corne -23.0, wanaka200 -25.0 in these runs.)
    hist = {}
    for g, a, b in gm:
        if g != 0 and xy(a, b) > 1e-6 and abs(b[2] - a[2]) < 1e-6:
            hist[round(b[2], 2)] = hist.get(round(b[2], 2), 0) + 1
    offs = {round(z - p, 2) for z in hist for p in planes}
    score, off = max((sum(hist.get(round(p + o, 2), 0) for p in planes), o) for o in offs)
    if hist.get(round(planes[0] + off, 2), 0) == 0:
        raise SystemExit(f"{run_dir} {op_comment}: no frame offset puts flat moves on the top plane")
    nc_planes = [round(p + off, 2) for p in planes]
    lev = [None] * n
    for i, (g, a, b) in enumerate(gm):
        if g != 0:
            below = [k for k, z in enumerate(nc_planes) if z <= b[2] + 0.01]
            lev[i] = names[planes[below[0]]] if below else names[planes[-1]]
    nxt = None
    for i in range(n - 1, -1, -1):
        if gm[i][0] == 0:
            lev[i] = nxt
        else:
            nxt = lev[i]
    levels = [names[z] for z in planes]
    return {"tp": tp, "moves": gm, "cls": cls, "rings": rings, "levels": levels, "lev": lev, "offset": off,
            "entries_plunge": sum(1 for e in entries if "keep-down link" not in e["label"]),
            "entries_keepdown": sum(1 for e in entries if "keep-down link" in e["label"])}




def xy(a, b):
    return math.hypot(b[0] - a[0], b[1] - a[1])


def op_numbers(d):
    gm, cls = d["moves"], d["cls"]
    return {
        "moves": len(gm),
        "entries": d["entries_plunge"],
        "keepdown_links": d["entries_keepdown"],
        "keepdown_mm": round(sum(xy(a, b) for (g, a, b), c in zip(gm, cls) if c == "link"), 1),
        "cut_xy_mm": round(sum(xy(a, b) for (g, a, b), c in zip(gm, cls) if c == "cut"), 1),
    }


def draw(d, lab, frame, path, title):
    gm, cls = d["moves"], d["cls"]
    fig, ax = plt.subplots(figsize=(4.2, 4.2 * (frame[3] - frame[2]) / (frame[1] - frame[0]) + 0.4))
    ax.set_aspect("equal")
    ax.set_xlim(frame[0], frame[1])
    ax.set_ylim(frame[2], frame[3])
    ax.tick_params(labelsize=6)
    ax.set_title(title, fontsize=8)
    seg = {"cut": [], "link": [], "rapid": []}
    n_ring = 0
    for i, ((g, a, b), c) in enumerate(zip(gm, cls)):
        if d["lev"][i] != lab or xy(a, b) < 1e-9:
            continue
        seg[c].append((a, b))
    for c, col, ls, lw in (("rapid", GREY, (0, (3, 2)), 0.5), ("cut", BLUE, "-", 0.45), ("link", ORANGE, "-", 0.9)):
        for a, b in seg[c]:
            ax.plot([a[0], b[0]], [a[1], b[1]], color=col, lw=lw, ls=ls)
    pts = [p for i, p in d["rings"] if d["lev"][i] == lab]
    if pts:
        ax.scatter([p[0] for p in pts], [p[1] for p in pts], s=22, facecolors="none", edgecolors=RED, linewidths=1.0, zorder=5)
    fig.savefig(path, dpi=120, bbox_inches="tight")
    plt.close(fig)
    return {"entries": len(pts),
            "keepdown_mm": round(sum(xy(a, b) for a, b in seg["link"]), 1),
            "cut_mm": round(sum(xy(a, b) for a, b in seg["cut"]), 1)}


def findings(run_dir):
    err = open(os.path.join(run_dir, "stderr.txt")).read()
    out = [ln.strip() for ln in err.splitlines() if "island(s) standing" in ln]
    m = re.search(r"Act on \(\d+\):\n((?:  .*\n)+)", err)
    if m:
        out += [ln.strip() for ln in m.group(1).splitlines()]
    return out


def main():
    spec = json.load(open(sys.argv[1]))
    numbers = {"binary": BIN, "binary_mtime": time.strftime("%Y-%m-%dT%H:%M:%S", time.localtime(os.path.getmtime(BIN))),
               "fixtures": {}}
    rows = []
    for fx in spec["fixtures"]:
        arms = [a for a in fx["arms"] if os.path.exists(os.path.join(a["dir"], "summary.json"))]
        missing = [a for a in fx["arms"] if a not in arms]
        ops = {a["label"]: [load_op(a["dir"], o["tp_glob"], o["comment"]) for o in fx["ops"]] for a in arms}
        img_op = 0
        common = None
        for a in arms:
            d0 = ops[a["label"]][img_op]
            labs = [lab for lab in d0["levels"]
                    if any(c == "cut" and l == lab for c, l in zip(d0["cls"], d0["lev"]))]
            common = labs if common is None else [x for x in common if x in labs]
        common = common or []
        pick = [common[0], common[len(common) // 2], common[-1]] if len(common) >= 3 else common
        pts = [p for a in arms for (g, p0, p) in ops[a["label"]][img_op]["moves"] if g != 0]
        xs, ys = [p[0] for p in pts], [p[1] for p in pts]
        pad = 3.0
        frame = (min(xs) - pad, max(xs) + pad, min(ys) - pad, max(ys) + pad)
        fxn = {"levels_shown": pick, "arms": {}}
        cols = []
        for a in arms:
            s = json.load(open(os.path.join(a["dir"], "summary.json")))
            an = {"commit": a["commit"], "cycle_time_s": round(s["total_runtime_s"]),
                  "cutting_mm": round(s["total_cutting_distance_mm"]),
                  "rapid_mm": round(s["total_rapid_distance_mm"]),
                  "air_cut_pct": round(s["air_cut_pct_of_total_runtime"], 1),
                  "collisions": s["collision_count"], "rapid_collisions": s["rapid_collision_count"],
                  "sim_resolution": s.get("simulation_resolution"),
                  "ops": {}, "images": {}, "findings": findings(a["dir"])}
            for o, d in zip(fx["ops"], ops[a["label"]]):
                an["ops"][o["comment"]] = op_numbers(d)
            imgs = []
            for k, lab in enumerate(pick):
                tag = re.sub(r"[^A-Za-z0-9]+", "_", f"{fx['name']}_{a['label']}_{k}").strip("_")
                rel = f"phase3/fix2/img/{tag}.png"
                os.makedirs(f"{RES}/phase3/fix2/img", exist_ok=True)
                im = draw(ops[a["label"]][img_op], lab, frame, f"{RES}/{rel}", f"{a['label']} - {lab}")
                an["images"][lab] = im
                sent = (f"At {lab}: {im['entries']} red ring(s) where the cutter goes down, "
                        f"{im['keepdown_mm']:.0f} mm of orange keep-down travel at depth, "
                        f"{im['cut_mm']:.0f} mm of blue cutting.")
                imgs.append((rel, lab, sent))
            fxn["arms"][a["label"]] = an
            cols.append((a, an, imgs))
        numbers["fixtures"][fx["name"]] = fxn
        rows.append((fx, pick, cols, missing))
    json.dump(numbers, open(f"{RES}/phase3/fix2/fixture3d_v2_numbers.json", "w"), indent=1)

    def col_html(fx, a, an, imgs):
        trs = "".join(f"<tr><td>{html.escape(op)}</td><td>{v['entries']}</td><td>{v['keepdown_links']}</td>"
                      f"<td>{v['keepdown_mm']:.0f}</td><td>{v['cut_xy_mm']:.0f}</td></tr>" for op, v in an["ops"].items())
        figs = "".join(f'<figure><img src="{r}" loading="lazy"><figcaption>{html.escape(sent)}</figcaption></figure>'
                       for r, lab, sent in imgs)
        bars = "".join(f"<li>{html.escape(x)}</li>" for x in an["findings"]) or "<li>none printed</li>"
        res = an["sim_resolution"]
        return f'''<div class="col"><h3>{html.escape(a["label"])} <span class="commit">({an["commit"]})</span></h3>
<div class="imgs">{figs}</div>
<table><tr><th>op</th><th>entries</th><th>keep-down links</th><th>keep-down mm</th><th>cut mm</th></tr>{trs}</table>
<table><tr><td>cycle time (rough ops)</td><td>{an["cycle_time_s"]} s</td></tr>
<tr><td>cutting / rapid distance</td><td>{an["cutting_mm"]} / {an["rapid_mm"]} mm</td></tr>
<tr><td>air cut</td><td>{an["air_cut_pct"]}% of runtime</td></tr>
<tr><td>collisions (rapid)</td><td>{an["collisions"]} ({an["rapid_collisions"]})</td></tr>
<tr><td>sim cell</td><td>{html.escape(json.dumps(res))}</td></tr></table>
<details><summary>Hard bars printed by the CLI</summary><ul>{bars}</ul></details></div>'''

    body = ""
    for fx, pick, cols, missing in rows:
        miss = "".join(f'<p class="note">{html.escape(m["label"])}: not run ({html.escape(m.get("why", "no output"))}).</p>' for m in missing)
        body += (f'<section><h2>{html.escape(fx["title"])}</h2><p class="note">{html.escape(fx["note"])} '
                 f'Levels shown (the same for every column): {", ".join(map(html.escape, pick))}.</p>{miss}'
                 f'<div class="grid" style="grid-template-columns:repeat({max(len(cols),1)},minmax(0,1fr))">'
                 + "".join(col_html(fx, a, an, imgs) for a, an, imgs in cols) + "</div></section>")
    page = f'''<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Fixture 3D v2</title><style>
:root{{--bg:#fff;--fg:#1a1a1a;--muted:#666;--line:#ddd;--card:#fafafa}}
@media (prefers-color-scheme:dark){{:root:not([data-theme="light"]){{--bg:#16181c;--fg:#e8e8e8;--muted:#a0a0a0;--line:#333;--card:#1e2126}}}}
:root[data-theme="dark"]{{--bg:#16181c;--fg:#e8e8e8;--muted:#a0a0a0;--line:#333;--card:#1e2126}}
body{{background:var(--bg);color:var(--fg);font:14px/1.45 system-ui,sans-serif;margin:0;padding:16px}}
h1{{font-size:20px;margin:0 0 6px}} h2{{font-size:16px;margin:18px 0 4px}} h3{{font-size:14px;margin:0 0 6px}}
.note,.meta{{color:var(--muted);font-size:13px}} .commit{{color:var(--muted);font-weight:normal;font-size:12px}}
.grid{{display:grid;gap:12px}} @media (max-width:1000px){{.grid{{grid-template-columns:1fr !important}}}}
.col{{background:var(--card);border:1px solid var(--line);border-radius:8px;padding:10px;min-width:0}}
.imgs{{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:6px}} @media (max-width:600px){{.imgs{{grid-template-columns:1fr}}}}
figure{{margin:0}} img{{width:100%;height:auto;background:#fff;border:1px solid var(--line)}} figcaption{{font-size:12px;margin-top:2px}}
table{{border-collapse:collapse;width:100%;font-size:12px;margin-top:6px}} td,th{{border:1px solid var(--line);padding:3px 5px;text-align:left}}
details{{font-size:12px;margin-top:4px}} .key span{{display:inline-block;margin-right:12px}}
</style></head><body>
<h1>Fixture 3D v2: one level per image</h1>
<div class="meta">{html.escape(spec["header"])}<br>Binary {html.escape(BIN)}, mtime {numbers["binary_mtime"]}.</div>
<p class="key"><span style="color:{BLUE}">&#9632; blue: cutting moves at that level</span><span style="color:{ORANGE}">&#9632; orange: keep-down links at depth</span><span style="color:{GREY}">- - grey dashed: rapids</span><span style="color:{RED}">&#9675; red ring: an entry (the cutter goes down)</span></p>
<p class="note">{html.escape(spec["method"])}</p>
{body}
<p class="note">{html.escape(spec["turn_note"])}</p>
</body></html>'''
    open(f"{RES}/fixture3d_v2.html", "w").write(page)
    print("page written;", json.dumps({k: v["levels_shown"] for k, v in numbers["fixtures"].items()}))


if __name__ == "__main__":
    main()
