# G16 — Optimizer architectural refactor

**Status.** Design draft, 2026-05-08. Awaiting independent review before
implementation begins.

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
nothing in the search space goes there. Investigation in conversation
2026-05-08.

The intent of "stay near baseline" is fine for a *warm start*, but the
search space itself should be the LUT envelope when present.

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
  Layer 2           │  OptimizableOp + SearchAxis         │   axis topology
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

### 3.0 Layer 0 — `SearchPolicy`

One struct, one file (`optimize/policy.rs`), every magic number named.

```rust
pub struct SearchPolicy {
    pub axes: AxesPolicy,
    pub feed: FeedPolicy,
    pub retarget: RetargetPolicy,
    pub stage2_survivor_count: usize,
    pub recommendation_cycle_delta_s: f64,
    pub bottleneck_fraction: f64,
}

pub struct AxesPolicy {
    pub doc: AxisPolicy,
    pub stepover: AxisPolicy,
    pub scallop_height: AxisPolicy,
    // future: angular_step, helix_pitch, ramp_angle
}

pub struct AxisPolicy {
    /// When LUT bounds are absent, search baseline × [lo, hi] of these
    /// multipliers. When present, LUT bounds win and these are ignored.
    pub baseline_mult_lo: f64,
    pub baseline_mult_hi: f64,
    /// Number of grid points (including endpoints) when sweeping this axis.
    pub grid_point_count: usize,
    /// Below this, the cut is rubbing not cutting (any tool / material).
    /// Hard physical floor; never violated.
    pub hard_floor: f64,
    /// Sort-and-dedup tolerance to prevent near-duplicate grid points.
    pub dedup_tolerance: f64,
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
    pub const fn default() -> Self {
        Self {
            axes: AxesPolicy {
                doc: AxisPolicy {
                    baseline_mult_lo: 0.7,
                    baseline_mult_hi: 1.4,
                    grid_point_count: 4,
                    hard_floor: 0.05,
                    dedup_tolerance: 0.005,
                },
                stepover: AxisPolicy { /* same shape */ },
                scallop_height: AxisPolicy { /* tighter floor 0.01 */ },
            },
            // ...
        }
    }
}
```

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

pub enum ChiploadVerdict {
    Within {
        peak_low_mm_per_tooth: f64,    // closest approach to LUT_min
        peak_high_mm_per_tooth: f64,   // closest approach to LUT_max
        bounds: ChipBounds,
        confidence: Confidence,
    },
    Exceeds {
        side: ChipSide,                // Low (BurnRisk) | High (BreakageRisk)
        peak_mm_per_tooth: f64,        // worst-direction sample value
        sample_idx: usize,
        bounds: ChipBounds,
        confidence: Confidence,
    },
    Unmodeled { reason: UnmodeledReason },
}

pub struct ChipBounds {
    pub min_mm_per_tooth: f64,
    pub max_mm_per_tooth: f64,
    pub source: BoundsSource,          // LUT row id, or extrapolated-from-id with scale factors
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
- `ChiploadVerdict::Exceeds.peak_mm_per_tooth` cannot be assigned to a
  power kW field. Each unit lives on its named field.
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

### 3.2 Layer 2 — `SearchAxis` and `OptimizableOp` trait

```rust
// crates/rs_cam_core/src/tool_load/optimize/axes.rs

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum SearchAxis {
    FeedRate,        // mm/min
    SpindleRpm,      // rpm
    DepthPerPass,    // mm
    Stepover,        // mm
    ScallopHeight,   // mm
    AngularStep,     // degrees      (RadialFinish; G3a)
    HelixPitch,      // mm           (Adaptive3d helix entry; future)
    RampAngle,       // degrees      (Adaptive3d / RampFinish; future)
}

impl SearchAxis {
    pub const fn unit(self) -> &'static str { /* "mm/min", "rpm", "mm", "deg" */ }
    pub const fn label(self) -> &'static str { /* "Feed rate", ... */ }
    pub const fn is_feed_axis(self) -> bool {
        matches!(self, SearchAxis::FeedRate | SearchAxis::SpindleRpm)
    }
}

/// An operation that the optimizer can search over. Implemented by every
/// op type whose toolpath generation responds to feed/RPM/DOC/stepover
/// changes. Drill-like ops (where chipload is meaningless and DOC is
/// fixed by hole depth) explicitly do NOT impl this — the orchestrator
/// short-circuits to `Skipped { SteadyStateSamplesNotPresent }`.
pub trait OptimizableOp: OperationParams {
    /// Static axis topology — depends only on the op type, not on
    /// instance values. A `&'static [SearchAxis]` lets the optimizer
    /// iterate axes without per-call allocation.
    fn search_axes(&self) -> &'static [SearchAxis];

