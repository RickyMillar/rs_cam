"""P40 B: the wood that the one-pass rule leaves after id 2, and the later op that meets it.
usage: left.py <tag before> <tag after>
For each node where the stock after 'Back rest 6 mm' of <after> stands more than BAR above the
stock of <before>: the later ops of <after> that lower it, the column that each takes there
(the stock before the op less the stock after it), and what stands at the end above the target.
"""
import sys
from gapmap import *
a, b = sys.argv[1], sys.argv[2]
A = np.load(f"out_{a}.npz"); B = np.load(f"out_{b}.npz"); names = list(B["names"])
k = [i for i, n in enumerate(names) if n.startswith("Back rest")][0]
SA = A[f"s{k}"].astype(float); SB = B[f"s{k}"].astype(float)
left = SB - SA > BAR
print(f"nodes that <{b}> leaves above <{a}> after '{names[k][:30]}' by > {BAR}: {left.sum()} = {left.sum()*CELL*CELL:.1f} mm2; tallest {float((SB-SA)[left].max()) if left.any() else 0:.2f} mm; volume {float((SB-SA)[left].sum())*CELL*CELL:.0f} mm3")
jj, ii = np.nonzero(left)
if left.any():
    print(f"   box G-code X {ii.min()*CELL:.1f}..{ii.max()*CELL:.1f} Y {jj.min()*CELL:.1f}..{jj.max()*CELL:.1f}")
prev = SB
for i in range(k + 1, len(names)):
    S_i = B[f"s{i}"].astype(float)
    took = (prev - S_i)
    m = left & (took > 0.05)
    if m.any():
        j, c = np.unravel_index(np.argmax(np.where(m, took, 0)), took.shape)
        print(f"   '{names[i][:44]}' lowers {m.sum()*CELL*CELL:7.1f} mm2 of it; the tallest column that it takes there: {took[m].max():.2f} mm at G-code ({c*CELL:.1f}, {j*CELL:.1f}) (stock {prev[j,c]:.2f} to {S_i[j,c]:.2f})")
    prev = S_i
T = target(); ok = np.isfinite(T)
end = left & ok & (prev - T > BAR)
print(f"   still above the target by > {BAR} at the end: {end.sum()*CELL*CELL:.1f} mm2; tallest {float((prev-T)[end].max()) if end.any() else 0:.2f} mm")
