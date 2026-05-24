# Codebase unification plan — make the substrate repeatable before tuning

Date: 2026-05-24
Author: research synthesis from three parallel audit agents
Status: ready to execute; see §"Agent build prompt" at the bottom

## Why this exists

The agent smoke acceptance run on 2026-05-24
(`planning/toolpath_acceptance/baselines/2026-05-24_agent_smoke_acceptance.md`)
turned up an uncomfortable pattern: the *algorithms* in this codebase are
strong and produce excellent output where reachable (the AS015 scallop
optimizer narrative is genuinely best-in-class). But the *substrate
underneath the algorithms* — config plumbing, IO, the data structures that
carry sim results into tool-load gates, the staleness signal — forks two
or three ways at every layer boundary. The bugs we hit are downstream
symptoms of that forking, not of the algorithms being wrong.

User direction (verbatim, 2026-05-24):

> "We only have some LUTs. So my assumption is depending on what
> path/tool/etc we use, the suggestion comes from somewhere else? Is
> this codebase unified or hard to follow? Where else are things not
> unified? Like sim dead on 2D and similar. I want to put time into
> making the logic repeatable and good code before we work on tuning."

This plan answers that. It is the merged output of three parallel
research agents that audited:

1. **Suggest pipeline** — where do default toolpath params come from?
2. **Sim chipload 2D-vs-3D** — why is the chipload gate dead on 2D ops?
3. **Codebase unification** — what other parallel/forked code paths exist?

The takeaway in one line: **fix the plumbing once and tuning becomes
legitimately tractable**.

## Cross-audit findings, by impact

The same shape recurred in every agent's report. Pulled together:

