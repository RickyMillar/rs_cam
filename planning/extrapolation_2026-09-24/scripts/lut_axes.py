#!/usr/bin/env python3
"""Phase 0: LUT coverage along each gap axis (read-only)."""
import collections, glob, json, pathlib
ROOT = pathlib.Path(__file__).resolve().parents[3]
obs = []
for f in sorted(glob.glob(str(ROOT / "crates/rs_cam_core/data/vendor_lut/observations/*.json"))):
    d = json.load(open(f)); obs += d["observations"] if isinstance(d, dict) else d
WOOD = {"softwood", "hardwood", "mdf", "plywood_hardwood", "plywood_softwood", "hdf", "particleboard"}
wood = [o for o in obs if o["material_family"] in WOOD and o.get("chipload_max_mm_tooth")]
print(f"wood rows with a chipload: {len(wood)} of {len(obs)}")

print("\n## G1 series with >=2 diameters (source, subfamily, material, flutes, role)")
s = collections.defaultdict(set)
for o in wood:
    if o.get("diameter_mm"):
        s[(o["source_id"], o["tool_family"], o.get("tool_subfamily"), o["material_family"], o.get("flute_count"), o["pass_role"])].add(o["diameter_mm"])
fams = collections.Counter()
for k, v in sorted(s.items()):
    if len(v) >= 2:
        fams[k[1]] += 1
        print(f"  {k[1]:18s} {k[0]:34s} {str(k[2])[:22]:22s} {k[3]:16s} f{k[4]} {k[5]:10s} {sorted(v)}")
print("  series per tool family:", dict(fams))
print("  smallest printed diameter per tool family:",
      {t: min(o["diameter_mm"] for o in wood if o["tool_family"] == t and o.get("diameter_mm")) for t in {o["tool_family"] for o in wood if o.get("diameter_mm")}})

print("\n## G2 one tool (source, subfamily, diameter, flutes, role) printed in >=2 categories")
c = collections.defaultdict(dict)
for o in wood:
    c[(o["source_id"], o["tool_family"], o.get("tool_subfamily"), o.get("diameter_mm"), o.get("flute_count"), o["pass_role"], o["operation_family"])][o["material_family"]] = (o.get("chipload_min_mm_tooth") or 0, o["chipload_max_mm_tooth"])
n = collections.Counter()
for k, v in c.items():
    if len(v) >= 2:
        n[(k[1], tuple(sorted(v)))] += 1
for k, v in sorted(n.items()): print(f"  {v:3d} {k}")

print("\n## G3 one tool (source, subfamily, diameter, flutes, material) printed in >=2 pass roles / op families")
r = collections.defaultdict(set)
for o in wood:
    r[(o["source_id"], o["tool_family"], o.get("tool_subfamily"), o.get("diameter_mm"), o.get("flute_count"), o["material_family"])].add((o["operation_family"], o["pass_role"], o["chipload_max_mm_tooth"]))
m = collections.Counter()
for k, v in r.items():
    roles = {x[1] for x in v}; vals = {x[2] for x in v}
    if len(roles) >= 2:
        m[(k[1], k[0], tuple(sorted(roles)), "distinct values" if len(vals) > 1 else "same value")] += 1
for k, v in sorted(m.items()): print(f"  {v:3d} {k}")

print("\n## G4 band shape per tool family and source")
b = collections.Counter()
for o in wood:
    lo, hi = o.get("chipload_min_mm_tooth"), o["chipload_max_mm_tooth"]
    b[(o["tool_family"], o["source_id"], "band" if lo and lo < hi else "one value")] += 1
for k, v in sorted(b.items()): print(f"  {v:3d} {k}")
spreads = collections.defaultdict(list)
for o in wood:
    lo, hi = o.get("chipload_min_mm_tooth"), o["chipload_max_mm_tooth"]
    if lo and lo < hi: spreads[o["tool_family"]].append(lo / hi)
for t, v in spreads.items():
    v.sort(); print(f"  min/max ratio {t}: n={len(v)} median {v[len(v)//2]:.2f} range {v[0]:.2f}-{v[-1]:.2f}")

print("\n## G6 drill rows:", sum(1 for o in obs if o["operation_family"] == "drill"))
print("## G5 V-bit rows with a chipload:", sum(1 for o in wood if o["tool_family"] == "chamfer_vbit"),
      "; V-bit rows with no chipload:", sum(1 for o in obs if o["tool_family"] == "chamfer_vbit" and not o.get("chipload_max_mm_tooth")))
