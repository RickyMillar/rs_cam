//! Milling cutter definitions.
//!
//! Every tool implements `MillingCutter`, providing:
//! - Profile functions: `height_at_radius(r)` and `width_at_height(h)`
//! - Drop-cutter contact: `vertex_drop`, `facet_drop`, `edge_drop`
//!
//! Reference: research/03_tool_geometry.md and research/raw_opencamlib_math.md

mod ball;
mod bullnose;
mod flat;
mod tapered_ball;
mod vbit;

pub use ball::BallEndmill;
pub use bullnose::BullNoseEndmill;
pub use flat::FlatEndmill;
pub use tapered_ball::TaperedBallEndmill;
pub use vbit::VBitEndmill;
// Re-export ToolDefinition (defined below the trait in this file)

use crate::compute::tool_config::ToolMaterial;
use crate::geo::{P3, Triangle};

/// Contact point from a drop-cutter test.
#[derive(Debug, Clone, Copy)]
pub struct CLPoint {
    /// Cutter-location position (tool tip)
    pub x: f64,
    pub y: f64,
    pub z: f64,
    /// True if at least one triangle contributed to this CL point's Z value.
    /// When false, the point is outside the mesh footprint and Z is NEG_INFINITY
    /// (or clamped by the caller).
    pub contacted: bool,
}

impl CLPoint {
    pub fn new(x: f64, y: f64) -> Self {
        Self {
            x,
            y,
            z: f64::NEG_INFINITY,
            contacted: false,
        }
    }

    #[inline]
    pub fn update_z(&mut self, z: f64) {
        if z > self.z {
            self.z = z;
            self.contacted = true;
        }
    }

    /// Initialize for a raise-cutter test (finds minimum Z contact from below).
    pub fn new_from_below(x: f64, y: f64) -> Self {
        Self {
            x,
            y,
            z: f64::INFINITY,
            contacted: false,
        }
    }

    /// Update Z to the minimum contact point (for raise-cutter / from-below).
    #[inline]
    pub fn update_z_min(&mut self, z: f64) {
        if z < self.z {
            self.z = z;
            self.contacted = true;
        }
    }

    pub fn position(&self) -> P3 {
        P3::new(self.x, self.y, self.z)
    }
}

/// The core trait for all milling cutter types.
///
/// Follows OpenCAMLib's template-method pattern: the drop-cutter algorithm
/// calls vertex_drop/facet_drop/edge_drop, and each cutter type implements
/// them according to its geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngagementMode {
    Climb,
    Conventional,
    Slot,
}

