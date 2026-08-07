#!/usr/bin/env python3
"""Measure the incumbent fixture's triangle-area distribution.

Reproduces REFERENCE_FIXTURE_SPEC.md §1.1, which corrects the widely-cited
"terrain.stl is a coarse TIN, 1.8% of triangles carry 40.8% of area".

Run from anywhere:  python3 measure_terrain_tin.py
"""

import math
import os
import struct
import sys

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", "..", ".."))
TARGETS = [
    os.path.join(REPO, "crates/rs_cam_core/tests/fixtures/terrain.stl"),
    os.path.join(REPO, "fixtures/terrain_small.stl"),
]


def load_binary_stl(path):
    with open(path, "rb") as f:
        d = f.read()
    if d[:5] == b"solid" and b"facet" in d[:2000]:
        raise ValueError(f"{path}: ASCII STL not handled")
    n = struct.unpack("<I", d[80:84])[0]
    off = 84
    tris = []
    for _ in range(n):
        v = struct.unpack("<12fH", d[off:off + 50])
        off += 50
        tris.append(v[3:12])
    return tris


def tri_geo(v):
    ax, ay, az, bx, by, bz, cx, cy, cz = v
    ux, uy, uz = bx - ax, by - ay, bz - az
    vx, vy, vz = cx - ax, cy - ay, cz - az
    nx = uy * vz - uz * vy
    ny = uz * vx - ux * vz
    nz = ux * vy - uy * vx
    nl = math.sqrt(nx * nx + ny * ny + nz * nz)
    if nl <= 0.0:
        return 0.0, 1.0, (az + bz + cz) / 3.0
    return 0.5 * nl, nz / nl, (az + bz + cz) / 3.0


def report(path):
    tris = load_binary_stl(path)
    n = len(tris)
    all_areas = []
    relief = []
    for v in tris:
        a, nz, zc = tri_geo(v)
        all_areas.append(a)
        # "relief" = upward-facing and above the base plane, i.e. the surface a
        # finishing pass actually machines -- as opposed to the stock box.
        if nz > 1e-6 and zc > 1e-6:
            relief.append(a)

    print(f"\n=== {os.path.relpath(path, REPO)} ===")
    for label, areas, denom_label in (
        ("WHOLE MESH", all_areas, "total"),
        ("UPWARD-FACING RELIEF ONLY", relief, "relief"),
    ):
        s = sorted(areas, reverse=True)
        m = len(s)
        tot = sum(s)
        print(f"\n{label}: {m} triangles ({100*m/n:.1f}% of {n}), area {tot:.0f} mm2")
        for frac in (0.005, 0.010, 0.018, 0.050, 0.100):
            k = max(1, int(round(frac * m)))
            print(f"  top {frac*100:5.1f}% ({k:6d} tris) carry {100*sum(s[:k])/tot:5.1f}% of {denom_label} area")
        c = 0.0
        for i, a in enumerate(s):
            c += a
            if c >= 0.408 * tot:
                print(f"  40.8% of {denom_label} area is carried by {i+1} tris = {100*(i+1)/m:.2f}%")
                break
        asc = sorted(s)
        q = lambda p: math.sqrt(asc[min(m - 1, int(p * m))])
        print(f"  facet edge sqrt(area): p50 {q(.50):.3f}  p99 {q(.99):.3f}  max {math.sqrt(asc[-1]):.3f} mm")

    print("\n  NOTE: the whole-mesh reading is dominated by the base box -- the")
    print("  single largest facet is the 100x100 mm bottom face's half, which is")
    print("  why 40.8% of area lands on ~99 triangles there but on ~7.7% of the")
    print("  relief. Neither reading is the cited 1.8% / 40.8%.")


if __name__ == "__main__":
    for p in TARGETS:
        if os.path.exists(p):
            report(p)
        else:
            print(f"skip (absent): {p}", file=sys.stderr)
