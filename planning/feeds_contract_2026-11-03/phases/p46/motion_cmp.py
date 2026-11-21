"""P46: compare two G-code files. Motion lines are the lines that are not a comment "( ... )".
usage: motion_cmp.py <before.nc> <after.nc>
Prints the count of motion lines, whether they are the same, and the comment lines that differ."""
import sys, difflib
def read(p):
    lines = open(p, errors="replace").read().splitlines()
    motion = [l for l in lines if l.strip() and not l.lstrip().startswith("(")]
    notes = [l for l in lines if l.lstrip().startswith("(")]
    return lines, motion, notes
a, am, an = read(sys.argv[1]); b, bm, bn = read(sys.argv[2])
print("lines %d and %d; motion lines %d and %d; comment lines %d and %d" % (len(a), len(b), len(am), len(bm), len(an), len(bn)))
if am == bm:
    print("MOTION SAME: each motion line is the same, in the same order")
else:
    first = next((i for i, (x, y) in enumerate(zip(am, bm)) if x != y), min(len(am), len(bm)))
    print("MOTION DIFFERS: first at motion line %d: %r | %r" % (first + 1, am[first:first+1], bm[first:first+1]))
print("BYTES SAME" if a == b else "the files differ")
gone = [l for l in an if l not in set(bn)]; new = [l for l in bn if l not in set(an)]
print("comment lines only before: %d; only after: %d" % (len(gone), len(new)))
for l in gone: print("  - " + l[:2500])
for l in new: print("  + " + l[:2500])
