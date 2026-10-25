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

import importlib.util as _ilu

_ps_spec = _ilu.spec_from_file_location("producer_split", os.path.join(os.path.dirname(os.path.abspath(__file__)), "producer_split.py"))
producer_split_mod = _ilu.module_from_spec(_ps_spec)
_ps_spec.loader.exec_module(producer_split_mod)
_ns_spec = _ilu.spec_from_file_location("normal_step", os.path.join(os.path.dirname(os.path.abspath(__file__)), "normal_step.py"))
normal_step_mod = _ilu.module_from_spec(_ns_spec)
_ns_spec.loader.exec_module(normal_step_mod)

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


_SIM = {}


def sim_samples(simart_dir):
    """The samples of one run's simulation.json, loaded once."""
    if simart_dir not in _SIM:
        _SIM[simart_dir] = json.load(open(os.path.join(simart_dir, "simulation.json")))["trace"]["samples"]
    return _SIM[simart_dir]


def move_intents(simart_dir, toolpath_id):
    """The source intent of each move index of one toolpath: the intent of
    its first simulation sample."""
    out = {}
    for sm in sim_samples(simart_dir):
        if sm["toolpath_id"] == toolpath_id and sm["move_index"] not in out:
            out[sm["move_index"]] = sm["source_intent"]
    return out


def load_op(run_dir, tp_glob, op_comment, simart_dir):
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
    # Link tags from the per-move intent (C26 emits held links as Linking
    # moves; the trace has no span for them): a fed move whose sample intent
    # is "linking" is a link, any other fed move with a sample is a cut; a
    # move with no sample keeps the class of the trace spans above.
    tpid = int(re.search(r"tp_(\d+)_", tp_glob).group(1))
    intents = move_intents(simart_dir, tpid)
    for i, (g, a, b) in enumerate(gm):
        if g != 0 and i in intents:
            cls[i] = "link" if intents[i] == "linking" else "cut"
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
        "keepdown_links": sum(1 for i, c in enumerate(cls) if c == "link" and (i == 0 or cls[i - 1] != "link")),
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


TOOL_D = 6.0  # both fixtures' first rough op: a 6 mm flat end mill (the project files)
MERGE = 0.05  # crossings closer than this along a ray are one pass


def ray_spacing(d, lab, centre):
    """Radial spacing of adjacent cut passes at level `lab` along 4 rays
    (0, 90, 180, 270 deg) from `centre`: the blue cut segments of the level,
    cut by each half-line; crossings within MERGE are one pass; a spacing
    over one tool diameter is a region edge, an island or an uncut band and
    is counted apart. Returns the kept spacings (pooled) and the count set
    apart."""
    kept, apart = [], 0
    for ux, uy in ((1, 0), (0, 1), (-1, 0), (0, -1)):
        ts = []
        for i, ((g, a, b), c) in enumerate(zip(d["moves"], d["cls"])):
            if c != "cut" or d["lev"][i] != lab:
                continue
            ex, ey = b[0] - a[0], b[1] - a[1]
            den = ux * ey - uy * ex
            if abs(den) < 1e-12:
                continue
            wx, wy = a[0] - centre[0], a[1] - centre[1]
            t = (wx * ey - wy * ex) / den   # along the ray
            u = (wx * uy - wy * ux) / den   # along the segment
            if t > 0 and 0 <= u <= 1:
                ts.append(t)
        ts.sort()
        passes = []
        for t in ts:
            if not passes or t - passes[-1] > MERGE:
                passes.append(t)
        for t0, t1 in zip(passes, passes[1:]):
            if t1 - t0 <= TOOL_D:
                kept.append(t1 - t0)
            else:
                apart += 1
    return kept, apart


