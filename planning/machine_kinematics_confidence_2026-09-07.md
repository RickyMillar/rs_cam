# Machine kinematics as a first-class analysis dimension (2026-09-07)

Status: PLAN — awaiting operator go. Branch target: `machine-kinematics-confidence`.

## Why this exists (first principles, not the incident)

A CAM analysis layer exists to give the operator justified confidence before
pressing go: predict the cut (geometry), the load (tool physics), the time
(machine physics) and the headroom (how much faster it can safely go).
rs_cam's cut-physics side is strong: dexel simulation, chipload / power /
deflection gates, entry-load, air-cut. Its machine-physics side is partial:
per-axis acceleration is modelled and drives the runtime integrator, but the
machine's per-axis maximum rates are not stored anywhere, and the machine's
kinematic reality is never surfaced to the operator as an analysis dimension.
The operator learned this by hand-analysing an exported `.nc` with a script.
That analysis belongs inside the tool, for every movement, every time.

The operator's framing: not "safety" alone, but confidence — know how fast the
machine can be pushed, and be told when a command is unrealisable.

## What the incident actually was (verified 2026-09-07)

Measured on the wanaka front rough export (`wanaka200_front_scrap_test.nc`):
Z-descent speed on fed moves reached 1807 mm/min against the op's 512 mm/min
plunge rate; the worst were pure-vertical fed descents (dZ = segment length)
carrying the lateral chipload-band-max feed. Three distinct things were
conflated in the first reaction:

1. **A machine-model gap.** `MachineKinematics` stores per-axis acceleration
   (`acceleration_xyz_mm_s2`, GRBL `$120/$121/$122`) and the integrator uses
   it direction-aware (`effective_accel = min_i(a_i / |dir_i|)`,
   `machine_kinematics.rs:221`). It stores NO per-axis maximum rate. The GRBL
   `$$` importer parses `$112` into `GrblImport.max_z_feed_mm_min` (tested to
   read `Some(1000.0)` from the 2026-05-26 capture) and the value is then
   dropped: nothing on `MachineProfile` holds it and nothing reads it. The
   integrator caps cruise velocity only by the global (X/Y) `max_feed_mm_min`.
2. **The root cause of the 1807.** The feed modulator skips plunges by INTENT
   (`should_skip_modulation`, `feed_modulation.rs:333`: Rapid, Retract,
   Drilling, `EntryPlunge`, LeadIn, LeadOut; `skipped_intents_keep_commanded_feed`
   proves a tagged plunge keeps its feed). The adaptive3d rough emits its
   vertical step-down / re-entry descents as plain cutting moves without the
   `EntryPlunge` tag, so they fall through the intent filter and are lifted to
   the lateral band maximum. Intent-keyed protection; untagged geometry escapes
   it. A downstream metric would report this forever; the fix is at the source.
3. **The confidence instrument** the operator actually wants: per toolpath,
   how hard is the machine working, where is the headroom, and which commands
   the machine cannot realise.

### Physics correction to the first-draft metric

A sloped fed descent is a RAMP: the flutes cut laterally at the ramp angle and
the chipload gate already governs it. Its Z-component exceeding `plunge_rate`
is not itself a hazard. Only a VERTICAL-DOMINANT fed descent is a plunge
(centre-cutting, chip evacuation, tip load) and must respect `plunge_rate`. A
raw "Z-descent rate" metric would false-alarm on every steep ramp. Classify
every fed move by angle from vertical into a motion class:

- `Plunge`     — within `PLUNGE_CLASS_HALF_ANGLE_DEG` (proposed 15°) of vertical
- `Ramp`       — descending, steeper than the flat threshold, not plunge-class
- `Lateral`    — Z-component below the ramp threshold
- `Retract`    — ascending / rapid (not graded)

