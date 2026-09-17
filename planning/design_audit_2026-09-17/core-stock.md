# core-stock — design and feature-debt audit

Group: `crates/rs_cam_core/src/stock/` (14 files, ~10k lines) and
`crates/rs_cam_core/src/dexel_stock/` (9 files, ~9k lines).
Auditor: read-only pass, 2026-09-17.

### STK-01 Six hand-copied cell-scan loops in the stamping kernel
- kind: design
- pattern: duplicate kernel bodies
- where: `crates/rs_cam_core/src/dexel_stock/stamping.rs:607` (`stamp_point_on_grid`), `:727` (`stamp_segment_on_grid`, swept), `:922` and `:1004` (`stamp_segment_on_band`, degenerate and swept), `:1383` and `:1529` (`stamp_segment_with_metrics`, degenerate and swept)
- evidence: `rg -n 'lut_h_with_edge_fallback' stamping.rs` finds six call
  sites and `rg -n 'FULL_COVERAGE'` finds six, one per loop. Each loop
  repeats the same seven steps in the same order: `scan_radius = radius +
  cs * SUBSAMPLE_HALF_EXTENT * SQRT_2`, `clamped_cell_bbox`, the mip
  `cell_can_remove` whole-stamp skip, the per-cell `coverage <= 0.0`
  reject, the per-cell `air_skip` reject, `lut_h_with_edge_fallback`, then
  the `from_high` fork over `ray_blend_above` / `ray_blend_below` and the
  `coverage >= FULL_COVERAGE` conservative-top update.
- proposal: extract one `for_each_covered_cell(target, lut, geometry,
  from_high, mip, |idx, coverage, dist_sq, depth| …)` driver that owns the
  scan box, the mip skip, the coverage reject and the conservative-top
  update, and let the four public kernels supply only the per-cell closure
  (nothing, volume accumulation, band bookkeeping). The band and the grid
  already differ only in `local(row, col)` versus `row * cols + col`, so one
  small `StampTarget` trait over `rays` / `conservative_top` / origins
  covers both.
- breaks: none (all four functions are `pub(super)`)
- effort: L
- risk: medium — this is the hot loop and the metric route must stay
  bit-comparable with the playback route.
- sentry: `playback_band_dispatch_s6` and `band_stamping_determinism_s3`
  already compare the two routes; add the existing
  `assert_grids_bit_identical` helper (`stamping.rs:1896`) as the extraction
  gate.
- owner:

### STK-02 Three copies of the grid constructor, one per axis
- kind: design
- pattern: enum dispatch replicated
- where: `crates/rs_cam_core/src/stock/dexel.rs:372` (`z_grid_from_bounds`), `:397` (`x_grid_from_bounds`), `:422` (`y_grid_from_bounds`)
- evidence: the three bodies are structurally identical. They differ only in which bbox extent
  feeds `clamp_cell_size`, which two components become `origin_u`/`origin_v`,
  which component spans the segment, and the `DexelAxis` literal. Each ends
  with the same five-line `Self { rays, rows, cols, origin_u, origin_v,
  cell_size, axis, conservative_top }`.
- proposal: keep the three names as one-line wrappers over
  `DexelGrid::from_bounds(bbox, cell_size, axis)`, which reads the axis
  permutation from a single `match axis` returning `(extent_u, extent_v,
  origin_u, origin_v, ray_lo, ray_hi)`. `StockCutDirection::decompose`
  (`dexel_stock/cut_direction.rs:49`) already holds the same permutation
  table for points; the two tables should be the same `match`.
- breaks: none if the three wrappers stay
- effort: S
- risk: low — pure refactor of constructors; the permutation is already
  written down twice and can be diffed. See STK-13: two of the three
  constructors have no production caller, so deletion may beat unification.
- sentry: `dexel_stock_z_frame_f024`; add a round-trip test asserting
  `decompose` and the grid's `(origin_u, origin_v)` agree for all six
  `StockCutDirection` variants.
