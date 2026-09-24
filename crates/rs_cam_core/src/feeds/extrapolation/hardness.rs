//! G2: the hardness transfer, and the soft/hard cap
//! (`planning/extrapolation_2026-09-24/EXTRAPOLATION_G2.md`, P2 step 4,
//! orchestrator decision 4).
//!
//! Inside a material category a matched row serves the query through the
//! Janka law `(row / query)^0.5` (`CHIPLOAD_HARDNESS_EXPONENT`). The law
//! exceeds what the vendors print. For a hardwood row on a generic softwood
//! query the law gives x1.55, but no chart prints a softwood/hardwood ratio
//! above 1.50 (EXTRAPOLATION_G2 §1.6, §1.8 point 4). So inside solid wood
//! the transfer upward is capped at the largest ratio that the charts of the
//! row's tool family print ([`SOFT_OVER_HARD_PRINTED_MAX`]).
//!
//! The cap is a typed clamp, not an [`super::Extrapolation`] impl: it has no
//! range of diameters, and one cell can carry a size claim and a cap at once
//! (P2_PLAN §4). [`hardness_basis`] is the one place where the hardness
//! ratio and the hardness scale of a row come from. It runs inside
//! `vendor_lookup::build_result`, so every consumer reads one number.
//!
//! The raw ratio stays unchanged under the cap. The extrapolation flag
//! (`LookupResult::is_extrapolated`) reads the raw ratios, so a capped row
//! stays flagged.

use super::Gap;
use crate::feeds::vendor_lookup::{
    CHIPLOAD_HARDNESS_EXPONENT, LookupQuery, SCALE_CLAMP_HI, SCALE_CLAMP_LO, apply_chipload_law,
    material_category,
};
use crate::feeds::vendor_lut::{HardnessKind, MaterialFamily, ToolFamily, VendorObservation};

/// The largest softwood/hardwood chipload ratio that the charts of each
/// tool family print. Each value is the upper end of the "softwood /
/// hardwood" range in EXTRAPOLATION_G2 §1.2 (table T2, the ratio of the
/// band mids of one tool in the two columns):
///
/// - ball nose 1.50: range [1.20, 1.50], n 7 (Amana v7; §1.5 table T5 at
///   3.175 mm: soft 0.152, hard 0.102 mm, printed as 0.006 and 0.004 in,
///   so 0.006 / 0.004 = 1.50);
/// - flat end 1.43: range [1.00, 1.43], n 14;
/// - V-bit 1.42: range [1.00, 1.42], n 19 (Amana insert V-groove v16, §1.3);
/// - tapered ball 1.00: 1.00, n 2 (Onsrud 77-100 prints one band for both);
/// - facing bit 1.30: 1.30, n 1.
///
/// A bull nose has no printed row in two categories (§1.9), so it borrows
/// the flat-end value ([`soft_hard_cap`]).
pub const SOFT_OVER_HARD_PRINTED_MAX: &[(ToolFamily, f64)] = &[
    (ToolFamily::BallNose, 1.50),
    (ToolFamily::FlatEnd, 1.43),
    (ToolFamily::ChamferVbit, 1.42),
    (ToolFamily::TaperedBallNose, 1.00),
    (ToolFamily::FacingBit, 1.30),
];

/// Where the row's hardness value comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JankaFrom {
    /// The row prints its own `hardness_value`.
    Row,
    /// The row has no hardness value. A solid-wood row reads the generic
    /// species of its family (`WoodSpecies::GenericSoftwood` or
    /// `GenericHardwood`), the table that the query reads too.
    FamilyGeneric,
}

/// The cap for one tool family.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SoftHardCap {
    /// The row's tool family.
    pub family: ToolFamily,
    /// The largest printed softwood/hardwood ratio of that family.
    pub ratio: f64,
    /// True when the family has no printed pair and the flat-end value
    /// stands in (a bull nose).
    pub borrowed_from_flat: bool,
}

/// The cap for the row's tool family. A bull nose borrows the flat-end
/// value.
#[must_use]
pub fn soft_hard_cap(family: ToolFamily) -> SoftHardCap {
    let (printed, borrowed_from_flat) = match family {
        ToolFamily::BullNose => (ToolFamily::FlatEnd, true),
        ToolFamily::FlatEnd
        | ToolFamily::BallNose
        | ToolFamily::TaperedBallNose
        | ToolFamily::ChamferVbit
        | ToolFamily::FacingBit => (family, false),
    };
    // Every family except the bull nose has an entry, and the sentry pins
    // them. The fallback 1.0 blocks all upward transfer, which is the
    // conservative side.
    let ratio = SOFT_OVER_HARD_PRINTED_MAX
        .iter()
        .find(|(f, _)| *f == printed)
        .map_or(1.0, |(_, r)| *r);
    SoftHardCap {
        family,
        ratio,
        borrowed_from_flat,
    }
}