Bands per class are physical: `Plunge` against `plunge_rate` (and, later, the
tool's centre-cutting capability); `Ramp` against the chipload band plus the
tool's maximum ramp angle; `Lateral` against the chipload band. The machine's
per-axis rates and acceleration cap all classes.

## The plan — instrument before change

Order follows the repo's own rules: commit the measuring instrument before
changing the thing it measures (`feedback_commit_instruments_before_gates`),
and measure emitted motion, never the plan (`feedback_measure_emitted_motion`).

Orchestration (reviewer-ruled): Phase 1 runs SOLO and self-verifies (it owns
the cargo lane). Wave 2 = Phase 2 kernel ‖ Phase 4 scaffold (diagnostic ids +
surfacing plumbing on disjoint files, no cargo), then one sequential verifier.
Wave 3 = Phase 3 alone with its paired A/B — it changes emitted feeds and must
be measured, not raced. Implementers do not commit; the orchestrator commits
per verified phase.

Phase 3 guard placement: it sits BEFORE the band lift — a `Plunge`-class move
is capped at `min(commanded, plunge_rate)`. `should_skip_modulation` stays
intent-based; geometry is not folded into it, or the distinction between
"operator-tuned, leave alone" and "physically a plunge, cap it" is lost.
Sentries at 14° and 16° bracket the 15° class threshold.

Phase 3 implementation pins (read 2026-09-07, `feed_modulation.rs`):
- Insertion point: `adaptive_feed_modulate`, inside the per-move loop, AFTER the
  strategy match returns `(new_feed, binding)` (`~636-650`) and BEFORE
  `outcome.per_move.insert(i, ..)` (`~652`). The guard reads the move's
  geometry (`toolpath.moves[i].target − toolpath.moves[i-1].target`, the seed
  move has no predecessor) through `kinematic_utilization::classify_move`
  (Phase 2, one construction site) and, for `MotionClass::Plunge`, sets
  `new_feed = new_feed.min(plunge_rate)` with binding
  `BindingConstraint::PlungeRate`. `should_skip_modulation` is untouched.
- `ModulationContext` gains `plunge_rate_mm_min: f64` (the op's own; the
  session build site at `session/compute.rs:~2907` already has
  `tc.operation.plunge_rate()` in scope — it is read at `~4362`). Construction
  sites to update: `session/compute.rs:~2907`, the in-file tests in
  `feed_modulation.rs`, `tests/constrained_max_modulation_f039.rs`,
  `tests/lead_in_out_feed_rates_f040.rs`.
- `BindingConstraint` gains `PlungeRate`; exhaustive matches / renderers to
  extend live in `feed_modulation.rs`, `session/compute.rs`,
  `tool_load/verdict.rs`, and the tests `constrained_max_modulation_f039.rs`,
  `chipload_boundary_g_chip_ulp.rs`.
- Paired A/B instrument: `kinematic_utilization::analyse_toolpath` over
  EMITTED motion, i.e. only AFTER a simulation. Verified by the Wave 2
  verifier (an earlier note here was wrong): the modulator modulates a
  clone and `session/compute.rs:~3095-3106` writes it back into
  `results[idx]` after the simulation, so post-sim the stored result carries
  emitted feeds (`SimulationCutTrace::modulated_feeds` yields the same;
  re-applying is idempotent); pre-sim it is the PLAN. Bars on the wanaka
  front rough: `plunge.peak_ratio` falls from ≈1.95 (the 1807 command after
  the `$112 = 1000` clamp) to ≤ 1.0; every `Lateral`/`Ramp` move's feed is
  byte-identical to the pre-guard arm; total fed-time delta reported.
- Wording consequence for Phase 4 surfaces: `narrate_toolpath` and the GUI
  pill are reachable on a generated-but-not-simulated toolpath and then read
  planned feeds beside gate pills saying `SimulationRequired`; five texts
  (`diagnostics/ids.rs:60,63`, `sim_triage.rs:393,886,943`) assert
  "emitted" unconditionally. Phase 3 keys the wording on whether a
  simulation with modulation has run (planned vs emitted).

Deferred from Wave 2 (verifier's flag): `SimulationState::cached_load_report`
returns `report.clone()` per call from three panels per frame, and `Clone`
still copies the kernel's `moves` vec (~1.5 MB per 12.6k-move toolpath per
panel per frame); `#[serde(skip)]` bounds the wire, not memory. Triage itself
is memoised on `edit_counter`. Acceptable now; a perf follow-up.

### Phase 1 — complete the machine model (~½ day)

- `MachineKinematics::max_rate_xyz_mm_min: Option<[f64; 3]>` — GRBL
  `$110/$111/$112`. All three axes, mirroring `acceleration_xyz_mm_s2`
  exactly (`#[serde(default)]`, `None` = today's behaviour, byte-identical
  for every existing preset and project file).
- `MachineKinematics::effective_max_rate(dir) = min_i(rate_i / |dir_i|)` —
  the direction-aware velocity cap, the exact analogue of `effective_accel`.
  The integrator caps each move's cruise velocity by it, so a Z-dominant move
  is throttled by the slow `$112` while a planar move keeps the X/Y rate.
- GRBL import: `GrblImport` already yields `max_feed_mm_min` (max of
  `$110/$111`) and `max_z_feed_mm_min` (`$112`). Populate `max_rate_xyz` from
  `$110/$111/$112` at the apply site (GUI + MCP `import_machine_settings`),
  so importing `$$` once persists all three.
- The tuned kinematics fn at `machine_kinematics.rs:~205` (the one carrying
  `Some([500, 500, 270])` and the 2026-05-26 `$$`-capture comment) gains
  `max_rate_xyz = [10000, 10000, 1000]`. **Do NOT attach kinematics to any
  `MachineProfile` preset**: every preset sets `kinematics: None`
  (`machine.rs:204`) and falls back to `generic_wood_router`; the `[500, 500,
  270]` the wanaka project reports is an inline `$$` import saved in its
  `.toml`. Attaching would silently change runtime predictions for every
  project on the VFD preset — the blast radius the byte-identical sentry
  forbids. The live project receives its rates by `$$` re-import or through
  the extended `set_machine_kinematics` MCP tool (new optional
  `max_rate_x/y/z_mm_min`).
- **One physics site (reviewer ruling).** P1 exposes
  `pub fn move_kinematics(length, dir, v_in, v_out, v_cmd, &kin, max_feed) ->
  MoveKinematics { peak_mm_min, binding: KinematicBinding }` and the
  integrator's per-move step CALLS it. Phase 2's instrument consumes the same
  struct, so the A/B verdict (instrument) and the modulator's `KinematicReach`
  cap (integrator) can never read two different trapezoids.
- Sentries: (a) unset ⇒ integrator output byte-identical to today on the
  existing kinematics fixtures; (b) a pure-Z move with `$112 = 1000` and a
  commanded 1807 integrates at 1000; (c) a planar move is untouched by `$112`;
  (d) `$$` round-trip persists all three rates; (e) the wanaka Back Rough
  wall-clock calibration (827 s, 2026-05-26, `machine_kinematics.rs:180`) is
  re-checked — the prediction must not get worse; it should get better on the
  Z-heavy passes.

### Phase 2 — the instrument kernel (~½ day)

A pure function over an emitted `Toolpath` + `MachineKinematics` + the op's
`plunge_rate`, no simulation required:

```
per move:   motion class, commanded feed, achieved feed (from the integrator's
            trapezoid: v_in / v_out / effective accel / effective max rate),
            binding constraint (FeedBound | AccelBound | RateBound{axis} |
            JunctionBound), plunge-class ratio = achieved_z_rate / plunge_rate
per toolpath: time-weighted commanded vs achieved, fraction of cutting time
            by binding constraint, plunge-class population + worst, ramp-angle
            population + worst, an `is_measured()` Observation (an op with no
            fed moves is absent, never "clean")
```

This is the Python analysis the operator ran, ported into core and pinned. It
is the A/B instrument for Phase 3. Sentries: pure-vertical move at 3×
plunge_rate ⇒ `Plunge` class, ratio 3.0; 10° ramp at high vector feed ⇒ `Ramp`,
not plunge-graded; all-lateral pass ⇒ plunge population absent; a 0.58 mm
average-move finish at 925 mm/min ⇒ roughly half `FeedBound` (the measured
wanaka figure, 52 %); the real wanaka rough trace reports the 1807 plunge-class
peak.

### Phase 3 — fix the modulator at the source (~½ day, paired A/B)

- Add a GEOMETRIC guard to the modulator: a `Plunge`-class move is capped at
  the op's `plunge_rate` and never lifted to the lateral band, whatever its
  intent tag says. Keep the intent skip (it is correct where the tag exists).
- Optionally tag the adaptive3d vertical descents `EntryPlunge` at the
  generator (the tagging gap), as a separate commit, so the intent path is
  also right.
- Phase 1 already tightens the modulator's existing `KinematicReach` cap
  (`feed_modulation.rs:465`) because the integrator now knows `$112`.
- Blast radius: emitted feeds change on every modulated project that has
  untagged vertical descents. Paired A/B with the Phase 2 kernel on the wanaka
  front rough: plunge-class peak must fall from 1807 to ≤ 512; lateral
  feeds must be unchanged; total runtime delta reported, not assumed.

### Phase 4 — surface it, two-sided (~1 day)

Per-toolpath **kinematic utilisation** in the places the operator already
reads: `get_diagnostics` triage, `get_tool_load_report`, `narrate_toolpath`,
the GUI diagnostics panel, the CLI `project` report.

- Headroom reading: "runs at 78 % of commanded feed; 52 % of cutting time is
  feed-bound — a constant-chipload rpm+feed rise would land ~12 % faster; the
  remaining 48 % is accel / Z-rate-bound and will not move." (The finish
  question of 2026-09-07, answered automatically for every op.)
- Over-command reading: commands the machine clamps (commanded > effective
  max rate) reported as wasted intent, per axis.
- Plunge-class backstop finding, non-blocking (`Caution` at > 1×
  `plunge_rate`, `Critical` at > 2×), `fix: None`, in the `actions` list
  beside `entry_load`. After Phase 3 it should be quiet on the shipped
  fixtures — a monitor that is silent because the generator is right, and
  that speaks the day a generator regresses.
- Not an export-blocking gate. Escalation to blocking is its own later
  decision.

## Sizing and risk

~2.5 days across four independently shippable phases. Risk concentrates in
Phase 3 (feeds change) and is contained by the Phase 2 instrument and the
paired A/B rule. Phase 1 is byte-identical when unset; the Shapeoko preset
change is deliberate and calibrated against recorded wall-clock data.

## Phase 3 result (2026-09-07, measured)

Paired A/B on the wanaka front rough (0.5 mm, ConstrainedMax, aggr 1.0; the
op's dials are plunge 541 / feed 750 mm/min — the "512" above was the earlier
hand-analysis figure). Arm A = guard disabled (`plunge_rate_mm_min =
f64::INFINITY`), arm B = guard on:

| | A | B |
|---|---|---|
| `plunge.peak_ratio` | 1.8484 (= 1000/541) | **1.3863** (= 750/541) |
| `plunge.over_1x` | 125 | 7 |
| — modulator-visited | 118 | **0** |
| — `EntryPlunge`-tagged (intent-skipped) | 7 | 7 |
| fed time | 2056.1 s | 2065.0 s (+0.43 %) |
| Lateral/Ramp feeds | — | **byte-identical** (6116 lines) |

The guard removed every over-limit plunge it is permitted to touch. **The
bar `peak_ratio ≤ 1.0` as written spanned two owners**: the residual 7 are
correctly tagged `EntryPlunge` moves descending at the op's 750 mm/min CUT
feed instead of its 541 mm/min plunge rate — a GENERATOR defect the guard
is forbidden to touch by the reviewer ruling that keeps
`should_skip_modulation` intent-based. Strong-but-uninstrumented
attribution: `crates/rs_cam_core/src/boundary.rs:311-316` re-tags a cutting
move as `EntryPlunge` on a boundary crossing while preserving its cut feed
(`feed_rate_of(&m.move_type)` → `feed_to_with_intent(.., EntryPlunge)`);
the adaptive3d peck ladder itself uses `params.plunge_rate`
(`adaptive3d/path.rs:89-101`). Test 9 is scoped accordingly: modulator-
visited over-1× == 0, and whole-population peak < 1.5 against the pre-guard
1.848.

Follow-ups opened by Phase 3:
- **G-BOUNDARYPLUNGE — FIXED 2026-09-07 (pre-merge).** The clip walk
  `boundary::clip_toolpath_to_boundary_set_with_provenance`
  (`crates/rs_cam_core/src/boundary.rs:332-423`, re-entry arm at `:369-390`)
  now re-emits the re-tagged
  `EntryPlunge` at the OPERATION's plunge rate instead of the preserved cut
  feed. The rate is threaded in as a new `plunge_rate_mm_min: Option<f64>`
  parameter on that walk, on `boundary::clip_annotated_to_boundary_set`, and
  on `ProjectSession::apply_boundary_clip` / `..._multi`; the three session
  call sites and the two GUI-worker call sites pass
  `Some(operation.plunge_rate())`. `None` means the caller holds no
  operation (the two convenience wrappers `clip_toolpath_to_boundary` /
  `clip_toolpath_to_boundary_with_provenance`, which no production path
  uses) and keeps the old cut feed; a non-finite or non-positive rate is
  read the same way, mirroring the Phase 3 guard's own disable condition.
  The rate is NOT capped to the cut feed — a generator-emitted plunge takes
  the plunge dial as commanded, exactly like the adaptive3d peck ladder. A
  re-entry that also moves in XY is plunge-rated too, deliberately: it is
  tagged `EntryPlunge`, so the modulator will never touch it. Sentry:
  `crates/rs_cam_core/tests/boundary_reentry_plunge_rate_g_boundaryplunge.rs`
  (red-then-green: the pre-fix walk emits F).
- **Narration source** — MCP `narrate_toolpath` narrates `state.gui.toolpath_rt`,
  the worker's PRE-modulation IR, so its kinematics sentence reads "planned"
  even after a simulation; export reads `session.results` (emitted). Same
  class as G-MODEXPORT. Ruling 3 pinned narration to one source — the wrong
  one for emitted readings. Fix: narrate the emitted result when a trace
  exists. **Still deferred.** The misleading *string* is fixed
  2026-09-07 (pre-merge): the Planned case on this surface no longer borrows
  `FeedsProvenance::qualifier()`'s "planned feeds — run a simulation for
  emitted", an instruction that cannot change this reading. It now says
  `planned — this surface narrates the pre-modulation plan; the emitted
  reading is in get_tool_load_report after a simulation`
  (`crates/rs_cam_viz/src/app/mcp.rs`, `kinematics_narration_sentence`). The
  CLI and GUI keep the shared wording, where it is true.
