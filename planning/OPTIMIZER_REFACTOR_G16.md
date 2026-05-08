# G16 — Optimizer architectural refactor

**Status.** Revision 2 — 2026-05-08. Independent reviewer approved with
required changes; this revision incorporates them.

**Revision log:**
- **R2 (2026-05-08).** Reviewer feedback addressed. Major changes:
  - §3.2 trait pattern reworked from `OptimizableOp` impls (silent
    fallthrough risk) to `OperationConfig::optimization_surface()` with
    explicit non-wildcard match. Adds genuine compile-time coverage.
  - §3.2 introduces `AxisContext` so spindle RPM resolution sees the
    project default — `Option<u32>` no longer conflated with "axis
    absent".
  - §3.2 introduces `AxisBinding` with semantics (LoadDriving /
    QualityTarget / CycleTimeDriving) so scallop_height isn't treated as
    interchangeable with stepover.
  - §3.3 `AxisBounds` split into `hard` / `preferred` / `warm_start`
    intervals. The wanaka TP 4 fix now requires *both* the bounds split
    *and* G17 (LUT-family routing) — previous draft overclaimed.
  - §3.4 `RetargetSolution` returns `Vec<AxisPatch>` so coupled
    feed/RPM/plunge changes are first-class. Plunge tracking moves from
    hidden side-effect to explicit coupling rule.
  - §3.5 multi-retarget composition deferred (separate candidates first).
  - §3.0 `PolicyValue<T>` adds rationale + source provenance to every
    constant.
  - §1.1 `ChiploadVerdict` shape moves from raw peak fields to
    `ChiploadMetric` carrying the statistic kind (peak vs median).
  - §5 migration reorder: typed verdicts move from Step 2 to Step 7
    (after search architecture proves out), per reviewer request.
  - §9 open questions marked resolved with reviewer answers.

**Scope.** Rewrite the structure of `tool_load/optimize.rs` (~4000 lines)
around explicit traits, typed search axes, sample-driven retargeting, and
named policy. **No backward-compat constraints** — wire format and
internal types may change freely; this is pre-PRD.

**Non-goals.** Do not change: the per-gate evaluation logic
(`chipload::evaluate`, `power::evaluate`, `deflection::evaluate` — these
already do the right thing); the vendor-LUT JSON format; the
`MillingCutter` trait surface; the simulation/trace infrastructure;
`ProjectSession`; `execute_operation`; the wider engine. The refactor is
isolated to **the optimizer's search/orchestration layer + the verdict
output types**.

---

## 1. Why this refactor

The optimizer body has been growing by addition for ~10 commits. Each
new gap closure (G1, G2, G3, G5+G6+G7, G13) added a conditional at a
seam. Five symptoms of structural drift, with concrete examples from
the current codebase:

### 1.1 `has_doc_knob` allowlist + scattered `Option`-checks

Adding a new operation type requires editing:
- `has_doc_knob()` at `optimize.rs:931` (DOC axis allowlist)
- The op's `OperationParams::depth_per_pass()` impl
- The op's `OperationParams::stepover()` impl
- The op's `OperationParams::scallop_height()` impl (G2 added this)
- The bipolar prescription's per-family branch (`bipolar_prescription`)
- The Stage 1 grid's gate at `optimize.rs:736–740`

Six places. Forgetting one fails silently — the optimizer just doesn't
explore that axis for that op. There's no compile-time signal. G2 was
literally a bug of this shape: ScallopConfig didn't expose
`scallop_height` because the trait method didn't exist; nothing
detected it.

### 1.2 Magic numbers without ownership

`optimize.rs` carries 14+ raw `f64` constants:

```
const DOC_HARD_FLOOR_MM: f64 = 0.05;
const DOC_DEDUP_TOLERANCE_MM: f64 = 0.005;
const STEPOVER_HARD_FLOOR_MM: f64 = 0.05;
const STEPOVER_DEDUP_TOLERANCE_MM: f64 = 0.005;
const SCALLOP_HEIGHT_HARD_FLOOR_MM: f64 = 0.01;
const SCALLOP_HEIGHT_DEDUP_TOLERANCE_MM: f64 = 0.001;
pub const BOTTLENECK_FRACTION: f64 = 0.30;
// plus inline literals: 0.7×, 1.3×, 1.4×, ±10% plunge gate, ±1% feed dedup,
// 0.336 (= ln 1.4) approx threshold, 50/200 µm deflection bounds, 1.2 headroom factors,
// 100.0 material-family score bonus, RECOMMENDATION_CYCLE_DELTA_S, ...
```

Each has a comment explaining itself locally. Collectively, the policy
is invisible — you can't read "what the optimizer's tuning posture is"
from one place.

### 1.3 Search anchored to baseline, not LUT

`build_stepover_variants` (and `build_doc_variants`, identical pattern):

```rust
let mult_lo = 0.7 * baseline;
let mult_hi = 1.3 * baseline;  // or 1.4× for 4-variant ops
let (lo, hi) = match lut_row {
    Some(row) => {
        let ae_min = row.ae_min_mm.unwrap_or(mult_lo);
        let ae_max = row.ae_max_mm.unwrap_or(mult_hi);
        (ae_min.max(mult_lo), ae_max.min(mult_hi))   // ← intersection, LUT can only narrow
    }
    None => (mult_lo, mult_hi),
};
```

**Concrete bug this caused:** wanaka TP 4 (Back Rough, 6 mm flat, baseline
stepover 0.84 mm). LUT row `amana-flat-hardwood-adaptive-6000-2f` has
ae_max = 0.95 mm. Multiplier hi = 1.3 × 0.84 = 1.09. Intersection = 0.95.
Search candidates: `[0.59, 0.84, 0.95, default-anchor 2.0]`. The user
*wants* 2.5–3.0 mm stepover (pocket-style on a 3D adaptive op), but
nothing in the search space goes there.

**Two separate problems combine here**, and the refactor needs to address
both:

1. **The intersection-with-multiplier shape** (covered by this refactor).
   Even with a clean LUT envelope, the multiplier intersection caps the
   search at min(LUT, baseline × multiplier) which is rarely the right
   intent.
2. **The matched LUT row's `ae_max=0.95` is itself the cap** — the LUT
   row chosen (`amana-flat-hardwood-adaptive-6000-2f`) is the vendor's
   *2D adaptive HSM* recommendation, with narrow stepover by design.
   Adaptive3d's path geometry is closer to pocket-style clearing; the
   matching is routing it to the wrong LUT family. **This is G17, a
   separate gap closure that depends on this refactor's foundations.**

The refactor (§3.3 below) fixes problem 1 cleanly via the
`hard / preferred / warm_start` bounds split, which also enables
*probing beyond* preferred bounds when policy permits. G17 fixes
problem 2 by improving LUT routing for 3D ops in wood. Both are
required for TP 4 specifically; this design doc owns problem 1 and
states problem 2 explicitly so the wanaka case isn't the misleading
worked example it would be otherwise.

### 1.4 Stage F retargeting uses commanded × RCTF, not sim ground truth

`solve_chipload_retarget` at `optimize.rs:1352` computes:

```
target_chipload_eff = LUT_target          (mid of LUT bounds)
target_chipload_nominal = target_chipload_eff × RCTF(commanded_ae, engaged_d)
target_feed = target_chipload_nominal × rpm × flutes
```

This implicitly asserts `sample_effective_chipload ≈ commanded × RCTF` —
true for steady-state 2D adaptive at constant engagement, **wrong for
adaptive3d at low stepover** where many samples have actual engagement
well below commanded.

**Concrete bug this caused:** TP 4's chipload gate fires at
`Exceeds(BurnRisk)` with sample peak 0.0253 mm/tooth (below LUT min 0.038).
Stage F's response is to *lower* feed from 3150 → 2490 mm/min, which makes
the sim-effective chipload drop further. Wrong direction.

The right answer is sample-driven: `feed_multiplier = LUT_min × headroom /
sample_peak`. For TP 4: `0.038 × 1.2 / 0.0253 = 1.80×` → target feed 5670
mm/min. Linear in commanded feed, no RCTF model needed (the sim already
captured the geometry).

### 1.5 Single `Verdict` enum, all gates share the same shape

```rust
pub enum Verdict {
    Within { peak: f64, confidence: Confidence },
    Exceeds { peak: f64, sample_range: Range<usize>, reason: ExceedsReason, confidence: Confidence },
    Unmodeled { reason: UnmodeledReason },
}
```

