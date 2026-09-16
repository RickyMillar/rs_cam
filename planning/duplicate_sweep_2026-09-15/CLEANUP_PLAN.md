# CLEANUP_PLAN.md — duplicate sweep remediation
Generated 2026-09-16 from the 10 investigation findings + review pass in
`findings/` (I01–I10 + REVIEW_PASS.md). Source sweep evidence:
`candidates.json`, protocol: `INVESTIGATION_PLAN.md`.

Work items are ordered: **drift fixes first (live bugs), merges second,
deletions and documentation last.** Each item names its finding, home, risk,
proof test and gate. Do not reorder Phase 1 items — each is independent and
bug-shaped; land them one per commit.

## Execution corrections (2026-09-16, orchestrator verification)

Read these before any item. They override the item text below where the
two disagree.

- **Ruling: no legacy support.** Core writes `format_version = 3` and
  stores alignment pins on the stock. All 102 project TOML files in the
  repository are `format_version = 3`. Any code that reads a pre-v3 shape
  is legacy support and is deleted, not ported.
- **C01** must also delete `build_session_from_legacy_job` and
  `kind_from_extension` in `controller/io.rs` (private fns; the
  `-D warnings` gate fails on orphans). `kind_from_extension` has one
  other caller (`controller/io.rs:294`); point it at core
  `io::infer_kind_from_path` (make it `pub` if it is not).
- **C02** shrinks after C12. Core `default_fixture_size` (30.0) serves
  both `size_x` and `size_y`; add `default_fixture_size_y` = 15.0 for
  `size_y` only and make `controller/events/model.rs:543` use the core
  defaults instead of hard-coded numbers. `size_x` stays 30.0.
- **C03 is rescoped.** Do NOT port the setup-level pin migration. Core
  refuses a project whose `format_version` is not 3 (a missing key
  defaults to 1 today, `project_file.rs:42`, so a missing key is refused
  too) with a clear `SessionError`. Proof: a v2 TOML with setup-level
  pins returns `Err`. Test fixtures that lack the key gain
  `format_version = 3`.
- **C06 is answered.** Both `walk_rows` copies drop rows at the same site
  on cancel. Reach's `row_fn` returns `Vec<T>` and never folds
  `DROP_CALLS` (mesh drop probes, not rows). No latent perf regression.
  Merge; reach rows contribute zero drops.
- **C12**: `state::job::Setup` is LIVE (`Setup::for_transforms` at
  `app/gpu_upload.rs:198/376/438`, `app/simulation.rs:567`) as a carrier
  for `transform_mesh`, `transform_heightmap_mesh`,
  `height_context_from_session` and the `session_*_bbox` helpers. Delete
  `JobState`, `Fixture`, `KeepOutZone` and the legacy-only `Setup`
  fields; change the transform helpers to take
  `&rs_cam_core::session::Setup` (or `(FaceUp, ZRotation)`) and delete
  `for_transforms`.
- **C13 gate**: the full heavy gate is forbidden (operator ruling
  2026-09-11). Use the named sentries `model_units_survive_reload_g_unitsreload`,
  `step_project_load`, `model_path_round_trip_g_modelrelink`,
  `multitool_plan_roundtrip_o1b` plus `cargo test -p rs_cam_core --lib session`.
- **C00**: chunks come from Qdrant with path + line range only. Read each
  `src` file from disk; the first `#[cfg(test)]` followed by `mod` to EOF
  is test scope. Report `src` pairs and test pairs separately.
- **C99** needs a fresh index: call SocratiCode `codebase_update` and
  confirm the chunk count moved before the re-run.
- **Follow-up candidate (not in scope)**: `project_file.rs` still carries
  `_legacy_feeds_auto` (:447), a one-shot dressup migration (:708) and a
  legacy `machine_ref` drop (:1119). These are in-core shims for v3
  files. Decide separately.

## Phase 0 — tooling (no product code)

