# Tech-debt programme orchestration log

Basis: `TECH_DEBT_RESEARCH_AND_FIX_PLAN.md` @ HEAD `17861bb`, branch `experiment/adaptive-spiral`.
Orchestrator: Fable (session 2026-07-29). Implementation/research: Opus subagents.

## Standing constraints (enforced on every agent)

- **One cargo job at a time** across the whole machine (only 4.1 GB free at session
  start). Parallel researchers get "no cargo"; exactly one cargo-enabled
  implementer at any moment. `pgrep -f "bin/[c]argo"` before every launch.
- **Wanaka is read-only**: `planning/airrun_2026-06-01/wanaka.toml` is user-modified
  in the working tree — never commit it, never revert it, never save over it.
- No behavioral change without its checkpoint (A–E). This session targets only the
  pre-checkpoint merge order: PR-0, PR-1, PR-1a, PR-1b, PR-2, PR-3 + checkpoint
  evidence packs.
- Zero-warning clippy before every commit; targeted tests, never workspace-wide
  `cargo test`.
- Commit per plan slice; `git add` specific paths only.

## Session plan (maps to Task list #1–#11)

| Wave | Work | Agent | Cargo? | Status |
|---|---|---|---|---|
| 1 | #1 PR-1a A/M8 semantic annotation | impl-1 (opus) | YES (exclusive) | launched |
| 1 | #2 H1 research TOOL_SCALE_SEMANTICS.md | research-h1 (opus) | no | launched |
| 1 | #3 M1 research MEASUREMENT_DOMAINS.md | research-m1 (opus) | no | launched |
| 2 | #4 PR-1b A/M9 standing-material channel | impl (opus) | YES | after #1 |
| 2 | #5 PR-1 M2.1 e2e tapered sentry | impl (opus) | YES | after #4 |
| 3 | #6 PR-0 measurement contract impl | impl (opus) | YES | after #3,#5 |
| 3 | #7 PR-2 H1 additive semantic API | impl (opus) | YES | after #2, seq |
| 4 | #8 PR-3 H3 explicit resolution policy | impl (opus) | YES | after #7 |
| 5 | #9 Checkpoint A evidence (valley matrix) | impl (opus) | YES | after #7 |
| 5 | #10 Checkpoint B evidence (resolution A/B) | impl (opus) | YES | after #8 |
| 6 | #11 checkpoint package + log | orchestrator | — | last |

Checkpoints A/B are **human review stops** — this session assembles evidence and
stops; no H2 routing behavior change, no shared-helper resolution change.

## Event log

