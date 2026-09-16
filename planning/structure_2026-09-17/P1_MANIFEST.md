# P1 manifest — keep and delete for the planning purge

Date: 2026-09-17. Agent: P1. Status: **manifest only. Nothing is deleted yet.**
The orchestrator reads this file, rules on section 6, then sends "go".

Baseline, measured today: **1415 tracked files under `planning/`,
1 199 472 022 bytes**, 46 package directories, 155 top-level files
(154 markdown + 1 JSON), 74 files under `planning/archive/`.

---

## 0. Corrections to the measured facts — READ THIS FIRST

The task brief states: *"No crate source, test, bench or script reads any
`planning/` path. `rg -n 'planning/' crates scripts` finds nothing. So P1 needs
NO cargo run at all."*

**Both halves are false.** The command finds **238 distinct `planning/` paths
cited from `crates/` and `scripts/`**, across roughly 500 sites. Seven of those
paths are already dangling today, at 16 sites. Nine paths are read at run time
by tests that fail when the file is absent.

I keep the conclusion — P1 needs no cargo run — but for a different reason: the
delete set below touches no file that a **non-ignored** test reads, so the core
suite cannot change colour. The orchestrator must still rule on section 6.1.

### 0.1 Class A — run-time reads by tests that are NOT `#[ignore]`d. Hard KEEP.

| Path | Reader | Behaviour when absent |
|---|---|---|
| `planning/toolpath_acceptance/cases_agent_smoke.csv` | `crates/rs_cam_core/tests/test_data_smoke_csv_alignment.rs:50` (4 plain `#[test]`s) and `crates/rs_cam_cli/src/main.rs:264` (`default_value`) | test failure; CLI default breaks |
| `planning/gcode_current_outputs/*.nc` (64 files) | `crates/rs_cam_core/tests/gcode_validator_baseline.rs:78`, test `baseline_findings_match_expected` — a plain `#[test]` | `panic!("read {path}")` |

### 0.2 Class B — run-time reads by `#[ignore]`d evidence harnesses. KEEP the read files.

Each harness asserts the file exists, so a delete turns a green `--ignored` run
red. They do not run in the normal suite.

| Path | Bytes | Reader |
|---|---|---|
| `planning/airrun_2026-08-19/wanaka200.toml` | 19 334 | `rapid_replay_shipped_gcode_s1.rs:110`, `swept_wanaka_ab_s1.rs:120` |
| `planning/airrun_2026-08-19/wanaka200_1_Setup_1.nc` | 537 200 | `rapid_replay_shipped_gcode_s1.rs:98` |
| `planning/airrun_2026-08-19/wanaka200_2_Setup_2___front.nc` | **13 595 318** | `rapid_replay_shipped_gcode_s1.rs:102` |
| `planning/airrun_2026-06-01/wanaka.toml` | 21 045 | `p1_headless_ab_wanaka.rs:39`, `finish_planner_wanaka_decompose.rs:39` (both `assert!(path.exists())`) |
| `planning/multitool_2026-08-23/wanaka200_mt2.toml`, `…_overlap02.toml` | 38 668 | `union_coverage_m1.rs:180` |
| `planning/deep_doc_modulation_2026-09-08/T3b_r10_scallop_islands_relink3.toml` | small | `tier_band_overlap_g_overlapfill.rs:334` — a real `.join(…)` |

`planning/deep_doc_modulation_2026-09-08/Q2_r20_s15.toml` is **class D, not
class B**: `reach_map_residual_p5_1.rs:661` names it in a doc comment only, and
that harness skips on a path outside the repository. I keep it anyway because
it is small and it is the project the reach-map table was measured on.

The 13.6 MB `.nc` is the one file where the brief's "delete every blob over
5 MB" and "a path a test reads is KEEP" collide. **Ruling needed** — see 6.2.

### 0.3 Class C — write-only targets. DELETE is safe.

`planning/review_2026-08-04/artifacts/e_impl` and `…/artifacts/w8` are created
by `std::fs::create_dir_all` in `reference_plate_contract.rs:1127` and
`band_run_off_reproduction_d16_1.rs:1081`. The tests write there; nothing reads
the committed contents. Deleting them cannot break a test.

### 0.4 Class D — prose citations (doc comments, assertion strings). ~500 sites.

These name a planning document as the provenance of a sentry or a constant. No
file is opened. Deleting the target leaves a dead reference in a comment.

The repository **already tolerates this**: seven paths are cited from live code
at 16 sites and do not exist today.

| Dangling today | Sites |
|---|---|
| `planning/adaptive_review_2026-04.md` | 6 |
| `planning/acceptance_loop/findings/F-035…`, `F-036a…`, `F-036b…`, `F-039…` (4 paths) | 6 |
| `planning/P3_TRANSIT_PEAK_DOC_RCA.md` | 3 (the file now sits under `archive/`) |
| `planning/feed_modulation_calibration/BENCH_CHECKLIST.md` | 1 |
| **Total** | **7 paths, 16 sites** |

**Two frozen files cite deleted-candidate documents and I may not edit them:**

- `crates/rs_cam_core/src/feeds/mod.rs:4615` → `planning/finishing_stack_review_2026-07.md`
- `crates/rs_cam_core/src/feeds/vendor_lut.rs:626` → `planning/phase_5_schema_unlock_2026-06-01.md`

Both are **message strings shown to a user**, not comments. Both documents are
therefore KEEP, reason `LINKED-CURRENT (frozen citer)`.

---

## 1. Method

Rules applied, in order:

1. The default is DELETE.
2. KEEP when the path is in the FREEZE set; or a kept index document links it
   and treats it as current; or its own status file says open, paused or
   pending an operator decision; or it is one of the three programme
   directories.
3. A single file inside an otherwise-deleted package may be kept alone. The
   row records it as a partial keep.
4. **Two rule extensions I applied. The orchestrator ratifies or overrules
   them in section 6.**
   - **E1 — CREDITS.md counts as a linking document.** Root `CLAUDE.md` names
     `CREDITS.md` as a source of truth for dataset and formula attribution.
     `CREDITS.md` cites 26 planning paths as the provenance of data that ships
     today. The alternative to keeping them is 26 rewrites of attribution text.
   - **E2 — sentry provenance keeps a small package.** A package under about
     200 kB whose documents a live sentry names as its pre-registration is
     KEEP. Deleting 47 kB to break eight sentry citations is a bad trade. A
     large package gets the tag rewrite instead. **E2 covers packages, not
     top-level files.** The four top-level files I keep for sentry provenance
     (2.9) are judgment calls, not a rule; section 5.2 lists the heavy citers
     the orchestrator can promote the same way.
