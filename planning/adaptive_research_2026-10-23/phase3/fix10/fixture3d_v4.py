"""C18/fix10: build fixture3d_v4.html ("Geodesic roughing G2"), the corrected G2 page.

Copied from phase3/fix8/fixture3d_v3.py (C35). Changes from v3:
- the lead's reading is phase3/fix9/lead_reading_v4.html, verbatim;
- section 4 of each part table has a row for the lift-over links that go
  down into stock (phase3/fix9/entries.json);
- the coverage row gives the area inside the part outline and the outer
  band (phase3/fix9 stock rebuild) beside the CLI triage;
- the images are the fix10 renders (render_v4.py, from the fix9 renders):
  one strict plane per image, the drape band, a 10 mm crop, the stock map
  and the Corne D18-vs-D20 Z 17 comparison. They follow the simulation
  samples, so an arc is drawn as an arc, not as its chord.

Run: python3 -I render_v4.py && python3 -I fixture3d_v4.py spec_v4.json
Writes ../../fixture3d_v4.html and fixture3d_v4_numbers.json, which
assert_fixture3d_v4.py re-derives from the raw files.
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

HERE = os.path.dirname(os.path.abspath(__file__))
FIX2 = os.path.join(HERE, "..", "fix2")

_ps_spec = _ilu.spec_from_file_location("producer_split", os.path.join(FIX2, "producer_split.py"))
producer_split_mod = _ilu.module_from_spec(_ps_spec)
_ps_spec.loader.exec_module(producer_split_mod)
_tm_spec = _ilu.spec_from_file_location("thick_measure", os.path.join(FIX2, "thick_measure.py"))
thick_measure_mod = _ilu.module_from_spec(_tm_spec)
_tm_spec.loader.exec_module(thick_measure_mod)
_THICK_CACHE = os.path.join(HERE, "thick_cache.json")


def thick_cached(run_dir, simart_dir, tp_glob):
    """C32's thick-stock measure, cached by the simulation file's size and
    mtime (it takes seconds to a minute per column)."""
    sim = os.path.join(simart_dir, "simulation.json")
    key = f"{os.path.abspath(simart_dir)}|{tp_glob}|{os.path.getsize(sim)}|{os.path.getmtime(sim)}"
    cache = json.load(open(_THICK_CACHE)) if os.path.exists(_THICK_CACHE) else {}
    if key not in cache:
        cache[key] = thick_measure_mod.thick_over(run_dir, simart_dir, tp_glob)
        json.dump(cache, open(_THICK_CACHE, "w"), indent=1)
    return cache[key]


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


def load_share_old(simart_dir, toolpath_id):
    """The load rule before D-FIX6 (kept for the side-by-side table): every
    fed cutting sample with linear or arc kinematics, any intent."""
    prev, tot, o1 = None, 0.0, 0.0
    for sm in sim_samples(simart_dir):
        p = sm["position"]
        ln = math.hypot(p[0] - prev[0], p[1] - prev[1]) if prev else 0.0
        prev = p
        if sm["toolpath_id"] != toolpath_id or not sm["is_cutting"] or sm["cut_kinematics"] not in ("linear", "arc"):
            continue
        tot += ln
        o1 += ln * (sm["engagement"]["radial_woc_fraction"] > C18_EXACT)
    return {"cut_mm": round(tot, 1), "over_exact_pct": round(100 * o1 / tot, 2)}


def findings(run_dir):
    err = open(os.path.join(run_dir, "stderr.txt")).read()
    out = [ln.strip() for ln in err.splitlines() if "island(s) standing" in ln]
    m = re.search(r"Act on \(\d+\):\n((?:  .*\n)+)", err)
    if m:
        out += [ln.strip() for ln in m.group(1).splitlines()]
    return out




# ---------------------------------------------------------------- v3 parts

ISL_HEAD = re.compile(r'"([^"]+)": the rough leaves (\d+) island\(s\) standing.*?(\d+) of them against a wall: (\d+\.\d+) mm2')
BAR_WORDS = ("gouge", "Gouge", "deeper than", "fed link cuts")


def load_v3(simart_dir, toolpath_id, sim_cell, tol=0.1, planner_cell=0.5, d=TOOL_D):
    """load_share (D-FIX6 rule) with the over lengths in mm as well."""
    screen = C18_CEILING + (planner_cell + sim_cell + tol) / d
    prev, tot, o1, o2 = None, 0.0, 0.0, 0.0
    for sm in sim_samples(simart_dir):
        p = sm["position"]
        ln = math.hypot(p[0] - prev[0], p[1] - prev[1]) if prev else 0.0
        prev = p
        if sm["toolpath_id"] != toolpath_id or not sm["is_cutting"] or sm["source_intent"] != "clearing_cut":
            continue
        r = sm["engagement"]["radial_woc_fraction"]
        tot += ln
        o1 += ln * (r > C18_EXACT)
        o2 += ln * (r > screen)
    return {"cut_mm": round(tot, 1), "over_mm": round(o1, 1), "over_pct": round(100 * o1 / tot, 2),
            "screen": round(screen, 4), "over_screen_mm": round(o2, 1), "over_screen_pct": round(100 * o2 / tot, 2)}


def bars(run_dir):
    """The four hard bars, each from its instrument's output file."""
    s = json.load(open(os.path.join(run_dir, "summary.json")))
    err = open(os.path.join(run_dir, "stderr.txt")).read()
    out = re.sub(r"\x1b\[[0-9;]*m", "", open(os.path.join(run_dir, "stdout.txt")).read())
    col = {"feed": s["collision_count"], "rapid": s["rapid_collision_count"], "failed_checks": s["collision_checks_failed"]}
    cov = [{"op": m.group(1), "count": int(m.group(2)), "at_wall": int(m.group(3)), "area_mm2": float(m.group(4))}
           for m in ISL_HEAD.finditer(err)]
    gouge = [ln.strip() for ln in err.splitlines() if any(w in ln for w in BAR_WORDS)]
    gen = re.search(r"generation plan finished steps=(\d+) simulations=(\d+) blocked=(\d+)", out)
    ops_ok = all(t["move_count"] > 0 for t in s["per_toolpath"])
    return {
        "collisions": col, "collisions_pass": all(v == 0 for v in col.values()),
        "coverage": cov, "coverage_pass": not cov,
        "gouge_lines": len(gouge), "gouge_pass": not gouge,
        "generation": {"steps": int(gen.group(1)), "blocked": int(gen.group(3)), "ops_with_moves": ops_ok},
        "generation_pass": gen is not None and int(gen.group(3)) == 0 and ops_ok,
        "export_refused": s["gcode_export"]["refused"], "machine_safety_errors": s["gcode_export"]["machine_safety_errors"],
    }