#[derive(Debug, Clone, Copy)]
pub struct ChipGeometry {
    pub max_chip_thickness_mm: f64,
    pub mean_chip_thickness_mm: f64,
    pub edge_engagement_length_mm: f64,
    pub instantaneous_flutes_in_cut: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngagementError {
    Unsupported { reason: String },
    OutOfRange { reason: String },
}

pub(crate) fn flat_chip_geometry_for_radius(
    radius: f64,
    helix_deg: f64,
    axial_doc_mm: f64,
    arc_engagement_radians: f64,
    feed_per_tooth_mm: f64,
    flute_count: u32,
) -> Result<ChipGeometry, EngagementError> {
    if radius <= 0.0
        || axial_doc_mm <= 0.0
        || arc_engagement_radians <= 0.0
        || feed_per_tooth_mm < 0.0
        || flute_count == 0
    {
        return Err(EngagementError::OutOfRange {
            reason: "non-positive engagement inputs".to_owned(),
        });
    }
    let arc = arc_engagement_radians.clamp(0.0, std::f64::consts::PI);
    let h_max = if arc >= std::f64::consts::PI {
        feed_per_tooth_mm
    } else {
        feed_per_tooth_mm * arc.sin().abs()
    };
    let mean = if arc > 1e-9 {
        (2.0 * h_max / arc) * (1.0 - (arc * 0.5).cos())
    } else {
        0.0
    };
    let helix_rad = helix_deg.to_radians();
    let edge_engagement_length_mm = axial_doc_mm / helix_rad.cos().abs().max(1e-6);
    let flute_pitch = std::f64::consts::TAU / flute_count as f64;
    let helix_wrap = axial_doc_mm * helix_rad.tan().abs() / radius;
    let instantaneous_flutes_in_cut =
        ((arc_engagement_radians + helix_wrap) / flute_pitch).clamp(0.0, flute_count as f64);
    Ok(ChipGeometry {
        max_chip_thickness_mm: h_max,
        mean_chip_thickness_mm: mean,
        edge_engagement_length_mm,
        instantaneous_flutes_in_cut,
    })
}

/// Tool-scale accessors answer SIX different geometric questions, and a
/// bare `f64` cannot tell them apart. The taxonomy, the full production
/// census, and the per-site verdicts live in
/// `planning/review_2026-07-29/TOOL_SCALE_SEMANTICS.md`:
///
/// | code | question | accessor |
/// |---|---|---|
/// | ENV | maximum swept envelope | [`MillingCutter::envelope_radius_mm`] (= [`MillingCutter::radius`]) |
/// | CUSP | tip-sphere feature scale | [`MillingCutter::cusp_radius_mm`] (= [`MillingCutter::cusp_radius`]) |
/// | WIDTH(d) | cutter width at axial engagement `d` | [`MillingCutter::engagement_radius_mm`] (= [`MillingCutter::engagement_radius`]) |
/// | CLEAR(r) | vertical clearance available at lateral radius `r` | [`MillingCutter::height_at_radius`] |
/// | VALLEY | two-wall / profile fit in a valley | no API today (H2) |
/// | HEURISTIC | path scale only, no physical contract | `radius()` by convention — say so at the site |
///
/// The `_mm`-suffixed names are documented aliases of the historical ones,
/// added by PR-2 (H1) with **no behavior change**: they exist so a reader
/// can see the semantic class at the call site, which is the exact defect
/// the audit found. Both spellings must always return the same number —
/// `tests/tool_scale_semantics_pr2.rs` pins that for every shape and for
/// the `ToolDefinition` wrapper.
///
/// There is deliberately **no** `feature_radius` and **no** fourth name for
/// `height_at_radius` (ADR, `TOOL_SCALE_SEMANTICS.md` §9).
pub trait MillingCutter: Send + Sync {
    fn diameter(&self) -> f64;
    fn radius(&self) -> f64 {
        self.diameter() / 2.0
    }
    /// ENVELOPE radius (mm) — the maximum lateral extent any part of the
    /// cutter sweeps, at any height. Documented alias of
    /// [`Self::radius`]; identical value, no new math.
    ///
    /// This is the correct and conservative answer for collision, bounding
    /// box padding, grid extent, spatial-query radii, swept-volume
    /// stamping, and coverage margins — and **only** for those. For a
    /// tapered ball it is the SHAFT radius (`diameter()` deliberately
    /// reports the widest point), so it overstates the cutter's reach at
    /// finishing depth by up to 14× — see [`Self::engagement_radius_mm`].
    fn envelope_radius_mm(&self) -> f64 {
        self.radius()
    }
    /// The radius that sets the FEATURE SCALE this tool can resolve — the
    /// tip sphere, not the widest point.
    ///
    /// [`Self::radius`] is the swept/collision radius, and for a tapered
    /// ball `diameter()` deliberately reports the SHAFT ("effective
    /// cutting diameter at widest point"). That is right for clearance and
    /// wrong for the question "how fine a feature does the CUTTING TIP
    /// resolve" — cusp height, minimum region area, morphological close
    /// radius, pencil claim floor, classification cell size.
    ///
    /// **This is not the same as "can the tool reach into that feature."**
    /// A tapered cutter's usable width grows with depth, so reach, fit and
    /// routing questions ("does an offset pass fit down this valley", "is
    /// this crease clearable") belong to a THIRD class that is neither the
    /// tip sphere nor the shaft envelope — the cone can foul a wall long
    /// before the tip bottoms out. Use [`Self::engagement_radius`] with the
    /// depth in hand for those; reaching for `cusp_radius()` there reports
    /// a cutter far more capable than it is. `Adaptive3dParams` already
    /// carries engagement and envelope radii separately for this reason.
    /// The finishing path still uses `radius()` at several such sites; they
    /// are inventoried in `planning/unified_v3_design.md` §14u.
    ///
    /// Getting this wrong is not academic: a Ø1 tip on a 6 mm shank reports
    /// `radius() = 3.0`, so decomposition dials derived from it came out
    /// 6× (lengths) and 36× (areas) too large, closing and absorbing every
    /// steep ribbon on a terrain that is 25% steeper than 55°. See
    /// `planning/unified_v3_design.md` §14q.
    fn cusp_radius(&self) -> f64 {
        match self.geometry_hint() {
            crate::feeds::ToolGeometryHint::TaperedBall { tip_radius, .. } => tip_radius,
            _ => self.radius(),
        }
    }
    /// CUSP radius (mm) — the tip-sphere radius that sets the finest
    /// feature this cutter can form. Documented alias of
    /// [`Self::cusp_radius`]; identical value, no new math.
    ///
    /// Use for cusp/scallop equations, minimum region area, morphological
    /// close radius, claim floors and classification cell size. **Not** an
    /// answer to "does the tool fit / reach in there" — see
    /// [`Self::cusp_radius`]'s doc for why, and
    /// [`Self::engagement_radius_mm`] / [`Self::height_at_radius`] for the
    /// queries that are.
    fn cusp_radius_mm(&self) -> f64 {
        self.cusp_radius()
    }
    fn length(&self) -> f64;
    fn helix_deg(&self) -> f64 {
        30.0
    }
    fn corner_radius_mm(&self) -> f64 {
        0.0
    }

    fn chip_geometry(
        &self,
        axial_doc_mm: f64,
        arc_engagement_radians: f64,
        feed_per_tooth_mm: f64,
        flute_count: u32,
        mode: EngagementMode,
    ) -> Result<ChipGeometry, EngagementError>;

    /// Profile height at radial distance r from tool axis.
    /// Returns the Z offset from the tool tip to the cutter surface at radius r.
    ///
    /// **This IS the profile-clearance query (CLEAR(r)).** It is the
    /// one-sided inverse of [`Self::width_at_height`]: for a
    /// vertical-walled slot of half-width `r`, the returned value is
    /// exactly how far the tip can descend below the rim before the
    /// profile touches a wall. `None` means `r` exceeds the whole
    /// envelope — the feature is wider than the cutter, which is a
    /// clearing job, not a fit question.
    ///
    /// Do **not** add a fourth accessor (`profile_height_mm`, …) as a
    /// synonym: this method already answers that question and is
    /// implemented by every shape (ADR, `TOOL_SCALE_SEMANTICS.md` §3.1/§9).
    ///
    /// The inverse is exact only where the profile is strictly widening.
    /// Where it is flat (a flat endmill's whole bottom, a bullnose inside
    /// its corner radius) many radii share height 0, so the round trip
    /// `width_at_height(height_at_radius(r))` returns the widest radius at
    /// that height and the guaranteed relation is `>= r`, not `== r`.
    /// `tests/tool_scale_semantics_pr2.rs` pins both forms.
    fn height_at_radius(&self, r: f64) -> Option<f64>;

    /// Profile radius at height h above tool tip.
    fn width_at_height(&self, h: f64) -> f64;

