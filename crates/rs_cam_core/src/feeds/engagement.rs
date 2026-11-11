//! CONTRACT K7 (feeds contract P9): one engagement definition, before and
//! after the simulation.
//!
//! [`Engagement`] holds the engaged geometry of one cut: the axial depth
//! `ap`, the radial width `ae`, the arc `psi`, the engaged diameter `d_eng`
//! and the cutter's own chip section. Three constructors build it, and no
//! other code can.
//!
//! P15 (the one `d_eng` door): every constructor reads its engaged diameter
//! from [`EngagedDiameter`], a type with a private field. Its two builders
//! read one width function per shape (`tool::ball_radius_at_height`,
//! `tool::bull_nose_radius_at_height`, `tool::ball_cone_radius_at_height`,
//! `tool::vbit_cone_radius_at_height`), the cutter's own:
//!
//! - [`Engagement::from_cut`]: a planned cut (Suggest, the envelope, the
//!   modulator). `psi` comes from [`immersion_angle`] (`cos psi = 1 - ae / r`).
//! - [`Engagement::from_tool_cut`]: a planned cut of a built tool; it calls
//!   the first.
//! - [`Engagement::from_sample`]: a simulation sample. The sample measures
//!   the arc; the width is the geometric inverse `ae = r (1 - cos psi)`
//!   (lead decision D-W, 2026-11-03), the exact inverse of
//!   [`immersion_angle`]. The gate's old slab `(psi / pi) x 2 r` is retired.
//!
//! P20d: the section is [`EngagedSection::side_cut`] of the cutter's shape
//! (a rectangle for a straight side, the cone for a V-bit). Every power
//! reader builds its
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
//!     section: rs_cam_core::feeds::section::EngagedSection::AxialCircle {
//!         diameter_mm: 6.0,
//!         core_diameter_mm: 0.0,
//!     },
//!     helix_deg: None,
//! };
//! ```
//!
//! The sibling: the constructor compiles.
//!
//! ```
//! use rs_cam_core::feeds::ToolGeometryHint;
//! use rs_cam_core::feeds::engagement::{EngagedDiameter, Engagement};
//! use rs_cam_core::feeds::envelope::CutPoint;
//! let e = Engagement::from_cut(
//!     CutPoint { ap_mm: 1.0, ae_mm: 3.0 },
//!     ToolGeometryHint::Flat,
//!     EngagedDiameter::of_hint(ToolGeometryHint::Flat, 6.0, 6.0, 1.0),
//!     None,
//! );
//! assert!(e.psi_rad() > 0.0);
//! ```

use serde::Serialize;

use crate::compute::catalog::{CutSpec, FirstPass, OperationConfig, PlannerWidth, Radial};
use crate::feeds::ToolGeometryHint;
use crate::feeds::envelope::CutPoint;
use crate::feeds::force::{DeflectionCapRefusal, immersion_angle};
use crate::feeds::section::{AxialSectionRefusal, EngagedSection};
use crate::material::force_line::ForceLine;
use crate::tool::MillingCutter;

/// Feeds contract P15: THE engaged diameter `d_eng` (mm) of a cut, the one
/// door of every [`Engagement`] constructor. The field is private, so no
/// caller passes a raw number. The two builders read the same width
/// function per shape:
///
/// - [`Self::of_hint`]: [`crate::feeds::effective_diameter`], for a caller
///   that holds a [`ToolGeometryHint`] (Suggest, the modulator, export).
/// - [`Self::of_tool`]: `2 x MillingCutter::engagement_radius`, for a built
///   tool (the planned cut of a tool, the simulation sample).
///
/// For a flat end mill, a ball and a bull nose the two are equal bit for
/// bit: each calls the cutter's own width (`tool::ball_radius_at_height`,
/// `tool::bull_nose_radius_at_height`). A bull nose below its corner radius
/// keeps its flat centre. A ball at depth 0 gives 0, and every reader of a
/// zero diameter returns its degenerate answer (no arc, no lift, no edge
/// term). One residual stays for a ruling (P15 phase file): a V-bit hint
/// adds its flat tip and the V-bit cutter does not. P22-0c-6b: a tapered
/// ball reads ONE cap on both doors, the body's cutting cap
/// (`ToolBody::max_cut_diameter_mm`, the width at the flute top, never
/// above the shaft), so the hint and the cutter give one width at every
/// depth. `tests/one_d_eng_door_for_every_engagement_p15.rs` holds the door.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EngagedDiameter {
    d_eng: f64,
    /// P20d: the cutter's nominal diameter, the cap of the V-groove
    /// section above the cone.
    nominal: f64,
}

impl EngagedDiameter {
    /// The engaged diameter of a hint tool at the axial depth `ap_mm`:
    /// [`crate::feeds::effective_diameter`]. `max_cut_diameter_mm` is the
    /// body's cutting cap ([`crate::feeds::CutBody`]); a tapered ball cuts
    /// no wider than it.
    #[must_use]
    pub fn of_hint(
        tool: ToolGeometryHint,
        diameter_mm: f64,
        max_cut_diameter_mm: f64,
        ap_mm: f64,
    ) -> Self {
        Self {
            d_eng: crate::feeds::effective_diameter(tool, diameter_mm, max_cut_diameter_mm, ap_mm),
            nominal: diameter_mm,
        }
    }

