import math, collections, sys
from load import get, F
import fixture3d_v2 as FX
seed=(51.05,58.55); s=1.2; R=3.0
c=(1-math.cos(1.05*math.acos(1-s/R)))/2;cd=2*R*c;rho_star=s*(2*R-s)/(2*(cd-s))
dist=lambda p:math.hypot(p[0]-seed[0],p[1]-seed[1])
for arm in ['agent','geodesic_local_zigzag','geodesic_local_oneway']:
    d=get(arm); tot=collections.Counter(); per_level=collections.Counter()
    for lab in d['levels']:
        for i,((g,a,b),k) in enumerate(zip(d['moves'],d['cls'])):
            if d['lev'][i]!=lab or k!='cut': continue
            L=F.xy(a,b); tot[lab]+=L
    z5=collections.Counter()
    for i,((g,a,b),k) in enumerate(zip(d['moves'],d['cls'])):
        if d['lev'][i]!='Z 5.00' or k!='cut': continue
        L=F.xy(a,b); da,db=dist(a),dist(b)
        if arm!='agent' and abs(da-db)<0.02:
            sag=da-math.sqrt(max(da*da-(L/2)**2,0))
            if sag<=0.5:
                z5['wrap inside rho*' if da<rho_star else 'wrap outside rho*']+=L
                per_level[round(da,1)]+=L
            else: z5['straight move across the cut']+=L
        else: z5['other cut']+=L
    print(arm,'cut xy by level',{k:round(v) for k,v in tot.items()},'all %.0f'%sum(tot.values()))
    if arm!='agent':
        print('  Z5:',{k:round(v) for k,v in z5.items()})
        inner=sorted(r for r in per_level if r<rho_star)
        # levels (cluster)
        lv=[]
        for r in inner:
            if lv and r-lv[-1]<0.3: continue
            lv.append(r)
        plain=(lv[-1]-R)/s if lv else 0
        print('  inner levels %d (%.2f..%.2f), plain spacing would give %.1f'%(len(lv),lv[0],lv[-1],plain))
print('rho*',rho_star,'cD',cd)
