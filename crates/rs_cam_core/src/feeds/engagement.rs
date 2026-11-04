//! CONTRACT K7 (feeds contract P9): one engagement definition, before and
//! after the simulation.
//!
//! [`Engagement`] holds the engaged geometry of one cut: the axial depth
//! `ap`, the radial width `ae`, the arc `psi`, the engaged diameter `d_eng`
//! and the cutter's own chip section. Two constructors build it, and no
//! other code can:
//!
//! - [`Engagement::from_cut`]: a planned cut (Suggest, the envelope, the
//!   modulator). `psi` comes from [`immersion_angle`] (`cos psi = 1 - ae / r`).
//! - [`Engagement::from_sample`]: a simulation sample. The sample measures
//!   the arc; the width is the geometric inverse `ae = r (1 - cos psi)`
//!   (lead decision D-W, 2026-11-03), the exact inverse of
//!   [`immersion_angle`]. The gate's old slab `(psi / pi) x 2 r` is retired.
//!
//! The section always comes from the cutter's `mrr_cross_section_mm2` (a
//! V-bit removes half the rectangle). Every power reader builds its
//! [`PowerModelInputs`] through [`Engagement::power_inputs`]; the fields of
//! that struct are private to this file, so the compiler refuses a second
//! builder.
//!
//! Ruling Q5: the simulation constructor is a READ of a sample, for the
//! gate and for the placement on the envelope. Suggest never calls it.
//!
//! The compiler refuses a literal outside this module:
//!
//! ```compile_fail
//! let _ = rs_cam_core::feeds::engagement::Engagement {
//!     ap_mm: 1.0,
//!     ae_mm: 1.0,
//!     psi_rad: 1.0,
//!     d_eng_mm: 6.0,
//!     section_mm2: 1.0,
//!     helix_deg: None,
//! };
//! ```
//!
//! The sibling: the constructor compiles.
//!
//! ```
//! use rs_cam_core::feeds::ToolGeometryHint;
//! use rs_cam_core::feeds::engagement::Engagement;
//! use rs_cam_core::feeds::envelope::CutPoint;
//! let e = Engagement::from_cut(
//!     CutPoint { ap_mm: 1.0, ae_mm: 3.0 },
//!     ToolGeometryHint::Flat,
//!     6.0,
//!     None,
//! );
//! assert!(e.psi_rad() > 0.0);
//! ```

use serde::Serialize;

use crate::compute::catalog::{CutSpec, FirstPass, OperationConfig, Radial};
use crate::feeds::ToolGeometryHint;
use crate::feeds::envelope::CutPoint;
use crate::feeds::force::{immersion_angle, lateral_cutting_force};
use crate::material::Material;
use crate::material::force_line::ForceLine;
use crate::tool::MillingCutter;

/// CONTRACT K7. Built only by [`Self::from_cut`] and [`Self::from_sample`].
///
/// G4 (lead, 2026-11-04): `ap`, `psi`, `d_eng` and the tool's helix stay
/// reachable through accessors, so the P11 force model adds a method here,
/// not a second type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Engagement {
    ap_mm: f64,
    ae_mm: f64,
    psi_rad: f64,
    d_eng_mm: f64,
    section_mm2: f64,
    helix_deg: Option<f64>,
}

impl Engagement {
    /// A planned cut. `d_eng_mm` is the engaged diameter the caller cuts
    /// with: `feeds::effective_diameter` at the depth (Suggest), or the
    /// diameter the modulator was given. `helix_deg` is the tool's helix,
    /// when the caller knows it.
    #[must_use]
    pub fn from_cut(
        cut: CutPoint,
        tool: ToolGeometryHint,
        d_eng_mm: f64,
        helix_deg: Option<f64>,
    ) -> Self {
        Self {
            ap_mm: cut.ap_mm,
            ae_mm: cut.ae_mm,
            psi_rad: immersion_angle(cut.ae_mm, d_eng_mm / 2.0),
            d_eng_mm,
            section_mm2: tool.mrr_cross_section_mm2(cut.ap_mm, cut.ae_mm),
            helix_deg,
        }
    }

