# Second technical-debt programme — orchestration log

Basis: `TECH_DEBT_2_RESEARCH_AND_FIX_PLAN.md` at HEAD `7a84472`.

## Tracker bootstrap — 2026-08-04

| ID | Work | Status | Checkpoint | Evidence |
|---|---|---|---|---|
| W0 | R3 adaptive3d red baseline | NOT_STARTED | A | — |
| W1 | R7 findings transport | NOT_STARTED | none for report wiring | — |
| W2 | R7 arcfit intent | NOT_STARTED | F1 | — |
| W3 | R1 feeds/Suggest census | NOT_STARTED | B | — |
| W4 | R2 adversarial 2D campaign | NOT_STARTED | C | — |
| W5 | R4 simulation issue channel | NOT_STARTED | D | — |
| W6 | R5 drill evidence/literature audit | NOT_STARTED | D | — |
| W7 | R6 reference fixture/P7/rest anomaly | NOT_STARTED | E | — |
| W8 | R7 finishing/export/read defects | NOT_STARTED | F2/F3 | — |
| W9 | R8 bounded scouts | NOT_STARTED | none | — |
| W10 | close-out/live validation | NOT_STARTED | G | — |

Agents append entries using the required format in plan §3.1. The orchestrator updates this bootstrap table after a wave, merge, re-scope, or human checkpoint ruling. No agent self-approves a checkpoint.

## W9 — bounded scouts, 2026-08-04

Status: DONE
Commit(s): `e9c6b46`
Parent/revision measured: `63d5e8b` (branch `experiment/adaptive-spiral`)

Question and pre-registered bars: R8's four questions — import/mesh trust boundaries; project-IO `Serialize`-only fields, dual-key emitters, manual wire views, version migrations, write-never-read; export paths that independently interpret move type/intent/coordinates (the D-LV.1 shape, hunted elsewhere); GUI `Default` / registry / serde / UI-initialiser divergence. Bars set before the scan, per the plan's acceptance gates: (1) every listed risk carries a concrete `file:line:symbol` plus a reproduction hypothesis — no generic fear is admissible; (2) a Default/serde divergence claim names **both** literal values and the load path that selects each; (3) maximum one page per area; (4) zero production, test, or schema change. Vacuity condition: an area that produced only "structurally duplicated but currently equal" items had to say so explicitly rather than promote a drift risk to a defect.

Fixture/population/resolution: not applicable — static source analysis only. Population = the four named subsystems at `63d5e8b`; instruments = `rg`/`grep`/`Read` plus `~/.cargo/registry` sources for the one dependency claim (`truck-stepio-0.3.0`). **No Cargo command of any kind was run** (no build, test, check, or metadata), per the no-Cargo research lane. Nothing here is a runtime observation.

Render/artifact paths: `planning/review_2026-08-04/UNTOUCHED_TERRITORY_RISK_MAP.md`.

Result (fact), interpretation, and uncertainty:

