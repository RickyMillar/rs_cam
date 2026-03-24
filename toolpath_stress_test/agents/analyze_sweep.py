#!/usr/bin/env python3
"""Analyze parameter sweep results and produce verdicts.

Usage:
    python3 analyze_sweep.py target/param_sweeps/
    python3 analyze_sweep.py target/param_sweeps/pocket/stepover/

Reads sweep_result.json files and applies validation rules to produce
PASS/FAIL/NO_EFFECT/UNEXPECTED verdicts for each parameter sweep.

Output: JSON array of verdicts to stdout, one per sweep.
"""

import json
import sys
import os
from pathlib import Path
from typing import Any

# Validation rules: what each parameter type should change
EXPECTED_EFFECTS = {
    # Geometric params that change pass count/density
    # NOTE: On 3D mesh ops, stepover also changes z_levels because different
    # grid points land at different surface heights. Only 2D ops keep z_levels fixed.
    "stepover": {
        "should_change": ["move_count", "cutting_distance_mm"],
        "should_not_change": [],  # On 3D ops, extreme stepover can eliminate passes
        "rule": "smaller_stepover_means_more_moves",
    },
    "angular_step": {
        "should_change": ["move_count"],
        "should_not_change": ["feed_rates"],
        "rule": "smaller_step_means_more_spokes",
    },
    "point_spacing": {
        "should_change": ["move_count"],
        "should_not_change": ["feed_rates"],
        "rule": "smaller_spacing_means_more_points",
    },
    "sampling": {
        "should_change": ["move_count"],
        "should_not_change": ["z_levels", "feed_rates"],
        "rule": "finer_sampling_means_smoother_contours",
    },
    "scallop_height": {
        "should_change": ["move_count", "cutting_distance_mm"],
        "should_not_change": ["feed_rates"],
        "rule": "smaller_scallop_means_more_passes",
    },

    # Depth params
    "depth": {
        "should_change": ["min_z", "z_levels"],
        "should_not_change": [],
        "rule": "depth_changes_z_levels",
    },
    "cut_depth": {
        "should_change": ["min_z", "z_levels"],
        "should_not_change": [],
        "rule": "depth_changes_z_levels",
    },
    "pocket_depth": {
        "should_change": ["min_z"],
        "should_not_change": [],
        "rule": "depth_changes_z_levels",
    },
    "depth_per_pass": {
        "should_change": ["z_level_count"],
        "should_not_change": ["min_z"],
        "rule": "smaller_pass_means_more_z_levels",
    },
    "z_step": {
        "should_change": ["z_level_count"],
        "should_not_change": [],
        "rule": "smaller_step_means_more_z_levels",
    },
    "max_stepdown": {
        "should_change": ["move_count"],
        "should_not_change": [],
        "rule": "smaller_stepdown_means_more_revolutions",
    },
    "fine_stepdown": {
        "should_change": ["z_level_count"],
        "should_not_change": [],
        "rule": "enables_intermediate_z_levels",
    },
    "max_depth": {
        "should_change": ["min_z"],
        "should_not_change": [],
        "rule": "limits_maximum_depth",
    },
    "min_z": {
        "should_change": ["min_z"],
        "should_not_change": [],
        "rule": "floor_clamp_changes",
    },
    "final_z": {
        "should_change": ["z_level_count"],
        "should_not_change": [],
        "rule": "changes_z_range",
    },

    # Feed/speed params (geometry should NOT change)
    "feed_rate": {
        "should_change": ["feed_rates", "max_feed_rate"],
        "should_not_change": ["move_count", "min_z", "z_levels"],
        "rule": "only_feed_values_change",
    },
    "plunge_rate": {
        "should_change": ["feed_rates"],
        "should_not_change": ["move_count", "min_z"],
        "rule": "only_feed_values_change",
    },

    # Height params
    "safe_z": {
        "should_change": ["max_z", "rapid_distance_mm"],
        "should_not_change": ["min_z"],
        "rule": "only_rapid_travel_changes",
    },

    # Side/direction params (geometry changes but aggregates may not)
    "side": {
        "should_change": [],  # bbox should shift
        "should_not_change": [],
        "rule": "bbox_shifts_by_tool_diameter",
    },
    "compensation": {
        "should_change": [],
        "should_not_change": [],
        "rule": "bbox_shifts_by_tool_radius",
    },
    "climb": {
        "should_change": [],
        "should_not_change": [],
        "rule": "direction_reversal_may_not_show_in_aggregates",
    },
    "direction": {
        "should_change": [],
        "should_not_change": [],
        "rule": "direction_change_may_not_show_in_aggregates",
    },

    # Boolean toggles (something must change)
    "slot_clearing": {
        "should_change": [],
        "should_not_change": [],
        "rule": "toggle_should_have_some_effect",
    },
    "dogbone": {
        "should_change": [],
        "should_not_change": [],
        "rule": "toggle_should_have_some_effect",
    },
    "z_blend": {
        "should_change": [],
        "should_not_change": [],
        "rule": "toggle_should_have_some_effect",
    },
    "detect_flat_areas": {
        "should_change": [],
        "should_not_change": [],
        "rule": "toggle_should_have_some_effect",
    },
    "steep_first": {
        "should_change": [],
        "should_not_change": [],
        "rule": "ordering_change_may_not_show",
    },
    "order_bottom_up": {
        "should_change": [],
        "should_not_change": [],
        "rule": "ordering_change_may_not_show",
    },
    "continuous": {
        "should_change": [],
        "should_not_change": [],
        "rule": "linking_change_affects_rapids",
    },

    # Geometry-altering params
    "chamfer_width": {
        "should_change": ["min_z"],
        "should_not_change": [],
        "rule": "width_affects_z_depth",
    },
    "threshold_angle": {
        "should_change": [],
        "should_not_change": [],
        "rule": "changes_steep_shallow_boundary",
    },
    "angle_threshold": {
        "should_change": [],
        "should_not_change": [],
        "rule": "changes_flat_area_detection",
    },
    "bitangency_angle": {
        "should_change": [],
        "should_not_change": [],
        "rule": "changes_crease_detection",
    },
    "num_offset_passes": {
        "should_change": [],  # on test geometry with no creases, may have no effect
        "should_not_change": [],
        "rule": "more_passes_means_more_moves_if_creases_exist",
    },
    "tolerance": {
        "should_change": ["move_count"],
        "should_not_change": ["min_z", "z_levels"],
        "rule": "tighter_tolerance_means_more_points",
    },
    "min_cutting_radius": {
        "should_change": [],
        "should_not_change": [],
        "rule": "corner_blending_changes_path",
    },
    "stock_offset": {
        "should_change": [],
        "should_not_change": ["min_z"],
        "rule": "extends_or_shrinks_pass_extents",
    },
    "clearing_strategy": {
        "should_change": [],
        "should_not_change": [],
        "rule": "different_algorithm_different_path",
    },
    "prev_tool_radius": {
        "should_change": [],
        "should_not_change": [],
        "rule": "changes_rest_region_geometry",
    },
    "angle": {
        "should_change": [],
        "should_not_change": ["min_z", "feed_rates"],
        "rule": "rotates_scan_lines",
    },
    "glue_gap": {
        "should_change": [],
        "should_not_change": [],
        "rule": "affects_male_plug_primarily",
    },
    "stock_to_leave": {
        "should_change": [],
        "should_not_change": [],
        "rule": "offsets_surface_position",
    },
    "cycle": {
        "should_change": ["move_count"],
        "should_not_change": ["min_z"],
        "rule": "different_cycle_different_retract_pattern",
    },
}