    /// A simulation sample. `None` when the sample removes no material,
    /// carries no arc, or gives no positive width.
    ///
    /// The engaged diameter is `2 x tool.engagement_radius(ap)`. The arc is
    /// the sample's own reading; the width is `r (1 - cos psi)` with psi
    /// clamped to a full slot (D-W).
    #[must_use]
    pub fn from_sample(
        sample: &crate::stock::simulation_cut::SimulationCutSample,
        tool: &crate::tool::ToolDefinition,
    ) -> Option<Self> {
        if sample.is_air() {
            return None;
        }
        let arc = sample.arc_engagement_radians?;
        let ap_mm = sample.axial_doc_mm;
        let r = tool.engagement_radius(ap_mm).max(0.0);
        let psi = arc.clamp(0.0, std::f64::consts::PI);
        let ae_mm = r * (1.0 - psi.cos());
        if ae_mm.is_nan() || ae_mm <= 0.0 {
            return None;
        }
        Some(Self {
            ap_mm,
            ae_mm,
            psi_rad: arc,
            d_eng_mm: 2.0 * r,
            section_mm2: tool.mrr_cross_section_mm2(ap_mm, ae_mm),
            helix_deg: Some(tool.helix_deg()),
        })
    }

    /// The axial depth `ap` (mm).
    #[must_use]
    pub const fn ap_mm(&self) -> f64 {
        self.ap_mm
    }

    /// The radial width `ae` (mm).
    #[must_use]
    pub const fn ae_mm(&self) -> f64 {
        self.ae_mm
    }

    /// The engagement arc psi (rad).
    #[must_use]
    pub const fn psi_rad(&self) -> f64 {
        self.psi_rad
    }

    /// The engaged diameter (mm).
    #[must_use]
    pub const fn d_eng_mm(&self) -> f64 {
        self.d_eng_mm
    }

    /// The cutter's chip section at `(ap, ae)` (mm²).
    #[must_use]
    pub const fn section_mm2(&self) -> f64 {
        self.section_mm2
    }

    /// The tool's helix angle (degrees), when the builder knew it.
    #[must_use]
    pub const fn helix_deg(&self) -> Option<f64> {
        self.helix_deg
    }

    /// The point of the (depth, width) plane.
    #[must_use]
    pub const fn cut_point(&self) -> CutPoint {
        CutPoint {
            ap_mm: self.ap_mm,
            ae_mm: self.ae_mm,
        }
    }

    /// The inputs of the two-term power model at `rpm` and `flute_count`.
    #[must_use]
    pub(crate) fn power_inputs(
        &self,
        line: ForceLine,
        rpm: f64,
        flute_count: f64,
    ) -> PowerModelInputs {
        PowerModelInputs {
            line,
            cross_section_mm2: self.section_mm2,
            axial_doc_mm: self.ap_mm,
            immersion_rad: self.psi_rad,
            engagement_diameter_mm: self.d_eng_mm,
            spindle_rpm: rpm,
            flute_count,
        }
    }

    /// The lateral cutting force (N) at the advance per tooth `fz_mm`:
    /// [`lateral_cutting_force`] at this `ap` and `psi`.
    #[must_use]
    pub fn lateral_force(&self, material: &Material, fz_mm: f64) -> Option<f64> {
        lateral_cutting_force(material, self.ap_mm, self.psi_rad, fz_mm)
    }
}

/// The feed-independent half of a power prediction: material, engaged
/// geometry and spindle speed. Feed is supplied separately because only
/// one term of the two-term model moves with it (see
/// [`crate::tool_load::power::PowerTerms`]).
///
/// K7: built only by [`Engagement::power_inputs`]; the fields are private
/// to this file. `line` is the material's force line;
/// `PowerTerms::of` applies its grain factor, so callers never
/// pre-multiply.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PowerModelInputs {
    /// The material's force line.
    line: ForceLine,
    /// The cutter's chip section (mm²); `section x feed` is the MRR.
    cross_section_mm2: f64,
    /// Axial engagement `ap` (mm). `F_edge` is a force per mm of axial
    /// engagement, so the edge term scales with this.
    axial_doc_mm: f64,
    /// Engagement arc psi (rad); the duty cycle is `z psi / 2 pi`.
    immersion_rad: f64,
    /// Cutting diameter at the engaged depth (mm): `Vc = pi D n`.
    engagement_diameter_mm: f64,
    /// Spindle speed (rev/min).
    spindle_rpm: f64,
    /// Flute count `z`.
    flute_count: f64,
}