    /// Generic getter, complementing the typed `feed_rate()`,
    /// `depth_per_pass()`, etc. on `OperationParams`. Returns Some
    /// for axes named in `search_axes()`, None otherwise.
    fn axis_value(&self, axis: SearchAxis) -> Option<f64>;

    /// Generic setter. Returns Err when the axis isn't present on this
    /// op type, or when the value is non-finite / non-positive.
    fn set_axis(&mut self, axis: SearchAxis, value: f64) -> Result<(), AxisError>;
}

#[derive(Debug)]
pub enum AxisError {
    NotPresent { axis: SearchAxis, op_type: OperationType },
    InvalidValue { axis: SearchAxis, value: f64 },
}
```

**Implementation strategy:** add the trait, impl for each
`OperationConfig` variant by delegating to existing typed accessors:

```rust
impl OptimizableOp for AdaptiveConfig {
    fn search_axes(&self) -> &'static [SearchAxis] {
        &[SearchAxis::FeedRate, SearchAxis::SpindleRpm,
          SearchAxis::DepthPerPass, SearchAxis::Stepover]
    }
    fn axis_value(&self, axis: SearchAxis) -> Option<f64> {
        match axis {
            SearchAxis::FeedRate => Some(self.feed_rate()),
            SearchAxis::SpindleRpm => self.spindle_rpm().map(f64::from),
            SearchAxis::DepthPerPass => Some(self.depth_per_pass),
            SearchAxis::Stepover => Some(self.stepover),
            _ => None,
        }
    }
    fn set_axis(&mut self, axis: SearchAxis, value: f64) -> Result<(), AxisError> {
        if !value.is_finite() || value <= 0.0 {
            return Err(AxisError::InvalidValue { axis, value });
        }
        match axis {
            SearchAxis::FeedRate => { self.set_feed_rate(value); Ok(()) }
            SearchAxis::SpindleRpm => { self.set_spindle_rpm(Some(value as u32)); Ok(()) }
            SearchAxis::DepthPerPass => { self.depth_per_pass = value; Ok(()) }
            SearchAxis::Stepover => { self.stepover = value; Ok(()) }
            other => Err(AxisError::NotPresent { axis: other, op_type: OperationType::Adaptive }),
        }
    }
}
```

**Compile-time wins:**
- `match axis { ... }` is exhaustive on the `SearchAxis` enum. Adding a
  new axis to the enum without updating every op's match is a compile
  error.
- `OperationConfig` enum dispatch can `match self` and call into each
  variant's `OptimizableOp` impl uniformly.
- Drill / AlignmentPinDrill don't impl `OptimizableOp`, so a call site
  taking `&dyn OptimizableOp` literally cannot accept them — the
  orchestrator must consciously branch to skip them.

**Eliminates:** `has_doc_knob`, the `OperationParams::stepover()` →
`Some(...)` heuristic for "does this op have stepover?", the per-op
allowlist branches in `bipolar_prescription`.

### 3.3 Layer 3 — `AxisBounds` and `SearchSpace`

```rust
// crates/rs_cam_core/src/tool_load/optimize/bounds.rs

pub struct AxisBounds {
    pub axis: SearchAxis,
    pub baseline: f64,        // current op value, for warm-start
    pub lo: f64,
    pub hi: f64,
    pub source: BoundsSource, // why these bounds, for debugging
}

