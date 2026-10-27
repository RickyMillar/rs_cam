"""D49: a copy of fix33/render_d47.py (writes into fix34). D49 changes: RUNS by set (argv 3: wanaka | corne); the level window is ±0.005 mm, not ±0.05 (D47 item 5: the wide window showed false near-duplicate levels). D47: a copy of fix32/render_d45.py. Changes: RUNS; the op-name pattern also takes "2 Back Rough"; the colour range is wanaka's Z. Lead/D45: toolpath renders from program.nc (G-code frame), per op.

For each run and op: a top view of every G1 move coloured by Z, with each
vertical feed descent (a plunge) as a black dot and each G0 as a thin grey
line. Also one image per op-1 level (moves whose two ends lie within 0.05 mm
of that Z). The sims are deleted, so this reads only the G-code."""
import os, re, sys
import numpy as np
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib.collections import LineCollection

H = os.path.dirname(os.path.abspath(__file__)); P3 = os.path.join(H, "..")
OUT = os.path.join(H, "img")
SETS = {"wanaka": {"wanaka geo s3.5 D47 (7bf7738a1)": "fix33/s3p5", "wanaka geo s3.5 D49 (453d52571)": "fix34/wanaka_s3p5"},
        "corne": {"Corne geo s2.1 B D45 (7bf7738a1)": "fix31/B_s2p1", "Corne geo s2.1 B D49 (453d52571)": "fix34/corne_B_s2p1"}}
SET = sys.argv[3] if len(sys.argv) > 3 else "wanaka"
RUNS = SETS[SET]
ZLIM = (float(sys.argv[1]), float(sys.argv[2])) if len(sys.argv) > 2 else (-18.0, 0.0)


def parse(path):
    ops, cur, x, y, z = [], None, 0.0, 0.0, 5.0
    for line in open(path):
        line = line.strip()
        m = re.match(r"^\(((?:3D|2 Back) Rough[^)]*)\)$", line)
        if m:
            cur = {"name": m.group(1), "g0": [], "g1": [], "plunge": []}; ops.append(cur); continue
        if cur is None or not line or line[0] != "G":
            continue
        g = line.split()[0]
        nx, ny, nz = x, y, z
        for k, v in re.findall(r"([XYZ])(-?[\d.]+)", line):
            if k == "X": nx = float(v)
            elif k == "Y": ny = float(v)
            else: nz = float(v)
        if g == "G0":
            cur["g0"].append((x, y, z, nx, ny, nz))
        elif g == "G1":
            if abs(nx - x) < 1e-6 and abs(ny - y) < 1e-6 and nz < z:
                cur["plunge"].append((nx, ny))
            else:
                cur["g1"].append((x, y, z, nx, ny, nz))
        x, y, z = nx, ny, nz
    return ops


def draw(ax, op, zsel=None, rapids=True):
    g1 = np.array(op["g1"]) if op["g1"] else np.zeros((0, 6))
    if zsel is not None and len(g1):
        g1 = g1[(abs(g1[:, 2] - zsel) < 0.005) & (abs(g1[:, 5] - zsel) < 0.005)]
    if rapids and op["g0"]:
        g0 = np.array(op["g0"])
        ax.add_collection(LineCollection(g0[:, [0, 1, 3, 4]].reshape(-1, 2, 2), colors="#bbbbbb", linewidths=0.3, zorder=1))
    if len(g1):
        lc = LineCollection(g1[:, [0, 1, 3, 4]].reshape(-1, 2, 2), array=(g1[:, 2] + g1[:, 5]) / 2,
                            cmap="viridis", linewidths=0.6, zorder=2)
        lc.set_clim(ZLIM[0], ZLIM[1]); ax.add_collection(lc)
    if op["plunge"] and zsel is None:
        p = np.array(op["plunge"]); ax.scatter(p[:, 0], p[:, 1], s=3, c="k", zorder=3)
    ax.set_aspect("equal"); ax.autoscale()
    return len(g1)


def main():
    rows = []
    for label, d in RUNS.items():
        ops = parse(os.path.join(P3, d, "program.nc"))
        tag = d.split("/")[-1]
        for i, op in enumerate(ops):
            fig, ax = plt.subplots(figsize=(9, 7))
            draw(ax, op)
            ax.set_title(f"{label}: {op['name']}  ({len(op['g1'])} cut moves, {len(op['plunge'])} plunges, {len(op['g0'])} rapids)", fontsize=9)
            fn = f"{SET}_{d.split('/')[0]}_{tag}_op{i}.png"; fig.savefig(os.path.join(OUT, fn), dpi=110, bbox_inches="tight"); plt.close(fig)
            rows.append((label, op["name"], fn))
            if i == 0:
                zs = sorted({round(s[2], 2) for s in op["g1"] if abs(s[2] - s[5]) < 1e-6}, reverse=True)
                counts = {z: sum(1 for s in op["g1"] if abs(s[2] - z) < 0.005 and abs(s[5] - z) < 0.005) for z in zs}
                levels = [z for z in zs if counts[z] > 200][:6]
                fig, axs = plt.subplots(1, len(levels), figsize=(4.2 * len(levels), 3.6))
                for ax, z in zip(np.atleast_1d(axs), levels):
                    n = draw(ax, op, zsel=z, rapids=False); ax.set_title(f"Z {z}  ({n} moves)", fontsize=8); ax.set_xticks([]); ax.set_yticks([])
                fn = f"{SET}_{d.split('/')[0]}_{tag}_op0_levels.png"; fig.suptitle(f"{label}: op 1 by level", fontsize=10)
                fig.savefig(os.path.join(OUT, fn), dpi=100, bbox_inches="tight"); plt.close(fig)
                rows.append((label, "op 1 by level", fn))
    with open(os.path.join(H, f"index_{SET}.html"), "w") as f:
        f.write("<!doctype html><meta charset=utf-8><title>D47 renders</title><body style='font-family:sans-serif;background:#fff'>")
        f.write("<h2>D49 toolpath renders ( from program.nc; colour = Z, black dot = plunge, grey = rapid)</h2>")
        for label, name, fn in rows:
            f.write(f"<h3>{label}: {name}</h3><img src='img/{fn}' style='max-width:100%'>")
    print("\n".join(r[2] for r in rows))


main()