- [x] **C00 — sweep-script test filter** (I05 item 3, review gap).
  `scripts/duplicate_sweep.py`: drop chunks inside inline `#[cfg(test)]`
  modules so the re-run verification (C99) is meaningful. Owner: any.

## Phase 1 — live-bug drift fixes (highest priority, independent)

- [ ] **C01 — delete the legacy fallback entirely** (I01 step 1;
  **DECIDED 2026-09-16: no legacy-format support — rs_cam is still in
  development, old project files will not be supported**). Delete the
  catch-every-`SessionError` fallback at `controller/io.rs:526-540`, the
  viz `load_project` fallback path, and let `ProjectSession::load` errors
  propagate (non-CAM TOML and pre-v1 files now simply fail to open, which
  is the intended behavior). No pre-v1 marker check needed — unsupported
  formats just stop loading. Proof: viz test that a non-CAM TOML and a
  pre-v1 file both return `Err` from `open_job_from_path`. Risk: low.
  Gate: `cargo test -p rs_cam_viz -q`.
- [ ] **C02 — fixture `size_y` default reconciliation** (I05 pair 4).
  Decide the correct default between viz `Fixture::new_default` 15.0
  (`state/job.rs:91`), core serde 30.0 (`session/mod.rs:544-546`), and the
  re-hardcode at `controller/events/model.rs:543`. **DECIDED 2026-09-16: use the GUI value, 15.0** — core serde default and all copies become 15.0. Legacy files missing
  `size_y` restore different geometry than the GUI creates. This is a
  user-facing decision — confirm which value is intended before landing.
  Risk: med (changes restored geometry for existing projects). Gate:
  legacy round-trip tests.
- [ ] **C03 — setup-pin migration into core** (I01 step 2, silent data loss).
  Add `alignment_pins: Vec<ProjectPinSection>` (serde default +
  skip_serializing_if) to core `ProjectSetupSection` (`project_file.rs:288`),
  porting the ≤v2 pins→stock migration from viz `io/project.rs:745-764`.
  Proof: new core sentry `project_loader_parity_i01.rs` (I01's TOML fixture
  asserting pins, datum, post format, tool type, resolved model path match
  between `ProjectSession::load` and the fallback) — fails today, passes
  after. Risk: low. Gate: `cargo test -p rs_cam_core -q`.
- [ ] **C04 — CLI sweep tool-field loss** (I10 pair 4, review-corrected).
  `sweep.rs:360-410` `SerializableToolDef` drops `shank_diameter`,
  `shank_length`, `holder_diameter`, `holder_length` from baselines. Restore
  the four fields; make `CliToolType: Serialize` (string form must match the
  Deserialize aliases). Better: derive `Serialize` on `job.rs`'s schema and
  delete the mirror entirely if string forms align. Risk: med (sweep
  baselines change — coordinate with anyone mid-sweep). Gate:
  `cargo test -p rs_cam_cli -q`.
- [ ] **C05 — artifact writers: collision-safe naming** (I04, review-confirmed).
  New `crates/rs_cam_core/src/artifact_io.rs`: host
  `sanitize_filename_component` (byte-identical ×3 today) and one writer
  using `simulation_cut.rs:1758-1788`'s pid+`WRITE_SEQ` naming (the August
  collision fix). Fold `debug_trace.rs:530-568` and `semantic_trace.rs:1205-1243`
  onto it. Keep the prune contract. Risk: low-med. Gate:
  `cargo test -p rs_cam_core -q` + artifact-writing sentries.
- [ ] **C06 — `walk_rows` drop-site answer** (I02, review condition).
  Before merging tier/reach `walk_rows`: determine whether reach's
  different drop site is a latent perf regression. If yes, fix in reach
  first; if no, merge with `walk_rows(Option<&AtomicU64>)` keeping
  tier_map's DROP_CALLS fold (tier_map authoritative). Risk: med.
  Gate: reach/tier perf sentries + `cargo test -p rs_cam_core -q`.

