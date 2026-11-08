//! Feeds contract P20d: THE engaged section of a cut, the one input of the
//! two-term power model (`tool_load::power::PowerTerms`).
//!
//! The power of a cut is the force line `dF = Ks h + F_edge` (ruling B6)
//! over the engaged section and the engaged edge:
//!
//! ```text
//! P = [Ks x A x vf + F_edge x Z x n x J] / 6e7          (kW)
//! A = the section the cut removes (mm^2)
//! J = integral over the engaged edge of psi x r ds     (mm^2)
//! ```
//!
//! `A x vf` is the removal rate. `J` is the edge length weighted by its
//! radius and by the arc `psi` it is engaged over: one element of edge at
//! the radius `r`, engaged over `psi` of each turn, moves `psi r` per turn.
//! Each variant below states its own `A` and `J`, derived by geometry from
//! the cutter shape and the cut. No variant has a default, and the two
//! constructors match [`ToolGeometryHint`] with no wildcard: a new shape is
//! a compile error, not a rectangle.
//!
//! - [`EngagedSection::Rectangle`]: a flat or a bull nose in a side cut.
//!   `A = ap ae`, `J = ap (d_eng / 2) psi`. For a ball and a tapered ball it
//!   is an upper bound: the crescent the tip removes is inside the
//!   rectangle on the engaged diameter.
//! - [`EngagedSection::VGroove`]: a V-bit in a side cut. The cone radius
//!   at the height `z` above the tip is `r(z) = tip/2 + z tan a` (the
//!   cutter's width rule, capped at `D/2` above the cone); the new width is
//!   `min(width, 2 r(z))` (two passes at one depth, `width` apart). A slot
//!   with no tip gives the triangle `A = h^2 tan a` and
//!   `J = pi R^2 / (2 sin a)`, `R = h tan a`.
//! - [`EngagedSection::AxialCircle`]: a flat end mill in a straight
//!   plunge. `A = pi D^2 / 4`; every flute cuts the whole turn along the
//!   radius `0..R`, so `J = integral of 2 pi r dr = pi D^2 / 4`. The chip is
//!   the axial chip `feed / (rpm Z)`.
//!
//! The force line is measured in side milling (Curti 2021, Goli 2018). A
//! V groove reads it on the cone; an axial plunge reads it on the end cut,
//! which no repo source measures for a flat end mill in wood
//! ([`SectionKind::basis_text`]). The axial figure is advisory: no feed,
//! speed or depth moves for it ([`SectionKind::gates`]).

use crate::feeds::ToolGeometryHint;
use crate::feeds::force::immersion_angle;

/// Feeds contract P20d: the engaged section of one cut. See the module doc
/// for `A` and `J` of each variant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EngagedSection {
    /// A side cut of a straight-sided cutter: the rectangle `ap x ae` on
    /// the engaged diameter `d_eng`, engaged over the arc `psi`.
    Rectangle {
        /// Axial depth (mm).
        ap_mm: f64,
        /// Radial width (mm).
        ae_mm: f64,
        /// Engaged diameter at the depth (mm).
        d_eng_mm: f64,
        /// Engagement arc (rad).
        psi_rad: f64,
    },
    /// A side cut of a V-bit: the cone (and the cylinder of D above it) to
    /// the depth, `width` wide at most at each height.
    VGroove {
        /// The depth of the cut below the stock top (mm).
        depth_mm: f64,
        /// Half the included angle (rad).
        half_angle_rad: f64,
        /// The flat tip diameter (mm); 0 for a pointed bit.
        tip_diameter_mm: f64,
        /// The width of new material at a height (mm): the stepover, or D
        /// (or more) for a slot.
        width_mm: f64,
        /// The cutter diameter (mm).
        diameter_mm: f64,
    },
    /// A straight plunge of a flat end mill: the full circle.
    AxialCircle {
        /// The cutter diameter (mm).
        diameter_mm: f64,
    },
}

/// The kind of an [`EngagedSection`], with its basis on the card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionKind {
    /// [`EngagedSection::Rectangle`].
    Rectangle,
    /// [`EngagedSection::VGroove`].
    VGroove,
    /// [`EngagedSection::AxialCircle`].
    AxialCircle,
}