#[derive(Debug, Clone)]
pub enum BoundsSource {
    /// LUT row had explicit bounds for this axis. Used directly.
    LutEnvelope { row_id: ObservationId, lut_lo: f64, lut_hi: f64 },
    /// Machine envelope was tighter than LUT bounds on at least one side.
    MachineClamped {
        lut_lo: f64, lut_hi: f64,
        machine_lo: f64, machine_hi: f64,
    },
    /// LUT had no bounds for this axis; derived from baseline × policy.
    BaselineMultiplier {
        policy_mult_lo: f64,
        policy_mult_hi: f64,
        applied_lo: f64,
        applied_hi: f64,
    },
    /// Hard physical floor was binding. Documents what the natural
    /// bound would have been.
    HardFloor { floor: f64, would_have_been: f64 },
}

/// Per-axis bound resolver. One function per axis to keep the rules
/// explicit. All take the same context but differ in which LUT field /
/// machine envelope component they read.
pub fn resolve_doc_bounds(
    op: &dyn OptimizableOp,
    lut: Option<&MatchedRow>,
    machine: &MachineProfile,
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
        op: &dyn OptimizableOp,
        lut: Option<&MatchedRow>,
        machine: &MachineProfile,
        policy: &SearchPolicy,
    ) -> Self {
        let bounds = op.search_axes()
            .iter()
            .map(|&a| (a, resolve_axis(a, op, lut, machine, policy)))
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

**Resolution rule for each axis is one function**, easy to read and
test. The DOC resolver is illustrative:

```rust
pub fn resolve_doc_bounds(
    op: &dyn OptimizableOp,
    lut: Option<&MatchedRow>,
    machine: &MachineProfile,
    policy: &SearchPolicy,
) -> AxisBounds {
    let baseline = op.axis_value(SearchAxis::DepthPerPass).unwrap_or(0.0);
    let p = &policy.axes.doc;

    // 1. LUT bounds win when present.
    if let Some(row) = lut
        && let (Some(ap_min), Some(ap_max)) = (row.ap_min_mm, row.ap_max_mm)
    {
        let lo = ap_min.max(p.hard_floor);
        let hi = ap_max;
        return AxisBounds {
            axis: SearchAxis::DepthPerPass, baseline, lo, hi,
            source: BoundsSource::LutEnvelope {
                row_id: row.observation_id.clone(),
                lut_lo: ap_min, lut_hi: ap_max,
            },
        };
    }

    // 2. No LUT bounds → baseline-multiplier envelope, floored.
    let lo = (baseline * p.baseline_mult_lo).max(p.hard_floor);
    let hi = (baseline * p.baseline_mult_hi).max(baseline);
    AxisBounds {
        axis: SearchAxis::DepthPerPass, baseline, lo, hi,
        source: BoundsSource::BaselineMultiplier {
            policy_mult_lo: p.baseline_mult_lo,
            policy_mult_hi: p.baseline_mult_hi,
            applied_lo: lo, applied_hi: hi,
        },
    }
}
```

**Wins:**
- Bounds policy is one function per axis, ~20 lines each. Adding axis
  rules is local.
- `BoundsSource` lets debugging answer "why did Stage 1 only try
  stepover up to 0.95?" by inspecting `bounds.source`.
- The bug from §1.3 is gone *by construction*: when LUT bounds are
  present, they're used directly, never intersected with multiplier.

**Existing `build_doc_variants` etc. become point-generators inside the
resolved bounds**, ~10 lines each.

### 3.4 Layer 4 — `Retargeter` trait

```rust
// crates/rs_cam_core/src/tool_load/optimize/retarget/mod.rs

/// Given a failing verdict and the search space, return an axis-target
/// pair that, applied to baseline, predicts to bring the verdict toward
/// Within. Sample-driven: reads `verdict.peak_*` (from sim) directly,
/// no commanded × RCTF assumption.
pub trait Retargeter {
    /// Verdict type this retargeter consumes. One impl per gate.
    type Verdict;

    /// Which axis does this retargeter drive?
    fn driving_axis(&self) -> SearchAxis;

    /// Compute target solution. None means: the verdict is fine, or no
    /// feasible target in bounds.
    fn target(
        &self,
        verdict: &Self::Verdict,
        space: &SearchSpace,
        baseline: &dyn OptimizableOp,
    ) -> Option<RetargetSolution>;
}

#[derive(Clone, Debug)]
pub struct RetargetSolution {
    pub axis: SearchAxis,
    pub target_value: f64,
    pub multiplier_from_baseline: f64,
    pub clamped: bool,        // was the target clamped to bounds?
    pub rationale: String,    // human-readable, surfaced in MCP report
}
```

Concrete implementations live in `optimize/retarget/{chipload,power,
deflection}.rs`, each ~50 lines. The chipload one:

```rust
// optimize/retarget/chipload.rs

pub struct ChiploadFeedRetargeter {
    low_headroom: f64,    // from policy.retarget.chipload_low_headroom
    high_headroom: f64,
}

impl Retargeter for ChiploadFeedRetargeter {
    type Verdict = ChiploadVerdict;

    fn driving_axis(&self) -> SearchAxis { SearchAxis::FeedRate }

    fn target(
        &self,
        verdict: &ChiploadVerdict,
        space: &SearchSpace,
        baseline: &dyn OptimizableOp,
    ) -> Option<RetargetSolution> {
        let ChiploadVerdict::Exceeds { side, peak_mm_per_tooth, bounds, .. } = verdict
        else { return None };

        let baseline_feed = baseline.axis_value(SearchAxis::FeedRate)?;

        let target_chipload = match side {
            ChipSide::Low  => bounds.min_mm_per_tooth * self.low_headroom,
            ChipSide::High => bounds.max_mm_per_tooth / self.high_headroom,
        };
        let multiplier = target_chipload / peak_mm_per_tooth;
        let raw_target = baseline_feed * multiplier;

        let feed_bounds = space.axis(SearchAxis::FeedRate)?;
        let clamped_target = raw_target.clamp(feed_bounds.lo, feed_bounds.hi);
        let was_clamped = (clamped_target - raw_target).abs() > 1e-6;

        Some(RetargetSolution {
            axis: SearchAxis::FeedRate,
            target_value: clamped_target,
            multiplier_from_baseline: clamped_target / baseline_feed,
            clamped: was_clamped,
            rationale: format!(
                "{:?}: scale feed by {:.2}× to lift sample chipload from {:.4} to LUT bound × {:.2}",
                side, multiplier, peak_mm_per_tooth,
                if matches!(side, ChipSide::Low) { self.low_headroom } else { 1.0 / self.high_headroom },
            ),
        })
    }
}
```

**No RCTF.** The verdict already encodes the actual sim peak; targeting
LUT bound × headroom by linear-scaling feed *is the correct math*
because feed scales sample chipload linearly (engagement geometry is
fixed by the toolpath, sample-by-sample, regardless of feed).

**Tests** for this single function are tiny and easy:

```rust
#[test]
fn burnrisk_doubles_feed_when_peak_is_half_lut_min() {
    let space = test_space_with_feed_bounds(1000.0, 6000.0);
    let verdict = ChiploadVerdict::Exceeds {
        side: ChipSide::Low,
        peak_mm_per_tooth: 0.025,
        bounds: ChipBounds { min: 0.05, max: 0.10, /* ... */ },
        sample_idx: 0,
        confidence: Confidence::Validated,
    };
    let baseline = test_op_with_feed(2000.0);
    let solution = ChiploadFeedRetargeter { low_headroom: 1.0, .. }.target(&verdict, &space, &baseline).unwrap();
    assert_eq!(solution.target_value, 4000.0);  // 2× feed → 2× sample chipload → 0.05 = LUT min
}
```

Symmetric retargeters exist for power (reduce feed when `peak_kw >
available × headroom`) and deflection (reduce DOC when `peak_mm >
threshold × headroom`).

**Wins:**
- Each retargeter is testable in isolation, ~5 unit tests each.
- New gate added later → one new retargeter, no edits to existing ones.
- The wrong-direction Stage F bug from §1.4 is gone *by construction* —
  there's no commanded × RCTF anywhere in the new code path.

### 3.5 Layer 5 — `OptimizationStrategy` trait

```rust
// crates/rs_cam_core/src/tool_load/optimize/strategy/mod.rs

pub trait OptimizationStrategy {
    fn name(&self) -> &'static str;

    /// Generate candidate ops. Pure: no sim, no mutation. Caller drives
    /// evaluation separately.
    fn candidates(
        &self,
        baseline: &dyn OptimizableOp,
        baseline_verdict: &ToolpathLoadVerdict,
        space: &SearchSpace,
    ) -> Vec<OperationConfig>;
}
```

Strategies (each in its own file under `optimize/strategy/`):

| Strategy | Was | Generates |
|---|---|---|
| `HeadroomScaleStrategy` | Stage 0 | When all gates are Within: a single feed-scaled candidate at the closed-form headroom point. |
| `RetargetStrategy` | Stage F | For each gate that's Exceeds: run that gate's `Retargeter`, produce a candidate at the target. Composes multiple retargets when multiple gates exceed (joint feed reduction for power + DOC reduction for deflection, etc.). |
| `AxisGridStrategy` | Stage 1 | Grid sweep over each axis the op exposes. Anchored at baseline, points distributed inside `AxisBounds`. Joint sweeps for ops with multiple axes (DOC × stepover × scallop_height for ScallopConfig). |
| `BaselineWarmStartStrategy` | implicit | Returns `[baseline.clone()]`. Always run first so the baseline is in the evaluated set. |

The orchestrator runs strategies in order; each contributes candidates
to a flat list; evaluation and ranking happen at the end uniformly.

**Wins:**
- Stage F + Stage 1 are now composable. If both want to act, both fire;
  the orchestrator doesn't need a special "what if both gates failed?"
  branch (currently absent → bug source).
- Adding a new search strategy (e.g., gradient-based local search around
  the best Stage 2 survivor) is a new file + one line in the
  orchestrator's strategies vector.
- Each strategy is unit-testable: feed it a synthetic `SearchSpace` and
  verdict, assert the candidate set.

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

    axes.rs                   # SearchAxis, OptimizableOp trait, impls (~300 LOC)
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

## 5. Migration sequence

Each step lands as a single commit, all tests green, with a wanaka MCP
smoke check. If a step fails, prior commits stand as strict improvement.

### Step 0 — Doc this design, get review (this step)

**Output:** this doc, plus operator approval after independent review.
**Risk:** none.

### Step 1 — Extract `policy.rs`

**Scope:** every `const` and inline literal magic number → named const
in a new `tool_load/optimize/policy.rs`. Behavior identical.

**Approach:** create `policy.rs` with the structs from §3.0; replace
literals in `optimize.rs` with `policy.axes.X.Y` references; pass
`SearchPolicy::default()` from the orchestrator entry point through to
the existing builder functions.

**Test gate:** all existing tests pass; wanaka MCP returns identical
verdicts and candidate sets.

**Risk:** small. ~half day.

### Step 2 — Per-gate `Verdict` types

**Scope:** add `ChiploadVerdict`, `PowerVerdict`, `DeflectionVerdict`.
Each gate's `evaluate` returns the typed verdict. `ToolpathLoadVerdict`
holds typed fields.

**Approach:**
1. Define types in `verdict.rs`.
2. Update `chipload::evaluate`, `power::evaluate`,
   `deflection::evaluate` to return typed.
3. Update every consumer (optimizer, MCP, GUI) to read typed fields.
   This is the biggest blast-radius step. Search every `Verdict::Within`
   pattern, replace with the typed equivalent.
4. Remove the old `Verdict` enum and `ExceedsReason` flat enum.

**Test gate:** all tests pass; MCP `get_tool_load_report` and
`optimize_toolpath` return new shape (acceptable, no PRD).

**Risk:** medium. Many edit sites. ~1 day.

### Step 3 — `OptimizableOp` trait + `SearchAxis`

**Scope:** add the trait and enum; impl for every `OperationConfig`
variant; adjust the orchestrator to take `&dyn OptimizableOp`.

**Approach:**
1. Define types in `optimize/axes.rs`.
2. Hand-write impls for each of the ~17 `OperationConfig` variants.
   Optional: a small derive macro to generate them, but probably faster
   to just write them.
3. Adjust `bipolar_prescription`, the Stage 1 grid, and the variant
   builders to read `op.search_axes()` instead of `has_doc_knob()`.
4. Delete `has_doc_knob` and the per-op type-checks it was used for.

**Test gate:** all tests pass. New tests: for every op type, assert
`search_axes()` returns the expected axes; assert `axis_value() != None`
exactly for those axes.

**Risk:** small (additive); the deletion at the end is the risky part.
~1 day.

### Step 4 — `AxisBounds` + `SearchSpace` + bounds resolvers

**Scope:** replace inline bound construction in `build_doc_variants`,
`build_stepover_variants`, `build_scallop_height_variants` with calls to
the resolvers.

**Approach:**
1. Define types in `optimize/bounds.rs` and `optimize/space.rs`.
2. Write `resolve_doc_bounds`, `resolve_stepover_bounds`,
   `resolve_scallop_bounds`, `resolve_feed_bounds` —
   one function each, ~30 LOC.
3. Reduce existing `build_*_variants` to point-generators inside
   resolved bounds (~10 LOC each).
4. **Behavioral change:** the LUT-as-outer-envelope semantics now apply.
   This is the first behavior-changing step.

**Test gate:** existing tests pass *with possibly different candidate
sets* — update tests to reflect new bounds. Wanaka MCP smoke check
should now show wider stepover/DOC search candidates for TPs whose
baseline is far from the LUT recommendation. Capture before/after in
the commit message.

**Risk:** medium-high (first behavior change). ~1 day.

### Step 5 — Sample-driven `Retargeter` impls

**Scope:** add the trait and concrete impls for each gate. Replace the
old `solve_chipload_retarget` with `RetargetStrategy` driven by the new
retargeters.

**Approach:**
1. Define `Retargeter` trait in `optimize/retarget/mod.rs`.
2. Write `ChiploadFeedRetargeter`, `PowerFeedRetargeter`,
   `DeflectionDocRetargeter` (~50 LOC each).
3. Write `RetargetStrategy` that runs the retargeters and emits
   candidates.
4. **Behavioral change:** Stage F's wrong-direction bug is gone. Wanaka
   TPs with chipload-Exceeds(BurnRisk) should now produce candidates
   with feed *raised*, not lowered.

**Test gate:** new unit tests for each retargeter (target value math,
clamping, rationale string). Wanaka MCP `optimize_toolpath` against
TP 4 should show a higher-feed candidate among `attempted`. Capture the
new `attempted` list in the commit.

**Risk:** medium-high (real behavior change with operator-visible
effects). ~1 day.

### Step 6 — `OptimizationStrategy` trait + migrate stages

**Scope:** convert `run_stage_0`, `run_stage_f_retarget`,
`run_stage_1_grid` into `OptimizationStrategy` impls. The orchestrator
becomes the linear pipeline from §3.6.

**Approach:**
1. One commit per stage. For each:
   - Add the strategy struct with `candidates(...)` method.
   - Replace the orchestrator's stage call with a strategy invocation.
   - Run wanaka regression — outcome should match prior commit.
2. After all three stages migrated, simplify the orchestrator to the
   linear loop in §3.6.

**Test gate:** outcome variants and candidate sets match prior commit
(strict refactor with no behavior change).

**Risk:** medium (touch the hot orchestration path). ~1 day spread
across three commits.

### Step 7 — Split into `optimize/` directory

**Scope:** physically move types into the file structure of §4. Pure
code motion, no logic change.

**Approach:** straightforward file moves, fix imports.

**Test gate:** all tests pass; clippy clean.

**Risk:** small. ~half day.

### Step 8 — Delete legacy

**Scope:** delete the transitional `Verdict` enum if any compatibility
shim remained, delete `has_doc_knob`, delete `solve_chipload_retarget`'s
dead code, etc.

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

- **Policy:** struct construction, default values pinned.
- **Verdict types:** serialization round-trip, gate-specific peak field
  reads.
- **`OptimizableOp`:** for every `OperationConfig` variant, assert
  `search_axes()` matches the typed accessor surface; assert
  `axis_value()` returns Some exactly for those axes; assert
  `set_axis()` round-trips.
- **`BoundsResolver`:** for representative (op, lut, machine) tuples,
  assert resolved bounds are physically sensible. Particular cases:
  no LUT row; LUT row without bounds; LUT bounds tighter than machine;
  hard-floor binding.
- **`Retargeter`:** for synthetic verdicts (representative Burn /
  Breakage / power / deflection cases), assert target values match
  hand-computed expectations. ~5 tests per retargeter.
- **`OptimizationStrategy`:** for synthetic search spaces, assert
  candidate sets contain expected variants.

### 6.2 Integration tests

- Wanaka project as the gold fixture. Run `optimize_toolpath` against
  every TP; assert outcome variant + cycle delta + verdict shape.
- Param-sweep system (`tests/param_sweep.rs`): existing 54 sweeps
  continue to produce stable fingerprints.

### 6.3 MCP smoke check (manual, per migration step)

- After each step that touches behavior, run
  `mcp__rs-cam__get_tool_load_report` and
  `mcp__rs-cam__optimize_toolpath` against wanaka.
- Capture before/after delta in commit message — what changed, why.
- The two operator-relevant outputs are: TP-level verdict shifts (e.g.,
  Within → Approximate; Exceeds → Within) and `attempted` candidate
  shape (which axes got searched, what targets were proposed).

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
  level. Each variant gains an `OptimizableOp` impl; that's it.

The refactor is scoped tightly to the optimizer's search/orchestration
layer and the verdict output types. Everything outside that — about 95 %
of the engine — is unaffected.

---

## 9. Open questions for review

1. **Do I have the right axes?** `SearchAxis` enum currently lists 8
   variants; should `MinCuttingRadius`, `Tolerance`, `EntryStyle` be in
   there? My read: those are not optimization knobs (operator picks them
   based on quality goals, not throughput), so they stay out. Confirm.
2. **Should `Drill` impl `OptimizableOp` with empty axes, or just not
   impl it?** I'd argue the latter — the type-system signal "drill is
   not optimizable" is more honest.
3. **Should `RetargetStrategy` compose multiple retargets in one
   candidate** (e.g., raise feed *and* reduce DOC if both gates fail),
   or emit them as separate candidates? My read: separate candidates,
   so the optimizer can rank them independently. The combined case can
   come later as a `JointRetargetStrategy` if needed.
4. **`BoundsSource::MachineClamped` shows up when LUT bounds are wider
   than machine envelope**. Currently we silently clamp; the new
   structure can surface "you're hitting machine ceiling" in the
   outcome's debug summary. Worth doing now or later?
5. **Property tests** (with `proptest`) for the bounds resolver and
   retargeters? Cheap insurance against edge cases. Lean yes.
6. **Should `SearchPolicy` be carried in `ProjectSession`** so per-
   project tuning is possible? Currently default-only is fine; flagging
   for future.

---

## 10. Sign-off

Once an independent reviewer signs off on the design, implementation
proceeds in the migration sequence above. Each step lands as its own
commit on master with green tests and a wanaka MCP smoke check. The
total work is estimated at ~5 days of focused effort.

After this lands, the three small fixes flagged earlier (G16 sample-
driven retarget, G17 Adaptive3d LUT routing, G18 LUT bounds as outer
envelope) reduce to:

- **G16** is solved by step 5 (Retargeter trait).
- **G17** becomes a one-line policy choice in `bounds.rs`'s LUT-family
  routing function.
- **G18** is solved by step 4 (BoundsResolver).

So the architectural refactor closes three gap items and pre-empts the
next several gap closures from accumulating new conditional drift.