- owner:

### STK-03 `axial_doc_mm` and `axial_engagement_mm` always carry the same number
- kind: design
- pattern: one concept with two representations
- where: `crates/rs_cam_core/src/dexel_stock/simulation.rs:1100`-`1101`; the fields at `crates/rs_cam_core/src/stock/simulation_cut.rs:186` and `:190`
- evidence: the only production emitter writes both from the same local:
  `sample.axial_doc_mm = axial_engagement_mm;` /
  `sample.axial_engagement_mm = axial_engagement_mm;`. `rg -n
  'axial_doc_mm\s*[:=]'` over `src/` finds no other assignment on a
  `SimulationCutSample`. The split the doc comment describes (plunge versus
  lateral) is already carried by the third field `plunge_descent_mm`, which
  the same `if` computes.
- proposal: delete `axial_doc_mm` from `SimulationCutSample` and rename every
  reader to `axial_engagement_mm`. Two axial channels — lateral engagement
  and plunge descent — is the model the emitter actually implements; the
  third name is the pre-split wire name and nothing distinguishes it.
- breaks: the `axial_doc_mm` wire key in the simulation-cut trace artifact
  (`SIMULATION_CUT_TRACE_SCHEMA_VERSION` 5 → 6) and the MCP diagnostic that
  republishes it. Ruled acceptable 2026-09-16.
- effort: S — `rg -c '\.axial_doc_mm\b|axial_doc_mm:'` outside `tool/`, `feeds/` and `tool_load/` finds 169 references in 49 files.
- risk: low — a mechanical rename; a missed site fails to compile.
- sentry: `engagement_denominator_m3`; the rename is guarded by the
  compiler.
- owner:

### STK-04 `leading_edge_speed_mm_min` is a copy of `feed_rate_mm_min` and no gate reads it
- kind: feature-debt
- pattern: dial nothing reads
- where: `crates/rs_cam_core/src/stock/simulation_cut.rs:124`-`126`; emitter `crates/rs_cam_core/src/dexel_stock/simulation.rs:1119`
- evidence: the field's doc says "For 3-axis lateral moves this is
  `feed_rate_mm_min`. Used by the chipload gate." The single assignment is
  `leading_edge_speed_mm_min: sample.feed_rate_mm_min` — unconditional, no
  lateral/other fork. `rg -n 'leading_edge_speed_mm_min'` over `src/` finds
  exactly four sites: the field, the emitter, the time-weighted average in
  `simulation_cut/accumulate.rs:63`, and the MCP JSON at
  `crates/rs_cam_viz/src/app/mcp/simulation.rs:1144`. No gate, no feeds
  path, no triage rule reads it.
- proposal: either delete the field and its `average_…` summary (the reading
  is `feed_rate_mm_min`, already published), or make it earn the name by
  computing the edge speed on helix and arc moves, where it genuinely
  differs from the commanded feed. Do not leave a field whose doc names a
  consumer that does not exist.
- breaks: the `leading_edge_speed_mm_min` / `average_leading_edge_speed_mm_min`
  wire keys and a golden field in `tests/perf_golden_sim_metrics.rs:167`.
- effort: S
- risk: low
- sentry: `tests/engagement_vector_step2.rs:147` constructs it; extend that
  test to assert the field differs from `feed_rate_mm_min` on a helix
  sample, or delete both together.
- owner:

### STK-05 `EngagementDirection` has three variants and production emits one
- kind: feature-debt
- pattern: half-built capability
- where: `crates/rs_cam_core/src/stock/simulation_cut.rs:65`-`70`; emitter `crates/rs_cam_core/src/dexel_stock/simulation.rs:1124`
- evidence: `rg -n 'EngagementDirection::' crates --glob '*.rs'` returns two
  lines in the whole workspace — the emitter's `::Mixed` and one test
  fixture's `::Mixed`. `Climb` and `Conventional` are never constructed, and
  `rg -n 'engagement\.direction'` over `stock/` and `dexel_stock/` returns
  nothing: the field is never read either. The emitter comment says the
  discrimination "needs perp-axis side info from stamping … to be threaded
  in a follow-up"; `stamp_segment_with_metrics` does compute `perp_min` /
  `perp_max` (`stamping.rs:1046`) and throws the side information away.