5. Retrieval for everything deleted is `git show
   planning-pre-purge-2026-09-17:<path>` plus one line in
   `planning/DELETED_INDEX.md`.

I read the status file of every package the ledger calls open, and every
citing sentence I relied on. Where a status file and the ledger disagree, the
row says which wins.

---

## 2. KEEP

### 2.1 Always

| Path | Reason | Evidence |
|---|---|---|
| `planning/PROGRESS.md` | ALWAYS | brief; `.claude/agents/cam-navigator.md:14` |
| `planning/CLAUDE.md` | ALWAYS | root `CLAUDE.md:14` |
| `planning/README.md` | ALWAYS | `README.md:52`; **rewrite required, see 6.3** |
| `planning/TECH_DEBT_REGISTER.md` | ALWAYS | brief; living register |
| `planning/feeds_literature_matrix_2026-06-03.md` | ALWAYS | `.claude/skills/refresh-lit-matrix/SKILL.md:15` |
| `planning/review_2026-07-29/SUPERSEDED_CONCLUSIONS.md` | ALWAYS (partial keep) | `planning/CLAUDE.md:19`, `PROGRESS.md:9`, `FEATURE_CATALOG.md:35` |
| `planning/metrology_2026-09-02/` (2 files) | ALWAYS | `FEATURE_CATALOG.md:34`; 19 sentry sites |

### 2.2 Frozen — another agent or account owns these. Never touch.

| Path | Files | Bytes |
|---|---|---|
| `planning/load_model_2026-09-16/` (holds an uncommitted `derate_levers.py`) | 12 | 203 163 |
| `planning/feeds_rework_2026-09-15/` | 6 | 892 907 |
| `planning/feed_modulation_calibration/` | 19 | 924 819 |
| `planning/ui_premium_2026-09-13/` | 36 | 22 138 917 |

### 2.3 Programme directories

| Path | Files | Bytes |
|---|---|---|
| `planning/duplicate_sweep_2026-09-15/` | 17 | 214 252 |
| `planning/tech_debt_2026-09-16/` | 21 | 1 546 564 |
| `planning/structure_2026-09-17/` | 1 | 8 556 |

`scripts/debt_scan.py:3` writes into `tech_debt_2026-09-16/evidence`.

### 2.4 Open — each confirmed against its own status file

| Path | Files | Status sentence I read | Verdict |
|---|---|---|---|
| `planning/linking_2026-09-09/` | 1 | `SPEC.md:3` "Status: SPEC. Code reads only, no crate edit" — the spec is written, not executed | **OPEN confirmed.** Ledger says paused; the file agrees. 8 sentry sites |
| `planning/island_clip_2026-09-09/` | 2 | `SPEC.md:4` "Status: SPEC, code reads only… The experiments below wait for the GUI" | **OPEN confirmed** |
| `planning/ui_review_2026-09-14/` | 8 | `STATUS.md:3` "Tracker for `PLAN.md`. The orchestrator edits this file. Never delete a row." Live rows | **OPEN confirmed**, and LINKED-CURRENT: `crates/rs_cam_viz/CLAUDE.md:42` "Consult … before touching the open UR4/UR5/…" |
| `planning/roughing_strategy_ab_2026-09-07/` | 25 | **No status file in the package.** Its status lives in the top-level `roughing_strategy_ab_results_2026-09-07.md`, which `PROGRESS.md:408` cites as current evidence; the ledger records an open operator ruling on the bar | **OPEN confirmed, with a caveat.** The two top-level `roughing_strategy_*_2026-09-07.md` files MOVE into this package (section 4) so the package carries its own status |
| `planning/arch_consolidation_2026-09-09/` | 13 | `STATUS.md:12` "The orchestrator updates THIS file only… Never delete a row" — live tracker; `PROGRESS.md:206` "…this block in `planning/arch_consolidation_2026-09-09/` when they complete" | **OPEN confirmed.** 35 sentry sites name `IMPLEMENTATION_PLAN.md` |

### 2.5 Run-time reads (class A and B)

| Path | Files | Bytes | Reason |
|---|---|---|---|
| `planning/toolpath_acceptance/` | 16 | 350 119 | RUNTIME (class A) + `CREDITS.md:519,600` |
| `planning/gcode_current_outputs/` | 64 | 24 928 | RUNTIME (class A) |
| `planning/multitool_2026-08-23/` | 12 | 678 776 | RUNTIME (class B) + `PROGRESS.md:655` names it a current campaign |
| `planning/airrun_2026-08-19/wanaka200.toml`, `wanaka200_1_Setup_1.nc`, `wanaka200_2_Setup_2___front.nc` | 3 | 14 151 852 | RUNTIME (class B) — **partial keep**, see 6.2 |
| `planning/airrun_2026-06-01/wanaka.toml` | 1 | 21 045 | RUNTIME (class B) — **partial keep** |
| `planning/deep_doc_modulation_2026-09-08/Q2_r20_s15.toml`, `T3b_r10_scallop_islands_relink3.toml`, `reach_truth_rasteriser.py` | 3 | small | RUNTIME (class B) + `PROGRESS.md:377` names the rasteriser as the instrument — **partial keep** |

### 2.6 Linked current, by a kept index document

