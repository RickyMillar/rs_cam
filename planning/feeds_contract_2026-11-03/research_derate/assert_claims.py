"""Derate audit: one assert per claim id on index.html and REPORT.md.

Reads the repo source (Rust, TOML, JSON, CSV, stored chart text) and the
stored CLI output riv_suggest.jsonl. Exits non-zero if a claim fails.
Run from anywhere:  python3 -I assert_claims.py
"""
import csv
import io
import json
import math
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", "..", ".."))
CORE = os.path.join(ROOT, "crates", "rs_cam_core")
SRC = os.path.join(CORE, "src")
HEAD = "0f55efda4"  # the hash every path:line citation uses


def rd(rel):
    # The tracked file at the cited hash, so a later edit does not move a citation.
    r = subprocess.run(["git", "-C", ROOT, "show", f"{HEAD}:{rel}"], capture_output=True, text=True)
    if r.returncode != 0:  # untracked (e.g. this folder before its commit): the working copy
        return open(os.path.join(ROOT, rel), encoding="utf-8").read()
    return r.stdout


def lines_of(rel, pat):
    """1-based line numbers in `rel` whose text matches regex `pat`."""
    return [i + 1 for i, l in enumerate(rd(rel).split("\n")) if re.search(pat, l)]


def at(rel, line, pat):
    """The citation `rel:line` holds `pat` (a stale citation fails)."""
    L = rd(rel).split("\n")
    assert re.search(pat, L[line - 1]), f"{rel}:{line} does not match {pat!r}: {L[line-1]!r}"


def const_value(rel, name):
    """The value of a `FormulaConstant::unsourced("..", VALUE, ..)` const."""
    s = rd(rel)
    m = re.search(r"const " + name + r": FormulaConstant = FormulaConstant::unsourced\(\s*\"[^\"]+\",\s*([0-9_.]+)", s)
    assert m, name
    return float(m.group(1).replace("_", ""))


def profile_rows():
    """{(family, role): (ap, ae)} from `operation_default_profile`."""
    s = rd("crates/rs_cam_core/src/feeds/mod.rs")
    body = s[s.index("fn operation_default_profile"):s.index("pub fn formula_constants")]
    out = {}
    for m in re.finditer(r"\(OperationFamily::(\w+), PassRole::(\w+)\) => DefaultProfile \{\s*ap_factor: ([0-9.]+),\s*ae_factor: ([0-9.]+)", body):
        out[(m.group(1), m.group(2))] = (float(m.group(3)), float(m.group(4)))
    return out


def registry():
    """[(label, feeds_family, pass_role, geometry rule)] from the registry."""
    s = rd("crates/rs_cam_core/src/compute/catalog/registry.rs")
    out = []
    for m in re.finditer(r"static (REG_\w+): OpRegistryEntry = OpRegistryEntry \{", s):
        c = s[m.start():]
        c = c[:c.index("\n};")]
        g = lambda p: re.search(p, c).group(1)
        out.append((g(r'label:\s*"([^"]+)"'), g(r"feeds_family:\s*FeedsOperationFamily::(\w+)"),
                    g(r"feeds_pass_role:\s*PassRole::(\w+)"), g(r"geometry:\s*crate::compute::geometry_default::(\w+)")))
    return out


def toml_block(rel, header):
    s = rd(rel)
    i = s.index(header)
    blk = s[i + len(header):].split("\n[", 1)[0]
    return {k: float(v) for k, v in re.findall(r"^(\w+)\s*=\s*([0-9.]+)", blk, re.M)}


def rust_struct_block(rel, start_pat):
    s = rd(rel)
    i = re.search(start_pat, s).start()
    blk = s[i:i + 900]
    return {k: float(v) for k, v in re.findall(r"(\w+): ([0-9.]+),", blk)}


def fm1():
    return list(csv.DictReader(io.StringIO(rd("crates/rs_cam_core/tests/fixtures/feeds_matrix_2026-09-23/matrix_2026-09-23.csv"))))


def fm1_cell(tool, d, op, mat):
    return next(r for r in fm1() if r["tool_type"] == tool and r["diameter_mm"] == d
                and r["operation"] == op and r["material"] == mat)


def lut_rows(fname):
    d = json.loads(rd("crates/rs_cam_core/data/vendor_lut/observations/" + fname))
    return d if isinstance(d, list) else d.get("observations", d.get("rows", []))


def riv():
    return [json.loads(l) for l in open(os.path.join(HERE, "riv_suggest.jsonl"), encoding="utf-8")]


def count_lines(paths, pat):
    n = 0
    for p in paths:
        n += len(lines_of(p, pat))
    return n


def rg_files(pat, root="crates"):
    """Tracked *.rs files under `root` at the cited hash whose text matches `pat` (git grep -E)."""
    r = subprocess.run(["git", "-C", ROOT, "grep", "-l", "-E", pat, HEAD, "--", f"{root}/*.rs"],
                       capture_output=True, text=True)
    assert r.returncode in (0, 1), r.stderr
    return [l.split(":", 1)[1] for l in r.stdout.splitlines()]


MOD = "crates/rs_cam_core/src/feeds/mod.rs"
GEO = "crates/rs_cam_core/src/feeds/geometry.rs"
MACH = "crates/rs_cam_core/src/machine/mod.rs"
VAL = "crates/rs_cam_core/src/compute/validate.rs"
PRED = "crates/rs_cam_core/src/feeds/predict.rs"
GD = "crates/rs_cam_core/src/compute/geometry_default.rs"
MAT = "crates/rs_cam_core/src/material/mod.rs"
ENV = "crates/rs_cam_core/src/feeds/envelope.rs"
SRCD = "crates/rs_cam_core/src/feeds/sourced.rs"
PROV = "crates/rs_cam_core/src/feeds/provenance.rs"
LIN = "crates/rs_cam_core/src/trace/lineage.rs"
CHART = "planning/extrapolation_2026-09-24/fetch/G10/sources/carbide3d_s3_feeds_250.txt"
RIVTOML = "planning/fixtures/rivmap100/rivmap100.toml"
WANTOML = "planning/rivmap_block_job/nz_wanaka_21.toml"

