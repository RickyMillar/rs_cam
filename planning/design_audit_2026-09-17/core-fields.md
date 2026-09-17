# Design and feature-debt audit — core-fields

Group: `crates/rs_cam_core/src/geometry/`, `surface/`, `maps/`, `trace/`

### FLD-01 Six grid/raster types, three field-name conventions, no shared struct
- kind: design
- pattern: one concept with two (here, three) representations
- where: `surface/rest_field.rs:247` (`RestGrid`), `maps/tier_map.rs:473` (`TierMap`), `maps/reach_map.rs:364` (`ReachMap`), `surface/slope.rs:90` (`SurfaceHeightmap`), `surface/slope.rs:456` (`SlopeMap`), `maps/grid.rs:24` (`GridSpec`)
- evidence: `RestGrid`, `TierMap` and `ReachMap` each declare the identical five fields verbatim (`pub nx: usize, pub ny: usize, pub origin_x: f64, pub origin_y: f64, pub cell_mm: f64`) — `rg -c "origin_x: f64" geometry surface maps trace` finds the pattern in 7 files. `maps/grid.rs`'s `GridSpec` already carries the same five fields (as `pub(crate)`) and is used internally by both `tier_map.rs` and `reach_map.rs` during the row walk (`use crate::maps::grid::{GridSpec, walk_rows}` at `maps/tier_map.rs:141` and `maps/reach_map.rs:134`) — but the two public result structs still hand-roll their own copies of the same five fields instead of embedding it. `SurfaceHeightmap`/`SlopeMap` use a fourth naming variant (`rows`/`cols`/`cell_size` instead of `nx`/`ny`/`cell_mm`), and `grid2.rs`'s own module doc (`geometry/grid2.rs:1-19`) states outright that `RestGrid`, `SurfaceHeightmap`, `SlopeMap`, `DropCutterGrid` and `adaptive::material_grid::MaterialGrid` are "deliberately NOT migrated" to the shared `Grid2<T>` primitive.
- proposal: promote `maps::grid::GridSpec` to `pub`, embed it as one field in `RestGrid`, `TierMap` and `ReachMap` in place of their five duplicated scalars, and add `x_of`/`y_of`/`index_of` call-through. Leave `SurfaceHeightmap`/`SlopeMap`/`DropCutterGrid` alone per `grid2.rs`'s own note (genuine cosmetic divergences: anisotropic steps, rotated frame).
- breaks: `RestGrid`, `TierMap`, `ReachMap` public field layout (`.nx` becomes `.grid.nx` etc.) — every reader in `maps/`, `surface/`, viz overlays and MCP `reach_map`/`preview_tier_map` handlers needs the field-path update.
- effort: M
- risk: medium — three structs are read by viz overlay code and MCP tools outside this group; a mechanical field-path rename is safe, but touches files this audit does not own.
- sentry: `cargo test -p rs_cam_core -q --test reach_map_p5`, `cargo test -p rs_cam_core -q --test tier_map_walk_t1`
- owner:

### FLD-02 `detect_rest_valleys` is a 534-line function with 7 hand-numbered phases
- kind: design
- pattern: god function with a visible seam
- where: `surface/rest_field.rs:652`-`surface/rest_field.rs:1186`
- evidence: the function body carries its own phase markers at `surface/rest_field.rs:674` ("--- 1. Grid build ---"), `:809` ("--- 2. Components ---"), `:877` ("--- 2b. Machining-region polygons ---"), `:910` ("--- 3. Chamfer distance transform ---"), `:915` ("--- 4. Ridge extraction ---"), `:926` ("--- 5. Skeleton tracing ---"), `:933` ("--- 6. Route each polyline by COVERAGE ---"), `:1057` ("--- 7. Clearing regions ---"). The author has already named and ordered the seams; they are just inline comments, not function boundaries.
- proposal: extract phases 1-3 (grid build, components, chamfer distance) and 7 (clearing regions) into named private helpers taking/returning the grid and mask; leave 4-6 (ridge extraction/skeleton/routing) together since they share the most local state. This is a mechanical extraction, not a redesign — the numbered comments are close to a target signature list already.
- breaks: none (private function, no signature change visible outside the module)
- effort: M
- risk: low — behaviour-preserving extraction; the module's own sentries (`reach_policy_pr4`, `rest_routing_probe_e9`, `catchment_basin_census_w0`) exercise the full pipeline end to end and would catch a wiring slip.
- sentry: `cargo test -p rs_cam_core -q --test rest_routing_probe_e9`
- owner:

### FLD-03 `ToolpathSemanticKind` is the one enum in `trace/` with no `ALL`/ordinal, so its two full 26-arm matches live outside the crate that owns it
- kind: design
- pattern: enum dispatch replicated in N places
- where: `trace/semantic_trace.rs:16` (`enum ToolpathSemanticKind`, 26 variants), `crates/rs_cam_viz/src/ui/sim_debug.rs:44` (`semantic_kind_label`, 26-arm match), `crates/rs_cam_viz/src/ui/sim_debug.rs:93` (`semantic_kind_color`, 26-arm match)
- evidence: `trace/semantic_trace.rs` has no `impl ToolpathSemanticKind` block at all (confirmed by `rg -n "ToolpathSemanticKind" trace/semantic_trace.rs`: only the enum declaration and struct-field/constructor uses, never a method). Its two sibling enums in the same folder are stricter: `SpanKind` (`trace/toolpath_spans.rs:157`) documents `SpanKind::ALL` as the thing that "makes [`Self::label`]'s match ... exhaustive" (`trace/toolpath_spans.rs:410`), and `SemanticKey` (`trace/semantic_trace.rs:72`) is built explicitly so "[`Self::ALL`] is what [`Self::from_key`] derives from" (module doc, `trace/semantic_trace.rs:60-66`). `ToolpathSemanticKind` alone skipped that discipline, so `sim_debug.rs`'s colour match assigns `SPAN_SCALE[i % 6]` by literally re-typing the 26 variants in declaration order — a variant reorder in core silently desyncs the colour ramp with no compiler link back to the enum.
- proposal: add `impl ToolpathSemanticKind { pub const ALL: [Self; 26] = [...]; pub const fn ordinal(self) -> usize {...} }` in `trace/semantic_trace.rs`, mirroring `SpanKind`'s own contract, so viz can derive the colour step from `ordinal() % 6` instead of a hand-copied match.
- breaks: none (additive)
- effort: S
- risk: low
- sentry: a new unit test asserting `ToolpathSemanticKind::ALL.len() == 26` and that `ordinal()` is injective; existing `cargo test -p rs_cam_core -q --test narrate_regions_closed_c8` is unaffected and would catch any wiring slip.
- owner:


