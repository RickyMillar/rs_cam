"""C18/fix10: the images of fixture3d_v4.html (from fix9/render.py, sized for a web page).

Copied from phase3/fix9/render.py. Changes: images below 1600 px wide and
quantised to a 256-colour palette; short titles (the page gives the counts);
the counts of every image are written to render_v4_numbers.json. The stock
maps read phase3/fix9/stock_*.npz (fix9/stockmap.py makes them) and the
entry marks read phase3/fix9/entries.json (fix9/entries.py).

fix9 text follows.

1. stockmap_<fixture>.png: the rebuilt stock after the first rough op
   (stockmap.py), as the height above the target, one shared colour scale per
   part (0 to one depth per pass), the part outline, the standing islands of
   the coverage triage (full D20b lists for D18 and D20 on Corne, the CLI's
   listed islands for the others), and on Corne the D20 seed with the
   25.45 mm radius of the C24 spacing law.
2. plane_<fixture>_<column>_<k>.png: one image per strict plane: only the
   sample steps whose two ends lie within PLANE_TOL of the plane Z.
3. drape_<fixture>_<column>.png: every cut step that lies on no plane,
   coloured by its Z.
4. crop_<fixture>_<column>.png: a 10 mm square at the same place for every
   column, on the middle plane, with every cut step whose Z is within one
   depth per pass above the plane.
All in the sim frame, from simulation.json (one sample per 0.3 mm on Corne,
1.0 mm on wanaka200). Writes crop_centres.json."""
import json, math, os, re
import numpy as np
from scipy import ndimage
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib.collections import LineCollection

H = os.path.dirname(os.path.abspath(__file__)); P3 = os.path.join(H, "..")
OUT = os.path.join(H, "img"); os.makedirs(OUT, exist_ok=True)
PLANE_TOL = 0.01
BLUE, ORANGE, RED = "#1f5fbf", "#e07000", "#d00000"
FX = {
    "corne": {"planes": [17.0, 11.0, 5.0, 0.5], "show": [17.0, 5.0, 0.5], "dpp": 6.0,
              "cols": {"contour_parallel": ("fix6/corne/contour_parallel", "fix6/corne/contour_parallel"),
                       "zigzag_pick D18": ("fix5/corne/geodesic_local_zigzag_pick", "fix5/corne/geodesic_local_zigzag_pick"),
                       "zigzag_pick D20": ("fix7/corne/geodesic_local_zigzag_pick", "fix7/corne/geodesic_local_zigzag_pick"),
                       "AgentSearch": ("fix1/corne/agent", "fix2/simart/corne/agent")}},
    "wanaka200": {"planes": [19.54, 14.08, 8.62, 4.0], "show": [19.54, 14.08, 8.62], "dpp": 5.46,
                  "cols": {"contour_parallel": ("fix6/wanaka200/contour_parallel", "fix6/wanaka200/contour_parallel"),
                           "zigzag_pick D18": ("fix2/wanaka200/geodesic_local_zigzag", "fix2/simart/wanaka200/geodesic_local_zigzag"),
                           "zigzag_pick D20": ("fix7/wanaka200/geodesic_local_zigzag_pick", "fix7/wanaka200/geodesic_local_zigzag_pick"),
                           "AgentSearch": ("fix2/wanaka200/agent", "fix2/simart/wanaka200/agent")}},
}
FIX9 = os.path.join(H, "..", "fix9")
ENTRIES = json.load(open(os.path.join(FIX9, "entries.json")))
NUM = {}


def save(fig, name, dpi, tight=True):
    from PIL import Image
    path = os.path.join(OUT, name)
    fig.savefig(path, dpi=dpi, bbox_inches="tight" if tight else None)
    plt.close(fig)
    im = Image.open(path).convert("RGB")
    assert im.size[0] < 1600, (name, im.size)
    im.quantize(colors=256, method=Image.Quantize.FASTOCTREE).save(path, optimize=True)
SEED = (55.866, -49.762)   # the D20 Corne first plunge (sim frame = model frame)
LAW_R = 25.45              # P2_C2_geodesic_fixes.md: rho* at stepover 1.2


