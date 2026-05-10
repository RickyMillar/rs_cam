# Structural Entry Spans + Locality Classifier Fix

**Status:** Drafted 2026-05-10 from wanaka MCP smoke that exposed the
G17 C1 filter operating on the wrong signal.
**Predecessor:** G17 C1+C2 (commit `1e31538`) — landed but mismeasures
what it filters; needs replacement once structural foundation is in
place.

---

## Snapshot

| Phase | Status | Hash | Date |
|---|---|---|---|
| D0. Prevalence research — kinematics histogram on real fixtures | ✅ done | `bb06611` | 2026-05-10 |
| D1. Operation generator survey — who emits Entry spans, who doesn't | ✅ done | (research only) | 2026-05-10 |
| D2. LUT calibration audit — slot vs partial engagement bound | ✅ done — **mismatch found** | (research only) | 2026-05-10 |
| D3. Decision point — stopgap revert vs roll-forward | ✅ Path A (revert) | `8e2a7fc` | 2026-05-10 |
| D4. Adaptive3d entry-emitter Entry spans (plunge/helix/ramp) | 🚫 blocked on D3 | — | — |
| D5. Other operation generators — Entry span coverage | 🚫 blocked on D4 | — | — |
| D6. Span-aware locality classifier | 🚫 blocked on D4 | — | — |
| D7. Span-aware C1 filter (replaces kinematics filter) | 🚫 blocked on D6 | — | — |
| D8. Cross-fixture re-validation | 🚫 blocked on D7 | — | — |
| D9. **Engagement-aware LUT comparison** (was: stretch — terrain-aware) | ⏳ **promoted — see D2** | — | — |

Legend: ⏳ pending · 🟡 in-progress · ✅ done · 🚫 blocked · ⏭️ skipped

---

## Why now

G17 C1 (commit `1e31538`) added `is_steady_state_for_gate(sample)`
returning `true` for everything except `CutKinematics::{Helix, Plunge}`,
on the premise that those kinematics indicate transient *entry* moves
that shouldn't drive the steady-state gate trip.

MCP smoke afterward showed wanaka_full_tuned.toml TP 1 (Back Rough)
flipping from `NoSafeImprovement` to `Ranked` with a winner at 38 %
cycle-time reduction. Looked like a clean win. It was not.

Reading the adaptive3d generator afterward exposed the mismatch:

> `Adaptive3dSegment::Cut(Vec<P3>)` is a **3D point sequence
> following the terrain surface**
> (`crates/rs_cam_core/src/adaptive3d/path.rs:53,106-113`). When the
> surface has any Z slope (which it does, almost everywhere on a 3D
> rough), sequential cut points have nonzero (dx, dy, dz). The
> simulator's `classify_cut_kinematics`
> (`crates/rs_cam_core/src/dexel_stock/simulation.rs:510-525`)
> classifies any segment with both XY and Z motion as
> `CutKinematics::Helix` — so **most steady-state cutting samples on a
> sloped 3D rough get tagged as Helix**, not as transient entries.

C1's filter is therefore excluding a large fraction of legitimate
cutting samples from the trip decision, not just transient entries.
The 0.0707 mm "helix entry" chipload reported on wanaka post-C1 is a
**real cutting condition** (full-slot engagement on a steep terrain
transition), not a transient — but C2 surfaces it under the misleading
"helix entry" label and C1 hides it from the trip.

Compounding bug: **TP 1 has zero `Entry` structural spans** despite
`entry_style: "plunge"` actually producing peck-plunge sequences.
`grep` confirms `SpanKind::Entry` is emitted only by `dressup.rs`'s
lead-in machinery (lines 110, 646), never by the operation generators'
own entry routines. The structural span tree doesn't reflect what the
operator configured.

So C1's intent (filter configured-entry transients) is sound; the
implementation reaches for kinematics because the structural signal
isn't available. The fix requires both: emit structural Entry spans
where they belong, then read them in the locality classifier.

This is downstream of the optimizer-explainability arc but spans a
wider surface (every operation generator, the locality classifier,
anything that consumes locality strings). Hence a separate doc.

---

## Shared vocabulary

- **Entry** (the span): a structural marker in `tp.spans` produced by
  the operation generator around moves that physically enter fresh
  material at the start of a region/Z-level/cut-run. Examples:
  peck-plunge into a region, helical descent into a Z-level, ramp
  into the start of a cut run, dressup lead-in arcs.
- **Steady-state cutting**: any in-cut sample whose `span_path` does
  NOT contain an Entry span ancestor. Includes contour-following over
  3D terrain, regardless of XY+Z motion.
- **Helix kinematics** (`CutKinematics::Helix`): a sample with nonzero
  (dx, dy, dz). Says nothing about whether the move is a configured
  helix entry or terrain-following — just describes the geometry.
- **Helix entry** (the strategy): a configured `EntryStyle3d::Helix`
  generates moves via `emit_helix` in `adaptive3d/path.rs:800-811`.
  Distinct from "Helix kinematics".

The conflation between "Helix kinematics" and "helix entry" is the
core defect. Post-fix, locality strings should describe the
configured entry style (or "steady-state cut") and never use
kinematics as a proxy.