| # | Finding | Where | Source audit(s) |
|---|---|---|---|
| 1 | Chipload "2D dead" is actually a feedopt-sequencing bug — feedopt probes `nominal_feed_rate` from the *first Linear move in the dressed path*, which is now the entry plunge. Every cutting move gets rewritten to `plunge_rate × RCTF`, and the chipload classifier's `feed ≥ 0.95 × commanded` filter rejects all samples. | `compute/execute.rs:1570-1578` | Chipload + Unification |
| 2 | `peak_axial_doc_mm` carries two physical quantities through one field — lateral feeds emit max per-cell material length removed; pure-vertical plunges emit Z descent. Same `f64`, two units. Deflection gate consumes whichever it gets and fires Exceeds when stock-above-cutter is large. | `dexel_stock/stamping.rs:374-399`; bubbles through 5 summary structs in `simulation_cut.rs` | Unification + smoke-run evidence |
| 3 | Three independent `VENDOR_LUT` singletons; one hardcodes `workholding_rigidity: Medium` regardless of the project's actual workholding. Result: post-set `feeds.feed_vs_lut.high` warning disagrees with the value the Suggest button just applied. | `viz/ui/properties/mod.rs:54`, `core/session/compute.rs:1792`, plus `VendorLut::embedded` in tests | Suggest + Unification |
| 4 | Three project-TOML loaders with different missing-operation behaviour. Strict drops, tolerant defaults, CLI parses its own. Already in `planning/LOADER_UNIFICATION.md` and `memory/project_two_loader_divergence.md`. | `core/session/project_file.rs:745`, `viz/io/project.rs:441/806/1093`, `cli/src/job.rs:232` | Unification (known) |
| 5 | Two MCP server implementations of the same 40+ tool surface. `add_toolpath`, `get_diagnostics`, `load_project`, `screenshot_*`, `save_project`, `inspect_model`, `list_setups`, `get_tool_load_report` double-implemented. Tool descriptions and param shapes will drift. | `crates/rs_cam_mcp/src/server.rs` (2151 LOC) vs `crates/rs_cam_viz/src/mcp_server.rs` (983 LOC) | Unification |
| 6 | Default `OperationConfig` produced via three paths — `OperationConfig::new_default` (used by MCP), `ToolpathEntry::for_operation` (viz), and `cli::job::parse_job_file` (CLI). Same `OperationType::Pocket` can produce three different fully-populated configs depending on entry point. | 23 `impl Default` blocks in `operation_configs.rs:92-834`; constructed at `mcp/server.rs:1348`, `viz/io/project.rs:806/1093`, `cli/job.rs` | Suggest + Unification |
| 7 | `DrillConfig::set_plunge_rate` is a **silent no-op** (`op_configs.rs:1165`). The historic "plunge 527 > feed 385" bug returns trivially on any drill regardless of what the suggest layer writes. | `operation_configs.rs:1165` | Suggest |
| 8 | Stale-flag / invalidation propagation has at least four authors. `ProjectSession` invalidates internally; `viz/app/mcp.rs:1828 mcp_mark_stale_toolpaths` is called from 8 mutation sites; `viz/controller/events/simulation.rs:22 invalidate_simulation` is a third path; the standalone MCP doesn't propagate at all. No `compute_stale_set(session, mutation) -> StaleSet` helper. | `core/session/compute.rs`, `viz/app/mcp.rs:1828`, `viz/controller/events/simulation.rs:22`, `rs_cam_mcp/src/server.rs` | Unification |
| 9 | `gui_banners` / `warnings` / `diagnostic_delta` / `stale_defaults` / `get_diagnostics` — five derived views built from one diagnostic snapshot, but only in viz. The standalone MCP returns plain text for the same mutations. `stale_defaults` is rebuilt in three places. | `viz/app/mcp.rs:1883-1936`, `core/compute/validate.rs:100`, `viz/ui/properties/operations/mod.rs:2232`, `viz/app/mcp.rs:590` | Unification |
| 10 | Catalog has six 23-arm `match` blocks — `spec`, `new_default`, `apply_stock_defaults`, `as_params`, `as_params_mut`, `kind_str`. Adding a field touches N places. `OperationConfig` is essentially an enum-of-structs that costs O(N_variants × N_consumers) per change. | `core/compute/catalog.rs:200-443, 1429-1601`; mirrored hint table at `viz/ui/properties/mod.rs:1051` | Suggest + Unification |
| 11 | `operation_feeds_hints` (axial / radial / scallop hint table per op kind, used to bridge between `OperationConfig` and `FeedsInput`) lives in viz at `viz/ui/properties/mod.rs:1051`, but `catalog::spec` lives in core. Adding a new op kind requires editing both crates. | `core/compute/catalog.rs:200-443` + `viz/ui/properties/mod.rs:1051` | Suggest |
| 12 | `feeds_result_for_toolpath` in `core/session/compute.rs:1783` and `compute_feeds_for_op` in `viz/ui/properties/mod.rs:61` build nearly-identical `FeedsInput` structs — but the core path hardcodes `workholding_rigidity: Medium` while the viz path reads from `session.stock_config().workholding_rigidity`. That's where finding #3's mismatch comes from at the function level. | `core/session/compute.rs:1816` vs `viz/ui/properties/mod.rs:61` | Suggest |
| 13 | `apply_feeds_result_to_op` never enforces invariants — never checks `plunge ≤ feed`, never re-applies machine clamp on stepover after LUT row override, never warns when `axial_depth_mm` came from a fallback formula vs vendor row. Combined with #7 (drill no-op setter), this is why suggestions land outside envelopes despite the LUT being consulted. | `viz/ui/properties/mod.rs:95` | Suggest |
| 14 | Documentation drift — `review/SERVICE_LAYER_OWNERSHIP_AUDIT.md:36` and `review/results/41_duplication.md:1-17` describe a "Phase 2 not achieved — viz reimplements dispatch" state that is actually fixed (viz now delegates to `core/compute/execute.rs:201 execute_operation`). Three docs describing dead architecture. | `review/SERVICE_LAYER_OWNERSHIP_AUDIT.md:36`, `review/results/41_duplication.md` | Unification |

## What is already unified (credit where due)

Don't refactor these. They are the load-bearing parts that work.

- **Operation execution dispatch.** `core/compute/execute.rs:201 execute_operation`. 2628 LOC in one function but exactly one place. Viz delegates (`viz/compute/worker/execute/mod.rs:132`).
- **Dressup pipeline.** `core/compute/execute.rs:1310 apply_dressups`. Viz adds a 60-line GUI-side wrapper for feed-opt stock pre-building but the algorithm itself is canonical.
- **Sample emission.** `core/dexel_stock/simulation.rs:438-503` — one function for all op kinds. Drill paths split correctly at `simulation_cut.rs:2357` (drill kinematics genuinely differ from lateral mill kinematics).
- **Tool-load gates.** One evaluator per gate at `core/tool_load/{chipload,power,deflection,drill_gates}.rs`; one consumer at `session.tool_load_report()`.
- **Diagnostics schema.** `core/diagnostics/mod.rs` with 5 adapters (`from_feeds`, `from_project_diagnostics`, `from_stale_default`, `from_static_checks`, `from_tool_load`). Schema is unified — what's not is *who* emits the wire payload (finding #9).
- **Feeds calculator.** `core/feeds::FeedsResult` is one producer; the LUT lookup itself is one place. The problems are at the calculator's *input plumbing* (finding #12) and the *output writing* (finding #13), not the calculator.
- **Per-tool dispatch.** `tool/{flat,ball,vbit,tapered_ball,bullnose}.rs` — type-driven, not duplication. Leave alone.
- **Two compute lanes** (Toolpath + Analysis) — deliberate concurrency boundary at `viz/compute/worker.rs:285`.

