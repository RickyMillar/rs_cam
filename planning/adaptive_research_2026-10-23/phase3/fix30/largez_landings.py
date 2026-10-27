"""D42: a copy of C22's fix28/c22/largez_landings.py with the run as an argument
(the LargeZ C40 entries of op 1 by landing Z and drop). Usage: largez_landings.py <run dir under phase3>"""
import collections, json, re, sys
from pathlib import Path
H = Path(__file__).resolve().parent; P3 = H.parent
run = sys.argv[1]
by_z = collections.Counter(); by_zd = collections.Counter(); by_exact = collections.Counter()
for ln in open(P3 / run / "stdout_plain.txt"):
    if "C40 entry kind" in ln and "cause=Some(LargeZ)" in ln:
        kv = dict(re.findall(r"(\w+)=([-0-9.]+)", ln.split("C40 entry", 1)[1]))
        z, d = float(kv["z"]), float(kv["drop_mm"])
        band = "4.0" if abs(z - 4.0) < 1e-3 else "18.5" if abs(z - 18.5) < 1e-3 else "0.5" if abs(z - 0.5) < 1e-3 else "17.0" if abs(z - 17.0) < 1e-3 else "17.0-18.5 draped rim" if 17.0 < z < 18.5 else "other"
        by_z[band] += 1; by_zd[f"{z:.2f} drop {d:.1f}"] += 1; by_exact[f"{z:.2f}"] += 1
out = {"run": run, "total": sum(by_z.values()), "by landing": dict(by_z.most_common()), "by landing z": dict(by_exact.most_common(12)), "top landing/drop": dict(by_zd.most_common(6))}
print(json.dumps(out)); json.dump(out, open(H / f"largez_landings_{run.replace('/', '_')}.json", "w"), indent=1)
