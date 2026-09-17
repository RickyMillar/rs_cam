//! Scallop finishing strategy — constant scallop height across the surface.
//!
//! Generates concentric offset contours with variable stepover that maintains
//! constant scallop height regardless of surface slope and curvature. On steep
//! walls the stepover is wider (ball endmill has larger effective radius), on
//! shallow convex areas it is tighter.
//!
//! Algorithm:
//! 1. Project mesh boundary onto XY → outer polygon
//! 2. Compute variable stepover from scallop height, local slope, and curvature
//! 3. Iteratively offset polygon inward by stepover
//! 4. Drop-cutter Z at each ring's points → 3D contour
//! 5. Chain rings into toolpath (with optional spiral connection)
//!
//! From Fusion 360 docs: "passes follow sloping and vertical walls to maintain
//! the stepover."

mod research;
mod ring_generation;

use crate::finish::finish_setup::FinishResolutionPolicy;
use crate::finish::scallop_math::variable_stepover;
use crate::geo::{P2, P3};
use crate::geometry::region_set::RegionSet;
use crate::interrupt::{CancelCheck, Cancelled};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::polygon::Polygon2;
use crate::tool::MillingCutter;
use crate::toolpath::Toolpath;
use crate::trace::debug_trace::ToolpathDebugContext;

pub(crate) use research::scallop_toolpath_research_with_stage;
pub use research::{
    ScallopRingBudget, scallop_toolpath_iso_field_with_cancel, scallop_toolpath_research,
    scallop_toolpath_structured_annotated_with_resolution_and_ring_budget,
};
use ring_generation::decimate_ring_polygon;
pub use ring_generation::generate_scallop_rings;

/// Direction for scallop contouring.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScallopDirection {
    /// Start from boundary, work inward (default).
    #[default]
    OutsideIn,
    /// Start from center, work outward.
    InsideOut,
}