- proposal: either thread the sign of the engaged perp extreme into
  `direction` (the accumulator already exists) so climb/conventional
  becomes a real reading, or delete the field and the enum. Climb versus
  conventional is a claim an operator would act on; shipping it as a type
  that only ever says "Mixed" is worse than not shipping it.
- breaks: the `direction` wire key on `Engagement` if deleted. NB this is
  `Engagement::direction`, not `SimGroupEntry::direction`, which the brief
  rules live.
- effort: M if implemented, S if deleted
- risk: low
- sentry: `tests/engagement_vector_step2.rs`; add an assertion that a
  known-climb raster pass reports `Climb`.
- owner:
### STK-06 Two open-coded `FaceUp` → `StockCutDirection` maps disagree with the canonical one
- kind: design
- pattern: one concept with two representations
- where: `crates/rs_cam_core/src/session/compute/simulation.rs:82`, `crates/rs_cam_core/src/session/compute.rs:1862`; the canonical accessor at `crates/rs_cam_core/src/compute/transform.rs:527`
- evidence: both call sites write the same two-arm match:
  `FaceUp::Bottom => StockCutDirection::FromBottom, _ =>
  StockCutDirection::FromTop`. `SetupTransformInfo::cut_direction()` has six
  arms and returns `FromFront` / `FromBack` / `FromLeft` / `FromRight` for
  the four lateral faces. The two hand maps therefore report a lateral setup
  as `FromTop` while the canonical accessor reports the lateral variant.
  The transform doc at `:493` records two earlier defects in exactly this
  table.
- proposal: call `SetupTransformInfo::cut_direction()` at both sites. If the
  group stock must stay `FromTop` for a lateral setup, name that rule in one
  function, for example `SimGroupEntry::stamp_direction()`. Do not leave the
  rule as an untitled `_` arm in two files.
- breaks: `SimGroupEntry.direction` starts to carry the lateral variant. Its
  one production reader is the S5 prefix hash,
  `crates/rs_cam_core/src/compute/sim_prefix.rs:425`
  (`format!("{:?}", group.direction).hash(&mut hasher)`), so a lateral setup
  takes one cache miss on the first run after the change. No metric and no
  viz branch moves: the per-setup stock is stamped `FromTop` unconditionally
  (`compute/simulate.rs:1006`), and the viz lateral test at
  `crates/rs_cam_viz/src/compute/worker/execute/mod.rs:161` reads
  `playback_direction`, which comes from `local_to_global`, not from this
  field.
- effort: S
- risk: medium — the S5 prefix cache keys on this value.
- sentry: `cut_direction_matches_transform_g_lateralsign`. Extend it to
  assert that the session-built `SimGroupEntry.direction` equals
  `SetupTransformInfo::cut_direction()` for all six faces.
- owner:

### STK-07 `Engagement` restates two scalars the sample already carries
- kind: design
- pattern: one concept with two representations
- where: `crates/rs_cam_core/src/dexel_stock/simulation.rs:1102`, `:1103`, `:1117`; the fields at `crates/rs_cam_core/src/stock/simulation_cut.rs:197`, `:201`, `:111`
- evidence: one emitter writes each pair from one local.
  `sample.arc_engagement_radians = arc_engagement_radians` and
  `arc_radians: arc_engagement_radians` take the same variable.
  `sample.effective_chip_thickness_mm = chip_stats.map(|s| s.mean_mm)` and
  `mean_chip_thickness_mm: chip_stats.map(|s| s.mean_mm)` take the same
  expression. The `Engagement` doc at `:99` states the identity itself:
  "Same value the chipload gate reads off
  `SimulationCutSample::effective_chip_thickness_mm`".