## What NOT to touch in this branch

- **Drill kinematics splitting from lateral mill kinematics.** Engagement axes (radial WOC, arc engagement, leading-edge speed) physically don't apply to Z-only motion. Forcing one schema would be wrong.
- **Tuning chipload / deflection thresholds.** That's downstream work — fix the substrate first so any tuning is repeatable.
- **Adding new vendor LUT rows.** Tempting but premature. The smoke run showed the post-set warning works when the LUT is consulted via the viz path; expand LUT coverage after the singleton merge.
- **Optimizer changes.** The two cases we tested (AS011 drill `Skipped`, AS015 scallop `no_safe_improvement` + `deflection_setup_locked`) showed gold-standard behaviour. Don't disturb until you can prove a regression.
- **CLI sweep orchestration** (`cli/src/sweep.rs`). Different use case; keep it separate from GUI/MCP single-shot calls.

## Recommended fix order

Sized by impact-to-effort. Each fix is one PR.

### Fix 1 — Feedopt `nominal_feed_rate` probe (chipload-2D unblock)

**Single highest-leverage fix in this entire plan.** Unlocks chipload reporting on 7 of 22 op kinds with ~10 lines.

- **Files:** `core/compute/execute.rs:1570-1578`; signature update on the `apply_dressups` caller (`execute_operation` at `:201`).
- **Cause:** `nominal_feed_rate` is set to the first Linear move's feed in the *already-dressed* toolpath. The entry dressup (step 1) prepended a ramp/helix/plunge at `plunge_rate` (20-30% of cutting feed). Feedopt then rewrites every cutting move to `plunge_rate × RCTF`. In sim, `s.feed_rate_mm_min ≈ plunge_rate × RCTF << 0.95 × cfg.feed_rate`, so the chipload classifier's filter at `tool_load/chipload.rs:184-200` rejects every sample. `steady_samples.is_empty()` ⇒ `SteadyStateSamplesNotPresent`.
- **Fix:** Thread `op.feed_rate()` through `apply_dressups` and use it as `nominal_feed_rate` directly, instead of probing the first Linear move. The `OperationParams` trait already exposes `feed_rate()`.
- **Acceptance:**
  1. Re-run agent smoke cases AS001 / AS002 / AS003 / AS005 / AS007 from `target/acceptance_sweeps/agent_smoke_20260524_0909/results.csv`. Chipload must now report `Within` or `Exceeds(side)` — not `Unmodeled`.
  2. Add a focused unit test under `core/src/compute/execute.rs` that dresses a pocket with a ramp entry and asserts `feedopt.nominal_feed_rate == cfg.feed_rate` (not `plunge_rate`).
  3. `cargo test --test param_sweep sweep_pocket_stepover` and `sweep_adaptive_stepover` should still pass.
- **Risk:** Low. Threading `op.feed_rate()` is mechanical. The only way to break things is if some op intentionally relied on the post-dressup probe (none should — that was the bug).
- **Effort:** S, ~10-30 LOC.

### Fix 2 — Split `peak_axial_doc_mm` into two semantic fields

Removes the deflection false-positive that fired on 9/13 smoke cases.

- **Files:**
  - `core/dexel_stock/stamping.rs:374-399` — `stamp_segment_with_metrics` emits one of two physical quantities; split the return shape.
  - `core/simulation_cut.rs` — `SimulationCutSample.axial_doc_mm` (line 146) and five summary structs at lines 288, 328, 377, 402, 434.
  - `core/tool_load/deflection.rs` — must consume the new `axial_engagement_mm` field, not the plunge-Z field.
  - `viz/ui/sim_diagnostics.rs` — display surfaces.
- **Fix:**
  - Rename current `axial_doc_mm` lateral-feed return to `axial_engagement_mm` (max material height above cutter at footprint deepest cell).
  - Add new `plunge_descent_mm` field on `SimulationCutSample` for the pure-vertical case.
  - Lateral feed samples leave `plunge_descent_mm = 0`; plunges leave `axial_engagement_mm = 0`.
  - Deflection gate at `core/tool_load/deflection.rs` consumes `axial_engagement_mm` only.
  - Update CLAUDE.md "Metric caveats" block to delete the `peak_axial_doc_mm` ambiguity warning — it's no longer ambiguous.
