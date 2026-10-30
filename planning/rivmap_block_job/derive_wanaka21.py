"""Derive the rs_cam two-sided job of the full-size RivMap oak block nz-wanaka-21.

Run:  uv run --with ezdxf --with numpy --with shapely python planning/rivmap_block_job/derive_wanaka21.py

The brief is project_rivmap_mono/manufacture/docs/rs_cam_full_block_prompt.md. The
script reads the block-stage output (read only) and writes:

- the oak block model and the DXF outlines in the back-view frame (= the rs_cam
  setup-1 frame) to /home/ricky/cnc_jobs/rivmap_block/nz_wanaka_21/;
- the project planning/rivmap_block_job/nz_wanaka_21.toml.

Method (operator, 2026-10-31: "use 3d ops for most of this ... 5-10 paths total"):
the back is a 3D rough of the finished oak block (mech oak_block_full.stl: the cavity,
the slots, the holes, the mech pockets and the through holes) with the 6 mm tool, then
a 3 mm rest rough; the V lines are one Project Curve op; the epoxy is a stock change;
the front is a 3D rough and a scallop finish of the terrain. 10 toolpaths.
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
PIN_PAIR_ACROSS = 100.0

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
    v_retract = cav_floor + SAFE_Z_CLEARANCE_MM         # rapids stay inside the convex cavity

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
    eb = ezdxf.readfile(BUILD / "terrain/nz-wanaka-21/machinable_edge_band.dxf")
    write_dxf("edge_outer.dxf", eb, ["EDGE_OUTER"], conv=lambda x, y: (bx1 - x, y - by0))

    # The finished oak block (stack.json world frame: block XY, Z = block Z + levels.z_block)
    # in the setup-1 frame: x = bx1 - X, y = Y - by0, z = T1 - (Z - z_block). A rotation (det +1).
    zb = stack["levels"]["z_block"]
    tris = read_stl(BUILD / "mech/nz-wanaka-21/oak_block_full.stl")
    assert abs(tris[..., 2].min() - zb) < 1e-3 and abs(tris[..., 2].max() - (zb + T)) < 1e-3
    s1 = np.empty_like(tris)
    s1[..., 0] = bx1 - tris[..., 0]
    s1[..., 1] = tris[..., 1] - by0
    s1[..., 2] = T1 - (tris[..., 2] - zb)
    write_stl(OUT / "oak_block_s1.stl", s1, "rivmap nz-wanaka-21 oak block, rs_cam setup-1 frame")
    files["oak_block_s1.stl"] = OUT / "oak_block_s1.stl"
    files["terrain_back.stl"] = BLOCK / "terrain_back.stl"

    # ── tools
    mt = {t["id"]: t for t in mapt["tools"]}
    ct = cam["tools"]
    contract = tomllib.loads((MONO / "design/rivmap.toml").read_text())["cutters"]
    order_tools = ["flat_6", "flat_3", "v20", "flat_6.35", "tbn_r1"]
    dia = {k: float(contract[mt[k]["cutter"]]) if "cutter" in mt[k] else ct[k]["diameter"] for k in order_tools}
    assert all(abs(dia[k] - ct[k]["diameter"]) < 1e-9 for k in order_tools), dia
    tid = {k: i for i, k in enumerate(order_tools)}

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

    # Pins: rs_cam preset "axis plus mirror pair", in the long-edge margins. A pin keeps
    # PIN_WALL to the stock edge and to the outline cut (the outer flat_6 width of the margin).
    pin_d = cam["pins"][0]["d"]
    band_lo = PIN_WALL_MM + pin_d / 2.0
    band_hi = mx - dia["flat_6"] - PIN_WALL_MM - pin_d / 2.0
    assert band_hi >= band_lo, (band_lo, band_hi)
    edge_off = (band_lo + band_hi) / 2.0
    axis_y = STOCK_Y / 2.0
    pins = [(edge_off, axis_y), (STOCK_X - edge_off, axis_y - PIN_PAIR_ACROSS),
            (STOCK_X - edge_off, axis_y + PIN_PAIR_ACROSS)]
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

    for k in order_tools:
        t, c = mt[k], ct[k]
        if c["kind"] == "flat":
            P.add(tool_table(tid[k], tid[k] + 1, t["name"], "end_mill", dia[k], c["reach"], t["shank"],
                             c["reach"], t["flutes"], comment=f"map.toml {k}; block.json cam.tools"))
        elif c["kind"] == "vbit":
            cone = (dia[k] / 2.0) / math.tan(math.radians(c["angle"] / 2.0))
            P.add(tool_table(tid[k], tid[k] + 1, t["name"], "v_bit", dia[k], cone, t["shank"], c["reach"],
                             t["flutes"], included_angle=c["angle"], taper_half_angle=c["angle"] / 2.0,
                             comment=f"map.toml {k}; size and angle from block.json cam.tools; cone {cone:.4f}"))
        else:
            P.add(tool_table(tid[k], tid[k] + 1, t["name"], "tapered_ball_nose", dia[k], t["flute_length"],
                             t["shank"], c["reach"], t["flutes"], taper_half_angle=t["taper_half_angle"],
                             comment=f"map.toml {k} (endmill.com.au TBN-R1.0-3.175-15)"))

    model_order = ["oak_block_s1.stl", "terrain_back.stl", "block_outline.dxf", "edge_outer.dxf",
                   "cavity_d9.00.dxf", "vgroove_v20.dxf"]
    assert sorted(model_order) == sorted(files)
    mids = {f: i for i, f in enumerate(model_order)}
    for f in model_order:
        P.add(model_table(mids[f], files[f]))

    tol = mapt["vgroove"]["tip_tol"]
    ramp = {"entry_style": "ramp", "segment_merge": True, "segment_merge_tolerance": tol}
    f6, f3, fv, f635, ftb = (feeds[k] for k in ("flat_6", "flat_3", "v20", "flat_6.35", "tbn_r1"))
    rig = machine["rigidity"]
    trough = tpl_ops["3D Rough 8"]["operation"]["params"]

    def face(name: str, depth: float, why: str, heights: dict | None = None) -> None:
        P.toolpath(name, "face", tid["flat_6"], mids["block_outline.dxf"], {
            "stepover": f6["woc_rough"], "depth": depth, "depth_per_pass": f6["doc_rough"],
            "feed_rate": f6["feed_safe"], "plunge_rate": f6["plunge"],
            "ramp_feed_rate": round(f6["feed_safe"] / 3.0, 3), "stock_offset": dia["flat_6"],
            "direction": "zigzag", "spindle_rpm": int(f6["rpm"])},
            why=why + "\nstock_offset = the tool diameter, so the last row reaches the far stock edge.",
            heights=heights, dressups=ramp)

    def rough3d(name: str, tool: str, model: str, f: dict, why: str, boundary: dict, leave: float,
                stock_source: str = "fresh", geometry: dict | None = None) -> None:
        # Adaptive geometry from the machine rigidity (adaptive_doc_factor x D, adaptive_woc_factor x D)
        # unless the caller gives it. Run Suggest to tune it.
        g = geometry or {"stepover": round(rig["adaptive_woc_factor"] * dia[tool], 4),
                         "depth_per_pass": round(rig["adaptive_doc_factor"] * dia[tool], 4)}
        P.toolpath(name, "adaptive3d", tid[tool], mids[model], {
            **g, "stock_to_leave_axial": leave, "feed_rate": f["feed_safe"], "plunge_rate": f["plunge"],
            "ramp_feed_rate": round(f["feed_safe"] / 3.0, 3), "tolerance": trough["tolerance"],
            "min_cutting_radius": 0.0, "entry_style": "ramp", "ramp_angle_deg": 3.0,
            "helix_radius_factor": trough["helix_radius_factor"], "helix_pitch": trough["helix_pitch"],
            "entry_clearance_mm": trough["entry_clearance_mm"], "detect_flat_areas": True,
            "region_ordering": trough["region_ordering"], "clearing_strategy": trough["clearing_strategy"],
            "trochoid_cap_mult": trough["trochoid_cap_mult"], "engagement_measure": trough["engagement_measure"],
            "z_blend": False, "spindle_rpm": int(f["rpm"]),
            "min_region_cut_length_mm": trough["min_region_cut_length_mm"],
            "stay_down_clearance_mm": trough["stay_down_clearance_mm"]},
            why=why, boundary=boundary, dressups={"segment_merge": True}, stock_source=stock_source)

    in_block = {"enabled": True, "containment": "inside", "offset": 0.0, "model_outline": mids["block_outline.dxf"]}
    in_relief = {"enabled": True, "containment": "inside", "offset": 0.0, "model_outline": mids["edge_outer.dxf"]}

    # ── Setup 1: Back
    P.setup(0, "1 Back", "top", f"Back up. Z0 = spoilboard. XY0 = stock corner (pin 1 is at {pins[0]}).")
    face("Face back to T1", cam["raw_top_max"] - T1,
         f"Face from raw_top_max {cam['raw_top_max']} to T1 {T1}.", heights={"top_z": cam["raw_top_max"]})
    P.toolpath("Pin drill, flip pins", "alignment_pin_drill", tid["flat_6"], mids["block_outline.dxf"], {
        "diameter": pin_d, "spoilboard_penetration": round(cam["pin_depth"] - T1, 6), "cycle": "peck",
        "peck_depth": dia["flat_6"], "feed_rate": f6["plunge"], "retract_z": 2.0, "spindle_rpm": int(f6["rpm"])},
        why=f"Three pins (axis + mirror pair). Depth = block.json cam.pin_depth {cam['pin_depth']} = the flat_6 reach.")
    rough3d("Back 3D rough 6 mm", "flat_6", "oak_block_s1.stl", f6,
            "The finished oak block: the cavity, the slots, the holes, the mech pockets, the through holes.\n"
            "Boundary: the block outline, tool inside (the margin with the pins stays).",
            in_block, 0.0)
    rough3d("Back 3D rest 3 mm", "flat_3", "oak_block_s1.stl", f3,
            "What the 6 mm tool cannot reach: the 4 mm slots, the light holes, the narrow mech pockets.\n"
            "Remaining stock: generate_all simulates the setup first.",
            in_block, 0.0, stock_source="from_remaining_stock")
    P.toolpath("V rivers and lakes", "project_curve", tid["v20"], mids["vgroove_v20.dxf"], {
        "projection": "point", "depth": round(-(blk["line_width"] / 2.0) / math.tan(math.radians(blk["vgroove_angle"] / 2.0)), 9),
        "point_spacing": tol, "feed_rate": fv["feed_safe"], "plunge_rate": fv["plunge"],
        "surface_model_id": mids["terrain_back.stl"], "direction": "from_below", "side": "center",
        "spindle_rpm": int(fv["rpm"]), "chain_distance_mm": 0.0},
        why=("Project Curve on the front surface from below, point projection: tip = surface - (w/2)/tan(a/2).\n"
             "One pass (no pre-pass). Retract inside the convex cavity."),
        heights={"top_z": cav_floor, "retract_z": v_retract})

    # ── Setup 2: the pour, then the skim
    P.setup(1, "2 Back skim", "top", "Back up, on the pins, after the cure. Same zero as setup 1.")
    overfill = mapt["paths"]["overfill"]
    level = cav_floor + overfill
    P.add("\n# The pour: fill the cavity to its floor + map.toml [paths].overfill.\n"
          + table("[setups.stock_changes]", {"id": 0, "name": "epoxy", "enabled": True, "op": "add",
                                              "cut_as": "stock_material"}).replace(
              "[[[setups.stock_changes]]]", "[[setups.stock_changes]]")
          + table("setups.stock_changes.geometry", {"kind": "outline_fill",
                                                     "model_ids": [mids["cavity_d9.00.dxf"]], "level_z": level})
          + table("setups.stock_changes.material.Custom", {"name": "Epoxy", "feed_scale_factor": 1.0}))
    P.toolpath("Skim epoxy to the cavity floor", "pocket", tid["flat_6"], mids["cavity_d9.00.dxf"], {
        "stepover": f6["woc_rough"], "floor": floor(overfill), "depth_per_pass": overfill,
        "feed_rate": f6["feed_safe"], "plunge_rate": f6["plunge"], "ramp_feed_rate": round(f6["feed_safe"] / 3.0, 3),
        "climb": True, "pattern": "zigzag", "angle": 0.0, "finishing_passes": 0, "spindle_rpm": int(f6["rpm"])},
        why=f"One pass: the overfill {level} down to the cavity floor {cav_floor} (block z {blk['cavity_depth']}).",
        heights={"top_z": level}, dressups=ramp)

    # ── Setup 3: Front (flipped about the X axis on the pins)
    P.setup(2, "3 Front", "bottom",
            f"Front up, flipped on the pins. Z0 = spoilboard = the back face. CAUTION: the pins must stay below "
            f"{cam['pin_top_max']} (the face pass crosses them).")
    face("Face front to T", cam["face_front"], f"Face the front from T1 {T1} to T {T} (block.json cam.face_front).")
    rough3d("Front 3D rough", "flat_6.35", "terrain_back.stl", f635,
            "rivmap350 '3D Rough 8' geometry; stock to leave = testpiece.toml [front].rough_stock. Boundary: EDGE_OUTER.",
            in_relief, tomllib.loads((MONO / "block/testpiece.toml").read_text())["front"]["rough_stock"],
            geometry={"stepover": trough["stepover"], "depth_per_pass": trough["depth_per_pass"]})
    tfin = tpl_ops["Scallop Finish"]["operation"]["params"]
    P.toolpath("Front scallop finish", "scallop", tid["tbn_r1"], mids["terrain_back.stl"], {
        "scallop_height": tfin["scallop_height"], "tolerance": tfin["tolerance"], "direction": tfin["direction"],
        "continuous": tfin["continuous"], "slope_from": 0.0, "slope_to": 90.0, "feed_rate": ftb["feed_safe"],
        "plunge_rate": ftb["plunge"], "stock_to_leave": 0.0, "spindle_rpm": int(ftb["rpm"]),
        "intra_pass_hookup_mm": tfin["intra_pass_hookup_mm"], "iso_field": tfin["iso_field"]},
        why="rivmap350 'Scallop Finish' strategy; feeds_oak tbn_r1. Boundary: EDGE_OUTER.",
        boundary=in_relief, dressups={"lead_in_out": True})
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