/// Parameters for scallop finishing.
pub struct ScallopParams {
    /// Desired scallop height (mm). This is the PRIMARY parameter.
    pub scallop_height: f64,
    /// Path tolerance for simplification.
    pub tolerance: f64,
    /// Direction of contouring.
    pub direction: ScallopDirection,
    /// Connect contours into a continuous spiral (fewer retracts).
    pub continuous: bool,
    /// Slope confinement: only machine slopes steeper than this (degrees).
    pub slope_from: f64,
    /// Slope confinement: only machine slopes shallower than this (degrees).
    pub slope_to: f64,
    /// Feed rate for cutting moves (mm/min).
    pub feed_rate: f64,
    /// Plunge rate (mm/min).
    pub plunge_rate: f64,
    /// Safe Z for rapid positioning.
    pub safe_z: f64,
    /// Stock to leave on the surface (mm).
    pub stock_to_leave: f64,
    /// A/M7 — cap on the XY gap a ring-to-ring **surface link** may span,
    /// in mm. `0.0` disables linking and restores the historical behaviour:
    /// every discrete ring pays `retract → rapid at safe_z → replunge`,
    /// unconditionally, however close the next ring starts.
    ///
    /// That unconditional round trip is the largest untapped population the
    /// A/M7 census found. Air cost in finishing is COUNT-bound — a hop pays
    /// two ~`safe_z` Z legs whatever its XY length — so on a pass of a few
    /// hundred rings the retracts, not the cutting, decide the wall clock.
    ///
    /// Only the JUNCTIONS change: `relink_fragments` copies every fragment
    /// interior verbatim, drop-cutters each candidate link to prove it
    /// stays on the surface, refuses any link that would leave
    /// `boundary_regions`, and (with `link_kinematics`) keeps a link only
    /// when it actually beats the retract it replaces.
    ///
    /// Ignored when `continuous` is set — spiral mode already joins its
    /// contours and has no ring-to-ring junctions to convert.
    pub intra_pass_hookup_mm: f64,
    /// Machine envelope used to COST a candidate surface link against the
    /// retract it would replace (F-034 integrator). `None` keeps any
    /// gouge-safe link within [`Self::intra_pass_hookup_mm`] — correct only
    /// when the caller knows rapid and feed rates are comparable.
    pub link_kinematics: Option<crate::machine::kinematics::LinkKinematics>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ScallopRuntimeEvent {
    Ring {
        ring_index: usize,
        ring_total: usize,
        continuous: bool,
        /// C8: which BOUNDARY REGION's independent ring set this ring
        /// belongs to. Scallop generates "one region at a time,
        /// concatenated in region order" (P2.3), but the annotation stream
        /// carried only a global ring index, so the region structure was
        /// lost before the semantic trace was built and `narrate_toolpath`
        /// reported `regions 0` for an operation that had run several.
        region_index: usize,
        /// How many boundary regions this run generated ring sets for.
        /// `1` when no machining boundary is set — the whole mesh footprint
        /// is one region, which is the honest count, not zero.
        region_total: usize,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScallopRuntimeAnnotation {
    pub move_index: usize,
    pub event: ScallopRuntimeEvent,
}

impl ScallopRuntimeEvent {
    pub fn label(&self) -> String {
        match self {
            Self::Ring { ring_index, .. } => format!("Ring {ring_index}"),
        }
    }
}

impl Default for ScallopParams {
    fn default() -> Self {
        Self {
            scallop_height: 0.1,
            tolerance: 0.05,
            direction: ScallopDirection::OutsideIn,
            continuous: false,
            slope_from: 0.0,
            slope_to: 90.0,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 30.0,
            stock_to_leave: 0.0,
            // A/M7: the LIBRARY default stays 0.0 — a bare `ScallopParams`
            // has no boundary and no kinematics, so it cannot cost or
            // confine a link. The shipped OPERATION default is 3.0 and
            // lives on `ScallopConfig`, which the op adapter fills in
            // alongside both (`compute::execute::generate_scallop`).
            intra_pass_hookup_mm: 0.0,
            link_kinematics: None,
        }
    }
}

/// Radius of the sphere that actually forms the cusp between adjacent
/// passes: the ball TIP for tapered tools, the full radius otherwise.
///
/// P2.f (user-caught "coarse steep stepover", 2026-07-09): the cusp math
/// previously used `cutter.radius()`, which for a tapered ball is the
/// SHANK radius — on the wanaka tool (Ø2 tip on a Ø6 shank) that computed
/// 0.51 mm ring spacing for h = 0.011 where the 1 mm tip needs 0.30 mm,
/// i.e. the real cusp was ~3× the dial. The tip sphere is the exactly
/// correct contact for every slope shallower than `90° − taper_half`
/// (~83° on a 7° taper) — the cone flank never forms the cusp inside the
/// scallop band. The feeds side already bounds on tip radius
/// (`cutter_constraints`'s TaperedBall arm); this brings generation in
/// line with it. Drop-cutter Z lifts always used the full tool profile —
/// only this scalar spacing dial was wrong.
/// Compute the ring's stepover from the slope map and scallop math —
/// the MINIMUM over the sampled points, so the scallop-height guarantee
/// holds at the ring's most demanding (steepest / most convex) stretch.
///
/// P2.f (2026-07-09): this was the MEAN, which on a dendritic mixed-slope
/// region under-tightens exactly where the terrain is steepest — the
/// residual +0.3–0.5 mm leftover band the fidelity instrument showed
/// after the chord fix, and the visibly coarse steep stepover the user
/// flagged. A per-ring constant is inherently a compromise (offsetting is
/// uniform per ring); min is its conservative end.
///
/// M4: the cascade now calls [`ring_stepover_with_policy`] directly so the
/// research seam can vary the reduction. This wrapper stays as the *named*
/// statement of what shipped, and the unit tests below pin it — deleting it
/// would leave `ScallopStepoverPolicy::SHIPPED` as the only description of
/// the shipped behaviour, which is a worse place for it to live.
#[cfg_attr(not(test), allow(dead_code))]
fn ring_stepover(
    ring: &[P2],
    slope_map: &crate::surface::slope::SlopeMap,
    cusp_r: f64,
    scallop_height: f64,
) -> f64 {
    ring_stepover_with_policy(
        ring,
        slope_map,
        cusp_r,
        scallop_height,
        ScallopStepoverPolicy::SHIPPED,
    )
    .selected
}

/// One ring's stepover decision, with the sample distribution it was reduced
/// from.
///
/// The spread is the whole point of the M4 research: a per-ring constant taken
/// as the MIN of its samples collapses to the tightest spot on the ring, so
/// `selected / p50` measures directly how much the ring is being slowed by a
/// minority of its own length.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RingStepoverDecision {
    /// The value the cascade will offset by (pre-clamp).
    pub selected: f64,
    pub sample_min: f64,
    pub sample_p50: f64,
    pub sample_max: f64,
    pub samples: usize,
}

/// [`ring_stepover`] with every compensation it stacks made selectable.
///
/// **Research seam (M4), `pub` so a harness can probe the decision without
/// re-implementing it.** [`ScallopStepoverPolicy::SHIPPED`] reproduces the
/// shipped path exactly, and that is what the private `ring_stepover` above
/// passes, so this adds no behaviour.
#[must_use]
pub fn ring_stepover_with_policy(
    ring: &[P2],
    slope_map: &crate::surface::slope::SlopeMap,
    cusp_r: f64,
    scallop_height: f64,
    policy: ScallopStepoverPolicy,
) -> RingStepoverDecision {
    let flat = crate::finish::scallop_math::stepover_from_scallop_flat(cusp_r, scallop_height);
    let fallback = RingStepoverDecision {
        selected: flat,
        sample_min: flat,
        sample_p50: flat,
        sample_max: flat,
        samples: 0,
    };
    if ring.is_empty() {
        return fallback;
    }

    let sample_step = match policy.sampling {
        RingSampling::Fixed20 => 1.max(ring.len() / 20),
        RingSampling::EveryVertex => 1,
    };

    let mut samples: Vec<f64> = Vec::new();
    for pt in ring.iter().step_by(sample_step) {
        let angle = slope_map.angle_at_world(pt.x, pt.y).unwrap_or(0.0);
        // SlopeMap convention: negative = physically convex (see slope.rs doc on
        // `curvatures`). scallop_math::variable_stepover expects the opposite
        // (positive = convex), so negate at this boundary.
        let curvature = -slope_map.curvature_at_world(pt.x, pt.y).unwrap_or(0.0);
        let curvature = policy.curvature.condition(curvature, cusp_r);
        let so = policy
            .geometry
            .stepover(cusp_r, scallop_height, angle, curvature);
        if so > 0.01 {
            samples.push(so);
        }
    }

    if samples.is_empty() {
        return fallback;
    }
    let mut sorted = samples.clone();
    sorted.sort_by(f64::total_cmp);
    // SAFETY: `sorted` is non-empty (checked above).
    #[allow(clippy::indexing_slicing)]
    let (lo, hi) = (sorted[0], sorted[sorted.len() - 1]);
    #[allow(clippy::indexing_slicing)]
    let p50 = sorted[sorted.len() / 2];
    let selected = policy.reducer.reduce(&sorted);

    RingStepoverDecision {
        selected,
        sample_min: lo,
        sample_p50: p50,
        sample_max: hi,
        samples: sorted.len(),
    }
}

/// How the per-sample stepovers on one ring collapse to the single scalar an
/// `offset_polygon` call can take.
///
/// M4 research seam. [`Self::Min`] is shipped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RingReducer {
    /// **Shipped.** The tightest sample sets the advance for the whole ring.
    /// Conservative by construction — and the mechanism `scallop.rs`'s own
    /// `max_rings` comment fingers for the cascade crawling on terrain.
    Min,
    /// The 10th percentile: still conservative, but one pathological sample
    /// on a long ring no longer owns it.
    P10,
    /// The plan's item 4, "median-ratio clamp" — **benchmark only**. It
    /// knowingly violates the cusp target on the tighter half of every ring
    /// and cannot be adopted without a quantified quality bound, which is
    /// exactly what the oracle now supplies.
    Median,
    /// What shipped before P2.f (2026-07-09), kept so the regression that
    /// motivated the change is reproducible rather than cited.
    Mean,
}

impl RingReducer {
    pub const ALL: [Self; 4] = [Self::Min, Self::P10, Self::Median, Self::Mean];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Min => "min",
            Self::P10 => "p10",
            Self::Median => "median",
            Self::Mean => "mean",
        }
    }

    /// `sorted` must be non-empty and ascending.
    #[allow(clippy::indexing_slicing)] // SAFETY: caller guarantees non-empty
    fn reduce(self, sorted: &[f64]) -> f64 {
        let n = sorted.len();
        match self {
            Self::Min => sorted[0],
            Self::P10 => sorted[((n - 1) as f64 * 0.10).round() as usize],
            Self::Median => sorted[n / 2],
            Self::Mean => sorted.iter().sum::<f64>() / n as f64,
        }
    }
}