def median(v):
    v = sorted(v)
    n = len(v)
    return (v[n // 2] if n % 2 else 0.5 * (v[n // 2 - 1] + v[n // 2])) if n else None


C18_CEILING = 0.3626  # the pass ceiling at s 2, R 3 (P3_C18_3d_oracle.md section 2)
C18_EXACT = 0.471     # C18's exact bound: ceiling + (planner cell + tolerance + pitch)/D


def load_share(simart_dir, toolpath_id, sim_cell, tol=0.1, planner_cell=0.5, d=TOOL_D):
    """Share of the cut length of one toolpath that the closing simulation
    reads over C18's exact bound (0.471) and over C18's screen for the run's
    sim cell (ceiling + (planner cell + sim cell + tolerance)/D). Cut length:
    the XY distance from the previous sample, summed over the cutting samples
    whose intent is clearing_cut, of any kinematics (entry and link samples left out)."""
    screen = C18_CEILING + (planner_cell + sim_cell + tol) / d
    smp = sim_samples(simart_dir)
    prev, tot, o1, o2 = None, 0.0, 0.0, 0.0
    for sm in smp:
        p = sm["position"]
        ln = math.hypot(p[0] - prev[0], p[1] - prev[1]) if prev else 0.0
        prev = p
        if sm["toolpath_id"] != toolpath_id or not sm["is_cutting"] or sm["source_intent"] != "clearing_cut":
            continue
        r = sm["engagement"]["radial_woc_fraction"]
        tot += ln
        o1 += ln * (r > C18_EXACT)
        o2 += ln * (r > screen)
    return {"cut_mm": round(tot, 1), "over_exact_pct": round(100 * o1 / tot, 2),
            "screen": round(screen, 4), "over_screen_pct": round(100 * o2 / tot, 2)}


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
        ops = {a["label"]: [load_op(a["dir"], o["tp_glob"], o["comment"], a["simart"]) for o in fx["ops"]] for a in arms}
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
        mid = pick[1] if len(pick) == 3 else None
        if mid is not None and "AgentSearch" in ops and "geodesic_local_zigzag" in ops:
            ag = ops["AgentSearch"][img_op]
            gz = ops["geodesic_local_zigzag"][img_op]
            a0 = [p for i, p in ag["rings"] if ag["lev"][i] == mid][0]
            centre = min((p for i, p in gz["rings"] if gz["lev"][i] == mid),
                         key=lambda p: (p[0] - a0[0]) ** 2 + (p[1] - a0[1]) ** 2)
            fxn["centre"] = [round(centre[0], 3), round(centre[1], 3)]
            fxn["mid_level"] = mid
            for a in arms:
                kept, apart = ray_spacing(ops[a["label"]][img_op], mid, centre)
                sp = {"n": len(kept), "apart": apart, "median": round(median(kept), 3),
                      "min": round(min(kept), 3), "max": round(max(kept), 3)}
                tpid = int(re.search(r"tp_(\d+)_", fx["ops"][0]["tp_glob"]).group(1))
                cell = json.load(open(os.path.join(a["simart"], "summary.json")))["simulation_resolution"]["mm"]
                ld = load_share(a["simart"], tpid, cell)
                eng = normal_step_mod.engaged_moves(a["simart"], tpid)
                fxn["arms"][a["label"]]["spacing"] = sp
                fxn["arms"][a["label"]]["normal_step"] = normal_step_mod.normal_step(ops[a["label"]][img_op], mid, centre, eng)
                fxn["arms"][a["label"]]["load"] = ld
                fxn["arms"][a["label"]]["producers"] = producer_split_mod.split(a["dir"], a["simart"], fx["ops"][0]["tp_glob"])
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

    def spacing_html(fx):
        fxn = numbers["fixtures"][fx["name"]]
        if "mid_level" not in fxn:
            return ""
        rows, old_rows = "", ""

        def pr_text(pr):
            per = "—" if pr["perimeter"] is None else f"{pr['perimeter']:.0f}"
            return f"{per} / {pr['body']:.0f} / {pr['waterline']:.0f} mm"
        for lab, an in fxn["arms"].items():
            ns, sp, ld = an["normal_step"], an["spacing"], an["load"]
            rows += (f"<tr><td>{html.escape(lab)}</td><td>{ns['median']:.2f} mm</td>"
                     f"<td>{ns['p10']:.2f} to {ns['p90']:.2f} mm (n {ns['n']}; {ns['apart']} over {TOOL_D:.0f} mm set apart)</td>"
                     f"<td>{ld['over_exact_pct']:.2f}%</td><td>{ld['over_screen_pct']:.2f}% (screen {ld['screen']:.3f})</td>"
                     f"<td>{pr_text(an['producers'])}</td></tr>")
            old_rows += (f"<tr><td>{html.escape(lab)}</td><td>{sp['median']:.2f} mm</td>"
                         f"<td>{sp['min']:.2f} to {sp['max']:.2f} mm (n {sp['n']}; {sp['apart']} gaps over {TOOL_D:.0f} mm set apart)</td></tr>")
        ag = fxn["arms"]["AgentSearch"]
        gz = fxn["arms"]["geodesic_local_zigzag"]
        cp = fxn["arms"]["contour_parallel"]
        f0 = fx["ops"][0]["comment"]
        s1 = (f"Against the main control: contour_parallel puts its passes {cp['normal_step']['median']:.2f} mm apart and the geodesic zigzag "
              f"{gz['normal_step']['median']:.2f} mm (stepover {fx['stepover_first_op']:.1f} mm); the geodesic zigzag cuts "
              f"{gz['load']['cut_mm'] / cp['load']['cut_mm']:.2f} times the contour_parallel length on '{f0}', and "
              f"{gz['load']['over_exact_pct']:.2f}% of it reads over 0.471 against {cp['load']['over_exact_pct']:.2f}% for contour_parallel.")
        over = ag["load"]["over_exact_pct"]
        s2 = (f"AgentSearch (secondary) is over the bound: {over:.2f}% of its cut length on '{f0}' reads over 0.471 in the simulation "
              f"({ag['load']['over_screen_pct']:.2f}% over the screen {ag['load']['screen']:.3f} for its sim cell)."
              if over > 0 else f"AgentSearch (secondary) is not over the bound: 0% of its cut length on '{f0}' reads over 0.471.")
        why = ("The rays read low because they start at the seed and over-sample the levels inside the radius where the law spaces "
               "levels closer than the stepover (25.45 mm at stepover 1.2, 22.7 mm at 2.0; P2_C2 section C24), and because they also "
               "cross the starter helix and, on one-way, the straight moves across the cut.")
        return (f'<h3 style="margin-top:12px">Pass spacing and load at {html.escape(fxn["mid_level"])} ({html.escape(f0)})</h3>'
                f'<table><tr><th>arm</th><th>pass step along the normal, length-weighted median</th><th>10th to 90th percentile</th>'
                f'<th>cut length over 0.471</th><th>over the C18 screen for this sim cell</th>'
                f'<th>over 0.471 by producer: perimeter / body / waterline</th></tr>{rows}</table>'
                f'<p>{html.escape(s1)} {html.escape(s2)}</p>'
                f'<p class="note">Spacing (C24\'s measure): the step along the normal between adjacent passes. From the midpoint of each '
                f'pass move the normal is cast both ways; the distance to the nearest other pass move is one sample, weighted by the move length. '
                f'A pass is a cut move of the level that the closing simulation reads over one sim cell / D (so a straight move across the cut, '
                f'which runs through cleared stock, is not a pass); moves within R + 0.05 mm of the starter centre '
                f'({fxn["centre"][0]}, {fxn["centre"][1]}) are left out (the starter helix); a step over one tool diameter is set apart. '
                f'Producer split (C30, phase3/c30): perimeter sweep (agent-slice arms only; contour_parallel has none, so —), region body, waterline cleanup, by the trace annotations. Load: the closing simulation of the --sim-artifact run (byte-equal program.nc), the radial width of cut of each cutting sample whose intent is a cut (clearing_cut, of any kinematics; entry and link samples left out), each '
                f'weighted by its XY length. 0.471 is the C18 exact bound for stepover 2.0 on a 3 mm radius (P3_C18_3d_oracle.md); '
                f'at the Corne stepover of 1.2 the ceiling is lower, so 0.471 under-counts there. The screen adds the sim cell '
                f'(Corne 0.3 mm, wanaka200 1.0 mm), because the simulation blends partly covered cells.</p>'
                f'<details><summary>The first measure (4 rays from the seed), kept for reference</summary>'
                f'<table><tr><th>arm</th><th>ray spacing, median</th><th>range</th></tr>{old_rows}</table>'
                f'<p>{html.escape(why)}</p></details>')

    body = ""
    for fx, pick, cols, missing in rows:
        miss = "".join(f'<p class="note">{html.escape(m["label"])}: not run ({html.escape(m.get("why", "no output"))}).</p>' for m in missing)
        body += (f'<section><h2>{html.escape(fx["title"])}</h2><p class="note">{html.escape(fx["note"])} '
                 f'Levels shown (the same for every column): {", ".join(map(html.escape, pick))}.</p>{miss}'
                 f'<div class="grid" style="grid-template-columns:repeat({max(len(cols),1)},minmax(0,1fr))">'
                 + "".join(col_html(fx, a, an, imgs) for a, an, imgs in cols) + "</div>" + spacing_html(fx) + "</section>")
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
