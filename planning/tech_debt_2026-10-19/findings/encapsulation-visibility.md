# Findings: encapsulation, visibility and escape hatches

Category `encapsulation-visibility`. Baseline 99b6fb41, branch `geo-select`.
Read-only review. Every fact below comes from `rg` or a file read at the
path:line given. Caller counts come from `rg`, not from the graph tools.

Summary: 11 findings. 3 are tier B, 1 is tier C, 1 is tier D, 6 are tier E.
One finding (11) is a note for the geometry-selection programme.

| # | Title | Tier | Cost | In flux | Known id |
|---|---|---|---|---|---|
| 1 | The multi-tool planner is a second write door that drops its `Effects` | B | M | yes | — |
| 2 | The viz mirrors `Effects` by hand at each site; the MCP core route does not clear the viewport simulation | B | M | yes | WP19 |
| 3 | Three `pub` feeds writers bypass the "single write" type gate | B | S | no | — (core-feeds map lead) |
| 4 | `apply_stale_default_fix` is a public session writer with no command and no invalidation | C | S | yes | — |
| 5 | Test doors: two conventions, and the sentry pins a fixed list | E | M | partly | S25, FLD-05, L12 |
| 6 | Every module is `pub mod`, so no lint can see over-wide `pub`; S29 regrew | E | L | partly | S29, T-1 |
| 7 | Invariant-bearing core types publish their fields | E | M | yes (`execute.rs`) | — |
| 8 | `detect_rest_valleys` is `pub`, but a rule says only `detect_rest` may call it | E | M | no | — |
| 9 | Two `pub(crate)` reason strings force viz to copy them as literals | D | S | no | — |
| 10 | One sealed trait; four core traits are `pub` with no outside user | E | S | no | — |
| 11 | `ZRange` and `DiameterRange` publish raw fields; MCP builds them unchecked | E | S | yes | selection programme |

---

## 1. The multi-tool planner is a second write door that drops its `Effects`

**Tier B · cost M · in flux (`session/multitool.rs`).**

**Claim.** `ProjectSession::plan_multitool_finishing` is a `pub fn (&mut self)`
that adds and removes toolpaths. It is not a `Command` row and it returns no
`Effects`. The viz door that calls it mirrors neither half of `Effects`. The
GUI remove door for the same toolpath mirrors both.

**Evidence.**
- `crates/rs_cam_core/src/session/multitool.rs:338` declares the `pub` method.
  It removes prior planner ops at `:815-844` and appends new ops through the
  raw half `add_toolpath_impl` at `:363`.
- `multitool.rs:835` calls `self.remove_toolpath(index)` in the shape
  `if let Err(e) = …`. The `Ok(Effects)` answer has no binding. The WP19
  sentry names this exact shape as a discard
  (`crates/rs_cam_viz/tests/effects_are_stamped_wp19.rs:17-21`), but it reads
  viz files only.
- Both raw halves drop the simulation:
  `crates/rs_cam_core/src/session/mutation/toolpath.rs:64` and `:122`.
- `crates/rs_cam_viz/src/controller/events/planner.rs:56-101`
  (`apply_multitool_plan`) does not call `invalidate_simulation` and does not
  call `stamp_stale`. `rg "invalidate_simulation|release_for_new_run"` on
  `planner.rs` returns nothing. It calls `mark_derived_rest_dependents_stale`
  instead.
- The GUI remove door, `crates/rs_cam_viz/src/controller/events/toolpath.rs:447-457`,
  takes `apply_quietly` (which mirrors `stale` and `simulation_cleared`,
  `controller/events/mod.rs:100-111`). Its comment says W1 retired the
  `mark_derived_rest_dependents_stale` twin because `Effects::stale` names
  every consumer. The planner still uses the retired twin.
- `crates/rs_cam_viz/src/controller/CLAUDE.md` states: "The view mirrors
  `Effects`. Use `Effects.stale`; never a narrower answer." and "A toolpath
  edit clears the viewport simulation."
- `crates/rs_cam_core/tests/setters_have_rows_wp15a.rs:96-101` lists the
  method in `COMPUTE_DOORS`. That allowlist is for the `Job` programme's
  compute entry points. This method is a mutation, not a compute job.