CLAIMS = {}


def claim(cid):
    def deco(f):
        CLAIMS[cid] = f
        return f
    return deco


# ---------------------------------------------------------------- A: base

@claim("A00")
def _():  # every citation uses 0f55efda4; rd() reads the tracked files at that hash
    r = subprocess.run(["git", "-C", ROOT, "cat-file", "-e", HEAD + "^{commit}"], capture_output=True)
    assert r.returncode == 0, f"the cited hash {HEAD} is not in this repository"


@claim("A01")
def _():  # the profile table: 21 (family, role) rows with factors = 42 cells, plus a Drill 0/0 arm; source Unsourced Shapeoko calcs.rs
    rows = profile_rows()
    assert len(rows) == 21 and len(rows) * 2 == 42
    at(MOD, 3513, r"^fn operation_default_profile")
    at(MOD, 3503, r"^const OPERATION_DEFAULT_PROFILE_SOURCE")
    assert "shapeoko_feeds_and_speeds calcs.rs (not in the repo)" in rd(MOD).split("\n")[3503]
    assert re.search(r"\(OperationFamily::Drill, _\) => DefaultProfile \{\s*ap_factor: 0\.0,\s*ae_factor: 0\.0", rd(MOD))


@claim("A02")
def _():  # 25 registry rows map to 8 (family, role) pairs; Face family is never reached
    reg = registry()
    assert len(reg) == 25
    pairs = {(f, r) for _, f, r, _ in reg}
    milling = {p for p in pairs if p[0] != "Drill"}
    assert milling == {("Pocket", "Roughing"), ("Contour", "Roughing"), ("Contour", "SemiFinish"), ("Contour", "Finish"),
                       ("Adaptive", "Roughing"), ("Trace", "Finish"), ("Parallel", "Finish"), ("Scallop", "Finish")}, milling
    assert not any(f == "Face" for _, f, _, _ in reg)
    assert dict((l, f) for l, f, _, _ in reg)["Face"] == "Pocket"


@claim("A03")
def _():  # 16 of 42 cells sit on a reached pair; 26 do not (6 Face, 20 on unreached roles)
    rows = profile_rows()
    reached = {(f, r) for _, f, r, _ in registry() if f != "Drill"}
    on = [k for k in rows if k in reached]
    off = [k for k in rows if k not in reached]
    assert 2 * len(on) == 16 and 2 * len(off) == 26
    assert 2 * sum(1 for k in off if k[0] == "Face") == 6


@claim("A04")
def _():  # the live values: Pocket R ae 0.35, Adaptive R 1.50/0.12, Trace F ae 0.02, Parallel F 0.03, Scallop F 0.025, Contour F 0.05
    r = profile_rows()
    assert r[("Pocket", "Roughing")][1] == 0.35
    assert r[("Adaptive", "Roughing")] == (1.50, 0.12)
    assert r[("Trace", "Finish")][1] == 0.02 and r[("Parallel", "Finish")][1] == 0.03
    assert r[("Scallop", "Finish")][1] == 0.025 and r[("Contour", "Finish")][1] == 0.05
    at(MOD, 3516, r"\(OperationFamily::Adaptive, PassRole::Roughing\)")
    at(MOD, 3529, r"\(OperationFamily::Pocket, PassRole::Roughing\)")


@claim("A05")
def _():  # every live ap_factor except Adaptive R is <= 1.0, so doc_derating_scale gives 1.0 on an unstated depth
    r = profile_rows()
    reached = {(f, ro) for _, f, ro, _ in registry() if f != "Drill"}
    big = [k for k in reached if r[k][0] > 1.0]
    assert big == [("Adaptive", "Roughing")], big
    at(GEO, 155, r"^pub fn doc_derating_scale")
    assert "ratio <= 1.0" in rd(GEO).split("\n")[155]


@claim("A06")
def _():  # calc_inputs gives Some(ap)/Some(ae) when the op states them; Radial::Slot states D
    at("crates/rs_cam_core/src/compute/catalog/cut_spec.rs", 246, r"pub const fn calc_inputs")
    assert "Radial::Slot => Some(diameter_mm)" in rd("crates/rs_cam_core/src/compute/catalog/cut_spec.rs")


@claim("A07")
def _():  # 5 of 42 table cells write a shipped number (all ae, through the add-door radial rule); 0 ap cells; Adaptive R ae is replaced on flat/bull/ball/tapered
    s = rd(GD)
    radial_consts = set()
    for m in re.finditer(r"pub\(crate\) const (\w+): &\[GeometryDefault\] =\s*&?\[(.*?)\];", s, re.S):
        if re.search(r"\bradial(_chord)?\(", m.group(2)):
            radial_consts.add(m.group(1))
    assert radial_consts == {"FACE", "POCKET", "ADAPTIVE", "ROUGH_3D", "STEPOVER_ONLY", "FINISH", "STEEP_SHALLOW", "UNIFIED", "PENCIL"}, radial_consts
    pairs = {(f, r) for _, f, r, g in registry() if g in radial_consts}
    writing = pairs - {("Adaptive", "Roughing")}
    assert writing == {("Pocket", "Roughing"), ("Trace", "Finish"), ("Parallel", "Finish"), ("Scallop", "Finish"), ("Contour", "Finish")}, writing
    assert len(writing) == 5
    # no axial rule reads the table: every axial()/axial_chord() names an AXIAL_K constant
    assert "KRow::FamilyProfile(ProfileAxis::Ap)" not in s
    body = rd(MOD)[rd(MOD).index("fn default_engagement"):][:5000]
    assert "ToolGeometryHint::Flat | ToolGeometryHint::Bull { .. } => {\n                    ap_basis = Basis::Rule(ADAPTIVE_FLAT_AP);" in body
    assert "ToolGeometryHint::VBit { .. } => (ap_factor, ae_factor)," in body


