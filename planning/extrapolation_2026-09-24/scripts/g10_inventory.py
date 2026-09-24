#!/usr/bin/env python3
"""G10 Phase 0 inventory: the entry parameters (ramp, helix, plunge, entry feed).

Read-only. It reads the Rust source of the working tree (regular expressions,
so a moved default shows as a changed value or as NOT FOUND) and the FM1
matrix CSV. It writes g10_inventory_cells.csv (one row per matrix cell) and
prints every table of INVENTORY_G10.md.

Every value that the source or the CSV does not print is labelled "derived"
in the output, with its formula.
"""
import collections, csv, math, pathlib, re, subprocess

ROOT = pathlib.Path(__file__).resolve().parents[3]
CORE = ROOT / "crates/rs_cam_core/src"
MATRIX = ROOT / "planning/feeds_matrix_2026-09-23/matrix_2026-09-23.csv"
OUT = pathlib.Path(__file__).resolve().parents[1] / "g10_inventory_cells.csv"

_cache = {}


def text(rel):
    if rel not in _cache:
        _cache[rel] = (ROOT / rel).read_text()
    return _cache[rel]


def find(rel, pattern, flags=re.S):
    """First match of `pattern` in `rel`: (file:line of group 1, group 1)."""
    s = text(rel)
    m = re.search(pattern, s, flags)
    if not m:
        return (f"{rel}: NOT FOUND", None)
    pos = m.start(1) if m.groups() else m.start()
    line = s.count("\n", 0, pos) + 1
    short = rel.replace("crates/rs_cam_core/src/", "core/").replace("crates/rs_cam_viz/src/", "viz/") \
        .replace("crates/rs_cam_cli/src/", "cli/")
    return (f"{short}:{line}", m.group(1) if m.groups() else m.group(0))


def num(v):
    return float(v) if v not in (None, "") else None


OPC = "crates/rs_cam_core/src/compute/operation_configs.rs"
CFG = "crates/rs_cam_core/src/compute/config.rs"
REG = "crates/rs_cam_core/src/compute/catalog/registry.rs"
FEEDS = "crates/rs_cam_core/src/feeds/mod.rs"
MAT = "crates/rs_cam_core/src/material/mod.rs"
DESC = "crates/rs_cam_core/src/dressup/entry_descent.rs"
DMOD = "crates/rs_cam_core/src/dressup/mod.rs"
DAPP = "crates/rs_cam_core/src/compute/execute/dressup_apply.rs"
F3D = "crates/rs_cam_core/src/compute/execute/finish_3d.rs"
A3P = "crates/rs_cam_core/src/adaptive3d/path.rs"
AENT = "crates/rs_cam_core/src/feeds/suggest/adaptive_entry.rs"
GCLS = "crates/rs_cam_core/src/feeds/geometry_class.rs"
SUG = "crates/rs_cam_core/src/feeds/suggest.rs"
INV = "crates/rs_cam_core/src/feeds/suggest/invariants.rs"
APPLY = "crates/rs_cam_core/src/feeds/suggest/apply.rs"
PROV = "crates/rs_cam_core/src/feeds/provenance.rs"
RAT = "crates/rs_cam_core/src/feeds/rationale.rs"
PSTRESS = "crates/rs_cam_core/src/tool_load/plunge_stress.rs"
PENCIL = "crates/rs_cam_core/src/finish/pencil/emission.rs"
AD2 = "crates/rs_cam_core/src/adaptive/path.rs"
AD2S = "crates/rs_cam_core/src/adaptive/search.rs"
TP = "crates/rs_cam_core/src/toolpath.rs"
CAT = "crates/rs_cam_core/src/compute/catalog.rs"
JOB = "crates/rs_cam_cli/src/job.rs"
UI_DRESS = "crates/rs_cam_viz/src/ui/properties/linking_dressup.rs"
UI_A3D = "crates/rs_cam_viz/src/ui/properties/operations/surface_3d.rs"

# ── The source values ───────────────────────────────────────────────────

V = {}


def val(key, rel, pattern):
    V[key] = find(rel, pattern)
    return V[key]


