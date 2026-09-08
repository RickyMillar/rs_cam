import sys, re, numpy as np, struct
from matplotlib.tri import Triangulation, LinearTriInterpolator
html, tag = sys.argv[1], sys.argv[2]
s = open(html).read()
m = re.search(r'const stockVerts = new Float32Array\(\[([^\]]*)\]\)', s)
v = np.fromstring(m.group(1), sep=',', dtype=np.float32).reshape(-1, 3)
# STL (binary) terrain
f = open('/home/ricky/Downloads/wanaka200/rivmap_export/terrain.stl','rb'); f.read(80)
n = struct.unpack('<I', f.read(4))[0]
data = np.frombuffer(f.read(), dtype=np.dtype([('n','<3f4'),('v','<9f4'),('a','<u2')]), count=n)
tri = data['v'].reshape(-1,3,3).astype(np.float64)
pts = tri.reshape(-1,3)
uniq, inv = np.unique(np.round(pts[:, :2], 4), axis=0, return_inverse=True)
z = np.zeros(len(uniq)); z[inv] = pts[:, 2]
faces = inv.reshape(-1, 3)
T = Triangulation(uniq[:,0], uniq[:,1], faces)
interp = LinearTriInterpolator(T, z)
# stock top per column: keep vertices inside footprint, take max z per xy
inside = (v[:,0] > 1.0) & (v[:,0] < 199.0) & (v[:,1] > 1.0) & (v[:,1] < 199.0)
w = v[inside]
key = np.round(w[:,0]*10).astype(np.int64)*100000 + np.round(w[:,1]*10).astype(np.int64)
order = np.lexsort((-w[:,2], key)); w = w[order]; key = key[order]
first = np.r_[True, key[1:] != key[:-1]]
top = w[first]
mz = interp(top[:,0].astype(np.float64), top[:,1].astype(np.float64))
mz = np.ma.filled(mz, np.nan)
ok = ~np.isnan(mz)
dev = top[ok,2].astype(np.float64) - mz[ok]
gx, gy = interp.gradient(top[ok,0].astype(np.float64), top[ok,1].astype(np.float64))
slope = np.degrees(np.arctan(np.hypot(np.ma.filled(gx,0), np.ma.filled(gy,0))))
print(f'{tag}: columns {ok.sum()}  dev p50 {np.percentile(dev,50):.3f} p90 {np.percentile(dev,90):.3f} p99 {np.percentile(dev,99):.3f} max {dev.max():.3f}  (negative=overcut min {dev.min():.3f})')
for lo,hi in [(0,25),(25,45),(45,60),(60,90)]:
    sel = (slope>=lo)&(slope<hi)
    d = dev[sel]
    if len(d)==0: continue
    print(f'  slope {lo:2d}-{hi:2d} deg: {len(d):7d} cols ({100*len(d)/len(dev):4.1f}%)  p50 {np.percentile(d,50):.3f}  p90 {np.percentile(d,90):.3f}  p99 {np.percentile(d,99):.3f}  >0.3mm {100*np.mean(d>0.3):4.1f}%  >0.5mm {100*np.mean(d>0.5):4.1f}%')
np.save(f'{tag}_dev.npy', np.c_[top[ok,0], top[ok,1], dev, slope])