def openings(run_dir):
    out = re.sub(r"\x1b\[[0-9;]*m", "", open(os.path.join(run_dir, "stdout.txt")).read())
    res = []
    for m in re.finditer(r"C31 region opening z=([\d.]+) region=(\d+) .*?decision=\"([^\"]+)\"", out):
        res.append({"z": round(float(m.group(1)), 2), "region": int(m.group(2)), "decision": m.group(3)})
    return res


def islands_diff():
    """D20b (phase3/fix7/d20b_islands.py rule): islands of one run with no
    island of the other within one cell (0.3 mm) + 0.05 mm in X, Y and height."""
    d18 = json.load(open(f"{RES}/phase3/fix7/islands_d18.json"))
    d20 = json.load(open(f"{RES}/phase3/fix7/islands_d20.json"))

    def unmatched(a, b):
        return [i for i in a["islands"] if not any(
            abs(i["x"] - j["x"]) <= 0.35 and abs(i["y"] - j["y"]) <= 0.35 and abs(i["height_mm"] - j["height_mm"]) <= 0.35
            for j in b["islands"])]
    r1, cp = "3D Rough 1", "3D Rough 1 (copy)"
    new = max(unmatched(d20[r1], d18[r1]), key=lambda i: i["area_mm2"])
    gone = max([i for i in unmatched(d18[r1], d20[r1]) if not i["at_wall"]], key=lambda i: i["area_mm2"])
    # Copy op: a D20 island that the XY rule calls new but that has a D18
    # island of equal height, width and area is the same island with its
    # peak point moved (D20b).
    key = lambda i: (i["height_mm"], i["width_mm"], i["area_mm2"])
    cp_new = unmatched(d20[cp], d18[cp])
    twins = [(i, next(j for j in d18[cp]["islands"] if key(j) == key(i))) for i in cp_new
             if any(key(j) == key(i) for j in d18[cp]["islands"])]
    moved_new, moved_old = max(twins, key=lambda t: t[0]["area_mm2"])
    others = [i for i in cp_new if i is not moved_new]
    return {"new_r1": new, "gone_r1": gone, "copy_moved_d20": moved_new, "copy_moved_d18": moved_old,
            "copy_other_new_count": len(others), "copy_other_new_max_mm2": max(i["area_mm2"] for i in others)}


