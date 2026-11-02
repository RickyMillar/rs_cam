"""Derive the rs_cam two-sided job of the full-size RivMap oak block nz-wanaka-21.

Run:  uv run --with ezdxf --with numpy --with shapely python planning/rivmap_block_job/derive_wanaka21.py

The brief is project_rivmap_mono/manufacture/docs/rs_cam_full_block_prompt.md. The
script reads the block-stage output (read only) and writes:

- the oak block model and the DXF outlines in the back-view frame (= the rs_cam
  setup-1 frame) to /home/ricky/cnc_jobs/rivmap_block/nz_wanaka_21/;
- the project planning/rivmap_block_job/nz_wanaka_21.toml.

Method (operator, 2026-10-31: "use 3d ops for most of this ... 5-10 paths total"):
the back is a 3D rough of the mech block back (block_back.stl: the cavity,
the mech pockets and the through holes) with the 6 mm tool, then a 3D rough of the
relief from behind (the operator's earlier method) that leaves block.json web_min of
oak under the front surface, so the V lines need no rough slots. The light holes are
drilled; the V lines and the day-strip slit are cut from the back before the pour; the
epoxy is a stock change; the front is a 3D rough and a scallop finish of the terrain.
The frame, T1, the raw top, the pin depth and the tools come from block.json `cam`.
The stock and the pins are the operator's (the brief, "The user's stock and pins").
The writers are the ones of derive.py (no time stamp: a second run writes the same bytes).
"""

from __future__ import annotations

import json
import math
import tomllib
from pathlib import Path

import ezdxf

import derive as base
import numpy as np

from derive import (Dxf, Project, entity_points, layer_entities, model_table, read_stl, table, tool_table,
                    write_if_changed, write_stl)

MONO = Path("/home/ricky/personal_repos/project_rivmap_mono/manufacture")
BUILD = MONO / "build"
BLOCK = BUILD / "block/nz-wanaka-21"
HERE = Path(__file__).resolve().parent
RS_CAM = HERE.parents[1]
OUT = Path("/home/ricky/cnc_jobs/rivmap_block/nz_wanaka_21")
TEMPLATE = Path("/home/ricky/Downloads/big/rivmap_export_350_500/rivmap350.toml")
MACHINE_FROM = HERE / "testpiece_mini.toml"   # the operator's Scylla grblHAL machine (GUI save)

# ── The operator's stock and pins (the brief, 2026-10-31) ────────────────────
STOCK_X, STOCK_Y = 395.0, 460.0      # the 460 mm length is along the grain = block Y
# The pair of the pin pattern is this far from the flip axis. A free choice: the
# operator moves it to clear the flaws near the long edges.
# Pins (operator 2026-11-02: "a few more, it is a big part"; keep them out of the clamp keep-outs).
# Five pins, mirror-symmetric about the flip axis (y = STOCK_Y / 2) so the flipped holes land on the
# same pins, and keyed: the left side holds three, the right side two. Offsets from the axis are a free
# choice inside the gaps the clamps leave (asserted below).
PIN_LEFT_OFFSETS = (0.0, -40.0, 40.0)
PIN_RIGHT_OFFSETS = (-120.0, 120.0)
# The operator's measured stock (2026-11-02): 26 mm thick (block.json cam.raw_top_max is the 27 mm
# planning bound). The model stock stays T1 thick: setup 3 zeroes on the faced back, so the 0.5 mm skin
# is not modelled (the back face and the clamp-pad pockets cut air in the simulation).
RAW_TOP_MM = 26.0
# The back hollow only as a channel along the V lines (operator idea 2026-11-02). Measured: the 6 mm
# adaptive rest then cuts 284 m (was 232) with 144 m of rapids (was 63); the whole job is about 1.2 h
# slower. Off until a 6 mm trace along the rivers replaces the adaptive in the channels.
HOLLOW_CHANNEL_ONLY = False
# Top clamps (operator 2026-11-02): 2 on each long side at the thirds of the 460 mm length, 20 mm wide,
# 4 mm tall, about 2 mm onto the stock. Keep-out: 40 mm along the edge, 10 mm onto the stock.
CLAMP_Y_STOCK = (STOCK_Y / 3.0, 2.0 * STOCK_Y / 3.0)
CLAMP_SPACE_ALONG = 40.0
CLAMP_SPACE_ONTO = 10.0
CLAMP_HEIGHT = 4.0
CLAMP_CLEARANCE = 1.0
# The operator's GUI edits of 2026-11-01 (front ops; the skim removed: "dont need it").
FRONT_ROUGH_6MM = {"stepover": 1.382, "depth_per_pass": 5.532, "feed_rate": 3900.0, "plunge_rate": 2360.0,
                   "ramp_feed_rate": 4720.0, "spindle_rpm": 20102}
FRONT_SCALLOP_FEED = {"feed_rate": 4700.0, "spindle_rpm": 20611, "plunge_rate": 240.0}  # plunge: Suggest (cap 300)
# The operator's bulk roughing bit (library catalog my_roughing, 2026-11-01).
ROUGH_LIB = Path.home() / ".config/rs_cam/tools/my_roughing.toml"
ROUGH_BIT = "18 mm 2F"
# The operator's tools (2026-11-02). The detail tool replaces the 3 mm on the lakes and the antenna;
# its shank is not stated (3.175 assumed: measure). The V-bit has a 0.3 mm flat tip, which rs_cam's
# tool table cannot hold (it models a sharp tip): the depth and the cone below use the tip.
DETAIL_TOOL = {"name": "DMM DSP1f-D2.0-L08, 2 mm 1F, 8 mm flute (shank 3.175 assumed: measure)",
               "diameter": 2.0, "cutting_length": 8.0, "shank": 3.175, "flutes": 1, "stickout": 20.0}
V20_TIP_MM = 0.3
V20_NAME = "3.175-20deg-0.3 V-bit (0.3 mm flat tip; rs_cam models a sharp tip), 25 mm stick-out (measure)"
# The front finish ball: R1 x 6 x 20 mm, 2F. The taper half angle is from those three numbers
# (atan((6/2 - 1) / 20)), not a printed value: measure.
FINISH_BALL = {"name": "R1 x 6 x 20 mm 2F tapered ball (taper from the size: measure)", "tip_diameter": 2.0,
               "shank": 6.0, "flute_length": 20.0, "flutes": 2}
FLAT_6_CUTTING_LENGTH = 25.0
BALL_BIT = "6 mm ball (R3)"
# The R3 ball hollow pass (operator idea 2026-11-01), measured slower (whole job 7.66 h against 6.89 h:
# the ball runs at Suggest's 1174 mm/min and the 6 mm rest still had 110 m). Off; kept for the next test.
BALL_PASS = False

# ── rs_cam source constants (asserted below) ─────────────────────────────────
PIN_WALL_MM = 2.0                    # compute/alignment_pins/mod.rs
SAFE_Z_CLEARANCE_MM = 5.0            # compute/config.rs
TAB_WIDTH_DEFAULT, TAB_HEIGHT_DEFAULT = 6.0, 2.0   # compute/operation_configs.rs


