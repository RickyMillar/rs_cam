# I02 — tier/reach twin family
Verdict: cache scaffolding TRUE_DUP (pairs 19/28/38); GridSpec TRUE_DUP (pair 37, constructors deliberately stay local); walk_rows DRIFTED_DUP (pair 41).

## Evidence
- Pair 19 (0.9535) `tier_map_cache.rs:161-185` ↔ `reach_map_cache.rs:112-136`: diff of exact ranges = `TierMapCacheStats` → `ReachMapCacheStats` only; statics/stats/reset_stats otherwise byte-identical.
- Pair 28 (0.9459) `tier_map_cache.rs:257-278` ↔ `reach_map_cache.rs:198-219` `put()`: diff = key/value type names + one comment word ("plan"→"selection"). Same sweep-dead-mesh, replace-or-evict-oldest, CAPACITY logic.
- Pair 38 (0.9384) `tier_map_cache.rs:186-194` ↔ `geom_cache.rs:227-235` `clear()`: one word ("maps"→"buffers"). Same `table()/Entry/Weak`-identity scaffolding also in `geom_cache.rs` (put generalised to a write-closure) and `finish_surface_cache.rs` (content-keyed, no Weak) — a 4-module pattern.
- Deliberate per-cache params (NOT drift): `CAPACITY` 2 (tier) vs 4 (reach), each justified in module docs; keys differ (TierMapKey ladder/ResidualTreatment vs ReachMapKey tool/ids).
- Pair 37 (0.9386) `tier_map.rs:737-747` ↔ `reach_map.rs:920-930` GridSpec: struct + `x_of`/`y_of` byte-identical (doc cross-ref only). Constructors diverge on purpose: reach adds `at_cell` + MAX_REACH_CELLS coarsen loop (`reach_map.rs:935-957`); tier pads by ladder finest envelope + has `cell_count()` (`tier_map.rs:753-771`).
- Pair 41 (0.9360) `tier_map.rs:784-833` ↔ `reach_map.rs:981-1022` `walk_rows`: same dual-cfg(parallel) row walk, one-poll-per-row. Tier threads `(Vec<T>, u64)` drops into `DROP_CALLS` and returns `TierMapError`; reach drops the counter and returns `Cancelled`. Reach doc cites tier's walk_rows as the rule it copies.
- Callers: `cached_tier_map` ← `session/multitool.rs:1210` (+`peek_tier_map` :449); `cached_reach_map` ← `session/reach.rs:204`, viz `compute/worker.rs:1468`, `app/mcp.rs:3640,3674`; geom_cache ← `session/compute.rs:2774/2878/3484`, `multitool.rs:522/538`, `reach.rs:161/166`. walk_rows ← `tier_map.rs:857/950/979`, `reach_map.rs:1444/1643`.
- Sentries: `tests/tier_map_cache_t3.rs` (hit = zero drop work via `drop_call_count`, capacity/eviction, dead-mesh sweep), `tests/tier_map_slope_t2.rs` (exact drop counts), `tests/reach_map_p5.rs:421` (memo hit, radius-edit miss), `tests/geometry_cache_g8.rs`, `tests/finish_surface_cache.rs`.
- Repo norm violated: shared cache code is *moved*, not copied — `tool_shape_key` was moved out of tier_map_cache when finish_surface_cache needed it (`tier_map_cache.rs` module doc, `tool_shape_key.rs:104`). Reach module doc admits "Verbatim tier_map_cache discipline".

## Drift / differences
- walk_rows: authoritative = `crate::tier_map::walk_rows` (instruments the T3 drop-work contract; reach's doc defers to it). Drift: reach copy omits the DROP_CALLS fold and uses `Cancelled`; cancellation semantics identical today, kept in sync by convention only — the "one site so granularity cannot drift" argument applies equally across modules.
- Side finding (within reach_map_cache, not a pair): module doc L35-38 says tool/model ids are "deliberately NOT in the key", but `ReachMapKey` includes `tool_id`/`model_id` since the file's first commit (5f665edf). Code is the safe side (prevents a hit returning stale stamped ids via `with_ids`); doc is stale. Sentry `reach_map_p5.rs:451` pins stamped ids.

## Proposed cleanup
- home: new `crates/rs_cam_core/src/memo/` — generic `MeshMemo<K, V, const CAPACITY>` owning table/Weak-identity entry/get/put(sweep+LRU); keep per-cache statics, key types, stats structs, build fns. Plus `grid.rs`: shared `GridSpec` struct + `x_of`/`y_of` (+ padding helper), constructors stay local; shared `walk_rows` taking `Option<&AtomicU64>` drop sink, returning `Result<_, Cancelled>` (tier keeps its `From<Cancelled> for TierMapError`, tier_map.rs:327).
- risk: med — perf-critical hot paths + instrumented drop counts, but all behavior pinned by existing sentries; no persisted formats/MCP wire types touched.
- proof test (smallest): `cargo test -p rs_cam_core --test tier_map_cache_t3 the_same_key_returns_the_cached_arc_and_does_zero_drop_work` (exercises merged memo + walk_rows drop fold), then `--test reach_map_p5 the_memo_hits_on_the_same_key_and_rebuilds_after_a_radius_edit`, `--test geometry_cache_g8`, `--test tier_map_slope_t2`; core full gate for lint.
