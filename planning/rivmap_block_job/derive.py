"""Derive the rs_cam two-sided jobs of the RivMap oak block.

Run:  uv run --with ezdxf --with numpy --with shapely python planning/rivmap_block_job/derive.py

The script reads the block-stage output of the monorepo (read only) and writes:

- the model files (one DXF per operation, the terrain in the setup-1 frame)
  to /home/ricky/cnc_jobs/rivmap_block/<part>/ (outside both repositories);
- the rs_cam project files planning/rivmap_block_job/nz_south.toml and
  testpiece.toml (absolute model paths).

A second run writes the same bytes. The DXF and the STL writers below are
local and have no time stamp or GUID, for that reason.

Every number comes from an input file, from an rs_cam source constant (named
with its file), or from one of the two operator values in OPERATOR below.
See planning/rivmap_block_job.md for the frame and the method.
"""

from __future__ import annotations

import csv
import json
import math
import re
import struct
import tomllib
from pathlib import Path

import ezdxf
import numpy as np
from shapely.geometry import Polygon

MONO = Path("/home/ricky/personal_repos/project_rivmap_mono/manufacture")
RS_CAM = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
OUT = Path("/home/ricky/cnc_jobs/rivmap_block")
TEMPLATE = Path("/home/ricky/Downloads/big/rivmap_export_350_500/rivmap350.toml")

# ── Operator values (the lead and the operator, 2026-10-08) ──────────────────
OPERATOR = {
    # The faced back thickness: the back is faced to this height over the
    # spoilboard in setup 1. The raw oak is >= 26 mm and uneven.
    "T1": 25.5,
    # The highest raw stock top that the setup-1 facing pass starts from.
    "raw_top": 27.0,
}

# ── rs_cam source constants (read from the source, asserted below) ───────────
# crates/rs_cam_core/src/compute/alignment_pins.rs
PIN_WALL_MM = 2.0
PIN_KEYING_ASYMMETRY_MM = 3.0
# crates/rs_cam_core/src/compute/config.rs
SAFE_Z_CLEARANCE_MM = 5.0
# crates/rs_cam_core/src/compute/operation_configs.rs (ProfileConfig::default)
TAB_WIDTH_DEFAULT = 6.0
TAB_HEIGHT_DEFAULT = 2.0


def assert_rs_cam_constants() -> None:
    src = RS_CAM / "crates/rs_cam_core/src"
    pins = (src / "compute/alignment_pins.rs").read_text()
    assert f"pub const PIN_WALL_MM: f64 = {PIN_WALL_MM};" in pins
    assert f"pub const PIN_KEYING_ASYMMETRY_MM: f64 = {PIN_KEYING_ASYMMETRY_MM};" in pins
    cfg = (src / "compute/config.rs").read_text()
    assert f"pub const SAFE_Z_CLEARANCE_MM: f64 = {SAFE_Z_CLEARANCE_MM};" in cfg
    ops = (src / "compute/operation_configs.rs").read_text()
    assert f"tab_width: {TAB_WIDTH_DEFAULT}," in ops
    assert f"tab_height: {TAB_HEIGHT_DEFAULT}," in ops
    kin = (src / "machine/kinematics.rs").read_text()
    assert "acceleration_xyz_mm_s2: Some([500.0, 500.0, 270.0])," in kin
    assert "max_rate_xyz_mm_min: Some([10_000.0, 10_000.0, 1_000.0])," in kin
    assert "junction_deviation_mm: 0.020," in kin


# ── Small deterministic writers ──────────────────────────────────────────────


def fmt(v: float) -> str:
    """A float for TOML and DXF: shortest exact repr, always with a point."""
    r = repr(float(round(v, 9)))
    return r if ("." in r or "e" in r) else r + ".0"


class Dxf:
    """A minimal AC1009 DXF: header units + ENTITIES. No handles, no time."""

    def __init__(self) -> None:
        self.lines: list[str] = []

    def _g(self, code: int, value: str) -> None:
        self.lines += [str(code), value]

    def polyline(self, layer: str, pts, closed: bool, bulges=None) -> None:
        self._g(0, "POLYLINE")
        self._g(8, layer)
        self._g(66, "1")
        self._g(10, "0.0")
        self._g(20, "0.0")
        self._g(30, "0.0")
        self._g(70, "1" if closed else "0")
        for i, (x, y) in enumerate(pts):
            self._g(0, "VERTEX")
            self._g(8, layer)
            self._g(10, fmt(x))
            self._g(20, fmt(y))
            self._g(30, "0.0")
            b = bulges[i] if bulges else 0.0
            if abs(b) > 1e-12:
                self._g(42, fmt(b))
        self._g(0, "SEQEND")
        self._g(8, layer)

    def circle(self, layer: str, cx: float, cy: float, r: float) -> None:
        self._g(0, "CIRCLE")
        self._g(8, layer)
        self._g(10, fmt(cx))
        self._g(20, fmt(cy))
        self._g(30, "0.0")
        self._g(40, fmt(r))

    def text(self) -> str:
        head = ["0", "SECTION", "2", "HEADER", "9", "$ACADVER", "1", "AC1009",
                "9", "$INSUNITS", "70", "4", "0", "ENDSEC",
                "0", "SECTION", "2", "ENTITIES"]
        tail = ["0", "ENDSEC", "0", "EOF"]
        return "\n".join(head + self.lines + tail) + "\n"


