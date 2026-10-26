"""C18/fix9: re-derive every number in README.md from the raw artefacts and
check that README.md states it. It does not import stockmap.py, level1.py or
render.py; it rebuilds the stock with its own code (a flat end mill of R
lowers every cell whose centre is within R of the densified tool path).
Run: python3 -I assert_fix9.py        (about 2 minutes)"""
import json, math, os, re, struct
import numpy as np
from scipy import ndimage

H = os.path.dirname(os.path.abspath(__file__)); P3 = os.path.join(H, ".."); RES = os.path.join(P3, "..")
R = 3.0
CORNE = {"contour_parallel": ("fix6/corne/contour_parallel", "fix6/corne/contour_parallel"),
         "zigzag_pick D18": ("fix5/corne/geodesic_local_zigzag_pick", "fix5/corne/geodesic_local_zigzag_pick"),
         "zigzag_pick D20": ("fix7/corne/geodesic_local_zigzag_pick", "fix7/corne/geodesic_local_zigzag_pick"),
         "AgentSearch": ("fix1/corne/agent", "fix2/simart/corne/agent")}
WAN = {"contour_parallel": ("fix6/wanaka200/contour_parallel", "fix6/wanaka200/contour_parallel"),
       "zigzag_pick D18": ("fix2/wanaka200/geodesic_local_zigzag", "fix2/simart/wanaka200/geodesic_local_zigzag"),
       "zigzag_pick D20": ("fix7/wanaka200/geodesic_local_zigzag_pick", "fix7/wanaka200/geodesic_local_zigzag_pick"),
       "AgentSearch": ("fix2/wanaka200/agent", "fix2/simart/wanaka200/agent")}
SEED = (55.866, -49.762)
got = {}


def stl_top(path, g):
    raw = open(path, "rb").read(); n = struct.unpack("<I", raw[80:84])[0]
    t = np.frombuffer(raw[84:84 + 50 * n], dtype=np.dtype([("n", "<f4", 3), ("v", "<f4", (3, 3)), ("a", "<u2")]))["v"].astype(float)
    lo = t.reshape(-1, 3).min(0); hi = t.reshape(-1, 3).max(0)
    x0, y0 = lo[0] - g, lo[1] - g; nx = int((hi[0] + g - x0) / g) + 1; ny = int((hi[1] + g - y0) / g) + 1
    top = np.full((ny, nx), -np.inf)
    for (ax, ay, az), (bx, by, bz), (cx, cy, cz) in t:
        d = (by - cy) * (ax - cx) + (cx - bx) * (ay - cy)
        if abs(d) < 1e-12: continue
        i0 = max(0, int((min(ax, bx, cx) - x0) / g)); i1 = min(nx - 1, int((max(ax, bx, cx) - x0) / g) + 1)
        j0 = max(0, int((min(ay, by, cy) - y0) / g)); j1 = min(ny - 1, int((max(ay, by, cy) - y0) / g) + 1)
        X, Y = np.meshgrid(x0 + np.arange(i0, i1 + 1) * g, y0 + np.arange(j0, j1 + 1) * g)
        l1 = ((by - cy) * (X - cx) + (cx - bx) * (Y - cy)) / d; l2 = ((cy - ay) * (X - cx) + (ax - cx) * (Y - cy)) / d; l3 = 1 - l1 - l2
        k = (l1 >= -1e-9) & (l2 >= -1e-9) & (l3 >= -1e-9)
        if k.any():
            sub = top[j0:j1 + 1, i0:i1 + 1]; sub[k] = np.maximum(sub[k], (l1 * az + l2 * bz + l3 * cz)[k])
    return top, x0, y0


