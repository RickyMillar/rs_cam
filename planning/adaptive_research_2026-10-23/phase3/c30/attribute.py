"""C30: per producer and per level, the XY length of '3D Rough 1' cut that the
simulation reads over each bound (linear and arc samples, as fixture3d_v2.load_share)."""
import json, math, re, sys, pickle, collections
run=sys.argv[1]; tag=sys.argv[2]
R=3.0
d=json.load(open(run+'/simulation.json'))
S=[x for x in d['trace']['samples'] if x['toolpath_id']==1]
t=json.load(open(run+'/tp_1_3D_Rough_1.json'))
ents=sorted([i for i in t['semantic_trace']['items'] if i['kind']=='entry'], key=lambda i:i['move_start'])
an=t['debug_trace']['annotations']
gen_of_pass={}
for a in an:
    m=re.match(r'Pass (\d+) — ',a['label'])
    if m: gen_of_pass[int(m.group(1))]=a['move_index']
fin=sorted((gen_of_pass[e['params']['values']['pass_index']], e['move_start']) for e in ents)
def final_of(g):
    for gi,fi in fin:
        if gi>=g: return fi
    return t['move_count']
levels=[];wls=[]
for a in an:
    if a['label'].startswith('Adaptive Z'): levels.append((float(a['label'].split()[2]), final_of(a['move_index'])))
    if a['label']=='Waterline cleanup': wls.append(final_of(a['move_index']))
n=t['move_count']
level_of=[None]*n; phase=['?']*n; body_start={}
for k,(z,st) in enumerate(levels):
    en=levels[k+1][1] if k+1<len(levels) else n
    wl=wls[k]
    le=[e for e in ents if st<=e['move_start']<wl]
    body=le[1]['move_start']; body_start[z]=body
    for m in range(st,en):
        level_of[m]=z; phase[m]='perimeter' if m<body else ('body' if m<wl else 'waterline')
# the starter run: from the body's first entry to the next entry
nexts={e['move_start']:(ents[i+1]['move_start'] if i+1<len(ents) else n) for i,e in enumerate(ents)}
starter=set()
for z,b in body_start.items():
    starter.update(range(b, nexts[b]))
# last sample position of each move (its end), and plunge origins
end={}
for x in S: end[x['move_index']]=x['position']
first_sample={}
for x in S: first_sample.setdefault(x['move_index'],x)
origin=[None]*n; o=None; prev=None
for m in range(n):
    xs=first_sample.get(m)
    if xs and xs['source_intent']=='entry_plunge' and m in end and abs(xs['position'][0]-end[m][0])<1e-6 and abs(xs['position'][1]-end[m][1])<1e-6 and end[m][2]<xs['position'][2]:
        o=(end[m][0],end[m][1])
    origin[m]=o
def departing(m):
    if origin[m] is None or m==0 or (m-1) not in end: return False
    s=end[m-1]; return math.hypot(s[0]-origin[m][0],s[1]-origin[m][1])<R-1e-9
ceil2=0.3626
def ceil(s):
    a=math.acos(1-s/R)/(2*math.pi)*1.05; return (1-math.cos(2*math.pi*a))/2
c12=ceil(1.2)
B={'exact_s2':0.471,'screen_s2':ceil2+(0.5+0.3+0.1)/6,'exact_s1.2':c12+(0.5+0.1+0.05)/6,'screen_s1.2':c12+(0.5+0.3+0.1)/6}
rows=[]; prev=None; tot=collections.Counter(); totlev=collections.Counter()
for x in S:
    p=x['position']
    ln=math.hypot(p[0]-prev[0],p[1]-prev[1]) if prev else 0.0
    dz=abs(p[2]-prev[2]) if prev else 0.0
    pv=prev; prev=p
    if not x['is_cutting']: continue
    m=x['move_index']; r=x['engagement']['radial_woc_fraction']; k=x['cut_kinematics']
    ph=phase[m]; z=level_of[m]
    if k=='plunge' or ln<1e-6: prod='vertical (entry, lift)'
    elif ph=='perimeter': prod='perimeter sweep'
    elif ph=='waterline': prod='waterline cleanup'
    elif m in starter: prod='starter'
    elif x['source_intent']=='linking': prod='keep-down link (traverse)'
    elif departing(m): prod='plunge departure'
    elif dz>0.01: prod='draped (dz) body cut'
    else: prod='body wraps/turns/mop'
    lin = k in ('linear','arc')
    if lin: tot[prod]+=ln; totlev[z]+=ln
    rows.append((m,z,prod,k,ln,r,p,pv,x['sample_index']))
pickle.dump(dict(rows=rows,B=B,levels=levels,wls=wls,body_start=body_start),open(f'rows_{tag}.pkl','wb'))
print(tag,'bounds',{k:round(v,4) for k,v in B.items()})
print(tag,'levels',levels,'waterline starts',wls,'body starts',body_start)
cut=sum(tot.values()); print(tag,'linear+arc cut mm',round(cut,1))
for bn,bv in B.items():
    o=collections.defaultdict(float); ol=collections.defaultdict(float); oh=collections.defaultdict(float)
    for (m,z,prod,k,ln,r,p,pv,si) in rows:
        if r>bv:
            if k in ('linear','arc'): o[prod]+=ln; ol[(z,prod)]+=ln
            else: oh[prod]+=1
    s=sum(o.values())
    print(f'\n{tag} over {bn}={bv:.4f}: {s:.1f} mm = {100*s/cut:.2f}% of linear+arc cut')
    for prod in sorted(set(list(o)+list(oh)), key=lambda q:-o.get(q,0)):
        print(f'   {prod:30s} {o.get(prod,0):8.1f} mm   (+{oh.get(prod,0)} helix/plunge samples)')
    for z,_ in levels:
        print('   level',z, {p_:round(v,1) for (zz,p_),v in sorted(ol.items()) if zz==z})
