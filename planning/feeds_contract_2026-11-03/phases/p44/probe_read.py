"""P44 instrument: read the output of the print-only probe build (probe1.patch, env P44PROBE).

The probe writes, per call of `detect_and_order_regions` (one level of one By Area job, or one
level of a Global op), one byte per planner cell:
  1  material cell: stock stands above the level floor (`cell_has_material`)
  2  the By Area job of the call owns the cell (or no mask)
  4  in the grid after the one-pass hold (`OnePass::hold_off`)
  8  in the grid after the rest region filter
  16 in the grid after the region clip (the grid that marching squares reads)
and text lines: `level`, `region` (kept or small, with the bound), `forecast` (F-038, with the
length and the bound), `planned` (cuts after the sweep), `nofield`, `groups`, and at the end
of the op `final` with the planner stock top and the tool-centre floor (f32 per cell).

This script gives, per op (by tool radius), the class of each cell where the planner stock
stands more than BAR above the floor of the plan at the end of the op: the class is the stage
that stopped the cell at the LAST level call where it was a material cell.
A cell is a tool-centre position. So each cell here is a place where the tool fits.

usage: probe_read.py <probe dir> <radius> [--bottom z] [--npy out.npy]
"""
import sys, os, re, collections
import numpy as np
from scipy import ndimage

BAR = 0.3
OFF = (18.5, 8.5)
PLACES = [  # G-code frame windows, as places.py
    ("1 antenna slot",   (243.5, 291.5, 54.0, 63.0)),
    ("2 control relief", (174.8, 190.2, 51.2, 65.8)),
    ("3 slit slot",      (48.7, 346.3, 91.5, 98.5)),
    ("4 ESP relief",     (138.5, 159.5, 39.0, 64.6)),
    ("5 step shelf",     (42.7, 352.0, 104.0, 118.5)),
    ("6 board pocket",   (46.7, 348.3, 41.0, 104.0)),
    ("7 cavity",         (43.0, 352.0, 118.5, 427.0)),
]
CLASSES = ["not a material cell at any call", "by area: no job call owns the cell at its last level", "one-pass hold (P41)",
           "rest region filter", "region clip (boundary)", "contour of 1 cell2 or less (no region)",
           "small-region bound", "F-038 forecast", "planned, 0 cuts (no field / all air)", "planned and cut, stock still stands"]


def parse(d, radius):
    levels = {}; cur = None
    for line in open(os.path.join(d, "probe.log")):
        w = line.split()
        if w[0] == "level":
            kv = dict(zip(w[1::2], w[2::2])) if False else None
            seq = int(w[2]); r = float(w[4])
            rec = dict(seq=seq, r=r, z=float(w[6]), job=int(w[8]), rows=int(w[10]), cols=int(w[12]),
                       ox=float(w[14]), oy=float(w[15]), cell=float(w[17]), regions=[], forecast=[], planned=[], nofield=[], groups=[])
            levels[seq] = rec
        elif w[0] in ("region", "forecast", "planned", "nofield", "groups"):
            seq = int(w[2])
            if seq not in levels: continue
            i = w.index("bbox"); bb = tuple(float(v) for v in w[i + 1:i + 5])
            rec = levels[seq]
            if w[0] == "region":
                rec["regions"].append(dict(kind=w[3], area=float(w[5]), bound=float(w[7]), bbox=bb))
            elif w[0] == "forecast":
                rec["forecast"].append(dict(area=float(w[6]), forecast=float(w[8]), bound=float(w[10]), drop=w[12] == "true", bbox=bb))
            elif w[0] == "planned":
                rec["planned"].append(dict(area=float(w[6]), grown=w[8] == "true", outlines=int(w[10]), cuts=int(w[12]), walk=float(w[14]), bbox=bb))
            elif w[0] == "nofield":
                rec["nofield"].append(dict(area=float(w[6]), bbox=bb))
            else:
                rec["groups"].append(dict(dropped=int(w[6]), total=int(w[8]), bbox=bb))
        elif w[0] == "final" and abs(float(w[2]) - radius) < 1e-6:
            fin = dict(rows=int(w[4]), cols=int(w[6]), ox=float(w[8]), oy=float(w[9]), cell=float(w[11]))
    return {s: v for s, v in levels.items() if abs(v["r"] - radius) < 1e-6}, fin


def near(bb, items):
    best = None
    for it in items:
        e = sum(abs(a - b) for a, b in zip(bb, it["bbox"]))
        if best is None or e < best[0]: best = (e, it)
    return best