@claim("H01")
def _():  # every other path:line the report cites holds its symbol at 0f55efda4
    cites = [
        (MOD, 3954, r"pub fn default_radial_width"), (MOD, 3962, r"fn default_engagement"),
        (MOD, 3479, r"bases: PointBases \{"), (MOD, 1532, r"const DRILL_TIER_SMALL_MAX_D_MM"),
        (MOD, 3872, r"const FINISH_BALL_AE"), (MOD, 3812, r"const ADAPTIVE_MULTI_FLUTE_AE_SCALE"),
        (MOD, 3832, r"const ADAPTIVE_MEDIUM_THRESHOLD"), (MOD, 2332, r"if let Some\(v_rpm\) = vendor_rpm"),
        (MOD, 2398, r"spindle_scale = raw_speedup\.min\(MAX_SPINDLE_SPEEDUP\)"),
        (MOD, 2439, r"let tier_row = milling_rpm_ceiling_row\(d\)"),
        (MOD, 2736, r"let available_power = machine\.power_at_rpm\(rpm\)"),
        (MOD, 3067, r"let machine_cut_ceiling = machine\.cutting_feed_ceiling_mm_min\(\)"),
        (MOD, 3201, r"plunge::resolve_plunge\(&plunge_basis, feed, rpm\)"),
        ("crates/rs_cam_core/src/feeds/suggest/invariants.rs", 206, r"pub\(crate\) fn rigidity_cap_mm"),
        (ENV, 1251, r"kind: LimitKind::Rigidity,"),
        ("crates/rs_cam_core/src/feeds/suggest/adaptive_entry.rs", 228, r"pub\(super\) fn rescale_feed_to_final_geometry"),
        (GD, 472, r"static AXIAL_K_0_5_MM"), (GD, 479, r"static AXIAL_K_1_0_MM"),
        (GD, 485, r"static AXIAL_K_1_5_MM"), (GD, 491, r"static AXIAL_K_2_0_MM"), (GD, 498, r"static AXIAL_K_3_0_MM"),
        (GD, 530, r"pub\(crate\) const FACE:"), (GD, 548, r"pub\(crate\) const FINISH:"),
        (GD, 550, r"pub\(crate\) const STEEP_SHALLOW:"), (GD, 557, r"pub\(crate\) const UNIFIED:"),
        (GD, 564, r"pub\(crate\) const PENCIL:"), (GD, 546, r"pub\(crate\) const STEPOVER_ONLY:"),
        ("crates/rs_cam_core/src/feeds/vendor_lut.rs", 377, r"^const EMBEDDED_FILES"),
        (MAT, 780, r"^const PLUNGE_SOLID_WOOD"), (MAT, 820, r"^const FEED_SCALE_JANKA_REFERENCE_LBF"),
    ]
    for rel, line, pat in cites:
        at(rel, line, pat)


# ------------------------------------------------- B: RigidityProfile facts

@claim("B01")
def _():  # RigidityProfile has 7 fields; 4 have no reader outside the MCP inspect dump
    at(MACH, 61, r"^pub struct RigidityProfile")
    fields = re.findall(r"pub (\w+): f64", rd(MACH)[rd(MACH).index("pub struct RigidityProfile"):][:500])
    assert len(fields) == 7
    dead = []
    for f in fields:
        users = [p for p in rg_files(r"\." + f + r"\b") if "/tests/" not in p and not p.endswith("tests.rs")]
        users = [p for p in users if p != "crates/rs_cam_viz/src/app/mcp/project.rs"]
        if not users:
            dead.append(f)
    assert sorted(dead) == sorted(["doc_finishing_factor", "woc_roughing_factor", "woc_roughing_max_mm", "woc_finishing_mm"]), dead


@claim("B02")
def _():  # RigidityProfile::default 0.25/2.0/0.25; generic_wood_router 0.20/1.5/0.20; MachineProfile::default = generic
    at(MACH, 166, r"^impl Default for RigidityProfile")
    d = rust_struct_block(MACH, r"impl Default for RigidityProfile")
    assert (d["doc_roughing_factor"], d["adaptive_doc_factor"], d["adaptive_woc_factor"]) == (0.25, 2.0, 0.25)
    at(MACH, 675, r"pub fn generic_wood_router")
    g = rust_struct_block(MACH, r"rigidity: RigidityProfile \{\s*doc_roughing_factor: 0\.20")
    assert (g["doc_roughing_factor"], g["adaptive_doc_factor"], g["adaptive_woc_factor"], g["woc_roughing_factor"]) == (0.20, 1.5, 0.20, 0.70)
    at(MACH, 579, r"^impl Default for MachineProfile")
    assert "Self::generic_wood_router()" in rd(MACH).split("\n")[580]


@claim("B03")
def _():  # corrected by RIGIDITY_DEFAULTS.md: `rigidity` has NO serde default; a machine table without it fails to load
    at(MACH, 257, r"pub rigidity: RigidityProfile")
    assert "#[serde(default)]" not in rd(MACH).split("\n")[255]
    at(MACH, 254, r"#\[serde\(default\)\]")
    at(MACH, 255, r"pub max_cutting_feed_mm_min")