---

## Research phases (D0 / D1 / D2)

### D0. Prevalence research — kinematics histogram on real fixtures

**Question:** how prevalent are Helix-classified samples in real
adaptive3d toolpaths, and what fraction of them are inside vs outside
true entry regions? This decides whether C1's filter is hiding a few
legitimate spike samples (manageable) or massively under-reporting
chipload exposure (urgent).

**Effort:** ~2h. **Files:** new test or CLI subcommand.

**Method:**
1. Add a small CLI subcommand or one-off test:
   `cargo run -p rs_cam_cli -- kinematics-histogram <project.toml> <toolpath_index>`
2. For each toolpath, dump:
   - Total in-cut sample count
   - Per-kinematics histogram (Linear / Arc / Helix / Plunge)
   - Per-kinematics chip-thickness percentiles (p50, p90, p95, p99,
     max)
   - Within Helix samples: chip-thickness distribution stratified by
     |dz/path_length| (ratio of vertical to total motion — steep
     terrain produces near-1 ratios, shallow terrain near-0)
3. Run on:
   - wanaka_full_tuned.toml TP 1 (the trigger case)
   - wanaka_full_tuned.toml TP 6 (the band-admit MarginalSafe case)
   - 1-2 other 3D rough fixtures from `test_data/`

**Expected outcomes:**
- If Helix-kinematics fraction is < 5 % and chip thickness percentiles
  match Linear's p99 → C1's blast radius is small; structural fix is
  cleanup not correctness emergency.
- If Helix fraction is > 20 % and Helix p50 chip thickness is
  noticeably above Linear's → C1 is hiding a meaningful slice of real
  cutting; replacement is urgent.
- Either way: the |dz/path_length| stratification tells us whether
  the fix can be a simple "Helix samples with steep-Z ratio = real
  entry, shallow-Z ratio = terrain follow" heuristic, or whether
  there's no clean kinematic separator and we must wait for D4–D6.

**Deliverable:** a one-page report appended to this doc's
deviations log with concrete numbers + recommendation.

### D1. Operation generator survey — who emits Entry spans

**Question:** which operations have configured entry strategies
(plunge / helix / ramp / lead-in) but don't currently emit `Entry`
structural spans? This is the scope estimate for D5.

**Effort:** ~1h. **Files:** survey only — read each operation's
generator.

**Method:** for each operation type, audit:

| Operation | Has configured entries? | Emits Entry spans today? |
|---|---|---|
| Adaptive3D | yes (plunge/helix/ramp) | no — confirmed |
| Pocket (2D) | likely (lead-in via dressup) | dressup-side yes, generator-side ? |
| Adaptive (2D) | ? | ? |
| Waterline | likely (plunge to Z-level) | ? |
| Scallop | ? | ? |
| DropCutter | ? | ? |
| ProjectCurve | ? | ? |
| VCarve | ? | ? |
| Trace | ? | ? |
| Profile | ? | ? |
| Drill / PinDrill | n/a (each move IS an entry) | ? |

For each row, answer with: file path, line number of entry generation
code, and whether `Span::new(_, _, SpanKind::Entry)` is called nearby.

**Deliverable:** filled-in table appended to this doc.

### D2. LUT calibration audit — slot vs partial engagement bound