    /// Effective cutting radius at a given depth of cut below the tool tip.
    ///
    /// This is the cutter's actual contact width when engaging material to
    /// depth `depth_of_cut`. For flat endmills it equals `radius()` for any
    /// non-trivial depth; for ball/tapered/v-bit it varies with depth as the
    /// profile widens.
    ///
    /// Use this for stepover sizing, region detection, or any computation
    /// that asks "how wide is the cut at this engagement depth". Use
    /// `radius()` for the full envelope (keep-out, bbox margins, collision).
    ///
    /// The default implementation calls `width_at_height(depth_of_cut)`,
    /// which is the right behavior for all current cutter shapes. Returns 0
    /// when `depth_of_cut <= 0` for ball-tipped tools (only the tip touches);
    /// callers that need a non-zero floor should clamp.
    fn engagement_radius(&self, depth_of_cut: f64) -> f64 {
        self.width_at_height(depth_of_cut)
    }

    /// ENGAGED radius (mm) at axial engagement `depth_mm` — how wide the
    /// cutter actually is where it is cutting. Documented alias of
    /// [`Self::engagement_radius`]; identical value, no new math.
    ///
    /// This is the answer for stepover sizing and any "how much of the cut
    /// does the body occupy" question. Returns 0 at `depth_mm == 0` for
    /// every ball-tipped shape (only the tip touches), so callers that
    /// divide by it must floor — the established floors are
    /// `.max(0.01)` (`compute/execute.rs`), `.max(1.0e-6)`
    /// (`feeds/cutter_constraints.rs`) and `.max(cusp_radius_mm())`.
    fn engagement_radius_mm(&self, depth_mm: f64) -> f64 {
        self.engagement_radius(depth_mm)
    }

    fn lookup_diameter_at(&self, axial_doc_mm: f64) -> f64 {
        let _ = axial_doc_mm;
        self.diameter()
    }

    // ── diagnostics geometry ──────────────────────────────────────────
    // Shape-dependent quantities the tool-load gates ask the cutter for.
    // Defaults reproduce the cylinder (flat-endmill) behaviour so existing
    // shapes are unchanged; cone/ball cutters override where the geometry
    // genuinely differs. See planning/tool_diagnostics_generic_plan.md.

    /// Engaged chip cross-section area (mm²) for one cut, given the axial
    /// depth of cut and the arc-equivalent radial slab width the power
    /// gate already derives. This is the area the spindle power formula
    /// multiplies by feed.
    ///
    /// Default is the rectangular `axial_doc · radial_width` slab — exact
    /// for flat/bull-nose endmills and the established approximation for
    /// ball/tapered. A V-bit removes a **triangular** groove cross-section,
    /// so [`VBitEndmill`] overrides this (≈ half the rectangular slab).
    fn mrr_cross_section_mm2(&self, axial_doc_mm: f64, radial_width_mm: f64) -> f64 {
        axial_doc_mm * radial_width_mm
    }

    /// Flat-tip diameter (mm). 0.0 for a fully pointed cutter (the default
    /// for every shape); flat-tip / truncated V-bits override to report
    /// the diameter of the flat at the tip, which feeds the V-carve
    /// line-width and engagement models.
    fn flat_tip_diameter(&self) -> f64 {
        0.0
    }

    /// Key parameters for the generalized facet contact formula:
    /// radiusvector = xy_normal_length * xyNormal + normal_length * surfaceNormal
    fn center_height(&self) -> f64;
    fn normal_length(&self) -> f64;
    fn xy_normal_length(&self) -> f64;

    /// Test contact with a triangle vertex. Updates cl.z if this gives a higher position.
    fn vertex_drop(&self, cl: &mut CLPoint, vertex: &P3) {
        let dx = vertex.x - cl.x;
        let dy = vertex.y - cl.y;
        let q = (dx * dx + dy * dy).sqrt();
        if let Some(h) = self.height_at_radius(q) {
            cl.update_z(vertex.z - h);
        }
    }

    /// Test contact with a triangle facet. Updates cl.z if contact found.
    /// Returns true if contact was on the facet (inside the triangle).
    fn facet_drop(&self, cl: &mut CLPoint, tri: &Triangle) -> bool {
        let n = &tri.normal;
        // Skip nearly-vertical triangles
        if n.z.abs() < 1e-12 {
            return false;
        }

        // Compute the XY-normalized normal for the radius vector
        let nxy_len = (n.x * n.x + n.y * n.y).sqrt();
        let (xy_nx, xy_ny) = if nxy_len > 1e-15 {
            (n.x / nxy_len, n.y / nxy_len)
        } else {
            (0.0, 0.0)
        };

        // CC = CL - radiusvector (XY only)
        let r1 = self.xy_normal_length();
        let r2 = self.normal_length();
        let cc_x = cl.x - r1 * xy_nx - r2 * n.x;
        let cc_y = cl.y - r1 * xy_ny - r2 * n.y;

        // Check if CC is inside the triangle
        if !tri.contains_point_xy(cc_x, cc_y) {
            return false;
        }

        // Compute CC.z on the triangle plane
        let Some(cc_z) = tri.z_at_xy(cc_x, cc_y) else {
            return false;
        };

        // Compute the radiusvector Z component
        let rv_z = r2 * n.z;

        // CL.z = CC.z + rv_z - center_height
        let tip_z = cc_z + rv_z - self.center_height();

        cl.update_z(tip_z);
        true
    }

    /// Sample the cutter profile as (radius, height) pairs from center to edge.
    ///
    /// Returns `n + 1` points at evenly-spaced radii from 0 to `self.radius()`.
    /// Useful for rendering and visualization without hand-rolling per-shape geometry.
    fn profile_points(&self, n: usize) -> Vec<(f64, f64)> {
        let r = self.radius();
        (0..=n)
            .map(|i| {
                let dist = (i as f64 / n.max(1) as f64) * r;
                (dist, self.height_at_radius(dist).unwrap_or(0.0))
            })
            .collect()
    }

