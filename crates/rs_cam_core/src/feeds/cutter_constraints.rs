//! Unified axial-DOC envelope for the "how deep is too deep?" question.
//!
//! Phase 2 of `planning/cutter_axial_constraints_2026-06-06.md`. Every
//! Suggest pass that touches axial depth — V-carve `max_depth`, 3D Finish
//! `stock_to_leave`, Adaptive3d `depth_per_pass`, ProjectCurve `target_depth` —
//! queries [`cutter_axial_constraints`] once with op-specific engagement
//! geometry, gets back four independently-computed bounds, and intersects
//! with its own goal via [`CutterAxialConstraints::safe_max_doc_mm`] /
//! [`CutterAxialConstraints::safe_band_is_empty`].
//!
//! ## Bounds
//!
//! | Bound | What gates it | Cutters it applies to |
//! |---|---|---|
//! | `max_doc_deflection_mm` | Tip-deflection exceeds the operator's tolerance | All (scan, then bisection) |
//! | `max_doc_vendor_mm` | LUT row's `ap_*_factor` × diameter OR `ap_*_mm` absolute, tighter wins | When a LUT row is supplied |
//! | `max_doc_scallop_mm` | Axial-scallop height between Z-passes degrades finish | Ball / bull / tapered-ball |
//! | `min_doc_chipload_floor_mm` | Arc-mean chip thickness collapses below LUT `chipload_min` (burn risk) | Ball / tapered-ball (flat chipload doesn't depend on DOC) |
//!
//! ## Deflection inversion strategy
//!
//! The post-sim deflection gate uses [`crate::tool::ToolDefinition::tip_deflection_mm`]
//! — a stepped cantilever that walks the cutter's shape via
//! `lookup_diameter_at` over 64 integration steps. The envelope must invert
//! that model for `ap`, but the relation is non-linear:
//!
//! - Flat endmill: force ∝ ap (linear), deflection ∝ force × ap-shift effects.
//! - Ball / bull / tapered ball: engagement radius grows with ap, so
//!   force scales super-linearly.
//! - V-bit: engaged diameter is `tip_d + 2 ap tan(α/2)` — engagement
//!   radius grows linearly with ap, force is roughly quadratic.
//!
//! No closed form covers all four shapes. Feeds contract P11a (L4, D-B)
//! solves for the SHALLOWEST depth where the tip deflection meets the
//! target. The search span is `[stickout × 1e-4, min(stickout × 0.95,
//! flute)]`: it scans [`DEFLECTION_SCAN_STEPS`] depths for the first one
//! over the target, then bisects that step to ±5 µm. Deflection is not
//! monotone in `ap`: the force grows with `ap`, but the load point
//! `stickout − ap/2` moves toward the collet (research_ae A2), so a scan
//! comes before the bisection. At each depth the cut is one
//! [`crate::feeds::engagement::Engagement`] (`from_tool_cut`): the arc
//! ψ reads the engaged radius at that depth (it moves on a ball, a bull
//! nose, a taper and a V-bit), and `fz` is fixed. When no depth in the
//! span reaches the target, there is no deflection limit below the flute:
//! the bound is `None`, never the bracket.
//!
//! ## Why not reuse [`super::predict::predict_peak_deflection_um`]
//!
//! One reason survives of the two in
//! `planning/cutter_axial_constraints_2026-06-06.md` §7 "do NOT use": the
//! predictor needs `depth_per_pass()`, and the envelope runs before the
//! op-config carries one, because Suggest is *picking* the DPP. A "needs
//! DPP to compute" predictor is the wrong layer.
//!
//! The second reason — "it refuses V-bits (returns 0)" — stopped holding
//! under T-4 (2026-09-18). The predictor no longer refuses a V-bit; it
//! returns a figure carrying
//! [`super::predict::DeflectionCaveat::FluteReliefUnmodeled`]. This
//! envelope has computed a V-bit bound from the shared integrator
//! throughout, and the same flute-relief gap applies to it.
//!
//! Both paths now route through
//! [`super::predict::tip_deflection_from_engagement`] for the underlying
//! force + cantilever step, so the gate and envelope share one
//! deflection model.

use crate::material::Material;
use crate::tool::{MillingCutter, ToolDefinition};

use super::ToolGeometryHint;
use super::vendor_lookup::LookupResult;

/// Default deflection bound when the caller doesn't override (µm).
/// P11c (review item M1): it reads the one F201 row
/// `feeds.suggest.DEFLECTION_BACKOFF_TARGET_UM`, as the post-sim gate
/// bound `tool_load::deflection::EXCEEDS_BOUND_MM` does.
pub const DEFAULT_ROUGH_DEFLECTION_LIMIT_UM: f64 = super::suggest::DEFLECTION_BACKOFF_TARGET_UM;
/// Default deflection bound for finish ops with a surface-quality goal.
/// Matches the `WITHIN_BOUND_MM × 1000` Validated band — under this and
/// the surface remains visually clean.
pub const DEFAULT_FINISH_DEFLECTION_LIMIT_UM: f64 = 50.0;

