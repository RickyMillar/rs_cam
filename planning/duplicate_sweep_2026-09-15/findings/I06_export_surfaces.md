# I06 — export surfaces (slugify + controller-comp mapping)
Verdict: TRUE_DUP (both pairs)

## Evidence

### Pair (a) — `slugify` ×2, score 0.9757
- `crates/rs_cam_viz/src/app/export.rs:260-270` and
  `crates/rs_cam_viz/src/ui/export_wizard.rs:304-314` — extracted both
  ranges, `diff` confirms **byte-identical** (12 lines each).
- Caller A: app/export.rs `render_filename` (L272) feeds the real save paths
  (Single L41, PerSetup L72, PerToolpath L111) of the wizard save handler.
- Caller B: export_wizard.rs `render_filename_preview` (L270, called L258) —
  the UI preview text of the same filename the save path will produce.
- Same purpose: sanitize `{job}/{setup}/{toolpath}` template values into
  filename-safe text. Both private `fn`, no unit tests in either module.
- Sentries: none directly pin sanitization. `crates/rs_cam_viz/tests/
  wizard_e2e.rs` (L187/212/249/486) exercises the save paths with templates
  `{job}.nc`, `{job}_{setup}.nc`, `{job}_{toolpath}.nc`, `{toolpath}.{ext}`;
  its per-setup filename massaging is done in-test (`replace(' ', "_")`),
  not via slugify.

### Pair (b) — `controller_comp_for_*_toolpath`, score 0.9416
- viz: `controller_comp_for_session_toolpath`,
  `crates/rs_cam_viz/src/io/export.rs:345-360`, private; sole caller
  `gcode_phase_for_session_toolpath` (L341), used by four viz export paths:
  `export_gcode_from_session_with_policy` (MCP export, L417), per-setup
  export (L474), single-toolpath export (L550), setup export (L617).
- core: `controller_comp_for_project_toolpath`,
  `crates/rs_cam_core/src/gcode/mod.rs:802-819`, private; sole caller
  inside `export_gcode_checked` (L374), the core checked-export door.
- Both: Profile op + `CompensationType::InControl` → identical (side×climb)
  → Left/Right mapping, `None` otherwise. Same inputs
  (`session::ToolpathConfig`), same output (`gcode::ControllerCompensation`).
- History: viz copy born 2026-04-08 (3959883c, "Implement G41/G42 controller
  compensation"); core copy cloned 2026-05-02 (105230ba) when
  `export_gcode_checked` was added. Core owns the enum
  (`gcode/mod.rs:61`), `GcodePhase`, and the G41/G42/G40 emitter
  (`gcode/program_builder.rs:574-583`).
- Sentries: `gcode/program_builder.rs` tests (comp emission);
  `tests/gcode_phase0_capture.rs` F16 `capture_f16_comp_round_trip`
  (G41→G40, ignored fixture capture); `tests/gcode_emulator_validation.rs`
  F16 (per-post G41/G40 acceptance). None call the mapping fn directly.

## Drift / differences
- None behavioral. Cosmetic only: let-binding+`return Some(dir)` (viz) vs
  `return Some(match …)` (core), inline `use` (core) vs top-of-file imports
  (viz), function names. Mapping tables are identical, no drift since 2026-05.

## Proposed cleanup
- (b) home: make core's fn `pub` in `crates/rs_cam_core/src/gcode/mod.rs`
  (rename e.g. `controller_compensation_for(&ToolpathConfig)`), delete the
  viz copy, call core from `gcode_phase_for_session_toolpath`. risk: low.
  proof: unit test in `gcode/mod.rs` covering all 4 (side, climb) arms +
  non-Profile → None; then `cargo test -p rs_cam_viz -q` (wizard_e2e export
  paths) and `cargo test -p rs_cam_core -q`.
- (a) home: single `pub(crate) fn slugify` in
  `crates/rs_cam_viz/src/app/export.rs` next to `render_filename`;
  `ui/export_wizard.rs` imports it. risk: low. proof: unit test
  `slugify("Job 1/Setup A") == "Job_1_Setup_A"` + assert wizard preview
  string equals the saved filename for a job containing a space.
