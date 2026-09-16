# Deleted planning material — index

The structure purge of 2026-09-17 removed **1010 tracked files** from
`planning/` and the repository root. Nothing was archived: the operator ruling
of 2026-09-16 retires archiving. Every removed file stays retrievable at an
annotated tag.

```
git tag -n1 planning-pre-purge-2026-09-17
git show planning-pre-purge-2026-09-17:planning/<path>
git show planning-pre-purge-2026-09-17:review/<path>
```

The tag names the commit before the first deletion. `git show <tag>:<path>`
prints any file at its pre-purge content. To list a directory as it was, use
`git ls-tree -r --name-only planning-pre-purge-2026-09-17 planning/archive`.

The keep/delete reasoning is `structure_2026-09-17/P1_MANIFEST.md`.

History was **not** rewritten. The 948 MB blob stays in the object database;
a `git filter-repo` pass is an operator decision for a moment when no session
and no clone is live.

---

## Reviews

| Package | Files | What it decided | Why it went |
|---|---|---|---|
| `review_2026-08-08/` | 187 | Tech-debt review 3: the apply-contract, heatmap-vocabulary, XVAC and LUT-boundary censuses, the arc-fit ratio evidence and the G-LV.2 crash capture | Every finding landed; the programme closed 2026-08-08 at `53b1c72`. `LIT_MATRIX_REFRESH_S2.md` is kept — `CREDITS.md:688` and a fixture data string cite it |
| `review_2026-08-04/` | 108 | Tech-debt review 2: the chipload literature verdict, the law-magnitude tables, the reference-plate fixture spec, the drill-gate evidence audit and the mega-harness policy | Closed via `TECH_DEBT_2_CLOSEOUT.md`. Three verdict files are kept for `CREDITS.md`. `artifacts/e_impl` and `artifacts/w8` were test **outputs**, recreated by the harness |
| `review_2026-07-29/` | 14 | Finishing-stack review: tool-scale semantics, measurement domains, checkpoints A–C, the radius audit and the antipattern backlog | All landed. Its one durable output, `SUPERSEDED_CONCLUSIONS.md`, is kept and cited by `planning/CLAUDE.md`, `PROGRESS.md` and `FEATURE_CATALOG.md` |
| `review_2026-07-27/` | 1 | A review stub | Superseded by `review_2026-07-29` four days later; no citation anywhere |
| `review/` (root) | 106 | The independent BREP/STEP review and its working notes | Its findings landed (`PROGRESS.md` 2026-06 entry). It sat at the repository root beside `planning/review_*` |

## UI packages

| Package | Files | What it decided | Why it went |
|---|---|---|---|
| `ui_review_2026-09-09/` | 137 | The UX review pass: 40+ findings including the depth-beyond-stock cautions, boundary-control visibility and the W03 source tracks | Every finding shipped with its own viz sentry. Superseded by `ui_review_2026-09-14`, which is kept and open |
| `ui_fix_2026-09-09/` | 62 | The UX fix wave: the bottom-Z pin note and the F4/J2 report fixes | All landed and sentried |
| `ui_overlays_ux_2026-09-08/` | 7 | Screenshot evidence for the overlay audit | The audit shipped as the P6 Overlays panel (`92a62851`) |
| `ui_audit/` | 97 | The GUI information-architecture audit, the component-layer architecture, the final design mockups and the egui 0.34.3 spike | It shipped: the upgrade landed, and the three 2026-09 UI programmes replaced the backlog. `PROGRESS.md` is corrected |

## Air-run and measurement campaigns

| Package | Files | What it decided | Why it went |
|---|---|---|---|
| `airrun_2026-08-19/` | 24 of 27 | The wanaka200 air run, the overnight tuning ladder and the phase-2 efficiency arms | The outcome is folded into `PROGRESS.md`. It carried `p2_a1_lakes_vbit_chk4.html` at 948 300 086 bytes — 79 % of the whole tracked planning tree. **Kept**: `wanaka200.toml`, `wanaka200_1_Setup_1.nc`, `wanaka200_2_Setup_2___front.nc`, which `rapid_replay_shipped_gcode_s1.rs` and `swept_wanaka_ab_s1.rs` read at run time |
| `airrun_2026-06-01/` | 10 of 11 | The first wanaka air run, its runbook and the marker-star test | The posted programs are superseded. **Kept**: `wanaka.toml`, which `p1_headless_ab_wanaka.rs` and `finish_planner_wanaka_decompose.rs` load |
| `deep_doc_modulation_2026-09-08/` | 120 of 123 | That iso-scallop dominates raster at matched finish, that the single-pass Q3 R1.5 iso-scallop matches both pairs at the bar in 67 % of the time, and that the island tier loses because confinement is retract-count-bound | 120 PNG and SVG renders. The conclusions live in `STUDY.md`, which moved into this directory and which two open specs cite. **Kept**: the rasteriser and the two harness projects |