    /// Geometry classification for feeds/speeds effective-diameter calculation.
    ///
    /// Each tool type should override to return its specific hint variant.
    fn geometry_hint(&self) -> crate::feeds::ToolGeometryHint {
        crate::feeds::ToolGeometryHint::Flat
    }

    /// Test contact with a triangle edge. Updates cl.z if contact found.
    fn edge_drop(&self, cl: &mut CLPoint, p1: &P3, p2: &P3);

    /// Run the full drop-cutter test against a single triangle.
    fn drop_cutter(&self, cl: &mut CLPoint, tri: &Triangle) {
        // Facet test first (if hit, edge/vertex are redundant per OpenCAMLib)
        if self.facet_drop(cl, tri) {
            return;
        }

        // Vertex tests
        for v in &tri.v {
            self.vertex_drop(cl, v);
        }

        // Edge tests
        self.edge_drop(cl, &tri.v[0], &tri.v[1]);
        self.edge_drop(cl, &tri.v[1], &tri.v[2]);
        self.edge_drop(cl, &tri.v[2], &tri.v[0]);
    }
}

/// A complete tool definition: cutting geometry + assembly dimensions.
///
/// Wraps a `Box<dyn MillingCutter>` and adds shank/holder dimensions so that
/// collision detection, feeds calculation, and rendering can all derive their
/// inputs from a single source of truth.
///
/// Implements `MillingCutter` by delegating to the inner cutter.
pub struct ToolDefinition {
    cutter: Box<dyn MillingCutter>,
    /// Shank diameter above the cutting flutes (mm).
    pub shank_diameter: f64,
    /// Shank length above the cutting flutes (mm).
    pub shank_length: f64,
    /// Holder / collet diameter (mm).
    pub holder_diameter: f64,
    /// Total stickout from holder face to cutter tip (mm).
    pub stickout: f64,
    /// Number of cutting flutes.
    pub flute_count: u32,
    /// Cutter substrate material.
    pub tool_material: ToolMaterial,
}

impl ToolDefinition {
    pub fn new(
        cutter: Box<dyn MillingCutter>,
        shank_diameter: f64,
        shank_length: f64,
        holder_diameter: f64,
        stickout: f64,
        flute_count: u32,
        tool_material: ToolMaterial,
    ) -> Self {
        Self {
            cutter,
            shank_diameter,
            shank_length,
            holder_diameter,
            stickout,
            flute_count,
            tool_material,
        }
    }

    /// Computed holder length from stickout minus cutting length and shank.
    pub fn holder_length(&self) -> f64 {
        (self.stickout - self.cutter.length() - self.shank_length).max(0.0)
    }

    /// Build a `ToolAssembly` for collision detection.
    ///
    /// Uses `self.cutter.radius()` for the cutter envelope, which for
    /// `TaperedBallEndmill` correctly returns `shaft_diameter / 2` (the maximum
    /// cutting radius), not the ball tip radius.
    pub fn to_assembly(&self) -> crate::collision::ToolAssembly {
        crate::collision::ToolAssembly {
            cutter_radius: self.cutter.radius(),
            cutter_length: self.cutter.length(),
            shank_diameter: self.shank_diameter,
            shank_length: self.shank_length,
            holder_diameter: self.holder_diameter,
            holder_length: self.holder_length(),
        }
    }

    /// Derive the feeds geometry hint from the inner cutter.
    pub fn to_geometry_hint(&self) -> crate::feeds::ToolGeometryHint {
        self.cutter.geometry_hint()
    }