val("a3d_ramp", OPC, r"fn default_adaptive3d_ramp_angle\(\) -> f64 \{\s*([\d.]+)")
val("a3d_hrf", OPC, r"fn default_adaptive3d_helix_radius_factor\(\) -> f64 \{\s*([\d.]+)")
val("a3d_pitch", OPC, r"fn default_adaptive3d_helix_pitch\(\) -> f64 \{\s*([\d.]+)")
val("a3d_style", OPC, r"impl Default for Adaptive3dConfig.*?entry_style: Adaptive3dEntryStyle::(\w+)")
val("a3d_radius_frame", F3D, r"radius: (ctx\.tool_def\.diameter\(\) \* cfg\.helix_radius_factor)")
val("dr_ramp", CFG, r"impl Default for DressupConfig.*?ramp_angle: ([\d.]+)")
val("dr_hr", CFG, r"impl Default for DressupConfig.*?helix_radius: ([\d.]+)")
val("dr_pitch", CFG, r"impl Default for DressupConfig.*?helix_pitch: ([\d.]+)")
val("dr_style", CFG, r"impl Default for DressupConfig.*?entry_style: DressupEntryStyle::(\w+)")
val("role_rough", CFG, r"UiProcessRole::Roughing => Self \{\s*entry_style: DressupEntryStyle::(\w+)")
val("role_semi", CFG, r"UiProcessRole::SemiFinish => Self \{\s*entry_style: DressupEntryStyle::(\w+)")
val("role_fin", CFG, r"UiProcessRole::Finish => Self \{\s*entry_style: DressupEntryStyle::(\w+)")
val("role_fin_lead", CFG, r"UiProcessRole::Finish => Self \{.*?lead_in_out: (Some\(LeadParams::default\(\)\))")
val("lead_r", CFG, r"impl Default for LeadParams.*?radius: ([\d.]+)")
val("lead_in_fb", DMOD, r"let li_feed = (lead_in_feed_rate\.unwrap_or\(plunge_rate\))")
val("lead_in_500", DMOD, r"MoveType::Linear \{ feed_rate \} => feed_rate,\s*_ => ([\d.]+),\s*\};\s*// F-040: lead-in")
val("entry_clear", DESC, r"pub\(crate\) const ENTRY_CLEARANCE: f64 = ([\d.]+)")
val("fold_laps", DESC, r"pub const RAMP_FOLD_MAX_LAPS: u32 = (\d+)")
val("fold_minrun", DMOD, r"min_run_mm: (tool_radius_mm\.max\([\d.]+\))")
val("dr_feed_half", DAPP, r"Some\((feed_rate \* [\d.]+)\)")
val("dr_feed_fb", DAPP, r"Some\(feed_rate \* [\d.]+\),\s*_ => None,\s*\}\)\s*\.unwrap_or\(([\d.]+)\)")
val("dr_feed_min", DMOD, r"(feed_rate\.min\(plunge_rate\))")
val("entry_contact", DESC, r"pub const ENTRY_CONTACT_CLEARANCE: f64 = ([\d.]+)")
val("dr_entry_clear", CFG, r"impl Default for DressupConfig.*?entry_clearance_mm: (default_entry_clearance_mm\(\))")
val("a3d_entry_clear", OPC, r"impl Default for Adaptive3dConfig.*?entry_clearance_mm: crate::compute::config::(default_entry_clearance_mm\(\))")
val("entry_clear_default", CFG, r"pub fn default_entry_clearance_mm\(\) -> f64 \{\s*(crate::dressup::ENTRY_CONTACT_CLEARANCE)")
val("rapid_floor_rule", DESC, r"let rapid_floor = (if measured \{\s*top \+ ENTRY_CONTACT_CLEARANCE\s*\} else \{\s*top \+ ENTRY_CLEARANCE\s*\})")
val("helix_start_rule", DESC, r"(contact_top\.unwrap_or\(material_top\)\.max\(end\.z\) \+ contact_clearance\.max\(0\.0\))")
val("emit_ramp_feed", DESC, r"fn emit_ramp.*?let ramp_feed = (safety\.ramp_feed\.unwrap_or\(feed_rate\))")
val("emit_helix_feed", DESC, r"fn emit_helix.*?let ramp_feed = (safety\.ramp_feed\.unwrap_or\(feed_rate\))")
val("dapp_ramp_feed", DAPP, r"(ramp_feed: ramp_feed_rate_mm_min),")
val("dapp_measured", DAPP, r"ramp_feed: ramp_feed_rate_mm_min,\s*(stock_top_measured: false)")
val("dapp_clearance", DAPP, r"(contact_clearance: cfg\.entry_clearance_mm)")
val("replay_measured", DMOD, r"(stock_top_measured: read\.is_some\(\))")
val("a3d_ramp_feed", A3P, r"(ramp_feed: params\.ramp_feed_rate)")
val("a3d_clearance", A3P, r"(contact_clearance: params\.entry_clearance_mm)")
val("ui_entry_clear", UI_DRESS, r'"entry_clearance_mm",.*?(\d+\.\d+\.\.=\d+\.\d+)')
val("a3d_buffer", A3P, r"const RAPID_DESCENT_BUFFER_MM: f64 = ([\d.]+)")
val("a3d_peck", A3P, r"const PECK_CLEARANCE_MM: f64 = ([\d.]+)")
val("a3d_entry_feed", A3P, r"EntryStyle3d::Helix \{ radius, pitch \} => \{\s*crate::dressup::emit_helix\([^)]*?(params\.plunge_rate)")
val("a3d_floor_r", A3P, r"EntryStyle3d::Helix \{ radius, \.\. \} => (tool_radius \+ radius\.max\(0\.0\))")
val("plunge_wood", MAT, r"fn plunge_rate_base.*?Material::SolidWoodByJanka \{ \.\. \} => ([\d.]+) / h")
val("plunge_sheet", MAT, r"fn plunge_rate_base.*?Material::SheetGood \{ \.\. \} => ([\d.]+) / h")
val("plunge_dscale", MAT, r"let d_scale = (\(tool_diameter_mm\.max\(0\.1\) / 6\.0\)\.clamp\([\d.]+, [\d.]+\))")
val("h_exp", MAT, r"Material::SolidWood \{ species \} => \(species\.janka_lbf\(\) / ([\d.]+)\)\.powf\(([\d.]+)\)")
val("ramp_feed", FEEDS, r"let ramp_feed = (\(feed \* [\d.]+\)\.max\(plunge\)\.min\(plunge \* [\d.]+\))")
val("ramp_feed_note", FEEDS, r"// Ramp feed: (capped at [^\n]*)")
val("ball_cap", FEEDS, r"let cap = ([\d.]+) \* tip_d;")
val("plunge_rule_note", FEEDS, r"diameters scale linearly with the audit\s*// (rule-of-thumb \([^)]*\))")
val("clamp_plunge", INV, r"pub\(super\) fn (clamp_plunge_to_feed)")
val("pstress", PSTRESS, r"const PLUNGE_CAP_PER_MM_TIP_DIAMETER: f64 = ([\d.]+)")
val("unstable", AENT, r"const PLUNGE_ENTRY_UNSTABLE_DPP_OVER_D: f64 = ([\d.]+)")
val("pick_to_ramp", AENT, r"Adaptive3dEntryStyle::(Ramp),\s*\"ramp\"")
val("headroom", GCLS, r"let required = (helix_radius_factor \* tool_diameter_mm \* [\d.]+ \+ tool_diameter_mm \* [\d.]+)")
val("scope_default", SUG, r"#\[default\]\s*(\w+),")
val("apply_writes", APPLY, r"if write_speeds \{\s*(operation\.set_feed_rate\(scratch\.feed_rate\(\)\);\s*operation\.set_plunge_rate)")
val("prov_plunge", PROV, r"(plunge_rate: Some\(chip\))")
val("card_plunge", RAT, r'PLUNGE_AT_MATERIAL_BASE_TEXT: &str = "([^"]*)"')
val("pencil_angle", PENCIL, r"const ENTRY_RAMP_MAX_ANGLE_DEG: f64 = ([\d.]+)")
val("pencil_frac", PENCIL, r"const ENTRY_RAMP_BITE_TIP_FRACTION: f64 = ([\d.]+)")
val("pencil_min", PENCIL, r"const ENTRY_RAMP_MIN_BITE_MM: f64 = ([\d.]+)")
val("pencil_max", PENCIL, r"const ENTRY_RAMP_MAX_BITE_MM: f64 = ([\d.]+)")
val("pencil_win", PENCIL, r"const ENTRY_RAMP_MIN_WINDOW_MM: f64 = ([\d.]+)")
val("pencil_laps", PENCIL, r"const ENTRY_RAMP_MAX_LAPS: usize = (\d+)")
val("ad2_helix_r", AD2, r"let (helix_r = tool_radius);")
val("ad2_req", AD2, r"let (required = [\d.]+ \* tool_radius);")
val("ad2_inscr", AD2S, r"let (min_inscribed = tool_radius \* [\d.]+);")
val("plunge_clear", TP, r"pub const PLUNGE_CLEARANCE_MM: f64 = ([\d.]+)")
val("safe_clear", CFG, r"pub const SAFE_Z_CLEARANCE_MM: f64 = ([\d.]+)")
val("lap_cap_roles", CAT, r"(UiProcessRole::Finish \| UiProcessRole::SemiFinish) => \{\s*Some\(crate::dressup::RAMP_FOLD_MAX_LAPS\)")
val("cli_ramp", JOB, r"dressups\.ramp_angle = ([\d.]+);")
val("cli_hr", JOB, r"dressups\.helix_radius = ([\d.]+);")
val("cli_pitch", JOB, r"dressups\.helix_pitch = ([\d.]+);")
val("cli_a3d_hrf", JOB, r'"helix_radius_factor", json!\(([\d.]+)\)')
val("cli_a3d_pitch", JOB, r'"helix_pitch", json!\(([\d.]+)\)')
val("cli_a3d_ramp", JOB, r'"ramp_angle_deg", json!\(([\d.]+)\)')
val("cli_entry_ops", JOB, r"(OperationType::Pocket \| OperationType::Profile \| OperationType::Adaptive)\s*\) && let Some\(entry\)")
val("ui_dr_ramp", UI_DRESS, r'"ramp_angle",.*?(\d+\.\d+\.\.=\d+\.\d+)')
val("ui_dr_hr", UI_DRESS, r'"helix_radius",.*?(\d+\.\d+\.\.=\d+\.\d+)')
val("ui_dr_pitch", UI_DRESS, r'"helix_pitch",.*?(\d+\.\d+\.\.=\d+\.\d+)')
val("ui_a3d_ramp", UI_A3D, r'"ramp_angle_deg", "Ramp Angle:"\),.*?(\d+\.\d+\.\.=\d+\.\d+)')
val("ui_a3d_hrf", UI_A3D, r'"helix_radius_factor",.*?(\d+\.\d+\.\.=\d+\.\d+)')
val("ui_a3d_pitch", UI_A3D, r'"helix_pitch", "Helix Pitch:"\),.*?(\d+\.\d+\.\.=\d+\.\d+)')

