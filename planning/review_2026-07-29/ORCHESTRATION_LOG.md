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

- impl-14 DONE -> **H2 ROUTING WAVE B COMMITTED: PR-6a `b8e3a0d`, PR-6b
  `be218fe`, PR-7 `e922931`** (behavioural, under the approved
  Checkpoint A). Wave A's parting note — "UnifiedFinish's derived stepover
  is the LAST envelope-scaled routing/fit number in the finishing stack" —
  is now false, which was the point.

  **PR-6a / H2.3 — the derived pencil stepover.** `unified_finish.rs` sized
  the crease/pencil fan at `cutter.envelope_radius_mm() * 0.5` (1.5 mm on
  the shipped Ø1-tip / 7° / Ø6-shank taper: half the SHANK, 3× the whole
  tip) and fed that ONE scalar to both the coverage routing criterion and
  the emission, overstating both sides of the coverage question at once.
  New `reach::suggested_offset_stepover_mm(cutter, depth)` = 50% of
  `working_half_width_mm` — `engagement_radius_mm(δ)` floored at the cusp
  radius, which is the SAME expression `coverage_cap_passes` already used
  for the centreline pass's own band, now factored out so there is one
  copy. Two properties are ASSERTED, not assumed: it is never coarser than
  the retired rule (`width_at_height` saturates at the envelope radius) and
  finer is the conservative direction (`offset_passes_per_side` FLOORS
  `reach/stepover`); and at this stepover the coverage cap FLOOR is exactly
  2 for every tool at every depth, so routing and emission agree by
  construction. Reference depth is `min_valley_depth` — the shallowest rest
  the detector reports, hence the narrowest band any emitted pass works,
  and the only depth in scope before the detector has run.
  **MEASURED: stepover 1.500 -> 0.250 mm; through the shipped pencil
  emitter over the same valley with the same tool the envelope spacing
  resolves ONE distinct lateral pass position (the fan was DEAD CODE —
  Checkpoint A probe 1's symptom, reproduced end to end) against SEVEN
  over a 1.5 mm band at 0.24 mm minimum gap.** RED-FIRST CONFIRMED by
  reverting the derivation in place: 3 of 4 new gates fail. Ball control
  0.750 vs 0.750, unchanged — a plain ball's cusp radius IS its envelope
  radius, so this migration moves TAPERED TOOLS ONLY, the same shape as
  wave A's task #12 fix.
  Telemetry took wave A's channel: `ClaimsReport` ->
  `GenerationFindings::derived_stepover` -> `ToolpathStats::derived_stepover`
  (boxed, `large_enum_variant`) -> `ids::CONFIG_DERIVED_STEPOVER`, `Info`,
  report-only; it carries the value, the depth, WHY that depth, and what
  the envelope rule would have given, and is SILENT when they agree.
  PR-2's finding is recorded in place: the retired comment justified the
  envelope value as parity with `PencilParams::default().offset_stepover`;
  that parity was FALSE (the default is the literal 0.5 mm), so nothing was
  owed to it.

  **PR-6b / H2.4 — the crease-own-region threshold.** `decompose` took a
  bare positional `tool_radius: f64` used for exactly one decision, and the
  only production caller passed `cutter.radius()` (ENVELOPE) while building
  the very `FinishPlannerParams` beside it from `cusp_radius()`: one
  struct, two tool scales, 6.0 mm vs 1.0 mm apart on the taper.
  MADE LIVE FIRST as required — the crease slice is dormant (`&[]`, always),
  so the new sentry drives `decompose` with a NON-EMPTY slice and straddles
  the threshold by ±1%, with the corridor asserted present on both arms so
  a degenerate fixture cannot pass it.
  **THE SCALE IS CUSP, and the field's doc comment says why: this is a
  planner-TERRITORY question** — asked in the same vocabulary as
  `pencil_claim_floor` / `close_radius_mm` / `min_region_area_mm2`, all
  cusp-derived — **and the FIT question was already answered upstream by
  `reach`**: a crease only reaches `decompose` when the detector's coverage
  criterion routed it to Pencil. Re-deriving reach there would be a second
  routing decision on a struct with no cross-section to derive it from.
  Consolidated rather than patched: `corridor_k` (unitless, no consumer
  outside the module) plus the positional scalar become ONE named value,
  `FinishPlannerParams::crease_own_region_half_width_mm` =
  `CREASE_OWN_REGION_K * cusp_radius`; `decompose`/`decompose_surface` lose
  the argument; 32 call sites updated.
  **PRODUCTION OUTPUT BYTE-IDENTICAL, PROVEN**: the gate-3 fingerprints
  were captured on the pre-H2.4 tree (`b8e3a0d`) with a throwaway probe and
  re-asserted after — taper 1855 moves / `0xe031_2509_7b20_8fb1`, ball 1301
  / `0xcfab_a674_0efc_aece`.

  **PR-7 / H2.5 — the generic rest analysis.**
  `attach_generic_rest_analysis` built its `RestFieldParams` with
  `..Default::default()`, so the coverage criterion ran against a LITERAL
  0.5 mm stepover and a 0-pass cap. `RestAnalysisConfig` gains
  `offset_stepover_mm: Option<f64>` and `num_offset_passes: Option<usize>`
  (serde `default` + `skip_serializing_if`, so pre-PR-7 projects load
  unchanged and unset dials are never written out); `None` is an
  INSTRUCTION — "ask the reach policy" — not an absent value, and the MCP
  path passes the `Option`s straight through for exactly that reason (only
  the generation path knows the cutter). No parallel formula remains in
  `execute.rs`. Same derived-stepover telemetry, site
  `"generic rest analysis routing"`; suppressed when the operator PINS the
  stepover, because that number is theirs. `record_derived_stepover` is
  FIRST-WRITER-WINS: one toolpath can derive twice (the op's own site, then
  this post-pass) and the op's own is the load-bearing one.
  **RED-FIRST DIFFERENTIAL**: on `grooved_block(8.0, 50.0, 5.0)` with a Ø12
  ball reference and a 4-pass cap, the median branch reach is 1.479 mm —
  between the policy threshold (4 × 0.25 = 1.0) and the retired one
  (4 × 0.5 = 2.0). The verdict FLIPS: retired -> 2 centrelines / 2 pencil
  components / 37.0 mm traced; policy -> 0 centrelines / 2 clearing
  components / 0.0 mm traced. The retired literal over-claimed the fan by
  2× and handed those branches to a pencil pass that cannot clear them.
  **HONEST LIMIT, ASSERTED AS A GATE (not buried in prose): the artifacts
  this pass ATTACHES do not move.** `region_polygons` is extracted from the
  cleaned rest mask BEFORE any branch is routed (`rest_field.rs` step 2b)
  and the pass discards `centerlines`/`clearing_regions` entirely, so the
  routing flip above changes nothing an operator sees today. Gate 3 pins
  that equality — region count, region area and grid cell count identical
  across the two arms — so the day this pass consumes the verdict or emits
  paths, the pin trips and the difference becomes real output. The
  operator-visible change PR-7 ships is the diagnostic and the two dials.

  Surfaces touched: `rs_cam_mcp::server::SetRestAnalysisConfigParam` (+2
  optional fields, additive), `mcp_bridge::McpRequestKind::
  SetRestAnalysisConfig`, `mcp_server`'s tool description, and
  `app::mcp::mcp_set_rest_analysis_config` (its five optional dials are now
  a named `RestAnalysisDials` struct — the argument-count lint, and every
  one of them means "I did not say", not "zero"). `viz::io::project`'s
  round-trip test now also covers the new fields staying `None`.

  **RG PROOF — ONE POLICY IMPLEMENTATION.** Across `pencil.rs`,
  `unified_finish.rs`, `compute/execute.rs`, `crease_paths.rs` and
  `rest_field.rs` there is no production envelope-scaled routing/fit scalar
  left. `pencil.rs`: zero hits at all. Everything that remains is one of:
  a DOC comment naming the retired rule (`unified_finish` 283/584/948/1050,
  `crease_paths` 34, `unified_finish` 1146); TELEMETRY recording the
  retired baseline so the diagnostic can quote it (`unified_finish`
  1075/1090, `execute` 2342); a Rule-4 padding / erosion / polygon
  reach-back, explicitly out of scope (`rest_field` 574 grid margin, 656
  erosion, 776 region dilation, 80 reference erosion radius); a TEST
  (`unified_finish` 2638 raster-edge tolerance, `rest_field` 2580); or one
  of the census's 63 locked non-finishing sites (`execute` 237/548/702/
  752/799/862/928/965/1110/1256 — 2D and clearing op params).

  Gates (all three commits): derived_stepover_pr6a 4/4 (new);
  crease_own_region_pr6b 3/3 (new); generic_rest_routing_pr7 6/6 (new);
  unified_finish_tapered_end_to_end_m21 9/9; unified_finish_semantic_regions
  4/4; standing_material_channel_am9 4/4;
  unified_finish_dropped_band_finding_d1 3/3; pencil_tip_float_channel_d1
  4/4; reach_policy_pr4 6/6; coverage_routing_pr5 6/6;
  checkpoint_a_valley_matrix 14/14; finish_resolution_policy_pr3 8/8 (FNV
  fingerprints UNCHANGED); tool_scale_semantics_pr2 8/8;
  tapered_cusp_radius_sentry 4/4; finish_planner_wanaka_decompose 1/1 (+4
  ignored); air_cut_denominators_lh1 4/4; `-p rs_cam_core --lib` 2183
  passed / exactly the 3 known adaptive3d reds; viz 227/227; cli 14/14;
  mcp 4/4; clippy --workspace --all-targets -D warnings exit 0.

  **WANAKA: DEFERRED TO LIVE VALIDATION, stated honestly.** Same reason as
  wave A: the only harness that reaches these paths is the `#[ignore]`d
  full-project ladder over the user-modified read-only `wanaka.toml`. No
  before/after was run and none is claimed. PR-6a is the highest-risk of
  the three on real relief — a 6× finer crease fan is 6× more passes
  wherever the claims pipeline is enabled (it is default-OFF), and the
  tighter coverage threshold routes more branches to clearing.

  ADJACENT DEFECTS SEEN, NOT FIXED: (1) **the generic rest pass throws its
  own routing away** — it runs a detector that produces `centerlines`,
  `clearing_regions` and a per-branch verdict and attaches only the mask
  grid and mask polygons, so `BoundarySource::DerivedRestRegions`
  boundaries cannot distinguish a band a pencil can cover from one it
  cannot; that is the change that would make PR-7 visible, and it is a
  behavioural decision nobody has taken. (2) The claims stepover is ONE
  scalar sized at `min_valley_depth`, while `reach` is per-point — the same
  generalisation `reach`'s module doc records for the cross-section, and it
  needs `centerline_cut_paths` to stop taking a scalar fan. (3)
  `GenerationFindings` now has ONE derived-stepover slot for what can be
  two derivations; first-writer-wins is a choice, not a representation.
  (4) `ToolpathStats` grew one more boxed `Option` — the `ComputeMessage`
  280-byte ceiling is closer again (it did not trip; clippy is clean).

  Notes for WAVE H3: `RampFinish` intermediate mode should take the named
  geo-mean policy variant (Checkpoint B ruling) — `reach` now has
  `working_half_width_mm` as the natural place for a second named scale,
  so add it there rather than in the op. The cone-gouge clamp wants
  `solve_reach`'s refusal, not a new inequality: the refused case IS "the
  cutter cannot hold this depth here". The `max_rings` experiment should
  derive its budget from `suggested_offset_stepover_mm` for consistency
  with this wave, and remember v3's lesson that a naive cap raise measured
  34× worse. Also still open from wave A: the sampled-cross-section
  generalisation that would retire the wall-angle V model entirely.

