# D — near-duplicate code at cosine ≥ 0.88

Source: `planning/tech_debt_2026-09-16/evidence/dup_sweep_0.88_src.md` (137 src
pairs, 31 clusters). The 14 pairs at ≥ 0.92 are the C99 answer key
(`planning/duplicate_sweep_2026-09-15/EXECUTION_SUMMARY.md`); this pass skips
them and judges the 54 pairs below 0.92 that the report prints.

**Coverage, stated.** The report prints at most 5 pairs per cluster
(`scripts/duplicate_sweep.py:456`, hard-coded `cl[:5]`), so 69 of the 137 pairs
are elided and no flag exposes them. A re-run of a patched copy recovered them
(index drifted: 11 994 chunks vs 10 808). The elided set is dominated by
`compute/operation_configs.rs` against the per-op engine config structs (I08
Pair B, SIBLING) and by `compute/catalog.rs` registry rows against that file.
About 15 further cross-module pairs are recovered and NOT fully triaged here;
the three that matter are D1 (inherited), the `mcp/server.rs` ↔
`session/command.rs` / `ui_command.rs` wire-vs-engine pairs (SIBLING), and
`execute.rs:2116` ↔ `execute/project_curve_chaining.rs:1-74` at 0.9170, which is
test scope — an out-of-line `#[cfg(test)]` module the sweep's filter missed.
No cargo ran; every verdict is a static read.

| id | tier | cost | verdict | pair | one-line claim | OWNER |
|---|---|---|---|---|---|---|
| D1 | B | M | inherited | `core/session/project_file.rs` ↔ `core/compute/stock_config.rs:91,474`, `core/session/mod.rs:542` | Pre-v3 in-core shims still read old shapes; already on record as a C99 follow-up candidate, so inherit it rather than re-find it. | — |
| D2 | C | S | TRUE_DUP | `core/compute/semantic_helpers.rs` ↔ `core/compute/spans.rs:340,382` | The whole module is dead and `pub`-re-exported; `spans.rs` carries its own drifted `CutRun` + `cutting_runs`. | — |
| D3 | C | S | FALSE_POSITIVE | `core/collision.rs:160` ↔ `core/compute/collision_check.rs:55` | `check_collisions` has in-file test callers only; production calls the interpolated form. | — |
| D4 | D | S | DRIFTED_DUP | `cli/sweep.rs:268` ↔ `cli/run.rs:319` | Two CLI string-to-JSON coercers with different type rules: one applies the value, the other only records it. | — |
| D5 | D | S | DRIFTED_DUP | `viz/compute/worker.rs:1521` ↔ `core/panic_message.rs:23` | Two panic-payload readers with different fallback text; core's module is `pub(crate)`, so viz cannot reach it. | — |
| D6 | D | S | TRUE_DUP | `core/tool_library.rs:94,320` ↔ `core/machine_library.rs:92,178` | `list_in` is identical apart from doc text; `rename_in` differs only in the error type. | — |
| D7 | D | S | DRIFTED_DUP | `core/geo.rs:256` ↔ `core/dressup.rs:862` ↔ `core/adaptive3d/clearing.rs:2571,2587` | Four polyline-length helpers; `geo` owns the 3D one, and both variants are copied again. | — |
| D8 | D | S | DRIFTED_DUP | `core/tier_map_cache.rs:146` ↔ `core/reach_map_cache.rs:102` ↔ `core/geom_cache.rs:196` ↔ `core/finish_surface_cache.rs:280` | The counter scaffold stands in four caches; C23 merged the table, not the counters. | — |
| D9 | D | S | DRIFTED_DUP | `viz/render/toolpath_render.rs:32` ↔ `viz/app.rs:1178` | Two dashed-line vertex emitters with different dash models. | — |
| D10 | D | S | SIBLING | `core/metrology/spacing.rs:67` ↔ `core/geo.rs:340` | 3D and 2D point-to-segment distance, with different degenerate epsilons. | — |
| D11 | D | M | DRIFTED_DUP | `viz/compute/worker.rs:163,246` ↔ `core/compute/simulate.rs:208,375` | The viz mirrors core's two simulation structs by hand; this mirror class already dropped 11 fields once (C04). | — |
| D12 | D | M | DRIFTED_DUP | `core/metrology/monge.rs:148` ↔ `core/reach_map.rs:891` | Two readers of "mesh surface Z at (x, y)" with different containment tests and epsilons. | — |
| D13 | D | M | DRIFTED_DUP | `core/diagnostics/adapters/from_generation.rs` ↔ `core/narrate.rs` | Each finding's operator sentence is written twice, and the copies already word it differently. | — |
| D14 | D | L | DRIFTED_DUP | `core/dexel_stock/playback.rs:219,368` ↔ `core/dexel_stock/whole_path.rs:261,425` | Two ~160-line band-dispatch drivers with drifted comments and diverged stats. | — |
| D15 | E | M | SIBLING | `core/feed_modulation.rs:117` ↔ `core/feeds/mod.rs:433` | Four public chipload-band types carry the same two numbers. | power-calcs |