@claim("B04")
def _():  # both Shapeoko presets use RigidityProfile::default
    s = rd(MACH)
    for fn in ("shapeoko_vfd", "shapeoko_makita"):
        body = s[s.index("pub fn " + fn):][:1400]
        assert "rigidity: RigidityProfile::default()" in body, fn


@claim("B05")
def _():  # rivmap100 rigidity 0.2/1.5/0.2 (woc 0.7); nz-wanaka-21 0.25/2.0/0.2 (woc 0.4)
    r = toml_block(RIVTOML, "[job.machine.rigidity]")
    assert (r["doc_roughing_factor"], r["adaptive_doc_factor"], r["adaptive_woc_factor"], r["woc_roughing_factor"]) == (0.2, 1.5, 0.2, 0.7)
    w = toml_block(WANTOML, "[job.machine.rigidity]")
    assert (w["doc_roughing_factor"], w["adaptive_doc_factor"], w["adaptive_woc_factor"], w["woc_roughing_factor"]) == (0.25, 2.0, 0.2, 0.4)


@claim("B06")
def _():  # the operator's Scylla machine file (outside the repo) states woc_roughing_factor 0.4 "40-60 % of D"
    p = os.path.expanduser("~/.config/rs_cam/machines/shapeoko_pro_xxl_scylla.toml")
    if not os.path.exists(p):  # machine-local evidence; B05 holds the same 0.4 in the repo
        print("SKIP B06: the operator machine file is not on this machine")
        return
    s = open(p, encoding="utf-8").read()
    assert re.search(r"^woc_roughing_factor = 0\.4$", s, re.M)
    assert "40-60 % of D" in s
    assert re.search(r"^adaptive_woc_factor = 0\.2$", s, re.M) and re.search(r"^doc_roughing_factor = 0\.25$", s, re.M)


@claim("B07")
def _():  # depth_cap_mm: Adaptive -> adaptive_doc_factor, other roughing -> doc_roughing_factor, finish -> none
    at(MACH, 143, r"pub fn depth_cap_mm")
    s = rd(MACH)
    assert "(OperationFamily::Adaptive, _) => self.adaptive_doc_factor" in s
    assert "(_, PassRole::Roughing) => self.doc_roughing_factor" in s
    assert "(_, PassRole::SemiFinish | PassRole::Finish) => return None" in s


@claim("B08")
def _():  # the machine module header says it is a port of the Shapeoko machine_profile.rs
    assert "Ported from reference/shapeoko_feeds_and_speeds/src/machine_profile.rs" in rd(MACH)


# ------------------------------------------- C: adaptive width and depth

@claim("C01")
def _():  # ADAPTIVE_FLAT_AE 0.14 (Unsourced, Shapeoko adaptive matrix), ADAPTIVE_FLAT_AP 1.20
    at(MOD, 3777, r"^const ADAPTIVE_FLAT_AE")
    at(MOD, 3772, r"^const ADAPTIVE_FLAT_AP")
    assert const_value(MOD, "ADAPTIVE_FLAT_AE") == 0.14 and const_value(MOD, "ADAPTIVE_FLAT_AP") == 1.20
    assert 'adaptive matrix of reference/shapeoko_feeds_and_speeds calcs.rs (not in the repo)' in rd(MOD)


@claim("C02")
def _():  # wood + flat/bull: ae = max(x, adaptive_woc).min(adaptive_woc) = adaptive_woc exactly
    at(MOD, 4104, r"ae_factor = ae_factor\.max\(machine\.rigidity\.adaptive_woc_factor\)")
    at(MOD, 4106, r"ae_factor = ae_factor\.min\(machine\.rigidity\.adaptive_woc_factor\)")
    for x in (0.05, 0.133, 0.14, 0.3):
        for w in (0.20, 0.25):
            assert min(max(x, w), w) == w


@claim("C03")
def _():  # White Oak: Janka 1360 -> feed scale 1.387 -> medium band (x0.95 ae) -> 0.14*0.95 = 0.133 < 0.20: the machine RAISES the Shapeoko width
    at(MAT, 153, r"WoodSpecies::WhiteOak => 1360\.0")
    j_ref = const_value(MAT, "FEED_SCALE_JANKA_REFERENCE_LBF")
    ex = const_value(MAT, "FEED_SCALE_EXPONENT")
    fs = (1360.0 / j_ref) ** ex
    assert abs(fs - 1.387) < 0.001
    assert const_value(MOD, "ADAPTIVE_MEDIUM_THRESHOLD") < fs < const_value(MOD, "ADAPTIVE_HARD_THRESHOLD")
    ae = const_value(MOD, "ADAPTIVE_FLAT_AE") * const_value(MOD, "ADAPTIVE_MEDIUM_AE_SCALE")
    assert abs(ae - 0.133) < 1e-9 and ae < 0.20


@claim("C04")
def _():  # rivmap100 3D Rough (6 mm flat, white oak, adaptive_woc 0.2) ships stepover 1.2 = 0.20 x 6
    r = riv()[1]
    assert r["toolpath_name"] == "3D Rough" and abs(r["suggested"]["stepover"] - 1.2) < 1e-9
    assert r["tool"]["diameter"] == 6.0
    assert 'species = "WhiteOak"' in rd(RIVTOML)


@claim("C05")
def _():  # FM1: 6 mm 2F Adaptive stepover 1.2 (0.20 D) and Pocket stepover 2.1 (0.35 D)
    assert fm1_cell("EndMill", "6.0000", "Adaptive", "hardwood")["stepover_mm"] == "1.2000"
    assert fm1_cell("EndMill", "6.0000", "Pocket", "hardwood")["stepover_mm"] == "2.1000"