def rebuild(simdir, g, flip, upto=None):
    sim = json.load(open(os.path.join(P3, simdir, "simulation.json")))
    lo, hi = list(sim["stock_bbox_min"]), list(sim["stock_bbox_max"])
    if flip:
        hi = [hi[i] - lo[i] for i in range(3)]; lo = [0.0, 0.0, 0.0]
    nx, ny = int(round((hi[0] - lo[0]) / g)), int(round((hi[1] - lo[1]) / g))
    top = np.full((ny, nx), float(hi[2]))
    P = np.array([s["position"] for s in sim["trace"]["samples"] if s["toolpath_id"] == 1 and (upto is None or s["move_index"] < upto)])
    pts = [P[:1]]
    for a, b in zip(P[:-1], P[1:]):
        k = max(1, int(math.ceil(math.hypot(b[0] - a[0], b[1] - a[1]) / (g / 2))))
        pts.append(a + (np.arange(1, k + 1) / k)[:, None] * (b - a))
    D = np.vstack(pts)
    ci = np.floor((D[:, 0] - lo[0]) / g).astype(int); cj = np.floor((D[:, 1] - lo[1]) / g).astype(int)
    r = int(math.ceil(R / g)) + 1
    for dj in range(-r, r + 1):
        for di in range(-r, r + 1):
            I, J = ci + di, cj + dj
            ok = (I >= 0) & (I < nx) & (J >= 0) & (J < ny)
            ok &= (lo[0] + (I + 0.5) * g - D[:, 0]) ** 2 + (lo[1] + (J + 0.5) * g - D[:, 1]) ** 2 <= R * R
            np.minimum.at(top, (J[ok], I[ok]), D[ok, 2])
    xs = lo[0] + (np.arange(nx) + 0.5) * g; ys = lo[1] + (np.arange(ny) + 0.5) * g
    return top, xs, ys, sim


def target_of(surf, g, leave):
    k = int(math.ceil(R / g)); yy, xx = np.mgrid[-k:k + 1, -k:k + 1]; disc = (xx * g) ** 2 + (yy * g) ** 2 <= R * R
    return ndimage.grey_erosion(ndimage.grey_dilation(surf, footprint=disc, mode="nearest"), footprint=disc, mode="nearest") + leave


def sample_map(top, x0, y0, g, X, Y):
    i = np.round((X - x0) / g).astype(int); j = np.round((Y - y0) / g).astype(int)
    ok = (i >= 0) & (i < top.shape[1]) & (j >= 0) & (j < top.shape[0])
    v = np.full(X.shape, -np.inf); v[ok] = top[j[ok], i[ok]]
    return v


# 1. What simulation.json holds.
keys = sorted(json.load(open(os.path.join(P3, "fix7/corne/geodesic_local_zigzag_pick/simulation.json"))).keys())
trace_keys = sorted(json.load(open(os.path.join(P3, "fix7/corne/geodesic_local_zigzag_pick/simulation.json")))["trace"].keys())
assert not any("stock" in k and k not in ("stock_bbox_min", "stock_bbox_max") for k in keys + trace_keys), (keys, trace_keys)
got["sim_keys"] = ", ".join(keys)

# 2. Corne.
ctop, cx0, cy0 = stl_top("/home/ricky/Downloads/Corne Case Hex.stl", 0.2)
_v = np.frombuffer(open("/home/ricky/Downloads/Corne Case Hex.stl", "rb").read()[84:], dtype=np.dtype([("n", "<f4", 3), ("v", "<f4", (3, 3)), ("a", "<u2")]))["v"].reshape(-1, 3)
ctop_lo_hi = (_v.min(0), _v.max(0))
g = 0.25
for col, (rd, sd) in CORNE.items():
    top, xs, ys, sim = rebuild(sd, g, False)
    X, Y = np.meshgrid(xs, ys)
    pv = sample_map(ctop, cx0, cy0, 0.2, X, Y); surf = np.where(np.isfinite(pv), pv, 0.0)
    a = top - target_of(surf, g, 0.5)
    sil = ndimage.binary_fill_holes(surf > 0)
    got[f"corne {col} inside"] = round(float(((a > 0.8) & sil).sum() * g * g), 1)
    lo, hi = ctop_lo_hi
    inb = (X >= lo[0]) & (X <= hi[0]) & (Y >= lo[1]) & (Y <= hi[1])
    got[f"corne {col} outer band"] = round(float(((a > 0.8) & inb & ~sil).sum() * g * g), 1)
    err = open(os.path.join(P3, rd, "stderr.txt")).read()
    got[f"corne {col} triage"] = float(re.search(r'"3D Rough 1": the rough leaves \d+ island.*?against a wall: (\d+\.\d+) mm2', err).group(1))
    r = np.hypot(X - SEED[0], Y - SEED[1])
    ring = (r < 50) & sil
    ann = (r >= 25.45) & (r < 50) & sil
    got[f"corne {col} annulus"] = (round(float(a[ann].mean()), 3), round(float(np.mean(a[ann] > 0.8)), 3))
    got[f"corne {col} r50 over"] = round(float(np.mean(a[ring] > 0.8)), 3)
    if col == "zigzag_pick D20":
        in35 = (r < 35) & ((X - cx0) >= 0)
        cells = sil & (r < 35)
        got["no-part share r<35"] = round(float(np.mean(surf[cells] <= 0)), 2)
        got["plate top r<35"] = round(float(np.median(surf[cells & (surf > 0)])), 2)
        got["nearest above Z17"] = round(float(r[surf + 0.5 > 17.0].min()), 2)
    # the stock after level Z 17: the moves before the level-2 annotation
    tp = json.load(open(os.path.join(P3, rd, "tp_1_3D_Rough_1.json")))
    ents = sorted([i for i in tp["semantic_trace"]["items"] if i["kind"] == "entry"], key=lambda i: i["move_start"])
    gen = {int(m.group(1)): an["move_index"] for an in tp["debug_trace"]["annotations"] for m in [re.match(r"Pass (\d+) — ", an["label"])] if m}
    fin = sorted((gen[e["params"]["values"]["pass_index"]], e["move_start"]) for e in ents)
    lv = [an["move_index"] for an in tp["debug_trace"]["annotations"] if an["label"].startswith("Adaptive Z")]
    l2 = next(f for gi, f in fin if gi >= lv[1])
    t1, _, _, _ = rebuild(sd, g, False, upto=l2)
    floor17 = np.maximum(target_of(surf, g, 0.5), 17.0)
    for r0, r1 in ((0, 25), (25, 50)):
        b = (r >= r0) & (r < r1) & sil
        got[f"corne {col} Z17 cut r{r0}-{r1}"] = round(float(np.mean(t1[b] <= floor17[b] + 0.8)), 3)

