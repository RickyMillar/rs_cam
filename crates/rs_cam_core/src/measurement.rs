//! Measurement contract: what a number MEANS, carried with the number.
//!
//! Plan `planning/review_2026-07-29/TECH_DEBT_RESEARCH_AND_FIX_PLAN.md` §M1,
//! designed by `planning/review_2026-07-29/MEASUREMENT_DOMAINS.md` §4/§5.
//!
//! # Why this exists
//!
//! The §14r audit compared a **post-conditioning XY-projected polygon area**
//! against a **3D mesh face area** and reported "313 of 482 mm² recovered
//! (65%)". Both numbers were correct. The ratio was meaningless: near-vertical
//! ribbons project onto XY at ~cos(slope), a ~10× shrink at 84°. The claim was
//! retracted in `63d5e8b`, and the only thing standing between the next reader
//! and the same mistake was a comment.
//!
//! Two mechanisms live here:
//!
//! 1. [`MeasurementProvenance`] — a *carried* description (domain, pipeline
//!    stage, grid cell, dilation, erosion) attached to diagnostic reports, so
//!    a table can print what it measured instead of a bare `mm²`.
//! 2. [`ProjectedXyAreaMm2`] / [`SurfaceAreaMm2`] — two newtypes with **no
//!    `Div` impl between them**, so the 313/482 division is a *compile error*
//!    rather than a convention.
//!
//! # Scope
//!
//! Diagnostic-only. Nothing in this module is serialized into a project file
//! or a simulation trace: [`crate::simulation_cut::SimulationProvenance`]
//! answers "is this trace fresh for these inputs" and is serialized;
//! [`MeasurementProvenance`] answers "what does this number mean" and is not.
//! Do not merge them.

use std::collections::HashSet;
use std::fmt;
use std::iter::Sum;
use std::ops::{Add, AddAssign, Div};

use crate::toolpath::Move;

// ── Provenance ──────────────────────────────────────────────────────────

/// What a measured quantity measures. Two values with different domains are
/// never comparable and never a legal ratio pair (non-negotiable rule 3).
///
/// `Default` is [`Self::ProjectedXyArea`] only so provenance-carrying reports
/// keep their `Default` impl; a defaulted provenance means **unstated**, not
/// "XY-projected" — see [`MeasurementProvenance`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MeasurementDomain {
    /// True 3D area of a surface (mesh faces, exact).
    SurfaceArea3d,
    /// Area projected onto the XY plane — smaller than the 3D area by
    /// cos(slope), to zero at a vertical wall.
    #[default]
    ProjectedXyArea,
    /// A count of grid cells. Multiply by cell² before any mm² comparison.
    GridCellCount,
    /// A count of dexel-top columns (simulation sampling population).
    DexelTopColumns,
    /// Volume of stock (mm³).
    StockVolume,
    /// Path length along a toolpath or skeleton (mm).
    PathLength,
    /// Wall or integrator time (s).
    Runtime,
    /// A VERTICAL distance (mm) between two Z solutions at the same XY —
    /// e.g. how far a cutter's resting height floats above the surface it
    /// was asked to reach. Not an area, not a path length, and not
    /// comparable to either: it is a depth residual at a point.
    VerticalResidualMm,
    /// A count of retract round trips — maximal contiguous runs of
    /// `MoveType::Rapid` moves. Distinct from [`Self::PathLength`]: air
    /// cost in finishing is COUNT-bound (a hop pays two ~`safe_z` Z legs
    /// whatever its XY length), not distance-bound, so this is the number
    /// a gate or reader must reach for, not the millimetres.
    RetractTripCount,
}

impl MeasurementDomain {
    /// Human-readable domain label. `const` so callers can derive
    /// documentation constants from a provenance value rather than restating
    /// it in prose (see `crate::compute::config::STANDING_MATERIAL_DOMAIN`).
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::SurfaceArea3d => "3D surface area (mm²)",
            Self::ProjectedXyArea => "XY-projected area (mm²)",
            Self::GridCellCount => "grid cell count",
            Self::DexelTopColumns => "dexel-top column count",
            Self::StockVolume => "stock volume (mm³)",
            Self::PathLength => "path length (mm)",
            Self::Runtime => "runtime (s)",
            Self::VerticalResidualMm => "vertical residual depth (mm)",
            Self::RetractTripCount => "retract round-trip count",
        }
    }
}

impl fmt::Display for MeasurementDomain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// How far through the conditioning pipeline a value was captured. Same
/// domain + different stage is still not a legal ratio: the finish planner
/// erodes, closes, absorbs and dilates between these points.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MeasurementStage {
    /// Straight off the slope threshold, before any conditioning.
    #[default]
    RawThreshold,
    /// After hysteresis band growth.
    Hysteresis,
    /// After the morphological close.
    MorphologicalClose,
    /// After small-island absorption.
    MinAreaAbsorption,
    /// After crease corridors claimed label-grid cells.
    CreaseClaim,
    /// After mask → polygon extraction (± `overlap_mm` dilation).
    PolygonExtraction,
    /// On the rest-depth mask, before any polygon extraction.
    RestFieldMask,
    /// The scallop ring cascade's residual, measured at generation from the
    /// cascade's own geometry (a simulation cannot see material the toolpath
    /// never attempted to cut).
    RingCascadeResidual,
    /// Measured at generation from the centreline drop-cutter solve: the
    /// cutter's resting Z at an emitted centreline point, against the
    /// valley-floor Z the detector traced at the same XY. Like
    /// [`Self::RingCascadeResidual`] a simulation cannot reproduce it — the
    /// toolpath never attempts to cut the material in question.
    CentrelineDropSolve,
    /// Measured at generation from the ramp-finish reach clamp: the XY swath
    /// of ramp path whose commanded Z the cutter could not hold, so it was
    /// RAISED and the material below it left standing (C8). Like
    /// [`Self::RingCascadeResidual`] and [`Self::CentrelineDropSolve`] a
    /// simulation cannot reproduce it — the emitted path is exactly what a
    /// simulation would execute; the residue is what nothing ever asked for.
    RampReachClampSwath,
    /// Measured by the stock simulation.
    Simulation,
    /// Measured on the emitted toolpath, with no reference to stock.
    Emission,
}