# Janka of the four FM1 materials, read from the source.
JANKA = {
    "softwood": num(find(MAT, r"WoodSpecies::GenericSoftwood => ([\d.]+)")[1]),
    "hardwood": num(find(MAT, r"WoodSpecies::GenericHardwood => ([\d.]+)")[1]),
    "mdf": num(find(MAT, r"SheetGoodKind::Mdf => ([\d.]+),")[1]),
    "plywood_hardwood": num(find(MAT, r"PlywoodGrade::BalticBirch => ([\d.]+),")[1]),
}

# Is the approved op field on master? (a text search of every crate source)
rg = subprocess.run(["rg", "-n", r"ramp_feed_rate\s*:", str(ROOT / "crates")], capture_output=True, text=True)
RAMP_FIELD = [l for l in rg.stdout.splitlines() if "operation_configs.rs" in l and "pub ramp_feed_rate: Option<f64>" in l]
# Does any feeds / Suggest code write the field? (the feeds session owns the value)
rg2 = subprocess.run(["rg", "-n", r"ramp_feed_rate", str(CORE / "feeds")], capture_output=True, text=True)
N_NONE = len(re.findall(r"^\s*ramp_feed_rate: None,", text(OPC), re.M))
RAMP_WRITERS = [l for l in rg2.stdout.splitlines() if "let ramp_feed_rate" not in l and "ramp_feed_mm_min: ramp_feed_rate" not in l]

