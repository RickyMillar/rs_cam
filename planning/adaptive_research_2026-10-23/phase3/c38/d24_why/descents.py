"""C38 D24 analysis, step 3: op 1's descent events (fix11/audit.py, in the
fix12 = C35 and fix14 = D24 audit json) by place, from the sample position
of each event's first move (model frame); and the per-op runtimes."""
import json
from pathlib import Path
import numpy as np
H = Path(__file__).resolve().parent
P3 = H.parent.parent
exec((H.parent / "d22_why" / "why.py").read_text().split("C_OP1 = ")[0])
out = {}
for run, aud in (("c35", "fix12/audit_corne.json"), ("d24", "fix14/audit_corne.json")):
    A = json.load(open(P3 / aud))
    S = np.load(H / f"samples_{run}.npz"); sel = S["tp"] == 1
    first = {}
    for i in np.nonzero(sel)[0]:
        first.setdefault(int(S["mv"][i]), i)
    ev = A["descents"] if isinstance(A["descents"], list) else []
    # the json keeps the per-descent list; events are runs of them
    pts, tt, z = [], [], []
    for d in ev:
        j = first.get(int(d["move"]))
        if j is None:
            continue
        pts.append(S["pos"][j].astype(float)); tt.append(d["time_s"])
    P = np.array(pts); w = wall(P); T = np.array(tt)
    strip = (w > 3.0) & (w <= 6.2)
    out[run] = {"descents_listed": int(len(P)), "in_outer_strip": int(strip.sum()),
                "inside_silhouette": int((w <= 0).sum()), "other": int(((w > 0) & ~strip).sum()),
                "time_s_in_strip": round(float(T[strip].sum()), 1), "time_s_all": round(float(T.sum()), 1),
                "strip_by_level": {f"{v:.1f}": int((strip & (np.abs(P[:, 2] - v) < 0.6)).sum()) for v in (17.0, 11.0, 5.0, 0.5)}}
for run, d in (("c35", "fix12/corne"), ("d24", "fix14/corne"), ("c38a", "c38/corne_c38a"), ("d23pick", "fix13/corne_pick")):
    pass
print(json.dumps(out, indent=1))
(H / "descents.json").write_text(json.dumps(out, indent=1))