`peak`'s units depend on which gate produced the verdict: chipload is
mm/tooth, power is kW, deflection is mm. The compiler can't tell when
they get mixed up. The recent G13 work changed deflection's `peak` from
"unitless L/D ratio" to "mm tip deflection" without renaming or
restyping; every consumer had to read the gate doc-comment to know the
units changed. Fragile.

`Exceeds.reason` is also a flat enum with cross-gate values
(`ChiploadBurnRisk`, `SpindlePowerExceeded`, `LongToolStiffnessUnsafe`).
This blocks per-gate retargeters from being meaningfully typed.

---

## 2. Architecture overview

Six layers, ordered low-to-high. Each layer has one responsibility, owns
its types, and depends only on the layers below.

```
                    ┌─────────────────────────────────────┐
  Layer 6           │  Orchestrator (optimize_toolpath)   │   linear pipeline
                    └─────────────────────────────────────┘
                    ┌─────────────────────────────────────┐
  Layer 5           │  OptimizationStrategy (trait)       │   was: stages
                    └─────────────────────────────────────┘
                    ┌─────────────────────────────────────┐
  Layer 4           │  Retargeter (trait, per gate)       │   sim-driven
                    └─────────────────────────────────────┘
                    ┌─────────────────────────────────────┐
  Layer 3           │  SearchSpace + AxisBounds           │   per-axis bounds
                    └─────────────────────────────────────┘
                    ┌─────────────────────────────────────┐
  Layer 2           │  OptimizationSurface + AxisBinding  │   axis topology
                    └─────────────────────────────────────┘
                    ┌─────────────────────────────────────┐
  Layer 1           │  Per-gate Verdict types             │   typed peaks
                    └─────────────────────────────────────┘
                    ┌─────────────────────────────────────┐
  Layer 0           │  SearchPolicy (named consts)        │   one place for tuning
                    └─────────────────────────────────────┘
```

**Layers 0–2 are infrastructure** — small, pure code, foundational.
**Layers 3–4 are domain logic** — the actual physics/math of search-bound
resolution and retargeting, decoupled from orchestration.
**Layer 5 is policy** — which strategies run, in what order, with what
budget.
**Layer 6 is wiring** — driven by the trait surface, no business logic.

Section 3 expands each layer with concrete signatures.

---

## 3. Layer-by-layer detail

### 3.0 Layer 0 — `SearchPolicy` with provenance

One struct, one file (`optimize/policy.rs`). Every value carries
provenance so future readers know *why* it has that value, not just
that it does. **Reviewer's "magic numbers in named struct are still
magic" critique addressed.**

```rust
/// A tunable policy value with provenance. Reviewers can read the
/// rationale instead of guessing why the constant is what it is, and
/// `source` makes it visible whether the value is physically derived
/// (don't change without analysis), handbook-empirical (defensible
/// but not derived), or a tuning choice (expect to revisit).
pub struct PolicyValue<T> {
    pub value: T,
    pub rationale: &'static str,
    pub source: PolicySource,
}

pub enum PolicySource {
    /// Physical / machine / safety limit. Don't change without
    /// re-deriving the analysis (e.g., 0.05 mm = rubbing floor).
    PhysicalLimit,
    /// Empirical handbook value, citation-backed (e.g., HSM
    /// 6%–16% × D adaptive stepover from machining handbooks).
    Handbook { citation: &'static str },
    /// Tuning choice we made; expect to revise as we collect data.
    /// Includes a hypothesis we'd test before changing.
    TuningChoice { hypothesis: &'static str },
    /// Derived at runtime from machine / tool / material — not a
    /// constant. Captures dependency for debugging.
    Derived { from: &'static str },
}

pub struct SearchPolicy {
    pub axes: AxesPolicy,
    pub feed: FeedPolicy,
    pub retarget: RetargetPolicy,
    pub stage2_survivor_count: PolicyValue<usize>,
    pub recommendation_cycle_delta_s: PolicyValue<f64>,
    pub bottleneck_fraction: PolicyValue<f64>,
}

pub struct AxesPolicy {
    pub doc: AxisPolicy,
    pub stepover: AxisPolicy,
    pub scallop_height: AxisPolicy,
    // future: angular_step, helix_pitch, ramp_angle
}

pub struct AxisPolicy {
    pub baseline_mult_lo: PolicyValue<f64>,
    pub baseline_mult_hi: PolicyValue<f64>,
    pub grid_point_count: PolicyValue<usize>,
    /// Below this, the cut is rubbing not cutting. Hard physical floor.
    pub hard_floor: PolicyValue<f64>,
    pub dedup_tolerance: PolicyValue<f64>,
    /// Whether candidate generation is allowed to probe beyond the
    /// LUT-preferred envelope when sim still verifies. The reviewer's
    /// hard/preferred/warm-start critique surfaces here as a policy
    /// choice the operator can disable for "stay in vendor envelope"
    /// runs vs enable for "explore aggressively, sim verifies" runs.
    pub allow_outside_preferred: PolicyValue<bool>,
}

pub struct FeedPolicy {
    /// Heuristic search range for ops with no LUT row. Derived from
    /// machine envelope when LUT is absent.
    pub baseline_mult_lo: f64,
    pub baseline_mult_hi: f64,
    /// Number of grid points for headroom-style scale-up search.
    pub grid_point_count: usize,
    /// Below this fractional change from baseline, treat candidates as
    /// duplicates (no-op).
    pub dedup_threshold: f64,
    /// Plunge tracks feed when feed changes by more than this fraction.
    pub plunge_tracking_threshold: f64,
}

pub struct RetargetPolicy {
    /// For BurnRisk: aim at LUT_min × this (must be > 1, gives margin).
    pub chipload_low_headroom: f64,    // default 1.20
    /// For BreakageRisk: aim at LUT_max / this.
    pub chipload_high_headroom: f64,   // default 1.20
    /// For power: aim at available × this (must be < 1).
    pub power_headroom: f64,           // default 0.85
    /// For deflection: aim at threshold × this (must be < 1).
    pub deflection_headroom: f64,      // default 0.75
    /// Approximate-band threshold for chipload extrapolation: |ln(scale)| > this
    /// → mark Approximate. ln(1.4) = 0.336 today.
    pub extrapolation_approx_threshold: f64,
}

impl SearchPolicy {
    pub fn default() -> Self {
        Self {
            axes: AxesPolicy {
                doc: AxisPolicy {
                    baseline_mult_lo: PolicyValue {
                        value: 0.7,
                        rationale: "Lower bound for baseline-multiplier sweep when LUT silent. \
                                    0.7× lets us search visibly shallower (test conservative) without \
                                    going below sub-rubbing depths for typical baselines.",
                        source: PolicySource::Handbook {
                            citation: "Engineering Default 9, optimizer redesign 2026-05-08",
                        },
                    },
                    baseline_mult_hi: PolicyValue {
                        value: 1.4,
                        rationale: "Upper bound for baseline-multiplier sweep. 1.4× pairs with \
                                    midpoint at 1.2× — a useful intermediate step when sweeping 4 points.",
                        source: PolicySource::Handbook { citation: "ED 9" },
                    },
                    grid_point_count: PolicyValue {
                        value: 4,
                        rationale: "[lo, baseline, mid, hi] for high-leverage axes (DOC for clearing ops).",
                        source: PolicySource::TuningChoice {
                            hypothesis: "4 points beats 3 enough to justify the extra sim cost",
                        },
                    },
                    hard_floor: PolicyValue {
                        value: 0.05,
                        rationale: "Below 0.05mm DOC, any tool is rubbing rather than cutting in wood. \
                                    Below this is unrecoverable — refuse rather than search there.",
                        source: PolicySource::PhysicalLimit,
                    },
                    dedup_tolerance: PolicyValue {
                        value: 0.005,
                        rationale: "Differences below 5µm are below sim resolution; treat as duplicate.",
                        source: PolicySource::Derived { from: "sim cell size" },
                    },
                    allow_outside_preferred: PolicyValue {
                        value: true,
                        rationale: "Sim verifies every candidate, so probing beyond LUT-preferred is safe \
                                    and unlocks cases where baseline drifted far outside vendor envelope.",
                        source: PolicySource::TuningChoice {
                            hypothesis: "wanaka TP4 needs this; verify with regression",
                        },
                    },
                },
                stepover: AxisPolicy { /* same shape, same provenance pattern */ },
                scallop_height: AxisPolicy { /* hard_floor 0.01, semantics QualityTarget */ },
            },
            // ...
        }
    }
}
```

