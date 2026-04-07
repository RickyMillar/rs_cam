# Vendor LUT GUI Plan

This file tracks the remaining GUI work around the feeds/speeds and vendor-LUT stack.

## Already shipped

- embedded vendor LUT loading in `rs_cam_core`
- LUT-assisted feeds/speeds calculation
- machine and material models in the desktop app
- feeds card in the properties panel
- auto/manual toggles for feed, plunge, stepover, DOC, and spindle speed

## Remaining UI work

### Workholding rigidity

- expose `Low` / `Medium` / `High` in the GUI
- stop hardcoding `Medium` in the feeds integration path

### Tool overhang feedback

- show L:D ratio near stickout
- display active derates when overhang drives a reduction

### Source-label polish

- replace raw observation IDs with readable vendor/source labels
- show whether the recommendation came from LUT data or the fallback formula

### Additional LUT loading

- add a settings or file-menu flow for loading extra observation files
- document how user-supplied data merges with embedded data

### Derate breakdown

- show the combined effect of overhang, workholding, safety factor, and power limiting

## Documentation expectation

When any of the items above ship, update:

- `FEATURE_CATALOG.md`
- `CREDITS.md` if new external data sources are introduced
- `crates/rs_cam_core/src/feeds/INTEGRATION.md`
