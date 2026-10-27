"""D60: the product job files with the edge op on: fix39's files with the edge op (Corne toolpath 5, wanaka toolpath 7)
enabled = true again. Its boundary is already R's d44 boundary (derived_rest_regions from toolpath 1, containment center,
offset 0), so against R's file only op 1's clearing_strategy moves (agent_search -> geodesic_adaptive). tomllib diff
against R's file below."""
import tomllib
from pathlib import Path
P3 = Path(__file__).resolve().parent.parent; H = Path(__file__).resolve().parent / "jobs"
SRC = {"corne_B_s2p1": (P3 / "d44/k_body_cp_edges_s2p1_d44_rest_regions.toml", 5),
       "wanaka_s2p0": (P3 / "fix33/jobs/wanaka200_body_cp_edges_d44_s2p0.toml", 7),
       "wanaka_s3p5": (P3 / "fix33/jobs/wanaka200_body_cp_edges_d44_s3p5.toml", 7)}
def flat(d, p=""):
    if isinstance(d, dict):
        for k, v in d.items(): yield from flat(v, f"{p}.{k}" if p else k)
    elif isinstance(d, list):
        for i, v in enumerate(d): yield from flat(v, f"{p}[{i}]")
    else: yield p, d
for tag, (rsrc, edge) in SRC.items():
    txt = (P3 / f"fix39/jobs/{tag}_geodesic_adaptive.toml").read_text()
    ie = txt.index(f"[[setups.toolpaths]]\nid = {edge}\n"); k = txt.index("enabled = false\n", ie)
    assert k < ie + 200
    txt = txt[:k] + "enabled = true\n" + txt[k + len("enabled = false\n"):]
    (H / f"{tag}_product_edges.toml").write_text(txt)
    o, n = dict(flat(tomllib.loads(rsrc.read_text()))), dict(flat(tomllib.loads(txt)))
    print(f"== {tag}: against {rsrc.relative_to(P3)}")
    for key in sorted(set(o) | set(n)):
        if o.get(key, "<absent>") != n.get(key, "<absent>"): print(f"  {key}: {o.get(key, '<absent>')!r} -> {n.get(key, '<absent>')!r}")
