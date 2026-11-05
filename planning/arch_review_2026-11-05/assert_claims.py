#!/usr/bin/env python3
"""Check each factual claim of REPORT.md against commit 666e6df06.

Run from anywhere:  python3 -I planning/arch_review_2026-11-05/assert_claims.py

The script reads only git objects (git show / git grep / git log); it does
not read the working tree, so a peer session's edits do not change the
result. It prints PASS or FAIL per claim ID and exits non-zero on any FAIL.
"""

import os
import re
import subprocess
import sys

REPO = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
REV = "666e6df06"
FEEDS_START = "f56000c54"
SEL_START = "97041cf82"

_cache = {}


def git(*args):
    return subprocess.run(
        ["git", "-C", REPO, *args], capture_output=True, text=True, check=False
    ).stdout


def show(path, rev=REV):
    key = (rev, path)
    if key not in _cache:
        _cache[key] = git("show", f"{rev}:{path}")
    return _cache[key]


def lines(path, rev=REV):
    return show(path, rev).split("\n")


def line(path, n, rev=REV):
    ls = lines(path, rev)
    return ls[n - 1] if 0 < n <= len(ls) else ""


def has(path, n, text, rev=REV):
    return text in line(path, n, rev)


def nlines(path, rev=REV):
    t = show(path, rev)
    return t.count("\n")


def nbytes(path, rev=REV):
    return len(show(path, rev).encode("utf-8"))


def maxline(path, rev=REV):
    return max(len(x) for x in lines(path, rev))


def grep(pattern, *paths, word=False, fixed=True, rev=REV):
    """git grep hits as (path, lineno, text) under `paths` at `rev`."""
    args = ["grep", "-n"]
    if fixed:
        args.append("-F")
    if word:
        args.append("-w")
    args += ["-e", pattern, rev, "--", *paths]
    out = []
    for raw in git(*args).splitlines():
        # rev:path:line:text
        _, path, ln, text = raw.split(":", 3)
        out.append((path, int(ln), text))
    return out


def src_hits(pattern, word=True):
    """Hits in crates/*/src, not in a tests.rs file, not in a comment line."""
    hits = grep(pattern, "crates", word=word)
    keep = []
    for path, ln, text in hits:
        if "/src/" not in path or not path.endswith(".rs") or path.endswith("tests.rs") or "/tests/" in path:
            continue
        if text.strip().startswith("//"):
            continue
        keep.append((path, ln, text))
    return keep


def count_in(pattern, *paths, rev=REV):
    return len(grep(pattern, *paths, rev=rev))


CORE = "crates/rs_cam_core/src/"
VIZ = "crates/rs_cam_viz/src/"
FC = "planning/feeds_contract_2026-11-03/"
GS = "planning/geometry_selection_2026-10-16/"


# --------------------------------------------------------------------------
# Claims. Each returns (ok, detail).
# --------------------------------------------------------------------------


def h1():
    p = CORE + "compute/execute/prologue.rs"
    s = CORE + "session/selection_status.rs"
    checks = [
        has(p, 505, "fn require_items("),
        has(p, 525, ".with_stock_grown("),
        has(p, 537, "view(resolved, item_use)"),
        has(p, 540, "drop_part_free_note("),
        has(p, 542, "fit_holes(resolved"),
        has(p, 544, "clear_edges("),
        has(s, 844, "with_stock_grown(radius)"),
        has(s, 989, "crate::selection::view(&frames.setup, item_use)"),
        has(s, 990, "drop_part_free_note("),
        has(s, 1067, "fit_holes(&frames.setup"),
        has(CORE + "session/pins.rs", 304, "crate::selection::view("),
        has(CORE + "session/pins.rs", 308, "fit_holes("),
        has(CORE + "compute/execute/drilling.rs", 75, "crate::selection::view("),
        has(CORE + "compute/execute/drilling.rs", 199, "crate::selection::view("),
        nlines(s) >= 2500,
        nlines(s, SEL_START) == 0,
    ]
    return all(checks), f"checks={checks} selection_status lines={nlines(s)}"


