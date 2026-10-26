"""C18/fix17: every table number of fixture3d_v5.html, per column, from the
run artefacts (writes numbers_v5.json). Rules:
- descents into stock: fix11/audit.py's descent events (vertical fed
  descent at one XY, pecks included, removing more than cell^2 * step);
- lift-over descents: entries_v5.json rows, span "keep-down link", drop >= 1.0 mm,
  removed > 0.1 mm3 (fix9 rule);
- plunge entries: the trace's "plunge entry" spans;
- planar turns > 30 deg and planar reversals (> 90 deg), split by where the
  junction lies (the start of the outgoing move, in the sim frame):
  inside the part outline (Corne: the hole-filled silhouette of the STL
  raster; wanaka200: the terrain footprint x 20..220, y 25..225) or outside;
  G-code to sim frame: Corne + (stock min x, stock min y, stock max z),
  wanaka200 setup 0 + (0, 0, 25);
- plunge-feed share on the program clock: fed vertical-down moves of
  program.nc at their F / the op's program time (C39 rule; G0 at the
  machine max_feed: Corne 4000, wanaka200 10000 mm/min);
- cycle: summary.json total_runtime_s (all rough ops; the clock: Corne naive,
  wanaka200 machine model, C39);
- 3-9 mm band (Corne): stock > 0.8 mm above target outside the hole-filled
  silhouette at 3 to 9 mm from it (fix15/bands_d25.py rule, on stock_v5);
- hard bars: summary.json collisions and export refusal; stderr lines
  (fed link cuts stock, deeper than, gouge); the op-1 island line;
- turn summary: the op-1 'C22 turn summary' line of stdout."""
import glob, json, math, os, re, sys
import numpy as np
from scipy import ndimage
H = os.path.dirname(os.path.abspath(__file__)); P3 = os.path.join(H, "..")
sys.path.insert(0, os.path.join(P3, "fix11"))
import audit  # noqa: E402
sys.path.insert(0, H)
from render_v5 import COLS_C, COLS_W, tag  # noqa: E402

PARTS = {"corne": {"cols": COLS_C, "op": "3D Rough 1", "rapid": 4000.0, "nom": 1518.75 * 1.2 * 6.0 / 60},
         "wanaka200": {"cols": COLS_W, "op": "2 Back Rough", "rapid": 10000.0, "nom": 1000.0 * 2.0 * 5.46 / 60}}
E = json.load(open(os.path.join(H, "entries_v5.json")))
part = np.load(os.path.join(P3, "fix9", "corne_part.npz"))
ptop, pg, px0, py0 = part["top"], float(part["g"]), float(part["x0"]), float(part["y0"])
SIL = ndimage.binary_fill_holes(np.isfinite(ptop) & (ptop > 1e-6))


def inside(fx, x, y):
    if fx == "wanaka200":
        return 20 <= x <= 220 and 25 <= y <= 225
    i, j = int(round((x - px0) / pg)), int(round((y - py0) / pg))
    return 0 <= j < SIL.shape[0] and 0 <= i < SIL.shape[1] and bool(SIL[j, i])