impl SectionKind {
    /// The wire id (`shipped_power.section`).
    #[must_use]
    pub const fn wire_id(self) -> &'static str {
        match self {
            Self::Rectangle => "rectangle",
            Self::VGroove => "v_groove",
            Self::AxialCircle => "axial_circle",
        }
    }

    /// The section and the basis of its power, one sentence for every
    /// surface.
    #[must_use]
    pub const fn basis_text(self) -> &'static str {
        match self {
            Self::Rectangle => {
                "section: rectangle ap x ae (exact for a flat or a bull nose; an upper bound \
                 for a ball or a tapered ball)"
            }
            Self::VGroove => {
                "section: V groove (the cone to the depth, derived from the cutter geometry on \
                 the side-milling force line)"
            }
            Self::AxialCircle => {
                "section: axial circle pi D^2 / 4 (straight plunge); derived: side-milling Ks \
                 applied to the axial section; no end-cutting source for a flat end mill; \
                 advisory, not a gate"
            }
        }
    }

    /// `true` when a figure of this section may move a feed, a speed or a
    /// depth (the power ladder, Suggest pass 10, the envelope). The axial
    /// plunge is advisory: its force line is not measured on an end cut.
    #[must_use]
    pub const fn gates(self) -> bool {
        match self {
            Self::Rectangle | Self::VGroove => true,
            Self::AxialCircle => false,
        }
    }
}

/// Why [`EngagedSection::axial_plunge`] gives no section: the tip shape of
/// the plunging tool has no axial power model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxialSectionRefusal {
    /// A ball, a bull nose, a V-bit or a tapered ball plunges on a curved
    /// or a conical tip. No repo source or derivation here gives its
    /// engaged section during the plunge.
    TipShapeNotModelled,
}

impl EngagedSection {
    /// The section of a side cut of a tool of shape `tool` at the depth
    /// `ap_mm` and the width `ae_mm`. `d_eng_mm` and `psi_rad` are the
    /// engaged diameter and the arc of the one engagement (K7, P15); the
    /// rectangle reads them, the V groove reads the cone itself.
    #[must_use]
    pub fn side_cut(
        tool: ToolGeometryHint,
        diameter_mm: f64,
        ap_mm: f64,
        ae_mm: f64,
        d_eng_mm: f64,
        psi_rad: f64,
    ) -> Self {
        match tool {
            ToolGeometryHint::Flat
            | ToolGeometryHint::Bull { .. }
            | ToolGeometryHint::Ball
            | ToolGeometryHint::TaperedBall { .. } => Self::Rectangle {
                ap_mm,
                ae_mm,
                d_eng_mm,
                psi_rad,
            },
            ToolGeometryHint::VBit {
                included_angle,
                tip_diameter,
            } => Self::VGroove {
                depth_mm: ap_mm,
                half_angle_rad: (included_angle * 0.5).to_radians(),
                tip_diameter_mm: tip_diameter,
                width_mm: ae_mm,
                diameter_mm,
            },
        }
    }

    /// The section of a straight plunge of a tool of shape `tool` and
    /// diameter `diameter_mm`.
    ///
    /// # Errors
    ///
    /// [`AxialSectionRefusal::TipShapeNotModelled`] for every shape but the
    /// flat end mill.
    pub fn axial_plunge(
        tool: ToolGeometryHint,
        diameter_mm: f64,
    ) -> Result<Self, AxialSectionRefusal> {
        match tool {
            ToolGeometryHint::Flat => Ok(Self::AxialCircle { diameter_mm }),
            ToolGeometryHint::Ball
            | ToolGeometryHint::Bull { .. }
            | ToolGeometryHint::VBit { .. }
            | ToolGeometryHint::TaperedBall { .. } => Err(AxialSectionRefusal::TipShapeNotModelled),
        }
    }

    /// The kind of this section.
    #[must_use]
    pub const fn kind(&self) -> SectionKind {
        match self {
            Self::Rectangle { .. } => SectionKind::Rectangle,
            Self::VGroove { .. } => SectionKind::VGroove,
            Self::AxialCircle { .. } => SectionKind::AxialCircle,
        }
    }

    /// The section area `A` (mm^2), never below 0. A degenerate input
    /// gives 0 (no section), as the rectangle always did.
    #[must_use]
    pub fn area_mm2(&self) -> f64 {
        let area = match *self {
            Self::Rectangle { ap_mm, ae_mm, .. } => ap_mm * ae_mm,
            Self::VGroove { .. } => self.v_integrals().map_or(0.0, |(a, _)| a),
            Self::AxialCircle { diameter_mm } => {
                std::f64::consts::PI * diameter_mm * diameter_mm / 4.0
            }
        };
        area.max(0.0)
    }