def classify(d, radius):
    levels, fin = parse(d, radius)
    rows, cols, cell = fin["rows"], fin["cols"], fin["cell"]
    cls = np.zeros((rows, cols), np.int8); lastz = np.full((rows, cols), np.nan, np.float32)
    info = {}
    order = sorted(levels)           # the call order
    # for By Area: the stage of a cell is taken from the last call (in call order) where it was a material cell
    # AND owned; a cell that no call owns keeps class 1 from its last material call.
    for seq in order:
        L = levels[seq]
        b = np.fromfile(os.path.join(d, "r%.1f_n%04d.bin" % (radius, seq)), np.uint8).reshape(rows, cols)
        mat = (b & 1) > 0; own = (b & 2) > 0
        c = np.zeros((rows, cols), np.int8)
        c[mat & ~own] = 1
        c[mat & own & ((b & 4) == 0)] = 2
        c[mat & own & ((b & 4) > 0) & ((b & 8) == 0)] = 3
        c[mat & own & ((b & 8) > 0) & ((b & 16) == 0)] = 4
        g = (b & 16) > 0
        lab, n = ndimage.label(g, structure=np.ones((3, 3)))
        sl = ndimage.find_objects(lab)
        for k in range(n):
            s = sl[k]
            # a cell centre is origin + index * cell (`cell_to_world`); the marching squares
            # outline stands half a cell outside the outer cell centres
            bb = (L["ox"] + (s[1].start - 0.5) * cell, L["oy"] + (s[0].start - 0.5) * cell,
                  L["ox"] + (s[1].stop - 0.5) * cell, L["oy"] + (s[0].stop - 0.5) * cell)
            m = lab[s] == (k + 1)
            hit = near(bb, L["regions"])
            code = 5; why = None
            if hit and hit[0] < 6 * cell:
                reg = hit[1]
                if reg["kind"] == "small":
                    code = 6; why = reg
                else:
                    f = near(reg["bbox"], L["forecast"]); p = near(reg["bbox"], L["planned"])
                    if f and f[0] < 1e-6 and f[1]["drop"]:
                        code = 7; why = f[1]
                    elif p and p[0] < 1e-6:
                        code = 8 if p[1]["cuts"] == 0 else 9; why = p[1]
                    else:
                        code = 9
            c[s][m] = code
            info[(seq, k + 1)] = (code, bb, m.sum() * cell * cell, why)
        upd = mat & (own | (cls <= 1))
        cls[upd] = c[upd]; lastz[upd] = L["z"]
    top = np.fromfile(os.path.join(d, "r%.1f_stocktop.f32" % radius), np.float32).reshape(rows, cols)
    floor = np.fromfile(os.path.join(d, "r%.1f_floor.f32" % radius), np.float32).reshape(rows, cols)
    return levels, fin, cls, lastz, top, floor, info


if __name__ == "__main__":
    d, radius = sys.argv[1], float(sys.argv[2])
    bottom = float(sys.argv[sys.argv.index("--bottom") + 1]) if "--bottom" in sys.argv else None
    levels, fin, cls, lastz, top, floor, info = classify(d, radius)
    rows, cols, cell = fin["rows"], fin["cols"], fin["cell"]
    zs = sorted({round(v["z"], 3) for v in levels.values()}, reverse=True)
    zb = min(zs) if bottom is None else bottom
    fl = np.maximum(floor, zb)
    wood = (top - fl) > BAR
    X = fin["ox"] + np.arange(cols) * cell + OFF[0]; Y = (fin["oy"] + np.arange(rows) * cell + OFF[1])[:, None]
    print(f"tool radius {radius}: {len(levels)} level calls, levels {zs}, plan bottom {zb}, cell {cell} mm")
    print(f"tool-centre cells where the planner stock stands more than {BAR} mm above the plan floor at the end: {wood.sum()*cell*cell:.1f} mm2")
    used = np.zeros((rows, cols), bool); masks = []
    for name, (x0, x1, y0, y1) in PLACES:
        m = (X >= x0) & (X <= x1) & (Y >= y0) & (Y <= y1)
        if name.startswith("6"): m = m & ~used
        else: used |= m
        masks.append((name, m))
    masks.append(("other", ~np.any([m for _, m in masks], axis=0)))
    hdr = "%-26s" % "place" + "".join("%9s" % ("c%d" % i) for i in range(len(CLASSES))) + "%10s" % "total"
    print("classes: " + "; ".join("c%d = %s" % (i, c) for i, c in enumerate(CLASSES)))
    print(hdr)
    tot = np.zeros(len(CLASSES))
    for name, m in masks:
        row = [((cls == i) & wood & m).sum() * cell * cell for i in range(len(CLASSES))]
        tot += row
        print("%-26s" % name + "".join("%9.1f" % v for v in row) + "%10.1f" % sum(row))
    print("%-26s" % "all" + "".join("%9.1f" % v for v in tot) + "%10.1f" % tot.sum())
    # the last level of the wood cells, per class
    print("\nwood cells by class and by the Z of their last material call (mm2):")
    for i in range(1, len(CLASSES)):
        sel = wood & (cls == i)
        if not sel.any(): continue
        zz = collections.Counter(np.round(lastz[sel], 3).tolist())
        print("  c%d %s: " % (i, CLASSES[i]) + ", ".join("z %.3f: %.1f" % (z, n * cell * cell) for z, n in sorted(zz.items(), reverse=True)))
    if "--npy" in sys.argv:
        np.savez_compressed(sys.argv[sys.argv.index("--npy") + 1], cls=cls, lastz=lastz, top=top, floor=fl, wood=wood,
                            ox=fin["ox"], oy=fin["oy"], cell=cell)
    if "--regions" in sys.argv:
        # each dropped region (small, forecast, 0 cuts) of each call, with its numbers, in the G-code frame
        print("\ndropped regions per call (G-code frame bbox x0 y0 x1 y1):")
        for (seq, k), (code, bb, area, why) in sorted(info.items()):
            if code in (5, 6, 7, 8):
                L = levels[seq]
                g = (bb[0] + OFF[0], bb[1] + OFF[1], bb[2] + OFF[0], bb[3] + OFF[1])
                extra = ""
                if why and code == 6: extra = "poly area %.1f bound %.1f" % (why["area"], why["bound"])
                if why and code == 7: extra = "poly area %.1f forecast %.2f bound %.1f" % (why["area"], why["forecast"], why["bound"])
                if why and code == 8: extra = "poly area %.1f grown %s outlines %d" % (why["area"], why["grown"], why["outlines"])
                print("  z %.3f job %d c%d cells %.1f mm2 bbox %.1f %.1f %.1f %.1f %s" % (L["z"], L["job"], code, area, *g, extra))
