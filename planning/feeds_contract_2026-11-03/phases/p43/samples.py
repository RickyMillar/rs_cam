"""P43: the simulation samples of one toolpath near given points. Reads simulation.json
(`--sim-artifact`). usage: samples.py <simulation.json> <toolpath id> <x,y,r[,z] in the G-code frame> ...
Prints for each window the samples with the largest axial_engagement_mm, and the deepest sample."""
import json, sys
d = json.load(open(sys.argv[1])); tid = int(sys.argv[2])
tr = d["trace"]; S = tr["samples"]
print("keys of a sample:", sorted(S[0].keys()))
OFF = (18.5, 8.5)
mine = [s for s in S if s.get("toolpath_id") == tid]
print("samples of the toolpath", len(mine))
dz = max(mine, key=lambda s: s.get("axial_engagement_mm") or 0)
def show(s):
    p = s["position"]
    return f"sample {s.get('sample_index')} move {s.get('move_index')} at G ({p[0]+OFF[0]:.2f},{p[1]+OFF[1]:.2f},{p[2]:.2f}) depth {s.get('axial_engagement_mm')} arc {s.get('arc_engagement_radians', s.get('engagement_arc_radians'))} radial {s.get('radial_engagement_mm')} removed {s.get('removed_volume_est_mm3')} feed {s.get('feed_rate_mm_min')}"
print("deepest:", show(dz))
for w in sys.argv[3:]:
    v = [float(t) for t in w.split(",")]
    x, y, r = v[:3]; z = v[3] if len(v) > 3 else None   # an optional 4th value: only the samples at this Z
    near = [s for s in mine if (z is None or abs(s["position"][2] - z) < 0.01) and (s["position"][0] + OFF[0] - x) ** 2 + (s["position"][1] + OFF[1] - y) ** 2 <= r * r]
    near.sort(key=lambda s: -(s.get("axial_engagement_mm") or 0))
    print(f"window ({x},{y}) r {r}: {len(near)} samples; the deepest:")
    for s in near[:4]: print("   ", show(s))
