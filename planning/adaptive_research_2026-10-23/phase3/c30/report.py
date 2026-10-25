import pickle, sys, collections, numpy as np
tag=sys.argv[1]
M=pickle.load(open(f'rows_{tag}.pkl','rb')); E=pickle.load(open(f'exact_{tag}.pkl','rb'))
B=M['B']; rows=[r for r in M['rows'] if r[3] in ('linear','arc')]
def tab(cond,label):
    o=collections.defaultdict(float); ol=collections.defaultdict(float)
    for r in rows:
        if cond(r): o[r[2]]+=r[4]; ol[(r[1],r[2])]+=r[4]
    print(f'  {label}: total {sum(o.values()):.1f} mm;', {k:round(v,1) for k,v in sorted(o.items(),key=lambda q:-q[1])})
    for z,_ in M['levels']:
        print('     Z',z,{p:round(v,1) for (zz,p),v in sorted(ol.items()) if zz==z})
print(tag)
tab(lambda r:r[5]>0.471,'sim > 0.471')
tab(lambda r:r[5]>B['screen_s2'],'sim > screen 0.5126')
tab(lambda r:r[5]>0.471 and E.get(r[8],0)>0.471,'sim > 0.471 AND exact > 0.471')
tab(lambda r:r[5]>0.471 and E.get(r[8],0)<=0.471,'sim > 0.471, exact <= 0.471 (sim-only)')
tab(lambda r:r[5]>B['exact_s1.2'],'sim > 0.3272 (s 1.2 exact bound)')
tab(lambda r:r[5]>B['exact_s1.2'] and E.get(r[8],0)>B['exact_s1.2'],'sim > 0.3272 AND exact > 0.3272')
# exact distribution per producer for sim>0.471
for prod in sorted(set(r[2] for r in rows if r[5]>0.471)):
    ex=np.array([E[r[8]] for r in rows if r[5]>0.471 and r[2]==prod and r[8] in E]); sm=np.array([r[5] for r in rows if r[5]>0.471 and r[2]==prod])
    print(f'  {prod}: n {len(ex)}, sim median {np.median(sm):.3f} max {sm.max():.3f}; exact median {np.median(ex):.3f} min {ex.min():.3f} max {ex.max():.3f}')
