#!/usr/bin/env python3
"""W0 artifact generator — renders the `adaptive3d::path::tests` fixture geometry.

No Cargo, no crate changes. Reproduces, in pure Python, the exact arithmetic
that `segments_to_toolpath` + `drape_point` + `drape_path_to_leave` +
`emit_peck_plunge` perform on the two red tests' fixtures, and draws the
XZ cross-section so the geometric contradiction is visible.

Sources mirrored (read-only):
  crates/rs_cam_core/src/adaptive3d/path.rs
    drape_point            (l.1140) z' = max(z, drop_cutter(x,y) + stock_to_leave)
    drape_path_to_leave    (l.1155) same, per densified cut point
    emit_peck_plunge       (l.39)   while current_z - entry.z > dpp + 1e-6
  crates/rs_cam_core/src/mesh.rs
    make_test_flat(100.0)  (l.769)  flat quad, z == 0, x,y in [-50, 50]

Outputs (written next to this script):
  peck_fixture_xz.svg
  rapid_fixture_xz.svg
  path_fixture_moves.txt
"""

import os

OUT = os.path.dirname(os.path.abspath(__file__))

# ── fixture constants, copied from minimal_params() in path.rs l.1616 ──
STOCK_TO_LEAVE = 0.5
PECK_CLEARANCE_MM = 0.5  # emit_peck_plunge const, path.rs l.41
MESH_Z = 0.0  # make_test_flat surface height
TOL = 1e-6


def drop_cutter(_x, _y):
    """Flat mesh at z=0 spans x,y in [-50,50]; every fixture XY is on it."""
    return MESH_Z


def drape(p, stock_to_leave=STOCK_TO_LEAVE):
    x, y, z = p
    return (x, y, max(z, drop_cutter(x, y) + stock_to_leave))


def emit_peck_plunge(entry, start_z, dpp, plunge_moves):
    """Mirror of path.rs emit_peck_plunge. Appends (kind, z) tuples."""
    dpp = max(dpp, 0.1)
    current_z = start_z
    while current_z - entry[2] > dpp + TOL:
        next_z = current_z - dpp
        plunge_moves.append(("EntryPlunge feed", next_z))
        plunge_moves.append(("Retract rapid", next_z + PECK_CLEARANCE_MM))
        current_z = next_z
    plunge_moves.append(("EntryPlunge feed", entry[2]))


def peck_test_moves(draped):
    """Test 1: peck_plunge_progresses_when_depth_per_pass_equals_retract_clearance.

    params.safe_z = 1.0, params.depth_per_pass = 0.5, one Rapid(0,0,0).
    """
    safe_z, dpp = 1.0, 0.5
    entry = (0.0, 0.0, 0.0)
    if draped:
        entry = drape(entry)
    moves = [("Linking rapid to (entry.xy, safe_z)", safe_z)]
    emit_peck_plunge(entry, safe_z, dpp, moves)
    moves.append(("final Retract rapid to safe_z", safe_z))
    return entry, moves


def rapid_test_paths(draped):
    """Test 2: rapid_segment_lifts_to_safe_z_before_traverse.

    Cut[(0,0,-3) -> (5,5,-3)], Rapid(20,20,-2), Cut[(20,20,-2) -> (25,25,-2)].
    """
    cut1 = [(0.0, 0.0, -3.0), (5.0, 5.0, -3.0)]
    entry = (20.0, 20.0, -2.0)
    cut2 = [(20.0, 20.0, -2.0), (25.0, 25.0, -2.0)]
    if draped:
        cut1 = [drape(p) for p in cut1]
        entry = drape(entry)
        cut2 = [drape(p) for p in cut2]
    return cut1, entry, cut2


# ── SVG helper ─────────────────────────────────────────────────────────
def svg(width, height, body, title):
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" '
        f'height="{height}" viewBox="0 0 {width} {height}">\n'
        f'<style>text{{font:12px monospace}} .t{{font:14px monospace;font-weight:bold}}'
        f'.s{{font:10px monospace;fill:#555}}</style>\n'
        f'<rect width="{width}" height="{height}" fill="#fff"/>\n'
        f'<text class="t" x="14" y="22">{title}</text>\n'
        + body
        + "\n</svg>\n"
    )