    /// Predicted tip deflection (mm) under a transverse point load
    /// `force_n` (N) applied at the midpoint of axial engagement
    /// (`axial_doc_mm / 2` above the tip).
    ///
    /// Models the tool as a stepped cantilever clamped at `x = 0` (the
    /// collet face) and free at `x = stickout` (the tip). The cutting
    /// region uses the per-cutter `lookup_diameter_at(axial_from_tip)`
    /// profile; above the flutes the cross-section is the uniform
    /// `shank_diameter`. Numerical integration via 64 mid-point segments
    /// along the bending region `[0, load_position]`; the section above
    /// the load (including the rigid cantilever extension to the tip)
    /// translates via the slope at the load point.
    ///
    /// Bending only — torsion is ignored because for a coaxial cutter
    /// twist rotates the tip around the axis without translating it.
    pub fn tip_deflection_mm(
        &self,
        force_n: f64,
        axial_doc_mm: f64,
        youngs_modulus_n_per_mm2: f64,
    ) -> f64 {
        let l = self.stickout;
        let e = youngs_modulus_n_per_mm2;
        if l <= 0.0 || e <= 0.0 || force_n == 0.0 {
            return 0.0;
        }
        let a_doc = axial_doc_mm.max(0.0);
        let load_pos = (l - a_doc * 0.5).max(0.0);
        if load_pos <= 0.0 {
            return 0.0;
        }
        let cutter_len = self.cutter.length();
        // Floor on local diameter to keep 1/d⁴ finite at degenerate
        // near-tip evaluations. 0.05 mm is well below any realistic
        // engaged cross-section.
        const D_FLOOR_MM: f64 = 0.05;
        // Split integration at the shank/cutter step so the discontinuity
        // never falls mid-segment (mid-point rule is O(1/N) at jumps,
        // O(1/N²) on smooth pieces).
        let shank_top = (l - cutter_len).max(0.0).min(load_pos);
        let mut delta_at_load = 0.0_f64;
        let mut slope_at_load = 0.0_f64;
        // Region 1: shank, x ∈ [0, shank_top].
        if shank_top > 0.0 {
            let d = self.shank_diameter.max(D_FLOOR_MM);
            let i_mm4 = std::f64::consts::PI * d.powi(4) / 64.0;
            const N_SHANK: usize = 32;
            let dx = shank_top / N_SHANK as f64;
            for i in 0..N_SHANK {
                let x = (i as f64 + 0.5) * dx;
                let arm = load_pos - x;
                let inv_ei = force_n / (e * i_mm4);
                delta_at_load += inv_ei * arm * arm * dx;
                slope_at_load += inv_ei * arm * dx;
            }
        }
        // Region 2: cutter, x ∈ [shank_top, load_pos].
        if load_pos > shank_top {
            const N_CUTTER: usize = 64;
            let span = load_pos - shank_top;
            let dx = span / N_CUTTER as f64;
            for i in 0..N_CUTTER {
                let x = shank_top + (i as f64 + 0.5) * dx;
                let axial_from_tip = l - x;
                let d = self
                    .cutter
                    .lookup_diameter_at(axial_from_tip)
                    .max(D_FLOOR_MM);
                let i_mm4 = std::f64::consts::PI * d.powi(4) / 64.0;
                let arm = load_pos - x;
                let inv_ei = force_n / (e * i_mm4);
                delta_at_load += inv_ei * arm * arm * dx;
                slope_at_load += inv_ei * arm * dx;
            }
        }
        let cantilever_extension = (l - load_pos).max(0.0);
        delta_at_load + slope_at_load * cantilever_extension
    }
}

impl MillingCutter for ToolDefinition {
    fn diameter(&self) -> f64 {
        self.cutter.diameter()
    }
    // EXPLICIT delegation for every tool-scale accessor, including the ones
    // that used to work only by inherited default over delegated primitives
    // (`radius` via `diameter()`, `cusp_radius` via `geometry_hint()`).
    // That was a latent trap: the first shape to override `cusp_radius()`
    // directly rather than through `geometry_hint()` would have seen
    // `ToolDefinition` — the only wrapper the production path actually
    // holds — silently revert to the trait default. `TOOL_SCALE_SEMANTICS.md`
    // §2.1/§7.4; pinned by `tests/tool_scale_semantics_pr2.rs`.
    fn radius(&self) -> f64 {
        self.cutter.radius()
    }
    fn envelope_radius_mm(&self) -> f64 {
        self.cutter.envelope_radius_mm()
    }
    fn cusp_radius(&self) -> f64 {
        self.cutter.cusp_radius()
    }
    fn cusp_radius_mm(&self) -> f64 {
        self.cutter.cusp_radius_mm()
    }
    fn engagement_radius_mm(&self, depth_mm: f64) -> f64 {
        self.cutter.engagement_radius_mm(depth_mm)
    }
    fn length(&self) -> f64 {
        self.cutter.length()
    }
    fn helix_deg(&self) -> f64 {
        self.cutter.helix_deg()
    }
    fn corner_radius_mm(&self) -> f64 {
        self.cutter.corner_radius_mm()
    }
    fn chip_geometry(
        &self,
        axial_doc_mm: f64,
        arc_engagement_radians: f64,
        feed_per_tooth_mm: f64,
        flute_count: u32,
        mode: EngagementMode,
    ) -> Result<ChipGeometry, EngagementError> {
        self.cutter.chip_geometry(
            axial_doc_mm,
            arc_engagement_radians,
            feed_per_tooth_mm,
            flute_count,
            mode,
        )
    }
    fn height_at_radius(&self, r: f64) -> Option<f64> {
        self.cutter.height_at_radius(r)
    }
    fn width_at_height(&self, h: f64) -> f64 {
        self.cutter.width_at_height(h)
    }
    fn engagement_radius(&self, depth_of_cut: f64) -> f64 {
        self.cutter.engagement_radius(depth_of_cut)
    }
    fn lookup_diameter_at(&self, axial_doc_mm: f64) -> f64 {
        self.cutter.lookup_diameter_at(axial_doc_mm)
    }
    fn mrr_cross_section_mm2(&self, axial_doc_mm: f64, radial_width_mm: f64) -> f64 {
        self.cutter
            .mrr_cross_section_mm2(axial_doc_mm, radial_width_mm)
    }
    fn flat_tip_diameter(&self) -> f64 {
        self.cutter.flat_tip_diameter()
    }
    fn center_height(&self) -> f64 {
        self.cutter.center_height()
    }
    fn normal_length(&self) -> f64 {
        self.cutter.normal_length()
    }
    fn xy_normal_length(&self) -> f64 {
        self.cutter.xy_normal_length()
    }
    fn profile_points(&self, n: usize) -> Vec<(f64, f64)> {
        self.cutter.profile_points(n)
    }
    fn geometry_hint(&self) -> crate::feeds::ToolGeometryHint {
        self.cutter.geometry_hint()
    }
    fn edge_drop(&self, cl: &mut CLPoint, p1: &P3, p2: &P3) {
        self.cutter.edge_drop(cl, p1, p2);
    }
    fn facet_drop(&self, cl: &mut CLPoint, tri: &Triangle) -> bool {
        self.cutter.facet_drop(cl, tri)
    }
    fn vertex_drop(&self, cl: &mut CLPoint, vertex: &P3) {
        self.cutter.vertex_drop(cl, vertex);
    }
    fn drop_cutter(&self, cl: &mut CLPoint, tri: &Triangle) {
        self.cutter.drop_cutter(cl, tri);
    }
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

