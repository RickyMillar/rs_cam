# rs_cam Agent Notes

> ## Regression sentries — F-024..F-040 + literature matrix
>
> The acceptance loop closed 2026-05-26 at 7/7 Within. Its regression
> net lives on as sentry tests under `crates/rs_cam_core/tests/` (e.g.
> `dexel_stock_z_frame_f024.rs`, `adaptive_feed_modulation_pipeline_f036b.rs`,
> `lead_in_out_feed_rates_f040.rs`) and the `_litmatrix_*.rs`
> feeds-validation suite. Run `ls crates/rs_cam_core/tests/` for the
> full inventory. Any re-surfacing of an F-XXX issue should fail one
> of these sentries before it reaches smoke.

## What this repo is

`rs_cam` is a Rust CAM workspace for 3-axis wood routers.

It has four crates:

- `crates/rs_cam_core`: CAM engine and shared data model
- `crates/rs_cam_cli`: batch CLI
- `crates/rs_cam_viz`: desktop CAM app (`rs_cam_gui`)
- `crates/rs_cam_mcp`: shared MCP parameter struct library consumed by the GUI-embedded MCP server

## Architecture guardrails

- keep the core library independent from GUI concerns
- treat the toolpath IR as the boundary between planning and post-processing/output
- keep import, tool modeling, operation generation, dressups, simulation, and export as distinct layers
- prefer extending the existing core + worker + UI wiring path instead of creating parallel one-off flows

## Current doc map

- product overview: `README.md`
- capability surface: `FEATURE_CATALOG.md`
- AI analysis reference: `AI_MACHINIST_ANALYSIS_REFERENCE.md`
- attribution and source lineage: `CREDITS.md`
- design docs: `architecture/`
- research notes: `research/`
- status and backlog: `planning/`

## Session workflow

1. Read `planning/PROGRESS.md`.
2. Check `FEATURE_CATALOG.md` before making claims about shipped functionality.
3. Update docs when the visible product surface changes.
4. Keep `CREDITS.md` current when adding external datasets, formulas, or algorithm references.

## SocratiCode codebase intelligence

This project is indexed with SocratiCode. Prefer the SocratiCode tools for codebase exploration before reading files directly.

Core workflow:

1. Start most explorations with `codebase_search` using broad conceptual queries or exact symbol/type names. Use `rg` instead when you already know the exact string or regex.
2. Read files only after search narrows the work to a small set of relevant paths; avoid speculative whole-file reads.
3. Use `codebase_graph_query` before following imports manually, and before modifying/deleting files to see file-level dependents.
4. Use symbol-level tools before refactors: `codebase_impact` for blast radius, `codebase_flow` for forward execution flow, `codebase_symbol` for callers/callees, and `codebase_symbols` for symbol discovery.
5. Use `codebase_graph_circular` / `codebase_graph_stats` when debugging architectural/import-order issues.
6. If search returns no results, call `codebase_status`; if indexing is incomplete, wait and poll status before searching.
7. Use `codebase_context` and `codebase_context_search` for non-code artifacts if `.socraticodecontextartifacts.json` exists.

When indexing has just been started, call `codebase_status` roughly every 60 seconds until complete; indexing is asynchronous and progress is checkpointed.

## Dependency reality

Use the actual manifests as source of truth:

- workspace: `Cargo.toml`
- core: `crates/rs_cam_core/Cargo.toml`
- CLI: `crates/rs_cam_cli/Cargo.toml`
- GUI: `crates/rs_cam_viz/Cargo.toml`

Do not document or rely on crates that are not currently in those manifests.

## Implementation expectations

- tests live close to the code they validate
- if GUI state adds a field, audit setup-sheet, project-IO, and any test initializers for required updates
- if a feature is only present in UI/state and not end-to-end wired, document that honestly

## Lint policy — zero warnings enforced

All 16 clippy lints below are **deny** at workspace level (`Cargo.toml`). Clippy must pass with zero warnings before committing.

