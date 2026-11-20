import sys, re
# usage: box_moves.py <nc> <op name prefix> x0 x1 y0 y1 ; prints the moves of the op whose end is in the box
nc, op = sys.argv[1], sys.argv[2]; x0, x1, y0, y1 = map(float, sys.argv[3:7])
x = y = z = 0.0; cur = None; inop = False; n = 0; f = None; out = []
for line in open(nc, errors="replace"):
    line = line.strip()
    if line.startswith("(") and not line.startswith("(WARNING"):
        name = line.strip("()")
        if name.startswith(op): inop = True; n = 0
        elif inop and re.match(r"^[A-Z0-9_].*", name) and not name[:5].islower() and "feeds" not in name and name != "" and not name.startswith(("cleanup", "entry", "ramp", "LOAD")):
            pass
    m = re.match(r"^(G0|G1|G2|G3)\b(.*)", line)
    if not m: 
        if line.startswith("(LOAD") and inop and n > 0: inop = False
        continue
    px, py, pz = x, y, z
    for a, v in re.findall(r"([XYZF])(-?[\d.]+)", m.group(2)):
        v = float(v)
        if a == "X": x = v
        elif a == "Y": y = v
        elif a == "Z": z = v
        else: f = v
    if inop:
        n += 1
        if x0 <= x <= x1 and y0 <= y <= y1:
            out.append((n, m.group(1), x, y, z, f))
prev = None
for o in out:
    if prev is not None and o[0] != prev + 1: print("   ...")
    print("%6d %s X%.2f Y%.2f Z%.3f F%s" % o); prev = o[0]
print(len(out), "moves")