# 3. wanaka200 (setup 0, face bottom up).
wtop, wx0, wy0 = stl_top("/home/ricky/Downloads/wanaka200/rivmap_export/terrain.stl", 0.5)
g = 0.5
for col, (rd, sd) in WAN.items():
    top, xs, ys, sim = rebuild(sd, g, True)
    X, Y = np.meshgrid(xs, ys)
    pv = sample_map(wtop, wx0, wy0, 0.5, X - 20.0, 225.0 - Y); surf = np.where(np.isfinite(pv), 7.0 - pv, 0.0)
    a = top - target_of(surf, g, 4.0)
    fpi = (X >= 23) & (X <= 217) & (Y >= 28) & (Y <= 222)
    got[f"wanaka {col} inside"] = round(float(((a > 0.8) & fpi).sum() * g * g), 1)
    got[f"wanaka {col} island line"] = '"2 Back Rough": the rough leaves' in open(os.path.join(P3, rd, "stderr.txt")).read()
    if col == "zigzag_pick D20":
        Q = np.array([s["position"] for s in sim["trace"]["samples"] if s["is_cutting"] and s["source_intent"] == "clearing_cut"])
        pv = sample_map(wtop, wx0, wy0, 0.5, Q[:, 0] - 20.0, 225.0 - Q[:, 1]); ok = np.isfinite(pv)
        got["wanaka gap p1"] = round(float(np.percentile(pv[ok] - (7.0 - Q[ok, 2]), 1)), 2)

# 4. The law radius and the images.
got["law radius in P2_C2"] = "**25.45 mm**" in open(os.path.join(RES, "P2_C2_geodesic_fixes.md")).read()
cc = json.load(open(os.path.join(H, "crop_centres.json")))
for fx, cols in (("corne", CORNE), ("wanaka200", WAN)):
    for col in cols:
        t = re.sub(r"[^A-Za-z0-9]+", "_", col).strip("_")
        for f in [f"plane_{fx}_{t}_{k}.png" for k in range(3)] + [f"drape_{fx}_{t}.png", f"crop_{fx}_{t}.png"]:
            assert os.path.exists(os.path.join(H, "img", f)), f
    assert os.path.exists(os.path.join(H, "img", f"stockmap_{fx}.png"))
    got[f"crop {fx}"] = (round(cc[fx]["x"], 1), round(cc[fx]["y"], 1), cc[fx]["plane"])