    /// The engaged diameter of a built tool at the axial depth `ap_mm`:
    /// `2 x tool.engagement_radius(max(ap, 0))`, never below 0. A tapered
    /// ball clamps it at the body's cutting cap
    /// (`ToolBody::max_cut_diameter_mm`, P22-0c-6 ruling 2), the cap of
    /// the hint door: the plan and the simulation read one width. The
    /// cap binds only a sample deeper than the flute length. The other
    /// shapes keep the cutter's own width (P22-0c-6, the 6b open questions).
    #[must_use]
    pub fn of_tool(tool: &crate::tool::ToolDefinition, ap_mm: f64) -> Self {
        let width = (2.0 * tool.engagement_radius(ap_mm.max(0.0))).max(0.0);
        let d_eng = match tool.to_geometry_hint() {
            ToolGeometryHint::TaperedBall { .. } => width.min(tool.body().max_cut_diameter_mm()),
            ToolGeometryHint::Flat
            | ToolGeometryHint::Ball
            | ToolGeometryHint::Bull { .. }
            | ToolGeometryHint::VBit { .. } => width,
        };
        Self {
            d_eng,
            nominal: tool.diameter(),
        }
    }

    /// The diameter (mm).
    #[must_use]
    pub const fn mm(self) -> f64 {
        self.d_eng
    }

    /// The cutter's nominal diameter (mm) (P20d).
    #[must_use]
    pub const fn nominal_mm(self) -> f64 {
        self.nominal
    }
}

/// CONTRACT K7. Built only by [`Self::from_cut`] (which
/// [`Self::from_tool_cut`] calls) and [`Self::from_sample`]. P15: each reads
/// its engaged diameter from [`EngagedDiameter`].
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
    section: EngagedSection,
    helix_deg: Option<f64>,
}

impl Engagement {
    /// A planned cut. `d_eng` is the engaged diameter, built only by
    /// [`EngagedDiameter::of_hint`] or [`EngagedDiameter::of_tool`]: the
    /// caller cannot pass a raw number (P15, the one `d_eng` door).
    /// `helix_deg` is the tool's helix, when the caller knows it.
    #[must_use]
    pub fn from_cut(
        cut: CutPoint,
        tool: ToolGeometryHint,
        d_eng: EngagedDiameter,
        helix_deg: Option<f64>,
    ) -> Self {
        let d_eng_mm = d_eng.mm();
        let psi_rad = immersion_angle(cut.ae_mm, d_eng_mm / 2.0);
        Self {
            ap_mm: cut.ap_mm,
            ae_mm: cut.ae_mm,
            psi_rad,
            d_eng_mm,
            section: EngagedSection::side_cut(
                tool,
                d_eng.nominal_mm(),
                cut.ap_mm,
                cut.ae_mm,
                d_eng_mm,
                psi_rad,
            ),
            helix_deg,
        }
    }

    /// A planned cut of a built tool (P11a, the open P9 diameters): the
    /// engaged diameter is [`EngagedDiameter::of_tool`] at the cut depth,
    /// the width rule of [`Self::from_sample`], and the helix is the
    /// tool's. It calls [`Self::from_cut`].
    #[must_use]
    pub fn from_tool_cut(cut: CutPoint, tool: &crate::tool::ToolDefinition) -> Self {
        Self::from_cut(
            cut,
            tool.to_geometry_hint(),
            EngagedDiameter::of_tool(tool, cut.ap_mm),
            Some(tool.helix_deg()),
        )
    }

    /// A simulation sample. `None` when the sample removes no material,
    /// carries no arc, or gives no positive width.
    ///
    /// The engaged diameter is [`EngagedDiameter::of_tool`]. The arc is
    /// the sample's own reading; the width is `r (1 - cos psi)` with psi
    /// clamped to a full slot (D-W).
    ///
    /// M4 (open item of P29, named in P29-1 and not changed): the depth is
    /// `axial_doc_mm`, the depth of the layer the sample removes (the
    /// section of the power model). The deflection gate's door,
    /// `tool_load::deflection::sample_engagement`, reads
    /// `axial_engagement_mm` (the engaged edge length). The power gate, the
    /// chip gate and the depth gate read this door.
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
        let r = EngagedDiameter::of_tool(tool, ap_mm).mm() / 2.0;
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
            section: EngagedSection::side_cut(
                tool.to_geometry_hint(),
                tool.diameter(),
                ap_mm,
                ae_mm,
                2.0 * r,
                arc,
            ),
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

    /// The cutter's chip section at `(ap, ae)` (mm²): the area of
    /// [`Self::section`].
    #[must_use]
    pub fn section_mm2(&self) -> f64 {
        self.section.area_mm2()
    }

