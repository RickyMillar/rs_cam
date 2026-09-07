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