**Consequence (read in code, not run).** After Apply in the planner dialog or
MCP `plan_multitool_finishing`, the core holds no simulation and the viewport
still holds one. `SimFreshness` derives `EditedSince` for it
(`crates/rs_cam_viz/src/state/freshness.rs:127-133`), so the label is honest,
but the picture is the one WP19 forbids. Downstream ops that the removal
invalidated get no `stale_since` stamp, so the auto-regen sweep does not queue
them (the WP5 class, `ui/properties/mod.rs:814-817`).

**Fix.** Add a `Command::PlanMultitoolFinishing` row (boxed spec, as
`PreviewTierMapArgs` does at `session/command.rs:1092-1096`) that returns
`Effects` plus the outcome. Make the method `pub(crate)` and take it off
`COMPUTE_DOORS`. Inside, fold the `Effects` of each remove and add into one
answer through `try_with_effects`. The viz door then takes
`apply_controller_command`. Enforcement now: CONVENTION (an allowlist row).
After: TYPES (the exhaustive `apply` match and `#[must_use] Effects`).

---

## 2. The viz mirrors `Effects` by hand at each site; the MCP core route does not clear the viewport simulation

**Tier B · cost M · in flux (`app/mcp/commands.rs`, `app/mcp.rs`, `ui/properties/mod.rs`, `ui/properties/panel_apply.rs`).**
**Known: WP19 (sentry `effects_are_stamped_wp19`).**

**Claim.** `AppState.session` is a `pub` field, so every viz site can call
`session.apply(...)` and then choose what to mirror. `rg` finds 25 production
`session.apply(`/`session.start(` sites in viz, in at least five mirroring
shapes. The WP19 sentry sees only a DISCARDED answer, not a PARTIAL mirror.
The MCP core route mirrors `Effects::stale` on every arm but
`simulation_cleared` on three arms only.

**Evidence.**
- `crates/rs_cam_viz/src/state/mod.rs:135`: `pub session: ProjectSession`.
- Mirroring shapes:
  `controller/events/mod.rs:65-111` (`apply_controller_command`,
  `apply_quietly`: both halves);
  `controller/io.rs:356-362` (`adopt_post_effects`: both halves plus the post
  mirror);
  `ui/properties/panel_apply.rs:130-140`, `:231`, `:339`, `:384`
  (`panel_side_effects.invalidate_simulation` flag);
  `ui/properties/mod.rs:834-846` (both halves only when `stale` is not empty);
  `ui/properties/mod.rs:1609-1617` (both halves only when `revision` is set).
- MCP core route: `app/mcp.rs:373-374` hands the outcome to `describe_core`
  (`app/mcp/commands.rs:2031`). `core_stale` (`:1877-1880`) stamps `stale`.
  `rg simulation_cleared` in `app/mcp/` finds only
  `commands.rs:2836` (`SetSimulationResolution`),
  `commands/stock_changes.rs:454` and `commands/obstacles.rs:343`. The
  `SetToolpathParam` arm (`commands.rs:2276-2301`) and the other toolpath-edit
  arms do not mirror it.
- `crates/rs_cam_viz/src/state/freshness.rs:127-133` records this as fact:
  "`EditedSince` is reached where no door mirrors: the MCP surface, …".