def assert_rs_cam_constants() -> None:
    src = RS_CAM / "crates/rs_cam_core/src"
    assert f"pub const PIN_WALL_MM: f64 = {PIN_WALL_MM};" in (src / "compute/alignment_pins/mod.rs").read_text()
    assert f"pub const SAFE_Z_CLEARANCE_MM: f64 = {SAFE_Z_CLEARANCE_MM};" in (src / "compute/config.rs").read_text()
    ops = (src / "compute/operation_configs.rs").read_text()
    assert f"tab_width: {TAB_WIDTH_DEFAULT}," in ops and f"tab_height: {TAB_HEIGHT_DEFAULT}," in ops


def toml_value(v) -> str:
    """derive.toml_value plus an inline table (the pocket `floor`)."""
    if isinstance(v, dict):
        return "{ " + ", ".join(f"{k} = {toml_value(x)}" for k, x in v.items()) + " }"
    return _base_toml_value(v)


_base_toml_value = base.toml_value
base.toml_value = toml_value


def floor(depth: float) -> dict:
    return {"from": "top", "depth": round(depth, 6)}


def current_schema(text: str) -> str:
    """Remove the keys that the derive.py writers emit and the loader now retires."""
    import re
    drop = re.compile(r"^(fixtures = \[\]|keep_out_zones = \[\]|air_bridge_policy = .*|shank_length = .*)$")
    lines = [l for l in text.split("\n") if not drop.match(l)]
    text = "\n".join(lines)
    text = re.sub(r'\ntype = "[a-z_0-9]+"\nenabled = ', "\nenabled = ", text)  # the toolpath kind (C2)
    # A model outline boundary is a selection of the model's closed shapes (geometry selection P2.5).
    return re.sub(
        r"\[setups\.toolpaths\.boundary\.source\.model_outline\]\nmodel_id = (\d+)\nholes = false",
        lambda m: ("[setups.toolpaths.boundary.source.selection]\nselection = { rules = [{ source = { model = "
                   + m.group(1) + ' }, filter = { kinds = { only = ["loop", "circle"] } } }] }\nholes = false'),
        text)


def machine_text() -> str:
    """The [job.machine*] tables of the operator's GUI save, verbatim."""
    text = MACHINE_FROM.read_text()
    a = text.index("[job.machine]")
    b = text.index("[job.simulation]")
    return "\n# The machine: copied from testpiece_mini.toml (the operator's Scylla grblHAL $$ import).\n" + text[a:b]


