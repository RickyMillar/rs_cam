"""Split a concatenated rs_cam program by setup and by toolpath (measurement only).

Run:  python3 planning/rivmap_block_job/split_gcode.py <project.toml> <program.nc> <out_dir>

Writes <out_dir>/setup_<n>.nc and <out_dir>/op_<id>.nc. Each file gets the
program preamble (the lines before the first toolpath comment), the modal
position at its start (a G0 to the last position, at the current Z first),
and M5/M30. Use `rs_cam_cli nc-time` on them for per-setup cycle times.
"""
import re
import sys
import tomllib
from pathlib import Path

proj = tomllib.loads(Path(sys.argv[1]).read_text())
lines = Path(sys.argv[2]).read_text().splitlines()
out = Path(sys.argv[3]); out.mkdir(parents=True, exist_ok=True)
names = {tp["name"].replace("(", "[").replace(")", "]"): (si, tp["id"]) for si, s in enumerate(proj["setups"]) for tp in s["toolpaths"]}
pre, blocks, cur = [], [], None
for ln in lines:
    m = re.fullmatch(r"\((.*)\)", ln.strip())
    if m and m.group(1) in names:
        cur = [m.group(1), [ln]]
        blocks.append(cur)
    elif cur is None:
        pre.append(ln)
    else:
        cur[1].append(ln)
pre = [l for l in pre if not l.startswith(("M5", "M30"))]
by_setup = {}
for name, body in blocks:
    body = [l for l in body if l.strip() not in ("M5", "M30")]
    si, tid = names[name]
    by_setup.setdefault(si, []).extend(body)
    (out / f"op_{tid}.nc").write_text("\n".join(pre + body + ["M5", "M30"]) + "\n")
for si, body in by_setup.items():
    (out / f"setup_{si + 1}.nc").write_text("\n".join(pre + body + ["M5", "M30"]) + "\n")
print(sorted(by_setup), len(blocks))