# ── The registry: role, feeds family, tool rule, dressup policy per op ───

REGISTRY = {}
for m in re.finditer(r"static REG_\w+: OpRegistryEntry = OpRegistryEntry \{(.*?)\n\};", text(REG), re.S):
    b = m.group(1)
    g = lambda p: (re.search(p, b, re.S) or [None, None])[1]
    op = g(r"op_type: OperationType::(\w+)")
    kinds = g(r"required_kinds: &\[([^\]]*)\]")
    REGISTRY[op] = {
        "role": g(r"ui_process_role: UiProcessRole::(\w+)"),
        "feeds_family": g(r"feeds_family: FeedsOperationFamily::(\w+)"),
        "tools": "any" if kinds is None else kinds.replace("CutterKind::", ""),
        "policy": g(r"dressup_policy: DressupPolicy::(\w+)"),
    }

ROLE_STYLE = {"Roughing": V["role_rough"][1], "SemiFinish": V["role_semi"][1], "Finish": V["role_fin"][1]}


def default_dressup_style(op):
    """`DressupConfig::for_op`: the role default, then the registry policy."""
    r = REGISTRY[op]
    style = ROLE_STYLE[r["role"]]
    if r["policy"] in ("strip_all", "FORCE_NO_ENTRY"):
        return "None"
    if r["policy"] == "PREFER_HELIX" and style == "Ramp":
        return "Helix"
    return style


