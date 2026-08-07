#!/usr/bin/env python3
"""W0 artifact generator — renders `planner_sim_dexel_parity_*` divergence.

Reads the verbatim stderr transcript of the parity test (captured with
`-- --exact --nocapture`) and draws the sampled interior violation cells on the
hemisphere footprint, coloured by sign of (planner_top - sim_top).

Why this render exists: it discriminates plan §H0 mechanism Candidate A
(planner stamp missing the drape -> divergence concentrated on the steep rim,
one consistent sign, max_dz > one cell) from Candidate C (F.a sub-cell blend
drift -> scattered on feature edges, both signs, max_dz ~ one cell).

Usage:
    python3 render_parity_violations.py <transcript.txt> <out.svg>

Transcript lines consumed (emitted by run_planner_sim_parity_with_mesh,
crates/rs_cam_core/src/adaptive3d/mod.rs:2196-2206):
    [<label>] PARITY: <d>/<t> cells differ > <tol>mm; interior <i>; \
        planner_higher <ph> (sim removed more); sim_higher <sh> (planner \
        removed more); max dz <m>mm
    (<row>, <col>) world (<x>, <y>) surface <s>: planner top <p>  sim top \
        <q>  D <dz>mm
"""

import re
import sys

HDR = re.compile(
    r"\[(?P<label>[^\]]+)\]\s+PARITY:\s+(?P<div>\d+)/(?P<total>\d+)\s+cells differ"
    r"\s+>\s+(?P<tol>[\d.]+)mm;\s+interior\s+(?P<interior>\d+);"
    r"\s+planner_higher\s+(?P<ph>\d+).*?sim_higher\s+(?P<sh>\d+).*?max dz\s+(?P<mdz>[\d.]+)mm"
)
VIO = re.compile(
    r"\(\s*(?P<row>\d+),\s*(?P<col>\d+)\)\s+world\s+\(\s*(?P<x>-?[\d.]+),"
    r"\s*(?P<y>-?[\d.]+)\)\s+surface\s+(?P<surf>-?[\d.]+):\s+planner top"
    r"\s+(?P<p>-?[\d.]+)\s+sim top\s+(?P<s>-?[\d.]+)\s+\S+\s+(?P<dz>[\d.]+)mm"
)

MESH_R = 20.0  # make_test_hemisphere(20.0, 16)
TOOL_R = 3.175


def parse(path):
    hdr, vios = None, []
    with open(path, errors="replace") as f:
        for line in f:
            m = HDR.search(line)
            if m:
                hdr = m.groupdict()
                continue
            m = VIO.search(line)
            if m:
                vios.append({k: float(v) for k, v in m.groupdict().items()})
    return hdr, vios


def render(hdr, vios, out):
    W, H = 700, 620
    cx, cy = 330, 300
    scale = 240.0 / (MESH_R + TOOL_R)

    def px(x):
        return cx + x * scale

    def py(y):
        return cy - y * scale

    b = []
    b.append(f'<rect width="{W}" height="{H}" fill="#fff"/>')
    lbl = hdr["label"] if hdr else "?"
    b.append(f'<text x="14" y="24" style="font:14px monospace;font-weight:bold">'
             f'W0 / test 3 — planner-vs-sim dexel divergence, sampled interior cells'
             f'</text>')
    b.append(f'<text x="14" y="44" style="font:11px monospace;fill:#444">'
             f'fixture: {lbl}; hemisphere r=20, flat endmill r=3.175, cell 0.5292 mm'
             f'</text>')
    # mesh footprint + stock footprint
    b.append(f'<circle cx="{px(0)}" cy="{py(0)}" r="{MESH_R*scale}" fill="#f4f0e8" '
             f'stroke="#8b5a2b" stroke-width="1.5"/>')
    b.append(f'<rect x="{px(-MESH_R-TOOL_R)}" y="{py(MESH_R+TOOL_R)}" '
             f'width="{2*(MESH_R+TOOL_R)*scale}" height="{2*(MESH_R+TOOL_R)*scale}" '
             f'fill="none" stroke="#aaa" stroke-dasharray="4 3"/>')
    # interior population boundary (mesh bbox inset 1mm) — what the test counts
    b.append(f'<rect x="{px(-MESH_R+1)}" y="{py(MESH_R-1)}" '
             f'width="{2*(MESH_R-1)*scale}" height="{2*(MESH_R-1)*scale}" '
             f'fill="none" stroke="#06c" stroke-dasharray="6 4"/>')
    b.append(f'<text x="{px(-MESH_R+1)}" y="{py(MESH_R-1)-6}" '
             f'style="font:10px monospace;fill:#06c">interior population '
             f'(mesh bbox inset 1 mm)</text>')
    # iso-radius guides — Candidate A predicts clustering at large rho
    for rho in (10, 15, 18, 19.5):
        b.append(f'<circle cx="{px(0)}" cy="{py(0)}" r="{rho*scale}" fill="none" '
                 f'stroke="#ddd"/>')
        b.append(f'<text x="{px(0)+rho*scale+2}" y="{py(0)-3}" '
                 f'style="font:9px monospace;fill:#bbb">r={rho}</text>')

    maxdz = max([v["dz"] for v in vios], default=1.0)
    for v in vios:
        s_higher = v["s"] > v["p"]          # planner removed more
        col = "#c0392b" if s_higher else "#1f6fb2"
        r = 3.0 + 6.0 * (v["dz"] / maxdz if maxdz else 0)
        b.append(f'<circle cx="{px(v["x"])}" cy="{py(v["y"])}" r="{r:.1f}" '
                 f'fill="{col}" fill-opacity="0.55" stroke="{col}"/>')

    y = H - 130
    b.append(f'<circle cx="26" cy="{y-4}" r="6" fill="#c0392b" fill-opacity="0.55" '
             f'stroke="#c0392b"/>')
    b.append(f'<text x="42" y="{y}" style="font:11px monospace">sim top higher — '
             f'the PLANNER removed more than its own emitted path does '
             f'(Candidate A signature)</text>')
    b.append(f'<circle cx="26" cy="{y+18}" r="6" fill="#1f6fb2" fill-opacity="0.55" '
             f'stroke="#1f6fb2"/>')
    b.append(f'<text x="42" y="{y+22}" style="font:11px monospace">planner top higher '
             f'— the SIMULATOR removed more</text>')
    b.append(f'<text x="14" y="{y+44}" style="font:10px monospace;fill:#666">'
             f'marker area scales with |dz|; largest = {maxdz:.2f} mm</text>')
    if hdr:
        b.append(f'<text x="14" y="{y+64}" style="font:11px monospace">'
                 f'{hdr["div"]}/{hdr["total"]} cells differ &gt; {hdr["tol"]} mm; '
                 f'interior {hdr["interior"]}; planner_higher {hdr["ph"]}; '
                 f'sim_higher {hdr["sh"]}; max dz {hdr["mdz"]} mm</text>')
        b.append(f'<text x="14" y="{y+82}" style="font:10px monospace;fill:#666">'
                 f'only the first 20 interior violations are collected by the '
                 f'harness — this is a sample, not the full field</text>')

    svg = (f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" '
           f'viewBox="0 0 {W} {H}">\n' + "\n".join(b) + "\n</svg>\n")
    with open(out, "w") as f:
        f.write(svg)


if __name__ == "__main__":
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    h, v = parse(sys.argv[1])
    if h is None:
        sys.exit(f"no PARITY header line found in {sys.argv[1]}")
    render(h, v, sys.argv[2])
    print(f"header: {h}")
    print(f"violations parsed: {len(v)}")
