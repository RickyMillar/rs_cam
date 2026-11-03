"""Assert the op 2 corner-sliver claims of 03_op2_rest_entry.md (idealised op 1 model)."""
import sys
src = open("band.py").read()
src = src.split("for t in sorted")[0]
exec(src)
sliver = (lab == [t for t in info if 14.4 < t[6] < 14.6][0][0])
h = (top - Z)[sliver]
d6 = D6[sliver]
print("sliver cells", sliver.sum(), "area mm2 %.1f" % (sliver.sum() * cell * cell))
print("sliver height max %.2f" % h.max())
print("sliver cells whose 6 mm drop height is above the stock top (planner reads no material at any level):",
      int((d6 >= top[sliver] - 0.01).sum()), "of", int(sliver.sum()))
assert abs(h.max() - 14.5) < 0.05, "sliver is not 14.5 mm tall"
assert (d6 >= top[sliver] - 0.01).all(), "a sliver cell is visible to cell_has_material"
jc = int(round((cy - lo_y) / cell)); ic = int(round((cx - lo_x) / cell))
assert D18[jc, ic] > 25.0, "the 18 mm can reach the entry"
print("OK: all claims hold")
