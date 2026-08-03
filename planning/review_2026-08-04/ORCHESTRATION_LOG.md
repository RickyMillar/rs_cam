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