    /// P20d: the engaged section of this side cut.
    #[must_use]
    pub const fn section(&self) -> EngagedSection {
        self.section
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
            section: self.section,
            spindle_rpm: rpm,
            flute_count,
        }
    }

    /// L5 (feeds contract P11a): the cutting force summed over the engaged
    /// edge, at the advance per tooth `fz_mm`, for a cutter with
    /// `flute_count` flutes. See [`EdgeForce`] and the model below.
    ///
    /// ```text
    /// dF(z)  = (Ks h(phi) + F_edge) dz      the force line (ruling B6)
    /// h(phi) = fz sin(phi), 0 <= phi <= psi  the chip of one edge element
    /// phi(z) = theta + j 2 pi / Z - lag(z)   lag(z) = z tan(helix) / r
    /// ```
    ///
    /// The force of one element points along the edge's cutting direction,
    /// at the angle `phi`. The radial force ratio `K_r` has no printed wood
    /// source in the repo (`research_ae/REPORT.md` section 4), so the
    /// element has no radial component (`K_r = 0`). The sum over `z` is
    /// exact (the lag is linear in `z`); the rotation `theta` is sampled
    /// ([`EDGE_SAMPLES_PER_PITCH`] per flute pitch). `r` is half the
    /// engaged diameter `d_eng` at this depth, for the whole engaged edge.
    ///
    /// `None` when the builder did not know the helix, the material line
    /// or an input is not usable, or no edge is engaged.
    #[must_use]
    pub fn edge_force(&self, line: ForceLine, fz_mm: f64, flute_count: u32) -> Option<EdgeForce> {
        let (ks, f_edge) = (line.ks_n_per_mm2(), line.f_edge_n_per_mm());
        if !(fz_mm.is_finite() && fz_mm >= 0.0 && ks.is_finite() && f_edge.is_finite()) {
            return None;
        }
        let sums = self.edge_sums(flute_count)?;
        let peak_resultant_n = sums
            .iter()
            .map(|s| s.resultant_n(ks * fz_mm, f_edge))
            .fold(0.0_f64, f64::max);
        Some(EdgeForce {
            mean_tangential_n: self.mean_tangential_n(ks, f_edge, fz_mm, flute_count),
            peak_resultant_n,
        })
    }

    /// The mean over one revolution of the scalar engaged-edge force (N):
    /// `ap Z / (2 pi) (Ks fz (1 - cos psi) + F_edge psi)`. The helix only
    /// shifts each element in phase, so the mean does not read it. Power
    /// is this force at the cutting speed (`PowerTerms`).
    fn mean_tangential_n(&self, ks: f64, f_edge: f64, fz_mm: f64, flute_count: u32) -> f64 {
        let psi = self.psi_rad.clamp(0.0, std::f64::consts::PI);
        self.ap_mm * f64::from(flute_count) / std::f64::consts::TAU
            * (ks * fz_mm * (1.0 - psi.cos()) + f_edge * psi)
    }

    /// L5: the largest advance per tooth (mm) at which the PEAK resultant
    /// engaged-edge force ([`Self::edge_force`]) stays at or below
    /// `budget_n`. Deflection is linear in force, so a deflection bound is
    /// a force budget (`bound / compliance`).
    ///
    /// At each rotation sample the force vector is `Ks fz A + F_edge B`
    /// (the shear and edge sums of [`EdgeSample`]), so `|F| = budget` is a
    /// quadratic in `fz`. The cap is the smallest positive root over the
    /// samples: exact at the samples, no bisection.
    ///
    /// # Errors
    ///
    /// - [`DeflectionCapRefusal::Unmodelled`]: no depth, no budget, no
    ///   helix, no slope `Ks`, or no finite answer.
    /// - [`DeflectionCapRefusal::NoLateralEngagement`]: no edge is engaged
    ///   (the arc is zero), so feed does not move the force.
    /// - [`DeflectionCapRefusal::EdgeForceOverBudget`]: the edge force
    ///   alone (`fz = 0`, the resultant of the edge sum) meets the budget
    ///   at some rotation.
    pub fn fz_cap_for_peak_force(
        &self,
        line: ForceLine,
        flute_count: u32,
        budget_n: f64,
    ) -> Result<f64, DeflectionCapRefusal> {
        let (ks, f_edge) = (line.ks_n_per_mm2(), line.f_edge_n_per_mm());
        let inputs_modelled = self.ap_mm.is_finite()
            && self.ap_mm > 0.0
            && budget_n.is_finite()
            && budget_n > 0.0
            && f_edge.is_finite();
        if !inputs_modelled {
            return Err(DeflectionCapRefusal::Unmodelled);
        }
        if !(self.psi_rad.is_finite() && self.psi_rad > 0.0) {
            return Err(DeflectionCapRefusal::NoLateralEngagement);
        }
        let sums = self
            .edge_sums(flute_count)
            .ok_or(DeflectionCapRefusal::Unmodelled)?;
        // The edge force alone (`fz = 0`) is the resultant of the edge sum,
        // the same measure the budget bounds.
        if sums.iter().any(|s| s.resultant_n(0.0, f_edge) >= budget_n) {
            return Err(DeflectionCapRefusal::EdgeForceOverBudget);
        }
        if !(ks.is_finite() && ks > 0.0) {
            return Err(DeflectionCapRefusal::Unmodelled);
        }
        let mut cap = f64::INFINITY;
        for s in &sums {
            if let Some(fz) = s.fz_at_budget(ks, f_edge, budget_n) {
                cap = cap.min(fz);
            }
        }
        if cap.is_finite() && cap > 0.0 {
            Ok(cap)
        } else if cap.is_infinite() {
            Err(DeflectionCapRefusal::NoLateralEngagement)
        } else {
            Err(DeflectionCapRefusal::Unmodelled)
        }
    }

    /// The engaged-edge sums at [`EDGE_SAMPLES_PER_PITCH`] rotations over
    /// one flute pitch. The flutes are evenly spaced, so the force repeats
    /// each pitch. `None` when the helix is unknown or an input is not
    /// usable.
    fn edge_sums(&self, flute_count: u32) -> Option<Vec<EdgeSample>> {
        use std::f64::consts::TAU;
        let helix = self.helix_deg?;
        let r = self.d_eng_mm / 2.0;
        let usable = self.ap_mm.is_finite()
            && self.ap_mm > 0.0
            && r.is_finite()
            && r > 0.0
            && helix.is_finite()
            && flute_count > 0;
        if !usable {
            return None;
        }
        let psi = self.psi_rad.clamp(0.0, std::f64::consts::PI);
        if psi <= 0.0 {
            return None;
        }
        let z = f64::from(flute_count);
        let pitch = TAU / z;
        // The wrap of the edge over the depth (rad), from the one helix
        // formula (`tool::helix_lag_rad`).
        let wrap = crate::tool::helix_lag_rad(self.ap_mm, r, helix);
        let arc = Arc::new(psi);
        let n = EDGE_SAMPLES_PER_PITCH;
        // Every angle below is a rotation sample plus a fixed offset, so its
        // sine and cosine come from the sample table and one rotation each
        // (no trigonometry in the loop).
        let table = theta_table(flute_count);
        let flute_rot: Vec<(f64, f64)> = (0..flute_count)
            .map(|j| (pitch * f64::from(j)).sin_cos())
            .collect();
        let (sin_w, cos_w) = wrap.sin_cos();
        let mut out = Vec::with_capacity(n);
        for (i, &(sin_t, cos_t)) in table.iter().enumerate() {
            let theta = pitch * (i as f64) / (n as f64);
            let mut sample = EdgeSample::default();
            for (j, &(sin_j, cos_j)) in flute_rot.iter().enumerate() {
                // The edge angle at the tip; higher elements trail it.
                let lead_u = theta + pitch * j as f64;
                let lead = Angle {
                    u: lead_u,
                    sin: sin_t * cos_j + cos_t * sin_j,
                    cos: cos_t * cos_j - sin_t * sin_j,
                };
                if wrap > 0.0 {
                    // u = phi(z) runs over [lead - wrap, lead]; dz = du * ap / wrap.
                    let trail = Angle {
                        u: lead_u - wrap,
                        sin: lead.sin * cos_w - lead.cos * sin_w,
                        cos: lead.cos * cos_w + lead.sin * sin_w,
                    };
                    sample.add_interval(&arc, trail, lead, self.ap_mm / wrap);
                } else {
                    // A straight flute: the whole depth at one angle.
                    sample.add_point(&arc, lead, self.ap_mm);
                }
            }
            out.push(sample);
        }
        Some(out)
    }
}