def entry_class(op):
    r = REGISTRY[op]
    if op == "Adaptive3d":
        return "E3 adaptive3d planner entry"
    if r["feeds_family"] == "Drill":
        return "E7 drill (no entry; plunge = drill feed)"
    if r["policy"] == "strip_all":
        return "E6 no entry (strip all)"
    if r["policy"] == "FORCE_NO_ENTRY":
        return "E5 no entry (forced none)"
    s = default_dressup_style(op)
    if s == "Ramp":
        return "E1 2.5D rough, dressup Ramp on"
    if s == "Helix":
        return "E2 2D adaptive, dressup Helix on"
    if r["tools"] == "VBit":
        return "E4a V-bit ops, dressup entry off (operator may set)"
    return "E4b 3D finishes, dressup entry off (operator may set)"


# ── Engine arithmetic, re-done from the source values ───────────────────

H_REF, H_EXP = (float(x) for x in re.search(r"/ ([\d.]+)\)\.powf\(([\d.]+)\)",
                                             text(MAT)[text(MAT).find("fn feed_scale_factor"):]).groups())
PL_WOOD, PL_SHEET = num(V["plunge_wood"][1]), num(V["plunge_sheet"][1])
DS_LO, DS_HI = (float(x) for x in re.search(r"clamp\(([\d.]+), ([\d.]+)\)", V["plunge_dscale"][1]).groups())
RF_K, RF_HI = (float(x) for x in re.search(r"feed \* ([\d.]+)\)\.max\(plunge\)\.min\(plunge \* ([\d.]+)", V["ramp_feed"][1]).groups())
CAP_K = num(V["ball_cap"][1])
DR_HALF = float(re.search(r"\* ([\d.]+)", V["dr_feed_half"][1]).group(1))


def plunge_base(material, d):
    """`Material::plunge_rate_base`: derived from the source constants."""
    h = (JANKA[material] / H_REF) ** H_EXP
    base = (PL_WOOD if material in ("softwood", "hardwood") else PL_SHEET) / h
    return base * min(max(max(d, 0.1) / 6.0, DS_LO), DS_HI)


def tip_d(tool, d):
    # FM1 builds the tapered ball with `diameter` as the tip (diameter_reason).
    return d if tool in ("BallNose", "TaperedBallNose") else None


def fmt(x, n=0):
    return "" if x is None else f"{x:.{n}f}"


# ── The matrix ──────────────────────────────────────────────────────────

rows = list(csv.DictReader(open(MATRIX)))
drill_chip = {}  # (tool, d, material) -> axial chip per tooth from the G6 claim
for x in rows:
    if x["operation"] == "Drill" and x["status"] == "ok":
        drill_chip[(x["tool_type"], x["diameter_mm"], x["material"])] = (
            float(x["feed_mm_min"]) / (float(x["rpm"]) * float(x["flutes"])))

