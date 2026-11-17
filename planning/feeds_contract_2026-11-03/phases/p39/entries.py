"""P39 instrument: each ramp entry of one 3D Rough on the replay of the emitted G-code.

The replay is the replay of `p37/gapmap.py` (nodes of 0.2 mm, G-code frame). This script adds:

- the entries of one op: the `entry` items of the semantic trace of the op (`tp_<id>_*.json` of
  the CLI run; move `m` of the op is G-code motion line `m + 2`, because the post writes two
  `G0 Z` lines before the first move). The ramp of an entry is each move of the item from the
  first fed move that leaves the entry XY to the end of the item (the last move ends at the
  entry). Its first point is the entry XY at the ramp start Z. The feed word does not identify
  a ramp move: the feed stages lower the feed of a steep piece. The planner read is `contact_z = ramp start Z - entry_clearance_mm`
  (`dressup::entry_descent::rapid_to_entry_top`).
- per entry: the stock top on the replay under the disc of the tool at the entry XY (the area
  that `plan_entry` reads), the stock top under the swept footprint of the shipped legs, and for
  each fed piece of the ramp the most that the piece lowers one node (`before - after`). The
  simulation measures the same quantity: `axial_engagement_mm` is "the maximum per-cell material
  length removed by this stamp" (`dexel_stock/stamping.rs`).
- the class of each piece that takes more than one lap step, by its cause. One lap of the
  zigzag descends `ENTRY_CLEARANCE` (2 mm), so a piece of the ramp as planned lowers a node by
  2 mm or less: that is the slot of the ramp, by design (class b; it is never deep). An entry
  is "clipped" when one of its pieces is flat or goes up: the model clip (G-RAMPTERRAIN,
  `clip_polyline_to_floor`) lifted a point of it.
    s  a steep piece (it goes down more than it goes along): the fall at the end of a lifted
       run. It is a plunge in effect;
    a  a piece of an entry that is not clipped: the leg, at its planned Z, meets stock outside
       the read disc that stood above the planner read;
    l  a flat or rising piece of a clipped entry: the leg rides the model floor, under stock;
    r  a descending piece of a clipped entry: the clip changed the laps before it, so they did
       not lower the stock that this piece meets;
    x  none of them (a deep node inside the read disc of an entry that is not clipped).
- the nodes that a ramp piece lowers under the target. The target of `gapmap.py` is the highest
  triangle over a node, so a node on a wall line reads the top of the wall. The test reads the
  target lowered over one node on each side (the reading of P38 section 5.2, P-6) and states
  the raw count too.

Environment: `P39_STOCK_TOP` (the Z of the flat stock top; 25.5), `P39_NO_TARGET=1`,
`P39_OFF=x,y` (G-code frame less setup frame; 18.5,8.5 on setup "1 Back", 0,0 on "3 Front").

usage: entries.py <nc> <op name prefix> <tp json> <tool radius> <out.json> [depth per pass]
"""
import json, math, re, sys, os
import numpy as np

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "p37"))
import gapmap as G

if os.environ.get("P39_OFF"):
    G.OFF = tuple(float(v) for v in os.environ["P39_OFF"].split(","))
CELL = G.CELL
CLEARANCE = 0.5          # entry_clearance_mm of ids 2 and 21 (the job file)
READ_TOL = 0.3           # the replay bar: one node (0.2) plus 0.1
EPS = 0.05
# The centre region of the op (the outer rectangle only; the keep-outs of the op are not read): `block_outline.dxf` (0..358 x 0..443, setup frame) moved to the
# G-code frame; the session clip keeps the tool centre one tool radius inside ("inside").
BLOCK = (0.0 + G.OFF[0], 0.0 + G.OFF[1], 358.0 + G.OFF[0], 443.0 + G.OFF[1])