/// L5: the rotation samples per flute pitch of the engaged-edge sum. A
/// numerical step, not a formula constant. The peak is the largest
/// sample; between two samples the force moves by at most its slope times
/// `pitch / N`. The step 0 reference model samples 720 per pitch
/// (`planning/feeds_contract_2026-11-03/research_p11_step0/model.py`,
/// `NT = 720`). Measured by the P11a agent (2026-11-05) on ten flat cuts
/// (D 3-12, Z 1-4, helix 20-45, ap 0.2-25, ae 0.05-1 D, white oak): the
/// peak at 180 is within 0.08 % of the peak at 720 (90 gives 0.6 %), at a
/// quarter of the cost. The envelope evaluates the beam a few hundred
/// times per plane, so the cost counts.
pub const EDGE_SAMPLES_PER_PITCH: usize = 180;

/// L5: the engaged-edge force of one cut. Built by
/// [`Engagement::edge_force`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EdgeForce {
    /// The mean over one revolution of the scalar sum of the element
    /// forces (N). Power reads this force (through `PowerTerms`, which
    /// gives the same number in closed form for a flat end mill).
    pub mean_tangential_n: f64,
    /// The largest magnitude over one revolution of the vector sum of the
    /// element forces (N). Lateral force and tip deflection read this
    /// force.
    pub peak_resultant_n: f64,
}

/// The engaged arc `[0, psi]` with its end values.
struct Arc {
    psi: f64,
    sin_psi: f64,
    cos_psi: f64,
}

impl Arc {
    fn new(psi: f64) -> Self {
        Self {
            psi,
            sin_psi: psi.sin(),
            cos_psi: psi.cos(),
        }
    }
}

/// An angle (rad) with its sine and cosine.
#[derive(Debug, Clone, Copy)]
struct Angle {
    u: f64,
    sin: f64,
    cos: f64,
}