def analyze_diff(diff: dict, param_name: str, value: Any) -> dict:
    """Analyze a single fingerprint diff and produce a verdict."""
    changed = {f["field"] for f in diff.get("changed_fields", [])}
    unchanged = set(diff.get("unchanged_fields", []))
    changes = {f["field"]: f for f in diff.get("changed_fields", [])}

    rules = EXPECTED_EFFECTS.get(param_name, {})
    should_change = set(rules.get("should_change", []))
    should_not_change = set(rules.get("should_not_change", []))
    rule = rules.get("rule", "unknown_parameter")

    issues = []

    # Check expected changes happened
    for field in should_change:
        if field not in changed:
            issues.append(f"EXPECTED {field} to change but it didn't")

    # Check unexpected changes didn't happen
    for field in should_not_change:
        if field in changed:
            c = changes[field]
            issues.append(
                f"UNEXPECTED {field} changed: {c['before']} -> {c['after']} "
                f"({c.get('delta_percent', '?'):.1f}%)"
                if isinstance(c.get('delta_percent'), (int, float))
                else f"UNEXPECTED {field} changed"
            )

    # Determine verdict
    if not changed:
        if should_change:
            verdict = "FAIL"
            issues.append("No changes detected but expected changes")
        else:
            verdict = "NO_EFFECT"
    elif issues:
        # Has issues but also has changes
        has_fail = any("EXPECTED" in i for i in issues)
        has_unexpected = any("UNEXPECTED" in i for i in issues)
        if has_fail:
            verdict = "FAIL"
        elif has_unexpected:
            verdict = "UNEXPECTED"
        else:
            verdict = "PASS"
    else:
        verdict = "PASS"

    # Build summary
    if changed:
        top_changes = []
        for f in list(changes.values())[:3]:
            pct = f.get("delta_percent")
            if isinstance(pct, (int, float)) and abs(pct) < 1e6:
                top_changes.append(
                    f"{f['field']}: {f['before']}->{f['after']} ({pct:+.1f}%)"
                )
            else:
                top_changes.append(f"{f['field']}: changed")
        summary = "; ".join(top_changes)
    else:
        summary = "No measurable changes in any metric"

    return {
        "param": param_name,
        "value": value,
        "verdict": verdict,
        "rule": rule,
        "changed_count": len(changed),
        "unchanged_count": len(unchanged),
        "summary": summary,
        "issues": issues,
    }