### FLD-04 Four cache-stats types are `pub`, exercised only by their own module's sentry test, and reach no diagnostic surface
- kind: feature-debt
- pattern: pub item used only from tests
- where: `maps/finish_surface_cache.rs:263` (`FinishSurfaceCacheStats`) + `:275` (`stats()`), `maps/geom_cache.rs:166` (`GeomCacheStats`) + `:186`, `maps/reach_map_cache.rs:100` (`ReachMapCacheStats`) + `:112`, `maps/tier_map_cache.rs:144` (`TierMapCacheStats`) + `:156`
- evidence: `rg -n "finish_surface_cache" --type rust crates | grep -v maps/finish_surface_cache.rs` shows the only call sites of its `stats()` are doc comments and `crates/rs_cam_core/tests/finish_surface_cache.rs:43,111,113,126`; the same pattern holds for the other three (`crates/rs_cam_core/tests/geometry_cache_g8.rs:375`, `crates/rs_cam_core/tests/reach_map_p5.rs:434`, `crates/rs_cam_core/tests/tier_map_cache_t3.rs:29`). No GUI panel, MCP tool or CLI command reads any of the four `builds`/`hits` counters — `rg -rn "CacheStats" crates/rs_cam_viz/src crates/rs_cam_mcp/src crates/rs_cam_cli/src` returns nothing.
- proposal: either wire the four counters into one product surface (a `get_diagnostics` field reporting cache hit-rate, useful for "why did this regenerate" reports — `maps/CLAUDE.md`'s own invariant that a stale cache key is a live bug class), or mark the four `stats()` functions `#[cfg(test)]`/`pub(crate)` so the crate stops advertising a diagnostic that nothing consumes.
- breaks: if downgraded to `pub(crate)`, breaks nothing outside the crate (no external crate depends on `rs_cam_core::maps::*_cache::stats`); if wired to `get_diagnostics`, no break, additive.
- effort: S (visibility narrowing) or M (wire to diagnostics)
- risk: low
- sentry: the four existing sentries (`geometry_cache_g8`, `reach_map_p5`, `tier_map_cache_t3`, `finish_surface_cache`) already assert on `stats()` and would need `#[cfg(test)]` visibility to keep passing if narrowed.
- owner:

### FLD-05 `reach_map_for_mesh`, `reset_drop_call_count`, `label_at`, `is_covered` are product-shaped `pub` APIs with zero non-test callers
- kind: feature-debt
- pattern: pub item used only from tests
- where: `maps/reach_map.rs:1834` (`reach_map_for_mesh`), `maps/tier_map.rs:175` (`reset_drop_call_count`), `maps/tier_map.rs:496` (`TierMap::label_at`), `surface/slope.rs:78` (`GridZ::is_covered`)
- evidence: `rg -n "\breach_map_for_mesh\b" --type rust crates` returns only the definition plus `crates/rs_cam_core/tests/reach_map_p5.rs`, `reach_map_residual_p5_1.rs` and `crates/rs_cam_viz/tests/reach_overlay_p5.rs` — the production path builds a `ReachMap` through `maps::reach_map::compute_reach_map`/`reach_map_cache`, never through this constructor. `reset_drop_call_count` and `label_at` are called only from `tests/tier_map_walk_t1.rs`, `tier_map_slope_t2.rs`, `tier_map_cache_t3.rs`. `is_covered` is called only from `tests/grid_z_uncovered_contract_c2.rs`.
- proposal: these are legitimate test-support APIs (a raw constructor bypassing the cache, a counter reset, a label accessor) — keep them, but mark each `#[cfg(any(test, feature = "test-support"))]` or move to a `test_support` submodule so a reader of the public API surface is not misled into thinking `reach_map_for_mesh` is a product entry point. `maps/CLAUDE.md` names `maps::reach_map::compute_reach_map` as canonical; the module exports both without distinguishing them.
- breaks: any external consumer of these four symbols (none found in-repo) would need the test feature enabled.
- effort: S
- risk: low
- sentry: `cargo test -p rs_cam_core -q --test reach_map_p5`, `cargo test -p rs_cam_core -q --test tier_map_walk_t1`, `cargo test -p rs_cam_core -q --test grid_z_uncovered_contract_c2`
- owner:

### FLD-06 `build_enriched_mesh` returns `Result<EnrichedMesh, String>`, the only bare-`String` error in the group
- kind: design
- pattern: error handling — String error
- where: `geometry/enriched_mesh.rs:347`
- evidence: `rg -n "-> Result<.*, String>" geometry surface maps trace --type rust` matches exactly this one signature in the whole group. Its only caller, `io/step_input.rs:181`, immediately wraps it: `build_enriched_mesh(...).map_err(|e| StepImportError::TessellationFailed { shell_index: 0, message: e })` — the crate already has a typed error (`StepImportError`) one call away, and the STEP import path is otherwise typed-error throughout.
- proposal: give `build_enriched_mesh` its own small error enum (`TooManyFaces`, `NoFaces`) in `geometry/enriched_mesh.rs` so the cause survives as data instead of a formatted sentence; `io/step_input.rs` maps each variant to its own `StepImportError` variant instead of stuffing free text into `message`. Note in passing: the wrapping call hardcodes `shell_index: 0` regardless of which shell actually failed — worth a look by whichever group owns `io/step_input.rs`, out of scope here.
- breaks: `build_enriched_mesh`'s public signature (return type); its one in-repo caller is a one-line update.
- effort: S
- risk: low
- sentry: `crates/rs_cam_core/src/geometry/enriched_mesh.rs`'s own unit tests around line 560 (`build_enriched_mesh(...).expect(...)`) exercise both the success and the "no faces" error path already.
- owner:

## Top three
1. **FLD-01** (grid/raster field unification) — every consumer of `RestGrid`, `TierMap` and `ReachMap` already agrees on five fields with the same names and the same meaning; the walk mechanism (`GridSpec`) is already shared. Promoting it to `pub` and embedding it removes three verbatim copies for a mechanical, low-risk change.
2. **FLD-03** (`ToolpathSemanticKind` ALL/ordinal) — small, additive, S-effort fix that closes a real gap against the discipline the folder's other two enums already follow, and removes a silent-desync hazard between the core enum's declaration order and a hand-copied 26-arm colour match in viz.
3. **FLD-02** (`detect_rest_valleys` extraction) — the function's own comments already name and order the seven phases; turning them into named helpers is close to free and makes the file's biggest function auditable in pieces instead of as one 534-line block.

## Checked and clear
- Drop-cutter and push-cutter do **not** duplicate the cutter-profile kernel: both go through the one `MillingCutter` trait (`tool/mod.rs:230`) — `dropcutter.rs` via `cutter.drop_cutter`/`vertex_drop`/`facet_drop`/`edge_drop`, `pushcutter.rs` via `cutter.width_at_height`/`center_height`/`xy_normal_length`/`normal_length` — two different, non-overlapping query shapes on one shared trait, not two kernels.
- No ad-hoc cache or memo exists beside the bounded ones in `maps/`: `rg -n "thread_local!|OnceCell|Lazy::new|static.*Mutex" geometry surface trace` (excluding `maps/`) finds nothing; the only `HashMap::new()` hits (`geometry/boundary.rs:618,624`, `trace/debug_trace.rs:199`) are function-local scratch structures, not caches.
- The small per-cache `get`/`put` wrappers in `maps/reach_map_cache.rs:185-193`, `maps/tier_map_cache.rs:239-247` and `maps/geom_cache.rs:231-243` that `dup_sweep_0.88_src.md` flags at 0.88-0.95 similarity are the shape `maps/memo.rs`'s own module doc argues for keeping local (key type, capacity and stats stay with each cache) — already-considered, not a fresh finding.
- The `dup_sweep_0.88_src.md` hit between `compute/execute.rs:452-464` and `trace/narrate.rs:1131-1144` (0.9030) does not hold up on read: the two ranges are unrelated code (a function-signature doc comment vs. a narration helper) — stale line numbers from before the 2026-09-17 folder split, not a live duplicate.
- No `TODO`/`FIXME`/`HACK`/"for now" comments anywhere in `geometry/`, `surface/`, `maps/` or `trace/` production code (`rg -n "TODO|FIXME|HACK|for now"` returns zero hits outside this check itself).
- `point_drop_cutter`/`CLPoint` correctly abstain rather than report zero: `CLPoint::contacted: bool` (`tool/mod.rs:35`) with `z = NEG_INFINITY` when unset, per `surface/CLAUDE.md`'s invariant; guarded by the `drop_cutter_off_mesh` sentry.
- `ToolpathSemanticKind`, `RegionSpanRole` (`trace/toolpath_spans.rs:387`) and `finish::unified_finish::RegionKind` are three deliberately distinct typed vocabularies for "what kind of region/span is this," each documented at the point of overlap (`trace/semantic_trace.rs:60-66`) — not an undocumented duplication.
- `RestFieldParams`, `TierMapParams` and `ReachMapParams` all stay at 4-6 fields despite the algorithmic complexity behind them; no boolean-mode-flag or >8-field config struct found in this group (the wide structs that do exist, e.g. `ReachMap` itself at `maps/reach_map.rs:364`, are measurement/report records with one documented statistic per field, not mode-encoding config).
- `RestFieldParams::min_cut_length` is genuinely read (`surface/rest_field.rs:930,1037,1678`), not a dead dial.
- `flow_accum.rs`'s `priority_flood_epsilon`/`resolve_flats`/`d8_accumulation` (`surface/flow_accum.rs:135,203,385`) have zero production callers anywhere in the workspace — only large research/falsifier test harnesses (`valley_branch_falsifier_h1.rs`, `valley_prize_census_h0.rs`, `pencil_spine_ab_p1.rs`, `catchment_basin_census_w0.rs`) exercise them. Not raised as a finding: the module is explicitly ruled to stay (brief's do-not-propose list, "`crate::flow_accum` stays").