def make_peck_svg():
    W, H = 980, 400
    x0, y0 = 90, 300          # origin of the plot, world (0, z=-1)
    sx, sz = 40.0, 60.0       # px per mm

    def px(x):
        return x0 + x * sx

    def pz(z):
        return y0 - (z + 1.0) * sz

    b = []
    # axes
    b.append(f'<line x1="{x0}" y1="{pz(1.6)}" x2="{x0}" y2="{y0}" stroke="#999"/>')
    for zt in (-1.0, -0.5, 0.0, 0.5, 1.0, 1.5):
        b.append(f'<line x1="{x0-5}" y1="{pz(zt)}" x2="{x0}" y2="{pz(zt)}" stroke="#999"/>')
        b.append(f'<text class="s" x="{x0-52}" y="{pz(zt)+4}">z={zt:+.1f}</text>')
    # mesh surface z=0 and leave plane z=+0.5
    b.append(f'<line x1="{x0}" y1="{pz(0)}" x2="{px(13)}" y2="{pz(0)}" '
             f'stroke="#8b5a2b" stroke-width="3"/>')
    b.append(f'<text x="{px(13)+6}" y="{pz(0)+4}" fill="#8b5a2b">'
             f'make_test_flat surface z=0</text>')
    b.append(f'<line x1="{x0}" y1="{pz(0.5)}" x2="{px(13)}" y2="{pz(0.5)}" '
             f'stroke="#c00" stroke-dasharray="6 4" stroke-width="2"/>')
    b.append(f'<text x="{px(13)+6}" y="{pz(0.5)+4}" fill="#c00">'
             f'leave floor = surface + 0.5 (drape target)</text>')
    b.append(f'<line x1="{x0}" y1="{pz(1.0)}" x2="{px(13)}" y2="{pz(1.0)}" '
             f'stroke="#06c" stroke-dasharray="3 3"/>')
    b.append(f'<text x="{px(13)+6}" y="{pz(1.0)+4}" fill="#06c">safe_z = 1.0</text>')

    # pre-drape ladder at x=2..5, post-drape at x=8..11
    for label, draped, xbase, col in (("EXPECTED (pre-fa27b08)", False, 2.0, "#080"),
                                      ("ACTUAL (HEAD)", True, 8.0, "#c00")):
        entry, moves = peck_test_moves(draped)
        plunges = [m for m in moves if m[0] == "EntryPlunge feed"]
        b.append(f'<text x="{px(xbase)}" y="{pz(1.55)}" fill="{col}">{label}</text>')
        b.append(f'<text class="s" x="{px(xbase)}" y="{pz(1.55)+14}">'
                 f'entry.z={entry[2]:+.2f}  EntryPlunge count = {len(plunges)}</text>')
        cx = px(xbase + 1.2)
        prev_z = 1.0
        step = 0
        for kind, z in moves:
            dash = ' stroke-dasharray="4 3"' if "apid" in kind else ""
            wid = 3 if kind == "EntryPlunge feed" else 1.5
            c = col if kind == "EntryPlunge feed" else "#888"
            xx = cx + step * 14
            b.append(f'<line x1="{xx}" y1="{pz(prev_z)}" x2="{xx}" y2="{pz(z)}" '
                     f'stroke="{c}" stroke-width="{wid}"{dash}/>')
            b.append(f'<circle cx="{xx}" cy="{pz(z)}" r="2.6" fill="{c}"/>')
            prev_z = z
            step += 1
    b.append(f'<text class="s" x="14" y="{H-30}">assert_eq!(entry_plunges, 2) — HEAD yields 1. '
             f'The peck loop never iterates because the draped entry sits only '
             f'depth_per_pass (0.5) below safe_z.</text>')
    b.append(f'<text class="s" x="14" y="{H-14}">Consequence: the test no longer reaches '
             f'the "dpp == PECK_CLEARANCE_MM" non-progressing-loop guard it is named for '
             f'— it is vacuous as well as red.</text>')
    return svg(W, H, "\n".join(b),
               "W0 / test 1 — peck_plunge_progresses… : XZ cross-section at entry XY (0,0)")


