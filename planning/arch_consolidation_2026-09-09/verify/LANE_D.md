# Lane D — findings 10, 11, 12 and the Phase 0 test inventory

Date: 2026-09-10. Tree: branch `machine-kinematics-confidence`, HEAD
`3c1aeb67`. The audit was taken at `5087490f`; **98 commits** have landed
since (`git log --oneline 5087490f..HEAD | wc -l` = 98).

**Method and its limit.** This lane READ source, tests, `git log` and
`git diff`. It ran NO cargo command — no `test`, `check`, `clippy`, `fmt`
or `build`. Every statement below is a reading. Where a claim needs a run
to settle, the row says UNVERIFIED and gives the reason. The gate figures
quoted anywhere in this file (299 binaries, 3773 passed, one binary red by
design) are the orchestrator's, given to this lane, not measured here.

`None` means NOT MEASURED. It never means clean.

## Corrections this lane makes to the audit, the plan and the code's own docs

Stated plainly, each with its evidence below.

1. **The audit UNDERCOUNTS finding 12.** There are THREE weak-identity
   memos, not two. `crates/rs_cam_core/src/geom_cache.rs` is the same
   pattern and is the OLDEST of the three (added 2026-08-20, against
   2026-08-27 and 2026-09-08).
2. **`STATUS.md`'s finding-2 row is half stale.**
   `ProjectSession::replace_toolpath_config` now DOES propagate
   invalidation (`crates/rs_cam_core/src/session/mutation.rs:1222`).
   `apply_toolpath_param_snapshot` still does not, and it is the path both
   undo and the OPTIMIZER use.
3. **`STATUS.md`'s UNVERIFIED STEP claim is TRUE, and its reachability is
   not what the row assumes.** The project door drops the STEP scale. The
   GUI cannot reach that arm; the CLI job route can.
4. **`polygon.rs:562` is stale by two.** It says "fifteen geometry-only
   call sites"; the count today is 13.
5. **The audit's finding-9 description of spiral is ACCURATE.** I read the
   emitter and confirm there is no per-run retract.

---

# Job 1 — the three findings

## Finding 10 [Medium] — offset failure handling is available but opt-in

### Verdict: STILL TRUE

### Evidence at today's tree

`offset_polygon` still delegates to the reporting function and drops the
failure channel:

- `crates/rs_cam_core/src/polygon.rs:565-567` — `pub fn offset_polygon(...)
  -> Vec<Polygon2> { offset_polygon_reported(polygon, distance).0 }`
- `crates/rs_cam_core/src/polygon.rs:583` — `offset_polygon_reported`, the
  structured door.
- `crates/rs_cam_core/src/polygon.rs:1508` — `OffsetRingSet::offset`, the
  same shape for the ring-set API.

Both callers the audit names are still on the dropping door. Their line
numbers moved by six and three lines respectively:

- `crates/rs_cam_core/src/rest.rs:102` — `let large_reachable =
  offset_polygon(polygon, params.prev_tool_radius);` (audit said `:96`).
  `rest_segments` then reads `large_reachable.is_empty()` at
  `rest.rs:114` as "the large tool cannot fit at all" and emits the whole
  pocket as rest region. A library failure produces the same empty vector
  and the same conclusion.
- `crates/rs_cam_core/src/inlay.rs:97` — `let inset_polygons =
  offset_polygon(polygon, -inset_dist);` (audit said `:94`).

### Counted census of the two doors

I separated production sites from `#[cfg(test)]` sites by reading each
file's test-module start line.

**13 production call sites still use the dropping door**, in 7 files:

| File:line | Test module starts at |
|---|---|
| `crates/rs_cam_core/src/region_set.rs:102` | 112 |
| `crates/rs_cam_core/src/scallop.rs:1502` | 2816 |
| `crates/rs_cam_core/src/project_curve.rs:283` | 400 |
| `crates/rs_cam_core/src/rest.rs:102` | 216 |
| `crates/rs_cam_core/src/inlay.rs:97` | 309 |
| `crates/rs_cam_core/src/adaptive3d/clearing.rs:1724, 1818, 1954` | 2601 |
| `crates/rs_cam_core/src/adaptive/path.rs:132, 858, 944, 1192, 1238` | none |

**8 production call sites already use the reporting door**, in 5 files:
`profile.rs:148`, `zigzag.rs:95`, `boundary.rs:75`, `boundary.rs:77`,
`boundary.rs:126`, `pocket.rs:272`, `pocket.rs:381` (the `offset_reported`
ring-set form) and `trace.rs:93`.

**Documentation drift to correct.** `polygon.rs:562-564` says the plain
name "keeps the fifteen geometry-only call sites at zero churn". The count
is **13** today, not 15. The number is stale by two. Nobody should plan
against the doc string.

### What changed since the audit

Nothing on this finding. One commit touched `polygon.rs` in the 98
(`fd135a04`, G-RAMPCONTAIN), and `git diff 5087490f..HEAD --
crates/rs_cam_core/src/polygon.rs` shows it added
`Polygon2::distance_to_boundary` for `entry_audit::fed_moves_outside_region`.
It changed no offset function and moved no offset call site.

### Is the recommendation still the right shape?

Yes, with one correction the plan must carry.

The recommendation says "make the structured outcome the default API".
That is right. But `CLAUDE.md` records a constraint the audit does not
state, and the plan's Policy row ("Offset failure? Require an explicit
caller decision") reads as stronger than the code can deliver:

- Two of the three offset panic classes are `debug_assert!`s inside
  `cavalier_contours`. In a **release** build the library proceeds on the
  unvalidated input instead of being caught, so the failure never reaches
  `OffsetFailure` at all. `offset_library_failures` is therefore **not
  comparable across builds**, and a lower release count is the expected
  divergence, not an improvement.
- Consequence: even after every caller moves to the reporting door, a
  release build cannot fully separate a library failure from a legitimate
  geometric collapse. The reporting door narrows the gap; it does not
  close it.
- Second constraint from `CLAUDE.md`: a `ToolContainment::Inside` boundary
  is not an unconditional guarantee. A collapsed boundary offset emits the
  path **UNCLIPPED** and leaves only the report-only `boundary_clip_dropped`
  finding, which has no narration adapter. `boundary.rs` is already on the
  reporting door (three sites), so this is a caller-policy defect, not a
  door defect. Phase 5C must decide it, not assume the door fixes it.

Recommend the plan's Policy table row be rewritten to say: require an
explicit caller decision **and** state that the decision is only as sound
as the build, naming the release-mode `debug_assert!` hole.

### Counted size estimate

- 13 call-site migrations across 7 files. Each is `let x = offset_polygon(a, b);`
  becoming `let (x, failure) = offset_polygon_reported(a, b);` plus a
  written disposition for `failure`.
- Two of the 13 are the ones that carry a real semantic decision
  (`rest.rs:102`, `inlay.rs:97`). The other 11 need a disposition recorded,
  which is the expensive part per the recommendation's own "do not impose
  one universal failure policy".
- 8 sites are already migrated and need no work.
- `adaptive/path.rs` alone holds 5 of the 13 and has **no test module in
  the file**; its coverage is in `adaptive/mod.rs` and
  `tests/adversarial_2d_campaign_r2.rs`. That file is the largest single
  block of work.
- Deleting `offset_polygon` outright is NOT advised as part of this: the
  ring-set twin `OffsetRingSet::offset` (`polygon.rs:1508`) has the same
  shape and only test callers today, so the two should move together or
  neither.

---

## Finding 11 [Medium] — vendor routing encodes incidents, not machining use

### Verdict: STILL TRUE

### Evidence at today's tree

`crates/rs_cam_core/src/feeds/vendor_normalize.rs:69-98` — `lut_query_for`
is 30 lines and is literally three special cases followed by a pass-through:

1. `operation_kind == Adaptive3d && operation_family == Adaptive`
   → `(Pocket, pass_role)` (line 75).
2. `operation_kind == DropCutter && tool_family == FlatEnd &&
   operation_family == Parallel` → `(Pocket, Roughing)` (line 83).
3. `operation_kind != ProjectCurve` → return the declared pair unchanged
   (line 89).
4. `ProjectCurve` by tool family, including a `None` refusal for bull nose,
   V-bit and facing bit (lines 92-97).

The function's own doc comment states the restricted scope in these words
(`vendor_normalize.rs:31-33`):

> `Waterline`, `RadialFinish`, `HorizontalFinish`, `SteepShallow` and
> `RampFinish` have the same hole for a flat tool and are **NOT** routed
> here — listed follow-up, not widened scope.

That is the finding, stated by the code itself.

`LookupQuery` (`crates/rs_cam_core/src/feeds/vendor_lookup.rs:13-23`) has
nine fields: `tool_family`, `tool_subfamily`, `diameter_mm`, `flute_count`,
`material_family`, `hardness_kind`, `hardness_value`, `operation_family`,
`pass_role`. **None carries substitution rationale or lookup provenance.**
The recommendation's "represent the resolved machining use and lookup
provenance explicitly" half is therefore entirely unbuilt.

Production consumers of the router: `tool_load/chipload.rs:131` (the gate)
and `tool_load/optimize/outcome.rs:448` (the optimizer/Suggest side). The
remaining hits are doc comments and tests.

### What changed since the audit

Nothing. `git log 5087490f..HEAD -- crates/rs_cam_core/src/feeds/vendor_normalize.rs`
returns no commit. G-DCFLAT, the newest arm, landed 2026-09-08 — one day
BEFORE the audit — so the audit already saw it.

### Test coverage of the routing today

`crates/rs_cam_core/tests/drop_cutter_flat_roughing_row_g_dcflat.rs` pins
the ONE substitution as a route identity: the routed flat drop-cutter query
resolves the same observation the `Adaptive3d` rough resolves on the same
tool and material, and the unrouted query still finds no row. That is the
strong kind of sentry — it runs the resolver.

**The five finish operations the doc names have no sentry at all.** I found
no test asserting that `Waterline`, `RadialFinish`, `HorizontalFinish`,
`SteepShallow` or `RampFinish` on a flat tool reads `Unmodeled(NoVendorData)`.
So the known hole is documented in a comment and unpinned in the suite. A
refactor that widened the routing by accident would not fail anything.

### Is the recommendation still the right shape?

Yes, and its `Do not` clause is the important half. Broadening the
DropCutter substitution to the five named finish ops is a physical-policy
decision — it hands a finishing raster a roughing chipload band. The plan's
Policy row already says "only with a documented physical rationale;
otherwise remain unmodeled", which is correct.