def h2():
    m = CORE + "feeds/mod.rs"
    checks = [
        has(m, 1990, "pub(crate) fn calculate_with("),
        line(m, 3467) == "}",
        has(m, 2708, "// --- Step 6: Power check ---"),
        has(m, 3055, "// --- Step 7: Machine feed ceiling ---"),
        has(m, 3275, "// --- Step 9c"),
        nlines(m, FEEDS_START) == 3539,
        nlines(m) == 4183,
    ]
    return all(checks), f"checks={checks}"


def h3():
    so = CORE + "feeds/sourced.rs"
    tc = CORE + "compute/tool_config.rs"
    checks = [
        has(so, 144, "pub const fn tool_fact() -> Self"),
        has(so, 147, "from: Vec::new()"),
        has(tc, 797, "pub fn cutting_length_planning_mm(&self) -> f64"),
        has(tc, 799, "LEGACY_DEFAULT_CUTTING_LENGTH_MM"),
        has(tc, 792, "pub fn stickout_planning_mm(&self) -> f64"),
        has(CORE + "feeds/suggest.rs", 984, "flute_length: tool.cutting_length_planning_mm()"),
        has(CORE + "feeds/suggest.rs", 1002, "tool_overhang_mm: Some(tool.stickout_planning_mm())"),
        has(CORE + "feeds/mod.rs", 1844, "Basis::tool_fact()"),
        len(src_hits("Basis::tool_fact()", word=False)) == 14,
    ]
    return all(checks), f"checks={checks}"


def h3b():
    m = CORE + "feeds/mod.rs"
    e = CORE + "feeds/envelope.rs"
    checks = [
        has(m, 1937, "pub(crate) const MIN_AP_MM: f64 = 0.05;"),
        has(m, 1940, "pub(crate) const MIN_AE_MM: f64 = 0.02;"),
        has(m, 2562, 'Basis::derived("MIN_AP_MM", Vec::new())'),
        has(m, 2571, 'Basis::derived("MIN_AE_MM", Vec::new())'),
        has(e, 1331, 'Basis::derived("MIN_AP_MM", Vec::new())'),
        has(e, 1337, 'Basis::derived("MIN_AE_MM", Vec::new())'),
    ]
    return all(checks), f"checks={checks}"


def m1():
    checks = [
        has(CORE + "tool_load/deflection.rs", 61, "pub const EXCEEDS_BOUND_MM: f64 = 0.200;"),
        has(CORE + "feeds/suggest/invariants.rs", 53, "DEFLECTION_BACKOFF_TARGET_UM: f64 = 200.0;"),
        has(CORE + "feeds/suggest/invariants.rs", 57, "DEFLECTION_BACKOFF_TARGET_ROW: FormulaConstant"),
        line(CORE + "feeds/suggest/invariants.rs", 59).strip() == "200.0,",
        has(CORE + "feeds/cutter_constraints.rs", 76, "DEFAULT_ROUGH_DEFLECTION_LIMIT_UM: f64 = 200.0;"),
        count_in('law: "tip deflection <= 200 um"', CORE + "feeds/envelope.rs") == 3,
    ]
    return all(checks), f"checks={checks}"


def m2():
    g = CORE + "feeds/geometry.rs"
    checks = [
        has(g, 17, "pub fn radial_chip_thinning_factor("),
        has(g, 29, ".clamp(1.0, 4.0)"),
        has(g, 55, "pub fn peak_chip_thinning_lift("),
        has(g, 72, "pub enum RadialThinning"),
        has(CORE + "dressup/feedopt.rs", 79, "radial_chip_thinning_factor(ae_fraction, 1.0)"),
        has(CORE + "dressup/feed_modulation.rs", 856, "let thinning = woc.sqrt()"),
        "BandMid" in line(FC + "STATUS.md", 159),
    ]
    return all(checks), f"checks={checks}"