def make_rapid_svg():
    W, H = 940, 410
    x0, y0 = 80, 330
    sx, sz = 22.0, 42.0

    def px(x):
        return x0 + x * sx

    def pz(z):
        return y0 - (z + 4.0) * sz

    b = []
    b.append(f'<line x1="{x0}" y1="{pz(1.2)}" x2="{x0}" y2="{y0}" stroke="#999"/>')
    for zt in (-4, -3, -2, -1, 0, 1):
        b.append(f'<line x1="{x0-5}" y1="{pz(zt)}" x2="{x0}" y2="{pz(zt)}" stroke="#999"/>')
        b.append(f'<text class="s" x="{x0-46}" y="{pz(zt)+4}">z={zt:+d}</text>')
    b.append(f'<line x1="{x0}" y1="{pz(0)}" x2="{px(28)}" y2="{pz(0)}" '
             f'stroke="#8b5a2b" stroke-width="3"/>')
    b.append(f'<text class="s" x="{px(28)+4}" y="{pz(0)+4}" fill="#8b5a2b">surface z=0</text>')
    b.append(f'<line x1="{x0}" y1="{pz(0.5)}" x2="{px(28)}" y2="{pz(0.5)}" '
             f'stroke="#c00" stroke-dasharray="6 4" stroke-width="2"/>')
    b.append(f'<text class="s" x="{px(28)+4}" y="{pz(0.5)+4}" fill="#c00">'
             f'surface + leave = 0.5</text>')

    pre = rapid_test_paths(False)
    post = rapid_test_paths(True)
    for (cut1, entry, cut2), col, lbl, dash in (
        (pre, "#080", "requested by the fixture (pre-fa27b08 emission)", ""),
        (post, "#c00", "emitted at HEAD after drape", ' stroke-dasharray="5 3"'),
    ):
        pts = " ".join(f"{px(p[0])},{pz(p[2])}" for p in cut1)
        b.append(f'<polyline points="{pts}" fill="none" stroke="{col}" '
                 f'stroke-width="3"{dash}/>')
        pts2 = " ".join(f"{px(p[0])},{pz(p[2])}" for p in cut2)
        b.append(f'<polyline points="{pts2}" fill="none" stroke="{col}" '
                 f'stroke-width="3"{dash}/>')
        b.append(f'<circle cx="{px(entry[0])}" cy="{pz(entry[2])}" r="4" fill="{col}"/>')
        for p in (cut1[0], cut1[1], cut2[1]):
            b.append(f'<circle cx="{px(p[0])}" cy="{pz(p[2])}" r="3" fill="{col}"/>')

    b.append(f'<text x="14" y="{H-72}" fill="#080">solid green — fixture intent: '
             f'cut1 ends (5,5,-3); Rapid entry (20,20,-2); cut2 at z=-2</text>')
    b.append(f'<text x="14" y="{H-54}" fill="#c00">dashed red — HEAD: every point '
             f'lifted to z=+0.5 by drape_path_to_leave / drape_point</text>')
    b.append(f'<text class="s" x="14" y="{H-32}">The test searches for a move landing '
             f'exactly at (5,5,-3) and panics "cut1 endpoint not found" — that move no '
             f'longer exists.</text>')
    b.append(f'<text class="s" x="14" y="{H-16}">The fixture asks the planner to cut '
             f'3.5 mm BELOW the finished surface while holding 0.5 mm of stock above it. '
             f'That is self-contradictory under post-fa27b08 semantics.</text>')
    return svg(W, H, "\n".join(b),
               "W0 / test 2 — rapid_segment_lifts_to_safe_z_before_traverse : XZ profile along x=y")


def make_moves_txt():
    lines = []
    lines.append("W0 — derived move tables for the two adaptive3d::path::tests reds")
    lines.append("Derived by mirroring path.rs arithmetic in Python; no crate change.")
    lines.append("")
    for draped, tag in ((False, "PRE-drape (fa27b08^ behaviour)"),
                        (True, "POST-drape (HEAD behaviour)")):
        entry, moves = peck_test_moves(draped)
        pl = [m for m in moves if m[0] == "EntryPlunge feed"]
        lines.append(f"== test 1  {tag}")
        lines.append(f"   draped entry = {entry}")
        for i, (kind, z) in enumerate(moves):
            lines.append(f"   [{i}] {kind:<38s} z={z:+.3f}")
        lines.append(f"   move count = {len(moves)}   EntryPlunge count = {len(pl)}"
                     f"   (assertion wants 2, and <= 6 moves)")
        lines.append("")
    for draped, tag in ((False, "PRE-drape (fa27b08^ behaviour)"),
                        (True, "POST-drape (HEAD behaviour)")):
        cut1, entry, cut2 = rapid_test_paths(draped)
        lines.append(f"== test 2  {tag}")
        lines.append(f"   cut1  = {cut1}")
        lines.append(f"   entry = {entry}")
        lines.append(f"   cut2  = {cut2}")
        found = any(abs(p[0] - 5.0) < 1e-9 and abs(p[1] - 5.0) < 1e-9
                    and abs(p[2] + 3.0) < 1e-9 for p in cut1)
        lines.append(f"   move at (5,5,-3) present? {found}"
                     f"   (test's `.expect(\"cut1 endpoint not found\")`)")
        lines.append("")
    return "\n".join(lines) + "\n"


if __name__ == "__main__":
    with open(os.path.join(OUT, "peck_fixture_xz.svg"), "w") as f:
        f.write(make_peck_svg())
    with open(os.path.join(OUT, "rapid_fixture_xz.svg"), "w") as f:
        f.write(make_rapid_svg())
    with open(os.path.join(OUT, "path_fixture_moves.txt"), "w") as f:
        f.write(make_moves_txt())
    print(make_moves_txt())