**`SearchPolicy` is no longer `const fn`** because `PolicyValue<T>` is
construction-time only. That's the cost of provenance; the benefit is
reviewers and future readers can audit every value without grep-and-
guess.

**Migration win:** every `0.7 *`, `1.3 *`, `0.05` literal becomes
`policy.axes.stepover.baseline_mult_lo`, etc. Search-and-replace finds
every site. Documentation lives next to the value. Tests can pass a
custom `SearchPolicy` to exercise edge cases (very wide / very narrow
multipliers) without recompiling.

### 3.1 Layer 1 — Per-gate Verdict types

Replace `Verdict` with three typed verdicts. Each carries enough
information for its retargeter to act without re-reading sim trace or LUT.

```rust
// crates/rs_cam_core/src/tool_load/verdict.rs

/// What statistic a chipload measurement represents. Reviewer flagged
/// that "peak" is ambiguous — current logic uses median effective chip
/// thickness for some checks, sample-peak for others. Make this
/// explicit in the type.
pub enum ChiploadStatistic {
    /// Lowest sample chipload across the toolpath. Drives BurnRisk
    /// detection — the worst (thinnest) sample defines the verdict.
    PeakLow,
    /// Highest sample chipload. Drives BreakageRisk detection.
    PeakHigh,
    /// Median across steady-state samples. Used for retarget targets
    /// to avoid responding to a single transient outlier sample.
    MedianLow,
    MedianHigh,
}

pub struct ChiploadMetric {
    pub observed_mm_per_tooth: f64,
    pub statistic: ChiploadStatistic,
    pub sample_idx: Option<usize>,
    pub bounds: ChipBounds,
}

pub enum ChiploadVerdict {
    Within {
        /// Worst approach to LUT_min observed (PeakLow).
        approach_to_min: ChiploadMetric,
        /// Worst approach to LUT_max observed (PeakHigh).
        approach_to_max: ChiploadMetric,
        confidence: Confidence,
    },
    Exceeds {
        side: ChipSide,
        /// The metric that triggered the verdict. Carries the
        /// statistic kind so retargeters know whether they're acting
        /// on a peak sample or median.
        triggering: ChiploadMetric,
        confidence: Confidence,
    },
    Unmodeled { reason: UnmodeledReason },
}

pub struct ChipBounds {
    pub min_mm_per_tooth: f64,
    pub max_mm_per_tooth: f64,
    pub source: BoundsSource,
}

pub enum ChipSide { Low, High }

pub enum PowerVerdict {
    Within { peak_kw: f64, available_kw: f64, confidence: Confidence },
    Exceeds { peak_kw: f64, available_kw: f64, sample_idx: usize, confidence: Confidence },
    Unmodeled { reason: UnmodeledReason },
}

pub enum DeflectionVerdict {
    Within { peak_mm: f64, threshold_mm: f64, confidence: Confidence },
    Exceeds { peak_mm: f64, threshold_mm: f64, sample_idx: usize, confidence: Confidence },
    Unmodeled { reason: UnmodeledReason },
}

pub struct ToolpathLoadVerdict {
    pub toolpath_id: usize,
    pub chipload: ChiploadVerdict,
    pub power: PowerVerdict,
    pub deflection: DeflectionVerdict,
}
```

**Compile-time wins:**
- A `ChiploadMetric::observed_mm_per_tooth` cannot be assigned to
  a `PowerVerdict::peak_kw` field. Each unit lives on its named field
  in its named per-gate type.
- A retargeter taking `&ChiploadVerdict` literally cannot read deflection
  data. Decoupling enforced by types.
- New gate (vibration, runout, …) = new `XVerdict` type, plus a field on
  `ToolpathLoadVerdict`. Zero edits to existing gates.
- Removing the `ExceedsReason` flat enum cleans up the cross-gate string
  matching in MCP/UI consumers.

**MCP/UI shape change:** the `kind: "exceeds"` JSON payload becomes
per-gate-typed. Consumers branch on the gate field, not on
`reason: "chipload_burn_risk"`. Acceptable because no backward compat is
required.

### 3.2 Layer 2 — `SearchAxis`, `OptimizationSurface`, `AxisBinding`, `AxisContext`

**Reviewer flagged three issues with the original `OptimizableOp` trait:**
(a) trait impls don't force every `OperationConfig` variant to declare
its status — silent drift remains possible; (b) `SpindleRpm` is
`Option<u32>` where None means "use project default", not "axis absent";
(c) `mm` axes aren't interchangeable (scallop_height ≠ stepover ≠ DOC).
The revised design fixes all three.

```rust
// crates/rs_cam_core/src/tool_load/optimize/axes.rs

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum SearchAxis {
    FeedRate,        // mm/min
    SpindleRpm,      // rpm — resolved via AxisContext (see below)
    DepthPerPass,    // mm
    Stepover,        // mm — radial engagement
    ScallopHeight,   // mm — quality target, NOT load-driving
    AngularStep,     // degrees      (RadialFinish; G3a)
    HelixPitch,      // mm           (Adaptive3d helix entry; future)
    RampAngle,       // degrees      (Adaptive3d / RampFinish; future)
}

/// Unit semantics — prevents `mm` axes from being treated as
/// interchangeable. Reviewer's concern.
pub enum AxisUnit { MmPerMin, Rpm, Mm, Deg }

/// What this axis *means* for optimization. Drives strategy choice:
/// LoadDriving axes feed into retargeters; QualityTarget axes are
/// only swept for cycle-time impact, never retargeted to fix gates.
pub enum AxisSemantics {
    /// Directly affects per-sample cutting load. Retargeters can
    /// drive these in response to chipload/power/deflection verdicts.
    LoadDriving {
        affects_chipload: bool,
        affects_force: bool,
    },
    /// Quality target — affects finish/tolerance. Retargeters do not
    /// drive these. The grid sweeps them only for cycle-time impact.
    QualityTarget,
    /// Cycle-time driving with no direct load impact (rare; future).
    CycleTimeDriving,
}

pub struct AxisBinding {
    pub axis: SearchAxis,
    pub field_name: &'static str,    // for debugging / logs
    pub unit: AxisUnit,
    pub semantics: AxisSemantics,
}

impl SearchAxis {
    pub const fn unit(self) -> AxisUnit { /* match self { ... } */ }
    pub const fn label(self) -> &'static str { /* "Feed rate", ... */ }
    pub const fn is_feed_axis(self) -> bool {
        matches!(self, SearchAxis::FeedRate | SearchAxis::SpindleRpm)
    }
    pub const fn semantics(self) -> AxisSemantics { /* per-axis */ }
}
```

```rust
/// Runtime context needed to resolve axis values that depend on
/// inheritance or environment. Critical for SpindleRpm, where
/// op.spindle_rpm == None means "use project default", not "absent".
pub struct AxisContext<'a> {
    pub project_default_rpm: u32,
    pub machine: &'a MachineProfile,
    pub tool: &'a ToolDefinition,
    pub material: &'a Material,
}
```

#### Compile-time coverage via `OperationConfig::optimization_surface`

**The key change:** instead of `OptimizableOp` trait impls (which can
silently fall through), `OperationConfig` exposes a method whose
implementation is a `match` over every variant with **no wildcard arm**.
Adding a new variant is an explicit compile error until the new variant
is classified.

