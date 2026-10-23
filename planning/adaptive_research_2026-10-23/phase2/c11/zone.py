# C11 prototype: the far-side zone of ux_2d_pocket in tool-centre space.
# Region: rounded rect 5..75 x 5..55 rx 8, island r 10 at (40,30); R 3.
import math
R=3.0; s=2.0; cell=0.5; cD=0.3626*6
cx,cy,hr=40.0,30.0,10.0+R
seed=(18.5,18.5)
def inside(x,y):
    if not (8<=x<=72 and 8<=y<=52): return False
    rr=8-R
    for (qx,qy) in ((13,13),(67,13),(67,47),(13,47)):
        if ((x<13 and qx==13) or (x>67 and qx==67)) and ((y<13 and qy==13) or (y>47 and qy==47)):
            if math.hypot(x-qx,y-qy)>rr: return False
    return math.hypot(x-cx,y-cy)>=hr
def hidden(x,y):
    ax,ay=seed; dx,dy=x-ax,y-ay; L2=dx*dx+dy*dy
    t=max(0,min(1,((cx-ax)*dx+(cy-ay)*dy)/L2)) if L2>0 else 0
    return math.hypot(ax+t*dx-cx,ay+t*dy-cy)<hr
nodes=[(8+i*cell,8+j*cell) for i in range(int(64/cell)+1) for j in range(int(44/cell)+1)]
nodes=[p for p in nodes if inside(*p)]
zone=[p for p in nodes if hidden(*p)]
vis=[p for p in nodes if not hidden(*p)]
print("reachable nodes",len(nodes),"area",len(nodes)*cell*cell)
print("zone nodes",len(zone),"area",len(zone)*cell*cell)
d=math.hypot(cx-seed[0],cy-seed[1]); a0=math.atan2(cy-seed[1],cx-seed[0]); h=math.asin(hr/d)
up=a0+h; lo=a0-h
print("shadow lines (deg)",math.degrees(lo),math.degrees(up))
# distance of zone nodes to each shadow line (perpendicular), split at the bisector
def dline(p,ang):
    return abs((p[0]-seed[0])*math.sin(ang)-(p[1]-seed[1])*math.cos(ang))
up_side=[p for p in zone if dline(p,up)<=dline(p,lo)]
lo_side=[p for p in zone if dline(p,up)>dline(p,lo)]
mu=max(dline(p,up) for p in up_side); ml=max(dline(p,lo) for p in lo_side)
print("upper front: nodes",len(up_side),"max depth",round(mu,2)," lower front: nodes",len(lo_side),"max depth",round(ml,2))
def law_levels(tmax):
    b=R-cD; rho=R; out=[0.0]
    while True:
        n=min(-b+math.sqrt(b*b+(rho+R)**2-R*R),rho+s)
        if n-R>tmax: break
        rho=n; out.append(rho-R)
    return out
print("zone levels under the law: upper",len(law_levels(mu))-1,"lower",len(law_levels(ml))-1)
print("zone levels plain: upper",int(mu//s),"lower",int(ml//s))
tv=max(math.hypot(p[0]-seed[0],p[1]-seed[1]) for p in vis)-R
print("first region max T",round(tv,2),"levels under the law",len(law_levels(tv))-1)
tz=max(math.hypot(p[0]-seed[0],p[1]-seed[1]) for p in nodes)-R