- The guard also caps an untagged zero-engagement vertical move that the
  strategy left at commanded (sentried in Phase 3's follow-up).

## Deferred, deliberately

- Tool centre-cutting capability and maximum ramp angle as tool-library
  fields (the `Plunge` / `Ramp` bands will consume them when they exist).
- Blocking export on the plunge-class finding.
- Per-axis rates for machines other than the Shapeoko presets (they arrive
  via `$$` import).

## Operator-facing outcome

Import `$$` once (or use the preset). Every simulation then reports, for every
toolpath and every movement: what the machine will actually do, where it is the
bottleneck, where there is headroom to push, and any command it cannot
realise — with the plunge hazard as one corner of that same instrument.

## Findings from the roughing A/B (rs-cam-38, 2026-09-07 evening)

Results: `planning/roughing_strategy_ab_results_2026-09-07.md` (+ artifacts dir).
Eight rough-only arms + three finish-stock runs on the wanaka200 fixture,
0.2 mm, all 0 collisions, all gates Within, `feeds_provenance = emitted`.

- **Kinematic answer (the instrument's first field use):** EVERY arm is
  ≥ 97 % feed-bound; `machine_bound` ≤ 0.7 % on parallel arms, 3–4.5 % on
  spiral arms only. The belt router's acceleration is NOT the binding limit
  on this terrain — the 0.025 mm/tooth rubbing-floor feed clamp is.
- **Hypothesis answer:** no arm reaches 40 % air (best 55.3 %); the residual
  is IN-CUT drape air (51 % of in-cut samples < 0.02 engagement on baseline),
  not inter-island rapids (baseline rapid 11.4 km, not the scoping doc's
  42.7 km). A boustrophedon fill (Arm F) would drape through the same air.
- Practical ranking: C2 (by_area ordering) −8.1 % with finish stock
  byte-identical to baseline; C2E (by_area + DPP 5.46) −18.1 % with finish
  peak bite at baseline but an 8.19 mm (1.37×D) peak axial bite; BE −25.7 %
  but air 56.5→73.8 % and 1.6× finish peak bite. Raised stay-down REFUTED
  (+12.5 %: fed links through air cost more than retract trips); ramp entry
  −5.8 % worse; spiral matched `recommend_clearing_strategy`'s prediction
  (1.244× predicted, 1.238× measured).

Open flags from the run (not investigated; no heavy work until the
terminal is stable):
1. **G-AIRDENOM** — on every one-op run `air_cut_pct_of_total_runtime` >
   `air_cut_pct_of_cutting_time` (56.5 > 36.3), the OPPOSITE of the
   documented relation (rapids excluded ⇒ cutting-time reading ≥ total-
   runtime reading); it flips to the documented order on the two-op finish
   runs (33 < 42). Mathematically impossible if both share one numerator —
   one of the two percentages has a different population. Instrument
   defect; find which surface aggregates differently before citing either.
2. `project.entry_load` absent from the finish's diagnostics — expected
   when no entry exceeds the bar (it is emitted only on rest-driven ops
   whose entries exceed), so probably not a defect; confirm the finish is
   rest-driven and its entries were under the bar.
3. `mill_shallow_areas = true` produced no visible sub-pass on any arm —
   possibly inert without `shallow_stepdown`; check before relying on it.

## G-AIRDENOM resolved as a DEFECT — and it reverses Reading 2 (2026-09-08)

Read-only investigation: `planning/ab_instrument_flags_2026-09-08.md`.
Both air percentages share one numerator (`air_cut_time_s`) and one
population. The inversion is possible only because `cutting_runtime_s` >
`total_runtime_s`: `air_cut_time_s` and `cutting_runtime_s` are naive dexel
seconds at the PRE-modulation commanded feed (`dexel_stock/simulation.rs:
~937`), while `total_runtime_s` is overwritten with the kinematics-
integrated wall clock at the MODULATED feed (`compute/simulate.rs:~1533`,
`session/compute.rs:~3184`). A mixed time base. The documented invariant
(`simulation_cut.rs:592-593`, `:628`, and the CLAUDE.md caveat) is false
whenever modulation changes the feed; the sign flips with the modulator's
direction (rough sped up → total % > cutting %; finish slowed → reverse).

Consequence for the roughing A/B: absolute air seconds
(`pct_total × total_runtime_s / 100`, comparable because every arm
commanded 750 mm/min) — A 1457, B 1450, C 2687, C2 1311, D 1662, E 1321,
BE 1414, **C2E 1241**. B/E/BE/C2E did NOT raise air; they shortened the
denominator. **C2E has the least absolute air of any arm and near-baseline
finish stock → C2E is the strict winner**, not C2. Arm C (raised
stay-down) is the only genuine air loser. BE's 1.6× finish-bite penalty is
a geometry measure and stands.

Fix DONE (G-AIRDENOM commit, 2026-09-08): all three time figures share the integrator's modulated wall clock, rebased per sample. Original note — put all three time figures on one base —
integrate `air_cut_time_s` and `cutting_runtime_s` at the modulated feed
(or compute the total-runtime percentage against the naive total) — then
correct the two doc comments and the CLAUDE.md caveat (replacement wording
is in the report). Until then, compare arms on ABSOLUTE air seconds, never
on either percentage.

Flag 2 (`entry_load` absent) = method error: the finish IS rest-driven
and eligible, but the finding lives only in triage `actions`
(`get_diagnostics`), never in `get_toolpath_diagnostics`; and drop_cutter
sets no `MoveIntent` with `entry_style = none`, so it may also be NOT
MEASURED. Flag 3 (`mill_shallow_areas`) = by design at dpp 5.46: the
level ladder collapses to one level so the only sub-level is below the
deepest surface (empty grid); plus the sub-pass dispatch emits no DepthPass
span, so a working sub-pass is invisible to `per_depth_pass` — a separate
instrument gap.

## Arm G finish pair + the real prize (rs-cam-38, 2026-09-08)

Arm G rough + R1.5 finish, oak, 0.2 mm: pair total 12739 s vs A pair 14245
(−10.6 %), BE 13545, C2E 13783; 0/0 collisions. The flat raster hands the
finish a slightly deeper but LESS peaky surface (crosses_standing peak 2.59
vs A 2.93, BE 3.14); finish gates Within, modulator lowered the finish's
feed (median −38 %, chipload_max binding). **The finish is ~90 % of the
pair (11415 of 12739 s).** Roughing strategy can only move the job ~10 %;
the finish is the prize — which is the operator's deep-DOC + modulation
thesis. Added to the plywood study: the 6 mm flat drop_cutter raster as
the ONLY pass (stepover 1.0 / 1.5 / 2.0) vs the rough+finish pair, with a
flat-tool cusp proxy (stepover × tan(slope), median + p90 slope), and the
same single-pass with the R1.5 ball.

Instrument gaps logged: (a) `EntryLoadObservation::is_measured` is not on
the `get_diagnostics` wire — an absent `project.entry_load` cannot be told
apart from NOT MEASURED (drop_cutter sets no MoveIntent with
`entry_style = none`); (b) simulation order is toolpath INDEX order, not
dependency order — a finish at a lower index than its rough simulates
first (the peer lost one run to this).

## Plywood single-pass matrix — the thesis holds (rs-cam-38, 2026-09-08)

Nine ball-tool arms, 0.2 mm, modulation ON, all 0/0 collisions, all gates
MODELED (except waterline). Whole-job single-pass times on plywood vs the
oak rough+finish pairs (A 14 245 s, G 12 739 s):

| arm | time | air | notes |
|---|---|---|---|
| iso-scallop R2.0 h0.27 | **2389 s** | 36.8 % | chipload Within (un-extrapolated 6000 scallop row), deflection 0.007, one 2.7× plunge (G-BOUNDARYPLUNGE class, pre-fix binary) |
| drop_cutter raster R1.5 s1.5 | **3274 s** | 9.8 % | 2 retracts, ZERO triage actions — passes every bar as written; cusp 0.20 mm |
| drop_cutter raster R1.5 s1.0 | 4788 s | | finer cusp |
| spiral R1.5 s1.5 | 3806 s | | 108/197 plunges at 3.6× (pre-fix), 18.7 km rapids from the square clip |
| iso-scallop R1.5 h0.38 | 8001 s | 81 % | 436 rings, fragmented; 1.7× the cutting distance of the R2.0 arm |
| radial 0.6° | 8718 s | 69 % | centre plunge cluster |
| waterline z1.5 | 18 368 s | | see flags below |

Rough-only references on plywood: A 2475 s; C2 (by_area) 2277 s with zero
over-limit plunges. Every arm ≥ 99 % feed-bound; the modulator binds on the
chipload band max on every ball arm. **A ball single pass on plywood is a
4–6× job at a 0.20–0.27 mm cusp.** Caveat stated in the results doc: no
plywood_hardwood tapered-ball pocket row exists, so the ball arms resolved
the MDF parallel row by category scoring (hardness × 0.957) — the band is a
proxy. Flat-tool arms (G-fine) wait for the G-DCFLAT rebuild.

Recommendation for plywood terrain (pending the flat arms and the post-fix
reruns): **drop_cutter with the R1.5 ball at 1.5 mm stepover, modulation
ON, as the only pass** — 55 min against the 4-hour pair, zero findings,
cusp 0.20 mm; iso-scallop R2.0 (40 min) if a 0.27 mm cusp is acceptable.

New instrument/generator flags (ledger):
- **G-WATERLINEAUTO** — a standalone waterline with Auto heights generates
  ZERO moves: `depth_semantics None` makes Auto `bottom_z` = stock top and
  the ladder collapses (the DroppedBandFinding mechanism at
  `config.rs:~1031`, but the standalone op has no band and no finding — only
  `project.generated_empty` fires). Pinning heights 7 / −3 fixed the arm.
- **G-WATERLINELINK** — waterline's helix-classified link class is emitted
  at ~130 mm/min by the generator/dressup (13 700 of 16 867 fed seconds),
  with no modulation summary — that is why the arm took 18 368 s. And
  waterline + the R1.5 tapered ball reads chipload UNMODELED
  (no_vendor_data) while drop_cutter with the same tool matched — the
  family-filter hole again (waterline family), same follow-up as the flat
  case.

Deliverable landed: `planning/deep_doc_modulation_2026-09-08.md` (+ artifacts
dir). Verdict paragraph (rs-cam-38): on plywood the hypothesis PASSES on
the ball tools; the win comes from DELETING the finish, not deepening the
rough (E-ply single-level adaptive3d = 1632 s rough-only still needs the
11 400 s finish); the modulator regulated the chipload band max on every
ball arm (median −22…−31 % on R1.5, −4 % on R2.0), deflection never above
0.020 mm, power 0.015 kW — never load-bound. Reproducibility clean (B15 /
S20 reproduce to the millisecond on reload). Three more ledger items:
- **G-LOADAUTOGEN** — `load_project` auto-generates DISABLED toolpaths too
  (Back Rough / front rough queue on every reload of an arm TOML); cancel
  until idle before touching the project. A disabled op must not generate.
- MCP `screenshot_simulation` cannot produce a lit close-up of cusps (flat
  colour map, no camera/light control) — crops of 4800-px renders with
  autocontrast were used and captioned honestly; a surfacing gap.
- Flat-tool arms, the modulated Arm G rerun and the plunge before/after
  reruns wait for the G-DCFLAT rebuild + MCP reset.

Review corrections to the deep-DOC doc (rs-cam-38, committed with this
note): the pass rule's baseline is the WHOLE job (rough + finish), never the
rough alone; ball arms' air readings carry the degraded-measurability flag
(blind fraction B15 0.14, S20 0.10, B10 0.20); deflection confidence is
`validated` on flat arms and `approximate` on every ball arm; ×D bite uses
6.0 (flat) and 3.0 (tapered-ball tip); absolute air seconds are approximate
where arms did not share one commanded base feed; A-ply 43.8 % and C2-ply
42.9 % are over the 40 % roughing bar. The 4.35× headline was
CROSS-MATERIAL (plywood single pass vs the oak pair) — the same-material
plywood pair is being measured.

**F-LUT2 / G-SUGGESTGATE (ledger, mechanism NOT yet verified in code):** on
every R1.5 drop_cutter arm `apply_feeds` wrote F776 @ 18 500 = 0.0210
mm/tooth and reported it "clamped to the matched band ceiling", while the
post-sim gate's band on the same tool/material read 0.0079–0.0145; the
modulator then cut the feed by a median 31 % to land on 0.0145. Ratio
1.45×. Likely: drop_cutter gives Suggest no axial hint (INTEGRATION.md
"none"), so Suggest derates at a default DOC while the gate derates at the
measured ~9 mm bite. Un-modulated, these arms would have run 45 % over the
gate's ceiling on Suggest's own numbers. Same class as the
probe-the-same-code-path lesson: Suggest (recipe resolver) and the gate
(envelope resolver) must quote one band. Follow-up: verify in code, then
give drop_cutter an axial hint or make Suggest derate on the planned DOC.

**G-CLIAIR (ledger, found by the lane agent, NOT fixed):**
`crates/rs_cam_cli/src/main.rs:~320` prints `air:` as
`trace.summary.total_runtime_s − ts.cutting_runtime_s − ts.rapid_runtime_s`
— the PROJECT total minus one toolpath's slices. That was never the air
time; after G-AIRDENOM it prints 0.0 on a single-toolpath project. The
correct value is `ts.air_cut_time_s`. One-line fix, outside this wave.

## P5 — reach map in the viewport (operator ask, 2026-09-08)

"If it is cheap to calculate the reach map, show it when any finishing op
is selected in the viewport." Spec sketch: for the selected finishing
toolpath's tool, drop the cutter over the model mesh (the drop-cutter
kernel already computes contact heights) and take the gap between the
contact surface and the true mesh; where the gap exceeds the op's cusp
tolerance the spot is UNREACHABLE by that tool. Render as a colour overlay
on the model in the Toolpaths workspace whenever a finishing op is
selected (green reached / red unreachable, shaded by gap depth), with two
numbers in the properties panel: % of surface area unreachable and max
gap. Cache per (mesh, tool, tolerance). Also surface via MCP
(`screenshot_toolpath` / a `reach_map` tool) so an agent can read it.
Stacking radii gives the "minimum ball that reaches X % of the surface"
curve — the tool-choice decision in one picture, and the honest basis for
"a bigger ball at a finer stepover beats a small tip" (cusp table: R2.0 at
s1.5 = 0.15 mm ≈ R1.0 at s1.0 = 0.13 mm, far stronger tip). Existing
pieces to build on: `preview_tier_map` (multitool planner tiers by
radius), `untouched_material_mm2` / `reached_uncut_estimate_mm2`
generation findings, the remaining-stock render.

**Landed 2026-09-08** (`5f665edf`, merged to master `6c532e4d`). The
primitive is NEITHER of the two named above: a one-rung tier map is
identically zero (its residual is tool-vs-finest, and with one tool those
are the same drop) and `TierMap` discards the residual anyway; the rest
centreline needs a prior op. It is built on the drop-cutter contact heights
underneath the tier map — `machined_z = min over CL p of [tip_z(p) +
height_at_radius(|p − (x,y)|)]`, `gap = machined_z − mesh_z` — exact on a
plane of any slope for a ball, a flat and a tapered ball, so no slope
estimate and no abstain-above-75° arm (`cl_offset_bias_mm` is the
spherical-tip law and would read 1.76 mm of phantom gap on a Ø6 flat at
45°). Tolerance = the op's `scallop_height()`, else 0.05 mm
(repo-authored); `stock_to_leave` excluded as an intended offset. Cache
key = mesh `Arc` identity + `ToolShapeKey` + params, capacity 4, no
invalidation code (a tool edit changes the key; a re-import is a new
`Arc`). Fourth lane `ComputeLane::Reach` — the Analysis lane's latest-wins
rule would let a selection cancel a simulation. The overlay REPLACES the
plain model draw for the frame (no z-fight); DXF/SVG curve models draw
through the line pipeline and are unaffected.

Ledger from P5:
- **P5-BLINK**: the overlay hides behind `computing…` on any project edit
  (key is `(ToolpathId, edit_counter)`); the honest fix exports
  `ToolShapeKey` from core so the viz can compare resolved requests.
- **P5-MULTIMESH**: a second 3D mesh model does not draw while the overlay
  is on (no wanaka project has one).
- **P5-STALL**: `REACH_STALL_GRACE` 3 s recovers a stuck `Computing` —
  needed because a scripted backend reports idle the instant after submit.
- **P5-FRAMELOOP**: MCP `reach_map` / `reach_overlay` run on the frame loop
  like `preview_tier_map` and inherit its cancel limitation.
- Cost measured in debug only: 2.28 s cold on 320 k triangles / 119 k
  cells. **Not seen on screen** — live GUI check pending the next MCP
  restart on the rebuilt release binary.

## Before/after on the rebuilt binary (rs-cam-38, 2026-09-08) — closing proof

iso-scallop R2.0 (S20) and spiral R1.5 (SP15) rerun with identical params
at 0.2 mm on the binary carrying the guard + G-BOUNDARYPLUNGE:
`plunge_class_load` GONE on both — S20 1/109 at 2.71× → 0/109 at 1.00×;
SP15 108/197 at 3.61× → 0/197 at 1.00×. Runtime −0.16 s / −6.3 s; every
gate, `crosses_standing`, the modulator's median delta and the achieved
feed reproduce. Doc §2.5 carries the table.

Two faces of the fix, measured: on SP15 the PLANNED (pre-modulation) IR
went 108 → 0 — all 108 were boundary re-entry re-tags, removed by the
generator fix; on S20 the planned IR still carries 65 UNTAGGED descents
that the geometric guard caps, and the generator fix removed exactly the
one tagged re-entry. Both mechanisms are needed.

Air comparability across the G-AIRDENOM boundary: S20's air moved 36.8 %
→ 31.1 % of total with the cut byte-identical, and the "absolute air"
figures (pct × total) computed on the pre-fix binary were themselves
mixed-base and OVER-state air on every modulator-slowed arm. Pre-fix and
post-fix air figures are NOT comparable; §2.5 is the bridge. Rankings that
lean on air must be re-read on post-fix runs.

**G-SIMRESKEY (ledger, MCP usability defect):** `run_simulation`'s
parameter is `resolution`; a call passing `resolution_mm` is accepted and
the key silently ignored, so the sim runs at the GUI's held value (0.4 mm
after a fresh load). One S20 sim went out that way and was discarded. The
response does echo `cell_mm`, which is how it was caught. Fix: reject
unknown parameter keys on MCP params (`deny_unknown_fields`), and put the
resolution actually used in the first line of the reply. Same class as
the "sim resolution is never a neutral default" rule in `generate_all`.