```rust
// crates/rs_cam_core/src/compute/catalog.rs (extension)

pub enum OptimizationSurface<'op> {
    Optimizable(AxisView<'op>),
    /// Op type is known and intentionally not optimizable. Carries the
    /// reason that surfaces in the orchestrator's outcome.
    NotOptimizable { reason: RefuseReason },
}

pub struct AxisView<'op> {
    pub op: &'op OperationConfig,
    pub bindings: &'static [AxisBinding],
    pub op_type: OperationType,
}

impl<'op> AxisView<'op> {
    pub fn axes(&self) -> impl Iterator<Item = SearchAxis> + '_ {
        self.bindings.iter().map(|b| b.axis)
    }

    /// Read an axis value. For `SpindleRpm`, falls back to
    /// `ctx.project_default_rpm` when the op's own value is None.
    pub fn axis_value(&self, axis: SearchAxis, ctx: &AxisContext<'_>) -> Option<f64> {
        // ... per-axis match
    }
}

impl OperationConfig {
    pub fn optimization_surface(&self) -> OptimizationSurface<'_> {
        match self {
            OperationConfig::Adaptive(c) => OptimizationSurface::Optimizable(AxisView {
                op: self,
                bindings: ADAPTIVE_AXES,
                op_type: OperationType::Adaptive,
            }),
            OperationConfig::Adaptive3d(c) => OptimizationSurface::Optimizable(AxisView {
                op: self, bindings: ADAPTIVE3D_AXES, op_type: OperationType::Adaptive3d,
            }),
            OperationConfig::Pocket(_) => OptimizationSurface::Optimizable(/* ... */),
            // ... every Optimizable variant explicitly named ...

            OperationConfig::Drill(_) => OptimizationSurface::NotOptimizable {
                reason: RefuseReason::SteadyStateSamplesNotPresent,
            },
            OperationConfig::AlignmentPinDrill(_) => OptimizationSurface::NotOptimizable {
                reason: RefuseReason::SteadyStateSamplesNotPresent,
            },
            // NO WILDCARD ARM. Adding a new OperationConfig variant
            // forces an explicit decision here at compile time.
        }
    }
}

const ADAPTIVE3D_AXES: &[AxisBinding] = &[
    AxisBinding {
        axis: SearchAxis::FeedRate,
        field_name: "feed_rate",
        unit: AxisUnit::MmPerMin,
        semantics: AxisSemantics::LoadDriving {
            affects_chipload: true, affects_force: true,
        },
    },
    AxisBinding {
        axis: SearchAxis::SpindleRpm, /* same shape; LoadDriving */
    },
    AxisBinding {
        axis: SearchAxis::DepthPerPass, /* LoadDriving */
    },
    AxisBinding {
        axis: SearchAxis::Stepover, /* LoadDriving */
    },
    // Adaptive3d does NOT expose ScallopHeight. ScallopConfig does.
];
```

#### Mutation: `AxisPatch` applies to a borrowed op

Mutation moves out of a trait method into a helper that takes the
patch struct from §3.4 and applies it to a clone of the baseline:

```rust
pub fn apply_patch_to_op(
    op: &OperationConfig,
    patch: &AxisPatch,
    ctx: &AxisContext<'_>,
) -> Result<OperationConfig, AxisError> {
    let mut out = op.clone();
    match (op, patch.axis) {
        (OperationConfig::Adaptive3d(_), SearchAxis::FeedRate) => {
            if let OperationConfig::Adaptive3d(c) = &mut out {
                c.feed_rate = patch.value;
            }
            Ok(out)
        }
        // ... per (op, axis) explicit match
        _ => Err(AxisError::NotPresent { axis: patch.axis, op_type: op.op_type() }),
    }
}
```

#### Test required by the design

```rust
#[test]
fn every_operation_type_has_explicit_optimization_surface() {
    for &op_type in OperationType::ALL {
        let op = OperationConfig::new_default(op_type);
        let surface = op.optimization_surface();
        // Just calling it is enough — the compiler enforces every
        // variant is named explicitly. This test guards against a
        // future "match any with wildcard" regression.
        match surface {
            OptimizationSurface::Optimizable(view) => {
                assert!(!view.bindings.is_empty(),
                    "{op_type:?} declared Optimizable with no axes");
            }
            OptimizationSurface::NotOptimizable { .. } => {}
        }
    }
}
```

**Compile-time + test-time wins:**
- New `OperationConfig` variant → compile error in `optimization_surface`
  until classified.
- New `SearchAxis` variant → compile error in every per-axis match.
- `SpindleRpm` resolved via `AxisContext` so the project default flows
  through, fixing the `Option<u32>` conflation.
- `LoadDriving`/`QualityTarget` semantics prevent retargeters from being
  pointed at scallop_height (which is a quality knob, not a load lever).
- Drill / AlignmentPinDrill explicitly classified as `NotOptimizable`,
  not just "doesn't impl trait" — auditable in one place.

**Eliminates:** `has_doc_knob`, the `OperationParams::stepover()` →
`Some(...)` heuristic, the per-op allowlist branches in
`bipolar_prescription`. Plus the silent-fallthrough risk of the
trait-only design the reviewer flagged.

### 3.3 Layer 3 — `AxisBounds` (hard / preferred / warm_start) and `SearchSpace`

**Reviewer's critique:** treating LUT bounds as a single hard envelope
fails the wanaka TP 4 case (LUT ae_max = 0.95 mm, user wants 2.5–3.0).
Splitting bounds into three intervals — hard physical limits, vendor-
preferred envelope, and baseline-anchored warm-start — gives candidate
generation the freedom to probe outside vendor-preferred when sim still
verifies, while never violating physical limits.

```rust
// crates/rs_cam_core/src/tool_load/optimize/bounds.rs

pub struct Interval { pub lo: f64, pub hi: f64 }

impl Interval {
    pub fn contains(&self, v: f64) -> bool { self.lo <= v && v <= self.hi }
    pub fn clamp(&self, v: f64) -> f64 { v.clamp(self.lo, self.hi) }
    pub fn intersect(&self, other: &Self) -> Option<Self> { /* ... */ }
}

pub struct AxisBounds {
    pub axis: SearchAxis,
    pub baseline: f64,

    /// Hard physical / machine / safety limits. Search MUST stay inside
    /// or be refused. Source: machine envelope, policy hard_floor,
    /// rubbing-prevention floor.
    pub hard: Interval,

    /// Vendor-recommended envelope (LUT row's `ae_*_mm` / `ap_*_mm`).
    /// `None` when the LUT has no bounds for this axis. Search prefers
    /// to stay inside; may probe beyond when policy.allow_outside_preferred
    /// is true. Sim verdict is the ground-truth verifier in either case.
    pub preferred: Option<Interval>,

    /// Where to anchor grid sweeps for routine candidates. Typically
    /// `baseline × [mult_lo, mult_hi]` clamped into `hard`. Expands
    /// toward the nearer boundary of `preferred` when baseline is far
    /// outside vendor envelope.
    pub warm_start: Interval,

    /// Multiple sources may contribute (e.g., LUT for preferred,
    /// machine for hard). Carry all for debugging.
    pub sources: Vec<BoundsSource>,
}

#[derive(Debug, Clone)]
pub enum BoundsSource {
    LutPreferred { row_id: ObservationId, lo: f64, hi: f64 },
    MachineEnvelope { lo: f64, hi: f64 },
    HardFloor { floor: f64, source: &'static str },
    BaselineMultiplier { mult_lo: f64, mult_hi: f64, baseline: f64 },
    HardCeiling { ceiling: f64, source: &'static str },
}

/// Per-axis bound resolver. One function per axis to keep the rules
/// explicit. All take the same context but differ in which LUT field /
/// machine envelope component they read.
pub fn resolve_doc_bounds(
    view: &AxisView<'_>,
    ctx: &AxisContext<'_>,
    lut: Option<&MatchedRow>,
    policy: &SearchPolicy,
) -> AxisBounds { /* uses ap_min_mm / ap_max_mm */ }

pub fn resolve_stepover_bounds(/* same shape */) -> AxisBounds { /* uses ae_min_mm / ae_max_mm */ }
pub fn resolve_feed_bounds(/* same shape */) -> AxisBounds { /* uses chipload bounds × rpm × flutes */ }
// ... one per axis

/// Top-level: build bounds for every axis the op exposes.
pub struct SearchSpace {
    pub bounds: BTreeMap<SearchAxis, AxisBounds>,
}

impl SearchSpace {
    pub fn build(
        view: &AxisView<'_>,
        ctx: &AxisContext<'_>,
        lut: Option<&MatchedRow>,
        policy: &SearchPolicy,
    ) -> Self {
        let bounds = view.bindings.iter()
            .map(|b| (b.axis, resolve_axis(b.axis, view, ctx, lut, policy)))
            .collect();
        Self { bounds }
    }

    pub fn axis(&self, axis: SearchAxis) -> Option<&AxisBounds> {
        self.bounds.get(&axis)
    }

    /// Debug summary, suitable for tracing / UI / refusal explanations.
    pub fn summary(&self) -> String { /* multiline, axis-by-axis */ }
}
```

**Resolution rule for each axis is one function.** The DOC resolver:

```rust
pub fn resolve_doc_bounds(
    view: &AxisView<'_>,
    ctx: &AxisContext<'_>,
    lut: Option<&MatchedRow>,
    policy: &SearchPolicy,
) -> AxisBounds {
    let baseline = view.axis_value(SearchAxis::DepthPerPass, ctx).unwrap_or(0.0);
    let p = &policy.axes.doc;
    let mut sources = Vec::new();

    // Hard interval: physical floor at the bottom, machine sanity at the top.
    let hard_lo = p.hard_floor.value;
    let hard_hi = ctx.tool.cutting_length;  // conservative upper sanity
    sources.push(BoundsSource::HardFloor { floor: hard_lo, source: "rubbing prevention" });

    // Preferred interval: LUT row's calibrated envelope, when present.
    let preferred = lut.and_then(|row| {
        let lo = row.ap_min_mm?;
        let hi = row.ap_max_mm?;
        sources.push(BoundsSource::LutPreferred {
            row_id: row.observation_id.clone(), lo, hi,
        });
        Some(Interval { lo, hi })
    });

    // Warm-start: baseline × multipliers, clamped into hard. Expanded
    // toward preferred when baseline is far outside.
    let raw_lo = (baseline * p.baseline_mult_lo.value).max(hard_lo);
    let raw_hi = (baseline * p.baseline_mult_hi.value).max(baseline);
    let warm_start = match &preferred {
        Some(pref) if !pref.contains(baseline) => {
            // Baseline outside vendor envelope — expand toward preferred so
            // the grid sweeps a useful range, not just baseline-local.
            let lo = raw_lo.min(pref.lo);
            let hi = raw_hi.max(pref.hi);
            Interval { lo: lo.max(hard_lo), hi: hi.min(hard_hi) }
        }
        _ => Interval { lo: raw_lo, hi: raw_hi.min(hard_hi) },
    };
    sources.push(BoundsSource::BaselineMultiplier {
        mult_lo: p.baseline_mult_lo.value, mult_hi: p.baseline_mult_hi.value, baseline,
    });

    AxisBounds {
        axis: SearchAxis::DepthPerPass,
        baseline,
        hard: Interval { lo: hard_lo, hi: hard_hi },
        preferred,
        warm_start,
        sources,
    }
}
```

#### Candidate generation against the three intervals

```rust
impl AxisBounds {
    /// Sweep points in warm_start (the routine search range).
    pub fn warm_start_grid(&self, n_points: usize) -> Vec<f64> { /* ... */ }

    /// Sweep points just outside preferred (probes), capped by hard.
    /// Returns empty if `policy.allow_outside_preferred` is false or
    /// preferred is None.
    pub fn outside_preferred_probes(&self, policy: &SearchPolicy) -> Vec<f64> { /* ... */ }
}
```

This is what fixes the §1.3 wanaka case (problem 1): even with LUT
ae_max=0.95, `warm_start` expands toward preferred, and
`outside_preferred_probes` adds candidates above 0.95 if policy allows.
The sim verifies every candidate, so probing is safe.

(Problem 2 — Adaptive3d routed to wrong LUT family — is G17, not this
refactor's job. But this design *enables* G17: if Adaptive3d gets
routed to the pocket row whose ae_max=2.2, the search picks up that
range automatically.)

**Wins:**
- Bounds policy is one function per axis.
- `BoundsSource` answers "why did Stage 1 only try stepover X?" via
  inspection of `bounds.sources` — every contributor surfaced.
- The §1.3 multiplier-intersection bug is fixed by construction; the
  hard/preferred split additionally allows probing outside vendor
  envelope when policy permits.

**Existing `build_doc_variants` etc. become point-generators inside the
resolved bounds**, ~10 lines each, calling
`bounds.warm_start_grid(n_points)`.

### 3.4 Layer 4 — `Retargeter` trait, multi-axis patches

**Reviewer's critique:** the original single-axis `RetargetSolution`
hides coupled changes. Current Stage F changes feed AND rpm AND plunge
together; a single-axis return type would lose information. Revised
design returns `Vec<AxisPatch>` so coupling is first-class.

```rust
// crates/rs_cam_core/src/tool_load/optimize/retarget/mod.rs

pub trait Retargeter {
    type Verdict;

    /// Axes this retargeter may drive. May return multiple — chipload
    /// retarget freezes RPM but moves feed (+ coupled plunge); power
    /// retarget moves feed alone; deflection retarget moves DOC.
    /// Declaring this in the trait is part of the contract: a
    /// retargeter that names only [FeedRate] commits to NOT touching
    /// RPM, which is what makes the chipload multiplier math linear.
    fn driving_axes(&self) -> &'static [SearchAxis];

    fn target(
        &self,
        verdict: &Self::Verdict,
        space: &SearchSpace,
        view: &AxisView<'_>,
        ctx: &AxisContext<'_>,
    ) -> Option<RetargetSolution>;
}

pub struct RetargetSolution {
    /// One or more patches. Multi-axis when retargeter has coupled
    /// levers (chipload retarget produces a feed patch and a coupled
    /// plunge patch).
    pub patches: Vec<AxisPatch>,
    pub rationale: String,
}

pub struct AxisPatch {
    pub axis: SearchAxis,
    pub value: f64,
    pub clamped: bool,
    pub source: PatchSource,
}

pub enum PatchSource {
    /// The retargeter's primary lever — the axis it consciously moved.
    Primary,
    /// Coupling rule fired in response to a primary patch.
    Coupled { from_axis: SearchAxis, rule: &'static str },
    /// Strategy-driven patch (Stage 1 grid sweep, headroom scale).
    Strategy { strategy: &'static str },
}
```

Concrete implementations live in `optimize/retarget/{chipload,power,
deflection}.rs`, each ~80 lines. The chipload one:

```rust
// optimize/retarget/chipload.rs

pub struct ChiploadFeedRetargeter {
    low_headroom: f64,
    high_headroom: f64,
    plunge_tracking_threshold: f64,
}

impl Retargeter for ChiploadFeedRetargeter {
    type Verdict = ChiploadVerdict;

    fn driving_axes(&self) -> &'static [SearchAxis] {
        &[SearchAxis::FeedRate]   // RPM intentionally NOT here
    }

    fn target(
        &self,
        verdict: &ChiploadVerdict,
        space: &SearchSpace,
        view: &AxisView<'_>,
        ctx: &AxisContext<'_>,
    ) -> Option<RetargetSolution> {
        let (side, triggering) = match verdict {
            ChiploadVerdict::Exceeds { side, triggering, .. } => (side, triggering),
            _ => return None,
        };

        // Sample-driven: read sim-observed metric, not commanded.
        // Linear scaling of feed scales sample chipload linearly —
        // exact because engagement geometry is fixed sample-by-sample.
        let target_chipload = match side {
            ChipSide::Low  => triggering.bounds.min_mm_per_tooth * self.low_headroom,
            ChipSide::High => triggering.bounds.max_mm_per_tooth / self.high_headroom,
        };
        let multiplier = target_chipload / triggering.observed_mm_per_tooth;

        let baseline_feed = view.axis_value(SearchAxis::FeedRate, ctx)?;
        let raw_target = baseline_feed * multiplier;
        let feed_bounds = space.axis(SearchAxis::FeedRate)?;
        let clamped = feed_bounds.hard.clamp(raw_target);
        let was_clamped = (clamped - raw_target).abs() > 1e-6;

        let mut patches = vec![AxisPatch {
            axis: SearchAxis::FeedRate,
            value: clamped,
            clamped: was_clamped,
            source: PatchSource::Primary,
        }];

        // Coupling rule: plunge tracks feed for non-trivial changes.
        // Captured as a Coupled patch so it's visible in the candidate's
        // rationale, not hidden as a side-effect of `apply_patches`.
        if (clamped / baseline_feed - 1.0).abs() > self.plunge_tracking_threshold {
            patches.push(AxisPatch {
                axis: SearchAxis::FeedRate,   // plunge isn't a search axis
                value: clamped,
                clamped: was_clamped,
                source: PatchSource::Coupled {
                    from_axis: SearchAxis::FeedRate,
                    rule: "plunge tracks feed when |Δfeed| > 10%",
                },
            });
        }

        Some(RetargetSolution {
            patches,
            rationale: format!(
                "{:?}: scale feed by {:.2}× to lift sample {:?} from {:.4} to LUT bound × headroom",
                side, multiplier, triggering.statistic, triggering.observed_mm_per_tooth,
            ),
        })
    }
}
```

**No RCTF anywhere.** `triggering.observed_mm_per_tooth` is the actual
sim measurement; linear feed scaling preserves engagement geometry, so
the multiplier is exact.

