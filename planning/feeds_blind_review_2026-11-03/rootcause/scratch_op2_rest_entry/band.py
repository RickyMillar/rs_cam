import sys, numpy as np
exec(open("probe_target.py").read().split("D6 = drop(3.0)")[0].replace("path, cx, cy, half = sys.argv[1], float(sys.argv[2]), float(sys.argv[3]), float(sys.argv[4])","path, cx, cy, half = sys.argv[1], float(sys.argv[2]), float(sys.argv[3]), float(sys.argv[4])"))
D6 = drop(3.0); D18 = drop(9.0)
# op1 levels 16.5 and 11.0, leave 0: after op1 a cell keeps stock above max(level_reached, surface)
# 18mm disc centred where drop18 <= level removes stock down to the level over its disc.
inner = (np.abs(xs[None,:]-cx) <= half) & (np.abs(ys[:,None]-cy) <= half)
k = int(np.ceil(9.0/cell))
top = np.full_like(Z, 25.5)
for lvl in (16.5, 11.0):
    ok = D18 <= lvl + 1e-6
    cut = np.zeros_like(ok)
    for dj in range(-k,k+1):
        for di in range(-k,k+1):
            if (di*cell)**2+(dj*cell)**2 > 81: continue
            cut |= np.roll(np.roll(ok,dj,0),di,1)
    top = np.where(cut, np.minimum(top, np.maximum(lvl, Z)), top)
rest = inner & (top > Z + 0.05)
print("rest cells (stock above target after op1, ideal 18mm model):", rest.sum(), "area mm2", rest.sum()*cell*cell)
# label connected components of rest
from collections import deque
lab = np.zeros(Z.shape, int); n=0; info=[]
for j in range(Z.shape[0]):
    for i in range(Z.shape[1]):
        if rest[j,i] and not lab[j,i]:
            n+=1; q=deque([(j,i)]); lab[j,i]=n; cells=[]
            while q:
                a,b=q.popleft(); cells.append((a,b))
                for da,db in ((1,0),(-1,0),(0,1),(0,-1)):
                    c,d=a+da,b+db
                    if 0<=c<Z.shape[0] and 0<=d<Z.shape[1] and rest[c,d] and not lab[c,d]:
                        lab[c,d]=n; q.append((c,d))
            cc=np.array(cells); h=(top-Z)[cc[:,0],cc[:,1]]
            info.append((n,len(cells)*cell*cell, xs[cc[:,1]].min(), xs[cc[:,1]].max(), ys[cc[:,0]].min(), ys[cc[:,0]].max(), h.max()))
for t in sorted(info,key=lambda t:-t[1])[:12]:
    print("region %d area %.1f x %.1f..%.1f y %.1f..%.1f max height above target %.1f"%t)
jc=int(round((cy-lo_y)/cell)); ic=int(round((cx-lo_x)/cell))
print("entry cell: op1 top %.2f target %.2f label %d"%(top[jc,ic],Z[jc,ic],lab[jc,ic]))
step=4
print("op1 top - target, 1 mm grid, x %.1f.."%(cx-12))
for j in range(jc+48, jc-49, -step):
    print("y=%6.1f "%ys[j] + " ".join("%4.1f"%(top[j,i]-Z[j,i]) for i in range(ic-48, ic+49, step)))