/// The hardness basis of one matched row for one query.
#[derive(Debug, Clone, PartialEq)]
pub enum HardnessBasis {
    /// No hardness transfer: the query or the row has no hardness value,
    /// or the two kinds differ. The scale is 1.0.
    Unscaled,
    /// A composite-board query (both plywoods, MDF, HDF, particleboard).
    /// These boards have no sourced Janka, so no scale applies (P2 step 3).
    CompositeBoard,
    /// The Janka law `(row / query)^0.5` on the raw ratio.
    Law {
        /// `row_hardness / query hardness`, clamped to
        /// `SCALE_CLAMP_LO..=SCALE_CLAMP_HI`.
        ratio_raw: f64,
        /// The applied scale, `ratio_raw^0.5`.
        scale: f64,
        /// The row's hardness value (lbf for Janka).
        row_hardness: f64,
        from: JankaFrom,
    },
    /// The law's scale is above 1 and above the cap of the row's tool
    /// family, inside solid wood. The applied scale is `cap.ratio`.
    Capped {
        ratio_raw: f64,
        /// The scale that the law gives, before the cap.
        law_scale: f64,
        row_hardness: f64,
        from: JankaFrom,
        cap: SoftHardCap,
    },
}

impl HardnessBasis {
    /// The raw ratio `row / query`. 1.0 when no transfer applies. The
    /// extrapolation flag reads this value.
    #[must_use]
    pub const fn ratio_raw(&self) -> f64 {
        match self {
            Self::Unscaled | Self::CompositeBoard => 1.0,
            Self::Law { ratio_raw, .. } | Self::Capped { ratio_raw, .. } => *ratio_raw,
        }
    }

    /// The scale that applies to the row's band.
    #[must_use]
    pub const fn scale(&self) -> f64 {
        match self {
            Self::Unscaled | Self::CompositeBoard => 1.0,
            Self::Law { scale, .. } => *scale,
            Self::Capped { cap, .. } => cap.ratio,
        }
    }

    /// The cap, when the basis is capped.
    #[must_use]
    pub const fn cap(&self) -> Option<&SoftHardCap> {
        match self {
            Self::Capped { cap, .. } => Some(cap),
            Self::Unscaled | Self::CompositeBoard | Self::Law { .. } => None,
        }
    }

    /// The short name of the basis, for the FM1 columns.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Unscaled => "Unscaled",
            Self::CompositeBoard => "CompositeBoard",
            Self::Law { .. } => "Law",
            Self::Capped { .. } => "Capped",
        }
    }

    /// The card text of a capped basis: a headline and a detail line.
    /// `None` for every other basis.
    ///
    /// Example (a ball nose, hardwood row 1450 on a 600 query):
    /// headline "extrapolated (G2 hardness): capped at x1.50";
    /// detail "hardness transfer capped at x1.50 (the largest
    /// softwood/hardwood ratio that ball nose charts print); the Janka law
    /// gives x1.55 (raw ratio 2.42, row 1450 lbf, printed on the row)".
    #[must_use]
    pub fn card_text(&self) -> Option<(String, String)> {
        let Self::Capped {
            ratio_raw,
            law_scale,
            row_hardness,
            from,
            cap,
        } = self
        else {
            return None;
        };
        let gap = Gap::Hardness;
        let ratio = cap.ratio;
        let headline = format!(
            "extrapolated ({} {}): capped at x{ratio:.2}",
            gap.group(),
            gap.label()
        );
        let whose = if cap.borrowed_from_flat {
            format!(
                "flat end mill charts print; no {} chart prints both columns, so the flat end \
                 mill value stands in",
                cap.family.label()
            )
        } else {
            format!("{} charts print", cap.family.label())
        };
        let origin = match from {
            JankaFrom::Row => "printed on the row",
            JankaFrom::FamilyGeneric => "the generic species of the row's family",
        };
        let detail = format!(
            "hardness transfer capped at x{ratio:.2} (the largest softwood/hardwood ratio that \
             {whose}); the Janka law gives x{law_scale:.2} (raw ratio {ratio_raw:.2}, row \
             {row_hardness:.0} lbf, {origin})"
        );
        Some((headline, detail))
    }
}

