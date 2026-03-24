# Toolpath Parameter Stress Test

Systematic catalog of every toolpath parameter for verification and regression testing.

## Purpose

1. Ensure every parameter change produces the expected effect
2. Verify defaults are sensible for common wood routing scenarios
3. Confirm no overlapping/duplicate parameters exist
4. Confirm every core parameter is exposed in the GUI
5. Identify placeholder/unimplemented GUI controls

## Files

| File | Contents |
|------|----------|
| `PARAMETER_MATRIX.md` | Master matrix: every parameter on every operation |
| `SHARED_SYSTEMS.md` | Heights, dressups, feeds/speeds, stock awareness |
| `DEFAULTS_ANALYSIS.md` | Default values and tool/stock sensitivity |
| `GAPS_AND_ISSUES.md` | GUI gaps, overlaps, placeholders, inconsistencies |
| `VALIDATION_PLAN.md` | How to verify each parameter with sim/debugger |