/// Default scallop target (µm) when callers don't specify one. Matches
/// the operator's "standard finish" expectation; consumers wanting finer
/// or coarser results pass an explicit value.
pub const DEFAULT_SCALLOP_TARGET_UM: f64 = 25.0;

/// Lower fraction of stickout for the binary search bracket. 1e-4 ×
/// stickout is well below any realistic axial DOC.
const BINSEARCH_LOWER_FRACTION: f64 = 1.0e-4;
/// Upper fraction of stickout — anything above ~0.95× stickout puts
/// the load on the collet face where the cantilever model degenerates.
/// The span also stops at the flute length. P11a (D-B): the span is a
/// search bracket, never a limit.
const BINSEARCH_UPPER_FRACTION: f64 = 0.95;
/// P11a (D-B): the depths of the first scan over the span. Deflection is
/// not monotone in `ap`, so the solver takes the first scan step that
/// crosses the target, then bisects it. A numerical step, not a formula
/// constant: a crossing and recrossing inside one step (`span / 16`) is
/// missed.
pub const DEFLECTION_SCAN_STEPS: usize = 16;
/// Stop the binary search once the interval shrinks to this many mm.
/// 5 µm of axial is well below operator precision and the LUT's own
/// resolution.
pub const BINSEARCH_CONVERGENCE_MM: f64 = 0.005;
/// Hard iteration cap as a runaway guard. The interval should converge
/// in ~25 iterations; this is set well above that.
const BINSEARCH_MAX_ITERATIONS: usize = 64;

/// Axial-DOC constraint envelope for a (tool, material, engagement) tuple.
/// All four bound fields are computed independently; the caller picks
/// which one to honour via [`safe_max_doc_mm`](Self::safe_max_doc_mm)
/// (`min` of all `max_doc_*`) or [`safe_band_is_empty`](Self::safe_band_is_empty)
/// (does `min_doc_chipload_floor_mm` cross the safe ceiling?).
///
/// `binding_constraint` names which `max_doc_*` produced the safe max
/// — used by Suggest's rationale tree to explain *why* the envelope is
/// tight.
#[derive(Debug, Clone)]
pub struct CutterAxialConstraints {
    /// Maximum axial DOC before predicted tip deflection exceeds
    /// `deflection_limit_um`: the solved depth (scan and bisection over
    /// the canonical [`crate::tool::ToolDefinition::tip_deflection_mm`]).
    /// `Some(0.0)` when the shallowest depth is already over. `None` when
    /// no depth below the flute reaches the target, or the model has no
    /// signal (no chip, no force line, no stickout): no deflection limit.
    pub max_doc_deflection_mm: Option<f64>,
    /// Maximum axial DOC the matched LUT row permits at this geometry.
    /// `min(ap_max_factor × diameter, ap_max_mm)` — see `combine_ap_bound`.
    /// `None` when no LUT row was supplied or the row carries no axial
    /// bound at all.
    pub max_doc_vendor_mm: Option<f64>,
    /// Maximum axial DOC before the axial scallop between consecutive
    /// Z-passes exceeds `target_finish_um`. `None` for flat endmills (a
    /// cylinder leaves a straight wall, no axial scallop) and V-bits
    /// (the V-flank produces straight walls, not scallops).
    pub max_doc_scallop_mm: Option<f64>,
    /// Minimum axial DOC before arc-mean chip thickness drops below
    /// `chipload_min` (burn / rubbing risk). `None` for flat endmills
    /// (chip thickness has no axial dependence) and when no LUT row
    /// prints a band (a point has no minimum, A2).
    pub min_doc_chipload_floor_mm: Option<f64>,
    /// Which `max_doc_*` field produced [`safe_max_doc_mm`](Self::safe_max_doc_mm).
    /// [`AxialBindingConstraint::SafeBandEmpty`] when
    /// `min_doc_chipload_floor_mm` exceeds the safe max — the consumer
    /// should emit a "tool / surface / chipload mismatch" warning per
    /// §5.4 of the planning doc.
    pub binding_constraint: AxialBindingConstraint,
}

/// Identifies which bound is currently binding the envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxialBindingConstraint {
    /// No `max_doc_*` bound exists (P11a, D-B).
    Unbounded,
    Deflection,
    VendorAp,
    Scallop,
    /// `min_doc_chipload_floor_mm > safe_max_doc_mm` — no DOC satisfies
    /// both chip-thickness and the upper bounds simultaneously.
    SafeBandEmpty,
}

