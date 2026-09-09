"""Island areas from a preview_tier_map SVG: exterior, holes, net, per path.
usage: python3 svg_island_area.py tier_map.svg"""
import re, sys
svg=open(sys.argv[1]).read()
paths=re.findall(r"<path d='([^']*)'([^>]*)>", svg)
own_net=own_holes=mach_net=mach_holes=0; hole_areas=[]
for d,attrs in paths:
    kind='owned' if 'evenodd' in attrs else 'machining'
    areas=[]
    for s in [s for s in d.split('M') if s.strip()]:
        pts=[tuple(map(float,p.split())) for p in re.findall(r'([-\d.]+ [-\d.]+)', s.replace('L',' ').replace('Z',''))]
        if len(pts)<3: continue
        areas.append(abs(0.5*sum(pts[k][0]*pts[(k+1)%len(pts)][1]-pts[(k+1)%len(pts)][0]*pts[k][1] for k in range(len(pts)))))
    ext, holes = areas[0], areas[1:]
    if kind=='owned': own_net += ext-sum(holes); own_holes += len(holes); hole_areas += holes
    else: mach_net += ext-sum(holes); mach_holes += len(holes)
hole_areas.sort()
def q(p): return hole_areas[int(p*(len(hole_areas)-1))] if hole_areas else 0
print(f"owned net {own_net:.0f} mm2 holes {own_holes} | machining net {mach_net:.0f} mm2 holes {mach_holes} | ratio {mach_net/own_net if own_net else 0:.2f}")
print(f"owned-hole area p10/p50/p90/max: {q(.1):.1f}/{q(.5):.1f}/{q(.9):.1f}/{q(1):.0f} ; holes < 4 mm2: {sum(a<4 for a in hole_areas)} ; < 16: {sum(a<16 for a in hole_areas)}")