@claim("C06")
def _():  # the add-door stamp of a family-profile stepover names the table row, not the machine field
    at(GD, 296, r"let stamp = ValueProvenance::of_basis\(&Basis::Rule\(row\.clone\(\)\)\);")
    at(GD, 202, r"pub fn row\(self, op_type")
    s = rd(GD)
    assert "crate::feeds::default_profile_row(" in s[s.index("pub fn row(self, op_type"):][:600]


@claim("C07")
def _():  # the one product of machine x table: adaptive_doc_factor * profile.ap_factor / 1.50; generic 1.5, default 2.0
    at(MOD, 4078, r"let rigidity_ap_floor = machine\.rigidity\.adaptive_doc_factor \* profile\.ap_factor")
    div = const_value(MOD, "ADAPTIVE_RIGIDITY_DOC_DIVISOR")
    ap_row = profile_rows()[("Adaptive", "Roughing")][0]
    assert div == 1.50 and ap_row == 1.50
    assert 1.5 * ap_row / div == 1.5 and 2.0 * ap_row / div == 2.0
    assert const_value(MOD, "ADAPTIVE_FLAT_AP") * const_value(MOD, "ADAPTIVE_MEDIUM_AP_SCALE") == 1.08


@claim("C08")
def _():  # doc_derating_scale at 1.5 D = 0.875 and at 2.0 D = 0.75 (printed 2xD -25 %)
    def s(r):
        if r <= 1.0:
            return 1.0
        if r <= 2.0:
            return 1.0 - 0.25 * (r - 1.0)
        if r <= 3.0:
            return 0.75 - 0.25 * (r - 2.0)
        return 0.5
    body = rd(GEO)[rd(GEO).index("pub fn doc_derating_scale"):][:400]
    assert "1.0 - 0.25 * (ratio - 1.0)" in body and "0.75 - 0.25 * (ratio - 2.0)" in body
    assert s(1.5) == 0.875 and s(2.0) == 0.75


@claim("C09")
def _():  # the add door states the adaptive depth: 2D Adaptive k = 2/6, 3D Rough k = 3/6 (Unsourced AXIAL_K rows)
    s = rd(GD)
    assert 'pub(crate) const ADAPTIVE: &[GeometryDefault] =\n    &[radial("stepover"), axial("depth_per_pass", &AXIAL_K_2_0_MM)];' in s
    assert 'pub(crate) const ROUGH_3D: &[GeometryDefault] =\n    &[radial("stepover"), axial("depth_per_pass", &AXIAL_K_3_0_MM)];' in s
    assert lines_of(GD, r"static AXIAL_K_") == [472, 479, 485, 491, 498]
    assert s.count("FormulaConstant::unsourced(\n    \"compute.geometry_default.AXIAL_K_") == 5


# ---------------------------------------------- D: depth stack, slot rule

@claim("D01")
def _():  # SLOTTING_THRESHOLD 0.85 D, SLOTTING_DOC_CAP 0.25 D, both Unsourced, machine-blind
    at(MOD, 1889, r"^const SLOTTING_THRESHOLD")
    at(MOD, 1895, r"^const SLOTTING_DOC_CAP")
    assert const_value(MOD, "SLOTTING_THRESHOLD") == 0.85 and const_value(MOD, "SLOTTING_DOC_CAP") == 0.25
    at(MOD, 2567, r"if ae > d \* SLOTTING_THRESHOLD\.value")
    at(ENV, 1262, r"kind: LimitKind::Slot,")
    body = rd(MOD)[rd(MOD).index("pub(crate) fn slot_rule"):][:500]
    assert "machine" not in body


@claim("D02")
def _():  # Profile is a slot op (Radial::Slot, ae = D) with feeds family Contour Roughing
    reg = {l: (f, r, g) for l, f, r, g in registry()}
    assert reg["Profile"][:2] == ("Contour", "Roughing")
    assert "A `Radial::Slot` op (Profile, Trace, Project Curve, Bore) states its width D" in rd("crates/rs_cam_core/src/feeds/CLAUDE.md")


@claim("D03")
def _():  # Profile 6 mm: rigidity cap generic 1.2 mm, Shapeoko/Scylla 1.5 mm; slot cap 1.5 mm; machine with doc 0.5 is still held at 1.5 mm
    d = 6.0
    slot = d * const_value(MOD, "SLOTTING_DOC_CAP")
    assert abs(d * 0.20 - 1.2) < 1e-12 and abs(slot - 1.5) < 1e-12 and abs(d * 0.25 - 1.5) < 1e-12
    assert min(d * 0.5, slot) == slot


@claim("D04")
def _():  # vendors print 1 x D (Amana Spektra, Onsrud hardwood stored text)
    assert "Depth of Cut: 1 x Tool Diameter" in rd("crates/rs_cam_core/data/vendor_lut/sources/amana_spektra_spiral_plunge_v24.txt")
    assert "DEPTH OF CUT: 1 x D Use recommended chip load" in rd("crates/rs_cam_core/data/vendor_lut/sources/onsrud_hard_wood_cutting_data.txt")


@claim("D05")
def _():  # Carbide 3D Shapeoko chart (.25 in tool): Mahogany DOC .1" = 0.4 D, Pine .4" = 1.6 D, Plywood .25" = 1.0 D, MDF .3" = 1.2 D
    s = rd(CHART)
    assert "S3_feeds_250.pdf" in s
    want = {"Mahogany": ".1", "Pine": ".4", "Plywood": ".25", "MDF": ".3"}
    for k, v in want.items():
        m = re.search(r"^" + k + r"\s+(\.[0-9]+)″", s, re.M)
        assert m and m.group(1) == v, k
    assert abs(0.1 / 0.25 - 0.4) < 1e-12 and abs(0.4 / 0.25 - 1.6) < 1e-12 and abs(0.3 / 0.25 - 1.2) < 1e-12