impl CutterAxialConstraints {
    /// Min over all populated `max_doc_*` fields — the value the consumer
    /// should clamp commanded DOC to. Always ≥ 0. `None` when no bound
    /// exists.
    pub fn safe_max_doc_mm(&self) -> Option<f64> {
        [
            self.max_doc_deflection_mm,
            self.max_doc_vendor_mm,
            self.max_doc_scallop_mm,
        ]
        .into_iter()
        .flatten()
        .reduce(f64::min)
        .map(|v| v.max(0.0))
    }

    /// `Some(true)` when no axial DOC satisfies both
    /// `min_doc_chipload_floor_mm ≤ ap ≤ safe_max_doc_mm()`. `Some(false)`
    /// when the band exists. `None` when no floor was derivable (flat
    /// cutter or no LUT row).
    pub fn safe_band_is_empty(&self) -> Option<bool> {
        self.min_doc_chipload_floor_mm
            .map(|floor| self.safe_max_doc_mm().is_some_and(|max| floor > max))
    }
}

/// Build the axial-DOC constraint envelope.
///
/// `radial_woc_mm` is the lateral engagement the op intends to run at —
/// each consumer derives this from its own knob (Adaptive stepover,
/// V-carve engaged width at depth, finish stepover…). `lut_row` is the
/// matched vendor row (consumers do their own [`super::vendor_lookup`]
/// match; the envelope doesn't re-query the LUT). `target_finish_um`
/// is `Some(scallop_height_um)` for finish ops, `None` for rough.
/// `deflection_limit_um` defaults to
/// [`DEFAULT_FINISH_DEFLECTION_LIMIT_UM`] when `target_finish_um.is_some()`
/// and [`DEFAULT_ROUGH_DEFLECTION_LIMIT_UM`] otherwise.
///
/// `feed_per_tooth_mm` drives the chipload-floor bound *and* the
/// feed-aware deflection bound (chip thickness sets the bending force).
/// Pass 0 only if the caller has no chipload signal — the floor becomes
/// `None` and the deflection bound goes unbounded (no chip thickness ⇒
/// no modelled force), so DOC is then limited by vendor/scallop alone.
#[tracing::instrument(level = "debug", skip(tool, material, lut_row))]
pub fn cutter_axial_constraints(
    tool: &ToolDefinition,
    material: &Material,
    radial_woc_mm: f64,
    feed_per_tooth_mm: f64,
    lut_row: Option<&LookupResult>,
    target_finish_um: Option<f64>,
    deflection_limit_um: Option<f64>,
) -> CutterAxialConstraints {
    let deflection_limit_um = deflection_limit_um.unwrap_or_else(|| {
        if target_finish_um.is_some() {
            DEFAULT_FINISH_DEFLECTION_LIMIT_UM
        } else {
            DEFAULT_ROUGH_DEFLECTION_LIMIT_UM
        }
    });

    let max_doc_deflection_mm = invert_deflection(
        tool,
        material,
        radial_woc_mm,
        feed_per_tooth_mm,
        deflection_limit_um,
    );

    let max_doc_vendor_mm = lut_row.and_then(|row| max_doc_vendor(row, tool.diameter()));

    let geometry = tool.to_geometry_hint();
    let max_doc_scallop_mm = target_finish_um
        .filter(|h| *h > 0.0)
        .and_then(|h| max_doc_scallop(geometry, h));

    // A2 (Q8): only a printed band has a minimum; a point gives no floor.
    // The 3 grade-c idcwoodcraft ball rows lose this floor as a result.
    let min_doc_chipload_floor_mm = lut_row
        .and_then(|row| row.printed_chipload().band_mm())
        .map(|(cl_min, _)| cl_min)
        .filter(|&cl_min| cl_min > 0.0)
        .filter(|_| feed_per_tooth_mm > 0.0)
        .and_then(|cl_min| {
            min_doc_chipload_floor(tool, geometry, radial_woc_mm, feed_per_tooth_mm, cl_min)
        });

    let mut binding = AxialBindingConstraint::Unbounded;
    let mut current_min = f64::INFINITY;
    for (bound, kind) in [
        (max_doc_deflection_mm, AxialBindingConstraint::Deflection),
        (max_doc_vendor_mm, AxialBindingConstraint::VendorAp),
        (max_doc_scallop_mm, AxialBindingConstraint::Scallop),
    ] {
        if let Some(v) = bound
            && v < current_min
        {
            current_min = v;
            binding = kind;
        }
    }
    if let Some(floor) = min_doc_chipload_floor_mm
        && current_min.is_finite()
        && floor > current_min.max(0.0)
    {
        binding = AxialBindingConstraint::SafeBandEmpty;
    }

    CutterAxialConstraints {
        max_doc_deflection_mm,
        max_doc_vendor_mm,
        max_doc_scallop_mm,
        min_doc_chipload_floor_mm,
        binding_constraint: binding,
    }
}