**RPM frozen by contract.** `driving_axes()` returns `&[FeedRate]` —
the trait declares the contract that this retargeter does not touch
RPM. That's what makes the linear math correct
(`chipload ∝ feed/rpm`; freeze rpm → linear in feed).

**Plunge coupling visible.** Reviewer flagged that the old
`solve_chipload_retarget` hid plunge tracking inside its
`RetargetSolution.target_plunge_mm_min`. New design surfaces it as a
`PatchSource::Coupled` patch with a named rule.

Symmetric retargeters: `PowerFeedRetargeter` (drives FeedRate;
`peak_kw → available × headroom` linearly), `DeflectionDocRetargeter`
(drives DepthPerPass; force scales with DOC × radial_width, so
multiplier ≈ `cube_root(threshold / peak)` — closer to a non-linear
solve, but tractable).

**Wins over original draft:**
- Coupled axes first-class in `RetargetSolution.patches`.
- RPM-freeze contract declared in `driving_axes()`, not implicit.
- Plunge tracking visible in candidate's `PatchSource` list.
- Each retargeter testable in isolation; ~5 unit tests each.
- No commanded × RCTF anywhere → wrong-direction bug gone by
  construction.

### 3.5 Layer 5 — `OptimizationStrategy` trait

```rust
// crates/rs_cam_core/src/tool_load/optimize/strategy/mod.rs

pub trait OptimizationStrategy {
    fn name(&self) -> &'static str;

    /// Generate candidate patches (not full OperationConfigs). Pure:
    /// no sim, no mutation. The orchestrator applies patches to the
    /// baseline and evaluates separately. Reviewer's recommendation:
    /// patches are the searchable atom, not full ops, so each
    /// candidate's provenance is inspectable as a list of
    /// (axis, value, source) triples.
    fn candidates(
        &self,
        baseline: &AxisView<'_>,
        baseline_verdict: &ToolpathLoadVerdict,
        space: &SearchSpace,
        ctx: &AxisContext<'_>,
    ) -> Vec<CandidatePatch>;
}

pub struct CandidatePatch {
    pub patches: Vec<AxisPatch>,
    pub strategy: &'static str,
    pub rationale: String,
}
```

Strategies (each in its own file under `optimize/strategy/`):

| Strategy | Was | Generates |
|---|---|---|
| `BaselineWarmStartStrategy` | implicit | Returns `[]` of patches (baseline is added by the orchestrator). |
| `HeadroomScaleStrategy` | Stage 0 | When all gates are Within: a single feed-scaled patch at the closed-form headroom point. |
| `PerGateRetargetStrategy` | Stage F | For *each* gate that's Exceeds: run that gate's `Retargeter` independently, emit one `CandidatePatch` per retarget. **Reviewer's recommendation: separate candidates first, no auto-composition.** A future `JointRetargetStrategy` can compose if monotonicity tests prove combined retargets behave well. |
| `AxisGridStrategy` | Stage 1 | Grid sweep over each axis the op's `bindings` declares. For `LoadDriving` axes, sweeps `bounds.warm_start_grid()` plus optional `outside_preferred_probes()` per policy. For `QualityTarget` axes (scallop_height), sweeps separately and only for cycle-time impact. |

**Multi-gate composition is deferred.** The original draft contradicted
itself on this (§3.5 said "composes", §9 asked "should it?"). Decision:
each retargeter emits an independent candidate. If both chipload and
power exceed, the optimizer evaluates two candidates (one per gate's
preferred fix), ranks them, and picks. A combined patch (e.g.,
"reduce feed AND reduce DOC") may be worth adding as a separate
`JointRetargetStrategy` later, with monotonicity tests pinning that
the combined response improves on both single-gate responses.

The orchestrator runs strategies in order; each contributes candidate
patches to a flat list; the orchestrator applies patches via
`apply_patches_to_op(baseline, &patch.patches)` and evaluates each.

**Wins:**
- Patches as the searchable atom: every candidate is an inspectable
  list of (axis, value, source) — full provenance.
- Single retarget per gate is straightforward; combined retarget is
  opt-in, not the default.
- Adding a new search strategy is a new file + one line in the
  orchestrator's strategies vector.
- Each strategy unit-testable in isolation.

### 3.6 Layer 6 — Orchestrator

```rust
// crates/rs_cam_core/src/tool_load/optimize/mod.rs

pub fn optimize_toolpath(
    session: &mut ProjectSession,
    trace: &SimulationCutTrace,
    toolpath_index: usize,
    cancel: &AtomicBool,
) -> OptimizeOutcome {
    let policy = SearchPolicy::default();

    // 1. Build context (tool, op, material, machine, baseline_op).
    let ctx = match build_context(session, trace, toolpath_index) {
        Ok(c) => c,
        Err(skip) => return OptimizeOutcome::Skipped { reason: skip },
    };

    // 2. Match LUT row (existing logic; unchanged).
    let lut = find_matched_lut_row(&ctx);

    // 3. Build search space.
    let space = SearchSpace::build(&ctx.baseline_op, lut.as_ref(), &ctx.machine, &policy);

    // 4. Evaluate baseline.
    let mut guard = BaselineRestoreGuard::new(session, toolpath_index);
    let baseline_candidate = match evaluate_candidate(&mut guard, &ctx, &ctx.baseline_op,
                                                     SearchStage::Baseline, cancel) {
        Ok(c) => c,
        Err(e) => return OptimizeOutcome::Skipped { reason: e },
    };

    // 5. Pre-flight: any gate so wrong that no axis change can rescue?
    //    Currently: deflection-Exceeds with no DOC retarget feasible inside
    //    the search space. Other gates always have a feasible retarget.
    if let Some(refusal) = preflight(&baseline_candidate.verdict, &space, &policy) {
        return refusal.into_outcome(baseline_candidate);
    }

    // 6. Run strategies, collect candidates.
    let strategies = vec![
        Box::new(BaselineWarmStartStrategy::default()) as Box<dyn OptimizationStrategy>,
        Box::new(HeadroomScaleStrategy::new(&policy)),
        Box::new(RetargetStrategy::new(&policy)),
        Box::new(AxisGridStrategy::new(&policy)),
    ];
    let mut all_candidates = vec![baseline_candidate.clone()];
    for strategy in &strategies {
        for cand_op in strategy.candidates(&ctx.baseline_op, &baseline_candidate.verdict, &space) {
            if cancel.load(Ordering::SeqCst) {
                return finalize_partial(baseline_candidate, all_candidates);
            }
            if let Ok(cand) = evaluate_candidate(&mut guard, &ctx, &cand_op,
                                                 SearchStage::Coarse, cancel) {
                all_candidates.push(cand);
            }
        }
    }

    // 7. Refine top-K survivors at full resolution (was Stage 2).
    let refined = refine_top_k(&mut guard, &ctx, all_candidates,
                               policy.stage2_survivor_count, cancel);

    // 8. Build outcome.
    drop(guard);
    build_outcome(baseline_candidate, refined, &policy)
}
```

Linear flow, no deep nesting. Each step is one function call; each
function is small. **A new contributor can read this top-to-bottom in
2 minutes and understand the whole pipeline.**

**Pre-flight** stays minimal — the only gate that genuinely refuses is
deflection-locked when no DOC retarget is feasible. Bipolar chipload
moves into the chipload retargeter (returns None when bipolar, the
orchestrator's RetargetStrategy notices and emits a different
candidate-set or annotates the outcome).

---

## 4. File structure

