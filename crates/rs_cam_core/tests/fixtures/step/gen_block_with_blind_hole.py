#!/usr/bin/env python3
"""Write `block_with_blind_hole.step` (geometry selection UX-7 fixture).

The part: a block 40 x 30 x 20 mm (X 0..40, Y 0..30, Z 0..20) with one
blind round hole of diameter 6 mm at (15, 15), 8 mm deep (floor at Z 12).

The faces: the top (with the hole rim as its inner bound), the bottom, four
sides, the hole wall (a cylinder with one seam edge) and the hole floor.
Each loop runs counter-clockwise about the outward normal of its face, so
each edge is used once forward and once backward (a closed shell).

Run: python3 -I gen_block_with_blind_hole.py   (writes beside this file)
No CAD kernel is needed; the script writes the AP214 text itself.
"""

import os

LINES = []
COUNT = [0]


def ent(text):
    COUNT[0] += 1
    LINES.append(f"#{COUNT[0]} = {text};")
    return f"#{COUNT[0]}"


def num(v):
    s = repr(float(v))
    return s[:-1] if s.endswith(".0") else s


def pt(x, y, z):
    return ent(f"CARTESIAN_POINT('',({num(x)},{num(y)},{num(z)}))")


def dr(x, y, z):
    return ent(f"DIRECTION('',({num(x)},{num(y)},{num(z)}))")


def axis(o, n, r):
    return ent(f"AXIS2_PLACEMENT_3D('',{pt(*o)},{dr(*n)},{dr(*r)})")


def plane(o, n, r):
    return ent(f"PLANE('',{axis(o, n, r)})")


VERTS = {}


def vertex(p):
    if p not in VERTS:
        VERTS[p] = ent(f"VERTEX_POINT('',{pt(*p)})")
    return VERTS[p]


EDGES = {}


def line_edge(a, b):
    """The edge a-b, made once; returns (edge, orientation for a->b)."""
    if (b, a) in EDGES:
        return EDGES[(b, a)], ".F."
    if (a, b) not in EDGES:
        d = [b[i] - a[i] for i in range(3)]
        length = sum(c * c for c in d) ** 0.5
        u = [c / length for c in d]
        vec = ent(f"VECTOR('',{dr(*u)},{num(length)})")
        line = ent(f"LINE('',{pt(*a)},{vec})")
        EDGES[(a, b)] = ent(f"EDGE_CURVE('',{vertex(a)},{vertex(b)},{line},.T.)")
    return EDGES[(a, b)], ".T."


def oriented(edge, sense):
    return ent(f"ORIENTED_EDGE('',*,*,{edge},{sense})")


def polygon_loop(pts):
    oes = []
    for i, a in enumerate(pts):
        b = pts[(i + 1) % len(pts)]
        e, s = line_edge(a, b)
        oes.append(oriented(e, s))
    return ent(f"EDGE_LOOP('',({','.join(oes)}))")


def face(bounds, surface, sense=".T."):
    return ent(f"ADVANCED_FACE('',({','.join(bounds)}),{surface},{sense})")


def outer(loop):
    return ent(f"FACE_OUTER_BOUND('',{loop},.T.)")


def inner(loop):
    return ent(f"FACE_BOUND('',{loop},.T.)")


X, Y, Z = 40.0, 30.0, 20.0
CX, CY, R, FLOOR = 15.0, 15.0, 3.0, 12.0

faces = []
# Bottom (normal -Z): counter-clockwise seen from below.
faces.append(face([outer(polygon_loop([(0, 0, 0), (0, Y, 0), (X, Y, 0), (X, 0, 0)]))],
                  plane((0, 0, 0), (0, 0, -1), (1, 0, 0))))
# Sides.
faces.append(face([outer(polygon_loop([(0, 0, 0), (X, 0, 0), (X, 0, Z), (0, 0, Z)]))],
                  plane((0, 0, 0), (0, -1, 0), (1, 0, 0))))
faces.append(face([outer(polygon_loop([(X, 0, 0), (X, Y, 0), (X, Y, Z), (X, 0, Z)]))],
                  plane((X, 0, 0), (1, 0, 0), (0, 1, 0))))
faces.append(face([outer(polygon_loop([(X, Y, 0), (0, Y, 0), (0, Y, Z), (X, Y, Z)]))],
                  plane((X, Y, 0), (0, 1, 0), (-1, 0, 0))))
faces.append(face([outer(polygon_loop([(0, Y, 0), (0, 0, 0), (0, 0, Z), (0, Y, Z)]))],
                  plane((0, Y, 0), (-1, 0, 0), (0, -1, 0))))

