"""D58: product job files for the Geodesic Adaptive strategy (tree 26d901943). From R's job files:
- corne_B_s2p1: phase3/d44/k_body_cp_edges_s2p1_d44_rest_regions.toml (fix31 B_s2p1, D45/D52);
- wanaka_s2p0 / wanaka_s3p5: phase3/fix33/jobs/wanaka200_body_cp_edges_d44_s{2p0,3p5}.toml (fix35 / fix34, D47-D54).
Changes: op 1 clearing_strategy agent_search -> geodesic_adaptive (the product value; R took the arm from the env and the
agent_search value); the edge op (Corne toolpath 5, wanaka toolpath 7) enabled = false (its DerivedRestRegions boundary
waits for step 7.5). A tomllib diff proves that nothing else moved."""
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
for tag, (src, edge) in SRC.items():
    txt = src.read_text()
    i1 = txt.index("[[setups.toolpaths]]\nid = 1\n"); j1 = txt.index("[[setups.toolpaths]]", i1 + 10)
    b1 = txt[i1:j1]; assert b1.count('clearing_strategy = "agent_search"') == 1
    txt2 = txt[:i1] + b1.replace('clearing_strategy = "agent_search"', 'clearing_strategy = "geodesic_adaptive"') + txt[j1:]
    ie = txt2.index(f"[[setups.toolpaths]]\nid = {edge}\n"); be = txt2[ie:ie + 200]; assert be.count("enabled = true\n") >= 1
    k = txt2.index("enabled = true\n", ie)
    txt2 = txt2[:k] + "enabled = false\n" + txt2[k + len("enabled = true\n"):]
    out = H / f"{tag}_geodesic_adaptive.toml"; out.write_text(txt2)
    o, n = dict(flat(tomllib.loads(txt))), dict(flat(tomllib.loads(txt2)))
    print(f"== {tag}: {src.relative_to(P3)} -> {out.name}")
    for key in sorted(set(o) | set(n)):
        if o.get(key, "<absent>") != n.get(key, "<absent>"): print(f"  {key}: {o.get(key, '<absent>')!r} -> {n.get(key, '<absent>')!r}")
