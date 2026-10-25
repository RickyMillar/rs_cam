import re,sys,math
import sys as _s; _s.path.insert(0, "../c23")
from seq import read
D='/home/ricky/personal_repos/rs_cam/.claude/worktrees/adaptive-head/target/tmp/adaptive_geodesic_2d_g1'
for name in sys.argv[1:]:
    ev=read(f'{D}/{name}.svg')
    ents=[p[0] for t,p in ev if t=='entry']
    turns=[tuple(map(float,m)) for m in re.findall(r'turn: \(([-\d.]+), ([-\d.]+)\) to \(([-\d.]+), ([-\d.]+)\)',open(f'{D}/{name}.txt').read())]
    hit=[t for t in turns if any(math.dist((t[0],t[1]),e)<0.05 for e in ents)]
    near=[t for t in turns if any(math.dist((t[0],t[1]),e)<0.6 for e in ents)]
    print(name,'turns',len(turns),'entries',len(ents),'turn start = entry',len(hit),'within 0.6',len(near))
    for t in near: print('   ',t)