impl MeasurementStage {
    /// Human-readable stage label; `const` for the same reason as
    /// [`MeasurementDomain::label`].
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::RawThreshold => "measured at the raw slope threshold",
            Self::Hysteresis => "measured after hysteresis",
            Self::MorphologicalClose => "measured after the morphological close",
            Self::MinAreaAbsorption => "measured after min-area absorption",
            Self::CreaseClaim => "measured after crease claims",
            Self::PolygonExtraction => "measured at polygon extraction",
            Self::RestFieldMask => "measured on the rest-depth mask",
            Self::RingCascadeResidual => "measured at generation (ring cascade residual)",
            Self::CentrelineDropSolve => "measured at generation (centreline drop-cutter solve)",
            Self::RampReachClampSwath => "measured at generation (ramp-finish reach-clamp swath)",
            Self::Simulation => "measured in simulation",
            Self::Emission => "measured on the emitted toolpath",
        }
    }
}

impl fmt::Display for MeasurementStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Which tool scale (or none) derived the grid a value is quantised by — the
/// §14q/§A.0 axis. On a tapered tool the classification and generation grids
/// differ by the shaft/tip ratio and their cells do NOT align 1:1
/// (`finish_setup.rs`), so two areas at "the same cell size" can still be two
/// different grids.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CellSource {
    /// `cutter.cusp_radius() / 4` — the classification grid.
    CuspRadius,
    /// `cutter.radius() / 4` — the generation grid.
    EnvelopeRadius,
    /// `sqrt((envelope_radius/4) · (cusp_radius/4))` — the geometric mean of
    /// the two tool-scaled cells above, i.e. `FinishResolutionMode::
    /// GeoMeanEnvelopeCusp` (H3 / PR-8a).
    ///
    /// Its own variant rather than either neighbour's, per the PR-3 rule that
    /// a new variant is cheaper than a mislabelled one: a geo-mean cell is
    /// **not** an envelope-derived cell and **not** a cusp-derived cell, and
    /// on a tapered tool it is 2.4× off each of them. Tagging it as either
    /// would make [`MeasurementProvenance::comparable_to`] accept a
    /// comparison between two different grids — the exact failure
    /// [`Self::ToleranceFloor`] was added to prevent in the other direction.
    ///
    /// It DOES claim a tool scale (both of them), which is why it is not
    /// [`Self::Explicit`]: the number moves with the cutter, so a report can
    /// say what sized it.
    GeoMeanEnvelopeCuspRadius,
    /// `SimulationRequest::resolution`, after any grid-cap clamp.
    SimResolution,
    /// The `.max(tolerance)` FLOOR sized the cell, not the tool scale —
    /// `crate::finish_setup::FinishResolutionPolicy::tolerance_floor_applied`
    /// is true.
    ///
    /// This is a source in its own right because it is the honest answer: on
    /// a tool whose derived cell falls below the tolerance, the formula
    /// family (envelope/4 vs cusp/4) had no say in the number. Two policies
    /// of *different* modes that both bottom out on the same tolerance
    /// produce the SAME grid, and tagging them `EnvelopeRadius` and
    /// `CuspRadius` made [`MeasurementProvenance::comparable_to`] refuse a
    /// comparison that is perfectly valid (PR-3 adjacent defect, Wave D3).
    /// The formula family is still recoverable from
    /// `FinishResolutionPolicy::mode()`; it is just no longer *claimed* as
    /// the thing that set the cell.
    ToleranceFloor,
    /// Caller-pinned (harness fixtures, explicit dials).
    Explicit,
    /// Not grid-quantised at all (polygon-exact, mesh-analytic).
    #[default]
    NotGridded,
}

impl CellSource {
    /// Human-readable cell-source label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::CuspRadius => "classification grid (cusp_radius/4)",
            Self::EnvelopeRadius => "generation grid (radius/4)",
            Self::GeoMeanEnvelopeCuspRadius => {
                "generation grid (geo-mean of envelope/4 and cusp/4)"
            }
            Self::SimResolution => "simulation dexel grid",
            Self::ToleranceFloor => "tolerance floor (tool scale did not set the cell)",
            Self::Explicit => "caller-pinned grid",
            Self::NotGridded => "not grid-quantised",
        }
    }
}