A3D_RAMP, A3D_HRF, A3D_PITCH = num(V["a3d_ramp"][1]), num(V["a3d_hrf"][1]), num(V["a3d_pitch"][1])
DR_RAMP, DR_HR, DR_PITCH = num(V["dr_ramp"][1]), num(V["dr_hr"][1]), num(V["dr_pitch"][1])

cells, mismatch = [], []
for x in rows:
    op, tool, d, mat = x["operation"], x["tool_type"], float(x["diameter_mm"]), x["material"]
    ec = entry_class(op)
    c = {k: x[k] for k in ("tool_type", "diameter_mm", "flutes", "operation", "material", "status")}
    c["entry_class"] = ec
    c["registry_tools"] = REGISTRY[op]["tools"]
    style = angle = hr = pitch = None
    if ec.startswith("E3"):
        style = "plunge (peck)"
        if "StrategyRewrote" in x["suggest_warnings"]:
            style = "ramp (Suggest rewrite)"
            angle = A3D_RAMP
        # Helix (operator choice) values, for the record:
        hr_op = A3D_HRF * d
    elif ec.startswith("E1"):
        style, angle = "ramp (dressup)", DR_RAMP
    elif ec.startswith("E2"):
        style, hr, pitch = "helix (dressup)", DR_HR, DR_PITCH
    elif ec.startswith("E4"):
        style = "none (operator may set ramp or helix)"
    else:
        style = "none"
    c["entry_style_default"] = style
    c["ramp_angle_deg"] = fmt(angle, 1)
    c["helix_radius_mm"] = fmt(hr, 2)
    c["helix_pitch_mm"] = fmt(pitch, 1)
    c["plunge_mm_min_csv"] = x["plunge_mm_min"]
    c["feed_mm_min_csv"] = x["feed_mm_min"]
    c["rpm_csv"] = x["rpm"]
    rule = rf_today = rf_dress = rf_new = rf_new_basis = ""
    if x["status"] == "ok":
        pl, feed, rpm = float(x["plunge_mm_min"]), float(x["feed_mm_min"]), float(x["rpm"])
        base = plunge_base(mat, d)
        cap = CAP_K * tip_d(tool, d) if tip_d(tool, d) else None
        if ec.startswith("E7"):
            rule = "drill feed (G6 claim)"
        else:
            cands = [("material base", base)]
            if cap:
                cands.append(("ball tip cap", cap))
            name, v = min(cands, key=lambda t: t[1])
            if feed < v:
                name, v = "clamped to feed", feed
            rule = name
            if abs(math.floor(v) - pl) > 1.0:
                mismatch.append((tool, d, op, mat, pl, round(v, 1), name))
        # derived: FeedsResult::ramp_feed_mm_min, with the shipped feed in place
        # of the calculator feed (the CSV does not carry the calculator feed).
        rf_today = fmt(min(max(RF_K * feed, base), RF_HI * base))
        if style in ("ramp (dressup)", "helix (dressup)"):
            rf_dress = fmt(min(pl, DR_HALF * pl))  # derived: min(plunge, 0.5 x first fed move = plunge)
        # derived: the approved ramp feed on the default entry of the cell
        theta = None
        if angle:
            theta = math.radians(angle)
        elif hr and pitch:
            theta = math.atan(pitch / (2 * math.pi * hr))
        if theta and not ec.startswith("E7"):
            chip = drill_chip.get((tool, x["diameter_mm"], mat))
            if chip:
                rf_new = fmt(min(feed, chip * rpm * float(x["flutes"]) / math.tan(theta)))
                rf_new_basis = f"G6 axial chip {chip:.4f} mm"
            else:
                rf_new = fmt(pl)
                rf_new_basis = "plunge fallback (no printed axial chip)"
    c["plunge_rule"] = rule
    c["ramp_feed_today_derived"] = rf_today
    c["dressup_entry_feed_derived"] = rf_dress
    c["ramp_feed_approved_derived"] = rf_new
    c["ramp_feed_approved_basis"] = rf_new_basis
    c["suggest_rewrote_style"] = "yes" if "StrategyRewrote" in x["suggest_warnings"] else ""
    cells.append(c)