def analyze_sweep_dir(sweep_dir: Path) -> list[dict]:
    """Analyze all sweeps in a directory tree."""
    results = []

    # Find all sweep_result.json files
    for root, dirs, files in os.walk(sweep_dir):
        if "sweep_result.json" not in files:
            continue

        with open(os.path.join(root, "sweep_result.json")) as f:
            sweep = json.load(f)

        op = sweep["operation"]
        param = sweep["parameter_name"]

        for variant in sweep["variants"]:
            diff = variant["diff"]
            value = variant["value"]
            verdict = analyze_diff(diff, param, value)
            verdict["operation"] = op
            results.append(verdict)

    return results


def main():
    if len(sys.argv) < 2:
        print("Usage: analyze_sweep.py <sweep_dir>", file=sys.stderr)
        sys.exit(1)

    sweep_dir = Path(sys.argv[1])
    if not sweep_dir.exists():
        print(f"Directory not found: {sweep_dir}", file=sys.stderr)
        sys.exit(1)

    results = analyze_sweep_dir(sweep_dir)

    # Aggregate
    total = len(results)
    pass_count = sum(1 for r in results if r["verdict"] == "PASS")
    fail_count = sum(1 for r in results if r["verdict"] == "FAIL")
    no_effect = sum(1 for r in results if r["verdict"] == "NO_EFFECT")
    unexpected = sum(1 for r in results if r["verdict"] == "UNEXPECTED")

    output = {
        "summary": {
            "total_variants": total,
            "pass": pass_count,
            "fail": fail_count,
            "no_effect": no_effect,
            "unexpected": unexpected,
        },
        "results": sorted(results, key=lambda r: (
            {"FAIL": 0, "UNEXPECTED": 1, "NO_EFFECT": 2, "PASS": 3}[r["verdict"]],
            r["operation"],
            r["param"],
        )),
    }

    json.dump(output, sys.stdout, indent=2)
    print()  # trailing newline

    # Also print human-readable summary to stderr
    print(f"\n{'='*60}", file=sys.stderr)
    print(f"SWEEP ANALYSIS: {total} variants across {len(set(r['operation'] for r in results))} operations", file=sys.stderr)
    print(f"  PASS: {pass_count}  FAIL: {fail_count}  NO_EFFECT: {no_effect}  UNEXPECTED: {unexpected}", file=sys.stderr)

    if fail_count > 0:
        print(f"\nFAILURES:", file=sys.stderr)
        for r in results:
            if r["verdict"] == "FAIL":
                print(f"  {r['operation']}/{r['param']}={r['value']}: {'; '.join(r['issues'])}", file=sys.stderr)

    if unexpected > 0:
        print(f"\nUNEXPECTED:", file=sys.stderr)
        for r in results:
            if r["verdict"] == "UNEXPECTED":
                print(f"  {r['operation']}/{r['param']}={r['value']}: {'; '.join(r['issues'])}", file=sys.stderr)

    if no_effect > 0:
        print(f"\nNO_EFFECT (parameter may be dead code or geometry-insensitive):", file=sys.stderr)
        for r in results:
            if r["verdict"] == "NO_EFFECT":
                print(f"  {r['operation']}/{r['param']}={r['value']}", file=sys.stderr)

    print(f"{'='*60}", file=sys.stderr)


if __name__ == "__main__":
    main()