@claim("D06")
def _():  # hardwood depth stack, 6 mm Pocket: vendor 1.0 D, Shapeoko chart 0.4 D, add door 0.25 D, rigidity 0.20 D (generic): 0.20 of vendor, 0.5 of the Shapeoko chart
    k = 1.5 / 6.0
    assert re.search(r"static AXIAL_K_1_5_MM: FormulaConstant = FormulaConstant::unsourced\(\s*\"[^\"]+\",\s*1\.5 / AXIAL_REFERENCE_D_MM", rd(GD))
    assert 'pub(crate) const POCKET: &[GeometryDefault] =\n    &[radial("stepover"), axial("depth_per_pass", &AXIAL_K_1_5_MM)];' in rd(GD)
    assert k == 0.25
    assert abs(0.20 / 1.0 - 0.20) < 1e-12 and abs(0.20 / 0.4 - 0.5) < 1e-12 and abs(0.25 / 0.4 - 0.625) < 1e-12
    assert fm1_cell("EndMill", "6.0000", "Pocket", "hardwood")["depth_per_pass_mm"] == "1.5000"


@claim("D07")
def _():  # FLUTE_GUARD_FACTOR 0.8 x flute (Unsourced); DEFLECTION target 200 um (Unsourced)
    at(MOD, 1838, r"^const FLUTE_GUARD_FACTOR")
    assert const_value(MOD, "FLUTE_GUARD_FACTOR") == 0.8
    at("crates/rs_cam_core/src/feeds/suggest/invariants.rs", 53, r"DEFLECTION_BACKOFF_TARGET_UM: f64 = 200\.0")


# --------------------------------------------- E: feed and rpm chain

@claim("E01")
def _():  # feed = rpm x chip x Z x depth_tier; the long-tool share and chip thinning are not in it
    at(MOD, 2679, r"let mut raw_feed = rpm \* chip_load \* input\.flute_count as f64 \* depth_tier;")
    at(MOD, 2677, r"let depth_tier = geometry::depth_tier_multiplier")
    lf = lines_of(MOD, r"\bld_factor\b")
    assert lf == [2693, 3441], lf  # defined, then only reported in FeedsDerates
    ct = lines_of(MOD, r"\bchip_thinning\b")
    assert ct == [2658, 3439], ct


@claim("E02")
def _():  # rubbing floor 0.025 mm warns only
    at(MOD, 1393, r"pub const RUBBING_FLOOR_MM_TOOTH: f64 = 0\.025;")


@claim("E03")
def _():  # the formula chip k0 D^p (1/H)^q; ChipLoadFormula default 0.024/0.61/1.26 "Shapeoko empirical"; every preset has the same three numbers
    at(MOD, 2106, r"let formula_chipload = cl\.k0 \* effective_d\.powf\(cl\.p\) \* \(1\.0 / feed_scale\)\.powf\(cl\.q\);")
    at(MACH, 47, r"^impl Default for ChipLoadFormula")
    s = rd(MACH)
    assert "Soft wood baseline from Shapeoko empirical data" in s
    assert len(re.findall(r"k0: 0\.024,\s*p: 0\.61,\s*q: 1\.26", s)) == 3  # default, shapeoko_vfd, shapeoko_makita
    assert s.count("chip_load: ChipLoadFormula::default()") == 1  # generic


@claim("E04")
def _():  # SFM rpm 200 m/min solid wood (Unsourced Shapeoko port) -> 10610 rpm at 6 mm
    at(MAT, 735, r"^const SPEED_SOLID_WOOD")
    assert const_value(MAT, "SPEED_SOLID_WOOD") == 200.0
    assert "ported from reference/shapeoko_feeds_and_speeds (not in the repo)" in rd(MAT)
    assert round(200.0 * 1000.0 / (math.pi * 6.0)) == 10610
    at(MOD, 2041, r"let \(ideal_rpm, sfm_basis\)")


@claim("E05")
def _():  # FM1: 524 ok cells; 464 vendor chip, 60 formula (Shapeoko) chip
    R = fm1()
    ok = [r for r in R if r["status"] == "ok"]
    assert len(R) == 1000 and len(ok) == 524
    assert sum(r["chipload_source"].startswith("VendorLut") for r in ok) == 464
    assert sum(r["chipload_source"].startswith("FormulaFallback") for r in ok) == 60


@claim("E06")
def _():  # 6 mm 2F hardwood: Trace (formula) chip 0.0459 rpm 10610 feed 973; Pocket (vendor) chip 0.1270 feed 3999
    t = fm1_cell("EndMill", "6.0000", "Trace", "hardwood")
    p = fm1_cell("EndMill", "6.0000", "Pocket", "hardwood")
    assert t["chipload_source"].startswith("FormulaFallback") and p["chipload_source"].startswith("VendorLut")
    assert (t["r_chip_load_mm"], t["rpm"], t["feed_mm_min"]) == ("0.0459", "10610.0000", "973.0000")
    assert (p["r_chip_load_mm"], p["feed_mm_min"]) == ("0.1270", "3999.0000")
    assert p["lut_observation_id"] == "amana-flat-hardwood-pocket-6000-2f-spektra"