def tag(s):
    return re.sub(r"[^A-Za-z0-9]+", "_", s).strip("_")


def steps(simdir):
    """Consecutive sample pairs of toolpath 1 that the tool feeds between: both
    samples fed (not rapid kinematics) and at most 1.5 sample steps apart in XY
    (so a move boundary leaves no gap)."""
    S = [s for s in json.load(open(os.path.join(P3, simdir, "simulation.json")))["trace"]["samples"] if s["toolpath_id"] == 1]
    P = np.array([s["position"] for s in S]); M = np.array([s["move_index"] for s in S])
    I = np.array([s["source_intent"] for s in S]); K = np.array([s["cut_kinematics"] for s in S])
    step = 0.3 if "corne" in simdir else 1.0
    fed = K != "rapid"
    same = fed[1:] & fed[:-1] & (np.hypot(*(P[1:, :2] - P[:-1, :2]).T) <= 1.5 * step + 1e-9)
    return P[:-1][same], P[1:][same], I[1:][same], K[1:][same], P, I


def islands(fx, col, rundir):
    if fx == "corne" and col in ("zigzag_pick D18", "zigzag_pick D20"):
        f = "islands_d18.json" if col.endswith("D18") else "islands_d20.json"
        return json.load(open(os.path.join(P3, "fix7", f)))["3D Rough 1"]["islands"], "full list (D20b)"
    err = open(os.path.join(P3, rundir, "stderr.txt")).read()
    op = "3D Rough 1" if fx == "corne" else "2 Back Rough"
    for ln in err.splitlines():
        if f'"{op}": the rough leaves' in ln:
            isl = re.findall(r" (\d+\.\d+) mm high, (\d+\.\d+) mm wide, (\d+\.\d+) mm2 at \((-?\d+\.\d+), (-?\d+\.\d+)\)", ln)
            return [{"height_mm": float(h), "area_mm2": float(a), "x": float(x), "y": float(y)} for h, w, a, x, y in isl], "CLI list (first 10)"
    return [], "no island line"


def stockmaps():
    for fx, cfg in FX.items():
        fig, axs = plt.subplots(2, 2, figsize=(16, 12.5), constrained_layout=True)
        for ax, (col, (rundir, simdir)) in zip(axs.flat, cfg["cols"].items()):
            m = np.load(os.path.join(FIX9, f"stock_{fx}_{tag(col)}.npz"))
            a = m["top"] - m["target"]; xs, ys = m["xs"], m["ys"]
            if fx == "corne":
                inside = ndimage.binary_fill_holes(m["surf"] > 0)
            else:
                X, Y = np.meshgrid(xs, ys); inside = (X >= 20) & (X <= 220) & (Y >= 25) & (Y <= 225)
            show = np.ma.masked_where(~inside, np.clip(a, 0, None))
            cm = plt.get_cmap("magma_r").copy(); cm.set_bad("#d8d8d8")
            im = ax.imshow(show, origin="lower", extent=(xs[0], xs[-1], ys[0], ys[-1]), cmap=cm, vmin=0, vmax=cfg["dpp"], interpolation="nearest")
            ax.contour(xs, ys, inside.astype(float), levels=[0.5], colors="#3060a0", linewidths=0.6)
            isl, src = islands(fx, col, rundir)
            if isl:
                ax.scatter([i["x"] for i in isl], [i["y"] for i in isl], s=[4 + 0.4 * i["area_mm2"] for i in isl],
                           facecolors="none", edgecolors="#00a0a0", linewidths=0.6)
            if fx == "corne":
                ax.add_patch(plt.Circle(SEED, LAW_R, fill=False, ls="--", lw=0.8, color="#2a8a2a"))
                ax.plot(*SEED, "+", color="#2a8a2a", ms=8)
            over = np.sum((a > 0.8) & inside) * float(m["g"]) ** 2
            ax.set_title(f"{col} (islands: {src})", fontsize=11)
            ax.set_aspect("equal"); ax.tick_params(labelsize=8)
        fig.colorbar(im, ax=axs, shrink=0.6, label="stock above target after the first rough op (mm); grey: outside the part", extend="max")
        save(fig, f"stockmap_{fx}.png", 95, tight=False)