No pair in this set is tier A. The one candidate I raised there is D4, and the
check inside that section refutes it.

---

## D1 — pre-v3 shims (recovered pairs, 0.8943 / 0.8911 / 0.8860) — inherited, tier B

`session/mod.rs:542` ↔ `project_file.rs:415`, `stock_config.rs:91` ↔
`project_file.rs:217` and `stock_config.rs:474` ↔ `project_file.rs:163` are the
in-core v3 shims EXECUTION_SUMMARY already lists under "Follow-up candidates":
`_legacy_feeds_auto`, the one-shot dressup migration, the legacy `machine_ref`
drop and the top-level `toolpaths` pre-setup reader. Core reads
`format_version = 3` only, so these are a workspace-rule violation, not a
duplicate. **Inherit the existing entry; do not open a new number.**

## D2 — `compute::semantic_helpers` is dead (0.8959) — TRUE_DUP, tier C

- `core/compute/semantic_helpers.rs` (153 lines) declares `pub struct CutRun`
  (`:6`), `pub fn cutting_runs` (`:21`), `bind_scope_to_run` (`:94`),
  `append_toolpath` (`:99`), `line_toolpath` (`:108`) and `contour_toolpath`
  (`:127`); `compute/mod.rs:67-69` re-exports all six.
- **No caller exists.** `rg` over `crates/` finds each name only in that file and
  in the re-export line; `append_toolpath` forwards to `semantic_trace.rs:949`,
  which callers reach directly. `compute/spans.rs` carries its own private
  `CutRun` (`:340`) and `cutting_runs` (`:382`), with three callers.
- The two `cutting_runs` have diverged: `spans.rs` closes a run on
  `is_cut && !next_is_cut` and tracks `z_min`, `semantic_helpers.rs` closes on
  `!is_cut || !next_is_cut` and delegates to `describe_run`. They return
  different run sets for one toolpath. Sentries for the live copy:
  `tests/capability_link_moves_safety.rs:167,178`; `steep_shallow.rs:628` is the
  other caller. Same class as open register row **T-1**.
- **Cleanup.** Delete the module and its six re-exports; keep `spans.rs`'s
  private pair. risk: low — nothing links to it. proof:
  `cargo test -p rs_cam_core --test capability_link_moves_safety -q` plus
  `cargo clippy -p rs_cam_core --all-targets -- -D warnings`.

## D4 — CLI string-to-JSON coercers (0.9094) — DRIFTED_DUP, tier D