One addition: before the routing is redesigned, the five holes should be
**pinned as PASSING boundary tests** — the F4.10 pattern from the UI
programme, where a known gap is asserted as it stands so it is visible
rather than assumed. That is cheap and it makes any widening deliberate.

### Counted size estimate

- 1 function, 30 lines, 3 special-case arms, 2 production consumers.
- 1 struct to extend (`LookupQuery`, 9 fields) plus its type alias
  `LookupCriteria`.
- The coverage review the recommendation asks for is a matrix of
  `OperationType` × `ToolFamily`. The registry has the operation list
  (`for_each_op!`, `crates/rs_cam_core/src/compute/catalog.rs:149`) and
  `ToolFamily` has 6 variants, so the matrix is enumerable from the
  registry rather than hand-written.
- 5 boundary sentries to add for the named holes; 1 existing sentry
  (`g_dcflat`) to keep green.
- The physical-policy half is NOT engineering work and must not be scoped
  as such.

---

## Finding 12 [Medium] — new caches copy the same infrastructure

### Verdict: STILL TRUE, and the audit UNDERCOUNTS it. There are THREE
copies, not two.

### Evidence at today's tree

`crates/rs_cam_core/src/reach_map_cache.rs` (219 lines) and
`crates/rs_cam_core/src/tier_map_cache.rs` (331 lines) carry the same
eleven items, name-for-name:

| Item | reach_map_cache.rs | tier_map_cache.rs |
|---|---|---|
| `CAPACITY` const | :56 (= 4) | :100 (= 2) |
| key struct + `new` | :61, :71 | :105, :117 |
| `struct Entry { mesh: Weak<TriangleMesh>, key, map }` | :83 | :129 |
| `Entry::matches` (upgrade + `Arc::ptr_eq`) | :92 | :139 |
| `fn table() -> &'static Mutex<Vec<Entry>>` + `OnceLock` | :101 | :148 |
| stats struct | :108 | :155 |
| `static BUILDS` / `static HITS` (`AtomicU64`) | :113 | :161 |
| `pub fn stats()` | :118 | :166 |
| `pub fn reset_stats()` | :127 | :175 |
| `pub fn cache_len()` | :134 | :183 |
| `pub fn clear()` | :141 | :190 |
| `fn get(...)` | :191 | :247 |
| `fn put(...)` — dead-entry sweep, in-place update, oldest-first evict | :199 | :255 |

**The third copy the audit does not name:**
`crates/rs_cam_core/src/geom_cache.rs` (460 lines) is the same weak-identity
memo pattern again — `CAPACITY` at :112, `struct Entry` at :141,
`Entry::matches` at :150, `fn table()` at :169, a stats struct at :176, SIX
`AtomicU64` counters at :185-190, `stats()` :196, `reset_stats()` :209,
`cache_len()` :224, `clear()` :231, `get()` :244, `put()` :253 with the same
sweep and the same `while table.len() >= CAPACITY { table.remove(0); }`
eviction. It differs in one design choice: it holds ONE entry per mesh with
three derivation slots, and its `get`/`put` take closures. Its module doc
(`geom_cache.rs:19-45`) contains the full ABA-soundness argument that
`reach_map_cache.rs:16-29` and `tier_map_cache.rs` restate in shorter form.

**Counted duplication.** In `reach_map_cache.rs` the memo infrastructure —
lines 83-146 and 191-219, excluding the domain key and the compute call —
is **82 non-blank lines**. `tier_map_cache.rs` carries the same 82 lines
plus `peek_tier_map`. `geom_cache.rs` carries a closure-generic variant of
the same. So roughly **250 lines across three files**, of which a private
generic weak-identity memo could remove on the order of 160.

`finish_surface_cache.rs` (523 lines) is the content-keyed cache the audit
says to preserve. **UNVERIFIED** — I read its line count and its name only.
The recommendation to leave it alone is unchallenged by this lane, not
confirmed by it.

### What changed since the audit

Nothing. `git log 5087490f..HEAD` returns no commit for either file the
audit names.

### Is the recommendation still the right shape?

Yes — "a small private generic weak-identity memo, keeping domain-specific
keys and computation separate" describes exactly what the three files
share. Three corrections for the plan:

1. **Scope it to three files, not two.** `geom_cache.rs` is the oldest and
   the largest of the three, and it holds the soundness argument the other
   two cite. Creation dates by `git log --diff-filter=A`: `geom_cache.rs`
   **2026-08-20**, `tier_map_cache.rs` **2026-08-27**, `reach_map_cache.rs`
   **2026-09-08**. The audit's "new caches copy the same infrastructure" is
   right about the direction of copying. If it is left out, the generic is written against the two
   younger copies and the original stays a fourth shape.
2. **The generic must carry `peek`.** `tier_map_cache::peek_tier_map`
   (`tier_map_cache.rs:238`) is a lookup that never builds, never inspects
   the cancel token and **never counts as a hit or a miss**. It exists for
   `plan_multitool_finishing`, whose contract forbids the walk, and its
   `None` means "not measured". A generic without it silently changes that
   contract.
3. **`geom_cache`'s entry shape does not fit a one-key-one-value generic.**
   One entry, three slots, six counters, closure-based accessors. Either
   the generic takes the closure form, or `geom_cache` becomes three
   instances of a simpler generic and its "one entry per mesh" bound is
   lost. That is a design decision to take before writing code, not during.

### Tests a Phase 7 refactor must keep green

