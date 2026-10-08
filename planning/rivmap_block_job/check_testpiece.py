"""Check the emitted G-code of the test-piece job (measurement only).

Run:  uv run --with ezdxf --with numpy --with shapely \
          python planning/rivmap_block_job/check_testpiece.py <testpiece.toml> <emitted.nc>

1. Web: the setup-3 (front) feed motion over the cavity footprint (+ tool
   radius) stays at or above block Z cavity depth + web_min.
2. Frame: the setup-1 channel motion lies inside the channel outlines of the
   back DXF (world frame), and the setup-3 motion lies inside the stock.
"""

from __future__ import annotations

import math
import sys
import tomllib
from pathlib import Path

import ezdxf
from ezdxf import path as ezpath
import numpy as np
from shapely import contains_xy
from shapely.geometry import Polygon
from shapely.ops import unary_union

sys.path.insert(0, str(Path(__file__).resolve().parent))
from check_gcode import feed_points, parse_program  # noqa: E402

MONO = Path("/home/ricky/personal_repos/project_rivmap_mono/manufacture")


def main() -> int:
    proj = tomllib.loads(Path(sys.argv[1]).read_text())
    nc = Path(sys.argv[2])
    tp = tomllib.loads((MONO / "block/testpiece.toml").read_text())
    contract = tomllib.loads((MONO / "design/rivmap.toml").read_text())
    stock = proj["job"]["stock"]
    T1, D = stock["z"], stock["y"]
    hx, hy = tp["block"]["stock_x"] / 2, tp["block"]["stock_y"] / 2
    setup_of = {t["name"]: s["name"] for s in proj["setups"] for t in s["toolpaths"]}
    tool_of = {t["name"]: t["tool_id"] for s in proj["setups"] for t in s["toolpaths"]}
    tools = {t["id"]: t for t in proj["tools"]}
    segs, _ = parse_program(nc, set(setup_of))
    ok = True

    # block-frame cavity -> setup-3 local frame: local = (Y + hy, D - (X + hx))
    s3 = ezdxf.readfile(MONO / "build/block/testpiece/setup3_back_cavity.dxf")
    e = next(e for e in s3.modelspace() if e.dxf.layer.startswith("BACK_CAVITY_Z"))
    back = [(v.x, v.y) for v in ezpath.make_path(e).flattening(0.01)]
    # back-view DXF: X = -dxf_x, Y = dxf_y
    cav_local = Polygon([(y + hy, D - (-x + hx)) for x, y in back])
    web_floor = tp["cavity"]["depth"] + contract["cavity"]["web_min"]
    for name, setup in setup_of.items():
        if not setup.startswith("3 ") or name not in segs:
            continue
        r = tools[tool_of[name]]["diameter"] / 2.0
        P = feed_points(segs[name], 0.25)
        inside = contains_xy(cav_local.buffer(r), P[:, 0], P[:, 1])
        if inside.any():
            zmin = float(P[inside, 2].min())
            print(f"[web] {name}: min Z over the cavity (+r {r}) = {zmin:.4f} (floor {web_floor})")
            ok &= zmin >= web_floor - 1e-6

    # channel motion inside the channel outlines (setup 1, world = G-code + 0 origin)
    s1 = ezdxf.readfile(MONO / "build/block/testpiece/setup1_back_channels.dxf")
    for name, setup in setup_of.items():
        if not name.startswith("Channel") or name not in segs:
            continue
        layer = name.split(" ", 1)[1]
        g = unary_union([Polygon([(y + hy, -x + hx) for x, y in
                                  [(v.x, v.y) for v in ezpath.make_path(e).flattening(0.01)]])
                         for e in s1.modelspace() if e.dxf.layer == layer])
        r = tools[tool_of[name]]["diameter"] / 2.0
        P = feed_points(segs[name], 0.25)
        P = P[P[:, 2] < T1 - 1e-6]
        tol = tp["paths"]["grid"] / 4.0   # 0.05 mm: under a quarter of the check grid
        outside = ~contains_xy(g.buffer(-r + tol), P[:, 0], P[:, 1])
        print(f"[frame] {name}: {int(outside.sum())} of {len(P)} cutting samples more than {tol} mm "
              "past the outline less r")
        ok &= int(outside.sum()) == 0
    print("PASS" if ok else "FAIL")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