| Path | Files | Bytes | Evidence line |
|---|---|---|---|
| `planning/entry_moves_2026-09-03/` | 2 | 25 121 | `PROGRESS.md:477` |
| `planning/lateral_setups_2026-08-22/` | 2 | 25 343 | `PROGRESS.md:665` |
| `planning/rapid_safety_2026-08-28/` | 11 | 156 634 | `PROGRESS.md:640`; 9 sentry sites |
| `planning/thin_organic_2026-08-27/` | 3 | 168 309 | `PROGRESS.md:611`; 32 sentry sites |
| `planning/perf_review_2026-08-19/` | 23 | 647 859 | `crates/rs_cam_core/Cargo.toml:67` (a manifest, a source of truth) and `PROGRESS.md:672` |
| `planning/conformal_finish_2026-08-28/` | 11 | 276 508 | `CREDITS.md:81,94,111,115` — paper extractions, attribution for shipped research (E1) |
| `planning/data_ingest_2026-05-29/` | 16 | 169 308 | `CREDITS.md:346` (E1) |
| `planning/ui_audit/` | 97 | 629 563 | `PROGRESS.md:833-837` "All docs in `planning/ui_audit/`… Status: nothing implemented yet; Wave 0 … is next" — rule 2 literal. **I believe this is stale; see 6.4** |
| `planning/fixtures/README.md` | 1 | 5 355 | cited from `crates/` |

### 2.7 Sentry provenance, small package (extension E2)

| Path | Files | Bytes | Sentry that names it |
|---|---|---|---|
| `planning/valley_tracing_2026-09-02/` | 2 | 47 072 | `catchment_basin_census_w0.rs`, `valley_prize_census_h0.rs`, `valley_branch_falsifier_h1.rs` (8 sites). **`TRACK.md:8` says "CLOSED 2026-09-02"; `PROGRESS.md:535` says "OPEN, phase V0". The status file wins — the package is CLOSED.** It is kept for sentry provenance, not for currency |
| `planning/spiral_finish_2026-09-01/` | 2 | 15 093 | `spiral_finish_compact_c1.rs:1187` |
| `planning/bikeseat_gate_2026-09-01/` | 2 | 15 792 | `bikeseat_gate_d1.rs:1167` |
| `planning/honest_raster_2026-09-01/` | 2 | 22 273 | 5 sites |
| `planning/pencil_watershed_spine_2026-09-03/` | 3 | 31 936 | 2 sites |
| `planning/ledger_2026-09-01/` | 4 | 53 796 | 2 sites |
| `planning/ui_declutter_2026-09-14/` | 1 | 25 690 | `the_toolpath_card_is_five_elements_dc1.rs:143` — the sentry says a ruling is still owed in `PLAN.md` |

### 2.8 Partial keeps inside deleted packages

| Kept file | Reason |
|---|---|
| `planning/data_ingest_2026-05-30/` — **all 25 files except the two `sweep_snapshot_*.tar.gz`** | `CREDITS.md:366,368,410,435,517,536,562` names `amana_long_tail.json`, `onsrud_ocr.json`, the `verification_report`, the `_gaps.md` **family** and three derivations, plus 6 sentry sites. Keeping the whole package less the tarballs costs 395 kB and removes every attribution risk; the two tarballs are 44.56 MB of the package's 44.96 MB (E1) |
| `planning/review_2026-08-04/CHIPLOAD_LITERATURE_VERDICT.md`, `LAW_MAGNITUDE_TABLES.md`, `DRILL_GATE_EVIDENCE_AUDIT.md` | `CREDITS.md:239,308,632` (E1) |
| `planning/review_2026-08-08/LIT_MATRIX_REFRESH_S2.md` | `CREDITS.md:688` **and** `crates/rs_cam_core/tests/literature_matrix/sources.toml:15` — a data string in a fixture, not a comment; rewriting it edits the fixture |
| `planning/review_2026-07-29/SUPERSEDED_CONCLUSIONS.md` | ALWAYS (2.1) |

### 2.9 Top-level files kept in place (38 files; the 4 movers of section 4 are listed here too)

| File | Reason | Evidence |
|---|---|---|
| `PROGRESS.md`, `CLAUDE.md`, `README.md`, `TECH_DEBT_REGISTER.md`, `feeds_literature_matrix_2026-06-03.md` | ALWAYS | 2.1 |
| `AGENT_CODEMAP.md` | PROGRAMME | `structure_2026-09-17/PROMPT.md` P2 step 2 orders an update to it |
| `finishing_stack_review_2026-07.md` | LINKED-CURRENT (frozen citer) | `feeds/mod.rs:4615` user message |
| `phase_5_schema_unlock_2026-06-01.md` | LINKED-CURRENT (frozen citer) | `feeds/vendor_lut.rs:626` user message; `CREDITS.md:463,495` |
| `UNIFIED_LOAD_MODEL_2026-06-18.md` | LINKED-CURRENT (E1) | `CREDITS.md:318` |
| `SUGGEST_SIM_OPTIMIZE_ACCEPTANCE.md` | LINKED-CURRENT (E1) | `CREDITS.md:599` |
| `feeds_data_ingest_consolidation_2026-05-29.md`, `…-05-30.md`, `…-06-01.md`, `feeds_data_ingest_phase4_2026-05-31.md`, `phaseB`, `phaseC`, `phaseD`, `phaseE` (8 files) | LINKED-CURRENT (E1) | `CREDITS.md:347,357,383,404,438,541,569,595` |
| `finishing_status_2026-09-01.md` | LINKED-CURRENT | `PROGRESS.md:543` names it the status board; `PROGRESS.md:598` "(start there)"; edited 2026-09-17 by the doc-correction wave |
| `finishing_synthesis_2026-08-30.md` | LINKED-CURRENT | `PROGRESS.md:599` "Theory and measurements: … (§12 is the closure — do not act on §11 alone)" — the same sentence as the kept status board; 13 sentry sites, one an assertion message at `conformal_spiral_synthetic_f2.rs:2126` |
| `deep_doc_modulation_2026-09-08.md` | LINKED-CURRENT | cited by **two open specs** (`linking_2026-09-09/SPEC.md:7`, `island_clip_2026-09-09/SPEC.md:3`) and `PROGRESS.md:414`. **MOVE, see section 4** |
| `machine_kinematics_confidence_2026-09-07.md` | SENTRY (E2) | 6 sentry sites; provenance of the shipped plunge guard |
| `post_reference_notes.md` | SENTRY | `gcode_emulator_validation.rs:146,373` print its path in a skip message an operator reads |
| `cycle_time_rebench.md` | OPEN | `PROGRESS.md:2059` "Known open work — F-034 cycle-time re-bench … re-measure per" |
| `ab_instrument_flags_2026-09-08.md` | SENTRY | `src/simulation_cut.rs` + `air_cut_one_time_base_g_airdenom.rs` |
| `roughing_strategy_ab_results_2026-09-07.md`, `roughing_strategy_terrain_2026-09-07.md` | OPEN | 2.4. **MOVE, see section 4** |
| `pencil_linking_2026-09-04.md` | LINKED-CURRENT | 2 sentry sites; superseded by the open `linking_2026-09-09`. **MOVE, see section 4** |
| `IMPLEMENTATION_PLAN.md`, `Performance_review.md`, `FUTURE_PLANS.md`, `WORKSPACE_UX_REDESIGN_PLAN.md`, `SIMULATION_WORKSPACE_VISION.md`, `WORKFLOW_TEST_PLAN.md`, `VOXEL_SIM_DESIGN.md`, `MULTI_SETUP_FEASIBILITY.md`, `MULTI_SETUP_UX_PLAN.md`, `ALIGNMENT_PINS_DESIGN.md`, `TOOL_LIBRARY_DESIGN.md` (11 files, ~115 kB) | LINKED-CURRENT, **rule-2 literal only** | `planning/README.md:10-20` calls each one active. **The index is stale; see 6.3** |
| `OPTIMIZER_REFACTOR_G16.md`, `G16_LAYERED_SCORING_PROGRESS.md`, `SERVICE_LAYER_EXTRACTION.md` (~140 kB) | LINKED-CURRENT, **rule-2 literal only** | `PROGRESS.md:1905-1906` "Current priorities … (in flight) … Tracker (read first)". **That block is from 2026-03/04; see 6.3** |