`crates/rs_cam_core/tests/tier_map_cache_t3.rs`,
`crates/rs_cam_core/tests/reach_map_p5.rs`,
`crates/rs_cam_core/tests/geometry_cache_g8.rs`,
`crates/rs_cam_core/tests/finish_surface_cache.rs`, plus the two in-file
unit tests in `tier_map_cache.rs` (`negative_zero_margin_is_a_distinct_key`
at :283, `ladder_order_is_part_of_the_key` at :301). The first of those two
pins a deliberately conservative key — `-0.0` and `0.0` are distinct keys,
so the memo errs toward an extra miss. A generic that normalises float bits
would break it, and breaking it is a correctness change, not a test fix.

### Counted size estimate

- 3 files, ~1 010 lines total, of which ~250 are the shared shape.
- 1 new private generic module.
- 4 public stats surfaces to preserve (`ReachMapCacheStats`,
  `TierMapCacheStats`, `GeomCacheStats`, and the six-counter split inside
  the last).
- 1 API divergence to carry (`peek`).
- 6 existing test binaries plus 2 in-file unit tests to keep green.
- No behaviour change intended, so a paired before/after of the counters on
  one generate is the acceptance evidence.

---

# Job 2 — Phase 0 inventory

## The table

Phase 0's "Characterization tests to add first" lists eight requirements.
Legend for **Strength**: **runs** = the test drives the production path;
**source** = the test reads production source text and asserts on strings;
**mixed** = both arms present.