/// The sine and cosine of the rotation samples `i x pitch / N` of a
/// cutter with `flute_count` flutes, `i` in `0..N`
/// ([`EDGE_SAMPLES_PER_PITCH`]). The table of one to eight flutes is built
/// once; any other count is built per call.
fn theta_table(flute_count: u32) -> std::borrow::Cow<'static, [(f64, f64)]> {
    use std::sync::OnceLock;
    const CACHED: usize = 8;
    static TABLES: [OnceLock<Vec<(f64, f64)>>; CACHED] = [const { OnceLock::new() }; CACHED];
    let build = || -> Vec<(f64, f64)> {
        let pitch = std::f64::consts::TAU / f64::from(flute_count.max(1));
        let n = EDGE_SAMPLES_PER_PITCH;
        (0..n)
            .map(|i| (pitch * (i as f64) / (n as f64)).sin_cos())
            .collect()
    };
    match usize::try_from(flute_count)
        .ok()
        .and_then(|z| z.checked_sub(1))
    {
        Some(k) if k < CACHED => match TABLES.get(k) {
            Some(cell) => std::borrow::Cow::Borrowed(cell.get_or_init(build).as_slice()),
            None => std::borrow::Cow::Owned(build()),
        },
        Some(_) | None => std::borrow::Cow::Owned(build()),
    }
}

/// The engaged-edge sums at one rotation, per unit `Ks fz` (shear) and per
/// unit `F_edge` (edge), in mm:
///
/// - `shear = sum sin(phi) (cos phi, sin phi) dz` (a vector),
/// - `edge = sum (cos phi, sin phi) dz` (a vector),
/// - `shear_len = sum sin(phi) dz` (a scalar),
/// - `edge_len = sum dz` (the engaged edge length).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct EdgeSample {
    shear: [f64; 2],
    edge: [f64; 2],
    shear_len: f64,
    edge_len: f64,
}

impl EdgeSample {
    /// The magnitude (N) of `shear_n_per_mm x shear + f_edge x edge`.
    fn resultant_n(&self, shear_n_per_mm: f64, f_edge: f64) -> f64 {
        let x = shear_n_per_mm * self.shear[0] + f_edge * self.edge[0];
        let y = shear_n_per_mm * self.shear[1] + f_edge * self.edge[1];
        x.hypot(y)
    }

    /// The scalar edge length (mm). The edge force alone at `fz = 0` is
    /// at most `F_edge` times this length (the resultant is shorter).
    #[cfg(test)]
    const fn edge_len(&self) -> f64 {
        self.edge_len
    }

    /// The positive `fz` at which the resultant equals `budget_n`, or
    /// `None` when the shear sum is zero (feed does not move this sample).
    /// The caller has checked that the edge force alone is under budget.
    fn fz_at_budget(&self, ks: f64, f_edge: f64, budget_n: f64) -> Option<f64> {
        let a2 = self.shear[0] * self.shear[0] + self.shear[1] * self.shear[1];
        if a2 <= 0.0 {
            return None;
        }
        let ab = self.shear[0] * self.edge[0] + self.shear[1] * self.edge[1];
        let b2 = self.edge[0] * self.edge[0] + self.edge[1] * self.edge[1];
        // |ks fz A + fe B|^2 = budget^2, a quadratic in fz.
        let qa = ks * ks * a2;
        let qb = 2.0 * ks * f_edge * ab;
        let qc = f_edge * f_edge * b2 - budget_n * budget_n;
        let disc = qb * qb - 4.0 * qa * qc;
        if !(disc.is_finite() && disc >= 0.0) {
            return None;
        }
        let root = (-qb + disc.sqrt()) / (2.0 * qa);
        (root.is_finite() && root > 0.0).then_some(root)
    }

    /// Add the elements whose angle `u` runs over `[lo, hi]`, each `dz =
    /// du x scale`, that fall in the arc (modulo a full turn).
    fn add_interval(&mut self, arc: &Arc, lo: Angle, hi: Angle, scale: f64) {
        use std::f64::consts::TAU;
        if hi.u <= lo.u {
            return;
        }
        let first = ((lo.u - arc.psi) / TAU).floor() as i64;
        let last = (hi.u / TAU).floor() as i64;
        for m in first..=last {
            let shift = TAU * m as f64;
            // Each end is the interval end (its sine and cosine do not move
            // with a full turn) or an end of the arc.
            let pa = if lo.u - shift > 0.0 {
                Primitive::at(lo.u - shift, lo.sin, lo.cos)
            } else {
                Primitive::at(0.0, 0.0, 1.0)
            };
            let pb = if hi.u - shift < arc.psi {
                Primitive::at(hi.u - shift, hi.sin, hi.cos)
            } else {
                Primitive::at(arc.psi, arc.sin_psi, arc.cos_psi)
            };
            if pb.phi > pa.phi {
                self.shear[0] += scale * (pb.shear_x - pa.shear_x);
                self.shear[1] += scale * (pb.shear_y - pa.shear_y);
                self.edge[0] += scale * (pb.edge_x - pa.edge_x);
                self.edge[1] += scale * (pb.edge_y - pa.edge_y);
                self.shear_len += scale * (pb.shear_s - pa.shear_s);
                self.edge_len += scale * (pb.phi - pa.phi);
            }
        }
    }

    /// Add a straight edge of length `len` at the angle `at`, if it falls
    /// in the arc (modulo a full turn).
    fn add_point(&mut self, arc: &Arc, at: Angle, len: f64) {
        let phi = at.u.rem_euclid(std::f64::consts::TAU);
        if phi <= arc.psi {
            let (s, c) = (at.sin, at.cos);
            self.shear[0] += len * s * c;
            self.shear[1] += len * s * s;
            self.edge[0] += len * c;
            self.edge[1] += len * s;
            self.shear_len += len * s;
            self.edge_len += len;
        }
    }
}