/// How densely a ring is sampled before the reducer runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RingSampling {
    /// **Shipped.** `step_by(len / 20)` — roughly 20 samples regardless of
    /// how long the ring is, so a 400 mm ring and a 4 mm ring get the same
    /// budget. The plan's fix-sequence item 1 targets this.
    Fixed20,
    /// Every vertex. Because ring polygons are decimated at `0.75 × cell`
    /// before this runs, "every vertex" IS the plan's distance-bounded
    /// sampling — the bound is the finish grid's own density, so no extremum
    /// the grid can resolve is missed.
    EveryVertex,
}

impl RingSampling {
    pub const ALL: [Self; 2] = [Self::Fixed20, Self::EveryVertex];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Fixed20 => "fixed-20",
            Self::EveryVertex => "every vertex",
        }
    }
}

/// Which stepover-vs-slope law the per-sample value is computed from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepoverGeometry {
    /// **Shipped.** [`crate::finish::scallop_math::variable_stepover`], whose slope
    /// term is `R_eff = R / cos θ` — it WIDENS the stepover on slope.
    Shipped,
    /// The measured law. Rings are offset in **XY**, so two adjacent rings on
    /// ground at slope θ end up `d · sec θ` apart *along the surface*, and the
    /// surface-normal cusp is `R − √(R² − (d·sec θ/2)²)`. Holding the cusp
    /// therefore requires scaling the XY stepover by **`cos θ`**, not by
    /// `1/√cos θ`.
    ///
    /// Ground truth: `scallop_oracle_validation_m4::inclined_plane_cusp_follows_the_secant_law`
    /// measures the law to ≤4.5% at 0–60° and asserts the shipped formula
    /// disagrees with it in the opposite direction (1.24× too wide at 30°,
    /// 2.84× at 60°).
    ///
    /// The curvature correction is unchanged — it is applied to the radius,
    /// as before, and only the slope factor is replaced.
    CosineSlope,
}

impl StepoverGeometry {
    pub const ALL: [Self; 2] = [Self::Shipped, Self::CosineSlope];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Shipped => "shipped R/cosθ",
            Self::CosineSlope => "cosθ (measured law)",
        }
    }

    fn stepover(self, cusp_r: f64, height: f64, angle: f64, curvature: f64) -> f64 {
        match self {
            Self::Shipped => variable_stepover(cusp_r, height, angle, curvature),
            Self::CosineSlope => {
                let base = crate::finish::scallop_math::stepover_from_scallop_curved(
                    cusp_r, height, curvature,
                );
                (base * angle.cos()).min(cusp_r * 4.0)
            }
        }
    }
}

/// What the per-sample curvature reading is allowed to claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurvaturePolicy {
    /// **Shipped.** The raw second-derivative estimate off the finish grid.
    /// Its magnitude scales as `1/cell²`, so the same terrain reports larger
    /// curvature on a finer grid — which is why a resolution change moves the
    /// selected stepover at all.
    Raw,
    /// Clamp `|κ| ≤ 1/cusp_r`. A ball of tip radius `R` physically cannot
    /// follow convex curvature tighter than `1/R` — it bridges it — so a
    /// reading beyond that describes a feature the tool cannot resolve and
    /// must not be allowed to set the ring's advance.
    ToolLimited,
}

impl CurvaturePolicy {
    pub const ALL: [Self; 2] = [Self::Raw, Self::ToolLimited];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Raw => "raw",
            Self::ToolLimited => "|κ| ≤ 1/R",
        }
    }

    fn condition(self, curvature: f64, cusp_r: f64) -> f64 {
        match self {
            Self::Raw => curvature,
            Self::ToolLimited => {
                if cusp_r <= 0.0 {
                    return curvature;
                }
                let cap = 1.0 / cusp_r;
                curvature.clamp(-cap, cap)
            }
        }
    }
}

/// How the several polygons alive at one cascade iteration agree on a single
/// offset distance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolygonReduce {
    /// **Shipped.** `fold(INFINITY, f64::min)` across every live polygon, so
    /// one tight branch of a multi-polygon cascade sets the advance for all
    /// of them — a second minimum stacked on top of the per-ring one.
    MinAcross,
    /// Each polygon offsets by its own ring's decision. Nothing about
    /// `offset_polygon` requires the distances to agree; they were tied
    /// together so "the cusp guarantee holds on every branch", which a
    /// per-polygon distance also achieves.
    PerPolygon,
}

impl PolygonReduce {
    pub const ALL: [Self; 2] = [Self::MinAcross, Self::PerPolygon];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::MinAcross => "min across polygons",
            Self::PerPolygon => "per polygon",
        }
    }
}

/// Where the ring polygons come from at all.
///
/// M4's phase B asks, in the plan's words, *"whether scallop remains an
/// offset-ring algorithm or should become an iso-field contour extractor"*.
/// This is the axis that question lives on. Both sources feed the SAME 3D
/// lift, chord refinement and emission, so a comparison across it isolates
/// ring PLACEMENT and nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RingSource {
    /// **Shipped.** Iterated `offset_polygon` from the region boundary
    /// inward, one scalar distance per iteration, bounded by `max_rings`.
    OffsetCascade,
    /// [`crate::finish::scallop_isofield`] — solve `|∇D| = 1/s(x,y)` from the
    /// boundary and take the integer level sets. No per-ring scalar, no
    /// repeated offsetting, and the ring count is `⌊max D⌋` rather than a cap.
    IsoField,
}

