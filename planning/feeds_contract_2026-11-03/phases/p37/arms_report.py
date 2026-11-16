import json, sys
from gapmap import *
T=target(); ok=np.isfinite(T)
def one(tag):
    d=np.load(f"out_{tag}.npz"); names=list(d["names"])
    out,S=report(f"out_{tag}.npz",tag)
    # stock before the light-hole drill: the snapshot of the op before 'Drill light holes'
    k=[i for i,n in enumerate(names) if n.startswith("Drill light holes")][0]
    print("  light holes, highest stock under R 3.2 before the Drill:", {a:round(b,2) for a,b in holes_top(d[f"s{k-1}"].astype(float)).items()})
    # roughs + new ops below target
    for i,n in enumerate(names[:k]):
        S_i=d[f"s{i}"].astype(float); print(f"    below target by > {BAR} after '{n[:40]}': {((S_i-T<-BAR)&ok).sum()*CELL*CELL:.1f} mm2")
    s=json.load(open(f"out_{tag}/summary.json"))
    print("  total_runtime_s %.0f  collisions %s  rapid %s  failed checks %s"%(s["total_runtime_s"],s["collision_count"],s["rapid_collision_count"],s["collision_checks_failed"]))
    for t in s["per_toolpath"]:
        print("    %2d %-48s moves %6d coll %s"%(t["id"],t["name"][:48],t["move_count"],t["collision_count"]))
    w=[x for x in s["gcode_export"]["warnings"] if "op_left_out" not in x]
    return set(w)
base=one("base")
for tag in sys.argv[1:]:
    w=one(tag)
    print("  warnings new:",sorted(w-base)); print("  warnings gone:",sorted(base-w))