---

## 3. DELETE

899 files, 1 145 842 903 bytes. One row per package. The rationale column is
the line that goes into `planning/DELETED_INDEX.md`.

### 3.1 Reviews

| Path | Files | Bytes | DELETED_INDEX line |
|---|---|---|---|
| `planning/review_2026-08-08/` (less `LIT_MATRIX_REFRESH_S2.md`) | 187 | 11 891 457 | Tech-debt review 3 (2026-08-08). Decided the apply-contract, heatmap-vocabulary and LUT-boundary censuses and the G-LV.2 crash capture; all findings landed and the programme closed 2026-08-08 at `53b1c72`. The literature refresh it produced is kept |
| `planning/review_2026-08-04/` (less 3 CREDITS-cited files) | 108 | 6 484 416 | Tech-debt review 2 (2026-08-04). Decided the chipload literature verdict, the reference-plate fixture and the drill-gate audit; closed via `TECH_DEBT_2_CLOSEOUT.md`. `artifacts/e_impl` and `artifacts/w8` are test **outputs**, recreated on demand (class C). The three cited verdict files are kept |
| `planning/review_2026-07-29/` (less `SUPERSEDED_CONCLUSIONS.md`) | 14 | 2 232 974 | Finishing-stack review (2026-07-29). Decided tool-scale semantics, measurement domains and checkpoints A–C; all landed. Its one durable output, the superseded-conclusions record, is kept |
| `planning/review_2026-07-27/` | 1 | 10 989 | One-file review stub, superseded by `review_2026-07-29` four days later. No citation anywhere |

### 3.2 UI packages

| Path | Files | Bytes | DELETED_INDEX line |
|---|---|---|---|
| `planning/ui_review_2026-09-09/` | 137 | 14 891 755 | UX review pass (2026-09-09). Decided 40+ UX findings (R03 depth-beyond-stock cautions, boundary-control visibility, W03 source tracks); every finding shipped with its own viz sentry. Superseded by `ui_review_2026-09-14` |
| `planning/ui_fix_2026-09-09/` | 62 | 2 639 501 | UX fix wave (2026-09-09). Decided the bottom-Z pin note and the F4/J2 report fixes; all landed and sentried. Superseded by `ui_review_2026-09-14` |
| `planning/ui_overlays_ux_2026-09-08/` | 7 | 3 881 762 | Screenshot evidence for the overlay audit. The audit shipped as the P6 Overlays panel (`92a62851`); the pictures are spent |

### 3.3 Airrun and measurement campaigns

| Path | Files | Bytes | DELETED_INDEX line |
|---|---|---|---|
| `planning/airrun_2026-08-19/` (less 3 run-time files) | 24 | 955 007 960 | wanaka200 air run + efficiency campaign (2026-08-19..23). Decided the overnight tuning ladder and phase-2 efficiency arms; outcome folded into `PROGRESS.md`. Carries the 948 MB `p2_a1_lakes_vbit_chk4.html` — 83 % of the whole planning tree. The three files two `#[ignore]`d harnesses read are kept |
| `planning/airrun_2026-06-01/` (less `wanaka.toml`) | 10 | 3 404 299 | First wanaka air run (2026-06-01). Decided the runbook and the marker-star test; the two posted `.nc` programs are superseded and one is named in `DO_NOT_RUN_THESE_PROGRAMS.md` lineage. `wanaka.toml` is the canonical project two harnesses load and is kept |
| `planning/deep_doc_modulation_2026-09-08/` (less 3 files) | 120 | 53 971 219 | Deep-DOC modulation study (2026-09-08/09). Decided that iso-scallop dominates raster at matched finish and that the island tier loses on retract count; the conclusions live in the kept top-level `deep_doc_modulation_2026-09-08.md`. 120 PNG/SVG renders deleted; the rasteriser and the two harness projects are kept |

### 3.4 Data ingest and probes

| Path | Files | Bytes | DELETED_INDEX line |
|---|---|---|---|
| `planning/data_ingest_2026-05-30/sweep_snapshot_5a67c1d.tar.gz`, `sweep_snapshot_phaseE_completion.tar.gz` | 2 | 44 563 556 | Two 22 MB parameter-sweep snapshot tarballs from the phase-E feeds ingest. The verdict JSON they summarise stays in the package, no document or test names the archives, and the LUT rows they produced are bundled |
| `planning/probe_artifacts/` | 35 | 21 814 850 | Loose AgentSearch probe renders. No citation in any document or test; the probe log they belong to is itself deleted |

### 3.5 Archive