impl RingSource {
    pub const ALL: [Self; 2] = [Self::OffsetCascade, Self::IsoField];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::OffsetCascade => "offset cascade",
            Self::IsoField => "iso-field level sets",
        }
    }
}

/// The five compensations the scallop ring cascade stacks, each selectable.
///
/// **M4 research seam. [`Self::SHIPPED`] is the only value any production
/// entry point passes**, and every field of it names the shipped choice, so
/// this enum family adds no behaviour and moves no default. It exists because
/// M4's phase B cannot attribute a defect to one compensation without turning
/// the others off, and `CHECKPOINT_B_EVIDENCE.md`'s `max_rings` addendum
/// closed with exactly that recommendation: *"the fix is in `ring_stepover`,
/// not in the budget."*
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScallopStepoverPolicy {
    pub reducer: RingReducer,
    pub sampling: RingSampling,
    pub geometry: StepoverGeometry,
    pub curvature: CurvaturePolicy,
    pub across_polygons: PolygonReduce,
    pub ring_source: RingSource,
    pub cleanup: RingCleanup,
    pub sample_bound: RingSampleBound,
}

/// How dense a flattened ring's STRAIGHT runs are, independent of how
/// accurately its curves are approximated.
///
/// Wave 14 seam, and it exists because the arc-carrying cascade separated two
/// things that used to arrive fused. [`crate::polygon::FlattenPolicy`] has a
/// deviation budget — how far a chord may sit from the arc it replaces — and
/// that budget puts points where the ring CURVES and nowhere else. Correct for
/// a consumer that wants a shape. Scallop is not one: it lifts every ring
/// vertex with a drop-cutter query and asks a coverage mask whether that XY
/// sits over real mesh, so a straight run's interior points are *samples of a
/// surface*, and a deviation budget owes it none of them.
///
/// So the sampling density is stated here rather than inherited, and the
/// variants are the candidates Checkpoint D's follow-up A/B scored on the M4
/// envelope oracle (`ring_sample_bound_w14.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RingSampleBound {
    /// **Shipped.** Spacing derived from the OPERATION'S OWN chord tolerance
    /// and the cusp-forming radius — the surface-sampling analogue of the
    /// scallop law, `2·√(2·r·tol)`, i.e. the chord across which a feature of
    /// tool-tip curvature can hide a `tol`-deep deviation from the two
    /// endpoints that bracket it. Tight tolerances buy density; loose ones
    /// do not.
    ToleranceScaled,
    /// The flat-ground stepover — the spacing the outer boundary rectangle is
    /// seeded at. Fixed with respect to the tolerance dial: it reads the cusp
    /// height where [`Self::ToleranceScaled`] reads the chord tolerance.
    #[default]
    FlatGroundStepover,
    /// No bound at all: the honest reading of "flatten to a tolerance", and
    /// wrong for this consumer — on a mesh of disjoint islands the surviving
    /// corners all sit off the part and the operation emits nothing.
    /// Retained as the isolator that proves the bound is load-bearing.
    ToleranceOnly,
}

impl RingSampleBound {
    pub const ALL: [Self; 3] = [
        Self::ToleranceScaled,
        Self::FlatGroundStepover,
        Self::ToleranceOnly,
    ];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::ToleranceScaled => "tolerance-scaled",
            Self::FlatGroundStepover => "flat-ground stepover",
            Self::ToleranceOnly => "tolerance only (no bound)",
        }
    }

    /// The maximum emitted segment length, or `None` for "deviation budget
    /// only". `cusp_r` is the cusp-forming radius, never the shank.
    #[must_use]
    pub fn max_segment_mm(
        self,
        cusp_r: f64,
        scallop_height: f64,
        chord_tolerance: f64,
    ) -> Option<f64> {
        let floor = cusp_r * 0.1;
        match self {
            Self::ToleranceScaled => Some(
                crate::finish::scallop_math::stepover_from_scallop_flat(cusp_r, chord_tolerance)
                    .max(floor),
            ),
            Self::FlatGroundStepover => Some(
                crate::finish::scallop_math::stepover_from_scallop_flat(cusp_r, scallop_height)
                    .max(floor),
            ),
            Self::ToleranceOnly => None,
        }
    }
}

/// What the cascade does to an offset ring before it becomes the next ring's
/// input.
///
/// The offset primitive doubles a concave ring's vertex count per pass
/// (`tests/offset_growth_m5.rs`: added vertices == arc-join segments, 1:1),
/// because `Polygon2::from_pline` throws away each arc join's bulge and keeps
/// its two endpoints. Every variant below except [`Self::ArcCascade`] is a
/// *brake* on that: something that removes vertices after the fact, at some
/// price in geometry.
///
/// [`Self::ArcCascade`] is what ships since Checkpoint D (2026-08-03): the
/// rings keep their arcs between offsets and are flattened exactly once, at
/// the boundary where they become feed moves, under
/// [`crate::polygon::FlattenPolicy`]. It removes the mechanism instead of
/// damping it — 0.0 µm off the erosion oracle where every brake is 4–300 µm,
/// 200 rings bounded and *shrinking*, and no eroded-area price (drop-only
/// decimation leaves +9.83% too much material on a comb).
///
/// The brakes are retained as research arms so the M4 envelope oracle can
/// keep scoring the alternatives; **only `ArcCascade` is reachable from a
/// production entry point.** They run on the flattened cascade, which is the
/// only way they are comparable to what they compensated for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RingCleanup {
    /// **Shipped since Checkpoint D.** Arcs are carried from ring to ring and
    /// flattened once, at the emission boundary.
    #[default]
    ArcCascade,
    /// Drop-only decimation at `0.75 × heightmap cell` — what shipped from
    /// P2.f until Checkpoint D. Retired from production: it is not bounded in
    /// world units (1.8 mm error, 0.54 mm mean over-cut on a coarse pocket
    /// boundary) and it rounds off features narrower than a few spacings
    /// (+9.83% eroded area on the comb fixture).
    DecimateAtCell,
    /// Nothing at all — the flattened cascade every single-shot consumer of
    /// `offset_polygon` runs today, with no brake.
    KeepEverything,
    /// `polygon::cleanup_collinear` at 1 nm: duplicates and genuinely
    /// collinear vertices only.
    CollinearDedup,
    /// `polygon::simplify_bounded` (RDP) at a tenth of the operation's chord
    /// tolerance — bounded error, self-intersection guarded.
    SimplifyBounded,
}