def m3():
    checks = [
        has(CORE + "feeds/mod.rs", 4163, "pub fn effective_diameter("),
        has(CORE + "dressup/feed_modulation.rs", 258, "crate::feeds::effective_diameter("),
        has(CORE + "feeds/engagement.rs", 108, "2.0 * tool.engagement_radius(cut.ap_mm.max(0.0))"),
        has(CORE + "feeds/engagement.rs", 129, "tool.engagement_radius(ap_mm)"),
        has(CORE + "feeds/geometry.rs", 432, "pub fn feed_ladder_diameter_mm("),
        has(CORE + "feeds/geometry.rs", 426, "`feeds::calculate` Step 5a, Suggest pass 9"),
        len(src_hits("feed_ladder_diameter_mm")) == 1,
        "two diameter" in line(FC + "STATUS.md", 134),
    ]
    return all(checks), f"checks={checks}"


def m4():
    checks = [
        has(CORE + "machine/mod.rs", 175, "adaptive_woc_factor: 0.25,"),
        has(CORE + "machine/mod.rs", 694, "adaptive_woc_factor: 0.20,"),
        has(CORE + "compute/validate.rs", 277, "let floor = 0.15 * d;"),
        has(CORE + "compute/validate.rs", 284, "let new_value = 0.20 * d;"),
        has(CORE + "feeds/predict.rs", 261, "PREDICTOR_NON_ADAPTIVE_WOC_FRACTION: f64 = 0.35;"),
        has(CORE + "feeds/predict.rs", 268, "PREDICTOR_ADAPTIVE_WOC_FRACTION: f64 = 0.3;"),
        has(CORE + "feeds/mod.rs", 4073, "ae_factor.max(machine.rigidity.adaptive_woc_factor)"),
        has(CORE + "feeds/mod.rs", 4075, "ae_factor.min(machine.rigidity.adaptive_woc_factor)"),
        "stepover numbers" in line(FC + "STATUS.md", 161),
    ]
    return all(checks), f"checks={checks}"


def m5():
    t = "crates/rs_cam_core/tests/every_feeds_formula_constant_states_its_source_f201.rs"
    pin = lambda rev: re.search(r"PINNED_UNSOURCED_COUNT: usize = (\d+);", show(t, rev))
    start = pin(FEEDS_START)
    end = pin(REV)

    def sites(pat, rev):
        return count_in(pat, CORE, rev=rev)

    cited0, cited1 = sites("::cited(", FEEDS_START), sites("::cited(", REV)
    uns0, uns1 = sites("::unsourced(", FEEDS_START), sites("::unsourced(", REV)
    checks = [
        start is not None and start.group(1) == "144",
        end is not None and end.group(1) == "159",
        cited0 == 3 and cited1 == 3,
        uns0 == 99 and uns1 == 117,
        "the unsourced count is pinned, 149 since P5" in show(CORE + "feeds/CLAUDE.md"),
    ]
    return all(checks), f"checks={checks} cited {cited0}->{cited1} unsourced {uns0}->{uns1}"


def m6():
    a = CORE + "feeds/suggest/apply.rs"
    cmd = CORE + "session/command.rs"
    args_block = "\n".join(lines(cmd)[1185:1194])
    checks = [
        "explicit operator acceptance recorded in the command" in line(FC + "PLAN.md", 158),
        has(a, 732, "pub fn accept_unsourced(self)"),
        has(a, 1112, "rec.accept_unsourced()"),
        has(CORE + "feeds/suggest.rs", 937, ".accept_unsourced()"),
        has(cmd, 1186, "pub struct ApplyFeedsArgs"),
        "accept" not in args_block and "pub explored" in args_block,
        "accept_unsourced" in show("crates/rs_cam_core/CONTRACTS.md"),
    ]
    return all(checks), f"checks={checks}"


def m7():
    r = CORE + "session/run_state.rs"
    checks = [
        has(r, 152, "d.severity != Severity::Blocking"),
        has(r, 157, "ids::COMPAT_TOOL_KIND_REFUSED"),
        has(r, 147, "A cut into the"),
        has(CORE + "compute/model_gouge.rs", 256, "pub fn blocks_export(&self) -> bool"),
        has(CORE + "session/compute/export.rs", 394, "pub fn model_gouge_refusal("),
        has(CORE + "diagnostics/mod.rs", 40, "[`Severity`] — consequence"),
        has(CORE + "diagnostics/ids.rs", 557, '"precondition.no_tool" => NOT_READY'),
        "`precondition.no_tool` (Blocking)" in show("FEATURE_CATALOG.md"),
    ]
    return all(checks), f"checks={checks}"