## Data ingest and probes

| Package | Files | What it decided | Why it went |
|---|---|---|---|
| `probe_artifacts/` | 35 | Loose AgentSearch probe renders | No document and no test names them; the probe log they belong to is in this purge |
| `data_ingest_2026-05-30/` tarballs | 2 | Two 22 MB parameter-sweep snapshots from the phase-E feeds ingest | The verdict JSON they summarise stays, and the LUT rows they produced are bundled. The other 25 files of the package are **kept** — `CREDITS.md` names them as the attribution for shipped feeds data |

## Archive

| Package | Files | What it decided | Why it went |
|---|---|---|---|
| `archive/` | 74 | The pre-2026-06 record: the BREP/STEP import plan and its two follow-ups, the remediation tracker, the 86-task sprint, the consolidation audit, the feature-gap report, the GUI wiring catalog, the vendor-LUT GUI plan, the phase1–5 UX plans and the 2026-08-05 mega-harness `.archived` sources | The operator ruling of 2026-09-16 retires archiving itself. Everything in it was already closed |

## Other packages

| Package | Files | What it decided | Why it went |
|---|---|---|---|
| `ux-fixes/` | 5 | Phased UX improvement plans 1–5: focus loss, quit protection, theme, multi-model, timeline, staleness, help, shortcuts and a 40-item polish pass | All shipped; superseded by three later UI programmes |

## Top-level documents — 127 files

| Family | Files | What it decided | Why it went |
|---|---|---|---|
| AgentSearch and adaptive investigations, 2026-04/05 | 15 | Diagnosed the AgentSearch entry, the corner clearing, the DPP islands, the link-distance divergence and the deep-channel gouge | All three `ClearingStrategy3d` variants are live, the gouge is fixed (`fa27b08`) and the boundary-walk entry landed 2026-04-15 |
| Optimizer and G16, 2026-05 | 8 | The optimizer reorg, its layered scoring, the explainability surface and the pre-optimize defaults audit | Shipped. `PROGRESS.md`'s "Current priorities" block, which still called two of them in flight, is corrected to history |
| RCA notes F-1..F-10 and P1..P5 | 8 | Root causes for the air-cut thresholds, the plunge stress gate, the drill metric suppression, the stale-defaults validator and the fresh-defaults policy | Each finding now has a named sentry under `crates/rs_cam_core/tests/` |
| UX dial-in, pain points and testing, 2026-05 | 7 | The first UX pain-point census, its fix plan, the IA audit workflow and the wanaka assessment | Roadmap A shipped; superseded by the IA audit and the 2026-09 review passes |
| Suggest / sim / optimize sweep prompts | 3 | Agent prompts and the sweep plan for the acceptance campaign | The acceptance record itself is kept for `CREDITS.md` |
| Spans and structure, 2026-05 | 7 | Structural and semantic spans, entry locality, the operation-transform audit and the dressup cleanup | Shipped across all 23 operation families and guarded by span-coverage tests |
| G-code and post, 2026-05/06 | 3 | The post-processor layer, its dialects and the export overhaul | Shipped and pinned by the `gcode_current_outputs` captures, which are kept and read by a test |
| Unification and refactor, 2026-04/06 | 10 | Unifying the two project loaders, removing `JobState`, the 2026-06-06 architectural refactor and its decision record | Completed 2026-06-08; the duplicate sweep of 2026-09-16 removed the last legacy loader and `JobState` outright |
| Feeds ingest working papers, 2026-05/06 | 19 | The coverage audit, the source acquisition, the phased ingest plan, the modal and workspace designs, the combined-suggest design, the Kc milling calibration and the cutter axial constraints | The bundled data's attribution is in `CREDITS.md`, which cites the eight consolidation and phase records that are **kept**. These are the drafts behind them |
| Finishing v3 campaign, 2026-07/08 | 17 | The unified-finishing planner, the v3 design, the campaign map, the workplan, the proof spec and the rest-cascade plan | CLOSED 2026-07-28 **not provable on this fixture** (coarse TIN). The surviving verdicts are in `SUPERSEDED_CONCLUSIONS.md`, `finishing_status_2026-09-01.md` and `finishing_synthesis_2026-08-30.md`, all kept |
| Pencil campaign, 2026-06/07 | 7 | Pencil's curvature, rest-depth and reference-fidelity detectors, and the post-mortem | Fixed at `29a6d61` — coverage 0.137 → 0.80. The watershed follow-up closed REJECT 2026-09-03 and keeps its package |
| `planning/README.md`'s 2026-03 index targets | 11 | The implementation plan, the performance review, the future plans, the workspace UX redesign, the simulation workspace vision, the workflow test plan, the voxel sim design, the two multi-setup documents, the alignment-pin design and the tool-library design | The index that called them active was itself stale. Judged on content: each is finished or abandoned. The index is rewritten in the same commit |
| Misc closed | 12 | Accel-friendly toolpaths, the strategy advisor, the heights setup-frame audit, the dexel Z-only roadmap, the two 2026-04/06 tech-debt audits, the datum and machine-run notes and the overlay audit | Each closed: conditioning is default-on for roughing, the advisor shipped, the frame bug was fixed 2026-06-12, the roadmap landed steps 0–5, the datum work landed at `e17b2ff0` and the overlay audit shipped at `92a62851` |