impl RingCleanup {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::ArcCascade => "arc cascade",
            Self::DecimateAtCell => "decimate@0.75cell",
            Self::KeepEverything => "keep-all",
            Self::CollinearDedup => "collinear+dedup",
            Self::SimplifyBounded => "simplify@tol/10",
        }
    }

    /// True when the rings are carried as arcs rather than as polygons.
    #[must_use]
    pub const fn carries_arcs(self) -> bool {
        matches!(self, Self::ArcCascade)
    }

    /// Reduce one offset result. `None` culls it. Only ever called on the
    /// flattened (research) cascade.
    fn apply(self, poly: &Polygon2, min_spacing: f64, chord_tolerance: f64) -> Option<Polygon2> {
        match self {
            // The arc cascade does its work in the arc domain, before this
            // point; nothing is dropped here.
            Self::ArcCascade | Self::KeepEverything => {
                (poly.exterior.len() >= 3).then(|| poly.clone())
            }
            Self::DecimateAtCell => decimate_ring_polygon(poly, min_spacing),
            Self::CollinearDedup => crate::polygon::cleanup_collinear(poly, 1e-5, 1e-6),
            Self::SimplifyBounded => crate::polygon::simplify_bounded(poly, chord_tolerance * 0.1),
        }
    }
}

impl ScallopStepoverPolicy {
    /// Exactly what ships today.
    pub const SHIPPED: Self = Self {
        reducer: RingReducer::Min,
        sampling: RingSampling::Fixed20,
        geometry: StepoverGeometry::Shipped,
        curvature: CurvaturePolicy::Raw,
        across_polygons: PolygonReduce::MinAcross,
        ring_source: RingSource::OffsetCascade,
        cleanup: RingCleanup::ArcCascade,
        sample_bound: RingSampleBound::FlatGroundStepover,
    };

    #[must_use]
    pub fn label(self) -> String {
        format!(
            "{} / {} / {} / {} / {} / {} / {} / {}",
            self.ring_source.label(),
            self.reducer.label(),
            self.sampling.label(),
            self.geometry.label(),
            self.curvature.label(),
            self.across_polygons.label(),
            self.cleanup.label(),
            self.sample_bound.label()
        )
    }

    /// **Test door.** The harnesses under `crates/rs_cam_core/tests` are the
    /// only callers. No production path reads it.
    #[must_use]
    pub const fn is_shipped(self) -> bool {
        matches!(self.reducer, RingReducer::Min)
            && matches!(self.sampling, RingSampling::Fixed20)
            && matches!(self.geometry, StepoverGeometry::Shipped)
            && matches!(self.curvature, CurvaturePolicy::Raw)
            && matches!(self.across_polygons, PolygonReduce::MinAcross)
            && matches!(self.ring_source, RingSource::OffsetCascade)
            && matches!(self.cleanup, RingCleanup::ArcCascade)
            && matches!(self.sample_bound, RingSampleBound::FlatGroundStepover)
    }

    /// The per-point stepover this policy's geometry+curvature choice yields,
    /// exposed so [`crate::finish::scallop_isofield`] and a harness can build the same
    /// field the cascade would sample.
    #[must_use]
    pub(crate) fn point_stepover(
        self,
        slope_map: &crate::surface::slope::SlopeMap,
        cusp_r: f64,
        scallop_height: f64,
        x: f64,
        y: f64,
    ) -> f64 {
        let angle = slope_map.angle_at_world(x, y).unwrap_or(0.0);
        let curvature = -slope_map.curvature_at_world(x, y).unwrap_or(0.0);
        let curvature = self.curvature.condition(curvature, cusp_r);
        let so = self
            .geometry
            .stepover(cusp_r, scallop_height, angle, curvature);
        // Same clamps the cascade applies, so the two sources are comparable.
        so.max(cusp_r * 0.05).min(cusp_r * 3.0)
    }
}

impl Default for ScallopStepoverPolicy {
    fn default() -> Self {
        Self::SHIPPED
    }
}

/// What one cascade run decided, iteration by iteration.
///
/// Research output only — the shipped entry points do not build it.
#[derive(Debug, Clone, Default)]
pub struct ScallopStepoverTrace {
    /// The clamped distance actually passed to `offset_polygon`, per
    /// iteration (the MIN over polygons when that is the policy).
    pub selected_mm: Vec<f64>,
    /// Per iteration, `(min, p50, max)` of the per-sample stepovers the
    /// reducer chose from, taken over every live polygon.
    pub sample_spread: Vec<(f64, f64, f64)>,
}

/// The three area figures one boundary region's ring cascade produces,
/// alongside its lifted rings (see [`RingCascade`]). A named struct rather
/// than a 3-tuple so `generate_scallop_rings_with_cancel`'s callers cannot
/// mix up which field is which — the exact mistake a positional tuple
/// invites once there is more than one `f64` in it.
///
/// Every field is a per-region figure; `scallop_toolpath_research` sums
/// these across every boundary region into [`ScallopReport`]'s
/// identically-named fields.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct RingCascadeMetrics {
    /// See [`ScallopReport::uncut_core_mm2`].
    pub uncut_core_mm2: f64,
    /// See [`ScallopReport::untouched_mm2`].
    pub untouched_mm2: f64,
    /// See [`ScallopReport::standing_mm2`].
    pub standing_mm2: f64,
}