| Lint | What it catches |
|------|-----------------|
| `unwrap_used` | `.unwrap()` — use `?`, `.unwrap_or()`, or `#[allow]` + SAFETY comment |
| `expect_used` | `.expect()` — same; `#[allow]` OK for provably-safe cases with comment |
| `panic` | `panic!()` in non-test code |
| `todo` / `unimplemented` | Placeholder code must not ship |
| `indexing_slicing` | `arr[i]` — use iterators, `.get()`, or `#[allow]` + SAFETY comment |
| `dbg_macro` | No `dbg!()` in production |
| `print_stdout` / `print_stderr` | Use `tracing` instead of `println!`/`eprintln!` |
| `map_err_ignore` | `.map_err(\|_\| ...)` — preserve the original error |
| `needless_pass_by_value` | Take `&[T]`/`&str` not `Vec<T>`/`String` when not consumed |
| `large_enum_variant` / `result_large_err` | Keep enums and error types small |
| `redundant_clone` | Don't `.clone()` what you already own |
| `unsafe_code` | No `unsafe` in this codebase |

**When you hit a lint:** run `/lint-fix` for approved fix patterns. Prefer fixing the code. If the pattern is provably safe (e.g. indexing bounded by a loop, `.expect()` after a `.is_some()` check), use `#[allow(clippy::the_lint)]` with a `// SAFETY:` comment on the specific line or block — never file-level.

**Test code** is exempt: test modules carry `#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]`.

## Dev workflow quick reference