| # | Phase 0 requirement | Covered by | Strength | Gap |
|---|---|---|---|---|
| 1 | Equivalent edits through **setter, undo, and optimizer application** invalidate the same dependencies | setter: 13 tests in the G-FRESHSTATE section of `crates/rs_cam_viz/src/controller/tests.rs` (F2.1 — count taken from the UI ledger, UNVERIFIED by this lane, which read the file only by grep); undo: 4 cases in the same file (F2.5, which found undo bypassing `invalidate_tool` in both directions); tool/model rebind: `crates/rs_cam_core/tests/toolpath_rebind_g_mcprebind.rs` (9) + `crates/rs_cam_viz/tests/mcp_rebind_surface_g_mcprebind.rs` (6); model refresh: `crates/rs_cam_viz/tests/model_refresh_invalidates_results_g_rescalestale.rs` (4) | runs | **PARTLY COVERED. The optimizer-application third is missing — see "The snapshot method" below.** Setter and undo are covered at the CONTROLLER level, which is the right level. But I grepped `apply_toolpath_param_snapshot` and `replace_toolpath_config` across `controller/tests.rs`, `crates/rs_cam_viz/tests/` and `crates/rs_cam_core/tests/`: one hit, and it is a **comment** (`controller/tests.rs:1114`). No test in either crate names either session mutator, and no test drives the optimizer's candidate-apply path to its invalidation. `crates/rs_cam_viz/tests/apply_contract_a3.rs` pins the *apply funnel* — which value gets written — not the invalidation the write must cause. The behaviour to characterize is also KNOWN WRONG on one of the two methods. Write the requirement as a three-way EQUIVALENCE assertion, not as three goldens. |
| 2 | **Disabled cached** operations do not enter exports | `crates/rs_cam_viz/tests/export_refuses_ungenerated_op_g_exportskip.rs:356` — `a_disabled_op_with_no_result_is_skipped_and_the_export_succeeds`; `crates/rs_cam_viz/tests/generate_all_skips_disabled_g_genalldisabled.rs` (2 arms per the UI ledger; UNVERIFIED — I read the file name and grepped it, not its bodies) | runs | **GAP, and it is precisely the word "cached".** The existing arm covers disabled + **no** result. I grepped both export sentries and `..._g_genalldisabled.rs` for `enabled: false` / `enabled = false`: **no hit** — the disabled config is built by a helper, and no test constructs a disabled op that HOLDS a result. The production filter exists (`crates/rs_cam_viz/src/io/export.rs:168`, `if !tc.enabled { return None; }`) and is untested for that case. One arm closes it. |
| 3 | **Core and GUI export agree** on coolant, RPM, tools, and datums | datums: `crates/rs_cam_core/tests/export_datum_setup_frame.rs`, `crates/rs_cam_core/tests/setup_datum_round_trip_p2.rs`; post dialect + M-code filter: `crates/rs_cam_core/tests/post_format_round_trip_p1.rs`; **the one test that does cross the boundary**: `crates/rs_cam_viz/tests/modulated_feeds_reach_gcode_g_modexport.rs`, whose own header states the case — "the core-side F-036b net already asserted bytes … through `rs_cam_core::gcode::export_gcode_checked`, i.e. the CLI path … Nothing crossed the viz export boundary. This file does"; refusal-text parity between the pre-flight modal and the export: `..._g_exportskip.rs:388`, `..._g_stalexport.rs` (point 5) | runs | **LARGEST GAP IN THE LIST.** I grepped `export_gcode_checked\|export_gcode_multi_setup\|export_gcode_phases` across `crates/rs_cam_viz/tests` and `crates/rs_cam_viz/src`. Four files name the core door: `modulated_feeds_reach_gcode_g_modexport.rs`, `wizard_e2e.rs`, `src/io/export.rs` and `src/state/runtime.rs`. `g_modexport` crosses the boundary for **one property** — the modulated per-move feed schedule. `wizard_e2e.rs` exercises the wizard's backend through the viz helpers and validates the file; it compares no second door. So the cross-door pattern EXISTS and covers one dial. It does not cover **coolant, per-operation RPM, tool changes or datums**. The "coolant" grep hits across 60+ test files are fixture fields (`coolant: CoolantMode::Off`); `post_format_round_trip_p1.rs` asserts an `M7` **post-gcode snippet**, which is the dialect filter, not the coolant dial. |
| 4 | **Generation agrees across session and GUI worker** entry points | `crates/rs_cam_viz/src/controller/results_parity_tests.rs` (TD3 B-5, ledger row G-RESULTS) pins both halves of the *store*: `drain_compute_results` → `ProjectSession::insert_result`, and the *read* — that `build_mcp_diagnostics` publishes the core `ToolpathDiagnostic` rather than hand-building from `gui.toolpath_rt`. `crates/rs_cam_viz/tests/generate_all_fixpoint_parity.rs` pins the shared fixpoint DECISION (`plan_fixpoint`) and the GUI entry's arm/refuse ends | runs | **GAP: no GEOMETRY parity.** The fixpoint test's own header says what it asserts — "the shared decision, not the loop". I grepped `generate_via_core` across `crates/rs_cam_viz/`: it is defined at `src/compute/worker/execute/mod.rs:43`, called once in production at `:636`, and called by **four in-file tests** at `:1224, :1290, :1317, :1358`. Those tests assert per-operation properties (tab placement, cut direction, a scallop tool refusal); **none compares its output against a `ProjectSession` generation of the same config**. That is exactly finding 3's ownership split (`session.results` vs `gui.toolpath_rt`), and Phase 1B/1C acceptance depends on it. |
| 5 | **Import and reload agree** on geometry and metadata | `crates/rs_cam_core/tests/model_units_survive_reload_g_unitsreload.rs` — the strong kind: `the_two_load_paths_agree_on_scale` asserts EQUIVALENCE across the two doors rather than a magic size; `crates/rs_cam_core/tests/model_path_round_trip_g_modelrelink.rs` (9); `crates/rs_cam_viz/tests/model_refresh_carries_targets_g_reloadtargets.rs` (5); `crates/rs_cam_core/tests/step_import.rs`, `crates/rs_cam_core/tests/step_project_load.rs` | runs | **GAP with a NAMED, VERIFIED, REACHABLE DEFECT — see "The STEP scale answer" below.** The units sentry covers DXF and SVG only (its fixtures are `dxf_fixture()` and `svg_fixture()`). **STL and STEP are not in it.** The STEP arm of the project door drops the scale, and the CLI job route reaches that arm. |
| 6 | **Unsupported parameter writes fail** rather than silently succeed | `crates/rs_cam_core/tests/schema_enum_values_g_schemaenum.rs` (G-SCHEMAENUM, `32a1fd46`) round-trips every advertised enum value through the registry; `crates/rs_cam_viz/tests/mcp_authoring_surface.rs` pins the SCHEMA and DESCRIPTION text the LLM receives; `crates/rs_cam_core/tests/op_precondition_static_validation_f015.rs`; `crates/rs_cam_core/tests/op_model_ref_static_validation_f023.rs`; rebind refusals in `toolpath_rebind_g_mcprebind.rs` | runs (schema arms are declarative, not string pins) | **PARTLY COVERED.** The arch STATUS row for 4B is right and this lane confirms it: nothing **validates** the `enum:a\|b\|c` schema string on write — it is documentation, and `g_schemaenum` proves the advertised values are *holdable*, not that an unadvertised one is *refused*. F1.16 in the UI programme's follow-on list is the live instance (`ProfileSide` advertises `on`, serde rejects it). No test asserts that a write of an unsupported KEY on a supported operation is fallible rather than a no-op. |
| 7 | **Disconnected finishing runs** have the required retract/entry structure | `crates/rs_cam_core/tests/capability_link_moves_safety.rs`; `crates/rs_cam_core/tests/retract_intent_move_type_census_w6.rs` (the census sentry); `crates/rs_cam_core/tests/entry_moves_stock_aware_g_rampterrain.rs`; `crates/rs_cam_core/tests/isoclip_entry_ramp_g_isoclipentry.rs`, `..._g_isocliprapid.rs`; `crates/rs_cam_core/tests/scallop_intra_pass_relink_am7.rs`; `crates/rs_cam_core/tests/link_stage_g_linkstage.rs`; `crates/rs_cam_core/tests/scallop_trace_survives_relink_g_linktrace.rs` | runs | **GAP on the operation the audit accuses, and the audit's description of it is ACCURATE — I read the emitter.** `crates/rs_cam_core/src/spiral_finish.rs:248-297`: per run it emits `rapid_to_with_intent((x, y, safe_z), Linking)` → `feed_to_with_intent(first_point, plunge_rate, EntryPlunge)` → cutting feeds, and **no per-run retract**; `tp.final_retract(params.safe_z)` runs once after every run (`:299`). So the next run's climb out of material is fused into a single rapid to a new XY at safe_z. Coverage: I grepped the 13 in-file tests and the 9 test binaries naming spiral. `safe_z_respected` asserts only the FIRST and LAST move's Z. `spiral_finish_no_chord_across_disjoint_patch_gap` asserts no cutting-feed chord across a gap — adjacent, and not the same contract. `spiral_finish_compact_c1.rs` is a Track C efficiency instrument by its own header. **No test asserts spiral's per-run approach / plunge / body / retract.** Plan 5B says "the suspected spiral transition defect needs reproduction before its fix is characterized"; nothing reproduces it today. |
| 8 | **Timing totals include drilling** and remain consistent after modulation | `crates/rs_cam_core/tests/drill_cycle_time_integration_g_drilltime.rs` (G-DRILLTIME); `crates/rs_cam_core/tests/air_cut_one_time_base_g_airdenom.rs` (carries its own pre-fix reproduction in the same run); `crates/rs_cam_core/tests/feed_modulation_cycle_time_f036c.rs`; `crates/rs_cam_core/tests/machine_kinematics_cycle_time_f034.rs`; `crates/rs_cam_core/tests/adaptive_feed_modulation_pipeline_f036b.rs` (the one binary red by design); `crates/rs_cam_viz/tests/cycle_time_basis_g_timeest.rs` | runs | **BEST-COVERED OF THE EIGHT.** Two residual gaps, both named in the arch STATUS 6A row and neither pinned: (a) `SimulationSemanticCutSummary`, `SimulationCutHotspot` and `KinematicsSummary::cutting_runtime_s` are still MIXED-BASE after G-AIRDENOM and have no sentry saying so; (b) G82 dwell is in `DrillToolpathSummary::dwell_time_s` and in no total — nothing adds the two, and no test asserts that it does not. |