## Study closed — the plywood terrain recipe (rs-cam-38, 2026-09-08)

All arms at cell 0.2, 0 collisions, provenance emitted, all three gates
MODELED (doc §1.2a, §2.6, §3–§5).
- **Arm G modulated** (6 mm flat raster rough, oak): 615.6 s vs 1117.8
  unmodulated vs 2578 adaptive — chipload Within validated on
  `amana-flat-hardwood-pocket-6000-2f` (modulator +96 %, achieved 1377);
  **deflection 0.0425 = 85 % of the 0.05 bar, the closest gate** — the load
  ceiling of that recipe is deflection, not chipload.
- **Flat raster as the only pass is a ROUGH, not a finish:** terrace proxy
  stepover × tan(slope) at the terrain's p50 45° slope = 1.0 / 1.5 / 2.0 mm
  for s1.0 / 1.5 / 2.0 (p90 63°: 2–4 mm). Times 1808 / 1178 / 869 s. Air not
  measurable (blind 0.51) at 0.2 mm on s1.0.
- **Q2 — R2.0 ball raster s1.5 is the time-at-quality winner:** 2089 s,
  cusp 0.146, deflection 0.007, air 17.5 %, and it owns 76 % of the model
  cells in `preview_tier_map`.
- **Q1a — R1.0 (2 mm tip) raster s1.0, plunge entry at the 300 mm/min ball
  cap:** 5078 s, cusp 0.134, all Within; the gate queried the band at the
  3.53 mm TAPER (not the tip) and deflection read 7 µm on the engaged-
  diameter denominator; peak bite 8.6 mm = 4.3× tip D; plunge 0/1. It is
  **the reach tool** — 17 islands / 7027 mm² that only it enters.
