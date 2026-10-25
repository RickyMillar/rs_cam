import math, collections
from load import get, F
seed=(51.05,58.55); s=1.2; R=3.0
c=(1-math.cos(1.05*math.acos(1-s/R)))/2;cd=2*R*c;b=R-cd
nxt=lambda r:min(-b+math.sqrt(b*b+(r+R)**2-R*R),r+s)
dist=lambda p:math.hypot(p[0]-seed[0],p[1]-seed[1])
for arm in ['geodesic_local_zigzag','geodesic_local_oneway']:
    d=get(arm); per=collections.defaultdict(float); rs=[]
    for i,((g,a,bb),k) in enumerate(zip(d['moves'],d['cls'])):
        if d['lev'][i]!='Z 5.00' or k!='cut': continue
        L=F.xy(a,bb); da,db=dist(a),dist(bb)
        if abs(da-db)<0.02 and da-math.sqrt(max(da*da-(L/2)**2,0))<=0.5 and da>3.05:
            rs.append((da,L))
    rs.sort(); lv=[]
    for r,L in rs:
        if lv and r-lv[-1][0]<0.3: lv[-1][1]+=L
        else: lv.append([r,L])
    steps=[(b2[0]-a[0],a[1],a[0]) for a,b2 in zip(lv,lv[1:]) if b2[0]-a[0]<=s+0.1]
    w=sorted((st,L) for st,L,r in steps); tot=sum(L for _,L in w); acc=0
    for st,L in w:
        acc+=L
        if acc>=tot/2: wm=st; break
    under=[(st,r) for st,L,r in steps if st< nxt(r)-r-0.1]
    print(arm,'levels',len(lv),'unweighted median %.3f'%sorted(x[0] for x in steps)[len(steps)//2],'wrap-length-weighted median %.3f'%wm,
          'steps under the law by >0.1:',len(under),'steps < s-0.01:',sum(1 for x in steps if x[0]<s-0.01),
          'of which inside rho* (law):',sum(1 for x in steps if x[0]<s-0.01 and x[2]<25.45))
    devs=[st-(nxt(r)-r) for st,L,r in steps]
    print('  step minus law: min %.3f median %.3f max %.3f'%(min(devs),sorted(devs)[len(devs)//2],max(devs)))
    for st,L,r in steps:
        law=nxt(r)-r
        if st<s-0.01 and r>=25.45 or st<law-0.1: print('   off: r %.2f step %.3f law %.3f len %.0f'%(r,st,law,L))
    ins=[st-(nxt(r)-r) for st,L,r in steps if r<25.45]
    print('  inside rho*: n %d mean step-law %.4f'%(len(ins),sum(ins)/len(ins)))
    allsteps=[(b2[0]-a[0],a[0]) for a,b2 in zip(lv,lv[1:])]
    C=collections.Counter()
    for st,r in allsteps:
        law=nxt(r)-r
        if st>s+0.1: C['over s+0.1 (reading: a level not found)']+=1
        elif st>=s-0.01: C['at s']+=1
        elif st>=law-0.1 and r<25.45: C['inside rho*, at the law']+=1
        else: C['under the law, outside the bracket']+=1
    print('  counts',dict(C),'total',len(allsteps))