- `cli/run.rs:319` `parse_json_value` parses `i64` first, then `f64`, then
  `"true"`/`"false"` (callers `run.rs:113,164`); its value reaches
  `Command::SetToolpathParam` (`core/session/command.rs:2272` →
  `session/compute.rs:1180`), where the 0/1 → bool coercion at `:1361` tests
  `n.as_i64()`. `cli/sweep.rs:268` `parse_value_to_json` parses `f64` first,
  then `bool`, else string; one caller, `sweep.rs:135`.
- **Not a behaviour defect — the premise I first wrote was wrong.** The sweep
  does not apply through `SetToolpathParam`. It patches TOML text (`sweep.rs:90`
  `patch_job_param` → `patch_toml_field:222`) from the raw string, and
  `parse_value_to_json` only fills the report field `SweepVariant.value`
  (`core/fingerprint.rs:1749`). The divergence is a representation one: the
  sweep report records `3` as `3.0` where `run --set` applies `3`, so a reader
  comparing the two artifacts sees two types for one value.
- Sentries: none. No test names either function.
- **Cleanup.** Home: one `pub(crate) fn param_value_from_str` in `cli/job.rs`
  with `run.rs`'s integer-first order; delete the sweep copy. risk: low — one
  report field changes its JSON type. proof: the sweep report round-trip test at
  `core/fingerprint.rs:1930`; then `cargo test -p rs_cam_cli -q`.

## D5 — panic-payload readers (0.9112) — DRIFTED_DUP, tier D

- `core/panic_message.rs:23` `panic_payload_message` is `pub(crate)` in a
  `pub(crate) mod` (`lib.rs:94`); three callers, all in `polygon.rs`
  (`:629,723,1291`). `viz/compute/worker.rs:1521` `panic_message` is the same
  two-arm downcast, with four callers (`:1017,1156,1319,1502`).
- Drift: the fallback strings differ — `"unknown panic"` in viz,
  `"non-string panic payload"` in core. Both reach an operator-facing job
  failure message. No sentry names either function.
- **Cleanup.** Home: make the module `pub` and `panic_payload_message` `pub`,
  delete the viz copy, adopt core's string. risk: low; one operator-visible
  string changes. proof: `cargo clippy --workspace --all-targets -- -D warnings`.

## D6 — library scaffolds (0.8961 / 0.8837) — TRUE_DUP, tier D

- `list_in` differs only in its doc comment (7 diff lines of 25, all doc).
  `rename_in` differs only in the error enum. Both files also carry `path_in`,
  `library_dir` and a parallel `save`/`load` pair. Callers:
  `viz/ui/tool_library_modal.rs`, `viz/ui/machine_library_modal.rs:545`,
  `viz/ui/properties/mod.rs:590,641`.
- **Cleanup.** Home: a `core/src/named_toml_library.rs` holding the directory
  walk and the rename, generic over the error through one
  `From<std::io::Error>` bound; both modules keep their public names as one-line
  delegations. risk: low-medium — file I/O on two live surfaces. proof: the
  round-trip tests in each file, then `cargo test -p rs_cam_core -q`.

## D7 — polyline length, four copies (0.9041 / 0.8833) — DRIFTED_DUP, tier D

- `geo::polyline_length` (`geo.rs:256`) is the canonical 3D length; R3
  (`8db8793b`) already deleted the `conformal_spiral.rs` copy against it.
  `adaptive3d/clearing.rs:2587` `polyline_length_3d` is a fourth copy of exactly
  that function; `:2571` `polyline_xy_length` and `dressup.rs:862`
  `polyline_xy_len` are the XY variant twice. `geo` publishes no XY form, so the
  XY copies had nowhere to go.
- **Cleanup.** Home: add `geo::polyline_xy_length` beside `polyline_length`;
  delete all three copies. risk: low. proof: the dressup air-cut tests and
  `tests/agent_search_coverage.rs`; then `cargo test -p rs_cam_core -q`.

## D8 — cache counter scaffold, four copies (0.9190 / 0.9127 / 0.8961) — DRIFTED_DUP, tier D