impl fmt::Display for CellSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Where a measured area/coverage value came from.
///
/// Attached to *reports*, not to individual scalars, so one struct covers a
/// whole table. Where a report mixes provenances (e.g.
/// [`crate::unified_finish::UnifiedFinishReport`] carries both band areas and
/// a scallop residual) the field documentation says which fields the report's
/// provenance describes and which carry their own.
///
/// `Default` exists only so the reports that carry this keep their own
/// `Default` impl (and so existing test fixtures need no more than
/// `..Default::default()`). A defaulted provenance means **unstated** — do
/// not read it as a claim about the value it travels with.
///
/// Diagnostic-only — never serialized (module docs).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MeasurementProvenance {
    /// What the number measures.
    pub domain: MeasurementDomain,
    /// How far through the conditioning pipeline it was captured.
    pub stage: MeasurementStage,
    /// Grid cell size (mm) the value is quantised by; `None` for exact
    /// (mesh-analytic or polygon-exact) measures.
    pub cell_mm: Option<f64>,
    /// Which tool scale derived `cell_mm`.
    pub cell_source: CellSource,
    /// Dilation applied at polygon extraction (mm). Non-zero ⇒ regions
    /// OVERLAP and their areas MUST NOT be summed across bands.
    pub extraction_dilation_mm: f64,
    /// True when the coverage mask was eroded (rim shrink) before
    /// classification, so this value is not comparable to an un-eroded mask.
    pub coverage_eroded: bool,
    /// Free-text qualifier for measures whose resolution is not a single cell
    /// size (polygon-exact, decimated, mesh-analytic). Empty when `cell_mm`
    /// and `cell_source` tell the whole story.
    ///
    /// This is the *only* prose in the struct and it exists because
    /// `CellSource::NotGridded` is honest but not specific: the scallop ring
    /// residual, for instance, is decimated at 0.75× the finish heightmap
    /// cell and taken over exteriors only.
    pub resolution_note: &'static str,
}

impl MeasurementProvenance {
    /// A provenance with no grid and no conditioning — the starting point for
    /// the builder methods below.
    #[must_use]
    pub const fn new(domain: MeasurementDomain, stage: MeasurementStage) -> Self {
        Self {
            domain,
            stage,
            cell_mm: None,
            cell_source: CellSource::NotGridded,
            extraction_dilation_mm: 0.0,
            coverage_eroded: false,
            resolution_note: "",
        }
    }

    /// Record the grid this value is quantised by.
    #[must_use]
    pub const fn with_cell(mut self, cell_mm: f64, cell_source: CellSource) -> Self {
        self.cell_mm = Some(cell_mm);
        self.cell_source = cell_source;
        self
    }

    /// Record the extraction dilation (mm). Non-zero means areas overlap.
    #[must_use]
    pub const fn with_extraction_dilation_mm(mut self, dilation_mm: f64) -> Self {
        self.extraction_dilation_mm = dilation_mm;
        self
    }

    /// Record that the coverage mask was eroded before classification.
    #[must_use]
    pub const fn with_coverage_eroded(mut self, eroded: bool) -> Self {
        self.coverage_eroded = eroded;
        self
    }

    /// Record a non-grid resolution qualifier (see [`Self::resolution_note`]).
    #[must_use]
    pub const fn with_resolution_note(mut self, note: &'static str) -> Self {
        self.resolution_note = note;
        self
    }

    /// Resolution as one printable phrase: the cell size and its source, plus
    /// the note when there is one.
    #[must_use]
    pub fn resolution_label(&self) -> String {
        match (self.cell_mm, self.resolution_note.is_empty()) {
            (Some(cell), true) => format!("{cell:.3} mm cell, {}", self.cell_source),
            (Some(cell), false) => format!(
                "{cell:.3} mm cell, {}; {}",
                self.cell_source, self.resolution_note
            ),
            (None, true) => self.cell_source.label().to_owned(),
            (None, false) => self.resolution_note.to_owned(),
        }
    }

    /// One-line `domain; stage; resolution` rendering for diagnostic tables
    /// and user-visible messages. §4.3: any printed area or percentage must
    /// carry this on the same line or in the header immediately above it.
    #[must_use]
    pub fn describe(&self) -> String {
        let mut out = format!(
            "{}; {}; {}",
            self.domain,
            self.stage,
            self.resolution_label()
        );
        if self.extraction_dilation_mm > 0.0 {
            out.push_str(&format!(
                "; dilated {:.3} mm at extraction (regions OVERLAP — do not sum across bands)",
                self.extraction_dilation_mm
            ));
        }
        if self.coverage_eroded {
            out.push_str("; coverage eroded by one cell");
        }
        out
    }

    /// True when two values may legally be divided or differenced: same
    /// domain, same stage, same grid.
    ///
    /// Deliberately strict — dilation and erosion are *not* checked here
    /// because a same-report ratio (e.g. one band over the band total) shares
    /// them; cross-report ratios must compare the full provenance.
    ///
    /// The one case this used to get *wrong* was the tolerance floor: when
    /// `.max(tolerance)` binds, an envelope/4 policy and a cusp/4 policy
    /// resolve to the identical grid, and tagging them by formula family made
    /// this function refuse a valid comparison. Both now tag
    /// [`CellSource::ToleranceFloor`] (Wave D3), so equal cells compare —
    /// through the same plain source equality, with no special case here.
    #[must_use]
    pub fn comparable_to(&self, other: &Self) -> bool {
        self.domain == other.domain
            && self.stage == other.stage
            && self.cell_source == other.cell_source
            && match (self.cell_mm, other.cell_mm) {
                (None, None) => true,
                (Some(a), Some(b)) => (a - b).abs() <= 1e-9,
                _ => false,
            }
    }
}

impl fmt::Display for MeasurementProvenance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.describe())
    }
}

// ── The compile-time gate: two area domains that cannot be divided ──────