| Task | Command |
|------|---------|
| Run GUI | `cargo run -p rs_cam_viz --bin rs_cam_gui` |
| Run CLI | `cargo run -p rs_cam_cli -- <subcommand>` |
| Test (per-crate) | `cargo test -p rs_cam_core -q` (also `-p rs_cam_cli`, `-p rs_cam_viz`, `-p rs_cam_mcp`) — avoid workspace-wide `cargo test`, it can loop on this repo |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` |
| Format | `cargo fmt --check` |
| Bench | `cargo bench -p rs_cam_core` |

Run `/dev` for the full reference. Run `/verify` before committing.

## MCP live control (rs-cam tools)

The GUI embeds an MCP server (`--mcp` flag) so Claude can control the live GUI in real-time. When the `rs-cam` MCP is connected, follow this workflow:

### Standard workflow

1. **Load**: `load_project` with a `.toml` file path
2. **Inspect**: `inspect_model` (geometry, bbox, triangle count), `inspect_stock` (dimensions, material), `inspect_machine` (spindle, power, rigidity)
3. **Review**: `list_toolpaths`, `get_toolpath_params` for each index
4. **Generate**: `generate_all` or `generate_toolpath` per index. `generate_all` runs a FIXPOINT loop by default (`fixpoint: true`, the default): generate → simulate → generate, repeating until nothing new becomes generatable, so a chain of `k` "remaining stock" ops resolves in one call instead of `k` manual rounds. Pass `fixpoint: false` for the old flat single-pass behavior. If the project has any enabled `StockSource::FromRemainingStock` toolpaths, `simulation_resolution_mm` is REQUIRED — the call refuses rather than guessing a cell size (collision counts and engagement both move with it), and the error names the blocking toolpath indices. The reply reports `rounds` and `simulations` taken, and separates real `errors` from `awaiting_prior_stock` (ops still waiting on upstream simulated stock — not failures). Toolpath status is one of `Pending / Computing / Done / AwaitingPriorStock / Disabled / Error`; `Disabled` is derived from `enabled: false` and never stored (`crates/rs_cam_core/src/compute/config.rs:32-90`).
5. **Simulate**: `run_simulation` (always collects metrics)
6. **Diagnose**: read `get_diagnostics`'s `triage` block first (bounded, typed, severity-ordered — see "Metric caveats" below), then `narrate_toolpath(index)` for agent-readable Z-level structure, cut runs vs marching-squares regions, engagement histogram, suspicious arcs, peak axial DOC, and air-cut %. Narration is **cheap**: timed on an idle lane 2026-08-06 at **4 ms** on a 12.6k-move pass with a 70k-sample cut trace. The "~12 min" figure this file and the tool description used to carry was a single wall-clock reading taken *during* a 40-minute `generate_all` — i.e. mostly queue time — and is retired. Use `get_cut_trace` only when drilling into raw metrics.
7. **Visualize**: `screenshot_simulation` / `screenshot_toolpath` to `.png` then Read the image
8. **Iterate**: `set_toolpath_param`, `set_tool_param`, regenerate, re-simulate

If a `generate_toolpath` / `generate_all` call is taking a long time: both take `timeout_s` — on timeout they return a `status: "running"` response instead of blocking, and the generate is **not** cancelled, it keeps running in the background. `generation_status` (lane state, in-flight toolpath/stage, elapsed time) and `cancel_generation` are served off the GUI frame loop independent of whatever is queued behind a long generate, and both answer within about a second (`crates/rs_cam_viz/src/mcp_server.rs:52,164,179-180`; tests in `crates/rs_cam_viz/tests/mcp_escape_hatches.rs`).

This workflow names a working subset — the embedded server registers roughly 68 tools total; also useful: `get_operation_schema`, `get_diagnostics`, `inspect_spans`, `get_toolpath_diagnostics`.

### Key diagnostic thresholds

| Metric | Good | Concern | Bad |
|--------|------|---------|-----|
| Air cutting % | per-operation threshold — see below | — | — |
| Rapid collisions | 0 | 1-10 | > 10 |
| Avg engagement | > 0.3 | 0.1-0.3 | < 0.1 |

Air-cut thresholds are set PER OPERATION TYPE against the total-runtime denominator (`air_cut_pct_of_total_runtime`), not one fixed band — see `OperationType::air_cut_high_threshold_pct` (`crates/rs_cam_core/src/compute/catalog.rs:456-491`): `None` (suppressed — dexel can't see Z-only moves) for `Drill`/`AlignmentPinDrill`; 97.0 for `ProjectCurve` (rivers/curves are inherently sparse); 30.0 for the 3D finish family (DropCutter, Scallop, UnifiedFinish, Waterline, Pencil, HorizontalFinish, SteepShallow, RampFinish, SpiralFinish, RadialFinish); 40.0 for 2.5D clearing/rough (Pocket, Face, Adaptive, Rest, Zigzag, Adaptive3d) and 2D contour ops (Profile, Chamfer, Inlay, VCarve, Trace).

**Drill-specific thresholds** — read from `cut_trace.drill_summaries` keyed by `toolpath_id`, gate verdicts from `get_tool_load_report().per_toolpath[].drill_gates`:

| Metric | Low / Within | Elevated | High / Exceeds |
|--------|--------------|----------|----------------|
| `max_depth_to_diameter` vs material threshold (Janka-banded: softwood 8, medium hardwood 6, dense/unknown 5; plywood/sheet 5, plastic 4) | < 0.75× | 0.75–1.0× | ≥ 1.0× |
| `per_peck_max_dtd` vs material per-peck threshold (Janka-banded 2026-06-03: softwood ≤700 lbf 6.0, medium hardwood 5.0, dense/unknown 4.0; plywood/sheet 1.5, plastic 1.0) | ≤ 1.0× | — | > 1.0× |
| Plunge feed / diameter (1/min) | inside material envelope | below min | above max |

Four things about that table an agent must not infer wrongly (all measured 2026-08-04/06, `planning/review_2026-08-04/DRILL_GATE_EVIDENCE_AUDIT.md`):

- **Every number in it is REPO-AUTHORED**, not vendor or handbook. The W6 audit retrieved the real Onsrud drill chart (`onsrud.com/images/Drill.pdf`) and the FPL Wood Handbook in both editions: neither contains any peck or depth-to-diameter guidance for wood, and the "3–8×D" figure the per-peck ceilings were built on is a *total-hole* regime statement being used as a *per-peck* bound. Checkpoint D held the values and corrected the citation instead; `material.rs` now says so at the constants. Do not cite these to a vendor.
- **`Elevated` is not an exceedance.** Chip welding's `Elevated` band is `[0.75t, t)` — observed *below* the threshold it names — and it used to print `Chip welding (D/d) exceeds: 7.33 vs 8.00`. Fixed 2026-08-04; the column header vocabulary is what produced the defect, so read the band, not the verb.
- **The peck model is rooted at the R-plane** (Fanuc G83), not at the hole top — because that is what the emitter does. On shipped defaults (depth 10, `Peck(3)`, Ø6, feed 300, R = +5) the true figures are `peck_count` **5** and `feed_time_s` **3.400**, not 4 and 2.000; the first descent is entirely in air. One expansion, `drill::fed_descents`, serves both. Load-bearing invariant, sentried by `r_plane_rooting_moves_no_gate_number`: **every gate reads cutting geometry, never fed distance.**
- **All three gates divide by the ENVELOPE radius** (`execute.rs` uses `tool_def.radius() * 2.0` with `ToolProfile::Flat` hardcoded, and `catalog.rs` carries no tool precondition for `Drill`). On a tapered ball that overstates diameter by up to 14×, so all three read `Within` on a grossly overloaded cutter — and two of them block export when they trip, which makes the failure mode a **silent pass**. Ledgered to the radius programme (`planning/review_2026-07-29/RADIUS_AUDIT.md`, R-12), not fixed.

**Metric caveats**:

- `rapid_collision_count` is the most reliable signal. Trust it as the primary "did anything bad happen" indicator regardless of operation type.
- **The chipload gate observes ADVANCE PER TOOTH, not chip thickness** (2026-08-06). It used to compare a dexel-measured arc-mean chip thickness in mm against a vendor `chipload_min/max_mm_tooth` band — two different physical quantities, differing by a per-row factor measured across the shipped LUT at **2.4×–40.4×, median 10.9×**. A literature wave verified from primary sources (Onsrud, Freud, Amana, Garr — verbatim `Chip Load = Feed Rate / (RPM × flutes)`, one chart numerically self-verifying) that the vendor column is an advance per tooth, and the gate-side normalisation was **deleted** rather than inverted: the observation is now `effective_feed ÷ (rpm · flutes)`. Consequences an agent should know: the gate is **no longer engagement-aware at all** (the sample's own arc cancelled even before the deletion), so the *sim* remains the operational arbiter of engagement; a shipped fixture's verdict moved `Within` + burn advisory → `Exceeds(High)`; and a separate fixture's "safe" 0.18 mm/tooth turned out to be **3.27× the band maximum** while the pre-fix gate said `Within`. The GUI viewport's chipload heat-map still colours by the old per-move quantity and carries the same mismatch on a visible surface — known, ledgered, not fixed.
- **Sub-Ø2 chipload verdicts are provisional.** The scaling laws that transfer a vendor row to another diameter/hardness are `D^0.61` and `Janka^-0.5`, adopted 2026-08-06 and **derived by this repo — no primary source publishes either exponent** (`CREDITS.md` says so in those words). On the reference fixture the diameter law moved the band by ×1.598 and reversed the verdict the unit deletion had produced four commits earlier. Neither move was wrong; they moved different sides of one comparison. Expect a bench measurement to move them.
- **Suggest's rubbing floor is subordinated to the matched band.** `RUBBING_FLOOR_MM_TOOTH = 0.025` was clamping a fine-tool recommendation to **3.47×** the row's own derated band maximum. It is now `min(floor, derated_band_max)` (`feeds::effective_rubbing_floor`), with the bare constant retained where no band exists. When no feed can both clear chip formation and stay inside the vendor window, `FeedsWarning::ChiploadClampedToFloor` carries `band_capped_from` and all three renderers say which guarantee was **not** met.
- `average_engagement` is the **cylinder-side radial-WOC fraction** (a.k.a. `engagement.radial_woc_fraction`), not leading-edge engagement. For adaptive3d it typically reads ~10× lower than the algorithmic target (~3% observed vs ~30% target from `target_engagement_fraction`). Use it for **relative** comparison between parameter variants, not as an absolute pass/fail bar. For axis-aware reporting (axial-DOC, arc, chip thickness, leading-edge speed by kinematics class) read the `per_kinematics` summary block — Step 2 of the dexel-fidelity roadmap landed the structured `Engagement` vector on every sample (see `planning/DEXEL_Z_ONLY_INVESTIGATION.md` §6.D / §6.H). Under Step 4 (F.a sub-cell stamping, 2026-05-19) the perp-extent measurement is gated on cells with stamp coverage ≥ 0.95 to prevent F.a boundary residuals from inflating engagement on repeated passes. A genuine full slot now reads radial ≈ 0.95 (limited by grid discretisation + sub-sample geometry) rather than 1.0; per-toolpath averages drop by roughly that proportion. Continue treating the scalar as a comparative signal, not an absolute fraction-of-diameter readout.
- `ProjectDiagnostics` carries TWO air-cut percentages with different denominators, and they are not interchangeable (`crates/rs_cam_core/src/session/mod.rs:846-864`, `crates/rs_cam_core/src/simulation_cut.rs:537-566`). `air_cut_pct_of_total_runtime` (air-cut time ÷ cutting + rapids) is the measure every shipped threshold is tuned against — the GUI's 20% banner, the CLI's 40% verdict, and every per-operation value in `OperationType::air_cut_high_threshold_pct`. `air_cut_pct_of_cutting_time` (rapids excluded, always ≥ the total-runtime reading) is what the MCP `narrate_toolpath` air-cut line reports. The legacy `air_cut_percentage` field is just an alias for the total-runtime reading, kept for wire compatibility. Plunge-and-retract-loop ops (project_curve, v_carve, drill) no longer inflate either reading from retract feeds — retracts are now tagged `MoveIntent::Retract` and excluded from cutting metrics (Step 1, 2026-05-19; see `planning/DEXEL_Z_ONLY_INVESTIGATION.md`). Drill toolpaths still set `metrics_not_applicable: true` (engagement axes don't apply to Z-only kinematics), but Step 3 PR2 added a parallel `drill_summaries` slot on `SimulationCutTrace` — drill ops produce **drill-native** metrics (per-peck `DrillSample`, per-toolpath `DrillToolpathSummary` with peck adequacy + chip-welding risk + cycle time) and three drill-specific gates on `ToolpathLoadVerdict.drill_gates` (chip welding, peck adequacy, plunge feed sanity). The legacy "treat as not-applicable" advice still applies to engagement metrics; consult `drill_summaries` / `drill_gates` for the actionable signal.
- `peak_axial_doc_mm` now reports lateral/arc/helix axial engagement only. Pure-vertical plunge distance is exposed separately as `plunge_descent_mm` / `peak_plunge_descent_mm`, so deflection gates no longer consume peck descent as cutter engagement. **F-024 (2026-05-25) follow-up:** the value also now reflects the *commanded* axial DOC rather than the full stock height. Pre-F-024 the per-setup dexel grid for identity setups (`face_up=Top`, `z_rotation=Deg0`) was rooted at zero-local Z (`(0,0,0)..(stock_x, stock_y, stock_z)`) while the toolpath emitted cuts in world frame (Z = -depth, with stock top at Z=0). The cutter sat below every dexel ray, `ray_blend_above` cleared the entire ray, and per-sample `axial_engagement_mm` read the full stock height (e.g. 12 mm on a 2 mm-DOC pocket pass). Identity setups now pass `local_stock_bbox = None` from `session/compute.rs`, so the per-setup grid uses the world-frame stock bbox and the measured axial matches the commanded DOC. Non-identity setups (face flips, Z-rotation) still use the zero-rooted effective bbox — that path's frame consistency is tracked separately if it surfaces again.
- **Read `triage` first, not `issue_count`.** Raw `issue_count` with thousands of `air_cut` entries is **emission noise**: every sample outside fresh material counts as an "issue", and three different quantities ship under that one name (segments, per-sample counts, and MCP's post-filter count — one `get_cut_trace` response can display all three). `ProjectSession::simulation_triage` is the single construction site for the bounded typed answer, consumed unchanged by the GUI panel, MCP `get_diagnostics` (`resp["triage"]`), the CLI `project` report and narration. Read `triage.safety` (collisions, holder strikes), then `triage.actions`, then `triage.advisories` — the advisories are capped (10 per toolpath, 50 per project) with `truncated` and a true pre-cap `total_matching`, and deduped on a ≥10 mm spatial key that never merges two distinct safety events. `hotspots` and `rapid_collision_count` remain trustworthy raw signals. **Since 2026-08-13 (TD3 B-5) `get_diagnostics`' `per_toolpath` rows are the core `ToolpathDiagnostic`** — the same record the CLI's `project` report publishes — so `op_kind`, per-toolpath `collision_count` / `rapid_collision_count` and the report-only finding areas (`truncated_core_mm2` + its deprecated `standing_material_mm2` duplicate, `untouched_material_mm2`, `reached_uncut_estimate_mm2`, `unmachined_band_area_mm2`, `tip_float_points`, `max_tip_float_mm`) are on the wire in GUI mode, with `null` meaning **not measured**. Before that they were absent from the GUI wire entirely and present on the CLI's, and the project-level `collision_count` was a literal `0`. Probe `build_info().features` for `diagnostics_row_core_parity` before reading a missing key as "not measured" — on an older binary it means "not published" (`planning/review_2026-08-08/RESULTS_PARITY.md`).
- **A metric can now say it was not measurable, and gates ABSTAIN when it does.** `sim_measurability::{MeasurabilityReport, Measurability, SimMetric, MeasurabilityReason}`. The motivating case: `FRESH_MATERIAL_THRESHOLD_MM = 0.05` gates the radial-engagement measurement, so a 0.02 mm-deep pass reads **air 95.9%, peak radial 0.0000, avg engagement 0.0000** while removing **63.7 mm³** — a hard zero dressed as a percent that clears every shipped bar. A `NotMeasurable` metric now abstains with a stated reason instead of feeding a healthy-looking verdict; **collision detection is never disabled**, and the GUI prints a `NOT MEASURED: …` strip above the diagnostics panel. Two independent conditions produce it (a fixed millimetre material floor, and a lateral-resolution condition from the 0.95 perp-coverage gate) — neither is "cell < cut depth". No threshold moved to add this.
- **A gate handed an empty population passes and looks healthy.** Measured 2026-08-05: three gates returned `Within` with `sample_range 0..0`, no locality and `available_kw 0.0`, indistinguishable on every surface from a measured clean cut. When judging whether a gate exonerated something, check its *population* first — `sample_count`, `sample_range` — and treat a bar written as a verdict comparison as vacuous until you have.
- `ToolpathStats` (`crates/rs_cam_core/src/compute/config.rs`) also carries **fourteen** report-only generation findings, of which eleven reach `narrate_toolpath` or the diagnostics list and **four do not** (`deprecated_dial`, `derived_stepovers` and `claims_reference` never reach narration; `boundary_clip_dropped` has no narration adapter — do not cite this list as evidence of coverage). They are: `truncated_core_mm2` (renamed from `standing_material_mm2` in wave 16, 2026-08-04 — the old spelling survives only as a legacy JSON key), `untouched_material_mm2`, `reached_uncut_estimate_mm2`, `dropped_band`, `clipped_band`, `tip_float`, `deprecated_dial`, `derived_stepovers`, `ramp_reach_clamp`, `claims_reference`, `retract_trips`, `zero_removal` (wave 16: a rest pass whose emitted cutting geometry never gets under the stock the prior op left — it costs full price in motion and removes nothing; a report, not a refusal), and the two Checkpoint C slots `offset_library_failures` and `boundary_clip_dropped` (2026-08-05). **Eleven share one contract**: `None` means **not measured** (the generator path never ran that check), `Some(0.0)` means measured and clean — never coerce an absent value to zero. **Three are deliberately outside it** and say so in their own docs: `derived_stepovers` is a `Vec` (empty = nothing derived a stepover, not "measured zero"); `zero_removal`'s `None` conflates "not measured" with "nothing to report", because it supports no ratio; and `boundary_clip_dropped` makes the same call for the same reason. Note also that `offset_library_failures` counts offset **calls**, not distinct rings — a depth-stepped op re-offsets the same geometry once per Z level, so one bad ring on a ten-level pocket reports ten. No gate consumes any of these; they are report-only.

### Model types and what they need

| Kind | Geometry | Typical operations |
|------|----------|-------------------|
| `stl` (3D mesh) | `inspect_model` → bbox, triangle count | adaptive3d (rough), drop_cutter/waterline/scallop (finish) |
| `step` (BREP) | `inspect_model` + `inspect_brep_faces` → face types, normals | Same as STL + face-selective operations |
| `svg`/`dxf` (2D) | `inspect_model` → polygon count, area, perimeter | pocket, profile, adaptive, v_carve, trace |
| Drill cycle (any model + hole positions) | Hole XY/Z from model centroids or stock `alignment_pins` snapshot | `drill`, `alignment_pin_drill` — bypass dexel stamping for analytical cone/cylinder removal; produce `DrillToolpathSummary` + `drill_gates` instead of engagement metrics |

### Tool selection guidance

- **Roughing**: Use end mills. `adaptive3d` for 3D surfaces, `adaptive`/`pocket` for 2.5D
- **Finishing**: Use ball nose for 3D surfaces (required for `scallop`). End mills OK for `drop_cutter`, `waterline`
- **Fine detail**: Smaller diameter = better detail but longer runtime
- Check `inspect_machine` for max shank diameter constraint

### Common pitfalls

- `stock_top_z` in roughing config must match actual stock height, not an arbitrary value
- Scallop requires a ball-tip tool (ball nose or tapered ball nose)
- Horizontal finish is useless on terrain — only cuts near-flat areas
- After `set_toolpath_param`, the toolpath is stale — must `generate_toolpath` again
- After modifying tools, ALL dependent toolpaths go stale
- `stock_to_leave` on UnifiedFinish is honoured by **all three bands** since 2026-08-06 (shallow raster, mid-steep scallop, very-steep waterline). Before that only the scallop band applied it, so any project or note written earlier assumed a dial that was inert on two thirds of the surface. It is a **vertical** (+Z) offset: what remains measured normal to a wall sloped at angle A is `stock_to_leave × cos A` — 0.71× at 45°, 0.26× at 75°.
- **A `ToolContainment::Inside` boundary is not an unconditional guarantee.** If the boundary offset collapses, the path is emitted **unclipped** rather than over-clipped, and the only trace is the report-only `boundary_clip_dropped` finding. A genuine library failure now refuses instead; a genuine collapse passes through with a typed finding. Check the finding before asserting a path was contained.
- **Two of the three offset panic classes are `debug_assert!`s in a dependency**, so debug and release do not agree: in release the library proceeds on unvalidated input (a malformed slice, a corrupt spatial index, a NaN arc centre) instead of being caught. `offset_library_failures` is therefore **not comparable across builds**, and a lower release count is the expected divergence, not an improvement.

## Agent skills

Project-level Claude Code customizations in `.claude/`:

| File | Type | Purpose |
|------|------|---------|
| `skills/verify/SKILL.md` | `/verify` | Run the CI quality gate locally |
| `skills/dev/SKILL.md` | `/dev` | Build, test, run, and module quick reference |
| `skills/sim-analysis/SKILL.md` | `/sim-analysis` | Simulation diagnostic interpretation guide |
| `skills/lint-fix/SKILL.md` | `/lint-fix` | Fix clippy lint violations with approved patterns |
| `skills/refresh-lit-matrix/SKILL.md` | `/refresh-lit-matrix` | Re-verify or replace stale literature-matrix sources |
| `agents/cam-navigator.md` | Agent | Codebase navigation: find operations, trace pipelines |
| `agents/sim-diagnostics.md` | Agent | Simulation diagnostic analysis and interpretation |

## Parallel agent teams

Use `TeamCreate` to spin up agent teams for tasks that benefit from parallel work:

- **Parameter sweeps**: 4 agents split by operation family (2D contour, 2D clearing, 3D raster, 3D contour) — see `toolpath_stress_test/agents/AGENT_INSTRUCTIONS.md`
- **Defect investigation**: one agent per finding from `toolpath_stress_test/FINDINGS.md`, each in an isolated worktree
- **Multi-crate refactors**: separate agents for core, CLI, and viz changes working on independent worktrees
- **Test + fix cycles**: one agent runs tests / sweeps, another fixes issues as they're reported

Teams share a task list for coordination. Use worktree isolation (`isolation: "worktree"`) when agents edit overlapping files. Agents go idle between turns — this is normal; send them messages to wake them.

## Parameter sweep infrastructure

Toolpath validation tooling lives in `crates/rs_cam_core/src/fingerprint.rs` and `crates/rs_cam_core/tests/param_sweep.rs`:

| Command | What it does |
|---------|-------------|
| `cargo test --test param_sweep` | Run all 56 parameter sweeps across the operation families |
| `cargo test --test param_sweep sweep_pocket` | Run sweeps for one operation family |
| `cargo run -p rs_cam_cli -- sweep job.toml --param X --values "..." --output-dir out/` | Full-pipeline sweep with dressups/depth stepping |
| `python3 toolpath_stress_test/agents/analyze_sweep.py target/param_sweeps/` | Automated verdict analysis |

Sweep output goes to `target/param_sweeps/{op}/{param}/` with JSON fingerprints, diffs, toolpath SVGs, and 6-view composite stock PNGs.