def c33_table():
    """The C33 cause table (phase3/c33/c33_output.txt) and the layer line (c33_layer.txt)."""
    txt = open(f"{RES}/phase3/c33/c33_output.txt").read()
    blk = txt.split("by cause:")[1].split("\n\n")[0].splitlines()[1:]
    rows = []
    for ln in blk:
        m = re.match(r"\s+(.+?)\s+([\d.]+)\s+([\d.]+)\s+([\d.]+)\s+([\d.]+)\s+sum\s+([\d.]+)$", ln)
        rows.append({"cause": m.group(1), "z17": float(m.group(2)), "z11": float(m.group(3)), "z5": float(m.group(4)),
                     "z05": float(m.group(5)), "sum": float(m.group(6))})
    lay = re.search(r"median ([\d.]+) mm, p10 ([\d.]+), p90 ([\d.]+)", open(f"{RES}/phase3/c33/c33_layer.txt").read())
    return {"rows": rows, "layer_median": float(lay.group(1)), "layer_p10": float(lay.group(2)), "layer_p90": float(lay.group(3))}


KNOWN = [
    ("G-ARCENDHOOK", "DEFECTS.md", "On the far-side geodesic modes (geodesic_curv, geodesic_curv_split), some arcs end with a short sideways hop into standing stock, then a lift. Research arms only. The zigzag_pick arm of this page is not one of these modes."),
    ("G-RINGGROW", "DEFECTS.md", "The C31 grown outline fills the hole of a thin frame ring. On wanaka200 at Z 4.0 the closing guard refuses region 1, and the region keeps the shipped sweep (see the openings row)."),
    ("G-PICKEXIT", "DEFECTS.md", "The C29 pick does not score the first entry after a region body. The pick can be wrong only when the two plans are within one plunge per region."),
    ("C33 outline-wrap load", "P3_C18_3d_oracle.md section C33", "After C31, the thick over-load of the Corne body is on the outermost wrap of the grown outline, at the outer wall (the cause table below)."),
    ("C31 part 2 (waterline skim)", "P3_C22_3d_turns.md section 11.6", "Parked. Not built. It waits for the measurement of part 1."),
    ("Flat-floor design (H1 guard)", "P3_C22_3d_turns.md section 11.11", "Design on paper: grow the 2D plan from the level's flat floor only, and leave the drape ring to the waterline cleanup. Not built."),
]


FIX9 = os.path.join(HERE, "..", "fix9")
RNUM = json.load(open(os.path.join(HERE, "render_v4_numbers.json")))
CROPS = json.load(open(os.path.join(HERE, "crop_centres.json")))
ENTRIES = json.load(open(os.path.join(FIX9, "entries.json")))


def lift_over(fx, label):
    """fix9 section 4: keep-down-link spans whose entry_plunge samples drop
    1 mm or more and remove stock (> 0.1 mm3)."""
    return sum(1 for r in ENTRIES[f"{fx} {label}"]["rows"]
               if r["style"] == "keep-down link" and r["descent_mm"] >= 1.0 and r["removed_mm3"] > 0.1)