**Question:** is `chip_load_max_mm` in the vendor LUT calibrated
against slot engagement (`arc = π`) or against typical partial
engagement (e.g. `arc = π/3`)? This decides whether full-slot
geometric chip thickness peaks (like wanaka's 0.0707) are *physically*
over the LUT bound or whether the formula is over-applying.

**Effort:** ~1h. **Files:**
`crates/rs_cam_core/src/dexel_stock/simulation.rs:480-489` (formula
doc), `crates/rs_cam_core/tests/chipload_formula_calibration.rs`
(calibration test), `CREDITS.md` + `AI_MACHINIST_ANALYSIS_REFERENCE.md`
(LUT source attribution).

**Method:**
1. Re-read the formula doc comment + calibration test.
2. Trace the LUT source: what was the original measurement
   condition?
3. Cross-check with one or two vendor datasheets if available.

**Outcomes:**
- LUT is slot-calibrated → 0.0707 IS over the physical bound;
  optimizer should refuse such candidates (but maybe with a tolerance
  band for transient slot engagement on terrain).
- LUT is partial-calibrated → the formula needs a per-engagement
  correction OR the LUT max needs to be scaled by `(arc / π)` before
  comparing against slot-engagement readings; the gate trip on
  geometric peak is misapplying the bound.
- Unknown / mixed → file as a research debt; default to slot-calibrated
  conservative interpretation.

**Deliverable:** decision recorded in this doc + CREDITS.md updated if
attribution is improved.

---

## D3. Decision point — stopgap revert vs roll-forward

After D0–D2, we have two paths:

### Path A — Stopgap revert (recommended if D0 shows large blast radius)

1. **Revert C1's kinematics filter** in `chipload.rs`, `power.rs`,
   `deflection.rs`. Keep C2's `entry_spikes` shape — the data is still
   useful — but stop populating it from the kinematics-based heuristic
   (or populate it as `Vec::new()` until D7 lands).
2. Wanaka TP 1 returns to `NoSafeImprovement` until D4–D8 land. Honest
   regression.
3. Then build D4–D8 in order without time pressure.

### Path B — Roll-forward (recommended if D0 shows small blast radius)

1. Keep C1 in place under a known-defect note in this doc.
2. Land D4–D8 as a single sequenced change. C1's filter swaps from
   kinematics-based to span-based on the same commit that lands
   structural Entry spans.
3. Wanaka TP 1's verdict shifts in one MCP smoke; no intermediate
   regression.

**Decision criteria** (filled in after D0, 2026-05-10):
- Helix-kinematics fraction in real adaptive3d cuts: **4.06 % (TP 1)
  / 37.91 % (TP 6)**.
- Helix chip thickness p99 vs Linear chip thickness p99: **TP 1 essentially
  equal (0.0465 vs 0.0462); TP 6 +52 % (0.0557 vs 0.0366)**.
- Number of toolpaths in available fixtures whose verdict materially
  changes if C1 is reverted: **2 of 2** (only adaptive3d 3D-rough
  toolpaths in scope; wanaka TP 1 reverts from `Ranked` to
  `NoSafeImprovement`; TP 6 likely re-trips on real terrain
  chipload).
- |dz/path_length| separator viable: **no** (zero near-vertical Helix
  samples on either fixture — Helix is purely terrain-following, no
  kinematic heuristic can rescue the entry signal).
- **Recommendation: Path A — stopgap revert.** Chosen 2026-05-10.

**Deliverable:** decision row added to this snapshot, plus a one-line
rationale in the deviations log.

---

## Implementation phases (D4 / D5 / D6 / D7 / D8)

### D4. Adaptive3d entry-emitter Entry spans

**Reference:** wraps the three entry-emit functions in
`crates/rs_cam_core/src/adaptive3d/path.rs`.
**Effort:** ~½d. **Files:** 1 + tests. **LOC:** ~50.

`emit_peck_plunge` (line 38), and the `emit_helix` /  `emit_ramp`
calls inside the `Adaptive3dSegment::Rapid` and `RapidWithFloor`
match arms (lines 794–873) all need to be wrapped:

```rust
let span_start = tp.moves.len();
emit_peck_plunge(&mut tp, entry, params.safe_z, params);
let span_end = tp.moves.len();
tp.spans.push(
    Span::new(span_start, span_end, SpanKind::Entry)
        .with_label("plunge entry") // or "helix entry" / "ramp entry"
);
```

**Verification:**
- Existing tests: regenerate adaptive3d toolpaths in test fixtures;
  `inspect_spans` should now report `Entry` spans matching configured
  `entry_style`.
- New test: `tests::adaptive3d_emits_entry_spans_for_each_style` —
  build a synthetic project with each entry style, generate, assert
  Entry span count matches the number of regions/depth-passes.
- MCP smoke: regenerate wanaka TP 1; `inspect_spans` should report
  ~12+ Entry spans (one per region per Z-level).

### D5. Other operation generators — Entry span coverage

**Reference:** apply D4's pattern to operations identified in D1.
**Effort:** ~1-2d, parallelizable. **Files:** per-op generators.

For each operation in D1's "no — needs Entry spans" column, add the
same `Span::new(_, _, SpanKind::Entry)` wrapping. Drill / PinDrill ops
where every cutting move IS an entry can either:
- Mark every cutting move as `SpanKind::Entry`, or
- Be skipped (the locality classifier can recognize Drill operations
  by `OperationType` and treat all in-cut samples as steady-state for
  gate purposes — drilling is its own gate model).

Decide per-op based on D1's table.

**Parallelization:** spawn a per-op Agent team (one Agent per operation
file), each in an isolated worktree, each owning their op's
generator + the test fixture for it. Coordinate via task list.

### D6. Span-aware locality classifier

**Reference:** rewrites `crates/rs_cam_core/src/tool_load/locality.rs`.
**Effort:** ~½d. **Files:** locality.rs + every caller.

Current signature:
```rust
pub fn classify_sample_locality(sample: &SimulationCutSample) -> Option<String>
```

New signature (proposal):
```rust
pub fn classify_sample_locality(
    sample: &SimulationCutSample,
    span_lookup: &SpanLookup, // resolves sample.span_path → ancestors
) -> Option<String>
```

Logic:
1. Walk `sample.span_path` ancestors via `span_lookup`.
2. If any ancestor is `SpanKind::Entry`, return the entry's label
   (e.g. `"plunge entry"`, `"helix entry"`, `"ramp entry"`,
   `"lead-in"` for dressup-emitted entries).
3. If any ancestor is `SpanKind::DressupArtifact` with relevant
   label, return that.
4. If any ancestor is `SpanKind::LinkBridge`, return `"region join"`.
5. Else: this is steady-state cutting. Return `None` or
   `"heavy engagement"` based on `radial_engagement` (preserve
   current heavy-engagement labeling).

Caller threading:
- Most callers already have access to the trace/span tree. Plumb
  `&SpanLookup` (or `&SimulationCutTrace`) through where needed.
- Verify no regression in narrative formatting: existing labels like
  `"heavy engagement"` should still appear; only the misuse of
  `"helix entry"` / `"plunge entry"` should change semantics.

**Verification:**
- Existing locality tests need rewriting: assertions that synthetic
  Helix-kinematics samples produce `"helix entry"` are now wrong. New
  tests should construct samples with explicit `span_path` containing
  an Entry span and assert the entry label, *and* tests that
  Helix-kinematics samples *without* an Entry ancestor return
  `"heavy engagement"` (or similar) not an entry label.

### D7. Span-aware C1 filter

**Reference:** replaces `is_steady_state_for_gate` in
`crates/rs_cam_core/src/tool_load/locality.rs`.
**Effort:** ~¼d. **Files:** locality.rs + chipload.rs / power.rs /
deflection.rs (filter callsites).

New predicate:
```rust
pub fn is_steady_state_for_gate(
    sample: &SimulationCutSample,
    span_lookup: &SpanLookup,
) -> bool {
    !ancestors_contain_kind(sample.span_path, span_lookup, SpanKind::Entry)
}
```

Plus: drill operations might need to be a separate path entirely
(they're all-entry, all-steady-state, but the gate model doesn't
fit; flag for follow-up rather than try to wedge in here).

The filter is now conservative-by-default: any sample whose
provenance can't be confirmed as steady-state (e.g. legacy operations
that don't emit Entry spans yet) trips like a normal sample. Safer
than the kinematics fallback.

**Verification:**
- Re-validate the C1 unit tests
  (`linear_steady_state_high_sample_trips_exceeds`,
  `helix_samples_route_to_entry_spike_not_trip`,
  `plunge_high_sample_routes_to_entry_spike_not_trip`,
  `mixed_trip_uses_steady_state_advisory_kept`) — they should be
  rewritten in terms of span_path context, not raw kinematics.

### D8. Cross-fixture re-validation

**Reference:** wide MCP smoke + diagnostic capture.
**Effort:** ~½d.

Run `optimize_toolpath` against every adaptive3d / pocket / scallop
toolpath in `test_data/` projects. Capture per-toolpath:
- pre-revert verdict (kinematics-filter behavior — what we have today
  on commit `1e31538`)
- post-D7 verdict (span-filter behavior)
- delta in candidates count, headline outcome, recommended winner

For wanaka TP 1 specifically:
- Predict outcome: likely back to `NoSafeImprovement` because the
  0.0707 reading IS a real cutting condition. If so, that becomes
  the trigger for D9.
- If somehow it stays `Ranked`, verify whether the steady-state
  cutting samples post-D7 actually reflect the real chipload
  envelope.

**Deliverable:** before/after table appended to this doc.

---

## D9. Stretch — terrain-aware gate semantics

**Reference:** rethink of how the chipload gate models terrain-driven
engagement spikes.
**Effort:** unknown — research direction, not implementation.

If D8 confirms wanaka TP 1 (and other 3D roughs) genuinely cannot
optimize because terrain spikes are inherent to 3D contour-following,
the gate's "compare peak chip thickness against LUT max" logic
deserves a rethink. Options:

1. **Separate peak vs sustained bounds.** Vendor LUT max applies to
   the N-percentile (e.g. p95) chip thickness, not the absolute peak.
   Transient spikes above the bound for < N samples are admitted.
2. **Engagement-stratified bounds.** Compute chip thickness against
   a bound that scales with engagement: at slot engagement use a
   wider band (since cutter loading is briefer per flute pass), at
   partial engagement use the strict LUT max.
3. **Terrain-mode tolerance.** A new `tolerance.terrain_spike` band
   in `ToleranceBands` that admits 5-10 % overshoot when the
   over-bound sample has high `|dz/path_length|` (i.e. a steep
   terrain transition). Operator-tunable per project.
4. **Mainline acceptance.** Decide that 3D rough operations *will*
   produce transient slot-engagement spikes by design, and the only
   honest mitigation is operator-side feed reduction. Document this
   in `OPTIMIZER_LOGIC.md`.

This is downstream of D8's evidence. Don't pre-commit to an option.

---

## Sequencing diagram

```
                   ┌─→ D0 (kinematics histogram) ─┐
research phases ───┼─→ D1 (operation survey) ─────┼──→ D3 (decide path)
                   └─→ D2 (LUT calibration) ──────┘        │
                                                            │
              ┌────────────────────── stopgap ──────────────┤
              ↓                                             │
   revert C1 kinematics filter                              │
   wanaka regresses (acceptable)                            │
              ↓                                             │
              └─→ D4 (adaptive3d Entry spans) ──┐           │
                                                ├─→ D6 ─→ D7 → D8 ─→ D9?
                                  D5 (others) ──┘
              ┌────────────────── roll-forward ─────────────┘
              ↓
   D4 → D5 ⫩ D6 → D7 → D8 ─→ D9?
   (D5 parallel to D6)
   wanaka shifts in one go
```

The key fork is D3. The research phases are fast (~½d combined) and
inform whether the existing C1 commit can stay in place during the
build-out or needs immediate revert.

---

## Risk register

| Risk | Likelihood | Mitigation |
|---|---|---|
| C1 revert reopens wanaka pain | ✅ if Path A | Document honestly in tracker; wanaka becomes the live trigger fixture for D7 validation |
| Adding Entry spans to all operations is wide and may regress span-counting tests | medium | D4 is the canonical pattern; D5 fanout is mechanical. Each op gets dedicated tests in same commit |
| Locality classifier signature change touches many sites | medium | Mechanical refactor; can be staged (introduce new fn alongside old, migrate callers, delete old) |
| Test fixture churn — current tests assert "helix entry" labels | high | Audit & rewrite as part of D6's commit; this is the intentional churn |
| D9 reveals the chipload gate fundamentally needs work | medium | This is the right time to find out — punt to dedicated G18 thread if scope expands |
| Scallop/DropCutter also follow terrain — same Helix-kinematics issue applies | high | D5 covers their Entry spans; D7's span-based filter handles them too. Cross-fixture validation in D8 catches regressions |
| 3D operations may have terrain-following Helix samples that are *also* legitimately problematic | medium | D9 is exactly this question; D0 prevalence research foreshadows |

---

## Costs (rough)

| Phase | Effort | Risk |
|---|---|---|
| D0 | 2h | low — pure measurement |
| D1 | 1h | low — survey |
| D2 | 1h | low — research |
| D3 | 30min | n/a — decision |
| D4 | ½d | low — narrow pattern |
| D5 | 1-2d (parallel) | medium — wide surface, mechanical per-op |
| D6 | ½d | medium — signature change ripples |
| D7 | ¼d | low — replaces existing predicate |
| D8 | ½d | low — measurement |
| D9 | unknown — research | n/a |

Total to "C1 filter is correct": **~3-4d** (D0–D7 + small D8).
Total to "all operations have honest Entry spans + cross-fixture
validated": **~5-6d**.

---

## Success criteria

- [ ] **D0 deliverable:** kinematics histogram report on wanaka TP 1
      + 2 other 3D roughs, with concrete numbers + Path A/B
      recommendation.
- [ ] **D1 deliverable:** filled survey table for all operation types.
- [ ] **D2 deliverable:** LUT calibration assumption documented in
      this doc + `CREDITS.md` if attribution improved.
- [ ] **D3 decision recorded** in snapshot table + deviations log
      with rationale.
- [ ] **D4 verification:** wanaka TP 1 has Entry spans matching
      configured plunge entries; existing adaptive3d tests pass
      after pattern application.
- [ ] **D5 verification:** every operation in the D1 survey emits
      Entry spans where applicable; per-op regression tests cover
      span emission.
- [ ] **D6 verification:** locality classifier returns entry labels
      ONLY for samples inside Entry spans, returns
      "heavy engagement" / steady-state for terrain-following Helix
      samples. Test coverage for both cases.
- [ ] **D7 verification:** C1 filter operates on span ancestry, not
      kinematics. Wanaka TP 1 verdict reflects the real chipload
      envelope (likely `NoSafeImprovement` again — that's the
      correct outcome).
- [ ] **D8 deliverable:** before/after table of every project's
      verdict shifts.
- [ ] **D9 trigger evaluated:** does the chipload gate need a
      terrain-aware mode? If yes, opens a follow-up thread.

---

## Notes / deviations log

### 2026-05-10 — D0 kinematics histogram report

Test: `crates/rs_cam_core/tests/kinematics_histogram.rs`
(`cargo test --release --test kinematics_histogram -- --nocapture`).
Loads `wanaka_full_tuned.toml`, applies the post-C1 winner params to TP 1
(feed=4000, stepover=2.2, depth_per_pass=3.0), generates everything,
runs sim with metrics at 0.5 mm resolution. Available adaptive3d
fixtures across the repo were limited to the wanaka project's TP 1 +
TP 6 — no other 3D-rough projects in `crates/*/tests/fixtures/` or
`fixtures/`, so this is the full sample.

**TP 1 — Back Rough (40 493 in-cut samples)**

| Kinematics | Share | n with chip | p50 | p90 | p95 | p99 | max |
|---|---|---|---|---|---|---|---|
| Linear | 65.49 % | 20 780 | 0.0326 | 0.0440 | 0.0449 | 0.0462 | 0.0465 |
| Plunge | 12.43 % | 0 (no chip data emitted) | — | — | — | — | — |
| Helix  |  4.06 % |  1 266 | 0.0389 | 0.0458 | 0.0463 | 0.0465 | **0.0707** |
| Arc    | 18.02 % |  5 786 | 0.0355 | 0.0442 | 0.0451 | 0.0462 | 0.0465 |

Helix |dz/path_length| stratification:
- near-flat (0.0–0.1): 89.12 % of Helix samples (the 0.0707 max sits here)
- 0.1–0.3: 10.52 %
- 0.3–0.6: 0.36 %
- ≥ 0.6: **0 samples**

**TP 6 — 3D Rough 6 (9 612 in-cut samples)**

| Kinematics | Share | n with chip | p50 | p90 | p95 | p99 | max |
|---|---|---|---|---|---|---|---|
| Linear | 35.69 % | 2 587 | 0.0262 | 0.0348 | 0.0360 | 0.0366 | 0.0366 |
| Plunge | 17.84 % | 0 (no chip data emitted) | — | — | — | — | — |
| Helix  | **37.91 %** | 2 769 | 0.0326 | 0.0365 | 0.0365 | **0.0557** | 0.0557 |
| Arc    |  8.55 % |   594 | 0.0278 | 0.0345 | 0.0359 | 0.0366 | 0.0366 |

Helix |dz/path_length| stratification:
- near-flat (0.0–0.1): 68.80 %
- 0.1–0.3: 30.30 %
- 0.3–0.6: 0.85 %
- 0.6–0.9: 0.05 % (2 samples)
- ≥ 0.9: 0 samples

**Findings**

1. **Helix kinematics is essentially never a configured entry on
   adaptive3d.** Across both fixtures, ~zero Helix samples have a
   parent-move |dz/path_length| ratio above 0.6. Helix kinematics is
   almost entirely terrain following (mostly near-flat XY+Z due to
   sloped surface contour). No kinematic heuristic
   ("steep-Z Helix = entry, shallow-Z = terrain") can rescue the
   structural signal — there's no separating ratio because there are
   no near-vertical Helix samples to pull out.
2. **C1's filter is partly redundant and partly incorrect.** Plunge
   samples emit **no `effective_chip_thickness_mm` data at all**
   (~5 000 samples on TP 1, ~1 700 on TP 6 — none of them feed the
   chipload gate). So C1's protection of "Plunge" was already given
   by the fact that those samples couldn't trip the gate anyway. The
   only observable effect of C1 is to drop Helix samples — which are
   terrain-following steady-state cuts, not entries.
3. **The Helix-vs-Linear chip-thickness divergence is fixture-dependent.**
   - TP 1: Helix p99 (0.0465) ≈ Linear p99 (0.0462). The 0.0707 spike
     is a single-sample outlier in near-flat terrain (a transient
     full-slot engagement). C1 hides exactly one influential
     sample's worth of trip pressure on this toolpath.
   - TP 6: Helix p99 (0.0557) is **52 % above** Linear p99 (0.0366),
     and Helix p50 (0.0326) is **24 % above** Linear p50 (0.0262), on
     ~38 % of the in-cut sample population. C1 is hiding a substantial
     slice of legitimate elevated steady-state cutting from the gate
     on this toolpath.
4. **The 0.0707 mm "helix entry" spike on wanaka TP 1 is real
   terrain-driven cutting, not a configured entry.** Confirmed by both
   the dressup-only emission of `SpanKind::Entry` and the |dz/L|
   stratification (the spike sits in the near-flat bucket). The label
   "helix entry" in C2's advisory is wrong.

**Path recommendation: A (stopgap revert).**

The blast-radius criteria from the plan resolve as:

| Criterion (plan) | Observed | Triggers Path A? |
|---|---|---|
| Helix fraction < 5 % AND Helix p99 ≈ Linear p99 → small blast | TP 1 only | partial |
| Helix fraction > 20 % AND Helix p50 noticeably above Linear's → large blast | **TP 6** | **yes** |
| |dz/path_length| separator viable? | no (zero near-vertical Helix) | n/a |

TP 6 alone hits the "large blast" criterion. The fixtures with Helix
fraction > 20 % are real wood-router 3D rough operations with sloped
terrain — exactly the use case the optimizer is meant to serve. Path B
(roll-forward without reverting) would mean shipping a known-defective
filter against the more aggressive 3D fixtures while D4–D8 land. That
risks giving operators a green-lit verdict on a toolpath whose real
steady-state chipload distribution exceeds the LUT bound.

Path A trade is: wanaka TP 1 returns to `NoSafeImprovement` until D4–D8
land. That is honest — TP 1's 0.0707 is a real over-bound steady-state
sample on terrain, and the optimizer telling the operator "your current
params can't be safely improved without reducing terrain chipload" is
the truthful verdict.

**Suggested D3 follow-on actions (separate phase entries)**

- Revert C1's `is_steady_state_for_gate` filter in
  `chipload.rs` / `power.rs` / `deflection.rs`. Keep the
  `EntrySpike` struct shape (C2) so D7 can re-populate it from spans;
  pass `Vec::new()` until D7 lands so narrative doesn't surface stale
  data.
- Mark in `OPTIMIZE_EXPLAINABILITY_AND_PEAK_FINDING.md` that C1 is
  reverted, with a pointer to this plan's deviations log.
- Note for D2 (LUT calibration audit): the 0.0707 mm chip-thickness
  spike is **mean chip thickness** of a slot-engagement sample
  (slot ≡ arc=π in the formula). Whether this exceeds the LUT bound
  depends on whether `chip_load_max_mm` is calibrated against slot or
  partial engagement — the same question that motivates D2. D2's
  outcome may justify a tolerance band that admits transient slot
  spikes on terrain, which would re-frame TP 1's verdict.
- Note for D1: the absence of `effective_chip_thickness_mm` on Plunge
  samples is itself worth flagging in the operation-survey table —
  drill-style operations may need a separate gate entirely.

### 2026-05-10 — D3 stopgap revert landed

Path A executed. Removed the `CutKinematics::{Helix, Plunge}` filter
from the trip-decision loops in:
- `crates/rs_cam_core/src/tool_load/chipload.rs` (entry_spike_high /
  _low tracking + collection block deleted; `entry_spikes:
  Vec::new()` on `Within`).
- `crates/rs_cam_core/src/tool_load/power.rs` (entry_spike_track
  + map block deleted; `entry_spike: None` on `Within`).
- `crates/rs_cam_core/src/tool_load/deflection.rs` (same shape).
- `crates/rs_cam_core/src/tool_load/locality.rs` (function deleted,
  comment placeholder pointing to D7).

Verdict struct fields (`entry_spikes: Vec<EntrySpike>` /
`entry_spike: Option<EntrySpike>`) and the `EntrySpike` struct itself
are preserved so D7 can re-populate from span ancestry without
another verdict-shape churn.

Tests rewritten:
- `helix_samples_route_to_entry_spike_not_trip` →
  `helix_high_sample_trips_exceeds` (asserts pre-C1 trip behavior).
- `plunge_high_sample_routes_to_entry_spike_not_trip` →
  `plunge_high_sample_trips_exceeds`.
- `mixed_trip_uses_steady_state_advisory_kept` removed (the "mixed
  trip" path no longer has separate semantics — both samples count).
- `linear_steady_state_high_sample_trips_exceeds` kept as-is.

Known-deferred warts the revert leaves in place:
- `classify_sample_locality` still labels Helix samples as
  `"helix entry"` in trip-evidence localities. Misleading on terrain-
  following Helix samples, but harmless since `entry_spikes` is
  empty so it only surfaces as the trip's locality string. D6 will
  rewrite the classifier to be span-aware and drop this label.
- `EntrySpike` is dead-data on the wire until D7. Documented in the
  field comment on each `Within` arm.

Validation:
- `cargo test -p rs_cam_core --lib` 1 332 passed.
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `cargo test --workspace` (workspace suite) — pending at time of
  writing.
- Wanaka MCP smoke deferred to next session — will confirm TP 1 ↔
  `NoSafeImprovement` regression as predicted by D0.

### 2026-05-10 — Wanaka MCP smoke (post-revert)

Confirmed regression as predicted. TP 1 (Back Rough,
feed=4000/stepover=2.2/DOC=3.0):

| Stage | Verdict | Triggering reading | Locality |
|---|---|---|---|
| baseline | chipload Exceeds High | 0.0707 mm/tooth (vs max 0.055) | helix entry |
| refined #1 (DOC↑→4.83, feed↓→2592) | chipload Exceeds Low + deflection Exceeds | median 0.0258 (-19% below min); 237 µm (+18% over) | — |
| refined #2 (DOC↑→4.83, feed↓→2592, stepover unchanged) | chipload Exceeds Low + deflection Exceeds | similar | helix entry |
| refined #3 (DOC↑→4.83, feed↓→2592, stepover↓→2.0) | chipload Exceeds Low + deflection Exceeds | similar | helix entry |

**Outcome:** `NoSafeImprovement`. Narrative headline:
"Tried 3 candidates; closest-to-safe still hit: chipload 0.0258
mm/tooth (-19% below LUT min 0.0320); deflection 237 µm (+18% over the
200 µm threshold)." Suggestions: raise feed > 3376 mm/min OR cap DOC ≤
3.88 mm.

The verdict is now an honest reflection of the gate's chipload bound.
Revert verified.

### 2026-05-10 — D1 operation-generator survey

Surveyed 23 operations. Result table (compressed):

| Group | Count | Detail |
|---|---|---|
| Generator-side native Entry strategies + emits Entry spans | 0 | — |
| Generator-side native Entry strategies + emits **no** Entry spans | **1** | **Adaptive3D** (`adaptive3d/path.rs:794-874` — `EntryStyle3d::{Plunge, Helix, Ramp}` dispatched but no `Span::new(_, _, SpanKind::Entry)` call) |
| Dressup-only Entry (default `entry_style = Ramp` in Finish/SemiFinish role) | ~13 | Waterline, Scallop, VCarve, Trace, Chamfer, Inlay, Pencil, RampFinish, SpiralFinish, RadialFinish, HorizontalFinish, SteepShallow + most Finish ops. Entry spans emitted by `dressup::apply_entry` in the post-generation dressup layer |
| Dressup conditionally available (Roughing role default `entry_style = None`) | ~6 | Pocket, Adaptive (2D), Profile, Face, Zigzag, Rest. User can opt-in via dressup panel; no native generator entry |
| Entry strategies stripped by `normalize_for_op` | 2 | DropCutter (geometrically incompatible), ProjectCurve (multi-ring, ramp would diagonal) |
| No Entry concept (every move IS an entry) | 2 | Drill, AlignmentPinDrill — DrillCycle variants only; needs separate gate model |

**Implications for D4–D5 scoping:**

- **D4 is the only structural Entry-span work needed for the gate-trip
  problem.** Wrap `emit_peck_plunge` / `emit_helix` / `emit_ramp` in
  `adaptive3d/path.rs:795-876` with `Span::new(_, _, SpanKind::Entry)`.
  ~50 LOC + tests.
- **D5 collapses to a quick audit.** Most other ops already have
  Entry spans via the dressup path. The audit is "verify
  `dressup::apply_entry` actually attaches a `SpanKind::Entry` span"
  and write a regression test. Drill/PinDrill stay outside the
  Entry-span model — D6's classifier will recognize `OperationType::Drill`
  and route differently (or D7 will skip the gate filter for those
  ops entirely).
- **D5 effort revises down from 1-2d to ~½d.** The fanout the original
  plan budgeted for doesn't exist.

### 2026-05-10 — D2 LUT calibration audit — **MISMATCH FOUND**

The simulator computes `effective_chip_thickness_mm` as the
**arc-average mean chip thickness over the actual engagement arc the
dexel saw** (`crates/rs_cam_core/src/dexel_stock/simulation.rs:478-508`,
`crates/rs_cam_core/src/tool/mod.rs:106-147` — `mean = (2·feed/arc) ×
(1 − cos(arc/2))`). At slot engagement (`arc = π`), mean ≈ 0.637 ×
feed. At half engagement (`arc = π/2`), mean ≈ 0.373 × feed.

The wanaka-tripping LUT row
(`crates/rs_cam_core/data/vendor_lut/observations/amana_flat_end.json:194-222`,
HardMaple Pocket Roughing, 6 mm flat end, `chipload_max_mm = 0.055`) is
sourced from Amana's *Spektra Spiral Plunge 2/3 Flute Chart v24*. The
chart explicitly carries:

```
ae_min_mm: 1.0,
ae_max_mm: 2.2,
ae_rule: "17% to 37%D"
```

So the LUT max is **authored at ~17-37% radial engagement**, not slot.
For comparison, the ball-nose `coverage_notes` in
`source_manifest.json` are the only LUT row explicitly calibrated for
slot/peripheral cutting.

**The gap:** the simulator's slot-engagement mean reading and the
LUT's partial-engagement-mean cap are **not on a common engagement
basis**. Wanaka's 0.0707 mm reading at `arc ≈ π` vs LUT cap 0.055 at
~27% radial (arc ≈ 1.10 rad, mean ≈ 0.477 × feed) over-penalizes by
the ratio `0.637 / 0.477 ≈ 1.34×`.

If the cap were slot-equivalent: `0.055 × 1.34 ≈ 0.073 mm` — and the
0.0707 reading **would pass**.

**Calibration test asymmetry:**
`crates/rs_cam_core/tests/chipload_formula_calibration.rs` builds a
half-engagement reference (`arc = π/2`) with wanaka feed (0.0875
mm/tooth), asserts the closed-form arc-average ≈ 0.0326 mm passes the
LUT cap. The test only exercises the half-engagement case the LUT was
authored against. There is no test exercising the `arc = π` case the
actual D0 measurement hit — so the asymmetry was invisible.

**Outcome: LUT-vs-formula mismatch.** Falls into the plan's "LUT is
partial-calibrated" outcome.

### Re-scoping the plan after D1 + D2

D2's finding **changes the priority order**. Even with D4–D7 perfectly
land, wanaka TP 1 still trips on a real terrain-following slot sample
that the LUT doesn't cover at slot engagement. The Entry-span work
fixes "configured entry transients shouldn't trip the gate"; the
calibration mismatch fixes "slot terrain samples shouldn't trip a
partial-engagement bound." Both are real bugs; D2 is the one wanaka
hits.

Recommended sequencing:
1. **D9 (now top-priority).** Engagement-aware LUT comparison. Two
   options from D2's report:
   - (a) Re-cast the LUT cap to slot-equivalent at compare-time, using
     the row's own `ae_min_mm`/`ae_max_mm` to derive the nominal arc.
   - (b) Re-cast the simulator's mean reading to LUT-equivalent
     (inverse of (a)).
   Either fixes wanaka without touching the structural-spans plan.
2. **D4 (Adaptive3D Entry spans).** Still needed for honest gate
   semantics — even after D9 lands, configured entry transients
   shouldn't drive the trip. Effort ~½d, narrow.
3. **D5 (other ops audit + Drill/PinDrill carve-out).** Smaller than
   originally scoped — most ops already have Entry spans via
   dressup. Effort ~½d.
4. **D6 + D7 (span-aware classifier + filter).** As planned. With D9
   in place these become "remove the misleading 'helix entry'/'plunge
   entry' labels from terrain samples and stop using kinematics
   anywhere", not "rescue wanaka".
5. **D8 (cross-fixture validation).** As planned.

Net effort estimate revises from ~3-4d ("C1 filter is correct") to:
- D9 alone (~1-2d) restores wanaka-class fixtures to honest verdicts
- D4–D8 (~1.5-2d combined, smaller than before) finishes the
  structural cleanup

Worth a separate decision: do D9 first as its own commit-train, or
bundle with D4? Recommend **D9 first** — it's the lever that
materially changes user-visible verdicts, while D4–D7 are
correctness-cleanup that doesn't move the needle on wanaka.