def write_if_changed(path: Path, data: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists() and path.read_bytes() == data:
        return
    path.write_bytes(data)


def read_stl(path: Path) -> np.ndarray:
    raw = path.read_bytes()
    n = struct.unpack_from("<I", raw, 80)[0]
    assert len(raw) == 84 + 50 * n, "binary STL expected"
    rec = np.frombuffer(raw, dtype=np.dtype([("n", "<f4", 3), ("v", "<f4", (3, 3)), ("a", "<u2")]),
                        count=n, offset=84)
    return rec["v"].astype(np.float64)


def write_stl(path: Path, tris: np.ndarray, header: str) -> None:
    n = len(tris)
    e1 = tris[:, 1] - tris[:, 0]
    e2 = tris[:, 2] - tris[:, 0]
    nv = np.cross(e1, e2)
    ln = np.linalg.norm(nv, axis=1)
    nv = nv / np.where(ln > 0, ln, 1.0)[:, None]
    rec = np.zeros(n, dtype=np.dtype([("n", "<f4", 3), ("v", "<f4", (3, 3)), ("a", "<u2")]))
    rec["n"] = nv
    rec["v"] = tris
    data = header.encode().ljust(80, b" ")[:80] + struct.pack("<I", n) + rec.tobytes()
    write_if_changed(path, data)


def entity_points(e):
    """(points xy, bulges, closed, z values) of an (LW)POLYLINE."""
    if e.dxftype() == "LWPOLYLINE":
        pts = [(p[0], p[1]) for p in e.get_points("xyb")]
        bul = [p[2] for p in e.get_points("xyb")]
        return pts, bul, bool(e.closed), [0.0] * len(pts)
    verts = list(e.vertices)
    pts = [(v.dxf.location.x, v.dxf.location.y) for v in verts]
    bul = [v.dxf.bulge for v in verts]
    zs = [v.dxf.location.z for v in verts]
    return pts, bul, bool(e.is_closed), zs


def layer_entities(doc, layer: str):
    return [e for e in doc.modelspace() if e.dxf.layer == layer]


def flat_polys(doc, layer: str, sagitta: float) -> list:
    """The closed polylines of a layer as shapely polygons, arcs flattened."""
    from ezdxf import path as ezpath
    out = []
    for e in layer_entities(doc, layer):
        pts = [(v.x, v.y) for v in ezpath.make_path(e).flattening(sagitta)]
        out.append(Polygon(pts).buffer(0))
    return out


def even_odd(polys) -> "Polygon":
    geom = Polygon()
    for p in sorted(polys, key=lambda p: -p.area):
        geom = geom.symmetric_difference(p)
    return geom


def slot_starts(doc, layers: list[str], ceiling: float, doc_mm: float, woc_mm: float,
                sagitta: float) -> dict:
    """The block depth where each slot layer starts its first pass.

    A layer holds the area whose floor is at that depth or deeper, so most of
    layer k lies inside layer k-1, and its material starts at that floor. The
    layers do not nest exactly: small parts of layer k lie outside layer j.
    There the material starts higher (at a shallower floor, or at the cavity
    ceiling), so the first pass cuts deeper than doc in that part.

    Rule: layer k starts at the floor of the deepest layer j < k for which every
    part of (layer k - layer j) keeps the first-pass chip area at or under the
    nominal one: width x (doc + extra depth) <= doc x woc (feeds_oak flat_3).
    The extra depth of a part is the floor of layer j less the highest material
    top in the part. No such j: the layer starts at the cavity ceiling.
    """
    from shapely import maximum_inscribed_circle
    from shapely.geometry import Point
    geoms = [even_odd(flat_polys(doc, l, sagitta)) for l in layers]
    zs = [float(l.split("_Z")[1]) for l in layers]
    budget = doc_mm * woc_mm
    out = {layers[0]: (ceiling, "The first layer: from where the material starts.")}

    def top_at(x, y, j):  # the material top (block depth) at (x, y) before layer j+1 runs
        top = ceiling
        for g, z in zip(geoms[: j + 1], zs[: j + 1]):
            if g.contains(Point(x, y)):
                top = z
        return top

    for k in range(1, len(layers)):
        chosen = None
        for j in range(k - 1, -1, -1):
            diff = geoms[k].difference(geoms[j])
            parts = [g for g in (diff.geoms if hasattr(diff, "geoms") else [diff]) if g.area > 0]
            ok = True
            worst = 0.0
            for part in parts:
                mic = maximum_inscribed_circle(part)
                width = 2.0 * mic.length
                c = mic.coords[0]
                xs = [c[0]] + [q[0] for q in part.exterior.coords]
                ys = [c[1]] + [q[1] for q in part.exterior.coords]
                extra = max(zs[j] - top_at(x, y, j - 1 if j else -1) if j else zs[j] - ceiling
                            for x, y in zip(xs, ys))
                area = width * (doc_mm + max(extra, 0.0))
                worst = max(worst, area)
                if area > budget + 1e-9:
                    ok = False
                    break
            if ok:
                chosen = (zs[j], f"Starts at the Z{zs[j]:.2f} floor: the parts outside it give a first-pass chip "
                                 f"area of at most {worst:.2f} mm2 (nominal doc x woc = {budget:.2f}).")
                break
        out[layers[k]] = chosen or (ceiling, "The parts outside the shallower layers are too wide: from where the material starts.")
    return out


# ── TOML project writer ──────────────────────────────────────────────────────


def toml_value(v) -> str:
    if isinstance(v, bool):
        return "true" if v else "false"
    if isinstance(v, int):
        return str(v)
    if isinstance(v, float):
        return fmt(v)
    if isinstance(v, str):
        return json.dumps(v, ensure_ascii=False)
    if isinstance(v, (list, tuple)):
        return "[" + ", ".join(toml_value(x) for x in v) + "]"
    raise TypeError(v)


def table(name: str, kv: dict, comment: str | None = None) -> str:
    out = []
    if comment:
        out += [f"# {line}" for line in comment.splitlines()]
    out.append(f"[{name}]")
    for k, v in kv.items():
        out.append(f"{k} = {toml_value(v)}")
    return "\n".join(out) + "\n"


DRESSUP_BASE = {
    "entry_style": "none",
    "ramp_angle": 3.0,
    "helix_pitch": 1.0,
    "entry_clearance_mm": 0.5,
    "dogbone": False,
    "dogbone_angle": 90.0,
    "lead_in_out": False,
    "lead_radius": 2.0,
    "link_moves": False,
    "link_max_distance": 10.0,
    "link_feed_rate": 500.0,
    "arc_fitting": False,
    "arc_tolerance": 0.05,
    "segment_merge": False,
    "segment_merge_tolerance": 0.3,
    "feed_optimization": False,
    "feed_max_rate": 3000.0,
    "feed_ramp_rate": 200.0,
    "optimize_rapid_order": True,
    "air_bridge_policy": "always",
}


class Project:
    def __init__(self, name: str, header_comment: str) -> None:
        self.name = name
        self.parts: list[str] = [header_comment.rstrip() + "\n\nformat_version = 3\n"]
        self.next_tp = 0
        self.ops: list[dict] = []  # for the report

    def add(self, text: str) -> None:
        self.parts.append(text)

    def setup(self, sid: int, name: str, face_up: str, notes: str) -> None:
        self.add(
            "\n[[setups]]\n"
            f"id = {sid}\n"
            f"name = {toml_value(name)}\n"
            f"face_up = {toml_value(face_up)}\n"
            'z_rotation = "0"\n'
            'xy_datum = "pins"\n'
            'z_datum = "table"\n'
            f"datum_notes = {toml_value(notes)}\n"
            "fixtures = []\n"
            "keep_out_zones = []\n"
        )
        self.cur_setup = name

    def toolpath(self, name: str, kind: str, tool_id: int, model_id: int, params: dict,
                 why: str, heights: dict | None = None, dressups: dict | None = None,
                 boundary: dict | None = None, stock_source: str = "fresh",
                 enabled: bool = True) -> int:
        tid = self.next_tp
        self.next_tp += 1
        out = ["", *(f"# {line}" for line in why.splitlines()), "[[setups.toolpaths]]",
               f"id = {tid}", f"name = {toml_value(name)}", f"type = {toml_value(kind)}",
               f"enabled = {toml_value(enabled)}", f"tool_id = {tool_id}", f"model_id = {model_id}",
               "boundary_inherit = true", f"stock_source = {toml_value(stock_source)}",
               'coolant = "off"', "", "[setups.toolpaths.operation]", f"kind = {toml_value(kind)}", ""]
        out.append(table("setups.toolpaths.operation.params", params))
        d = dict(DRESSUP_BASE)
        d.update(dressups or {})
        out.append(table("setups.toolpaths.dressups", d))
        h = {"clearance_z": None, "retract_z": None, "feed_z": None, "top_z": None, "bottom_z": None}
        h.update(heights or {})
        for k, v in h.items():
            if v is None:
                out.append(table(f"setups.toolpaths.heights.{k}", {"mode": "auto"}))
            else:
                out.append(table(f"setups.toolpaths.heights.{k}", {"mode": "manual", "value": float(v)}))
        if boundary is not None:
            b = dict(boundary)
            src = b.pop("model_outline", None)
            out.append(table("setups.toolpaths.boundary", b))
            if src is not None:
                out.append(table("setups.toolpaths.boundary.source.model_outline",
                                 {"model_id": src, "holes": False}))
        self.add("\n".join(out))
        self.ops.append({"id": tid, "setup": self.cur_setup, "name": name, "kind": kind,
                         "tool_id": tool_id, "model_id": model_id, "why": why,
                         "heights": heights or {}, "params": params})
        return tid

    def text(self) -> str:
        return "\n".join(self.parts)


def machine_tables(feeds: dict) -> str:
    """The Shapeoko XXL with the 1.5 kW VFD and the real $$ (rs_cam source)."""
    kin_axes = [500.0, 500.0, 270.0]
    return (
        "\n# The machine: MachineProfile::shapeoko_vfd() (spindle 6000..24000 rpm, 1.5 kW;\n"
        "# feeds_oak.toml says RPM 6000..24000) with MachineKinematics::shapeoko_xxl_ricky_tuned()\n"
        "# (the $$ capture: $120/$121/$122 = 500/500/270 mm/s2, $110/$111/$112 = 10000/10000/1000\n"
        "# mm/min, $11 = 0.020). Source: crates/rs_cam_core/src/machine/{mod,kinematics}.rs.\n"
        + table("job.machine", {"name": "Shapeoko XXL (1.5 kW VFD, $$ 2026-05-26)",
                                "max_feed_mm_min": 10000.0, "max_shank_mm": 7.0,
                                "aggressiveness": 0.85})
        + table("job.machine.spindle.Variable", {"min_rpm": 6000.0, "max_rpm": 24000.0})
        + table("job.machine.power.VfdConstantTorque", {"rated_power_kw": 1.5, "rated_rpm": 24000.0})
        + table("job.machine.chip_load", {"k0": 0.024, "p": 0.61, "q": 1.26})
        + table("job.machine.rigidity", {"doc_roughing_factor": 0.25, "doc_finishing_factor": 0.1,
                                         "woc_roughing_factor": 0.8, "woc_roughing_max_mm": 6.35,
                                         "woc_finishing_mm": 0.635, "adaptive_doc_factor": 2.0,
                                         "adaptive_woc_factor": 0.25})
        + table("job.machine.kinematics", {"acceleration_mm_s2": sum(kin_axes) / 3.0,
                                           "acceleration_xyz_mm_s2": kin_axes,
                                           "max_rate_xyz_mm_min": [10000.0, 10000.0, 1000.0],
                                           "junction_deviation_mm": 0.02})
    )


def tool_table(tid: int, number: int, name: str, ttype: str, diameter: float, cutting_length: float,
               shank: float, stickout: float, flutes: int, included_angle: float = 0.0,
               taper_half_angle: float = 0.0, corner_radius: float = 0.0, comment: str = "") -> str:
    return "\n" + table("[tools]", {
        "id": tid, "name": name, "type": ttype, "diameter": diameter,
        "cutting_length": cutting_length, "helix_deg": 30.0, "corner_radius_mm": corner_radius,
        "corner_radius": corner_radius, "included_angle": included_angle,
        "taper_half_angle": taper_half_angle, "shaft_diameter": shank, "holder_diameter": 25.0,
        "shank_diameter": shank, "shank_length": 20.0, "stickout": stickout, "flute_count": flutes,
        "tool_number": number, "tool_material": "carbide", "cut_direction": "up_cut",
        "vendor": "", "product_id": ""}, comment).replace("[[[tools]]]", "[[tools]]")


def model_table(mid: int, path: Path) -> str:
    kind = "stl" if path.suffix == ".stl" else "dxf"
    return ("\n[[models]]\n" f"id = {mid}\n" f"path = {toml_value(str(path))}\n"
            f"name = {toml_value(path.name)}\n" f"kind = {toml_value(kind)}\n\n"
            "[models.units]\n" 'kind = "millimeters"\n')


def contract_d6() -> float:
    return tomllib.loads((MONO / "design/rivmap.toml").read_text())["cutters"]["cavity_d"]


def feeds_row(feeds: dict, key: str) -> dict:
    return feeds["tools"][key]


# ═════════════════════════════════════════════════════════════════════════════
# nz-south
# ═════════════════════════════════════════════════════════════════════════════


def derive_nz_south() -> dict:
    T1 = OPERATOR["T1"]
    blk = json.loads((MONO / "build/block/nz-south/block.json").read_text())
    mapt = tomllib.loads((MONO / "block/map.toml").read_text())
    feeds = tomllib.loads((MONO / "block/feeds_oak.toml").read_text())
    tpl = tomllib.loads(TEMPLATE.read_text())
    T = blk["thickness"]
    bx0, by0, bx1, by1 = blk["block"]
    # The back-view DXF frame (block_back.dxf): x = bx1 - X, y = Y - by0.
    def to_back(X, Y):
        return bx1 - X, Y - by0

    out = OUT / "nz_south"
    # Check the frame on every hole and every V vertex of the CSVs.
    with open(MONO / "build/block/nz-south/holes.csv") as f:
        holes = list(csv.DictReader(f))
    with open(MONO / "build/block/nz-south/vgroove_paths.csv") as f:
        vrows = list(csv.DictReader(f))
    for r in holes + vrows:
        x, y = to_back(float(r["x_world"]), float(r["y_world"]))
        assert abs(x - float(r["x_dxf"])) < 1.5e-3 and abs(y - float(r["y_dxf"])) < 1.5e-3, r

    # The V tip offset past the front surface (block.json line_width, vgroove_angle).
    half = math.radians(blk["vgroove_angle"] / 2.0)
    v_offset = (blk["line_width"] / 2.0) / math.tan(half)
    for r in vrows:  # depth - surface_z = the offset, to the CSV rounding
        assert abs(float(r["depth"]) - float(r["surface_z"]) - v_offset) < 1.5e-3, r

    v20 = next(t for t in mapt["tools"] if t["id"] == "v20")
    flat6 = next(t for t in mapt["tools"] if t["id"] == "flat_6")
    flat3 = next(t for t in mapt["tools"] if t["id"] == "flat_3")
    cone = (v20["diameter"] / 2.0) / math.tan(math.radians(v20["angle"] / 2.0))
    usable_cone = cone - mapt["vgroove"]["body_margin"]

    # ── DXF models, one per operation, in the back-view (= setup-1 world) frame
    bc = ezdxf.readfile(MONO / "build/block/nz-south/back_channels.dxf")
    mech = ezdxf.readfile(MONO / "build/mech/nz-south/block_back.dxf")
    files: dict[str, Path] = {}

    def copy_layers(doc, layers: list[str], fname: str, keep_z: bool = False) -> Path:
        d = Dxf()
        n = 0
        for layer in layers:
            for e in layer_entities(doc, layer):
                t = e.dxftype()
                if t == "CIRCLE":
                    d.circle(layer, e.dxf.center.x, e.dxf.center.y, e.dxf.radius)
                elif t in ("LWPOLYLINE", "POLYLINE"):
                    pts, bul, closed, _ = entity_points(e)
                    d.polyline(layer, pts, closed, bul)
                elif t == "LINE":
                    # A 2-point path gives no Trace motion (a ring under 3 points is
                    # skipped), so the line gets its mid point as a third vertex.
                    a, b = (e.dxf.start.x, e.dxf.start.y), (e.dxf.end.x, e.dxf.end.y)
                    mid = ((a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0)
                    d.polyline(layer, [a, mid, b], False)
                else:
                    raise ValueError(f"{layer}: {t}")
                n += 1
        assert n > 0, layers
        p = out / fname
        write_if_changed(p, d.text().encode())
        files[fname] = p
        return p

    bc_layers = sorted({e.dxf.layer for e in bc.modelspace()})
    mech_layers = sorted({e.dxf.layer for e in mech.modelspace()})
    copy_layers(bc, ["BLOCK_OUTLINE"], "outline.dxf")
    copy_layers(bc, ["CAVITY_ROUGH_D8.80"], "cavity_rough_d8.80.dxf")
    copy_layers(bc, ["CAVITY_SKIM_D9.00"], "cavity_skim_d9.00.dxf")
    slot_layers = sorted(l for l in bc_layers if l.startswith("ROUGH_SLOT_Z"))
    hole_layers = sorted(l for l in bc_layers if l.startswith("HOLE_"))
    for l in slot_layers:
        copy_layers(bc, [l], l.lower() + ".dxf")
    for l in hole_layers:
        copy_layers(bc, [l], l.lower() + ".dxf")
    copy_layers(bc, ["VGROOVE_V20_RIVER"], "vgroove_v20_river.dxf")
    copy_layers(bc, ["VGROOVE_V20_LAKE"], "vgroove_v20_lake.dxf")
    # The relief extent (EDGE_OUTER of the terrain export) in the back frame.
    eb = ezdxf.readfile(MONO / "build/terrain/nz-south/machinable_edge_band.dxf")
    d = Dxf()
    for e in layer_entities(eb, "EDGE_OUTER"):
        pts, _, closed, _ = entity_points(e)
        d.polyline("EDGE_OUTER", [to_back(x, y) for x, y in pts], closed)
    write_if_changed(out / "edge_outer.dxf", d.text().encode())
    files["edge_outer.dxf"] = out / "edge_outer.dxf"

    # mech back cuts of the oak block (block_back.dxf)
    mech_pockets = [l for l in mech_layers if re.match(
        r"^(BOARD_POCKET|STRIP_CHANNEL|SLIT_SLOT|PARTS_RELIEF|CONTROL_RELIEF|ESP_RELIEF|ANTENNA|WIRE)_.*D[0-9.]+$", l)]
    # A stable order: shallow first, so a deeper pocket inside a shallow one follows it.
    def dval(l):
        return float(re.search(r"D([0-9.]+)$", l).group(1))
    mech_pockets.sort(key=lambda l: (dval(l), l))
    for l in mech_pockets:
        copy_layers(mech, [l], "mech_" + l.lower() + ".dxf")
    # THROUGH_D25.00: the two circles are control holes; the polyline is the
    # block outline (the setup-3 profile cuts it).
    d = Dxf()
    n_through = 0
    for e in layer_entities(mech, "THROUGH_D25.00"):
        if e.dxftype() == "CIRCLE":
            d.circle("THROUGH_D25.00", e.dxf.center.x, e.dxf.center.y, e.dxf.radius)
            n_through += 1
        else:
            pts, _, closed, _ = entity_points(e)
            xs = [p[0] for p in pts]; ys = [p[1] for p in pts]
            assert closed and (min(xs), max(xs), min(ys), max(ys)) == (0.0, bx1 - bx0, 0.0, by1 - by0)
    write_if_changed(out / "mech_through_d25.00.dxf", d.text().encode())
    files["mech_through_d25.00.dxf"] = out / "mech_through_d25.00.dxf"
    v60_layer = next(l for l in mech_layers if l.startswith("VGROOVE_V60_D"))
    copy_layers(mech, [v60_layer], "mech_" + v60_layer.lower() + ".dxf")
    excluded = [l for l in mech_layers if l.startswith("PILOT_D")] + ["CAVITY_D9.00"]
    used = set(mech_pockets) | {"THROUGH_D25.00", v60_layer} | set(excluded)
    assert used == set(mech_layers), set(mech_layers) - used

    # ── the terrain in the setup-1 frame: (bx1 - X, Y - by0, T1 - Z), a rotation
    tris = read_stl(MONO / "build/terrain/nz-south/terrain.stl")
    t1 = np.empty_like(tris)
    t1[..., 0] = bx1 - tris[..., 0]
    t1[..., 1] = tris[..., 1] - by0
    t1[..., 2] = T1 - tris[..., 2]
    # det of diag(-1, 1, -1) = +1: the winding stays valid, no re-order.
    write_stl(out / "terrain_s1.stl", t1, "rivmap nz-south terrain, rs_cam setup-1 frame (derive.py)")
    files["terrain_s1.stl"] = out / "terrain_s1.stl"
    top_z_min = float(tris[..., 2][tris[..., 2] > 0.5].min())

    # ── stock, margins, pins
    assert flat6["cutter"] == "cavity_d" and flat3["cutter"] == "back_d"
    contract = tomllib.loads((MONO / "design/rivmap.toml").read_text())
    flat6_d = float(contract["cutters"]["cavity_d"])
    flat3_d = float(contract["cutters"]["back_d"])
    tool_d = flat6_d          # the outline profile tool
    pin_d = flat6_d           # the pin drill tool: the hole is its diameter
    m_lo = 2 * PIN_WALL_MM + pin_d + tool_d           # -X: wall, pin, wall, outline cut
    m_hi = m_lo + PIN_KEYING_ASYMMETRY_MM              # +X: 3 mm more, so the pins key
    m_y = tool_d + PIN_WALL_MM                         # +-Y: outline cut + wall
    W = (bx1 - bx0) + m_lo + m_hi
    D = (by1 - by0) + 2 * m_y
    pins = [(PIN_WALL_MM + pin_d / 2, D / 2),
            (W - (PIN_WALL_MM + pin_d / 2 + PIN_KEYING_ASYMMETRY_MM), D / 2)]

    # ── heights in the setup-1 frame (spoilboard Z0)
    ceil = T1 - blk["cavity_rough"]            # the rough cavity ceiling
    skim_floor = T1 - blk["cavity_depth"]
    v_retract = ceil + SAFE_Z_CLEARANCE_MM     # rapids stay inside the convex cavity
    f3 = feeds_row(feeds, "flat_3")
    # No flat_6 row: the flat_6.35 row, with the depth and the width of cut scaled by
    # the diameter ratio (the chip load, the rpm and the feed stay).
    f6 = dict(feeds_row(feeds, "flat_6.35"))
    k6 = float(contract_d6()) / 6.35
    for key in ("doc_rough", "woc_rough", "doc_finish", "woc_finish"):
        f6[key] = round(f6[key] * k6, 4)
    tpl_tools = {t["id"]: t for t in tpl["tools"]}
    tpl_ops = {tp["name"]: tp for s in tpl["setups"] for tp in s["toolpaths"]}
    tv = tpl_ops["Rivers"]["operation"]["params"]           # the template V feeds
    trough = tpl_ops["3D Rough 8"]["operation"]["params"]
    tfin = tpl_ops["Scallop Finish"]["operation"]["params"]
    slot_feed = f3["feed_safe"] / 2.0          # a full-width slot: half (process_sheet.md rule)
    reach6 = flat6["reach"]
    pin_pen = reach6 - T1                       # bore = the stick-out (testpiece rule: 30 = reach)

    P = Project("nz_south", "\n".join([
        "# rs_cam two-sided job: RivMap oak block nz-south.",
        "# GENERATED by planning/rivmap_block_job/derive.py. Do not edit; change the script.",
        f"# World frame = setup-1 frame = the back-view DXF frame (block_back.dxf):",
        f"#   x = {bx1} - X_block, y = Y_block + {-by0}, z = T1 - Z_block, T1 = {T1}.",
        "# Z datum MachineTable on every setup: Z0 = the spoilboard.",
        "# Setup 1 emits world Z (back face at T1). Setup 3 (Bottom) emits local Z = block Z.",
        "# XY zero (both setups): the stock-local (0, 0) corner, which the flip keeps in place;",
        f"#   it is ({-pins[0][0]}, {-pins[0][1]}) from pin 1.",
    ]))
    P.add(table("job", {"name": "rivmap nz-south block (two-sided)"}))
    P.add(table("job.stock", {
        "x": W, "y": D, "z": T1, "origin_x": bx0 - bx0 - m_lo, "origin_y": -m_y, "origin_z": 0.0,
        "padding": 0.0, "auto_from_model": False, "flip_axis": "horizontal"},
        comment=(f"Stock = block {bx1 - bx0} x {by1 - by0} + margins -X {m_lo}, +X {m_hi}, +-Y {m_y}\n"
                 f"(-X: 2 x PIN_WALL {PIN_WALL_MM} + pin {pin_d} + outline tool {tool_d}; +X: + PIN_KEYING {PIN_KEYING_ASYMMETRY_MM};\n"
                 f" Y: outline tool + PIN_WALL). z = T1: the faced back thickness.")))
    P.add(table("job.stock.material.SolidWoodByJanka", {
        "janka_lbf": float(feeds["janka_lbf"]), "label": "Oak (feeds_oak.toml janka_lbf)",
        "source_id": "rivmap feeds_oak.toml"}))
    for x, y in pins:
        P.add(table("[job.stock.alignment_pins]", {"x": x, "y": y, "diameter": pin_d}).replace(
            "[[[job.stock.alignment_pins]]]", "[[job.stock.alignment_pins]]"))
    P.add(table("job.post", {"format": "grblhal", "spindle_speed": int(f6["rpm"]),
                             "safe_z": T1 + SAFE_Z_CLEARANCE_MM, "high_feedrate_mode": False,
                             "high_feedrate": 5000.0, "spindle_strategy": "match_chart"},
                comment="safe_z = T1 + SAFE_Z_CLEARANCE_MM: a MachineTable datum does not shift the post Z (SAFEZ-LOCAL)."))
    P.add(table("job.simulation", {"resolution_mm": mapt["paths"]["grid"]},
                comment="resolution = map.toml [paths].grid (0.25)"))
    P.add(machine_tables(feeds))

    # tools (ids are rs_cam tool ids)
    T_F6, T_F3, T_V20, T_V60, T_R635, T_TBN = 0, 1, 2, 3, 4, 5
    P.add(tool_table(T_F6, 1, flat6["name"], "end_mill", flat6_d, flat6["reach"], flat6["shank"],
                     flat6["reach"], flat6["flutes"], comment="map.toml flat_6"))
    P.add(tool_table(T_F3, 2, flat3["name"], "end_mill", flat3_d, flat3["reach"], flat3["shank"],
                     flat3["reach"], flat3["flutes"], comment="map.toml flat_3"))
    P.add(tool_table(T_V20, 3, v20["name"], "v_bit", v20["diameter"], cone, v20["shank"], v20["reach"],
                     v20["flutes"], included_angle=v20["angle"], taper_half_angle=v20["angle"] / 2,
                     comment=f"map.toml v20: cone = (body/2)/tan(angle/2) = {cone:.4f}"))
    # The day-strip slit bit: mech check.txt "60 deg V-bit, 1/4 in (6.35 mm) diameter".
    v60_d, v60_a = 6.35, 60.0
    v60_cone = (v60_d / 2) / math.tan(math.radians(v60_a / 2))
    P.add(tool_table(T_V60, 4, "1/4 in 60 deg V-bit (day strip slit; mech check.txt)", "v_bit", v60_d,
                     v60_cone, v60_d, 25.0, 2, included_angle=v60_a, taper_half_angle=v60_a / 2,
                     comment="mech check.txt slit line: 60 deg, 6.35 mm. Stick-out: measure (25 assumed, open item)."))
    t0 = tpl_tools[0]
    P.add(tool_table(T_R635, 5, t0["name"] + " 6.35 (front rough; rivmap350 tool 0)", t0["type"],
                     t0["diameter"], t0["cutting_length"], t0["shank_diameter"], t0["stickout"],
                     t0["flute_count"], comment="rivmap350.toml tool 0"))
    t5 = tpl_tools[5]
    P.add(tool_table(T_TBN, 6, t5["name"], t5["type"], t5["diameter"], t5["cutting_length"],
                     t5["shank_diameter"], t5["stickout"], t5["flute_count"],
                     included_angle=t5["included_angle"], taper_half_angle=t5["taper_half_angle"],
                     comment="rivmap350.toml tool 5 (front finish)"))
    P.parts[-1] = P.parts[-1].replace('vendor = ""', f'vendor = {toml_value(t5["vendor"])}').replace(
        'product_id = ""', f'product_id = {toml_value(t5["product_id"])}')

    # models
    mids: dict[str, int] = {}
    order = (["terrain_s1.stl", "outline.dxf", "edge_outer.dxf", "cavity_rough_d8.80.dxf",
              "cavity_skim_d9.00.dxf"] + [l.lower() + ".dxf" for l in slot_layers + hole_layers]
             + ["vgroove_v20_river.dxf", "vgroove_v20_lake.dxf"]
             + ["mech_" + l.lower() + ".dxf" for l in mech_pockets]
             + ["mech_through_d25.00.dxf", "mech_" + v60_layer.lower() + ".dxf"])
    assert sorted(order) == sorted(files), set(files) ^ set(order)
    for i, fname in enumerate(order):
        mids[fname] = i
        P.add(model_table(i, files[fname]))

    # Segment merge joins the short segments of the tessellated arcs, so the machine
    # can reach the feed; the tolerance is map.toml [vgroove].tip_tol.
    merge = {"segment_merge": True, "segment_merge_tolerance": mapt["vgroove"]["tip_tol"]}
    ramp6 = {"entry_style": "ramp", **merge}
    helix = {"entry_style": "helix", **merge}

    # ── Setup 1: Back
    P.setup(0, "1 Back", "top",
            f"Back up. Z0 = spoilboard (MachineTable). XY0 = stock corner = pin 1 - ({pins[0][0]}, {pins[0][1]}).")
    P.toolpath("Face back to T1", "face", T_F6, mids["outline.dxf"], {
        "stepover": f6["woc_rough"], "depth": OPERATOR["raw_top"] - T1,
        "depth_per_pass": f6["doc_rough"], "feed_rate": f6["feed_safe"], "plunge_rate": f6["plunge"],
        "ramp_feed_rate": f6["feed_safe"] / 3.0, "stock_offset": tool_d, "direction": "zigzag",
        "spindle_rpm": int(f6["rpm"])},
        why=(f"Face from the raw top {OPERATOR['raw_top']} to T1 {T1} (the model stock is T1 thick: the sim cuts air).\n"
             "stock_offset = the tool diameter: the face rows start at the rectangle less r and step by the\n"
             "stepover with no row at the far edge, so an offset of r leaves a strip (0.6 mm here) unfaced."),
        heights={"top_z": OPERATOR["raw_top"]}, dressups=ramp6)
    P.toolpath("Pin drill, flip pins", "alignment_pin_drill", T_F6, mids["outline.dxf"], {
        "holes": [], "spoilboard_penetration": pin_pen, "cycle": "peck", "peck_depth": flat6_d,
        "feed_rate": f6["plunge"], "retract_z": 2.0, "spindle_rpm": int(f6["rpm"])},
        why=(f"Two pins on the flip axis y = D/2, in the X margins. Depth = T1 + {pin_pen} = the flat_6\n"
             f"stick-out {reach6} (the testpiece rule: bore = reach). Peck = the tool diameter."))
    P.toolpath("Cavity rough D8.80", "pocket", T_F6, mids["cavity_rough_d8.80.dxf"], {
        "stepover": f6["woc_rough"], "depth": blk["cavity_rough"], "depth_per_pass": f6["doc_rough"],
        "feed_rate": f6["feed_safe"], "plunge_rate": f6["plunge"], "ramp_feed_rate": f6["feed_safe"] / 3.0,
        "climb": True, "pattern": "contour", "angle": 0.0, "finishing_passes": 0,
        "spindle_rpm": int(f6["rpm"])},
        why=f"CAVITY_ROUGH_D8.80: back face {T1} to the ceiling {ceil}.", dressups=ramp6)
    slot_start = slot_starts(bc, slot_layers, blk["cavity_rough"], f3["doc_rough"], f3["woc_rough"],
                             mapt["paths"]["grid"] / 50.0)
    for k, l in enumerate(slot_layers):
        z = float(l.split("_Z")[1])
        z0, start_note = slot_start[l]
        P.toolpath(f"Rough slot Z{z:.2f}", "pocket", T_F3, mids[l.lower() + ".dxf"], {
            "stepover": f3["woc_rough"], "depth": z - z0, "depth_per_pass": f3["doc_rough"],
            "feed_rate": slot_feed, "plunge_rate": f3["plunge"], "ramp_feed_rate": slot_feed / 3.0,
            "climb": True, "pattern": "contour", "angle": 0.0, "finishing_passes": 0,
            "spindle_rpm": int(f3["rpm"])},
            why=(f"{l}: from {T1 - z0:.2f} (block {z0}) to {T1 - z:.2f} (block {z}). {start_note}\n"
                 "Retract inside the convex cavity."),
            heights={"top_z": T1 - z0, "retract_z": v_retract}, dressups=ramp6)
    for l in hole_layers:
        z = float(l.split("_Z")[1])
        P.toolpath(f"Light holes Z{z:.2f} helix", "pocket", T_F3, mids[l.lower() + ".dxf"], {
            "stepover": f3["woc_rough"], "depth": z - blk["cavity_rough"], "depth_per_pass": f3["doc_rough"],
            "feed_rate": slot_feed, "plunge_rate": f3["plunge"], "ramp_feed_rate": slot_feed / 3.0,
            "climb": True, "pattern": "contour", "angle": 0.0, "finishing_passes": 0,
            "spindle_rpm": int(f3["rpm"])},
            why=f"{l}: circle pockets with a helix entry, ceiling {ceil} to {T1 - z:.2f}.",
            heights={"top_z": ceil, "retract_z": v_retract}, dressups=helix)
    pre = -v_offset + usable_cone / 2.0
    for kind, fname in (("river", "vgroove_v20_river.dxf"), ("lake", "vgroove_v20_lake.dxf")):
        for label, depth in (("pre-pass", pre), ("final", -v_offset)):
            P.toolpath(f"V {kind} {label}", "project_curve", T_V20, mids[fname], {
                "depth": depth, "point_spacing": mapt["vgroove"]["tip_tol"],
                "feed_rate": tv["feed_rate"], "plunge_rate": tv["plunge_rate"],
                "surface_model_id": mids["terrain_s1.stl"], "direction": "from_below",
                "side": "center", "spindle_rpm": tv["spindle_rpm"], "chain_distance_mm": 0.0},
                why=(f"Project Curve onto the front surface (it faces down in setup 1: From Below).\n"
                     f"z = surface + depth; final depth = -(w/2)/tan(a/2) = {-v_offset:.4f} (past the front);\n"
                     f"pre-pass = final + usable cone/2 = {pre:.4f} (half the V below the ceiling).\n"
                     f"point_spacing = map.toml [vgroove].tip_tol. Feeds: rivmap350 'Rivers'."),
                heights={"retract_z": v_retract})
    # A mech pocket that lies inside the union of shallower mech pockets of depth >= d
    # starts at d (its material starts there); else at the back face.
    from shapely.ops import unary_union
    mech_geom = {l: unary_union(flat_polys(mech, l, mapt["paths"]["grid"] / 50.0)) for l in mech_pockets}
    def mech_start(l):
        for d in sorted({dval(m) for m in mech_pockets if dval(m) < dval(l)}, reverse=True):
            region = unary_union([mech_geom[m] for m in mech_pockets if d <= dval(m) < dval(l)])
            if region.buffer(1e-6).contains(mech_geom[l]):
                return d
        return 0.0
    for l in mech_pockets:
        dep = dval(l)
        d0 = mech_start(l)
        # The tool: flat_6 where its radius fits in every polygon of the layer, else flat_3.
        polys = [Polygon(entity_points(e)[0]) for e in layer_entities(mech, l)]
        fits6 = all(not p.buffer(-flat6_d / 2.0).is_empty for p in polys)
        assert fits6 or all(not p.buffer(-flat3_d / 2.0).is_empty for p in polys), l
        tid, fr = (T_F6, f6) if fits6 else (T_F3, f3)
        feed = fr["feed_safe"] if fits6 else slot_feed
        P.toolpath(f"Mech {l}", "pocket", tid, mids["mech_" + l.lower() + ".dxf"], {
            "stepover": fr["woc_rough"], "depth": dep - d0, "depth_per_pass": fr["doc_rough"],
            "feed_rate": feed, "plunge_rate": fr["plunge"], "ramp_feed_rate": feed / 3.0,
            "climb": True, "pattern": "contour", "angle": 0.0, "finishing_passes": 0,
            "spindle_rpm": int(fr["rpm"])},
            why=(f"block_back.dxf {l}: {T1 - d0:.2f} to {T1 - dep:.2f}"
                 + (f" (inside the shallower pockets of depth >= {d0}: it starts there)" if d0 else " (from the back face)")
                 + ". Tool: " + ("flat_6." if fits6 else "flat_3 (the 6 mm tool does not fit; slot feed).")),
            heights={"top_z": T1 - d0}, dressups=ramp6)
    P.toolpath("Mech THROUGH holes helix", "pocket", T_F6, mids["mech_through_d25.00.dxf"], {
        "stepover": f6["woc_rough"], "depth": T1, "depth_per_pass": f6["doc_rough"],
        "feed_rate": f6["feed_safe"], "plunge_rate": f6["plunge"], "ramp_feed_rate": f6["feed_safe"] / 3.0,
        "climb": True, "pattern": "contour", "angle": 0.0, "finishing_passes": 0,
        "spindle_rpm": int(f6["rpm"])},
        why=f"THROUGH_D25.00 circles: through the full stock T1 = {T1} to the spoilboard (Z 0).",
        dressups=helix)
    slit_floor = dval(next(l for l in mech_pockets if l.startswith("SLIT_SLOT")))
    v60_depth = dval(v60_layer)
    P.toolpath("Day strip slit V60", "trace", T_V60, mids["mech_" + v60_layer.lower() + ".dxf"], {
        "depth": v60_depth - slit_floor, "depth_per_pass": (v60_depth - slit_floor) / 2.0,
        "feed_rate": tv["feed_rate"], "plunge_rate": tv["plunge_rate"], "compensation": "none",
        "spindle_rpm": tv["spindle_rpm"]},
        why=(f"{v60_layer}: the V tip from the slit slot floor {T1 - slit_floor:.3f} to {T1 - v60_depth:.3f}\n"
             f"({v60_depth - T1:.3f} into the spoilboard). Two passes. Feeds: rivmap350 'Rivers' (open item)."),
        heights={"top_z": T1 - slit_floor})

    # ── Setup 2: Back skim (after the pour and the cure)
    P.setup(1, "2 Back skim", "top", "Back up, on the pins, after the cure. Same zero as setup 1.")
    P.toolpath("Cavity skim D9.00", "pocket", T_F6, mids["cavity_skim_d9.00.dxf"], {
        "stepover": f6["woc_rough"], "depth": blk["cavity_depth"] - blk["cavity_rough"],
        "depth_per_pass": f6["doc_finish"], "feed_rate": f6["feed_safe"], "plunge_rate": f6["plunge"],
        "ramp_feed_rate": f6["feed_safe"] / 3.0, "climb": True, "pattern": "zigzag", "angle": 0.0,
        "finishing_passes": 0, "spindle_rpm": int(f6["rpm"])},
        why=(f"CAVITY_SKIM_D9.00: one pass {ceil} -> {skim_floor}. Generation does not read the stock\n"
             "(a 2.5D pocket from a pinned top); the project simulation runs on the setup-1 result."),
        heights={"top_z": ceil}, dressups=ramp6)

    # ── Setup 3: Front (Bottom flip)
    P.setup(2, "3 Front", "bottom", "Front up, flipped on the pins. Z0 = spoilboard = the back face. Local Z = block Z.")
    P.toolpath("Face front to T", "face", T_F6, mids["outline.dxf"], {
        "stepover": f6["woc_rough"], "depth": T1 - T, "depth_per_pass": f6["doc_rough"],
        "feed_rate": f6["feed_safe"], "plunge_rate": f6["plunge"], "ramp_feed_rate": f6["feed_safe"] / 3.0,
        "stock_offset": tool_d, "direction": "zigzag", "spindle_rpm": int(f6["rpm"])},
        why=f"Face the front from {T1} to the block front face {T}.", dressups=ramp6)
    bnd = {"enabled": True, "containment": "inside", "offset": 0.0, "model_outline": mids["edge_outer.dxf"]}
    P.toolpath("Front 3D rough", "adaptive3d", T_R635, mids["terrain_s1.stl"], {
        "stepover": trough["stepover"], "depth_per_pass": trough["depth_per_pass"],
        "stock_to_leave_axial": tomllib.loads((MONO / "block/testpiece.toml").read_text())["front"]["rough_stock"],
        "feed_rate": feeds_row(feeds, "flat_6.35")["feed"],
        "plunge_rate": feeds_row(feeds, "flat_6.35")["plunge"], "tolerance": trough["tolerance"], "min_cutting_radius": 0.0,
        "entry_style": trough["entry_style"], "ramp_angle_deg": trough["ramp_angle_deg"],
        "helix_radius_factor": trough["helix_radius_factor"], "helix_pitch": trough["helix_pitch"],
        "entry_clearance_mm": trough["entry_clearance_mm"], "detect_flat_areas": False,
        "region_ordering": trough["region_ordering"], "clearing_strategy": trough["clearing_strategy"],
        "trochoid_cap_mult": trough["trochoid_cap_mult"], "engagement_measure": trough["engagement_measure"],
        "z_blend": False, "spindle_rpm": int(f6["rpm"]),
        "min_region_cut_length_mm": trough["min_region_cut_length_mm"],
        "stay_down_clearance_mm": trough["stay_down_clearance_mm"]},
        why=("rivmap350 '3D Rough 8' settings (stepover, depth per pass, strategy); stock to leave 0.5\n"
             "(testpiece.toml [front].rough_stock); feeds: feeds_oak flat_6.35. Boundary: EDGE_OUTER, inside."),
        boundary=bnd, dressups={"segment_merge": True})
    P.toolpath("Front scallop finish", "scallop", T_TBN, mids["terrain_s1.stl"], {
        "scallop_height": tfin["scallop_height"], "tolerance": tfin["tolerance"],
        "direction": tfin["direction"], "continuous": tfin["continuous"], "slope_from": 0.0,
        "slope_to": 90.0, "feed_rate": tfin["feed_rate"], "plunge_rate": tfin["plunge_rate"],
        "stock_to_leave": 0.0, "spindle_rpm": tfin["spindle_rpm"],
        "intra_pass_hookup_mm": tfin["intra_pass_hookup_mm"], "iso_field": tfin["iso_field"]},
        why="rivmap350 'Scallop Finish' settings and feeds (tool 5). Boundary: EDGE_OUTER, inside.",
        boundary=bnd, dressups={"lead_in_out": True})
    n_tabs = 8
    P.toolpath("Outline profile with tabs", "profile", T_F6, mids["outline.dxf"], {
        "side": "outside", "depth": T, "depth_per_pass": f6["doc_rough"], "feed_rate": f6["feed_safe"],
        "plunge_rate": f6["plunge"], "ramp_feed_rate": f6["feed_safe"] / 3.0, "climb": True,
        "tab_count": n_tabs, "tab_width": TAB_WIDTH_DEFAULT, "tab_height": TAB_HEIGHT_DEFAULT,
        "finishing_passes": 0, "compensation": "in_computer", "spindle_rpm": int(f6["rpm"])},
        why=(f"BLOCK_OUTLINE (= mech THROUGH_D25.00 outline): from {T} to the spoilboard 0, outside,\n"
             f"{n_tabs} tabs (two per side; rs_cam default width/height). The pins hold only the margin."),
        heights={"top_z": T}, dressups=ramp6)

    text = P.text()
    write_if_changed(HERE / "nz_south.toml", text.encode())
    return {
        "T1": T1, "T": T, "W": W, "D": D, "m_lo": m_lo, "m_hi": m_hi, "m_y": m_y, "pins": pins,
        "v_offset": v_offset, "cone": cone, "usable_cone": usable_cone, "ceil": ceil,
        "skim_floor": skim_floor, "v_retract": v_retract, "pin_pen": pin_pen,
        "top_z_min": top_z_min, "excluded_mech": excluded, "mech_pockets": mech_pockets,
        "ops": P.ops, "web_floor_block": blk["cavity_depth"] + blk["web_min"],
    }


# ═════════════════════════════════════════════════════════════════════════════
# testpiece
# ═════════════════════════════════════════════════════════════════════════════


def derive_testpiece() -> dict:
    T1 = OPERATOR["T1"]
    tp = tomllib.loads((MONO / "block/testpiece.toml").read_text())
    feeds = tomllib.loads((MONO / "block/feeds_oak.toml").read_text())
    contract = tomllib.loads((MONO / "design/rivmap.toml").read_text())
    T = tp["block"]["thickness"]
    sx, sy = tp["block"]["stock_x"], tp["block"]["stock_y"]
    assert sx == sy, "the 90 degree turn below needs a square stock"
    hx, hy = sx / 2, sy / 2
    out = OUT / "testpiece"
    src = MONO / "build/block/testpiece"
    # The test piece flips about its north-south line X = 0. rs_cam models one
    # flip: about a line parallel to machine X. So the rs_cam world turns the
    # piece by 90 degrees: north to +x. Block frame -> world:
    #   x = Y + hy, y = X + hx, z = T1 - Z     (det +1: a rotation)
    # Back-view DXF (dxf x = -X, dxf y = Y) -> world: x = dxf_y + hy, y = -dxf_x + hx.
    def back_to_world(x, y):
        return y + hy, -x + hx

    def front_to_world(x, y):  # block-frame (setup 4) drawings
        return y + hy, x + hx

    files: dict[str, Path] = {}

    def copy(docpath: Path, layers, fname: str, conv) -> Path:
        doc = ezdxf.readfile(docpath)
        d = Dxf()
        n = 0
        for layer in layers:
            for e in layer_entities(doc, layer):
                if e.dxftype() == "CIRCLE":
                    cx, cy = conv(e.dxf.center.x, e.dxf.center.y)
                    d.circle(layer, cx, cy, e.dxf.radius)
                else:
                    pts, bul, closed, _ = entity_points(e)
                    # Both maps have det -1 in 2D for the front drawings and +1 for
                    # the back drawings; a mirror reverses the bulge sign.
                    a = conv(1.0, 0.0); b = conv(0.0, 1.0); o = conv(0.0, 0.0)
                    det = (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
                    sgn = 1.0 if det > 0 else -1.0
                    d.polyline(layer, [conv(x, y) for x, y in pts], closed, [sgn * v for v in bul])
                n += 1
        assert n > 0, layers
        p = out / fname
        write_if_changed(p, d.text().encode())
        files[fname] = p
        return p

    s1 = ezdxf.readfile(src / "setup1_back_channels.dxf")
    ch_layers = sorted({e.dxf.layer for e in s1.modelspace() if e.dxf.layer.startswith("BACK_CHANNEL_Z")})
    copy(src / "setup1_back_channels.dxf", ["STOCK_OUTLINE"], "stock_outline.dxf", back_to_world)
    copy(src / "setup1_back_channels.dxf", ["DOWEL_THROUGH"], "dowel_through.dxf", back_to_world)
    for l in ch_layers:
        copy(src / "setup1_back_channels.dxf", [l], l.lower() + ".dxf", back_to_world)
    # The channel layers use the slot rule of nz-south (slot_starts): a layer
    # starts at the floor of a shallower layer when the parts outside it are thin.
    copy(src / "setup3_back_cavity.dxf", ["BACK_CAVITY_Z9.00"], "back_cavity_z9.00.dxf", back_to_world)
    copy(src / "setup4_front.dxf", ["FRONT_3D_BOUNDARY"], "front_3d_boundary.dxf", front_to_world)
    # dowel check: the two DXFs agree after the two maps
    s3 = ezdxf.readfile(src / "setup3_back_cavity.dxf")
    s4 = ezdxf.readfile(src / "setup4_front.dxf")
    a = sorted(back_to_world(e.dxf.center.x, e.dxf.center.y) for e in layer_entities(s1, "DOWEL_THROUGH"))
    b = sorted(back_to_world(e.dxf.center.x, e.dxf.center.y) for e in layer_entities(s3, "DOWEL_REF"))
    c = sorted(front_to_world(e.dxf.center.x, e.dxf.center.y) for e in layer_entities(s4, "DOWEL_REF"))
    assert np.allclose(a, b) and np.allclose(a, c), (a, b, c)

    tris = read_stl(src / "setup4_front_surface.stl")
    t1 = np.empty_like(tris)
    t1[..., 0] = tris[..., 1] + hy
    t1[..., 1] = tris[..., 0] + hx
    t1[..., 2] = T1 - tris[..., 2]
    # (x, y, z) -> (y, x, -z): det = +1, a rotation; the winding stays valid.
    write_stl(out / "front_surface_s1.stl", t1, "rivmap testpiece front surface, rs_cam setup-1 frame")
    files["front_surface_s1.stl"] = out / "front_surface_s1.stl"

    tools = {t["id"]: t for t in tp["tools"]}
    f3, f635, fb = (feeds_row(feeds, k) for k in ("flat_3", "flat_6.35", "ball_3.18"))
    back_d = float(contract["cutters"]["back_d"])
    slot_feed = f3["feed_safe"] / 2.0
    pins = [(y + hy, x + hx) for x, y in [(0.0, yy) for yy in tp["dowels"]["y"]]]
    pin_pen = tools[tp["dowels"]["tool"]]["reach"] - T1

    P = Project("testpiece", "\n".join([
        "# rs_cam two-sided job: the RivMap epoxy test piece (block/TESTPIECE.md).",
        "# GENERATED by planning/rivmap_block_job/derive.py. Do not edit; change the script.",
        "# The piece flips about its north-south line; rs_cam flips about a line parallel to",
        f"# machine X. World: x = Y_block + {hy}, y = X_block + {hx}, z = T1 - Z_block, T1 = {T1}.",
        "# So: put the piece on the machine with NORTH TO THE RIGHT (+X), not to the back.",
        "# Z datum MachineTable on every setup (Z0 = spoilboard).",
    ]))
    P.add(table("job", {"name": "rivmap epoxy test piece (two-sided)"}))
    P.add(table("job.stock", {"x": sy, "y": sx, "z": T1, "origin_x": 0.0, "origin_y": 0.0, "origin_z": 0.0,
                              "padding": 0.0, "auto_from_model": False, "flip_axis": "horizontal"},
                comment="testpiece.toml [block] stock_x x stock_y; z = T1 (the method)."))
    P.add(table("job.stock.material.SolidWoodByJanka", {
        "janka_lbf": float(feeds["janka_lbf"]), "label": "Oak (feeds_oak.toml janka_lbf)",
        "source_id": "rivmap feeds_oak.toml"}))
    for x, y in pins:
        P.add(table("[job.stock.alignment_pins]", {"x": x, "y": y, "diameter": tp["dowels"]["diameter"]}).replace(
            "[[[job.stock.alignment_pins]]]", "[[job.stock.alignment_pins]]"))
    P.add(table("job.post", {"format": "grblhal", "spindle_speed": int(f635["rpm"]),
                             "safe_z": T1 + SAFE_Z_CLEARANCE_MM, "high_feedrate_mode": False,
                             "high_feedrate": 5000.0, "spindle_strategy": "match_chart"}))
    P.add(table("job.simulation", {"resolution_mm": tp["paths"]["grid"]},
                comment="resolution = testpiece.toml [paths].grid"))
    P.add(machine_tables(feeds))
    T_F3, T_F635, T_B318 = 0, 1, 2
    for tid, key, ttype in ((T_F3, "flat_3", "end_mill"), (T_F635, "flat_6.35", "end_mill"),
                            (T_B318, "ball_3.18", "ball_nose")):
        t = tools[key]
        dia = back_d if t.get("cutter") == "back_d" else t["diameter"]
        P.add(tool_table(tid, tid + 1, t["name"], ttype, dia, t["reach"], t["shank"], t["reach"],
                         t["flutes"], comment=f"testpiece.toml tool {key}"))
    order = (["front_surface_s1.stl", "stock_outline.dxf", "dowel_through.dxf"]
             + [l.lower() + ".dxf" for l in ch_layers] + ["back_cavity_z9.00.dxf", "front_3d_boundary.dxf"])
    assert sorted(order) == sorted(files)
    mids = {}
    for i, f in enumerate(order):
        mids[f] = i
        P.add(model_table(i, files[f]))
    ramp = {"entry_style": "ramp"}
    # Setup 1 (TESTPIECE setup 1)
    P.setup(0, "1 Back channels", "top", "TESTPIECE setup 1. North to +X. Z0 = spoilboard.")
    P.toolpath("Face back to T1", "face", T_F635, mids["stock_outline.dxf"], {
        "stepover": f635["woc_rough"], "depth": OPERATOR["raw_top"] - T1, "depth_per_pass": f635["doc_rough"],
        "feed_rate": f635["feed_safe"], "plunge_rate": f635["plunge"], "ramp_feed_rate": f635["feed_safe"] / 3.0,
        "stock_offset": tools["flat_6.35"]["diameter"], "direction": "zigzag", "spindle_rpm": int(f635["rpm"])},
        why="TESTPIECE 1.1 STOCK_OUTLINE face; here from the raw top to T1 (the method).",
        heights={"top_z": OPERATOR["raw_top"]}, dressups=ramp)
    P.toolpath("Dowel bores, helix, through", "pocket", T_F3, mids["dowel_through.dxf"], {
        "stepover": f3["woc_rough"], "depth": T1 + pin_pen, "depth_per_pass": f3["doc_rough"],
        "feed_rate": slot_feed, "plunge_rate": f3["plunge"], "ramp_feed_rate": slot_feed / 3.0,
        "climb": True, "pattern": "contour", "angle": 0.0, "finishing_passes": 0,
        "spindle_rpm": int(f3["rpm"])},
        why=(f"TESTPIECE 1.2 DOWEL_THROUGH: helical bore Ø{tp['dowels']['diameter']} with flat_3 to the\n"
             f"tool reach ({T1} + {pin_pen:.1f} into the spoilboard). The native Pin Drill plunges the tool's own\n"
             "diameter, and no 6 mm drill is in testpiece.toml, so a helix pocket cuts the pin holes."),
        dressups={"entry_style": "helix"})
    starts = slot_starts(s1, ch_layers, 0.0, f3["doc_rough"], f3["woc_rough"], tp["paths"]["grid"] / 50.0)
    for l in ch_layers:
        z = float(l.split("_Z")[1])
        z0, note = starts[l]
        top = T1 - z0
        P.toolpath(f"Channel {l}", "pocket", T_F3, mids[l.lower() + ".dxf"], {
            "stepover": f3["woc_rough"], "depth": z - z0, "depth_per_pass": f3["doc_rough"],
            "feed_rate": slot_feed, "plunge_rate": f3["plunge"], "ramp_feed_rate": slot_feed / 3.0,
            "climb": True, "pattern": "contour", "angle": 0.0, "finishing_passes": 0,
            "spindle_rpm": int(f3["rpm"])},
            why=f"TESTPIECE 1.3 {l}: {top:.2f} to {T1 - z:.2f}. {note}",
            heights={"top_z": top}, dressups=ramp)
    # Setup 2 (TESTPIECE setup 3)
    P.setup(1, "2 Back cavity", "top", "TESTPIECE setup 3, after the pour and the cure. Same zero.")
    overfill = tp["paths"]["overfill"]
    P.toolpath("Face back overfill", "face", T_F635, mids["stock_outline.dxf"], {
        "stepover": f635["woc_rough"], "depth": overfill, "depth_per_pass": f635["doc_rough"],
        "feed_rate": f635["feed_safe"], "plunge_rate": f635["plunge"], "ramp_feed_rate": f635["feed_safe"] / 3.0,
        "stock_offset": tools["flat_6.35"]["diameter"], "direction": "zigzag", "spindle_rpm": int(f635["rpm"])},
        why=f"TESTPIECE 3.1: remove the epoxy overfill ({overfill}) down to T1.",
        heights={"top_z": T1 + overfill}, dressups=ramp)
    cav = tp["cavity"]["depth"]
    P.toolpath("Back cavity Z9.00", "pocket", T_F635, mids["back_cavity_z9.00.dxf"], {
        "stepover": f635["woc_rough"], "depth": cav, "depth_per_pass": f635["doc_rough"],
        "feed_rate": f635["feed_safe"], "plunge_rate": f635["plunge"], "ramp_feed_rate": f635["feed_safe"] / 3.0,
        "climb": True, "pattern": "contour", "angle": 0.0, "finishing_passes": 0,
        "spindle_rpm": int(f635["rpm"])},
        why=f"TESTPIECE 3.2 BACK_CAVITY_Z9.00: {T1} to {T1 - cav:.2f}. Fresh stock: the channels are full of epoxy.",
        dressups=ramp)
    # Setup 3 (TESTPIECE setup 4)
    P.setup(2, "3 Front", "bottom", "TESTPIECE setup 4: flipped on the dowels. Local Z = block Z.")
    P.toolpath("Face front to T", "face", T_F635, mids["stock_outline.dxf"], {
        "stepover": f635["woc_rough"], "depth": T1 - T, "depth_per_pass": f635["doc_rough"],
        "feed_rate": f635["feed_safe"], "plunge_rate": f635["plunge"], "ramp_feed_rate": f635["feed_safe"] / 3.0,
        "stock_offset": tools["flat_6.35"]["diameter"], "direction": "zigzag", "spindle_rpm": int(f635["rpm"])},
        why=f"TESTPIECE 4.1 FRONT_FACE_Z25.00: {T1} to {T}.", dressups=ramp)
    P.toolpath("Front 3D rough", "adaptive3d", T_F635, mids["front_surface_s1.stl"], {
        "stepover": f635["woc_rough"], "depth_per_pass": f635["doc_rough"],
        "stock_to_leave_axial": tp["front"]["rough_stock"], "feed_rate": f635["feed_safe"],
        "plunge_rate": f635["plunge"], "tolerance": 0.1, "min_cutting_radius": 0.0, "entry_style": "ramp",
        "ramp_angle_deg": 3.0, "helix_radius_factor": 0.3, "helix_pitch": 2.0, "entry_clearance_mm": 0.5,
        "detect_flat_areas": False, "region_ordering": "global", "clearing_strategy": "contour_parallel",
        "trochoid_cap_mult": 1.6, "engagement_measure": "DiskArea", "z_blend": False,
        "spindle_rpm": int(f635["rpm"]), "min_region_cut_length_mm": 15.0, "stay_down_clearance_mm": 0.5},
        why="TESTPIECE 4.2: 3D rough, leave [front].rough_stock, pass doc_rough.")
    P.toolpath("Front finish parallel N-S", "drop_cutter", T_B318, mids["front_surface_s1.stl"], {
        "stepover": tp["front"]["finish_stepover"], "feed_rate": fb["feed_safe"], "plunge_rate": fb["feed_safe"] / 3.0,
        "min_z": 0.0, "slope_from": 0.0, "slope_to": 90.0, "spindle_rpm": int(fb["rpm"]), "hookup_mm": 0.0},
        why=("TESTPIECE 4.3: parallel finish, stepover [front].finish_stepover. The raster rows run along\n"
             "local X, which is block Y (north-south), as the sheet asks."))
    write_if_changed(HERE / "testpiece.toml", P.text().encode())
    return {"T1": T1, "T": T, "pins": pins, "ops": P.ops, "pin_pen": pin_pen,
            "cavity_floor_block": cav, "web_min": contract.get("cavity", {}).get("web_min")}


def main() -> None:
    assert_rs_cam_constants()
    nz = derive_nz_south()
    tpc = derive_testpiece()
    summary = {"nz_south": {k: v for k, v in nz.items() if k != "ops"},
               "testpiece": {k: v for k, v in tpc.items() if k != "ops"}}
    write_if_changed(OUT / "derive_summary.json", (json.dumps(summary, indent=1, sort_keys=True) + "\n").encode())
    print(json.dumps(summary, indent=1, sort_keys=True))


if __name__ == "__main__":
    main()