| Path | Files | Bytes | DELETED_INDEX line |
|---|---|---|---|
| `planning/archive/` | 74 | 22 686 119 | The pre-2026-06 archive: BREP/STEP plans, the remediation tracker, the consolidation audit, the phase1–5 UX plans and the 2026-08-05 mega-harness `.archived` sources. Retired by the operator ruling of 2026-09-16 — obsolete planning material is deleted, not archived. `planning/CLAUDE.md:22-23` must lose its "preserve under `planning/archive/`" sentence |

### 3.6 Other packages

| Path | Files | Bytes | DELETED_INDEX line |
|---|---|---|---|
| `planning/ux-fixes/` | 5 | 146 786 | Phased UX improvement plans, phases 1–5 (focus loss, theme, timeline, help, polish). All shipped; superseded by three later UI programmes |

### 3.7 Top-level files — 113 files, 2 215 260 bytes

Grouped by family. Every one of the 155 top-level files appears exactly once in
section 2.9 (38 kept in place, plus the 4 movers listed there), section 4
(4 moved) or this table. 38 + 4 + 113 = 155.

| Family | Files | DELETED_INDEX line |
|---|---|---|
| **AgentSearch and adaptive investigations, 2026-04/05** — `AGENTSEARCH_INVESTIGATION_LOG.md`, `AGENTSEARCH_NEXT_SESSION.md`, `agent_search_diagnosis_plan.md`, `agent_search_morning_report_2026-04-16.md`, `agent_search_probe_log.md`, `agent_debugging_improvements.md`, `adaptive_corner_clearing_investigation_2026-05-28.md`, `adaptive_remediation_phase2_probes_2026-04-12.md`, `ADAPTIVE3D_INCOMPLETE_CLEARING.md`, `ADAPTIVE3D_LINK_DIST_DIVERGENCE.md`, `ADAPTIVE3D_DPP_ISLANDS_AND_SHALLOW_MILL.md`, `ADAPTIVE3D_DEEP_CHANNEL_GOUGE_2026-06-16.md`, `ADAPTIVE_CLEARING_ALGO_REVIEW_2026-06-12.md`, `ROUGH_ENGAGEMENT_INVESTIGATION.md`, `F4_HELIX_AGENT_SEARCH_RCA.md` | 15 | Diagnosed the AgentSearch entry and clearing defects of 2026-04/06. All three `ClearingStrategy3d` variants are live and the deep-channel gouge is fixed (`fa27b08`); the boundary-walk entry landed 2026-04-15 |
| **Optimizer and G16, 2026-05** — `OPTIMIZER_LOGIC.md`, `OPTIMIZER_UX_PLAN.md`, `OPTIMIZER_UX_DIALIN_FIXES.md`, `OPTIMIZE_EXPLAINABILITY_AND_PEAK_FINDING.md`, `PRE_OPTIMIZE_DEFAULTS_AUDIT.md` | 5 | Designed the optimizer reorg and its explainability surface; shipped. The two trackers `PROGRESS.md` still calls in-flight are kept (2.9) |
| **RCA notes, 2026-05** — `F1_RCA.md`, `F10_RCA.md`, `P1_AIR_CUT_THRESHOLDS_RCA.md`, `P2_PLUNGE_STRESS_GATE_RCA.md`, `P4_DRILL_METRIC_SUPPRESSION_RCA.md`, `P5_STALE_DEFAULTS_VALIDATOR_RCA.md`, `F5_FRESH_DEFAULTS_POLICY.md`, `PARAMETER_WARNINGS.md` | 8 | Root-cause notes for acceptance findings F-1..F-10 and P1..P5. Each finding is now guarded by a named sentry under `crates/rs_cam_core/tests/` |
| **UX dial-in and testing, 2026-05** — `UX_DIALIN_FIX_PLAN_2026-05-20.md`, `UX_DIALIN_REVIEW_2026-05-20.md`, `UX_PAIN_POINTS_2026-05-11.md`, `UX_TESTING_SESSION_PLAN.md`, `UX_TESTING_SESSION_PROMPT.md`, `UI_IA_AUDIT_WORKFLOW.md`, `WANAKA_ASSESSMENT_2026-05-19.md` | 7 | The first UX pain-point census and its fix plan. Roadmap A shipped; superseded by the IA audit, the 2026-09 review passes and `ui_premium_2026-09-13` |
| **Suggest / sim / optimize sweep, 2026-05** — `SUGGEST_SIM_OPTIMIZE_AGENT_RUN_PROMPT.md`, `SUGGEST_SIM_OPTIMIZE_SWEEP_PLAN.md`, `SUGGEST_SIM_OPTIMIZE_SYNTHESIS_PROMPT.md` | 3 | Agent prompts and the sweep plan for the acceptance campaign. The acceptance record itself is kept (2.9) |
| **Spans and structure, 2026-05** — `SPANS_EVERYWHERE.md`, `STRUCTURAL_ENTRY_SPANS_AND_LOCALITY.md`, `SIMULATION_SPAN_COVERAGE.md`, `STEP3_PREP_OPTIMIZATION_SURFACE.md`, `STEP5_PREP_RETARGETERS.md`, `OPERATION_TRANSFORM_AUDIT.md`, `DRESSUP_TRANSFORM_CLEANUP.md` | 7 | Designed structural and semantic spans and the entry-locality model; shipped across all 23 operation families and guarded by span-coverage tests |
| **G-code and post, 2026-05/06** — `GCODE_EXPORT_OVERHAUL.md`, `gcode_gap_report.md`, `POST_LAYER_AUDIT_2026-06-11.md` | 3 | Specified the post-processor layer and its dialects; shipped and pinned by `gcode_current_outputs` captures, which are kept |
| **Unification and refactor, 2026-04/06** — `LOADER_UNIFICATION.md`, `CLI_PROJECT_UNIFICATION.md`, `CODEBASE_UNIFICATION_PLAN.md`, `PHASE_4F_JOBSTATE_REMOVAL.md`, `architectural_refactor_2026-06-06.md`, `architectural_refactor_2026-06-06_v2.md`, `architectural_refactor_2026-06-06_v2_decisions.json`, `architectural_refactor_orchestrator_prompt.md`, `DEFECT_CLASS_CLEANUP_2026-06-10.md`, `phase_4_promotion_plan_2026-06-01.md` | 10 | Unified the two project loaders and removed `JobState`. Completed 2026-06-08; the duplicate sweep of 2026-09-16 removed the last legacy loader and `JobState` outright |
| **Feeds data ingest working papers, 2026-05/06** — `feeds_data_coverage_audit_2026-05-29.md`, `feeds_data_source_acquisition_2026-05-29.md`, `feeds_data_ingest_2026-05-30_phased_plan.md`, `feeds_data_ingest_completion_2026-05-31.md`, `feeds_data_ingest_completion_results_2026-05-31.md`, `feeds_phase5_consolidation_2026-06-01.md`, `feeds_modal_enhancements_2026-06-02.md`, `feeds_modal_enhancements_PROMPT_2026-06-02.md`, `feeds_workspace_design_2026-06-02.md`, `feeds_workspace_breakout_investigation_2026-06-02.md`, `combined_suggest_design_2026-06-03.md`, `combined_suggest_v3_2026-06-04.md`, `cutting-calcs-data-gaps.md`, `feed_modulation_roadmap.md`, `KC_MILLING_CALIBRATION_2026-06-17.md`, `tool_kinematics_chipload_audit_2026-05-31.md`, `cutter_axial_constraints_2026-06-06.md`, `tool_library_modal_plan.md`, `tool_diagnostics_generic_plan.md` | 19 | Working papers for the feeds/speeds ingest. The bundled data's attribution lives in `CREDITS.md`, which cites the eight consolidation and phase records kept in 2.9; these are the drafts behind them |
| **Finishing campaign, 2026-07/08** — `finishing_speedup_project.md`, `multitool_finishing_plan.md`, `unified_finishing_pass_plan.md`, `unified_finishing_probe_prompt.md`, `unified_finish_planner_design.md`, `unified_v3_design.md`, `unified_v3_design_prompt.md`, `v3_campaign_map.md`, `v3_workplan.md`, `v3_proof_spec.md`, `v3_process_proof_prompt.md`, `rest_cascade_optimal_plan.md`, `p2b_decomposition_prompt.md`, `p2d_router_prompt.md`, `p2f_fidelity_parity_prompt.md`, `p2g_quality_matrix_prompt.md`, `p2_selective_finishing_prompt.md` | 17 | The unified-finishing v3 campaign. CLOSED 2026-07-28 **not provable on this fixture** (coarse TIN); the surviving verdicts are in `review_2026-07-29/SUPERSEDED_CONCLUSIONS.md`, `finishing_status_2026-09-01.md` and `finishing_synthesis_2026-08-30.md`, all three kept |
| **Pencil campaign, 2026-06/07** — `pencil_investigation_2026-07.md`, `pencil_investigation_prompt.md`, `pencil_curvature_detector_prompt.md`, `pencil_restdepth_detector_prompt.md`, `pencil_reference_fidelity_prompt.md`, `pencil_review_prompt.md`, `pencil_postmortem_and_rest_driven_design.md` | 7 | Diagnosed pencil's valley targeting and its detectors. Fixed at `29a6d61` (coverage 0.137 → 0.80); the watershed-spine follow-up closed REJECT 2026-09-03 and its package is kept |
| **Misc closed** — `3D_FINISH_BUGS.md`, `3D_FINISH_FIX_PROMPT.md`, `ACCEL_FRIENDLY_TOOLPATHS_2026-06-20.md`, `STRATEGY_ADVISOR_2026-06-17.md`, `HEIGHTS_SETUP_FRAME_AUDIT_2026-06-12.md`, `DEXEL_Z_ONLY_INVESTIGATION.md`, `TECH_DEBT_AUDIT.md`, `TECH_DEBT_REVIEW_2026-06-10.md`, `remaining_stock_sim_dependency_ux.md`, `machine_run_and_datum_2026-09-07.md`, `ui_overlays_ux_2026-09-08.md`, `ui_overlays_dead_duplicate_2026-09-08.md` | 12 | Each closed: accel conditioning is default-on for roughing; the strategy advisor shipped; the identity-setup frame bug was fixed 2026-06-12; the dexel Z-only roadmap landed steps 0–5; the two 2026-04/06 tech-debt audits are named by `TECH_DEBT_REGISTER.md:5` **as point-in-time evidence, not as current** (rule 2 not met); the datum work landed at `e17b2ff0`; the overlay audit shipped as the P6 panel at `92a62851` |

