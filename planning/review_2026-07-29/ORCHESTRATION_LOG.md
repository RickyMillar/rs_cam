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

Remaining programme work (post-checkpoint): H2 routing slices (PR-4..7),
A/M6 claims_reference (PR-3a), PR-8..9 resolution consumers, M3 classifier,
M4 scallop (Checkpoint C), M5 offset_polygon (Checkpoint D), A/M7 retract
trips + A/M10 descent/resolution, H4 re-measurement ledger (Checkpoint E),
L1 docs sweep. Deferred defect tasks: #12 tapered-pencil SelfReferenced,
#13 LH-1..LH-4 measurement hazards, #15 Auto-heights VerySteep drop (+ new:
RampFinish cone gouge, tip-float silent residual, 0.9 µm segments).