/// The primitives of the element sums at the arc angle `phi`:
/// `int sin = -cos`, `int sin cos = sin^2 / 2`,
/// `int sin^2 = phi / 2 - sin cos / 2`, `int cos = sin`.
struct Primitive {
    phi: f64,
    shear_s: f64,
    shear_x: f64,
    shear_y: f64,
    edge_x: f64,
    edge_y: f64,
}

impl Primitive {
    /// The primitives at the arc angle `phi`, with `s = sin(phi)` and
    /// `c = cos(phi)`.
    fn at(phi: f64, s: f64, c: f64) -> Self {
        Self {
            phi,
            shear_s: -c,
            shear_x: 0.5 * s * s,
            shear_y: 0.5 * phi - 0.5 * s * c,
            edge_x: s,
            edge_y: -c,
        }
    }
}

/// The feed-independent half of a power prediction: material, engaged
/// geometry and spindle speed. Feed is supplied separately because only
/// one term of the two-term model moves with it (see
/// [`crate::tool_load::power::PowerTerms`]).
///
/// K7: built only by [`Engagement::power_inputs`] (a side cut) and
/// [`Self::axial_plunge`] (P20d); the fields are private to this file.
/// `line` is the material's force line; `PowerTerms::of` applies its grain
/// factor, so callers never pre-multiply.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PowerModelInputs {
    /// The material's force line.
    line: ForceLine,
    /// P20d: the engaged section (`area x feed` is the MRR; the section
    /// states its own edge term).
    section: EngagedSection,
    /// Spindle speed (rev/min).
    spindle_rpm: f64,
    /// Flute count `z`.
    flute_count: f64,
}

impl PowerModelInputs {
    /// P20d: the inputs of a straight plunge of a tool of shape `tool`,
    /// diameter `diameter_mm` and core `core_diameter_mm` (0 for a tool
    /// that cuts at its centre; P22-0c-6 D6)
    /// ([`EngagedSection::axial_plunge`]).
    ///
    /// # Errors
    ///
    /// [`AxialSectionRefusal`] when the tip shape has no axial section.
    pub(crate) fn axial_plunge(
        tool: ToolGeometryHint,
        diameter_mm: f64,
        core_diameter_mm: f64,
        line: ForceLine,
        rpm: f64,
        flute_count: f64,
    ) -> Result<Self, AxialSectionRefusal> {
        Ok(Self {
            line,
            section: EngagedSection::axial_plunge(tool, diameter_mm, core_diameter_mm)?,
            spindle_rpm: rpm,
            flute_count,
        })
    }

    /// The force line.
    pub(crate) const fn line(&self) -> ForceLine {
        self.line
    }
    /// The engaged section.
    pub(crate) const fn section(&self) -> EngagedSection {
        self.section
    }
    /// The chip section (mm²).
    pub(crate) fn cross_section_mm2(&self) -> f64 {
        self.section.area_mm2()
    }
    /// The spindle speed (rev/min).
    pub(crate) const fn spindle_rpm(&self) -> f64 {
        self.spindle_rpm
    }
    /// The flute count.
    pub(crate) const fn flute_count(&self) -> f64 {
        self.flute_count
    }

    /// Test door: the same inputs with one edge input of the rectangle set
    /// to a degenerate value, so the model's degrade rule can be held.
    #[cfg(test)]
    pub(crate) const fn with_degenerate(
        self,
        immersion_rad: Option<f64>,
        engagement_diameter_mm: Option<f64>,
    ) -> Self {
        let mut out = self;
        if let EngagedSection::Rectangle {
            ap_mm,
            ae_mm,
            d_eng_mm,
            psi_rad,
        } = self.section
        {
            out.section = EngagedSection::Rectangle {
                ap_mm,
                ae_mm,
                d_eng_mm: match engagement_diameter_mm {
                    Some(v) => v,
                    None => d_eng_mm,
                },
                psi_rad: match immersion_rad {
                    Some(v) => v,
                    None => psi_rad,
                },
            };
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
    /// The widest width that the planner holds: the pass ceiling `c D` (2D
    /// Adaptive, 3D Rough under Geodesic Adaptive; P23a), or the unsourced
    /// `trochoid_cap_mult x ae`, at most D (3D Rough under Contour
    /// Parallel).
    PlannerCap,
    /// A slot (ae = D): `FirstPass::Slot` or `Radial::Slot`.
    Slot,
    /// P20d: a straight plunge (`Radial::Axial`, a drill cycle): the axial
    /// circle, one case.
    AxialPlunge,
}

impl PowerCase {
    /// The name that the card, the CLI and MCP print.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Nominal => "nominal",
            Self::PlannerCap => "planner cap",
            Self::Slot => "slot",
            Self::AxialPlunge => "axial plunge",
        }
    }
}

/// The set of pre-sim power cases of one cut (K7 change 3). Built from the
/// `CutSpec` (K1) and the planner width; [`Self::NOMINAL`] is the set of a
/// cut that states no slot and no planner bound.
///
/// P23a: the set also answers [`Self::full_feed_width`], the width at which
/// Suggest sizes the chip-thinning lift. One value, built in one place, for
/// the power case and the lift.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PowerCases {
    slot: bool,
    planner: PlannerWidth,
}