- proposal: pick one home per quantity. `Engagement` is the stated canonical
  vector, so delete `arc_engagement_radians` and
  `effective_chip_thickness_mm` from `SimulationCutSample` and point their
  readers at `sample.engagement`. Together with STK-03 this removes four
  duplicated scalars from a 24-field struct.
- breaks: the `arc_engagement_radians` and `effective_chip_thickness_mm`
  wire keys. `SIMULATION_CUT_TRACE_SCHEMA_VERSION` 5 → 6.
- effort: M
- risk: low — the compiler finds every reader.
- sentry: `tests/chipload_formula_calibration.rs` and
  `tests/engagement_vector_step2.rs` both read the pair.
- owner:

### STK-08 `peak_chip_thickness_mm` turns a measured zero into "not measured"
- kind: feature-debt
- pattern: abstention reports the wrong thing
- where: `crates/rs_cam_core/src/stock/simulation_cut/accumulate.rs:97`-`99`; the honest sibling at `:39`-`:40`
- evidence: the accumulator publishes
  `peak_chip_thickness_mm: if self.peak_chip_thickness_mm > 0.0 {
  Some(self.peak_chip_thickness_mm) } else { None }`. The sibling
  `peak_axial_doc_fraction` uses an `Option`-preserving max:
  `Some(self.peak_axial_doc_fraction.map_or(axial, |p| p.max(axial)))`. The
  struct doc at `crates/rs_cam_core/src/stock/simulation_cut.rs:361` states
  the contract the sentinel breaks: "`Some(0.0)` means measured and zero."
  One struct carries two contracts.
- proposal: track an observed flag or an `Option` accumulator for the peak,
  exactly as `peak_axial_doc_fraction` does. Then a class that measured a
  true zero peak reports `Some(0.0)`.
- breaks: none on the wire. A consumer that reads `None` as "no chip" sees
  `Some(0.0)` instead.
- effort: S
- risk: low
- sentry: `measurability_abstention_r8`. Add a case that feeds one cutting
  sample with `peak_chip_thickness_mm: Some(0.0)` and asserts the summary
  reports `Some(0.0)`.
- owner:

### STK-09 `whole_path.rs` and `playback.rs` are two copies of one batch dispatcher
- kind: design
- pattern: duplicate module
- where: `crates/rs_cam_core/src/dexel_stock/whole_path.rs` (636 lines) and `crates/rs_cam_core/src/dexel_stock/playback.rs` (580 lines)
- evidence: the two files define eleven methods with the same names in the
  same order — `dispatch_override`, `default`, `resolved`, `for_grid`,
  `push`, `build_buckets`, `batch_is_due`, `is_empty`, `stats`, `run_batch`,
  `clear_batch`. Both structs hold `jobs`, `buckets`, `outs`, `reduced`,
  `bands`, `active_rows`, `pending_partials`, `pending_visits`,
  `visit_budget`, `stats`. Five constants share their names, and four share
  their values: `MAX_JOBS_PER_BATCH` 8_192, `MAX_PARTIALS_PER_BATCH`
  262_144, `BATCH_VISIT_BUDGET_PASSES` 4, and the band floor 4.
  `MIN_JOBS_PER_BATCH` is the one real difference: 64 against 16.
- proposal: make one `BandBatch<P: BandPartial>` generic over the partial
  type and the kernel, with the batch thresholds as constructor arguments.
  The `dexel_stock/CLAUDE.md` invariant says the metric route and the
  playback route must agree on the stamped volume; one dispatcher makes that
  structural instead of reviewed.
- breaks: none — both modules are crate-internal.
- effort: L
- risk: medium — the reduction order fixes the `f64` volume sum. The
  existing note at `stamping.rs:1046` documents that banding reassociates it.