- **Q3 — R1.5 iso-scallop h0.20** (cusp-matched to B15): 4372 s, 121 rings
  (436 at h0.38), untouched 0.0 mm² (the only arm with a measured reach),
  34 % slower than the B15 raster on the same tool.
- **Ranking on reach + time + cusp: Q2 > B15 > Q3 > Q1a.** The tier map's
  recipe — **R2.0 raster whole-surface + R1.0 confined to its 17 islands**
  (the multitool ladder, `plan_multitool_finishing`) — is the recommendation;
  the pair itself was not run.

Corrections the peer made to its own doc: the gate DOES apply a DOC derate
(same row reads 0.0320–0.0548 at a 4.2 mm bite and 0.0275–0.0471 at 9.3 mm
— ×0.86, unlabelled, milder than expected); the tier map's 20 440
"unassigned" cells are the grid margin, not unreachable valleys.

Ledger:
- **G-SUGGESTGATE is bidirectional** (strong, still not code-verified): on
  the R1.5 ball Suggest's ceiling was 1.45× ABOVE the gate's; on the 6 mm
  flat Suggest's post-derate chipload read 0.0244 (floor-clamp message) vs
  the gate's 0.0471 ceiling — 1.93× BELOW. The recipe resolver and the
  envelope resolver disagree in both directions depending on tool.
- **G-DCENTRY** — DropCutter's `dressup_policy` `strip_all_reason` forces
  `entry_style = None` in `normalize_for_op`, and `set_dressup_field`
  ACCEPTS "ramp" then reads back "none": a silently ignored setting. A
  ramp entry for a single-pass ball raster on fresh stock (the tip-load
  mitigation for small balls) is therefore not expressible today. Fix:
  refuse or warn at the setter, and decide whether drop_cutter should
  honour a ramp entry.
- The gate's DOC derate is applied but unlabelled — name it in the band
  explanation so a reader can see why the same row reads two bands.

### Correction — the ladder is not a raster ladder (rs-cam-38, 2026-09-08)

The operator asked for the winning formula in the sim. The multitool
planner's ladder (`plan_multitool_finishing`, tools [R2.0, R1.0], cusp
0.14) totals **10 710 s — 5.1× Q2 alone, 3.3× the single B15 raster** —
because the planner emits `unified_finish` (tier 0 spends 40 876 of
56 329 moves in its mid-steep scallop band) and the R1.0 island tier with
its 2 mm overlap cuts 66 km in 79 218 scallop moves and 1 104 retract trips
— more than the whole-surface R1.0 raster. All gates modeled Within, 0/0.
So the "R2.0 whole-surface + R1.0 on its 17 islands" recipe is a RANKING
inference, not a runnable job: **the raster pairing it implies
(drop_cutter R2.0 everywhere + drop_cutter R1.0 confined to the island
boundary) is not expressible through any MCP/GUI path today.** Honest
recommendation as of now: the single R2.0 raster (Q2, 2089 s) accepting
its unreached valley floors, or B15 (R1.5, 3274 s, cusp 0.20).

Ledger:
- **G-RASTERLADDER (product gap, the real follow-on):** let a drop_cutter
  raster take a region boundary from the tier map / reach map (P5) — the
  planner's island set — so a raster-per-tier ladder exists; and measure
  why the island tier's overlap and retract count balloon (2 mm overlap,
  1 104 trips). This is where P5's reach map feeds planning, not just
  display.
- **G-ISOCLIPENTRY (2026-09-09, FIXED df1232fd):** a rest-driven entry on a
  surface-riding pass took its whole bite in one move. On a
  `FromRemainingStock` pass the fed part of an entry descent is exactly the
  material the upstream tool could not reach, and two emitters took it
  unbudgeted: the boundary clip's island re-entry (rapid at safe Z, vertical
  fed descent to cut Z — `optimize_entry_descents` lowers only the rapid),
  and `emit_ramp`'s two-leg zigzag, whose legs clip to the MODEL surface
  under G-RAMPTERRAIN — below the material on a rest pass, so the closing
  leg returns to the start column at full depth. Live (rs-cam-38, 0.2 mm):
  entry_load CRITICAL on the island-clipped iso-scallop (T2, peak 1.44 mm)
  and contour scallop (T3, 1.79 mm); the whole-board R1.0 rest raster on
  the same stock fires nothing. Fix: both doors plan
  `pencil::plan_entry_ramp` (the G-ENTRYLOAD lap ladder) — `RestEntryRamp`
  on `dressup::optimize_entry_descents*`, `rest_stock` on
  `EntrySurfaceProbe`, set by the session and the GUI worker only for a
  rest-driven op whose `entry_probe_leave` rides the surface; fresh-stock
  passes are byte-identical. Headless T3 at 0.5 mm: CRITICAL → Caution,
  peak 1.67 → 0.50 mm (2× the per-lap budget), project time −20.6 % (the
  ~1.2 mm ladder replaces a 38 mm zigzag). The clip door alone moved the
  peak by nothing — both doors were needed. Sentry:
  `tests/isoclip_entry_ramp_g_isoclipentry.rs` (two arms assert the full
  bite with each door off).