impl PowerCases {
    /// The nominal case only.
    pub const NOMINAL: Self = Self {
        slot: false,
        planner: PlannerWidth::None,
    };

    /// The cases that `spec` admits, with the planner width `planner`
    /// ([`OperationConfig::planner_width`]).
    #[must_use]
    pub fn of_cut(spec: &CutSpec, planner: PlannerWidth) -> Self {
        let slot =
            matches!(spec.first_pass, FirstPass::Slot) || matches!(spec.radial, Radial::Slot);
        Self { slot, planner }
    }

    /// The cases of `operation`: its `CutSpec` and its planner width.
    #[must_use]
    pub fn of_operation(operation: &OperationConfig) -> Self {
        Self::of_cut(&operation.cut_spec(), operation.planner_width())
    }

    /// P23a: the width (mm) at which the chip-thinning lift is sized: the
    /// widest width that the planner holds at the full feed
    /// ([`PlannerWidth::full_feed_width_mm`]); the nominal width `ae` for
    /// an op with no planner bound. `d` is the tool diameter. Every reader
    /// of the Suggest lift takes its width here: Step 5, the unlifted feed,
    /// the power ladder's width rung, pass 9, the efficiency figure and the
    /// load context of the gates.
    #[must_use]
    pub fn full_feed_width(self, ae: f64, d: f64) -> f64 {
        self.planner.full_feed_width_mm(ae, d)
    }