- `tier_map_cache.rs:146-170`, `reach_map_cache.rs:102-126` and
  `finish_surface_cache.rs:265-289` each hold `static BUILDS`, `static HITS`,
  `stats()`, `reset_stats()` and `cache_len()`; the first two differ by their
  stats type name alone, and `geom_cache.rs:196-213` differs only because it
  loops over three counter pairs.
- I02 judged the tier/reach scaffolding TRUE_DUP, and C23 merged the table into
  `memo.rs` / `grid.rs` while keeping the statics per cache by design. The
  counters were not merged; `geom_cache` and `finish_surface_cache` are the two
  copies under 0.92. Drift: `finish_surface_cache::cache_len` reads `t.len()`
  where the other two read `t.entry_count()`.
- **Cleanup.** Home: a `CacheCounters { builds, hits }` value type in `memo.rs`;
  each cache keeps one `static` of it and its own public stats struct. risk:
  low — test hooks only. proof: the capacity tests in the four files; then
  `cargo test -p rs_cam_core -q`.

## D11 — viz simulation DTOs ↔ core (0.9160 / 0.9101) — DRIFTED_DUP, tier D

- `viz/compute/worker.rs:163` `SimulationRequest` and `:246` `SimulationResult`
  restate core's field lists by hand. It is an adapter, not a parallel engine:
  `viz/compute/worker/execute/mod.rs:39` converts the viz request into core's at
  `:109`, so every field crosses by hand.
- Drift already present: the viz request adds `use_predicted_feed_in_gates`,
  `max_feed_mm_min` and `memoize_prefix`, the viz result adds `playback_data`
  and `cut_trace_path`, and core's result documents a `Clone` cost contract the
  viz copy has no equivalent of. The same mirror class is what C04 found: the
  CLI sweep baseline "had dropped eleven [fields], not four".
- **The flow behind it is already on record.** The GUI simulates off the frame
  loop rather than through `ProjectSession::run_simulation`
  (`viz/ui/readiness.rs:410` doc). That is **G-MCPSIMMIRROR**, open under WP28
  parts 1 and 4 (`planning/arch_consolidation_2026-09-09/STATUS.md:273,550`).
  Inherit that ID for the flow; this row is only the struct mirror.
- **Cleanup.** Home: keep core's structs and give the viz a small
  `SimulationRequestExtras` beside core's request, instead of a second full
  struct. risk: medium — the worker fills the request over several call sites.
  proof: `viz/src/controller/results_parity_tests.rs` and
  `viz/src/compute/worker/gen_parity_p0_tests.rs`; then
  `cargo test -p rs_cam_viz -q --lib`.

## D12 — mesh surface Z (0.9033) — DRIFTED_DUP, tier D

- `metrology::monge::surface_z` (`monge.rs:148`) walks `index.cell_triangles_at`,
  reads `mesh.triangles` / `mesh.vertices`, and tests containment with
  barycentric coordinates at `BARY_EPS = 1e-9` (callers
  `metrology/ownership.rs:191`, `union_coverage.rs:235`).
  `reach_map::surface_z_at` (`reach_map.rs:891`) walks `index.query`, reads
  `mesh.faces`, and tests containment with `tri.contains_point_xy` +
  `plane_z_at` (callers `reach_map.rs:630,1381,1655`).
- Both take the `max` over hits and answer "how high is the surface here".
  `reach_map`'s doc claims it shares its containment test with
  `dropcutter::point_is_over_mesh_xy` "so ... [they] can never disagree" — the
  `monge` copy sits outside that guarantee and admits triangles the `reach_map`
  copy rejects.
- **Cleanup.** Home: `reach_map::surface_z_at`, the copy tied to the drop-cutter
  predicate. Give it a `P2` form; make `monge::surface_z` a delegation. risk:
  medium — metrology numbers can shift at triangle edges, so measure one fixture
  before and after. proof: `tests/reach_policy_pr4.rs` and the metrology
  coverage sentries; then `cargo test -p rs_cam_core -q`.