- **G-ISOCLIPRAPID (2026-09-09, FIXED 38f8d151):** `apply_lead_in_out`
  planted its pre-position rapid at `moves[i-1].target.z` — often a CUTTING
  move's Z (a lead-out arc, a stepped pass) — so the lead-in traversed the
  work at cutting depth (T2 move 68538, 4.7 mm lateral, both ends
  sub-stock). Fix: a `retract_z` plane that only ever raises the inherited
  Z, plus a pure vertical Retract lift first. Lateral sub-stock rapids in
  the T2 .nc 3 → 0. Sentry `isoclip_link_rapid_g_isocliprapid.rs` (entry
  style None, so it attributes THIS fix — G-ISOCLIPENTRY alone had already
  removed the counted collision). Report:
  `planning/island_clip_2026-09-09/G-ISOCLIPRAPID_RAMPFALL_report.md`.
- **G-ISOCLIPRAMPFALL (2026-09-09, FIXED 38f8d151):** the entry residual
  (peak 1.39 mm at (115.8, 182.4, −1.59) on both fields). `emit_ramp`'s
  rest-driven arm abstained when its 1.18 mm ladder window left the mesh
  or held nothing past the budget, and fell through to the legacy 38 mm
  two-leg zigzag clipped to the MODEL floor — below the material on a rest
  pass — so the return leg crossed standing rest stock 5.6 mm from a
  correctly-read entry column. Fix: with a rest stock in scope the emitter
  is ladder-or-plunge and never reaches the legacy legs. T2 @0.2: peak
  1.39 CRITICAL → 0.50 mm Caution (= the tool-scaled per-lap budget;
  quieting it on R1.0 finish passes is a dial decision, not a defect),
  entry samples > 1 mm 80 → 0, time −6.8 %. Sentry arm g (chord-sampling
  measure; the target-only measure reads 0 on a gouging leg).
- **G-LINEVIS (2026-09-09, FIXED 241efd45):** the P5.3 release crashed on
  launch — the line shader's fragment stage read `uniforms.dim` while the
  bind-group-layout entry stayed VERTEX-only, and wgpu refused the
  pipeline. Found by rs-cam-38 on the live launch; the fix is the
  visibility flag. No gate creates a device, so no gate could see it.
- **G-PIPESMOKE (2026-09-09, DONE `render_pipelines_headless_g_pipesmoke.rs`; proved red on the G-LINEVIS flag, green on the fix, lavapipe 0.22 s):** a headless wgpu
  software-adapter test that constructs every render pipeline in
  `render/mod.rs`, so a layout/shader mismatch fails in `cargo test`
  instead of at the operator's launch.
- **G-RASTERLADDER note (2026-09-09, rs-cam-28/38):** the no-code stand-in
  for raster-per-island (planner tier 1 as `unified_finish` forced to its
  raster band: steep_threshold 85 clamped, waterline 89, overlap 1.25, after
  the Q2 raster) took ~15 min to GENERATE tier 1 alone on the wanaka board
  — the decomposition + per-region raster path is itself the cost, before
  any simulation. The 0.2 mm simulation of that three-toolpath project
  was then OOM-killed (journal 09:36: `rs_cam_gui` SIGKILL, oom-kill; the
  box carried a 11 GB rust-analyzer and a full 8 GB swap at the time). No
  toml was saved; recipe above for the rerun. RETRACTED the same day: the
  scallop already generates one ring set per region (scallop.rs P2.3,
  `pre_boundary_regions` threaded from the planner), so there is no
  "post-generation clip walk" to replace. What the code shows instead:
  `plan_tier_operation` sets `continuous: true` on every scallop tier;
  under continuous the spiral connector falls back to
  retract/rapid/replunge on any hop over the ring-spacing bound and the
  intra-pass relink is skipped — T3's 484 rings / 989 retracts is every
  ring unchained. Hypothesis under test: the island fragmentation is a
  PLANNER DEFAULT, not a clip defect (one dial: T3 with continuous:false,
  hookup 3.0 / 6.0). Also to check: whether the DropCutter arm receives
  `ctx.boundary_regions` — ANSWERED (rs-cam-15, `planning/island_clip_2026-09-09/SPEC.md`):
  it does (execute.rs DropCutter arm → `raster_toolpath_from_grid`, each
  row cut into engaging runs per island crossing) and compute.rs applies
  no op-family gate to `PlannedTierRegions`, so the core raster is already
  region-aware. G-RASTERLADDER is therefore a two-surface gap, not a
  generator gap: (a) `tier_strategies` (multitool.rs) has no raster value
  — the missing arm emits `OperationConfig::DropCutter` with the
  equal-cusp stepover; (b) MCP `set_boundary_config` accepts stock /
  model_silhouette / derived_rest_regions only, no planned_tier_regions.
  A hand-edited project file carries the boundary as data (T5 fixture).
- **G-TIERCONTINUOUS (2026-09-09, FIXED fb6027fb — planner default):** `plan_tier_operation` sets
  `continuous: true` on every per-island scallop tier; under continuous
  the connector retracts/rapids/replunges on any hop over the ring-spacing
  bound and the intra-pass relink is skipped (scallop.rs). Retracts per
  ring measured: T3 2.04, T2 1.44. VERDICT (T3b/T3c, 0.2 mm): real but
  small — continuous:false + hookup 3.0: retracts 929 → 613 (1.02/ring),
  pair 9 535 → 8 065 s (−15 %), entry_load falls to Caution (peak 0.81);
  hookup 6.0 buys 46 more links (567 / 7 820). Fix the default in
  `plan_tier_operation`; do not expect a collapse to the region count.
- **G-OVERLAPFILL (2026-09-09, MEASURED CONSEQUENCE, not a defect):**
  `preview_tier_map` at the planner's dials says tier 1 OWNS 12 224 mm²
  (10 islands); its MACHINING copy (grown by the 1.25 mm seam band in
  `tier_islands.rs::region_polygons_from_mask_reported`) nets 29 954 mm²:
  the big owned island is a 31 255 mm² outline with 1 329 holes and the
  band closes 925 of them, because a hole narrower than 2 × overlap
  collapses under a correct dilation BY DEFINITION, and tier_islands.rs:81
  states that reaching into the coarser tier's territory is the band's
  purpose. On a dendritic map the fine tier therefore machines 75 % of the
  board, which is why every island trial cut near whole-board distances.
  Not a band redesign; the levers are DIALS: an overlap below half the
  coarse tool's sliver width (median hole 3.6 mm² here), or a tier
  tolerance at or above the coarse tool's own cusp (0.146 mm for R2.0 at
  s1.5: owned 4 482 vs machining 17 169 mm²). The code change is to make
  the consequence visible — owned vs machining area, ratio and holes
  closed per tier in the planner output and UI, with an advisory naming
  the two dials (branch `tier-overlap`). Evidence: `svg_island_area.py`,
  `tier_map_r20_r10_tol*.svg`.
  **Shipped as a REPORT (fb6027fb):** `TierIslandSet` publishes
  machining area, both hole counts, median owned hole area and the
  overlap; `TierBandAdvisory` above 1.5× names the two dials, on
  `preview_tier_map`, the planner panel and `plan_multitool_finishing`.
  Instrument at the T3b dials: tol 0.05 2.52×, 0.146 1.52×, 0.30 2.12×;
  a clamp to half the sliver width measured 2.11× — a dial, not shipped.
  Sub-row A: the band also CREATES holes (a sealed bay encloses; 155 →
  255 at 0.146) — both counts ship. Sub-row B, OPEN: the cap's
  close-radius auto-raise (`first · 1.5³` = 1.688 mm when the island count
  exceeds `max_regions_per_tier`) welds the dendritic network into slabs
  and inflates OWNED territory 7.9× at tol 0.146 (2 261 → 17 812 mm²),
  more than the band's 1.52×; owned area is non-monotonic in the
  tolerance, and any owned figure quoted without `cap.close_raises`
  beside it is a slab area. Not fixed.