/// The Janka value (lbf) of a solid-wood row that has no per-row
/// `hardness_value`. The row and the query read ONE table: the generic
/// species of `material::WoodSpecies` (extrapolation P2 step 3,
/// orchestrator decision 3).
///
/// - A softwood row reads `WoodSpecies::GenericSoftwood.janka_lbf()` (600).
/// - A hardwood row reads `WoodSpecies::GenericHardwood.janka_lbf()` (1450).
/// - Every other family returns `None`.
///
/// Before 2026-09-24 this function had its own table (softwood 500,
/// hardwood 1290, MDF 700, plywood 550 / 1100, HDF 900, particleboard
/// 600). The query side read other values (`WoodSpecies`, `PlywoodGrade`,
/// `SheetGoodKind`). So a printed row got a hardness scale on a query of its
/// own family. Example: a printed MDF row with no Janka scaled from 700 on
/// an MDF query (1100). The result was (700 / 1100)^0.5 = x0.80 and the
/// "extrapolated" flag (extrapolation INVENTORY §2, item 2).
///
/// The row default still has a function. A hardwood row with no Janka on an
/// Ipe query (Janka 3510) derates by (1450 / 3510)^0.5. Without the default,
/// the query gets the full chipload of the row, 2 to 3 times too high
/// (`planning/feeds_literature_matrix_2026-06-03.md`, round 4
/// D_ipe_chipload finding).
fn family_default_janka(family: MaterialFamily) -> Option<f64> {
    use crate::material::WoodSpecies;
    match family {
        MaterialFamily::Softwood => Some(WoodSpecies::GenericSoftwood.janka_lbf()),
        MaterialFamily::Hardwood => Some(WoodSpecies::GenericHardwood.janka_lbf()),
        // The composite boards have no sourced Janka (see
        // `hardness_basis`). Plastics, aluminium and fibreglass have no
        // family Janka. The ratio is 1.0 for these families.
        _ => None,
    }
}

/// The hardness basis of the row `obs` for `query`. This is the one place
/// where the raw hardness ratio and the applied hardness scale come from.
///
/// The rules, in order:
///
/// 1. A composite-board query (`material_category` 5, the plywoods, or 6,
///    MDF, HDF and particleboard): [`HardnessBasis::CompositeBoard`]. This
///    applies to per-row values and to defaults. A composite board has no
///    sourced Janka. The only sourced figure is the particleboard minimum
///    of 500 lbf (ANSI A208.1), and a minimum is not a value to scale on.
///    The `PlywoodGrade` and `SheetGoodKind` values are density proxies for
///    the formula path; they do not scale a printed row.
/// 2. The query and the row have a hardness value of the same kind: the
///    law on `row / query` (`JankaFrom::Row`).
/// 3. The query has a Janka value and the row has none: a solid-wood row
///    reads `family_default_janka` (`JankaFrom::FamilyGeneric`). So an
///    extreme hardwood such as Ipe (Janka 3510) derates below a generic
///    hardwood row.
/// 4. Otherwise [`HardnessBasis::Unscaled`].
///
/// Chipload is roughly inverse with hardness: a softer material takes a
/// larger chipload at the same RPM. So a hardwood row on a softwood query
/// (`query < row`) gives a scale above 1.
///
/// The cap: when the query and the row are solid wood, the law's scale is
/// above 1, and it is above the cap of the ROW's tool family
/// ([`soft_hard_cap`]), the basis is [`HardnessBasis::Capped`]. This covers
/// every softer query, so a softer hardwood (Walnut) or a softer softwood
/// is capped too. The tapered-ball cap of 1.00 blocks every upward
/// transfer on a tapered row.
#[must_use]
pub fn hardness_basis(query: &LookupQuery, obs: &VendorObservation) -> HardnessBasis {
    if matches!(material_category(query.material_family), 5 | 6) {
        return HardnessBasis::CompositeBoard;
    }
    let row = match (
        query.hardness_kind,
        query.hardness_value,
        obs.hardness_kind,
        obs.hardness_value,
    ) {
        (Some(qk), Some(qv), Some(ok), Some(ov)) if qk == ok && qv > 0.0 && ov > 0.0 => {
            Some((qv, ov, JankaFrom::Row))
        }
        (Some(HardnessKind::Janka), Some(qv), _, _) if qv > 0.0 => {
            family_default_janka(obs.material_family).map(|ov| (qv, ov, JankaFrom::FamilyGeneric))
        }
        _ => None,
    };
    let Some((query_hardness, row_hardness, from)) = row else {
        return HardnessBasis::Unscaled;
    };
    let ratio_raw = (row_hardness / query_hardness).clamp(SCALE_CLAMP_LO, SCALE_CLAMP_HI);
    let law_scale = apply_chipload_law(ratio_raw, CHIPLOAD_HARDNESS_EXPONENT);
    let solid_wood = material_category(query.material_family) == 0
        && material_category(obs.material_family) == 0;
    if solid_wood && law_scale > 1.0 {
        let cap = soft_hard_cap(obs.tool_family);
        if law_scale > cap.ratio {
            return HardnessBasis::Capped {
                ratio_raw,
                law_scale,
                row_hardness,
                from,
                cap,
            };
        }
    }
    HardnessBasis::Law {
        ratio_raw,
        scale: law_scale,
        row_hardness,
        from,
    }
}