/// One boundary region's lifted rings, paired with what the cascade found
/// out about the geometry it just cut. `bool` per point is `ring_to_3d`'s
/// keep flag.
pub type RingCascade = (Vec<Vec<(P3, bool)>>, RingCascadeMetrics);

/// What the ring cascade found out about the geometry it just cut.
///
/// Exists so a generation-time finding can reach the diagnostics pipeline
/// instead of dying in a `tracing::warn!`. The campaign lost weeks to a
/// 28 mm block of standing material that was warned about on every run and
/// visible to nobody — see `planning/unified_v3_design.md` §13/§14c.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ScallopReport {
    /// Region-interior area (mm²) the ring cascade LEFT UNCUT because it
    /// hit `max_rings` before the offsets collapsed, summed over every
    /// boundary region. `0.0` when every cascade collapsed normally.
    ///
    /// Known defect (`planning/v3_workplan.md`): the cap is budgeted from
    /// the FLAT-ground (widest) stepover while the loop selects a smaller
    /// one on every slope. Raising the cap alone measured far worse
    /// (Op B +92% time, deep over-cut 34×), so this reports the symptom
    /// until `ring_stepover`'s min-across-ring collapse is fixed.
    ///
    /// **HOLE-BLIND** (M4 §5b, `MEASUREMENT_DOMAINS.md` X-5): computed from
    /// each truncated polygon's EXTERIOR shoelace area only, so an island
    /// inside the truncated core — a hole the cascade's own offset created,
    /// or one the region boundary carried in — is counted as uncut even
    /// though it is not part of the region. Kept exactly as-is (same
    /// formula, same call sites) for continuity with every existing
    /// consumer; see [`Self::untouched_mm2`] for the corrected figure.
    pub uncut_core_mm2: f64,
    /// Hole-aware net area (mm²) the ring cascade never reached — the same
    /// truncated-cascade polygons as [`Self::uncut_core_mm2`], but summing
    /// `|exterior shoelace| − Σ|hole shoelace|` per polygon (floored at 0)
    /// instead of the exterior alone, then summed over every boundary
    /// region. `untouched_mm2 <= uncut_core_mm2` always; the gap is exactly
    /// the hole-blindness [`Self::uncut_core_mm2`] carries forward.
    ///
    /// This is the production analogue of the M4 research oracle's
    /// `OracleReport::untouched_mm2` — "area no cutter position ever
    /// covered" — named identically so the two instruments are recognisable
    /// as measuring the same thing (see
    /// `tests/common/scallop_oracle.rs::OracleReport::untouched_mm2`'s doc).
    /// `0.0` when every cascade collapsed normally, same condition as
    /// [`Self::uncut_core_mm2`]. See [`Self::UNTOUCHED_PROVENANCE`] for the
    /// measurement contract (M1).
    pub untouched_mm2: f64,
    /// Estimated area (mm²) the cascade's rings PASSED OVER but did not cut,
    /// because the per-point keep predicate (`ring_to_3d`'s coverage flag —
    /// no finite drop-cutter contact, or outside the finish heightmap's
    /// coverage) dropped those ring vertices. Summed, over every offset ring
    /// in every boundary region, as `Σ (arc length owned by dropped points)
    /// × (that ring's offset stepover)` — see `dropped_arc_length_mm` for
    /// "owned arc length". `0.0` when every ring point on every ring was
    /// kept. Excludes the seed boundary ring (see
    /// `generate_scallop_rings_with_cancel`'s comment at its push site: no
    /// offset stepover produced it).
    ///
    /// This is the production analogue of the M4 research oracle's
    /// `OracleReport::standing_mm2` — "reached, but left high" — though the
    /// analogy is not exact; see [`Self::STANDING_PROVENANCE`] for the
    /// estimator's stated limitations, in particular that it cannot tell
    /// "dropped because off-part" from "dropped because left high" and so
    /// can over-report on a mesh with real off-part gaps inside the region.
    pub standing_mm2: f64,
    /// Rings the offset cascade PRODUCED, summed over every boundary region
    /// — before the coverage / slope / boundary keep-predicate has had a say.
    ///
    /// Read with [`Self::ring_count`]: `cascade_ring_count` is how far the
    /// cascade got (and so how close it ran to `max_rings`), while
    /// `ring_count` is what actually reached the toolpath. A large gap means
    /// the rings were generated and then filtered away, which is a different
    /// story from a cascade that stopped early.
    pub cascade_ring_count: usize,
    /// Ring (or partial-ring run) spans actually EMITTED into the toolpath —
    /// exactly one per [`ScallopRuntimeAnnotation`].
    ///
    /// Wave D3: the Checkpoint-B harness had to count runtime annotations to
    /// report this, which made a report-only number depend on a side channel
    /// the report already knew. Report-only: nothing gates on it.
    pub ring_count: usize,
    /// G-LINKVISIBLE: what the ring-to-ring link stage did.
    ///
    /// The counters used to reach a `tracing::info!` line and nothing else,
    /// so the ACCEPTANCE measure for G-LINKSTAGE
    /// ([`crate::finish::surface_link::RelinkReport::at_depth_links`]) was readable
    /// only by scraping a headless run's stdout. This carries it to the op
    /// adapter, which publishes it on
    /// [`crate::compute::config::ToolpathStats::relink`].
    ///
    /// * `None` — **not measured**. The pass never ran:
    ///   `intra_pass_hookup_mm` is `0.0` (which disables it), `continuous`
    ///   (spiral mode chains its own contours, so no ring-to-ring junction
    ///   is left), or the cascade produced no ring at all. Never read it as
    ///   "nothing retracted".
    /// * `Some(t)` with every counter zero — the pass ran and found no
    ///   junction to act on.
    ///
    /// Report-only: no gate consumes it.
    pub relink: Option<crate::finish::unified_finish::RelinkTotals>,
}