## Phase 2 — I/O consolidation (I01, sequenced; depends on C01)

With legacy support removed (C01), this phase is pure deletion + API
alignment: core becomes the single project I/O layer with no converter to
maintain.

- [ ] **C10 — additive core API** (I01 step 3). `pub` the model-kind helper
  as `io::infer_kind_from_path`; delete `project_file::infer_model_kind`,
  viz `io/project.rs::infer_model_kind`, and
  `controller/io.rs::kind_from_extension`. Add a typed
  `Vec<ProjectLoadWarning>` channel to `from_project_file` so viz keeps its
  toasts. Optional: `visible`/`locked`/`auto_regen` on core's toolpath
  section if the GUI wants them persisted (decide). Risk: low (additive).
- [ ] **C11 — delete the viz ProjectFile family** (I01 step 4). Delete viz
  `save_project` (test-only, DRIFTED_DUP-dead) and `load_typed_project` +
  struct family. Port the `boundary_controls_always_visible_g_boundaryinherit.rs:34`
  import to core's type (review gap). Move the legacy round-trip tests
  (`io/project.rs:1824-1911`) to the new converter before deleting.
  Risk: med. Gate: `cargo test -p rs_cam_viz -q`.
- [ ] **C12 — delete the legacy format support outright** (I01 step 5;
  **DECIDED 2026-09-16: no `project_legacy` converter needed**). Delete
  viz `LegacyProjectFile`, `load_legacy_project`, `load_legacy_model`,
  `restore_project_*`, `build_session_from_legacy_job`
  (`controller/io.rs:626-780`) and the entire
  `state::job::{JobState, Setup, Fixture, KeepOutZone}` second data model
  (I05 pairs 4+5 become deletions, per review resolution 2). Legacy
  fixture files under `tests/fixtures/` are deleted with it. Add
  `bbox`/`clearance_bbox`/`to_key` to core `session::{Fixture, FixtureKind}`
  only if live viz code still needs them after the deletion. Risk: med
  (delete surface), low (no replacement to get right). Gate:
  `cargo test -p rs_cam_viz -q`.
- [ ] **C13 — collapse the two in-core model doors** (I01 step 6, drift item 9).
  `io::load_model_file` vs `project_file::load_model_geometry`: single door,
  keeping `winding_report` and the four documented divergences as one
  behavior. Risk: med-high (G-UNITSRELOAD/G-STEPUNITS territory). Gate:
  `cargo test -p rs_cam_core --features heavy-tests --no-fail-fast -- -q`.

## Phase 3 — mechanical merges (low risk, parallelizable after Phase 1-2)

- [ ] **C20 — `ui/components/format.rs`** (I06a + I07 + I10 pair 1; review
  resolution 1 — ONE home). Move `slugify` (from `app/export.rs:260` and
  `ui/export_wizard.rs:304`), `format_cycle` (`optimize_modal.rs:1152` /
  `optimize_project.rs:537`), `format_delta` (`optimize_modal.rs:953` /
  `optimize_project.rs:516`). Risk: low. Gate: `cargo test -p rs_cam_viz -q`.
- [ ] **C21 — `controller_compensation_for` mapping to core gcode**
  (I06b). Make core `gcode`'s (side×climb)→G41/G42 mapping `pub`; viz
  `io/export.rs:345` delegates. Sentries: F16 G41→G40 round-trip +
  `program_builder.rs` comp tests. Risk: low.
- [ ] **C22 — I05 mechanical pair merges.** (a) make core `polygons_bbox`
  `pub`, delete `state/job.rs:436-463`, adapt the 3 call sites for the
  `Option<&[Polygon2]>` signature (review gap 7); (b) single
  `SimBoundary` type re-exported, delete `compute/worker.rs:204-212` and viz
  `ToolpathBoundary` + 3 map sites; (c) rewrite viz inline test fixtures with
  `..SimulationCutSample::test_fixture()`. Risk: low-med. Gate:
  `cargo test -p rs_cam_viz -q`.