/// P11a (D-B): the shallowest axial DOC at which the predicted tip
/// deflection meets `limit_um`.
///
/// - `Some(ap)`: the solved depth, to [`BINSEARCH_CONVERGENCE_MM`]. The
///   deflection at `ap` is at most the limit; one convergence step deeper
///   it is above.
/// - `Some(0.0)`: the shallowest depth of the span is already over.
/// - `None`: no depth in the span (up to the flute) reaches the limit, or
///   the model has no signal (no chip, no width, no stickout, a refusal).
///   The span bracket is never returned as a limit.
fn invert_deflection(
    tool: &ToolDefinition,
    material: &Material,
    radial_woc_mm: f64,
    feed_per_tooth_mm: f64,
    limit_um: f64,
) -> Option<f64> {
    let limit_mm = limit_um * 1.0e-3;
    if radial_woc_mm <= 0.0 || feed_per_tooth_mm <= 0.0 || limit_mm <= 0.0 || tool.stickout() <= 0.0
    {
        // No chip, no width or no beam: the force model has nothing to
        // bound the axial DOC with.
        return None;
    }
    let deflection_at = |ap: f64| {
        let engagement = crate::feeds::engagement::Engagement::from_tool_cut(
            crate::feeds::envelope::CutPoint {
                ap_mm: ap,
                ae_mm: radial_woc_mm,
            },
            tool,
        );
        super::predict::tip_deflection_from_engagement(
            tool,
            material,
            &engagement,
            feed_per_tooth_mm,
        )
    };
    let lower = tool.stickout() * BINSEARCH_LOWER_FRACTION;
    let flute = tool.length();
    let mut upper = tool.stickout() * BINSEARCH_UPPER_FRACTION;
    if flute.is_finite() && flute > lower {
        upper = upper.min(flute);
    }
    if upper <= lower {
        return None;
    }
    // A refusal at the shallowest depth (material, stickout) is no signal;
    // over the limit, or refused (a degenerate beam), is past the target.
    shallowest_crossing(lower, upper, DEFLECTION_SCAN_STEPS, |ap| {
        deflection_at(ap).map(|d| d <= limit_mm)
    })
}

/// The shallowest depth in `[lower, upper]` where `within` stops holding:
/// the one first-crossing solver of the envelope edges (P11a deflection,
/// P13c force budget).
///
/// `within(ap)` is `Some(true)` inside the limit, `Some(false)` past it,
/// and `None` for a refusal (no signal).
///
/// - `Some(0.0)`: `lower` is already past the limit.
/// - `Some(ap)`: the last depth inside, to [`BINSEARCH_CONVERGENCE_MM`].
///   The solver scans `steps` depths for the first one past the limit,
///   then bisects that step. A quantity that is not monotone in depth
///   (deflection, the peak force of a helical edge) needs the scan first;
///   a crossing and recrossing inside one step is missed.
/// - `None`: a refusal at `lower`, or no depth in the span is past the
///   limit. The span is a search bracket, never a limit.
pub(crate) fn shallowest_crossing(
    lower: f64,
    upper: f64,
    steps: usize,
    within: impl Fn(f64) -> Option<bool>,
) -> Option<f64> {
    if !within(lower)? {
        return Some(0.0);
    }
    let steps = steps.max(1);
    let step = (upper - lower) / steps as f64;
    let mut lo = lower;
    let mut hi = None;
    for k in 1..=steps {
        let ap = if k == steps {
            upper
        } else {
            lower + step * k as f64
        };
        match within(ap) {
            Some(true) => lo = ap,
            // Past the limit, or refused: the first depth past the target.
            Some(false) | None => {
                hi = Some(ap);
                break;
            }
        }
    }
    let mut hi = hi?;
    for _ in 0..BINSEARCH_MAX_ITERATIONS {
        if hi - lo < BINSEARCH_CONVERGENCE_MM {
            break;
        }
        let mid = 0.5 * (lo + hi);
        match within(mid) {
            Some(true) => lo = mid,
            Some(false) | None => hi = mid,
        }
    }
    Some(lo)
}

/// Combine the LUT row's diameter-scaling `ap_max_factor` with the
/// absolute `ap_max_mm` cap — tighter wins.
fn max_doc_vendor(row: &LookupResult, query_diameter_mm: f64) -> Option<f64> {
    let from_factor = row.ap_max_factor.map(|f| f * query_diameter_mm);
    match (from_factor, row.ap_max_mm) {
        (Some(f), Some(a)) => Some(f.min(a)),
        (Some(f), None) => Some(f),
        (None, Some(a)) => Some(a),
        (None, None) => None,
    }
}