## D13 — one finding, two operator sentences (0.9133 / 0.9091) — DRIFTED_DUP, tier D

- Each `compute::config` finding is rendered twice: as a `Diagnostic` message in
  `from_generation.rs` and as a narration line in `narrate.rs`. Sampled pairs:
  `boundary_clip_dropped` (`:167` ↔ `narrate.rs:1154`) and `inert_claims_dial`
  (`:121` ↔ `narrate.rs:1186`).
- The sentences are deliberately alike and have drifted: "not with a tighter
  one" (adapter) against "not with a smaller one" (narration); the inert-dial
  pair diverges further. The re-run recovers more of the family
  (`execute.rs:452` ↔ `narrate.rs:1147`, `execute.rs:311` ↔
  `from_generation.rs:109`, `narrate.rs:1174` ↔ `config.rs:952`), so scope the
  work by the family, not by the two pairs.
- **Cleanup.** Home: one `fn message(&self) -> String` per finding struct in
  `compute/config.rs` — the shape `DepthBeyondStock::message`
  (`from_static_checks.rs:449`) already uses — with both surfaces calling it.
  risk: low-medium; operator text changes where the copies differ. proof:
  `tests/findings_transport_join_h21.rs` and `tests/inert_claims_dial_f4.rs`;
  then `cargo test -p rs_cam_core -q`.

## D14 — dexel band dispatch (0.9041 / 0.9002) — DRIFTED_DUP, tier D

- `PlaybackBandDispatch` (`playback.rs:219`) and `BandDispatch`
  (`whole_path.rs:261`) hold the same ten fields in the same order, differing
  only in the job and partial element types. `batch_is_due`, `is_empty` and
  `build_buckets` are the same code; `run_batch` (`:368` / `:425`) shares its
  whole prologue and its band-span guard.
- Drift: `PlaybackBandDispatch` adds `band_tasks_run: AtomicU64` and folds it in
  at `stats()`; `BandDispatch` returns `self.stats` plain. The two cancellation
  doc blocks state one rule in different words.
- **Cleanup.** Home: `core/src/dexel_stock/band_dispatch.rs` with a
  `BandDispatch<J, P>` generic over the job and partial types, and a small trait
  for `run_batch`'s per-band body. risk: medium-high — the wave notes pin this
  loop's fan-out, so the merge must be byte-identical on a fixture. proof: the
  non-vacuity sentries named in `whole_path.rs`'s `stats` doc, plus one A/B.

## Short rows — D3, D9, D10, D15

- **D3** (tier C, FALSE_POSITIVE). `collision.rs:160` `check_collisions` is the
  `step_mm = 0.0` wrapper its own doc calls "legacy behavior". Its only callers
  are the tests in the same file (`:729,758,791,851,875`); production uses
  `check_collisions_interpolated*` through `compute/collision_check.rs:55`,
  which also adds the W0.1 fixture pass. The pair is a FALSE_POSITIVE; the dead
  public wrapper is the tier C row. Cleanup: make it `#[cfg(test)]`. risk: low.
  proof: the in-file collision tests.
- **D9** (tier D). `toolpath_render.rs:32` `push_dashed_segment` emits a fixed
  0.35 / 0.65 centre gap; `app.rs:1178` `push_dashed_line_vertices` walks a
  `dash_len` / `gap_len` cycle. Two dash models on one renderer. Cleanup: keep
  the parameterised one in `render/` and express the other as one call. risk:
  low; the dash changes unless the arguments reproduce the thirds. proof: a GUI
  look at the overlays.
- **D10** (tier D, SIBLING). `metrology/spacing.rs:67` `dist_point_segment` (3D,
  degenerate guard `1e-18`) and `geo.rs:340` `point_to_segment_distance` (2D,
  `1e-20`) are the same algorithm at two dimensions. No action; note the
  epsilons if either moves.