@claim("E07")
def _():  # the formula chip reproduces FM1 hardwood (Hard Maple, Janka 1450): 0.024 x 6^0.61 x (1/1.423)^1.26 = 0.0459
    at(MAT, 150, r"WoodSpecies::HardMaple => 1450\.0")
    h = (1450.0 / const_value(MAT, "FEED_SCALE_JANKA_REFERENCE_LBF")) ** const_value(MAT, "FEED_SCALE_EXPONENT")
    chip = 0.024 * 6.0 ** 0.61 * (1.0 / h) ** 1.26
    assert abs(chip - 0.0459) < 0.00006, chip


@claim("E08")
def _():  # the vendor row prints 0.127 mm at 18000 rpm; the formula chip is 0.361 of it; formula feed is 0.213 of the row feed
    row = next(r for r in lut_rows("amana_flat_end.json") if r["observation_id"] == "amana-flat-hardwood-pocket-6000-2f-spektra")
    assert row["chipload_max_mm_tooth"] == 0.127 and row["rpm_nominal"] == 18000.0 and row["flute_count"] == 2
    ratio_chip = 0.0459 / 0.127
    row_feed = 0.127 * 2 * 18000.0
    ratio_feed = 973.0 / row_feed
    assert abs(ratio_chip - 0.361) < 0.0005 and abs(row_feed - 4572.0) < 1e-9 and abs(ratio_feed - 0.213) < 0.0005


@claim("E09")
def _():  # rivmap100 Face 5 ships the printed row rpm 18000 and its chip 0.1311 (G2 scaled from 0.127)
    r = riv()[0]
    c = r["profile"]["calculator"]
    assert r["toolpath_name"] == "Face 5" and c["rpm"] == 18000.0
    assert c["vendor_source"] == "amana-flat-hardwood-pocket-6000-2f-spektra" and abs(c["chip_load_mm"] - 0.1311) < 0.0001
    assert r["profile"]["point_source"] == {"ae": "operation", "ap": "operation", "rpm": "operation"}


@claim("E10")
def _():  # milling RPM tiers (22k/20k/18k/16k/14k) and MAX_SPINDLE_SPEEDUP 1.5: Unsourced
    s = rd(MOD)
    for v in ("22_000.0", "20_000.0", "18_000.0", "16_000.0", "14_000.0"):
        assert v in s
    at(MOD, 374, r"pub const MAX_SPINDLE_SPEEDUP: f64 = 1\.5;")
    assert "no file in the repo prints them" in s


# -------------------------------------------- F: typed duplicates, LUT

@claim("F01")
def _():  # validate.rs types 0.15 D and 0.20 D (machine-blind)
    at(VAL, 259, r"^fn check_wood_adaptive_stepover")
    at(VAL, 277, r"let floor = 0\.15 \* d;")
    at(VAL, 284, r"let new_value = 0\.20 \* d;")


@claim("F02")
def _():  # predict.rs types 0.35 (non-adaptive) and 0.3 (adaptive move count); 0.35 equals the Pocket R table ae
    at(PRED, 260, r"const PREDICTOR_NON_ADAPTIVE_WOC_FRACTION: f64 = 0\.35;")
    at(PRED, 267, r"const PREDICTOR_ADAPTIVE_WOC_FRACTION: f64 = 0\.3;")
    at(PRED, 374, r"machine\.rigidity\.adaptive_woc_factor\.max\(0\.0\) \* diameter_mm")
    assert profile_rows()[("Pocket", "Roughing")][1] == 0.35


@claim("F03")
def _():  # the LUT keeps industrial rows out for a Shapeoko-class router (a machine margin in the data layer)
    s = rd("crates/rs_cam_core/data/vendor_lut/industrial_only/README.md")
    assert "Shapeoko-class router" in s and "0.46–0.69 mm/tooth" in s
    assert "freud_solid_carbide_industrial.json" not in rd("crates/rs_cam_core/src/feeds/vendor_lut.rs").split("const EMBEDDED_FILES")[1].split("];")[0]


@claim("F04")
def _():  # IDC Woodcraft (hobby chart) and Whiteside Fusion 360 rows are embedded; 0 FM1 cells ship from them
    emb = rd("crates/rs_cam_core/src/feeds/vendor_lut.rs").split("const EMBEDDED_FILES")[1].split("];")[0]
    assert '"idcwoodcraft_millmage.json"' in emb and '"whiteside_fusion360.json"' in emb
    assert len(lut_rows("idcwoodcraft_millmage.json")) == 10 and len(lut_rows("whiteside_fusion360.json")) == 13
    R = fm1()
    assert not any("idcwoodcraft" in r["lut_observation_id"] or "fusion360" in r["lut_observation_id"] for r in R)


@claim("F05")
def _():  # the amana adaptive ae bands (8-20 % D) are repo-authored, not printed
    rows = [r for r in lut_rows("amana_flat_end.json") if r.get("ae_rule") == "8% to 20%D"]
    assert len(rows) == 2 and all("repo-authored" in (r.get("notes") or "") for r in rows)


# --------------------------------------------------------- G: lineage

@claim("G01")
def _():  # Basis::Derived holds law + from only (no value in, factor, value out)
    at(SRCD, 102, r"^pub enum Basis")
    at(SRCD, 121, r"^    Derived \{")
    blk = rd(SRCD).split("\n")[120:127]
    fields = [l.strip() for l in blk if re.match(r"\s+\w+:", l)]
    assert fields == ["law: &'static str,", "from: Vec<Basis>,"], fields


@claim("G02")
def _():  # MachineField::Rigidity names "machine.rigidity" only: no field, no value
    at(SRCD, 68, r"^pub enum MachineField")
    assert 'Self::Rigidity => "machine.rigidity",' in rd(SRCD)
    assert re.search(r"Self::Machine\(field\) => map\.serialize_entry\(\"field\", field\.name\(\)\)\?", rd(SRCD))


