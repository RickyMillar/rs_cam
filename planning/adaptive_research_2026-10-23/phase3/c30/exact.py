"""C30: the exact swept width (C18's exact_width_at, on the sample path) of each
linear/arc sample the simulation reads over the lowest bound."""
import json, math, sys, pickle, time
import numpy as np
from scipy.spatial import cKDTree
run=sys.argv[1]; tag=sys.argv[2]
R=3.0; PITCH=0.05
d=json.load(open(run+'/simulation.json'))
S=[x for x in d['trace']['samples'] if x['toolpath_id']==1]
P=np.array([x['position'] for x in S]); N=len(S)
meta=pickle.load(open(f'rows_{tag}.pkl','rb'))
lo=min(meta['B'].values())
# index of each sample in S by sample_index
pos_of={x['sample_index']:i for i,x in enumerate(S)}
todo=[r for r in meta['rows'] if r[5]>lo and r[3] in ('linear','arc')]
print(tag,'samples to read',len(todo)); t0=time.time()
g=np.arange(-R,R+PITCH/2,PITCH); GX,GY=np.meshgrid(g,g); inside=GX**2+GY**2<=R*R
GX=GX[inside]; GY=GY[inside]
out={}
for (m,z,prod,k,ln,r,p,pv,si) in todo:
    i=pos_of[si]
    if i==0: continue
    frm=P[i-1,:2]; to=P[i,:2]; mid=(frm+to)/2
    dv=to-frm; l=max(np.hypot(*dv),1e-9); sx,sy=-dv[1]/l,dv[0]/l
    Q=P[:i]
    msk=(np.abs(Q[:,0]-mid[0])<=2*R+1e-9)&(np.abs(Q[:,1]-mid[1])<=2*R+1e-9)&(Q[:,2]<=P[i,2]+0.01)
    L=np.column_stack([mid[0]+GX,mid[1]+GY])
    if msk.any():
        dist,_=cKDTree(Q[msk,:2]).query(L,distance_upper_bound=R+1e-12)
        unc=~(dist<=R)
    else:
        unc=np.ones(len(L),bool)
    if not unc.any(): w=0.0
    else:
        u=GX[unc]*sx+GY[unc]*sy; w=(u.max()-u.min())/(2*R)
    out[si]=w
print(tag,'done',round(time.time()-t0,1),'s')
pickle.dump(out,open(f'exact_{tag}.pkl','wb'))