## Phase 0's other two asks

| Ask | State | Gap |
|---|---|---|
| A compact representative **fixture set** covering nine listed cases | `crates/rs_cam_core/tests/fixtures/` and `crates/rs_cam_core/tests/common/` exist; `crates/rs_cam_core/tests/common_fixtures_smoke_c6.rs` is the smoke over them; `crates/rs_cam_viz/tests/fixtures/` for the GUI side. Named coverage exists for: lateral setups (`lateral_setup_end_to_end.rs`, `lateral_scrub_playback_stock_g_lateralscrub.rs`), flipped setups (`flipped_setup_axial_doc_repro.rs`, `profile_side_survives_setup_flip_g_profile_flip.rs`), identity setups (`identity_setup_emission_frame_audit.rs`, `dexel_stock_z_frame_f024.rs`), nonzero stock origin (`safe_z_emission_frame_g_safez_local.rs` — the origin_z > 5 case), datums (`export_datum_setup_frame.rs`, `setup_datum_round_trip_p2.rs`), drilling and pin drilling (`drill_op_step3.rs`, `pindrill_emission_frame_g_pindrill.rs`), remaining-stock chains (`generate_all_fixpoint_parity.rs`, `sim_prefix_memo_s5.rs`), modulation on/off (`f036a`/`f036b`/`f036c`), save/reload (`model_units_survive_reload_g_unitsreload.rs`, `post_format_round_trip_p1.rs`) | **The fixtures are SCATTERED, not a set.** Each was built for its own sentry. Phase 0's value here is not new fixtures — it is one shared, named, reusable set, so a later refactor can run every characterization against the same nine cases. **Tool changes, per-operation RPM and coolant have no fixture at all** on the cross-door question (see requirement 3). |
| **Registry-driven** coverage for operation availability, "rather than another handwritten operation count" | The registry exists and is the right source: `for_each_op!` at `crates/rs_cam_core/src/compute/catalog.rs:149` generates `OperationType` (:222) and the config dispatch (:256); `OperationType::registry_entry` at :299. Seven test binaries already drive off it: `schema_enum_values_g_schemaenum.rs`, `depth_beyond_stock_core_g_depthstockcore.rs`, `pinned_bottom_z_reaches_motion_g_bottompin.rs`, `reach_map_p5.rs`, `reach_tolerance_source_p5_1.rs`, `adversarial_2d_campaign_r2.rs`, `chipload_thinning_magnitude_survey.rs`, plus `crates/rs_cam_viz/tests/mcp_authoring_surface.rs` and `crates/rs_cam_viz/tests/bottom_z_pin_note_g_bottompin.rs` | **Mostly paid.** The pattern is established and the plan should say "extend it", not "introduce it". |

## The STEP scale answer — the arch STATUS's UNVERIFIED CLAIM, settled

`planning/arch_consolidation_2026-09-09/STATUS.md` marks one claim
UNVERIFIED and asks that it be checked before Phase 4A is scoped:
"Interactive STEP loading applies the requested scale. Project STEP loading
does not apply its computed scale."

**I read both arms. The claim is TRUE.**

- Interactive door — `crates/rs_cam_core/src/io.rs:104-124`:
  ```
  ModelKind::Step => {
      let mut enriched = crate::step_input::load_step(path, 0.1)...;
      if (scale - 1.0).abs() > 1e-9 {
          enriched.apply_uniform_scale(scale);
      }
      ...
      units: Some(ModelUnits::Millimeters),
  ```
  It applies the scale, then **rewrites the model's declared units to
  `Millimeters`**, because the scale is now baked into the geometry.

- Project door — `crates/rs_cam_core/src/session/project_file.rs:665-686`:
  the `ModelKind::Step` arm calls `load_step` and returns
  `LoadedGeometry::Enriched(enriched)`. The `scale` computed at
  `project_file.rs:617-621` is **never used in this arm**. The STL, DXF and
  SVG arms all consume it; STEP does not.

