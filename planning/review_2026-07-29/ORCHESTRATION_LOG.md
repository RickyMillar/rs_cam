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