def moves(nc):
    """Yield (op, tool, mode, p0, p1, feed) for each G0 and each fed straight piece."""
    pos = [0.0, 0.0, 36.0]; tool = None; op = None; mode = None; feed = None; line_no = -1; last_op = None
    word = re.compile(r"([GXYZIJFTM])(-?\d+\.?\d*)")
    for line in open(nc):
        line = line.strip()
        if line.startswith("("):
            t = line[1:-1]
            if not t.startswith(G.SKIP):
                op = t
            continue
        w = word.findall(line)
        if not w:
            continue
        d = {}
        for k, val in w:
            if k == "G":
                g = int(float(val))
                if g in (0, 1, 2, 3):
                    mode = g
            elif k == "T":
                tool = int(val)
            elif k == "F":
                feed = float(val)
            elif k != "M":
                d[k] = float(val)
        if not any(k in d for k in "XYZ"):
            continue
        new = [d.get("X", pos[0]), d.get("Y", pos[1]), d.get("Z", pos[2])]
        if op != last_op:
            line_no = -1; last_op = op
        line_no += 1   # the index of the motion line in its op
        if mode in (0, 1):
            yield op, tool, mode, tuple(pos), tuple(new), feed, line_no
        elif mode in (2, 3):
            cx, cy = pos[0] + d.get("I", 0.0), pos[1] + d.get("J", 0.0)
            r = math.hypot(pos[0] - cx, pos[1] - cy)
            a0 = math.atan2(pos[1] - cy, pos[0] - cx); a1 = math.atan2(new[1] - cy, new[0] - cx)
            if mode == 2:
                sweep = (a0 - a1) % (2 * math.pi) or 2 * math.pi; sg = -1
            else:
                sweep = (a1 - a0) % (2 * math.pi) or 2 * math.pi; sg = 1
            n = max(2, int(math.ceil(sweep / (2 * math.acos(max(0.0, 1 - 0.05 / max(r, 0.06)))))))
            prev = tuple(pos)
            for k in range(1, n + 1):
                f = k / n; a = a0 + sg * sweep * f
                q = (cx + r * math.cos(a), cy + r * math.sin(a), pos[2] + f * (new[2] - pos[2])) if k < n else tuple(new)
                yield op, tool, 1, prev, q, feed, line_no
                prev = q
        pos = new


def box(p0, p1, R, pad=0.0):
    x0, y0 = min(p0[0], p1[0]) - R - pad, min(p0[1], p1[1]) - R - pad
    x1, y1 = max(p0[0], p1[0]) + R + pad, max(p0[1], p1[1]) + R + pad
    ia = max(0, int(math.ceil(x0 / CELL))); ib = min(G.NX - 1, int(math.floor(x1 / CELL)))
    ja = max(0, int(math.ceil(y0 / CELL))); jb = min(G.NY - 1, int(math.floor(y1 / CELL)))
    return ia, ib, ja, jb