- impl-15 DONE -> **H3 WAVE COMMITTED: PR-8a `18faaf9`, PR-8b `074789e`,
  PR-8c `22a6a22`, PR-8d = THIS COMMIT** (behavioural, under the approved
  Checkpoint B). Two of the four slices ended somewhere other than where
  the evidence pointed, and both are recorded that way.

  **PR-8a / RampFinish -> a NAMED intermediate mode.**
  `FinishResolutionMode::GeoMeanEnvelopeCusp` =
  `sqrt((envelope/4)·(cusp/4)).max(tolerance)` — 0.306 mm on the shipped
  Ø1-tip / 7° / Ø6-shank taper against 0.750. Named by its FORMULA, not by
  a judgement: `Intermediate` would hide which two numbers it sits between.
  GEOMETRIC because the quantity traded is a RATIO, and the gate asserts
  that equal-factor property (2.449× each way) rather than the value.
  Computed from the two SIBLING formulas so the claim cannot drift from
  what the modes resolve to; tolerance floor applied ONCE, to the mean.
  `CellSource::GeoMeanEnvelopeCuspRadius` per the PR-3 rule (a new variant
  is cheaper than a mislabelled one — wave D3's `ToleranceFloor` pattern):
  asserted to compare with NEITHER factor, so a report cannot put a
  0.306 mm measurement beside a 0.750 mm one under one heading.
  **Measured (§3.2's own fixtures + its 0.05 mm reference ruler):** narrow
  valley deepest gouge −2.3939 -> **0.0000** mm, 2 -> 0 samples past 50 µm,
  137 -> 181 moves (1.32×); narrow ridge −0.1621 -> **0.0000**, 2 -> 0,
  64 -> 85 (1.33×). Both legacy numbers are §3.2's to four decimals.
  Ball control is a BYTE-IDENTICAL toolpath equality (the geo-mean of two
  equal cells is that cell), not a cell-size equality. **The PR-2 tripwire
  needed no update and was not touched — 8/8, zero edits: checked, not
  assumed, it pins `build_finish_surface_with_cancel`, the shared legacy
  ADAPTER, which since PR-3 no production op calls.**

  **PR-8b / the cone-gouge clamp — and §8.2's hypothesis was WRONG.**
  §8.2 read the 4.2 mm gouge as "the ramp descends into a 62° cone the tool
  profile cannot enter". Two separate errors produce it and only one is
  about the cone. (1) `z_bottom` came from `SurfaceHeightmap::min_z()`, the
  minimum over ALL cells — and a finish grid is padded by one envelope
  radius per side, so the ladder bottom was the MESH BBOX FLOOR on
  essentially every ramp-finish run ever generated (−3.000 requested
  against −2.407 holdable). (2) `match_contours` pairs loops by nearest
  centroid and `ramp_between_contours` interpolates them by arc-length
  fraction; neither correspondence is geometric.
  **(2) is what produces the 4.2 mm, and it is NOT in the cone**: the
  deepest gouge sits at (3.981, 4.113) on the flank of a convex 50° DOME,
  path Z −1.757 against a reachable surface at +2.472. Clamping only the
  ladder bottom leaves it at −4.229 unchanged (probed, then reverted). A
  dome flank has no valley, no rim and no walls — **fifth time this
  programme has had a confidently-named mechanism turn out not to be the
  cause.**
  So the clamp is PER POINT against `dropcutter::point_drop_cutter`, the
  same query that builds the generation surface, asked at the ramp point's
  own XY. **DEVIATION FROM THE BRIEF, stated plainly:** the brief asked for
  `solve_reach`'s refusal to do the clamping. It does not, and the reason
  is PR-6b's own precedent — the fit question is already answered upstream,
  exactly, against the real mesh, and `reach`'s module doc already records
  solving directly against the sampled cross-section as the strict
  generalisation of the V model. Re-deriving a coarser model beside an
  exact one is what PR-6b declined to do in `decompose`. `solve_reach` IS
  load-bearing for the LADDER half, as the independent oracle: it REFUSES
  3 mm in this 61.9° cone for this taper and does not refuse 0.3 mm, so
  "the cutter cannot hold this depth here" is the canonical policy's own
  verdict on the number `min_z()` was handing the ladder.
  Per-point and not grid-read because the defect is resolution-INDEPENDENT:
  a nearest-cell lookup at the shipped 0.306 mm cell lands the worst gouge
  at −0.225 mm (half a cell across a 50° flank), the exact query at
  **−0.020 mm**, which is the 0.05 mm REFERENCE field's own bilinear error.
  **4.2293 -> 0.0199 mm; 486 of 653 ramp points (74%) were commanded below
  reach.**
  **THE BIGGER FINDING: the clamp fires on a FLAT PLANE.** On a single 17°
  plane, with no pit and a ladder needing no lifting, **567 of 1182 ramp
  points sit below the reachable surface by up to 4.71 mm on a 4.8 mm-tall
  plane.** The blend defect is GENERIC. Every ramp-finish operation on
  every model has been doing this; the new diagnostic will fire on
  essentially every ramp-finish toolpath; fixing the correspondence is a
  rewrite of the op's core and was NOT attempted.
  Channel: `RampReachClamp` -> `GenerationFindings` -> `ToolpathStats`
  (boxed) -> `ids::GEOM_RAMP_REACH_CLAMP`, `Caution`, report-only, on BOTH
  the session path and the GUI worker's parallel copy; worded as UNCUT
  MATERIAL because the emitted path is now safe and a dexel run replays a
  clean pass. **A/M9 standing-material: RampFinish has NO wiring and this
  adds none** — it runs no ring cascade, so `standing_material_mm2` stays
  `None` ("not measured"), correct under A/M9's own rule. The material the
  clamp leaves is reported as a lift magnitude, never as an area. Gap
  recorded.
  **PR-8a's residual claim is SUBSUMED by this commit and was RESTATED, not
  hidden**: the clamp is resolution-independent, so it removes §3.2's
  gouges on the LEGACY arm too and PR-8a's red evidence stopped being
  reproducible. The gate became
  `ramp_finish_geo_mean_policy_halves_the_descent_chords` and asserts what
  PR-8b does NOT subsume — the cell IS the sampling step and the clamp
  constrains only ENDPOINTS: shortest cutting chord 1.2953 -> 0.5916 mm
  (2.19×) on the valley and 1.1909 -> 0.4656 (2.56×) on the ridge, against
  a 2.45× cell. A mean-chord gate was tried first and was the WRONG
  instrument (1.25× — `simplify_path_3d` collapses the collinear
  stretches); recorded in the test. Honest note: the clamp record does NOT
  show the finer cell reducing truncation (38% / 36% / 45% of points
  clamped at envelope/4 / geo-mean / cusp/4), so PR-8a's remaining
  justification is chord fidelity between the scored endpoints, which the
  endpoint-based residual instrument cannot see.

  **PR-8c / max_rings experiment — REJECT, with the arithmetic.** New
  additive `ScallopRingBudget` seam (production passes `FlatGroundStepover`
  only; PR-3's scallop fingerprint unchanged). At the cusp/4 arm:

  | fixture | budget | rings | uncut core mm² |
  |---|---|---|---|
  | narrow ridge | flat-ground (shipped) | 51 | **19.32** |
  | | reach policy | 55 | **12.79** |
  | | clamp floor (v3 control) | 70 | **0.00** |
  | mixed ribbon | flat-ground (shipped) | 51 | **33.24** |
  | | reach policy | 55 | **25.78** |
  | | clamp floor (v3 control) | 79 | **0.00** |

  The ruling's adopt condition was standing -> 0. Ridge −34%, ribbon −22%;
  not met. **It could never have worked:** the reach stepover is 0.250 mm
  against the flat-ground 0.280, a 12% wider budget, while the control
  shows the cascade collapsing NATURALLY at 70/79 rings — the requirement
  is 1.4–1.6×. No tool-scaled stepover lands there because the shortfall is
  not a tool scale: `ring_stepover` takes the MIN across a whole ring, so a
  budget from any NOMINAL stepover describes a spacing the loop never uses.
  **The v3 control cannot be adjudicated on these fixtures, and that is
  itself the finding**: it drives standing to 0.00 everywhere for +11–20%
  time with the ribbon's deepest gouge 3× worse — directionally v3's
  regression, nowhere near its magnitude (+92%, 34×), because these are
  16×16 mm fixtures where the cascade collapses at ~70 rings against v3's
  THOUSANDS at ~25 µm on real relief. Nothing here weakens v3's result and
  nothing here may be used to argue for it. Evidence: dated addendum
  §A.1–A.5 in `CHECKPOINT_B_EVIDENCE.md`.
  **SteepShallow deferral written into the CODE** (doc comment on
  `steep_shallow_generation_resolution`, where someone about to change the
  selector looks), with the measured zero delta, the measured 10.6–13.3%
  label error that does NOT absolve it, and the two readings that fit. The
  addendum names what would discriminate: all four fixtures carry
  contiguous steep territory far wider than a 0.75 mm cell while the op
  dilates steep by 2.0 mm and erodes shallow by 1.0 mm, so a mislabelled
  boundary cell is absorbed before it reaches an emitted move — a
  discriminating fixture needs interdigitated steep fingers NARROWER than
  the overlap distance and must score on `SteepShallowSplit`'s move ranges,
  not on residuals.

  **PR-8d / the 0.9 µm segment floor.** ROOT-CAUSED before fixing: probed
  over 4 fixtures × 4 arms, the offender is **exactly two segments of
  0.000891 mm, both inside `SteepShallowSplit::steep`** (the waterline
  half), bit-identical on every arm — so the cell is not the cause and a
  finer grid is not the cure. SOURCE is
  `contour_extract::weave_contours`: marching squares places each cell-edge
  vertex at an EXACT fiber interval boundary
  (`find_interval_boundary_x`/`_y` return the interval ENDPOINT inside the
  edge span, not the edge midpoint), so two adjacent cells whose boundaries
  resolve to the same crossing chain two coincident-to-noise vertices.
  Of the three consumers of `weave_contours`, `ramp_finish` already
  RDP-simplifies; this op and `waterline.rs` emit raw, and this op carries
  a `tolerance` field documented as "Path tolerance for simplification"
  that it has never used for that. **The missing filter/merge decision is
  at the emission site**, and that is what was added.
  The floor is NOT invented: `MIN_EMITTED_SEGMENT_MM = 0.001` is the
  coordinate quantum of the COARSEST shipped post (`grbl.toml` and
  `grblhal.toml` emit XYZ at 3 dp; `linuxcnc`/`mach3` at 4), below which a
  move rounds to the same coordinate words as its predecessor and leaves a
  literally zero-length G1. `the_floor_is_the_coarsest_shipped_post_quantum`
  DERIVES the bound from the shipped TOMLs, so adding a coarser post is a
  red test. Explicitly NOT an accel floor — that is
  `condition::merge_linear_runs`' job at a caller-chosen tolerance, and the
  doc says so in both places.
  **Distribution after, on §8.1's fixture:** shortest cutting segment
  0.000891 -> **0.078616 mm**, 2723 -> 2721 segments, total cutting length
  4757.9 -> **4757.9284 mm** (four parts in ten million). The two fixtures
  that never exhibited it are asserted byte-unchanged (0.122003 / 0.500000
  mm minima, totals within 0.5 mm).
  **PR-3's SteepShallow fingerprint did NOT move and was not touched.**
  Checked, not assumed: `ridge_mesh()` has no degenerate segments, so the
  floor drops nothing there. The behaviour change is real but is confined
  to paths that contained sub-quantum output.

  Gates (all four commits): finish_resolution_policy_pr3 10/10 (8 -> 10);
  checkpoint_b_resolution_ab 8/8 fast (5 -> 8, +2 ignored);
  ramp_reach_clamp_pr8b 6/6 (new); steep_shallow_min_segment_pr8d 5/5
  (new); tool_scale_semantics_pr2 8/8 ZERO EDITS;
  unified_finish_tapered_end_to_end_m21 9/9; unified_finish_semantic_regions
  4/4; standing_material_channel_am9 4/4;
  unified_finish_dropped_band_finding_d1 3/3; pencil_tip_float_channel_d1
  4/4; reach_policy_pr4 6/6; coverage_routing_pr5 6/6;
  checkpoint_a_valley_matrix 14/14; derived_stepover_pr6a 4/4;
  crease_own_region_pr6b 3/3; generic_rest_routing_pr7 6/6;
  air_cut_denominators_lh1 4/4; capability_link_moves_safety 17/17;
  `--lib scallop::` 17/17, `--lib steep_shallow::` 13/13, `--lib
  ramp_finish::` 14/14; `-p rs_cam_core --lib` 2183 passed / exactly the 3
  known adaptive3d reds; viz 227/227; cli 14/14; mcp 4/4; clippy
  --workspace --all-targets -D warnings exit 0. `ComputeMessage` gained one
  boxed `Option` (PR-8b) and did NOT trip its 280-byte ceiling.

  **WANAKA: DEFERRED, stated honestly.** Same reason as waves A and B — the
  only harness that reaches these paths is the `#[ignore]`d full-project
  ladder over the user-modified read-only `wanaka.toml`. No before/after
  was run and none is claimed. PR-8b is the highest-risk of the four on
  real relief: the clamp fires on a plane, so it will fire everywhere, and
  every ramp-finish toolpath in every saved project will change.

  ADJACENT DEFECTS SEEN, NOT FIXED: (1) **`ramp_finish`'s contour
  correspondence is broken generically** — nearest-centroid loop matching +
  arc-length blending produce points unrelated to either loop, measured on
  a FLAT PLANE; the clamp makes this safe, not correct, and the real fix is
  a rewrite of `match_contours`/`ramp_between_contours`. (2)
  **`waterline.rs` has PR-8d's defect from the same source** — it emits
  `weave_contours` vertices raw with no floor, deliberately left alone to
  keep the commit one operation wide. (3) **RampFinish has no
  standing-material channel**, so the area the clamp leaves is reported
  only as a lift magnitude. (4) `SurfaceHeightmap::min_z()`'s
  uncovered-clamp trap is now documented and has a `min_covered_z()`
  counterpart, but **every other consumer of `min_z()` was left
  unaudited** — `steep_shallow` uses it for its own Z ladder bottom, and
  whether that is the intended reading there was not checked. (5) A
  scripted splice truncated five tests off the Checkpoint B harness
  mid-implementation; caught by the test COUNT and restored from `18faaf9`,
  which is an argument for reading the count on every run.

  Notes for whoever picks up H3's remainder: **scallop resolution adoption
  is still open and still gated behind Checkpoint C** — §3.1's cusp/4 arm
  buys an on-dial cusp and costs 19–33 mm² of standing material, and PR-8c
  showed no budget candidate closes that without becoming v3's regression.
  **The SteepShallow discriminating fixture is specified but not built**
  (addendum §A.5). The `ramp_finish` correspondence rewrite is now the
  biggest single defect in the finishing stack by measured magnitude.

---

## C-SEQUENCE WAVE 1 (C5+C10), 2026-07-30

Addendum C's first sequencing step: "C5 + C10 — one mechanical wave, zero
risk, ends two recurring taxes." Three commits, deliberately not squashed,
so the formatting churn can never hide a semantic change.

**Commits**

| # | Hash | Item | Diffstat |
|---|------|------|----------|
| 1 | `288b19b` | C10a whole-repo `cargo fmt` | 35 files, +542 / -264 |
| 2 | `88ea12f` | C5 box `ComputeMessage::Toolpath` | 5 files, +211 / -131 |
| 3 | `9e43b78` | C10b `.gitignore` anchoring audit | 4 files, +197 / -7 |

### Commit 1 — C10a, formatting only

35 `.rs` files, all pre-existing drift (the tree was clean before the run).
Verified formatting-only mechanically, not by eye: each file's content was
hashed with all whitespace stripped, then again with whitespace, commas and
brackets stripped. The four files that still differed under the second
normalisation were read in full — `diagnose.rs` and two test files are
`use`-statement reordering, `v3_cascade_ab.rs` is line wrapping. No
semantic change.

ONE non-fmt edit rode along, and it had to: rustfmt turned a match arm's
tail expression into a block, which then tripped
`clippy::semicolon_if_nothing_returned` (denied) at
`v3_cascade_ab.rs:2701`. A one-character `;`. Stated here rather than
quietly folded in, because the whole point of an isolated fmt commit is
that its diff is auditable as pure formatting.

### Commit 2 — C5, box the variant

`ComputeMessage::Toolpath` carried a whole `ComputeResult` inline.
`ToolpathStats` rides inside it, so every wave that added a generation
finding pushed the enum toward the denied `large_enum_variant` ceiling —
and three waves each paid by boxing one more rarely-`Some` finding
(`dropped_band`, `deprecated_dial`, `derived_stepover`, `ramp_reach_clamp`).
The tax was being paid in the wrong place: the constraint is a property of
the channel enum, not of the measurement record.

Boxed the variant. 16 sites: 1 definition, 2 producers in `worker.rs`, 8
constructions in `controller/tests.rs`, 4 patterns in
`compute/worker/tests.rs`, 1 consumer (`controller/events/compute.rs`)
which needed NO edit — partial moves out of a `Box` field are allowed.
The four `matches!`/`match` patterns could not stay as struct patterns
(box patterns are unstable) and became binding-plus-guard forms.

Added `compute::size_tests::compute_message_stays_small`, asserting
`size_of::<ComputeMessage>() <= 128`. Without it this is a fix that decays;
with it, re-inlining a large payload is a red test that names the remedy.
Checked that no OTHER variant inherits the ceiling: after boxing, the
largest is `Collision` at ~64 bytes (two `Vec`s and an `f64`).

The four inner `Box`es were LEFT boxed — unboxing them is ~96 lines across
20 files for no simplification, which is the perfection the brief said not
to chase. But their doc comments each justified the boxing by citing
`clippy::large_enum_variant` on `ComputeMessage`, and that reason is now
dead. Rewrote all four to state the reason that survives (`ToolpathStats`
is cloned per toolpath into session results and GUI state) and to say
explicitly that the enum is no longer why. A stale rationale is a lie a
later wave would have cited.

### Commit 3 — C10b, .gitignore audit

Two `.gitignore` files in the repo. Audited every entry against real and
hypothetical source paths with `git check-ignore -v --no-index` (the
`--no-index` matters: without it, git reports TRACKED files as "not
ignored", which hides exactly the shadowing this audit is looking for —
`fixtures/debug_adaptive/wanaka_diag.json` and the tracked `.claude` files
both read as clean until the flag went on).

Anchored: `demos/` → `/demos/`, `reference/` → `/reference/`,
`profile.json.gz`, `EEPROM.DAT`,
`fixtures/debug_adaptive/wanaka_diag.json` (already effectively anchored by
its embedded slash; leading slash added for uniformity), and
`tests/step_validation`'s `Cargo.lock`. `reference/` is the sharp one —
`src/.../reference/` is a plausible module name and would have vanished.

Left global, with reasons in the file: `/target` (already anchored),
`target/` in the nested crate (build output, never a source-directory
name), `/diagnostics/` (already fixed by the original incident).

**The audit found a LIVE second instance.** `.claude/` is the documented
home of this project's checked-in agents and skills — CLAUDE.md has a table
of them — and the unanchored entry had already eaten
`.claude/agents/sim-diagnostics.md` and `.claude/skills/sim-analysis/`.
Both documented in CLAUDE.md, both referenced in the agent-team notes,
neither in the repo. Identical to `from_generation.rs`: written, documented,
committed against, never landed. A blanket `.claude/` cannot be negated
(git will not re-include a file whose parent directory is excluded), so the
fix ignores the CONTENTS — `**/.claude/*` — and re-includes
`!**/.claude/agents/` and `!**/.claude/skills/`. Local state
(`settings*.json`, `worktrees/`) stays ignored, at every depth. The two
rescued files were added in the same commit; that is scope creep over a
pure anchoring brief, and it is deliberate, because an anchoring fix whose
recovered files are left untracked fixes nothing.

Verified: `git ls-files` 1181 before, 1181 after the ignore edit (nothing
tracked became ignored), 1183 after adding the two rescued files;
`git status --ignored --short` lists the same ignored set as before plus
nothing.

**Gates**

- `cargo fmt --check`: exit 0 (commits 1 and 2).
- `cargo clippy --workspace --all-targets -- -D warnings`: exit 0 before
  each of commits 1 and 2. Commit 3 touches no code.
- `cargo test -p rs_cam_viz -q`: 217 + 11 passed, 0 failed.
- `cargo test -p rs_cam_core -q --lib`: 2183 passed, 3 failed — exactly the
  three known adaptive3d reds
  (`peck_plunge_progresses_when_depth_per_pass_equals_retract_clearance`,
  `rapid_segment_lifts_to_safe_z_before_traverse`,
  `planner_sim_dexel_parity_agent_search`). No new red.
- One cargo job at a time throughout; `free -g` and `pgrep -af "carg[o]"`
  polled before each heavy invocation.

**Untouched, as instructed**: `planning/airrun_2026-06-01/wanaka.toml`
(user-modified) and `planning/review_2026-07-27/`. Also left unstaged:
`TECH_DEBT_RESEARCH_AND_FIX_PLAN.md`, which acquired a second, unrelated
"Addendum C" (the `generate_all` rest-stock fixpoint defect, A/M11) from
another session while this wave was running. Not this wave's to commit.

**Note for the next C item**: C5's ceiling is now enforced by a test rather
than by discipline, so C8's "single slots → collections" work is free to
grow `ToolpathStats` — the reason it was previously constrained is gone.

---

## LIVE VALIDATION 2026-07-30 — behavioral waves on wanaka

Report-only run. **Nothing was fixed**; every defect below is a report
line. Driven over the rs-cam MCP against the live GUI.

**Under validation:** `df41169..81e0012` (11 commits — PR-4, PR-5, PR-6a,
PR-6b, PR-7, PR-8a, PR-8b, PR-8c, PR-8d + 2 log commits), at HEAD
`b3b1d39`. Build reported `git_desc: b3b1d39-dirty`, core 0.1.0 — matches.

**HEAD moved during this run.** The measured binary was built at
`b3b1d39`. By the time this section was written the branch had advanced to
`61bd97c` (C1 harness + structural-safety work) with ~21 source files
modified in the working tree. **Everything below describes the `b3b1d39`
build and nothing later.** Any wave landed after `b3b1d39` is outside this
report, and re-validating against current HEAD needs a fresh binary — per
plan B.4, built and confirmed complete BEFORE reconnecting.

**Fixture:** `planning/airrun_2026-06-01/wanaka.toml`, `wanaka_full_tuned`,
2 setups / 9 toolpaths, stock 140 x 150 x 25. **Loaded read-only; never
saved.** Standing rule honoured.

### Method, and the one config change

Only the sanctioned keeper was applied: toolpath 8 `claims_reference`
`self_probe` -> `machined_stock` (§14p). Everything else left exactly as
loaded — notably `territory_clip: false`, `pencil_claims: false`,
`min_rest_depth_mm: 0.02`, `waterline_threshold_deg: 75`,
`raster_stepover: 0.3`, `scallop_height: 0.011`, `z_step: 0.3`, and
`rest_analysis.enabled: false`.

The brief named `set_rest_analysis_config` as the vehicle for
`claims_reference`. It is not — `claims_reference` is a UnifiedFinish
operation param (`set_toolpath_param`); `rest_analysis` is a separate,
disabled sub-config on the same op. The op param was set.

Sequence actually required (five steps, see A/M11 below):

| # | action | outcome |
|---|---|---|
| 1 | `run_simulation` @0.1 (only ops 0,1,2 generated) | OK |
| 2 | `generate_all` | 5 generated, **2 failed** (Lakes id 6, Unified id 15) |
| 3 | `run_simulation` @0.1 | OK; `semantic_summary_count` 0 -> 247 |
| 4 | `generate_all` | >40 min; all 7 enabled ops Done |
| 5 | `run_simulation` @0.1 | final measurement |

All sims at **0.1 mm** (tip-matched) as required. No cross-resolution
comparison was made.

### Per-op measurement (final sim, 0.1 mm)

Project: `collision_count` **0**, `rapid_collision_count` **0**,
verdict **OK**, `total_runtime_s` 14 660.5 (4.07 h),
`air_cut_pct_of_total_runtime` 15.459, `air_cut_pct_of_cutting_time`
61.827, `average_engagement` 0.19435, `hotspot_count` 467,
`issue_count` 85 745, `semantic_summary_count` **467**.

| idx | name | op | cutting mm | rapid mm | moves |
|---|---|---|---|---|---|
| 0 | Pin Drill | PinDrill | 74.0 | 912.1 | 68 |
| 1 | Back Rough | Adaptive3d | 12 602.0 | 2 623.9 | 3 345 |
| 2 | Holes | Drill | 744.0 | 950.2 | 216 |
| 3 | Rivers (copy) | ProjectCurve | — DISABLED | | |
| 4 | Rivers (back) | ProjectCurve | 2 479.0 | 7 617.8 | 2 043 |
| 5 | Lakes (back, inside) | ProjectCurve | 1 585.1 | 2 413.7 | 1 802 |
| 6 | 3D Rough 6 | Adaptive3d | 4 896.9 | 1 712.0 | 2 217 |
| 7 | 3D Finish 6 | DropCutter | — DISABLED | | |
| 8 | Unified Finish 6 (live v2) | UnifiedFinish | 62 703.4 | 38 244.4 | 148 429 |

**Do not compare op 8's 62 703 mm against §14p's 5 259 mm.** That
measurement had `territory_clip: true` and `pencil_claims: true`; this run
has both `false`. With `territory_clip: false` the op is effectively
all-over, not a rest pass. Different configuration, **not** a regression.
Stated explicitly because forming exactly this ratio is the error the
radius audit caught.

### PASS — the new report-only channels all populate

`narrate_toolpath(8)`, verbatim:

- `"Semantic trace: 202 items (189 move-linked); depth levels 0, regions
  12, rings 185."`
- `"Region mix: VerySteep band (waterline) x8 (7374 moves), MidSteep band
  (scallop) x1 (57627 moves), Shallow band (raster) x3 (83418 moves)."`
- `"Standing material: none — 0 mm² measured, the ring cascade collapsed
  normally. XY-projected area (mm²); measured at generation (ring cascade
  residual)."`
- `"Unmachined band: none — every planned finish band still cut after
  height resolution."`
- `"Tip float: not measured — this operation emits no valley centrelines,
  so no reach residual exists to report. Absence of a number is not a
  zero."`

| channel | verdict |
|---|---|
| A/M8 semantic annotation | **PASS** — `regions 12` where the defect was `regions 0`; full strategy mix; `rings 185` |
| A/M9 standing material | **PASS** — populated at 0 mm², and it **declares domain + stage**, satisfying M1 provenance |
| D1 dropped-band | **PASS** — populated, correctly silent |
| D1 tip-float | **PASS** — and the wording is exemplary: *"Absence of a number is not a zero"* distinguishes not-measured from zero |

**The radius fix is visible live.** The VerySteep band carries **8 regions
/ 7 374 moves**. §14q measured **zero** VerySteep regions pre-fix on this
terrain. This is an **existence result, not a measured delta** — no live
A/B was run, and none is claimed.

### NOT EXERCISED — this fixture cannot validate three of the four waves

The project's op inventory is PinDrill, Adaptive3d x2, Drill,
ProjectCurve x3, DropCutter (disabled), UnifiedFinish. Therefore:

| wave | why unexercised |
|---|---|
| PR-8a RampFinish geo-mean resolution | **no RampFinish op exists in this project** |
| PR-8b cone-gouge reach clamp (`GEOM_RAMP_REACH_CLAMP`) | same — the finding whose headline is *"fires on essentially every ramp-finish toolpath"* has nothing here to fire on |
| PR-8d SteepShallow segment floor | **no Steep/Shallow op** |
| PR-6a derived pencil stepover | `pencil_claims: false`; narration independently confirms *"emits no valley centrelines"*, so no fan is built and no stepover is derived. `CONFIG_DERIVED_STEPOVER` correctly **ABSENT** |
| PR-7 `route_width_factor` deprecation | `rest_analysis.enabled: false`, so the retired dial is never deserialized |
| PR-4/5 routing counts (centerlines, offset fans, refusals) | no standalone Pencil op and no claims path active — **zero routing decisions occurred** |

These are **fixture gaps, not fix failures**. Recorded as NOT EXERCISED,
deliberately not as PASS. Declaring a pass on an unexercised path is the
precise failure mode the radius audit was commissioned to catch.

**Action required:** the behavioral waves need a fixture with a RampFinish
op, a SteepShallow op, and a Pencil op with claims enabled. Checkpoint B's
own synthetic fixtures (narrow valley / narrow ridge / 17° plane) are the
natural home for the ramp clamp; they are already written and already
measured, and they — not wanaka — are that wave's evidence.

### CONCERN 1 — `load.chipload.within` contradicts its own evidence

`get_toolpath_diagnostics(8)`, verbatim:
`"Chipload within band (0.0007 mm/tooth)"`, severity `info`, with attached
evidence `min 0.004579474936024669 / max 0.009158949872049339 /
observed 0.0007371346137784619`, `row_id vendor_lut`,
`extrapolated: true`.

Observed is **~6x below the stated band minimum** and the verdict is
`Within`. Meanwhile ops 4 and 10, with observed 0.012877 against
min 0.032, both return `Exceeds { side: low }`. The same relationship
(observed < min) yields opposite verdicts.

The distinguishing feature is visible in `get_tool_load_report`: op 15's
bounds are `vendor_lut_extrapolated` with `confidence: approximate`
(*"extrapolated from row amana-tapered-hardwood-scallop-3175-2f
(calibrated d=3.175mm): diameter scale x0.42, hardness scale x1.00"*),
while 4 and 10 are `vendor_lut` / `validated`. **HYPOTHESIS, not a
conclusion:** the gate declines to fail on extrapolated bounds. If that is
the design it is defensible — but the surfaced message says "within band"
and discloses none of it, and op 15's Within arm simultaneously carries a
`burn_advisory` at the same 0.000737 value.

Four chipload numbers appear for this one operation and no two agree:

| source | mm/tooth |
|---|---|
| narration nominal | 0.0714 |
| `feeds.chipload_clamped_to_floor` pre -> post | 0.0044 -> 0.0250 |
| gate observed | 0.0007 |
| gate band | 0.00458 .. 0.00916 |

No mechanism is asserted. This subsystem has now had **five** confidently
named mechanisms turn out not to be the cause.

### CONCERN 2 — feed is throttled 87% by machine kinematics, not by load

`modulation_summary` for op 15: `median_feed_delta_pct` **-87.18**,
`moves_touched` 112 747 / 124 697, `strategy: constrained_max`,
`binding_constraint_distribution`: **`kinematic_reach` 0.904**,
`machine_max_feed` 0.096. Both roughing ops show the same shape
(-66.47%, `kinematic_reach` 0.988 and 0.966).

So the 4.07-hour project runtime is dominated by the machine being unable
to accelerate to commanded feed across 148 429 short moves — not by
cutting load, which is comfortably within every gate (peak power
0.00035 kW of 0.6; peak deflection 8 µm of 200 µm). This is the
accel-friendly-toolpaths thesis measured on a live finishing path, and it
is a stronger lever here than anything in the load model.

### CONCERN 3 — peak axial DOC exceeds the commanded step

- op 8: `"peak axial DOC 1.86mm at sample 132236 (move 4821, Linear,
  z=20.076, position (101.9, 108.4)). commanded depth_per_pass is
  unknown."` — `z_step` is 0.3 mm, so ~6x.
- op 10 (`3D Rough 6`), `per_depth_pass` pass 2 at z 19.8:
  `peak_axial_doc_mm` **5.200000762939453** against a 2.6 mm Z step —
  **exactly 2x**, in both `helix` and `linear` kinematics. Pass 1 at the
  same Z step reads exactly 2.6.
- op 4 (`Back Rough`) by contrast reads **exactly 3.0** on all five
  passes against a 3.0 mm step — clean.

The exact-2x on one pass of one op, beside exact-1x everywhere on another,
is a concrete lead. Related in KIND to the open A/L2 Rivers 6.07 mm spike.
**No mechanism named** — narration offers arc-fit overshoot / lift
bridging / uncleared stock, and arc-fit was already exonerated for the
Rivers case.

### CONCERN 4 — air-cut and engagement are structurally unmeasurable here

Narration reports `"71.0% of CUTTING time is air-cut"` and
`average engagement 0.079`, with the in-cut distribution
(n = 1 221 857): air 64.9%, thin 27.2%, light 5.2%, heavy 2.7%.

**These are instrument artifacts, not findings about the toolpath.**
`scallop_height` is **0.011 mm**; the sim cell is **0.100 mm**. The cusps
this operation removes are ~9x smaller than the dexel cell, so the grid
cannot represent the material being taken and the samples read as air.

The brief's rule — cell below the tool TIP radius — was followed (0.1 vs
0.5 mm). It is not sufficient: the cell must also be below the **cut
depth**. Resolving an 0.011 mm scallop needs ~0.005 mm cells, i.e. 28 000
cells per axis on a 140 mm stock. Infeasible. **Conclusion: for finish
passes at fine scallop heights, dexel air-cut% and engagement are
unmeasurable at any feasible resolution.** This does NOT affect
`rapid_collision_count`, which is geometric rather than volumetric — and
which is the signal the standing rules already designate as primary.

### Rendered surfaces — required, and they change nothing

- `planning/review_2026-07-29/liveval_2026-07-30/op8_toolpath_norapids.png`
  (6-view, `include_rapids: false`, 1600x1100)
- `planning/review_2026-07-29/liveval_2026-07-30/final_stock.png`
  (6-view machined stock, checkpoint 6 = after op 8)

Gross-defect checks **pass**: the machined area is continuous, there is no
standing block of the kind the v3 gate once ranked above a clean op, drill
holes are present, and the smooth/textured division in the stock matches
the toolpath render exactly. The toolpath view shows dense all-over
coverage — consistent with `territory_clip: false` — and a striking
density of short vertical excursions across the textured region, which is
the **A/M7 intra-node retract round-trip finding made visible**.

**Limits of this visual check, stated:** the tool documents only
`green = cutting, orange = rapid`, but both renders use blue and red as
well, so some features cannot be attributed with confidence. And a 6-view
composite cannot adjudicate fine surface quality — the trustworthy
pointwise instrument is `SimulationResult::column_deviations` (FIDELITY-
COLUMNS), which is a test-harness capability **not exposed over MCP**.
That is a gap for every live validation, this one included.

### Gate summary (`get_tool_load_report`)

7 toolpaths: **within 5, exceeds 2**, `fully_unmodeled` 0,
`not_applicable` 0. Both exceedances are `chipload / side: low` on the
roughing ops (`Back Rough` id 4, `3D Rough 6` id 10) at 0.012877 against
a 0.032 minimum — pre-existing, unrelated to these waves. Drill gates on
ops 14 and 7 all Within. Deflection Within on every modelled op
(8–25 µm against 200 µm), consistent with the audit's H1 note that the
deflection gate only bites for long/thin tools.

### New defects filed to the plan from this run

Two were significant enough to become plan items rather than report lines
(`TECH_DEBT_RESEARCH_AND_FIX_PLAN.md`, Addendum C):

- **A/M11 — `generate_all` is not a fixpoint over the rest-stock chain.**
  A chain of `k` dependent rest ops needs `k` sim->generate rounds and
  nothing tells you `k`. Sharpest evidence: `Rivers` (4) generated fine in
  round 2 while `Lakes` (5) failed **in the same pass**, because prior
  stock comes from a *simulation* and ops generated earlier in the pass are
  invisible. Plus: disabled ops keep live-looking error text (the user
  independently read `3D Finish 6` as "errored" when it is merely off);
  the message names no blocking op; and re-simulating raises no staleness
  signal.
- **A/M12 — MCP serializes every call behind generation, including its own
  escape hatches.** During the >40 min round-4 generate: `list_toolpaths`
  blocked and was **aborted after 1800 s**; `cancel_generation` —
  documented *"Instant response"* — blocked, and was serviced only after
  the job had already ended, returning `was_busy: false`. `generate_all`'s
  own timeout message recommends both of those calls as the remedy. The
  only working diagnosis was `/proc` thread accounting.
  Also observed: `narrate_toolpath` took ~12 min on 148 429 moves, and it
  is the workflow's documented *first* diagnostic.

Correction recorded for honesty: mid-run I read the CPU drop from ~720% to
~136% as "the cancel landed". It did not — the generation had completed on
its own. A `was_busy: false` from a queued cancel reports the state when it
was finally serviced, not when it was issued.

### VERDICT by subsystem

| subsystem | verdict |
|---|---|
| Routing (PR-4/5/6a/6b/7) | **NOT EXERCISED** — no Pencil op, `pencil_claims: false`, `rest_analysis` disabled. Zero routing decisions occurred |
| Ramp reach clamp (PR-8b) + geo-mean resolution (PR-8a) | **NOT EXERCISED** — no RampFinish op in this project |
| Segment floor (PR-8d) | **NOT EXERCISED** — no SteepShallow op |
| Diagnostics channels (A/M8, A/M9, D1 x2) | **PASS** — all four populate, with declared provenance |
| Radius fix live (§14q/§14r) | **PASS (existence)** — VerySteep 8 regions / 7 374 moves vs zero pre-fix; no A/B, so no delta claimed |
| Safety at 0.1 mm | **PASS** — 0 collisions, 0 rapid collisions, verdict OK on a freshly generated chain |
| Tool load | **CONCERN** — 2 pre-existing chipload-low exceedances; one apparent gate/message contradiction on op 8 |
| Runtime economy | **CONCERN** — feed throttled 87% median by `kinematic_reach`, not load |
| MCP workflow | **FAIL** — filed as A/M11 + A/M12 |

**What this run does and does not establish.** It establishes that the
report-only instrumentation waves work on a real project, that the chain
generates and simulates clean at tip-matched resolution with zero
collisions, and that steep territory now exists where it previously did
not. It establishes **nothing** about the routing or ramp waves, which this
fixture cannot reach. No strategy-value conclusion is drawn, and none
should be read into the op 8 numbers.

## C-SEQUENCE WAVE 2 (C1), 2026-07-30

Addendum C's second sequencing step: "C1 provenance contract — before any
motion/link work." Three commits, split on the line that matters for
review: **what provably changes nothing**, and **what deliberately
changes**.

**Commits**

| # | Hash | Scope | Diffstat |
|---|------|-------|----------|
| 1 | `61bd97c` | Refactor-invariance harness, captured at HEAD `5d32150` | 1 file, +377 |
| 2 | `77267ce` | Typestate contract + all transforms and call sites + carrier retirement | 22 files, +1074 / -430 |
| 3 | `60e2c0f` | Items 4a/4b — per-dressup attribution, unconditional reconcile path | 3 files, +126 / -8 |

### Research pass — the inventory

**Post-generation transforms that move indices** (everything else runs
BEFORE annotation, as ae10cb2 also found; `dressup::apply_tabs` inserts
moves but does so inside generation, before spans exist):

| Transform | What it does to indices |
|---|---|
| `dressup::apply_entry` | 1→K fan-out (plunge becomes ramp/helix) |
| `dressup::apply_dogbones` | insertion (overcut + return per corner) |
| `dressup::apply_lead_in_out_with_feeds` | insertion (lead arcs) |
| `dressup::apply_link_moves` | 3→1 collapse (retract triple → bridge) |
| `dressup::filter_air_cuts` | DELETION — the only one |
| `arcfit::fit_arcs` | N→1 collapse |
| `condition::merge_linear_runs` | N→M collapse |
| `tsp::optimize_rapid_order` | PERMUTATION (its own drop rule) |
| `feedopt::optimize_feed_rates` | none — content only |
| `boundary::clip_toolpath_to_boundary[_set]_with_provenance` | insertion (retract/rapid pairs) |
| `dressup::optimize_entry_descents_with_provenance` | insertion (split plunges) |

**Index-carrying channels**: `AnnotatedToolpath::spans` (region-node spans
among them) and `ToolpathSemanticTrace`'s per-item move links. Checked and
ruled out: `planner_engagement` (point-keyed, survives reshaping by
construction), `rest_grid` / `rest_regions` (coordinate-keyed).
**Found and NOT wired, deliberately**: `ToolpathDebugTrace`'s spans carry
`move_start`/`move_end` too, and `core_generate`'s is stale by
construction (set pre-dressups). It is debug-artifact-only and wiring it
would be a second unrequested diagnostics diff; recorded here so the next
wave can add it as one `RemapConsumer` impl plus one constructor argument.

**Call sites accepting a transformed toolpath back**: 5, as scoped —
`ProjectSession::generate_toolpath`'s dressups / boundary clip / entry-
descent split, and the viz worker's three equivalents.

### The contract, and why spans are not in the `ReconcileSet`

`Transformed<Unreconciled>` has no accessor for its payload;
`.reconcile(&mut ReconcileSet)` is the only door, and `into_inner` exists
only on `Transformed<Reconciled>`. `ReconcileSet::new` takes one argument
per registered channel — the arity IS the registry — and `RemapConsumer`
is sealed so a new channel must be declared next to the constructor it
breaks.

Spans deliberately ride INSIDE the payload rather than in the set. Not a
loophole: a transform does not merely translate span indices, it *emits*
spans (`Entry`, `DressupArtifact`, `LinkBridge`), and the reorder has to
drop spans a permutation interleaved with foreign moves. No generic
consumer can do either. Spans are therefore unforgettable for a different
structural reason — they are inside the value you cannot obtain without
reconciling everything else. Stated in the module doc so the next reader
does not have to re-derive it.

The reorder's foreign-intrusion rule was private to `tsp::remap_spans`; it
is now `MoveRemap::foreign_intrusion`, shared with the channels, so the
spans and the trace cannot disagree about what "scattered" means.

### The oracle

`transform_provenance_fingerprints` was written and run FIRST, on a clean
tree (the in-progress source edits were stashed for the capture), so its
constants cannot have been back-fitted. It pins two things per fixture:
the FNV-1a hash of the `Debug`-rendered move list, and where every
semantic item landed.

| Fixture | Geometry | Link landing sites |
|---|---|---|
| `three_pass` (barriered TSP + ramp + dogbones + leads + links + arc fit + merge) | `(23, 14756822782673573601)` | head (0,4) body (5,13) tail (14,22) whole (0,22) |
| `arc_raster` (same pipeline, 192 moves in, arc fit + merge actually collapse) | `(40, 9877459821106430315)` | head (0,16) body (16,27) tail (27,39) whole (0,39) |
| `face_full_chain` stage 1 dressups | `(74, 9692869450022244402)` | — |
| … stage 2 boundary clip | `(97, 3258911278473560309)` | — |
| … stage 3 descent split (6 splits) | `(103, 2154614841165484301)` | head (0,32) body (33,74) tail (75,102) whole (0,102) |

**All fifteen values identical before and after.** The carrier and the
contract agree index for index — which is the whole claim of commit 2.

### The deliberate diff (commit 3, isolated)

- **4a**: eleven per-dressup items each bound `0..len`, so per-step
  attribution could not distinguish the arc fitter from the ramp entry.
  They now bind the provenance's touched range, and BOTH cases declare
  themselves in a new `move_scope` param (`touched_moves` / `whole_path`).
  Visible: narrower `move_start`/`move_end` and re-derived bboxes on
  dressup items in a debug semantic trace. Nothing else.
- **4b**: the worker's remap calls were inside `if let Some(recorder)`,
  and the recorder was debug-gated — so the code keeping channels in step
  never ran in the shipping configuration. The `ReconcileSet` is now built
  unconditionally (the recorder stays debug-gated); new sentry
  `worker_reconciles_semantic_links_with_debug_options_disabled` drives
  the default configuration with index-moving dressups and asserts every
  link is in bounds, that not everything unlinked, and that each dressup
  item declares its scope.

### Retired

`SemanticLinkCarrier`, `ToolpathSemanticRecorder::remap_move_links`,
`SpanKind::SemanticLink`, `SpanPayload::SemanticLink` — the carrier's
transport vocabulary, which existed only to smuggle links through the span
vector. `SpanKind::ALL` is 9. ae10cb2's two carrier unit tests were ported
with their OUTCOME assertions unchanged; the two assertions about the
carrier's own bookkeeping (one carrier span per distinct range, all
stripped again) are gone because there are no carrier spans, replaced by
"reconciling adds no spans to the shipped toolpath". Its four integration
sentries are unchanged apart from the mechanical last-argument swap
(`None` → `&mut ReconcileSet::empty()`).

### Gates

- `cargo fmt --check`: exit 0 before each commit.
- `cargo clippy --workspace --all-targets -- -D warnings`: zero warnings
  before each commit.
- Doctests 5/5, including **four `compile_fail`** proofs: `into_inner` on
  an unreconciled value, dropping a `#[must_use]` `Transformed`,
  `ReconcileSet::new()` with a channel omitted, and an out-of-module
  `RemapConsumer` impl.
- `cargo test -p rs_cam_core --tests --no-fail-fast`: 2191 pass in the lib
  plus every integration target, with **four** reds — the three known
  adaptive3d ones
  (`peck_plunge_progresses_when_depth_per_pass_equals_retract_clearance`,
  `rapid_segment_lifts_to_safe_z_before_traverse`,
  `planner_sim_dexel_parity_agent_search`) and
  `wanaka_suggest_integration::wanaka_suggest_baseline`, which fails with
  "Toolpath id 11 missing from suggest cases — wanaka.toml shape
  changed?". That is the standing user-modified-wanaka trap, not this
  wave: the working-tree `wanaka.toml` has flipped `enabled` flags and an
  added toolpath 15 relative to HEAD, and no code change can remove a
  toolpath from a TOML. Nothing in this wave was staged from that file.
- Sentries: m21 9/9, unified_finish_semantic_regions 4/4,
  standing_material_channel_am9 4/4, finish_resolution_policy_pr3 10/10,
  tool_scale_semantics_pr2 8/8, capability_link_moves_safety 17/17,
  dressup_span_invariants 4/4, boundary_clip_invalidates_spans 2/2,
  adaptive3d_post_tsp_z_monotonicity 2/2,
  `region_node_ranges_tile_the_stitched_toolpath` ok.
- viz 218 + 11 (was 217 + 11; +1 is 4b's sentry), cli 14, mcp 4.
- One cargo job at a time throughout. The session opened with a foreign
  `sysml-spec-tests` run holding the slot and under 20 GB free; all work
  up to the baseline capture was done without cargo and the capture waited
  for the slot — the same collision P10 records.

**Deviation from the suggested slicing**: the brief proposed four commits
(harness / typestate core / migration / item 4). Commits 2 and 3 of that
plan were merged, because a commit introducing `Transformed` with nothing
migrated is dead code with no gate to pass, and the migration is what
makes it compile-enforced. The split that was kept is the one a reviewer
needs: everything provably identical in `77267ce`, everything deliberately
different in `60e2c0f`. Commit 2's state was gated with the sentry set,
fingerprints, doctests, clippy and fmt; the full core suite was run on the
final tree.

**Untouched, as instructed**: `planning/airrun_2026-06-01/wanaka.toml`
(user-modified) and `planning/review_2026-07-27/`. This wave's log entry
was appended on top of the LIVE VALIDATION section that landed from the
other session mid-wave (`9e5a65b`); none of its text was altered.

**Note for C2**: A/M7 is now unblocked in the sense C1 was gating it — a
motion-economy wave that adds or changes link transforms will be born
under the contract, and cannot ship a transform that does not report where
the moves went.

## C-SEQUENCE WAVE 3 (C2), 2026-07-30

Addendum C's third sequencing step: "C2 sentinel sweep — before M3 (the
classifier reads grids) and to close the live steep_shallow exposure." Two
commits plus this log entry, split the same way C1 was: the type and the
audit that justifies every call site in the first, the field-by-field
sweep in the second.

**Commits**

| # | Hash | Scope | Diffstat |
|---|------|-------|----------|
| 1 | `ce426d6` | `GridZ` + private grid storage + the `min_z()` consumer audit, incl. steep_shallow's verdict and its sentry | 17 files, +645 / -138 |
| 2 | `96bc300` | Sentinel-field sweep: `Engagement::axial_doc_fraction`, the two `KinematicsSummary` axial fields, `classify_rest_regions`' denominator, `effective_diameter_mm`'s tested contract | 7 files, +232 / -50 |

### The headline: the named suspect was NOT defective

PR-8b's log entry named `steep_shallow`'s Z-ladder bottom as the live
exposure — "every other `min_z()` consumer is unaudited (steep_shallow uses
it for its ladder bottom)". The audit says **the bbox floor is correct
there**, and the brief's instruction not to force a fix the audit does not
support is what this section exists to honour.

The steep half is a WATERLINE pass. It slices the MESH with
`waterline_contours` at each Z level; the drop-cutter grid only supplies
the steep/shallow classification. A vertical wall runs from the top face
down to the bbox floor and no drop-cutter grid can ever see it, so on a
plateau `min_covered_z()` IS the top face. Ladder levels below real
material are free (the contour routine returns nothing there); a
covered-only bottom costs every wall pass.

Counterfactual, run red-then-green on a 20x20x5 plateau fixture (Ø3 ball,
0.5 mm cell) rather than argued:

- `z_bottom = min_covered_z()` -> sentry FAILS, `the plateau must produce
  steep passes`: `z_top == z_bottom == 0.0`, the steep range is empty.
- `z_bottom = min_z_or_bbox_floor()` (shipped) -> passes, deepest steep cut
  is on the wall, below the covered minimum.

**Instrument lesson, recorded because it nearly produced the wrong
verdict**: the first version of that sentry measured the MERGED toolpath
and passed under the counterfactual. The shallow raster drop-cutters the
same padded grid, so IT reaches the bbox floor whatever the steep ladder
does. The sentry now measures `SteepShallowSplit::steep` only. A sentry
that cannot fail under the change it guards is not a sentry.

The consumer that WAS reading the wrong number is the one PR-8b already
fixed (`ramp_finish`, clamped to `min_covered_z()`), and one latent case
this wave declined to change: see the audit table in commit 1's body.

### What the type does

`enum GridZ { Covered(f64), Uncovered { z_or_bbox_floor }, OutOfBounds }`.
`SurfaceHeightmap::{z_values, covered}` are private (a Z without its
coverage flag is the defect the type prevents); `from_parts` panics on a
length mismatch. Typed reads are `z_at` / `z_at_index` / `z_at_world`; the
escape hatch is the `z_or_bbox_floor_*` family, and `min_z()` became
`min_z_or_bbox_floor()` because on ANY padded finish grid that is exactly
what it returns. Nothing computes a different number.

### Audit and sweep

Both tables are in the commit bodies verbatim, as the brief required:
commit 1 carries the `min_z()` / grid-read consumer audit (17 rows,
including two recorded OUT OF SCOPE — `DropCutterGrid`, which computes no
coverage mask at all, and `SlopeMap`'s angles, which are differentiated
across padding cells sitting at the bbox floor); commit 2 carries the
sentinel sweep (4 converted, 2 documented-and-tested, 8 ruled out).

### Behaviour invariance

- Every call site computes the same number it computed before, by
  construction: the first commit is renames plus one `if/else` -> typed
  `map_or` at `path.rs`'s pre-clear, and the audit table names the intent
  of each.
- `escape_hatch_is_byte_identical_to_the_untyped_read_everywhere` walks
  every cell of a padded grid and asserts the new accessors return exactly
  what the old ones did, `NEG_INFINITY` off-grid included.
- `fully_covered_grids_make_the_two_minima_identical` is the invariance
  half of C2's gate: on a grid with no uncovered cells nothing can observe
  this change.
- ONE consumer changes behaviour on partially-measured data and it is
  deliberate: `KinematicsSummary::average_axial_doc_fraction` now averages
  over the runtime that measured an axial fraction instead of over all
  cutting time. Justification: it is the contract `average_arc_radians`
  beside it has had since Step 2, the old arithmetic averaged unknowns in
  as zeros, and on any real trace the two denominators are the same number
  (only `is_cutting` samples reach the accumulator, and all of those come
  from the dexel emitter, which always measures).

### Gates

- `cargo fmt --check`: exit 0 before each commit.
- `cargo clippy --workspace --all-targets -- -D warnings`: zero warnings
  before each commit (one clippy red found and fixed en route: a redundant
  `Range::clone` in the new sentry).
- `cargo test -p rs_cam_core --tests --no-fail-fast`: lib 2193 pass / **3 fail**, and
  all 41 integration targets green. The three are the known adaptive3d
  reds (`peck_plunge_progresses_when_depth_per_pass_equals_retract_clearance`,
  `rapid_segment_lifts_to_safe_z_before_traverse`,
  `planner_sim_dexel_parity_agent_search`). The fourth known red,
  `wanaka_suggest_baseline`, did NOT reproduce this run — it passed, with
  the same user-modified `wanaka.toml` still dirty in the tree. Recorded as
  observed, not explained; nothing in this wave touches suggest's case
  list. (2193 vs C1's 2191 = the two new sentries in this wave.)
- viz **218 + 11 serially**; cli 14 (5 + 9), mcp 4.
  Three consecutive PARALLEL viz runs each failed ONE
  `compute::worker::tests` cancellation test and **a different one each
  time** (`cancelled_toolpath_returns_partial_debug_trace`,
  `cancel_all_marks_both_lanes_cancelling`,
  `analysis_cancel_completes_quickly` — the last is a wall-clock assertion
  by name). All 40 worker tests pass when the module runs alone, and the
  whole suite is 218/218 with `--test-threads=1`. That family is
  load-timing flaky under a busy machine (the run happened with 0.6 GB free
  while the foreign job compiled); this wave touches no worker,
  cancellation, or timing code — the only viz edit is the rest-footprint
  display plumbing. Flagged rather than filed as a wave red.
- New: `grid_z_uncovered_contract_c2.rs` 6/6.
- Commit 1 was gated **on its own tree**, not only as part of the union:
  after committing it the second slice was stashed by explicit path,
  `fmt --check` + `clippy --workspace --all-targets -D warnings` re-run
  clean, and the C2 / PR-3 / decompose sentries re-run green, then popped.
- C1's `transform_provenance_fingerprints` harness stays green — it pins
  the dressup/TSP pipeline, which this wave does not touch, so it is NOT
  the oracle for the heightmap consumers; their invariance evidence is the
  escape-hatch equality test plus the core suite's adaptive3d /
  steep_shallow / finish sentries.
- One cargo job at a time throughout. The session opened with the same
  foreign `sysml-spec-tests` loop holding the slot under 20 GB free; all
  editing was done without cargo and each gate waited for the slot.

**Untouched, as instructed**: `planning/airrun_2026-06-01/wanaka.toml`
(user-modified, still dirty in the tree) and `planning/review_2026-07-27/`.
Every commit staged file-by-file.

**Note for M3**: the classifier work can now ask a grid cell whether it is
backed by mesh without re-deriving the mask, and cannot read the bbox-floor
clamp by accident. The two OUT OF SCOPE rows above are the remaining holes:
`DropCutterGrid` has no mask at all, and `SlopeMap` angles at the mesh rim
are computed against padding.

---

## C-SEQUENCE WAVE 4 (A/M12 + A/M11), 2026-07-30

Addendum D, in D's own binding order: the escape hatches first, the loop
second — "never ship an unobservable, unabortable loop". Four commits plus
this log entry.

**Commits**

| # | Hash | Scope | Diffstat |
|---|------|-------|----------|
| 1 | `3f47bcc` | A/M12a — `GenerationControl` / `LaneControl` off-GUI cancel, `McpReadCache` snapshot fallback for the five cheap reads, `LaneSnapshot` gains the in-flight index/id | 13 files, +900 / -114 |
| 2 | `665cd1d` | A/M12b — `generation_status` tool; four corrected docstrings; the advancing sentry and the generate→status→cancel drive | 5 files, +197 / -15 |
| 3 | `7642bf0` | A/M11a — `ComputeStatus::{AwaitingPriorStock, Disabled}` + `effective/label/error_text/blocked_on/detail/needs_generation`, eleven-site consumer sweep | 11 files, +690 / -123 |
| 4 | `6edaaa1` | A/M11b — the fixpoint loop, moved onto `AppController`; caller-specified resolution or refusal; five acceptance gates | 9 files, +902 / -162 |

### A/M12 — the serializer, named

The plan listed three candidates: one mutex over `ProjectSession` held for
the whole generation; a single-threaded MCP request handler; or the compute
lane and request lane sharing one lock. **All three are wrong**, and saying
so matters because the obvious fix — "relax the session lock to an RwLock" —
would have relaxed nothing.

- There is no lock over `ProjectSession` at all. The GUI thread owns it
  outright; the worker never sees it.
- Generation runs on its own thread (`spawn_toolpath_lane`). Its
  `Mutex<LaneInner>` critical sections are a `VecDeque` push/pop and a
  phase-string swap — microseconds, never the duration of a job.
- rmcp 1.3 is not the serializer either: `service.rs` spawns a tokio task
  per inbound request (`spawn_service_task`), so the transport reads and
  dispatches concurrently.

The serializer is **`RsCamApp::drain_mcp_requests`** — the single point
where every MCP request is dispatched, running on the egui main thread,
once per repaint, and handling the drained `Vec<McpRequest>` strictly in
order. Nothing arbitrates between a cheap read and a 12-minute
`narrate_toolpath`; they are the same queue. So any main-thread stall — a
heavy in-band handler, a GPU re-upload of a 148k-move result, or the winit
loop being starved while the rayon pool is saturated — takes the entire MCP
surface with it, `cancel_generation` and `list_toolpaths` included. That is
the whole defect: the escape hatches were behind the thing they escape.

**The lock design, and why it is the smallest sound one.** Since there is no
lock to relax, the fix is a second door rather than a wider one:

- `GenerationControl` wraps the lane's existing `AtomicBool` + its
  short-critical-section mutex behind a `Send + Sync` trait object, cloned
  into the MCP server thread at startup. `cancel_generation` and
  `generation_status` are answered synchronously on the caller's thread and
  never touch the request channel. `McpRequestKind::CancelGeneration` was
  **deleted** — being on that channel was the defect, so leaving the variant
  behind would leave the trap.
- `McpReadCache` is an `Arc<RwLock<_>>` of five already-rendered JSON
  strings, republished by the GUI at ≤2 Hz. It is not a lock over state and
  makes no claim to be live. Reads take it only after losing a 750 ms race
  against the real GUI round-trip, and **only while the lane is active** —
  when the lane is idle the behaviour is byte-identical to before. The
  answer is labelled `served_from: "snapshot"` with its age and the
  in-flight op; a cache that has never been published refuses rather than
  handing back an empty list that reads like "this project has no
  toolpaths".

An `RwLock<ProjectSession>` would have been bigger, riskier, and — since the
GUI thread would still hold the write side for the same stalls — no better.

**A `was_busy: false` is now trustworthy.** The live run recorded reading a
CPU drop as "the cancel landed" when the generation had simply finished; the
queued cancel reported the lane state at service time, not at ask time. It
is now serviced on the asking thread, so the two coincide.

### A/M12 gate results

| gate | result |
|---|---|
| `list_toolpaths` < 1 s with a generation in flight | PASS — measured in `list_toolpaths_answers_from_the_snapshot_when_the_frame_loop_stalls`; the whole 9-test file runs in **1.51 s** wall clock, and it contains four 750 ms fallbacks plus one deliberate 1.5 s timeout |
| `cancel_generation` < 1 s **and** actually stops the job | PASS — asserts both the latency and that the lane double's cancel flag is set. No GUI thread exists in that test at all |
| `generation_status` names index + stage, sentry asserts it advances | PASS — stage tracks the planner across calls and `elapsed_s` strictly increases; an idle lane reports idle rather than a stale last job |
| integration test over the MCP surface, not the library API | PASS — `generate_then_status_then_cancel_over_the_mcp_surface` calls the tool methods |
| no generation throughput regression | See below |

**Throughput, and how it was checked.** Not by benchmark, because the honest
answer is structural and a benchmark would have dressed it up: **no code was
added to the compute lane's hot path.** `spawn_toolpath_lane`'s body is
unchanged apart from one `Option<usize>` write inside a critical section
that already existed (`inner.active_toolpath_index = Some(...)`, beside the
`active_toolpath_id` write two lines up). `request_cancel` takes the same
short `inner` mutex `submit_toolpath` already takes, and only when an MCP
cancel arrives. Everything else is on the GUI thread at ≤2 Hz. Measured
what is measurable: 10 000 publish+get round trips on the cache complete
well inside 500 ms (`read_cache_publish_and_get_are_cheap`), and the 40
worker-lane tests — which include the wall-clock cancellation assertions —
pass at their usual timings.

**The test seam, disclosed.** `send_with_progress` now takes
`Option<Peer<RoleServer>>` and `generate_all_without_peer` exposes the tool
body. An rmcp `Peer` cannot be constructed outside a live service, so the
alternative was to test the layer underneath — which is exactly what the
gate forbids. The tool method itself is unchanged and still passes
`Some(peer)`.

### A/M11 — the taxonomy, and the eleven sites

`ComputeStatus` gained `AwaitingPriorStock { blocking_toolpath_id,
blocking_toolpath_index, message }` and `Disabled`. Matches were made
**exhaustive rather than wildcarded**, so the compiler enumerated every
consumer instead of silently accepting the new variants:

`app/mcp.rs` list_toolpaths · `app/mcp.rs` runtime_status_for_toolpath_id ·
`app/mcp.rs` runtime_error_diagnostics · `controller/events/compute.rs`
build_mcp_diagnostics · `controller/events/compute.rs`
notify_mcp_toolpath_complete (single-toolpath arm) ·
`controller/events/compute.rs` generate_all accounting ·
`ui/toolpath_panel.rs` status chip · `ui/toolpath_panel.rs` quick-generate
button · `ui/toolpath_panel.rs` dep-stale probe · `ui/properties/mod.rs`
status line · `ui/workspace_bar.rs` pending badge.

Two design calls worth recording:

- **`Disabled` is derived, never stored.** `ComputeStatus::effective(enabled,
  raw)` lets `enabled: false` win over whatever the op last recorded.
  Storing it would mean deciding what to restore on re-enable; deriving it
  makes the live defect structurally impossible — a switched-off op cannot
  carry the error it had while it was on, because nobody reads the stored
  value directly.
- **A block is not an error, and the split is at the JSON boundary, not just
  in the enum.** `runtime_errors` carries genuine failures only; blocked ops
  get their own `awaiting_prior_stock` array; disabled ops appear in
  neither. That is the list an agent triages, so the separation has to hold
  where the agent reads it.

The message now names the blocker (nearest enabled upstream op in the same
setup) *and* distinguishes the two waits: blocker already generated → "ONE
simulation is enough"; blocker not generated → "the cycle may need
repeating". That distinction is the number the operator could not otherwise
know, and a sentry pins both shapes.

### A/M11 — the fixpoint loop

Semantics: round 1 generates every enabled op. A round continues only when
(a) at least one op is blocked *purely* on sequencing, and (b) the previous
round generated at least one NEW op. Between rounds the loop runs one
simulation at the caller's resolution; the next round regenerates exactly
the blocked set.

**Termination, three independent stops.** (a) genuine failures record
`Error` and are never retried, so a broken op cannot keep (a) true — this is
what makes a failing chain stop rather than spin. (b) an op reaches `Done`
at most once per call, so (b) can hold at most `enabled_count` times.
(c) a hard bound of `rest_dependent_ops + 1`, since a stock chain cannot be
longer than the number of links in it. Plus two unwind paths that would
otherwise hang: a simulation that fails or is cancelled disables the loop
and reports `loop_error`, and a simulation that could not be submitted at
all (`run_simulation_with_all` now returns whether it submitted) unwinds the
same way instead of waiting for a completion that will never drain.

Round count reporting: `{"rounds": n, "simulations": m}` on the JSON reply,
and in the headline sentence — "Generated 4 toolpaths (in 4 generate rounds,
3 simulations)".

Resolution is caller-specified or the call refuses. The refusal names the
ops that forced it, says why a default would be wrong (collision counts and
engagement both move with cell size — A/M10), says what to pass, and says
how to opt out. Nothing is submitted on that path.

### A/M11 gate results

| gate | result |
|---|---|
| 3-deep rest chain fully generated from cold in ONE call, reporting rounds | PASS — `generated: 4`, `rounds: 4`, `simulations: 3`, nothing left blocked, every op `Done` |
| terminates on a genuinely-failing op, bounded rounds, clear final error | PASS — poisoned middle link; `ok: false`, `failed: 1`, rounds inside the 1..=4 bound, downstream ops reported as *waiting* rather than broken |
| disabled rest op reports disabled | PASS — generated by nobody, in neither list, `effective(...)` → `"Disabled"` |
| resolution caller-specified, no silent default | PASS — immediate refusal, nothing submitted, message contains the parameter name, the reason, and the opt-out |
| MCP and GUI consume the same taxonomy | PASS by construction — one enum, one resolver, exhaustive matches; there is no second taxonomy to drift |

The synthetic backend models the one rule that makes the ladder necessary:
prior stock for an op appears only from a simulation that runs AFTER its
predecessor generated. Without that rule the test would pass on a loop that
does not loop.

### Verification

- `cargo fmt --check` clean; `cargo clippy --workspace --all-targets -D
  warnings` zero, before each of the four commits.
- `rs_cam_viz`: 187 lib + 9 `mcp_escape_hatches` + 11 `wizard_e2e`, all
  green. `rs_cam_mcp` 4/4.
- **The known load-flaky family bit again and was handled per the rule.**
  During the taxonomy commit's run,
  `compute::worker::tests::analysis_cancel_completes_quickly` (362 ms
  against a 250 ms wall-clock assertion) and
  `cancel_all_marks_both_lanes_cancelling` failed under a foreign
  `sysml-runtime` / `sysml-lsp-server` load. Re-run with
  `--test-threads=1`: **40/40 green**, twice. Not filed as a wave red — and
  worth noting that this wave DOES touch that module, so the re-run was a
  real check rather than a formality: `LaneControl` adds an impl beside
  `snapshot()`, and the lane loop gains one field write.
- `rs_cam_core --lib`: 2193 passed, 3 failed — exactly the three named
  known reds (`peck_plunge_progresses_when_depth_per_pass_equals_retract_clearance`,
  `rapid_segment_lifts_to_safe_z_before_traverse`,
  `planner_sim_dexel_parity_agent_search`). Nothing new.
- One cargo job at a time; `free -g` + bracketed `pgrep` before every heavy
  command. A foreign sysml loop held the machine for most of the session;
  `cargo check` was run concurrently only above the 20 GB threshold.

**Untouched, as instructed**: `planning/airrun_2026-06-01/wanaka.toml` and
`planning/review_2026-07-27/`. Every commit staged file-by-file.

### What a live re-validation gets, and what it still does not

The next wanaka run should cost one `generate_all` instead of five steps,
can watch it with `generation_status`, and can stop it. Two things this wave
deliberately did NOT fix, so nobody reads them as covered:

- **`narrate_toolpath` still runs on the GUI thread** and still cost ~12 min
  on 148 429 moves. It is now *documented* as such, and it no longer blocks
  cancel/status — but the workflow's documented first diagnostic is still a
  main-thread grind, and while it runs the cheap reads degrade to snapshots.
- **`get_toolpath_params` and the other parameterised reads** are not in the
  snapshot set. Publishing every op's params every frame is not free and the
  gate did not ask for it; they still queue behind the frame loop.

## C-SEQUENCE WAVE 5 (C6), 2026-08-02

Addendum C's fourth sequencing step: "C6 fixture library — before M4's
fixture-heavy work." Two commits plus this log entry: the shared module and
its bit-identity harness first, the opportunistic migrations second.

**Commits**

| # | Hash | Scope | Diffstat |
|---|------|-------|----------|
| 1 | `c8e40f7` | `tests/common/{meshes,tools,session,fingerprint}.rs` + `common_fixtures_smoke_c6.rs` | 6 files, +1224 / -0 |
| 2 | `1cd6eee` | Opportunistic migrations: C2 plateau, Checkpoint A, PR-3 fingerprints, A/M9 | 4 files, +102 / -327 |

### What was actually duplicated

The survey, before writing anything:

| fixture | copies | how they differed |
|---|---|---|
| `grooved_block` | 5 (`checkpoint_a_valley_matrix`, `reach_policy_pr4`, `pencil_tip_float_channel_d1`, `coverage_routing_pr5`, `generic_rest_routing_pr7`) | X-sampling window (±3/±4/±5 mm), density (0.05/0.1), breakpoints forced or not, symmetric vs skewed walls — four knobs, one body |
| FNV-1a-over-`Debug` fingerprint | 4 (`transform_provenance_fingerprints`, `checkpoint_b_resolution_ab`, `crease_own_region_pr6b`, `finish_resolution_policy_pr3`) | return shape only: `(len, hash)` vs `hash` |
| 17-field `ToolpathConfig` literal | 30 files | nothing structural — it has no `Default` |
| Ø1/7°/Ø6 taper + Ø3 ball control | 3 + 2 | nothing; identical constructors under three names |
| height-field mesher | 2 (`checkpoint_b`, `standing_material_channel_am9`'s sawtooth) | grid extents and step; winding and vertex order identical |

### Placement decision

`tests/common/mod.rs` consumed via `mod common;`. Cargo builds one test
binary per `.rs` file *directly* under `tests/`, so a directory module is
never a target of its own. This is the stock convention AND already what 13
files in this suite do — C6 grew the existing module rather than introducing
a second mechanism. The `#[path]` include style used by `literature_matrix/`
was rejected on purpose: that splits ONE harness's private internals, which
is a different job from sharing fixtures across binaries.

### The hazard, and the guard

"Generalise" quietly becoming "subtly change" is the whole risk here.
Several consumers pin toolpath fingerprints computed over these meshes, so a
vertex differing in the last ulp moves a constant nobody can later
attribute.

`common_fixtures_smoke_c6.rs` carries the DONOR implementations verbatim and
asserts the shared versions reproduce them vertex-for-vertex (`to_bits()`
comparison, not epsilon) and triangle-for-triangle, at every parameter set
the five donor call sites use — **including the three groove copies not yet
migrated**. So when `reach_policy_pr4`, `pencil_tip_float_channel_d1`,
`coverage_routing_pr5` and `generic_rest_routing_pr7` eventually migrate,
their meshes are already pinned and the migration is a no-op by
construction.

Unifying the symmetric and skewed groove bodies is the one non-obvious step:
`skew(1.0)` makes `tan_r == tan_l` and `floor_r == floor_l`, and the two
branch structures then agree at every boundary including `x = ±rim` and
`x = 0`. That is asserted, not argued
(`symmetric_default_and_unit_skew_agree`), and was additionally cross-checked
in an independent Python reimplementation of both donor bodies before the
Rust was compiled — all 8 parameter sets produced identical sample vectors
and identical Z values.

### API style

Small functions with sensible defaults; overrides via Rust's own
struct-update syntax and one `FnOnce(&mut ToolpathConfig)` hook. No config
megastruct. The `ToolpathConfig` case is the point of the item: the literal
now lives in one place and callers spell `..toolpath_config(...)`, which is
inherently field-addition-proof — the exact pain CLAUDE.md's "if GUI state
adds a field, audit test initializers" note describes. `GroovedBlock` is the
only builder-struct, because it genuinely has four independent knobs. Every
fixture's docstring names its reference consumers.

### Migration policy, honoured

Four tests migrated as proof of generality; **the other ~26
`ToolpathConfig` literal sites and the three remaining `grooved_block`
copies were left alone**, per the plan's binding "opportunistically, never
in bulk". A sentry's value is that it has not changed.

| test | what it took | why this one |
|---|---|---|
| `grid_z_uncovered_contract_c2` | `plateau`, `ball_cutter` | the plan named it |
| `checkpoint_a_valley_matrix` | `grooved_block` (0.1 mm variant), `wanaka_taper`, `steep_taper`, `ball_control` | the checkpoint-harness consumer |
| `finish_resolution_policy_pr3` | the fingerprint + the taper, imported UNDER THEIR OLD NAMES | it pins three fingerprint constants — the strongest possible proof |
| `standing_material_channel_am9` | all of it | it IS the template the whole item is about |

**Assertions and pinned constants unchanged in all four.** PR-3's three
constants — `(1318, 4897619324930985607)`,
`RAMP_FINISH_GEO_MEAN_FINGERPRINT = (277, 18_231_352_062_362_901_444)` and
`(913, 14129959905444107510)` — are byte-for-byte where they were, and they
pass: same mesh, same tool, same hash. Test names are unchanged throughout.
The only `assert`/constant lines anywhere in the migration diff are
*deletions* of extracted fixture internals (the groove's own "groove is a V,
not a trapezoid" guard, and the FNV basis/prime), both of which moved into
the shared module.

Importing `move_fingerprint as fingerprint` and `wanaka_taper as taper` in
PR-3 was deliberate: zero call-site churn keeps the diff readable as "the
helper moved", with nothing else able to hide inside it.

### Out of scope, explicitly

`v3_cascade_ab.rs` and `p2c_headless_ab_wanaka.rs` were not touched — that
is C7.

### Verification

- `cargo fmt --check` clean and `cargo clippy --workspace --all-targets -D
  warnings` zero, before each commit. rustfmt was run on the two new files
  individually and `git status` checked afterwards for the known
  sibling-cascade behaviour — no unrelated file moved.
- Migrated targets, each run individually: `common_fixtures_smoke_c6` 8/8,
  `grid_z_uncovered_contract_c2` 6/6, `finish_resolution_policy_pr3` 10/10,
  `standing_material_channel_am9` 4/4, `checkpoint_a_valley_matrix` 14/14.
- `rs_cam_core --lib`: 2193 passed, 3 failed — exactly the three named known
  reds (`peck_plunge_progresses_when_depth_per_pass_equals_retract_clearance`,
  `rapid_segment_lifts_to_safe_z_before_traverse`,
  `planner_sim_dexel_parity_agent_search`). Nothing new.
- Blast radius, covered: this wave changes **test code only** — no
  production file is touched. The one file existing tests already depend on
  is `tests/common/mod.rs`, and its diff is four additive `pub mod` lines
  plus documentation; not one existing helper changed. `cargo clippy
  --workspace --all-targets` compiles every one of the 13 `mod common;`
  consumers, so the additive change is proven not to break any of them.
- **A full `cargo test -p rs_cam_core --no-fail-fast` was started and
  deliberately stopped, and that is recorded rather than papered over.** It
  cleared `--lib` plus 8 integration targets green, then sat on
  `adaptive3d_interior_cell_parity_f029` for ~25 minutes with no output.
  Rather than assume that was pre-existing slowness, the target was re-run
  on its own: **2/2 green in 581 s** (~9.7 min), with the harness itself
  printing "has been running for over 60 seconds" for BOTH of its tests. So
  it was slow, not hung, and it passes — it is an AS013 sentry driving
  `ProjectSession::run_simulation` over the 10 MB `terrain.stl`, i.e. minutes
  by construction, and the earlier 25 minutes was it plus whatever else the
  `--no-fail-fast` run was executing in parallel. It consumes only
  `common::repo_root`, untouched by this wave.
  This is the hazard CLAUDE.md documents ("avoid workspace-wide `cargo
  test`, it can loop on this repo"): anyone re-validating should prefer the
  per-target list above to a whole-crate run, and should expect ~10 min for
  this one target alone.
- One cargo job at a time. A foreign `sysml-lsp-server` test held the machine
  for most of the session (its test binary alone was 11 GB resident);
  `free -g` + a bracketed `pgrep` gated every command, and the first
  `cargo check` was deferred until the machine crossed the 20 GB threshold.
  The waiting time was spent on the Python cross-check above rather than on
  running anything.

**Untouched, as instructed**: `planning/airrun_2026-06-01/wanaka.toml` and
`planning/review_2026-07-27/`. Every commit staged file-by-file.

### What this does and does not buy

It removes a recurring per-wave tax and, more importantly, removes the
*silent* version of it: the next field added to `ToolpathConfig` breaks one
builder instead of thirty initializers. What it does NOT do is make the
remaining copies safe by fiat — three `grooved_block` copies and ~26
`ToolpathConfig` literals are still out there. They are pinned in advance by
the smoke harness where meshes are concerned, and the policy is deliberately
lazy: they migrate when someone is already editing them.

---

## C-SEQUENCE WAVE 6 (A/M6), 2026-08-02

C-sequencing step 5: `claims_reference` — the shipped-default footgun that
made every cascade measurement re-cut the whole part. Three commits plus
this entry.

**Commits**

| # | Hash | Scope | Diffstat |
|---|------|-------|----------|
| 1 | `5c24d62` | core: `ClaimsReference` dial + `ClaimsReferenceResolution` provenance + `ClaimsReferenceFinding` through `GenerationFindings`/`ToolpathStats`/diagnostics; deser-compat matrix | 11 files, +609 / −38 |
| 2 | `21edaea` | GUI "Rest Claims" block + resolved readout; MCP `runtime.claims_reference`; catalog enum widened | 6 files, +221 / −9 |
| 3 | `486364f` | `tests/claims_reference_cascade_am6.rs` — cascade gate (ball + taper), no-silent-fallback both directions, diagnostic halves | 1 file, +686 |

### The defect, and what actually drives it

§14p measured the symptom: 46 366 mm of cutting under `self_probe` against
5 259 mm under `machined_stock`, one variable, 0.1 mm sim, after a full
same-tool finish. The mechanism is narrower than "the analytic reference is
wrong", and finding it changed what needed fixing:

`territory_clip` — the dial that turns a `UnifiedFinish` into a rest pass by
masking already-finished territory out of coverage before `decompose` — is
**gated on the resolved reference being `MachinedStock`**. Under a
self-probe reference the orchestrator logs a `tracing::warn!` and skips the
clip. So the reference dial does not merely mis-measure rest; it silently
switches the operation from a rest pass to an all-over pass. That is why the
numbers came out at ~100% of the finish pass rather than merely inflated,
and it is what the sentry asserts.

Second finding, worth recording because it bounds the blast radius:
`claims_reference` is **inert unless `pencil_claims` is on**, and that dial
defaults `false` (`claims_cfg = cfg.pencil_claims.then(...)`). The 2026-07-30
live-validation section above reports flipping `claims_reference` on wanaka
op 8 while `pencil_claims: false` — that flip changed nothing, and op 8's
62 703 mm is not comparable to §14p's numbers for that reason as well as the
one already noted there.

### The fix: derive, do not re-default

The dial is now three-valued (`ClaimsReference`: `auto` | `self_probe` |
`machined_stock`) and resolves at generation time to the two-valued
`CreaseReference` the detector switches on. Keeping "decide later" out of
the type a detector matches against is the point — `ClaimsConfig` still
takes a resolved value only, and `generate_unified_finish` is the single
bridge because it is the only site that sees both the operator's dial and
what is in scope.

`ClaimsReferenceResolution` carries the provenance, following the
`CellSource` / `FinishResolutionPolicy` precedent (an enum whose variants ARE
the record, not a bare bool). Six variants for the six
(setting × prior-in-scope) combinations, `resolve()` as the only mapping
site, and `reference()` / `setting()` / `prior_stock_in_scope()` /
`is_derived()` / `needs_attention()` / `label()` / `why()` on top. A
`resolve` table test asserts the mapping is total and that the three inputs
round-trip out of every outcome — including that nothing can report a
machined-stock reference it does not have.

### Default decision: `Auto`, and why not "keep `self_probe` and warn"

The plan offered both. `Auto` was taken:

- The right value is **contextual** — defensible for a first finish op,
  indefensible for a rest op in a cascade — so any fixed default is wrong
  half the time by construction. Warning-only leaves the wrong toolpath
  shipped and asks the operator to fix what the system already knows.
- The blast radius is small and bounded. With `pencil_claims` off the dial
  is inert, so the only projects whose behaviour can move are those that
  turned claims ON **and** left `claims_reference` unwritten **and** cut
  remaining stock — exactly the A/M6 case.
- Deserialization compatibility is binding and pinned:
  `unified_finish_claims_reference_serde_round_trip_and_backcompat` holds
  all three cells. `"self_probe"` → pinned self-probe, `"machined_stock"` →
  pinned machined stock, **field absent** → `Auto`. Wire names are the
  pre-A/M6 ones, so no project file needs migrating and no operator's
  written value changes meaning. Only the absent case moved, and it moves
  visibly (below).

**Stated limitation, not hidden.** `Auto` keys on the PRESENCE of a prior
stock, not its QUALITY. On a rough→finish chain the S1 lesson still holds
(a stock-referenced detector reads roughing terraces as a phantom dendritic
crease network) and the operator should pin `self_probe`. The derived
finding's own sentence says so, and a sentry asserts the word `ROUGHING`
survives into the operator-facing text.

### No silent fallback, in either direction

Every resolution — footgun, degraded, or uneventful — is recorded on
`GenerationFindings` → `ToolpathStats::claims_reference` (session path AND
the GUI worker's parallel copy) and rendered as `config.claims_reference`:

| outcome | severity | why it is said |
|---|---|---|
| `ExplicitSelfProbeOverridingPrior` | Caution | the A/M6 footgun in its exact shape |
| `ExplicitMachinedStockWithoutPrior` | Caution | a dial that could not be honoured |
| any resolution with `territory_clip` requested and skipped | Caution | a rest pass silently became an all-over pass |
| everything else | Info | which of two fields the detector read is derivable from nothing else |

The pre-existing `tracing::warn!` stays as a backstop. It was never the
signal: nothing in the GUI installs a subscriber, which is the same "a
warning nobody sees is not a warning" rule the v3 closure left standing.

### A/M11 integration, not duplication

The brief's case 3(b) — Auto wants machined stock, none available — is
**not** given a parallel signal, because the taxonomy already covers it. An
op cutting `FromRemainingStock` with no snapshot never reaches generation:
`generate_toolpath` refuses it before any geometry work and the GUI records
`ComputeStatus::AwaitingPriorStock`, which wave 4 built and whose
`generate_all` fixpoint loop resolves the ladder. What *does* reach
generation with nothing in scope is an op that asked for **fresh** stock —
a configuration statement, not a block. It resolves to
`DerivedSelfProbeNoPrior` and its `why()` names both remedies (set the stock
source; if that leaves it blocked, the ladder message takes over). A sentry
pins this reading, and the sentry's own docstring records that it is
deliberately not a second signal.

### Surfaces

- **GUI**: the four claims dials had **no widget at all** before this — the
  panel exposed none of `pencil_claims`, `claims_reference`,
  `territory_clip`, `min_rest_depth_mm`. New "Rest Claims" block with all
  four, a three-way combo with per-option hover text, and a readout of what
  the LAST generation resolved to (amber + glyph when it needs attention or
  the clip was skipped, green tick otherwise, `why()` on hover). Plus a
  panel-local notice when claims are on over FRESH stock, because `Auto`'s
  input is the stock source and that lives in a different section of the
  same panel.
- **MCP**: `get_toolpath_params` gains `runtime.claims_reference` (setting /
  resolved / resolution token / derived / prior_stock_in_scope /
  needs_attention / territory_clip_requested / territory_clip_skipped /
  why). Under `runtime`, not `params`, deliberately — it is not a param.
- **Catalog**: `enum:auto|self_probe|machined_stock`.
- **Setup sheet**: audited per CLAUDE.md. `io/setup_sheet.rs` enumerates no
  operation params, so no change. Project IO round-trips through the
  config's own serde and no wire name moved.

### The sentry, and the three fixtures that lied first

`crates/rs_cam_core/tests/claims_reference_cascade_am6.rs`, 8 tests, ~35 s.
Built on C6's shared library rather than hand-rolled. Measured, printed on
every run:

| tool | finish | rest, `self_probe` | rest, derived | control (gate < skin) |
|---|---|---|---|---|
| ball Ø3 | 3209.5 mm | **3209.9 mm** | 0.0 mm | 160.0 mm |
| taper Ø1 tip / Ø6 shank | 3290.5 mm | **3290.7 mm** | 822.7 mm | 822.7 mm |

The red-first number is column 3: the "rest" pass re-cuts the finish pass it
follows to within 0.4 mm in 3200 — 100.0%, on both tool shapes. §14p's
46 366-vs-5 259, reproduced on a 40 mm fixture. That assertion runs BEFORE
the gate, so a fixture that stops reproducing the defect fails loudly rather
than passing a gate that then proves nothing.

Gate held at `derived < 0.35 × finish` and `derived < 0.35 × forced`
(≥ −65% from one dial). The bound is 0.35 and not tighter for a reason worth
recording: part of what ANY correctly-referenced rest pass finds is not
gate-driven. Untrusted classification cells keep their coverage by design
(the detector's erosion rim must never amputate band area), and the model
sits inside a 2 mm stock margin no finish pass ever machined, which is
genuine rest. The taper keeps more of it — 25% vs the ball's 0% — because
its cusp-derived conditioning is finer. Both are a rest pass doing rest
work; neither is the defect.

**Three fixture versions produced a GREEN sentry measuring nothing**, and
each is recorded in the file so the next author does not re-earn them:

1. Two ops cutting to the same surface leaves the rest pass no material
   anywhere; the air-cut filter erases every move and both references read
   0.0. Fixed with a uniform sub-gate skin — the small fixture's stand-in
   for wanaka's 22 µm raster cusps.
2. `common::session::stock_over` roots the block at `origin_z = 0`, so a
   surface built around `z = 0` sits entirely BELOW the stock. Generation
   still emitted a full pass (fresh-stock generation never consults stock),
   the simulation removed nothing, and the rest op was filtered to four
   moves. The F-024 dexel-frame trap arriving from the test side.
3. `overlap_mm` at its 2.0 default dilates every extracted region enough to
   swamp the confinement (derived read 27% of finish, and the control arm
   was indistinguishable). At 0.5 the mechanism is visible. Held equal
   across arms either way.

The **control arm** exists because a confined pass and an emptied pass look
identical from one number: same machined-stock reference, same machinery,
gate moved BELOW the skin, must still emit real work.

### Gates

- `cargo fmt --check`: exit 0 before each commit.
- `cargo clippy --workspace --all-targets -- -D warnings`: exit 0 before
  each commit. One file-scoped `#[allow(clippy::print_stderr)]` on the new
  sentry, justified in place: the measured numbers ARE the record, and
  printing them makes a regression readable in CI output rather than only as
  a threshold that tripped.
- `cargo test -p rs_cam_core --lib`: 2194 passed, 3 failed — exactly the
  three known adaptive3d reds
  (`peck_plunge_progresses_when_depth_per_pass_equals_retract_clearance`,
  `rapid_segment_lifts_to_safe_z_before_traverse`,
  `planner_sim_dexel_parity_agent_search`). No new red.
- `cargo test -p rs_cam_core --no-fail-fast` (whole crate, ~50 min):
  **106 targets green**, 2 failed — the `--lib` target with exactly the three
  reds above, and `wanaka_suggest_integration::wanaka_suggest_baseline`, the
  known intermittent/environmental one. No new red anywhere in the
  integration suite.
- `cargo test -p rs_cam_viz`: 227 + 9 + 11 passed, 0 failed.
- `cargo test -p rs_cam_cli`: 5 + 9 passed. `cargo test -p rs_cam_mcp`: 4 passed.
- New target `claims_reference_cascade_am6`: 8 passed, 0 failed, ~35 s.
- One cargo job at a time; `free -g` + bracketed `pgrep` before each heavy
  command. A foreign `sysml` workspace test held the machine for much of the
  session; `cargo check` was allowed to overlap only above the 20 GB
  threshold, per the relaxed rule.

**Untouched, as instructed**: `planning/airrun_2026-06-01/wanaka.toml`
(user-modified) and `planning/review_2026-07-27/`. Every commit staged
file-by-file.

### What this unblocks, and one thing it does not

A/M6 was H4's dependency and the second required step of the re-measurement
sequencing: cascade measurements taken through the analytic reference were
measuring an all-over pass. They can now be re-earned.

What it does NOT do is make `Auto` correct on a rough→finish chain. That
needs the reference to know the QUALITY of the prior stock, not just its
presence, and nothing in `ExecutionContext` carries it — the signal would
have to come from the session's view of the upstream chain. Deliberately not
built here: the plan's fix shape asked for presence, and inventing the
quality channel would have doubled the blast radius of a default flip. The
limitation is stated in the enum doc, in the operator-facing sentence, and
here.

---

## C-SEQUENCE WAVE 7a (M3 research), 2026-08-02

Addendum C's next sequencing step: M3 "optimise classification without
changing its answer" — **research phase only**. This wave produces the
baseline instrument, the candidate prototypes behind a test-only strategy
enum, the equivalence harness and the study. **No production path moved**:
`finish_setup::build_classification_surface_with_policy_and_cancel` still
calls the tiny-ball drop-cutter unconditionally, and step 6 of the M3 fix
sequence is deliberately NOT ticked — it completes when an implementation
lands.

**Commits**

| # | Hash | Scope | Diffstat |
|---|------|-------|----------|
| 1 | `efabeec` | `benches/classification.rs` + the `[[bench]]` entry | 2 files, +217 / −0 |
| 2 | `05cac97` | `src/classify_probe.rs`, `SpatialIndex::query_into`/`QueryScratch`/`cell_triangles_at`, `tests/classification_strategy_m3.rs`, 3 fixtures in `tests/common/meshes.rs` | 6 files, +2123 / −0 |
| 3 | `6c73810` | `CLASSIFICATION_PERF_STUDY.md` + this entry | |

Deliverable: `planning/review_2026-07-29/CLASSIFICATION_PERF_STUDY.md`.

### The headline

At the 849² production row on the 10 MB terrain fixture (214 997 triangles),
release, machine quiet, one arm at a time:

| arm | wall s | CPU s | speed-up |
|---|---|---|---|
| shipped tiny-ball drop-cutter | 2.990 | 40.70 | 1.0× |
| candidate 0 — same math, hoisted allocations | 3.034 | 40.77 | **0.99×** |
| candidate 1 — triangle raster (single-threaded) | 0.053 | 0.05 | 56× |
| candidate 2 — vertical ray | 0.025 | 0.43 | 120× |
| candidate 3 — tile raster | **0.016** | **0.13** | **187×** |

M3's bar was 2×. Recommendation: **candidate 3**, conditional on a
Checkpoint-B-shaped human decision, because it changes the classifier's
answer (below).

### A hypothesis pre-registered and refuted

`SpatialIndex::query` allocates a mesh-sized dedup bitset per call, and the
classifier issues two queries per cell — 38.8 GB of zeroed memory per 849²
terrain classification. Candidate 0 exists to test whether that is the
bottleneck. **It is not**: 0.99×. At the probe's 0.025 mm radius against
0.61 mm index cells virtually every query lands in one index cell, so the
bitset is barely touched; the criterion `query_only` row puts the entire
query cost at ~a fifth of the classifier's CPU, and the drop-cutter contact
math at the rest. Recorded in the study rather than dropped, because it is
the reason the recommendation is not "tidy the allocations" — there is no
version of that change that reaches 2×.

### Why the fast arms are not a free swap

The direct arms sample the model surface; the shipped classifier samples the
**CL surface of a Ø0.05 mm ball**, which sits `R·(1 − n.z)/n.z` above it —
10 µm at 45°, 119 µm at 80°. That offset is slope-DEPENDENT, so it does not
cancel in a gradient. Against a `cusp/4` = 0.125 mm cell it moves 1.8–4.3%
of cells across a band boundary; on terrain at 849² it costs 5 mid-steep
components and gains 7 more plus 13 very-steep ones.

`direct_arm_divergence_is_the_probe_offset` settles the attribution instead
of arguing it: shrink the probe 10× and label disagreement goes to **zero on
every fixture**, while max Z difference scales exactly 10× per 10× of radius
(0.191900 → 0.019190 → 0.001919). The direct arms are not approximating the
surface — they are the surface, and the divergence is the shipped
classifier's own artefact. Which also means `finish_setup.rs`'s standing
"the probe's own offset is negligible at finish cell sizes" is measured
**false** at the resolution production actually uses.

That makes the switch a product decision, not a performance one, so the
study stops at a recommendation and hands the decision on. §8/§9 of the
study spell out the sequencing and the parity gates, including the one this
wave did not build: an end-to-end `UnifiedFinish` A/B on wanaka scored on
COLUMNS, not on cell counts. *Never gate on an aggregate without rendering
the surface.*

### Instrument defects found and fixed inside the wave

1. **The first release timing run was invalid.** It ran under cargo's
   default two test threads; `cpu_time()` reads process-wide
   `/proc/self/stat` and rayon gave both tests the same 24 cores, so the two
   timing tests measured each other — CPU/wall of 14–25× alongside
   sub-millisecond walls for arms that cannot be that fast. Numbers
   discarded and re-taken; the harness now holds a `TIMING_LOCK` so the
   mistake cannot recur silently, and the study says so in §4.
2. **A real lattice bug in the raster arm**, caught by the cross-arm check:
   tight `ceil`/`floor` bbox bounds skipped cells whose centres were a
   fraction of an ulp inside a triangle. Now rounds outward one cell and
   lets `contains_point_xy` be the only filter.
3. **Bit-identity between the direct arms is unachievable, for a legitimate
   reason.** At a cell centre on an edge shared by two triangles — all 386
   profile-breakpoint cells of the grooved block — both contain the point
   and evaluate the same height to ±0.0 or one ulp, and the scatter and
   gather arms break the tie in different visit orders. The plan's
   "deterministic ties" case. Each arm is internally deterministic; the
   assertion is now "equal to 1 pm AND label-identical", which is still
   tight enough to have caught (2).

### Gate design: red evidence turned into pinned characterisation

Candidates 1–3 genuinely fail M3's "no loss of narrow steep regions" gate.
Leaving that as a red test would have left a permanent red in the suite;
deleting it would have thrown away the finding. Both were rejected. The gate
now applies to the arm that CLAIMS answer preservation, and the direct arms'
divergence is pinned as numbers
(`direct_arms_redistribute_steep_territory_on_the_mixed_slope_fixture`
asserts the 2 lost mid-steep regions with a ceiling, and the matrix asserts
the label divergence stays under 5%), so a future change that makes it worse
still fails.

Per §A.0 and P7, verdicts are read off the **label grid**: per-class cell
counts, per-cell disagreement, and 4-connected components read twice — raw
and with a minimum component size, because raw component count is
scale-sensitive for band-shaped classes. Area appears nowhere as a gate.

### Gates

- `cargo fmt --check`: exit 0.
- `cargo clippy --workspace --all-targets -- -D warnings`: exit 0.
- `cargo test -p rs_cam_core --lib classify_probe`: 6 passed, 0 failed.
- `cargo test -p rs_cam_core --test classification_strategy_m3
  -- --test-threads=1`: 9 passed, 0 failed, 2 ignored (22 s).
- `cargo test -p rs_cam_core --release --test classification_strategy_m3
  -- --ignored --test-threads=1`: 2 passed, 0 failed (10 s).
- `cargo bench -p rs_cam_core --bench classification`: runs; heavy rows
  correctly skipped without `RS_CAM_M3_HEAVY`.
- One cargo job at a time. A foreign `sysml` workspace test held the machine
  for the first ~2 h; every measurement was taken after `pgrep` came back
  clear. Swap was full (8/8 GiB) all session — noted in the study rather
  than hidden.

**Untouched, as instructed**: `planning/airrun_2026-06-01/wanaka.toml`
(user-modified) and `planning/review_2026-07-27/`. Every commit staged file
by file.

### What this unblocks, and what it does not

The implementation wave now has a winner, a measured 187×, a proven
attribution for the only thing standing in its way, and eight parity gates
to pass. What it does NOT have is permission: the classifier's answer
changes, and §8 routes that to a human the same way Checkpoint B did.

---

## C-SEQUENCE WAVE 8 (C3+C4+C8), 2026-08-02

Addendum C step 7 — the consolidation/diagnostics bundle that feeds H4's
oracles. Ten commits in three clusters. Every item found something the
backlog had not: three of the eleven entries were **wrong about the
defect**, and saying so is most of what this wave is worth.

**Commits**

| # | Hash | Cluster | Scope |
|---|------|---------|-------|
| 1 | `f792fe9` | C3 | `feeds/geometry.rs`, `feeds/mod.rs`, `session/mod.rs`, `tests/tapered_width_model_parity_c3.rs` |
| 2 | `92eaea1` | C3 | `waterline.rs`, `finish_setup.rs`, `steep_shallow.rs`, `tests/waterline_shared_finish_setup_c3.rs`, `ANTIPATTERNS_BACKLOG.md` |
| 3 | `c44a2be` | C3 | `rs_cam_cli/src/project.rs` |
| 4 | `ad445f3` | C4 | `toolpath_spans.rs`, `compute/spans.rs`, `compute/annotate.rs`, `compute/execute.rs` |
| 5 | `b514046` | C4 | `unified_finish.rs`, `p2c_headless_ab_wanaka.rs`, `v3_cascade_ab.rs` |
| 6 | `263ba5d` | C4 | `semantic_trace.rs` + 110 call sites across 8 files |
| 7 | `6d134c8` | C8 | `compute/{execute,config,stats}.rs`, `session/compute.rs`, `from_generation.rs`, 2 tests, viz worker |
| 8 | `c1db76f` | C8 | `unified_finish.rs`, `config.rs`, `narrate.rs`, `diagnostics/*`, `tests/unified_finish_partial_clip_finding_c8.rs` |
| 9 | `f5925f8` | C8 | `compute/annotate.rs`, `scallop.rs`, `narrate.rs`, `tests/narrate_regions_closed_c8.rs` |
| 10 | `ef9dfec` | C8 | `ramp_finish.rs`, `measurement.rs`, `narrate.rs`, `from_generation.rs`, `tests/ramp_reach_clamp_pr8b.rs` |

### C3 — one implementation per concept

**The tapered width models: "~5% off" was generous, and the model was dead.**
The parity sentry was written first, as the plan required, and measured the
straight cone against `MillingCutter::width_at_height` over four shipped
taper geometries × seven DOCs at the binding every production call site
produces (`tip_r == nominal_d / 2`):

| | divergence | where |
|---|---|---|
| worst overstatement | **+290.6%** | Ø3 tip / 15° taper @ 0.05 mm DOC |
| worst understatement | **−44.1%** | Ø0.5 tip / 3° taper @ 4.00 mm DOC |
| missing tangency alone, clamp removed | **+8.75%** | Ø1 tip / 5.26° @ 0.5 mm |

The sign flips at the tangency height, and "~5%" described only the
neighbourhood where the two error sources cancel. Worse, the growth term was
CLAMPED DEAD: `(nominal_d + 2·ap·tan α).clamp(0.01, nominal_d)` is the
constant `nominal_d` for every `ap ≥ 0`, so a tapered ball was fed as if it
engaged its full tip diameter at any depth while a plain ball of the same
tip got the exact contact circle. The function's own unit test passed
`nominal_d = 6.0` with `tip_r = 0.5` — a binding nothing produces — so it
exercised the one unclamped branch and read healthy.

**Verdict: UNIFY, not document.** There was no deliberate approximation to
document. The one caller now delegates to
`ToolGeometryHint::engaged_diameter_at_doc`, which is the same geometry as
the cutter trait and was already sentried against it. Three models → two,
and the two are one math on two carriers.

**Behavioural delta, stated not buried.** Suggest's recommended feed for a
Ø1-tip / 5.26° taper, GenericSoftwood, Shapeoko VFD, scallop finish at
0.1 mm WOC: **+26.3% at 0.05 mm DOC, +25.8% at 0.10, +8.4% at 0.25, +0.2%
at 0.50.** A feed INCREASE on shallow tapered-ball finishing. Bounded —
reached only through the Suggest button, so no project and no generated
toolpath moves on its own — and not a tuning override, because the old
number was identical at every depth. **A human should look at this.**

*Not fixed, stated*: below tangency the tapered tool and its ball twin still
land on different chipload rows (1515 vs 2514 mm/min @ 0.05 mm), because
`engaged_diameter_at_doc` selects the vendor-LUT row at the ENGAGED diameter
for tapered/V but at NOMINAL for flat/ball/bull. Pre-existing policy,
deliberately untouched, and now measured so the residual is attributed
rather than assumed to be the same defect.

**Waterline: two of the three claims were real, one was a year-old lie.**

* The Z-ladder copy was real. `waterline_z_levels` was `finish_setup::
  z_ladder`'s `snap_to_bottom = false` arm with its epsilon hard-coded. Now
  an adapter naming `WATERLINE_LADDER_EPSILON` and delegating.
* **The `execute.rs` slope-sentinel copy does not exist and has not since
  `4b105da`** — the very commit that created `finish_setup.rs` migrated it in
  the same diff. `finish_setup.rs`'s module header and its
  `slope_filter_active` doc comment have both said "out of scope for this
  pass" for a year about work that was already done, and the backlog copied
  them. All three corrected.
* PR-8d's floor, red-then-green on the Checkpoint B mixed-slope ribbon:

  | | segments | below the 0.001 mm quantum | shortest | total |
  |---|---|---|---|---|
  | before | 942 | 2 | 0.000891141 mm | 1224.9001 mm |
  | after | 940 | 0 | 0.05745541 mm | 1224.9001 mm |

  **§8.1's attribution is REVERSED.** `0.000891` is Checkpoint B §8.1's
  number bit for bit, and §8.1 attributed it to `SteepShallowSplit::steep`.
  That is where it was OBSERVED. It is MADE in `waterline_contours`, which
  the steep half calls — so PR-8d filtered the offenders out of the
  steep/shallow output while every direct waterline op kept shipping them.

**The CLI diagnostic.** It cannot BE the core struct (extra fields, and two
wire keys existing scripts read), so it became a borrowing serde view whose
only constructor EXHAUSTIVELY DESTRUCTURES the core diagnostic with no `..`.
A new core field is now a compile error in the CLI until someone decides
whether to publish it — the mechanism D3 lacked when it fixed five
silently-missing fields by hand. Two derivations retired (`tool_name`
lookup, and the name/op-type/stats reads); two stay CLI-local and say why in
their field docs. **Wire byte-stability proven twice**: a pinned exact-JSON
sentry, and mechanically against `HEAD` — the pre-C3 struct's field list in
declaration order diffs EMPTY against the pinned key list, 17 keys, same
order, same names.

### C4 — typed vocabularies

**Drill nesting: two variants, not the one the backlog asked for.**
`annotate_drill_spans` rebuilt the entire hole→peck hierarchy from
`label.starts_with("Hole ") && !label.contains("plunge")`. Naming only the
child would have left "a hole is a `GeneratorPass` in a drill operation" as
an unwritten rule a generic consumer cannot apply, so `DrillHole` and
`DrillPeck` are both explicit. `RegionSpanRole` gains the `ALL`/`from_key`
pair `SpanKind` has carried since C1; `Span::has_region_role` folds in the
boundary-and-kind guard the label predicate did by accident. MCP wire:
drill spans now read `drill_hole`/`drill_peck` where they read
`generator_pass` — produced at one site, consumed nowhere.

**The mega-harness keys.** Both spelled out strings that
`RegionKind::span_label` writes. `RegionKind` gains `ALL` and
`from_span_label`, and the two sites ask it. A payload field was declined
deliberately: putting `RegionKind` on `SpanPayload::Region` would drag
`FinishBand` and the finishing module into `toolpath_spans`, which has no
finishing dependencies. So the string stays, one place knows it, and a
round-trip sentry makes a label change break LOUDLY there instead of
silently at each consumer. The v3 migration is proven value-preserving:
`from_span_label(..).map(|k| k.strategy().label())` reproduces the retired
four-arm table exactly, `None` for `Ring N` included.

**`SemanticKey`.** 74 key literals across four production files, with
`narrate.rs` carrying its own copies of six for reading. A typo on either
side produced a silently absent parameter — and `insert` already swallows
serialisation failures, so nothing surfaced. 110 call sites migrated; zero
key literals remain at any producer or consumer. The wire is pinned as 74
literal strings **transcribed by hand on purpose** — deriving the list from
`as_str` would assert the enum equals itself.

H4's constraint is recorded in three doc comments (`SemanticKey`,
`RegionSpanRole`, `RegionKind::from_span_label`): mix tables must be built
on roles or keys, never labels.

### C8 — findings representation

**The derived-stepover slot.** Two sites derive a stepover on one toolpath;
the second was discarded without trace. Now a `Vec` on both
`GenerationFindings` and `ToolpathStats`; `GenerationFindings` loses `Copy`
and moves to `RefCell`, and the `Box` goes with the `Option`. The diagnostic
adapter fans out one per NOTEWORTHY derivation, so an unremarkable first can
no longer silence a noteworthy second. **The PR-6a rationale was orphaned**:
its `///` block ran into the next with no blank line, so the whole
first-writer-wins justification was attached to `record_ramp_reach_clamp`
and `record_derived_stepover` had no doc at all. A rule nobody could find is
most of how it survived.

**Partial height clipping.** Wave D1 already MEASURED the clip on every band
and threw it away unless the region emitted nothing, reasoning that a band
which still cuts would bury the total collapse. That is an argument about
SEVERITY. Measured on `two_groove_plateau` with `bottom_z = −4.0`:
**185.98 mm² of the VerySteep band laddered 5 of its 9 levels and stopped,
leaving 4.311 mm of an 8.31 mm groove wall unfinished** — generation
succeeded, the toolpath cut, and every surface said nothing. `DroppedBand`
(Caution) and `ClippedBand` (Info) now travel in separate collections,
disjoint by construction, with a gate driving the Auto/Auto arm to prove it.
`BandHeightClip` carries requested-vs-delivered Z BOUNDS, not only level
counts, because bounds are what an operator can act on. *Not fixed, stated*:
only the `VerySteep` arm measures a clip at all — MidSteep and Shallow never
set one, so a partial clip there is still invisible. Recorded on the finding
type itself.

**`regions 0` — three different causes wearing one symptom.**

| op | what it actually has | now |
|---|---|---|
| Scallop | a REAL partition — an independent ring cascade per boundary region (P2.3) — dropped in transit, because the event carried only a global ring index | `regions 1, rings 14` |
| SpiralFinish | no partition: one continuous traversal that SKIPS out-of-region samples | `regions 1, rings 19` |
| Trace | a NAMING divergence — 15 families route the same structural spans to `Region`, Trace to `Chain`, and narration's line had no chain counter, so a 40-contour engraving read as no structure at all | `regions 1, chains 2` |

Inventing a per-boundary partition for the latter two would have reported a
structure the generator does not have. `1` is a measurement; `0` was an
absence.

**Two `Region` populations, and the trap between them.** Narration counts
semantic `Region` items with one counter, but the crate has two kinds: the
planner's territory nodes (A/M8) and a generator's pass groupings (the
generic families, now scallop). Scallop is BOTH depending on its caller.
The first cut emitted its grouping unconditionally and **broke A/M8's 1:1
reconciliation gate outright** — 4 semantic regions vs 3 structural nodes,
with a stray `"Region 1/1 (scallop)"` among the band labels. Hence
`ScallopRegionGrouping`: `ByBoundaryRegion` standalone, `Flat` as a
sub-generator, with its own sentry.

**RampFinish standing material.** PR-8b reported one worst-case DEPTH and a
point count. Measured on its own fixture: **486 of 653 ramp points lifted,
worst lift 4.231 mm, 370.5 mm² of ramp swath left standing.**
`lifted_area_mm2` follows A/M9's three-valued contract, and `lifted_area()`
returns the `ProjectedXyAreaMm2` newtype with its provenance or neither.
A new `MeasurementStage::RampReachClampSwath` rather than a borrowed one,
and the sentry asserts the provenance is NOT `ScallopReport::PROVENANCE` —
the two sit side by side in `ToolpathStats` and must never be summed. The
swath width is the cutter's CUSP diameter and the note says so; the envelope
would be the SHANK on a tapered tool, three times too wide, which is the
exact error C3 retired from the feeds path earlier in this same wave.
RampFinish also had NO narration line at all — it has one now.

### Gates

- `cargo fmt --check`: exit 0 before every commit.
- `cargo clippy --workspace --all-targets -- -D warnings`: exit 0 before
  every commit.
- `cargo test -p rs_cam_core --no-fail-fast` (the WHOLE crate, 113 targets):
  **111 green, 2 red, both known and neither this wave's**:
  - `--lib` 2207 passed / 3 failed — the adaptive3d trio
    (`peck_plunge_progresses_when_depth_per_pass_equals_retract_clearance`,
    `rapid_segment_lifts_to_safe_z_before_traverse`,
    `planner_sim_dexel_parity_agent_search`), untouched here;
  - `wanaka_suggest_integration` 2 passed / 1 failed —
    `wanaka_suggest_baseline`, the documented environmental red. Its own
    panic names the cause: *"Toolpath id 11 missing from suggest cases —
    wanaka.toml shape changed?"*. It reads the WORKING-TREE
    `planning/airrun_2026-06-01/wanaka.toml`, which carries 114 uncommitted
    user insertions and no longer offers that toolpath. **Not the C3 feed
    change**: the assertion that fires is a missing toolpath, not a feed
    value, and it fires before any number is compared. The fixture is
    untouchable by standing instruction, so this stays red.
- `cargo test -p rs_cam_viz --no-fail-fast -- --test-threads=1`: 227 + 9 + 11
  across five targets, 0 failed.
- `cargo test -p rs_cam_cli --no-fail-fast`: bin 7/7 (2 new), integration
  9/9.
- `cargo test -p rs_cam_mcp`: 0 tests (the crate is a parameter-struct
  library), builds clean.
- New sentries: tapered width parity 5/5, waterline finish-setup 5/5,
  partial clip 4/4, narrate regions 5/5, PR-8b ramp 8/8 (2 new).
- Pre-existing sentries that had to keep passing and do: A/M8 semantic
  regions 4/4, M2.1 tapered end-to-end 9/9, Wave D1 dropped band 3/3, PR-8d
  min-segment 5/5, PR-6a derived stepover 4/4, PR-7 generic rest 6/6,
  lookup parity 2/2, wanaka defaults 3/3, waterline param sweeps 4/4.
- One cargo job at a time; `free -g` + `pgrep` before every heavy command.

**Untouched, as instructed**: `planning/airrun_2026-06-01/wanaka.toml`
(user-modified) and `planning/review_2026-07-27/`. Every commit staged file
by file.

### What a human still owns

The C3 feed change. Everything else in this wave is report-only or
behaviour-preserving; that one moves a number the operator acts on, in the
direction that deserves eyes, and the wave states it rather than shipping it
quietly.

---

## C-SEQUENCE WAVE 7b (M3 implementation), 2026-08-02

Wave 7a produced the study and **declined to merge**: every direct arm
cleared M3's 2× bar by two orders of magnitude, but the winner changes the
classifier's *answer* — 1.8–4.3% of cells across a band boundary, which is
region ownership, which is which operation cuts which territory. §8 of the
study called that a Checkpoint-B-shaped decision and said the wave had no
mandate to take it.

**The human took it.** Ruling, 2026-08-02: *"Adopt, COLUMNS-gated"* —
production switches to candidate 3 behind the eight parity gates of §9.3; the
decisive gate is an end-to-end wanaka-class `UnifiedFinish` A/B scored on
COLUMNS quality, **not on cell counts**; if COLUMNS regresses, production
falls back to the shipped classifier and M3 closes with the study as its
record.

COLUMNS did not regress. Production is on the tile raster.

**Commits**

| # | Hash | Scope | Diffstat |
|---|------|-------|----------|
| 1 | `0711568` | production switch through `finish_setup`, `SurfaceSampler` provenance, `classification_sampler` on `UnifiedFinishParams`/`UnifiedFinishConfig`, the restated acceptance gate, tripwires, reduction pin, production-path determinism/cancel gates | 8 files, +823 / −101 |
| 2 | `5d8c115` | `tests/classification_columns_ab_m3.rs` — the COLUMNS A/B and its verdict | 1 file, +696 |
| 3 | *(this commit)* | study §10, plan checklist + restated gate, this entry, the table footnote the study's §10.7 describes | |

No commit 3-for-downstream-expectations exists, and that is the finding
below.

### The decision, measured

`M3_AB_WINDOW_MM=0` — the full 100 mm terrain, Ø1 tip on a Ø6 shank at 7° so
the classification cell is the production `cusp/4` = 0.125 mm, 0.1 mm
tip-matched measurement grid, **1 001 987 dexel columns common to both
branches**:

| | A (shipped probe) | B (production raster) | Δ | tolerance |
|---|---|---|---|---|
| p50 \|dev\| | 28.59 µm | 28.69 µm | **+0.11** | +5 |
| p90 \|dev\| | 219.03 µm | 223.47 µm | **+4.44** | +10 |
| on-size ±10 µm | 26.665% | 26.570% | −0.095 pp | — |
| on-size ±25 µm | 47.596% | 47.514% | **−0.082 pp** | −0.5 |
| max \|dev\| | 3870.2 µm | 3990.4 µm | +120 | reported, not gated |
| rapid collisions | 0 | 0 | 0 | no increase |
| **generation wall** | 249.0 s | **192.9 s** | **−22.5%** | — |
| cutting time | 7188.8 s | 7588.5 s | **+5.6%** | — |

Thresholds were written above the assertions before the run and justified
against a physical scale, not against the data: the dials ask for a 22.5 µm
cusp (`0.3²/(8·0.5)`), so p50 may rise a quarter of that and p90 half of it.
Every gated statistic clears by an order of magnitude, in both directions.
**This is a wash on quality, not a win** — and it is exactly the answer the
ruling was shaped to accept or reject.

Two numbers deserve to be read together. The classifier's 187× microbenchmark
becomes **−22.5% of whole-operation generation wall**, which is where a user
feels it. And B costs **+5.6% cutting time**, because it finds 84.5 mm² more
very-steep territory (`§9.2` risk 2, predicted in advance) and waterline is
the priciest strategy per area. Quality did not pay for the extra territory;
the clock did. On the 30 mm window the same comparison went the other way
(−2.5%), so this is fixture-dependent rather than a law.

### The gate that was wrong, and what replaced it

The plan's first acceptance gate reads *"no loss of narrow steep regions
relative to the current fine classifier"*. Taken literally the winner fails
it: 5 mid-steep components lost at terrain 849².

It fails because the gate names the wrong reference. The shipped classifier
samples the CL surface of a Ø0.05 mm ball, which sits `R·(1 − n.z)/n.z` above
the model — 10 µm at 45°, 25 µm at 60°, 119 µm at 80°. That offset is
**slope-dependent**, so it does not cancel in the classification stencil; it
adds gradient of its own. Study §5.3 discriminated the two possible causes by
shrinking the probe: label disagreement with the direct arms goes to zero at
the first 10× shrink on every fixture, and max |Δz| falls exactly 10× per 10×
of radius. The "lost" regions are the probe's artefact.

Restated and shipped as `production_loses_no_region_against_the_true_surface`:
*no loss relative to the TRUE SURFACE, with the shipped classifier's
probe-shrunk limit as the reference*. Deliberately not one of the direct arms
scoring itself — the reference is the oracle's own algorithm at Ø0.0005, an
independent construction that agrees only if both are right.

Measured on all six fixtures:

- the production sampler moves **0 labels and loses 0 regions**. It does not
  approximate the surface; it is the surface.
- the shipped probe moves labels on three fixtures and **fabricates** regions
  rather than losing them — mixed-slope's 6 real mid-steep components read as
  8.

The direction is the opposite of the plan's phrasing, which is why the first
attempt at a non-vacuity assertion (`the shipped classifier must lose
something`) failed red and had to be rewritten to assert what actually
happens. Both facts are asserted now, so the gate cannot go vacuous in either
direction.

### Downstream: the expectation changes that did not happen

Nineteen sentries that consume region ownership were run against the switch:
`unified_finish_semantic_regions`, `checkpoint_a_valley_matrix`,
`coverage_routing_pr5`, `crease_own_region_pr6b`, `generic_rest_routing_pr7`,
`unified_finish_tapered_end_to_end_m21`, `steep_shallow_min_segment_pr8d`,
`finish_resolution_policy_pr3`, `tool_scale_semantics_pr2`,
`tapered_cusp_radius_sentry`, `waterline_shared_finish_setup_c3`,
`finish_planner_wanaka_decompose`, `derived_stepover_pr6a`,
`unified_finish_dropped_band_finding_d1`,
`unified_finish_partial_clip_finding_c8`, `narrate_regions_closed_c8`,
`checkpoint_b_resolution_ab`, `ramp_reach_clamp_pr8b`, `reach_policy_pr4`.

**All green. Not one pinned value moved, so the wave has no
expectation-update commit.**

That is a result, not luck, and the study explains it: §5.2's per-fixture
table shows the direct arms EXACT on flat ground and clean analytic walls,
which is what these sentries are built from. The probe artefact needs
*varying* slope at cell scale to bite. It bites on terrain — which is where
the A/B measured it, on a million columns.

One thing did change that is not a sentry and must be said: the two
`#[ignore]`d wanaka probe harnesses (`p2c_headless_ab_wanaka`,
`v3_cascade_ab`) now spell `classification_sampler: TileRaster` explicitly,
by NAME rather than via `PRODUCTION`, so their arms keep measuring one named
classifier if the default ever moves again. Their pinned constants
(`PINNED_A_PROJECT_S` / `PINNED_A_FINISH_S`, the v3 cascade rows) were
measured before this switch and are therefore **stale**: a re-run will move.
Those harnesses already carry the standing instruction to re-measure when the
chain or simulator changes materially, and a classifier change is material.
They were not re-measured here — they read the user-live `wanaka.toml`, which
this wave is forbidden to touch.

### What the parity net actually pins now

| gate | wave 7a | wave 7b |
|---|---|---|
| label movement per fixture | printed as characterisation | `LABEL_MOVE_TRIPWIRES` — zero-pinned on the three EXACT fixtures, 25% headroom elsewhere, unknown fixture = hard fail |
| narrow-steep survival | scored against the probe | scored against the **surface**, with the probe's fabrication asserted as the non-vacuity control |
| the per-cell reduce | an inline `>` | `keep_higher`, a named function with a test that fails on `f64::max` — signed-zero ties keep the first-seen candidate, a NaN neither wins a cell nor is laundered out of one |
| thread determinism | the five arms | the arms **and** `build_classification_surface_*` itself, diffing Z, coverage and labels, with a band-population non-vacuity check |
| cancellation | "an arm returns `Cancelled`" | poll-counted **latency** bound + a ≥3-tile-band fixture guard + a production-path twin |
| the config seam | — | serde matrix: default is production, production is never written to a project file, an absent field loads as production, a pinned non-production sampler round-trips |

The cancellation tightening earned its keep inside the wave. The new
"fixture must be ≥3 tile bands" guard immediately failed this wave's own
first attempt, which cancelled on the third poll of a **3**-band grid — the
cancel was landing on the last band and the latency claim proved nothing. It
now cancels on poll 2 of a 3-band grid and asserts the arm stops there.

### Consolidation, and one duplicate retired

The classification grid arithmetic existed twice — once in `finish_setup`,
once in `ClassificationGridSpec::for_mesh` — with a sentry whose whole job was
policing the copy. Production now calls the helper, so the sentry was
rewritten to assert the **contract** instead: padding is one ENVELOPE radius
(physical sweep, `TOOL_SCALE_SEMANTICS.md` §8 row 2), the cell is the
resolution policy's, the grid reaches the far padded edge so the
mask→polygon extractor keeps its non-contact margin ring, and the floor is
the mesh bbox floor.

Provenance follows the `CellSource` precedent one level up.
`FinishSurface.sampler: SurfaceSampler` is `CutterOffset` for generation
grids and `Classification(sampler)` for classification grids: `CellSource`
says what **sized** the cells, this says what was **measured into** them. Two
grids of identical geometry over identical meshes hold different surfaces
depending on this field, and the switch moved production cells by changing
nothing else.

§9.4's standing correction landed too. `CLASSIFICATION_PROBE_DIAMETER_MM` no
longer says the probe's offset is "negligible at finish cell sizes" — it
carries the measured numbers, and the number it was hiding is 8–20% of a cell
in Z.

### Instrument design: the traps this harness was built around

The A/B does **not** read `planning/airrun_2026-06-01/wanaka.toml`. A gate
that depends on a live, user-modified project has a verdict that changes when
somebody drags a slider — `wanaka_suggest_baseline` has been red for exactly
that reason for weeks. The fixture is the committed `tests/fixtures/terrain.stl`
and the config is built in-test from the C6 helpers.

The FOOTPRINT is a knob; the SCALE is not. Everything that makes the switch
discriminating is a property of the 0.125 mm cell against a 10–25 µm probe
offset, so the tool and the cell are the production ones and only the area
moves.

Branches are scored on the **common** column population, indexed by
`(row, col)` and never by inverting a transform (P2.g Task 1's lesson). The
relevance filter can admit a column in one branch and not the other;
comparing two p50s over two populations compares two questions.

And three PNGs are written to `target/m3_columns_ab/` before any verdict is
read, per the v3 campaign's closing rule — *never gate on an aggregate
without rendering the surface*. The B−A map is the phase texture P2.g
characterised: mottled, local mean zero, **no coherent block of standing
material** anywhere. 66.9% of columns move between branches, so the two
surfaces really are different surfaces being compared rather than a rounding
difference.

### One reading trap, documented rather than papered over

`print_equivalence_rows` scores against whichever grid the caller passed as
the ORACLE. Where that is the shipped probe, a `FAILED` row means the arm
dropped a region **the probe reports** — characterisation since this wave,
not a gate. `terrain@849` still prints `FAILED` for all three direct arms
while the suite is green. Rather than silently changing the verdict function,
the harness now prints a footnote under every table saying so, and study
§10.7 records it. A table that says FAILED next to a green suite is precisely
the kind of thing that misleads the next reader.

### Gates

- `cargo fmt --check`: exit 0 before every commit.
- `cargo clippy --workspace --all-targets -- -D warnings`: exit 0 before
  every commit.
- `cargo test -p rs_cam_core --no-fail-fast` (the WHOLE crate, 114 targets):
  **112 green, 2 red, both known and neither this wave's** — 2644 passed /
  4 failed:
  - `--lib` 2210 passed / 3 failed — the adaptive3d trio
    (`peck_plunge_progresses_when_depth_per_pass_equals_retract_clearance`,
    `rapid_segment_lifts_to_safe_z_before_traverse`,
    `planner_sim_dexel_parity_agent_search`), untouched here;
  - `wanaka_suggest_integration` 2 passed / 1 failed —
    `wanaka_suggest_baseline`, the documented environmental red: it reads the
    WORKING-TREE `planning/airrun_2026-06-01/wanaka.toml`, which carries
    uncommitted user edits and no longer offers the toolpath it looks for.
    The fixture is untouchable by standing instruction, so this stays red.
- `cargo test -p rs_cam_viz --no-fail-fast -- --test-threads=1`: 5 targets,
  247 passed, 0 failed.
- `cargo test -p rs_cam_cli --no-fail-fast`: 2 targets, 16 passed, 0 failed.
- `cargo test -p rs_cam_mcp --no-fail-fast`: 2 targets, 4 passed, 0 failed.
- New/extended sentries, all green: `classification_strategy_m3` 13 passed
  (4 new: the restated true-surface gate, production-path thread determinism,
  production-path cancellation, the config serde matrix) + 2 ignored study
  rows, and 2 new unit tests in `classify_probe` (the reduction pin, the
  `PRODUCTION`/`Default` pin). `classification_columns_ab_m3` 1 ignored gate,
  run manually on both windows.
- Timing rows re-earned in the final tree (gate 8), release,
  `--test-threads=1`, `pgrep` clear: terrain 849² shipped **3.161 s wall /
  40.38 s CPU** vs tile raster **0.016 s / 0.13 s** — **198× wall, 311×
  CPU** (wave 7a: 187×). Analytic 849² 0.721 s → 0.004 s, **180×** (wave 7a:
  230×). Peak RSS 135 MB. The ratios reproduce; the absolute walls wobble
  ~6%, which is what §9.2 risk 6 said one machine and one run buys.
- The terrain label-movement rows re-measured **exactly**: 709 / 3174 /
  17 687 at 143²/425²/849². The wave-7a table reproduces, so the tripwires
  are pinned to real numbers rather than to a lucky run.
- One cargo job at a time; `free -g` + `pgrep` before every heavy command.

**Untouched, as instructed**: `planning/airrun_2026-06-01/wanaka.toml`
(user-modified) and `planning/review_2026-07-27/`. Every commit staged file
by file.

### What a human still owns

**The +5.6% cutting time on the full terrain.** Quality is a wash and
generation got 22.5% faster, but the shipped operation will take longer on
wanaka-class relief because the honest classifier finds more very-steep
ground and waterline is expensive. That is the correct answer geometrically
and a slower one on the clock, and it is the kind of trade that deserves eyes
rather than a silent merge. The lever if it is unwanted is
`waterline_threshold_deg`, not the classifier.

### What this closes, and what it does not

M3 fix-sequence steps 1–4 are done. Steps 5 (cached classification fields)
and 6 (adaptive refinement) are recommended **closed rather than carried**,
for §8's reason now sharpened by measurement: a classification that takes
16 ms is buying a cache hit rate against 16 ms, and cannot repay the
invalidation risk `TriangleMesh` has no revision field to manage.

Candidate 0 (`query_into` / `QueryScratch`) remains answer-preserving,
recommended on its own merits under 11 call sites, and **unlanded** — it is
worth nothing here and the study says so; it should be taken where a profile
shows it helps, not on M3's authority.

Still unexercised: down-wound faces differ by 2R between the two sampler
families (§9.2 risk 3). Uniform on `stacked_shelf`, a 50 µm step on a real
overhang, and nothing in this repo's fixtures has one. The switch moves
production onto the correct side of it.

---

## C-SEQUENCE WAVE 9 (M4 research), 2026-08-02

Addendum C's step 8: M4 "repair the scallop algorithm instead of stacking
compensations" — **research phase only**. Oracle, candidate prototypes behind
a strategy seam, the comparison harness, and the study. **No production path
moved**: `ScallopStepoverPolicy::SHIPPED` is the only value any production
entry point passes, PR-3's scallop fingerprints are unchanged, and step 8 of
the sequencing checklist is deliberately NOT ticked — it completes when an
implementation lands.

**Commits**

| # | Hash | Scope | Diffstat |
|---|------|-------|----------|
| 1 | `ae5eed6` | `tests/common/scallop_oracle.rs` + `tests/scallop_oracle_validation_m4.rs` + the `common` module table | 3 files, +1537 / −0 |
| 2 | `28503db` | `ScallopStepoverPolicy` + `RingSource` + `scallop_toolpath_research` in `scallop.rs`, new `scallop_isofield.rs`, `tests/scallop_candidates_m4.rs`, oracle `residual_map` / `tool_reach_floor` / parallel scoring | 6 files, +2099 / −61 |
| 3 | *(this commit)* | `CHECKPOINT_C_EVIDENCE.md`, this entry | |

Deliverable: `planning/review_2026-07-29/CHECKPOINT_C_EVIDENCE.md`.

### The headline: the fingered culprit is not the culprit

Checkpoint B's `max_rings` addendum closed with *"the fix is in
`ring_stepover`, not in the budget."* Measured against an oracle validated on
closed-form ground truth: **the fix is in neither.**

`ring_stepover`'s min-across-ring costs up to 1.68× in stepover and up to 37%
of the ring count — real time — but retiring it moves achieved cusp by ~0%
(narrow ridge 231.7 → 231.0 µm, ribbon 564.2 → 539.4 µm). The overshoot is
2.37× the dial on **flat ground** and 3.95× on a smooth **dome**, where the
collapse ratio is 1.00× and 1.01× and the mechanism is doing nothing at all. A
mechanism that is inactive cannot be the cause.

The addendum's recommendation was reasonable on its evidence — it had ring
counts and uncut area, and min-across-ring drives both. It did not have a cusp
measurement that could be trusted on sloped ground. That is what phase A was
for, and it is the second time in this programme that building the instrument
first overturned the conclusion the previous wave was about to act on.

### What the instrument found instead

`scallop_math::variable_stepover`'s slope term is **inverted**. Rings are
offset in XY, so on ground at slope θ two adjacent rings end up `d·sec θ`
apart along the surface and the normal cusp is `R − √(R² − (d·sec θ/2)²)`.
Holding the cusp requires scaling the XY stepover by **`cos θ`**.
`variable_stepover` scales it by `1/√cos θ` — 1.24× too wide at 30°, 1.69× at
45°, 2.84× at 60°, and cusp goes as `d²`.

Measured against the oracle at 0/15/30/45/60°: the law holds to ≤4.5%, 0.36%
at 45°. Asserted with a non-vacuity check that the shipped formula disagrees
with it *in the opposite direction*.

This also puts a retroactive caveat on Checkpoint B §3.1: `cusp/4` bringing
achieved cusp "down to on-dial" is partly accidental. A finer grid raises the
raw curvature estimate (`κ` ∝ `1/cell²`), which tightens the stepover and
masks the loosening from the inverted slope term. Two errors partially
cancelling is not a working dial.

### The trap that makes the obvious fix worse than the bug

Correcting the slope law **inside the cascade** is the worst arm in the study:
194–211× the dial, 38–102 mm² of never-touched material, 67–146 mm² of
`uncut_core`. Both arms report exactly 51 rings — `max_rings` — and the
residual map shows a 6.4 mm square hole in the middle of the part.

A correct law is *tighter*; `max_rings` is budgeted from the *flat-ground*
stepover. **The stepover law and the ring budget are one problem, not two.**
This is also the retroactive explanation for v3's "+92% time, 34× over-cut"
naive cap raise, and for why no budget derived from a nominal stepover —
including PR-8c's `ReachPolicyStepover` — could ever have worked.

### The candidate that survives

`scallop_isofield` (plan item 3): solve `|∇D| = 1/s(x,y)` from the region
boundary by Godunov fast sweeping, take the integer level sets. No per-ring
scalar, so both minima have nothing to reduce; no repeated offsetting, so the
decimation compensation has nothing to contain; ring count is `⌊max D⌋`, known
before a ring is emitted — M4's fix-sequence item 4, met by construction
rather than by a better guess. Both ring sources feed the same lift, chord
refinement and emission, so the comparison isolates placement.

Iso-field + corrected law beats shipped on **every** fixture at comparable
time: cusp 2.02–4.90× vs 2.37–11.58×, standing material 2.14–3.90 mm² vs
5.64–19.87 mm², zero truncation at both resolutions.

**And it unblocks Checkpoint B.** That ruling held scallop at
`LegacyEnvelopeQuarter` because `cusp/4` left 19–33 mm² standing. The harness
reproduces that figure exactly (narrow ridge, shipped, `cusp/4` → 19.32 mm²,
bit-identical), then shows the rejection is a property of the **offset
cascade, not the resolution**: under the iso-field, `cusp/4` gives zero uncut
core everywhere, less standing material on four of five fixtures, and better
cusp on four of five. On flat ground every arm reads exactly 1.00× the dial at
`cusp/4`.

### Two instrument saves worth recording

**The terrain fixture cannot adjudicate this, and now says so with a number.**
All ten arms land within 4 µm of each other on the 20 mm crop. The oracle
gained a `tool_reach_floor` — the envelope of the cutter dropped at every
reachable cell, i.e. what an infinitely dense path would still leave. It reads
**p99 425.9 µm** against a best arm of 395.9 and a shipped arm of 470.6.
Nearly the whole terrain residual is geometry no ring placement controls. The
v3 campaign hit this same wall with this same fixture and diagnosed it in
prose; it is now a column. Its own error is measured, not assumed: −28.6 →
−6.4 → −0.0 µm as the fixture mesh refines 0.20 → 0.10 → 0.05 mm.

**The diff maps drew never-reached cells the same black as off-model.** The
first render collapsed "untouched" into `NaN` — reintroducing at the last step
the exact confusion the oracle exists to prevent. Caught by looking at the
picture before writing the verdict, which is the v3 standing rule and earned
its keep again: reading `resid_flat_ground_A0.png` is what turned "2.37× on
flat ground" from an anomaly into the corner-decimation finding.

### Honest limits

* `PolygonReduce` (min-across-polygons) is a **byte-identical no-op** on all
  six fixtures — unfalsified, not exonerated. Needs a dendritic region-scoped
  fixture.
* No arm reaches the dial in absolute terms on sloped ground; §3.5 attributes
  the remainder to placement quantisation but does not exhibit an arm closing
  it.
* The iso-field has a **localised 10× deeper gouge** on the grooved block
  (−1115 vs −108.6 µm) even though its gouge *area* is 2.4–5.0 vs 12.3 mm².
  Open risk, must be understood before it ships.
* Continuous mode unmeasured; all timings are 16 mm fixtures at sub-second
  scale, which does not establish a ranking on a real part.

### Gates

`cargo fmt --check` clean; `cargo clippy --workspace --all-targets -D
warnings` zero. `--lib` 2213 passed with only the three known adaptive3d reds.
62/62 on the ten scallop-adjacent integration binaries, including
`finish_resolution_policy_pr3` (the scallop fingerprint pins) and
`checkpoint_b_resolution_ab` (the previous wave's own harness). 56/56 param
sweeps. 9/9 oracle validations, 3/3 candidate guards.

**Checkpoint C is a human decision.** The evidence document closes with a
four-option menu plus two sub-decisions; the recommendation is option 1 —
adopt the iso-field ring source together with the corrected slope law and the
removal of `max_rings`, as one change, behind an end-to-end COLUMNS A/B in the
shape of the M3 ruling.

---

## C-SEQUENCE WAVE 9b (M4 implementation), 2026-08-02

Checkpoint C ruled **option 1 — "adopt the iso-field, gated"**: the iso-field
ring source, the corrected `cos θ` law and the retirement of `max_rings` land
as ONE change, behind an end-to-end COLUMNS A/B and behind HARD gates on the
research wave's two self-flagged defects — the −1115 µm localised gouge, and
the sub-10 µm segment tail. Fallback pre-registered: if the gates fail after
genuine attempts, production stays on the shipped cascade and M4 closes
honestly.

**They failed, and the reason they failed is the wave's result.**

**Commits**

| # | Hash | Scope | Diffstat |
|---|------|-------|----------|
| 1 | `dde7a54` | `refine_chord` + `CHORD_REFINE_MIN_SPLIT_MM` / `CHORD_REFINE_ACCEPT_FRACTION` + iso-field decimation in `scallop.rs`; `seed_boundary` + signed field in `scallop_isofield.rs`; `deepest_gouge_normal_um` in the oracle; new `tests/scallop_isofield_gouge_m4.rs` | 4 files, +792 / −11 |
| 2 | `a376b1e` | PR-3 scallop fingerprint re-pin + history table | 1 file, +29 / −2 |
| 3 | `99e1bdb` | `scallop_math::variable_stepover` doc + renamed defect-pinning test (sub-decision 5a) | 1 file, +61 / −7 |
| 4 | *(this commit)* | `CHECKPOINT_C_EVIDENCE.md` §10–15, checklist step 8, this entry | |

### The gouge was three defects, and two of them were shipped

§3.8 guessed "a contour crossing a groove rim where the cascade's rings ran
parallel to it". Measured first: a diagnostic separating *vertex dives* from
*chord sag* found **zero vertex dives in every arm**. All of it is chord sag.

1. `refine_chord` declined to probe any chord it could not fit one
   `probe_step` interval into. Sound for a gridded surface; false for a
   drop-cutter query, which is exact at any XY. Invisible while every chord
   came from a decimated offset ring — but refinement's own halves are not
   decimated, so splitting a 0.56 mm chord makes two 0.28 mm ones that nothing
   then checks.
2. The iso-field's Dirichlet condition, `D = 0` at grid nodes outside the
   polygon, is a statement about the grid rather than the region. Every level
   set was pushed outward by up to a full cell, and level 1 could interpolate
   to a position off the part — where drop-cutter answers with the cutter's
   rim riding the mesh edge, ~1 mm low, and a 0.75 mm coverage mask cannot
   veto a 19 µm excursion. `seed_boundary` pins every node within a cell of
   the edge to its exact **signed** sub-cell distance in local stepovers.
3. The tolerance check was sampled at the generation cell, so a 0.7 mm chord
   on a 0.75 mm cell got one probe, at its midpoint, while the worst deviation
   sat at t = 0.296. Shipped had the identical hole and got lucky: 97.8 µm
   probed against a 131.7 µm true maximum, passing a 100 µm tolerance it was
   violating.

Fix order mattered and was informative: decimation alone made the iso-field
**worse** (−995 → −1015 µm), because longer chords made refinement engage and
leave two unrefined halves. That is what pointed at (1), and (1) alone was not
enough, which pointed at (2).

### The instrument was ranking chord density as if it were stepover

Fixing (1) and (3) changed the **shipped** arm, and that settles Checkpoint C.
Achieved cusp is p99 of *positive* residual — material left — and a sparsely
chorded path leaves material between its points that scores exactly like a
wide stepover.

| fixture | A0 before | A0 after | A9 after | gap before | gap after |
|---|---|---|---|---|---|
| flat ground | 2.37 | 2.37 | 2.11 | 0.35 | **0.26** |
| grooved block | 6.28 | **3.11** | 3.02 | 3.42 | **0.09** |
| narrow ridge | 11.58 | **4.37** | 4.24 | 6.68 | **0.13** |
| ribbon | 28.21 | 26.92 | 26.65 | 1.93 | **0.27** |
| dome | 3.95 | 3.98 | 3.91 | 1.29 | **0.07** |

The two fixtures carrying §7's recommendation are the two where shipped gained
most. The iso-field won a comparison substantially about how densely each
source chords the surface; its undecimated marching-squares vertices gave it
the denser path for free. A9 is still better on cusp and standing material on
all five fixtures — by **1–11%**, not 15–58%.

### Gates

* **Sub-10 µm segments — MET.** 0 of ~5000 on every fixture, both arms,
  against up to 32 before. Two mechanisms: the iso-field inherits the ring
  decimation the cascade always had, and refinement is floored at
  `CHORD_REFINE_MIN_SPLIT_MM` (50 µm — 50× PR-8d's `MIN_EMITTED_SEGMENT_MM`,
  5× the junction bar) so it cannot manufacture one either.
* **Max local gouge ≤ shipped — NOT MET.** Surface-normal, A9 exceeds A0 on 3
  of 5 by 0.3 / 3.2 / 14.4 µm and is 24.1 µm better on the ribbon; gouge AREA
  is worse on 3 of 5, which is a distribution rather than a tail. The specific
  −1115 vs −108.6 µm the ruling named IS cleared: −72.6 µm normal / −89.7 µm
  vertical, 12.4×.
* **Verdict: FALLBACK.** Production stays on the offset cascade;
  `RingSource::IsoField` stays behind the seam. Not a technicality — the gate
  missed, and the margin that justified "the largest blast radius in this
  programme so far" is now a few percent.

### A third instrument correction, and it changed the ranking

The first draft of the chord sentry measured **vertical** drop and condemned
the shipped cascade at 174 µm against a 100 µm tolerance on the narrow ridge.
On a 76° flank vertical is `1/cos θ` = 4.1× the real deviation: 174 µm
vertical is 42 µm perpendicular, inside tolerance. `deepest_gouge_um` — the
number §3.8 raised the alarm with — had the same flaw. Vertically A9 looks
12.4 µm worse than A0 on the ridge and 37.3 µm worse on the ribbon; normal to
the surface the ridge gap is 3.2 µm and the ribbon **reverses** to A9 being
24.1 µm better. `OracleReport::deepest_gouge_normal_um` now sits beside it.

Three waves, three instruments caught measuring the wrong quantity. The
pattern is worth naming: each one compared a number taken in the tool's frame
against a dial written in the surface's.

### Downstream

Exactly **one** red across the whole scallop-adjacent set —
`finish_resolution_policy_pr3::scallop_fingerprint`, `(1318, …)` →
`(1423, …)`, +105 moves (+8.0%), re-pinned with the old value kept in a
history table and the reason beside it. Everything the recon flagged as
at-risk stayed green **without an edit**, because the ring cascade, its
stepover law and its `max_rings` budget were never touched:
`checkpoint_b_resolution_ab` 8/8 (including
`ball_control_collapses_the_two_named_arms`),
`standing_material_channel_am9` 4/4 (its `max_rings` truncation fixture
intact), `narrate_regions_closed_c8` 5/5, `capability_link_moves_safety`
17/17, `scallop_candidates_m4` 3/3 including the byte-for-byte shipped-policy
guard on all five fixtures, `common_fixtures_smoke_c6` 8/8,
`generic_rest_routing_pr7` 6/6. Both scallop param sweeps pass. `--lib`
scallop 50/50, `scallop_math` 12/12. `cargo fmt --check` clean;
`cargo clippy --workspace --all-targets -- -D warnings` zero.

### Honest limits

* **The end-to-end COLUMNS A/B was not run.** It grades adoption; adoption
  failed its precondition. Running two UnifiedFinish generations and two
  tip-matched dexel sims to grade a ruled-out candidate is not evidence, it is
  ceremony.
* **Sub-decision 5b is not done** — `ScallopReport` still has no
  untouched/standing split, and `uncut_core_mm2` stays hole-blind and
  cascade-only. It is independent of the ring source and still worth doing.
* **`max_rings` stays** — and it has **no user-facing dial anywhere**
  (`ScallopConfig`, `SCALLOP_PARAMS`, the GUI, every project TOML: nothing).
  Retiring it would have been purely internal; the PR-5 `route_width_factor`
  deprecation route was scoped for and never needed.
* **Continuous mode remains unmeasured**, as Checkpoint C §9 said.
* **Any future iso-field benchmark must re-baseline against `dde7a54`.** Every
  number in Checkpoint C §2.1 was taken against a cascade that was not
  chording honestly.

The wave is not a null result. The chord-refinement defect is a **shipped**
fidelity bug that was silently violating the operator's path tolerance on
every scalloped part, on both ring sources, and it is fixed.

> **ERRATUM (wave 11, 2026-08-03) — `dde7a54` moved a SECOND fingerprint and
> left it red.** `a376b1e` re-pinned the scallop-side fingerprint, which the
> wave knew it had moved. It did not re-pin
> `crease_own_region_pr6b::production_unified_finish_output_is_byte_identical`,
> whose taper arm also embeds scallop (`UnifiedFinish` routes its MidSteep
> band through it). That target has been red from `dde7a54` onward. Proven by
> surgical revert: restoring `scallop.rs` + `scallop_isofield.rs` to
> `dde7a54^` reproduces the pinned `(1855, 0xe031…)` exactly. Re-pinned in
> wave 11 with the fragment-level A/B — 88 → 88 fragments, deep groove
> byte-identical, all 19 added moves inside the shallow V and none entering
> new depth territory. **Lesson: a fingerprint lives wherever the geometry is
> embedded, not only where the subject module is named.** When a wave re-pins
> one, grep the crate for others before declaring the sweep clean.

## C-SEQUENCE WAVE 10 (C9), 2026-08-03

C9 is the model-debt bundle: four items from `ANTIPATTERNS_BACKLOG.md` P9,
each gated on its own evidence, each free to close honestly if it could not
prove itself. Three landed as behavioural change. The fourth is research, and
it found something nobody was looking for.

**Commits**

| # | Hash | Scope | Diffstat |
|---|------|-------|----------|
| 1 | `9963128` | `reach::solve_reach_sampled` + `ReachModel` + `SampledCrossSection` + `PRODUCTION_REACH_MODEL`; `rest_field::measure_cross_section` returns both readings off one walk; new `tests/checkpoint_c9_sampled_reach.rs`; C9 gate added to `checkpoint_a_valley_matrix.rs` | 5 files, +1357 / −24 |
| 2 | `0dff17e` | per-point stepover through `crease_paths`/`pencil` (`offset_polyline_variable`, `OffsetFan::stepover`); new `tests/per_point_claims_fan_c9.rs`; `derived_stepover_pr6a` Gate 2 rewritten | 5 files, +761 / −70 |
| 3 | `ef4011c` | `RemapIndex` + `AggregateTree` in `toolpath_spans.rs`, wired into `tsp::remap_spans` and `MoveRemap::remap_spans`; new `tests/remap_interval_index_c9.rs` | 3 files, +1193 / −7 |
| 4 | `d8d09da` | new `tests/rest_grid_resolution_c9.rs` (research only) | 1 file, +276 |
| 5 | *(this commit)* | this entry, checklist step 9 | |

### Sub-item 1 — the sampled model is right, and it still does not ship

`solve_reach_sampled` erodes the cutter against the sampled `surface_z`
cross-section and reads no wall angle anywhere. `measure_cross_section` now
takes BOTH readings off ONE perpendicular walk, so the two models can never be
compared across different measurements — the wave's own rule about isolating
the variable under test, applied to itself.

It earned every model claim. On the full 176-cell Checkpoint A matrix, all
three tools, five pitches: **zero gouge and zero float-blind at every pitch**,
including the shipped 0.5 mm cell. Those are the two failure modes that reach
the workpiece. On the shapes the V is genuinely wrong about, at 0.01 mm pitch
on the shipped taper:

| fixture | CLR+θ (shipped) | SAMP (new) |
|---|---|---|
| chamfered groove | 54 gouge, 67.3 % coverage, worst over-claim **1.423 mm** | **0** gouge, 83.3 %, **0.000 mm** |
| circular-arc valley | 12 gouge, 8 miss, 35.6 % | **0** gouge, 1 miss, 41.3 % |
| straight V + plain trapezoid (control) | 0 gouge, 89.8 % | 0 gouge, 89.8 % (tie) |

**Production does not switch, and the blocker is the grid.** The approved
column's other two bars — 100 % coverage, zero routing over-claims — are
resolution-bound, because the model places a wall at the inner end of the
segment bracketing it and is short by up to one pitch:

| rest cell | Ø1/7° taper | Ø1/15° taper | Ø3 ball |
|---|---|---|---|
| 0.5 mm (shipped) | 53.3 %, 10 route-over | 50.5 %, 14 | 52.0 %, 11 |
| 0.1 mm | 90.7 %, 1 | 91.4 %, 2 | 92.2 %, 0 |
| 0.01 mm | 98.1 %, 0 | **100 %, 0** | **100 %, 0** |
| 0.002 mm | **100 %, 0** | 100 %, 0 | 100 %, 0 |

50× to 250× finer than the shipped cell — 2 500× to 62 500× the cells.
`PRODUCTION_REACH_MODEL` carries the ruling, the table and the flip
instructions; `rest_field` already dispatches on it, so nothing else moves.

**Two findings worth more than the feature.**

*A plain symmetric trapezoid is not a shape the V misreads* — and `reach.rs`'s
own limitation note said it was, which is why it was picked as fixture 1. The
tip can never go below the floor, so the only surface that can constrain it is
the straight wall, and "rim distance + steepest gradient" encodes a straight
wall exactly. `X_v` and `X_true` reduce to the same expression. Proved
algebraically, then measured (the models tie), the module doc corrected, and
the trapezoid kept as a CONTROL. The discriminating trapezoid is a CHAMFERED
one — two wall angles, so the modelled wall is a line the real chamfer runs
inside. Had the wave taken the doc's word for it, fixture 1 would have
"passed" by measuring nothing.

*The gate found a real bug in the thing it was gating.* The first
implementation dropped the exactly-touching constraint (strict `>` on
`ξ − width`). A Ø3 ball's `width_at_height` SATURATES at the shank radius, so
equality is REACHED rather than approached — the binding wall sample was
skipped and the next sample out set the reach. One cell in 176, 50 µm
over-claim, invisible to every fixture except the full matrix.

The between-samples reading is the upper envelope of the two bracketing
samples, so a coarse grid can only COST reach, never invent it. An
interpolating reading needs far less grid; it was tried first, and it
over-claimed. The pitch requirement is the price of that guarantee, and it is
recorded as the lever someone will reach for.

### Sub-item 2 — the fan was violating the overlap the policy specifies

`centerline_cut_paths` solved reach per point and then spaced the fan at one
scalar, sized by both callers at `min_valley_depth`. The first fixture framed
this as under-coverage and produced numbers that argued against their own
prose: 23.08 % shortfall → 20.62 %, with FEWER passes. That is not evidence.

Re-framed on the real invariant, it is unambiguous. `SUGGESTED_STEPOVER_OVERLAP`
specifies 50 %; on a branch ramping 0.2 → 2.0 mm deep, the retired scheme
spaces the deep end at 0.2500 mm where the working half-width is 0.6879 mm —
**63.66 % overlap**. Those passes re-cut a band already cut instead of
reaching outward, and the error grows with how much deeper a branch runs than
the detector's floor. Measured on real emitted geometry from a hand-built
`RestCenterline` (literal depths, no detector noise): **50.00 % at both ends**,
0 of 176 points placing a pass beyond its local reach, Ø3 ball control
unmoved.

Coverage barely moves on this fixture because the 4-pass cap was already
binding under the old scheme. The test says so.

One sentry moved and had to: `derived_stepover_pr6a`'s Gate 2 asserted that
passing a better SCALAR resolves more distinct pass positions. That lever no
longer exists. Rewritten to assert the strictly stronger property — the two
arms are now identical AND still spaced at 0.280 mm rather than the retired
envelope rule's 1.500 mm, so "identical" cannot be satisfied by a regression
that puts both arms back on the Ø6 shank.

### Sub-item 3 — and the first fix was 4.5× too expensive

`RemapIndex` replaces the O(spans × moves) scan. 200 000 moves / 261 spans /
5 reps: linear scan 4.44 s → **324 ms, 13.7×**, at **14.87 MB**.

The first draft measured 5.0× at **66.75 MB**, and the gap was a redundancy,
not a tuning knob: `remap_range` had its own O(n log n) sparse table restating
the `(min_start, max_end)` aggregate the intrusion tree already stored.
Deleting it cut memory 4.5× AND nearly tripled the speedup, because building
that table was most of the per-call cost. Reviewing the memory number rather
than accepting the speedup is what surfaced it.

Identity is the point: C1's 15-value fingerprint harness 3/3,
`region_node_ranges_tile_the_stitched_toolpath` (in
`unified_finish_semantic_regions` 4/4), `dressup_span_invariants` 4/4,
`boundary_clip_invalidates_spans` 2/2, `tsp::` 13/13, `toolpath_spans::`
47/47, plus 2000 random remaps across nine shapes against an independently
written brute-force oracle — asserting the CHOSEN intruder index and range,
not merely existence, because a diagnostic that names a different move is a
different diagnostic.

Open follow-up, deliberately not taken: u32 storage would roughly halve the
footprint again, but it raises a `u32::MAX` sentinel question that wants
deciding rather than slipping in under a perf commit.

### Sub-item 4 — an anomaly, recorded as an anomaly

The decisive half of the resolution question is sub-item 1's: the sampled
model needs 0.002–0.010 mm, so no plausible rest cell reaches it, and
deriving the cell from tip radius the way `FinishResolutionPolicy` does would
not help — the required pitch is two to three orders BELOW the 0.5 mm tip
radius, so it is not a tip-scaled quantity at all.

The other two thirds — feature detection and fan width, on the shipped
detector — did not produce the expected curve. They produced the opposite:

* tip-scale groove (0.8 mm rim half-width): the SHIPPED 0.5 mm cell finds one
  centreline of 19.0 mm; the 0.25 mm and 0.10 mm cells find **nothing**;
* wide control: detected length flat under refinement (38.0 / 37.0 / 36.4 mm),
  but median per-point reach **collapses** 0.583 → 0.242 → **0.000 mm** — two
  fan passes at the shipped cell, none at 0.1 mm.

The coarse reading is the suspicious one: `measure_cross_section` reports
`rim_distance = rim_cells × cell`, so a coarse walk quantises the rim distance
UP to the cell. But a hand-check says neither end is obviously true — a Ø1 tip
on a 7° cone is ≈ 0.65 mm wide at 1.2 mm depth inside a groove ≈ 2.06 mm
half-wide there, arguing for ≈ 1.4 mm of reach, more than the coarse reading
and far more than the fine one.

So the harness asserts only what it can defend and PINS the anomaly itself: if
the 0.10 mm cell ever starts finding the tip-scale groove, or the reach
collapse stops reproducing, the test fails and asks for the doc to be
rewritten rather than absorbing the change. **Recommendation: leave
`cell_mm` at 0.5 mm** — not because the cell is right, but because the model
that would justify changing it cannot be adopted at any plausible value, and
the reason the current value looks wrong is not yet understood. Whoever next
touches `measure_cross_section` owns it; the first thing to check is whether
the ridge polyline and its perpendicular walk survive refinement.

### Gates

`cargo fmt --check` clean. `cargo clippy --workspace --all-targets -D
warnings` zero. `rs_cam_core --lib` 2216 passed / 3 failed — the three KNOWN
adaptive3d reds and nothing else. Every reach-, pencil- and span-adjacent
sentry green, run individually and listed per sub-item above:
`reach_policy_pr4` 6/6, `coverage_routing_pr5` 6/6, `generic_rest_routing_pr7`
6/6, `pencil_tip_float_channel_d1` 4/4, `ramp_reach_clamp_pr8b` 8/8,
`derived_stepover_pr6a` 4/4, `checkpoint_a_valley_matrix` 15/15,
`checkpoint_c9_sampled_reach` 6/6, `per_point_claims_fan_c9` 3/3,
`rest_grid_resolution_c9` 2/2, `remap_interval_index_c9` 2/2,
`transform_provenance_fingerprints` 3/3, `unified_finish_semantic_regions`
4/4, `dressup_span_invariants` 4/4, `boundary_clip_invalidates_spans` 2/2,
`tsp::` 13/13, `toolpath_spans::` 47/47.

The exhaustive `cargo test -p rs_cam_core --tests --no-fail-fast` sweep
(~35 minutes; it carries the ~10-minute `adaptive3d_interior_cell_parity_f029`)
reported **exactly two failing binaries, both KNOWN reds and neither this
wave's**:

* `--lib` 2216 / 3 — the three adaptive3d reds above;
* `wanaka_suggest_integration` 2 / 1 — `wanaka_suggest_baseline`, the
  environmental red, which reads the user-modified
  `planning/airrun_2026-06-01/wanaka.toml` and touches nothing C9 changed.

Nothing else in the crate moved. That matters most for sub-item 2, which is
the only one of the four that changes emitted geometry: no fingerprint, A/B or
end-to-end harness anywhere in the crate shifted under it.

### Honest limits

* **No live wanaka validation.** Three of four sub-items are gated on
  synthetic fixtures and analytic truth. Sub-item 3 is identity-preserving so
  that is sufficient; sub-item 2 changes emitted geometry on measured
  centrelines and has been proven only on a hand-built one.
* **Sub-item 1's production path is dark code.** The `SampledCrossSection`
  arm of `detect_rest_valleys` is reachable only by flipping a constant, and
  is exercised only through the direct-call harnesses. It is wired, not run.
* **Sub-item 4 is one third answered.** The reach-fidelity axis is decisive;
  the detection axis produced a contradiction instead of a measurement.
* **The C9 fixtures are analytic cross-sections, not meshes.** They say what
  the reach MODEL does, not what the detector feeding it does — which is
  precisely the gap sub-item 4 fell into.

> **ERRATUM (wave 11, 2026-08-03) — the exhaustive-sweep claim above is
> wrong.** This section reports "exactly two failing binaries, both KNOWN
> reds". Wave 11's baseline at the same tree (`31c1c99`, reproduced on a
> pristine checkout with wave 11's work stashed) found **three**: the two
> named here plus
> `crease_own_region_pr6b::production_unified_finish_output_is_byte_identical`.
> That third red is **not C9's** — it belongs to `dde7a54` (M4), see the
> erratum in the wave 9b section — so the substantive C9 conclusions stand,
> including "no fingerprint, A/B or end-to-end harness shifted under
> sub-item 2": the claims fan (`0dff17e`) was the first suspect and was
> **exonerated** by surgical revert. What is retracted is the *sweep result*,
> not the attribution. The likely mechanism is that the C9 sweep was the run
> recorded as orphaned/unreadable, and the count was reconstructed rather
> than read. **Lesson: an inherited red is still a red — a sweep result must
> be read from the log it produced, never reconstructed from what the wave
> believes it changed.** Wave 11's log is preserved at
> `scratchpad/w11/baseline_head_114b92f.txt`.

## C-SEQUENCE WAVE 11 (A/M10 + A/M7), 2026-08-03

The motion-economy pair, taken in the order the plan specified: the safety
half first, because a link that stays low is only safe if the descent model
is resolution-honest. Both halves landed. Neither landed the way it was
scoped, and the wave spent its first hour on somebody else's red.

**Commits**

| # | Hash | Scope | Diffstat |
|---|------|-------|----------|
| 1 | `f19bb1f` | re-pin `crease_own_region_pr6b` taper arm + errata to waves 9b and 10 | 2 files, +52 / −2 |
| 2 | `74571b5` | `DexelGrid::conservative_top` + `max_conservative_top_z_in_disc` + 3 stamping kernels + `optimize_entry_descents`; new `tests/descent_resolution_stability_am10.rs` | 6 files, +330 / −29 |
| 3 | `a2741a5` | `ToolpathStats::retract_trips` + `compute_retract_trips` + narration + viz/MCP passthrough; `measurement::swept_footprint_area` / `swept_footprint_mm2_per_s`; new `tests/retract_trip_channel_am7.rs` | 10 files, +971 / −35 |
| 4 | `6cc2d1e` | `ScallopParams::intra_pass_hookup_mm` + `relink_fragments` wiring + config/catalog; new `tests/scallop_intra_pass_relink_am7.rs`; 14 harness opt-outs | 11 files, +523 |
| 5 | *(this commit)* | this entry, checklist step 10 | |

### The baseline stopped the wave, and the first two suspects were innocent

The mandatory pre-wave sweep found **three** failing targets where wave 10
reported two: the 3 known adaptive3d `--lib` reds, the known environmental
`wanaka_suggest_baseline`, and
`crease_own_region_pr6b::production_unified_finish_output_is_byte_identical`.

Attribution took three attempts.

*C9's `0dff17e`* (the per-point claims fan) was the obvious suspect and the
one the orchestrator and this agent both assumed: it is the only commit in
the window whose own log says it changes emitted geometry, and the fixture
sets `pencil_claims: true`, so it runs straight through the changed code.
Reverting its three source files reproduces the failing 1874 **exactly**.
Innocent.

The mover is **`dde7a54` — M4's iso-field gouge fix**, five commits earlier.
Restoring `scallop.rs` + `scallop_isofield.rs` to `dde7a54^` reproduces the
pinned `(1855, 0xe031…)` to the bit. M4 knew it had moved scallop geometry
and re-pinned the scallop-side fingerprint in `a376b1e`; what it missed is
that `UnifiedFinish` routes its MidSteep band through scallop, so a **second
pin lives in a file whose name says "crease"**. C9 inherited the red and
reported the crate clean.

Adjudicated on the geometry, not on the story. Decomposing both toolpaths
into cutting fragments:

| | pre-`dde7a54` | HEAD |
|---|---|---|
| fragments | 88 | 88 |
| DEEP groove (x∈[3.5,6]) | 336 moves / 644.2994 mm | **identical** |
| SHALLOW groove (x∈[−8.5,−3.5]) | 708 moves / 357.3665 mm | 727 / 358.7768 (+0.39%) |

All 19 added moves sit in 4 of the 5 fragments inside the shallow 4 mm 45°
V; no pass added, none dropped, nothing outside the groove rims. Decisively,
**none enters new depth territory** — the groove's long fragment already
reached z = −1.7891 and still does, while the three short ones converge
toward it (−1.6513/−1.7358/−1.6931 → −1.7802/−1.7641/−1.7789) without
passing it. Denser, more consistent sampling of the same surface.
Legitimate; re-pinned with the A/B in the commit body, errata appended to
both wave sections.

**Two rules earned here.** A fingerprint lives wherever the geometry is
EMBEDDED, not only where the subject module is named — grep the crate before
declaring a re-pin complete. And an inherited red is still a red: a sweep
result must be read from the log it produced, never reconstructed from what
a wave believes it changed.

### A/M10 — the pad was the wrong shape, so it was deleted

Mechanism chosen: **the plan's option 2**, a conservative sliver-aware model
that is resolution-independent. Option 1 (snapshot at the simulation's
resolution) is not implementable in principle — generation precedes the
user's choice of verification resolution; `gen_initial_stock` simply
inherits whatever `run_simulation` last used, which is 0.5 headless and 0.1
under GUI auto on a Ø1 tip. Option 3 (per-candidate fine sampling) needs
prior-toolpath geometry threaded into `dressup`, which holds only a stock
snapshot.

The root cause is sharper than "aliasing". Sub-cell coverage blends a
partly-swept cell DOWN toward the cut floor, so an uncut rib narrower than
one cell reads as a half-cut column — at a height with no relation to the
cell size. A `2 × cell_size` pad is the right shape for a ridge CREST and no
shape at all for that class, which is why padding cleared only ~25% of the
grazes and why the residue was unbounded.

`DexelGrid::conservative_top` answers the question the code was actually
asking — *how high can material be ANYWHERE in this cell* — lowered only on
complete coverage and only to an upper bound of the cutter surface across
the whole cell. It over-estimates pointwise, and refining the cell can only
LOWER it, so it converges downward to the truth instead of jumping around
it. The pad is gone.

Measured on a synthetic rib fixture (0.30 mm wide, 18 mm tall, between two
swaths — not wanaka):

| | before | after |
|---|---|---|
| rib top read at 0.5 mm / 0.1 mm | 12.125 / 20.000 | unchanged (the grid is the grid) |
| **collisions at 0.5 / 0.25 / 0.1 mm** | **[0, 1, 1]** | **[0, 0, 0]** |
| descent over fully-swept ground | z = 5.000 | **z = 4.000** |

The before-column is the TP15 signature at fixture scale: clean at the
resolution it was planned against, dirty at every finer one. Entry time does
not regress — it improves, by exactly the deleted pad. Conservatism is local
to the sliver, pinned by a counterweight test so a trivially-safe-everywhere
ceiling cannot pass, plus a non-vacuity guard that the fixture really does
hide the rib from the coarse grid.

### A/M7 — the prize is real, and it is not bankable yet

Instruments first, so the conversion could not be judged with the tools that
produced the void number. `ToolpathStats::retract_trips` carries the count
with its in-node/between-node split (X-19 throughout: `None` is unmeasured,
and an untrustworthy split reports unmeasured rather than zero), counting
rule copied bit-for-bit from `v3_cascade_ab`. First census on a synthetic
two-node UnifiedFinish: **82 of 85 trips in-node, 96.5%**, against wanaka's
99.7% — the premise is STRUCTURAL, not a property of one job.
`swept_footprint_area` replaces the centreline bins with a radius-aware XY
disc carrying its own `MeasurementProvenance`, per `MEASUREMENT_DOMAINS.md`
§7 option (a), named a *footprint* so nobody reads it as fresh area.

The conversion target was scallop's discrete-ring branch, which retracted to
safe Z at **every** ring junction unconditionally and which nothing above it
relinked. A/B on a corrugated all-over fixture, Ø3 ball:

| | off | on (3.0 mm) |
|---|---|---|
| retract trips | 24 | **3** (−87.5%) |
| cycle time | 407.91 s | **166.18 s** (−59.3%) |
| swept footprint | 1910 mm² | 1906 mm² |
| mm²/s | 4.6825 | **11.4693** (+144.9%) |
| collisions @ **0.1 mm** | 0 | **0** |

**It ships default OFF**, because the third gate fired:
`surface_link::relink_fragments` drops **exactly one cut position per link**
— 21 losses across 21 converted junctions, set-membership and
order-independent, so a real hole and not a reordering. A 59% saving is
exactly the size of prize that gets a defect waved through.

The defect is not scallop's. It lives in the shared relinker that also backs
`unified_finish::intra_region_hookup_mm` — shipped, also default-off, and
carrying the same loss for anyone who enables it. **Second time this
programme has found a dial defaulted off ahead of an A/B that would have
found its defect.**

Worth more than the feature: **the first version of that gate asserted the
wrong invariant.** It compared `FinishingCut`-*labelled* moves and found 2
differences, which turned out to be honest re-labelling — a link legitimately
replaces a plunge. Widening it to cut POSITIONS, what actually reaches the
workpiece, turned 2 cosmetic hits into 21 real ones. Asserting the label
instead of the cut would have shipped this at +144.9% and a gapped surface.
The test pins the one-per-link RATIO, not the count, so a changed mechanism
fails rather than silently re-baselining.

### Gates

`cargo fmt --check` clean. `cargo clippy --workspace --all-targets -D
warnings` zero. `cargo test -p rs_cam_core --lib` 2221 passed / 3 failed —
the three known adaptive3d reds and nothing else (2216 before; +5 new unit
tests). Sentries run individually: `descent_resolution_stability_am10` 3/3,
`retract_trip_channel_am7` 4/4, `scallop_intra_pass_relink_am7` 3/3,
`crease_own_region_pr6b` 3/3, `capability_link_moves_safety` 17/17,
`transform_provenance_fingerprints` 3/3, `dressup_span_invariants` 4/4,
`sub_cell_stamping_fa` 6/6, `dexel_stock_z_frame_f024` 3/3,
`scallop_isofield_gouge_m4` 3/3, `scallop_candidates_m4` 3/3,
`common_fixtures_smoke_c6` 2/2, `standing_material_channel_am9` 4/4,
`param_sweep` 56 ignored (unchanged).

One deliberate re-pin beyond the pr6b adjudication:
`optimize_entry_descents_uses_dexel_ceiling_above_mesh` 9.0 → 7.0, which is
exactly the removed pad on untouched stock.

The registry caught a real omission: adding `intra_pass_hookup_mm` to
`ScallopConfig` without a matching `ParamDef` failed
`operation_schema_params_match_params_with_nulls_for_every_op`. The
"miss nothing or don't compile" net working as designed, one layer down.

### Honest limits

* **No live wanaka validation.** Both halves are gated on synthetic
  fixtures. The A/M10 rib and the A/M7 corrugation are built to isolate one
  mechanism each; neither says what the numbers are on a real part.
* **The A/M7 prize is measured but unclaimed.** −59.3% is what the
  conversion is worth once the relinker's position loss is fixed, not what
  the shipped default delivers, because the shipped default is off.
* **The relinker defect is diagnosed, not fixed.** It is an off-by-one in
  the re-emit path; the fix belongs with whoever owns `surface_link`, and
  `scallop_intra_pass_relink_am7` is its target and regression net.
* **A/M10's conservative bound costs memory**: one extra `f32` per cell on
  every dexel grid, cloned with the grid into `prior_stocks`. Negligible at
  production cell sizes (~3.8 MB at 0.25 mm over 300×200 mm), but it scales
  with the 16 M-cell cap.
* **`capability_link_moves_safety` was extended only by opt-out.** The new
  scallop link path is gated by its own file rather than folded into that
  suite; merging them is the tidier end state.
