"""C38: where are the regions of Corne "3D Rough 1 (copy)" that F-038 skips?

The copy op: tool 1, a 3 mm flat end mill (R 1.5), stepover 0.6, Depth/Pass
3, axial leave 0.5, `stock_source = "from_remaining_stock"`, boundary =
silhouette with `containment = "center"`, `offset = 3.0`
(corne_job_corrected_c12_agent_search.toml). The debug run
(corne_d20_debug/grep_lines.txt) logs 3 regions per level (about 259, 72-81
and 280 mm2, 611-621 mm2 in all) with forecast 0, but not their place.

This script rebuilds the copy op's level grid from the D20 stock map after
the first rough op (../fix9/stock_corne_zigzag_pick_D20.npz, a rebuild from
the samples, see fix9/README) and the part heightmap (../fix9/corne_part.npz):
- the tool-centre floor of a 3 mm flat: the part height dilated by a disc of
  R 1.5 (drop cutter of a flat end mill);
- a cell holds material when the stock top is over max(floor + 0.5, z) + 0.01
  (`cell_has_material`), and the boundary clear (O5b) empties the cells
  whose centre is more than 3 mm outside the silhouette.
Cells beyond the mesh bbox are not on the part map, so they are left out:
the rebuilt total is a lower bound. Asserts the claims of the C38 note, 8.
"""
from pathlib import Path

import numpy as np
from scipy import ndimage

FIX9 = Path(__file__).resolve().parent.parent / "fix9"
part = np.load(FIX9 / "corne_part.npz")
top, g, x0, y0 = part["top"], float(part["g"]), float(part["x0"]), float(part["y0"])
stock = np.load(FIX9 / "stock_corne_zigzag_pick_D20.npz")
st, xs, ys, sg = stock["top"], stock["xs"], stock["ys"], float(stock["g"])

R = 1.5
n = int(np.ceil(R / g))
yy, xx = np.mgrid[-n : n + 1, -n : n + 1]
disc = (xx * g) ** 2 + (yy * g) ** 2 <= R * R
cl = ndimage.grey_dilation(np.where(np.isfinite(top), top, -1e9), footprint=disc, mode="constant", cval=-1e9)
sil = ndimage.binary_fill_holes(np.isfinite(top) & (top > 1e-6))
dout = ndimage.distance_transform_edt(~sil) * g

X, Y = np.meshgrid(xs, ys)
ci = np.round((X - x0) / g).astype(int)
ri = np.round((Y - y0) / g).astype(int)
on = (ci >= 0) & (ci < top.shape[1]) & (ri >= 0) & (ri < top.shape[0])
CL = np.zeros(st.shape)
D = np.full(st.shape, np.inf)
CL[on] = cl[ri[on], ci[on]]
D[on] = dout[ri[on], ci[on]]
inside_boundary = D <= 3.0

for z in (14.0, 11.0, 8.0, 5.0, 2.0, 0.5):
    mat = (st > np.maximum(CL + 0.5, z) + 0.01) & inside_boundary
    total = float(mat.sum()) * sg * sg
    outside = float((mat & (D > 0)).sum()) * sg * sg
    d_med = float(np.median(D[mat]))
    print(f"Z {z}: material {total:.0f} mm2, outside the silhouette {outside:.0f} mm2, "
          f"median wall distance {d_med:.2f} mm")
    # at Z 14 to 5 at least 95 % of the copy op's material is in the outer strip,
    # at Z 2 and 0.5 at least 85 % (small floors inside the case join there):
    # outside the silhouette, the tool-centre band from wall + R to the
    # containment line (+ 3 mm)
    assert outside / total >= (0.95 if z >= 5.0 else 0.85), (z, outside, total)
    assert 1.5 <= d_med <= 3.0, (z, d_med)
    assert 250 <= total <= 620, (z, total)
print("ASSERTS OK")
