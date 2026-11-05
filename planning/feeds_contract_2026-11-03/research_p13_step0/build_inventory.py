"""G: the inventory of inputs and assumptions per output. Writes inventory.json.

Run: python3 -I build_inventory.py   Each row names a code site as (path, needle); the script
finds the needle in `git show 36fd50c54:<path>` and stores the line. A row with no site says
"no site". Status: a = the user states it, b = the model derives it, c = a typed constant stands
in for it, d = ignored with no stand-in. Rank: 1 = likely decides the result, 2 = can move it,
3 = minor. The ranks are this study's judgement; the page labels them so.
"""
import json
import os
import subprocess

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", "..", ".."))
REV = "36fd50c54"
C = "crates/rs_cam_core/src/"
ALL = ["stepover", "depth", "feed", "rpm", "plunge_ramp"]

ROWS = [
    # id, factor, outputs, status, stand-in, f201, hidden assumption, stated?, site path, needle, hobby, industrial, note
    ("G01", "Machine frame stiffness X/Y (gantry, belts, rails)", ["stepover", "depth", "feed"], "d", "none",
     "n/a", "The frame and the drives are rigid: the tool beam is clamped at the collet face and nothing behind it moves.",
     "partly: the beam doc says 'clamped at x = 0 (the collet face)'; no text says the frame is rigid",
     C + "tool/mod.rs", "stepped cantilever clamped at `x = 0` (the", 1, 3,
     "No printed machine stiffness in the repo (search report B). Section B back-solves an implied value."),
    ("G02", "Z axis and spindle mount stiffness", ["depth", "plunge_ramp", "feed"], "d", "none", "n/a",
     "The Z carriage and the spindle clamp are rigid (same clamp at the collet face).", "no",
     C + "tool/mod.rs", "stepped cantilever clamped at `x = 0` (the", 1, 3,
     "Adams prints 'The rigidity of your spindle and Z axis determines your plunge rate' (G6 source :241), no number."),
    ("G03", "Workholding stiffness (clamps, tape, vacuum)", ["stepover", "depth", "feed"], "d", "none (deleted)",
     "n/a", "The part does not move or vibrate.", "yes: R4 Q8 deleted the unsourced workholding scale",
     C + "feeds/mod.rs", "The workholding rigidity is gone (ruling R4 Q8", 2, 2, ""),
    ("G04", "Collet or holder type and stiffness", ["depth", "feed"], "d", "none", "n/a",
     "The collet is a rigid clamp; ER11 and shrink-fit are the same.", "no",
     C + "tool_load/deflection.rs", "**Bending only.**", 2, 2,
     "Leitz prints a relative claim only (shrink-fit vs collet), no stiffness number."),
    ("G05", "Tool stickout", ["depth", "stepover"], "a", "LEGACY_DEFAULT_STICKOUT_MM = 45 when unknown",
     "F201 Unsourced", "When the tool states no stickout, it sticks out 45 mm.", "yes: the lineage warns tool.fact_unknown",
     C + "compute/tool_config.rs", "pub const LEGACY_DEFAULT_STICKOUT_MM: f64 = 45.0;", 2, 2,
     "Moves only the tool beam; at 30 mm on D 6 the tool bends 5 um at the convention point."),
    ("G06", "Runout (TIR)", ["feed", "rpm"], "d", "none", "n/a",
     "Every flute cuts the same chip; the peak chip is the band chip.", "no", None, None, 2, 2,
     "On a 2-flute tool, runout loads one flute more; no site models it."),
    ("G07", "Flute count", ALL, "a", "LEGACY_DEFAULT_FLUTE_COUNT = 2 when unknown", "F201 Unsourced",
     "An unknown tool has 2 flutes.", "yes (F201 row)", C + "compute/tool_config.rs",
     "pub const LEGACY_DEFAULT_FLUTE_COUNT: u32 = 2;", 3, 3, ""),
    ("G08", "Helix angle", ["depth", "stepover"], "c", "default_helix_deg = 30", "typed, not in F201",
     "A tool with no stated helix has a 30 deg helix.", "no", C + "compute/tool_config.rs",
     "fn default_helix_deg() -> f64 {", 3, 3, "Moves the peak force only."),
    ("G09", "Flute length", ["depth"], "a", "LEGACY_DEFAULT_CUTTING_LENGTH_MM = 25; flute guard 0.8 x flute",
     "F201 Unsourced (both)", "The usable depth is 0.8 of the flute.", "yes (F201 rows)", C + "feeds/mod.rs",
     '"feeds.calculate.flute_guard_factor",', 3, 3, ""),
    ("G10", "Coating and series (ZrN, O-flute, compression)", ["feed", "rpm"], "a", "lookup tie-break only",
     "n/a", "A coating changes which row wins a tie, never a limit.", "yes", C + "feeds/series.rs",
     "table holds the ZrN rows (no subfamily states the coating in a", 3, 3, ""),
    ("G11", "Tool material stiffness (E)", ["depth"], "c", "CARBIDE_YOUNGS_MODULUS = 600 000 N/mm2",
     "typed, not in F201", "Every carbide grade has E = 600 GPa.", "comment: 'canonical handbook value', no file",
     C + "compute/tool_config.rs", "pub(crate) const CARBIDE_YOUNGS_MODULUS_N_PER_MM2: f64 = 600_000.0;", 3, 3, ""),
    ("G12", "Fluted section (bending diameter)", ["depth"], "c", "ENDMILL_EQUIVALENT_DIAMETER_FRACTION = 0.80",
     "typed, not in F201", "The fluted section bends as a solid rod of 0.8 D.",
     "yes: sentry the_bending_section_is_one_sourced_fraction_g_bendeq (Kops and Vo, not captured)",
     C + "feeds/predict.rs", "pub const ENDMILL_EQUIVALENT_DIAMETER_FRACTION: f64 = 0.80;", 3, 3, ""),
    ("G13", "Tool wear and the tool-life target", ["feed", "rpm"], "d", "none (u is a display proxy)", "n/a",
     "The tool is new and sharp; no life target.", "no", C + "feeds/efficiency.rs", "`u` is a wear proxy.", 3, 1,
     "Vendor chips are for new tools; an industrial user prices tool life."),
    ("G14", "Chip evacuation (slot depth, flute volume, air or vacuum)", ["depth", "stepover", "feed"], "d",
     "slot_doc_factor = 0.25 D (the only slot-depth stand-in)", "Unsourced (hobby default)",
     "Chips always clear, at any slot depth.", "no (drill pecks only credit evacuation)",
     C + "tool_load/drill_gates.rs", "Evacuation-credited ratio (F1)", 1, 1,
     "No printed numeric wood or plastic rule (section D); qualitative: upcut for slots (Onsrud, Vortex)."),
    ("G15", "Heat and burn", ["feed", "rpm", "stepover"], "c", "RUBBING_FLOOR_MM_TOOTH = 0.025 (warns only)",
     "F201 Unsourced", "No burn above the rubbing floor; no thermal model.", "yes (F201 row, warn-only)",
     C + "feeds/mod.rs", "pub const RUBBING_FLOOR_MM_TOOTH: f64 = 0.025;", 2, 2, ""),
    ("G16", "Material species and density", ALL, "a", "density from FPL rows", "Cited (FPL, Curti 2021)",
     "Plywood has no force line: power and deflection refuse.", "yes", C + "material/mod.rs",
     "Material::Plywood { .. } => Err(ForceLineRefusal::Plywood),", 2, 2, "Modelled for solid wood and MDF."),
    ("G17", "Grain direction", ["feed", "depth"], "c", "upper envelope over grain angle 0-180 deg",
     "Cited (Curti 2021)", "Every cut meets the worst grain angle.", "yes", C + "material/force_line.rs",
     "pub fn curti_helix0_envelope() -> (f64, f64) {", 3, 3, "Conservative by design."),
    ("G18", "Moisture content", ["feed"], "c", "FPL_12PCT_MOISTURE_FACTOR = 1.12 (12 % MC)", "Cited (FPL)",
     "The wood is at 12 % MC.", "yes", C + "material/force_line.rs",
     "pub const FPL_12PCT_MOISTURE_FACTOR: f64 = 1.12;", 3, 3, ""),
    ("G19", "Climb against conventional", ["feed", "stepover"], "c", "the larger Curti mode", "Cited",
     "The force is the worse mode; the sim does not split a slot.", "yes", C + "tool_load/power.rs",
     "climb/conventional split not modeled", 3, 3, ""),
    ("G20", "Radial force ratio K_r", ["depth", "stepover"], "c", "K_r = 0", "no source (stated)",
     "The cutting force has no radial part; the lateral force is under-stated.", "yes", C + "feeds/engagement.rs",
     "element has no radial component (`K_r = 0`)", 2, 2, ""),
    ("G21", "Surface finish target (wall, floor)", ["stepover", "depth", "feed"], "c",
     "DEFLECTION_BACKOFF_TARGET_UM = 200", "F201 Unsourced", "A 200 um tip deflection is acceptable in a rough.",
     "yes (F201 row)", C + "feeds/suggest/invariants.rs", '"feeds.suggest.DEFLECTION_BACKOFF_TARGET_UM",', 2, 2,
     "The only deflection limit. No Ra or wall-error model."),
    ("G22", "Machine feed cap", ["feed", "rpm", "stepover"], "a",
     "DEFAULT_CUTTING_FEED_CAP_MM_MIN = 6000 when unset", "typed, not in F201",
     "A machine with no stated cutting cap cuts at min(6000, travel).", "comment only", C + "machine/mod.rs",
     "pub const DEFAULT_CUTTING_FEED_CAP_MM_MIN: f64 = 6000.0;", 1, 3,
     "Binds at the FM1 point (feed 3999 of 4000)."),
    ("G23", "Machine acceleration", ["feed", "stepover"], "d", "none in feeds (cycle time only)", "n/a",
     "Every move reaches the commanded feed.", "no", C + "machine/mod.rs",
     "pub fn effective_kinematics(&self) -> MachineKinematics {", 2, 2,
     "Small pockets never reach the cap; the feeds side does not read it."),
    ("G24", "Spindle power and torque at the rpm", ["depth", "feed", "rpm"], "a",
     "generic_wood_router 0.8 kW when no machine", "typed preset", "A machine with no table has 0.8 kW at every rpm.",
     "no", C + "machine/mod.rs", "power: PowerModel::ConstantPower { power_kw: 0.8 },", 3, 1,
     "FM1 point uses 0.060 kW of 0.8 (slot case)."),
    ("G25", "Chatter (stability)", ["depth", "stepover", "rpm"], "d", "none", "n/a",
     "The cut is stable at every rpm and depth.", "no (hover copy only)", C + "tool_load/metric_guide.rs",
     "You hear chatter", 2, 1, "Section C."),
    ("G26", "Operator risk and tool-cost tolerance", ALL, "d", "none (D9 removed the dial)", "n/a",
     "One risk level: the vendor recommendation.", "yes: D9 sentry",
     "crates/rs_cam_core/tests/the_retired_aggressiveness_key_refuses_with_its_name_d9.rs",
     "aggressiveness dial is removed", 2, 2, ""),
    ("G27", "Depth convention", ["depth"], "c", "rigidity.doc_roughing_factor 0.25; AXIAL_K_1_5_MM 0.25 D",
     "Unsourced (hobby default; F201 row)", "A hobby machine cuts 0.25 D deep.", "yes (basis text)",
     C + "compute/geometry_default.rs", '"compute.geometry_default.AXIAL_K_1_5_MM",', 1, 2,
     "Printed hobby slots: Sienci hardwood 0.39 D, Carbide 3D mahogany 0.40 D."),
    ("G28", "Stepover convention (Pocket)", ["stepover"], "c", "operation_default_profile Pocket ae 0.35 D",
     "Unsourced (profile source)", "A Pocket steps 0.35 D.", "yes (Unsourced claim)", C + "feeds/mod.rs",
     "DOC/WOC matrix of reference/shapeoko_feeds_and_speeds calcs.rs (not in the repo)", 1, 2,
     "No computed limit binds below D (P11c sentry)."),
    ("G29", "Slot depth cap", ["depth"], "c", "slot_doc_factor 0.25 D above slotting_threshold_d 0.85",
     "Unsourced (both)", "A slot is 0.25 D deep on every hobby machine.", "yes", C + "feeds/mod.rs",
     '"feeds.calculate.slotting_threshold_d",', 1, 2, ""),
    ("G30", "Pocket first pass is a slot", ["depth", "feed"], "b", "PowerCase::Slot", "n/a",
     "Power is judged at ae = D for every Pocket width.", "yes", C + "feeds/engagement.rs",
     "A slot (ae = D): `FirstPass::Slot` or `Radial::Slot`.", 2, 2,
     "The force side of power reads ae = D; only the feed (through the lift) varies with the stepover."),
    ("G31", "Vendor chip and rpm (printed row)", ["feed", "rpm"], "a", "row rpm_nominal (MatchChart)",
     "Printed", "The vendor's machine class is this machine.", "partly: Amana banner says INDUSTRIAL",
     "crates/rs_cam_core/data/vendor_lut/sources/amana_spektra_spiral_plunge_v24.txt", "INDUSTRIAL", 2, 3, ""),
    ("G32", "Vendor depth table 1/2/3 x D", ["depth"], "b", "vendor_depth_table_ceiling_ratio", "Printed",
     "The table holds at any width.", "yes", C + "feeds/geometry.rs",
     "pub fn vendor_depth_table_ceiling_ratio(chip_share: f64) -> f64 {", 3, 2, ""),
    ("G33", "Pocket corner coverage (offset geometry)", ["stepover"], "d", "none", "n/a",
     "Any stepover up to D clears the corners.", "no", None, None, 2, 2,
     "Derivable geometry: where the inward offsets keep a sharp 90 deg corner (cavalier_contours, polygon.rs:77), the gap closes for s <= r (1 + 1/sqrt 2) = 0.854 D. Other corner angles change the bound."),
    ("G34", "Gantry thrust (stepper force)", ["feed", "depth"], "d", "none", "n/a",
     "The axes deliver any force the cut needs.", "no (TECH_DEBT_REGISTER: no such field)", None, None, 2, 3,
     "Agent prose cites forum skip forces 80-187 N (URL only, not captured)."),
    ("G35", "Ramp angle and plunge", ["plunge_ramp"], "c", "DRESSUP_RAMP_ANGLE_DEG = 3; G10 plunge fraction",
     "ramp: repo rule; plunge: Cited (G10)", "A 3 deg ramp suits every machine.", "yes (entry notes)",
     C + "compute/config.rs", "pub const DRESSUP_RAMP_ANGLE_DEG: f64 = 3.0;", 2, 3, ""),
    ("G36", "Long tool (L/D) derate", ["feed"], "c", "long_tool_load_share (warn only)", "Unsourced",
     "A long tool needs no lower feed.", "yes: R4 Q7", C + "feeds/mod.rs",
     "**Not a feed factor since ruling R4 Q7 (2026-09-24).**", 3, 3, ""),
    ("G37", "Surface speed when no row", ["rpm"], "c", "SPEED_SOLID_WOOD etc.", "F201 Unsourced",
     "A cutting speed constant per material class.", "yes", C + "material/mod.rs",
     '"material.base_cutting_speed_m_min.solid_wood",', 3, 3, "The row rpm wins where a row exists."),
]


def line_of(path, needle):
    src = subprocess.run(["git", "-C", REPO, "show", f"{REV}:{path}"], capture_output=True, text=True,
                         check=True).stdout
    for i, ln in enumerate(src.splitlines(), 1):
        if needle in ln:
            return i
    raise SystemExit(f"needle not found: {path}: {needle}")


def main():
    out = []
    for r in ROWS:
        (rid, factor, outs, status, standin, f201, assumption, stated, path, needle, rh, ri, note) = r
        site = f"{path}:{line_of(path, needle)}" if path else "no site"
        out.append({"id": rid, "factor": factor, "outputs": outs, "status": status, "stand_in": standin,
                    "f201": f201, "assumption": assumption, "stated": stated, "site": site, "needle": needle,
                    "rank_hobby": rh, "rank_industrial": ri, "note": note})
    json.dump(out, open(os.path.join(HERE, "inventory.json"), "w"), indent=1)
    for o in out:
        print(o["id"], o["status"], o["site"])


if __name__ == "__main__":
    main()