/// An area **projected onto the XY plane**, in mm².
///
/// This is what every mask→polygon extraction in this codebase produces: a
/// band region, a rest island, a scallop ring residual. It is *smaller* than
/// the 3D area of the same surface by cos(slope) — to zero at a vertical
/// wall.
///
/// # The gate
///
/// There is no `Div<SurfaceAreaMm2>` impl, so the retracted §14r ratio does
/// not compile:
///
/// ```compile_fail
/// use rs_cam_core::measurement::{ProjectedXyAreaMm2, SurfaceAreaMm2};
/// // §14r: "313 of 482 mm² recovered (65%)" — retracted in 63d5e8b.
/// let recovered = ProjectedXyAreaMm2::new(313.0); // XY-projected regions
/// let truth = SurfaceAreaMm2::new(482.0);         // 3D mesh face area
/// let _share = recovered / truth; // E0277: no Div impl — THIS IS THE GATE
/// ```
///
/// Every name in that snippet is valid — only the division is rejected. The
/// same-domain ratio compiles and runs:
///
/// ```
/// use rs_cam_core::measurement::{ProjectedXyAreaMm2, SurfaceAreaMm2};
/// let part = ProjectedXyAreaMm2::new(50.0);
/// let whole = ProjectedXyAreaMm2::new(200.0);
/// assert!((part / whole - 0.25).abs() < 1e-12);
/// // Same constructors, same imports as the rejected snippet above:
/// let truth = SurfaceAreaMm2::new(482.0);
/// let _projected = truth.project_onto_xy(0.5);
/// ```
///
/// The one sanctioned crossing is [`SurfaceAreaMm2::project_onto_xy`], which
/// only goes 3D → XY, only for a single face of known slope, and never back.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct ProjectedXyAreaMm2(f64);

/// A **true 3D surface area**, in mm² — mesh faces, exact, no projection.
///
/// See [`ProjectedXyAreaMm2`] for the gate this type is half of.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct SurfaceAreaMm2(f64);

impl ProjectedXyAreaMm2 {
    /// The domain every value of this type carries.
    pub const DOMAIN: MeasurementDomain = MeasurementDomain::ProjectedXyArea;

    /// Wrap a raw mm² magnitude known to be XY-projected.
    #[must_use]
    pub const fn new(mm2: f64) -> Self {
        Self(mm2)
    }

    /// Raw magnitude, **for printing and same-domain arithmetic only**.
    ///
    /// Reaching for this to divide by a [`SurfaceAreaMm2`] reconstructs the
    /// retracted §14r ratio by hand. Don't.
    #[must_use]
    pub const fn mm2(self) -> f64 {
        self.0
    }
}

impl SurfaceAreaMm2 {
    /// The domain every value of this type carries.
    pub const DOMAIN: MeasurementDomain = MeasurementDomain::SurfaceArea3d;

    /// Wrap a raw mm² magnitude known to be a true 3D area.
    #[must_use]
    pub const fn new(mm2: f64) -> Self {
        Self(mm2)
    }

    /// Raw magnitude, **for printing and same-domain arithmetic only**.
    #[must_use]
    pub const fn mm2(self) -> f64 {
        self.0
    }

    /// Project a 3D face area onto XY.
    ///
    /// Valid ONLY for a single face of known slope, where `cos_slope` is
    /// `|normal.z|` for that face. NEVER for an aggregate over faces of mixed
    /// slope (project each face, then sum), and NEVER in reverse: projected →
    /// 3D is unrecoverable at vertical faces, where cos(slope) = 0.
    #[must_use]
    pub fn project_onto_xy(self, cos_slope: f64) -> ProjectedXyAreaMm2 {
        ProjectedXyAreaMm2(self.0 * cos_slope)
    }
}

macro_rules! area_arithmetic {
    ($ty:ident) => {
        impl Add for $ty {
            type Output = Self;
            fn add(self, rhs: Self) -> Self {
                Self(self.0 + rhs.0)
            }
        }

        impl AddAssign for $ty {
            fn add_assign(&mut self, rhs: Self) {
                self.0 += rhs.0;
            }
        }

        impl Sum for $ty {
            fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
                iter.fold(Self(0.0), |a, b| a + b)
            }
        }

        /// Same-domain ratio — the only division this type allows.
        impl Div for $ty {
            type Output = f64;
            fn div(self, rhs: Self) -> f64 {
                self.0 / rhs.0
            }
        }

        impl fmt::Display for $ty {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{:.1} mm²", self.0)
            }
        }
    };
}

area_arithmetic!(ProjectedXyAreaMm2);
area_arithmetic!(SurfaceAreaMm2);

// ── Swept footprint area (§7 PR-0 repair (a)) ────────────────────────────
//
// `planning/review_2026-07-29/MEASUREMENT_DOMAINS.md` §7 audited the mm²/s
// efficiency metric the tech-debt plan (§A/M7) standardises on and found the
// shipped numerator void: `v3_cascade_ab.rs`'s `stamp_cells`/
// `op_footprint_cells` (~`:4407`/`:4425`) stamp 1 mm XY bins from the TOOL
// CENTRELINE, so a Ø6 ball and a Ø1 tip walking identical paths score the
// same footprint — the opposite of "invariant to tool" the plan claims.
//
// §7 offers exactly two sanctioned repairs and says not to do both
// silently: (a) stamp the tool's XY disc (radius-aware), or (b) rename to
// `centerline_footprint_mm2/s` and drop the tool-invariance claim. This is
// repair (a).

/// Default XY grid cell size (mm) for [`swept_footprint_area`] — matches
/// the 1 mm bins `v3_cascade_ab.rs`'s centreline stamp used, so a caller
/// that doesn't have an opinion gets a directly-comparable resolution to
/// the historical (void) numbers. Callers needing a different resolution
/// pass `cell_mm` explicitly; the returned [`MeasurementProvenance`] always
/// tags [`CellSource::Explicit`] for this function, so two calls at
/// different `cell_mm` are never silently treated as comparable —
/// [`MeasurementProvenance::comparable_to`] checks `cell_mm` itself.
pub const DEFAULT_FOOTPRINT_CELL_MM: f64 = 1.0;