with open(OUT, "w", newline="") as f:
    w = csv.DictWriter(f, fieldnames=list(cells[0].keys()))
    w.writeheader()
    w.writerows(cells)


# ── Printing ────────────────────────────────────────────────────────────

def loc(k):
    return V[k][0]


def v(k):
    return V[k][1]


print("## A. Source values (file:line, value)\n")
for k, (where, value) in V.items():
    print(f"- {k}: `{value}` at {where}")
print(f"- Janka (FM1 materials): {JANKA}")
print(f"- `ramp_feed_rate: Option<f64>` on {len(RAMP_FIELD)} op configs in operation_configs.rs; "
      f"`ramp_feed_rate: None,` in {N_NONE} defaults. Writers in feeds/ (Suggest): {RAMP_WRITERS or 'none'}")

print("\n## B. Registry: per-operation entry defaults (DressupConfig::for_op)\n")
print("| Operation | Role | Feeds family | Tools | Dressup policy | Default dressup entry | Entry class |")
print("|---|---|---|---|---|---|---|")
for op in REGISTRY:
    r = REGISTRY[op]
    print(f"| {op} | {r['role']} | {r['feeds_family']} | {r['tools']} | {r['policy']} | "
          f"{default_dressup_style(op)} | {entry_class(op)} |")

print("\n## C. Matrix reach: per tool kind x size x entry class\n")
print("Cells: all 4 woods. Plunge from the CSV (ok cells). The ramp and helix values are the "
      "defaults of the class; the CSV carries no entry column.\n")
print("| Tool | D | Entry class | Cells | Ok | Default entry | Angle / helix | Plunge (CSV) | Plunge rule | "
      "Dressup entry feed (derived) | Approved ramp feed (derived) |")
print("|---|---|---|---|---|---|---|---|---|---|---|")
grp = collections.OrderedDict()
for c in sorted(cells, key=lambda c: (c["tool_type"], float(c["diameter_mm"]), c["entry_class"])):
    grp.setdefault((c["tool_type"], c["diameter_mm"], c["entry_class"]), []).append(c)


def rng(vals):
    vals = sorted(float(t) for t in vals if t not in ("", None))
    if not vals:
        return "-"
    return f"{vals[0]:.0f}" if vals[0] == vals[-1] else f"{vals[0]:.0f}-{vals[-1]:.0f}"


for (tool, d, ec), cs in grp.items():
    ok = [c for c in cs if c["status"] == "ok"]
    styles = sorted({c["entry_style_default"] for c in ok}) or sorted({c["entry_style_default"] for c in cs})
    geo = sorted({(c["ramp_angle_deg"] and f"{c['ramp_angle_deg']} deg") or
                  (c["helix_radius_mm"] and f"r {c['helix_radius_mm']} mm, p {c['helix_pitch_mm']} mm") or "-"
                  for c in (ok or cs)})
    rules = sorted({c["plunge_rule"] for c in ok if c["plunge_rule"]})
    basis = sorted({c["ramp_feed_approved_basis"].split(" (")[0].replace("G6 axial chip", "G6 chip")
                    for c in ok if c["ramp_feed_approved_basis"]})
    newrf = rng(c["ramp_feed_approved_derived"] for c in ok)
    print(f"| {tool} | {float(d):g} | {ec.split(' ')[0]} | {len(cs)} | {len(ok)} | {'; '.join(styles)} | "
          f"{'; '.join(geo)} | {rng(c['plunge_mm_min_csv'] for c in ok)} | {'; '.join(rules) or '-'} | "
          f"{rng(c['dressup_entry_feed_derived'] for c in ok)} | "
          f"{newrf}{' (' + '; '.join(basis) + ')' if basis else ''} |")

print("\n## D. Counts\n")
ok = [c for c in cells if c["status"] == "ok"]
cnt = collections.Counter(c["entry_class"] for c in cells)
cnt_ok = collections.Counter(c["entry_class"] for c in ok)
print("| Entry class | Cells | Ok cells |")
print("|---|---|---|")
for k in sorted(cnt):
    print(f"| {k} | {cnt[k]} | {cnt_ok[k]} |")