- And the project door stores the units it was GIVEN
  (`project_file.rs:858` `let model_units = model_section.units;`, written
  back at :876/:898/:921/:942), so it does not perform the interactive
  door's normalisation either.

**Reachability — this is the part that decides the priority, and it is not
what I expected.**

- **Through the GUI: NOT reachable.** `rescale_model`
  (`crates/rs_cam_viz/src/controller/io.rs:70`) returns `Ok(None)` early
  for `ModelKind::Step` (`io.rs:83-85`), and `import_step_path`
  (`io.rs:54`) passes scale `1.0`. So every STEP model a GUI session
  creates carries `units: Some(Millimeters)`, the project file saves
  `Millimeters`, and the reload's missing scale is a no-op. The two doors
  agree only because the interactive door rewrites the units.
- **Through the CLI: REACHABLE.** `crates/rs_cam_cli/src/job.rs:524` builds
  `let units = op.scale.map(ModelUnits::Custom);` and calls
  `LoadedModel::from_file(...)`. `LoadedModel::from_file`
  (`crates/rs_cam_core/src/session/mod.rs:245-261`) routes to
  `project_file::load_model_geometry` — the **project** door. So a CLI job
  file with `scale = …` on a `.step` input **silently ignores the scale**,
  while the same scale on a `.stl`, `.dxf` or `.svg` input is applied. The
  comment beside it (`job.rs:523`) says "Pre-T9 only the STL ops honored
  `scale`; keep that scoping" — that scoping was already widened for DXF
  and SVG by G-UNITSRELOAD, and STEP was not included.
- **Through a hand-edited project TOML: REACHABLE.** A `[[model]]` section
  with `kind = "step"` and `units = "inches"` loads unscaled.

**Conclusion for Phase 4A.** The defect is real and is the same class as
G-UNITSRELOAD, but it is a **CLI and file-format** defect, not a GUI one.
It should be scoped as one arm added to
`model_units_survive_reload_g_unitsreload.rs` (which today covers DXF and
SVG only) plus the missing `apply_uniform_scale` call, and it should NOT be
used to argue that the GUI ships a silent 25.4× error today, because the
GUI has no door that writes non-millimetre units onto a STEP model.

Note also a **third import door** the audit's pair does not name:
`LoadedModel::from_file` (`crates/rs_cam_core/src/session/mod.rs:245`)
constructs a synthetic `ProjectModelSection` and calls the project door. It
is the CLI's route. Phase 4A's "one imported-geometry bundle" must account
for three callers, not two.

## The snapshot method — requirement 1's actual contract

The arch STATUS row for finding 2 says both mutators are still defective.
**That row is now half stale, and the half that stands is worse than the
row states.** I read both methods.

- `ProjectSession::replace_toolpath_config`
  (`crates/rs_cam_core/src/session/mutation.rs:1205-1223`) **DOES
  propagate.** It ends with `self.invalidate_result_chain(index, enabled)`,
  and the comment beside it gives the reason: "R0.1 §4.3: a wholesale
  config replacement is an input edit like any other, so it invalidates the
  downstream stock chain too, not only this toolpath's own slot." That is
  UI-programme work from the last two days. The arch STATUS row should drop
  `replace_toolpath_config` from its "STILL OPEN" list.

- `ProjectSession::apply_toolpath_param_snapshot`
  (`crates/rs_cam_core/src/session/mutation.rs:1234-1250`) **does NOT.** Its
  body writes `tc.operation`, `tc.dressups` and `tc.face_selection`, then
  calls `self.drop_result(index)` and sets `self.simulation = None`. It
  never calls `invalidate_result_chain` and never calls
  `invalidate_output_dependents` (`mutation.rs:246`, the propagation half).
  So a downstream operation that machines the stock this one leaves keeps
  its cached result.

- Its own doc comment (`mutation.rs:1256-1258`) names both callers: "This
  is the session-API path used by the GUI undo stack **and by the
  optimizer's candidate-apply path**." `set_feeds_provenance`
  (`mutation.rs:1253-1266`) confirms the second — its doc says it stamps
  the optimizer's provenance "after the operation values themselves are
  installed via `apply_toolpath_param_snapshot`", and that it does not
  touch the caches "because the snapshot call already invalidated them".
  The snapshot call invalidated ONE slot.

This is requirement 1 exactly. Two doors that must have equivalent
dependency effects have measurably different ones, and one of the two is
the optimizer. Write the characterization as an EQUIVALENCE — the same
logical edit through the setter, through undo and through optimizer
application must drop the same set of results — not as three goldens.
Three goldens would freeze the snapshot method's behaviour as intended,
which the plan's own line forbids.

## Hand-rolled mirrors — the class, counted

Phase 0 exists to produce evidence a refactor cannot silently break. A test
that mirrors a list, or asserts on production source text, breaks that
promise quietly: it keeps compiling and passing after the thing it mirrors
has moved.

### Confirmed parallel copies