impl PowerModelInputs {
    /// The force line.
    pub(crate) const fn line(&self) -> ForceLine {
        self.line
    }
    /// The chip section (mm²).
    pub(crate) const fn cross_section_mm2(&self) -> f64 {
        self.cross_section_mm2
    }
    /// The axial depth (mm).
    pub(crate) const fn axial_doc_mm(&self) -> f64 {
        self.axial_doc_mm
    }
    /// The arc (rad).
    pub(crate) const fn immersion_rad(&self) -> f64 {
        self.immersion_rad
    }
    /// The engaged diameter (mm).
    pub(crate) const fn engagement_diameter_mm(&self) -> f64 {
        self.engagement_diameter_mm
    }
    /// The spindle speed (rev/min).
    pub(crate) const fn spindle_rpm(&self) -> f64 {
        self.spindle_rpm
    }
    /// The flute count.
    pub(crate) const fn flute_count(&self) -> f64 {
        self.flute_count
    }

    /// Test door: the same inputs with one edge input set to a degenerate
    /// value, so the model's degrade rule can be held.
    #[cfg(test)]
    pub(crate) const fn with_degenerate(
        self,
        immersion_rad: Option<f64>,
        engagement_diameter_mm: Option<f64>,
    ) -> Self {
        let mut out = self;
        if let Some(v) = immersion_rad {
            out.immersion_rad = v;
        }
        if let Some(v) = engagement_diameter_mm {
            out.engagement_diameter_mm = v;
        }
        out
    }
}

/// K7 change 3: the cases of the pre-sim power set. Suggest evaluates each
/// case that the `CutSpec` admits and ships the maximum with its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PowerCase {
    /// The stated (or the default) width.
    #[default]
    Nominal,
    /// The planner's engagement cap: `trochoid_cap_mult x ae`, at most D
    /// (3D Rough).
    PlannerCap,
    /// A slot (ae = D): `FirstPass::Slot` or `Radial::Slot`.
    Slot,
}

impl PowerCase {
    /// The name that the card, the CLI and MCP print.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Nominal => "nominal",
            Self::PlannerCap => "planner cap",
            Self::Slot => "slot",
        }
    }
}

/// The set of pre-sim power cases of one cut (K7 change 3). Built from the
/// `CutSpec` (K1) and the planner cap; [`Self::NOMINAL`] is the set of a
/// cut that states no slot and no cap.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PowerCases {
    slot: bool,
    planner_cap_mult: Option<f64>,
}

impl PowerCases {
    /// The nominal case only.
    pub const NOMINAL: Self = Self {
        slot: false,
        planner_cap_mult: None,
    };

    /// The cases that `spec` admits. `planner_cap_mult` is the planner's
    /// engagement cap as a multiple of the nominal width (3D Rough
    /// `trochoid_cap_mult`); a value that is not finite and positive gives
    /// no cap case.
    #[must_use]
    pub fn of_cut(spec: &CutSpec, planner_cap_mult: Option<f64>) -> Self {
        let slot =
            matches!(spec.first_pass, FirstPass::Slot) || matches!(spec.radial, Radial::Slot);
        Self {
            slot,
            planner_cap_mult: planner_cap_mult.filter(|m| m.is_finite() && *m > 0.0),
        }
    }

    /// The cases of `operation`: its `CutSpec` and its planner cap.
    #[must_use]
    pub fn of_operation(operation: &OperationConfig) -> Self {
        Self::of_cut(
            &operation.cut_spec(),
            operation.planner_engagement_cap_mult(),
        )
    }

    /// Each admitted case with its width, in the tie order (a later case
    /// wins a tie: the more specific name). `ae` is the nominal width and
    /// `d` the tool diameter.
    #[must_use]
    pub fn widths(self, ae: f64, d: f64) -> Vec<(PowerCase, f64)> {
        let mut out = vec![(PowerCase::Nominal, ae)];
        if let Some(m) = self.planner_cap_mult {
            out.push((PowerCase::PlannerCap, (m * ae).min(d)));
        }
        if self.slot {
            out.push((PowerCase::Slot, d));
        }
        out
    }

    /// The case of the largest `power(ae)` over [`Self::widths`], with that
    /// power and its width. A tie goes to the later (more specific) case.
    pub fn worst(self, ae: f64, d: f64, power: impl Fn(f64) -> f64) -> (PowerCase, f64, f64) {
        let mut best = (PowerCase::Nominal, power(ae), ae);
        for (case, width) in self.widths(ae, d).into_iter().skip(1) {
            let p = power(width);
            if p >= best.1 {
                best = (case, p, width);
            }
        }
        best
    }
}