## Add-a-thing count
Main extension point in this group: adding a new **`SpanKind`** (a new category of move range in a generated toolpath — `trace/toolpath_spans.rs:157`). `SpanKind::ALL` is the compile-time-exhaustive contract, so today an engineer touches at least:
- `crates/rs_cam_core/src/trace/toolpath_spans.rs` — add the variant, extend `ALL`, extend the exhaustive `label`/`is_boundary`-style matches (e.g. `:779`, `:830`).
- `crates/rs_cam_core/src/tool_load/locality.rs` — decide whether the new kind is a phantom-transit span (`is_phantom_transit`'s set).
- `crates/rs_cam_core/src/dressup/tsp.rs` and `crates/rs_cam_core/src/dressup/arcfit.rs` — decide whether TSP may reorder across it / whether arc-fit may replace moves inside it.
- `crates/rs_cam_viz/src/ui/sim_op_list.rs` — three separate exhaustive matches (`:708` rank, `:723`/`:747` label) need the new arm.
- `crates/rs_cam_viz/src/app/mcp/diagnostics.rs` — the span-kind filter (`:277`, `:664`) needs the new arm if it should be selectable from MCP.

For the sibling **`ToolpathSemanticKind`** (`trace/semantic_trace.rs:16`) extension point, the count is smaller today only because nothing outside `sim_debug.rs` matches it exhaustively (see FLD-03): `trace/semantic_trace.rs` (add the variant), `crates/rs_cam_viz/src/ui/sim_debug.rs` (`semantic_kind_label` and `semantic_kind_color`, two 26-arm matches). Adding `ALL`/`ordinal` per FLD-03 would not shrink this list, but would make the two viz matches fail to compile on a missed arm instead of silently mis-colouring.