def run(nc, prefix, tpjson, R, out, dpp):
    deep_bar = dpp + READ_TOL
    lap_bar = 2.0 + READ_TOL   # one lap of the zigzag descends ENTRY_CLEARANCE = 2 mm
    top = np.full((G.NY, G.NX), float(os.environ.get("P39_STOCK_TOP", G.STOCK_TOP)))
    # P39_NO_TARGET=1: a setup with no target map (the front): the target test reads nothing.
    T0 = np.full((G.NY, G.NX), -np.inf) if os.environ.get("P39_NO_TARGET") else G.target()
    okT = np.isfinite(T0)
    from scipy import ndimage
    T = ndimage.grey_erosion(np.where(okT, T0, G.STOCK_TOP), size=(3, 3), mode="nearest")
    entries = []; cur = None; plunges = []
    last_mode = None; body_deep = 0; seen = False
    # the ramp moves of each entry item: the last run of the item at the ramp feed
    items = [i for i in json.load(open(tpjson))["semantic_trace"]["items"] if i["kind"] == "entry"]
    mine_moves = [m for m in moves(nc) if m[0] is not None and m[0].startswith(prefix)]
    by_line = {}
    for m in mine_moves:
        by_line.setdefault(m[6], m)
    ramp_of = {}; styles = {}; no_ramp = 0; bad_end = 0
    for k, it in enumerate(items):
        v = it["params"]["values"]; styles[k] = v["style"]
        a, b = it["move_start"] + 2, it["move_end"] + 2
        # the ramp: from the first move that leaves the entry XY from the entry XY, to the end
        ex, ey = v["entry_x"] + G.OFF[0], v["entry_y"] + G.OFF[1]
        run_ = []
        for ln in range(a, b + 1):
            m = by_line.get(ln)
            if m is None:
                continue
            at0 = math.hypot(m[3][0] - ex, m[3][1] - ey) < 2e-3
            at1 = math.hypot(m[4][0] - ex, m[4][1] - ey) < 2e-3
            if not run_ and not (m[2] == 1 and at0 and not at1):
                continue
            run_.append(ln)
        if not run_:
            no_ramp += 1
            continue
        end = by_line[b][4]
        if math.hypot(end[0] - v["entry_x"] - G.OFF[0], end[1] - v["entry_y"] - G.OFF[1]) > 2e-3:
            bad_end += 1
        for ln in run_:
            ramp_of[ln] = k
    print(f"entry items {len(items)}: with a ramp {len(set(ramp_of.values()))}, with no ramp move (air, or a plunge) {no_ramp}; items whose last move is not at the entry XY: {bad_end}")

    def close():
        nonlocal cur
        if cur is not None:
            entries.append(cur); cur = None

    for op, tool, mode, p0, p1, feed, ln in moves(nc):
        mine = op is not None and op.startswith(prefix)
        if not mine and seen:
            break      # the op is done
        seen = seen or mine
        if mode == 0:
            if mine:
                close()
            last_mode = 0
            continue
        if not mine:
            G.stamp(top, tool, p0, p1); last_mode = 1
            continue
        is_ramp = ln in ramp_of
        if cur is not None and (not is_ramp or ramp_of[ln] != cur['item']):
            close()
        ia, ib, ja, jb = box(p0, p1, R)
        before = top[ja:jb + 1, ia:ib + 1].copy()
        if is_ramp and cur is None:
            # the entry starts: read the stock before the first ramp piece
            e = p0
            da, db_, dc, dd = box(e, e, R)
            X = np.arange(da, db_ + 1) * CELL; Y = (np.arange(dc, dd + 1) * CELL)[:, None]
            disc = (X - e[0]) ** 2 + (Y - e[1]) ** 2 <= R * R + 1e-9
            sub = top[dc:dd + 1, da:db_ + 1]
            cur = dict(item=ramp_of[ln], style=styles[ramp_of[ln]], x=e[0], y=e[1], z_start=e[2], contact=e[2] - CLEARANCE,
                       disc_top=float(sub[disc].max()) if disc.any() else float("nan"),
                       pts=[e], pieces=[], foot_top=-1e9, out_area=0.0, below=0, below_max=0.0,
                       x_max=e[0], x_min=e[0], y_min=e[1], y_max=e[1])
        if not is_ramp:
            close()
        G.stamp(top, tool, p0, p1)
        after = top[ja:jb + 1, ia:ib + 1]
        diff = before - after
        cut = diff > EPS
        d = float(diff[cut].max()) if cut.any() else 0.0
        L = math.dist(p0, p1)
        if not is_ramp:
            vertical = math.hypot(p1[0] - p0[0], p1[1] - p0[1]) < 1e-6
            if vertical and last_mode == 0 and p1[2] < p0[2]:
                plunges.append((p0[0], p0[1], p0[2], p1[2], d))
            if d > deep_bar and not vertical:
                body_deep += 1
            last_mode = 1
            continue
        last_mode = 1
        X = np.arange(ia, ib + 1) * CELL; Y = (np.arange(ja, jb + 1) * CELL)[:, None]
        # the swept footprint of the piece: nodes within R of the chord
        dx, dy = p1[0] - p0[0], p1[1] - p0[1]; L2 = dx * dx + dy * dy
        if L2 < 1e-18:
            dist2 = (X - p0[0]) ** 2 + (Y - p0[1]) ** 2
        else:
            t = np.clip(((X - p0[0]) * dx + (Y - p0[1]) * dy) / L2, 0.0, 1.0)
            dist2 = (X - p0[0] - t * dx) ** 2 + (Y - p0[1] - t * dy) ** 2
        foot = dist2 <= R * R + 1e-9
        if foot.any():
            cur["foot_top"] = max(cur["foot_top"], float(before[foot].max()))
        in_disc = (X - cur["x"]) ** 2 + (Y - cur["y"]) ** 2 <= R * R + 1e-9
        cur["out_area"] += float((cut & ~in_disc).sum()) * CELL * CELL
        Tsub = T[ja:jb + 1, ia:ib + 1]; oks = okT[ja:jb + 1, ia:ib + 1]
        bel = cut & oks & (after < Tsub - READ_TOL)
        cur["below_raw"] = cur.get("below_raw", 0) + int((cut & oks & (after < T0[ja:jb + 1, ia:ib + 1] - READ_TOL)).sum())
        if bel.any():
            cur["below"] += int(bel.sum()); cur["below_max"] = max(cur["below_max"], float((Tsub - after)[bel].max()))
        cls = None; node = None
        if cut.any():
            j, i = np.unravel_index(np.argmax(np.where(cut, diff, -1.0)), diff.shape)
            stood = float(before[j, i]); inside = bool(in_disc[j, i])
            node = (float(X[i]), float(Y[j, 0]), stood, float(after[j, i]))
            if p0[2] - p1[2] > math.hypot(dx, dy):
                cls = "s"
            elif p1[2] >= p0[2] - 1e-9:
                cls = "l"
            elif stood > cur["contact"] + READ_TOL and not inside:
                cls = "a"      # "r" when the entry is clipped: set at the end of the entry
            else:
                cls = "x"
        cur["pieces"].append(dict(d=d, L=L, cls=cls, node=node, p0=p0, p1=p1, up=bool(p1[2] >= p0[2] - 1e-9 and L2 > 1e-12), below=int(bel.sum()), below_max=float((Tsub - after)[bel].max()) if bel.any() else 0.0))
        cur["pts"].append(p1)
        cur["x_max"] = max(cur["x_max"], p1[0]); cur["x_min"] = min(cur["x_min"], p1[0])
        cur["y_max"] = max(cur["y_max"], p1[1]); cur["y_min"] = min(cur["y_min"], p1[1])
    close()
    print("styles of the entries with a ramp:", {st: sum(e["style"] == st for e in entries) for st in set(styles.values())})

    # the report
    n = len(entries)
    print(f"ramp entries {n}; straight feeds down after a rapid (approach or plunge) {len(plunges)}; body pieces deeper than {deep_bar:.1f} mm: {body_deep}")
    rows = []
    for k, e in enumerate(entries):
        ps = e["pieces"]
        dmax = max((p["d"] for p in ps), default=0.0)
        worst = max(ps, key=lambda p: p["d"]) if ps else None
        e["clipped"] = any(p["up"] for p in ps)
        for p in ps:
            if e["clipped"] and p["cls"] in ("a", "x"):
                p["cls"] = "r"
        e["d_max"] = dmax; e["cls"] = worst["cls"] if worst else None
        e["leg"] = max(e["x_max"] - e["x"], e["x"] - e["x_min"], e["y_max"] - e["y"], e["y"] - e["y_min"])
        e["drop"] = e["z_start"] - e["pts"][-1][2]
        e["closed"] = math.hypot(e["pts"][-1][0] - e["x"], e["pts"][-1][1] - e["y"]) < 1e-3
        # the tool centre against the centre region of the op
        e["out_of_centre"] = max(BLOCK[0] + R - e["x_min"], e["x_max"] - (BLOCK[2] - R),
                                 BLOCK[1] + R - e["y_min"], e["y_max"] - (BLOCK[3] - R))
        rows.append(e)
    d = np.array([e["d_max"] for e in rows])
    print(f"entry drop (ramp start to entry Z): min {min(e['drop'] for e in rows):.2f} max {max(e['drop'] for e in rows):.2f} mm; longest leg {max(e['leg'] for e in rows):.2f} mm")
    print(f"entries that do not end at their start XY: {sum(not e['closed'] for e in rows)}")
    print(f"replay disc top against the planner read (disc_top - contact): min {min(e['disc_top']-e['contact'] for e in rows):.2f} max {max(e['disc_top']-e['contact'] for e in rows):.2f}")
    over = np.array([e["foot_top"] - e["contact"] for e in rows])
    print(f"stock top under the swept footprint above the planner read by > {READ_TOL}: {(over > READ_TOL).sum()} entries; by > {dpp}: {(over > dpp).sum()}; max {over.max():.2f} mm")
    for b in (lap_bar, deep_bar, 9.0, 12.0, 14.0):
        print(f"entries with a piece deeper than {b:5.1f} mm: {(d > b).sum()}")
    print(f"deepest ramp piece {d.max():.3f} mm")
    for bar, tag in ((deep_bar, f"deep (> {deep_bar:.1f} mm = one Depth/Pass + the bar)"), (lap_bar, f"over one lap (> {lap_bar:.1f} mm)")):
        print(f"-- pieces {tag}, by class")
        for c in "salrx":
            pp = [p for e in rows for p in e["pieces"] if p["d"] > bar and p["cls"] == c]
            ee = [e for e in rows if any(p["d"] > bar and p["cls"] == c for p in e["pieces"])]
            print(f"   class {c}: {len(pp):5d} pieces, {sum(p['L'] for p in pp):9.1f} mm fed, {len(ee):4d} entries, deepest {max((p['d'] for p in pp), default=0):.2f} mm")
        ee = [e for e in rows if e["d_max"] > bar]
        for c in "salrx":
            print(f"   entries whose deepest piece is class {c}: {sum(e['cls'] == c for e in ee)}")
    allp = [p for e in rows for p in e["pieces"]]
    print(f"ramp pieces {len(allp)}, fed {sum(p['L'] for p in allp):.1f} mm")
    print(f"area that the ramps lower outside the read disc (the stamp of the planner is the column): {sum(e['out_area'] for e in rows):.1f} mm2 (node visits)")
    print(f"ramp nodes under the raw target by > {READ_TOL}: {sum(e.get('below_raw', 0) for e in rows)}")
    print(f"ramp nodes under the target (lowered over one node) by > {READ_TOL}: {sum(e['below'] for e in rows)} nodes in {sum(e['below'] > 0 for e in rows)} entries, max {max(e['below_max'] for e in rows):.2f} mm")
    print(f"entries that the model clip touched: {sum(e['clipped'] for e in rows)} of {n}")
    by = {}
    for e in rows:
        for p in e["pieces"]:
            if p["below"]:
                k = p["cls"] or ("up" if p["up"] else "plan"); by[k] = by.get(k, 0) + p["below"]
    print("   the nodes under the lowered target, by the class of the piece:", by)
    hi = [e for e in rows if e["contact"] - e["disc_top"] > 0.5]
    air = sum(p["L"] for e in hi for p in e["pieces"] if p["d"] <= EPS)
    print(f"entries whose planner read is more than 0.5 mm above the replay disc top: {len(hi)} (max {max((e['contact'] - e['disc_top'] for e in rows)):.2f} mm); their ramp pieces that cut nothing: {air:.1f} mm fed")
    print(f"ramp pieces that cut nothing, all entries: {sum(p['L'] for e in rows for p in e['pieces'] if p['d'] <= EPS):.1f} mm fed")
    oc = [e for e in rows if e["out_of_centre"] > 0.011]
    print(f"entries with a ramp point outside the centre region: {len(oc)} (max {max((e['out_of_centre'] for e in rows)):.2f} mm)")
    json.dump(dict(entries=[{k: v for k, v in e.items()} for e in rows], plunges=plunges), open(out, "w"), default=float)
    return rows


if __name__ == "__main__":
    run(sys.argv[1], sys.argv[2], sys.argv[3], float(sys.argv[4]), sys.argv[5], float(sys.argv[6]) if len(sys.argv) > 6 else 6.0)