    #[test]
    fn engagement_radius_matches_shape() {
        // Flat: full radius at any depth >= 0.
        let flat = FlatEndmill::new(6.0, 25.0);
        assert!((flat.engagement_radius(0.0) - 3.0).abs() < 1e-10);
        assert!((flat.engagement_radius(1.5) - 3.0).abs() < 1e-10);

        // Ball: 0 at the tip, full radius once depth >= ball_radius.
        let ball = crate::tool::BallEndmill::new(6.0, 25.0);
        assert!(ball.engagement_radius(0.0).abs() < 1e-10);
        assert!((ball.engagement_radius(3.0) - 3.0).abs() < 1e-10);
        // Mid-depth: chord through the sphere.
        let mid = ball.engagement_radius(1.0); // sqrt(2*3*1 - 1) = sqrt(5)
        assert!((mid - 5.0_f64.sqrt()).abs() < 1e-10);

        // Tapered ball: cone-widened above the ball region.
        let tapered = crate::tool::TaperedBallEndmill::new(2.0, 7.0, 6.0, 25.0);
        // At ball tip — zero engagement.
        assert!(tapered.engagement_radius(0.0).abs() < 1e-10);
        // At depth_per_pass=1.5, well into the cone.
        // Tip ball radius = 1.0, so at 1.5 above tip we're past the ball
        // and into the cone. Cone radius < shank/2 = 3.0.
        let r_at_dpp = tapered.engagement_radius(1.5);
        assert!(
            r_at_dpp > 1.0 && r_at_dpp < 3.0,
            "tapered ball engagement at 1.5mm should be tip<r<shank, got {}",
            r_at_dpp
        );
    }