- Contracts that this breaks: `controller/CLAUDE.md` ("A toolpath edit clears
  the viewport simulation"); `app/CLAUDE.md` ("An MCP mutation dispatches
  through the same command path as the UI"); the WP19 file doc
  (`effects_are_stamped_wp19.rs:9-11`: "a session that drops it must not leave
  the viewport showing one").

**Fix.** Make `AppState.session` private. Give the viz one door that takes a
`Command` and returns a `Mirrored<Effects>` (or a unit) only after it has run
`stamp_stale` and the simulation clear. A site that needs a narrower stamp
(the idle-frame guard, `ui/properties/mod.rs:838`) asks the door for it by
name. Read access goes through `&ProjectSession`. Enforcement now: SENTRY
(source scan for discards). After: TYPES (a write is impossible without the
door).

---

## 3. Three `pub` feeds writers bypass the "single write" type gate

**Tier B · cost S · not in flux. Owner: power-calcs (`feeds/**`, per the 09-16 plan).**

**Claim.** The feeds module states that one write function exists and that a
refused pairing cannot reach it. Three more `pub` writers take a raw
`FeedsResult` and write an `OperationConfig` with no `ApplicableRecommendation`.
None has a production caller.

**Evidence.**
- Contract: `crates/rs_cam_core/src/feeds/suggest/apply.rs:663-668` ("There is
  exactly one write function ([`apply`])") and `:812-820` ("**The single
  write.** … because there is one function, a write cannot skip the clamps").
  `ApplicableRecommendation` is constructible only through
  `FeedsPreview::applicable` (`:726-735`, `:739-744`).
- Bypass writers: `apply_feeds_result_to_op` `:390`, `apply_speeds_to_op`
  `:421`, `apply_cut_geometry_to_op` `:452`. All three are re-exported at
  `crates/rs_cam_core/src/feeds/suggest.rs:21-27`. `resolve_operation_invariants`
  `:856` is `pub` and re-exported too.
- Callers (`rg`): only `feeds/suggest/tests.rs`,
  `crates/rs_cam_core/tests/pill_writes_clamped_value_g_pillclamp.rs`,
  `crates/rs_cam_viz/tests/apply_contract_a3.rs`,
  `crates/rs_cam_viz/tests/the_chipload_verdict_is_one_row_g_chipverdict.rs:893`.
  The `crates/rs_cam_cli/src/project.rs:720,1223` hits are comments.
  `resolve_operation_invariants` has one test caller (`suggest/tests.rs:649`).
- A3 census (`apply.rs:656-661`) is the history: 7 of 13 GUI affordances once
  wrote the raw calculator output with no clamp.

**Fix.** Gate the three writers and `resolve_operation_invariants` behind
`#[cfg(any(test, feature = "test-support"))]`, or rewrite the tests to build a
`FeedsPreview` and call `apply` with an `ApplyScope`. Then the doc claim is
true by construction. Enforcement now: DOC. After: TYPES.

---

## 4. `apply_stale_default_fix` is a public session writer with no command and no invalidation

**Tier C · cost S · in flux (`compute/validate.rs`, `session/mod.rs`).**

**Claim.** `compute::validate::apply_stale_default_fix` is `pub`. It writes
`tc.operation` and `tc.feeds_provenance` through the crate-private hatch
`toolpath_configs_mut`. It runs no `Command`, builds no `Effects` and drops no
result, so the cached result stays current after its input changed. Only its
own tests call it. A doc tells consumers to call it.

**Evidence.**
- `crates/rs_cam_core/src/compute/validate.rs:164-175` (the writer);
  `crates/rs_cam_core/src/compute/mod.rs:30` (`pub mod validate`).
- Callers: `validate.rs:631` and `:772`, both in its `#[cfg(test)]` module.
- `crates/rs_cam_core/src/diagnostics/fix.rs:31-33`: "carried so consumers can
  call the existing `apply_stale_default_fix` path".
- It is the ONLY production caller of `toolpath_configs_mut`
  (`session/mod.rs:1809`); the other is `session/mutation/tests.rs:1176`.
  `session/CLAUDE.md` says "There are no public `*_mut` hatches"; this `pub fn`
  is one by another name. `hatches_are_crate_private_wp7.rs` checks the
  hatch's visibility, not who wraps it.
- The GUI fix button does not use it: `ui/properties/tab_badges.rs:283` calls
  `apply_stale_default_to_op` on a draft entry, which then goes through
  `ReplaceToolpathConfig`.

**Fix.** Delete `apply_stale_default_fix`. Move its two tests to
`apply_stale_default_to_op` plus `Command::ReplaceToolpathConfig`. Then put
`toolpath_configs_mut` behind `#[cfg(test)]` as `setups_mut` already is
(`session/mod.rs:1825-1827`), and correct `diagnostics/fix.rs:31-33`.
Enforcement now: CONVENTION. After: TYPES (no production path can reach the
hatch).

---

## 5. Test doors: two conventions, and the sentry pins a fixed list

**Tier E · cost M · partly in flux (`polygon.rs`, `session/multitool.rs`, `feeds/suggest/apply.rs` no).**
**Known: S25 (09-16 plan rank 13: W2 core done `3bd1ac95`, viz HELD), FLD-05, L12.**

**Claim.** A production item that exists for tests takes one of two forms.
About 47 carry `#[cfg(feature = "test-support")]`. About 29 carry only the doc
label "**Test door.** … No production path reads it." and stay in the default
public API. The sentry for the gated form names a fixed list of 24 items in
`maps/` and `surface/`. A `#[doc(hidden)] pub` probe survives beside it.

**Evidence.**
- Count: a script over every `Test door` doc comment in `crates/*/src` found
  47 gated and 31 ungated items (two of the 31 are false matches, for example
  `geo.rs:12`).
- Ungated examples: `polygon.rs:166` `with_holes_closed`;
  `session/multitool.rs:721` `planned_tier_boundary_polys`;
  `export/fingerprint.rs:856`, `:1009`; `compute/toolpath_stats.rs:898`,
  `:1806`, `:1820`; `compute/sim_prefix.rs:792`; `finish/scallop.rs:724`,
  `:970`; `surface/flow_accum.rs:131`, `:203`, `:385`;
  `metrology/census.rs:97`, `:105`; `machine/mod.rs:678` `from_key`.
- Gated examples: `maps/reach_map.rs:1851-1854`, `maps/tier_map.rs:174-176`,
  `surface/slope.rs:76`.
- Sentry: `crates/rs_cam_core/tests/the_test_doors_are_gated_fld0405.rs:1-44`
  pins 24 named declarations. `surface/flow_accum.rs` sits in `surface/` but
  is not in its list.
- `crates/rs_cam_core/src/adaptive3d/mod.rs:37-38`:
  `#[doc(hidden)] pub use path::debug_adaptive_3d_segments_for_f029_probe`,
  with "Will be removed once F-029 lands". Four integration tests bind it
  (`adaptive3d_global_gate_drapes_below_floor.rs:111`,
  `adaptive3d_by_area_matches_global_stock.rs:161`,
  `adaptive3d_planner_never_ahead_of_emitted_path.rs:180`,
  `adaptive3d_boundary_clear_parity.rs:317`).
  `adaptive3d/path.rs:560-563` puts two "Test-only" `pub` fields on a
  production result struct.
- Changed since 09-16: L12 (`19dc6766`) kept `MachineProfile::from_key` `pub`
  because the tests crate is external. FLD-05 then added the `test-support`
  feature, which removes that reason. `from_key` also falls back to the generic
  profile for an unknown key (`machine/mod.rs:670`).
- Instrument `instruments/test_only_pub_api.md` lists 44 rows but does not see
  the `cfg` gate, so some of its rows are already gated (`reach_map_for_mesh`,
  `reset_drop_call_count`).

**Fix.** One rule: a test door carries `#[cfg(feature = "test-support")]`.
Replace the fixed-list sentry with a generic one: every item whose doc says
"Test door" has the attribute in its attribute run. Gate the 29, the F-029
probe and its two fields. Add `required-features` rows for the binding tests.
Enforcement now: DOC plus a SENTRY for 24 named items. After: SENTRY for the
whole class, with TYPES (a cfg gate) at each item.

---

## 6. Every module is `pub mod`, so no lint can see over-wide `pub`; S29 regrew

**Tier E · cost L · partly in flux. Known: S29 (09-16 W4: mcp `ce095f79`, cli `1ef6b7f8`, viz `b5e2cd84`, core `1f9ff765`), T-1 (archetype).**

**Claim.** S29 demoted 199 own-file-only `pub` items on 2026-09-17. The
instrument now finds 141 again (94 core, 45 viz, 1 mcp, 1 mcp_proxy) and 11
with zero references. The compiler cannot help: `dead_code` does not fire on
a `pub` item of a library, and `unreachable_pub` is inert while every module
on the path is `pub mod`. The workspace enables neither `unreachable_pub` nor
`clippy::redundant_pub_crate`.

**Evidence.**
- `instruments/dead_pub_surface.md:5-19` (11 zero-reference rows) and `:21`
  (141 own-file-only rows).
- `git blame` at 99b6fb41 on the 140 definition lines: 123 were last touched
  after 2026-09-17. (Last touched, not introduced: a fmt cascade also moves the
  blame.)
- `crates/rs_cam_core/src/lib.rs`: 34 `pub mod`, 0 private modules.
  `rs_cam_core/src/**/mod.rs`: 209 `pub mod` against 75 private or
  `pub(crate)` modules. `crates/rs_cam_viz/src/lib.rs:3-21`: every module is
  `pub` except `host` and `mcp_lifecycle`, because 115 integration test files
  import viz internals.
- `Cargo.toml` `[workspace.lints.rust]` holds `unsafe_code` only.
- Sample sites: `compute/execute/dressup_apply.rs:182-230` (nine `pub const`
  used in their own file); `budget/guard.rs:113`, `:147`;
  `compute/obstacle.rs:1056`; `feeds/mod.rs:62-67` (`pub static EMBEDDED_LUT`
  beside the getter `embedded_vendor_lut`; only `feeds/suggest/tests.rs` names
  the static).

**Fix.** Per crate, one folder at a time: make child modules `pub(crate)` and
publish the cross-crate surface through explicit `pub use` in the facade.
Then turn on `unreachable_pub` (warn, then deny) for that crate. `dead_code`
then fires on what is left. Viz needs a decision first: keep `pub mod` for
the test surface, or move the GUI-internal tests into the crate. Enforcement
now: NONE. After: LINT.

---

## 7. Invariant-bearing core types publish their fields

**Tier E · cost M · in flux (`compute/execute.rs`).**

**Claim.** Three core types carry an invariant in their docs and publish
every field. No production site breaks the invariant today; nothing stops one.

**Evidence.**
- `DexelGrid`, `crates/rs_cam_core/src/stock/dexel.rs:359-404`: all fields
  `pub`, including `rays` and `conservative_top`. The doc (`:369-399`) says
  `conservative_top` is lowered only when a stamp covers the cell completely.
  `ray_mut` (`:620`) is `pub`. `adaptive3d/path.rs:778` and `:819` lower rays
  through it without touching `conservative_top` (safe only because a stale
  high bound fails safe, `dexel.rs:396-398`). `compute/stock_carry.rs:187-218`
  writes `conservative_top` directly. `TriDexelStock`
  (`dexel_stock/mod.rs:88-120`) publishes `z_grid`, `x_grid`, `y_grid`,
  `stock_bbox`.
- `AnnotatedToolpath`, `crates/rs_cam_core/src/trace/toolpath_spans.rs:457-465`:
  `toolpath`, `spans`, `spans_valid`, `produced` are `pub`. `check_invariants`
  (`:765`) is the only span-bound check; `rg` finds callers in test modules
  only (`arcfit.rs:1298`, `condition.rs:390`, `toolpath_spans.rs:2062`). The
  core-io-output map claim "called from generation" is wrong. Every direct
  field write that `rg` finds is in a `#[cfg(test)]` module.
- `ExecutionContext`, `crates/rs_cam_core/src/compute/execute.rs:318-403`: 22
  `pub` fields, `findings: &RefCell<GenerationFindings>` at `:324`. `record`
  (`:448-450`) is "THE door"; `compute/execute/dressup_apply.rs:105-127`
  writes `findings.borrow_mut().record(..)` beside it. No crate outside core
  names `compute::execute::` in code.

**Fix.** Private fields and methods that keep the invariant:
`DexelGrid::subtract_above(row, col, z)` that keeps `conservative_top` in
step; `AnnotatedToolpath` built only by constructors that check span bounds,
with `toolpath()`/`spans()` readers; `ExecutionContext` fields `pub(crate)`
and `findings` private behind `record`. Enforcement now: DOC. After: TYPES.

---

## 8. `detect_rest_valleys` is `pub`, but a rule says only `detect_rest` may call it

**Tier E · cost M · not in flux.**

**Claim.** `finish/CLAUDE.md` says "A product rest field comes only from
`rest_detector::detect_rest`". The raw detector it wraps is `pub` in
`surface::rest_field`, so any module can call it.

**Evidence.**
- `crates/rs_cam_core/src/surface/rest_field.rs:1032` (`pub fn detect_rest_valleys`).
- Production caller: `crates/rs_cam_core/src/finish/rest_detector.rs:127`
  only.
- Test callers outside the crate: 15 sites in 8 files under
  `crates/rs_cam_core/tests/` (for example `reach_policy_pr4.rs:130`,
  `rest_routing_probe_e9.rs:178`). These block a plain `pub(crate)`.

**Fix.** Gate the `pub` form behind `test-support` and give core a
`pub(in crate::finish)` re-export, or move the tests to `detect_rest`.
Enforcement now: CONVENTION (doc). After: TYPES.

---

## 9. Two `pub(crate)` reason strings force viz to copy them as literals

**Tier D · cost S · not in flux.**

**Claim.** The core reason strings for "cut metrics not measured" are
`pub(crate)`. The MCP `get_diagnostics` reply in viz copies them as literals.
The core doc promises one answer on two surfaces.

**Evidence.**
- `crates/rs_cam_core/src/session/diagnostics_types.rs:331-334`
  (`CUT_METRICS_NOT_MEASURED_NO_SIMULATION`, doc: "two surfaces give one
  answer") and `:343-344` (`CUT_METRICS_NOT_MEASURED_NO_TRACE`).
- Copies: `crates/rs_cam_viz/src/controller/events/compute.rs:2469` and
  `:2471`.
- One viz test pins one of the two strings by literal:
  `controller/tests/diagnostics_null_without_trace_u2.rs:100`.
- Cold-start task 4 found this as a silent miss ("duplicated literals across a
  visibility wall").

**Fix.** Publish one typed reason (an enum with `as_str`, or a
`ProjectDiagnostics` method that returns the reason) and read it in viz.
Enforcement now: SENTRY (one string). After: TYPES.

---

## 10. One sealed trait; four core traits are `pub` with no outside user

**Tier E · cost S · not in flux.**

**Claim.** Only `trace/transform_provenance.rs` seals its traits. Four core
traits that only core implements are `pub` and unsealed, and no other crate
names them.

**Evidence.**
- Sealed: `trace/transform_provenance.rs:68-69`, `:88`, `:436`.
- `pub trait` with zero use in viz, cli or mcp (`rg -l`): `MetricEvaluator`
  (`tool_load/mod.rs:472`, 0 outside files), `OpParams`
  (`compute/catalog/schema.rs:133`, 0), `ParamType` (`schema.rs:144`, 0).
  `OperationParams` (`compute/catalog.rs:673`) is implemented only by
  `impl_operation_params!` (`compute/operation_configs.rs`) and named in 2
  viz files.
- Excluded on purpose: `MillingCutter` (implemented twice in
  `crates/rs_cam_core/tests/tool_scale_semantics_pr2.rs`), `UsageProbe` and
  `ToolpathPhaseSink` (implemented in `rs_cam_viz/src/compute/worker.rs`).

**Fix.** Make `MetricEvaluator`, `OpParams` and `ParamType` `pub(crate)` if
the derive output names a crate-local path; otherwise seal them. Seal
`OperationParams`. Enforcement now: NONE. After: TYPES.

---

## 11. `ZRange` and `DiameterRange` publish raw fields; MCP builds them unchecked

**Tier E · cost S · in flux (`selection/rule.rs`, `rs_cam_mcp/src/server.rs`). Selection programme note.**

**Claim.** The new selection filter types have `pub` fields and no
constructor. The MCP wire conversion builds them by literal. An inverted Z
range, a negative tolerance or a NaN is representable and matches nothing,
with no refusal.

**Evidence.**
- `crates/rs_cam_core/src/selection/rule.rs:43-52` (`DiameterRange`,
  `contains` = `|d - nominal| <= tolerance`) and `:78-87` (`ZRange`,
  `contains` = `min <= z <= max`).
- `crates/rs_cam_mcp/src/server.rs:986-989` and `:1017-1020` copy the wire
  fields straight in.

**Fix.** Private fields with `ZRange::new(min, max) -> Result<_, _>` and
`DiameterRange::new(..)`; `TryFrom` on the wire params; a `SelectionRefusal`
cause for the bad range. Enforcement now: NONE. After: TYPES.

---

## Not filed, on purpose

- The three session `*_mut` hatches: WP7 and S30 closed them. Finding 4 is the
  residue.
- The six compute doors in `COMPUTE_DOORS` other than the planner: §25
  ruling 4 permits them, and `generate_toolpath` goes through `AdoptResult`
  (`session/compute.rs:2554-2584`).
- `MachineProfile::from_key` as a defect on its own: L12 ruled it. Finding 5
  records why that ruling can now change.