# 5. Section 0: arcs drawn as chords on the Z 17 plane (own G-code reader).
SEED_G = (55.866 - 0.31372880935668945, -49.762 + 109.31554412841797)
WRD = re.compile(r"([GXYZIJF])(-?\d+(?:\.\d+)?)")
for col, rd in (("zigzag_pick D18", "fix5/corne/geodesic_local_zigzag_pick"), ("zigzag_pick D20", "fix7/corne/geodesic_local_zigzag_pick"), ("contour_parallel", "fix6/corne/contour_parallel")):
    L = open(os.path.join(P3, rd, "program.nc")).read().splitlines()
    i0 = L.index("(3D Rough 1)") + 1; i1 = L.index("(3D Rough 1 [copy])")
    gm, p = 0, (0.0, 0.0, 0.0); res = {"in": [0, 0.0, 0.0], "ring": [0, 0.0, 0.0]}
    for ln in L[i0:i1]:
        w = {k: float(v) for k, v in WRD.findall(ln)}
        gm = int(w.get("G", gm))
        if not ("X" in w or "Y" in w or "Z" in w): continue
        q = (w.get("X", p[0]), w.get("Y", p[1]), w.get("Z", p[2]))
        if gm in (2, 3) and abs(p[2] + 6) < 0.01 and abs(q[2] + 6) < 0.01:
            rr = math.hypot((p[0] + q[0]) / 2 - SEED_G[0], (p[1] + q[1]) / 2 - SEED_G[1])
            key = "in" if rr < 25.45 else ("ring" if rr < 50 else None)
            if key:
                c = (p[0] + w.get("I", 0), p[1] + w.get("J", 0)); rad = math.hypot(p[0] - c[0], p[1] - c[1])
                t0, t1 = math.atan2(p[1] - c[1], p[0] - c[0]), math.atan2(q[1] - c[1], q[0] - c[0])
                sw = ((t0 - t1) if gm == 2 else (t1 - t0)) % (2 * math.pi) or 2 * math.pi
                res[key][0] += 1; res[key][1] += rad * sw; res[key][2] += math.hypot(q[0] - p[0], q[1] - p[1])
        p = q
    got[f"arcs {col}"] = {k: (v[0], round(v[1], 1), round(v[2], 1)) for k, v in res.items()}
    print(col, got[f"arcs {col}"])
blk = open(os.path.join(P3, "fix7/corne/geodesic_local_zigzag_pick/program.nc")).read()
assert "G1 X74.377 Y59.554 Z0.500\nG1 X75.502 Y59.554 Z0.500 F1519\nG1 X75.502 Y59.554 Z-5.500 F750\nG0 X75.502 Y59.554 Z-5.000\nG1 X75.502 Y59.554 Z-6.000 F750\nG1 X75.097 Y63.554 Z-6.000 F4000" in blk

# 6. Section 4: entries (own reader of the trace spans and the sim samples).
for fx, cols in (("corne", CORNE), ("wanaka200", WAN)):
    for col, (rd, sd) in cols.items():
        tpf = [f for f in os.listdir(os.path.join(P3, rd)) if f.startswith("tp_1_")][0]
        sp = sorted((e["move_start"], e["move_end"], e["params"]["values"]["style"]) for e in json.load(open(os.path.join(P3, rd, tpf)))["semantic_trace"]["items"] if e["kind"] == "entry")
        S = [x for x in json.load(open(os.path.join(P3, sd, "simulation.json")))["trace"]["samples"] if x["toolpath_id"] == 1]
        grp = {}
        for x in S:
            if x["source_intent"] == "entry_plunge":
                k = next((t for t in sp if t[0] <= x["move_index"] <= t[1]), None)
                grp.setdefault(k, []).append(x)
        dsc = {"plunge entry": 0, "keep-down link": 0}; z17 = {"plunge entry": 0, "keep-down link": 0}; ml = 0; drops = []; vols = []
        for k, xs in grp.items():
            zz = [x["position"][2] for x in xs]; drop = max(zz) - zz[-1]; vol = sum(x["removed_volume_est_mm3"] for x in xs)
            real = drop >= 1.0 and vol > 0.1
            if real: dsc[k[2]] += 1
            if fx == "corne" and abs(zz[-1] - 17.0) <= 0.35 and (k[2] == "plunge entry" or real): z17[k[2]] += 1
            if col == "zigzag_pick D20" and fx == "corne" and k[2] == "keep-down link" and drop >= 1.0:
                drops.append(drop); vols.append(vol)
                if abs(xs[-1]["position"][1] + 49.76) < 0.2: ml += 1
        got[f"entries {fx} {col}"] = (sum(1 for t in sp if t[2] == "plunge entry"), dsc["plunge entry"], dsc["keep-down link"], z17["plunge entry"], z17["keep-down link"])
        if drops:
            got["D20 lift-over median drop"] = round(float(np.median(drops)), 2); got["D20 lift-over median vol"] = round(float(np.median(vols)), 3)
            got["D20 meeting line"] = ml
        if fx == "corne" and col == "zigzag_pick D20":
            I = [x["source_intent"] for x in S]
            got["old marks D20 Z17"] = sum(1 for i in range(len(S) - 1) if I[i] == "entry_plunge" and I[i + 1] != "entry_plunge" and abs(S[i]["position"][2] - 17.0) <= 0.35)
