"""C35 D21b model: replay a core spiral of exact circles on the 2D lattice.
Measures swept width as adaptive/search.rs swept_width_on (no slivers) and
measure_step (end reading plus along readings with the swept capsule)."""
import math, sys
import numpy as np
R=3.0; D=6.0; s=2.0; tol=0.1
c=(1-math.cos(1.05*math.acos(1-s/R)))/2; cD=c*D
def levels(r0, k):
    out=[r0]
    for _ in range(k):
        p=out[-1]
        n=-(R-cD)+math.sqrt((R-cD)**2+(p+R)**2-R**2)
        out.append(min(n,p+s))
    return out
def spiral(r0,K,per_turn,phase=0.0):
    lv=levels(r0,K)
    d=2*math.pi/per_turn; pts=[]
    for j in range(per_turn*(K+1)+1):
        th=j*d; turns=th/(2*math.pi)
        if turns>=K: r=lv[K]
        else:
            n=int(math.floor(turns)); r=lv[n]+(turns-n)*(lv[n+1]-lv[n])
        pts.append((r*math.cos(th+phase), r*math.sin(th+phase)))
    return pts, lv
class Grid:
    def __init__(s_,cell,ext,ox,oy):
        s_.cell=cell; n=int(2*ext/cell)+1
        s_.x0=-ext+ox; s_.y0=-ext+oy
        xs=s_.x0+np.arange(n)*cell; ys=s_.y0+np.arange(n)*cell
        s_.X,s_.Y=np.meshgrid(xs,ys); s_.m=np.ones_like(s_.X,bool)
    def clear_circle(s_,x,y,r): s_.m&=~((s_.X-x)**2+(s_.Y-y)**2<=r*r)
    def segd2(s_,a,b,X,Y):
        dx,dy=b[0]-a[0],b[1]-a[1]; L=dx*dx+dy*dy
        t=np.clip(((X-a[0])*dx+(Y-a[1])*dy)/L,0,1) if L>1e-20 else 0
        return (a[0]+t*dx-X)**2+(a[1]+t*dy-Y)**2
    def clear_segment(s_,a,b,r): s_.m&=~(s_.segd2(a,b,s_.X,s_.Y)<=r*r)
    def width(s_,cx,cy,ang,swept=None):
        sel=s_.m&((s_.X-cx)**2+(s_.Y-cy)**2<=R*R)
        if swept is not None and swept[0]!=swept[1]:
            sel&=~(s_.segd2(swept[0],swept[1],s_.X,s_.Y)<=R*R)
        elif swept is not None:
            sel&=~((s_.X-swept[0][0])**2+(s_.Y-swept[0][1])**2<=R*R)
        if not sel.any(): return 0.0
        u=(s_.X[sel]-cx)*(-math.sin(ang))+(s_.Y[sel]-cy)*math.cos(ang)
        return (u.max()-u.min())/D
    def step(s_,a,b):
        ang=math.atan2(b[1]-a[1],b[0]-a[0])
        w=s_.width(b[0],b[1],ang)
        if w>c: return w
        n=max(1,math.ceil(math.hypot(b[0]-a[0],b[1]-a[1])/s_.cell)); worst=w; prev=a
        for k in range(1,n):
            t=k/n; at=(a[0]+t*(b[0]-a[0]),a[1]+t*(b[1]-a[1]))
            r=s_.width(at[0],at[1],ang,(a,prev))
            if r>c: return r
            worst=max(worst,r); prev=at
        return worst
def subdivide(pts,L):
    out=[pts[0]]
    for a,b in zip(pts,pts[1:]):
        n=max(1,math.ceil(math.hypot(b[0]-a[0],b[1]-a[1])/L))
        for k in range(1,n+1): out.append((a[0]+k/n*(b[0]-a[0]),a[1]+k/n*(b[1]-a[1])))
    return out
def run(cell,per_turn,ox=0.0,oy=0.0,r0=3.0,K=4,step_len=1.5,verbose=False):
    pts,lv=spiral(r0,K,per_turn)
    g=Grid(cell,lv[-1]+R+1,ox,oy); g.clear_circle(0,0,r0+R)
    sp=subdivide(pts,step_len); g.clear_circle(*sp[0],R)
    overs=[]; mx=0
    for a,b in zip(sp,sp[1:]):
        w=g.step(a,b); mx=max(mx,w)
        if w>c:
            overs.append((math.hypot(*a),math.hypot(*b),w))
        g.clear_segment(a,b,R)
    return lv,overs,mx
if __name__=="__main__":
    print(f"c {c:.4f} cD {cD:.4f}")
    outer=levels(3.0,4)[-1]
    pt=math.ceil(math.pi/math.acos(1-tol/outer)); print("levels",[round(x,3) for x in levels(3.0,4)],"per_turn",pt)
    for cell,label in ((0.5,"lattice 0.5"),(0.05,"fine 0.05")):
        for per in (pt,4*pt):
            res=[]
            for (ox,oy) in ((0,0),(0.17,0.31),(0.25,0.25),(0.41,0.07)) if cell==0.5 else ((0,0),):
                lv,ov,mx=run(cell,per,ox,oy)
                res.append((len(ov),round(mx,4)))
            print(label,"per_turn",per,"(overs,max) by lattice phase:",res)