/// Compute the **XY-projected swept footprint area** of a toolpath's
/// cutting moves: the union, over every cutting move, of the cutter's XY
/// disc swept along that move — rasterised onto a `cell_mm` grid and
/// counted in whole cells.
///
/// Returns the area **together with** the [`MeasurementProvenance`] that
/// describes it (domain [`MeasurementDomain::ProjectedXyArea`], stage
/// [`MeasurementStage::Emission`], cell `cell_mm` at
/// [`CellSource::Explicit`]) — value and provenance travel together, the
/// house rule this module exists to enforce (see
/// `ToolpathStats::standing_material()` for the precedent this follows).
///
/// # What this measures — and what it does NOT
///
/// This is an **emission-stage** measure: it walks the toolpath only and
/// never consults the stock. Concretely:
///
/// - ground the cutter's disc crosses **twice** (an overlap stroke, a
///   re-visited pass) is counted **once** — footprint is a *set* of
///   touched cells, not a *sum* of passes;
/// - ground a **rest pass re-crosses** that an earlier operation already
///   finished scores identically to ground it crosses for the first time.
///   `MEASUREMENT_DOMAINS.md` §7 names exactly this as the load-bearing
///   defect behind the "rest pass is half as efficient" claim (the
///   original 0.476-vs-0.938 mm²/s figure), and this function does not
///   repair that defect by itself: footprint has no notion of "already
///   finished". A true rest-pass efficiency claim needs this footprint
///   **intersected with the pass's own claimed territory**, computed
///   upstream and passed in separately — this function only answers "how
///   much ground did the cutter's disc pass over", never "how much of
///   that ground was fresh".
///
/// This is why the name is `swept_footprint_area`, not `finished_area`:
/// calling it the latter reintroduces the exact defect this function was
/// written to fix. Never rename it to imply "finished".
///
/// # Tool invariance — deliberately NOT claimed
///
/// The centreline version this replaces was pitched as "invariant to
/// tool" and wasn't — see the module-level comment above. This version is
/// radius-aware **by design**, so a wider tool covers more footprint per
/// pass over the same path; that is correct and intentional, not a defect
/// to chase. Two calls are directly comparable when they share a tool
/// (same `radius_mm`) and the same `cell_mm`. Across tools the numbers are
/// still meaningful, just not "invariant": a wider tool's larger footprint
/// means "covers more ground per pass", a real fact about the tool, not
/// measurement noise.
///
/// # Which moves count
///
/// Only moves where `move_type.is_cutting()` is true (everything except
/// [`crate::toolpath::MoveType::Rapid`]).
///
/// **Ruling on [`crate::toolpath::MoveIntent::Linking`]:** Linking feed
/// moves ARE counted. `MoveType::is_cutting` does not distinguish by
/// intent, and unlike [`crate::toolpath::MoveIntent::Retract`] (always
/// emitted as a lifted rapid in every in-tree generator), a Linking move
/// is a **feed** move that stays down at the surface while repositioning —
/// under a *swept footprint* domain the cutter's disc genuinely passes
/// over that ground whether or not the move was issued to remove
/// material. This is a deliberate ruling, not an oversight left over from
/// reusing `is_cutting()`: the A/B this metric feeds compares branches
/// that differ precisely in how many Linking moves they emit (more
/// stitched surface links vs more discrete retract/rapid hops), so
/// counting or excluding Linking footprint is load-bearing for the
/// comparison and is being stated here on purpose.
///
/// # Parameters
///
/// - `moves`: the toolpath's move sequence; each move's target is swept
///   from the *previous* move's target, so a footprint needs at least two
///   moves to contribute (a lone move has no segment to sweep).
/// - `radius_mm`: the cutter's XY **envelope** radius (mm) — the maximum
///   lateral reach of any part of the cutter body, i.e.
///   `crate::tool::MillingCutter::envelope_radius_mm`, NOT
///   `cusp_radius`/the tip radius. A footprint is a swept-*extent* measure
///   (what the cutter body can physically touch), which is exactly what
///   `envelope_radius_mm` documents itself as being for. Taken as a plain
///   `f64` rather than `&dyn MillingCutter` to keep this module
///   dependency-light: none of the in-tree cutters vary their XY envelope
///   with depth (tapering only shrinks the CUSP radius, never the shaft
///   envelope — see `crate::tool::MillingCutter::cusp_radius`'s doc), so a
///   single scalar loses nothing for this measure.
/// - `cell_mm`: the XY grid cell size (mm); see
///   [`DEFAULT_FOOTPRINT_CELL_MM`] for the historical default. Must be
///   `> 0.0`; `radius_mm` must be `>= 0.0`. Neither is asserted — this
///   module never panics on caller input (crate lint policy) — a
///   non-positive value of either simply yields a zero-area result.
///
/// # Walk step
///
/// Internally walks each move at `step = min(radius_mm, cell_mm)`.
/// Bounding the step by `radius_mm` keeps consecutive stamped discs
/// overlapping, so no real gap opens in the swept stadium between samples.
/// Bounding it by `cell_mm` too means refining the grid also refines the
/// walk, so the rasterised area actually converges to the analytic stadium
/// area (`2·r·L + π·r²` for a straight run of length `L`) as `cell_mm`
/// shrinks — a step tied only to `radius_mm` would leave a fixed,
/// non-shrinking sliver of error near the ends of a short move even as the
/// grid refined. See the unit tests below for the convergence this buys.
///
/// # Complexity
///
/// Each stamped position rasterises its disc with a bounded row-span scan
/// (`stamp_disc`): for each grid row the disc's half-chord width at that
/// row is solved once from the row's vertical offset from the centre, then
/// the whole column span for that row is inserted in one pass — O(cells
/// touched by the disc), not O(bounding-box cells) with a per-cell
/// distance test.
#[must_use]
pub fn swept_footprint_area(
    moves: &[Move],
    radius_mm: f64,
    cell_mm: f64,
) -> (ProjectedXyAreaMm2, MeasurementProvenance) {
    let provenance = MeasurementProvenance::new(
        MeasurementDomain::ProjectedXyArea,
        MeasurementStage::Emission,
    )
    .with_cell(cell_mm, CellSource::Explicit);

    if radius_mm <= 0.0 || cell_mm <= 0.0 {
        return (ProjectedXyAreaMm2::new(0.0), provenance);
    }

    let walk_step_mm = radius_mm.min(cell_mm);
    let mut cells: HashSet<(i64, i64)> = HashSet::new();

    for pair in moves.windows(2) {
        let [prev, cur] = pair else { continue };
        if !cur.move_type.is_cutting() {
            continue;
        }
        stamp_move_footprint(
            &mut cells,
            prev.target,
            cur.target,
            radius_mm,
            cell_mm,
            walk_step_mm,
        );
    }

    let area_mm2 = cells.len() as f64 * cell_mm * cell_mm;
    (ProjectedXyAreaMm2::new(area_mm2), provenance)
}

