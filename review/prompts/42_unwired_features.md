# Review: Unwired / Partial Features

## Scope
Features that exist in code but are not fully end-to-end wired. Per FEATURE_CATALOG.md and investigation.

## Known candidates (verify each)
1. Manual G-code dressups: `pre_gcode` / `post_gcode` editable in UI but not emitted in export
2. G41/G42 cutter compensation: "In Control" in UI but output not wired
3. Feed optimization: limited to fresh-stock flat-stock workflows
4. Rapid collision rendering: detection exists, not rendered
5. Simulation deviation coloring: helper exists, not fed to renderer
6. Vendor LUT GUI: backend loader exists, no UI entry
7. Workholding rigidity UI: calculator supports all levels, GUI hardcodes "Medium"

## What to review

### For each candidate
1. Confirm it's actually unwired (code exists but not connected)
2. How much work to wire it? (estimate: trivial / moderate / significant)
3. Is there a reason it's unwired? (technical limitation, design decision, just not done yet?)
4. Risk of the partial state (user sees a UI control that doesn't work)

### Additional discovery
- Search for TODO, FIXME, HACK, unimplemented!, todo! in the codebase
- Search for `#[allow(dead_code)]` — what's dead and why?
- Search for commented-out code
- Any operations that generate toolpaths but can't export them?

## Output
Write findings to `review/results/42_unwired_features.md` with a table: Feature | Current State | Wiring Gap | Effort | Risk.
