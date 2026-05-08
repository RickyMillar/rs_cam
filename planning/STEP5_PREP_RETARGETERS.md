# Step 5 prep — sample-driven retargeter implementations

Pre-work for G16 Step 5. Specifies the three retargeter
implementations (chipload, power, deflection) so that parallel agents
can implement each in isolation after Step 3+4 land. Each retargeter
is ~80 LOC + 5 unit tests.

Authored 2026-05-08 in parallel with reviewer's Step 1.

## The shared trait (from §3.4 of the design doc)

```rust
pub trait Retargeter {
    type Verdict;

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
    Primary,
    Coupled { from_axis: SearchAxis, rule: &'static str },
    Strategy { strategy: &'static str },
}
```

**Note on `Verdict`:** Step 5 reads the *existing* flat
`Verdict` enum (per the migration order). Each retargeter knows its
gate's units and reads `Verdict::Exceeds.peak` accordingly. Step 7
later swaps to typed verdicts; that change is local to each
retargeter file (~5 LOC each).

For Step 5, the retargeters take `&Verdict` and check for
`Verdict::Exceeds { reason: ExceedsReason::ChiploadBurnRisk | ChiploadBreakageRisk, .. }`
etc. After Step 7, they take `&ChiploadVerdict` etc.

---

## 1. `ChiploadFeedRetargeter`

**File:** `crates/rs_cam_core/src/tool_load/optimize/retarget/chipload.rs`

**Math:** sample-driven feed multiplier. For BurnRisk (sample chipload
< LUT min): `multiplier = LUT_min × headroom / observed_peak`. For
BreakageRisk (sample chipload > LUT max): `multiplier = LUT_max /
(headroom × observed_peak)`. Linear in feed because at fixed RPM and
toolpath geometry, sample chipload scales linearly with feed.

**Driving axes:** `&[SearchAxis::FeedRate]`. RPM is intentionally
frozen so the multiplier math stays linear.

**Coupling:** plunge tracks feed when `|Δfeed/baseline| > 10%`. Emitted
as a `PatchSource::Coupled` patch.