---

## 4. MOVE INTO A PACKAGE

| File | Destination | Reason |
|---|---|---|
| `planning/roughing_strategy_ab_results_2026-09-07.md` | `planning/roughing_strategy_ab_2026-09-07/RESULTS.md` | The package has 25 arm files and no status file; this is its status |
| `planning/roughing_strategy_terrain_2026-09-07.md` | `planning/roughing_strategy_ab_2026-09-07/TERRAIN.md` | Same campaign, same date |
| `planning/deep_doc_modulation_2026-09-08.md` | `planning/deep_doc_modulation_2026-09-08/STUDY.md` | The package survives as a three-file partial keep; the study document is what the two open specs cite |
| `planning/pencil_linking_2026-09-04.md` | `planning/linking_2026-09-09/PENCIL_2026-09-04.md` | Its measurements are the second acceptance case of the open G-LINKSTAGE spec |

A move rewrites every citing line. Section 5 lists them.

---

## 5. Dangling links and the planned fix

### 5.1 In kept index documents — these must be fixed in the delete commits

Built mechanically: every path in the DELETE set, matched against the kept
documents by full path and by bare file name, then each hit read to drop the
false positives that a shared name such as `STATUS.md` or `IMPLEMENTATION_PLAN.md`
produces. **31 lines in 4 documents.**

