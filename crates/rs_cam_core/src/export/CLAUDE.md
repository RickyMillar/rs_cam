# `export/` — output that is not G-code

The preview, the fingerprint diff and the G-code validator. The entry point
is `export::mod`, which re-exports the four surfaces.

## Files

- `mod.rs` — the facade.
- `viz.rs` — the SVG and HTML preview output.
- `ribbon.rs` — the toolpath ribbon mesh and the per-vertex colour ramps.
- `fingerprint.rs` — toolpath fingerprinting and diffing for parameter checks.
- `gcode_validator.rs` — the G-code invariant validator.
- `artifact_io.rs` — one home for the JSON artifact dumps core writes to disk.

## Invariants

- The datum rule for Z zero lives in `../gcode/CLAUDE.md`.
- The validator reads emitted bytes, not the plan.
- The 6-view composite renderer keeps one camera convention: every panel uses
  `StockConfig::origin`, one shared scale and consistent polar handedness.
- Colour lives here, not in the core model. `ribbon.rs` reads one classifier,
  `AnnotatedToolpath::classify_span_path`, so a PNG and the GUI viewport
  stratify the same way (X-1).

## Sentries

- `cargo test -p rs_cam_core -q --test composite_render_convention`
- `cargo test -p rs_cam_core -q --test gcode_validator_baseline`
- `cargo test -p rs_cam_core -q --test param_sweep`
- `cargo test -p rs_cam_core -q --test exporter_span_classifier_x1`

## Do not

- Do not write an artifact from another module. Route it through
  `artifact_io.rs`.
