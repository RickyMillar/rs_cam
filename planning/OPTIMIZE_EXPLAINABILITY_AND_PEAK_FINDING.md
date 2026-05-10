# Optimizer — Explainability + Peak-Finding

**Status:** Drafted 2026-05-10 after wanaka MCP TP 1 run exposed the
"the magic didn't work" surface. Two threads, sequenced.
**Predecessors:** G16 §11 (layered scoring) — closed by `3e68d48`.
G16 §11.6.4 reserves the Bayesian slot but defers it behind gating
criteria.

---

## Snapshot

| Thread | Phase | Status | Hash | Date |
|---|---|---|---|---|
| A. Explainability | A1. Structured failure narrative in `OptimizeOutcome` | ✅ | `9b940ca` | 2026-05-10 |
| A. Explainability | A2. Modal "what was tried, why each failed" view | ✅ | `4618023` | 2026-05-10 |
| A. Explainability | A3. Sample locality classification (kinematics + arc engagement) | ✅ | `fc17798` | 2026-05-10 |
| A. Explainability | A4. Operator-facing suggestion lever | ✅ | `2c7ffe5` | 2026-05-10 |
| A. Explainability | A5. Search-frontier heatmap (feed × stepover) | ⏳ optional | — | — |
| C. Gate semantics | C1. Steady-state gate trip (minimal) | ✅ | `1e31538` | 2026-05-10 |
| C. Gate semantics | C2. C1 + entry-spike advisory | ✅ | `1e31538` | 2026-05-10 |
| C. Gate semantics | C3. Locality-aware suggestions | ⏳ proposed | — | — |
| C. Gate semantics | C4. Per-locality gate verdict breakdown | ⏭️ deferred | — | — |
| B. Peak-finding | B0. Gating prerequisites re-evaluated | 🚫 blocked on C1 | — | — |
| B. Peak-finding | B1. `OptimizationStrategy` trait contract | 🚫 blocked on B0 | — | — |
| B. Peak-finding | B2. Closed-loop retarget composition (§3.5 deferred) | 🚫 blocked on B1 | — | — |
| B. Peak-finding | B3. Gaussian-process strategy (§11.6.4) | 🚫 blocked on B2 + re-gate | — | — |
| B. Peak-finding | B4. Strategy selection policy | 🚫 blocked on B3 | — | — |

Legend: ⏳ pending · 🟡 in-progress · ✅ done · 🚫 blocked · ⏭️ skipped

---

## Why now

The MCP smoke run for G16 §11 phase 2c (commit `3e68d48`, 2026-05-10)
showed wanaka_full_tuned.toml TP 1 (Back Rough) returning
`OptimizeOutcome::NoSafeImprovement` with three refined candidates,
all chipload Exceeds High at peak 0.0707 (28% over LUT max 0.055).
Two also Exceeds deflection at 237 µm.

The user-visible result was three "invalid" rows in the modal with no
guidance on:

1. Why the candidates all share the same chipload reading despite
   different stepover/DOC (peak_high statistic dominated by a hot
   slot section, not by bulk stepover).
2. What axis the search couldn't push past (feed at 4000 mm/min was
   the upper end of the search envelope).
3. What manual lever would unblock it (cap feed ~3700 mm/min and
   re-optimize).
4. Whether the optimizer simply gave up or genuinely had no feasible
   point in its envelope.

**Two design tensions exposed:**

- **Phase 1 (5% breakage_tolerance) absorbs small overshoots; phase 4
  (Adaptive3d → Pocket LUT reroute) opens up larger stepover candidates
  that produce larger chipload swings.** These two pull opposite
  directions on this fixture.
- **No closed-loop multi-retarget composition.** §3.5 of the G16 design
  doc deferred composition; the search makes one decision direction
  and doesn't iterate. Once stepover widens (search direction A) and
  chipload swings over the high side, no retargeter pulls feed back
  down (search direction B).

Two complementary fixes:

- **Thread A — Explainability surface:** make the "no safe improvement"
  case operator-actionable. Surfaces data already present in
  `OptimizeOutcome::attempted`. Mostly UI work, some narrative-string
  generation in core. Cheap.
- **Thread B — Peak-finding search:** the §11.6 deferred Layer 4. A
  closed-loop strategy that, when chipload swings high after a
  stepover widening, drops feed and re-runs. Eventually a Bayesian
  surrogate. Expensive.