- [ ] **C23 — core memo module** (I02). `crates/rs_cam_core/src/memo.rs`
  (`MeshMemo<K, V, const CAPACITY>`); migrate `tier_map_cache`,
  `reach_map_cache`, `geom_cache`, `finish_surface_cache` (4th member found
  by I02). Keep per-cache CAPACITY/key shapes (deliberate). Shared
  `grid.rs`: `GridSpec` + merged `walk_rows` (after C06). Also fix the stale
  reach_map_cache doc (key DOES include tool/model ids). Risk: med. Gate:
  core cache sentries (`tier_map_cache_t3` etc.).
- [ ] **C24 — resampler merge** (I08 pair A). Home `crate::geo`; pencil.rs's
  `resample_polyline` authoritative (spacing floor; capacity-safe); project_curve
  delegates. **Run the project_curve tests against the shared function first**
  (review gap: I08's hand-trace is not evidence). Also delete pencil's
  private `polyline_length` dupe if the shared helper covers it. Risk: low-med.
- [ ] **C25 — ops helper merges** (I08 pairs E/F). `runtime_annotations_to_labels`
  via a 2-method trait in `compute/spans.rs` (6 copies: adaptive, adaptive3d,
  scallop, pencil, ramp_finish, spiral_finish). Risk: low.
- [ ] **C26 — vendor-normalize delegation** (I09 P2). `context.rs:152-168`
  delegates to `vendor_normalize::op_family_to_lut` (byte-identical 8-arm
  match; a drift-tripwire test already exists — keep it). Risk: low.
- [ ] **C27 — dexel dead-path deletion** (I03). Delete
  `z_grid_to_solid_mesh_heightmap` + `find_matching_gap` +
  `UNCUT_*/CUT_*` legacy copies (~430 lines; `dexel_mesh_mc.rs` is the
  marching-cubes implementation, `z_grid_to_solid_mesh` already delegates).
  Update the three stale doc references: `rest_heatmap_mesh.rs:18`,
  `planning/PROGRESS.md:903`, `planning/DEXEL_Z_ONLY_INVESTIGATION.md:25`
  (review gap 4). Risk: low. Gate: `step5_marching_cubes` +
  `lateral_scrub_playback_stock_g_lateralscrub`.
- [ ] **C28 — bench helper** (I10 pair 5). Single `rolling_field` in a
  benches helper. Risk: low.

## Phase 4 — documentation / no-action items

- [ ] **C40 — SIBLING/NO-ACTION records**: I08 pairs B/C/D, I09 P1/P3,
  I10 pairs 2/3, I05 pair 2, adaptive↔adaptive3d engines (I08 F), the
  finish_surface_cache 4th member note. Each already documented in its
  finding file; CLEANUP_PLAN references them as the answer to
  "why didn't you merge X?".
- [ ] **C99 — verification re-run.** After the phases land:
  `python3 scripts/duplicate_sweep.py --threshold 0.92` (with C00's test
  filter). Expect the src pair count to drop from 59 to the ~15 pairs that
  are SIBLING/NO-ACTION. Attach the report here.

## Notes for the executing swarm

- Phase 1 items are independent — parallelize freely; one commit each.
- Phases 2 and 3 items depend on named predecessors (C10→C11→C12; C06→C23);
  otherwise parallel.
- Every merge must keep its sentry green (named per item). Zero-warning
  clippy gate (`cargo clippy --workspace --all-targets -D warnings`) at the
  end of each phase.
- I05's `apply(Command)` contract was verified intact — cleanup must not
  introduce a `*_mut` shortcut while moving `state/job.rs` code.
- Line numbers are from the 2026-09-16 review pass; re-locate by content if
  they drift during execution.
