"""D58: op 1 of the product run against R, move by move from program.nc.
- geometry: the sequence of (G word, X, Y, Z, I, J) of op 1's motion lines, with F removed; the first differing line is
  printed with both lines;
- feeds: for each pair of equal G1/G2/G3 lines, F product against F R (the modal F): equal / slower / faster, and the
  ratio range; G-code plunges (vertical G1 descents).
Usage: geom_diff.py <R program.nc> <product program.nc> <op 1 name>"""
import re, sys
def op_lines(path, name):
    out, on, f = [], False, None
    for ln in open(path):
        ln = ln.strip()
        if ln.startswith("("):
            if re.match(r"^\((2 Back Rough|3D Rough 1)", ln): on = ln == f"({name})"
            continue
        if not on or not ln or ln[0] != "G": continue
        m = re.search(r"F(-?[\d.]+)", ln)
        if m: f = float(m.group(1))
        out.append((re.sub(r"\s*F-?[\d.]+", "", ln), f))
    return out
r, p = op_lines(sys.argv[1], sys.argv[3]), op_lines(sys.argv[2], sys.argv[3])
print(f"op 1 motion lines: R {len(r)}, product {len(p)}")
first = next((i for i, (a, b) in enumerate(zip(r, p)) if a[0] != b[0]), None)
if first is None and len(r) == len(p): print("geometry: IDENTICAL (every motion line equal without F)")
else:
    i = first if first is not None else min(len(r), len(p))
    print(f"geometry: FIRST DIFFERENCE at op-1 motion line {i}:\n  R:       {r[i] if i < len(r) else None}\n  product: {p[i] if i < len(p) else None}")
eq = slow = fast = 0; lo, hi = 9, 0; plR = plP = 0; zR = zP = None
for (a, fa), (b, fb) in zip(r, p):
    if a != b: break
    g = a.split()[0]
    if g in ("G1", "G2", "G3") and fa and fb:
        if abs(fa - fb) < 1e-9: eq += 1
        elif fb < fa: slow += 1; lo = min(lo, fb / fa)
        else: fast += 1; hi = max(hi, fb / fa)
print(f"fed lines: equal {eq}, product slower {slow} (ratio min {lo:.3f}), product faster {fast} (ratio max {hi:.3f})")
def plunges(L):
    x = y = 0.0; z = 50.0; n = 0
    for a, _ in L:
        g = a.split()[0]; nx, ny, nz = x, y, z
        for k, v in re.findall(r"([XYZ])(-?[\d.]+)", a):
            if k == "X": nx = float(v)
            elif k == "Y": ny = float(v)
            else: nz = float(v)
        if g == "G1" and nx == x and ny == y and nz < z: n += 1
        x, y, z = nx, ny, nz
    return n
print(f"G-code plunges (vertical G1 descents): R {plunges(r)}, product {plunges(p)}")
# the faster lines, with the line before
k = 0
for i, ((a, fa), (b, fb)) in enumerate(zip(r, p)):
    if a != b: break
    if a.split()[0] in ("G1", "G2", "G3") and fa and fb and fb > fa + 1e-9:
        print(f"  faster line {i}: {a}  R F{fa} -> product F{fb}; previous: {r[i-1][0]} (R F{r[i-1][1]}, product F{p[i-1][1]})")