- sentry: `playback_band_dispatch_s6` and `band_stamping_determinism_s3`
  cover both routes today.
- owner:

### STK-10 `CollisionEvent.segment` is a string, and three collision shapes share no type
- kind: design
- pattern: stringly-typed door
- where: `crates/rs_cam_core/src/stock/collision.rs:143` (the field), `:263` and `:397` (the two classifiers), `:423` (`RapidCollision`), `:135` (`CollisionEvent`)
- evidence: the field is `pub segment: String`, and both producers build it
  the same way: `let seg_name = if seg_radius > assembly.shank_diameter /
  2.0 - 0.01 { "holder" } else { "shank" };` followed by
  `.to_owned()`. The only consumers are a `format!` in
  `crates/rs_cam_viz/src/state/simulation/issue_triage.rs:384`, a `format!`
  in `crates/rs_cam_viz/src/app/mcp/simulation.rs:142`, and one
  `assert_eq!(hits[0].segment, "holder")`. Beside it, `RapidCollision`
  (`move_index`, `start`, `end`) shares no field name and no trait with
  `CollisionEvent` (`move_idx`, `position`, `penetration_depth`, `segment`,
  `kind`) — even the move index is spelled two ways.
- proposal: replace `segment: String` with an
  `enum AssemblySegment { Shank, Holder }`, and move the radius comparison
  into one `ToolAssembly::segment_at(radius)`. Then give
  `RapidCollision` and `CollisionEvent` one `move_index` name and one
  `position`, so the triage can treat all three collision sources through a
  single accessor instead of three match arms at
  `issue_triage.rs:337`-`:389`.
- breaks: the `segment` MCP JSON value changes from `"holder"` to
  `"Holder"` unless the enum keeps a `snake_case` serde rename.
- effort: M
- risk: low
- sentry: `collision.rs:1169` asserts the string today; convert it to the
  enum.
- owner:

### STK-11 The sample emitter allocates one `Vec<SpanId>` per sample
- kind: design
- pattern: Vec rebuilt per call
- where: `crates/rs_cam_core/src/dexel_stock/stamping.rs:1679` (inside the subsegment loop of `sample_segment_runtime`), `crates/rs_cam_core/src/dexel_stock/simulation.rs:1059`
- evidence: both sample constructors write `span_path:
  params.span_path.to_vec()`. In `sample_segment_runtime` the `push` sits
  inside `for subsegment in 0..subsegments`, so one heap allocation lands
  per emitted sample. `span_path` comes from `SegmentSampleParams`
  (`stamping.rs:1623`) and `CuttingCaptureParams` (`stamping.rs:513`), both
  built once per move, so every sample of a move copies the same slice.
- proposal: change the field to `Arc<[SpanId]>` and clone the `Arc` per
  sample, or hold one interned `span_path` table on
  `SimulationCutTrace` and store a `u32` index on the sample. `Arc<[SpanId]>`
  is the smaller change and keeps the serde shape.
- breaks: the public field type of `SimulationCutSample`. The JSON wire form
  of an `Arc<[T]>` is unchanged.
- effort: M
- risk: low — the change is type-directed.
- sentry: `benches/hot_paths.rs` already benches the sample path; the sim
  metric goldens in `tests/perf_golden_sim_metrics.rs` guard the values.
- owner:

### STK-12 The GUI rebuilds its own issue list beside `SimulationTriage`
- kind: design
- pattern: one concept with two representations
- where: `crates/rs_cam_viz/src/state/simulation/issue_triage.rs:300`-`:420`; the core answer at `crates/rs_cam_core/src/stock/sim_triage.rs:217`
- evidence: `SimulationTriage` documents itself as "The page-one answer. One
  object, four consumers" and carries `safety`, `actions`, `advisories`,
  `counts`, a `DedupKey`, a severity order (`sort_findings`,
  `sim_triage.rs:1099`) and a measurability gate. The viz file builds a
  parallel `Vec<SimulationIssue>` from four raw streams — hotspots,
  `trace.issues`, `checks.rapid_collision_move_indices` and
  `checks.collision_report.collisions` — with its own `format!` labels, its
  own `issue_kind_rank` order and its own `issue_cache_key` memo. Four UI
  and CLI surfaces read that second list:
  `crates/rs_cam_viz/src/ui/sim_diagnostics.rs:63`,
  `crates/rs_cam_viz/src/ui/sim_op_list.rs:219`,
  `crates/rs_cam_viz/src/ui/sim_timeline.rs:2223`,
  `crates/rs_cam_cli/src/main.rs:350`.