v3 = json.load(open(os.path.join(P3, "fix8", "fixture3d_v3_numbers.json")))
assert v3["fixtures"]["corne"]["arms"]["zigzag_pick D20"]["ops"]["3D Rough 1"]["entries"] == got["entries corne zigzag_pick D20"][0] == 60
for k, v in got.items():
    print(f"{k}: {v}")
json.dump(got, open(os.path.join(H, "assert_values.json"), "w"), indent=1, default=str)

# 5. README.md states each value.
readme = open(os.path.join(H, "README.md")).read() if os.path.exists(os.path.join(H, "README.md")) else ""
checks = 0
if readme:
    need = [f"{v:.1f} mm²" for k, v in got.items() if k.endswith(" inside") or k.endswith(" triage") or k.endswith(" outer band")]
    from PIL import Image
    w0 = Image.open(os.path.join(H, "img", "plane_corne_zigzag_pick_D20_0.png")).size[0]
    wc = Image.open(os.path.join(H, "img", "crop_corne_zigzag_pick_D20.png")).size[0]
    assert w0 >= 3 * 445
    need += [f"image is {w0} px wide, against 445 px for v3", f"The crop image is {wc} px wide"]
    need += [f"{got[f'corne {c} Z17 cut r0-25']:.3f} / {got[f'corne {c} Z17 cut r25-50']:.3f}" for c in CORNE]
    need += [f"{got[f'corne {c} r50 over']:.3f}" for c in CORNE]
    need += [f"{got['no-part share r<35']:.2f}", f"{got['plate top r<35']:.2f} mm", f"{got['nearest above Z17']:.2f} mm",
             f"{got['wanaka gap p1']:.2f} mm", "25.45 mm", got["sim_keys"]]
    need += [f"({got[f'crop {fx}'][0]:.1f}, {got[f'crop {fx}'][1]:.1f}) on Z {got[f'crop {fx}'][2]:.2f}" for fx in ("corne", "wanaka200")]
    for c in ("contour_parallel", "zigzag_pick D20", "zigzag_pick D18"):
        m_, sh = got[f"corne {c} annulus"]
        need.append(f"mean {m_:.3f} mm, {sh:.3f} of the cells over 0.8 mm")
    a20, a18 = got["arcs zigzag_pick D20"], got["arcs zigzag_pick D18"]
    assert got["arcs contour_parallel"]["ring"][0] == 0 and got["arcs contour_parallel"]["in"][0] == 0
    need += [f"D20 cuts {a20['ring'][0]} arcs, {a20['ring'][1]:.1f} mm long", f"as {a20['ring'][2]:.1f} mm of chords",
             f"D18 cuts {a18['ring'][0]} arcs, {a18['ring'][1]:.1f} mm\n  long, drawn as {a18['ring'][2]:.1f} mm".replace("\\n", "\n"),
             f"(D20 {a20['in'][0]}\n  arcs, {a20['in'][1]:.1f} mm drawn as {a20['in'][2]:.1f} mm)".replace("\\n", "\n")]
    names = {"corne": "Corne", "wanaka200": "wanaka200"}
    for fx, cols in (("corne", CORNE), ("wanaka200", WAN)):
        for c in cols:
            t, pd, kd, zp, zk = got[f"entries {fx} {c}"]
            last = f"{zp} / {zk}" if fx == "corne" else "—"
            need.append(f"| {names[fx]} {c} | {t} | {pd} | {kd} | {last} |")
    need += [f"That is {got['old marks D20 Z17']} marks", f"median drop of {got['D20 lift-over median drop']:.2f} mm",
             f"median of {got['D20 lift-over median vol']:.3f} mm³", f"On Corne D20, {got['D20 meeting line']} of them are on the meeting line"]
    for n in need:
        assert n in readme, ("README.md does not state", n)
        checks += 1
    assert got["law radius in P2_C2"] and not any(got[f"wanaka {c} island line"] for c in WAN)
    checks += 2
    print(f"fix9: all {checks} README checks hold")