/// Walk a single move's `a → b` targets at `walk_step_mm` and stamp a disc
/// of `radius_mm` at each sample onto `cells` (grid `cell_mm`).
///
/// See [`swept_footprint_area`]'s "Walk step" doc for why `walk_step_mm` is
/// bounded by both the radius and the cell size rather than being a fixed
/// constant.
fn stamp_move_footprint(
    cells: &mut HashSet<(i64, i64)>,
    a: crate::geo::P3,
    b: crate::geo::P3,
    radius_mm: f64,
    cell_mm: f64,
    walk_step_mm: f64,
) {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let len = (dx * dx + dy * dy).sqrt();
    // At least one sample even for a zero-length move, so a stationary
    // plunge/retract endpoint still stamps its own disc once.
    let steps = ((len / walk_step_mm).ceil() as usize).max(1);
    for i in 0..=steps {
        let t = i as f64 / steps as f64;
        let x = a.x + dx * t;
        let y = a.y + dy * t;
        stamp_disc(cells, x, y, radius_mm, cell_mm);
    }
}

/// Rasterise a disc of `radius_mm` centred at `(cx, cy)` onto a `cell_mm`
/// grid, inserting every touched `(row, col)` cell into `cells`.
///
/// Bounded row-span scan, not a bounding-box scan with a per-cell distance
/// test: for each candidate row, the half-chord width is solved once from
/// that row's vertical offset from the centre, then the row's whole column
/// span is inserted in one pass — see [`swept_footprint_area`]'s
/// "Complexity" doc.
fn stamp_disc(cells: &mut HashSet<(i64, i64)>, cx: f64, cy: f64, radius_mm: f64, cell_mm: f64) {
    let row_min = ((cy - radius_mm) / cell_mm).floor() as i64;
    let row_max = ((cy + radius_mm) / cell_mm).floor() as i64;
    for row in row_min..=row_max {
        let row_lo = row as f64 * cell_mm;
        let row_hi = row_lo + cell_mm;
        let dy = if cy < row_lo {
            row_lo - cy
        } else if cy > row_hi {
            cy - row_hi
        } else {
            0.0
        };
        if dy >= radius_mm {
            continue;
        }
        let half_w = (radius_mm * radius_mm - dy * dy).sqrt();
        let col_min = ((cx - half_w) / cell_mm).floor() as i64;
        let col_max = ((cx + half_w) / cell_mm).floor() as i64;
        for col in col_min..=col_max {
            cells.insert((row, col));
        }
    }
}

