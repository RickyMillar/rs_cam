# Core-edges design and feature-debt audit — 2026-09-17

Group: core-edges — `gcode/`, `export/`, `io/`, `tool/`, `machine/`, `material/`,
`diagnostics/`, `metrology/`, `util/`, and the spine files at
`crates/rs_cam_core/src/` (`geo.rs`, `polygon.rs`, `mesh.rs`, `toolpath.rs`,
`ids.rs`, `interrupt.rs`, `measurement.rs`).

### EDG-01 Folklore Kc reported with the same confidence as literature Kc
- kind: feature-debt
- pattern: guard/refusal reports the wrong thing
- where: `crates/rs_cam_core/src/material/mod.rs:826-880`
- evidence: `kc_n_per_mm2` doc says `None` means "no primary measurement — the
  tool-load gates refuse ... rather than predicting force from a fabricated
  constant" (line 826), yet the match arms for `WoodSpecies::RadiataPine`
  (line 856, comment "folklore retained. TODO: source from CSIRO or FRI
  publications."), `Jarrah` (line 873) and `Ipe` (line 876) all return
  `Some(value)`, identical in shape to the FPL-Table-5-3a-cited species right
  next to them. Three of eleven `WoodSpecies` arms are folklore wearing a
  cited number's clothes.
- proposal: give these three arms a distinct `Confidence`/provenance tag (or
  route them through `Material::SolidWoodByJanka`'s already-`None`-capable
  path) so a downstream gate can tell "measured" from "folklore" instead of
  reading `Some(19.0)` as equally authoritative as `Some(13.8)` (FPL white
  oak). At minimum, name the three constants so `rg` finds them without
  reading source comments.
- breaks: none (Kc values unchanged; only the confidence signal changes) —
  a `Confidence`-plumbing change to `tool_load`/`feeds` is owner: power
  session territory, so scope this to adding the distinction in `material/`.
- effort: M
- risk: medium — touches a load-bearing force constant three species deep
  into the deflection/power gates; needs a sentry before any numeric change.
- sentry: none found for the three folklore species specifically; nearest is
  `crates/rs_cam_core/tests/wood_species_library_provenance.rs` (covers the
  *library*, not the `Material::SolidWood` first-class arms).
- owner: blank (touches feeds/tool_load consumers — coordinate with power
  session before wiring a new confidence field through)

### EDG-02 Wood species library is code; the sibling catalogues are data
- kind: design
- pattern: one concept, two representations
- where: `crates/rs_cam_core/src/material/wood_species_library.rs:1-841`
  vs. `crates/rs_cam_core/src/io/tool_library.rs`,
  `crates/rs_cam_core/src/io/machine_library.rs`
- evidence: `wood_species_library.rs` is an 841-line `pub const
  WOOD_SPECIES_LIBRARY: &[WoodSpeciesEntry]` Rust literal (~130 entries,
  each `display_name`/`scientific_name`/`janka_lbf`/`source_id`) baked into
  the binary. `io/CLAUDE.md` names `tool_library.rs`/`machine_library.rs` as
  "the two on-disk TOML catalogues"; `material/CLAUDE.md` does not exist to
  say otherwise. Regeneration is manual: the file's own header says
  "Regenerate via the script captured in `phaseE_2026-05-31.md` if either
  source updates."
- proposal: move `WOOD_SPECIES_LIBRARY` to a shipped TOML/CSV data file
  loaded once at startup (or via `OnceLock`, matching the `gcode/post.rs`
  `include_str!` + parse pattern already used for the four posts), so
  adding a species is a data edit, not a recompile, and the entries are
  diffable by an operator without reading Rust.
- breaks: none if kept as a shipped read-only asset; breaks any code that
  pattern-matches `WoodSpeciesEntry` as a `'static` const (none found
  outside `material/`).
- effort: M
- risk: low — read-only reference data, well covered by
  `wood_species_library_provenance.rs`.
- sentry: `cargo test -p rs_cam_core -q --test wood_species_library_provenance`
- owner: blank

### EDG-03 DXF and SVG import have no file-size guard; STEP does
- kind: feature-debt
- pattern: guard applied to one importer, missing on siblings
- where: `crates/rs_cam_core/src/io/step_input.rs:35-53` vs.
  `crates/rs_cam_core/src/io/dxf_input.rs:30-35`,
  `crates/rs_cam_core/src/io/svg_input.rs:23-38`
- evidence: `step_input.rs` defines `StepImportError::FileTooLarge` and
  checks `std::fs::metadata(path)?.len()` against `MAX_FILE_SIZE` before
  parsing (lines 46-53). `DxfError` (dxf_input.rs:30) has only `Io`/
  `NoEntities`; `SvgError` (svg_input.rs:23) has only `Io`/`Parse`/
  `NoPaths`. Neither calls `fs::metadata` or checks a size limit before
  `std::fs::read`/parsing (`rg -n "fs::metadata|FileTooLarge" io/dxf_input.rs
  io/svg_input.rs` returns no matches).
- proposal: add the same size check ahead of `std::fs::read` in
  `dxf_input.rs`/`svg_input.rs`, with a `FileTooLarge`-shaped error variant
  on each existing error enum (matching `named_toml_library.rs`'s policy of
  keeping each importer's own error type but sharing the *mechanism*).
- breaks: adds a new error variant to `DxfError`/`SvgError` (non-exhaustive
  match sites would need updating — `rg` shows none outside `io/` and its
  tests match these exhaustively via `?`, so low blast radius).
- effort: S
- risk: low
- sentry: `cargo test -p rs_cam_core -q --test step_import` is the model to
  copy; no DXF/SVG equivalent exists today.
- owner: blank

### EDG-04 Point-to-segment distance duplicated between geo.rs and metrology
- kind: design
- pattern: duplicate helper
- where: `crates/rs_cam_core/src/geo.rs:374-393` (`point_to_segment_distance`,
  `P2`) vs. `crates/rs_cam_core/src/metrology/spacing.rs:69-77`
  (`dist_point_segment`, `P3`)
- evidence: both implement the identical clamped-projection algorithm
  (`t = dot(ap,ab)/dot(ab,ab)` clamped to `[0,1]`, then distance to the
  clamped point); `geo.rs`'s is 2D on `P2`, `spacing.rs`'s is 3D on `P3`
  but only ever called with samples that share a common `z` per call site
  (`metrology/spacing.rs:159`). Structure-programme evidence
  (`planning/structure_2026-09-17/evidence_round2/dup_sweep_0.88_src.md`)
  flags the pair at 0.8909 similarity. `dist_point_segment` has exactly one
  production call site (`spacing.rs:159`) plus its own unit test.
- proposal: since `geo.rs` is the crate's declared spine for geometric
  primitives (`crates/rs_cam_core/CLAUDE.md`), add a `P3` (or generic)
  point-to-segment-distance helper there and have `metrology/spacing.rs`
  call it, deleting the private copy.
- breaks: none — `dist_point_segment` is `pub` but only used inside
  `spacing.rs` and its own tests (`rg -n "dist_point_segment"` — 1 call
  site outside definition/tests).
- effort: S
- risk: low
- sentry: `cargo test -p rs_cam_core -q --test spacing_prize_split_f1`;
  keep `spacing.rs`'s own `dist_point_segment_clamps_and_quantile_rounds`
  unit test alive against the moved function.
- owner: blank

### EDG-06 MCP export silently discards machine-safety findings the GUI shows
- kind: feature-debt
- pattern: operation missing from one of the four surfaces
- where: `crates/rs_cam_viz/src/io/export.rs:8-33` (`log_machine_safety`),
  called from `export_gcode_from_session_with_policy` (line ~412), called
  from `crates/rs_cam_viz/src/app/mcp.rs:968` (`mcp_export_gcode`); contrast
  `crates/rs_cam_viz/src/ui/export_wizard.rs:17,778,916` and
  `crates/rs_cam_viz/src/app/export.rs:3,232` which call
  `rs_cam_core::export::gcode_validator::validate`/`validate_machine_safety`
  directly and surface `Finding`s to the interactive GUI.
- evidence: `log_machine_safety` runs `validate_machine_safety` (checks
  `RapidBelowClearance`, `SpindleNotRunningAtCut`, `ZBelowProgramFloor`,
  `SpindleLeftRunning`, `ProgramEndsBelowClearance` —
  `crates/rs_cam_core/src/export/gcode_validator.rs:60-88`) and only
  `tracing::warn!`s the count/first message (io/export.rs:24-32); it never
  returns the findings. `mcp_export_gcode` (app/mcp.rs:967-980) on success
  returns only `text(format!("G-code exported to {path}"))` — no findings,
  no severity, no line numbers. `rg -n "gcode_validator" crates/rs_cam_cli
  crates/rs_cam_mcp` returns nothing: the CLI and the MCP wire-type crate
  never see this validator either. An MCP-driven export that trips
  `Severity::Error` (a genuine safety/correctness issue per the enum's own
  doc) is indistinguishable, to the calling agent, from a clean export.
- proposal: have `mcp_export_gcode` append a findings summary (count by
  severity, first N messages) to its success `text(...)` when
  `log_machine_safety`'s result is non-empty, instead of only logging
  server-side; consider giving the CLI's export path the same call.
- breaks: none — additive to the MCP tool's returned text.
- effort: S
- risk: low
- sentry: none found for the MCP export path surfacing validator findings;
  nearest is `cargo test -p rs_cam_core -q --test gcode_validator_baseline`
  (validator correctness only, not surface wiring).
- owner: blank

### EDG-07 Cycle-time and predicted-feed integrators duplicate the same pass
- kind: design
- pattern: god function / duplicated seam
- where: `crates/rs_cam_core/src/machine/kinematics.rs:754-901`
  (`compute_cycle_time_breakdown`, ~148 lines of logic before its trailing
  doc/test material) and `crates/rs_cam_core/src/machine/kinematics.rs:952-1041`
  (`predicted_feeds_for_toolpath`, ~90 lines)
- evidence: both functions build an identical private `struct MoveDigest`
  (fields `length`, `dir`, `v_cmd_mm_s`, `v_ceiling_mm_s`, `accel`,
  `is_rapid`, differing only in `intent` vs. `source_index`) via the same
  loop (`for i in 1..toolpath.moves.len()` computing `chord_length`,
  `unit_vec`, `cruise_ceiling`, `kinematics.effective_accel`), then the
  same junction-velocity/`solve_move` walk. `predicted_feeds_for_toolpath`'s
  own doc admits it: "Walks the same pairwise integrator F-034's
  `compute_cycle_time` uses" (line 968). `rg -c "struct MoveDigest"
  machine/kinematics.rs` → 2.
- proposal: extract one `fn digest_moves(...) -> Vec<MoveDigest>` (digest
  carrying both `intent` and `source_index`) and one
  `fn walk_junctions(&[MoveDigest], kinematics, impl FnMut(usize,
  f64/*dt*/, f64/*peak*/))` shared by both callers; each caller supplies its
  own accumulation closure (bucket-by-intent vs. peak-velocity map).
- breaks: none — both functions keep their public signatures.
- effort: M
- risk: medium — this is the load-bearing trapezoidal-profile cycle-time
  model (F-034/F-035); a refactor must not change any emitted number.
- sentry: `cargo test -p rs_cam_core -q --test kinematics_per_axis_rate_p1`;
  add a differential test asserting `compute_cycle_time_breakdown`'s
  per-move `dt` and `predicted_feeds_for_toolpath`'s per-move peak agree
  on `v_in`/`v_out`/`v_ceiling` before/after the extraction.
- owner: blank

### EDG-05 Three dead `pub` structs in the group's spine/machine files
- kind: design
- pattern: dead pub surface
- where: `crates/rs_cam_core/src/machine/kinematics.rs:175` (`GrblImport`),
  `crates/rs_cam_core/src/machine/kinematics.rs:488` (`MoveKinematics`),
  `crates/rs_cam_core/src/mesh.rs:24` (`WindingReport`)
- evidence: `planning/structure_2026-09-17/evidence_round2/dead_pub_surface.md`
  lists all three as `pub` with no external reader; read at the cited lines
  to confirm the struct is defined and exported but not referenced outside
  its own module (evidence file is 2026-09-17, same day as this audit).
- proposal: drop `pub` (or delete if truly unread even inside the module)
  on each; re-check with `rg -n "GrblImport|MoveKinematics|WindingReport"
  --type rust` before removing to confirm no MCP/CLI/GUI caller was missed
  by the automated sweep.
- breaks: public API surface for three types nothing external constructs —
  no legacy support obligation (operator ruling 2026-09-16).
- effort: S
- risk: low
- sentry: `cargo test -p rs_cam_core -q --test kinematics_per_axis_rate_p1`
  covers `machine/kinematics.rs` broadly; run it after de-pub-ing.
- owner: blank

## Top three

1. **EDG-06** (MCP export drops machine-safety findings) — smallest effort
   (S, additive text change to one MCP tool handler) against the highest
   real-world consequence: a safety/correctness validator already runs on
   every MCP export today and its `Severity::Error` findings are computed
   and then thrown away except into a server log line the calling agent
   never reads. Fixing it is a few lines in `app/mcp.rs`.
2. **EDG-04** (duplicate point-to-segment distance) — S effort, one caller
   to redirect, deletes a private near-copy of a spine primitive, and gives
   `metrology/` one fewer geometry algorithm to keep in sync with `geo.rs`
   by hand.
3. **EDG-01** (folklore Kc reported at literature-Kc confidence) — M effort
   but the highest-leverage correctness gap: three of eleven `WoodSpecies`
   arms feed the deflection/power gates a made-up-but-plausible number with
   no way for a caller (or an operator reading a diagnostic) to learn it
   isn't a citation, in a crate whose own `diagnostics/CLAUDE.md` invariant
   is "`None` means not measured. `Some(0.0)` means measured clean." — this
   is that same confusion in a different layer.

## Checked and clear

- Post-processors (GRBL/grblHAL/LinuxCNC/Mach3) are data (`PostDefinition`
  parsed from `posts/*.toml`), not four emitter branches — `gcode/post.rs`
  and `gcode/mod.rs:972-1035`'s single `from_token`/`to_token` resolver pair
  (W9/P-1 already fixed the old four-hand-written-matches bug).
- `tool/`'s five cutter families dispatch through one `Box<dyn
  MillingCutter>` trait object (`tool/mod.rs:230`, `tool/mod.rs:571`), not an
  enum matched around the crate; `feeds::CutterKind` is a separate,
  power-session-owned mapping enum for the feeds/vendor-LUT side and out of
  scope here.
- `diagnostics/` is genuinely one `Diagnostic` schema with typed
  `Severity`/`Category`/`Confidence`/`DiagnosticState` and a `DiagnosticId`
  newtype whose values all come from named constants in `ids.rs` (76 of
  them) — no adapter builds an ad-hoc string ID.
- `io/tool_library.rs` and `io/machine_library.rs` share directory mechanics
  via `named_toml_library.rs`'s `LibraryError` trait but deliberately keep
  two error enums (message text differs); this is a documented, intentional
  split, not debt.
- `interrupt.rs`'s `CancelCheck`/`Cancelled`/`NeverCancel` is already the
  single cancellation idiom (its own doc says it replaced 26 hand-rolled
  `|| false` closures); the crate's other `cancel` usages (`compute::execute`,
  `adaptive/spiral.rs`, etc.) all route through it or `AtomicBool` polled the
  same way — no second mechanism found in this group's files.
- `measurement.rs`'s `MeasurementProvenance`/`ProjectedXyAreaMm2`/
  `SurfaceAreaMm2` newtypes are live, consumed by 8 files across
  `finish/`, `surface/`, `maps/` and `compute/config.rs` — not a designed-
  but-unwired mechanism.
- `util::build_info` constants are surfaced identically on all three
  product surfaces (CLI `--version`, GUI title bar, MCP `build_info` block)
  — no missing-surface gap.
- The apparent duplicate between `diagnostics/adapters/from_static_checks.rs`
  (`depth_beyond_stock`, lines 411-445) and
  `rs_cam_viz/src/ui/properties/operations/mod.rs` that
  `dup_sweep_0.88_src.md` flags at L2387-2454 is stale: that viz file is 766
  lines today (not 2430+), and `from_static_checks.rs`'s own doc comment
  says F1.18 already moved this predicate out of the GUI so "one rule
  serves every surface" — the dup sweep predates that move or measured a
  version before the file was split.
- `tool/`'s per-family `MillingCutter` impls (`ball.rs`, `bullnose.rs`,
  `flat.rs`, `tapered_ball.rs`) score 0.88-0.92 similar in the structure
  sweep, but the shared shape is expected trait-impl boilerplate (trivial
  `diameter`/`length`/`helix_deg` getters over each struct's own fields)
  around a genuinely distinct `chip_geometry` calculation per family; not
  worth collapsing further without a field-composition change that would
  touch every cutter constructor.

## Add-a-thing count

Main extension point: adding a fifth G-code post-processor (controller
dialect). Files an engineer edits today:

1. `crates/rs_cam_core/posts/<name>.toml` — new post-processor data file
   (new).
2. `crates/rs_cam_core/src/gcode/post.rs` — add
   `<NAME>_TOML`/`static <NAME>: OnceLock<PostDefinition>` and a
   `pub fn <name>() -> &'static PostDefinition` accessor (mirrors
   `grbl()`/`grblhal()`/`linuxcnc()`/`mach3()`, lines 328-373).
3. `crates/rs_cam_core/src/gcode/mod.rs` — add the variant to `PostFormat`
   (line 956) and one arm each to `ALL` (974), `label` (982), `definition`
   (991), `to_token` (1002), `from_token` (1027).
4. A test post-round-trip case, e.g. in
   `crates/rs_cam_core/tests/post_format_round_trip_p1.rs`.

Three files (plus a new data file and a test) — smaller than most
add-a-strategy surfaces in this codebase, and the emitter itself
(`gcode/emitter.rs`) needs no change, which is the folder's own invariant
holding up under inspection.