DEAD_WARNINGS = [
    "AxialDocBelowBurnFloor", "AxialDocClampedByEnvelope", "AxialEnvelopeSafeBandEmpty",
    "ChiploadStillLowAfterRecalibration", "DeflectionBackoffFigureIsAFloor",
    "DeflectionBackoffUnmodeled", "DepthClampedToCuttingLength", "DppCappedByDeflection",
    "FeedRaisedForChipload", "FinishEnvelopeAdvisory", "ProjectCurveDepthInfeasible",
    "RoughingDepthClampedToRigidity", "StepoverClampedToToolDiameter", "StepoverRaisedForRuntime",
]


def suggest_warning_variants():
    ls = lines(CORE + "feeds/suggest.rs")
    start = next(i for i, l in enumerate(ls) if l.startswith("pub enum SuggestWarning"))
    depth, out, end = 0, [], start
    for i in range(start, len(ls)):
        if depth == 1:
            mm = re.match(r"    ([A-Z][A-Za-z0-9]*)\s*(\{|\(|,|$)", ls[i])
            if mm:
                out.append(mm.group(1))
        depth += ls[i].count("{") - ls[i].count("}")
        if i > start and depth == 0:
            end = i + 1
            break
    return out, start + 1, end


def m8():
    variants, enum_start, enum_end = suggest_warning_variants()
    rationale_tests = next(
        i + 1 for i, l in enumerate(lines(CORE + "feeds/rationale.rs")) if l.startswith("#[cfg(test)]")
    )
    bad = []
    for v in DEAD_WARNINGS:
        for path, ln, text in src_hits(v):
            if path == CORE + "feeds/suggest.rs" and enum_start <= ln <= enum_end:
                continue  # the definition
            if path == CORE + "feeds/suggest.rs" and text.strip().startswith("///"):
                continue
            if path == CORE + "feeds/rationale.rs" and (ln >= rationale_tests or "SuggestWarning::" in text):
                continue  # a match arm or a test
            if path == VIZ + "ui/feeds/why.rs" and "| SuggestWarning::" in text:
                continue  # a match arm
            if path == CORE + "session/compute.rs" and ("=>" in text or "reason =" in text):
                continue  # a projection arm
            bad.append((path, ln, text.strip()[:80]))
    ok = len(variants) == 26 and not bad and "16 `SuggestWarning` variants" in line(FC + "STATUS.md", 298)
    return ok, f"variants={len(variants)} unexpected producers={bad}"


def m9():
    checks = [
        has(VIZ + "ui/feeds/compare.rs", 860, "CalculatorOperatingPoint {"),
        has(VIZ + "app/mcp/generation.rs", 33, "CalculatorOperatingPoint {"),
        has("crates/rs_cam_cli/src/suggest.rs", 94, "CalculatorOperatingPoint {"),
        has("crates/rs_cam_cli/src/suggest.rs", 142, '"shipped_power": shipped_power'),
        count_in("power_at_operating_point", VIZ + "app/mcp") == 0,
        count_in("shipped_power", VIZ + "app/mcp") == 0,
    ]
    return all(checks), f"checks={checks}"


def m10():
    t = VIZ + "ui/toolpath_panel.rs"
    o = CORE + "session/op_draft.rs"
    checks = [
        has(t, 1317, "pub fn add_menu_offers("),
        has(t, 1319, "session.models().iter().any(|m| m.mesh.is_some())"),
        has(t, 1328, "ItemsAndMesh => has_items && has_mesh"),
        has(o, 268, "pub fn serves(&self, op_type: OperationType) -> bool"),
        has(o, 269, "let has_mesh = self.mesh.is_some();"),
        "add_menu_offers" in line(GS + "PROMPT_NEXT.md", 61),
    ]
    return all(checks), f"checks={checks}"