```
crates/rs_cam_core/src/tool_load/
  mod.rs                      # public API, evaluate_toolpath
  verdict.rs                  # per-gate verdict types
  chipload.rs                 # ChiploadVerdict producer (existing logic)
  power.rs                    # PowerVerdict producer (existing logic)
  deflection.rs               # DeflectionVerdict producer (existing logic)

  optimize/
    mod.rs                    # optimize_toolpath orchestrator (~150 LOC)
    policy.rs                 # SearchPolicy + named constants (~120 LOC)
    context.rs                # EvaluationContext, BaselineRestoreGuard (~150 LOC)

    axes.rs                   # SearchAxis, AxisBinding, AxisView, AxisContext, OptimizationSurface (~300 LOC)
    bounds.rs                 # AxisBounds, BoundsSource, resolve_*_bounds (~250 LOC)
    space.rs                  # SearchSpace builder + summary (~80 LOC)

    retarget/
      mod.rs                  # Retargeter trait, RetargetSolution (~50 LOC)
      chipload.rs             # ChiploadFeedRetargeter (~80 LOC)
      power.rs                # PowerFeedRetargeter (~60 LOC)
      deflection.rs           # DeflectionDocRetargeter (~80 LOC)

    strategy/
      mod.rs                  # OptimizationStrategy trait (~30 LOC)
      warm_start.rs           # BaselineWarmStartStrategy (~30 LOC)
      headroom.rs             # HeadroomScaleStrategy (was Stage 0) (~150 LOC)
      retarget.rs             # RetargetStrategy (was Stage F) (~120 LOC)
      grid.rs                 # AxisGridStrategy (was Stage 1) (~250 LOC)

    candidate.rs              # OptimizeCandidate, evaluate_candidate (~250 LOC)
    rank.rs                   # refine_top_k, ranking (~150 LOC)
    delta.rs                  # ParamDelta, GateDeltas, comparators (~150 LOC)
    outcome.rs                # OptimizeOutcome, build_outcome (~200 LOC)

    preflight.rs              # preflight_classify (much smaller post-refactor) (~80 LOC)
    refusal.rs                # RefuseReason, prescription text builders (~150 LOC)
```

**Total LOC** stays roughly the same (~3500 LOC across all files vs ~4000
today). Distribution: each file ≤ 300 LOC, most ≤ 150. **Navigation goes
from "search the 4000-line file" to "go to the file named after what you
want."**

---

## 5. Migration sequence (revised — typed verdicts moved late)

**Reviewer's reorder:** typed verdicts (originally Step 2) had the
biggest blast radius before any of the search architecture existed.
Reordered so search-architecture steps prove out first; verdict-shape
change happens after retargeting works against the existing flat
`Verdict` enum.

Each step lands as a single commit, all tests green, with a wanaka MCP
smoke check including a **candidate-set snapshot** (axes searched,
bounds source, generated values, retarget rationale) — not just final
outcome — so silent search-space shrinkage is caught immediately.

### Step 0 — This doc + reviewer sign-off

**Output:** this doc (revision 2). **Risk:** none.

### Step 1 — Extract `policy.rs` with `PolicyValue<T>` provenance

**Scope:** every magic number → named `PolicyValue<T>` field with
rationale + source. Behavior identical.

**Approach:** define `SearchPolicy`, `AxisPolicy`, `FeedPolicy`,
`RetargetPolicy`, `PolicyValue<T>` in `optimize/policy.rs`. Replace
literals in `optimize.rs` with `policy.X.Y.value` reads. Snapshot
existing wanaka MCP outcome before commit.

**Test gate:** all existing tests pass; wanaka MCP returns *identical*
verdicts and candidate sets (snapshot-asserted).

**Risk:** small. ~half day.

### Step 2 — File split into `optimize/` directory

**Scope:** physical move into the structure of §4. Pure motion, no
logic change. Done early so subsequent steps land in the correct
files.

**Approach:** move types and helpers in batches keeping each
sub-commit compiling. Fix imports.

**Test gate:** all tests pass; clippy clean.

**Risk:** small. ~half day.

### Step 3 — `SearchAxis` + `OptimizationSurface` + `AxisContext` + `AxisBinding`

**Scope:** add the explicit `OperationConfig::optimization_surface()`
method (no wildcard arm — compile-time coverage); add `SearchAxis`,
`AxisBinding`, `AxisContext`. Impl `AxisView` for each
`OperationConfig` variant.

**Approach:**
1. Define types in `optimize/axes.rs`.
2. Add `optimization_surface()` to `OperationConfig` with the explicit
   match. Compile error until every variant is classified.
3. Add `every_operation_type_has_explicit_optimization_surface` test.
4. Adjust `bipolar_prescription`, `has_doc_knob` callers, Stage 1 grid
   to read `view.bindings` instead of the old allowlist.
5. Delete `has_doc_knob`.

**Test gate:** all existing tests pass. New tests: surface coverage
test, per-op axis-list assertions, `AxisContext` correctly resolves
project-default RPM when op's spindle_rpm is None.

**Risk:** small (additive); the deletion is the risky part. ~1 day.

### Step 4 — `AxisBounds` (hard / preferred / warm_start) + bounds resolvers

**Scope:** replace inline bound construction in `build_doc_variants`,
`build_stepover_variants`, `build_scallop_height_variants` with calls
to `resolve_*_bounds`. **First behavior-changing step** — the
LUT-bounds-as-preferred semantics activate here, plus
`outside_preferred_probes` when policy permits.

**Approach:**
1. Define types in `optimize/bounds.rs` and `optimize/space.rs`.
2. Write per-axis resolvers, ~30 LOC each.
3. Reduce existing `build_*_variants` to call
   `bounds.warm_start_grid(n)` + `bounds.outside_preferred_probes()`.
4. Snapshot wanaka candidate sets before/after.

**Test gate:** existing tests pass; some test expectations update to
reflect wider candidate sets. Wanaka snapshot shows wider stepover/DOC
search for TPs whose baseline is far from LUT-preferred. Property
tests: bounds always have `lo <= baseline <= hi` when intended;
`hard` always contains `warm_start`; resolvers never produce
non-finite values.

**Risk:** medium-high (first behavior change). ~1 day.

### Step 5 — Sample-driven `Retargeter` impls

**Scope:** add the `Retargeter` trait and per-gate impls. Retargeters
read the *existing* flat `Verdict` enum at this stage (typed verdicts
come later), reading `peak` with the unit each retargeter knows applies
to its gate. **Second behavior change:** Stage F's wrong-direction bug
fixed here.

**Approach:**
1. Define `Retargeter` trait, `RetargetSolution`, `AxisPatch` in
   `optimize/retarget/mod.rs`.
2. Write `ChiploadFeedRetargeter`, `PowerFeedRetargeter`,
   `DeflectionDocRetargeter` (~80 LOC each).
3. `PerGateRetargetStrategy` that runs each gate's retargeter and emits
   one `CandidatePatch` per gate Exceeds.
4. Wire into orchestrator alongside (not replacing) the old
   `solve_chipload_retarget` for differential testing.
5. Differential-test: same wanaka inputs through old vs new path.
   Once new is at-or-better on every TP, delete old.

**Test gate:** unit tests for each retargeter (5 each: target math,
clamping, rationale, multi-axis patch shape, RPM-frozen contract).
Wanaka MCP shows higher-feed candidates among `attempted` for TP 4.
Property tests: retarget direction is correct for low/high chipload;
patches always have `axis ∈ driving_axes()`.

**Risk:** medium-high (real behavior change). ~1 day.

### Step 6 — `OptimizationStrategy` trait + migrate stages

**Scope:** convert `run_stage_0`, the new retarget logic, and
`run_stage_1_grid` into `OptimizationStrategy` impls returning
`Vec<CandidatePatch>`. Orchestrator becomes the linear pipeline of
§3.6.

**Approach:**
1. One commit per stage. For each: add strategy struct, replace
   orchestrator call, snapshot-assert outcome unchanged.
2. After all three stages migrated, simplify orchestrator.

**Test gate:** outcome variants and candidate sets match prior
commit's snapshot (strict refactor).

**Risk:** medium. ~1 day across three commits.

### Step 7 — Per-gate `Verdict` types (large blast radius)

**Scope:** add `ChiploadVerdict`, `PowerVerdict`, `DeflectionVerdict`
(with `ChiploadMetric` for chipload). Update gate evaluators, MCP
serialization, GUI rendering, retargeters.

**Approach:**
1. Define types in `tool_load/verdict.rs`.
2. Update `chipload::evaluate`, `power::evaluate`,
   `deflection::evaluate` to return typed.
3. Refactor retargeters from generic `&Verdict` reads to typed
   `&ChiploadVerdict` / `&PowerVerdict` / `&DeflectionVerdict`.
4. Update MCP serialization — wire format changes (acceptable).
5. Update GUI rendering of verdict displays.
6. Delete the old `Verdict` enum and `ExceedsReason` flat enum.

**Test gate:** all tests pass with typed reads. MCP returns new shape
(documented in commit). Snapshot tests of outcome candidate sets stay
identical.

**Risk:** medium (large blast radius, many edit sites, but all
mechanical now that the search architecture is settled). ~1 day.

### Step 8 — Delete legacy

**Scope:** delete `has_doc_knob` if anything still references it,
delete dead code from old `solve_chipload_retarget`, delete any
transitional shims.

**Test gate:** all tests pass; clippy clean.