**Refusal:** returns None when verdict is not `ExceedsReason::ChiploadBurnRisk`
or `ExceedsReason::ChiploadBreakageRisk`. Returns None when the
matched LUT row has no chipload bounds (caller checks; we get them via
`SearchSpace`'s `BoundsSource`).

### Implementation skeleton

```rust
use crate::tool_load::verdict::{ExceedsReason, Verdict};
use crate::tool_load::optimize::{
    axes::{AxisContext, AxisView, SearchAxis},
    bounds::SearchSpace,
    retarget::{AxisPatch, PatchSource, Retargeter, RetargetSolution},
};

pub struct ChiploadFeedRetargeter {
    pub low_headroom: f64,    // policy.retarget.chipload_low_headroom (>1.0)
    pub high_headroom: f64,   // policy.retarget.chipload_high_headroom (>1.0)
    pub plunge_tracking_threshold: f64,  // policy.feed.plunge_tracking_threshold
}

impl Retargeter for ChiploadFeedRetargeter {
    type Verdict = Verdict;  // Step 5; becomes ChiploadVerdict at Step 7

    fn driving_axes(&self) -> &'static [SearchAxis] {
        &[SearchAxis::FeedRate]
    }

    fn target(
        &self,
        verdict: &Verdict,
        space: &SearchSpace,
        view: &AxisView<'_>,
        ctx: &AxisContext<'_>,
    ) -> Option<RetargetSolution> {
        // 1. Match the chipload-Exceeds variants only.
        let (peak, side) = match verdict {
            Verdict::Exceeds { peak, reason: ExceedsReason::ChiploadBurnRisk, .. }
                => (*peak, Side::Burn),
            Verdict::Exceeds { peak, reason: ExceedsReason::ChiploadBreakageRisk, .. }
                => (*peak, Side::Breakage),
            _ => return None,
        };

        // 2. LUT bounds. Read from the SearchSpace's feed bounds source —
        //    when feed bounds were derived from LUT chipload × rpm × flutes,
        //    we can reconstruct LUT_min/max from the bounds source.
        //    Alternative: pass LUT bounds explicitly via `SearchSpace`.
        //    For Step 5, accept the helper:
        let (lut_min, lut_max) = space.chipload_bounds()?;

        // 3. Compute target chipload with headroom margin.
        let target_chipload = match side {
            Side::Burn     => lut_min * self.low_headroom,
            Side::Breakage => lut_max / self.high_headroom,
        };
        let multiplier = target_chipload / peak;

        // 4. Apply to baseline feed; clamp to feed bounds.
        let baseline_feed = view.axis_value(SearchAxis::FeedRate, ctx)?;
        let raw_target = baseline_feed * multiplier;
        let feed_bounds = space.axis(SearchAxis::FeedRate)?;
        let clamped = feed_bounds.hard.clamp(raw_target);
        let was_clamped = (clamped - raw_target).abs() > 1e-6;

        // 5. Build patches. Primary feed patch + coupled plunge if
        //    feed change is significant.
        let mut patches = vec![AxisPatch {
            axis: SearchAxis::FeedRate,
            value: clamped,
            clamped: was_clamped,
            source: PatchSource::Primary,
        }];

        if (clamped / baseline_feed - 1.0).abs() > self.plunge_tracking_threshold {
            // Plunge isn't a search axis (driven by safety cap, not
            // optimization). Captured as a Coupled patch so the candidate's
            // rationale lists it explicitly. The actual plunge value is
            // computed in apply_patches_to_op via the coupling rule.
            patches.push(AxisPatch {
                axis: SearchAxis::FeedRate,  // marker; coupling rule keys on this
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
                "{:?}: scale feed by {:.2}× to lift sample peak from {:.4} to LUT × headroom",
                side, multiplier, peak,
            ),
        })
    }
}

enum Side { Burn, Breakage }
```

### Required tests (5)

```rust
#[test]
fn burnrisk_doubles_feed_when_peak_is_half_lut_min() {
    // peak = 0.025, LUT_min = 0.05, headroom = 1.0 → target = 0.05, mult = 2.0
    let space = test_space()
        .with_chipload_bounds(0.05, 0.10)
        .with_feed_bounds(1000.0, 6000.0);
    let view = test_view().with_feed(2000.0);
    let verdict = Verdict::Exceeds {
        peak: 0.025,
        sample_range: 0..1,
        reason: ExceedsReason::ChiploadBurnRisk,
        confidence: Confidence::Validated,
    };
    let r = ChiploadFeedRetargeter {
        low_headroom: 1.0, high_headroom: 1.0, plunge_tracking_threshold: 0.10,
    };
    let solution = r.target(&verdict, &space, &view, &test_ctx()).unwrap();
    let primary = solution.patches.iter()
        .find(|p| matches!(p.source, PatchSource::Primary)).unwrap();
    assert!((primary.value - 4000.0).abs() < 1e-6);
}

#[test]
fn breakagerisk_halves_feed_when_peak_is_double_lut_max() {
    // peak = 0.20, LUT_max = 0.10, headroom = 1.0 → target = 0.10, mult = 0.5
    // ... assert primary.value == baseline_feed × 0.5
}

#[test]
fn coupled_plunge_patch_emitted_when_feed_change_exceeds_10pct() {
    // 50% feed reduction → plunge coupling fires
    // ... assert patches.len() == 2 and one is Coupled
}

#[test]
fn no_coupled_plunge_patch_for_small_feed_change() {
    // 5% feed reduction → no coupled patch
    // ... assert patches.len() == 1
}

#[test]
fn returns_none_for_non_chipload_verdict() {
    let verdict = Verdict::Exceeds {
        peak: 0.5,
        sample_range: 0..1,
        reason: ExceedsReason::SpindlePowerExceeded,
        confidence: Confidence::Validated,
    };
    let solution = ChiploadFeedRetargeter::default().target(/* ... */);
    assert!(solution.is_none());
}

#[test]
fn target_is_clamped_to_feed_bounds() {
    // Multiplier wants 5× feed but feed_bounds.hard.hi is only 2× baseline
    // → primary.value == feed_bounds.hard.hi, primary.clamped == true
}
```

### Wanaka TP 4 expected output

For TP 4 baseline (feed=3150, RPM=18000, peak=0.0253) with the
matched LUT row's chipload [0.038, 0.07] and headroom 1.20:

```
target_chipload = 0.038 × 1.20 = 0.0456
multiplier = 0.0456 / 0.0253 = 1.802
target_feed = 3150 × 1.802 = 5677 mm/min
```

If feed_bounds.hard.hi is high enough (machine envelope allows it),
the target lands at ~5677. Plunge tracks (>10% change). This is the
**right direction** for BurnRisk — opposite to the current Stage F
behavior (which lowered feed to 2490).

---

## 2. `PowerFeedRetargeter`

**File:** `crates/rs_cam_core/src/tool_load/optimize/retarget/power.rs`

**Math:** linear feed reduction. For
`peak_kw > available_kw × safety_factor`: `multiplier = available_kw ×
headroom_factor / peak_kw` where `headroom_factor < 1.0` adds margin.

**Driving axes:** `&[SearchAxis::FeedRate]`. (Reducing RPM also reduces
chipload, which we don't want here — keep RPM fixed.)

**Coupling:** plunge tracks feed (same rule as chipload retarget).

**Refusal:** returns None for non-`SpindlePowerExceeded` verdicts.

### Implementation skeleton

```rust
pub struct PowerFeedRetargeter {
    pub headroom: f64,  // policy.retarget.power_headroom (<1.0, e.g., 0.85)
    pub plunge_tracking_threshold: f64,
}

impl Retargeter for PowerFeedRetargeter {
    type Verdict = Verdict;

    fn driving_axes(&self) -> &'static [SearchAxis] {
        &[SearchAxis::FeedRate]
    }

    fn target(
        &self,
        verdict: &Verdict,
        space: &SearchSpace,
        view: &AxisView<'_>,
        ctx: &AxisContext<'_>,
    ) -> Option<RetargetSolution> {
        let peak_kw = match verdict {
            Verdict::Exceeds { peak, reason: ExceedsReason::SpindlePowerExceeded, .. }
                => *peak,
            _ => return None,
        };

        // Available power at current RPM × machine safety factor — pulled
        // via the SearchSpace's power context.
        let available_kw = space.power_available_kw()?;
        let target_kw = available_kw * self.headroom;
        let multiplier = target_kw / peak_kw;

        let baseline_feed = view.axis_value(SearchAxis::FeedRate, ctx)?;
        let raw_target = baseline_feed * multiplier;
        let feed_bounds = space.axis(SearchAxis::FeedRate)?;
        let clamped = feed_bounds.hard.clamp(raw_target);
        let was_clamped = (clamped - raw_target).abs() > 1e-6;

        let mut patches = vec![AxisPatch {
            axis: SearchAxis::FeedRate, value: clamped,
            clamped: was_clamped, source: PatchSource::Primary,
        }];
        if (clamped / baseline_feed - 1.0).abs() > self.plunge_tracking_threshold {
            patches.push(AxisPatch {
                axis: SearchAxis::FeedRate, value: clamped, clamped: was_clamped,
                source: PatchSource::Coupled {
                    from_axis: SearchAxis::FeedRate,
                    rule: "plunge tracks feed when |Δfeed| > 10%",
                },
            });
        }
        Some(RetargetSolution {
            patches,
            rationale: format!(
                "scale feed by {:.2}× to bring power peak {:.3}kW under available × headroom = {:.3}kW",
                multiplier, peak_kw, target_kw,
            ),
        })
    }
}
```

### Required tests (5)

```rust
#[test]
fn power_exceeds_halves_feed_when_peak_is_double_available() {
    // peak = 1.0kW, available = 0.5kW, headroom = 1.0 → mult = 0.5
}

#[test]
fn power_exceeds_clamps_to_min_feed_when_overshoot_extreme() {
    // peak = 100kW, available = 0.5 → mult = 0.005 → clamped to feed_bounds.hard.lo
    // ... assert clamped == true
}

#[test]
fn coupled_plunge_emitted_for_significant_feed_change() {
    // 30% reduction → coupled patch present
}

#[test]
fn returns_none_for_non_power_verdict() {
    // ChiploadBurnRisk → None
}

#[test]
fn returns_none_when_within_verdict() {
    // Verdict::Within { peak: 0.3 } → None
}
```

---

## 3. `DeflectionDocRetargeter`

**File:** `crates/rs_cam_core/src/tool_load/optimize/retarget/deflection.rs`

**Math:** non-linear in DOC. Force scales as `Kc × DOC × radial_width`,
deflection scales as `F × L³ / (3·E·I)`. So at fixed stepover and feed,
`δ ∝ DOC`. Linear approximation: `multiplier = threshold × headroom /
peak_delta_mm`. For better accuracy in some regimes the relationship
isn't strictly linear in DOC because radial_width also changes with
DOC for tapered tools — but the linear approximation is adequate for
seeding a candidate that the sim then verifies.

**Driving axes:** `&[SearchAxis::DepthPerPass]`. Reducing DOC reduces
force; deflection follows.

**Coupling:** none. Plunge unchanged when DOC changes alone.

**Refusal:** None for non-`LongToolStiffnessUnsafe` verdicts. None when
the op doesn't expose DepthPerPass (no DOC axis to drive).

### Implementation skeleton

```rust
pub struct DeflectionDocRetargeter {
    pub headroom: f64,  // policy.retarget.deflection_headroom (<1.0, e.g., 0.75)
}

impl Retargeter for DeflectionDocRetargeter {
    type Verdict = Verdict;

    fn driving_axes(&self) -> &'static [SearchAxis] {
        &[SearchAxis::DepthPerPass]
    }

    fn target(
        &self,
        verdict: &Verdict,
        space: &SearchSpace,
        view: &AxisView<'_>,
        ctx: &AxisContext<'_>,
    ) -> Option<RetargetSolution> {
        let peak_mm = match verdict {
            Verdict::Exceeds {
                peak, reason: ExceedsReason::LongToolStiffnessUnsafe, ..
            } => *peak,
            _ => return None,
        };
        let baseline_doc = view.axis_value(SearchAxis::DepthPerPass, ctx)?;
        // Op doesn't expose DOC → can't retarget; return None.
        if !view.bindings.iter().any(|b| b.axis == SearchAxis::DepthPerPass) {
            return None;
        }

        let threshold_mm = space.deflection_threshold_mm()?;
        let target_delta = threshold_mm * self.headroom;
        let multiplier = target_delta / peak_mm;
        let raw_target = baseline_doc * multiplier;

        let doc_bounds = space.axis(SearchAxis::DepthPerPass)?;
        let clamped = doc_bounds.hard.clamp(raw_target);
        let was_clamped = (clamped - raw_target).abs() > 1e-6;

        Some(RetargetSolution {
            patches: vec![AxisPatch {
                axis: SearchAxis::DepthPerPass,
                value: clamped,
                clamped: was_clamped,
                source: PatchSource::Primary,
            }],
            rationale: format!(
                "scale DOC by {:.2}× to bring tip deflection {:.0}µm under {:.0}µm × headroom",
                multiplier, peak_mm * 1000.0, threshold_mm * 1000.0,
            ),
        })
    }
}
```

### Required tests (5)

```rust
#[test]
fn deflection_exceeds_halves_doc_when_peak_is_double_threshold() {
    // peak = 0.4mm, threshold = 0.2mm, headroom = 1.0 → mult = 0.5
}

#[test]
fn deflection_returns_none_for_op_without_doc_axis() {
    // ProjectCurve has no DOC binding → None
}

#[test]
fn deflection_clamps_to_doc_floor_for_extreme_overshoot() {
    // peak = 5mm threshold = 0.2 → mult = 0.04 → clamped to doc_bounds.hard.lo
}

#[test]
fn returns_none_for_non_deflection_verdict() {
    // ChiploadBurnRisk → None
}

#[test]
fn no_coupled_patches_emitted() {
    // Solution has exactly one patch (Primary), no Coupled
}
```

---

## What `SearchSpace` needs to expose

Each retargeter reads:
- `space.axis(axis)` — `Option<&AxisBounds>` (already in design doc)
- `space.chipload_bounds()` — `Option<(f64, f64)>` (LUT min, max)
- `space.power_available_kw()` — `Option<f64>` (machine power × safety at baseline RPM)
- `space.deflection_threshold_mm()` — `Option<f64>` (deflection EXCEEDS_BOUND_MM from policy)

These are accessor methods on `SearchSpace`, populated at
`SearchSpace::build()` time from the matched LUT row, machine, and
policy. ~20 LOC total addition to bounds.rs.

## Test scaffolding (shared across retargeters)

A shared `tests/retarget_test_support.rs`:

```rust
pub fn test_space() -> SearchSpaceBuilder { ... }
pub fn test_view() -> AxisViewBuilder { ... }
pub fn test_ctx() -> AxisContext<'static> { ... }

pub struct SearchSpaceBuilder { /* ... */ }
impl SearchSpaceBuilder {
    pub fn with_feed_bounds(mut self, lo: f64, hi: f64) -> Self { ... }
    pub fn with_doc_bounds(mut self, lo: f64, hi: f64) -> Self { ... }
    pub fn with_chipload_bounds(mut self, lo: f64, hi: f64) -> Self { ... }
    pub fn with_power_available(mut self, kw: f64) -> Self { ... }
    pub fn with_deflection_threshold(mut self, mm: f64) -> Self { ... }
    pub fn build(self) -> SearchSpace { ... }
}
```

Avoids per-retargeter boilerplate; agents implementing each retargeter
can use the same helpers.

## Agent prompts (for parallel dispatch after Step 4 lands)

Three nearly-identical prompts, one per retargeter. Template:

> Implement `XRetargeter` in `crates/rs_cam_core/src/tool_load/
> optimize/retarget/{X}.rs` per the prep doc at
> `planning/STEP5_PREP_RETARGETERS.md`, section "{X}".
>
> Follow the `Retargeter` trait from `optimize/retarget/mod.rs`.
> Use shared test helpers from `tests/retarget_test_support.rs`.
> Land all 5 unit tests from the prep doc. Run `cargo test
> --lib tool_load::optimize::retarget::{x}`. Don't touch any other
> file. Report the test output.

Each agent works on its own file; no merge conflicts. Three agents in
parallel = ~80 LOC × 3 + tests, all done in one wall-clock pass.

## Open questions resolved (from §9 of design doc)

- **Multi-gate composition?** Each retargeter emits independent
  candidates. No `JointRetargetStrategy` in Step 5.
- **Plunge tracking?** Captured as `PatchSource::Coupled` patches,
  visible in candidate rationale.
- **Verdict typing?** Step 5 uses flat `Verdict`; Step 7 swaps to
  typed verdicts. Each retargeter file gets ~5 LOC of pattern updates
  at Step 7.