- **Fact.** 20 risks with code locations, 12 items ruled out with evidence. By disposition: 3 `fix now`, 17 `candidate for next programme`, 12 `ruled out`.
- **Fact.** Two writers emit the post token `"grblhal"` (`crates/rs_cam_viz/src/state/runtime.rs:329`, `crates/rs_cam_viz/src/app/input.rs:256`). Three readers parse it back. Only `gcode::get_post_definition` (`crates/rs_cam_core/src/gcode/mod.rs:862-869`) knows it, and its only callers are `crates/rs_cam_cli/src/main.rs:490` and `sweep.rs:307`, both on the job-file path. The two project-path readers — `GuiState::post_from_session` (`runtime.rs:311-315`) and `gcode::export_project_gcode` (`gcode/mod.rs:228-232`) — fall through to `PostFormat::Grbl`. `PostFormat::GrblHal` is user-selectable (`gcode/mod.rs:833-838` rendered at `crates/rs_cam_viz/src/ui/properties/post.rs:17-19`) and has a shipped definition (`gcode/post.rs:350`). `gcode/mod.rs:1052` already asserts the token resolves.
- **Fact.** `SetupRuntime { datum, model_ids }` (`crates/rs_cam_viz/src/state/runtime.rs:220-224`) has exactly two constructors, both empty (`:300`, `:356`); it is written by the Setup properties panel (`crates/rs_cam_viz/src/ui/properties/setup.rs:113-180`) and read by the viewport (`crates/rs_cam_viz/src/app/gpu_upload.rs:604-645`). Core `ProjectSetupSection` (`crates/rs_cam_core/src/session/project_file.rs:288-305`) and `SetupData` (`crates/rs_cam_core/src/session/mod.rs:426-443`) carry neither field.
- **Fact.** The v<=2 -> v3 alignment-pin migration exists only in `crates/rs_cam_viz/src/io/project.rs:731-752`, which `Controller::open_job_from_path` (`crates/rs_cam_viz/src/controller/io.rs:166-230`) reaches only when `ProjectSession::load` **errors**. Core's setup section has no `alignment_pins` field and no `deny_unknown_fields`, so a v2 file parses successfully and the fallback never runs.
- **Fact.** Core's `ToolpathDiagnostic` wire is a hand-written `serialize_struct` (`crates/rs_cam_core/src/session/mod.rs:1429-1470`) with no exhaustiveness guard; its CLI sibling solves the same problem with a `..`-free destructure (`crates/rs_cam_cli/src/project.rs:104-129`). Field counts verified correct today.
- **Fact.** Four independent estimated-time formulas (`crates/rs_cam_viz/src/ui/readiness.rs:132-146`, `crates/rs_cam_viz/src/io/setup_sheet.rs:77-90`, `crates/rs_cam_viz/src/ui/export_wizard.rs:935-938`, `crates/rs_cam_viz/src/ui/toolpath_panel.rs:480`), already divergent in their guards: the first two skip `feed <= 0`, the wizard uses `.max(1.0)`.
- **Fact.** `truck-stepio-0.3.0/src/in/mod.rs:71` declares `pub shell: HashMap<u64, ShellHolder>` over `std::collections::HashMap` (import at `:14`); `crates/rs_cam_core/src/step_input.rs:68` derives `FaceGroupId` from `.values().enumerate()`. `crates/rs_cam_core/src/enriched_mesh.rs:19` documents these IDs as deterministic.
- **Interpretation.** Three of the four areas' top risks are one pattern: a value has two interpreters, one complete and one incomplete, and the incomplete one is on the production path. That is the same pattern as D-LV.1 and as the `intra_region_hookup_mm` prior, which suggests a class fix (single resolver / single classifier) rather than four point fixes.
- **Uncertainty, stated.** Every claim is static. Three items explicitly require a runtime check before being called defects and say so in the map: I-2 (the SVG px factor depends on the document's viewBox), G-1 (`auto_resolution: true` may override the GUI's `0.25` seed), and I-1 (single-shell STEP files are stable; only multi-shell ordering varies). The `ruled out` column is "inspected and found sound at `63d5e8b`", not "proven safe".

Red-first evidence / fingerprints changed: none. No test was added, no fingerprint captured or re-pinned, no production file touched. Per plan §M4 "Preferred fix shape", the three `fix now` items are **logged, not fixed**; each still needs a reproducer that fails on the parent revision before any change, and P-1's fix touches a serialized token, which is a checkpoint-class decision.

Verification (focused commands + exact known-red state): no commands run — this wave holds no Cargo slot by design, so it contributes no test evidence and changes no known-red state. Verification of the map is re-reading the cited locations. The three `fix now` items each carry a one-sentence GUI/MCP reproduction in the map for whoever holds the Cargo slot next.

NOT FIXED, STATED — owner and re-open condition:

- **P-1 grblHAL downgrade** — owner: orchestrator to route (touches `rs_cam_viz` state + `rs_cam_core::gcode`, i.e. lane F). Re-open condition: a red-first test asserting `post_from_session("grblhal") == PostFormat::GrblHal` and that `export_project_gcode` selects `post::grblhal()`; the fix changes which post definition a saved project exports with, so it needs an operator ruling before it lands.
- **P-2 setup datum not persisted** — owner: orchestrator. Re-open condition: a decision on whether datum belongs in `SetupData` (schema addition, therefore checkpoint-gated) or should be removed from the UI as unimplemented. Do not add the field silently.
- **P-3 v<=2 alignment pins dropped** — owner: orchestrator. Re-open condition: confirmation that `format_version <= 2` files still exist for this operator. If none do, downgrade to `ruled out (no population)` rather than fixing.
- **I-1..I-5, P-4, P-5, X-1..X-5, G-1..G-5** — owner: next programme's intake. Re-open condition: R8's mandate was a ranked map, not a fix queue; none of these may be started inside this programme.
- Not scouted by design and untouched here: B7 GUI worker findings transport (W1), `arcfit` intent (W2), D-LV.1 `screenshot_toolpath` (W8). X-1 is a *second* site of D-LV.1's shape and should be handed to W8 rather than opened independently.

Next action / checkpoint request: **no checkpoint requested** — R8 carries none. Orchestrator action: mark W9 `DONE` in the bootstrap table with evidence `e9c6b46`; decide whether P-1 is promoted out of the intake list into a checkpoint-gated fix, and hand X-1 to W8's brief.

## W0 — adaptive3d red baseline, 2026-08-04

Status: AWAITING_CHECKPOINT
Commit(s): `d40768a`
Parent/revision measured: `9f52378` (branch `experiment/adaptive-spiral`)

Question and pre-registered bars: H0/R3's five research questions — the asserted contract of each red, first-bad commit, determinism vs wall-clock flakiness, which layer the failure lives in (planner geometry / rapid-peck emission / simulator interpretation / fixture / stale assertion), and whether a render confirms the test population reaches the claimed geometry. Bars set before running: (1) determinism = at least two runs, byte-identical failure text, on a machine holding no foreign Cargo job; (2) a first-bad claim must be either a build bisect or a `git log -S` single-commit result plus a closed-form derivation whose predicted failure value is written down **before** the run; (3) no verdict published before its render is written and read; (4) zero production, test, or fixture change. Vacuity condition, applied to my own conclusions: a mechanism model was only accepted if it also correctly predicted which sibling tests stay green.

Fixture/population/resolution: tests 1-2 — `make_test_flat(100.0)` (flat quad at z=0), `FlatEndmill(6.35, 25.0)`, `minimal_params` with `stock_to_leave` 0.5, `max_stay_down_distance_mm: Some(0.0)`; no resolution parameter (pure emission). Test 3 — `make_test_hemisphere(20.0, 16)`, `FlatEndmill(6.35, 25.0)`, dexel/heightmap cell **0.5292 mm** shared by planner and simulator, same `initial_stock` object on both sides, `sample_step_mm` 0.5, stock 0->25 mm, dpp 3.0 / leave 0.5 / stepover 1.0 / tolerance 0.5. Population = interior cells (mesh bbox inset 1 mm); tolerance = one cell. No cross-resolution comparison anywhere (plan rule 8); `resolution_clamped` not applicable — both sides use the same fixed grid.

Render/artifact paths: `planning/review_2026-08-04/ADAPTIVE3D_RED_BASELINE.md`; artifacts in `planning/review_2026-08-04/artifacts/w0/` — `peck_fixture_xz.svg`, `rapid_fixture_xz.svg`, `parity_evidence.svg`, `parity_violations_agent_search.svg`, plus transcripts `parity_agent_search_run1.txt`, `parity_bisect_evidence.txt`, `path_fixture_moves.txt` and three standalone generators. All four renders were rasterised and read before the verdicts were written.

Result (fact), interpretation, and uncertainty:

- **Fact.** All three fail deterministically, 3/3 runs, byte-identical. Test 1: `assertion left == right failed, left: 1, right: 2` at `path.rs:1676`. Test 2: panic `cut1 endpoint not found` at `path.rs:1717`. Test 3: interior **854** vs bar **792**, `max dz` **25.000 mm** (= full stock height), `sim_higher` 2228 / `planner_higher` 464.
- **Fact.** Test 3 runs in **2.0 s** (2.2 s wall, 84 MB RSS). It is not in the ten-minute class; `ps -L` liveness handling was not needed.
- **Fact, bisected.** `git bisect run` over `be0dcbf..9f52378`, 343 revisions, 9 probes, in an **isolated `git worktree`** with a scratch `CARGO_TARGET_DIR` — the main working tree was never checked out, so `planning/airrun_2026-06-01/wanaka.toml` was never touched, staged, or reverted; the worktree was removed and pruned. Good anchor `be0dcbf` verified by running it (interior 625), not trusted from its commit body. **First bad commit `fa27b08`**; parent `7a95614` good at interior **664**, `fa27b08` bad at **945**.
- **Fact.** Tests 1 and 2 have the same cause, established closed-form: `drape_point` / `drape_path_to_leave` (both introduced by `fa27b08`, sole `-S` hit) lift every fixture point on the flat-at-z=0 mesh to z=+0.5. The predicted values — `EntryPlunge` count 2->1, and disappearance of the (5,5,-3) landmark — were derived from source and written down before either test was run, and matched exactly.
- **Fact.** The directional counters **reverse** at `fa27b08` on both strategies: AgentSearch `planner_higher`/`sim_higher` 778/562 -> 443/2404; ContourParallel 502/**12** -> 113/**1144** (95x).
- **Fact.** Cut-only probes at HEAD: `ContourParallel flat` **0** interior divergent cells, `AgentSearch flat` **21**, `AgentSearch hemisphere` **479**, `ContourParallel hemisphere` **614**.
- **Fact.** `planner_sim_dexel_parity_contour_parallel` is **green** at interior 429 while carrying the same directional skew (987/231) and the same 25 mm `max dz`; its gated count *fell* 459 -> 406 across `fa27b08`.
- **Interpretation.** One un-mirrored transform explains all three. `segments_to_toolpath` gained a drape; `stamp_emitted_segment` still mirrors only `simplify_path_3d` + `blend_corners_3d`. The drape only raises Z, so the planner's bookkeeping now over-states removal; because that bookkeeping gates `material_remaining_at_level` / `material_remaining_in_region`, the operator-visible failure mode is **skipped passes / standing material** on curved and steep terrain, not a gouge. The flat-vs-curved contrast identifies the divergence as a *sampling* mismatch — the emitter drapes with an exact `point_drop_cutter`, the planner lifts from a **nearest-cell** heightmap (`slope.rs:321-333`) — rather than a broken stamping kernel, which would also diverge on flat stock. Proposed verdicts: **FIX_TEST, FIX_TEST, FIX_CODE**. `fa27b08` is itself correct (wanaka over-cut 67 -> 0) and is **not** proposed for reverting.
- **Uncertainty, stated.** (a) Which convergence option is cheaper — mirroring the drape inside the planner's stamp, or moving the planner's lift onto the exact drop-cutter — is **not** measured here; both are output-changing and the cost claim is reasoning, not a benchmark. (b) The 20-violation render is a row-major *sample* the harness happens to collect first, so it cannot support any claim about spatial distribution; the curvature claim rests on the flat-vs-curved probe numbers instead, and the figure says so. (c) Tests 1 and 2 were not build-bisected individually; their first-bad attribution rests on a single-commit `-S` result plus a pre-registered closed-form prediction, corroborated by test 3's independent bisect landing on the same commit.

Red-first evidence / fingerprints changed: none — no test added, no fingerprint captured or re-pinned, no production file touched. The red-first material for the approved wave is nonetheless already **measured**: the proposed `planner_stamp_mirrors_emitter_drape` directional assertion is known to fail at `fa27b08` and pass at `7a95614`.

Verification (focused commands + exact known-red state): `cargo test -p rs_cam_core --lib <test> -- --exact --nocapture` for each of the three, 3 runs each, on a quiet machine (foreign `sysml-lsp-server` Cargo job waited out; `free -g` and bracketed `pgrep -af "carg[o]"` checked before every launch; ~20 GB available at run time). Also run for context: `planner_sim_dexel_parity_contour_parallel` (**ok**, interior 429) and the four `#[ignore]`d cut-only probes (**4 ok**). Known-red state is **unchanged by this wave**: the same three `--lib` reds, plus `wanaka_suggest_baseline` as the declared environmental exception. No new red, no test greened, nothing ignored.

NOT FIXED, STATED — owner and re-open condition:

- **All three reds** — owner: **operator at Checkpoint A**; execution by a later wave. Re-open condition: a recorded Checkpoint A ruling. W0 proposes and must not execute.
- **`planner_sim_dexel_parity_contour_parallel` carries the same defect sub-threshold** — owner: the §3 fix wave. Re-open condition: it is green today and must **not** be read as evidence a §3 fix worked; it has to be re-checked by the directional measure.
- **The parity bar is mis-pre-registered** (interior count gated against a tenth of the whole-grid count) and was already 79 % spent when set (625/792 at `be0dcbf`, from 28 at birth) — owner: the §3 fix wave. Re-open condition: restate against the interior population and re-measure against fixed code; do not copy the existing 10 % pin (plan rule 7).
- **`legacy_test_mesh`'s doc comment is false since `fa27b08`** ("the heightfield never gets queried") — owner: whoever lands the §1/§2 fix, same commit. Re-open condition: Checkpoint A.
- **Which convergence option to take for §3** — owner: the approved implementation wave. Re-open condition: a measured generation-time comparison; W0 deliberately did not benchmark it.
- **`wanaka_suggest_baseline`** — out of W0 scope by plan rule 9; not this programme.

Next action / checkpoint request: **Checkpoint A requested** on asks A1-A4 in `ADAPTIVE3D_RED_BASELINE.md` §5. Nothing blocks the checkpoint — the baseline has zero unclassified adaptive3d failures, which is H0's stated acceptance gate for this stage. Orchestrator action: set W0 to `AWAITING_CHECKPOINT` with evidence `d40768a`; note for W2 that §2's proposed sentry asserts `MoveIntent` at the emitter and should be sequenced against the `arcfit` intent work rather than concurrently.

## W1 — findings transport, 2026-08-04

Status: DONE
Commit(s): `41cb426` (join + sentry + call-site rewiring), `1de1094` (census)
Parent/revision measured: `d40768a` (branch `experiment/adaptive-spiral`; W0's `dd3429f` landed mid-wave and is docs-only, so the code parent is unaffected)

Question and pre-registered bars: H2's RQ1 — can the CLI's exhaustive-destructuring serde-view pattern be used for the GUI worker mapping without dragging GUI-only state into core, and what is core-owned / GUI-owned / deprecated-wire-only / computed-after-generation? Acceptance gate pre-registered by the plan: **adding a synthetic `GenerationFindings` field must fail compilation in every required adapter until consciously handled.** Two bars I set before starting: (1) the gate is only satisfied if the PARENT is shown to PASS the same probe — a check that errors after a fix but was never run before it proves nothing about the defect; (2) a merged join may not change any published number, and "unchanged" had to be established by construction, not by a tolerance on a fixture.

Fixture/population/resolution: not a measurement wave. The population is every site in the four crates that copies a generation finding or a `ToolpathStats` field across a module/crate boundary — ten, enumerated with `file:symbol` provenance in the census. The sentry's fixture is a four-move hand-traced toolpath (4 moves, 2 mm cutting, 2 mm rapid, one retract trip) whose expected move-derived stats are known by inspection, plus a fully-populated `GenerationFindings` with every field set to a distinguishable value. No resolution parameter anywhere.

Render/artifact paths: `planning/review_2026-08-04/FINDINGS_PIPELINE_CENSUS.md` (§3 boundary table, §5 surface-coverage matrix, §7 the executed gate with both halves of the probe). No image renders — the subject is a type-level transport, and the honest instrument for it is the compiler's own output, quoted verbatim in §7.

Result (fact), interpretation, and uncertainty:

- **Fact.** Ten boundaries. Exactly **one** carried a compile-time guard against a new upstream field before this wave: the CLI's `ToolpathDiagnostic::from_core` (`rs_cam_cli/src/project.rs:103`), the precedent the plan named.
- **Fact.** The two adapters that matter for H2.1 both failed the gate on the parent. With `pub synthetic_probe_mm2: Option<f64>` added to `GenerationFindings`, `d40768a` builds clean: `cargo check -p rs_cam_core` finished in 19.88 s and `cargo check -p rs_cam_viz` in 38.57 s, both with **no error and no warning about the new field**. The defect is reproduced, not inferred. (Run in a throwaway `git worktree` at `d40768a`; the main working tree was never checked out, so `planning/airrun_2026-06-01/wanaka.toml` was never touched, and the worktree was removed afterwards.)
- **Fact.** After the fix the same probe gives `error[E0027]: pattern does not mention field 'synthetic_probe_mm2'` at `crates/rs_cam_core/src/compute/stats.rs:141`, and `-p rs_cam_viz` cannot build past `rs_cam_core`. Probe removed; `git diff --stat` on the probed file empty; both checks clean again.
- **Fact.** Field classification: **eleven core-owned** findings, **four computed-after-generation** (`move_count`, `cutting_distance`, `rapid_distance`, `retract_trips`), **one deprecated-wire-only** key (`standing_material_mm2` — a JSON key carried by no Rust identifier, emitted beside the new name at `session/mod.rs:1454` and `cli/project.rs:146`), and **zero GUI-owned** fields.
- **Fact.** No finding was being dropped at the findings->stats boundary today: both paths carried all eleven, verified field by field. This wave removes the *capability* to drop the next one, it does not recover a lost one.
- **Fact.** `CLAUDE.md` says the twelve report-only findings are "surfaced through `narrate_toolpath` and the diagnostics list". Three never reach narration (`deprecated_dial`, `derived_stepovers`, `claims_reference`) and `retract_trips` has no diagnostic adapter.
- **Interpretation.** The zero-GUI-owned-fields fact is what decides the design. The worker's inputs and output are all core types, so its mapping had nothing GUI-shaped in it and never needed to live in viz; duplicating an exhaustive destructure into the worker would have preserved two copies of one join and kept exactly the divergence that produced five patches. The join therefore lives in core (`compute::stats::stats_with_findings`) with three guards — E0027 on the `GenerationFindings` pattern, E0027 on the move-derived `ToolpathStats` pattern, E0063 on the returned literal — and both call sites collapse to one line each. The narration gap is a *projection* gap, not a core/GUI divergence: both narration literals agree with each other.
- **Uncertainty, stated.** (a) The gate proves a new field cannot be silently dropped **at the findings->stats boundary**. It says nothing about the three boundaries downstream of it that remain unguarded (census §6) — a field can still be routed into `ToolpathStats` and then fail to reach narration or the MCP wire, silently. (b) "Numbers unchanged" is a construction argument (`MoveType` has exactly four variants, so `total_cutting_distance`'s three-variant match and the helper's not-`Rapid` match are the same set), corroborated by sentries, not by an A/B of a generated project. If a fifth `MoveType` variant is ever added, that argument expires and the session path's cutting distance would move — worth knowing, not currently true.

Red-first evidence / fingerprints changed: no fingerprint captured or re-pinned; no gate, threshold or severity touched; every channel involved is report-only. The red-first material is the parent control in §7 — the defect demonstrated on `d40768a` before the fix existed, with the same probe that errors after it. A conventional failing *test* was not available for this defect class: the failure mode is a field that no code mentions, which no runtime assertion can observe. The compile-time probe is the executable form of it. New sentry `crates/rs_cam_core/tests/findings_transport_join_h21.rs` (3 tests) pins completeness, move-derived parity, and the X-19 not-measured default.

Verification (focused commands + exact known-red state):

- `cargo check -p rs_cam_core --all-targets`, `cargo check -p rs_cam_viz --all-targets` — clean.
- `cargo test -p rs_cam_core --test findings_transport_join_h21` — **3 passed**.
- Nine neighbouring findings-channel sentries, each `cargo test -p rs_cam_core --test <name>`: `standing_material_channel_am9` (6), `retract_trip_channel_am7` (5), `ramp_reach_clamp_pr8b` (8), `derived_stepover_pr6a` (4), `claims_reference_cascade_am6` (8), `zero_removal_rest_pass_a4` (2), `unified_finish_dropped_band_finding_d1` (3), `unified_finish_partial_clip_finding_c8` (4), `pencil_tip_float_channel_d1` (4) — **44 passed, 0 failed**. The A/M9 sentry matters most here: it pins both the dual-key wire and the rule that a reader must not combine `serde(alias)` with it, and it is green.
- `cargo test -p rs_cam_viz --lib compute::worker` — **47 passed**.
- `cargo test -p rs_cam_core --lib` — **2227 passed, 3 failed, 12 ignored**. The three are exactly W0's declared permanent reds: `adaptive3d::path::tests::peck_plunge_progresses_when_depth_per_pass_equals_retract_clearance`, `adaptive3d::path::tests::rapid_segment_lifts_to_safe_z_before_traverse`, `adaptive3d::tests::planner_sim_dexel_parity_agent_search`. Not chased — W0's territory, Checkpoint A pending.
- `cargo fmt --check` — clean (hand-applied, not `rustfmt <file>`, to avoid the sibling-module cascade).
- `cargo clippy --workspace --all-targets -- -D warnings` — clean; the only output is the pre-existing future-incompat note for the transitive `nom 3.2.1` / `quick-xml 0.22.0` deps.
- `wanaka_suggest_baseline` — declared environmental, not run.
- Slot discipline: `free -g` and bracketed `pgrep -af "carg[o]"` before every launch; one foreign `sysml` job and W0's adaptive3d reproduction were both waited out rather than run alongside.

NOT FIXED, STATED — owner and re-open condition:

- **B4, the hand-written `Serialize` for `session::ToolpathDiagnostic`** — a new field is silently unpublished. Owner: whoever next widens that wire. Re-open condition: it cannot become a `derive` while the A6 dual key ships from one field, so any fix must keep `tests/standing_material_channel_am9.rs`'s dual-key and no-alias-combination assertions green.
- **B7, the two parallel `ToolpathNarrationContext` literals** (`session/compute.rs:2848`, `rs_cam_viz/src/app/mcp.rs:1098`) — a new `ToolpathStats` field is silently un-narrated, and three findings plus `retract_trips` already sit on the wrong side of that gap. Owner: W8 (agent reads) or a follow-up in this lane. Re-open condition: a finding needs to reach narration, or a wave claims narration coverage. A `ToolpathNarrationContext::absorb_stats(&ToolpathStats)` helper with the same destructure shape would close it in one core-owned site.
- **The `CLAUDE.md` narration-coverage sentence is wrong for four of the twelve channels** — owner: documentation lane. Re-open condition: fix the sentence or widen B7; do not cite it as evidence of coverage in the meantime.
- **B3, the lossy `ToolpathStats` -> `session::ToolpathDiagnostic` projection** — deliberate, six of twelve published. Owner: whoever needs a finding on the MCP/CLI per-toolpath summary. Re-open condition: a consumer asks for one; widening it is a wire change, not report wiring.

Next action / checkpoint request: **no checkpoint requested** — the plan records "none for report wiring" for this item and nothing here changes generated geometry, a gate, or a threshold. Orchestrator action: mark W1 `DONE` with evidence `41cb426` + `1de1094`. Two hand-offs: give W8 the B7 narration-context gap (it is adjacent to H2.6's read paths and is the same parallel-copy shape this wave just closed one layer up), and note for any wave adding a generation finding that the compiler now names the one place to route it — `compute::stats::stats_with_findings`.
