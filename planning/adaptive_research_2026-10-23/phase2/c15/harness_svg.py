# Read the harness renders (phase2/renders5/*.svg): the pale grey paths
# (#d0d0d0) are Run2d::wraps, the planned level-set wraps; blue (#1f5fbf) are
# the clearing cuts. The page transform is the harness View::over(outline):
# min_x 5, max_y 55, scale (900 - 2*24) / 70, margin 24 (render harness :232).
import os
import re
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
RENDERS = os.path.join(HERE, "..", "renders5")
CANVAS, MARGIN = 900.0, 24.0
MIN_X, MAX_Y = 5.0, 55.0
SCALE = (CANVAS - 2 * MARGIN) / 70.0

_path = re.compile(r'<path d="([^"]+)" fill="none" stroke="(#[0-9a-f]{6})"')


def paths(name, colour):
    txt = open(os.path.join(RENDERS, name + ".svg")).read()
    out = []
    for d, col in _path.findall(txt):
        if col != colour:
            continue
        pts = []
        for m in re.finditer(r"[ML]([-0-9.]+),([-0-9.]+)", d):
            px, py = float(m.group(1)), float(m.group(2))
            pts.append((MIN_X + (px - MARGIN) / SCALE, MAX_Y - (py - MARGIN) / SCALE))
        if len(pts) >= 2:
            out.append(np.array(pts))
    return out


def wraps(name):
    return paths(name, "#d0d0d0")


def cuts(name):
    return paths(name, "#1f5fbf")


def paths_view(path, colour, view):
    """Paths of one colour from a harness SVG, with that shape's View
    (min_x, max_y, scale) (render harness View::over)."""
    min_x, max_y, scale = view
    txt = open(path).read()
    out = []
    for d, col in _path.findall(txt):
        if col != colour:
            continue
        pts = [(min_x + (float(m.group(1)) - MARGIN) / scale, max_y - (float(m.group(2)) - MARGIN) / scale)
               for m in re.finditer(r"[ML]([-0-9.]+),([-0-9.]+)", d)]
        if len(pts) >= 2:
            out.append(np.array(pts))
    return out
