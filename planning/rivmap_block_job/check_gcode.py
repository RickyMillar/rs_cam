"""Check the emitted G-code of the RivMap block job against the block-stage tables.

Run:  uv run --with ezdxf --with numpy --with scipy --with shapely \
          python planning/rivmap_block_job/check_gcode.py <project.toml> <emitted.nc> [out.json]

The G-code is measurement only (the CLI `project --emit-gcode` output). The
script reads the toolpath names from the project, splits the program by the
toolpath comment lines, and measures the emitted motion:

1. V accuracy: emitted tip Z of the V "final" passes at each CSV vertex
   (vgroove_paths.csv) against T1 - depth; the implied front line width;
   the XY distance from each CSV vertex to the emitted path.
2. V-bit body: the cone top (tip + cone) against the highest wood under the
   body disc after the setup-1 roughing (slot floors, cavity ceiling).
3. Web: setup-3 feed motion over the cavity footprint (+ tool radius) stays
   at or above block Z cavity_depth + web_min.

Every assertion prints its numbers. The script exits non-zero if one fails.
"""

from __future__ import annotations

import csv
import json
import math
import re
import sys
import tomllib
from pathlib import Path

import ezdxf
from ezdxf import path as ezpath
import numpy as np
from scipy.spatial import cKDTree
from shapely import contains_xy
from shapely.geometry import Point, Polygon
from shapely.prepared import prep

MONO = Path("/home/ricky/personal_repos/project_rivmap_mono/manufacture")
CNC = Path("/home/ricky/cnc_jobs/rivmap_block/nz_south")


def parse_program(nc: Path, names: set[str]):
    """{toolpath name: list of (kind, x0,y0,z0, x1,y1,z1)} with modal state."""
    segs: dict[str, list] = {}
    cur = None
    x = y = z = None
    word = re.compile(r"([GXYZF])(-?[0-9.]+)")
    arcs = 0
    for raw in nc.read_text().splitlines():
        line = raw.strip()
        m = re.fullmatch(r"\((.*)\)", line)
        if m and m.group(1) in names:
            cur = m.group(1)  # the emitter writes "(" ")" in a name as "[" "]"; the derive names have none
            segs.setdefault(cur, [])
            continue
        if not line or line.startswith("("):
            continue
        g = None
        nx, ny, nz = x, y, z
        for k, v in word.findall(line.split("(")[0]):
            if k == "G":
                g = int(float(v)) if g is None else g
            elif k == "X":
                nx = float(v)
            elif k == "Y":
                ny = float(v)
            elif k == "Z":
                nz = float(v)
        if g in (2, 3):
            arcs += 1
        if g in (0, 1, 2, 3) and None not in (x, y, z) and cur is not None:
            segs[cur].append((g, x, y, z, nx, ny, nz))
        x, y, z = nx, ny, nz
        if g in (0, 1, 2, 3):
            last_g = g
    return segs, arcs


def feed_points(seglist, step: float, lateral_only: bool = False):
    """Sample feed segments (G1) every `step` mm; returns (N,3).

    `lateral_only` drops the vertical legs (plunge and retract), which share the
    XY of a path end and would answer a nearest-XY query with a wrong Z."""
    pts = []
    for g, x0, y0, z0, x1, y1, z1 in seglist:
        if g == 0:
            continue
        if lateral_only and math.hypot(x1 - x0, y1 - y0) < 1e-6:
            continue
        L = math.dist((x0, y0, z0), (x1, y1, z1))
        n = max(1, int(math.ceil(L / step)))
        t = np.linspace(0.0, 1.0, n + 1)
        pts.append(np.column_stack([x0 + (x1 - x0) * t, y0 + (y1 - y0) * t, z0 + (z1 - z0) * t]))
    return np.vstack(pts) if pts else np.zeros((0, 3))


def even_odd(polys):
    geom = Polygon()
    for p in sorted(polys, key=lambda p: -p.area):
        geom = geom.symmetric_difference(p)
    return geom


def pct(a, qs=(0, 1, 5, 50, 95, 99, 100)):
    a = np.asarray(a)
    return {f"p{q}": round(float(np.percentile(a, q)), 4) for q in qs}