def m11():
    bad = {}
    for f in ("woc_roughing_factor", "woc_roughing_max_mm", "woc_finishing_mm"):
        others = [
            (p, ln) for p, ln, _ in src_hits(f)
            if p != CORE + "machine/mod.rs" and p != VIZ + "app/mcp/project.rs"
        ]
        mcp = [ln for p, ln, _ in src_hits(f) if p == VIZ + "app/mcp/project.rs"]
        if others or len(mcp) != 1:
            bad[f] = (others, mcp)
    checks = [
        not bad,
        has(CORE + "machine/mod.rs", 169, "doc_roughing_factor: 0.25,"),
        has(CORE + "machine/mod.rs", 688, "doc_roughing_factor: 0.20,"),
    ]
    return all(checks), f"checks={checks} bad={bad}"


def m12():
    c = CORE + "compute/CLAUDE.md"
    s = CORE + "session/CLAUDE.md"
    f = CORE + "feeds/CLAUDE.md"
    checks = [
        nbytes(c) == 24030 and nlines(c) == 40 and maxline(c) >= 6000,
        nbytes(s) == 21234 and nbytes(s, SEL_START) == 3077,
        nbytes(f) == 16120 and nbytes(f, FEEDS_START) == 8938,
        nlines("crates/rs_cam_core/CONTRACTS.md") == 164,
        "40 lines or fewer" in line(FC + "PLAN.md", 354),
        nlines(VIZ + "app/CLAUDE.md") == 41 and nlines(VIZ + "app/CLAUDE.md", FEEDS_START) == 40,
        nlines(VIZ + "ui/CLAUDE.md") == 43 and nlines(VIZ + "ui/CLAUDE.md", FEEDS_START) == 40,
    ]
    return all(checks), f"checks={checks}"


def l1():
    n = len(grep("enforce_invariants", "crates", word=True))
    defs = len(grep("fn enforce_invariants(", "crates"))
    test_file = git("ls-tree", "--name-only", REV, "crates/rs_cam_core/tests/suggest_feed_matches_final_geometry.rs").strip()
    checks = [
        n >= 20,
        defs == 0,
        has(CORE + "feeds/mod.rs", 4158, "suggest_feed_matches_final_geometry.rs"),
        test_file == "",
        has(CORE + "feeds/mod.rs", 1938, "Suggest pass 6b"),
    ]
    return all(checks), f"checks={checks} enforce_invariants mentions={n}"


def l2():
    sc = CORE + "compute/catalog/schema.rs"
    checks = [
        has(sc, 617, "pub struct DressupPolicy"),
        has(sc, 618, "entry ramps"),
        "Replaces `DressupPolicy`" in line(FC + "PLAN.md", 254),
        count_in("dressup_policy: DressupPolicy::", CORE + "compute/catalog/registry.rs") >= 25,
    ]
    return all(checks), f"checks={checks}"


def l3():
    sites = [("flat.rs", 27), ("ball.rs", 26), ("tapered_ball.rs", 93), ("bullnose.rs", 42), ("vbit.rs", 61)]
    checks = [has(CORE + "tool/" + f, n, "helix_deg: 30.0,") for f, n in sites]
    checks.append("30 degree helix" in line(FC + "STATUS.md", 136))
    return all(checks), f"checks={checks}"


def l4():
    hits = grep("DRILL_DWELL_S", "crates")
    checks = [
        has(CORE + "compute/operation_configs.rs", 158, "pub const DRILL_DWELL_S: f64 = 0.5;"),
        not any("FormulaConstant" in t or "unsourced(" in t for _, _, t in hits),
    ]
    return all(checks), f"checks={checks}"


def l5():
    li = CORE + "trace/lineage.rs"
    checks = [
        has(li, 435, "No production path calls it."),
        has(li, 436, "#[doc(hidden)]"),
        has(li, 438, "pub fn unrecorded() -> Self"),
    ]
    return all(checks), f"checks={checks}"