# The hole: two circles (rim at Z, floor at FLOOR) and one seam line.
top_v = vertex((CX + R, CY, Z))
bot_v = vertex((CX + R, CY, FLOOR))
top_circle = ent(f"EDGE_CURVE('',{top_v},{top_v},"
                 f"{ent(f'CIRCLE({chr(39)}{chr(39)},{axis((CX, CY, Z), (0, 0, 1), (1, 0, 0))},{num(R)})')},.T.)")
bot_circle = ent(f"EDGE_CURVE('',{bot_v},{bot_v},"
                 f"{ent(f'CIRCLE({chr(39)}{chr(39)},{axis((CX, CY, FLOOR), (0, 0, 1), (1, 0, 0))},{num(R)})')},.T.)")
seam_vec = ent(f"VECTOR('',{dr(0, 0, 1)},{num(Z - FLOOR)})")
seam = ent(f"EDGE_CURVE('',{bot_v},{top_v},{ent(f'LINE({chr(39)}{chr(39)},{pt(CX + R, CY, FLOOR)},{seam_vec})')},.T.)")

# Top (normal +Z): the block outline counter-clockwise, the rim clockwise.
top_outer = polygon_loop([(0, 0, Z), (X, 0, Z), (X, Y, Z), (0, Y, Z)])
top_hole = ent(f"EDGE_LOOP('',({oriented(top_circle, '.F.')}))")
faces.append(face([outer(top_outer), inner(top_hole)], plane((0, 0, Z), (0, 0, 1), (1, 0, 0))))
# Hole floor (normal +Z): the floor circle counter-clockwise.
floor_loop = ent(f"EDGE_LOOP('',({oriented(bot_circle, '.T.')}))")
faces.append(face([outer(floor_loop)], plane((CX, CY, FLOOR), (0, 0, 1), (1, 0, 0))))
# Hole wall (normal towards the axis): the cylinder reversed.
wall_loop = ent("EDGE_LOOP('',({}))".format(",".join([
    oriented(seam, ".F."),
    oriented(bot_circle, ".F."),
    oriented(seam, ".T."),
    oriented(top_circle, ".T."),
])))
cylinder = ent(f"CYLINDRICAL_SURFACE('',{axis((CX, CY, FLOOR), (0, 0, 1), (1, 0, 0))},{num(R)})")
faces.append(face([outer(wall_loop)], cylinder, ".F."))

shell = ent(f"CLOSED_SHELL('',({','.join(faces)}))")
solid = ent(f"MANIFOLD_SOLID_BREP('block_with_blind_hole',{shell})")
units_len = ent("( LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT(.MILLI.,.METRE.) )")
units_ang = ent("( NAMED_UNIT(*) PLANE_ANGLE_UNIT() SI_UNIT($,.RADIAN.) )")
units_sol = ent("( NAMED_UNIT(*) SI_UNIT($,.STERADIAN.) SOLID_ANGLE_UNIT() )")
unc = ent(f"UNCERTAINTY_MEASURE_WITH_UNIT(LENGTH_MEASURE(1.E-07),{units_len},'distance_accuracy_value','confusion accuracy')")
ctx = ent("( GEOMETRIC_REPRESENTATION_CONTEXT(3) GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT(("
          f"{unc})) GLOBAL_UNIT_ASSIGNED_CONTEXT(({units_len},{units_ang},{units_sol})) "
          "REPRESENTATION_CONTEXT('Context #1','3D Context with UNIT and UNCERTAINTY') )")
origin = axis((0, 0, 0), (0, 0, 1), (1, 0, 0))
ent(f"ADVANCED_BREP_SHAPE_REPRESENTATION('',({origin},{solid}),{ctx})")

HEADER = """ISO-10303-21;
HEADER;
FILE_DESCRIPTION(('rs_cam UX-7 fixture: block with a blind hole'),'2;1');
FILE_NAME('block_with_blind_hole.step','2026-10-22T00:00:00',(''),(''),'gen_block_with_blind_hole.py','','');
FILE_SCHEMA(('AUTOMOTIVE_DESIGN { 1 0 10303 214 1 1 1 1 }'));
ENDSEC;
DATA;
"""
out = os.path.join(os.path.dirname(os.path.abspath(__file__)), "block_with_blind_hole.step")
with open(out, "w", encoding="ascii") as f:
    f.write(HEADER + "\n".join(LINES) + "\nENDSEC;\nEND-ISO-10303-21;\n")
print(out)