- 2026-07-29: session start. Tasks #1–#11 created. Wave 1 launched.
- research-h1 DONE → `TOOL_SCALE_SEMANTICS.md`. 76 production radius() sites
  (8 need semantic change, 5 doc-only, 63 correctly envelope-locked). ADR:
  additive named methods, NO newtypes; `height_at_radius` already IS the
  profile-clearance query — add no fourth name. H2.6 resolved: large-arc stays
  envelope-relative (arcfit cap parity), collapses to doc+sentry in PR-2.
  **NEW HIGH defect (task #12): tapered pencil `resolve_reference_cutter`
  compares vs SHANK dia → silently SelfReferenced (5th instrument defect,
  goes to H4 ledger, not fixed this session).** Also: ToolDefinition never
  explicitly delegates radius()/cusp_radius() (latent trap for PR-2);
  crease_paths.rs offset-pass count is dead code on tapered (n always 0).
- research-m1 DONE → `MEASUREMENT_DOMAINS.md`. 97 rows; 25 serialized fields
  (sim-trace schema v5 + MCP wire), 76 rename-free; NO area/volume in project
  TOML → PR-0 has zero project-format risk. 4 LIVE HAZARDS (task #13): air-cut%
  two denominators under one name (GUI vs MCP vs docs); part_area_fraction
  bbox-rect denominator; 313/482 still constructible in wanaka decompose
  harness; v3_cascade_ab cell-count-vs-mm² line + overlap double-count in
  build_band_map (H4's territory oracle). PR-0 slice 1 = additive
  MeasurementProvenance; slice 2 = ProjectedXyAreaMm2/SurfaceAreaMm2 newtypes
  with no Div → 313/482 becomes a compile error. **A/M7's mm²/s figure
  (0.476 vs 0.938) is VOID: numerator is centreline 1mm bins, ignores tool
  radius and stock — carry to H4 ledger.**
- impl-1 DONE → **PR-1a COMMITTED `93b43e8`** (A/M8). One shared region_table
  projects BOTH structural spans and semantic items (no silent divergence);
  narration gains `Region mix:` line; 4-test sentry (red-first evidence
  recorded); clippy zero; viz 227/227; 3 --lib failures are the pre-existing
  adaptive3d known reds (stash-verified). Adjacent defects logged, not fixed:
  SpanKind::Region overloads node vs ring spans (label-string parsing only
  discriminator); raster/waterline nodes emit no sub-region events (H4
  attribution region-granular for 2 of 3 bands); Trace/Scallop/SpiralFinish
  still report `regions 0` (same structural class); repo not cargo-fmt clean
  at HEAD (~50 pre-existing sites — left alone).
- impl-2 DONE → **PR-1b COMMITTED `52fa78a`** (A/M9). standing_material_mm2 now
  Option<f64> (None = not measured — X-19 silent-zero trap closed); narration
  line + GUI ribbon + MCP diagnostic (serialises null, never 0.0); report-only.
  Domain/stage/resolution as consts in compute/config.rs. Red-first with
  calibrated corrugated-plate truncation fixture (13.33 mm² standing). Gates:
  4/4 sentry, A/M8 4/4 undisturbed, --lib 2161 pass (+3 known reds), viz
  227/227, clippy clean. **CRITICAL side-find FIXED in same commit: .gitignore
  unanchored `diagnostics/` was eating crates/rs_cam_core/src/diagnostics/
  — from_generation.rs was never committed; HEAD didn't compile from a clean
  clone. Lesson: rg respects .gitignore; use grep -r when a file may be
  ignored.** Adjacent (not fixed): uncut_core_mm2 hole-blind upper bound
  (M4's job); CLI has a parallel ToolpathDiagnostic without the new field;
  tapered truncation fixture impossible at shank-derived grid resolution
  (cusp-stepover vs shank-cell asymmetry — H3 evidence).
- Research docs + log committed `ee677f5`.
- impl-3 DONE → **PR-1 COMMITTED `cd0ab06`** (M2.1). 7 tests / 1.46 s:
  end-to-end tapered + Ball control through ProjectSession::generate_toolpath;
  band↔strategy pairing pinned; tip-scale dials pinned end-to-end (non-shallow
  region 48.5/63.5 mm² < the 144 mm² shank floor); historical red RECONSTRUCTED
  in-test (pre-fix dial pair → 0 non-shallow regions, live → 2, non-vacuity
  guarded). Adjacent defects: **(task #14) semantic trace move links never
  remapped by dressups/clip/descent transforms — can point past end of moves,
  consumer panic class; A/M8 equality only holds pre-transform**; **(task #15)
  UnifiedFinish Auto heights silently drop the VerySteep band entirely
  (unmachined feature, no diagnostic)**; orphaned rapids at node boundaries
  excluded from per-region totals.
- impl-4 DONE → **PR-0 COMMITTED `4f5745f`** (M1 slices 1-2). New
  measurement.rs: MeasurementProvenance + domain/stage/cell-source enums;
  provenance on DecomposeStats, UnifiedFinishReport, RestFieldReport,
  ScallopReport (const), ToolpathStats accessor, SimulationResult
  column_grid_cell_mm, FinishSurface::cell_source. Area newtypes
  ProjectedXyAreaMm2/SurfaceAreaMm2 with NO cross-Div — 313/482 is now a
  COMPILE ERROR (compile_fail doctest verified load-bearing). A/M9 strings
  const-derived from provenance, byte-identical (sentry passes with zero
  edits). comparable_to() rejects same-domain/different-stage (X-12). Gates
  all green; workspace check re-verified by orchestrator. NOTE: **disk at 97%
  (32 GB free), one agent build hit ENOSPC mid-run** — watch this. LH-1
  (air-cut dual denominator) deferred to slice 3 (task #13).
- impl-5 DONE → **PR-2 COMMITTED `606b8d5`** (H1 additive API). Named accessors
  envelope_radius_mm/cusp_radius_mm/engagement_radius_mm; ToolDefinition
  delegation trap FIXED (red-verified via DirectlyOverridingCutter fixture);
  H2.6 closed (narrate↔arcfit parity sentry, red-verified); property tests
  over all 5 shapes; centerline_cut_paths scalar DELETED (inertness
  independently reproduced). Deviations (both sound): alias direction
  inverted vs doc row 1; finish_setup.rs:95 generation cell left as radius()
  — it is H3's B1 decision, not a rename. Research-doc errata: §4.5 inverse
  overstated (flat/bull many-to-one at height 0 — property is >=, not ==);
  §7.2 self-contradicting sentence (conclusion right); line numbers stale
  (re-locate by symbol). Sentry finish_surface_cell_source_names_the_radius…
  is now the TRIPWIRE for any cell-size move — PR-3 must update it in the
  same commit if a size changes (it must not).
- impl-6 DONE → **PR-3 COMMITTED `ce0365d`** (H3 steps 1-2).
  FinishResolutionPolicy {LegacyEnvelopeQuarter, CuspQuarter, Explicit};
  policy builders are the only grid builders (legacy entry points = thin
  adapters); per-consumer selector fns; UnifiedFinish MidSteep delegates to
  scallop's selector BY IDENTITY (H3 step 5 collapses to "understand
  scallop"). Cell sizes proven unchanged: PR-2 tripwire zero edits +
  pre-refactor FNV toolpath fingerprints (scallop/ramp/steep, tapered) +
  ball-equality gate. Adjacent: tolerance-floor makes comparable_to falsely
  refuse (needs ToleranceFloor CellSource variant, behavior change deferred);
  SteepShallow classifies on its own envelope-scaled generation grid (H3
  bites hardest there); scallop stepover cusp-scaled vs grid envelope-scaled
  = the H3 question stated. Checkpoint-B harness must drive POLICY arms (not
  cell-size entry point, which erases mode provenance).
- ALL SIX PRE-CHECKPOINT MERGE-ORDER PRS LANDED: PR-0 4f5745f, PR-1 cd0ab06,
  PR-1a 93b43e8, PR-1b 52fa78a, PR-2 606b8d5, PR-3 ce0365d.
- impl-7 DONE → **task #14 COMMITTED `ae10cb2`** (semantic-trace remap fix).
  Transforms = the 3 named + a 4th site (viz worker inline clip). Mechanism:
  SemanticLinkCarrier rides the SAME provenance maps as spans → agreement
  holds by construction post-transform. Deleted-move policy = UNLINK (never
  clamp — clamping fabricates ranges). M2.1 lenient gate TIGHTENED to full
  range equality (9/9 now). FNV fingerprints unchanged (no geometry change);
  viz 227/227; clippy clean; workspace check re-verified by orchestrator.
  Adjacent: tsp remap_spans O(spans×moves) wants interval index; semantic
  xy_bbox/z coords never re-derived post-clip (indices fixed, coords stale);
  per-dressup items claim whole path (attribution not meaningful);
  expand_span_kind_synonyms silently under-covers new variants.
- impl-8 DONE → **CHECKPOINT A EVIDENCE COMMITTED `90d1292`** (matrix 12 tests
  4.5s + CHECKPOINT_A_EVIDENCE.md, numbers from real run, truth
  cross-validated vs closed-form to 0.25 µm). Headline (gouge/miss/coverage,
  176 cells/tool): ENV 1/82/2% (taper), CUSP 13/0/100%, ENG 9/0/100%,
  CLR 9/0/98%, **CLR+local-wall-angle 0/0/100% on all three tools** — the
  winning hybrid. ENV emits 0 offset passes on 107 sub-shank cells (48
  physically support one). KEY COUPLING: route_width_factor×r and
  (half_width−r)/stepover are the same number — swapping radius alone flips
  64/176 cells pencil→clearing (coverage 100%→0% as routed); H2.1 must
  re-decide routing, not substitute a scalar. Ball DOES move under the
  winning model (needs the plan's "separately justified correction" call).
  Open questions §9: depth statistic (per-sample local, not per-branch);
  route_width_factor retire-or-rescale (serialised user dial); wall-angle
  plumbing vs fallback; asymmetric fan; gouge-vs-time gate framing. NEW
  defect: tip float has no diagnostic channel (silent unreachable
  centreline, up to 5.2 mm residual). NO behavior changed — awaiting human
  Checkpoint A.
- impl-9 DONE → **CHECKPOINT B EVIDENCE COMMITTED `4bc8f92`** (harness 5 fast
  tests 24s + ignored full grid 354s debug + CHECKPOINT_B_EVIDENCE.md, real
  numbers). Headline cusp/4 vs envelope/4 (tapered): Scallop 3.5× time,
  dial-accurate cusp BUT +19–33 mm² standing material (flat-ground max_rings
  blows on fine grids → scallop resolution is GATED BEHIND CHECKPOINT C);
  RampFinish 3.7× time, eliminates 2.39 mm + 0.16 mm gouges — win already
  lands at the 0.306 mm INTERMEDIATE cell for 1.3–2.6×; SteepShallow ZERO
  delta (bit-identical residuals, fingerprints converge above ~0.3 mm).
  VerySteep class DOES NOT EXIST at envelope/4 on narrow fixtures (0 regions
  vs truth 8/4); labels 10.6–15.9% wrong at envelope/4. §A.0 reproduced:
  coarse grid OVERSTATES area 34–93% while collapsing topology. Recommendation
  (human decides): per-consumer policy; move RampFinish to intermediate;
  hold Scallop behind Checkpoint C; hold SteepShallow pending discriminating
  fixture; global cusp grid NOT recommended. Wanaka deferred to checkpoint
  session. NEW defects: RampFinish gouges 4.2 mm into a 62° cone at EVERY
  resolution with no diagnostic; steep_shallow emits 0.9 µm segments at every
  resolution.

## FINAL SESSION STATUS (2026-07-29)

Nine commits, all gates green at every step, zero behavioral changes to
production toolpaths:

| Commit | Item |
|---|---|
| `93b43e8` | PR-1a A/M8 semantic annotation |
| `52fa78a` | PR-1b A/M9 standing-material channel (+ .gitignore source-eating fix) |
| `ee677f5` | H1 + M1 research oracles |
| `cd0ab06` | PR-1 M2.1 end-to-end tapered sentry |
| `4f5745f` | PR-0 MeasurementProvenance + area newtypes (313/482 = compile error) |
| `606b8d5` | PR-2 named tool-scale accessors + delegation parity |
| `ce0365d` | PR-3 explicit FinishResolutionPolicy per consumer |
| `ae10cb2` | Semantic-trace remap through post-generation transforms |
| `90d1292` | Checkpoint A evidence (valley matrix) |
| `4bc8f92` | Checkpoint B evidence (resolution A/B) |

**STOPPED at the plan's human checkpoints.** Decisions now needed:
- **Checkpoint A** (CHECKPOINT_A_EVIDENCE.md): reach model = profile-clearance
  + local wall angle (0/0/100%); routing criterion must be re-decided WITH the
  radius (route_width_factor coupling); ball behavior moves; §9 open questions.
- **Checkpoint B** (CHECKPOINT_B_EVIDENCE.md): per-consumer policy; RampFinish
  → intermediate cell; Scallop gated behind Checkpoint C (max_rings/M4);
  SteepShallow needs a discriminating fixture; §open questions.

## CHECKPOINT DECISIONS (2026-07-29, user-approved)

**Checkpoint A: APPROVED. Checkpoint B: APPROVED.** User rulings:
- **Ball tools MIGRATE** to the winning reach model (the matrix is the plan's
  "separately justified correction"; 0/0/100% vs 11-gouge/35-miss/74%).
- Architecture defaults locked in by delegation ("best architecture wins"):
  per-sample local depth in paths_from_sampled; retire route_width_factor for
  the coverage criterion X_reach ≤ cap × stepover (old dial deserialized with
  deprecation finding); wall angle = finite difference off RestGrid::surface_z
  (CLR+θ model); per-side reach + asymmetric fan; gate on GOUGE, report
  wasted-time; max_rings budget derived from SELECTED stepover as an H3-scoped
  EXPERIMENT first (adopt only if standing→0 with no over-cut regression;
  remember v3: naive cap raise was 34× worse); RampFinish gets a NAMED
  intermediate policy variant (geo-mean), not op-owned Explicit; scallop
  acceptance gate = achieved-cusp-vs-dial; SteepShallow deferred WITH written
  reason + discriminating-fixture task; global cusp grid retired; wanaka
  characterisation per behavioral PR, after direction.
- **Wave D first** (safe report-only/plumbing defect fixes), then behavioral
  waves — instruments before behavior, the programme's own lesson.

Wave D roster: D1 loud findings (Auto-heights VerySteep drop + tip-float
channel); D2 = task #13 LH-1..LH-4 (air-cut% published under BOTH honest names
with no numeric behavior change; part_area_fraction footprint denominator;
harness newtyping); D3 plumbing (SpanKind node/ring discriminator, stale
semantic bbox post-clip, CLI ToolpathDiagnostic field, ScallopReport ring
count, ToleranceFloor CellSource variant).

Behavioral waves after D: H2 routing PR-4..7 (incl. #12 SelfReferenced + ball
migration + cone-gouge reach clamp), RampFinish intermediate mode, max_rings
experiment, SteepShallow fixture investigation, 0.9 µm segment gate.

- impl-10 DONE → **Wave D1 COMMITTED `cbe8503`**. Dropped-band finding (48.5mm²
  85° groove now loud: was 706.8mm of cutting silently gone with default
  heights) + tip-float channel (closed-form-verified 2.3748mm float on Ø3
  ball/1.2mm groove; hooks in paths_from_sampled with 5 preservation notes for
  H2's rewrite — floor = SECOND drop with Ø0.1 probe ball, centreline only,
  stock_to_leave backed out, NaN = examined-not-floating, threshold ==
  reach_gap_threshold MOVE TOGETHER). FNV fingerprints unchanged; all gates
  green. Fixed in passing: from_generation adapter early-return would have
  silently gated all future findings on the ring cascade. NEW ledger items:
  partial height clipping unreported (only total collapse); ComputeMessage
  enum at its 280-byte clippy ceiling — next ToolpathStats field needs
  ComputeMessage::Toolpath boxed (~16 sites); top_z-only clip attribution
  untested.

- impl-11 DONE -> **Wave D2 COMMITTED `f525fe9`** (task #13, LH-1..LH-4).

  **LH-1 air-cut denominator — NO numeric behaviour change.** New
  `simulation_cut::AirCutRatios` trait (impl'd for the 4 summary types +
  `SummaryAccumulator`) publishes `air_cut_pct_of_total_runtime()` and
  `air_cut_pct_of_cutting_time()`; every surface now calls one BY NAME and
  says which in its label. **LEDGER NOTE — threshold semantics: every shipped
  air-cut threshold follows the TOTAL-RUNTIME measure** (GUI banner 20%, CLI
  verdict 40%, and every `OperationType::air_cut_high_threshold_pct` band).
  Nothing was retuned; the cutting-time reading (what `narrate_toolpath`
  reports, and what CLAUDE.md's metric-caveats block describes) now ships
  BESIDE it everywhere rather than under the same word. **CLAUDE.md still
  says air-cut is "calibrated against cutting-time, not wall-clock" — true
  only of the narration path; correcting that line is deliberately left to
  the L1 docs sweep / plan slice 7** so this wave stayed code-only.
  Serialization: `ProjectDiagnostics` keeps `air_cut_percentage` (same
  total-runtime value) and gains two named keys -> `serialize_struct` arity
  8 -> 10; the CLI `ProjectSummary` JSON likewise keeps its legacy key and
  gains both named ones; `OptimizeCandidate::air_cut_pct` was renamed in code
  to `air_cut_fraction_of_total_runtime` with `#[serde(rename =
  "air_cut_pct")]` so persisted optimizer results still load. Its doc-comment
  claimed "fraction of cutting time" while dividing by total runtime — an
  unlisted fifth instance of LH-1, corrected (doc, not value).

  **LH-2 `part_area_fraction` -> `part_footprint_fraction`; denominator is
  now the covered footprint.** RED-FIRST EVIDENCE (probe run at the old
  denominator, then deleted): a diagonal part on a 40x40 @ 0.5 mm rest grid
  covers 205.0 mm2 of its 400.0 mm2 bbox (51%); a single 110 mm2 rest region
  is 53.7% of the real footprint but only 27.5% of the bbox, so the >=0.5
  warning stayed SILENT — `expected the giant-region warning to fire, got
  None`. Fix: `RestGrid::covered_cell_count` / `covered_footprint_area_mm2`
  (cells with finite `surface_z` x cell2) + `footprint_provenance()`; the GUI
  now passes each toolpath's OWN rest-grid footprint (and the boundary-source
  picker carries the SOURCE toolpath's, added as a 5th tuple field), and the
  mesh-XY-bbox computation is deleted. No grid -> 0.0 -> silence, never a
  guess. `RestRegionPathology` is not serde, so no alias was needed.
  **This CHANGES when the warning fires — it now fires on parts it used to
  miss, which is the point.** Sentries:
  `giant_region_denominator_is_the_covered_footprint_not_the_bbox` (keeps the
  bbox reading as recorded evidence) + `covered_footprint_ignores_uncovered_cells`.

  **LH-3** was already newtyped by PR-0; swept the rest of the harness and
  its sibling — every remaining bare `mm^2` / `%` column in
  `finish_planner_wanaka_decompose.rs` now carries its domain in the header
  or the row (`XY-proj`, `3D-surf`, `cells`), including the slope-distribution
  table that prints a 3D-area% and a cell% side by side.
  `tapered_cusp_radius_sentry.rs` was already clean.

  **LH-4** `v3_cascade_ab.rs` `BAND COVERAGE` no longer prints a cell count
  and overlap-dilated mm2 on one line: covered cells (with their mm2
  equivalent) and per-band extracted areas are separate lines, each carrying
  `MeasurementProvenance::describe()`, and the only ratio left is cells/cells
  on one grid. The **"~17% of covered area reclaimed" docstring is RETRACTED**
  in place (void, not falsified — wrong domain AND a numerator that
  double-counts the 2 mm dilation ring). `build_band_map` gained a doc block
  stating the overlap double-count and the established rule that verdicts
  must attribute by `Region` spans, not by a dilated band map. Extraction
  redesign deliberately NOT attempted (H4).

  Gates: new `tests/air_cut_denominators_lh1.rs` 4/4 (incl. a grep-style
  sentry over the 5 viz/CLI surfaces — retarget it, never delete it, if a
  file moves); standing_material_channel_am9 4/4; unified_finish_tapered_
  end_to_end_m21 9/9; finish_resolution_policy_pr3 8/8 (fingerprints
  unchanged); dropped_band_finding_d1 3/3; pencil_tip_float_channel_d1 3/3;
  `-p rs_cam_core --lib` 2171 passed / exactly the 3 known adaptive3d reds;
  `-p rs_cam_viz` 227/227; `-p rs_cam_cli` 14/14; clippy workspace clean.
  ADJACENT DEFECTS SEEN, NOT FIXED: `v3_cascade_ab` `OP B STRATEGY MIX`
  `%area` shares still sum past 100% on overlapping centreline footprints
  (X-15) and `EFFICIENCY WITH BOUNDS` still divides a tool-CENTRELINE
  footprint by time while claiming tool invariance (X-14) — both are A/M7
  scope; `SimulationSemanticCutSummary`/`SimulationCutHotspot` carry
  `wasted_runtime_s` with no denominator of their own.

- impl-12 DONE -> **Wave D3 COMMITTED `68f65b3`** (five diagnostics-plumbing
  fixes, NO toolpath geometry change — FNV-pinned suites byte-identical).

  **D3.1 SpanKind node-vs-ring discriminator.** New
  `toolpath_spans::RegionSpanRole { Node, GeneratorPass }`, carried as a
  FIELD on `SpanPayload::Region` (not a new `SpanKind`) so every existing
  pattern was a compile error until audited — 12 match sites in core + viz,
  6 construction sites. `Node` = planner territory (UnifiedFinish
  `region_table`, the partition); `GeneratorPass` = one ring / drill hole /
  peck / adaptive region / cutting run (nested, NOT a partition, own id
  space). Read it via `Span::region_role()` / `is_region_node()`.
  **A/M8's and M2.1's label-string filters (`label.ends_with(" band")`) are
  GONE** — the doc comment now records why: a free-text label was
  load-bearing, and a producer renaming a band would have emptied the node
  list and made the reconcile assertion pass VACUOUSLY. Also consolidated
  the span-kind string vocabulary into core (`SpanKind::as_key` exhaustive +
  `ALL` + `from_key`); viz's `parse_span_kind_filter` now delegates and
  `expand_span_kind_synonyms` parses to the enum and matches EXHAUSTIVELY
  (it had a silent `other => literal` fallback, so `waterline_cleanup` and
  any future variant fell through un-noticed). MCP `inspect_spans` gains a
  `region_role` key (additive) so agents never parse the label either.

  **D3.2 stale semantic geometry after clip — the coords now follow the
  indices.** `ae10cb2` fixed the INDICES and explicitly left `xy_bbox` /
  `z_min` / `z_max` at generation-time values, so a surviving item could
  describe moves that no longer exist. Same policy as the index fix, stated
  the same way: still linked -> RE-DERIVE from the surviving moves;
  unlinked -> `None` all three (keeping them is the fabrication UNLINK
  exists to prevent); never had geometry -> untouched (an item acquires no
  claim it did not make). `bind_to_toolpath` and the re-derivation now share
  ONE deriver (`range_geometry`), so a re-derived bbox is the same function
  of the same moves and an untouched item is byte-identical.
  `remap_move_links` now takes the post-transform `&Toolpath` instead of a
  bare move count — the geometry cannot be re-derived from a number, and
  taking the toolpath is what makes forgetting impossible at the 5 call
  sites. `SemanticLinkCarrier::detach` re-derives from `annotated.toolpath`.
  **RED-FIRST EVIDENCE** (probe run with the re-derivation stubbed out, then
  reverted): 6-move staircase (move i at x=i, z=-(i+1)), clip keeps 0..3 —
  `bbox must describe the SURVIVING moves (x max 2.0), got 5`; z_min read
  -6.0 for a path whose deepest surviving move is -3.0.

  **D3.3 CLI ToolpathDiagnostic parity.** Kept as its own struct (it carries
  the full debug + semantic traces and `min_safe_stickout`, which the core
  summary does not, and its key names are what scripts read) but the five
  fields both have are now COPIED OFF the core diagnostic — `op_kind`,
  `standing_material_mm2`, `unmachined_band_area_mm2`, `tip_float_points`,
  `max_tip_float_mm`. One derivation, so a CLI batch and the same session's
  GUI cannot report different numbers. Purely additive: no key renamed or
  removed.

  **D3.4 ScallopReport ring count.** `ring_count` (rings/runs EMITTED — one
  per `ScallopRuntimeAnnotation`) plus `cascade_ring_count` (rings the
  offset cascade PRODUCED, pre keep-predicate). The gap between them is the
  signal: "cascade stopped early" and "rings generated then filtered away"
  are different stories and used to be indistinguishable. Checkpoint-B
  harness migrated off `anns.len()` and now asserts the two agree.

  **D3.5 `CellSource::ToleranceFloor`.** `FinishResolutionPolicy::
  cell_source()` returns it when `tolerance_floor_applied` — when `.max
  (tolerance)` binds, the formula family had no say in the number, so the
  tag must not claim a radius. Two floor-bound policies of DIFFERENT modes
  now compare (`comparable_to` needed no special case: plain source
  equality does it once both tag `ToleranceFloor`), while a floor-bound cell
  still does not compare with a tool-scaled cell of the same size.
  `FinishResolutionMode::cell_source()` keeps the family mapping.
  **The PR-2 tripwire (`finish_surface_cell_source_names_the_radius…`) did
  NOT need updating and was not touched: 8/8 unchanged.** Its tapered
  fixture uses tolerance 0.01 against envelope/4 = 0.75 and cusp/4 = 0.125,
  so the floor does not bind there — verified, not assumed. PR-3's
  `assert_policy` helper was made honest (expects `ToleranceFloor` when
  `floor`), all four production consumers still pass `floor = false`.
  `p2c_headless_ab_wanaka.rs` compile-checked via `clippy --all-targets`,
  not run, as instructed.

  Gates: semantic_trace unit 10/10 (2 new); toolpath_spans unit 45/45 (1
  new); unified_finish_semantic_regions 4/4; unified_finish_tapered_
  end_to_end_m21 9/9; finish_resolution_policy_pr3 8/8; tool_scale_
  semantics_pr2 8/8 (tripwire zero edits); standing_material_channel_am9
  4/4; air_cut_denominators_lh1 8/8; dropped_band_finding_d1 3/3;
  pencil_tip_float_channel_d1 3/3; boundary_clip_invalidates_spans 3/3;
  dressup_span_invariants 3/3; checkpoint_b_resolution_ab 5/5 (+1 ignored);
  `--lib scallop::` 17/17; `-p rs_cam_core --lib` 2174 passed / exactly the
  3 known adaptive3d reds; `-p rs_cam_viz` 227/227; `-p rs_cam_cli` 14/14;
  `-p rs_cam_mcp` 4/4; clippy workspace `--all-targets -D warnings` clean.
  `ComputeMessage` was NOT touched (no `ToolpathStats` field added), so its
  280-byte ceiling is untested by this wave and still one field from
  needing `ComputeMessage::Toolpath` boxed.
  ADJACENT DEFECTS SEEN, NOT FIXED: `p2c_headless_ab_wanaka.rs:3754` and
  `v3_cascade_ab.rs:4327` still key off the `"Pencil claims"` LABEL — same
  class D3.1 just closed, left alone to keep the blast radius honest;
  `unified_finish` does NOT aggregate the new scallop ring counts into
  `UnifiedFinishReport` (per-band ring totals are still uncounted);
  `RegionSpanRole` has no third role for the drill hole-vs-peck nesting,
  which is still label-distinguished (`"Hole N"` vs `"Hole N plunge M"`);
  `semantic_trace` re-derivation deliberately does NOT give geometry to
  items that never had any (e.g. the whole-path dressup items), so those
  remain geometry-blind by choice, not by accident.

Remaining programme work (post-checkpoint): H2 routing slices (PR-4..7),
A/M6 claims_reference (PR-3a), PR-8..9 resolution consumers, M3 classifier,
M4 scallop (Checkpoint C), M5 offset_polygon (Checkpoint D), A/M7 retract
trips + A/M10 descent/resolution, H4 re-measurement ledger (Checkpoint E),
L1 docs sweep. Deferred defect tasks: #12 tapered-pencil SelfReferenced,
#15 Auto-heights VerySteep drop (+ new:
RampFinish cone gouge, tip-float silent residual, 0.9 µm segments).

- impl-13 DONE -> **H2 ROUTING WAVE A COMMITTED: PR-4 `df41169`, PR-5 `5a2c39a`**
  (the programme's FIRST behavioural slice, under the user-approved
  Checkpoint A).

  **New module `rs_cam_core::reach` — ONE canonical policy.** CLR+θ as
  approved, per side: `X = rim_distance - G(delta, cot θ)` with
  `G = max over u in [0,delta] of [width_at_height(u) + (delta-u)*cot θ]`,
  and two-wall fouling (`X_left + X_right < 0`) as the refusal. Two
  structural properties, not conveniences: a VERTICAL wall (`cot θ = 0`)
  reduces `G` EXACTLY to `engagement_radius_mm(delta)`, so the "no wall
  angle" fallback is the same function called with a zero rather than a
  second model; and the `height_at_radius(w) == None` reading the oracle
  proposed is not used at all (it cost the ball control 95 points of fit
  coverage, §10). `G` is scanned and the one-sided sampling error
  `cot θ * Δ` is ADDED, so reach is never overstated and refusal never
  under-fires — conservative in the direction the plan's bar asks for.
  API: `solve_reach` / `route` / `offset_passes_per_side` /
  `coverage_cap_passes` over `LocalValley { rest_depth_mm, left, right }`
  and `ValleySide { rim_distance_mm, wall_rise_mm }`.

  **Coverage cap derivation.** `cap` in `X_reach <= cap x offset_stepover`
  is the op's own `num_offset_passes` (the matrix's ground truth calls a
  cell clearing iff `X_true > cap*s` with `cap = num_offset_passes`, so
  anything else would reproduce a different rule than the evidence scored),
  floored UPWARD by `ceil(engagement_radius(delta).max(cusp_radius)/stepover)`
  and by 1. The floor is load-bearing: the shipped `num_offset_passes`
  default is **ZERO**, at which `cap x stepover` is literally 0 and EVERY
  branch routes to clearing — the pencil op would stop cutting. Asserted
  never to bind on the matrix (`coverage_cap_floor_never_binds_on_the_matrix`:
  2/2/3 passes vs the matrix's 4), so the approved routing column is
  reproduced, not approximated.

  **THE GATE, on production code.** `production_reach_policy_reproduces_the_
  approved_matrix_column` drives `rs_cam_core::reach` — not a
  reimplementation — against the same erosion-sampler truth that produced
  `CHECKPOINT_A_EVIDENCE.md`, and lands the approved column exactly,
  fan-cell counts included:

  | tool | PROD | ENV (baseline, same run) |
  |---|---|---|
  | taper Ø1/7°/Ø6 | fan=89 gouge 0 / miss 0 / route_over 0 / route_under 0 / float_blind 0 / **coverage 100%** | 1 / 82 / 63 / 0 / 24 / **2%** |
  | taper Ø1/15°/Ø6 | fan=88, same zeros, 100% | 1 / 81 / 63 / 0 / 25 / 2% |
  | ball Ø3 | fan=71, same zeros, 100% | 11 / 35 / 15 / 1 / 56 / 74% |

  89 / 88 / 71 are the CLR+θ fan-cell counts §2/§3/§4 record. Ball migrates
  too, per the user ruling.

  **ROUTING-COUNT DELTAS (tapered Ø1/7°/Ø6, 5 mm-wide 70° groove 1.2 mm
  deep, real pencil path).** Under the retired envelope equation every row
  read `offset_total = 1` — `half_width - 3.0` is negative for every valley
  narrower than the Ø6 shank, so the width-aware fan was DEAD CODE:

  | num_offset_passes | chains | max offset_total | offsets emitted (mm) |
  |---:|---:|---:|---|
  | 0 | 2 | 1 | 0.0 |
  | 1 | 2 | 2 | -0.5, 0.0, 0.5 |
  | 2 | 2 | 3 | -1.0 .. 1.0 |
  | 4 | 2 | 5 | -2.0 .. 2.0 |

  Checkpoint A probe 1 — the plan's verbatim headline, *"offset passes
  emitted = 0 under envelope on a 3 mm valley the tip could ladder"* — now
  reads `offset_total = 3` on the same fixture through the same call. BALL
  characterisation (`ball_characterisation_what_the_new_model_changes`,
  Ø3, 6 fixtures): 3 of 6 fan counts changed, and a 3 mm-half-width 60° V at
  4.39 mm rest depth is now REFUSED instead of getting a 3-pass fan (the
  ball wedges and floats there). Nothing asserts the ball did not move —
  the approved delta is recorded row by row.

  **Task #12 (tapered `SelfReferenced`) FIXED.** `resolve_reference_cutter`
  takes `&dyn MillingCutter` and compares against `cusp_radius_mm() * 2` —
  the scale the pencil CUTS with — not `diameter()`, which for a tapered
  ball deliberately reports the SHANK. The old comparison was `6.0 > 6.0`
  at the shipped default, so every tapered pencil op silently fell to the
  self-referenced probe. Tip diameter IS `diameter()` for every non-tapered
  shape, so only the tapered path moves. Probe 3 now asserts the FIX
  (default 6.0 -> 2 chains; a genuinely self-referenced 0.5 -> 1); field
  level, 46.21 mm3 vs 13.57 mm3 of measured rest on one fixture.

  **PER-SAMPLE / PER-SIDE machinery.** `RestCenterline::samples` carries the
  measured cross-section and its resolved reach ONE PER POINT (ruling: a
  per-branch scalar is optimistic as a median and detail-suppressing as a
  peak, §8.5). Serde AUDITED: `RestCenterline` has no derive, reaches no
  project file or wire format, is rebuilt every generate — noted on the
  field. `measure_cross_section` is the approved finite difference off
  `RestGrid::surface_z`: walk the ridge perpendicular to the rim (where rest
  falls back to the mask threshold) and take the STEEPEST single-cell
  gradient inside the depth band. `paths_from_sampled` takes an `OffsetFan
  { left, right, reach }`: per-side counts (the matrix measured left/right
  reach differing 22x, §6) and per-point truncation via the existing
  non-contact marker, so `contact_runs` splits a pass into the runs that are
  real. Observed on a groove tapering 5x along its length: 12 offset passes
  emitted, 2 truncated away entirely — a branch scalar can produce neither
  that nor a split.

  **Deprecation, not deletion.** `route_width_factor` still deserializes,
  still saves, and is no longer read; a project carrying a NON-DEFAULT value
  raises `DeprecatedDialFinding` -> `ids::CONFIG_DEPRECATED_DIAL` on both
  the session and GUI-worker stats paths, and the GUI widget is relabelled
  "(retired)". Default value -> silence. `RestFieldParams` swapped
  `route_width_factor` + `pencil_radius`(->`routing_radius_mm` in PR-4) for
  `offset_stepover_mm` + `num_offset_passes_cap`, so the detector routes
  against the fan the caller actually emits.

  **Rule 4 held.** Grid padding, trust erosion and polygon reach-back still
  read the cutter envelope; `routing_dials_do_not_move_the_envelope_derived_
  geometry` runs the detector at cap 0 and cap 64 and asserts identical grid
  extent and identical region polygons.

  **UnifiedFinish inherited the fit AUTOMATICALLY** — its crease node calls
  `centerline_cut_paths`, so the per-side reach fan applies with no edit to
  `unified_finish.rs`'s emission. Its derived stepover
  (`cutter.envelope_radius_mm() * 0.5`) is H2.3 / wave B and was NOT
  touched; its claims-pipeline `RestFieldParams` now names that same
  stepover and the design-doc 4-pass cap so routing and emission agree.

  **D1 TIP-FLOAT MOVED, with justification.** Gate 1's fixture (Ø3 ball,
  1.2 mm-wide 2.5 mm-deep groove) is exactly the float-blind case the new
  routing REFUSES, so the rest-depth arm now emits 0 mm of cutting there and
  the instrument has nothing to measure. Split rather than weakened:
  **Gate 1a** pins the new behaviour (0 cutting; float channel stays
  `None`/zero-points, never a fabricated zero) and **Gate 1b** re-targets the
  original measurement to the DIHEDRAL arm, which has no routing at all, on
  the SAME geometry — 98/98 points floating, worst 2.196 mm against the
  2.375 mm closed form (the dihedral trace sits on the crease at x=+-0.159
  rather than the trough centre, inside the test's own 0.9x band). Gate 1b
  also needed `bisector_strength: 0.0`: the default 1.0 walks a Ø3 ball's
  trace ~1.5 mm sideways, clean out of a 1.2 mm groove and onto the flat
  top, where the float reads a true and useless zero. `TIP_FLOAT_THRESHOLD_MM
  == reach_gap_threshold()` was NOT touched — the reach model does not change
  what "floating" means, only who gets driven there. 4/4.

  Gates (both commits): reach_policy_pr4 6/6; coverage_routing_pr5 6/6;
  checkpoint_a_valley_matrix 14/14 (12 pre-existing undisturbed);
  unified_finish_tapered_end_to_end_m21 9/9; unified_finish_semantic_regions
  4/4; standing_material_channel_am9 4/4; pencil_tip_float_channel_d1 4/4
  (3 -> 4, split above); unified_finish_dropped_band_finding_d1 3/3;
  finish_resolution_policy_pr3 8/8 FNV fingerprints UNCHANGED;
  tool_scale_semantics_pr2 8/8 (tripwire zero edits); air_cut_denominators_lh1
  4/4; capability_link_moves_safety 17/17; param_sweep 56 ignored (unchanged);
  `-p rs_cam_core --lib` 2180 passed / exactly the 3 known adaptive3d reds;
  viz 227/227; cli 14/14; mcp 4/4; clippy --workspace --all-targets
  -D warnings exit 0.

  **WANAKA: DEFERRED TO LIVE VALIDATION, stated honestly.** The only harness
  that reaches rest routing is `p2c_headless_ab_wanaka.rs`, a set of
  `#[ignore]`d full-project dexel ladders needing release-mode minutes, and
  its project file is the user-modified read-only `wanaka.toml`. No
  before/after was run, and none is claimed. **This is the highest-risk item
  in the wave**: the refusal verdict and the coverage criterion both change
  which branches get cut on real relief, and only a live run can size that.

  ADJACENT DEFECTS SEEN, NOT FIXED: **the V is a MODEL** — a trapezoidal
  groove is not a V, and a single wall angle either understates the wall
  (apex-to-rim secant) or ignores the floor (steepest local gradient, what
  ships); eroding directly against the sampled `surface_z` cross-section
  needs no wall angle at all, handles trapezoids/curvature/asymmetry
  natively, and is a strict generalisation of what landed (recorded in
  `reach`'s module doc as the wave-B follow-up). The ridge sits off the
  trough centre on symmetric grooves (Checkpoint A §6's "the apex is not the
  mask centre"), which is why a symmetric fixture can emit a one-sided fan —
  real, measured, not fixed here. `attach_generic_rest_analysis` (H2.5) now
  routes through the canonical policy but takes the DETECTOR DEFAULT fan
  because `RestAnalysisConfig` carries no stepover — fine for a report-only
  pass, wrong the moment it emits paths. `ComputeMessage` grew one boxed
  `Option` closer to its 280-byte clippy ceiling. `RestCenterline::samples`
  is measured on the rest-grid cell (0.5 mm shipped) while the taper's tip is
  Ø1 — the H3 resolution question applies to the cross-section too.

  Notes for WAVE B: H2.3 UnifiedFinish derived stepover
  (`envelope_radius * 0.5`) is now the only remaining envelope-scaled
  routing/fit number in the finishing stack and the reach policy can size it
  (`engagement_radius(delta)` or the cusp target) — untouched by design;
  H2.4 crease-own-region `tool_radius` in `finish_planner::decompose` is
  still dormant and still needs its make-it-live sentry first; H2.5 wants a
  real fan on `RestAnalysisConfig`; the sampled-cross-section generalisation
  above; and the wanaka before/after this wave could not run.