def l6():
    e = CORE + "session/compute/export.rs"
    checks = [
        nlines(e, FEEDS_START) == 947,
        nlines(e) == 1315,
        has(e, 1195, "pub fn lineage_measured("),
        has(e, 634, "pub fn diagnose_toolpath("),
    ]
    return all(checks), f"checks={checks}"


def l7():
    t = "crates/rs_cam_core/tests/every_feeds_formula_constant_states_its_source_f201.rs"
    pins = []
    for rev in ("c0db8dcb6", "bbaf105ef", "c7936ddd8", "84bfeab68"):
        mm = re.search(r"PINNED_UNSOURCED_COUNT: usize = (\d+);", show(t, rev))
        pins.append(mm.group(1) if mm else None)
    checks = [
        nlines(FC + "opgolden_known_46_rows.txt") == 46,
        "10 known red targets" in line(FC + "STATUS.md", 96),
        pins == ["146", "149", "154", "159"],
    ]
    return all(checks), f"checks={checks} pins={pins}"


def l8():
    checks = [
        has(VIZ + "app/mcp/project.rs", 687, "normal.z.abs() > 0.95"),
        has(CORE + "geometry/enriched_mesh.rs", 312, "HORIZONTAL_FACE_MIN_NORMAL_Z: f64 = 0.95;"),
        has(CORE + "selection/view.rs", 630, "< HORIZONTAL_FACE_MIN_NORMAL_Z"),
    ]
    return all(checks), f"checks={checks}"


def held():
    checks = [
        has(VIZ + "controller/events/mod.rs", 1079, "Command::ApplyFeeds(rs_cam_core::session::ApplyFeedsArgs {"),
        has("crates/rs_cam_cli/src/project.rs", 1362, "Command::ApplyFeeds(rs_cam_core::session::ApplyFeedsArgs {"),
        has(VIZ + "controller/events/mod.rs", 1124, "self.apply_feeds_through_funnel("),
        has(CORE + "session/compute/export.rs", 165, "pub fn export_warnings("),
        has(VIZ + "io/export.rs", 357, "session.export_warnings("),
        len(src_hits("FeedsHints")) == 0,
        len(src_hits("EntryStylePolicy")) == 0,
        len(src_hits("PlungeKnown")) == 0,
        has(CORE + "feeds/mod.rs", 1861, "pub(crate) fn slot_rule()"),
        has(CORE + "feeds/mod.rs", 1838, "pub(crate) fn flute_guard_of("),
        has(CORE + "selection/flat_regions.rs", 20, "HORIZONTAL_FACE_MIN_NORMAL_Z"),
        len(grep(r"pub fn [a-z_]*_mut\b", CORE + "session", fixed=False)) == 0,
        has(VIZ + "app/mcp/commands/selection.rs", 487, "#[cfg(test)]"),
    ]
    return all(checks), f"checks={checks}"


CLAIMS = [
    ("H1", h1), ("H2", h2), ("H3", h3), ("H3b", h3b),
    ("M1", m1), ("M2", m2), ("M3", m3), ("M4", m4), ("M5", m5), ("M6", m6),
    ("M7", m7), ("M8", m8), ("M9", m9), ("M10", m10), ("M11", m11), ("M12", m12),
    ("L1", l1), ("L2", l2), ("L3", l3), ("L4", l4), ("L5", l5), ("L6", l6),
    ("L7", l7), ("L8", l8), ("HELD", held),
]


def main():
    if not git("rev-parse", "--verify", REV + "^{commit}").strip():
        print(f"FAIL setup: {REV} not found in {REPO}")
        return 1
    fails = 0
    for cid, fn in CLAIMS:
        try:
            ok, detail = fn()
        except Exception as exc:  # a claim that crashes is a FAIL
            ok, detail = False, f"exception: {exc!r}"
        print(f"{'PASS' if ok else 'FAIL'} {cid}" + ("" if ok else f"  {detail}"))
        fails += 0 if ok else 1
    print(f"{len(CLAIMS) - fails}/{len(CLAIMS)} claims pass at {REV}")
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main())