**Risk:** small. ~half day.

---

**Total: ~5 days.** Each commit is independent and reverts cleanly.
After step 8, G16/G17/G18 (the three small fixes from the gap doc)
become trivial — they're just policy values or routing rules in the new
structure.

---

## 6. Test strategy

Three test layers, all in CI:

### 6.1 Unit tests per layer

- **Policy:** struct construction, default values pinned, `PolicyValue`
  rationale strings non-empty.
- **Verdict types:** serialization round-trip, gate-specific metric
  reads, `ChiploadStatistic` discriminates correctly.
- **`OperationConfig::optimization_surface`:** the
  `every_operation_type_has_explicit_optimization_surface` test from
  §3.2; for every Optimizable op, asserts `bindings` non-empty and
  every binding's `axis_value()` resolves cleanly via `AxisContext`
  (especially that `SpindleRpm` falls back to `project_default_rpm`).
- **`BoundsResolver`:** for representative `(op, lut, machine)` tuples,
  assert `hard ⊇ warm_start` always; assert `preferred ⊆ hard`;
  cases — no LUT, LUT without bounds, LUT outside machine, baseline
  outside preferred.
- **`Retargeter`:** for synthetic verdicts, assert target values match
  hand-computed expectations. **Property tests:** retarget direction
  is correct for Burn vs Breakage; multipliers stay within bounds;
  patches always have `axis ∈ driving_axes()`.
- **`OptimizationStrategy`:** for synthetic search spaces, assert
  candidate-patch sets contain expected variants.

### 6.2 Integration tests + candidate-set snapshots

**Reviewer's recommendation:** snapshot candidate sets, not just final
outcomes. Otherwise a future change can silently shrink search space
while still returning `NoSafeImprovement`.

For each fixture (wanaka TPs 4, 10, 12 plus tapered-ball TPs 5, 6, 11
plus a Pocket and an Adaptive3d edge case from synthetic projects),
snapshot:
- Axes searched (from `view.bindings`)
- Per-axis bounds with `BoundsSource` list
- Generated candidate patches (axis, value, source)
- Retarget rationales when retargeters fired
- Final verdict per gate

These snapshots live in `tests/snapshots/` and are version-controlled.
Changes to expected snapshots are explicit per-commit and reviewable.

Param-sweep system (`tests/param_sweep.rs`): existing 54 sweeps
continue to produce stable fingerprints.

### 6.3 MCP smoke check (manual, per behavior-changing migration step)

- After steps 4 and 5 (the two behavior-changing steps), run
  `mcp__rs-cam__get_tool_load_report` and
  `mcp__rs-cam__optimize_toolpath` against wanaka.
- Capture before/after delta in commit message — what changed, why.
- Outputs to inspect: TP-level verdict shifts and `attempted` candidate
  shape (axes searched, target values, retarget rationales).

---

## 7. Risks

### 7.1 Outcome regression on wanaka

**Risk:** a refactor step inadvertently shrinks the search space for
some op type, producing strictly worse candidates than today.

**Mitigation:** every behavior-changing step (steps 4, 5) compares the
candidate set to the prior commit's snapshot. Significant changes are
documented in commit message with rationale. The MCP smoke check
catches operator-visible regressions before they land.

### 7.2 Performance

**Risk:** trait dynamic dispatch on hot paths slows the optimizer.

**Mitigation:** `Box<dyn Retargeter>` and `Box<dyn OptimizationStrategy>`
are called O(strategies × candidates) ≈ O(20) times per
`optimize_toolpath` call. Per-candidate cost is dominated by the
simulation (seconds). Trait dispatch is in the noise. Confirm with a
quick `cargo bench` after step 6 if any concern remains.

### 7.3 Test coverage gaps

**Risk:** existing tests are mostly orchestrator-level; the new
trait-based design makes per-helper unit testing easy, but the migration
depends on snapshot tests of the orchestrator to guarantee no
regression.

**Mitigation:** before step 4 (first behavior change), audit the
orchestrator-level tests for coverage. Add snapshot tests for any
under-tested code path. Tests run in CI; differential failures show up
immediately.

### 7.4 MCP/UI shape change

**Risk:** per-gate verdict types change the JSON wire format. Consumers
break.

**Mitigation:** acknowledged; no PRD constraints. Update MCP tool docs
and any GUI rendering code in step 2 atomically. The change is one-shot,
not gradual.

---

## 8. What stays the same

Worth flagging explicitly:

- **Vendor LUT** JSON format and parsing: untouched. Existing
  `MatchedRow` struct stays; the resolvers just read it more
  carefully.
- **`MillingCutter` trait** and per-cutter geometry: untouched. The
  G13 `tip_deflection_mm` integrator stays where it is.
- **Per-gate evaluation logic** (`chipload::evaluate`,
  `power::evaluate`, `deflection::evaluate`): the *output type*
  changes (per-gate verdict struct), but the per-sample math is
  identical.
- **Simulation/trace infrastructure**: untouched.
- **`ProjectSession`** API: untouched. The optimizer still takes
  `&mut session` and runs project sims.
- **`execute_operation`** dispatch: untouched.
- **MCP tool names** (`optimize_toolpath`, `get_tool_load_report`):
  unchanged. The JSON shape inside changes.
- **`OperationConfig` enum** and its variants: untouched at the type
  level. Each variant gains an arm in `optimization_surface()`; that's it.

The refactor is scoped tightly to the optimizer's search/orchestration
layer and the verdict output types. Everything outside that — about 95 %
of the engine — is unaffected.

---

## 9. Open questions — resolved by reviewer

All six original open questions answered by the independent reviewer.
Carrying forward as decisions:

1. **Right axes?** Keep current 8 (FeedRate, SpindleRpm, DepthPerPass,
   Stepover, ScallopHeight, AngularStep, HelixPitch, RampAngle). Keep
   out: `Tolerance`, `MinCuttingRadius`, `EntryStyle` (quality picks,
   not throughput). Future: V-bit/chamfer depth axes when those gaps
   close; `PathSpacing` may emerge as semantically distinct from raw
   stepover for some ops.
2. **Drill `OptimizableOp` impl?** Neither — Drill returns
   `OptimizationSurface::NotOptimizable { reason: SteadyStateSamplesNotPresent }`
   from its arm of the explicit `match` in `optimization_surface()`.
   Auditable across all ops, no silent fallthrough.
3. **Compose multi-gate retargets?** Separate candidates first.
   `JointRetargetStrategy` deferred until monotonicity tests prove
   combined retargets behave well. Documented in §3.5.
4. **Surface `MachineClamped`?** Now. Cheap and exactly the
   transparency this refactor delivers. `BoundsSource` carries machine
   contributions explicitly.
5. **Property tests with `proptest`?** Yes. Bounds never produce
   non-finite values; `lo ≤ baseline ≤ hi` when intended; `hard`
   always contains `warm_start`; retarget direction correct for
   low/high chipload; candidate dedup never accidentally removes all
   candidates.
6. **`SearchPolicy` in `ProjectSession`?** Not yet, but structure for
   it. `optimize_toolpath` accepts an `OptimizeOptions { policy:
   SearchPolicy }` so per-call override is possible without wedging
   into `ProjectSession`.

---

## 10. Sign-off

Revision 2 incorporates the independent reviewer's required changes:
explicit `OptimizationSurface` for compile-time op coverage, `AxisContext`
for RPM resolution, hard/preferred/warm_start bounds split,
`Vec<AxisPatch>` retarget solutions with declared `driving_axes()`
contract, decision against multi-gate auto-composition, `PolicyValue<T>`
provenance, `ChiploadMetric` with explicit statistic, and the migration
reorder placing typed verdicts after search architecture.

Implementation begins on reviewer sign-off of revision 2. Each step
lands as its own commit on master with green tests, a wanaka MCP
smoke check, and a candidate-set snapshot diff in the commit message.

**Total work: ~6 days** (revised up from 5 in revision 1; the file
split moved earlier and the per-gate verdict step is more involved
once retargeters depend on it).

After this lands, three previously-flagged smaller fixes reduce to
trivial follow-ups:

- **Sample-driven retarget** (was a separate gap) — solved by Step 5.
- **Adaptive3d LUT-family routing** — a single-rule addition to
  `bounds.rs`'s row-matching policy.
- **LUT bounds as outer envelope** — solved by Step 4.

So the architectural refactor closes three gap items as side effects
and pre-empts further conditional drift in the optimizer.
