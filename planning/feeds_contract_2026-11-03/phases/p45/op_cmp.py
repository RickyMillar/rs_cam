"""P45: compare the motion lines of one op between two G-code files. usage: op_cmp.py <a.nc> <b.nc> <op prefix>..."""
import sys
SKIP = ("WARNING", "MSG", "feeds", "drill feeds", "Generated", "LOAD", "INFO", "NOTE")
def body(f, name):
    out = []; on = False
    for l in open(f, errors="replace"):
        if l.startswith("("):
            t = l.strip()[1:-1]
            if not t.startswith(SKIP): on = t.startswith(name)
            continue
        if on: out.append(l)
    return out
for name in sys.argv[3:]:
    a, b = body(sys.argv[1], name), body(sys.argv[2], name)
    n = 0
    while n < min(len(a), len(b)) and a[n] == b[n]: n += 1
    print(f"'{name}': {len(a)} and {len(b)} motion lines; {'THE SAME' if a == b else 'DIFFERENT, common prefix %d lines' % n}")