    /// The edge term (kW) at `rpm` for `flute_count` flutes, with
    /// `edge_scale = grain factor x F_edge` (N/mm). 0 when an input is not
    /// usable (no arc, no depth, no diameter, no speed, no flute): the
    /// model then reads the shear term alone.
    ///
    /// The rectangle keeps the pre-P20d arithmetic in its order,
    /// `edge_scale ap (pi d n) (Z psi / 2pi) / 6e7`, so a flat cell moves
    /// by no bit.
    #[must_use]
    pub(crate) fn edge_kw(&self, edge_scale: f64, spindle_rpm: f64, flute_count: f64) -> f64 {
        let speed_usable = spindle_rpm.is_finite()
            && spindle_rpm > 0.0
            && flute_count.is_finite()
            && flute_count > 0.0;
        if !speed_usable {
            return 0.0;
        }
        match *self {
            Self::Rectangle {
                ap_mm,
                d_eng_mm,
                psi_rad,
                ..
            } => {
                let psi = psi_rad.clamp(0.0, std::f64::consts::PI);
                let usable = psi > 0.0
                    && ap_mm.is_finite()
                    && ap_mm > 0.0
                    && d_eng_mm.is_finite()
                    && d_eng_mm > 0.0;
                if !usable {
                    return 0.0;
                }
                let duty = flute_count * psi / std::f64::consts::TAU;
                let vc_mm_min = std::f64::consts::PI * d_eng_mm * spindle_rpm;
                edge_scale * ap_mm * vc_mm_min * duty / 60_000_000.0
            }
            Self::VGroove { .. } => match self.v_integrals() {
                Some((_, j)) if j > 0.0 => {
                    edge_scale * flute_count * spindle_rpm * j / 60_000_000.0
                }
                Some(_) | None => 0.0,
            },
            Self::AxialCircle { diameter_mm } => {
                if !(diameter_mm.is_finite() && diameter_mm > 0.0) {
                    return 0.0;
                }
                let j = std::f64::consts::PI * diameter_mm * diameter_mm / 4.0;
                edge_scale * flute_count * spindle_rpm * j / 60_000_000.0
            }
        }
    }

    /// The V groove's `(A, J)`. `None` when the shape is degenerate (no
    /// depth, no width, an angle outside `(0, 90)` deg, no diameter, or a
    /// tip at least as wide as the cutter).
    fn v_integrals(&self) -> Option<(f64, f64)> {
        let Self::VGroove {
            depth_mm,
            half_angle_rad,
            tip_diameter_mm,
            width_mm,
            diameter_mm,
        } = *self
        else {
            return None;
        };
        let (sin_a, tan_a) = (half_angle_rad.sin(), half_angle_rad.tan());
        let r_tip = tip_diameter_mm.max(0.0) / 2.0;
        let r_max = diameter_mm / 2.0;
        let usable = depth_mm.is_finite()
            && depth_mm > 0.0
            && width_mm.is_finite()
            && width_mm > 0.0
            && tan_a.is_finite()
            && tan_a > 0.0
            && sin_a > 0.0
            && r_max.is_finite()
            && r_max > r_tip;
        if !usable {
            return None;
        }
        // The cone runs from the tip (r_tip) to the cutter radius over
        // `cone_h`; above it the cutter is the cylinder of D.
        let cone_h = (r_max - r_tip) / tan_a;
        let h_cone = depth_mm.min(cone_h);
        let r_hi = r_tip + h_cone * tan_a;
        // On the cone dz = dr / tan a and ds = dr / sin a.
        let mut area = (width_integral(r_hi, width_mm) - width_integral(r_tip, width_mm)) / tan_a;
        let mut j = (arc_integral(r_hi, width_mm) - arc_integral(r_tip, width_mm)) / sin_a;
        if depth_mm > cone_h {
            let over = depth_mm - cone_h;
            let w = width_mm.min(diameter_mm);
            area += w * over;
            j += immersion_angle(w, r_max) * r_max * over;
        }
        Some((area, j))
    }
}