print(f"| total | {len(cells)} | {len(ok)} |")
print()
print("Plunge rule on the ok cells:", dict(collections.Counter(c["plunge_rule"] for c in ok)))
print("Suggest rewrote the Adaptive3d entry style (FM1, no model bbox, so to Ramp):",
      sum(1 for c in cells if c["suggest_rewrote_style"]), "of",
      sum(1 for c in ok if c["operation"] == "Adaptive3d"), "ok Adaptive3d cells")
print("Adaptive3d ok cells left at Plunge:",
      [(c["tool_type"], c["diameter_mm"], c["material"]) for c in ok
       if c["operation"] == "Adaptive3d" and not c["suggest_rewrote_style"]])
print("Cells whose default entry has a G6 printed axial chip:",
      sum(1 for c in ok if c["ramp_feed_approved_basis"].startswith("G6")),
      "; plunge fallback:", sum(1 for c in ok if c["ramp_feed_approved_basis"].startswith("plunge")))
print("Plunge re-derivation mismatches (> 1 mm/min):", mismatch or "none")
ratios = [float(c["ramp_feed_today_derived"]) / float(c["plunge_mm_min_csv"]) for c in ok
          if c["ramp_feed_today_derived"] and not c["entry_class"].startswith("E7")]
print(f"FeedsResult::ramp_feed_mm_min (derived, shipped feed in place of the calculator feed) / CSV plunge: "
      f"{min(ratios):.2f}-{max(ratios):.2f} over {len(ratios)} ok non-drill cells; "
      f"at the {RF_HI:g} x base cap on {sum(1 for c in ok if c['ramp_feed_today_derived'] and not c['entry_class'].startswith('E7') and abs(float(c['ramp_feed_today_derived']) - RF_HI * plunge_base(c['material'], float(c['diameter_mm']))) < 1)} cells")

print("\n## E. Context: the 6 mm flat end mill in hardwood, Adaptive3d (derived)\n")
ctx = next(x for x in rows if x["tool_type"] == "EndMill" and x["diameter_mm"].startswith("6")
           and x["material"] == "hardwood" and x["operation"] == "Adaptive3d")
feed, pl, rpm, dpp = (float(ctx[k]) for k in ("feed_mm_min", "plunge_mm_min", "rpm", "depth_per_pass_mm"))
chip = drill_chip[("EndMill", ctx["diameter_mm"], "hardwood")]
print(f"CSV: feed {feed:.0f} mm/min, plunge {pl:.0f} mm/min, RPM {rpm:.0f}, depth/pass {dpp:.3f} mm, "
      f"flutes {ctx['flutes']}; G6 axial chip {chip:.4f} mm/tooth (Drill cell feed / (RPM x Z)).")
print("Formulas: helix path per mm of depth L = sqrt((2 pi r)^2 + p^2) / p; time per mm = 60 L / F; "
      "helix angle = atan(p / (2 pi r)); approved feed = min(feed, chip x RPM x Z / tan(angle)).\n")
print("| Helix | r (mm) | p (mm) | Angle (deg) | L (mm/mm) | s/mm at plunge | Approved feed | s/mm at approved | "
      "s per entry at dpp, plunge | s per entry at dpp, approved | Ratio |")
print("|---|---|---|---|---|---|---|---|---|---|---|")
for label, r, p, plunge in (("op default", A3D_HRF * 6.0, A3D_PITCH, pl),
                            ("rivmap100 arm", A3D_HRF * 6.0, 1.0, pl),
                            ("rivmap100 arm at its plunge 500", A3D_HRF * 6.0, 1.0, 500.0)):
    L = math.hypot(2 * math.pi * r, p) / p
    ang = math.degrees(math.atan(p / (2 * math.pi * r)))
    fa = min(feed, chip * rpm * float(ctx["flutes"]) / math.tan(math.radians(ang)))
    t_pl, t_fa = 60 * L / plunge, 60 * L / fa
    print(f"| {label} | {r:.2f} | {p:.1f} | {ang:.2f} | {L:.3f} | {t_pl:.3f} | {fa:.0f} | {t_fa:.3f} | "
          f"{t_pl * dpp:.2f} | {t_fa * dpp:.2f} | {t_pl / t_fa:.2f} |")
print("\nThe whole-rough ratio needs the entry count and the cut length of a generated toolpath. "
      "The CSV has neither, so the script does not compute it.")