def level_images():
    centres = {}
    for fx, cfg in FX.items():
        data = {col: steps(sd) for col, (rd, sd) in cfg["cols"].items()}
        allp = np.vstack([d[4] for d in data.values()])
        frame = (allp[:, 0].min() - 3, allp[:, 0].max() + 3, allp[:, 1].min() - 3, allp[:, 1].max() + 3)
        planes = np.array(cfg["planes"])
        mid = 8.62 if fx == "wanaka200" else cfg["show"][1]   # the crop plane (fix9's)
        # crop centre: the 10 mm square with the most strict-plane cut at the middle plane in its poorest column
        best = None
        for cx in np.arange(frame[0] + 10, frame[1] - 10, 2.5):
            for cy in np.arange(frame[2] + 10, frame[3] - 10, 2.5):
                v = []
                for col, (a, b, it, k, P, I) in data.items():
                    on = (np.abs(a[:, 2] - mid) <= PLANE_TOL) & (np.abs(b[:, 2] - mid) <= PLANE_TOL) & (it == "clearing_cut")
                    inn = on & (np.abs(a[:, 0] - cx) <= 5) & (np.abs(a[:, 1] - cy) <= 5)
                    v.append(np.hypot(*(b[inn, :2] - a[inn, :2]).T).sum())
                if best is None or min(v) > best[0]:
                    best = (min(v), float(cx), float(cy))
        centres[fx] = {"x": best[1], "y": best[2], "plane": mid, "min_cut_mm": round(best[0], 1)}
        for col, (a, b, it, k, P, I) in data.items():
            on_any = np.zeros(len(a), bool)
            for z in planes:
                on_any |= (np.abs(a[:, 2] - z) <= PLANE_TOL) & (np.abs(b[:, 2] - z) <= PLANE_TOL)
            for kk, z in enumerate(cfg["show"]):
                on = (np.abs(a[:, 2] - z) <= PLANE_TOL) & (np.abs(b[:, 2] - z) <= PLANE_TOL)
                fig, ax = plt.subplots(figsize=(10, 10 * (frame[3] - frame[2]) / (frame[1] - frame[0]) + 0.5))
                for cls, col_, lw in (("clearing_cut", BLUE, 0.25), ("linking", ORANGE, 0.5)):
                    sel = on & (it == cls)
                    ax.add_collection(LineCollection(np.stack([a[sel, :2], b[sel, :2]], 1), colors=col_, linewidths=lw))
                # Entries (entries.py): a red ring where a plunge-entry span of the trace
                # ends its descent on this plane (the G2 page's plunge count); a small
                # orange dot where a keep-down-link span descends 1 mm or more into
                # stock and ends on this plane (the lift-over links).
                er = ENTRIES[f"{fx} {col}"]["rows"]
                pe = [r for r in er if r["style"] == "plunge entry" and r["end_plane"] == z]
                ko = [r for r in er if r["style"] == "keep-down link" and r["end_plane"] == z and r["descent_mm"] >= 1.0 and r["removed_mm3"] > 0.1]
                if ko:
                    ax.scatter([r["x"] for r in ko], [r["y"] for r in ko], s=9, color=ORANGE, zorder=5)
                if pe:
                    ax.scatter([r["x"] for r in pe], [r["y"] for r in pe], s=40, facecolors="none", edgecolors=RED, linewidths=1.2, zorder=6)
                ax.add_patch(plt.Rectangle((centres[fx]["x"] - 5, centres[fx]["y"] - 5), 10, 10, fill=False, color="#2a8a2a", lw=0.8))
                ax.set_xlim(frame[0], frame[1]); ax.set_ylim(frame[2], frame[3]); ax.set_aspect("equal")
                cut = np.hypot(*(b[on & (it == 'clearing_cut'), :2] - a[on & (it == 'clearing_cut'), :2]).T).sum()
                ax.set_title(f"{col}: strict plane Z {z:.2f}", fontsize=11)
                NUM[f"plane_{fx}_{tag(col)}_{kk}"] = {"z": z, "cut_mm": round(float(cut), 3), "plunge_entries": len(pe), "lift_over": len(ko)}
                save(fig, f"plane_{fx}_{tag(col)}_{kk}.png", 140)
            dr = ~on_any & (it == "clearing_cut") & (np.hypot(*(b[:, :2] - a[:, :2]).T) > 1e-9)
            fig, ax = plt.subplots(figsize=(10, 10 * (frame[3] - frame[2]) / (frame[1] - frame[0]) + 0.5))
            lc = LineCollection(np.stack([a[dr, :2], b[dr, :2]], 1), array=(a[dr, 2] + b[dr, 2]) / 2, cmap="viridis", linewidths=0.35)
            ax.add_collection(lc); fig.colorbar(lc, ax=ax, shrink=0.7, label="Z of the cut step (mm)")
            ax.set_xlim(frame[0], frame[1]); ax.set_ylim(frame[2], frame[3]); ax.set_aspect("equal")
            ax.set_title(f"{col}: drape band (cut steps on no plane)", fontsize=11)
            NUM[f"drape_{fx}_{tag(col)}"] = {"cut_mm": round(float(np.hypot(*(b[dr, :2] - a[dr, :2]).T).sum()), 3)}
            save(fig, f"drape_{fx}_{tag(col)}.png", 140)
            c = centres[fx]
            near = (it == "clearing_cut") & (a[:, 2] >= c["plane"] - PLANE_TOL) & (a[:, 2] <= c["plane"] + cfg["dpp"])
            fig, ax = plt.subplots(figsize=(6, 6))
            lc = LineCollection(np.stack([a[near, :2], b[near, :2]], 1), array=a[near, 2], cmap="viridis", linewidths=1.0,
                                norm=matplotlib.colors.Normalize(c["plane"], c["plane"] + cfg["dpp"]))
            ax.add_collection(lc)
            ax.set_xlim(c["x"] - 5, c["x"] + 5); ax.set_ylim(c["y"] - 5, c["y"] + 5); ax.set_aspect("equal"); ax.grid(lw=0.3)
            ax.set_title(f"{col}: 10 mm crop", fontsize=10)
            fig.colorbar(lc, ax=ax, shrink=0.8, label="Z (mm)")
            save(fig, f"crop_{fx}_{tag(col)}.png", 130)
    json.dump(centres, open(os.path.join(H, "crop_centres.json"), "w"), indent=1)
    json.dump(NUM, open(os.path.join(H, "render_v4_numbers.json"), "w"), indent=1)