@claim("G03")
def _():  # moved_basis drops before/after: it keeps only law + [b, cap]
    at(MOD, 1727, r"^fn moved_basis")
    body = "\n".join(rd(MOD).split("\n")[1726:1734])
    assert "Basis::derived(law, vec![b, cap])" in body


@claim("G04")
def _():  # the stored stamp is a projection to one (source, reference): ValueProvenance holds 2 fields
    at(PROV, 152, r"pub fn of_basis")
    blk = rd(PROV)[rd(PROV).index("pub struct ValueProvenance"):][:200]
    assert re.findall(r"pub (\w+):", blk) == ["source", "reference"]


@claim("G05")
def _():  # Lineage::of reads stored stamps and runs no calculator
    at(LIN, 63, r"^pub struct Lineage \{")
    at(LIN, 336, r"pub fn of\(inputs")
    assert "it runs no calculator." in rd(LIN).split("\n")[333]


@claim("G06")
def _():  # the envelope speeds grid serializes rpm, feed, plunge, chip as value only (4 fields strip the basis)
    at(ENV, 373, r"^fn value_only")
    assert lines_of(ENV, r'serialize_with = "value_only"') == [350, 353, 356, 359]


@claim("G07")
def _():  # rivmap100 CLI output: the only basis tree is the proposal, depth 1, two operator leaves; speed points carry no basis
    for r in riv():
        env = r["profile"]["envelope"]
        b = env["proposal"]["basis"]
        assert b["kind"] == "derived" and b["law"] == "proposal: min(stated, binding cap)"
        # depth 1: each child is an operator leaf or an empty law ("fixed by the cut")
        assert all(x["kind"] == "operator" or (x["kind"] == "derived" and x["from"] == []) for x in b["from"])
        assert "value" not in json.dumps(b)  # no node carries a number
        assert all("basis" not in p for p in env["speeds"]["points"])
    assert "bases" not in json.dumps(riv()[0]["profile"]["calculator"])


@claim("G08")
def _():  # the formula chip basis names k0, p, q but not the feed scale (Janka law)
    body = rd(MOD)[rd(MOD).index("let formula_chip_basis"):][:1200]
    assert "machine.ChipLoadFormula.default.k0" in body and "feed_scale" not in body.split("Basis::derived")[1][:400]


@claim("G09")
def _():  # the feed basis law names the depth tier but holds no record of its value; pass 9 rescale writes a warning, no basis
    at(MOD, 2680, r"let feed_law = \|")
    assert '"chip x RPM x Z x depth tier"' in rd(MOD)
    ae = rd("crates/rs_cam_core/src/feeds/suggest/adaptive_entry.rs")
    body = ae[ae.index("pub(super) fn rescale_feed_to_final_geometry"):][:6000]
    assert "Basis" not in body and "FeedRescaledToFinalGeometry" in body


@claim("G10")
def _():  # size: 69 lines match Basis::derived( or moved_basis( in 6 core files
    files = rg_files(r"Basis::derived\(|moved_basis\(", "crates/rs_cam_core/src")
    n = count_lines(files, r"Basis::derived\(|moved_basis\(")
    assert len(files) == 6 and n == 69, (files, n)


@claim("G11")
def _():  # size: ValueProvenance in 18 files; Basis:: in 63 files; MachineField::Rigidity 4 lines
    vp = rg_files(r"ValueProvenance")
    assert len(vp) == 18, len(vp)
    assert len(rg_files(r"Basis::")) == 63
    n = sum(len(lines_of(p, r"MachineField::Rigidity")) for p in rg_files(r"MachineField::Rigidity"))
    assert n == 4, n


@claim("G12")
def _():  # MCP get_toolpath_lineage and get_suggest_rationale exist; CLI suggest prints no bases
    mcp = rd("crates/rs_cam_viz/src/mcp_server.rs")
    assert 'name = "get_toolpath_lineage"' in mcp and 'name = "get_suggest_rationale"' in mcp
    assert ".bases" not in rd("crates/rs_cam_cli/src/suggest.rs")


@claim("G13")
def _():  # Basis derives Serialize only (no Deserialize): persisting a tree needs a read path
    s = rd(SRCD)
    i = s.index("pub enum Basis")
    head = s[s.rfind("#[derive", 0, i):i]
    assert "Deserialize" not in head and "impl Serialize for Basis" in s


# ------------------------------------------------------------ run

def page_ids():
    html = open(os.path.join(HERE, "index.html"), encoding="utf-8").read()
    return set(re.findall(r'data-claim="([A-Z][0-9]{2})"', html))


def report_ids():
    p = os.path.join(HERE, "REPORT.md")  # the lead writes it from the agent text
    if not os.path.exists(p):
        return set()
    md = open(p, encoding="utf-8").read()
    return set(re.findall(r"\[([A-H][0-9]{2})\]", md))


if __name__ == "__main__":
    fails = 0
    for cid, f in CLAIMS.items():
        try:
            f()
            print(f"PASS {cid}")
        except Exception as e:  # noqa: BLE001
            fails += 1
            print(f"FAIL {cid}: {type(e).__name__}: {e}")
    pids, rids = page_ids(), report_ids()
    for name, ids in (("index.html", pids), ("REPORT.md", rids)):
        missing = sorted(ids - set(CLAIMS))
        if missing:
            fails += 1
            print(f"FAIL {name} cites claims with no assert: {missing}")
    unused = sorted(set(CLAIMS) - pids - {"A00"})
    if unused:
        fails += 1
        print(f"FAIL asserts with no claim on index.html: {unused}")
    print(f"{len(CLAIMS)} claims, {len(CLAIMS) - fails if fails <= len(CLAIMS) else 0} pass, {fails} fail")
    sys.exit(1 if fails else 0)