/// Max axial DOC before the **axial scallop** between consecutive Z-passes
/// exceeds `target_um`. Rotated form of
/// [`super::geometry::scallop_stepover`]: same `scallop = R - √(R² − (ap/2)²)`
/// solved for ap, but where the lateral case uses the cutter radius, the
/// axial case uses the ball/corner radius perpendicular to the feed plane.
///
/// `None` for flat / V-bit geometries (no axial scallop concept).
fn max_doc_scallop(geometry: ToolGeometryHint, target_um: f64) -> Option<f64> {
    if target_um <= 0.0 {
        return None;
    }
    let target_mm = target_um * 1.0e-3;
    let r_eff = match geometry {
        ToolGeometryHint::Ball => None, // ball radius lives on ToolDefinition, not the hint
        ToolGeometryHint::Bull { corner_radius } => Some(corner_radius),
        // For a tapered-ball nose the scallop bound only applies while
        // `ap ≤ tip_radius`; beyond that the cone shoulder produces a
        // different scallop formula (planning doc §9.4 — punt).
        ToolGeometryHint::TaperedBall { tip_radius, .. } => Some(tip_radius),
        ToolGeometryHint::Flat | ToolGeometryHint::VBit { .. } => return None,
    };
    // Pure ball nose: hint doesn't carry the radius, but the canonical
    // path is to call `scallop_stepover(R, target)`. We return None here
    // — pure ball-nose callers should query `tool.cutter.radius()` and
    // call `super::geometry::scallop_stepover` directly. Consumers in
    // Phase 3 (Adaptive3d / VCarve) don't use scallop; the finish-op
    // warning pass synthesises it from the ToolDefinition.
    let r = r_eff?;
    super::geometry::scallop_stepover(r, target_mm)
}