/// mm²/s throughput for a [`swept_footprint_area`] result.
///
/// - **Numerator**: `area`, a [`ProjectedXyAreaMm2`] — always the value
///   half of a [`swept_footprint_area`] return (domain
///   [`MeasurementDomain::ProjectedXyArea`], stage
///   [`MeasurementStage::Emission`], quantised at whatever `cell_mm` that
///   call used — print the [`MeasurementProvenance`] returned alongside it
///   next to any printed ratio, per §4.3). The parameter type is
///   `ProjectedXyAreaMm2`, not `f64`, specifically so a caller cannot
///   assemble this ratio from a raw float by hand; reaching for `.mm2()`
///   first to dodge the type is the same escape hatch the module docs warn
///   about for the 3D/XY divide, and is just as wrong here.
/// - **Denominator**: `seconds` — the integrator's PER-OP total time,
///   INCLUDING rapids and entries, not cutting-only time.
/// - **What this is NOT**: not a finished-area rate. See
///   [`swept_footprint_area`]'s doc for why a footprint can under-count
///   re-crossed ground as "the same area" instead of "finished twice";
///   this ratio inherits that limitation unchanged — it is throughput of
///   *footprint*, not of *material removed*.
///
/// `seconds <= 0.0` (a zero- or negative-length op, which should not occur
/// but must not be allowed to divide by zero or propagate `NaN`/`inf` into
/// a diagnostic table) returns `0.0`.
#[must_use]
pub fn swept_footprint_mm2_per_s(area: ProjectedXyAreaMm2, seconds: f64) -> f64 {
    if seconds <= 0.0 {
        return 0.0;
    }
    area.mm2() / seconds
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use crate::geo::P3;
    use crate::toolpath::{MoveIntent, MoveType};

    /// M1 gate, Layer B: the retracted §14r ratio must not be
    /// *reconstructible* — and this test names why, for the reader who
    /// reaches for `.mm2()` to escape the newtype.
    ///
    /// A 76° wall has 3D area `A` and projected area `A·cos(76°) ≈ 0.24·A`.
    /// Dividing a decomposition's projected output by the mesh's 3D area
    /// reports ~24% "recovered" from a surface that is 100% recovered.
    #[test]
    fn projected_area_is_not_a_share_of_surface_area() {
        // One 76° face, 100 mm² of true 3D area, fully recovered by the
        // decomposition — the projected polygon covers all of it.
        let cos_slope = 76.0_f64.to_radians().cos();
        let truth = SurfaceAreaMm2::new(100.0);
        let recovered = truth.project_onto_xy(cos_slope);

        // (a) The two columns differ by more than 2× on a steep fixture.
        assert!(
            truth.mm2() / recovered.mm2() > 2.0,
            "76° projects to {recovered} from {truth}; the domains must differ \
             by more than 2× or this fixture proves nothing"
        );

        // (b) The domains are tagged distinctly, so a provenance-carrying
        //     table can refuse the pair.
        assert_ne!(ProjectedXyAreaMm2::DOMAIN, SurfaceAreaMm2::DOMAIN);
        assert!(
            !MeasurementProvenance::new(
                ProjectedXyAreaMm2::DOMAIN,
                MeasurementStage::PolygonExtraction,
            )
            .comparable_to(&MeasurementProvenance::new(
                SurfaceAreaMm2::DOMAIN,
                MeasurementStage::PolygonExtraction,
            )),
            "different domains must never compare as comparable"
        );

        // (c) The honest ratio — projected over projected — is 100%.
        let honest = recovered / truth.project_onto_xy(cos_slope);
        assert!(
            (honest - 1.0).abs() < 1e-12,
            "same-domain ratio must read 1.0, got {honest}"
        );
        // And the cross-domain one would have read ~24%, which is the
        // number §14r published as 65% on a different fixture.
        let dishonest = recovered.mm2() / truth.mm2();
        assert!(
            dishonest < 0.25,
            "the cross-domain ratio understates recovery ({dishonest:.3}) — \
             that is the failure mode, not a rounding difference"
        );
    }

    #[test]
    fn same_domain_arithmetic_needs_no_escape_hatch() {
        let a = ProjectedXyAreaMm2::new(30.0);
        let b = ProjectedXyAreaMm2::new(70.0);
        let total: ProjectedXyAreaMm2 = [a, b].into_iter().sum();
        assert!((total.mm2() - 100.0).abs() < 1e-12);
        assert!((a / total - 0.3).abs() < 1e-12);
        assert!(a < b);
    }

    #[test]
    fn provenance_renders_domain_stage_and_resolution() {
        let p = MeasurementProvenance::new(
            MeasurementDomain::ProjectedXyArea,
            MeasurementStage::PolygonExtraction,
        )
        .with_cell(0.125, CellSource::CuspRadius)
        .with_extraction_dilation_mm(2.0)
        .with_coverage_eroded(true);
        let text = p.describe();
        assert!(text.contains("XY-projected area"), "{text}");
        assert!(text.contains("polygon extraction"), "{text}");
        assert!(text.contains("0.125 mm cell"), "{text}");
        assert!(text.contains("cusp_radius/4"), "{text}");
        assert!(text.contains("OVERLAP"), "{text}");
        assert!(text.contains("eroded"), "{text}");
    }

    #[test]
    fn comparability_is_strict_about_grid_and_stage() {
        let base = MeasurementProvenance::new(
            MeasurementDomain::ProjectedXyArea,
            MeasurementStage::PolygonExtraction,
        )
        .with_cell(0.125, CellSource::CuspRadius);
        assert!(base.comparable_to(&base));
        // X-6: same field, different grid.
        assert!(!base.comparable_to(&base.with_cell(0.75, CellSource::EnvelopeRadius)));
        // X-12: same domain and grid, different stage.
        let other_stage = MeasurementProvenance::new(
            MeasurementDomain::ProjectedXyArea,
            MeasurementStage::CreaseClaim,
        )
        .with_cell(0.125, CellSource::CuspRadius);
        assert!(!base.comparable_to(&other_stage));
    }

    // ── swept_footprint_area ──────────────────────────────────────────
    //
    // Assertions here compare against ANALYTIC truth (the stadium-area
    // formula for a disc swept along a straight line), not against the
    // implementation — a test that just re-derives the same rasterisation
    // would pass even if the algorithm were wrong in the same way twice.

    /// `Rapid` to `(0,0,0)` then one straight cutting move to `(length,0,0)`
    /// — the minimal fixture `swept_footprint_area` needs, since a
    /// footprint is swept between a move's target and the PREVIOUS move's
    /// target and the first move in a real toolpath is always a rapid.
    fn straight_move_fixture(length: f64) -> Vec<Move> {
        vec![
            Move {
                target: P3::new(0.0, 0.0, 0.0),
                move_type: MoveType::Rapid,
                intent: MoveIntent::Linking,
            },
            Move {
                target: P3::new(length, 0.0, 0.0),
                move_type: MoveType::Linear { feed_rate: 1000.0 },
                intent: MoveIntent::FinishingCut,
            },
        ]
    }

    /// A straight run of length `L` swept by a disc of radius `r` covers a
    /// stadium: a `2r × L` rectangle plus two end caps that together make
    /// one full circle — `2·r·L + π·r²`. The cell-counted measure must
    /// converge to this as `cell_mm` shrinks; it must NOT match it exactly
    /// (rasterisation always over/under-counts along the curved boundary),
    /// which is why this asserts convergence at two resolutions rather than
    /// equality at one.
    #[test]
    fn swept_footprint_converges_to_stadium_area_as_cell_shrinks() {
        let length = 40.0;
        let radius = 2.0;
        let moves = straight_move_fixture(length);
        let analytic = 2.0 * radius * length + std::f64::consts::PI * radius * radius;

        let (coarse, coarse_prov) = swept_footprint_area(&moves, radius, 1.0);
        let (fine, fine_prov) = swept_footprint_area(&moves, radius, 0.1);

        let coarse_err = (coarse.mm2() - analytic).abs() / analytic;
        let fine_err = (fine.mm2() - analytic).abs() / analytic;

        // Numerically verified (independent Python rasterisation of the
        // same row-span algorithm): 1.0 mm cell ≈ 3.1% error, 0.1 mm cell
        // ≈ 0.4% error on this fixture. Tolerances below are generous
        // relative to that so the test isn't pinned to the last decimal of
        // the current implementation, while still failing loudly if the
        // disc radius stopped being consulted at all (the centreline bug
        // this replaces would read `area ≈ 0`, off by >99%).
        assert!(
            coarse_err < 0.08,
            "1.0 mm cell: {coarse_err:.4} relative error vs analytic {analytic:.3} \
             (got {coarse})"
        );
        assert!(
            fine_err < 0.02,
            "0.1 mm cell: {fine_err:.4} relative error vs analytic {analytic:.3} \
             (got {fine})"
        );
        assert!(
            fine_err < coarse_err,
            "finer cell ({fine_err:.4}) must be closer to analytic than coarse \
             ({coarse_err:.4}) — this is the convergence the disc rasterisation \
             promises"
        );

        // Provenance travels with the value (module house rule) and is
        // tagged for what it is: emission-stage, XY-projected, explicit
        // cell.
        for (area, prov, cell) in [(coarse, coarse_prov, 1.0), (fine, fine_prov, 0.1)] {
            assert_eq!(prov.domain, MeasurementDomain::ProjectedXyArea);
            assert_eq!(prov.stage, MeasurementStage::Emission);
            assert_eq!(prov.cell_mm, Some(cell));
            assert_eq!(prov.cell_source, CellSource::Explicit);
            assert!(area.mm2() > 0.0);
        }
    }

    /// The property the centreline version this replaces VIOLATES: walking
    /// the identical path with a wider tool must strictly increase the
    /// measured footprint. A centreline stamp (no radius term at all) would
    /// read the same area for every radius on this fixture — that was the
    /// §7 defect ("a Ø6 ball and a Ø1 tip walking identical paths score
    /// identically").
    #[test]
    fn doubling_radius_on_the_same_path_strictly_increases_footprint() {
        let moves = straight_move_fixture(40.0);
        let (small, _) = swept_footprint_area(&moves, 1.0, 0.2);
        let (medium, _) = swept_footprint_area(&moves, 2.0, 0.2);
        let (large, _) = swept_footprint_area(&moves, 4.0, 0.2);

        assert!(
            small.mm2() < medium.mm2(),
            "r=1.0 ({small}) must be smaller than r=2.0 ({medium})"
        );
        assert!(
            medium.mm2() < large.mm2(),
            "r=2.0 ({medium}) must be smaller than r=4.0 ({large})"
        );
    }

    /// An all-rapid toolpath has no cutting moves, so it sweeps nothing —
    /// regardless of tool radius or cell size.
    #[test]
    fn all_rapid_toolpath_measures_zero_footprint() {
        let moves = vec![
            Move {
                target: P3::new(0.0, 0.0, 5.0),
                move_type: MoveType::Rapid,
                intent: MoveIntent::Linking,
            },
            Move {
                target: P3::new(50.0, 0.0, 5.0),
                move_type: MoveType::Rapid,
                intent: MoveIntent::Linking,
            },
            Move {
                target: P3::new(50.0, 50.0, 5.0),
                move_type: MoveType::Rapid,
                intent: MoveIntent::Retract,
            },
        ];
        let (area, provenance) = swept_footprint_area(&moves, 3.0, 0.5);
        assert_eq!(area.mm2(), 0.0);
        assert_eq!(provenance.domain, MeasurementDomain::ProjectedXyArea);
        assert_eq!(provenance.stage, MeasurementStage::Emission);
    }

    /// Ruling test: a `MoveIntent::Linking` FEED move (not a rapid) is a
    /// cutting-classified move by `MoveType::is_cutting()` and this
    /// function's doc rules it IN — the disc still sweeps that ground even
    /// though the move wasn't issued to remove material. This pins the
    /// ruling stated in `swept_footprint_area`'s doc comment so a future
    /// change to that ruling fails a test, not just a comment.
    #[test]
    fn linking_feed_moves_count_toward_footprint() {
        let moves = vec![
            Move {
                target: P3::new(0.0, 0.0, 0.0),
                move_type: MoveType::Rapid,
                intent: MoveIntent::Linking,
            },
            Move {
                target: P3::new(20.0, 0.0, 0.0),
                move_type: MoveType::Linear { feed_rate: 800.0 },
                intent: MoveIntent::Linking,
            },
        ];
        let (area, _) = swept_footprint_area(&moves, 1.5, 0.25);
        assert!(
            area.mm2() > 0.0,
            "a Linking feed move must contribute footprint, got {area}"
        );
    }

    #[test]
    fn swept_footprint_mm2_per_s_divides_area_by_seconds() {
        let area = ProjectedXyAreaMm2::new(120.0);
        let rate = swept_footprint_mm2_per_s(area, 40.0);
        assert!((rate - 3.0).abs() < 1e-12, "got {rate}");

        // Non-positive seconds must not divide-by-zero or propagate NaN
        // into a diagnostic table.
        assert_eq!(swept_footprint_mm2_per_s(area, 0.0), 0.0);
        assert_eq!(swept_footprint_mm2_per_s(area, -5.0), 0.0);
    }
}