- proposal: make the viz list a projection of `SimulationTriage`. The core
  object already holds severity, category, occurrences and worst evidence;
  the GUI needs only a global move index per finding, which
  `global_move_for_local` supplies today. A second rank in a second crate is
  the exact defect the census D1 note in this same file describes.
- breaks: the GUI list gains core's dedup and advisory caps, so long air-cut
  runs collapse into fewer rows.
- effort: L
- risk: medium — it changes what an operator sees in the timeline.
- sentry: `simulation_issue_channel_m1`; add a viz test that the panel row
  count equals `triage.safety.len() + triage.actions.len() +
  triage.advisories.items.len()`.
- owner:

### STK-13 No product surface ever stamps the X or Y grid
- kind: feature-debt
- pattern: unreachable capability
- where: `crates/rs_cam_core/src/dexel_stock/mod.rs:184` (`ensure_grid`), `crates/rs_cam_core/src/compute/simulate.rs:1022` and `:1205`, `crates/rs_cam_viz/src/compute/worker/execute/mod.rs:160` and `:173`
- evidence: `ensure_grid` is the only door to `x_grid` / `y_grid`, and it
  lazily builds them from `direction.grid_axis()`. Every production caller
  forces `FromTop` on a lateral setup:
  - `compute/simulate.rs:1006` — the per-setup stock: `let direction =
    StockCutDirection::FromTop;`.
  - `compute/simulate.rs:1205` — the global playback stamp runs under
    `if !lateral_playback`, and `lateral_playback` (`:1022`) is exactly
    `!matches!(playback_direction, FromTop | FromBottom)`. So the
    `playback_direction` reaching `simulate_toolpath_with_lut_cancel` at
    `:1230` is always a Z-grid direction.
  - `crates/rs_cam_viz/src/compute/worker/execute/mod.rs:173` — live
    playback: `direction: StockCutDirection::FromTop` on the `lateral` arm.
  - `crates/rs_cam_cli/src/main.rs:684` and
    `crates/rs_cam_cli/src/sweep.rs:332` pass `FromTop` literally.
  `rg -n 'StockCutDirection::(FromLeft|FromRight|FromFront|FromBack)'` finds
  production hits only in `compute/transform.rs:527` (the accessor) and
  `crates/rs_cam_core/src/dexel_stock/mod.rs` tests. The reason is recorded
  at `compute/simulate.rs:303`-`:314` (G-LATERALSCRUB): the side grids are
  appended as open per-segment surfaces with no boolean against the Z solid,
  "so they are drawn inside an intact block and occluded by it".
- proposal: delete `TriDexelStock::x_grid` and `y_grid`, the four lateral
  `StockCutDirection` variants, `ensure_grid`'s X and Y arms,
  `DexelGrid::x_grid_from_bounds` / `y_grid_from_bounds` and
  `dexel_mesh::side_grid_to_mesh`. State in `dexel_stock/CLAUDE.md` that the
  stock is a Z-grid and that a lateral setup is simulated in its own local
  frame. This also settles STK-02: two of the three duplicated constructors
  go away. If lateral stamping is wanted later, the honest form is three
  marching-cubes solids and an intersection, not open side surfaces.
