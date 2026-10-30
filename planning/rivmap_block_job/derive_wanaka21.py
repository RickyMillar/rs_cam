"""Derive the rs_cam two-sided job of the full-size RivMap oak block nz-wanaka-21.

Run:  uv run --with ezdxf --with numpy --with shapely python planning/rivmap_block_job/derive_wanaka21.py

The brief is project_rivmap_mono/manufacture/docs/rs_cam_full_block_prompt.md. The
script reads the block-stage output (read only) and writes:

- one DXF per operation (the back-view frame = the rs_cam setup-1 frame) and the
  edge band to /home/ricky/cnc_jobs/rivmap_block/nz_wanaka_21/;
- the project planning/rivmap_block_job/nz_wanaka_21.toml.

The operations, their order, their tools and their depths come from cam_ops.json.
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
from derive import (Dxf, Project, entity_points, layer_entities, model_table, table, tool_table,
                    write_if_changed)

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
    rows = json.loads((BLOCK / cam["ops"]).read_text())
    mapt = tomllib.loads((MONO / "block/map.toml").read_text())
    feeds = tomllib.loads((MONO / "block/feeds_oak.toml").read_text())["tools"]
    tpl = tomllib.loads(TEMPLATE.read_text())
    tpl_ops = {tp["name"]: tp for s in tpl["setups"] for tp in s["toolpaths"]}

    T1 = cam["T1"]
    T = blk["thickness"]
    bx0, by0, bx1, by1 = blk["block"]
    BW, BD = bx1 - bx0, by1 - by0                       # 358 x 443
    assert cam["frame"].startswith(f"back view") and f"x = {bx1} - X, y = Y + {-by0}" in cam["frame"]
    # The block is centred on the operator's stock.
    mx, my = (STOCK_X - BW) / 2.0, (STOCK_Y - BD) / 2.0  # 18.5, 8.5
    ceil = T1 - blk["cavity_rough"]                     # setup-1 Z of the rough cavity ceiling
    v_retract = ceil + SAFE_Z_CLEARANCE_MM              # rapids stay inside the convex cavity

    # ── models: one DXF per operation row, in the back-view frame
    docs = {"back_channels.dxf": ezdxf.readfile(BLOCK / "back_channels.dxf"),
            "block_back.dxf": ezdxf.readfile(BUILD / "mech/nz-wanaka-21/block_back.dxf")}
    files: dict[str, Path] = {}

    def copy_layer(doc, layer: str, fname: str) -> Path:
        d = Dxf()
        n = 0
        for e in layer_entities(doc, layer):
            t = e.dxftype()
            if t == "CIRCLE":
                d.circle(layer, e.dxf.center.x, e.dxf.center.y, e.dxf.radius)
            elif t in ("LWPOLYLINE", "POLYLINE"):
                pts, bul, closed, _ = entity_points(e)
                xs = [p[0] for p in pts]; ys = [p[1] for p in pts]
                if layer == "THROUGH_D25.00" and (min(xs), max(xs), min(ys), max(ys)) == (0.0, BW, 0.0, BD):
                    continue  # the block outline: the setup-3 profile cuts it (BLOCK_OUTLINE)
                d.polyline(layer, pts, closed, bul)
            elif t == "LINE":
                # Trace skips a ring under 3 points: the line gets its mid point.
                a, b = (e.dxf.start.x, e.dxf.start.y), (e.dxf.end.x, e.dxf.end.y)
                d.polyline(layer, [a, ((a[0] + b[0]) / 2, (a[1] + b[1]) / 2), b], False)
            else:
                raise ValueError(f"{layer}: {t}")
            n += 1
        assert n > 0, layer
        p = OUT / fname
        write_if_changed(p, d.text().encode())
        files[fname] = p
        return p

    jobs = [r for r in rows if r["setup"] != "hand"]
    hand = [r["layer"] for r in rows if r["setup"] == "hand"]
    for r in jobs:
        copy_layer(docs[r["file"]], r["layer"], r["layer"].lower() + ".dxf")
    # The relief extent (EDGE_OUTER, block frame) in the back frame.
    eb = ezdxf.readfile(BUILD / "terrain/nz-wanaka-21/machinable_edge_band.dxf")
    d = Dxf()
    for e in layer_entities(eb, "EDGE_OUTER"):
        pts, _, closed, _ = entity_points(e)
        d.polyline("EDGE_OUTER", [(bx1 - x, y - by0) for x, y in pts], closed)
    write_if_changed(OUT / "edge_outer.dxf", d.text().encode())
    files["edge_outer.dxf"] = OUT / "edge_outer.dxf"
    terrain = BLOCK / "terrain_back.stl"

    # ── tools
    mt = {t["id"]: t for t in mapt["tools"]}
    ct = cam["tools"]
    contract = tomllib.loads((MONO / "design/rivmap.toml").read_text())["cutters"]
    dia = {k: float(contract[mt[k]["cutter"]]) if "cutter" in mt[k] else ct[k]["diameter"] for k in ct}
    assert all(abs(dia[k] - ct[k]["diameter"]) < 1e-9 for k in ct), dia
    order_tools = ["flat_6", "flat_3", "v20", "v60", "flat_6.35", "tbn_r1"]
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

    # Pins: one on the flip axis plus a mirror pair (rs_cam preset axis_plus_mirror_pair), in the
    # long-edge margins. Margin = mx; the outline profile cuts the outer flat_6 width of it.
    # A pin keeps PIN_WALL to the stock edge and to the profile cut; centred in that band.
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

    model_order = [str(terrain), "edge_outer.dxf"] + sorted(files.keys() - {"edge_outer.dxf"})
    mids = {}
    for i, f in enumerate(model_order):
        mids[f] = i
        P.add(model_table(i, Path(f) if f.startswith("/") else files[f]))
    M_TERRAIN = mids[str(terrain)]

    def mid(layer: str) -> int:
        return mids[layer.lower() + ".dxf"]

    def fr(tool: str) -> dict:
        return feeds[tool]

    def pocket_params(tool: str, depth: float, slot: bool) -> dict:
        f = fr(tool)
        feed = f["feed_safe"] / 2.0 if slot else f["feed_safe"]
        return {"stepover": f["woc_rough"], "floor": floor(depth), "depth_per_pass": f["doc_rough"],
                "feed_rate": feed, "plunge_rate": f["plunge"], "ramp_feed_rate": round(feed / 3.0, 3),
                "climb": True, "pattern": "contour", "angle": 0.0, "finishing_passes": 0,
                "spindle_rpm": int(f["rpm"])}

    tol = mapt["vgroove"]["tip_tol"]
    merge = {"segment_merge": True, "segment_merge_tolerance": tol}
    entry = {"ramp": {"entry_style": "ramp", **merge}, "helix": {"entry_style": "helix", **merge}}
    f6 = fr("flat_6")
    flat6_d = dia["flat_6"]

    # ── Setup 1: Back
    P.setup(0, "1 Back", "top", f"Back up. Z0 = spoilboard. XY0 = stock corner (pin 1 is at {pins[0]}).")
    P.toolpath("Face back to T1", "face", tid["flat_6"], mids["edge_outer.dxf"], {
        "stepover": f6["woc_rough"], "depth": cam["raw_top_max"] - T1, "depth_per_pass": f6["doc_rough"],
        "feed_rate": f6["feed_safe"], "plunge_rate": f6["plunge"], "ramp_feed_rate": round(f6["feed_safe"] / 3.0, 3),
        "stock_offset": flat6_d, "direction": "zigzag", "spindle_rpm": int(f6["rpm"])},
        why=(f"Face from raw_top_max {cam['raw_top_max']} to T1 {T1}. stock_offset = the tool diameter, so the\n"
             "last row reaches the far stock edge (an offset of r leaves a raw strip)."),
        heights={"top_z": cam["raw_top_max"]}, dressups=entry["ramp"])
    P.toolpath("Pin drill, flip pins", "alignment_pin_drill", tid["flat_6"], mids["edge_outer.dxf"], {
        "diameter": pin_d, "spoilboard_penetration": round(cam["pin_depth"] - T1, 6), "cycle": "peck",
        "peck_depth": flat6_d, "feed_rate": f6["plunge"], "retract_z": 2.0, "spindle_rpm": int(f6["rpm"])},
        why=f"Three pins (axis + mirror pair). Depth = block.json cam.pin_depth {cam['pin_depth']} = the flat_6 reach.")

    sections = {1: [], 2: [], 3: []}
    for r in jobs:
        sections[r["setup"]].append(r)
    for r in sections[1]:
        layer, tool, z0, z1 = r["layer"], r["tool"], r["z_start"], r["z_end"]
        slot = tool == "flat_3"
        inside_cavity = r["file"] == "back_channels.dxf" and z0 >= blk["cavity_rough"]
        heights = {"top_z": T1 - z0} if z0 > 0 else {}
        if inside_cavity:
            heights["retract_z"] = v_retract
        if r["op"] == "pocket":
            P.toolpath(layer, "pocket", tid[tool], mid(layer), pocket_params(tool, z1 - z0, slot),
                       why=(f"cam_ops.json {layer}: block z {z0} -> {z1} (setup-1 Z {T1 - z0:.3f} -> {T1 - z1:.3f}).\n"
                            + ("Slot feed: half feed_safe. " if slot else "")
                            + ("Retract inside the convex cavity." if inside_cavity else "")),
                       heights=heights, dressups=entry[r["entry"]])
        elif r["op"] == "v_path":
            half = math.radians(blk["vgroove_angle"] / 2.0)
            v_offset = (blk["line_width"] / 2.0) / math.tan(half)
            cone = (dia["v20"] / 2.0) / math.tan(math.radians(ct["v20"]["angle"] / 2.0))
            usable = cone - mapt["vgroove"]["body_margin"]
            fv = fr("v20")
            for label, depth in (("pre-pass", -v_offset + usable / 2.0), ("final", -v_offset)):
                P.toolpath(f"{layer} {label}", "project_curve", tid["v20"], mid(layer), {
                    "projection": "point", "depth": round(depth, 9), "point_spacing": tol,
                    "feed_rate": fv["feed_safe"], "plunge_rate": fv["plunge"], "surface_model_id": M_TERRAIN,
                    "direction": "from_below", "side": "center", "spindle_rpm": int(fv["rpm"]),
                    "chain_distance_mm": 0.0},
                    why=(f"Project Curve on the front surface from below, point projection. Final depth\n"
                         f"-(w/2)/tan(a/2) = {-v_offset:.4f}; pre-pass = final + usable cone/2 = {depth:.4f}."),
                    heights={"top_z": ceil, "retract_z": v_retract})
        elif r["op"] == "trace":
            fv = fr(tool)
            P.toolpath(layer, "trace", tid[tool], mid(layer), {
                "depth": round(z1 - z0, 6), "depth_per_pass": round((z1 - z0) / 2.0, 6),
                "feed_rate": fv["feed_safe"], "plunge_rate": fv["plunge"], "compensation": "none",
                "spindle_rpm": int(fv["rpm"])},
                why=(f"{layer}: V tip from the slit slot floor (block z {z0}) to {z1} "
                     f"({z1 - T1:.3f} into the spoilboard). Two passes."),
                heights={"top_z": T1 - z0})
        else:
            raise ValueError(r)

    # ── Setup 2: Back skim, after the pour
    P.setup(1, "2 Back skim", "top", "Back up, on the pins, after the cure. Same zero as setup 1.")
    cav_row = next(r for r in sections[1] if r["layer"].startswith("CAVITY_ROUGH"))
    overfill = mapt["paths"]["overfill"]
    P.add("\n# The pour: fill the cavity to its ceiling + map.toml [paths].overfill.\n"
          + table("[setups.stock_changes]", {"id": 0, "name": "epoxy", "enabled": True, "op": "add",
                                              "cut_as": "stock_material"}).replace(
              "[[[setups.stock_changes]]]", "[[setups.stock_changes]]")
          + table("setups.stock_changes.geometry", {"kind": "outline_fill",
                                                     "model_ids": [mid(cav_row["layer"])],
                                                     "level_z": ceil + overfill})
          + table("setups.stock_changes.material.Custom", {"name": "Epoxy", "feed_scale_factor": 1.0}))
    for r in sections[2]:
        layer, z0, z1 = r["layer"], r["z_start"], r["z_end"]
        p = pocket_params(r["tool"], z1 - z0, False)
        p["pattern"] = "zigzag"
        p["depth_per_pass"] = round(z1 - z0, 6)
        P.toolpath(layer, "pocket", tid[r["tool"]], mid(layer), p,
                   why=(f"cam_ops.json {layer}: one pass, block z {z0} -> {z1}. Fresh stock in generation: a 2.5D pocket\n"
                        "from a pinned top does not read the stock, and remaining stock makes it wait for a setup-1 simulation."),
                   heights={"top_z": T1 - z0}, dressups=entry[r["entry"]])

    # ── Setup 3: Front (flipped about the X axis on the pins)
    P.setup(2, "3 Front", "bottom",
            f"Front up, flipped on the pins. Z0 = spoilboard = the back face. CAUTION: the pins must stay below "
            f"{cam['pin_top_max']} (the face pass crosses them).")
    P.toolpath("Face front to T", "face", tid["flat_6"], mids["edge_outer.dxf"], {
        "stepover": f6["woc_rough"], "depth": cam["face_front"], "depth_per_pass": f6["doc_rough"],
        "feed_rate": f6["feed_safe"], "plunge_rate": f6["plunge"], "ramp_feed_rate": round(f6["feed_safe"] / 3.0, 3),
        "stock_offset": flat6_d, "direction": "zigzag", "spindle_rpm": int(f6["rpm"])},
        why=f"Face the front from T1 {T1} to T {T} (block.json cam.face_front).", dressups=entry["ramp"])
    bnd = {"enabled": True, "containment": "inside", "offset": 0.0, "model_outline": mids["edge_outer.dxf"]}
    trough = tpl_ops["3D Rough 8"]["operation"]["params"]
    f635 = fr("flat_6.35")
    P.toolpath("Front 3D rough", "adaptive3d", tid["flat_6.35"], M_TERRAIN, {
        "stepover": trough["stepover"], "depth_per_pass": trough["depth_per_pass"],
        "stock_to_leave_axial": tomllib.loads((MONO / "block/testpiece.toml").read_text())["front"]["rough_stock"],
        "feed_rate": f635["feed_safe"], "plunge_rate": f635["plunge"],
        "ramp_feed_rate": round(f635["feed_safe"] / 3.0, 3), "tolerance": trough["tolerance"],
        "min_cutting_radius": 0.0, "entry_style": trough["entry_style"], "ramp_angle_deg": trough["ramp_angle_deg"],
        "helix_radius_factor": trough["helix_radius_factor"], "helix_pitch": trough["helix_pitch"],
        "entry_clearance_mm": trough["entry_clearance_mm"], "detect_flat_areas": False,
        "region_ordering": trough["region_ordering"], "clearing_strategy": trough["clearing_strategy"],
        "trochoid_cap_mult": trough["trochoid_cap_mult"], "engagement_measure": trough["engagement_measure"],
        "z_blend": False, "spindle_rpm": int(f635["rpm"]),
        "min_region_cut_length_mm": trough["min_region_cut_length_mm"],
        "stay_down_clearance_mm": trough["stay_down_clearance_mm"]},
        why="rivmap350 '3D Rough 8' strategy; stock to leave = testpiece.toml [front].rough_stock; feeds_oak flat_6.35.",
        boundary=bnd, dressups={"segment_merge": True})
    tfin = tpl_ops["Scallop Finish"]["operation"]["params"]
    ftb = fr("tbn_r1")
    P.toolpath("Front scallop finish", "scallop", tid["tbn_r1"], M_TERRAIN, {
        "scallop_height": tfin["scallop_height"], "tolerance": tfin["tolerance"], "direction": tfin["direction"],
        "continuous": tfin["continuous"], "slope_from": 0.0, "slope_to": 90.0, "feed_rate": ftb["feed_safe"],
        "plunge_rate": ftb["plunge"], "stock_to_leave": 0.0, "spindle_rpm": int(ftb["rpm"]),
        "intra_pass_hookup_mm": tfin["intra_pass_hookup_mm"], "iso_field": tfin["iso_field"]},
        why="rivmap350 'Scallop Finish' strategy; feeds_oak tbn_r1.", boundary=bnd, dressups={"lead_in_out": True})
    for r in sections[3]:
        layer, z0, z1 = r["layer"], r["z_start"], r["z_end"]
        assert r["op"] == "profile_outside" and z1 == 0.0
        f = fr(r["tool"])
        P.toolpath("Outline profile with tabs", "profile", tid[r["tool"]], mid(layer), {
            "side": "outside", "floor": floor(z0 - z1), "depth_per_pass": f["doc_rough"],
            "feed_rate": f["feed_safe"], "plunge_rate": f["plunge"], "ramp_feed_rate": round(f["feed_safe"] / 3.0, 3),
            "climb": True, "tab_count": 8, "tab_width": TAB_WIDTH_DEFAULT, "tab_height": TAB_HEIGHT_DEFAULT,
            "finishing_passes": 0, "compensation": "in_computer", "spindle_rpm": int(f["rpm"])},
            why=f"cam_ops.json {layer}: {z0} to the spoilboard, outside, 8 tabs (rs_cam default width and height).",
            heights={"top_z": z0}, dressups=entry[r["entry"]])

    write_if_changed(HERE / "nz_wanaka_21.toml", current_schema(P.text()).encode())
    print(json.dumps({"stock": [STOCK_X, STOCK_Y, T1], "origin": [-mx, -my], "pins_stock_local": pins,
                      "pin_band_from_long_edge": [band_lo, band_hi], "hand_skipped": hand,
                      "ops": [(o["setup"], o["name"], order_tools[o["tool_id"]]) for o in P.ops]}, indent=1))


if __name__ == "__main__":
    main()