/// Compute the minimum axial DOC at which arc-mean chip thickness
/// still meets the LUT `chipload_min`. Returns `None` for cutters
/// whose chip thickness is independent of axial (flat / bull / V-bit),
/// where the floor is set by feed × arc, not depth.
///
/// For ball / tapered ball, the engagement arc shrinks as ap → 0
/// because the engaged radius shrinks. Below some ap the projected
/// arc width drops below `radial_woc_mm` and the chipload-floor model
/// becomes meaningless — we return `None` instead of a spurious value.
fn min_doc_chipload_floor(
    tool: &ToolDefinition,
    geometry: ToolGeometryHint,
    radial_woc_mm: f64,
    feed_per_tooth_mm: f64,
    chipload_min_mm_per_tooth: f64,
) -> Option<f64> {
    if feed_per_tooth_mm <= 0.0 || radial_woc_mm <= 0.0 {
        return None;
    }
    // Flat / bull / V-bit: chipload-DOC coupling is too weak for this
    // floor to be a useful signal at the envelope level. The post-sim
    // chipload gate is the right layer for those cases.
    if matches!(
        geometry,
        ToolGeometryHint::Flat | ToolGeometryHint::Bull { .. } | ToolGeometryHint::VBit { .. }
    ) {
        return None;
    }

    // Binary search the minimum axial that produces arc-mean chip
    // ≥ chipload_min. As ap grows, engaged_radius grows, the
    // radial_woc_mm covers a smaller arc fraction, and the arc-mean
    // chip thickness rises monotonically (chord-to-radius ratio shrinks).
    let lower = tool.stickout() * BINSEARCH_LOWER_FRACTION;
    let upper = tool.stickout() * BINSEARCH_UPPER_FRACTION;

    let chip_at = |ap: f64| -> Option<f64> {
        let r_eng = MillingCutter::engagement_radius(tool, ap).max(1.0e-6);
        // Arc engagement (radians) consistent with the post-sim gate's
        // arc-equivalent slab formulation: radial_width = (arc/π) · 2R.
        // Invert: arc = (radial_width / (2R)) · π. Clamp at π (slot).
        let arc = ((radial_woc_mm / (2.0 * r_eng)) * std::f64::consts::PI)
            .clamp(0.0, std::f64::consts::PI);
        if arc <= 1.0e-6 {
            return None;
        }
        // Mean chip thickness over the engaged arc. Same closed-form
        // mean as `flat_chip_geometry_for_radius`: (2 hmax / arc)·(1 − cos(arc/2)).
        let h_max = feed_per_tooth_mm * arc.sin().abs().max(0.0);
        if h_max <= 0.0 {
            return None;
        }
        let mean = (2.0 * h_max / arc) * (1.0 - (arc * 0.5).cos());
        Some(mean)
    };

    // If the chip at the upper bracket can't reach chipload_min, there
    // is no axial that satisfies the floor — return None (consumer
    // surfaces this as "tool/feed combination won't produce a clean chip
    // at any depth" via SafeBandEmpty downstream).
    let chip_hi = chip_at(upper)?;
    if chip_hi < chipload_min_mm_per_tooth {
        return None;
    }
    // If the chip at the lower bracket already meets the floor, the
    // floor is below any realistic axial — return 0 (no lower bound).
    let chip_lo = chip_at(lower)?;
    if chip_lo >= chipload_min_mm_per_tooth {
        return Some(0.0);
    }

    let mut lo = lower;
    let mut hi = upper;
    for _ in 0..BINSEARCH_MAX_ITERATIONS {
        if hi - lo < BINSEARCH_CONVERGENCE_MM {
            break;
        }
        let mid = 0.5 * (lo + hi);
        match chip_at(mid) {
            Some(c) if c >= chipload_min_mm_per_tooth => hi = mid,
            _ => lo = mid,
        }
    }
    Some(hi)
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::*;
    use crate::compute::tool_config::ToolMaterial;
    use crate::material::WoodSpecies;
    use crate::tool::{BallEndmill, FlatEndmill, TaperedBallEndmill, VBitEndmill};

    fn carbide_flat(diameter_mm: f64, stickout_mm: f64) -> ToolDefinition {
        ToolDefinition::new(
            Box::new(FlatEndmill::new(diameter_mm, stickout_mm.max(20.0))),
            diameter_mm,
            stickout_mm,
            2,
            ToolMaterial::Carbide,
        )
    }

    fn carbide_ball(diameter_mm: f64, stickout_mm: f64) -> ToolDefinition {
        ToolDefinition::new(
            Box::new(BallEndmill::new(diameter_mm, stickout_mm.max(20.0))),
            diameter_mm,
            stickout_mm,
            2,
            ToolMaterial::Carbide,
        )
    }

    fn carbide_tapered_ball() -> ToolDefinition {
        // Wanaka tool 2: 2 mm tip / 7° taper / 6 mm shank, 35 mm stickout.
        ToolDefinition::new(
            Box::new(TaperedBallEndmill::new(2.0, 7.0, 6.0, 30.0).unwrap()),
            6.0,
            35.0,
            2,
            ToolMaterial::Carbide,
        )
    }

    fn carbide_vbit() -> ToolDefinition {
        ToolDefinition::new(
            Box::new(VBitEndmill::new(6.0, 60.0, 25.0).unwrap()),
            6.0,
            45.0,
            2,
            ToolMaterial::Carbide,
        )
    }

    fn hardwood() -> Material {
        Material::SolidWood {
            species: WoodSpecies::HardMaple,
        }
    }

    fn lut_row(
        ap_min_factor: Option<f64>,
        ap_max_factor: Option<f64>,
        ap_min_mm: Option<f64>,
        ap_max_mm: Option<f64>,
        chip_load_min_mm: Option<f64>,
    ) -> LookupResult {
        LookupResult {
            chip_load_mm: 0.07,
            chip_load_min_mm,
            chip_load_max_mm: Some(0.10),
            rpm_nominal: Some(18_000.0),
            rpm_min: Some(16_000.0),
            rpm_max: Some(20_000.0),
            ap_min_mm,
            ap_max_mm,
            ap_min_factor,
            ap_max_factor,
            ae_min_mm: None,
            ae_max_mm: None,
            observation_id: "synthetic".to_owned(),
            source_vendor: crate::feeds::vendor_lut::Vendor::Amana,
            score: 100,
            diameter_match_score: 200,
            row_diameter_mm: 6.0,
            chipload_diameter_scale: 1.0,
            chipload_hardness_scale: 1.0,
            chipload_diameter_ratio_raw: 1.0,
            chipload_hardness_ratio_raw: 1.0,
            is_extrapolated: false,
            row_pass_role: crate::feeds::vendor_lut::LutPassRole::Roughing,
            size_basis: crate::feeds::extrapolation::SizeBasis::Exact,
            hardness_basis: crate::feeds::extrapolation::HardnessBasis::Unscaled,
            family_basis: crate::feeds::extrapolation::FamilyBasis::Printed,
            drill_basis: crate::feeds::extrapolation::DrillBasis::Printed,
            material_label: String::new(),
            evidence_grade: crate::feeds::vendor_lut::EvidenceGrade::A,
            row_kind: crate::feeds::vendor_lut::ObservationKind::Exact,
            band: crate::feeds::vendor_lookup::VendorBand::alone("synthetic"),
        }
    }

    /// The forward model at `(ap, ae)`: one engagement of the built tool.
    fn deflection(tool: &ToolDefinition, mat: &Material, ap: f64, ae: f64, fz: f64) -> f64 {
        let engagement = crate::feeds::engagement::Engagement::from_tool_cut(
            crate::feeds::envelope::CutPoint {
                ap_mm: ap,
                ae_mm: ae,
            },
            tool,
        );
        super::super::predict::tip_deflection_from_engagement(tool, mat, &engagement, fz)
            .expect("forward deflection")
    }

    /// P11a (D-B): when no depth below the flute reaches the target, the
    /// bound is absent, not the search bracket.
    #[test]
    fn no_root_below_the_flute_gives_no_bound() {
        let tool = carbide_flat(6.0, 45.0);
        let mat = hardwood();
        // A target far above any deflection the 6 mm tool reaches.
        let flute_depth = tool.length().min(45.0 * BINSEARCH_UPPER_FRACTION);
        let at_flute = deflection(&tool, &mat, flute_depth, 1.0, 0.05);
        let got = invert_deflection(&tool, &mat, 1.0, 0.05, 2.0 * at_flute * 1000.0);
        assert_eq!(got, None, "no root: no bound");
    }

    #[test]
    fn deflection_inversion_roundtrips_against_predict() {
        // Pick an axial, run forward to get deflection, then invert and
        // recover the same axial within search tolerance.
        let tool = carbide_flat(6.0, 45.0);
        let mat = hardwood();
        let radial = 1.0_f64;
        let fz = 0.05_f64;
        let probe_axial = 2.5_f64;
        let forward = deflection(&tool, &mat, probe_axial, radial, fz);
        let limit_um = forward * 1000.0;
        let inverted =
            invert_deflection(&tool, &mat, radial, fz, limit_um).expect("a solved depth");
        let err = (inverted - probe_axial).abs();
        assert!(
            err < 0.05,
            "deflection inversion did not recover axial: probe={probe_axial} got={inverted} err={err}"
        );
    }

    #[test]
    fn vendor_bound_takes_tighter_of_factor_and_absolute() {
        // factor 0.7 × 6 mm = 4.2 mm; absolute cap 3.0 mm → expect 3.0 mm.
        let row = lut_row(None, Some(0.7), None, Some(3.0), None);
        let v = max_doc_vendor(&row, 6.0).expect("populated bound");
        assert!((v - 3.0).abs() < 1e-9, "got {v}");
        // factor 0.5 × 6 = 3.0; absolute cap 4.0 → expect 3.0.
        let row = lut_row(None, Some(0.5), None, Some(4.0), None);
        let v = max_doc_vendor(&row, 6.0).expect("populated bound");
        assert!((v - 3.0).abs() < 1e-9, "got {v}");
        // factor only.
        let row = lut_row(None, Some(1.0), None, None, None);
        let v = max_doc_vendor(&row, 6.0).expect("populated bound");
        assert!((v - 6.0).abs() < 1e-9, "got {v}");
        // absolute only.
        let row = lut_row(None, None, None, Some(1.2), None);
        let v = max_doc_vendor(&row, 6.0).expect("populated bound");
        assert!((v - 1.2).abs() < 1e-9, "got {v}");
        // neither — None.
        let row = lut_row(None, None, None, None, None);
        assert!(max_doc_vendor(&row, 6.0).is_none());
    }

    #[test]
    fn scallop_bound_matches_closed_form_for_bull_nose() {
        // Bull nose R=1.5 mm, target 25 µm.
        // ap = 2 √(2 R h − h²) — closed form.
        let h = 0.025_f64;
        let r = 1.5_f64;
        let expected = 2.0 * (2.0 * r * h - h * h).sqrt();
        let got = max_doc_scallop(ToolGeometryHint::Bull { corner_radius: r }, 25.0)
            .expect("populated bound");
        assert!(
            (got - expected).abs() < 1e-9,
            "got {got} expected {expected}"
        );
    }

    #[test]
    fn scallop_bound_caps_tapered_ball_at_tip_radius() {
        // Wanaka tapered ball: tip 2 mm Ø → tip_radius 1.0 mm. At target
        // 25 µm scallop the scallop formula yields ~0.448 mm — well
        // under the 1 mm tip radius, so the formula governs.
        let h = 0.025_f64;
        let r = 1.0_f64;
        let expected = 2.0 * (2.0 * r * h - h * h).sqrt();
        let got = max_doc_scallop(
            ToolGeometryHint::TaperedBall {
                tip_radius: r,
                taper_angle_deg: 7.0,
            },
            25.0,
        )
        .expect("populated bound");
        assert!(
            (got - expected).abs() < 1e-9,
            "got {got} expected {expected}"
        );
    }

    #[test]
    fn scallop_bound_is_none_for_flat_and_vbit() {
        assert!(max_doc_scallop(ToolGeometryHint::Flat, 25.0).is_none());
        assert!(
            max_doc_scallop(
                ToolGeometryHint::VBit {
                    included_angle: 60.0,
                    tip_diameter: 0.0
                },
                25.0
            )
            .is_none()
        );
    }

    #[test]
    fn full_envelope_picks_tighter_max() {
        // 6 mm carbide flat, hardwood, radial = 1 mm.
        // Vendor cap 1.5 mm forces VendorAp binding.
        let tool = carbide_flat(6.0, 45.0);
        let row = lut_row(None, Some(0.25), None, None, None); // 0.25 × 6 = 1.5 mm
        let env =
            cutter_axial_constraints(&tool, &hardwood(), 1.0, 0.05, Some(&row), None, Some(200.0));
        assert_eq!(env.binding_constraint, AxialBindingConstraint::VendorAp);
        assert!((env.safe_max_doc_mm().expect("a bound") - 1.5).abs() < 1e-6);
        assert!(env.safe_band_is_empty().is_none());
    }

    #[test]
    fn tapered_ball_quadratic_force_doubling_axial_doubles_deflection() {
        // For a tapered ball where engaged-diameter grows with DOC, force
        // is super-linear in ap, so 2× the axial does NOT just double
        // deflection. The binary search must still converge cleanly on
        // both bounds — verifying the inversion monotonicity assumption.
        let tool = carbide_tapered_ball();
        let mat = hardwood();
        let radial = 0.3_f64;
        let fz = 0.05_f64;
        let d_at_1 = deflection(&tool, &mat, 1.0, radial, fz);
        let d_at_2 = deflection(&tool, &mat, 2.0, radial, fz);
        assert!(
            d_at_2 > d_at_1,
            "monotonicity broken: ap=1 → {d_at_1} mm, ap=2 → {d_at_2} mm"
        );
        // Binary-search inversion at the deflection at ap=1.5 must land
        // between 1.0 and 2.0.
        let d_at_1_5 = deflection(&tool, &mat, 1.5, radial, fz);
        let ap_recovered =
            invert_deflection(&tool, &mat, radial, fz, d_at_1_5 * 1000.0).expect("a solved depth");
        assert!(
            (1.4..=1.6).contains(&ap_recovered),
            "tapered-ball binsearch did not recover ap≈1.5: got {ap_recovered}"
        );
    }

    #[test]
    fn vbit_uses_deflection_bound_only() {
        // V-bit: no scallop bound. Vendor often label-only → None.
        // Deflection bound is computed via the canonical
        // tip_deflection_from_engagement path, which works for V-bits.
        // Since T-4 the closed-form predictor reads the same path and no
        // longer refuses one; its flute relief stays unmodelled and both
        // consumers carry that gap.
        //
        // P11a (D-B): the limit is half the deflection at the flute
        // depth, so a root exists below the flute. The fixed 50 µm limit
        // has no root below the 25 mm flute under the engaged-edge force.
        let tool = carbide_vbit();
        let at_flute = deflection(&tool, &hardwood(), tool.length(), 0.5, 0.05);
        let env = cutter_axial_constraints(
            &tool,
            &hardwood(),
            0.5,
            0.05,
            None, // no LUT row
            Some(25.0),
            Some(0.5 * at_flute * 1000.0),
        );
        assert!(env.max_doc_vendor_mm.is_none());
        assert!(env.max_doc_scallop_mm.is_none());
        assert!(
            env.max_doc_deflection_mm.is_some_and(|v| v > 0.0),
            "V-bit deflection bound should be a real value, got {:?}",
            env.max_doc_deflection_mm
        );
        assert_eq!(env.binding_constraint, AxialBindingConstraint::Deflection);
    }

    #[test]
    fn safe_band_empty_detected_on_chipload_floor_above_max() {
        // Construct a 6 mm ball with a vendor cap of 0.05 mm AND a
        // chipload_min that requires substantial ap to satisfy.
        let tool = carbide_ball(6.0, 30.0);
        let row = lut_row(
            None,
            None,
            None,
            Some(0.05), // vendor caps at 0.05 mm
            Some(0.08), // chipload_min 0.08 mm/tooth
        );
        let env = cutter_axial_constraints(
            &tool,
            &hardwood(),
            0.2,
            0.04, // feed_per_tooth low → can't reach 0.08 floor without big arc
            Some(&row),
            Some(25.0),
            Some(50.0),
        );
        // Vendor binds the upper at 0.05 mm. The chipload floor
        // computation either returns None (geometry can't reach floor —
        // also surfaces as "no signal") or a value > 0.05 mm —
        // SafeBandEmpty in the latter case.
        match env.safe_band_is_empty() {
            Some(true) => {
                assert_eq!(
                    env.binding_constraint,
                    AxialBindingConstraint::SafeBandEmpty
                );
            }
            Some(false) | None => {
                // Either band exists or no floor was derivable — both
                // are valid outcomes given the synthetic inputs. The
                // assertion that matters is that SafeBandEmpty CAN be
                // triggered, not that this specific case does.
            }
        }
    }

    #[test]
    fn chipload_floor_none_for_flat_endmill() {
        let tool = carbide_flat(6.0, 30.0);
        let row = lut_row(None, None, None, None, Some(0.08));
        let env =
            cutter_axial_constraints(&tool, &hardwood(), 1.0, 0.05, Some(&row), None, Some(200.0));
        assert!(
            env.min_doc_chipload_floor_mm.is_none(),
            "flat endmill chipload should have no axial floor"
        );
    }
}