## Root-level deletions

| Path | Files | What it was | Why it went |
|---|---|---|---|
| `review/` | 106 | The independent BREP/STEP review (listed under Reviews above) | Findings landed; it duplicated the `planning/review_*` convention at the root |
| `G13_PROMPT.md` | 1 | An agent prompt for the G13 work package | No reference outside itself |
| `AGENT_PROMPT.md` | 1 | The wanaka regression agent prompt | Its only two references, `G13_PROMPT.md:77` and `planning/cutting-calcs-data-gaps.md:391`, are both in this purge |
| `fixtures/debug_adaptive/` generated outputs | 5 | `wanaka_adaptive.nc`, `wanaka_diag.json`, `wanaka_toolpath.svg`, `adaptive_terrain.svg`, `contour_parallel_terrain.svg` — 5 341 561 bytes | `job.toml` **writes** them and nothing reads them; `cases_agent_smoke.csv` holds no `debug_adaptive` row. `job.toml` and `traces/` stay |

**Kept at the root**, and why: `research/` and `architecture/` (`CREDITS.md`
cites nine and two of their files as algorithm-lineage attribution),
`toolpath_stress_test/` (`AI_MACHINIST_ANALYSIS_REFERENCE.md:350` names its
analyse script), and `AI_MACHINIST_ANALYSIS_REFERENCE.md` itself.
`tests/step_validation/` and its `Cargo.toml` exclude line are left for P3,
which can run cargo.

## Moves, not deletions

| Was | Is now |
|---|---|
| `planning/roughing_strategy_ab_results_2026-09-07.md` | `planning/roughing_strategy_ab_2026-09-07/RESULTS.md` |
| `planning/roughing_strategy_terrain_2026-09-07.md` | `planning/roughing_strategy_ab_2026-09-07/TERRAIN.md` |
| `planning/deep_doc_modulation_2026-09-08.md` | `planning/deep_doc_modulation_2026-09-08/STUDY.md` |
| `planning/pencil_linking_2026-09-04.md` | `planning/linking_2026-09-09/PENCIL_2026-09-04.md` |

## Citations that were deliberately left dangling

`crates/` and `scripts/` cite 238 distinct `planning/` paths across about 500
sites, almost all as the pre-registration of a sentry. The purge breaks **293
sites over 103 paths**. The operator ruled on 2026-09-17 that these are **not**
rewritten: two of the citers sit in a frozen file, and the tag is the retrieval
path. A dead `planning/…` path in a doc comment is expected, not a defect.