impl ScallopReport {
    /// What [`Self::uncut_core_mm2`] means (M1). Fixed for this measure, so it
    /// is a constant rather than a settable field — a field could drift from
    /// the code that fills it, and this number's whole failure mode is being
    /// read as something it is not.
    ///
    /// This is the single source the user-visible standing-material strings
    /// are derived from
    /// ([`crate::compute::config::TRUNCATED_CORE_DOMAIN`] and friends).
    pub const PROVENANCE: crate::measurement::MeasurementProvenance =
        crate::measurement::MeasurementProvenance::new(
            crate::measurement::MeasurementDomain::ProjectedXyArea,
            crate::measurement::MeasurementStage::RingCascadeResidual,
        )
        // Wave 14: the rings are no longer decimated at the heightmap cell —
        // they are arc-carrying offsets flattened once, at a tenth of the
        // op's chord tolerance (`polygon::FlattenPolicy`). The old note named
        // a step that no longer runs, which is exactly the drift this
        // constant exists to prevent.
        .with_resolution_note(
            "ring polygons flattened at 0.1x the op chord tolerance (exterior shoelace)",
        );

    /// What [`Self::untouched_mm2`] means (M1). **Same domain and stage as
    /// [`Self::PROVENANCE`]** — both are exact shoelace areas taken from the
    /// SAME truncated-cascade polygons at the same generation-time cascade
    /// residual stage — but a DIFFERENT resolution: this one nets out each
    /// polygon's holes before summing, where [`Self::PROVENANCE`] sums
    /// exteriors only. Sharing the stage is deliberate (M4 §5b ruling): a
    /// reader who already knows what `PROVENANCE` means should recognise
    /// this as "the same measurement, corrected for holes", not an unrelated
    /// figure.
    pub const UNTOUCHED_PROVENANCE: crate::measurement::MeasurementProvenance =
        crate::measurement::MeasurementProvenance::new(
            crate::measurement::MeasurementDomain::ProjectedXyArea,
            crate::measurement::MeasurementStage::RingCascadeResidual,
        )
        .with_resolution_note(
            "ring polygons flattened at 0.1x the op chord tolerance; hole-aware net area \
             (exterior shoelace minus each hole's shoelace, floored at 0 per polygon) — the \
             same truncated-cascade geometry as PROVENANCE, corrected for interior holes",
        );

    /// What [`Self::standing_mm2`] means (M1). A DIFFERENT
    /// [`crate::measurement::MeasurementStage`] from [`Self::PROVENANCE`] /
    /// [`Self::UNTOUCHED_PROVENANCE`] on purpose —
    /// [`crate::measurement::MeasurementStage::RingCascadeStandingEstimate`]
    /// — even though all three come from the same cascade run, so
    /// [`crate::measurement::MeasurementProvenance::comparable_to`] refuses
    /// to treat this ESTIMATOR as interchangeable with either exact polygon
    /// area: it cannot be summed with or ratio'd against `uncut_core_mm2` or
    /// `untouched_mm2` just because the domain label matches.
    pub const STANDING_PROVENANCE: crate::measurement::MeasurementProvenance =
        crate::measurement::MeasurementProvenance::new(
            crate::measurement::MeasurementDomain::ProjectedXyArea,
            crate::measurement::MeasurementStage::RingCascadeStandingEstimate,
        )
        .with_resolution_note(
            "estimator, not a polygon area: per ring, (arc length owned by ring points the \
             keep predicate dropped) x (that ring's offset stepover), summed over every \
             emitted offset ring in every boundary region; excludes the seed boundary ring; \
             ignores residual DEPTH (unlike the oracle's standing_mult gate) and cannot \
             distinguish a dropped point caused by off-part geometry from one caused by a \
             real left-high residual",
        );

    /// [`Self::uncut_core_mm2`] tagged with its domain — XY-projected, never
    /// a 3D surface area and never a share of one.
    ///
    /// **Test door.** The harnesses under `crates/rs_cam_core/tests` are the
    /// only callers. No production path reads it.
    #[must_use]
    pub const fn uncut_core(&self) -> crate::measurement::ProjectedXyAreaMm2 {
        crate::measurement::ProjectedXyAreaMm2::new(self.uncut_core_mm2)
    }

    /// [`Self::untouched_mm2`] tagged with its domain — see
    /// [`Self::UNTOUCHED_PROVENANCE`].
    #[must_use]
    pub const fn untouched(&self) -> crate::measurement::ProjectedXyAreaMm2 {
        crate::measurement::ProjectedXyAreaMm2::new(self.untouched_mm2)
    }

    /// [`Self::standing_mm2`] tagged with its domain — see
    /// [`Self::STANDING_PROVENANCE`]. Remember this is an ESTIMATOR, not a
    /// polygon area: the newtype only gates the domain arithmetic (rule 3),
    /// it does not upgrade the estimator into an exact measurement.
    #[must_use]
    pub const fn standing(&self) -> crate::measurement::ProjectedXyAreaMm2 {
        crate::measurement::ProjectedXyAreaMm2::new(self.standing_mm2)
    }
}

/// The resolution policy scallop generates on (H3 step 2).
///
/// Scallop selects `FinishResolutionMode::LegacyEnvelopeQuarter` — the same
/// `(envelope_radius/4).max(tolerance)` cell it has always used, now stated
/// here rather than inherited from a shared builder. Because the choice is
/// local, moving scallop onto `FinishResolutionMode::CuspQuarter` (H3 step 4
/// starts with scallop: it has the strongest fidelity instrument) is an edit
/// to this one function and does not move `ramp_finish` or `steep_shallow`.
///
/// NOTE the asymmetry this function makes visible: scallop's STEPOVER is
/// cusp-scaled (`cusp_radius_mm()` below) while its GRID is envelope-scaled,
/// so on a tapered ball the rings are ~6× finer than the surface they are
/// sampled from. That is the H3 question, not a bug fixed here.
#[must_use]
pub fn scallop_generation_resolution(
    cutter: &dyn MillingCutter,
    tolerance: f64,
) -> FinishResolutionPolicy {
    FinishResolutionPolicy::legacy_envelope_quarter(cutter, tolerance)
}