- breaks: `StockCutDirection` loses four variants, so
  `SetupTransformInfo::cut_direction()` and
  `cut_direction_matches_transform_g_lateralsign` both change shape. The
  `FaceUp` model itself is untouched — the setup transform still carries the
  six faces.
- effort: M — a deletion with a wide but compiler-checked surface.
- risk: low — no production path reaches the code being deleted.
- sentry: `cut_direction_matches_transform_g_lateralsign` is the test that
  must be rewritten; the four `stamp_from_*_creates_*_grid_and_cuts` tests
  at `dexel_stock/mod.rs:1062`-`:1156` are the ones that would be deleted
  with it.
- owner:

### STK-14 `stock_mesh.rs` holds toolpath ribbon geometry and per-vertex colours
- kind: design
- pattern: render concern in the core model
- where: `crates/rs_cam_core/src/stock/stock_mesh.rs:86` (`with_dimmed_colors`), `:103` (`height_gradient_colors`), `:181` (`toolpath_to_tube_mesh_with_spans`), `:268` (`toolpath_to_tube_mesh`), `:323` (`auto_ribbon_radius`)
- evidence: five of the file's eleven public items describe a toolpath
  ribbon or a colour ramp, not stock. `height_gradient_colors` returns
  `Vec<[f32; 3]>` and `with_dimmed_colors(factor)` scales RGB. The readers
  are the GPU upload path
  (`crates/rs_cam_viz/src/app/gpu_upload.rs:88`), the overlays panel
  (`crates/rs_cam_viz/src/ui/overlays/panel.rs:444`) and the screenshot
  exporter (`crates/rs_cam_core/src/export/fingerprint.rs:796`). The crate
  contract in `crates/rs_cam_core/CLAUDE.md` says "Keep the crate GUI-free.
  Render and controller state are not part of the core model."
- proposal: move the ribbon builders and the colour ramps to `export/`,
  which is the folder that owns "output that is not G-code" and is already
  their only core reader. Leave `StockMesh`, `append`,
  `append_transformed` and `vertex_count` in `stock/`.
- breaks: the `rs_cam_core::stock::stock_mesh::…` import paths in four viz
  files and one core test. The operator ruling of 2026-09-16 allows the
  break, and the brief records that re-export shims were deliberately not
  added.
- effort: S
- risk: low — a move, not a rewrite.
- sentry: `tests/exporter_span_classifier_x1.rs` imports
  `toolpath_to_tube_mesh_with_spans` and follows it.
- owner:

## Top three

1. **STK-01** — Six hand-copied cell-scan loops in the stamping kernel. One
   driver replaces the largest duplication in the group, in the file that
   every stamping defect has landed in. The two route sentries already
   compare the results, so the extraction is measurable.
2. **STK-03 with STK-07** — Four scalars that `SimulationCutSample` and
   `Engagement` both carry. Together they remove four fields from a 24-field
   struct, and each removal is a mechanical rename the compiler drives.
3. **STK-13** — No product surface stamps the X or Y grid. A compiler-checked
   deletion removes two grid constructors, the side-face mesh path and four
   `StockCutDirection` variants, and it subsumes two thirds of STK-02.
   STK-06 is the cheap runner-up: two open-coded `FaceUp` maps that
   disagree with the canonical six-arm accessor.

## Checked and clear

- **One stamping kernel per cutter family: no.** `RadialProfileLUT::from_cutter`
  (`stock/radial_profile.rs:35`) samples any `&dyn MillingCutter` through
  `height_at_radius`, so the kernel is shape-generic. Adding a cutter family
  costs zero files in `stock/` and `dexel_stock/`.
- **Three grid axes in the stamping kernel: one path.**
  `stamp_point_on_grid` and `stamp_segment_on_grid` take `(u, v, depth)` and
  never name X, Y or Z. `StockCutDirection::decompose`
  (`dexel_stock/cut_direction.rs:49`) holds the whole permutation in one
  three-arm match. Only the constructors are copied — see STK-02.
