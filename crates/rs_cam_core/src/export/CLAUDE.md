# `export/` — output that is not G-code

The preview, the fingerprint diff and the G-code validator. The entry point
is `export::mod`, which re-exports the four surfaces.

## Files

- `mod.rs` — the facade.
- `viz.rs` — the SVG and HTML preview output.
- `fingerprint.rs` — toolpath fingerprinting and diffing for parameter checks.
- `gcode_validator.rs` — the G-code invariant validator.
- `artifact_io.rs` — one home for the JSON artifact dumps core writes to disk.

## Invariants

- Export zeroes at the stock top. `StockTop` maps to Z equal to zero. A
  flipped setup zeroes to the presented top, not to the original top.
- The validator reads emitted bytes. It is the answer to "did it cut too
  deep", not the plan.
- A preview is evidence of shape, not of load. Do not quote a preview as a
  safety verdict.

## Sentries

- `cargo test -p rs_cam_core -q --test composite_render_convention`
- `cargo test -p rs_cam_core -q --test gcode_validator_baseline`
- `cargo test -p rs_cam_core -q --test param_sweep`

## Do not

- Do not write an artifact from another module. Route it through
  `artifact_io.rs`.
