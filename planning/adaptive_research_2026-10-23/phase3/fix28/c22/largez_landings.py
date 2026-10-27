"""C22 §19.2: the LargeZ entries of D38 (route_default_cut_feed, op 1) by landing Z and drop (C40 entry lines).
Writes largez_landings.json."""
import collections, json, re
from pathlib import Path
H = Path(__file__).resolve().parent
by_z = collections.Counter(); by_zd = collections.Counter()
for ln in open(H.parent / "route_default_cut_feed" / "stdout_plain.txt"):
    if "C40 entry kind" in ln and "cause=Some(LargeZ)" in ln:
        kv = dict(re.findall(r"(\w+)=([-0-9.]+)", ln.split("C40 entry", 1)[1]))
        z, d = float(kv["z"]), float(kv["drop_mm"])
        band = "4.0" if abs(z - 4.0) < 1e-3 else "18.5" if abs(z - 18.5) < 1e-3 else "0.5" if abs(z - 0.5) < 1e-3 else "17.0" if abs(z - 17.0) < 1e-3 else "17.0-18.5 draped rim"
        by_z[band] += 1; by_zd[f"{z:.2f} drop {d:.1f}"] += 1
out = {"total": sum(by_z.values()), "by landing": dict(by_z.most_common()), "top landing/drop": dict(by_zd.most_common(6))}
print(json.dumps(out, indent=1)); json.dump(out, open(H / "largez_landings.json", "w"), indent=1)
