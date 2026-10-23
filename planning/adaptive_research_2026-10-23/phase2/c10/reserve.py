"""C10: the band spacing delta' that a growing bite of span <= L needs at
front radius rho (a margin reserved for the entry; a ruling, not built)."""
import math, sys
sys.path.insert(0, sys.argv[1])
from model import *

def need(rho, L):
    lo, hi = 0.5, S
    if (g := growing_bite_span(rho, hi)) is not None and g <= L:
        return hi
    for _ in range(60):
        mid = (lo + hi) / 2
        g = growing_bite_span(rho, mid, n=1500)
        if g is not None and g <= L:
            lo = mid
        else:
            hi = mid
    return lo

for lvl, rho, L in [(11, 23.05, 10.6), (12, 25.05, 8.9), (13, 27.05, 7.2), (14, 29.05, 5.8), (15, 31.05, 4.2)]:
    d = need(rho, L)
    print(f"level {lvl} rho {rho} shortest star arc {L}: delta' {d:.3f} mm (s {S})")
print("inside rho* (the law level spacing there is the cap):")
for lvl, rho, L, law in [(8, 17.10, 16.4, 1.945), (9, 19.06, 14.2, 1.968), (10, 21.05, 12.4, 1.986)]:
    d = need(rho, L)
    print(f"level {lvl} rho {rho} shortest star arc {L}: delta' {d:.3f} mm (law {law})")
