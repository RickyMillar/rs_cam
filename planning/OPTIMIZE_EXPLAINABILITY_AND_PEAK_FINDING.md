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
| A. Explainability | A3. Hot-spot localization (sample → span name) | ⏳ | — | — |
| A. Explainability | A4. Operator-facing suggestion lever | ⏳ | — | — |
| A. Explainability | A5. Search-frontier heatmap (feed × stepover) | ⏳ optional | — | — |
| B. Peak-finding | B0. Gating prerequisites re-evaluated | 🚫 blocked on A2 | — | — |
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
- **Some trip but A's narrative made the gap acceptable:** defer
  again, document what changed.
- **None trip:** archive thread B; the gap was perceptual, not
  algorithmic.

- [ ] Run optimize on 5+ projects with A2 modal active.
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
A1 ─→ A2 ─→ A3 ─→ A4
              │
              └─→ B0 ─→ B1 ─→ B2 ─→ (re-gate) ─→ B3 ─→ B4
```

A2 is the cut point: thread B does not start until A2 ships and the
narrative vocabulary contract is real. After B2 lands, re-evaluate
whether B3 is needed (it may not be).

A5 (heatmap) is independent of B and can ship anytime after A2.

---

## Effort / risk summary

| Phase | Effort | Risk | Notes |
|---|---|---|---|
| A1 | ½d | low | Pure data-shape refactor + builder |
| A2 | 1d | medium | UI rework; need design eye for visual hierarchy |
| A3 | 1d | low | Reads existing span data |
| A4 | ½d | low | Heuristic synthesis from existing fields |
| A5 | 2d | medium | Optional; heatmap UX needs care |
| B0 | ½d | n/a | Re-evaluate prerequisites |
| B1 | ½d | low | Trait extraction |
| B2 | 3-5d | medium | Closed-loop iteration; care with termination |
| B3 | 1-2w | high | GP integration; may not be needed if B2 closes the gap |
| B4 | ½d | low | Policy literal + selector |

Total to "magic isn't gone but is explained": **A1+A2+A3+A4 ≈ 3d**.
Total to "magic also doesn't fail as often": **+B1+B2 ≈ 4-6d**.

---

## Risks

- **Thread A risk: narrative becomes verbose / overwhelming.**
  Mitigation: ship A1+A2 first, gather feedback before A3+A4. Each
  field is opt-in for the renderer.
- **Thread A risk: suggestion lever (A4) gives bad advice.** Heuristics
  are coarse. Mitigation: phrase as "you'd need roughly X" not "set
  X". Never auto-apply.
- **Thread B risk: closed-loop retarget oscillates.** Two opposing
  retargeters could ping-pong (drop feed → chipload OK but cycle
  worse → bump feed → chipload Exceeds again). Mitigation: bound
  iterations, treat oscillation as termination signal, surface
  "search converged on a local minimum" in narrative.
- **Thread B risk: GP over-engineering.** B2 may close the gap. Hard
  re-gate after B2 lands; do not start B3 by inertia.
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