/// Generate a scallop finishing toolpath.
///
/// Produces concentric offset contours with variable stepover that maintains
/// constant scallop height across the surface regardless of slope and curvature.
#[tracing::instrument(skip(mesh, index, cutter, params), fields(scallop_height = params.scallop_height))]
#[allow(clippy::indexing_slicing)] // ring/filtered indexing is guarded by len checks
pub fn scallop_toolpath(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &ScallopParams,
) -> Toolpath {
    let (tp, _, _) = scallop_toolpath_structured_annotated(mesh, index, cutter, params, None);
    tp
}

impl crate::compute::spans::RuntimeLabel for ScallopRuntimeAnnotation {
    fn move_index(&self) -> usize {
        self.move_index
    }

    fn label(&self) -> String {
        self.event.label()
    }
}

fn scallop_toolpath_structured_annotated(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &ScallopParams,
    debug: Option<&ToolpathDebugContext>,
) -> (Toolpath, Vec<ScallopRuntimeAnnotation>, ScallopReport) {
    crate::interrupt::run_uncancellable(|cancel| {
        scallop_toolpath_structured_annotated_with_cancel(
            mesh, index, cutter, params, debug, None, cancel,
        )
    })
}

/// Cancellable variant of `scallop_toolpath_structured_annotated`. Polls
/// `cancel` once per ring during 3D ring generation (`ring_to_3d`'s
/// per-point drop-cutter queries are the expensive step) and once per ring
/// again while chaining rings into the toolpath.
///
/// `boundary_regions` (P2.3): when `Some`, generation is pre-clipped to
/// these machining-boundary regions instead of the full mesh footprint —
/// exactly one region is used directly as the ring boundary (replacing the
/// hardcoded mesh-bbox rectangle below); multiple disjoint regions each get
/// their own independent ring set, concatenated (regions are disjoint by
/// construction, so no de-duplication is needed). `None` reproduces
/// today's single mesh-bbox-rectangle behavior byte-for-byte.
#[allow(clippy::too_many_arguments)]
pub fn scallop_toolpath_structured_annotated_with_cancel(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &ScallopParams,
    debug: Option<&ToolpathDebugContext>,
    boundary_regions: Option<&RegionSet<'_>>,
    cancel: &dyn CancelCheck,
) -> Result<(Toolpath, Vec<ScallopRuntimeAnnotation>, ScallopReport), Cancelled> {
    scallop_toolpath_structured_annotated_with_cancel_and_stage(
        mesh,
        index,
        cutter,
        params,
        debug,
        boundary_regions,
        None,
        cancel,
    )
}

/// [`scallop_toolpath_structured_annotated_with_cancel`] with the shared
/// finishing link stage (G-LINKSTAGE).
///
/// The shape this op takes for `planning/linking_2026-09-09/SPEC.md` §3.1,
/// decided and recorded here: the stage RUNS INSIDE THE GENERATOR and its
/// configuration is threaded in as a parameter, exactly as
/// `unified_finish_toolpath_with_cancel_and_ceiling` threads its ceiling. The
/// alternative — run the stage in the `execute.rs` adapter — would have to
/// expose the per-ring annotations as a provenance channel, and the
/// annotation reconcile already lives beside the relink call. The stage is
/// NOT a `ScallopParams` field for a mechanical reason: that struct is built
/// by literal in the op adapter, the unified-finish mid-steep band, the
/// multitool planner and a dozen tests, and a borrowed field would put a
/// lifetime on every one of them.
///
/// `None` is the legacy relink, byte for byte: `reorder: false`,
/// `link_ceiling: None`, no loop rotation. That is what the ISO-FIELD entry
/// point passes, so the field's fingerprint does not move.
#[allow(clippy::too_many_arguments)]
pub fn scallop_toolpath_structured_annotated_with_cancel_and_stage(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &ScallopParams,
    debug: Option<&ToolpathDebugContext>,
    boundary_regions: Option<&RegionSet<'_>>,
    link_stage: Option<&crate::finish::surface_link::FinishingLinkStage<'_>>,
    cancel: &dyn CancelCheck,
) -> Result<(Toolpath, Vec<ScallopRuntimeAnnotation>, ScallopReport), Cancelled> {
    let (tp, anns, report, _trace) = scallop_toolpath_research_with_stage(
        mesh,
        index,
        cutter,
        params,
        debug,
        boundary_regions,
        scallop_generation_resolution(cutter, params.tolerance),
        ScallopRingBudget::FlatGroundStepover,
        ScallopStepoverPolicy::SHIPPED,
        link_stage,
        cancel,
    )?;
    Ok((tp, anns, report))
}

/// [`scallop_toolpath_structured_annotated_with_cancel`] with the generation
/// grid resolution supplied by the caller instead of selected by
/// [`scallop_generation_resolution`].
///
/// **Research seam, not a production entry point.** H3's Checkpoint B harness
/// (`tests/checkpoint_b_resolution_ab.rs`) has to hold tool, params, boundary
/// and tolerance fixed while varying ONLY the generation cell — the shared
/// builder's own `..._with_cell_size_...` adapter cannot do that from outside,
/// because scallop resolves its policy internally. Passing
/// `scallop_generation_resolution(cutter, params.tolerance)` reproduces the
/// shipped path exactly (that is literally what the wrapper above does), so
/// this adds no behavior and changes no default.
#[allow(clippy::too_many_arguments)]
pub fn scallop_toolpath_structured_annotated_with_resolution(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &ScallopParams,
    debug: Option<&ToolpathDebugContext>,
    boundary_regions: Option<&RegionSet<'_>>,
    resolution: FinishResolutionPolicy,
    cancel: &dyn CancelCheck,
) -> Result<(Toolpath, Vec<ScallopRuntimeAnnotation>, ScallopReport), Cancelled> {
    scallop_toolpath_structured_annotated_with_resolution_and_ring_budget(
        mesh,
        index,
        cutter,
        params,
        debug,
        boundary_regions,
        resolution,
        ScallopRingBudget::FlatGroundStepover,
        cancel,
    )
}

#[cfg(test)]
mod tests;
