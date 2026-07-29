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

use std::fmt;
use std::iter::Sum;
use std::ops::{Add, AddAssign, Div};

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

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;

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
}
