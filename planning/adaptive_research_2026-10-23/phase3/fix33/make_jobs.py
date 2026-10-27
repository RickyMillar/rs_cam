"""D47: the wanaka200 job files. A copy of planning/multitool_2026-08-23/wanaka200_mt1_c12_rough_only_body_cp_edges.toml with
(a) toolpath 7's boundary (the CP-rest edge op) = DerivedRestRegions of toolpath 1, containment center, offset 0
    (as phase3/d44/k_body_cp_edges_s2p1_d44_rest_regions.toml), and (b) for s3p5, toolpath 1's stepover 2.0 -> 3.5 (1.75x).
Text edits on the original; then a tomllib field-by-field diff proves that nothing else moved."""
import re, sys, tomllib
from pathlib import Path
R = Path("/home/ricky/personal_repos/rs_cam/planning")
SRC = R / "multitool_2026-08-23/wanaka200_mt1_c12_rough_only_body_cp_edges.toml"
H = Path(__file__).resolve().parent / "jobs"
txt = SRC.read_text()
i7 = txt.index("[[setups.toolpaths]]\nid = 7\n"); j7 = txt.index("[[setups]]", i7)
blk = txt[i7:j7]
old = '[setups.toolpaths.boundary]\nenabled = true\nsource = "model_silhouette"\ncontainment = "inside"\noffset = 0.0\n'
new = '[setups.toolpaths.boundary]\nenabled = true\ncontainment = "center"\noffset = 0.0\n\n[setups.toolpaths.boundary.source.derived_rest_regions]\nsource_toolpath_id = 1\n'
assert blk.count(old) == 1
base = txt[:i7] + blk.replace(old, new) + txt[j7:]
i1 = base.index("[[setups.toolpaths]]\nid = 1\n")
def with_step(s):
    b = base[i1:]; k = b.index("stepover = 2.0\n")
    return base[:i1] + b[:k] + f"stepover = {s}\n" + b[k + len("stepover = 2.0\n"):]
def flat(d, p=""):
    if isinstance(d, dict):
        for k, v in d.items(): yield from flat(v, f"{p}.{k}" if p else k)
    elif isinstance(d, list):
        for i, v in enumerate(d): yield from flat(v, f"{p}[{i}]")
    else: yield p, d
o = dict(flat(tomllib.loads(txt)))
for tag, s in (("s2p0", "2.0"), ("s3p5", "3.5")):
    t = with_step(s); (H / f"wanaka200_body_cp_edges_d44_{tag}.toml").write_text(t)
    n = dict(flat(tomllib.loads(t)))
    print(f"== {tag}: fields changed against the original")
    for k in sorted(set(o) | set(n)):
        if o.get(k, "<absent>") != n.get(k, "<absent>"): print(f"  {k}: {o.get(k, '<absent>')!r} -> {n.get(k, '<absent>')!r}")
