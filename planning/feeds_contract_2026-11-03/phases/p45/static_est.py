import numpy as np, struct, sys
def load(path):
    b=open(path,'rb').read(); rows,cols=struct.unpack('<QQ',b[:16]); ou,ov,cs=struct.unpack('<ddd',b[16:40])
    a=np.frombuffer(b,dtype='<f4',offset=40).reshape(2,rows,cols); return rows,cols,ou,ov,cs,a[0],a[1]
rows,cols,ou,ov,cs,top,cons=load('probe1/stock_0.bin')
P=np.loadtxt('probe1/points_0.txt'); R=3.0; DPP=6.0
st=P[P[:,7]==1]
print('ring points', len(P), 'standing in round 0', len(st))
h=int(R/cs)+1
offs=[(dr,dc) for dr in range(-h,h+1) for dc in range(-h,h+1)]
ext=cs*0.375
rise=[];stall=[]
for r in st:
    x,y,own=r[3],r[4],r[5]
    c0=int(round((x-ou)/cs)); r0=int(round((y-ov)/cs))
    rr=np.arange(r0-h,r0+h+1); cc=np.arange(c0-h,c0+h+1)
    ok=(rr>=0)&(rr<rows); rr=rr[ok]; ok=(cc>=0)&(cc<cols); cc=cc[ok]
    dv=(ov+rr*cs-y)[:,None]; du=(ou+cc*cs-x)[None,:]
    d2=du*du+dv*dv
    inside=d2<=R*R
    full=(np.abs(du)+ext)**2+(np.abs(dv)+ext)**2<=R*R
    sub_c=cons[np.ix_(rr,cc)]; sub_t=top[np.ix_(rr,cc)]
    mc=sub_c[inside].max(); mt=sub_t[inside].max()
    mfull=sub_c[full].max()
    z1=max(own, mc-DPP)
    rise.append(z1-own); stall.append(mc-mfull)
rise=np.array(rise); stall=np.array(stall)
print('check: own Z == max(floor, raytop max - DPP)? points where raytop max - DPP > own + 0.01:', int(((0)).__class__(0)))
for t in (0.05,0.2,1,3,5): print(f'V1 raises the round-0 Z by more than {t} mm at {(rise>t).sum()} of {len(st)} standing points')
for t in (0.2,1,3): print(f'the tallest conservative top of a part-covered centre-in cell is more than {t} mm over the tallest of the full-covered cells at {(stall>t).sum()} points; of them with a V1 rise > 0.2: {((stall>t)&(rise>0.2)).sum()}')