- **Confinement is retract-count-bound (2026-09-09, eight runs):** T5
  (drop_cutter on the planner islands, tol 0.05) confirms the core raster
  honours the region set (path = valley network) yet loses to whole-board
  T1: 953 row fragments / 954 retracts / 7 985 s vs 76 / 6 478. T5b (tol
  0.146, 21 thin islands) 2 172 retracts / 10 497 s; T3d (scallop, 0.146)
  1 266 / 10 146, entry CRITICAL again (peak 1.05). Pair time mostly
  follows retract count rather than area — T2 (2 207 retracts, 19 804 s)
  vs T5b (2 172, 10 497 s) is the exception, the iso field's over-coverage
  — so the lever is a surface link between fragments INSIDE a region (the
  intra-region link item), not a tighter boundary. DELIVERED FINISH
  (§2.7b, cusp_measure.py, same bar as §2.8): T1 26.7 % > 0.3 mm at
  6 478 s and T3c 27.7 % at 7 820 s are the same surface within the
  0.2 mm quantisation, so the T1-over-T3c ranking HOLDS at delivered
  finish (the R1.0 raster at s1.0 is already at a 0.13 mm flat-law cusp;
  the residual is reach). The single-pass R1.5 iso-scallop Q3 (25.5 % at
  4 372 s) reaches the same bar in 67 % of T1's time (25.5 vs 26.7 % is
  inside the same quantisation); the pairs win only the flat-band median
  (0.052 vs 0.093). The §2.8 inversion is untested here: T3c is a contour
  scallop over the 75 % machining set, not an iso pairing. The TAILS are
  the same for T1 / T3c / Q3 (p99 1.20 / 1.70 / 1.23 mm, max 4.46 / 4.79 /
  4.27): the R1.0 buys no reach over the R1.5 on this terrain — the deep
  residual is beyond every ball in the ladder and belongs to the
  river/pencil work, not to a finer tier. Recommendation on record: Q3 one
  pass; T1 if the flats must carry the R1.0 median.
- **G-ISOCLIPENTRY residual (2026-09-09, live 0.2 reruns on the fix):**
  T3 pair 11 105 → 9 535 s, over-bar 7 756 → 1 879, peak 1.79 → 1.39 mm;
  T2 25 580 → 19 804 s, 17 538 → 5 335, peak 1.44 → 1.39 — STILL CRITICAL
  on both, and the peak sits at the same point (115.8, 182.4, −1.59) on
  both fields: one ring start shared by both. For the entry agent. T2's
  rapid collision at move 68538 is GONE on the rerun (0) before the rapid
  fix landed — so that collision was an entry-path rapid; G-ISOCLIPRAPID's
  fix is still being confirmed headlessly.
- **G-RETRACTDIAL (2026-09-09, OPEN — dead dial):** `retract_strategy`
  (`RetractStrategy`, compute/config.rs) is defined, defaulted, shown in
  the GUI properties panel and named in the MCP `set_dressup_config`
  description, and consumed by NO generator or dressup in core. T5 with
  `retract_strategy = "minimum"` is byte-identical to T5 (46 720 moves,
  954 retracts, 7 985.08 s; fixture `T5m_r10_raster_islands_retract_min.toml`).
  An operator-facing dial with no effect. Either build the Minimum
  behaviour (the linking SPEC in `planning/linking_2026-09-09/` takes it
  as a fallback that must now be BUILT, not enabled) or remove the dial
  from every surface.
- **G-LINKSTAGE (2026-09-09, SPEC `planning/linking_2026-09-09/SPEC.md`,
  3 UNVERIFIED marks):** a shared surface-link stage for fragmented
  finishing passes. Findings the spec spot-checked against code: the
  scallop relink joins few rings because of the CANDIDATE SET, not the
  distance/boundary/kinematics tests — rings come out breadth-first by
  offset level (scallop.rs ~1500-1535) so adjacent entries are different
  loops tens of mm apart, the relink is called with `reorder: false`
  (~2557) and never rotates a closed ring to the point nearest the previous
  exit (rotation exists only in the continuous branch ~2379);
  `surface_link::relink_fragments` is already the shared kernel and the gap
  is the 12 call sites plus two defects — scallop passes
  `link_ceiling: None` (no lifted hop on a rest-driven island) and the TSP's
  `internal_link_ceiling_z` (execute.rs ~3500) returns None for every
  non-drill family (a Minimum-retract fallback would be re-planted at
  safe_z by the reorder). Design: one execute.rs helper building
  RelinkParams (reorder on, ceiling from initial stock, boundary,
  kinematics), fragment kind OpenRun/ClosedLoop with loop rotation, the
  Minimum-retract fallback selected by the now-inert `retract_strategy`
  dial (G-RETRACTDIAL), byte-identity via hookup_mm == 0 defaults.
  Experiments L1-L6 (§5) are one-dial GUI runs; L1 is a stderr counter
  read of the RelinkReport at scallop.rs ~2580. Tier-islands cost rule
  (§ cost): fill a hole when its area < h·v·t_j summed over the crossing
  rows — break-even ~25 mm (~500 mm²) at t_j 1 s, i.e. every hole on this
  map.
  **L1 READ (SPEC §7, headless CLI at 0.5, RUST_LOG=info — the counters
  land on STDOUT):** T3b (hookup 3) fragments 600, surface_links 89,
  retract_links 510, too_far 493, off_surface 0, slower_than_retract 0,
  outside_boundary 17; T3c (hookup 6) 600 / 124 / 475 / 451 / 0 / 0 / 24.
  The candidate-set diagnosis is CONFIRMED: too_far is 82 % / 75 % of
  junctions, the kinematics and surface tests refused nothing, and
  doubling the cap moved 42 out of too_far. Implementation can proceed
  from the spec's design.
- **G-LEADGATE (2026-09-09, OPEN):** the GUI worker gates the entry probe on
  `entry_style != None` (`worker/helpers.rs`) while `apply_dressups` also
  feeds it into lead-in/out, so an op with `entry_style = None` and
  `lead_in_out = true` gets stock-blind lead arcs in the GUI; the session
  door has no such gate.
- **G-MODSUMMARY:** tier 0's `modulation_summary` read `moves_touched 0`
  with `median_feed_delta_pct −5.3 %` — inconsistent fields in one summary.
  Reproduced twice (rs-cam-38, 2026-09-08): the FIRST `run_simulation`
  after a `generate_all` fixpoint loop reports 0 touched on tier 0; a
  second simulation of the same unchanged toolpath reports 53 877. The
  summary is stale or unpopulated on the sim that follows the fixpoint,
  not on the modulator.