def main() -> None:
    assert_rs_cam_constants()
    blk = json.loads((BLOCK / "block.json").read_text())
    cam = blk["cam"]
    mapt = tomllib.loads((MONO / "block/map.toml").read_text())
    feeds = tomllib.loads((MONO / "block/feeds_oak.toml").read_text())["tools"]
    rows = json.loads((BLOCK / "cam_ops.json").read_text())
    stack = json.loads((BUILD / "mech/nz-wanaka-21/stack.json").read_text())
    tpl = tomllib.loads(TEMPLATE.read_text())
    tpl_ops = {tp["name"]: tp for s in tpl["setups"] for tp in s["toolpaths"]}
    machine = tomllib.loads(MACHINE_FROM.read_text())["job"]["machine"]

    T1 = cam["T1"]
    T = blk["thickness"]
    bx0, by0, bx1, by1 = blk["block"]
    BW, BD = bx1 - bx0, by1 - by0                       # 358 x 443
    assert cam["frame"].startswith("back view") and f"x = {bx1} - X, y = Y + {-by0}" in cam["frame"]
    mx, my = (STOCK_X - BW) / 2.0, (STOCK_Y - BD) / 2.0  # the block is centred: 18.5, 8.5
    cav_floor = T1 - blk["cavity_depth"]                # setup-1 Z of the finished cavity floor
    # Rapids at the clearance plane above the measured raw top: an in-cavity retract was safe in the
    # simulation, but the G-code validator flags any XY rapid below clearance (637 errors, 2026-11-02),
    # and the top clamps stand on the stock.
    v_retract = RAW_TOP_MM + SAFE_Z_CLEARANCE_MM

    files: dict[str, Path] = {}

    def write_dxf(fname: str, doc, layers: list[str], conv=None) -> None:
        d = Dxf()
        n = 0
        for layer in layers:
            for e in layer_entities(doc, layer):
                pts, bul, closed, _ = entity_points(e)
                if conv:
                    pts = [conv(x, y) for x, y in pts]
                d.polyline(layer, pts, closed, bul)
                n += 1
        assert n > 0, layers
        write_if_changed(OUT / fname, d.text().encode())
        files[fname] = OUT / fname

    bc = ezdxf.readfile(BLOCK / "back_channels.dxf")
    write_dxf("block_outline.dxf", bc, ["BLOCK_OUTLINE"])
    write_dxf("cavity_d9.00.dxf", bc, ["CAVITY_SKIM_D9.00"])
    write_dxf("vgroove_v20.dxf", bc, ["VGROOVE_V20_RIVER", "VGROOVE_V20_LAKE"])
    holes_layers = sorted(l for l in {e.dxf.layer for e in bc.modelspace()} if l.startswith("HOLE_"))
    d = Dxf()
    for l in holes_layers:
        for e in layer_entities(bc, l):
            d.circle(l, e.dxf.center.x, e.dxf.center.y, e.dxf.radius)
    write_if_changed(OUT / "light_holes.dxf", d.text().encode())
    files["light_holes.dxf"] = OUT / "light_holes.dxf"
    mech = ezdxf.readfile(BUILD / "mech/nz-wanaka-21/block_back.dxf")
    write_dxf("antenna_d5.00.dxf", mech, ["ANTENNA_D5.00"])
    write_dxf("slit_slot_f6_d20.67.dxf", mech, ["SLIT_SLOT_F6_D20.67"])
    v60_layer = next(l for l in {e.dxf.layer for e in mech.modelspace()} if l.startswith("VGROOVE_V60_D"))
    d = Dxf()
    for e in layer_entities(mech, v60_layer):
        # Trace skips a ring under 3 points: the line gets its mid point.
        a, b = (e.dxf.start.x, e.dxf.start.y), (e.dxf.end.x, e.dxf.end.y)
        d.polyline(v60_layer, [a, ((a[0] + b[0]) / 2, (a[1] + b[1]) / 2), b], False)
    write_if_changed(OUT / "slit_v60.dxf", d.text().encode())
    files["slit_v60.dxf"] = OUT / "slit_v60.dxf"
    # The day labels (cam_ops.json setup 3, rebuilt 2026-11-01): open single strokes. Trace skips a path
    # under 3 points, so a 2-point stroke gets its mid point.
    eng_row = next(r for r in rows if r["setup"] == 3 and r["op"] == "trace")
    d = Dxf()
    for e in layer_entities(mech, eng_row["layer"]):
        pts, bul, closed, _ = entity_points(e)
        assert not closed and not any(bul), eng_row["layer"]
        if len(pts) == 2:
            a, b = pts
            pts = [a, ((a[0] + b[0]) / 2, (a[1] + b[1]) / 2), b]
        d.polyline(eng_row["layer"], pts, False)
    write_if_changed(OUT / "engrave.dxf", d.text().encode())
    files["engrave.dxf"] = OUT / "engrave.dxf"
    # The clamp keep-outs in the setup-1 frame (stock-local - the stock origin offset). The four boxes
    # are symmetric about the stock centre in X and in Y, so the flipped setup 3 uses the same numbers.
    clamp_boxes = []
    for yc in CLAMP_Y_STOCK:
        for x0, x1 in ((-20.0, CLAMP_SPACE_ONTO), (STOCK_X - CLAMP_SPACE_ONTO, STOCK_X + 20.0)):
            clamp_boxes.append((x0 - mx, yc - CLAMP_SPACE_ALONG / 2.0 - my, x1 - mx, yc + CLAMP_SPACE_ALONG / 2.0 - my))
    # The pads the back face leaves under the clamps: a keep-out grows by its clearance, and the 18 mm
    # face leaves its own radius beside it.
    pad_grow = CLAMP_CLEARANCE + 9.0
    d = Dxf()
    for x0, y0, x1, y1 in clamp_boxes:
        d.polyline("CLAMP_PAD", [(x0 - pad_grow, y0 - pad_grow), (x1 + pad_grow, y0 - pad_grow),
                                 (x1 + pad_grow, y1 + pad_grow), (x0 - pad_grow, y1 + pad_grow)], True)
    write_if_changed(OUT / "clamp_pads.dxf", d.text().encode())
    files["clamp_pads.dxf"] = OUT / "clamp_pads.dxf"
    eb = ezdxf.readfile(BUILD / "terrain/nz-wanaka-21/machinable_edge_band.dxf")
    write_dxf("edge_outer.dxf", eb, ["EDGE_OUTER"], conv=lambda x, y: (bx1 - x, y - by0))

    # The mech block back (stack.json world frame: block XY, Z = block Z + levels.z_block): the
    # cavity, the mech pockets and the through holes, 8k triangles (oak_block_full.stl adds the
    # relief and the channels: 1.4M triangles the back rough does not need).
    # Setup-1 frame: x = bx1 - X, y = Y - by0, z = T1 - (Z - z_block). A rotation (det +1).
    zb = stack["levels"]["z_block"]
    tris = read_stl(BUILD / "mech/nz-wanaka-21/block_back.stl")
    assert abs(tris[..., 2].min() - zb) < 1e-3 and abs(tris[..., 2].max() - (zb + T)) < 1e-3
    s1 = np.empty_like(tris)
    s1[..., 0] = bx1 - tris[..., 0]
    s1[..., 1] = tris[..., 1] - by0
    s1[..., 2] = T1 - (tris[..., 2] - zb)
    # The relief: the front-surface triangles of terrain_back.stl only (they face down in the
    # setup-1 frame). One mesh for the hollow (a drop cutter from above meets the sheet, not the
    # flat back), the V projection (from below) and the front ops (setup 3 turns it face up).
    tb = read_stl(BLOCK / "terrain_back.stl")
    nz = np.cross(tb[:, 1] - tb[:, 0], tb[:, 2] - tb[:, 0])[:, 2]
    sheet = tb[nz < 0.0]
    write_stl(OUT / "relief_sheet_s1.stl", sheet, "rivmap nz-wanaka-21 front relief sheet, setup-1 frame")
    files["relief_sheet_s1.stl"] = OUT / "relief_sheet_s1.stl"

    # ── The back target: ONE surface for the one back rough (operator: "back hollow, antenna and
    # back 3d rough could all just be one path"). A drop cutter meets the highest triangle, so:
    # - the mech block back without its triangles inside the cavity (the cavity walls stay);
    # - inside the cavity: the relief sheet moved web_min toward the back (the hollow), and
    #   inside the closed lake outlines the sheet moved exposure_margin past the front (the
    #   lakes open at the front; operator: "mill out the lakes from the back too").
    from shapely import contains_xy
    from shapely.geometry import Polygon as ShPolygon
    from shapely.ops import unary_union
    web = blk["web_min"]
    margin = float(tomllib.loads((MONO / "design/rivmap.toml").read_text())["paths"]["exposure_margin"])
    # The V at the front: line_width + 2 margin tan(a/2) (the exposure margin, 2026-10-31). With the
    # 0.3 mm flat tip the tip goes (w - tip) / (2 tan(a/2)) past the surface for that width.
    t_half = math.tan(math.radians(blk["vgroove_angle"] / 2.0))
    v_line = blk["line_width"] + 2.0 * margin * t_half
    v_offset = (v_line - V20_TIP_MM) / (2.0 * t_half)
    cavity_poly = unary_union([ShPolygon(entity_points(e)[0]) for e in layer_entities(bc, "CAVITY_SKIM_D9.00")])
    lake_rings = [entity_points(e) for e in layer_entities(bc, "VGROOVE_V20_LAKE")]
    lakes_poly = unary_union([ShPolygon(pts).buffer(0) for pts, _, closed, _ in lake_rings if closed])
    n_open_lakes = sum(1 for _, _, closed, _ in lake_rings if not closed)
    inner = cavity_poly.buffer(-1e-3)
    c_bb = s1.mean(axis=1)
    keep_bb = s1[~contains_xy(inner, c_bb[:, 0], c_bb[:, 1])]
    c_sh = sheet.mean(axis=1)
    in_cav = contains_xy(inner, c_sh[:, 0], c_sh[:, 1])
    in_lake = contains_xy(lakes_poly, c_sh[:, 0], c_sh[:, 1])
    # The hollow only where a V needs it (operator 2026-11-02: from the back the front valleys are high
    # and the mountains deep; a hollow under the mountains does no work). A channel along every V20 line
    # and around every closed lake, half-width = the 6 mm tool radius + the V half-width at the hollow
    # ((web + v_offset) tan(a/2)), so the 6 mm reaches the hollow over the whole V. Elsewhere the target
    # is the flat cavity floor.
    from shapely.geometry import LineString as ShLine
    band_half = 3.0 + (web + v_offset) * t_half
    v_lines = [entity_points(e)[0] for lyr in ("VGROOVE_V20_RIVER", "VGROOVE_V20_LAKE") for e in layer_entities(bc, lyr)]
    band_poly = unary_union([ShLine(pts).buffer(band_half) for pts in v_lines if len(pts) > 1]
                            + [lakes_poly.buffer(band_half)])
    in_band = contains_xy(band_poly, c_sh[:, 0], c_sh[:, 1])
    hollow = sheet[in_cav & ~in_lake].copy()
    hollow[..., 2] += web
    if HOLLOW_CHANNEL_ONLY:
        out_band = ~in_band[in_cav & ~in_lake]
        hollow[out_band, :, 2] = cav_floor
        hollow[..., 2] = np.minimum(hollow[..., 2], cav_floor)
    lakes = sheet[in_cav & in_lake].copy()
    lakes[..., 2] -= margin
    target = np.concatenate([keep_bb, hollow, lakes])
    hollow_floor = float(min(hollow[..., 2].min(), lakes[..., 2].min()))  # the deepest hollow or lake point
    # The hollow and the lakes alone: the surface the R3 ball follows over the cavity.
    write_stl(OUT / "hollow_s1.stl", np.concatenate([hollow, lakes]), "rivmap nz-wanaka-21 hollow + lakes, setup-1 frame")
    files["hollow_s1.stl"] = OUT / "hollow_s1.stl"
    write_stl(OUT / "back_target_s1.stl", target, "rivmap nz-wanaka-21 back rough target, setup-1 frame")
    files["back_target_s1.stl"] = OUT / "back_target_s1.stl"
    # The lakes alone, for the 3 mm lake pass (the 6 mm reaches a lake only where its disc fits).
    write_stl(OUT / "lakes_s1.stl", lakes, "rivmap nz-wanaka-21 lakes past the front, setup-1 frame")
    files["lakes_s1.stl"] = OUT / "lakes_s1.stl"
    write_dxf("lakes_closed.dxf", bc, ["VGROOVE_V20_LAKE"])

    # ── The lake pockets (operator 2026-11-02): a lake is water, so its surface is flat (the range
    # inside one outline is at most 0.57 mm). A 2.5D pocket with vertical walls gives the outline at
    # the front at any depth, and the deeper pour keeps dust and bubbles off the visible edge
    # (operator: "letting the pour go deeper could be better"). Floor of a lake = its lowest sheet
    # point - 2 x exposure_margin (the target is one margin past the front; the pour gets one more).
    # A group of lakes shares the floor of its deepest lake while each wall stays inside the 2 mm
    # flute less map.toml [vgroove].body_margin (the same Z margin the V-bits keep).
    # Lakes narrower than the tool stay V-only. The wall top uses the sheet triangles near the
    # outline (1 mm): the shore. A small lake with no triangle centre inside takes its floor from them.
    lake_tool_r = DETAIL_TOOL["diameter"] / 2.0
    wall_cap = DETAIL_TOOL["cutting_length"] - mapt["vgroove"]["body_margin"]
    lake_list = []
    for pts, _, closed, _ in lake_rings:
        if not closed:
            continue
        poly = ShPolygon(pts).buffer(0)
        near = contains_xy(poly.buffer(1.0), c_sh[:, 0], c_sh[:, 1])
        inside = contains_xy(poly, c_sh[:, 0], c_sh[:, 1])
        assert near.any(), poly.bounds
        if poly.buffer(-lake_tool_r).is_empty:
            continue
        # The floor from the triangles inside the outline: the band can hold the river that leaves
        # the lake, which runs lower than the lake.
        z_lo = float(sheet[inside if inside.any() else near][..., 2].min())
        lake_list.append({"pts": pts, "floor": z_lo - 2.0 * margin, "wall_top": float(sheet[near][..., 2].max()) + web})
    lake_list.sort(key=lambda l: -l["floor"])
    lake_groups: list[list[dict]] = []
    for lk in lake_list:
        for g in lake_groups:
            f = min(lk["floor"], min(m["floor"] for m in g))
            if all(m["wall_top"] - f <= wall_cap for m in g + [lk]):
                g.append(lk)
                break
        else:
            assert lk["wall_top"] - lk["floor"] <= wall_cap, (lk["floor"], lk["wall_top"], wall_cap, len(lk["pts"]))
            lake_groups.append([lk])
    lake_pockets = []
    for gi, g in enumerate(lake_groups):
        d = Dxf()
        for lk in g:
            d.polyline("LAKE_POCKET", lk["pts"], True)
        fname = f"lake_pocket_{gi}.dxf"
        write_if_changed(OUT / fname, d.text().encode())
        files[fname] = OUT / fname
        lake_pockets.append({"file": fname, "n": len(g), "floor": min(m["floor"] for m in g),
                             "wall_top": max(m["wall_top"] for m in g)})
    n_lakes_closed = sum(1 for _, _, c, _ in lake_rings if c)

    # ── tools
    mt = {t["id"]: t for t in mapt["tools"]}
    ct = cam["tools"]
    contract = tomllib.loads((MONO / "design/rivmap.toml").read_text())["cutters"]
    cam_tools = ["flat_6", "v20", "v60", "flat_6.35", "tbn_r1"]
    order_tools = cam_tools + ["dsp_2", "rough_18", "ball_r3"]
    dia = {k: float(contract[mt[k]["cutter"]]) if "cutter" in mt[k] else ct[k]["diameter"] for k in cam_tools}
    assert all(abs(dia[k] - ct[k]["diameter"]) < 1e-9 for k in cam_tools), dia
    tid = {k: i for i, k in enumerate(order_tools)}
    # The operator's drill cycle (testpiece.toml, GUI edit 2026-10-09).
    drill_op = next(tp["operation"]["params"] for s in tomllib.loads((HERE / "testpiece.toml").read_text())["setups"]
                    for tp in s["toolpaths"] if tp["operation"]["kind"] == "drill")
    big = next(t for t in tomllib.loads(ROUGH_LIB.read_text())["tools"] if t["name"].startswith(ROUGH_BIT))
    dia["rough_18"] = big["diameter"]
    dia["dsp_2"] = DETAIL_TOOL["diameter"]
    ball = next(t for t in tomllib.loads(ROUGH_LIB.read_text())["tools"] if t["name"].startswith(BALL_BIT))
    dia["ball_r3"] = ball["diameter"]

    P = Project("nz_wanaka_21", "\n".join([
        "# rs_cam two-sided job: RivMap oak block nz-wanaka-21 (21 x 21 LEDs, block 358 x 443 x 25).",
        "# GENERATED by planning/rivmap_block_job/derive_wanaka21.py from the block stage of 2026-10-31.",
        f"# World frame = setup-1 frame = the back-view DXF frame: x = {bx1} - X_block, y = Y_block + {-by0},",
        f"#   z = T1 - Z_block, T1 = {T1}. Z datum MachineTable on every setup: Z0 = the spoilboard.",
        f"# Stock {STOCK_X} x {STOCK_Y} (the operator's), the block centred: {mx} from each long edge,",
        f"#   {my} from each short edge. XY zero: the stock corner, which the flip keeps in place.",
    ]))
    P.add(table("job", {"name": "rivmap nz-wanaka-21 block (two-sided)"}))
    P.add(table("job.stock", {
        "x": STOCK_X, "y": STOCK_Y, "z": T1, "origin_x": -mx, "origin_y": -my, "origin_z": 0.0,
        "padding": 0.0, "auto_from_model": False},
        comment=(f"The operator's stock {STOCK_X} x {STOCK_Y}, 26-27 thick (measure); the model is T1 = {T1} thick\n"
                 f"(the back face pass starts at raw_top_max {cam['raw_top_max']}). The block is centred.")))
    P.add(table("job.stock.material.SolidWood", {"species": "WhiteOak"}))

    # Pins: in the long-edge margins (layout above). A pin keeps
    # PIN_WALL to the stock edge and to the outline cut (the outer flat_6 width of the margin).
    pin_d = cam["pins"][0]["d"]
    band_lo = PIN_WALL_MM + pin_d / 2.0
    band_hi = mx - dia["flat_6"] - PIN_WALL_MM - pin_d / 2.0
    assert band_hi >= band_lo, (band_lo, band_hi)
    edge_off = (band_lo + band_hi) / 2.0
    axis_y = STOCK_Y / 2.0
    pins = ([(edge_off, axis_y + d) for d in PIN_LEFT_OFFSETS]
            + [(STOCK_X - edge_off, axis_y + d) for d in PIN_RIGHT_OFFSETS])
    for _, py in pins:
        for yc in CLAMP_Y_STOCK:
            gap = abs(py - yc) - CLAMP_SPACE_ALONG / 2.0 - CLAMP_CLEARANCE - pin_d / 2.0 - PIN_WALL_MM
            assert gap >= 0.0, (py, yc, gap)
    for x, y in pins:
        P.add(table("[job.stock.alignment_pins]", {"x": x, "y": y, "diameter": pin_d}).replace(
            "[[[job.stock.alignment_pins]]]", "[[job.stock.alignment_pins]]"))
    P.add(table("job.post", {"format": "grblhal", "spindle_speed": int(feeds["flat_6"]["rpm"]),
                             "safe_z": T1 + SAFE_Z_CLEARANCE_MM, "high_feedrate_mode": False,
                             "high_feedrate": 5000.0, "spindle_strategy": "match_chart",
                             "controller_waits_for_spindle": True},
                comment="safe_z = T1 + SAFE_Z_CLEARANCE_MM: a MachineTable datum does not shift the post Z."))
    P.add(machine_text())
    P.add(table("job.simulation", {"resolution_mm": mapt["paths"]["grid"]}, comment="map.toml [paths].grid"))

    for k in cam_tools:
        t, c = mt[k], ct[k]
        if c["kind"] == "flat":
            cut = FLAT_6_CUTTING_LENGTH if k == "flat_6" else c["reach"]
            P.add(tool_table(tid[k], tid[k] + 1, t["name"], "end_mill", dia[k], cut, t["shank"],
                             c["reach"], t["flutes"], comment=f"map.toml {k}; block.json cam.tools; cutting length {cut}"))
        elif c["kind"] == "vbit":
            tip = V20_TIP_MM if k == "v20" else 0.0
            cone = ((dia[k] - tip) / 2.0) / math.tan(math.radians(c["angle"] / 2.0))
            P.add(tool_table(tid[k], tid[k] + 1, V20_NAME if k == "v20" else t["name"], "v_bit", dia[k], cone,
                             t["shank"], c["reach"], t["flutes"], included_angle=c["angle"],
                             taper_half_angle=c["angle"] / 2.0,
                             comment=f"map.toml {k}; size and angle from block.json cam.tools; cone {cone:.4f} (tip {tip})"))
        else:
            fb = FINISH_BALL
            taper = math.degrees(math.atan((fb["shank"] / 2.0 - fb["tip_diameter"] / 2.0) / fb["flute_length"]))
            P.add(tool_table(tid[k], tid[k] + 1, fb["name"], "tapered_ball_nose", fb["tip_diameter"],
                             fb["flute_length"], fb["shank"], c["reach"], fb["flutes"], taper_half_angle=round(taper, 4),
                             comment=f"The operator's finish ball (2026-11-02); taper {taper:.3f} deg from its size"))
    dt = DETAIL_TOOL
    P.add(tool_table(tid["dsp_2"], tid["dsp_2"] + 1, dt["name"], "end_mill", dt["diameter"], dt["cutting_length"],
                     dt["shank"], dt["stickout"], dt["flutes"], comment="The operator's detail tool (2026-11-02)."))
    P.add(tool_table(tid["rough_18"], big["tool_number"], big["name"], "end_mill", big["diameter"],
                     big["cutting_length"], big["shank_diameter"], big["stickout"], big["flute_count"],
                     comment=f"The operator's bulk roughing bit ({ROUGH_LIB.name}). Stick-out: measure."))
    P.add(tool_table(tid["ball_r3"], ball["tool_number"], ball["name"], "ball_nose", ball["diameter"],
                     ball["cutting_length"], ball["shank_diameter"], ball["stickout"], ball["flute_count"],
                     comment=f"The operator's R3 ball ({ROUGH_LIB.name}). Flute and stick-out: measure."))

    model_order = ["back_target_s1.stl", "hollow_s1.stl", "lakes_s1.stl", "lakes_closed.dxf", "relief_sheet_s1.stl", "block_outline.dxf",
                   "edge_outer.dxf", "cavity_d9.00.dxf", "vgroove_v20.dxf", "light_holes.dxf",
                   "antenna_d5.00.dxf", "slit_slot_f6_d20.67.dxf", "slit_v60.dxf", "engrave.dxf", "clamp_pads.dxf"] + [lp["file"] for lp in lake_pockets]
    assert sorted(model_order) == sorted(files)
    mids = {f: i for i, f in enumerate(model_order)}
    for f in model_order:
        P.add(model_table(mids[f], files[f]))

    tol = mapt["vgroove"]["tip_tol"]
    ramp = {"entry_style": "ramp", "segment_merge": True, "segment_merge_tolerance": tol}
    f6, fv, f635, ftb = (feeds[k] for k in ("flat_6", "v20", "flat_6.35", "tbn_r1"))
    rig = machine["rigidity"]
    trough = tpl_ops["3D Rough 8"]["operation"]["params"]

    def face(name: str, depth: float, why: str, heights: dict | None = None) -> None:
        # The bulk bit faces (operator 2026-11-01: fewer tool changes, a wide stepover):
        # stepover = the machine woc_roughing_factor x D, one pass; feeds: run Suggest.
        P.toolpath(name, "face", tid["rough_18"], mids["block_outline.dxf"], {
            "stepover": round(rig["woc_roughing_factor"] * dia["rough_18"], 4), "depth": depth,
            "depth_per_pass": depth, "stock_offset": dia["rough_18"], "direction": "zigzag"},
            why=why + "\nThe 18 mm bulk bit; stock_offset = the tool diameter, so the last row reaches the far edge.",
            heights=heights, dressups=ramp)

    def clamps(top_z: float) -> None:
        for i, (x0, y0, x1, y1) in enumerate(clamp_boxes):
            side = "X min" if x0 < 0 else "X max"
            P.add(table("[setups.obstacles]", {"id": i, "name": f"Clamp {side} {i // 2 + 1}", "enabled": True,
                                               "clearance": CLAMP_CLEARANCE}).replace(
                      "[[[setups.obstacles]]]", "[[setups.obstacles]]")
                  + table("setups.obstacles.geometry", {"kind": "box", "min": [round(x0, 4), round(y0, 4), 0.0],
                                                        "max": [round(x1, 4), round(y1, 4), round(top_z, 4)]}))

    def rough3d(name: str, tool: str, model: str, f: dict, why: str, boundary: dict, leave: float,
                stock_source: str = "fresh", geometry: dict | None = None,
                heights: dict | None = None, strategy: str = "geodesic_adaptive",
                detect_flat: bool = True, min_region: float | None = None) -> None:
        # Adaptive geometry from the machine rigidity (adaptive_doc_factor x D, adaptive_woc_factor x D)
        # unless the caller gives it. Run Suggest to tune it.
        g = geometry or {"stepover": round(rig["adaptive_woc_factor"] * dia[tool], 4),
                         "depth_per_pass": round(rig["adaptive_doc_factor"] * dia[tool], 4)}
        P.toolpath(name, "adaptive3d", tid[tool], mids[model], {
            **g, "stock_to_leave_axial": leave,
            **({"feed_rate": f["feed_safe"], "plunge_rate": f["plunge"], "ramp_feed_rate": round(f["feed_safe"] / 3.0, 3),
                "spindle_rpm": int(f["rpm"])} if f else {}),
            "tolerance": trough["tolerance"],
            "min_cutting_radius": 0.0, "entry_style": "ramp", "ramp_angle_deg": 3.0,
            "helix_radius_factor": trough["helix_radius_factor"], "helix_pitch": trough["helix_pitch"],
            "entry_clearance_mm": trough["entry_clearance_mm"], "detect_flat_areas": detect_flat,
            "region_ordering": trough["region_ordering"], "clearing_strategy": strategy,
            "trochoid_cap_mult": trough["trochoid_cap_mult"], "engagement_measure": trough["engagement_measure"],
            "z_blend": False,
            "min_region_cut_length_mm": trough["min_region_cut_length_mm"] if min_region is None else min_region,
            "stay_down_clearance_mm": trough["stay_down_clearance_mm"]},
            # Merge tolerance = tip_tol: a merged chord may cut below the target by its tolerance, and the
            # default 0.3 would thin the web_min of the hollow.
            why=why, boundary=boundary, dressups={"segment_merge": True, "segment_merge_tolerance": tol},
            stock_source=stock_source,
            heights=heights)

    in_block = {"enabled": True, "containment": "inside", "offset": 0.0, "model_outline": mids["block_outline.dxf"]}
    in_cavity = {"enabled": True, "containment": "inside", "offset": 0.0, "model_outline": mids["cavity_d9.00.dxf"]}
    in_relief = {"enabled": True, "containment": "inside", "offset": 0.0, "model_outline": mids["edge_outer.dxf"]}

    # ── Setup 1: Back
    P.setup(0, "1 Back", "top", f"Back up. Z0 = spoilboard. XY0 = stock corner (pin 1 is at {pins[0]}). "
            f"Top clamps at y {CLAMP_Y_STOCK[0]:.0f} and {CLAMP_Y_STOCK[1]:.0f} on both long sides.")
    clamps(RAW_TOP_MM + CLAMP_HEIGHT)
    face("Face back to T1", RAW_TOP_MM - T1,
         f"Face from the measured raw top {RAW_TOP_MM} to T1 {T1} (keeps out of the clamps).", heights={"top_z": RAW_TOP_MM})
    # The V tip goes the overcut for the line width plus exposure_margin past the front (operator:
    # "punch the rivers through more"). The V then cuts web_min + that of its usable cone; the
    # rest caps the depth per pass of the back rough (a terrace cannot take the V past its cone).
    # The line at the front: line_width + 2 margin tan(a/2) (the exposure margin, 2026-10-31). With the
    # 0.3 mm flat tip the tip goes (w - tip) / (2 tan(a/2)) past the surface for that width, and the
    # cone reaches the body (D - tip) / (2 tan(a/2)) above the tip.
    cone = ((dia["v20"] - V20_TIP_MM) / 2.0) / math.tan(math.radians(ct["v20"]["angle"] / 2.0))
    usable = cone - mapt["vgroove"]["body_margin"]
    step = math.floor((usable - web - v_offset) * 10.0) / 10.0
    assert step > 0.0, step
    # Levels: the cavity floor (the hollow channels are narrower than the bit), then the board-pocket
    # floor (block_back.dxf BOARD_POCKET_D<depth>), so the 6 mm rest does not meet a full-height pocket.
    board_floor = T1 - float(next(l for l in {e.dxf.layer for e in mech.modelspace()}
                                  if l.startswith("BOARD_POCKET_D")).split("_D")[1])
    assert T1 - cav_floor <= big["cutting_length"] and cav_floor - board_floor <= big["cutting_length"]
    span = T1 - hollow_floor
    n_big = math.ceil(span / big["cutting_length"])
    bulk_geo = ({"depth_per_pass": round(T1 - cav_floor, 4)}, round(board_floor, 4)) if HOLLOW_CHANNEL_ONLY else \
        ({"depth_per_pass": round(span / n_big, 4)}, round(hollow_floor, 4))
    rough3d(f"Back bulk rough {big['diameter']:g} mm", "rough_18", "back_target_s1.stl", None,
            (f"The bulk of the back target with the {big['diameter']:g} mm bit. Levels: with the channel hollow, the\n"
             f"cavity floor {cav_floor} then the board-pocket floor {board_floor}; else the top to the deepest hollow or\n"
             f"lake point {hollow_floor:.2f} in {n_big} even passes (measured best). No flat-area levels.\n"
             "Feeds: run Suggest (no feeds row for this bit)."),
            in_block, 0.0, geometry={"stepover": round(rig["adaptive_woc_factor"] * big["diameter"], 4), **bulk_geo[0]},
            heights={"bottom_z": bulk_geo[1]}, detect_flat=False)
    if BALL_PASS:
      P.toolpath("Back hollow R3 ball scallop", "scallop", tid["ball_r3"], mids["hollow_s1.stl"], {
          "scallop_height": step, "tolerance": trough["tolerance"], "direction": "outside_in", "continuous": False,
          "slope_from": 0.0, "slope_to": 90.0, "stock_to_leave": 0.0, "intra_pass_hookup_mm": 3.0, "iso_field": True},
          why=(f"One surface pass over the hollow and the lakes (operator idea 2026-11-01: 'a rough scallop with a r3 bit').\n"
               f"It takes the bulk bit's terraces (<= {span / n_big:.2f}) off down to the target. Scallop height = {step}: the V\n"
               "budget left after web_min and the overcut, so the cusps never take the V past its cone.\n"
               "Feeds: run Suggest."),
          boundary=in_cavity, stock_source="from_remaining_stock")
    rough3d("Back rest 6 mm (cavity, hollow, lakes, pockets)", "flat_6", "back_target_s1.stl", f6,
            (f"The rest after the bulk bit: corners, terraces, narrow parts, the pockets and the through holes.\n"
             f"One surface: the mech block back, the hollow under the relief inside the cavity (web_min {web}\n"
             f"of oak left), and the {len(getattr(lakes_poly, 'geoms', [lakes_poly]))} closed lakes opened {margin} past the front ({n_open_lakes} open lake\n"
             f"paths are V-cut only). The V cuts at most {web + v_offset:.2f} of its usable cone {usable:.2f}; the rest,\n"
             f"{step}, is the depth per pass. Remaining stock: generate_all simulates the bulk pass first."),
            in_block, 0.0, geometry=None if BALL_PASS else {"stepover": round(rig["adaptive_woc_factor"] * dia["flat_6"], 4),
                                                            "depth_per_pass": step},
            stock_source="from_remaining_stock")
    P.toolpath("Pin drill, flip pins", "alignment_pin_drill", tid["flat_6"], mids["block_outline.dxf"], {
        "diameter": pin_d, "spoilboard_penetration": round(cam["pin_depth"] - T1, 6), "cycle": "peck",
        "peck_depth": dia["flat_6"], "feed_rate": f6["plunge"], "retract_z": 2.0, "spindle_rpm": int(f6["rpm"])},
        why=f"{len(pins)} pins (3 left, 2 right, symmetric about the flip axis). Depth = block.json cam.pin_depth {cam['pin_depth']} = the flat_6 reach.")
    hole_top = max(float(l.split("_Z")[1]) for l in holes_layers)
    P.toolpath("Drill light holes 6 mm end mill", "drill", tid["flat_6"], mids["light_holes.dxf"], {
        "floor": {"from": "z", "z": round(T1 - hole_top, 4)}, "cycle": drill_op["cycle"],
        "peck_depth": drill_op["peck_depth"], "dwell_time": drill_op["dwell_time"],
        "retract_amount": drill_op["retract_amount"], "retract_z": drill_op["retract_z"],
        "size_tolerance": round(max(abs(dia["flat_6"] - h) for h in blk["holes"]["diameters"]) + 0.01, 3)},
        why=(f"All {blk['holes']['count']} holes ({', '.join(holes_layers)}) with the 6 mm end mill (operator 2026-11-01: the\n"
             f"tool is already in): a flat bottom at block z {hole_top} (the deepest layer; the front cut opens the same\n"
             f"disc). Ø{dia['flat_6']:g} for the drawn Ø{blk['holes']['diameters']}: size_tolerance takes the difference.\n"
             "Cycle: the operator's drill op (testpiece.toml); feeds: run Suggest (G6 flat end mill drill rule)."),
        heights={"top_z": cav_floor})
    n_closed_lakes = n_lakes_closed
    n_cut = sum(lp["n"] for lp in lake_pockets)
    for gi, lp in enumerate(lake_pockets):
        # Top: the highest hollow at the shore + one rest step (the 6 mm rest leaves terraces of at
        # most its depth per pass above the hollow); above it is air.
        top = math.ceil((lp["wall_top"] + step) * 100.0) / 100.0
        P.toolpath(f"Lake pocket 2 mm, {lp['n']} lake{'s' if lp['n'] > 1 else ''} to Z {lp['floor']:.2f}", "pocket",
                   tid["dsp_2"], mids[lp["file"]], {
            "stepover": round(rig["woc_roughing_factor"] * DETAIL_TOOL["diameter"], 4),
            "floor": {"from": "z", "z": round(lp["floor"], 4)},
            "depth_per_pass": round(rig["doc_roughing_factor"] * DETAIL_TOOL["diameter"], 4),
            "climb": True, "pattern": "contour", "angle": 0.0, "finishing_passes": 1,
            "region_ordering": "global"},
            why=(f"{lp['n']} of the {n_closed_lakes} closed lakes ({n_cut} cut; {n_closed_lakes - n_cut} are narrower than the 2 mm tool: V only).\n"
                 f"Flat floor = the deepest lake of the group - 2 x exposure_margin ({2 * margin}); the walls are vertical, so\n"
                 f"the front shows the outline at any depth. Wall {top - lp['floor']:.2f} from top Z {top} (the shore hollow\n"
                 f"{lp['wall_top']:.2f} + one rest step {step}); flute less body_margin = {wall_cap}. One finishing contour\n"
                 "for a clean wall. Retract: the cavity floor + clearance (the hollow between lakes stands above\n"
                 "the pocket top). Plunge entry: the ramp entry dressup on this pocket left wood at two ring\n"
                 "starts that the next level's rapid descent hit (21 rapid collisions, 2026-11-02). Feeds: run Suggest."),
            heights={"top_z": top, "retract_z": v_retract},
            dressups={"segment_merge": True, "segment_merge_tolerance": tol})
    P.toolpath("ANTENNA_D5.00 pocket 2 mm", "pocket", tid["dsp_2"], mids["antenna_d5.00.dxf"], {
        "stepover": round(rig["woc_roughing_factor"] * DETAIL_TOOL["diameter"], 4), "floor": floor(5.0),
        "depth_per_pass": round(rig["doc_roughing_factor"] * DETAIL_TOOL["diameter"], 4),
        "climb": True, "pattern": "contour", "angle": 0.0, "finishing_passes": 0},
        why=("cam_ops.json ANTENNA_D5.00: 4 mm wide; the 2 mm detail tool (no tool change after the lakes).\n"
             "Stepover and depth: the machine woc/doc roughing factors x D. Feeds: run Suggest."),
        dressups=ramp)
    P.toolpath("V rivers and lakes", "project_curve", tid["v20"], mids["vgroove_v20.dxf"], {
        "projection": "point", "depth": round(-v_offset, 9),
        "point_spacing": mapt["vgroove"]["max_seg"], "feed_rate": fv["feed_safe"], "plunge_rate": fv["plunge"],
        "surface_model_id": mids["relief_sheet_s1.stl"], "direction": "from_below", "side": "center",
        "spindle_rpm": int(fv["rpm"]), "chain_distance_mm": 0.0},
        why=(f"Project Curve on the front surface from below, point projection: tip = surface - (w/2)/tan(a/2)\n"
             f"- exposure_margin {margin} = {-v_offset:.4f}; the line at the front is about {2 * v_offset * math.tan(math.radians(blk['vgroove_angle'] / 2.0)):.2f} wide.\n"
             "One pass: the hollow leaves web_min of oak, so the V cuts the web and the overcut. Retract in the cavity.\n"
             "point_spacing = map.toml [vgroove].max_seg: the vertex spacing that the block stage checks the tip at."),
        heights={"top_z": cav_floor, "retract_z": v_retract})
    fv60 = feeds["v60"]
    slit_z0 = float(next(l for l in {e.dxf.layer for e in mech.modelspace()} if l.startswith("SLIT_SLOT")).split("_D")[1])
    slit_z1 = float(v60_layer.split("_D")[1])
    P.toolpath("Day strip slit V60", "trace", tid["v60"], mids["slit_v60.dxf"], {
        "depth": round(slit_z1 - slit_z0, 6), "depth_per_pass": round((slit_z1 - slit_z0) / 2.0, 6),
        "feed_rate": fv60["feed_safe"], "plunge_rate": fv60["plunge"], "compensation": "none",
        "spindle_rpm": int(fv60["rpm"])},
        why=(f"{v60_layer}: the V tip from the slit slot floor (block z {slit_z0}) to {slit_z1}\n"
             f"({slit_z1 - T1:.3f} into the spoilboard; the slit is open underneath: seal it before the pour). Two passes."),
        heights={"top_z": T1 - slit_z0})

    # ── Setup 1b: the clamp pads (pins in, top clamps off; the side pushers hold)
    P.setup(3, "1b Clamp pads", "top",
            "Pins in, top clamps OFF (the side pushers hold). Face the pads the back face left under the clamps, "
            "so the flipped back sits flat. Same zero.")
    pad_depth = RAW_TOP_MM - T1
    P.toolpath("Face clamp pads 6 mm", "pocket", tid["flat_6"], mids["clamp_pads.dxf"], {
        "stepover": round(rig["woc_roughing_factor"] * dia["flat_6"], 4), "floor": {"from": "z", "z": T1},
        "depth_per_pass": round(pad_depth, 4), "climb": True, "pattern": "contour", "angle": 0.0,
        "finishing_passes": 0, "region_ordering": "by_area"},
        why=(f"The four pads under the top clamps: the keep-out + its clearance + the 18 mm face radius, from the\n"
             f"measured raw top {RAW_TOP_MM} to T1 {T1} in one pass. Feeds: run Suggest."),
        heights={"top_z": RAW_TOP_MM, "retract_z": v_retract}, dressups={"entry_style": "none"})

    # ── Setup 2: the pour, then the skim
    P.setup(1, "2 Epoxy pour", "top", "The pour and the cure (no cut: the operator removed the skim, 2026-11-01).")
    overfill = mapt["paths"]["overfill"]
    level = cav_floor + overfill
    P.add("\n# The pour: fill the cavity to its floor + map.toml [paths].overfill.\n"
          + table("[setups.stock_changes]", {"id": 0, "name": "epoxy", "enabled": True, "op": "add",
                                              "cut_as": "stock_material"}).replace(
              "[[[setups.stock_changes]]]", "[[setups.stock_changes]]")
          + table("setups.stock_changes.geometry", {"kind": "outline_fill",
                                                     "model_ids": [mids["cavity_d9.00.dxf"]], "level_z": level})
          + table("setups.stock_changes.material.Custom", {"name": "Epoxy", "feed_scale_factor": 1.0})
          + "\n# The same pour fills the day-strip slit up to the slit slot floor.\n"
          + table("[setups.stock_changes]", {"id": 1, "name": "epoxy slit", "enabled": True, "op": "add",
                                              "cut_as": "stock_material"}).replace(
              "[[[setups.stock_changes]]]", "[[setups.stock_changes]]")
          + table("setups.stock_changes.geometry", {"kind": "outline_fill",
                                                     "model_ids": [mids["slit_slot_f6_d20.67.dxf"]],
                                                     "level_z": T1 - slit_z0})
          + table("setups.stock_changes.material.Custom", {"name": "Epoxy", "feed_scale_factor": 1.0}))

    # ── Setup 3: Front (flipped about the X axis on the pins)
    P.setup(2, "3 Front", "bottom",
            f"Front up, flipped on the pins. Z0 = spoilboard = the back face. CAUTION: the pins must stay below "
            f"{cam['pin_top_max']} (the face pass crosses them). Top clamps as in setup 1.")
    clamps(T1 + CLAMP_HEIGHT)
    face("Face front to T", cam["face_front"], f"Face the front from T1 {T1} to T {T} (block.json cam.face_front).")
    eng = eng_row
    P.toolpath("Engrave day labels V60", "trace", tid[eng["tool"]], mids["engrave.dxf"], {
        "depth": round(eng["z_start"] - eng["z_end"], 6), "depth_per_pass": round(eng["z_start"] - eng["z_end"], 6),
        "feed_rate": fv60["feed_safe"], "plunge_rate": fv60["plunge"], "compensation": "none",
        "spindle_rpm": int(fv60["rpm"])},
        why=(f"cam_ops.json {eng['layer']} (rebuilt 2026-11-01): the day labels, single-stroke text in the chin\n"
             f"under the slit, block z {eng['z_start']} -> {eng['z_end']} (local Z in this setup). After the face,\n"
             "before the outline profile. Open strokes: Trace cuts each from its first point to its last."),
        heights={"top_z": eng["z_start"]})
    front_leave = tomllib.loads((MONO / "block/testpiece.toml").read_text())["front"]["rough_stock"]
    # The flip maps setup-1 z to T1 - z, so the deepest relief point in setup 3 is T1 - (the highest sheet z),
    # and the relief is the highest sheet z deep below the stock top.
    front_span = float(sheet[..., 2].max())
    n_front = math.ceil(front_span / FRONT_ROUGH_6MM["depth_per_pass"])
    rough3d(f"Front bulk rough {big['diameter']:g} mm", "rough_18", "relief_sheet_s1.stl", None,
            (f"The bulk of the front relief with the 18 mm bit, {n_front} even levels over the {front_span:.2f} mm\n"
             "relief depth (a level cuts only where the surface is below it, so the bit needs several),\n"
             f"leave {front_leave}. No flat-area levels. Feeds: run Suggest."),
            in_relief, front_leave, geometry={"stepover": round(rig["adaptive_woc_factor"] * big["diameter"], 4),
                                              "depth_per_pass": round(front_span / n_front, 4)},
            detect_flat=False)
    fr6 = FRONT_ROUGH_6MM
    rough3d("Front 3D rough", "flat_6", "relief_sheet_s1.stl",
            {"feed_safe": fr6["feed_rate"], "plunge": fr6["plunge_rate"], "rpm": fr6["spindle_rpm"]},
            "The operator's GUI edit (2026-11-01): the 6 mm end mill and its geometry and feeds; now the rest\n"
            "after the 18 mm bulk pass (remaining stock).\n"
            "Stock to leave = testpiece.toml [front].rough_stock. Boundary: EDGE_OUTER, tool CENTRE on it, so the\n"
            "6 mm reaches the edge band the finish ball would otherwise meet at full height (2026-11-02).",
            dict(in_relief, containment="center"), tomllib.loads((MONO / "block/testpiece.toml").read_text())["front"]["rough_stock"],
            geometry={"stepover": fr6["stepover"], "depth_per_pass": fr6["depth_per_pass"]},
            stock_source="from_remaining_stock")
    # A whole-relief R1 rest (remaining stock) was measured 2026-11-02: 4266 m of cut, 3248 m of rapids
    # (the job went to 48 h). rs_cam has no "only where the 6 mm could not reach" region for a 3D Rough.
    tfin = tpl_ops["Scallop Finish"]["operation"]["params"]
    P.toolpath("Front scallop finish", "scallop", tid["tbn_r1"], mids["relief_sheet_s1.stl"], {
        "scallop_height": tfin["scallop_height"], "tolerance": tfin["tolerance"], "direction": tfin["direction"],
        "continuous": tfin["continuous"], "slope_from": 0.0, "slope_to": 90.0,
        "feed_rate": FRONT_SCALLOP_FEED["feed_rate"], "plunge_rate": FRONT_SCALLOP_FEED["plunge_rate"], "stock_to_leave": 0.0,
        "spindle_rpm": FRONT_SCALLOP_FEED["spindle_rpm"],
        "intra_pass_hookup_mm": tfin["intra_pass_hookup_mm"], "iso_field": tfin["iso_field"]},
        why=("rivmap350 'Scallop Finish' strategy; feed and rpm: the operator's GUI edit (2026-11-01). Boundary: EDGE_OUTER.\n"
             "Segment merge at 0.02 (under the 0.05 scallop tolerance): 562 k -> 466 k moves, job 6.64 -> 6.53 h."),
        boundary=in_relief, dressups={"lead_in_out": True, "segment_merge": True, "segment_merge_tolerance": 0.02})
    P.toolpath("Outline profile with tabs", "profile", tid["flat_6"], mids["block_outline.dxf"], {
        "side": "outside", "floor": floor(T), "depth_per_pass": f6["doc_rough"],
        "feed_rate": f6["feed_safe"], "plunge_rate": f6["plunge"], "ramp_feed_rate": round(f6["feed_safe"] / 3.0, 3),
        "climb": True, "tab_count": 8, "tab_width": TAB_WIDTH_DEFAULT, "tab_height": TAB_HEIGHT_DEFAULT,
        "finishing_passes": 0, "compensation": "in_computer", "spindle_rpm": int(f6["rpm"])},
        why=f"BLOCK_OUTLINE: {T} to the spoilboard, outside, 8 tabs (rs_cam default width and height).",
        heights={"top_z": T}, dressups=ramp)

    write_if_changed(HERE / "nz_wanaka_21.toml", current_schema(P.text()).encode())
    print(json.dumps({"stock": [STOCK_X, STOCK_Y, T1], "origin": [-mx, -my], "pins_stock_local": pins,
                      "ops": [(o["setup"], o["name"], order_tools[o["tool_id"]]) for o in P.ops]}, indent=1))


if __name__ == "__main__":
    main()