| Citing line | Cites | Fix |
|---|---|---|
| `planning/CLAUDE.md:22-23` | "Preserve useful measurements … under `planning/` or `planning/archive/`" | Rewrite: the archive is gone; name the tag instead |
| `planning/README.md:26-48` | `ux-fixes/` table and the whole "Archived" list (11 rows) | Delete both sections; rewrite the file as a short index (see 6.3) |
| `planning/PROGRESS.md:348,350` | `ui_overlays_ux_2026-09-08.md`, `ui_overlays_dead_duplicate_2026-09-08.md` | Append "(deleted 2026-09-17; `git show planning-pre-purge-2026-09-17:planning/<file>`)" |
| `planning/PROGRESS.md:377` | `deep_doc_modulation_2026-09-08/reach_truth_rasteriser.py` | No change — the file is kept |
| `planning/PROGRESS.md:414` | `deep_doc_modulation_2026-09-08.md` | Repoint to `deep_doc_modulation_2026-09-08/STUDY.md` (move) |
| `planning/PROGRESS.md:408` | `roughing_strategy_ab_results_2026-09-07.md` | Repoint to `roughing_strategy_ab_2026-09-07/RESULTS.md` (move) |
| `planning/PROGRESS.md:533-545` | `valley_tracing_2026-09-02` "Status: **OPEN, phase V0 … not run**" | Correct to CLOSED — `TRACK.md:8` says closed, and the same PROGRESS block says so eleven lines later |
| `planning/PROGRESS.md:717` | `review_2026-08-04/TECH_DEBT_2_CLOSEOUT.md` | Tag form |
| `planning/PROGRESS.md:721` | `review_2026-08-04/ORCHESTRATION_LOG.md` | Tag form |
| `planning/PROGRESS.md:759,794` | `review_2026-07-29/ORCHESTRATION_LOG.md` (bare name) | Tag form |
| `planning/PROGRESS.md:792` | `review_2026-07-29/TECH_DEBT_RESEARCH_AND_FIX_PLAN.md` | Tag form |
| `planning/PROGRESS.md:805` | `review_2026-07-29/TOOL_SCALE_SEMANTICS.md` (bare name) | Tag form |
| `planning/PROGRESS.md:812` | `review_2026-07-29/CHECKPOINT_A_EVIDENCE.md` (bare name) | Tag form |
| `planning/PROGRESS.md:814` | `review_2026-07-29/CHECKPOINT_B_EVIDENCE.md` (bare name) | Tag form |
| `planning/PROGRESS.md:869,965,1133,1238,1353,1434,1494` (7 lines) | `DEXEL_Z_ONLY_INVESTIGATION.md` | Tag form |
| `planning/PROGRESS.md:1562,1711` | `UX_PAIN_POINTS_2026-05-11.md` | Tag form |
| `planning/PROGRESS.md:1718` | `cutting-calcs-data-gaps.md` | Tag form |
| `planning/PROGRESS.md:1764` | `SIMULATION_SPAN_COVERAGE.md` | Tag form |
| `planning/PROGRESS.md:1770` | `TECH_DEBT_AUDIT.md` (a relative markdown link) | Tag form; the link must stop being a link |
| `planning/PROGRESS.md:1879` | `planning/CONSOLIDATION_AUDIT.md` | **Already dangling today** (the file is in `archive/`). Tag form |
| `planning/PROGRESS.md:1918` | `review/BREP_STEP_REVIEW.md` (root `review/`) | Depends on the root ruling, 6.5 |
| `planning/PROGRESS.md:2002` | `architecture/TRI_DEXEL_SIMULATION.md` | Depends on the root ruling, 6.5 |
| `planning/PROGRESS.md:2016` | `HEIGHTS_SETUP_FRAME_AUDIT_2026-06-12.md` | Tag form |
| `planning/PROGRESS.md:2027` | `ADAPTIVE_CLEARING_ALGO_REVIEW_2026-06-12.md` | Tag form |
| `planning/TECH_DEBT_REGISTER.md:5` | `TECH_DEBT_AUDIT.md`, `TECH_DEBT_REVIEW_2026-06-10.md` | Tag form |
| `CREDITS.md` (26 citations) | see 2.8 and 2.9 | **No change** — extension E1 keeps every cited file |
| `README.md:24` | "`planning/`: active backlog, status, and archived planning snapshots" | Drop "and archived planning snapshots" |
| `README.md:50-51` | `architecture/README.md`, `research/README.md` | Depends on the root ruling, 6.5 |
| `FEATURE_CATALOG.md:34,35` | `metrology_2026-09-02/FINDINGS.md`, `review_2026-07-29/SUPERSEDED_CONCLUSIONS.md` | **No change** — both kept |
| `crates/rs_cam_viz/CLAUDE.md:42` | `ui_review_2026-09-14/` | **No change** — kept |

### 5.2 In crate source and tests — the class D problem

About 500 sites cite a planning document in a doc comment or an assertion
string. **The delete set breaks 293 sites, over 103 distinct paths.** I counted
them; the number is exact. I cannot repair them in P1:
two are inside the FREEZE set, and any repair of that size needs `cargo fmt`
and a compile, which P1 is told not to run.

Recommendation, for the orchestrator's ruling in 6.1: **do not rewrite them.**
Add one sentence to root `CLAUDE.md` and `crates/rs_cam_core/CLAUDE.md`:

> A `planning/…` path in a doc comment that no longer exists is retrievable
> with `git show planning-pre-purge-2026-09-17:<path>`.

The 16 sites already dangling today (section 0.4) show the repository accepts
this. The alternative costs 293 edits, touches two frozen files, and risks the
`rustfmt` cascade.

The heaviest deleted citers, if the orchestrator rules the other way — these
are also the files it could promote to KEEP instead:

| Deleted path | Sites broken |
|---|---|
| `planning/unified_v3_design.md` | 20 |
| `planning/review_2026-07-29/TECH_DEBT_RESEARCH_AND_FIX_PLAN.md` | 15 |
| `planning/DEXEL_Z_ONLY_INVESTIGATION.md` | 13 |
| `planning/unified_finish_planner_design.md` | 11 |
| `planning/airrun_2026-08-19/RUN_LOG.md` | 10 |
| `planning/ADAPTIVE_CLEARING_ALGO_REVIEW_2026-06-12.md` | 10 |
| `planning/review_2026-08-04/FINISHING_OPEN_DEFECTS_EVIDENCE.md` | 9 |
| `planning/review_2026-08-04/FEEDS_CENSUS.md` | 7 |
| `planning/cutter_axial_constraints_2026-06-06.md` | 7 |
| `planning/review_2026-08-08/XVAC_CENSUS.md` | 6 |
| `planning/review_2026-07-29/ORCHESTRATION_LOG.md` | 6 |
| the remaining 92 deleted paths | 179 |

---

## 6. What the rules could not decide — the orchestrator rules on these