/// `integral of min(w, 2 r) dr` from 0 to `r`: `r^2` while the cone is
/// narrower than the width (`r <= w / 2`), then `w^2 / 4 + w (r - w/2)`.
fn width_integral(r: f64, w: f64) -> f64 {
    if r <= w / 2.0 {
        r * r
    } else {
        w * w / 4.0 + w * (r - w / 2.0)
    }
}

/// `integral of psi(r) r dr` from 0 to `r`, with `psi(r) = pi` while
/// `2 r <= w` and `acos(1 - w / r)` above. Closed form above `w / 2`:
/// `r^2/2 acos(1 - w/r) + (w/4) sqrt((2r - w)/w) ((2r - w)/3 + w)`, which
/// equals `pi w^2 / 8` at `r = w/2`, the value of the lower branch there.
fn arc_integral(r: f64, w: f64) -> f64 {
    if r <= w / 2.0 {
        std::f64::consts::PI * r * r / 2.0
    } else {
        let s = 2.0 * r - w;
        r * r / 2.0 * (1.0 - w / r).clamp(-1.0, 1.0).acos()
            + (w / 4.0) * (s / w).sqrt() * (s / 3.0 + w)
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use super::*;

    fn v60(depth: f64, width: f64, d: f64) -> EngagedSection {
        EngagedSection::side_cut(
            ToolGeometryHint::VBit {
                included_angle: 60.0,
                tip_diameter: 0.0,
            },
            d,
            depth,
            width,
            0.0,
            0.0,
        )
    }

    /// A V slot is the triangle: `A = h^2 tan a`, `J = pi R^2 / (2 sin a)`.
    #[test]
    fn a_v_slot_is_the_triangle() {
        let a = 30.0_f64.to_radians();
        for h in [0.3, 1.0, 2.6] {
            let s = v60(h, 6.35, 6.35);
            let r = h * a.tan();
            assert!((s.area_mm2() - h * h * a.tan()).abs() < 1e-12);
            let edge = s.edge_kw(1.0, 1.0, 1.0) * 60_000_000.0;
            assert!((edge - PI * r * r / (2.0 * a.sin())).abs() < 1e-9 * edge);
        }
    }

    /// The closed form of the arc integral meets a fine midpoint sum.
    #[test]
    fn the_arc_integral_meets_the_midpoint_sum() {
        let (w, r) = (0.7_f64, 2.3_f64);
        let n = 200_000;
        let step = r / n as f64;
        let sum: f64 = (0..n)
            .map(|i| {
                let x = (i as f64 + 0.5) * step;
                immersion_angle(w.min(2.0 * x), x) * x * step
            })
            .sum();
        assert!(
            (arc_integral(r, w) - sum).abs() < 1e-6,
            "{} vs {sum}",
            arc_integral(r, w)
        );
    }

    /// A narrow width cuts a band of the width on the cone.
    #[test]
    fn a_narrow_width_is_clipped() {
        let a = 30.0_f64.to_radians();
        let (h, w) = (3.0, 0.4);
        let z_star = w / (2.0 * a.tan());
        let want = w * h - w * z_star / 2.0;
        assert!((v60(h, w, 12.7).area_mm2() - want).abs() < 1e-12);
    }

    /// Above the cone the cutter is the cylinder of D.
    #[test]
    fn above_the_cone_the_section_is_the_cylinder() {
        let a = 30.0_f64.to_radians();
        let d = 6.35;
        let cone_h = d / 2.0 / a.tan();
        let s = v60(cone_h + 2.0, d, d);
        let want = cone_h * cone_h * a.tan() + d * 2.0;
        assert!((s.area_mm2() - want).abs() < 1e-9);
    }

    /// Only a flat end mill has an axial section; every other tip refuses.
    #[test]
    fn only_a_flat_tip_has_an_axial_section() {
        let s = EngagedSection::axial_plunge(ToolGeometryHint::Flat, 6.0).map(|s| s.area_mm2());
        assert_eq!(s, Ok(PI * 9.0));
        for hint in [
            ToolGeometryHint::Ball,
            ToolGeometryHint::Bull { corner_radius: 1.0 },
            ToolGeometryHint::VBit {
                included_angle: 60.0,
                tip_diameter: 0.0,
            },
            ToolGeometryHint::TaperedBall {
                tip_radius: 0.5,
                taper_angle_deg: 7.0,
            },
        ] {
            assert_eq!(
                EngagedSection::axial_plunge(hint, 6.0),
                Err(AxialSectionRefusal::TipShapeNotModelled)
            );
        }
    }
}
