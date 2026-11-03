"""Assert each numeric claim of REPORT.md. Inputs cite the repo.
Ks, F_edge: white oak Curti line at rho 761.6 (tool_load/power.rs:901).
D = 6 mm, z = 2, stickout 45 mm (rivmap100_boundary_strike.toml:69,78,79).
fz = 0.05 mm is an EXAMPLE value only (a chip inside Curti range 0.04-0.10, force_line.rs:78)."""
from math import acos, sin, cos, pi, isclose
KS, FE = 58.52, 4.595
D, Z, L = 6.0, 2, 45.0
r = D / 2
fz = 0.05
psi = lambda ae: acos(max(-1, min(1, 1 - ae / r)))          # force.rs:57
sin_pk = lambda ae: sin(min(psi(ae), pi / 2))                # force.rs:94
h_mean = lambda ae: fz * (1 - cos(psi(ae))) / psi(ae)        # operating_point.rs:242
rctf = lambda ae: 1 / ((1 - (1 - 2 * min(ae / D, .5)) ** 2) ** .5) if ae < D / 2 else 1.0  # geometry.rs:17
res = {}
for name, ae in [("slot", D), ("half", D / 2), ("0.2D", 0.2 * D), ("0.1D", 0.1 * D)]:
    p = psi(ae)
    res[name] = dict(ae=ae, psi=p, duty=Z * p / (2 * pi), hpk=fz * sin_pk(ae), hm=h_mean(ae),
                     u=KS + FE * D * p / (2 * ae * fz), Fper_ap=KS * fz * sin_pk(ae) + FE, rctf=rctf(ae))
for k, v in res.items():
    print(k, {a: round(b, 4) for a, b in v.items()})
s, l = res["slot"], res["0.2D"]
assert isclose(l["psi"], acos(0.6)) and abs(l["psi"] - 0.9273) < 1e-4
assert isclose(s["duty"], 1.0) and abs(l["duty"] - 0.2952) < 1e-4
# same ap, same fz: shear x5, edge x pi/psi
assert isclose(s["ae"] / l["ae"], 5.0); assert abs(s["psi"] / l["psi"] - 3.388) < 1e-3
# same MRR (slot at ap/5): shear equal, slot edge = 0.2*pi/psi
assert abs(0.2 * s["psi"] / l["psi"] - 0.6776) < 1e-3
# peak chip: slot fz, 0.2D 0.8 fz; RCTF 1.25 = 1/0.8
assert isclose(l["hpk"], 0.8 * fz) and isclose(l["rctf"], 1.25)
# mean chip: slot 2/pi fz, 0.2D 0.431 fz
assert abs(s["hm"] / fz - 2 / pi) < 1e-9 and abs(l["hm"] / fz - 0.4314) < 1e-4
# specific energy (N/mm^2 = mJ/mm^3): slot 202.9, 0.2D 271.5 at fz 0.05
assert abs(s["u"] - 202.9) < 0.1 and abs(l["u"] - 271.5) < 0.1
# model lateral force per mm ap: slot / 0.2D = 1.084
assert abs(s["Fper_ap"] / l["Fper_ap"] - 1.0844) < 1e-3
# half immersion and slot: same peak force (saturation, force.rs:33-36)
assert isclose(res["half"]["Fper_ap"], s["Fper_ap"])
# Deflection vs ap, uniform cantilever, point load F=k*ap at a = L - ap/2:
# tip deflection ∝ ap * a^2 * (3L - a) (textbook; same form tool/mod.rs integrates)
f = lambda ap: ap * (L - ap / 2) ** 2 * (3 * L - (L - ap / 2))
aps = [i * 0.01 for i in range(1, int(0.95 * L * 100) + 1)]
apmax = max(aps, key=f)
print("deflection peak ap", apmax, "ratio peak/upper", f(apmax) / f(0.95 * L))
assert isclose(0.95 * L, 42.75)
assert 25 < apmax < 0.95 * L and f(apmax) > f(0.95 * L) * 1.05
print("ALL CLAIMS HOLD")

# --- Part 2: depth tier, chip-first, helix (helix 30 deg: rivmap toml:71) ---
from math import tan, radians
def doc_scale(ratio):                      # feeds/geometry.rs:155-165
    if ratio <= 1: return 1.0
    if ratio <= 2: return 1 - .25 * (ratio - 1)
    if ratio <= 3: return .75 - .25 * (ratio - 2)
    return .5
assert doc_scale(25 / 6) == 0.5 and abs(0.131 * 0.5 - 0.0655) < 1e-9
fz2 = 0.0655
hm02 = fz2 * (1 - cos(psi(1.2))) / psi(1.2)
print("rivmap 3D rough at 0.2D, ap 25: peak chip", fz2 * 0.8, "mean chip", hm02)
assert abs(fz2 * 0.8 - 0.0524) < 1e-4 and abs(hm02 - 0.02826) < 1e-4
assert hm02 < 0.04                         # below Curti fit range (force_line.rs:78)
beta = radians(30.0)
lag_len = psi(1.2) * r / tan(beta)         # axial length one tooth's arc spans
ap_smooth = (2 * pi / Z) * r / tan(beta)   # ap where helix wrap = flute pitch (tool/mod.rs:252-253)
print("axial span of one arc at 0.2D", lag_len, "ap for wrap = pitch", ap_smooth)
assert abs(lag_len - 4.818) < 1e-3 and abs(ap_smooth - 16.32) < 1e-2
# one-tooth peak model vs mean tangential force over engaged edge (duty z.psi/2pi, power.rs:36)
def ratio(ae, f):
    p = psi(ae); hm = f * (1 - cos(p)) / p
    model = KS * f * sin(min(p, pi / 2)) + FE
    mean = Z * p / (2 * pi) * (KS * hm + FE)
    return model / mean
print("one-tooth model / mean force: 0.2D", ratio(1.2, fz), "slot", ratio(6.0, fz))
assert abs(ratio(1.2, fz) - 4.01) < 0.01 and abs(ratio(6.0, fz) - 1.165) < 0.01
print("PART 2 CLAIMS HOLD")