def column(fx, col, rd):
    cfg = PARTS[fx]; run = os.path.join(P3, rd)
    r = audit.audit(run, run, cfg["op"], "tp_1_*.json", cfg["nom"])
    tp = json.load(open(glob.glob(os.path.join(run, "tp_1_*.json"))[0])); n = tp["move_count"]
    mv = audit.op_moves(os.path.join(run, "program.nc"), cfg["op"])
    allm, mv = mv, mv[1:n + 1]
    sim = json.load(open(os.path.join(run, "simulation.json")))
    off = (sim["stock_bbox_min"][0], sim["stock_bbox_min"][1], sim["stock_bbox_max"][2]) if fx == "corne" else (0.0, 0.0, 25.0)
    xy = lambda m: math.hypot(m["b"][0] - m["a"][0], m["b"][1] - m["a"][1])
    turns = {"in": [0, 0, 0], "out": [0, 0, 0]}   # junctions, > 30, > 90
    prev = None
    for m in mv:
        if m["g"] == 0:
            prev = None
            continue
        t0, t1 = audit.tangents(m)
        if t0 is None:
            continue
        if prev is not None and prev[1] and xy(m) >= audit.GCODE_EPS:
            c = max(-1.0, min(1.0, sum(p * q for p, q in zip(prev[0], t0)))); t = math.degrees(math.acos(c))
            where = "in" if inside(fx, m["a"][0] + off[0], m["a"][1] + off[1]) else "out"
            turns[where][0] += 1; turns[where][1] += t > 30 + 1e-6; turns[where][2] += t > 90 + 1e-6
        prev = (t1, xy(m) >= audit.GCODE_EPS)
    def length(m):
        if m["g"] in (2, 3):
            cx, cy = m["a"][0] + m["i"], m["a"][1] + m["j"]; rad = math.hypot(m["a"][0] - cx, m["a"][1] - cy)
            a0, a1 = math.atan2(m["a"][1] - cy, m["a"][0] - cx), math.atan2(m["b"][1] - cy, m["b"][0] - cx)
            sw = ((a0 - a1) if m["g"] == 2 else (a1 - a0)) % (2 * math.pi) or 2 * math.pi
            return math.hypot(rad * sw, m["b"][2] - m["a"][2])
        return math.dist(m["a"], m["b"])
    prog = sum(length(m) / (cfg["rapid"] if m["g"] == 0 else m["f"]) * 60 for m in allm)
    plunge = sum(length(m) / m["f"] * 60 for m in mv if m["g"] != 0 and xy(m) < audit.GCODE_EPS and m["b"][2] - m["a"][2] < -audit.GCODE_EPS)
    s = json.load(open(os.path.join(run, "summary.json")))
    err = open(os.path.join(run, "stderr.txt")).read()
    out = re.sub(r"\x1b\[[0-9;]*m", "", open(os.path.join(run, "stdout.txt")).read())
    isl = re.search(rf'"{re.escape(cfg["op"])}": the rough leaves (\d+) island\(s\).*?(\d+) of them against a wall: (\d+\.\d+) mm2', err)
    ts = re.search(r"generate_toolpath\{index=0\}.*?C22 turn summary kind=\"turns\" planned=(\d+) cut=(\d+) fed=(\d+) unfed=(\d+)", out)
    rows = E[f"{fx} {col}"]["rows"]
    res = {"descents": r["descent_events"],
           "lift_over": sum(1 for w in rows if w["style"] == "keep-down link" and w["descent_mm"] >= 1.0 and w["removed_mm3"] > 0.1),
           "plunge_entries": sum(1 for e in tp["semantic_trace"]["items"] if e["kind"] == "entry" and e["params"]["values"]["style"] == "plunge entry"),
           "turns_in": turns["in"], "turns_out": turns["out"],
           "plunge_s": round(plunge, 1), "program_s": round(prog, 1), "plunge_pct": round(100 * plunge / prog, 1),
           "cycle_s": round(s["total_runtime_s"]),
           "collisions": [s["collision_count"], s["rapid_collision_count"], s["collision_checks_failed"]],
           "export_refused": s["gcode_export"]["refused"] is not None,
           "fed_link_lines": sum(1 for ln in err.splitlines() if "fed link cuts stock" in ln),
           "deeper_lines": sum(1 for ln in err.splitlines() if "deeper than" in ln),
           "gouge_lines": sum(1 for ln in err.splitlines() if "gouge" in ln.lower()),
           "fed_links": [[m.group(1), int(m.group(2)), float(m.group(3)), m.group(4)] for m in re.finditer(r'"([^"]+)": a fed link cuts stock.*?link move (\d+) removes (\d+\.\d+) mm at \(([^)]*)\)', err)],
           "deeper": [[float(m.group(1)), float(m.group(2))] for m in re.finditer(r"deeper than this pass was planned to cut: (\d+\.\d+)% of cutting samples.*?peaking at (\d+\.\d+) mm", err)],
           "islands": [int(isl.group(1)), int(isl.group(2)), float(isl.group(3))] if isl else None,
           "turn_summary": [int(ts.group(k)) for k in (1, 2, 3, 4)] if ts else None}
    if fx == "corne":
        m = np.load(os.path.join(H, f"stock_{fx}_{tag(col)}.npz")); top, target, xs, ys, g = m["top"], m["target"], m["xs"], m["ys"], float(m["g"])
        X, Y = np.meshgrid(xs, ys)
        i = np.round((X - px0) / pg).astype(int); j = np.round((Y - py0) / pg).astype(int)
        ok = (i >= 0) & (i < ptop.shape[1]) & (j >= 0) & (j < ptop.shape[0])
        t = np.zeros(X.shape); t[ok] = np.nan_to_num(ptop[j[ok], i[ok]], nan=0.0, neginf=0.0)
        sil = ndimage.binary_fill_holes(t > 1e-6); dout = ndimage.distance_transform_edt(~sil) * g
        st = (top - target) > 0.8
        res["band_3_9_mm2"] = round(float((st & ~sil & (dout > 3) & (dout <= 9)).sum() * g * g))
    return res


def image_facts():
    """Two things the images show, as numbers: (1) on Corne the median stock
    above target in a 2 mm disc at three hole centres (C35/D24/D25 leave a
    solid disc there); (2) the length of linking steps on the strict plane
    Z 17 in the box x 95..130, y -70..-15 (the D24 band of link arcs)."""
    from render_v5 import steps
    F = {"holes": HOLES, "disc": {}, "links_z17_box_mm": {}}
    for col in COLS_C:
        m = np.load(os.path.join(H, f"stock_corne_{tag(col)}.npz")); a = m["top"] - m["target"]
        X, Y = np.meshgrid(m["xs"], m["ys"])
        F["disc"][col] = [round(float(np.median(a[np.hypot(X - cx, Y - cy) <= 2.0])), 2) for cx, cy in HOLES]
        p, q, it, k, P, I = steps(COLS_C[col][1])
        on = (np.abs(p[:, 2] - 17.0) <= 0.01) & (np.abs(q[:, 2] - 17.0) <= 0.01) & (it == "linking")
        box = on & (p[:, 0] >= 95) & (p[:, 0] <= 130) & (p[:, 1] >= -70) & (p[:, 1] <= -15)
        F["links_z17_box_mm"][col] = round(float(np.hypot(*(q[box, :2] - p[box, :2]).T).sum()))
    return F


HOLES = [[28.3, -30.0], [80.3, -40.0], [45.6, -80.1]]   # centroids of the three > 2.5 mm components of about 50 mm2 (C35 stock map)


if __name__ == "__main__":
    N = {}
    for fx, cfg in PARTS.items():
        for col, (rd, sd) in cfg["cols"].items():
            N[f"{fx} {col}"] = column(fx, col, rd)
            print(fx, col, N[f"{fx} {col}"])
    N["image_facts"] = image_facts(); print(N["image_facts"])
    N["c40"] = json.load(open(os.path.join(P3, "fix15", "c40_join.json")))
    json.dump(N, open(os.path.join(H, "numbers_v5.json"), "w"), indent=1)