def compare_d18_d20():
    """Corne Z 17 strict plane, D18 (the run of the v2 page's zigzag_pick
    column, fix5) and D20 side by side, same frame and scale."""
    cols = {"zigzag_pick D18 (= the v2 page's pick column, fix5)": "fix5/corne/geodesic_local_zigzag_pick",
            "zigzag_pick D20 (fix7)": "fix7/corne/geodesic_local_zigzag_pick"}
    fig, axs = plt.subplots(1, 2, figsize=(22, 9), constrained_layout=True)
    for ax, (lab, sd) in zip(axs, cols.items()):
        a, b, it, k, P, I = steps(sd)
        on = (np.abs(a[:, 2] - 17.0) <= PLANE_TOL) & (np.abs(b[:, 2] - 17.0) <= PLANE_TOL)
        for cls, c_, lw in (("clearing_cut", BLUE, 0.3), ("linking", ORANGE, 0.5)):
            sel = on & (it == cls)
            ax.add_collection(LineCollection(np.stack([a[sel, :2], b[sel, :2]], 1), colors=c_, linewidths=lw))
        ax.add_patch(plt.Circle(SEED, LAW_R, fill=False, ls="--", lw=0.8, color="#2a8a2a"))
        ax.add_patch(plt.Circle(SEED, 50.0, fill=False, ls=":", lw=0.8, color="#2a8a2a"))
        ax.set_xlim(0, 154); ax.set_ylim(-110, 3); ax.set_aspect("equal"); ax.set_title(f"{lab}: strict plane Z 17.00", fontsize=11)
    save(fig, "compare_corne_D18_D20_Z17.png", 70, tight=False)


if __name__ == "__main__":
    stockmaps()
    level_images()
    compare_d18_d20()