- **D15** (tier E, SIBLING, `OWNER: power-calcs`). Core publishes four types
  over the same two numbers: `feed_modulation::ChiploadBand` (`:117`),
  `feeds::ChiploadBounds` (`feeds/mod.rs:433`),
  `feeds::quantities::VendorChiploadBand` (`:203`) and
  `tool_load::verdict::ChipBounds` (`:927`). Each doc explains why it exists, so
  this is a SIBLING set, not a merge — but the surface is wide and one drift
  between two of them is already on record. Route it to the power-calcs owner as
  an API-surface question, not a cleanup.

---

## No action

- **I08 / C99 answer key (14 pairs ≥ 0.92)** — skipped by instruction.
- **Delegation, caller to callee** (FALSE_POSITIVE): `feedopt.rs:70` `rctf` →
  `feeds::geometry::radial_chip_thinning_factor`; `gpu_upload::build_advance_bands`
  → `tool_load::chipload_envelopes_for_session`;
  `tool_load::deflection::sample_tip_deflection_mm` →
  `feeds::predict::tip_deflection_from_engagement` (those three
  `OWNER: power-calcs`); `viz/ui/readiness.rs:410` → `ProjectSession::query`
  (the WP9 wrapper, G-TIMEEST holds); `viz/ui/properties/operations/mod.rs:2392-2459`
  → `from_static_checks` (a `pub use` plus two one-line adapters, F1.18 holds);
  `pills.rs:41` → `components::suggest::Suggestion`.
- **Aggregate over per-item record** (FALSE_POSITIVE):
  `unified_finish::DroppedBand` ↔ `config::DroppedBandFinding`, the relation I08
  Pair D judged. **Different functions**: `viz::toolpath_cutting_z_range` ↔
  `narrate::representative_cut_z_in_range`. **Already merged** (SIBLING):
  `spiral_finish_toolpath_annotated` ↔ `ramp_finish_toolpath_annotated` both
  call `compute::spans::runtime_annotations_to_labels` (C28).
  **Chunk-boundary artifacts**: P11, P12, P34, P38, P44, P47, P52 — the embedder
  matched two unrelated `match` arms, two `Default` impls or a module header.
- **Intentional siblings** (one line each): `tool/{ball,flat,bullnose,tapered_ball}.rs`
  per-shape `MillingCutter` impls; `RetargetStrategyOutput` ↔
  `RetargetStageOutput`, distinct stages cross-referenced in both docs
  (`OWNER: power-calcs`); `LutOperationFamily` ↔ `op_family_to_lut`, the enum and
  its one canonical mapping (`OWNER: power-calcs`); `mcp/server.rs` ↔
  `session/command.rs` / `ui_command.rs`, wire versus engine; the viz panel and
  modal draw entries (`machine_library_modal`, `tool_library_modal`,
  `toolpath_panel`, `toolpath_row_controls`, `optimize_modal`,
  `optimize_project` — the last two sit below the I07 merge, C25 / C26).
- **Sweep-tool defect, not code debt**: P22, P53 and the recovered
  `execute/project_curve_chaining.rs` pairs are `#[cfg(test)]` module bodies.
  The sweep claims to drop them; the filter misses a chunk that starts on the
  `#[cfg(test)]` line, and it missed this out-of-line file entirely.

---

## What to fix first

1. **D2** — a dead `pub` module with a drifted twin of a live function; the T-1
   trap, and deleting it costs nothing.
2. **D13** — operator text drifts silently, and the family exceeds the two
   sampled pairs. Scope it before it grows again.
3. **D5, D6, D7, D8, D9** — five small merges with named homes and low risk.
4. **D11 and D12** need a measurement, not confidence: one is a cross-crate
   mirror behind an open ID, the other moves metrology numbers at mesh edges.
5. **D14 last** — the simulator's hot loop needs a paired A/B.