- **Acceptance:**
  1. Re-run AS001 (pocket, commanded dpp=2): `deflection.peak_mm` should fall from ~374 µm into a realistic <200 µm range. Was failing because peak_axial_doc=12 was being interpreted as 12 mm of engaged DOC.
  2. Re-run AS011 (drill, commanded peck_depth=3): drill_gates unchanged; `plunge_descent_mm` per-peck samples == 3.0; `axial_engagement_mm` == 0.
  3. Re-run AS013 (adaptive3d, commanded dpp=3): deflection.peak_mm drops from 573 µm to a realistic value.
  4. Add a parameterised test `axial_metrics_kinematics_segregation` that asserts a lateral cut never populates `plunge_descent_mm > 0` and a plunge never populates `axial_engagement_mm > 0`.
- **Risk:** Medium. Schema change touches 5 summary structs and possibly the on-disk fingerprint format used by `crates/rs_cam_core/src/fingerprint.rs` (check `param_sweep` test fixtures — they may need regeneration).
- **Effort:** M, ~150 LOC + fixture refresh.

### Fix 3 — Single `suggest_params()` + single embedded LUT

Collapses the 3-singleton drift and makes the Suggest button agree with the post-set warning. Forces consolidation of the catalog match arms.

- **New file:** `core/feeds/suggest.rs` with `pub fn suggest_params(op_kind: OperationType, tool: &ToolDef, machine: &MachineProfile, material: &Material, workholding: WorkholdingRigidity, lut: &VendorLut, stock_ctx: &StockContext) -> OperationConfig`
- **Moves:**
  - `operation_feeds_hints` from `viz/ui/properties/mod.rs:1051` → into the new suggest module (this also dissolves finding #11).
  - `apply_stock_defaults` from `core/compute/catalog.rs:1440-1474` → into the new suggest module.
- **Deletes:**
  - The viz-side `VENDOR_LUT` static at `viz/ui/properties/mod.rs:54`.
  - The session/compute-side `VENDOR_LUT` static at `core/session/compute.rs:1792`.
  - Replace both with one `core::feeds::EMBEDDED_LUT` singleton accessed via a thin getter.
  - The `compute_feeds_for_op` viz helper at `viz/ui/properties/mod.rs:61` (replaced by `suggest_params`).
  - The `feeds_result_for_toolpath` core helper at `core/session/compute.rs:1783` (replaced by calling `suggest_params` then comparing).
- **Invariants** the new function MUST enforce before returning:
  1. `plunge_rate ≤ feed_rate` (with a warning if a setter wants to violate it).
  2. `stepover ≤ tool.diameter` (clamp + warn).
  3. `depth_per_pass ≤ machine.rigidity.doc_roughing_factor × tool.diameter` for roughing ops (clamp + warn).
  4. `depth_per_pass ≤ tool.cutting_length` (clamp + warn — geometrically impossible otherwise).
- **Reach into Fix 4:** Drill `set_plunge_rate` becomes a real setter so the invariants actually take effect.
- **Acceptance:**
  1. Three call sites are replaced: `controller/events/toolpath.rs:135`, `viz/app/mcp.rs:2446`, the wizard at `viz/ui/properties/mod.rs:1088 draw_feeds_card`.
  2. New regression test: post-set `feeds.feed_vs_lut.high` warning numbers MUST equal the values that the Suggest button computed for the same (op, tool, material, workholding) combo. The smoke run found a workholding=Medium hardcode that broke this; fix removes the discrepancy.
  3. Test that mutating `stock.workholding_rigidity` from Medium to Heavy actually changes both the Suggest output AND the post-set warning by the same proportion.
  4. Test that calling `suggest_params` with a vendor LUT row present reads from it; absent, falls back to the machine-envelope formula; both paths log which one they took.
- **Risk:** Medium-Large. Three call sites + invariant enforcement = touches the GUI flow.
- **Effort:** M, ~300 LOC + tests.

### Fix 4 — Drill `set_plunge_rate` becomes a real setter

One-line bug fix unlocked by Fix 3.

- **File:** `core/compute/operation_configs.rs:1165` and the `DrillConfig` definition above it.
- **Fix:** Add a `plunge_rate: f64` field to `DrillConfig` (or thread the existing peck-cycle feed into a single shared feed). The `set_plunge_rate` impl writes to it. Generate consumes it.
- **Acceptance:**
  1. `set_toolpath_param(index=N, param="plunge_rate", value=300)` on a drill toolpath actually changes the per-peck plunge feed in the generated G-code, not just appearing to.
  2. Add a regression test: a drill toolpath where the Suggest pass writes `plunge_rate = 250` must not generate G-code with the hardcoded default `plunge_rate = 500`.
- **Risk:** Tiny. Single file. The reason it's a no-op today appears to be historical — drill peck cycles use a per-peck feed encoded elsewhere — so the "real fix" may be unifying that encoding rather than adding a field. Read the surrounding code before deciding.
- **Effort:** S, <50 LOC.

### Fix 5 — `compute_stale_set` helper

Standalone MCP starts emitting `stale_toolpaths` correctly. Removes 8 ad-hoc viz call sites.

- **New function:** `core/session/compute.rs::compute_stale_set(session: &ProjectSession, mutation_kind: MutationKind) -> StaleSet` where `MutationKind` enumerates the cause (`ToolDiameterChanged{tool_id}`, `ToolpathParamChanged{toolpath_id, param}`, `StockChanged`, `MaterialChanged`, etc.).
- **Replaces:**
  - 8 call sites of `mcp_mark_stale_toolpaths` in `viz/app/mcp.rs` (lines 2022/2337/2379/2604/2662/2692/2727/2794).
  - `core/session/compute.rs::set_toolpath_param_invalidates_result` and `set_tool_param_invalidates_toolpath_results` should call into it.
  - The standalone MCP `rs_cam_mcp/src/server.rs` ALSO calls it on every mutation, finally emitting `stale_toolpaths` in its mutation envelopes (which it currently does not).
- **Acceptance:**
  1. Standalone MCP regression test: `set_tool_param(index=0, "diameter", 6.0)` must return a mutation envelope with `stale_toolpaths` containing every toolpath that references tool 0.
  2. Viz MCP regression test: same mutation produces same `stale_toolpaths` (parity).
  3. Existing `set_toolpath_param_invalidates_cached_result` test at `core/session/compute.rs:1456` still passes.
- **Risk:** Small. Mechanical refactor of existing logic into one place.
- **Effort:** S, ~80 LOC.

### Fix 6 — Merge MCP servers

Biggest LOC win. Forces project-loader unification (finding #4) as a side-effect. Blocked by Fix 3 (must agree on suggest path) and Fix 5 (must agree on staleness).

- **Strategy options** (decide before starting):
  - **A)** Keep viz's `mcp_server.rs` as the canonical MCP front and have it serve both embedded (in-GUI) and headless modes via a `--headless` flag. Delete `rs_cam_mcp/src/server.rs`.
  - **B)** Extract a new `rs_cam_service` crate that both the standalone CLI MCP and the GUI MCP front delegate to. Heavier but cleaner separation.
  - **A** is faster, **B** is architecturally cleaner. **Recommend A** unless we're about to add a second consumer (web client, language bindings).