    #[test]
    fn lookup_diameter_uses_shape_specific_engagement() {
        let flat = FlatEndmill::new(6.0, 25.0);
        assert!((flat.lookup_diameter_at(0.2) - 6.0).abs() < 1e-10);

        let ball = crate::tool::BallEndmill::new(6.0, 25.0);
        assert!((ball.lookup_diameter_at(0.2) - 6.0).abs() < 1e-10);

        let tapered = crate::tool::TaperedBallEndmill::new(2.0, 7.0, 6.0, 25.0);
        let tapered_lookup = tapered.lookup_diameter_at(1.5);
        assert!((tapered_lookup - 2.0 * tapered.engagement_radius(1.5)).abs() < 1e-10);

        let vbit = crate::tool::VBitEndmill::new(6.0, 90.0, 10.0);
        assert!((vbit.lookup_diameter_at(1.0) - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_cl_point() {
        let mut cl = CLPoint::new(5.0, 3.0);
        assert_eq!(cl.z, f64::NEG_INFINITY);
        cl.update_z(10.0);
        assert_eq!(cl.z, 10.0);
        cl.update_z(5.0); // lower, should not update
        assert_eq!(cl.z, 10.0);
        cl.update_z(15.0);
        assert_eq!(cl.z, 15.0);
    }

    #[test]
    fn test_profile_points_flat() {
        let tool = FlatEndmill::new(10.0, 25.0);
        let pts = tool.profile_points(10);
        assert_eq!(pts.len(), 11);
        // First point is center
        assert!((pts[0].0).abs() < 1e-10);
        assert!((pts[0].1).abs() < 1e-10);
        // Last point is at tool radius
        assert!((pts[10].0 - 5.0).abs() < 1e-10);
        // All heights should be 0 for flat endmill
        for &(_r, h) in &pts {
            assert!((h).abs() < 1e-10, "flat endmill profile should be 0");
        }
    }

    #[test]
    fn test_profile_points_ball() {
        let tool = BallEndmill::new(10.0, 25.0);
        let pts = tool.profile_points(10);
        assert_eq!(pts.len(), 11);
        // Center height = 0
        assert!((pts[0].1).abs() < 1e-10);
        // Heights should increase monotonically
        for i in 1..pts.len() {
            assert!(pts[i].1 >= pts[i - 1].1 - 1e-10);
        }
        // At full radius, height = R (top of hemisphere)
        let last = pts[10];
        assert!((last.0 - 5.0).abs() < 1e-10);
        assert!((last.1 - 5.0).abs() < 1e-8);
    }

    #[test]
    fn test_profile_points_tapered_ball() {
        let tool = TaperedBallEndmill::new(3.175, 15.0, 6.35, 25.0);
        let pts = tool.profile_points(50);
        assert_eq!(pts.len(), 51);
        // Center height = 0
        assert!((pts[0].1).abs() < 1e-10);
        // Heights should increase monotonically (no dips at junction)
        for i in 1..pts.len() {
            assert!(
                pts[i].1 >= pts[i - 1].1 - 1e-10,
                "profile not monotonic at i={}: h[{}]={} > h[{}]={}",
                i,
                i - 1,
                pts[i - 1].1,
                i,
                pts[i].1
            );
        }
    }

    #[test]
    fn test_geometry_hint_flat() {
        let tool = FlatEndmill::new(10.0, 25.0);
        assert_eq!(tool.geometry_hint(), crate::feeds::ToolGeometryHint::Flat);
    }

    #[test]
    fn test_geometry_hint_ball() {
        let tool = BallEndmill::new(10.0, 25.0);
        assert_eq!(tool.geometry_hint(), crate::feeds::ToolGeometryHint::Ball);
    }

    #[test]
    fn test_geometry_hint_bullnose() {
        let tool = BullNoseEndmill::new(10.0, 2.0, 25.0);
        let hint = tool.geometry_hint();
        match hint {
            crate::feeds::ToolGeometryHint::Bull { corner_radius } => {
                assert!((corner_radius - 2.0).abs() < 1e-10);
            }
            _ => panic!("expected Bull hint, got {:?}", hint),
        }
    }

    #[test]
    fn test_geometry_hint_vbit() {
        let tool = VBitEndmill::new(10.0, 90.0, 25.0);
        let hint = tool.geometry_hint();
        match hint {
            crate::feeds::ToolGeometryHint::VBit {
                included_angle,
                tip_diameter,
            } => {
                assert!((included_angle - 90.0).abs() < 1e-10);
                assert!((tip_diameter).abs() < 1e-10); // pointed
            }
            _ => panic!("expected VBit hint, got {:?}", hint),
        }
    }

    #[test]
    fn test_geometry_hint_tapered_ball() {
        let tool = TaperedBallEndmill::new(3.175, 15.0, 6.35, 25.0);
        let hint = tool.geometry_hint();
        match hint {
            crate::feeds::ToolGeometryHint::TaperedBall {
                tip_radius,
                taper_angle_deg,
            } => {
                assert!((tip_radius - 3.175 / 2.0).abs() < 1e-10);
                assert!((taper_angle_deg - 15.0).abs() < 1e-10);
            }
            _ => panic!("expected TaperedBall hint, got {:?}", hint),
        }
    }

    #[test]
    fn test_tool_definition_delegates() {
        let cutter = Box::new(BallEndmill::new(10.0, 25.0));
        let td = ToolDefinition::new(cutter, 6.35, 20.0, 25.0, 45.0, 2, ToolMaterial::Carbide);
        // Trait methods delegate correctly
        assert!((td.diameter() - 10.0).abs() < 1e-10);
        assert!((td.radius() - 5.0).abs() < 1e-10);
        assert!((td.length() - 25.0).abs() < 1e-10);
        assert!((td.center_height() - 5.0).abs() < 1e-10);
        // Profile sampling delegates
        let pts = td.profile_points(4);
        assert_eq!(pts.len(), 5);
    }

    #[test]
    fn test_tool_definition_assembly_flat() {
        let cutter = Box::new(FlatEndmill::new(10.0, 25.0));
        let td = ToolDefinition::new(cutter, 6.35, 20.0, 25.0, 50.0, 2, ToolMaterial::Carbide);
        let asm = td.to_assembly();
        assert!((asm.cutter_radius - 5.0).abs() < 1e-10);
        assert!((asm.cutter_length - 25.0).abs() < 1e-10);
        assert!((asm.shank_diameter - 6.35).abs() < 1e-10);
        assert!((asm.shank_length - 20.0).abs() < 1e-10);
        assert!((asm.holder_diameter - 25.0).abs() < 1e-10);
        assert!((asm.holder_length - 5.0).abs() < 1e-10); // 50 - 25 - 20
    }

    #[test]
    fn test_tool_definition_assembly_tapered_ball_uses_shaft_radius() {
        // This is the bug regression: collision must use shaft_radius, not ball_radius
        let cutter = Box::new(TaperedBallEndmill::new(3.175, 15.0, 6.35, 25.0));
        let td = ToolDefinition::new(cutter, 6.35, 20.0, 25.0, 50.0, 2, ToolMaterial::Carbide);
        let asm = td.to_assembly();
        // cutter_radius must be shaft_diameter/2 = 3.175, NOT ball_diameter/2 = 1.5875
        assert!(
            (asm.cutter_radius - 6.35 / 2.0).abs() < 1e-10,
            "expected shaft_radius {}, got {}",
            6.35 / 2.0,
            asm.cutter_radius
        );
    }

    #[test]
    fn test_tool_definition_geometry_hint() {
        let cutter = Box::new(BullNoseEndmill::new(10.0, 2.0, 25.0));
        let td = ToolDefinition::new(cutter, 6.35, 20.0, 25.0, 50.0, 2, ToolMaterial::Carbide);
        match td.to_geometry_hint() {
            crate::feeds::ToolGeometryHint::Bull { corner_radius } => {
                assert!((corner_radius - 2.0).abs() < 1e-10);
            }
            other => panic!("expected Bull, got {:?}", other),
        }
    }

    #[test]
    fn tip_deflection_uniform_cylinder_matches_closed_form() {
        // Uniform 6 mm flat cantilever (shank diameter = cutter diameter,
        // cutting_length covers stickout). Closed-form for a cantilever
        // with point load at distance `a` from clamp, deflection at tip
        // x = L:  δ = F·a²·(3L − a) / (6·E·I)
        let cutter = Box::new(FlatEndmill::new(6.0, 60.0));
        let td = ToolDefinition::new(cutter, 6.0, 100.0, 25.0, 45.0, 2, ToolMaterial::Carbide);
        let force = 270.0;
        let axial_doc = 6.0;
        let l = 45.0;
        let a = l - axial_doc * 0.5;
        let i = std::f64::consts::PI * 6.0_f64.powi(4) / 64.0;
        let e = 600_000.0;
        let expected = force * a * a * (3.0 * l - a) / (6.0 * e * i);
        let got = td.tip_deflection_mm(force, axial_doc, e);
        let rel_err = (got - expected).abs() / expected;
        assert!(
            rel_err < 0.01,
            "uniform cantilever integrator within 1%: got {got}, expected {expected}, rel_err={rel_err}"
        );
    }

    #[test]
    fn deflection_chain_matches_hand_calc_from_published_formulas() {
        // End-to-end external cross-check: the production deflection a user
        // sees (`feeds::predict::tip_deflection_from_engagement`) must equal
        // an independent hand calculation built from two published formulas
        // and nothing from the model internals:
        //   1. Wood force (woodresearch.sk): F = ap·(49.95·h + 5.30),
        //      h = fz·sin(θ_peak). At full immersion θ_peak = π/2 ⇒ h = fz.
        //   2. Textbook cantilever, point load at distance a from the clamp,
        //      tip deflection: δ = F·a²·(3L − a)/(6·E·I), a = L − ap/2.
        // Material/E are shared inputs (documented properties, not the thing
        // under test); the FORMULAS are what this pins. GenericHardwood is
        // the literature anchor wood, so its coefficients are the raw
        // 49.95 / 5.30 published values.
        use crate::material::{Material, WoodSpecies};
        let d = 6.0_f64;
        let l = 45.0_f64;
        // Uniform cylinder (cutting_length covers stickout) ⇒ the two-section
        // integrator reduces to the single-section textbook beam.
        let tool = ToolDefinition::new(
            Box::new(FlatEndmill::new(d, l)),
            d,
            0.0,
            25.0,
            l,
            2,
            ToolMaterial::Carbide,
        );
        let mat = Material::SolidWood {
            species: WoodSpecies::GenericHardwood,
        };
        let e = tool.tool_material.youngs_modulus_n_per_mm2();
        let ap = 3.0_f64;
        let fz = 0.05_f64;
        let immersion = std::f64::consts::PI; // full slot

        // (1) hand force from the published wood equation.
        let h = fz; // sin(π/2) = 1
        let force_hand = ap * (49.95 * h + 5.30);
        // (2) hand beam from the textbook cantilever formula.
        let a = l - ap / 2.0;
        let i = std::f64::consts::PI * d.powi(4) / 64.0;
        let delta_hand_mm = force_hand * a * a * (3.0 * l - a) / (6.0 * e * i);

        // Production path (force model + integrated beam, the real code).
        let delta_model_mm =
            crate::feeds::predict::tip_deflection_from_engagement(&tool, &mat, ap, immersion, fz)
                .expect("modeled deflection");

        let rel_err = (delta_model_mm - delta_hand_mm).abs() / delta_hand_mm;
        assert!(
            rel_err < 0.01,
            "production deflection chain must match hand calc from published formulas within 1%: \
             model {delta_model_mm:.6} mm, hand {delta_hand_mm:.6} mm (force {force_hand:.2} N), \
             rel_err {rel_err:.4}"
        );
    }

    #[test]
    fn tip_deflection_two_segment_stepped_matches_hand_calc() {
        // Cutter diameter 6 mm, cutting_length 20 mm; shank 12 mm;
        // stickout 40 mm; load at axial_doc/2 = 5 from tip → a = 35.
        // Hand-derivation in the G13 plan: δ_tip = (F/E) × 41.917 mm³.
        let cutter = Box::new(FlatEndmill::new(6.0, 20.0));
        let td = ToolDefinition::new(cutter, 12.0, 30.0, 25.0, 40.0, 2, ToolMaterial::Carbide);
        let force = 270.0;
        let axial_doc = 10.0;
        let e = 600_000.0;
        let expected = force * 41.917 / e;
        let got = td.tip_deflection_mm(force, axial_doc, e);
        let rel_err = (got - expected).abs() / expected;
        assert!(
            rel_err < 0.01,
            "stepped integrator within 1% of hand calc: got {got}, expected {expected}, rel_err={rel_err}"
        );
    }

    #[test]
    fn tip_deflection_tapered_ball_lies_between_shank_and_tip_limits() {
        // For a tapered ball with 2 mm tip / 6 mm shank, taper 7°,
        // cutting_length 35 mm, stickout 45 mm, the integrated tip
        // deflection should sit BETWEEN:
        //  - all-shank (uniform 6 mm cantilever) — stiffest bound
        //  - all-tip   (uniform 2 mm cantilever) — most-flexible bound
        // because the real profile transitions from 2 mm at the tip to
        // 6 mm at the top of the cutting region.
        let tapered = Box::new(TaperedBallEndmill::new(2.0, 7.0, 6.0, 35.0));
        let td_real = ToolDefinition::new(tapered, 6.0, 30.0, 25.0, 45.0, 2, ToolMaterial::Carbide);
        let td_shank = ToolDefinition::new(
            Box::new(FlatEndmill::new(6.0, 100.0)),
            6.0,
            30.0,
            25.0,
            45.0,
            2,
            ToolMaterial::Carbide,
        );
        let td_tip = ToolDefinition::new(
            Box::new(FlatEndmill::new(2.0, 100.0)),
            2.0,
            30.0,
            25.0,
            45.0,
            2,
            ToolMaterial::Carbide,
        );
        let force = 5.0;
        let axial_doc = 4.0;
        let e = 600_000.0;
        let real = td_real.tip_deflection_mm(force, axial_doc, e);
        let shank = td_shank.tip_deflection_mm(force, axial_doc, e);
        let tip = td_tip.tip_deflection_mm(force, axial_doc, e);
        assert!(
            shank < real && real < tip,
            "expected shank-limit {shank} < real {real} < tip-limit {tip}"
        );
    }

    #[test]
    fn tip_deflection_zero_force_is_zero() {
        let cutter = Box::new(FlatEndmill::new(6.0, 60.0));
        let td = ToolDefinition::new(cutter, 6.0, 30.0, 25.0, 45.0, 2, ToolMaterial::Carbide);
        assert_eq!(td.tip_deflection_mm(0.0, 6.0, 600_000.0), 0.0);
    }

    #[test]
    fn tip_deflection_carbide_stiffer_than_hss_by_3x() {
        // Same tool, same load, swap E. δ ∝ 1/E so the HSS deflection is 3×
        // the carbide deflection (200 vs 600 GPa).
        let cutter = Box::new(FlatEndmill::new(6.0, 60.0));
        let td = ToolDefinition::new(cutter, 6.0, 30.0, 25.0, 45.0, 2, ToolMaterial::Carbide);
        let carbide = td.tip_deflection_mm(100.0, 5.0, 600_000.0);
        let hss = td.tip_deflection_mm(100.0, 5.0, 200_000.0);
        let ratio = hss / carbide;
        assert!(
            (ratio - 3.0).abs() < 0.01,
            "HSS/Carbide deflection ratio should be 3.0, got {ratio}"
        );
    }

    #[test]
    fn test_tool_definition_holder_length() {
        let cutter = Box::new(FlatEndmill::new(10.0, 25.0));
        let td = ToolDefinition::new(cutter, 6.35, 20.0, 25.0, 50.0, 2, ToolMaterial::Carbide);
        assert!((td.holder_length() - 5.0).abs() < 1e-10);
        // Clamps to 0 if stickout is short
        let td2 = ToolDefinition::new(
            Box::new(FlatEndmill::new(10.0, 25.0)),
            6.35,
            20.0,
            25.0,
            30.0, // 30 - 25 - 20 = -15 -> clamped to 0
            2,
            ToolMaterial::Carbide,
        );
        assert!((td2.holder_length()).abs() < 1e-10);
    }
}