def main() -> int:
    proj_path, nc_path = Path(sys.argv[1]), Path(sys.argv[2])
    out_json = Path(sys.argv[3]) if len(sys.argv) > 3 else None
    proj = tomllib.loads(proj_path.read_text())
    stock = proj["job"]["stock"]
    T1 = stock["z"]
    ox, oy, D = stock["origin_x"], stock["origin_y"], stock["y"]
    setup_of = {tp["name"]: s["name"] for s in proj["setups"] for tp in s["toolpaths"]}
    tool_of = {tp["name"]: tp["tool_id"] for s in proj["setups"] for tp in s["toolpaths"]}
    tools = {t["id"]: t for t in proj["tools"]}
    segs, arcs = parse_program(nc_path, set(setup_of))
    blk = json.loads((MONO / "build/block/nz-south/block.json").read_text())
    mapt = tomllib.loads((MONO / "block/map.toml").read_text())
    ok = True
    res: dict = {"arcs_in_program": arcs, "toolpaths_found": sorted(segs)}
    missing = sorted(set(setup_of) - set(segs))
    res["toolpaths_missing"] = missing

    # setup-1 G-code XY = world - stock origin (identity setup, stock-relative)
    def s1_to_world(p):
        return np.column_stack([p[:, 0] + ox, p[:, 1] + oy, p[:, 2]])

    # ── 1. V accuracy (final passes)
    with open(MONO / "build/block/nz-south/vgroove_paths.csv") as f:
        rows = list(csv.DictReader(f))
    half = math.radians(blk["vgroove_angle"] / 2.0)
    overcut = (blk["line_width"] / 2.0) / math.tan(half)
    res["v"] = {}
    for kind, name in (("river", "V river final"), ("lake", "V lake final")):
        if name not in segs:
            continue
        sub = [r for r in rows if r["kind"] == kind]
        V = np.array([[float(r["x_dxf"]), float(r["y_dxf"]), T1 - float(r["depth"]),
                       T1 - float(r["surface_z"])] for r in sub])
        P = s1_to_world(feed_points(segs[name], 0.01, lateral_only=True))
        # keep the cutting points only (below the ceiling): the plunge/retract legs are vertical
        tree = cKDTree(P[:, :2])
        d, i = tree.query(V[:, :2])
        dz = P[i, 2] - V[:, 2]                     # + = tip higher (less past the front)
        past = V[:, 3] - P[i, 2]                   # how far the tip passes the front surface
        width = 2.0 * past * math.tan(half)
        # the reverse XY check: every emitted cutting point near a CSV path
        cut = P[P[:, 2] < T1 - blk["cavity_rough"] - 1e-6]
        dr, _ = cKDTree(V[:, :2]).query(cut[:, :2])
        r = {"vertices": len(V), "xy_vertex_to_path_mm": pct(d), "tip_dz_mm": pct(dz),
             "line_width_mm": pct(width), "path_to_vertex_mm": pct(dr),
             "n_dz_over_0.05": int((np.abs(dz) > 0.05).sum()), "overcut_nominal": round(overcut, 4)}
        worst = np.argsort(-np.abs(dz))[:5]
        r["worst"] = [{"path": sub[k]["path"], "i": int(sub[k]["i"]), "x_dxf": V[k, 0], "y_dxf": V[k, 1],
                       "dz": round(float(dz[k]), 4), "xy": round(float(d[k]), 4)} for k in worst]
        res["v"][kind] = r
        print(f"[1] V {kind}: {len(V)} vertices; |XY| {pct(d)}; dz {pct(dz)}; width {pct(width)}")
        # PASS: the median line within 0.05 mm of 0.8 and 99 % of tips within tip_tol (map.toml)
        tip_tol = mapt["vgroove"]["tip_tol"]
        frac = float((np.abs(dz) <= tip_tol).mean())
        r["frac_within_tip_tol"] = round(frac, 5)
        if frac < 0.99:
            print(f"    FAIL: only {frac:.4f} of the tips are within tip_tol {tip_tol}")
            ok = False

    # ── 2. V-bit body against the wood after the setup-1 roughing
    bc = ezdxf.readfile(MONO / "build/block/nz-south/back_channels.dxf")
    levels = []
    for layer in sorted({e.dxf.layer for e in bc.modelspace() if e.dxf.layer.startswith("ROUGH_SLOT_Z")}):
        # the arcs (bulges) are flattened: the vertices alone cut the round slot ends
        polys = [Polygon([(v.x, v.y) for v in ezpath.make_path(e).flattening(0.005)]).buffer(0)
                 for e in bc.modelspace() if e.dxf.layer == layer]
        levels.append((float(layer.split("_Z")[1]), even_odd(polys)))
    levels.sort()
    v20 = next(t for t in mapt["tools"] if t["id"] == "v20")
    cone = (v20["diameter"] / 2.0) / math.tan(math.radians(v20["angle"] / 2.0))
    body_r = v20["diameter"] / 2.0
    ceil_z = T1 - blk["cavity_rough"]
    res["body"] = {}
    for name in ("V river pre-pass", "V river final", "V lake pre-pass", "V lake final"):
        if name not in segs:
            continue
        P = s1_to_world(feed_points(segs[name], 0.25))
        P = P[P[:, 2] < ceil_z]                     # in the wood
        # wood top under the body disc: sample the disc (centre + 2 rings)
        ang = np.linspace(0, 2 * np.pi, 12, endpoint=False)
        offs = [(0.0, 0.0)] + [(rr * math.cos(a), rr * math.sin(a)) for rr in (body_r / 2, body_r) for a in ang]
        wood_top = np.full(len(P), -np.inf)
        for ox_, oy_ in offs:
            qx, qy = P[:, 0] + ox_, P[:, 1] + oy_
            floor = np.full(len(P), ceil_z)
            # the deepest level that contains the sample sets the floor there
            for z, g in levels:
                floor = np.where(contains_xy(g, qx, qy), T1 - z, floor)
            # the wood top under the body = the highest floor in the disc
            wood_top = np.maximum(wood_top, floor)
        margin = (P[:, 2] + cone) - wood_top
        r = {"points": len(P), "cone_top_minus_wood_mm": pct(margin),
             "n_below_0": int((margin < 0).sum()),
             "n_below_body_margin": int((margin < mapt["vgroove"]["body_margin"] - 1e-6).sum())}
        res["body"][name] = r
        print(f"[2] {name}: cone top - wood under body {pct(margin)}; <0: {r['n_below_0']}")
        if r["n_below_0"] > 0:
            ok = False
            print("    FAIL: the body enters wood")

    # ── 3. web over the cavity, setup-3 motion
    sk = ezdxf.readfile(CNC / "cavity_skim_d9.00.dxf")
    e = next(iter(sk.modelspace()))
    pts = [(v.dxf.location.x, v.dxf.location.y) for v in e.vertices]
    cav_world = Polygon(pts)  # bulge corners as chords: the hull is inside the arc, a small under-reach
    web_floor = blk["cavity_depth"] + blk["web_min"]
    res["web"] = {"floor_block_z": web_floor}
    for name, setup in setup_of.items():
        if not setup.startswith("3 ") or name not in segs:
            continue
        rad = tools[tool_of[name]]["diameter"] / 2.0
        zone = prep(cav_world.buffer(rad))
        Pl = feed_points(segs[name], 0.25)
        # setup-3 local -> world (Bottom: x_l = x_w - ox, y_l = D - (y_w - oy), z_l = H - z_w)
        xw = Pl[:, 0] + ox
        yw = (D - Pl[:, 1]) + oy
        inside = contains_xy(cav_world.buffer(rad), xw, yw)
        if not inside.any():
            res["web"][name] = {"points_over_cavity": 0}
            continue
        zmin = float(Pl[inside, 2].min())
        res["web"][name] = {"points_over_cavity": int(inside.sum()), "min_local_z": round(zmin, 4),
                            "margin_mm": round(zmin - web_floor, 4)}
        print(f"[3] {name}: min Z over the cavity (+r {rad}) = {zmin:.4f} (floor {web_floor})")
        if zmin < web_floor - 1e-6:
            ok = False
            print("    FAIL: the front cut enters the web")

    res["pass"] = ok
    if out_json:
        out_json.write_text(json.dumps(res, indent=1) + "\n")
    print("PASS" if ok else "FAIL")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
