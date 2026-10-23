"""C10: assert each number the note P2_C10_leadin_guard.md quotes.
Run: python3 -I assert_c10.py <this dir>."""
import io, math, sys, contextlib
from collections import Counter
sys.path.insert(0, sys.argv[1])
import model as M
from model import *
import fixed as FX
import reserve as RS  # prints its table on import

close = lambda a, b, t: abs(a - b) <= t
assert close(CD, 2.1756, 1e-9) and CELL == 0.5 and close(rho_star(), 22.78, 0.005)
assert close(growing_bite_span(math.inf, S), 7.70, 0.01)          # EntryLaw doc value
for rho, rl, g in [(25, 22.80, 14.43), (30, 22.95, 11.61), (35, 23.13, 10.57), (60, 23.88, 8.99)]:
    assert close(lead_in_radius_pivot(rho, S)[0], rl, 0.01) and close(growing_bite_span(rho, S), g, 0.01)
with contextlib.redirect_stdout(io.StringIO()):
    _, star = M.run(Star())
    _, pocket = M.run(Pocket())
s0 = Counter(r["why"] for r in star if r["orient"] == 0)
p0 = Counter(r["why"] for r in pocket if r["orient"] == 0)
assert sum(s0.values()) == 39 and s0["LEAD-IN"] == 0
assert s0["start not reached by a right angle"] == 24 and s0["no margin"] + s0["radius > rho - delta/2"] == 15
assert sum(p0.values()) == 12 and p0["LEAD-IN"] == 0 and p0["no margin"] == 10
# every star band that reached the trace has its cut arc shorter than the ideal span
assert all(r["total"] < 12.0 for r in star if r["why"].startswith("start"))
assert all(r["grow"] is None or r["grow"] >= r["total"] for r in star)   # growing bite: 0 fits
assert all(r["grow"] is None or r["grow"] >= r["total"] for r in pocket)
print("all C10 claims hold")
