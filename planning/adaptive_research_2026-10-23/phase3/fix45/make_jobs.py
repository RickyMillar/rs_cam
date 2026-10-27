"""D64: one job file per op-1 strategy and part, from fix39's D58 product files (op 1 only matters; the edge op disabled):
corne_B_s2p1 (stepover 2.1) and wanaka_s2p0 (2.0). Only op 1's clearing_strategy changes. tomllib diff in jobs/diff.txt."""
import tomllib
from pathlib import Path
P3 = Path(__file__).resolve().parent.parent; H = Path(__file__).resolve().parent / "jobs"
STRATS = ("contour_parallel", "adaptive", "agent_search", "contour_spiral", "geodesic_adaptive")
def flat(d, p=""):
    if isinstance(d, dict):
        for k, v in d.items(): yield from flat(v, f"{p}.{k}" if p else k)
    elif isinstance(d, list):
        for i, v in enumerate(d): yield from flat(v, f"{p}[{i}]")
    else: yield p, d
lines = []
for part in ("corne_B_s2p1", "wanaka_s2p0"):
    src = (P3 / f"fix39/jobs/{part}_geodesic_adaptive.toml").read_text(); assert src.count('clearing_strategy = "geodesic_adaptive"') == 1
    for s in STRATS:
        t = src.replace('clearing_strategy = "geodesic_adaptive"', f'clearing_strategy = "{s}"'); (H / f"{part}_{s}.toml").write_text(t)
        o, n = dict(flat(tomllib.loads(src))), dict(flat(tomllib.loads(t)))
        lines.append(f"{part}_{s}: " + ("; ".join(f"{k}: {o.get(k)!r} -> {n.get(k)!r}" for k in sorted(set(o) | set(n)) if o.get(k) != n.get(k)) or "= fix39's file"))
(H / "diff.txt").write_text("\n".join(lines) + "\n"); print("\n".join(lines))
