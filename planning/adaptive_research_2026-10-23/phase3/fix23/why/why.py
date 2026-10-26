import json,sys,numpy as np,matplotlib
matplotlib.use("Agg"); import matplotlib.pyplot as plt
from collections import Counter
P3,out=sys.argv[1],sys.argv[2]
def load(d):
    S=[s for s in json.load(open(f"{P3}/{d}/simulation.json"))["trace"]["samples"] if s["toolpath_id"]==1]
    P=np.array([s["position"] for s in S]); I=np.array([s["source_intent"] for s in S])
    W=np.array([s["engagement"]["radial_woc_fraction"] or 0 for s in S]); F=np.array([s["feed_rate_mm_min"] or 0 for s in S])
    return P,I,W,F
for part,cols in [("corne",[("fix12/corne","C35 spiral"),("fix23/default/corne","D33 default")]),
                  ("wanaka200",[("fix12/wanaka200","C35 spiral"),("fix23/default/wanaka200","D33 default")])]:
    fig,axs=plt.subplots(1,2,figsize=(18,8),constrained_layout=True)
    for ax,(d,t) in zip(axs,cols):
        P,I,W,F=load(d); cut=I=="clearing_cut"
        ax.scatter(P[cut,0],P[cut,1],s=0.2,c="#bbbbbb",lw=0)
        o=cut&(W>0.471)
        sc=ax.scatter(P[o,0],P[o,1],s=4,c=P[o,2],cmap="viridis",lw=0)
        zc=Counter(np.round(P[o,2],1)); top=sorted(zc.items(),key=lambda x:-x[1])[:4]
        ax.set_title(f"{t}: {o.sum()} cut samples over woc 0.471 (colour=Z); top Z {top}",fontsize=9); ax.set_aspect("equal")
        fig.colorbar(sc,ax=ax,shrink=0.6)
    fig.savefig(f"{out}/why_{part}.png",dpi=80)
print("ok")