| Sentry | Where | What it mirrors | Why it matters |
|---|---|---|---|
| `gui_and_mcp_diagnostic_ids_match` (**known, J8.4**) | `crates/rs_cam_viz/src/ui/properties/operations/mod.rs:2657` | The GUI side of the test **hand-rebuilds the session's own inputs** — `validate_one_toolpath`, `feeds_result_for_toolpath`, `ResolvedHeights::from_context`, `project_load_report`, and a replicated `PreconditionContext` — to compare against `session.diagnose_toolpath_with_trace`. Its own comments say "replicate", "match the session's … computation". | It compares two lists it assembles two ways. It cannot fail when the session path changes, because the test's own copy is updated by whoever changes the session path — or is not, and the test still passes. J8.4 is accurate. |
| `ResolvedHeights::from_context` in that same test | same file | The test's "GUI arm" calls `ResolvedHeights::from_context` — the exact call `session/compute.rs:4269` uses on the **MCP** route, and the one **J8.1 records as dropping a pinned Top Z** | J8.1 also records that the GUI **production** path now hands core the entry's RESOLVED heights and does not have that gap. So this arm no longer models the GUI path at all: **it models the MCP path.** The test compares MCP against a replica of MCP, under a name that promises GUI-versus-MCP. |

### Source-level string pins — the weaker kind, 10 files

These read production source and assert on its text. Each is legitimate
where a behaviour cannot be reached from an integration test (a `pub(crate)`
item, an egui call site), and each is fragile in the same way.

| File | Pinned source |
|---|---|
| `crates/rs_cam_viz/tests/bottom_z_pin_note_g_bottompin.rs:144` | **Self-declared**: the function is literally named `the_heights_tab_consults_the_note_weaker_source_level_arm`. The repo already knows this arm is the weak kind. |
| `crates/rs_cam_viz/tests/apply_contract_a3.rs:171-204` | **10** `include_str!` of production modules. Its own header (line 26) says "controller-level integration, plus two source-level sentries" and that the button → `AppEvent` mapping is "read from source … rather than clicked". |
| `crates/rs_cam_viz/tests/freshness_surfaces_g_freshrender.rs:27-32, 223` | **7** production sources including `src/io/export.rs` |
| `crates/rs_cam_viz/tests/open_guard_asks_before_discarding_g_openguard.rs:39-41` | 3 sources |
| `crates/rs_cam_viz/tests/overlays_registry.rs:44` | Parses `src/state/viewport.rs` for `pub <name>: bool` fields. **Header is honest about it**: "Two of these read SOURCE rather than behaviour, on purpose … the trigger lives in a `pub(crate)` struct an integration test cannot name." The other arms are registry-driven and strong. |
| `crates/rs_cam_viz/tests/workspace_menu_complete_g_wsmenu.rs:42` | `src/ui/menu_bar.rs` |
| `crates/rs_cam_viz/tests/inspector_header_wraps_g_reachwrap.rs:67` | `src/ui/properties/mod.rs`. The ledger row records it was "re-quoted after the rename" — the failure mode already happened once. |
| `crates/rs_cam_viz/tests/mcp_toasts_report_outcome_g_mcptoast.rs:37` | `src/app/mcp.rs` |
| `crates/rs_cam_viz/tests/boundary_controls_always_visible_g_boundaryinherit.rs:68` | reads UI source at run time |
| `crates/rs_cam_viz/tests/ui_string_hygiene.rs:182` | sweeps UI sources for string rules |

### One more the ledger already flags

`crates/rs_cam_core/tests/save_temp_path_unique_f124.rs` — the UI
programme's own F1.25 row records that its arm
`the_temp_name_is_not_the_pid_alone` **knows the pre-fix name**, so a
future rename of the temp prefix makes that arm vacuous. The lane recorded
it in the sentry's own header. That is the right handling and it is the
model the other ten should follow: if an arm is the weaker kind, the file
should say so where a reader will meet it.

## Headline — how much of Phase 0 is already paid

Counting the eight characterization requirements:

- **Well covered, run-the-path evidence, little to add: 1 of 8** —
  requirement 8 (timing with drilling and modulation), with two named
  residual gaps.
- **Substantially covered, one third missing: 3 of 8** — requirement 1
  (setter and undo covered, optimizer application missing and the target
  behaviour is known-wrong), requirement 5 (DXF and SVG covered, STL and
  STEP missing, and STEP carries a verified reachable defect), requirement
  6 (values covered, refusal of unsupported writes not).
- **Adjacent tests exist but do NOT assert the stated contract: 3 of 8** —
  requirement 2 (the word is "cached" and no test builds a disabled op that
  holds a result), requirement 4 (the store is pinned, geometry parity is
  not), requirement 7 (link and entry structure is well covered across the
  suite, and the one operation the audit accuses — spiral — has no run-
  structure assertion).
- **Largest single item, one dial of many covered: 1 of 8** — requirement
  3, core versus GUI export agreement. Exactly one test crosses the two
  doors (`modulated_feeds_reach_gcode_g_modexport.rs`) and it covers one
  property, the modulated feed schedule. Coolant, per-operation RPM, tool
  changes and datums have no cross-door assertion.

Two asks beyond the eight: the **registry-driven coverage** ask is mostly
paid (nine binaries already drive off `for_each_op!`; the plan should say
"extend", not "introduce"). The **fixture set** ask is not paid as a *set* —
the cases exist, scattered, one per sentry, and tool change / RPM / coolant
has no fixture on the cross-door question.

So Phase 0's measured position is this. Roughly **half of the stated evidence already
exists and runs the path**, one requirement is close to untouched, and the
inventory's real product is not "write eight new tests" but "write two
cross-door parity tests, one spiral run-structure reproduction, three small
missing arms, and REWRITE requirement 1 as an equivalence rather than three
goldens of a known-wrong behaviour."