    /// Each admitted case with its width, in the tie order (a later case
    /// wins a tie: the more specific name). `ae` is the nominal width and
    /// `d` the tool diameter.
    #[must_use]
    pub fn widths(self, ae: f64, d: f64) -> Vec<(PowerCase, f64)> {
        let mut out = vec![(PowerCase::Nominal, ae)];
        if let Some(width) = self.planner.power_width_mm(ae, d) {
            out.push((PowerCase::PlannerCap, width));
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

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use std::f64::consts::PI;

    use super::*;
    use crate::material::{Material, WoodSpecies};
    use crate::tool_load::power::PowerTerms;

    const D: f64 = 6.0;
    const Z: u32 = 2;

    fn oak_line() -> ForceLine {
        Material::SolidWood {
            species: WoodSpecies::WhiteOak,
        }
        .force_line()
        .expect("white oak has a force line")
    }

    fn flat(ap: f64, ae: f64, helix: f64) -> Engagement {
        Engagement::from_cut(
            CutPoint {
                ap_mm: ap,
                ae_mm: ae,
            },
            ToolGeometryHint::Flat,
            EngagedDiameter::of_hint(ToolGeometryHint::Flat, D, D, ap),
            Some(helix),
        )
    }

    /// L5: power reads the mean engaged-edge force. For a flat end mill
    /// `PowerTerms` gives `Ks ap ae feed + F_edge ap Vc Z psi / (2 pi)`
    /// and the mean force times `Vc = pi D n` gives the same sum, because
    /// `ae = r (1 - cos psi)` (the inverse of `immersion_angle`). The two
    /// differ only by rounding: a few products and quotients (each at most
    /// 2^-52 relative) and the `acos` / `cos` round trip of `psi`. The
    /// tolerance 1e-9 relative is far above that and far below any model
    /// difference.
    #[test]
    fn the_mean_engaged_edge_force_is_the_power_model() {
        let line = oak_line();
        let rpm = 18_000.0;
        let fz = 0.05;
        let feed = fz * rpm * f64::from(Z);
        for (ap, ae) in [
            (1.0, 0.3),
            (6.0, 1.2),
            (16.32, 3.0),
            (25.0, 6.0),
            (3.0, 5.1),
        ] {
            let e = flat(ap, ae, 30.0);
            let f = e.edge_force(line, fz, Z).expect("a modelled cut");
            let vc_mm_min = PI * D * rpm;
            let from_force = line.grain_factor() * f.mean_tangential_n * vc_mm_min / 60_000_000.0;
            let from_power =
                PowerTerms::of(e.power_inputs(line, rpm, f64::from(Z))).kw_at_feed(feed);
            assert!(
                (from_force - from_power).abs() <= 1e-9 * from_power,
                "ap {ap} ae {ae}: mean force gives {from_force} kW, PowerTerms {from_power} kW"
            );
        }
    }

    /// The mean over the rotation samples converges on the closed form.
    /// The scalar sum `F(theta)` is periodic over one pitch and Lipschitz:
    /// each flute adds `(1/k) (g(end) - g(start))` to `dF/dtheta`, with
    /// `0 <= g <= Ks fz + F_edge` and `1/k = ap / wrap`. So
    /// `|dF/dtheta| <= Z (Ks fz + F_edge) ap / wrap`, and the left-point
    /// mean of N samples differs from the exact mean by at most that slope
    /// times `pitch / (2 N)`.
    #[test]
    fn the_sampled_mean_meets_the_closed_form() {
        let line = oak_line();
        let (ks, fe) = (line.ks_n_per_mm2(), line.f_edge_n_per_mm());
        let fz = 0.05;
        for (ap, ae) in [(1.0, 0.3), (6.0, 1.2), (16.32, 3.0), (25.0, 6.0)] {
            let e = flat(ap, ae, 30.0);
            let sums = e.edge_sums(Z).expect("a modelled cut");
            let sampled = sums
                .iter()
                .map(|s| ks * fz * s.shear_len + fe * s.edge_len)
                .sum::<f64>()
                / sums.len() as f64;
            let closed = e.mean_tangential_n(ks, fe, fz, Z);
            let wrap = crate::tool::helix_lag_rad(ap, D / 2.0, 30.0);
            let pitch = 2.0 * PI / f64::from(Z);
            let bound = f64::from(Z) * (ks * fz + fe) * ap / wrap * pitch
                / (2.0 * EDGE_SAMPLES_PER_PITCH as f64);
            assert!(
                (sampled - closed).abs() <= bound,
                "ap {ap} ae {ae}: sampled mean {sampled}, closed {closed}, bound {bound}"
            );
        }
    }

    /// A straight flute (helix 0) puts the whole depth at one angle. In a
    /// slot the peak is then one tooth at `phi = pi/2`: `ap (Ks fz +
    /// F_edge)`, the retired one-tooth form. The helix model holds the old
    /// model as its zero-helix limit.
    #[test]
    fn a_straight_flute_slot_peaks_at_one_tooth() {
        let line = oak_line();
        let (ks, fe) = (line.ks_n_per_mm2(), line.f_edge_n_per_mm());
        let (ap, fz) = (3.0, 0.05);
        let f = flat(ap, D, 0.0).edge_force(line, fz, Z).expect("a slot");
        let want = ap * (ks * fz + fe);
        assert!(
            (f.peak_resultant_n - want).abs() <= 1e-9 * want,
            "{} vs {want}",
            f.peak_resultant_n
        );
    }

    /// The feed cap inverts onto the budget: the peak force at the cap is
    /// the budget.
    #[test]
    fn the_feed_cap_inverts_onto_the_budget() {
        let line = oak_line();
        for (ap, ae, helix) in [(3.0, D, 30.0), (16.32, 1.2, 30.0), (6.0, 2.0, 0.0)] {
            let e = flat(ap, ae, helix);
            let base = e
                .edge_force(line, 0.05, Z)
                .expect("modelled")
                .peak_resultant_n;
            let budget = 1.5 * base;
            let fz = e
                .fz_cap_for_peak_force(line, Z, budget)
                .expect("the budget is above the edge force");
            let at_cap = e
                .edge_force(line, fz, Z)
                .expect("modelled")
                .peak_resultant_n;
            assert!(
                (at_cap - budget).abs() <= 1e-9 * budget,
                "ap {ap} ae {ae} helix {helix}: peak at the cap {at_cap} vs budget {budget}"
            );
        }
    }

    /// The edge force alone is a vector sum: across an arc it is shorter
    /// than `F_edge` times the engaged edge length. A budget between the
    /// two has a positive feed cap at every rotation, not a refusal.
    #[test]
    fn a_budget_above_the_edge_resultant_has_a_feed_cap() {
        let line = oak_line();
        let fe = line.f_edge_n_per_mm();
        let e = flat(16.32, D, 30.0);
        let sums = e.edge_sums(Z).expect("a modelled cut");
        let resultant = sums
            .iter()
            .map(|s| s.resultant_n(0.0, fe))
            .fold(0.0_f64, f64::max);
        let scalar = sums
            .iter()
            .map(|s| fe * s.edge_len())
            .fold(0.0_f64, f64::max);
        assert!(resultant < scalar, "{resultant} vs {scalar}");
        let budget = 0.5 * (resultant + scalar);
        let fz = e
            .fz_cap_for_peak_force(line, Z, budget)
            .expect("a positive root exists at every rotation");
        let at_cap = e
            .edge_force(line, fz, Z)
            .expect("modelled")
            .peak_resultant_n;
        assert!(
            (at_cap - budget).abs() <= 1e-9 * budget,
            "peak at the cap {at_cap} vs budget {budget}"
        );
    }

    #[test]
    fn the_feed_cap_names_each_refusal() {
        let line = oak_line();
        let e = flat(3.0, D, 30.0);
        let edge_only = e
            .edge_force(line, 0.0, Z)
            .expect("modelled")
            .peak_resultant_n;
        assert_eq!(
            e.fz_cap_for_peak_force(line, Z, 0.5 * edge_only),
            Err(DeflectionCapRefusal::EdgeForceOverBudget)
        );
        assert_eq!(
            e.fz_cap_for_peak_force(line, Z, 0.0),
            Err(DeflectionCapRefusal::Unmodelled)
        );
        assert_eq!(
            flat(3.0, 0.0, 30.0).fz_cap_for_peak_force(line, Z, 100.0),
            Err(DeflectionCapRefusal::NoLateralEngagement)
        );
        let no_helix = Engagement::from_cut(
            CutPoint {
                ap_mm: 3.0,
                ae_mm: D,
            },
            ToolGeometryHint::Flat,
            EngagedDiameter::of_hint(ToolGeometryHint::Flat, D, D, 3.0),
            None,
        );
        assert_eq!(
            no_helix.fz_cap_for_peak_force(line, Z, 100.0),
            Err(DeflectionCapRefusal::Unmodelled)
        );
        assert!(no_helix.edge_force(line, 0.05, Z).is_none());
    }
}