- **The `StockCutDirection` invariant holds.** `cuts_from_high_side`
  (`cut_direction.rs:40`) and `grid_axis` (`:28`) are total matches over the
  six variants. The per-setup stock is stamped `FromTop` unconditionally
  (`compute/simulate.rs:1006`), consistent with the folder invariant that a
  2D operation cuts at negative Z.
- **`conservative_top` is a real sliver-safe channel, not a duplicate of
  `top_z_at`.** `lower_conservative_top` (`stock/dexel.rs:521`) is monotone
  and only a full-coverage stamp calls it. The reasoning is recorded at
  `dexel.rs:257`-`:285`.
- **`SimulationTriage::worst_severity` is a test-only door and already
  labelled one.** It carries `#[cfg(test)]` and the comment names S29,
  2026-09-16. No action.
- **`dedup` in `sim_triage.rs:1048` is not a quadratic hazard.** The linear
  scan runs over distinct keys, not over all findings, and the advisory
  lists are capped at 10 per toolpath and 50 per project
  (`sim_triage.rs:62`, `:64`).
- **The batch scratch buffers are hoisted, not per-move.**
  `dexel_stock/simulation.rs:442` and `:454` allocate `arc_buf` and
  `band_scratch` once per run; `build_buckets` clears the buckets instead of
  reallocating when the band count is unchanged.
- **`z_grid_to_solid_mesh` is a one-line delegation to
  `dexel_mesh_mc::z_grid_marching_cubes`** (`dexel_mesh.rs:178`-`:180`), not
  a second implementation. Harmless indirection; the doc explains the
  history.
- **`TriageInputs` has nine fields but is not a god parameter struct.**
  Every field is borrowed, and the doc at `sim_triage.rs:259` states why:
  "Borrowed, so no caller has to clone a trace."
- **`SimGroupEntry.direction` is live, as the brief says.** Its production
  reader is the S5 prefix hash at
  `crates/rs_cam_core/src/compute/sim_prefix.rs:425`. Not proposed for
  deletion.
- **`feeds/` and `tool_load/` numbers were not examined.** They belong to
  the power session. `Engagement` is read by feeds gates, so STK-03 and
  STK-07 need that session to review the rename.

## Add-a-thing count

The group's real extension point is **one new triage rule** — a new
simulation finding an operator can act on. Adding a cutter family costs
nothing here (see "Checked and clear"), so it is not the measure.

An engineer edits these files today:

1. `crates/rs_cam_core/src/diagnostics/ids.rs` — the `DiagnosticId` constant
   and its doc block (`:36`-`:133` hold the existing six).
2. `crates/rs_cam_core/src/stock/sim_triage.rs` — the rule function, the
   `Category` and `Severity` choice, the `DedupKey` construction, and the
   call arm inside `build_with_rest_context`.
3. `crates/rs_cam_core/src/stock/sim_measurability.rs` — a `SimMetric`
   variant if the rule must abstain, which means four edits in one file:
   the enum, `ALL`, `as_str` and `label`.
4. `crates/rs_cam_core/src/stock/sim_triage.rs` again — `TriageInputs`, if
   the rule needs an input the builder does not already borrow.
5. `crates/rs_cam_core/src/session/compute/diagnostics.rs` — the
   `TriageInputs` construction at `:550`, to supply that input.
6. `crates/rs_cam_viz/src/state/simulation/issue_triage.rs` — a
   `SimulationIssueKind` variant and an `issue_kind_rank` arm, because the
   GUI list is a second representation (STK-12).
7. `crates/rs_cam_viz/src/ui/sim_diagnostics.rs` — the panel row.
8. `crates/rs_cam_cli/src/project.rs` — the CLI report line.

That is **eight edits across seven distinct files** (items 2 and 4 are the
same file). Items 4 to 8 exist only because the finding must be restated three times: once as a core `Finding`, once as a
viz `SimulationIssue`, and once as a CLI line. STK-12 removes items 6 and 7.