Sequenced: A first (independent value — even with B, "no safe
improvement" cases will still happen on some fixtures), B gated on
A2's narrative landing first so engine and UI converge on the same
vocabulary.

---

## Mid-thread-A reflection (2026-05-10) — gate-decision is the bottleneck, not search

Thread A landed end-to-end. Re-running wanaka MCP smoke after A3
(locality classifier) revealed something the original plan missed:

> **Every gate violation across both wanaka TPs sits in a helical
> entry move, not steady-state cutting.** TP 1 chipload high,
> TP 1 deflection exceed, TP 6 chipload band-admit (high *and* low),
> TP 1 burn-side narrative — all carry `locality: "helix entry"` (or
> "heavy engagement"). Not one gate trip is in steady-state cut.

Implications for thread B:

1. **Bayesian (B3) wouldn't move the needle on wanaka.** GP would
   search the same `(feed × stepover × DOC)` space the grid already
   probes. Every candidate it proposes still trips on its own helix
   entry. The §11.6.4 gating criterion #1 — "optimizer settles on a
   local optimum > 10s slower than a hand-found global, both Within"
   — doesn't cleanly apply: there is no Within candidate within the
   search envelope under the current gate-trip rule.
2. **B2 (closed-loop retarget) has the same property.** Drops feed
   when chipload spikes; the spike is in the entry; the new
   candidate's entry still spikes; symptom-treating loop.
3. **The actual rule that's biting is gate-trip semantics, not
   search coverage.** The chipload evaluator already has a
   `steady_state_samples_for_toolpath` filter — used to find the LUT
   row to compare against, but *not* to drive the trip decision.
   Result: the trip decision is dominated by transient entry
   samples. The bulk cut in TP 1 is probably already Within.

This isn't a thread-A or thread-B problem. It's a third concern:
**what samples should drive the gate trip?** Hence Thread C below.

A small meta-observation: thread A worked exactly as designed.
Surfacing the locality field added 2 hours of work and rerouted a
1-2 week investment. Worth remembering for future explainability
phases — the structured data has compounding value beyond the UI it
was built for.

---

## Shared vocabulary contract

Both threads converge on the same operator-facing language. **Avoid
"Bayesian" in operator copy.** The search strategy is exposed as
"closed-loop retarget" or just "search"; the GP surrogate is an
implementation detail.

Failure-narrative phrases the modal renders today are mostly free
strings (`OptimizeOutcome::NoSafeImprovement::explanation`,
`OptimizeOutcome::MarginalSafe::explanation`). A1 promotes them to
structured fields; B2/B3 must read those structures (not invent new
prose) when emitting per-probe explanations during a closed-loop run.

---

## Thread A — Explainability surface

### A1. Structured failure narrative in `OptimizeOutcome`

**Reference:** new contract; replaces the free `explanation: String`.
**Effort:** ~½d. **Files:** 2-3. **LOC:** ~150 + tests.

Today: `OptimizeOutcome::NoSafeImprovement { reason, explanation, attempted }`
carries one prose string. The modal renders it as a single line.

Goal: structured fields the UI can render selectively + render with
operator-meaningful phrasing.

```rust
// crates/rs_cam_core/src/tool_load/optimize/outcome.rs
pub enum OptimizeOutcome {
    // ...
    NoSafeImprovement {
        reason: RefuseReason,
        narrative: FailureNarrative,
        attempted: Vec<OptimizeCandidate>,
    },
    MarginalSafe {
        candidates: Vec<OptimizeCandidate>,
        narrative: FailureNarrative,  // explains *why* band-admitted
    },
    TradeOff {
        candidates: Vec<OptimizeCandidate>,
        narrative: TradeOffNarrative,
    },
    // ...
}

pub struct FailureNarrative {
    /// One-line headline ("Couldn't find safe improvement at this stepover/feed envelope").
    pub headline: String,
    /// Per-gate limiting reading at the closest-to-safe candidate.
    pub limiting_gates: Vec<LimitingGate>,
    /// Search envelope reached (max feed, max stepover, etc).
    pub envelope: SearchEnvelopeReached,
    /// Operator-actionable suggestions ("cap feed below ~3700 mm/min and re-run").
    pub suggestions: Vec<OperatorSuggestion>,
}

pub struct LimitingGate {
    pub gate: GateKind,           // Chipload | Power | Deflection
    pub side: Option<GateSide>,   // Low / High for chipload; None for power/defl
    pub observed: f64,
    pub bound: f64,
    pub overshoot_pct: f64,
    pub band_admitted: bool,      // true if absorbed by tolerance band
}
```

- [ ] New `FailureNarrative` and `TradeOffNarrative` structs in `outcome.rs`.
- [ ] Builder `optimize/narrative.rs` (new) constructs from an attempted
      candidate set + baseline + policy.
- [ ] `build_outcome` in `outcome.rs` calls the builder for the three
      non-Ranked variants.
- [ ] `RefuseReason` retained on NoSafeImprovement (machine-readable);
      narrative is the human surface.
- [ ] Tests: 3 narrative-builder fixtures (chipload-high cliff,
      defl-exceeds, power-saturated).
- [ ] MCP description updated to mention the new structured fields.

### A2. Modal "what was tried, why each failed" view

**Reference:** consumes A1's `FailureNarrative`.
**Effort:** ~1d. **Files:** 1. **LOC:** ~250.

Today: `optimize_modal.rs` MarginalSafe / TradeOff arms render flat
candidate tables; NoSafeImprovement renders one prose line + a flat
attempted table.

Goal: structured per-outcome view that surfaces:

- Headline + 1-2 sentence narrative
- Search envelope reached (axes that hit max/min)
- Per-candidate "limiting gate" badge instead of generic
  "chipload Exceeds" — say "Chipload 0.071 (LUT max 0.055, +28%)"
- Highlighted suggestion lever (if `narrative.suggestions` non-empty)

- [ ] Re-architect `render_no_safe_improvement` to consume
      `FailureNarrative` instead of the prose string.
- [ ] Re-architect `render_marginal_safe` similarly.
- [ ] Re-architect `render_trade_off` similarly.
- [ ] Per-row badge with limiting gate + reading + overshoot %.
- [ ] Suggestion lever rendered as an inline callout (egui group
      with separate accent color); not a button (operator must
      manually act).
- [ ] No new colors — reuse existing yellow (caution) / red (exceeded)
      tokens from MarginalSafe arm.
- [ ] Manual smoke: load wanaka_full_tuned.toml, optimize TP 1
      (NoSafeImprovement), confirm new modal renders narrative +
      per-candidate limiting gate badges.

### A3. Hot-spot localization

**Reference:** uses existing `evidence.sample_range` (no new sim data).
**Effort:** ~1d. **Files:** 2-3. **LOC:** ~120.

Today: a `LimitingGate` says "chipload 0.071 at sample 25735". The
sample index is opaque.

Goal: surface "peak in slot section near corner X" using span /
region naming that already exists in the toolpath IR.

- [ ] Add `sample_index_to_span_label(toolpath_id, sample_idx) → Option<String>`
      in `optimize/narrative.rs` or a new `optimize/locality.rs`.
      Reads `AnnotatedToolpath` spans (`spans.rs`).
- [ ] Narrative builder calls it for each `LimitingGate` and stores
      `pub locality: Option<String>` on the struct.
- [ ] Modal renders locality as a sub-line under the badge: "in 1
      slot section near corner".
- [ ] Tests: 2 fixtures using the existing wanaka span data.

### A4. Operator-facing suggestion lever

**Reference:** consumes per-gate narrative + the search envelope.
**Effort:** ~½d. **Files:** 1-2. **LOC:** ~80.

Today: nothing. Operator stares at "no safe improvement" with no
hint at what to change.

Goal: emit one or two concrete suggestions per failure case,
generated from the limiting-gate readings.

```rust
pub enum OperatorSuggestion {
    /// "Cap feed below ~3700 mm/min and re-optimize."
    CapAxisAt { axis: KnobAxis, ceiling: f64, units: &'static str },
    /// "Reduce stepover below ~1.6 mm to avoid full-slot engagement."
    NarrowAxisBelow { axis: KnobAxis, ceiling: f64, units: &'static str },
    /// "No vendor LUT data above ~0.055 chipload — calibration would help."
    DataGapHere { reason: String },
}
```

- [ ] Suggestion synthesis lives in `optimize/narrative.rs::suggest_levers`.
      Inputs: `LimitingGate` set + the policy's tolerance bands +
      the search envelope reached. Output: ranked `Vec<OperatorSuggestion>`.
- [ ] Heuristic for chipload-high: invert the formula
      `chipload = feed / (RPM × teeth)` to compute "feed needed to
      bring observed chipload down to LUT max". Subtract a 5% margin.
- [ ] Heuristic for deflection-exceeds: suggest reducing axial DOC
      (no model needed; deflection scales roughly linearly with DOC).
- [ ] Heuristic for power-saturated: suggest reducing radial stepover
      (proxy for engagement).
- [ ] Tests: 3 suggestion-builder fixtures.

### A5. Search-frontier heatmap (optional, feature-flag first ship)

**Reference:** visual of attempted set; no new data.
**Effort:** ~2d. **Files:** new `ui/optimize_frontier.rs`. **LOC:** ~400.

Today: attempted candidates render as a flat list. Operator can't
see which (feed, stepover) corners were probed and how each fared.

Goal: 2D plot of the attempted set, coloured by verdict
(green = Within, yellow = band-admitted, red = Exceeds), with the
baseline highlighted. Operator sees the search frontier visually.

- [ ] Two-axis selection: most ops want feed × stepover. Different
      op kinds may swap (drop_cutter wants slope_to / stepover, etc).
      Driven by `OperationConfig` axis topology.
- [ ] Heatmap rendered via egui_plot; cells labelled with cycle time.
- [ ] Click a cell → highlights that candidate in the table above.
- [ ] **Defer behind a feature flag** (`optimize_frontier_heatmap`)
      for the first ship. Real ergonomics value but high LOC; bake
      after A1-A4 ship.

---

## Thread C — Gate semantics refinement

**Premise** (from the mid-thread-A reflection above): the gate-trip
decision currently fires on transient entry samples (helix / plunge),
not on steady-state cutting. On wanaka, every gate violation lives
in an entry move — the bulk cut is probably already Within. Search
improvements (B2 / B3) won't change this; they'd find the same
local optima and trip on the same entry samples.

Thread C is small, evaluator-side, and *might* dissolve thread B's
premise for wanaka entirely. Worst case it shrinks the search-coverage
gap so B's ROI is easier to evaluate honestly. Either way, do C
before B.

### C1. Steady-state gate trip (minimal)

**Reference:** mid-thread-A reflection.
**Effort:** ~½d. **Files:** 3 evaluators (`chipload.rs`, `power.rs`,
`deflection.rs`). **LOC:** small — re-routing existing data, no new
filter to write.

Today: the chipload evaluator's `peak_above` / `peak_below` /
`peak_in_range` loop walks `steady_state_samples` (already filtered
for steady-state via `steady_state_samples_for_toolpath`) — but the
input filter is wide. Look at the actual filter — it walks samples
where `is_cutting && radial_engagement >= 0.02 && |feed - operation_feed| < tolerance`.
That admits helix-entry samples (they're cutting, engaged, at the
operation feed). The filter excludes air cuts and rapids, not
transient entries.

Goal: tighten the gate-trip filter so transient entry samples
(`CutKinematics::Helix`, `CutKinematics::Plunge`) don't drive the
trip decision. Same change applied to power + deflection evaluators.

- [ ] Add `is_steady_state_for_gate(sample)` predicate near
      `steady_state_samples_for_toolpath`. Returns false for
      `CutKinematics::Helix` and `CutKinematics::Plunge`. (The
      narrative module's `classify_sample_locality` already
      enumerates these; reuse the same condition.)
- [ ] Apply in chipload's main sample loop: continue past
      transient-entry samples for both `peak_above` and
      `burn_samples`. The bounds-matching pass that picks the LUT
      row's `lookup_axial_doc_mm` keeps reading every sample (we
      want the entry's DOC for matching, just not for the trip).
- [ ] Apply in power's `peak_idx` selection.
- [ ] Apply in deflection's `peak_idx` selection.
- [ ] Do **not** mutate the verdict's evidence to claim the entry
      sample didn't exist — the existing `confidence.detail` strings
      ("slot engagement (arc >= π) — climb/conventional split not
      modeled") still surface entry context. The change is *only*
      in which samples drive the trip decision.
- [ ] Tests: 3 fixtures per evaluator (entry-only spike → Within;
      steady-state spike → Exceeds; mixed → Exceeds on the
      steady-state reading not the entry). One regression for
      wanaka_full_tuned TP 1 in `tests/wanaka_*.rs` if a fixture
      can be cooked up there.
- [ ] Wanaka MCP smoke afterward: TP 1 should land Ranked instead
      of NoSafeImprovement; suggestion lever should change to
      "no action needed" (or empty).

**Risk:** moderate. The behavior change is real. A user who genuinely
broke a tool on a bad helix entry would have seen `Exceeds` before;
post-C1 they'd see `Within` plus a confidence detail string. C2
addresses this by surfacing entry spikes as informational advisories.

**Counter-evidence to gather:** is there any project where the entry
sample IS the legitimate concern? Worth running C1 against fixtures
beyond wanaka before locking in the rule.

### C2. C1 + entry-spike advisory

**Reference:** mitigates C1's "did we hide a real entry failure?"
risk.
**Effort:** ~1d after C1 lands. **Files:** verdict + narrative + modal. **LOC:** ~80.

Goal: when an entry sample exceeds the bound but the steady-state
trip says Within, surface it as an advisory — visible on the verdict
+ narrative without flipping the gate decision.

Two approaches; option B is preferred:

- **Option A:** new variant `EntryWarning` between `Within` and
  `Exceeds` on each gate verdict. Three-state → four-state
  enum. Touches every verdict consumer. Lots of mechanical churn for
  one new bit.
- **Option B (preferred):** add `entry_spike: Option<EntrySpike>`
  field to each verdict's `Within` arm. When set, contains the worst
  entry-sample reading + bound + locality. UI renders as a "Note:"
  line under the verdict badge, neutral colour (not red). Narrative's
  `LimitingGate` could carry an `advisory: bool` flag mirroring
  `band_admitted` for similar treatment.

- [ ] Decide A vs B (recommend B).
- [ ] Plumb the chosen shape through the three evaluators —
      record entry spikes during the same pass that runs C1's
      filtered trip decision.
- [ ] Modal renders advisory line on the per-row verdict badge:
      "Note: helix entry sample reached 0.0707 — consider gentler
      helix."
- [ ] Tests: 2 per evaluator (entry-only spike → Within +
      advisory; mixed entry spike + steady-state Exceeds →
      Exceeds, no advisory needed because the trip is already
      visible).

**Risk:** low. Pure additive shape change.

### C3. Locality-aware suggestions

**Reference:** A4's `suggest_levers` reads `LimitingGate.observed`
and `bound`; it doesn't read `locality`. The wanaka run showed it
suggesting "cap feed at 2954 mm/min" when the limiting reading was
in a helix entry — the cap would slow the bulk cut and barely
improve the entry. Wrong leverage.
**Effort:** ~½d. **Files:** `narrative.rs`. **LOC:** ~40.

Goal: when `LimitingGate.locality` is "helix entry" / "plunge entry"
/ "heavy engagement", emit entry-strategy suggestions instead of
bulk-parameter caps.

- [ ] New variants on `OperatorSuggestion`:
      - `WidenHelix { current: f64, suggested: f64 }` ("Try
        increasing helix_radius_factor above ~0.5")
      - `SlowEntry { current: f64, suggested: f64 }` ("Try
        reducing helix_pitch below ~1.5 mm")
      - `SwitchEntryStyle { from: EntryStyle, to: EntryStyle }`
        ("Try ramp entry instead of plunge")
- [ ] In `suggest_for_gate`, branch on `g.locality` first; only
      fall through to bulk-parameter caps when locality is None or
      "slot section".
- [ ] Heuristic for entry-style suggestions: rough reciprocals of
      the same chipload formula, with operator-friendly defaults
      (e.g. helix_radius_factor target = current × 1.5, capped at 0.7).
- [ ] Modal `format_suggestion` renders the new variants.
- [ ] Tests: 2 per new variant.

Independent of C1/C2 — could ship before or after, but most useful
*after* C1 lands (since post-C1 some "no safe improvement" cases
will *only* have entry-locality limiting gates left, and the
suggestion needs to match).

### C4. Per-locality gate verdict breakdown

**Reference:** most thorough — emit gate state per locality bucket.
**Effort:** ~2d. **Files:** verdict, narrative, modal. **LOC:** ~250.
**Status:** ⏭️ **Deferred.**

The shape would be: each gate verdict carries
`Map<Locality, GateState>` so the UI can render a small table:

| Region | Chipload | Power | Deflection |
|---|---|---|---|
| Steady-state | Within | Within | Within |
| Helix entry | Exceeds High | Within | Exceeds |
| Plunge entry | — | — | — |

Defer until C1+C2+C3 prove insufficient. The shape is also more
invasive than C2's optional advisory and the UI gets busy fast.

---

## Thread B — Peak-finding search

### B0. Gating prerequisites re-evaluated

**Reference:** G16 §11.6.4 (criteria 1, 2, 3).
**Effort:** ~½d (data review only). **Files:** 0.

After A2 ships and operators run the new modal across 5+ projects,
re-check whether the original §11.6.4 gating criteria still trip:

1. Wanaka + 3 fixture projects show the optimizer settling on a
   local optimum > 10s slower than a hand-found global optimum on
   any TP.
2. The local-optimum candidate is `Within` on all gates AND the
   global optimum is also `Within` (search-coverage gap, not policy).
3. Adding probes to the existing grid does not close the gap.

Possible outcomes:

- **All three trip:** start B1.
- **Some trip but A's narrative + C's gate semantics made the gap
  acceptable:** defer again, document what changed.
- **None trip:** archive thread B; the gap was perceptual or a
  gate-rule artefact, not algorithmic.

- [ ] **Pre-requisite: C1 must land first.** The mid-thread-A
      reflection showed wanaka's gate trips are entry-sample driven,
      not search-coverage driven. Re-running B0 before C1 would
      attribute a gate-rule problem to a search-coverage problem.
- [ ] Run optimize on 5+ projects with A2 modal + C1 active.
- [ ] Hand-search the same projects for global optima within the same
      search envelope.
- [ ] Document gap data in this tracker; gate B1 explicitly on the
      result.

### B1. `OptimizationStrategy` trait contract

**Reference:** G16 §11.6.4 reserves `strategy/gaussian_process.rs`
implementing `OptimizationStrategy`.
**Effort:** ~½d. **Files:** new `optimize/strategy/mod.rs`. **LOC:** ~80.

```rust
pub trait OptimizationStrategy {
    /// Strategy name for narrative ("closed-loop retarget", "grid sweep").
    fn name(&self) -> &'static str;

    /// Propose the next batch of candidates given the history.
    /// Returns None when the strategy has nothing more to try.
    fn propose(
        &mut self,
        history: &[OptimizeCandidate],
        baseline: &OptimizeCandidate,
        policy: &SearchPolicy,
    ) -> Option<Vec<OperationConfig>>;

    /// Per-probe explanation appended to the candidate's narrative
    /// ("dropped feed 8% because chipload was 28% over LUT max").
    fn last_probe_rationale(&self) -> Option<String>;
}
```

- [ ] Trait + doc-comments.
- [ ] Existing grid-sweep refactored to implement it (rename
      `grid_strategy.rs`; minimal behaviour change).
- [ ] Per-probe rationale flows into A1's `FailureNarrative` structs
      (vocabulary contract enforced).
- [ ] Tests: existing grid behaviour preserved.

### B2. Closed-loop retarget composition

**Reference:** §3.5 deferred from G16; the §11.6.4 design predecessor.
**Effort:** ~3-5d. **Files:** new `optimize/strategy/closed_loop.rs`. **LOC:** ~400.

Goal: when a probe surfaces a violated gate, the strategy retargets
the offending knob and re-probes. Hill-climb within the feasible
band.

Algorithm sketch:

1. Take current best candidate's verdict.
2. If it's `Within` and faster than baseline by >ε: emit it.
3. If it `Exceeds` on gate G: ask the existing per-gate retargeter
   (G16 step 5/6 retargeters) for a knob delta that would bring G
   into bounds.
4. Apply the delta. Probe. Repeat.
5. Bound iteration count (e.g. 5) and ε on knob delta to guarantee
   termination.

Key insight: most "no safe improvement" cases this layer fixes
look like the wanaka TP 1 case — chipload high after a stepover
widening, fixable by a feed-down probe.

- [ ] `ClosedLoopStrategy` implementing `OptimizationStrategy`.
- [ ] Iteration loop with termination bounds.
- [ ] Each iteration's per-probe rationale flows into A1 narrative.
- [ ] Re-uses existing per-gate retargeters from G16 step 5/6.
- [ ] Stage 2 grid is the seed; closed-loop runs on top of the
      Stage 2 best per region.
- [ ] Wanaka MCP smoke: optimize TP 1 should now return
      `OptimizeOutcome::Ranked` (or MarginalSafe, depending on data),
      with an attempted set showing the iteration trail.
- [ ] **Re-evaluate gating criteria after this lands.** B2 may close
      the gap without B3.

### B3. Gaussian-process strategy (§11.6.4)

**Reference:** G16 §11.6.4 (deferred). Run only if B2 leaves a measurable gap.
**Effort:** ~1-2w. **Files:** new `strategy/gaussian_process.rs`. **LOC:** ~600.

Goal: GP surrogate over already-evaluated candidates. Proposes next
probe by maximizing acquisition function (expected improvement)
constrained to the feasible band defined by the gates.

- [ ] Pick crate: prefer pure Rust (`gprs` or hand-rolled small impl)
      over Python interop.
- [ ] Kernel selection: Matérn 5/2 with separate length scales per
      axis is the default; revisit if data argues otherwise.
- [ ] Per-probe rationale: "expected improvement +X.Xs at this point;
      uncertainty Yσ".
- [ ] Strategy fall-back: if GP regression diverges (e.g. <5 sample
      points), fall back to closed-loop B2.
- [ ] Operator-facing label: still "closed-loop search" in the modal;
      GP is internal.

### B4. Strategy selection policy

**Reference:** new `SearchStrategyPolicy` in `optimize/policy.rs`.
**Effort:** ~½d. **Files:** 1. **LOC:** ~50.

Goal: choose the strategy per op kind. Fast ops get grid sweep;
slow ops get closed-loop or GP.

- [ ] `SearchStrategyPolicy` enum: GridOnly, ClosedLoop, Adaptive.
- [ ] Per-op-kind defaults in `RankingPolicy::default()` (or
      separate sub-policy).
- [ ] Override path for power-user via project file.

---

## Sequencing

```
A1 ✅ ─→ A2 ✅ ─→ A3 ✅ ─→ A4 ✅
                            │
                            ▼
                     C1 ─→ (wanaka MCP smoke)
                            │
                            ├─→ wanaka now Ranked? ──→ C2 ─→ C3 ─→ (other-fixture survey) ──┐
                            │                                                                ▼
                            └─→ wanaka still NoSafeImprovement? ─→ B0 ─→ B1 ─→ B2 ─→ (re-gate) ─→ B3 ─→ B4
```

**C1 is the new cut point.** The mid-thread-A reflection showed that
on wanaka the search isn't the bottleneck — the gate-trip rule is.
C1 is small (~½ day) and may dissolve thread B's premise for this
project. If C1 transforms the picture, C2+C3 polish the gate-semantics
story and B becomes "for other projects, eventually". If C1 doesn't
help, B0's gating data is sharper because we've ruled out the
gate-rule explanation.

A5 (heatmap) is independent of everything and can ship anytime.

---

## Effort / risk summary

| Phase | Effort | Risk | Notes |
|---|---|---|---|
| A1 | ½d | low | Pure data-shape refactor + builder |
| A2 | 1d | medium | UI rework; need design eye for visual hierarchy |
| A3 | 1d | low | Reads existing span data |
| A4 | ½d | low | Heuristic synthesis from existing fields |
| A5 | 2d | medium | Optional; heatmap UX needs care |
| C1 | ½d | medium | Behaviour change; needs cross-fixture sanity |
| C2 | 1d | low | Pure additive shape change; mitigates C1 risk |
| C3 | ½d | low | Heuristic synthesis on top of A4 |
| C4 | 2d | medium | Deferred; covered by C1+C2+C3 today |
| B0 | ½d | n/a | Re-evaluate prerequisites (now post-C1) |
| B1 | ½d | low | Trait extraction |
| B2 | 3-5d | medium | Closed-loop iteration; care with termination |
| B3 | 1-2w | high | GP integration; may not be needed if B2 closes the gap |
| B4 | ½d | low | Policy literal + selector |

Total to "magic isn't gone but is explained": **A1+A2+A3+A4 ≈ 3d** (✅ shipped).
Total to "wanaka actually optimises (best case)": **+C1 ≈ ½d**.
Total to "wanaka optimises + safe entry-spike surface + better suggestions": **+C1+C2+C3 ≈ 2d**.
Total to "magic also doesn't fail as often on other projects": **+B1+B2 ≈ 4-6d** (only after C0 data review).

---

## Risks

- **Thread A risk: narrative becomes verbose / overwhelming.**
  Mitigation: ship A1+A2 first, gather feedback before A3+A4. Each
  field is opt-in for the renderer.
- **Thread A risk: suggestion lever (A4) gives bad advice.** Heuristics
  are coarse. Mitigation: phrase as "you'd need roughly X" not "set
  X". Never auto-apply. (Confirmed real on wanaka — A4 suggests
  capping bulk feed when the limiting reading is in the entry.
  Mitigated by C3.)
- **Thread C risk: hides legitimate entry failures.** C1 changes
  what samples drive the trip; an operator who genuinely broke a
  tool on a bad helix entry would have seen `Exceeds` before C1.
  Mitigation: C2 adds entry-spike advisory so the data is still
  visible without flipping the trip; ship C1+C2 together if
  possible.
- **Thread C risk: wanaka-specific bias.** The "every gate trip is
  in the entry" pattern is from one project. Could be a wanaka
  helix-config quirk, not a general issue. Mitigation: run C1
  against ≥2 other fixtures before locking the rule. If we don't
  have suitable fixtures, log the risk and accept it.
- **Thread B risk: closed-loop retarget oscillates.** Two opposing
  retargeters could ping-pong (drop feed → chipload OK but cycle
  worse → bump feed → chipload Exceeds again). Mitigation: bound
  iterations, treat oscillation as termination signal, surface
  "search converged on a local minimum" in narrative.
- **Thread B risk: GP over-engineering.** B2 may close the gap;
  C1 may dissolve the premise. Hard re-gate after C1 *and* after
  B2; do not start B3 by inertia.
- **Vocabulary drift:** B emits per-probe rationale that doesn't
  match A's narrative phrasing. Mitigation: B reuses A's enums
  (`LimitingGate`, `OperatorSuggestion`) as the rationale carrier.

---

## Open questions

- **Naming for thread B in operator copy.** Avoid "Bayesian";
  "closed-loop search" or just "search refining"?
- **Should A5 (heatmap) be behind a feature flag for the first ship?**
  Recommended yes — defer until A1-A4 settle.
- **Suggestion lever data ownership** — heuristic in core
  (`optimize/narrative.rs`) or in the UI? Recommended core, so MCP
  exposes the same suggestions to agent-driven flows.
- **B2 termination policy** — fixed iteration cap, or
  diminishing-returns threshold on cycle improvement?

---

## Compact prompt — A1

Paste verbatim into a fresh session.

```
G17-A1: structured failure narrative in OptimizeOutcome. Plan in
planning/OPTIMIZE_EXPLAINABILITY_AND_PEAK_FINDING.md.

CONTEXT
G16 §11 phase 2c (commit 3e68d48) closed the layered scoring work.
Wanaka MCP smoke showed TP 1 (Back Rough) returning NoSafeImprovement
with an opaque "magic didn't work" surface for the operator. A1
promotes the free `explanation: String` on NoSafeImprovement /
MarginalSafe / TradeOff to structured `FailureNarrative` /
`TradeOffNarrative` carrying limiting-gate readings, search envelope,
and operator suggestions.

DO
1. New structs `FailureNarrative`, `TradeOffNarrative`, `LimitingGate`,
   `SearchEnvelopeReached`, `OperatorSuggestion` in
   crates/rs_cam_core/src/tool_load/optimize/outcome.rs.
2. New module `crates/rs_cam_core/src/tool_load/optimize/narrative.rs`
   with builder `build_failure_narrative(attempted, baseline, policy)`.
3. `build_outcome` in outcome.rs calls the builder for the three
   non-Ranked variants.
4. Tests: 3 narrative-builder fixtures (chipload-high cliff,
   defl-exceeds, power-saturated). Add to outcome.rs `mod tests`.
5. UI render-side untouched in this commit — the narrative is built
   and stored, A2 will render it.

HARD GATES
- cargo test -p rs_cam_core --lib pass
- cargo clippy --workspace --all-targets -- -D warnings clean
- per-file git add (workspace fmt drift in tool_load/optimize sibling
  files; revert anything rustfmt cascades — see memory:
  feedback_rustfmt_cascade.md)

UPDATE TRACKER
Edit planning/OPTIMIZE_EXPLAINABILITY_AND_PEAK_FINDING.md, mark A1
✅ done with hash + date. Backfill `OptimizeOutcome` shape if it
diverged from this prompt.
```

(Compact prompts for A2-A4 and B-series omitted; write at start of
each phase using A1 as template.)

---

## Notes / deviations log

(Append entries here as phases land.)

- **2026-05-10 — A1.** Kept `explanation: String` alongside `narrative`
  on NoSafeImprovement and MarginalSafe to preserve the UI render path
  untouched (per A1's "no UI changes" constraint). A2 will swap the UI
  to read `narrative.headline` and the explanation field can be
  retired then.
- **2026-05-10 — A1.** Boxed `narrative` (Box<FailureNarrative> /
  Box<TradeOffNarrative>) inside the variants to keep
  `clippy::large_enum_variant` quiet. The narrative carries a String +
  Vec<LimitingGate> + envelope, which would make the failure variants
  ~190 bytes vs Ranked at ~24 bytes without boxing.
- **2026-05-10 — A1.** Converted `TradeOff(Vec<OptimizeCandidate>)`
  to `TradeOff { candidates, narrative }` struct variant. Touched 11
  pattern sites across core + viz + tests; mechanical syntax changes
  only.
- **2026-05-10 — A1.** Cancel-path / pre-flight refusal sites in
  `optimize/mod.rs`, `optimize/candidate.rs`, and viz worker emit
  `narrative: Box::default()` (empty narrative) — no attempted set yet,
  no envelope to compute. Production sites going through `build_outcome`
  always emit a real narrative.
- **2026-05-10 — A1.** `config_axes` enumerates only the four families
  the optimizer targets today (Pocket / Adaptive / Adaptive3d /
  Scallop) and falls through to `(None, …)` for everything else. The
  envelope stays empty for ops the optimizer doesn't move; A2 / future
  ops add their own arms when needed.
- **2026-05-10 — A2.** Re-exported narrative types (`FailureNarrative`,
  `TradeOffNarrative`, `LimitingGate`, `SearchEnvelopeReached`,
  `GateKind`, `KnobAxis`, `OperatorSuggestion`, `AxisExtent`) from
  `optimize/mod.rs` so the viz crate can read structured fields
  without depending on the private `narrative` module.
- **2026-05-10 — A2.** Added pub `limiting_gates_for_verdict(verdict)`
  in `narrative.rs` so per-row UI rendering can compute the limiting
  reading on each candidate (not just the recommended one). Surfaces
  both Exceeds and band-admitted readings in one call.
- **2026-05-10 — A2.** Modal arms for NoSafeImprovement / MarginalSafe
  / TradeOff now read `narrative.headline` instead of the old free
  `explanation` string. The `explanation` field is kept on
  NoSafeImprovement / MarginalSafe for any consumer not yet migrated
  (notably project rollup uses both at lines 314 / 608); it can be
  retired once all consumers move to `narrative.headline`.
- **2026-05-10 — A2.** Per-row "status" cell in the attempted table
  swapped from generic "gate" / "slower" / "ok" to a specific
  limiting-gate reading ("chipload 0.0707 (+29%)", "defl 237 µm
  (+19%)"). Coloured red for hard Exceeds, yellow for band-admitted.
- **2026-05-10 — A2.** Manual GUI smoke deferred. New format helpers
  (`format_envelope_summary`, `format_limiting_gate`) covered by 4
  unit tests with wanaka-realistic values; MCP-side JSON smoke pending
  binary rebuild.
- **2026-05-10 — A3 scope reduction.** Original plan called for
  full sample → AnnotatedToolpath span-path lookup ("in slot section
  near corner X"). That requires threading AnnotatedToolpath into all
  three per-gate evaluators (chipload / power / deflection), which is
  invasive. Switched to a lightweight classifier that reads only
  fields already on `SimulationCutSample` (cut_kinematics + arc
  engagement). Trade-off is precision — we get "slot section" but
  not "near corner X". The sample-locality enum captures the
  operator-actionable insight (the sample is in a slot region →
  reduce stepover) without invasive plumbing. If the precision gap
  matters later, span-path lookup can be added in a follow-up that
  threads `&AnnotatedToolpath` through evaluator signatures.
- **2026-05-10 — A3.** New module `tool_load/locality.rs` with
  `classify_sample_locality(&SimulationCutSample) → Option<String>`.
  Returns: "plunge entry" / "helix entry" (kinematics override),
  "slot section" (arc ≥ π), "heavy engagement" (arc ≥ π/2), or
  `None` (steady-state / no arc captured). Order matters —
  kinematics dominates engagement classification.
- **2026-05-10 — A3.** Added `locality: Option<String>` field +
  `with_locality(self)` builder on `SampleEvidence`. All 3 evaluators
  (chipload / power / deflection) attach locality at the moment they
  pick a triggering sample — same `idx` they record into the evidence,
  no extra lookups. `LimitingGate.locality` propagates from there.
- **2026-05-10 — A3.** Modal `format_limiting_gate` appends locality
  as " — <label>" suffix. Wanaka TP 1 case will read
  "chipload 0.0707 (+29%) — slot section". Headline strings
  untouched — locality only renders in the per-row badge. A future
  polish could fold a shared locality into the headline if all rows
  agree.
- **2026-05-10 — A4.** Added `RaiseAxisAbove { axis, floor }` variant
  to `OperatorSuggestion` to cover the burn-side chipload case
  (operator needs to *raise* feed, not cap it). The plan's original
  enum only had `CapAxisAt` and `NarrowAxisBelow`; merged
  `NarrowAxisBelow` semantics into `CapAxisAt` since both are caps —
  the only operator-meaningful distinction is the direction (cap vs
  raise), not "narrow" vs "cap".
- **2026-05-10 — A4.** `suggest_levers(limiting_gates, candidate)` in
  `narrative.rs`. Heuristics per gate:
    - chipload high → `CapAxisAt(Feed, current_feed × bound/observed × 0.95)`
    - chipload low → `RaiseAxisAbove(Feed, current_feed × bound/observed × 1.05)`
    - deflection exceeds → `CapAxisAt(DepthPerPass, current_doc × bound/observed × 0.95)`
    - power exceeds → `CapAxisAt(Stepover, current_stepover × bound/observed × 0.95)`
  Each emits at most one suggestion per limiting gate; the operator
  picks which lever they prefer.
- **2026-05-10 — A4.** Suggestions wired into `build_failure_narrative_no_safe`
  (NoSafeImprovement). Skipped MarginalSafe — those candidates are
  inside the band already, "verify on a scrap" is the entire action.
- **2026-05-10 — A4.** Modal renders suggestions as a "Try this"
  callout with bullet list (no buttons — operator must manually act,
  per A4 plan). `format_suggestion(s)` produces operator copy: "Cap
  feed at ~2961 mm/min and re-optimize." / "Raise feed above ~4032
  mm/min and re-optimize." Avoids "Bayesian" / "closed-loop" /
  engine vocabulary.
- **2026-05-10 — A4 MCP smoke (post-rebuild).** TP 1 narrative
  carries `suggestions: [cap_axis_at(feed, 2954.67), cap_axis_at(depth_per_pass, 3.88)]`
  — both heuristic outputs match closed-form math. TP 6 (MarginalSafe)
  carries `suggestions: []` as designed.
- **2026-05-10 — Thread B re-gated on C1.** Mid-thread-A reflection
  (see body) showed wanaka's gate trips are entry-sample driven, not
  search-coverage driven. Bayesian (B3) wouldn't fix wanaka because
  it'd search the same space and trip on the same entry samples.
  Inserted Thread C (gate semantics refinement) ahead of B0; B0's
  data-collection now requires C1 to land first so we don't attribute
  a gate-rule artefact to a search-coverage gap. C1 is small (~½ day);
  may dissolve B's premise for wanaka entirely. Worst case it
  shrinks the gap so B's ROI is easier to evaluate honestly.
- **2026-05-10 — A3+A4 finding worth filing for B / future polish.**
  A3 locality across both wanaka TPs reveals that **every chipload /
  deflection / power peak in this project sits in a helical entry
  move, not steady-state cutting**. The real bottleneck on TP 1 is
  the helix entry strategy (helix_pitch, helix_radius_factor, or
  switching to ramp entry), not bulk roughing parameters. The current
  `suggest_levers` doesn't read locality — it suggests capping bulk
  feed when the actual fix is at the entry. Two enhancements worth
  considering after thread B:
    - `suggest_levers` reads `LimitingGate.locality` and prefers
      entry-style suggestions ("Try ramp entry instead of plunge"
      or "Increase helix_radius_factor above ~0.5") when the
      limiting sample sits in an entry phase.
    - Steady-state-only mode for the chipload evaluator that ignores
      transient entry samples when computing the gate trigger,
      similar to the `steady_state_samples_for_toolpath` filter
      already present for chipload bounds matching but not for the
      gate-trip decision itself.
  These are out of scope for thread A; logging here so the insight
  isn't lost.
- **2026-05-10 — A2 MCP smoke (post-rebuild).** Verified narrative
  serializes through MCP on `wanaka_full_tuned.toml`:
    - **TP 1 (NoSafeImprovement):** headline reads "Tried 3
      candidates; closest-to-safe still hit: chipload 0.0707 mm/tooth
      (+29% over LUT max 0.0550); deflection 237 µm (+18% over the
      200 µm threshold)." Envelope captures feed 3150→4000, stepover
      0.84→2.2, DOC 3.0→4.83, RPM held at 18000.
    - **TP 6 (MarginalSafe):** headline reads "Best candidate is +1%
      past the strict chipload bound — admitted by the tolerance band;
      verify on a scrap before applying." Envelope captures feed
      3150→3500, RPM 18000→20000, stepover 2.0→2.6, DOC collapsed at
      3.0 (UI suppresses collapsed extents).
    - **Unexpected (filed for polish):** TP 6's recommended candidate
      is band-admitted on BOTH chipload sides simultaneously (peak
      0.0557 just over LUT max 0.055, median 0.0309 just under LUT
      min 0.032). 5% `breakage_tolerance` and `burn_tolerance` both
      fire. `headline_marginal` only surfaces the first
      (`find(|g| g.band_admitted)` returns High side); could
      summarize both in a future polish pass.
- **2026-05-10 — C1.** Added `is_steady_state_for_gate(sample)` in
  `tool_load/locality.rs` returning `true` for everything except
  `CutKinematics::{Helix, Plunge}`. Wired into chipload, power, and
  deflection evaluators so only steady-state samples can drive the
  `Exceeds` verdict. Pre-existing `steady_state_samples_for_toolpath`
  filter (chipload-bound matching) was untouched — these are
  complementary; the new predicate gates the *trip decision*, the old
  filter affects *which row of the LUT* gets matched. Renamed
  `helix_steady_state_samples_are_kept` →
  `helix_samples_route_to_entry_spike_not_trip` to reflect inverted
  semantics; added 3 new tests covering steady-state high trip, plunge
  routing, and mixed steady+entry overshoot.
- **2026-05-10 — C2.** Added `EntrySpike` to `verdict.rs` (observed,
  bound, locality, optional `side: ChipSide`). ChiploadVerdict::Within
  carries `entry_spikes: Vec<EntrySpike>` (high+low possible);
  PowerVerdict::Within and DeflectionVerdict::Within carry
  `entry_spike: Option<EntrySpike>` (single side). Each evaluator
  tracks the worst entry-side overshoot in a separate scalar so the
  steady-state peak picker is unaffected. Narrative side: added
  `EntryAdvisory` + `entry_advisories_for_verdict(verdict)` →
  `Vec<EntryAdvisory>`; populated in both `build_failure_narrative_no_safe`
  and `build_failure_narrative_marginal`. Modal renders advisories as
  muted "Note: helix entry chipload reached … (+N% over LUT max …)
  — consider gentler entry." lines under the headline. Suggestion
  copy stays out of advisory rendering — C3 will wire locality-aware
  suggestions; for now the advisory is purely informational so a
  legitimate entry failure isn't hidden by C1.
