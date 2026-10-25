import sys, math, collections
from load import get, F
R=3.0
def law_tables(s):
    c=(1-math.cos(1.05*math.acos(1-s/R)))/2;cd=2*R*c;b=R-cd
    nxt=lambda r:min(-b+math.sqrt(b*b+(r+R)**2-R*R),r+s)
    return cd,nxt
def analyse(arm, s=1.2, seed=(51.05,58.55), lab='Z 5.00', out=True):
    d=get(arm)
    cd,nxt=law_tables(s)
    dist=lambda p:math.hypot(p[0]-seed[0],p[1]-seed[1])
    wrapr=[]; kind=[]
    for i,((g,a,b),c) in enumerate(zip(d['moves'],d['cls'])):
        k=None
        if d['lev'][i]==lab and F.xy(a,b)>1e-9:
            if c=='cut':
                da,db=dist(a),dist(b)
                L=F.xy(a,b); sag=da-math.sqrt(max(da*da-(L/2)**2,0)) if abs(da-db)<0.02 else 0
                k=('wrap' if sag<=0.5 else 'chord') if abs(da-db)<0.02 else 'other'
                if k=='wrap': wrapr.append(da)
            else: k=c
        kind.append(k)
    wrapr.sort()
    levels=[]
    for r in wrapr:
        if levels and r-levels[-1][-1]<0.05: levels[-1].append(r)
        else: levels.append([r])
    lv=[sum(x)/len(x) for x in levels if len(x)>=3]
    steps=[b-a for a,b in zip(lv,lv[1:])]
    # classify steps
    cls=collections.Counter(); dev=[]
    for a,b in zip(lv,lv[1:]):
        st=b-a; law=nxt(a)-a
        dev.append(st-law)
        if st> s+0.1: cls['gap > s (a level missing in the slice or a lobe)']+=1
        elif st>=law-0.1: cls['at the law (within the 0.075 bracket + 0.025 read)']+=1
        else: cls['under the law by > 0.1']+=1
    # rays: crossings of all cut moves
    ray=collections.Counter(); kept=[]; inner_outer=[]
    for ux,uy in ((1,0),(0,1),(-1,0),(0,-1)):
        ts=[]
        for i,((g,a,b),c) in enumerate(zip(d['moves'],d['cls'])):
            if c!='cut' or d['lev'][i]!=lab: continue
            ex,ey=b[0]-a[0],b[1]-a[1]; den=ux*ey-uy*ex
            if abs(den)<1e-12: continue
            wx,wy=a[0]-seed[0],a[1]-seed[1]
            t=(wx*ey-wy*ex)/den; u=(wx*uy-wy*ux)/den
            if t>0 and 0<=u<=1: ts.append((t,kind[i]))
        ts.sort(); P=[]
        for t,k in ts:
            if not P or t-P[-1][0]>0.05: P.append([t,{k}])
            else: P[-1][1].add(k)
        for (t0,k0),(t1,k1) in zip(P,P[1:]):
            g=t1-t0
            if g>6: continue
            kept.append(g)
            ray['_inner' if t0<25.45 else '_outer']+=0
            inner_outer.append((t0,g))
            both='wrap' in k0 and 'wrap' in k1
            if both:
                law=nxt(t0)-t0
                ray['wrap-wrap, at the law' if g>=law-0.1 and g<=s+0.1 else ('wrap-wrap, over s (skipped level)' if g>s+0.1 else 'wrap-wrap, under the law')]+=1
            elif 'chord' in k0 or 'chord' in k1: ray['involves a straight move across the cut (both ends on one level, sagitta > 0.5 mm)']+=1
            else: ray['involves a non-wrap cut (wall/residue/mop/turn/starter)']+=1
    med=lambda v:sorted(v)[len(v)//2]
    if out:
        print(arm,'levels',len(lv),'step median %.3f'%med(steps),'min %.3f max %.3f'%(min(steps),max(steps)))
        print('  steps vs law: median dev %.3f'%med(dev), dict(cls))
        print('  steps < s-0.1 (law inside rho*):',sum(1 for x in steps if x<s-0.1),'of',len(steps))
        print('  ray gaps n',len(kept),'median %.3f'%med(kept))
        for k,v in ray.most_common():
            if not k.startswith('_'): print('   ',v,k)
        rs=s*(2*R-s)/(2*(cd-s))
        inn=[g for t,g in inner_outer if t<rs]; out_=[g for t,g in inner_outer if t>=rs]
        print('  ray gaps inside rho* %.2f: %d, median %.3f; outside: %d, median %.3f'%(rs,len(inn),med(inn),len(out_),med(out_)))
        wk=[g for g in kept]
    return lv,steps,ray,kept
if __name__=='__main__':
    for arm in sys.argv[1:]: analyse(arm)