def coverage_rebuild(fx, label):
    """fix9 section 1: the area more than 0.8 mm above the target after the
    first rough op (the rebuilt stock, fix9/stock_*.npz): inside the part
    outline, and (Corne) outside it but inside the part's bounding box."""
    import numpy as np
    from scipy import ndimage
    m = np.load(os.path.join(FIX9, f"stock_{fx}_{re.sub(r'[^A-Za-z0-9]+', '_', label).strip('_')}.npz"))
    a = m["top"] - m["target"]; g = float(m["g"]); X, Y = np.meshgrid(m["xs"], m["ys"])
    if fx == "corne":
        sil = ndimage.binary_fill_holes(m["surf"] > 0)
        box = (X >= 5.314) & (X <= 148.074) & (Y >= -104.316) & (Y <= -3.121)
        return {"inside_mm2": round(float(((a > 0.8) & sil).sum() * g * g), 1),
                "outer_band_mm2": round(float(((a > 0.8) & box & ~sil).sum() * g * g), 1)}
    fp = (X >= 23) & (X <= 217) & (Y >= 28) & (Y <= 222)
    return {"inside_mm2": round(float(((a > 0.8) & fp).sum() * g * g), 1), "outer_band_mm2": None}


def main():
    spec = json.load(open(sys.argv[1]))
    numbers = {"fixtures": {}, "islands_diff": islands_diff(), "c33": c33_table()}
    sections = []
    for fx in spec["fixtures"]:
        arms = fx["arms"]
        ops = {a["label"]: [load_op(a["dir"], o["tp_glob"], o["comment"], a["simart"]) for o in fx["ops"]] for a in arms}
        common = None
        for a in arms:
            d0 = ops[a["label"]][0]
            labs = [lab for lab in d0["levels"] if any(c == "cut" and l == lab for c, l in zip(d0["cls"], d0["lev"]))]
            common = labs if common is None else [x for x in common if x in labs]
        common = common or []
        pick = [common[0], common[len(common) // 2], common[-1]] if len(common) >= 3 else common
        pts = [p for a in arms for (g, p0, p) in ops[a["label"]][0]["moves"] if g != 0]
        xs, ys = [p[0] for p in pts], [p[1] for p in pts]
        frame = (min(xs) - 3.0, max(xs) + 3.0, min(ys) - 3.0, max(ys) + 3.0)
        tpid = int(re.search(r"tp_(\d+)_", fx["ops"][0]["tp_glob"]).group(1))
        fxn = {"levels_shown": pick, "arms": {}}
        cols = []
        for a in arms:
            s = json.load(open(os.path.join(a["dir"], "summary.json")))
            cell = json.load(open(os.path.join(a["simart"], "summary.json")))["simulation_resolution"]["mm"]
            an = {"commit": a["commit"], "role": a["role"], "cycle_time_s": round(s["total_runtime_s"]),
                  "cutting_mm": round(s["total_cutting_distance_mm"]), "rapid_mm": round(s["total_rapid_distance_mm"]),
                  "air_cut_pct": round(s["air_cut_pct_of_total_runtime"], 1), "sim_cell_mm": cell,
                  "ops": {o["comment"]: op_numbers(d) for o, d in zip(fx["ops"], ops[a["label"]])},
                  "bars": bars(a["dir"]), "images": {}}
            an["load"] = load_v3(a["simart"], tpid, cell)
            an["producers"] = producer_split_mod.split(a["dir"], a["simart"], fx["ops"][0]["tp_glob"])
            th = thick_cached(a["dir"], a["simart"], fx["ops"][0]["tp_glob"])
            th["pct"] = round(100 * th["total_mm"] / an["load"]["cut_mm"], 2)
            an["thick"] = th
            if a.get("openings"):
                an["openings"] = openings(a["dir"])
                an["opening_tags"] = a.get("opening_tags", {})
            an["lift_over"] = lift_over(fx["name"], a["label"])
            an["coverage_rebuild"] = coverage_rebuild(fx["name"], a["label"])
            t = re.sub(r"[^A-Za-z0-9]+", "_", a["label"]).strip("_")
            imgs = []
            for k in range(3):
                n = RNUM[f"plane_{fx['name']}_{t}_{k}"]
                an["images"][f"plane {k}"] = n
                imgs.append((f"phase3/fix10/img/plane_{fx['name']}_{t}_{k}.png",
                             f"Strict plane Z {n['z']:.2f}: {n['cut_mm']:.1f} mm of cut, {n['plunge_entries']} plunge entries (red rings), "
                             f"{n['lift_over']} lift-over descents into stock (orange dots)."))
            n = RNUM[f"drape_{fx['name']}_{t}"]
            an["images"]["drape"] = n
            imgs.append((f"phase3/fix10/img/drape_{fx['name']}_{t}.png",
                         f"Drape band: {n['cut_mm']:.1f} mm of cut steps that lie on no plane, coloured by Z."))
            cc = CROPS[fx["name"]]
            imgs.append((f"phase3/fix10/img/crop_{fx['name']}_{t}.png",
                         f"10 mm crop at ({cc['x']:.1f}, {cc['y']:.1f}), cut steps from Z {cc['plane']:.2f} up one depth per pass."))
            fxn["arms"][a["label"]] = an
            cols.append((a, an, imgs))
        numbers["fixtures"][fx["name"]] = fxn
        sections.append((fx, pick, cols))
    json.dump(numbers, open(f"{HERE}/fixture3d_v4_numbers.json", "w"), indent=1)
    open(f"{RES}/fixture3d_v4.html", "w").write(render(spec, sections, numbers))
    print("page written;", json.dumps({k: v["levels_shown"] for k, v in numbers["fixtures"].items()}))


def pf(ok, fail_word="FAIL"):
    return '<b class="pass">PASS</b>' if ok else f'<b class="fail">{fail_word}</b>'


def cell_bars(an):
    b = an["bars"]
    c = b["collisions"]
    cov = "; ".join(f"{html.escape(x['op'])}: {x['count']} islands ({x['at_wall']} at a wall), {x['area_mm2']:.1f} mm²" for x in b["coverage"])
    g = b["generation"]
    return {
        "collisions": f"{pf(b['collisions_pass'])} {c['feed']} feed / {c['rapid']} rapid / {c['failed_checks']} failed checks",
        "coverage": (f"triage: {pf(b['coverage_pass'], 'FLAGGED')} {cov}" if cov else "triage: no island line")
                    + f"<br>rebuild after the first rough op: {an['coverage_rebuild']['inside_mm2']:.1f} mm² inside the part"
                    + (f", {an['coverage_rebuild']['outer_band_mm2']:.1f} mm² outer band" if an['coverage_rebuild']['outer_band_mm2'] is not None else ""),
        "gouge": f"{pf(b['gouge_pass'])} {b['gouge_lines']} line(s)",
        "generation": f"{pf(b['generation_pass'])} {g['steps']} steps, {g['blocked']} blocked",
    }


def render(spec, sections, numbers):
    # The lead's reading, verbatim (phase3/fix9/lead_reading_v4.html).
    lead = open(os.path.join(FIX9, "lead_reading_v4.html")).read()
    body = ""
    for fx, pick, cols in sections:
        grid = ""
        for a, an, imgs in cols:
            trs = "".join(f"<tr><td>{html.escape(op)}</td><td>{v['entries']}</td><td>{v['keepdown_links']}</td>"
                          f"<td>{v['keepdown_mm']:.0f}</td><td>{v['cut_xy_mm']:.0f}</td></tr>" for op, v in an["ops"].items())
            figs = "".join(f'<figure><a href="{r}"><img src="{r}" loading="lazy" alt="{html.escape(a["label"])}: {html.escape(sent)}"></a>'
                           f'<figcaption>{html.escape(sent)}</figcaption></figure>' for r, sent in imgs)
            grid += (f'<div class="col"><h3>{html.escape(a["label"])} <span class="commit">({html.escape(an["role"])}; {an["commit"]})</span></h3>'
                     f'<div class="imgs">{figs}</div>'
                     f'<table><tr><th>op</th><th>plunges</th><th>keep-down links</th><th>keep-down mm</th><th>cut mm</th></tr>{trs}</table></div>')
        labs = [a["label"] for a, an, imgs in cols]
        ans = [an for a, an, imgs in cols]
        f0 = fx["ops"][0]["comment"]

        def row(name, cells, cls=""):
            return f'<tr class="{cls}"><th>{name}</th>' + "".join(f"<td>{c}</td>" for c in cells) + "</tr>"
        cb = [cell_bars(an) for an in ans]
        t = row("", [f"{html.escape(l)}<br><span class=\"commit\">{html.escape(an['role'])}</span>" for l, an in zip(labs, ans)], "head")
        t += '<tr class="grp"><th colspan="5">1. Hard bars (pass / fail, with the instrument)</th></tr>'
        t += row("Collisions<br><span class=\"commit\">closing simulation, summary.json</span>", [c["collisions"] for c in cb])
        t += row("Coverage: standing islands<br><span class=\"commit\">CLI coverage triage, stderr.txt</span>", [c["coverage"] for c in cb])
        t += row("Gouge and stock crossing<br><span class=\"commit\">CLI stderr lines: gouge, deeper than commanded, fed link cuts stock</span>", [c["gouge"] for c in cb])
        t += row("Generation<br><span class=\"commit\">CLI stdout: generation plan, blocked steps</span>", [c["generation"] for c in cb])
        t += f'<tr class="grp"><th colspan="5">2. Load held, first rough op \'{html.escape(f0)}\'</th></tr>'
        t += row("Over the exact bound 0.471", [f"{an['load']['over_mm']:.1f} mm = {an['load']['over_pct']:.2f}% of {an['load']['cut_mm']:.1f} mm cut" for an in ans])
        t += row("Over 0.471 by producer: perimeter / body / waterline",
                 ["{} / {:.1f} / {:.1f} mm".format("—" if an["producers"]["perimeter"] is None else f"{an['producers']['perimeter']:.1f}",
                                                   an["producers"]["body"], an["producers"]["waterline"]) for an in ans])
        t += row("Thick: over 0.471 in stock at least one planner cell (0.5 mm) thick",
                 ["{:.1f} mm = {:.2f}% ({} / {:.1f} / {:.1f} mm)".format(an["thick"]["total_mm"], an["thick"]["pct"],
                  "—" if an["thick"]["perimeter"] is None else f"{an['thick']['perimeter']:.1f}", an["thick"]["body"], an["thick"]["waterline"]) for an in ans])
        t += row("Over the sim-cell screen", [f"{an['load']['over_screen_mm']:.1f} mm = {an['load']['over_screen_pct']:.2f}% (screen {an['load']['screen']:.3f})" for an in ans])
        t += '<tr class="grp"><th colspan="5">3. C31 region openings</th></tr>'
        op_cells = []
        for an in ans:
            if "openings" not in an:
                op_cells.append("—")
                continue
            grown = sum(1 for o in an["openings"] if o["decision"] == "grown, no sweep")
            tags = an.get("opening_tags", {})
            items = "<br>".join(f"Z {o['z']:.2f} region {o['region']}: {html.escape(o['decision'])}"
                                + (f" ({html.escape(tags[f'{o['z']:.2f}/{o['region']}'])})" if f"{o['z']:.2f}/{o['region']}" in tags else "")
                                for o in an["openings"])
            op_cells.append(f"{grown} of {len(an['openings'])} grown<br>{items}")
        t += row("Decision per planned region<br><span class=\"commit\">stdout.txt, 'C31 region opening'</span>", op_cells)
        t += '<tr class="grp"><th colspan="5">4. Entries, travel and time</th></tr>'
        t += row(f"Plunges ('{html.escape(f0)}')", [f"{an['ops'][f0]['entries']}" for an in ans])
        t += row(f"Lift-over links that go down into stock (≥ 1 mm, removes stock) ('{html.escape(f0)}')<br><span class=\"commit\">fix9 section 4, entries.json</span>", [f"{an['lift_over']}" for an in ans])
        t += row(f"Keep-down links ('{html.escape(f0)}')", [f"{an['ops'][f0]['keepdown_links']}, {an['ops'][f0]['keepdown_mm']:.0f} mm" for an in ans])
        t += row("Air cut", [f"{an['air_cut_pct']}% of runtime" for an in ans])
        t += row("Cut length (rough ops)", [f"{an['cutting_mm']} mm" for an in ans])
        t += row("Cycle time (rough ops)", [f"{an['cycle_time_s']} s" for an in ans])
        extra = ""
        if fx["name"] == "corne":
            dd = numbers["islands_diff"]
            n, g, c18, c20 = dd["new_r1"], dd["gone_r1"], dd["copy_moved_d18"], dd["copy_moved_d20"]
            extra += ('<h3>Corne coverage, D18 against D20 (D20b: the full island lists, '
                      'phase3/fix7/islands_d18.json and islands_d20.json)</h3><ul>'
                      f"<li>After '3D Rough 1', D20 adds one real wall island: {n['height_mm']:.1f} mm high, "
                      f"{n['area_mm2']:.1f} mm² at ({n['x']:.1f}, {n['y']:.1f}).</li>"
                      f"<li>The D18 free island ({g['height_mm']:.1f} mm high, {g['area_mm2']:.1f} mm² at ({g['x']:.1f}, {g['y']:.1f})) is not in D20.</li>"
                      f"<li>After the second rough op: the {c20['height_mm']:.2f} mm high, {c20['area_mm2']:.1f} mm² wall island is in both runs "
                      f"(equal height, width and area); only its peak point moved, from ({c18['x']:.1f}, {c18['y']:.1f}) to ({c20['x']:.1f}, {c20['y']:.1f}). "
                      f"So the new rough-1 island does not stand as a separate island after the second rough op. "
                      f"The other {dd['copy_other_new_count']} islands that the match rule calls new in D20 are each {dd['copy_other_new_max_mm2']:.1f} mm² or less.</li></ul>")
            c = numbers["c33"]
            trs = "".join(f"<tr><td>{html.escape(r['cause'])}</td><td>{r['z17']:.1f}</td><td>{r['z11']:.1f}</td><td>{r['z5']:.1f}</td>"
                          f"<td>{r['z05']:.1f}</td><td>{r['sum']:.1f}</td></tr>" for r in c["rows"])
            extra += ("<h3>D20 Corne: the thick body over-load by cause (C33, phase3/c33/c33_output.txt)</h3>"
                      "<table><tr><th>cause</th><th>Z 17</th><th>Z 11</th><th>Z 5</th><th>Z 0.5</th><th>sum (mm)</th></tr>"
                      f"{trs}</table><p class=\"note\">The layer above the tip on the Z 17 step: median {c['layer_median']:.2f} mm "
                      f"(p10 {c['layer_p10']:.2f}, p90 {c['layer_p90']:.2f}; phase3/c33/c33_layer.txt).</p>")
        body += (f'<section><h2>{html.escape(fx["title"])}</h2><p class="note">{html.escape(fx["note"])} '
                 f'Strict planes shown (the same for every column): {", ".join(f"Z {RNUM[f'plane_{fx["name"]}_contour_parallel_{k}']['z']:.2f}" for k in range(3))}.'
                 + (" On wanaka200 most of the cut drapes over the terrain, so the strict planes hold little of it; the drape image shows the rest. The bottom level Z 4.00 holds no strict-plane cut." if fx["name"] == "wanaka200" else "") + '</p>'
                 f'<div class="tablewrap"><table class="cmp">{t}</table></div>{extra}'
                 f'<h3>Stock after the first rough op (fix9 rebuild)</h3>'
                 f'<figure><a href="phase3/fix10/img/stockmap_{fx["name"]}.png"><img class="wide" src="phase3/fix10/img/stockmap_{fx["name"]}.png" loading="lazy" '
                 f'alt="Stock above the target after the first rough op, four columns"></a><figcaption>The stock height above the target '
                 f'(the floor the tool can reach plus the axial leave), from 0 to one depth per pass; grey is outside the part. '
                 f'Cyan rings: the islands of the coverage triage.{" Green: the D20 seed and the 25.45 mm radius of the C24 spacing law." if fx["name"] == "corne" else ""}</figcaption></figure>'
                 + ('<h3>Corne, strict plane Z 17: D18 (the run of the v2 page\'s zigzag_pick column) against D20</h3>'
                    '<figure><a href="phase3/fix10/img/compare_corne_D18_D20_Z17.png"><img class="wide" src="phase3/fix10/img/compare_corne_D18_D20_Z17.png" loading="lazy" '
                    'alt="Corne Z 17, D18 and D20 side by side"></a><figcaption>Same frame and scale. Dashed: the 25.45 mm radius of the C24 '
                    'spacing law; dotted: 50 mm.</figcaption></figure>' if fx["name"] == "corne" else "")
                 + f'<h3>Per column: three strict planes, the drape band and a 10 mm crop (click an image for full size)</h3>'
                 f'<div class="grid">{grid}</div></section>')
    known = "".join(f"<li><b>{html.escape(tag)}</b> ({html.escape(src)}): {html.escape(txt)}</li>" for tag, src, txt in KNOWN)
    method = ("Load: the closing simulation of each run. A sample counts when its intent is a cut (clearing_cut, of any kinematics); "
              "its weight is its XY step. 0.471 is the C18 exact bound for stepover 2.0 on a 3 mm radius; at the Corne stepover of 1.2 the "
              "ceiling is lower, so 0.471 under-counts there. The screen adds the sim cell (Corne 0.3 mm, wanaka200 1.0 mm). "
              "Thick (C32): the exact swept width again, with only stock at least 0.5 mm above the tool tip. "
              "Producers (C30): the trace annotations. 'Perimeter' is the span from a level start to the level's second entry. "
              "Where C31 cuts no sweep (D20), that span is the start of the body; on wanaka200 at Z 4.0 the D20 regions keep the shipped sweep. "
              "Plunges: the plunge-entry spans of the trace. Keep-down links: runs of link moves.")
    return f'''<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>{html.escape(spec["title"])}</title><style>
:root{{--bg:#fff;--fg:#1a1a1a;--muted:#666;--line:#ddd;--card:#fafafa;--pass:#1b7f3a;--fail:#b3261e;--grp:#eef1f5}}
@media (prefers-color-scheme:dark){{:root:not([data-theme="light"]){{--bg:#16181c;--fg:#e8e8e8;--muted:#a0a0a0;--line:#333;--card:#1e2126;--pass:#5cc97d;--fail:#ff8a80;--grp:#23272e}}}}
:root[data-theme="dark"]{{--bg:#16181c;--fg:#e8e8e8;--muted:#a0a0a0;--line:#333;--card:#1e2126;--pass:#5cc97d;--fail:#ff8a80;--grp:#23272e}}
body{{background:var(--bg);color:var(--fg);font:14px/1.45 system-ui,sans-serif;margin:0;padding:16px;max-width:1500px}}
h1{{font-size:20px;margin:0 0 6px}} h2{{font-size:17px;margin:22px 0 4px}} h3{{font-size:14px;margin:12px 0 6px}}
.note,.meta{{color:var(--muted);font-size:13px}} .commit{{color:var(--muted);font-weight:normal;font-size:12px}}
.pass{{color:var(--pass)}} .fail{{color:var(--fail)}}
.tablewrap{{overflow-x:auto}} table{{border-collapse:collapse;width:100%;font-size:12.5px;margin-top:6px}}
td,th{{border:1px solid var(--line);padding:4px 6px;text-align:left;vertical-align:top}}
table.cmp th{{width:18%;font-weight:600}} tr.grp th{{background:var(--grp)}} tr.head td{{font-weight:600}}
.grid{{display:grid;grid-template-columns:repeat(4,minmax(0,1fr));gap:12px;margin-top:12px}} @media (max-width:1100px){{.grid{{grid-template-columns:1fr}}}}
.col{{background:var(--card);border:1px solid var(--line);border-radius:8px;padding:10px;min-width:0}}
.imgs{{display:grid;grid-template-columns:1fr;gap:6px}}
figure{{margin:0 0 8px}} img{{width:100%;height:auto;background:#fff;border:1px solid var(--line)}} img.wide{{max-width:1540px}} figcaption{{font-size:12px;margin-top:2px}}
#lead-reading{{border:2px solid var(--line);border-radius:8px;padding:10px;margin:12px 0;min-height:2em}}
.key span{{display:inline-block;margin-right:12px}}
</style></head><body>
<h1>{html.escape(spec["title"])}</h1>
<div class="meta">{html.escape(spec["header"])}</div>
<div id="lead-reading">{lead}</div>
<p class="note">{html.escape(method)}</p>
<p class="key"><span style="color:{BLUE}">&#9632; blue: cutting moves on the plane</span><span style="color:{ORANGE}">&#9632; orange line: a link at depth</span><span style="color:{RED}">&#9675; red ring: a plunge entry</span><span style="color:{ORANGE}">&#9679; orange dot: a lift-over link that goes down into stock</span></p>
<p class="note">Arcs: the v2 and v3 pages drew each G2/G3 arc as its chord, so they showed a sparse ring on Corne that is not in the path. This page draws the path from the simulation samples (one per 0.3 mm on Corne, 1.0 mm on wanaka200), so each arc is drawn as an arc.</p>
{body}
<h2>Known items</h2><ul>{known}</ul>
</body></html>'''


if __name__ == "__main__":
    main()