**6.1 The P1 gate is not achievable as written.** `PROMPT.md` requires that
`rg -n "planning/" crates/*/tests crates/*/src` show no path that no longer
exists. 238 distinct paths are cited; the tree already fails this gate today at
16 sites. The purge breaks 293 more. Ruling needed: accept the tag-retrieval
sentence (5.2) and re-scope the gate to "no path a test **reads at run time**
is missing", or order the 293 rewrites as a separate work package.

**6.2 The 13.6 MB `wanaka200_2_Setup_2___front.nc`.** It is a blob over 5 MB
*and* a file an `#[ignore]`d harness asserts exists. Keep the file, or delete it
together with `crates/rs_cam_core/tests/rapid_replay_shipped_gcode_s1.rs`. I
kept it; the harness is the only rapid-safety replay instrument and the memory
ledger says its zeros are not to be trusted before 2026-08-28, so the
instrument still has work to do.

**6.3 Rule 2 leaks through two stale indexes.** `planning/README.md:10-20` calls
eleven 2026-03 documents active, and `PROGRESS.md:1905-1909` calls three more
"Current priorities (in flight)". A literal reading of rule 2 keeps all
fourteen. Their content says they are finished or abandoned. I kept them,
because a document I am about to rewrite should not be the thing that condemns
its own targets. **Recommendation:** rewrite `planning/README.md` first, then
judge the fourteen on content in the same commit. That would delete about
255 kB more.

**6.4 `planning/ui_audit/` — 97 files.** `PROGRESS.md:833-837` says "All docs in
`planning/ui_audit/` … Status: nothing implemented yet; Wave 0 … is next". That
is rule-2 current, so I marked it KEEP. But the egui 0.34.3 upgrade it planned
shipped, and three later UI programmes replaced it. **Recommendation:** DELETE
and rewrite `PROGRESS.md:833-837`. That removes 97 files and 630 kB.

**6.5 Root-level directories — the brief asks for them here, but P3 owns them.**
Facts measured; no decision taken:

| Path | Tracked files | Bytes | Finding |
|---|---|---|---|
| `review/` | 106 | 671 945 | `PROGRESS.md:1918` cites `review/BREP_STEP_REVIEW.md` as the history of a fix. Sits beside `planning/review_*`. **Recommend DELETE with an index line**, and the tag form at `PROGRESS.md:1918` |
| `research/` | 23 | 459 703 | `CREDITS.md` cites nine `research/*.md` files as algorithm-lineage attribution (lines 23,24,38,39,56,57,71,72,755-763); `README.md:51` links `research/README.md`. **KEEP** — extension E1 applies with more force here than anywhere |
| `architecture/` | 8 | 72 409 | `CREDITS.md:182,841` and `PROGRESS.md:1909,2002` cite `architecture/TRI_DEXEL_SIMULATION.md` and `high_level_design.md`; `README.md:50` links `architecture/README.md`. **KEEP** |
| `toolpath_stress_test/` | 9 | 97 425 | **Cited by two kept documents**: `AI_MACHINIST_ANALYSIS_REFERENCE.md:350` (a root product doc) and `planning/toolpath_acceptance/baselines/2026-05-24_tier0_baseline.md:47` name `toolpath_stress_test/agents/analyze_sweep.py` as a runnable command. **KEEP** |
| `tests/` = `tests/step_validation/` | 4 | 8 335 | A standalone crate, named in root `Cargo.toml:10` under `exclude`. A delete must also drop that line, so it is a manifest change. `planning/conformal_finish_2026-08-28/primitives_inventory.md:103` describes it as truck-only. **Recommend DELETE in P3, with the `Cargo.toml` edit** |
| `demos/`, `reference/` | 0 | 0 | No tracked files. `reference/validators/linuxcnc` is looked up at run time by `gcode_emulator_validation.rs:373`, but that test **skips gracefully** when the binary is absent (it only panics under `ci_required_for`). Removing the untracked directories is an operator decision |
| `G13_PROMPT.md` | 1 | — | No reference outside itself. **Recommend DELETE** |
| `AGENT_PROMPT.md` | 1 | — | Two references, and both are in the delete set: `G13_PROMPT.md:77` and `planning/cutting-calcs-data-gaps.md:391`. **Recommend DELETE together with them** |
| `AI_MACHINIST_ANALYSIS_REFERENCE.md` | 1 | — | `planning/README.md:53` names it. Root product doc. **KEEP** |
| `fixtures/debug_adaptive/` | 7 | 5 360 640 | `job.toml` is a CLI job that writes `wanaka_diag.json`, `wanaka_adaptive.nc` and `wanaka_toolpath.svg`. **Nothing reads those outputs.** `planning/toolpath_acceptance/cases_agent_smoke.csv` holds no `debug_adaptive` row (grep count 0). **Recommend DELETE the five generated files** — `wanaka_adaptive.nc`, `wanaka_diag.json`, `wanaka_toolpath.svg`, `adaptive_terrain.svg`, `contour_parallel_terrain.svg` = 5 341 561 bytes — and keep `job.toml` plus `fixtures/debug_adaptive/traces/…json`. Note: the **tracked** `wanaka_diag.json` is 134 bytes, not 230 MB; the 230 MB file the brief names is the untracked regenerated copy |

**6.6 `README.md:24` and `:52`.** Line 24 says `planning/` holds "active
backlog, status, and archived planning snapshots" — the last clause dies with
the archive. Line 52 links `planning/README.md`, which survives but needs the
rewrite of 6.3.

---

## 7. Totals

| | Files | Bytes |
|---|---|---|
| Baseline under `planning/` | 1 415 | 1 199 472 022 |
| **DELETE** | **899** | **1 145 842 903** |
| After the purge | 516 | 53 629 119 |

That deletes 63.5 % of the files and 95.5 % of the bytes.

Of the deleted bytes, 955 007 960 (83 %) is one package, `airrun_2026-08-19`,
and 948 300 086 of that is one file, `p2_a1_lakes_vbit_chk4.html`.

Top-level files: 155 today = **38 kept in place + 4 moved into a package +
113 deleted**.

Package directories: 46 today → 36 kept whole or in part, 10 deleted whole.

A `git filter-repo` pass to drop the 948 MB blob from history is **not** part
of this programme. It stays an operator decision for a moment when no session
and no clone is live.