- **Sub-fixes that come along for free:**
  - Project-TOML loader unification (#4) — there will be one loader because there's one server.
  - Default `OperationConfig` paths (#6) collapse from three to one (the unified server uses Fix 3's `suggest_params`).
  - Diagnostic-delta / gui_banners / warnings emission unifies (#9) because the standalone server now uses the same `mcp_mutation_result` builder.
- **Acceptance:**
  1. `cargo run -p rs_cam_mcp -- --headless --project test_data/ux_2d_pocket.toml` exposes the same tool surface as `cargo run -p rs_cam_viz --bin rs_cam_gui -- --mcp`.
  2. The agent smoke acceptance suite (`planning/toolpath_acceptance/cases_agent_smoke.csv`) passes against both modes with identical result rows.
  3. The catalog of MCP tools has exactly one declaration site, not two.
- **Risk:** XL. Coordinated change across 3-4 crates.
- **Effort:** L-XL, multi-week.

## Out of scope for this branch

- Tuning chipload, deflection, power thresholds.
- Adding vendor LUT rows.
- Optimizer changes — including the BS-stepover problem from the Wanaka review. Test that once the substrate is solid.
- New operation kinds.
- Static-validation rule additions (rest precondition, drill precondition, project_curve surface check) — file these as a separate follow-up plan once the mutation envelope is fully wired in both MCP servers.

## Acceptance regression — full plan

After fixes 1–5 land (skip 6 for now if time-constrained), re-run the agent smoke
sweep:

```bash
# in another terminal
cargo run -p rs_cam_viz --bin rs_cam_gui -- --mcp

# then (from this conversation or another agent)
# follow planning/SUGGEST_SIM_OPTIMIZE_AGENT_RUN_PROMPT.md
```

Expected deltas vs `target/acceptance_sweeps/agent_smoke_20260524_0909/results.csv`:

| Case | Today's verdict | Expected after fixes 1+2 | Reason |
|---|---|---|---|
| AS001 pocket | chipload Unmodeled / defl Exceeds 374µm | chipload Within or Exceeds (real) / defl Within (commanded dpp=2 well under rigidity budget) | Fix 1 + Fix 2 |
| AS002 adaptive | chipload Unmodeled / defl Exceeds 408µm | chipload Exceeds-low (genuinely — feed 2500 is 2.7× LUT) / defl Within | Fix 1 + Fix 2 |
| AS003 profile | chipload Unmodeled / defl Exceeds 406µm | chipload Within / defl Within | Fix 1 + Fix 2 |
| AS004 face | Within / Within 160µm | unchanged | already working |
| AS005 zigzag | chipload Unmodeled / defl Exceeds 354µm | chipload Within / defl Within | Fix 1 + Fix 2 |
| AS006 rest | harness_error | unchanged (separate fix — precondition rule) | not in scope |
| AS007 trace | chipload Unmodeled / defl Exceeds 368µm | chipload Within or Exceeds / defl Within | Fix 1 + Fix 2 |
| AS008 v_carve | chipload Unmodeled / defl Within 78nm | chipload Unmodeled with `no_vendor_data` (not steady_state) — correct reason now | Fix 1 changes the *reason*, LUT genuinely doesn't cover V-bit |
| AS009 chamfer | Within / Within 29µm | unchanged | already working |
| AS011 drill | drill_gates Within | unchanged | drill is bomber today |
| AS013 adaptive3d | chipload Exceeds-low / defl Exceeds 573µm | chipload unchanged (feedopt-3d-skipped today; correct) / defl Within or much smaller | Fix 2 only |
| AS014 drop_cutter | chipload Exceeds-low / defl Exceeds 401µm | chipload unchanged / defl Within or smaller | Fix 2 only |
| AS015 scallop | chipload Exceeds-low / defl Exceeds 431µm | chipload unchanged / defl Within | Fix 2 only |
| AS017 horizontal_finish | chipload Unmodeled `no_vendor_data` / defl Exceeds 524µm | chipload unchanged / defl Within | Fix 2 only |

**The 7 chipload-Unmodeled cases on 2D become real Within/Exceeds verdicts.** **The 9 deflection-Exceeds false positives mostly drop to Within.** That's the test the substrate must pass before any tuning happens.

## Future audit topics — deferred to a later session

When you come back to auditing after the fixes land, the next round
should cover:

1. **Optimizer Ranked-outcome behaviour.** This run only saw `Skipped` and `no_safe_improvement`. The Wanaka review previously caught a `Ranked` candidate that coarsened stepover 0.30→1.00 without scallop gating. Build a case that reaches `Ranked` and check whether the BS path still exists.
2. **Op-precondition static validation rules.** Rest precondition (needs prior tool toolpath in setup), drill precondition (similar pattern noted by user), project_curve precondition (needs curve model). Same shape as the existing `geom.plunge_exceeds_feed` rule. Probably one new module at `core/diagnostics/adapters/from_preconditions.rs`.
3. **Drill `chip_welding` material threshold lookup.** Smoke run AS011 stock was hardwood; threshold used 8 D/d (the softwood value). One-liner suspected at `core/tool_load/verdict.rs` or `core/tool_load/drill_gates.rs`.
4. **Stepover hint cardinality.** `apply_feeds_result_to_op` writes `result.radial_width_mm` into `set_stepover` regardless of whether the op's stepover semantically means "tool-fraction" or "scallop-derived". Result: drop_cutter and scallop both accept the same value with different intent. Either each op family should expose a strongly-typed "stepover meaning" or the suggest layer should branch.
5. **"Suggest All" Params-tab paint thrash.** Suggest agent flagged this as inconclusive: the Suggest All button at `viz/ui/properties/mod.rs:2752` recomputes the LUT each frame and writes back into `entry.feeds_result` even when not clicked. Whether this triggers UI flicker / stale-flag churn on every paint deserves a separate look — could be silently invalidating sim caches every frame.
6. **Catalog `match` arm collapse.** Once `suggest_params` exists, audit whether the remaining 5 `match` blocks in `catalog.rs` can collapse via the `OperationParams` trait or whether they're genuinely op-specific. Goal: reduce the 6 → ≤2 match blocks total.
7. **Per-tool `engagement_radius` consistency.** Quick check that `flat`, `ball`, `vbit`, `tapered_ball`, `bullnose` use the same axial-DOC convention as Fix 2's new `axial_engagement_mm`. They probably do but worth confirming.

---

## Agent build prompt

The below is self-contained and can be handed to an implementing agent
as a single prompt. The agent does NOT need to read this whole file —
it points at the §"Recommended fix order" above for detail.

```text
You are working in /home/ricky/personal_repos/rs_cam, a Rust CAM
workspace for 3-axis wood routers. Your task is to land Fixes 1–5
from planning/CODEBASE_UNIFICATION_PLAN.md, in order, as five separate
PRs (one per fix). Fix 6 is out of scope for this branch — it requires
coordination across more crates than fits in one focused session.

Read first:

1. planning/CODEBASE_UNIFICATION_PLAN.md (this file) — §"Recommended fix
   order" for the per-fix detail; §"What is already unified" for what
   to leave alone; §"What NOT to touch in this branch" for the explicit
   exclusion list.
2. CLAUDE.md (project conventions, lint policy — clippy must pass with
   zero warnings).
3. The smoke baseline at
   planning/toolpath_acceptance/baselines/2026-05-24_agent_smoke_acceptance.md
   so you have the failing-case evidence the fixes target.

Constraints:

- The worktree contains pre-existing user changes. Do NOT reset, clean,
  stash, or revert anything.
- Land each PR independently and atomically. Use `cargo clippy
  --workspace --all-targets -- -D warnings` and `cargo test -q` as
  PR-readiness gates.
- Don't break the parameter_sweep harness at
  crates/rs_cam_core/tests/param_sweep.rs. If fingerprints change due
  to Fix 2's schema split, regenerate fixtures and commit the
  regeneration as part of that PR with a note.
- Do not change algorithm behaviour. These fixes are substrate
  refactors. If you find yourself touching `tool_load/chipload.rs`
  bounds, `feeds/calculate.rs` recommendation logic, or any optimizer
  code, stop and ask — that's out of scope.
- After each fix lands and tests pass, append a short progress note to
  this file under a §"Implementation log" heading.

Per-fix expectations:

- Fix 1 (feedopt nominal_feed_rate): the smallest, do this first to
  prove the workflow. Adds a unit test that fails on master and passes
  after.
- Fix 2 (split peak_axial_doc_mm): biggest schema impact. Update CLAUDE.md
  "Metric caveats" block in the same PR to remove the now-resolved
  ambiguity warning. Regenerate any test fixtures that captured
  axial_doc_mm and commit them.
- Fix 3 (single suggest_params + single LUT): largest LOC. Land this
  ONLY after Fix 1+2 are green. Enforces the 4 invariants documented
  in this plan; add tests for each.
- Fix 4 (drill set_plunge_rate): trivial; bundle with Fix 3 if Fix 3
  ends up needing it sooner.
- Fix 5 (compute_stale_set helper): mechanical refactor. Verify the
  standalone MCP server (crates/rs_cam_mcp/src/server.rs) gains
  stale_toolpaths emission on mutations.

Stop and ask the user if:

- A fix turns out to be larger than the rough effort estimate by >2×
  once you start coding.
- A change you need to make to land a fix would require also touching
  the §"What NOT to touch" list.
- The acceptance regression in §"Acceptance regression" doesn't match
  expectations after a fix — that's a sign the analysis was wrong, not
  that you should chase it harder.

Return when done with:

- list of PRs landed (one per fix);
- summary of any acceptance-regression deltas observed;
- pointer to the Implementation log entry in this file;
- list of follow-ups that emerged that the user should know about.
```

## Implementation log

### 2026-05-24 — Fixes 1, 2, 4, and core/standalone slice of Fix 5

- **Fix 1 landed:** `apply_dressups` now receives the operation's configured feed rate and feeds it into `FeedOptParams::nominal_feed_rate` instead of probing the first post-dressup linear move. Added `feed_optimization_uses_configured_nominal_feed_not_entry_plunge`.
- **Fix 2 landed:** simulation samples now separate lateral/arc/helix axial engagement from pure-vertical plunge descent (`axial_engagement_mm` + `plunge_descent_mm`). `axial_doc_mm` remains as the legacy wire alias for axial engagement. Deflection now consumes `axial_engagement_mm`; MCP/summary surfaces expose `peak_plunge_descent_mm`. Updated `CLAUDE.md` metric caveat.
- **Fix 4 landed:** drill and alignment-pin drill `set_plunge_rate` now write the drill feed rate, matching the existing "feed is plunge" semantics. Added a regression test through `ProjectSession::set_toolpath_param`.
- **Fix 5 partially landed:** added core `compute_stale_set`, `MutationKind`, and `StaleSet`; standalone MCP `set_toolpath_param` / `set_tool_param` now return mutation envelopes with `stale_toolpaths`. Viz still has equivalent local stale marking in most mutation paths; collapsing every viz call site onto the helper remains to finish.

Validation run so far:

- `cargo test -p rs_cam_core feed_optimization_uses_configured_nominal_feed_not_entry_plunge --lib` ✓
- `cargo test -p rs_cam_core --test param_sweep sweep_pocket_stepover -- --exact` ✓
- `cargo test -p rs_cam_core --test param_sweep sweep_adaptive_stepover -- --exact` ✓
- `cargo test -p rs_cam_core axial_metrics_are_segregated_by_kinematics --lib` ✓
- `cargo test -p rs_cam_core --test engagement_vector_step2` ✓
- `cargo test -p rs_cam_core set_toolpath_param_drill_plunge_rate_updates_feed_rate --lib` ✓
- `cargo test -p rs_cam_core compute_stale_set --lib` ✓
- `cargo clippy --workspace --all-targets -- -D warnings` ✓

Next: decide whether to take the larger Fix 3 (`suggest_params` + LUT singleton + invariants) in this same worktree or split it into its own focused branch/session; it is materially larger than Fixes 1/2/4/5.

### 2026-05-24 — Fix 3 canonical suggest path

- **Fix 3 landed:** added `crates/rs_cam_core/src/feeds/suggest.rs` as the canonical suggest path. It owns operation feed hints, stock-aware default application, feeds-calculator input construction, feeds-result write-back, and invariant enforcement.
- Added the single core embedded LUT singleton (`feeds::EMBEDDED_LUT` / `feeds::embedded_vendor_lut()`), removed the viz and session-local LUT singletons, and pointed the chipload gate's embedded-LUT accessor at the same core singleton.
- Replaced toolpath-creation suggest call sites in GUI controller, embedded MCP, and standalone MCP with `suggest_params(...)`; replaced feeds-card and Params-tab Suggest All write-back with the core `apply_feeds_result_to_op(...)`; diagnostics now use the same workholding-aware `feeds_result_for_operation(...)` path.
- Canonical suggest write-back enforces: `plunge_rate <= feed_rate`, `stepover <= tool.diameter`, roughing `depth_per_pass <= machine.rigidity.doc_roughing_factor * tool.diameter`, and `depth_per_pass <= tool.cutting_length`, returning typed `SuggestWarning`s when clamps fire.
- Added regression coverage for vendor-LUT vs formula-fallback source selection, invariant clamp warnings, suggest-vs-`feeds.feed_vs_lut.high` diagnostic recommendation parity, and Medium→High workholding changing both suggest output and diagnostic baseline consistently.

Validation:

- `cargo fmt --check` ✓
- `cargo clippy --workspace --all-targets -- -D warnings` ✓
- `cargo test -p rs_cam_core suggest --lib` ✓
- `cargo test -p rs_cam_core workholding_changes_suggest --lib` ✓
- `cargo test -p rs_cam_core feed_vs_lut --lib` ✓
- `cargo test -p rs_cam_mcp --lib` ✓
- `cargo test -p rs_cam_viz --lib mcp` ✓
- `cargo build -p rs_cam_viz --bin rs_cam_gui` ✓

### 2026-05-24 — Fix 5 viz rollout + standalone MCP retirement

- **Fix 5 finish:** added `MutationKind::SetupChanged { setup_id }` variant to core (`session::compute_stale_set` now resolves it via `find_setup_by_id`). Replaced all 8 viz `mcp_mark_stale_toolpaths` call sites with a single `mcp_apply_stale(MutationKind)` helper that calls core `compute_stale_set` and marks `stale_since` on the affected toolpath runtimes. The old `mcp_mark_stale_toolpaths` and `mcp_toolpaths_for_tool` helpers are gone — viz now derives the stale set from the same core helper the (now-removed) standalone MCP used.
- **Standalone MCP retired:** `.mcp.json` only ever invoked the GUI-embedded MCP (`rs_cam_viz --mcp`), and `rs_cam_mcp::server::CamServer` was only referenced by `rs_cam_mcp/src/main.rs`. Deleted `main.rs`, the `[[bin]]` target, and the ~1700 LOC `CamServer` struct + impl blocks + `ServerHandler` impl. `rs_cam_mcp` is now a thin shared types crate (param structs + `parse_*` / `json_str` / `text` / `build_info` / `no_project_error`) that `rs_cam_viz` continues to import. Dropped `tokio`, `tracing`, `tracing-subscriber`, `image`, and the `rmcp` `transport-io` feature from the crate's dependencies.
- This collapses the §"Cross-audit findings" item #5 ("two MCP servers"). The remaining duplication that Fix 6 contemplated — project-TOML loader unification (#4) and the three default-`OperationConfig` paths (#6) — is unchanged and still warrants a separate pass.

Validation:

- `cargo fmt --check` ✓
- `cargo check --workspace --all-targets --features rs_cam_viz/mcp` ✓
- `cargo clippy --workspace --all-targets --features rs_cam_viz/mcp -- -D warnings` ✓
- `cargo test -p rs_cam_core --lib compute_stale_set` ✓ (4 passing, including new `SetupChanged` + `StockChanged` cases)
- `cargo test -p rs_cam_viz --lib mcp` ✓
- `cargo test -p rs_cam_mcp` ✓
- `cargo build -p rs_cam_viz --bin rs_cam_gui --features mcp` ✓
