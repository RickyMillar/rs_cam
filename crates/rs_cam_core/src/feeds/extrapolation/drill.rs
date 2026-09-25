//! G6: the drill claim. A flat end mill plunge reads the tool's own printed
//! side row (`planning/extrapolation_2026-09-24/B5_PLAN.md` §1, ruling B5).
//!
//! The LUT has no rows filed under the drill family (`vendor_lut`,
//! `KNOWN_EMPTY`). One chart prints a plunge figure for a router end mill:
//! the Amana Spektra Spiral Plunge chart v24. It prints "Ramp Down" = feed
//! rate / flutes at one RPM (18 000). The feed rate is RPM x side chip x Z,
//! so the axial advance per tooth is the side chip / Z
//! ([`DRILL_RULE_TEXT`]). The side rows are in the LUT
//! (`amana_flat_end.json`, filed under (Pocket, Roughing)). A
//! [`DrillRule`] states which rows are of this kind, and [`DrillClaim`]
//! moves the row's chip by x1/Z.
//!
//! The claim is a G6 sibling of G3, not a G3 rule: a family rule moves no
//! number (`family`), and this rule moves one.
//!
//! The rule works in the same three places as a family rule, all in
//! `vendor_lookup`: `passes_must_match` lets the home row through,
//! `score_observation` gives the +45 role term, and `beats` prefers a row
//! printed in the queried family. [`drill_basis`] runs in `build_result`,
//! and `build_result` multiplies the row's chip by [`DrillBasis::scale`].
//! A printed point stays a point.
//!
//! The recipe holds the chip per tooth. Step 2c of `feeds::calculate`
//! clamps the chart's 18 000 RPM to the engine drill RPM cap (a repo rule,
//! tiered by diameter: 8 000-14 000 at D <= 6 mm, 6 000-10 000 at D <= 10 mm,
//! 4 000-8 000 above that), so the feed comes out at about 0.78x, 0.56x or
//! 0.44x the printed Ramp Down, by tier. The card states it (orchestrator
//! decision 1).

use std::ops::RangeInclusive;

use super::size::EXACT_DIAMETER_TOLERANCE;
use super::{ClaimConfidence, Gap};
use crate::feeds::vendor_lookup::LookupQuery;
use crate::feeds::vendor_lut::{LutOperationFamily, LutPassRole, ToolFamily, VendorObservation};

/// The drill rule in words, for the detail line.
pub const DRILL_RULE_TEXT: &str = "Amana prints Ramp Down = feed rate / flutes at one RPM: \
     axial chip per tooth = side chip / Z";

/// The peck depth line of the card. No vendor prints a per-peck depth, so
/// the depth that Suggest writes is a repo rule
/// (`Material::drill_per_peck_max_dtd`, `apply_drill_defaults`).
pub const DRILL_PECK_TEXT: &str = "peck depth: repo rule (per-peck max 6/5/4 x D by Janka, \
     1.5 x D sheet; Suggest writes half); no vendor prints a per-peck depth (G6 found none)";

/// One drill rule: the side rows it reads and the plunges it serves.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DrillRule {
    /// The tool family of the rows. The query must have the same family, so
    /// the bull-to-flat tool fallback never carries the claim onto a bull
    /// nose.
    pub tool_family: ToolFamily,
    /// The `source_id` values of the charts that print the rule.
    pub source_ids: &'static [&'static str],
    /// The `tool_subfamily` of the rows.
    pub tool_subfamily: &'static str,
    /// The (operation family, pass role) that the LUT files the side rows
    /// under. The adaptive copies of the same chart line are not the home,
    /// so no printed line serves a plunge twice.
    pub home: (LutOperationFamily, LutPassRole),
    /// The smallest and the largest diameter (mm) where a verified Ramp
    /// Down figure supports the rule. Both the query key and the row must
    /// be inside it.
    pub range_mm: (f64, f64),
    /// The flute counts that the chart prints.
    pub flutes: &'static [u32],
    /// What supports the rule.
    pub witness: &'static str,
}

/// The Amana Spektra chart (2 and 3 flutes).
const AMANA_SPEKTRA_SOURCES: &[&str] = &["amana_spektra_spiral_plunge_v24"];

/// The drill rules (ruling B5; range widened 2026-09-25, the Spektra sizes
/// ruling, `fetch/G6/SPEKTRA_SIZES.md` §5-6).
///
/// The range is 3.0-12.7 mm: the Ramp Down identity (Feed Rate / flutes)
/// holds at every printed size in it, both flute counts, both columns (28
/// cells; the worst case is +0.9 %, 3 Flute 3/8 in MDF). Below 3.0 mm every
/// printed row carries the chart's breakage warning. The only printed row
/// above 12.7 mm (3 Flute 3/4 in) does not agree with the chart's 18 000 RPM
/// header (its feed implies about 12 000 RPM) and is held out. Amana prints
/// 2 and 3 flutes only, so a 1- or 4-flute end mill refuses (decision 6).
pub const DRILL_RULES: &[DrillRule] = &[DrillRule {
    tool_family: ToolFamily::FlatEnd,
    source_ids: AMANA_SPEKTRA_SOURCES,
    tool_subfamily: "spektra_spiral_plunge",
    home: (LutOperationFamily::Pocket, LutPassRole::Roughing),
    range_mm: (3.0, 12.7),
    flutes: &[2, 3],
    witness: "Amana Spektra Spiral Plunge chart v24 prints Ramp Down = Feed Rate IPM / # of \
              flutes at 18,000 RPM; the identity holds at every printed size from 3.0 mm to \
              1/2 in (12.7 mm), 2 and 3 flutes, Wood/Plywood and MDF/Laminate (28 cells; worst \
              case +0.9%, 3 Flute 3/8 in MDF)",
}];

/// True when `d` (mm) is inside the rule's range, with
/// [`EXACT_DIAMETER_TOLERANCE`] (relative) at each end.
fn in_range(rule: &DrillRule, d: f64) -> bool {
    let (lo, hi) = rule.range_mm;
    d.is_finite()
        && d > 0.0
        && d / lo >= 1.0 - EXACT_DIAMETER_TOLERANCE
        && d / hi <= 1.0 + EXACT_DIAMETER_TOLERANCE
}

/// The rule that carries `obs` into the queried drill cell, or `None`.
///
/// A rule applies when all of these conditions are true:
///
/// - the query family is `Drill`, and the row is from another family;
/// - the query, the row and the rule have the same tool family (flat end);
/// - the row's `source_id` is one of the rule's sources, its
///   `tool_subfamily` is the rule's subfamily, and it is filed under the
///   rule's home;
/// - the query key and the row diameter are both inside the rule's range;
/// - the query's flute count and the row's flute count are both printed.
///
/// The caller still applies the material filter.
#[must_use]
pub fn drill_rule(query: &LookupQuery, obs: &VendorObservation) -> Option<&'static DrillRule> {
    if query.operation_family != LutOperationFamily::Drill
        || obs.operation_family == query.operation_family
    {
        return None;
    }
    DRILL_RULES.iter().find(|rule| {
        rule.tool_family == query.tool_family
            && obs.tool_family == rule.tool_family
            && rule.source_ids.contains(&obs.source_id.as_str())
            && obs.tool_subfamily.as_deref() == Some(rule.tool_subfamily)
            && (obs.operation_family, obs.pass_role) == rule.home
            && rule.flutes.contains(&query.flute_count)
            && rule.flutes.contains(&obs.flute_count)
            && in_range(rule, query.diameter_mm)
            && obs.diameter_mm.is_some_and(|d| in_range(rule, d))
    })
}

/// One drill claim: the side row's chip / Z is the axial chip per tooth of
/// a straight plunge.
#[derive(Debug, Clone, PartialEq)]
pub struct DrillClaim {
    /// Always [`Gap::Drill`].
    pub gap: Gap,
    /// The rule in words ([`DRILL_RULE_TEXT`]).
    pub rule: &'static str,
    /// The scale on the row's chip: `1 / flutes`.
    pub scale: f64,
    /// The query's flute count (Z).
    pub flutes: u32,
    /// The side row that the claim reads.
    pub side_row: String,
    /// The row's `source_id`.
    pub source_id: String,
    /// The (family, role) the side row is filed under.
    pub home: (LutOperationFamily, LutPassRole),
    /// The row's printed material label (the chart column).
    pub material_label: String,
    /// The query's diameter (mm). The rule's range now spans three RPM
    /// tiers (`drill_rpm_envelope_for_diameter`), so the card reads the
    /// cap at the query's own diameter, not at the range's upper end.
    pub diameter_mm: f64,
    /// The diameters (mm) where the rule is valid.
    pub range_mm: RangeInclusive<f64>,
    /// The RPM the chart prints (the row's `rpm_nominal`).
    pub printed_rpm: Option<f64>,
    /// What supports the rule.
    pub witness: &'static str,
    pub confidence: ClaimConfidence,
}

impl DrillClaim {
    /// The card text: a headline and a detail line, in plain words.
    ///
    /// Example: headline "vendor row, plunge claim (G6 drill): the side chip
    /// of the pocket row / 2 flutes (x0.50)"; the detail states the rule,
    /// the row and its column, the range, the "Ramp Down" reading, the
    /// chip held per tooth, the RPM cap and the feed ratio, the peck depth
    /// rule and the witness.
    #[must_use]
    pub fn card_text(&self) -> (String, String) {
        let headline = format!(
            "vendor row, plunge claim ({} {}): the side chip of the {} row / {} flutes (x{:.2})",
            self.gap.group(),
            self.gap.label(),
            self.home.0.label(),
            self.flutes,
            self.scale
        );
        let lo = *self.range_mm.start();
        let hi = *self.range_mm.end();
        // The Wood/Plywood column is one column for all wood and plywood.
        // Softwood and hardwood read it as a derived grade b row (R5, A4).
        let column = if self
            .material_label
            .to_ascii_lowercase()
            .contains("wood/plywood")
        {
            format!(
                "the chart's \"{}\" column (softwood and hardwood read it as derived grade b)",
                self.material_label
            )
        } else {
            format!("the chart's \"{}\" column", self.material_label)
        };
        // The engine drill RPM cap is tiered by diameter
        // (`drill_rpm_envelope_for_diameter`); the range now spans three
        // tiers (<=6 mm, <=10 mm, >10 mm), so the card reads the cap at the
        // query's own diameter, not at the range's upper end.
        let (cap_lo, cap_hi) = crate::feeds::drill_rpm_envelope_for_diameter(self.diameter_mm);
        let tier_hi = if self.diameter_mm <= 6.0 {
            6.0
        } else if self.diameter_mm <= 10.0 {
            10.0
        } else {
            self.diameter_mm
        };
        let rpm = match self.printed_rpm {
            Some(printed) if printed > 0.0 => {
                let ratio = cap_hi.min(printed) / printed;
                format!(
                    "the chart's {printed:.0} RPM is replaced by the engine drill RPM cap \
                     ({cap_lo:.0}-{cap_hi:.0} RPM at D <= {tier_hi:?} mm, a repo rule), and the \
                     feed follows (about x{ratio:.2} of the printed Ramp Down)"
                )
            }
            _ => format!(
                "the chart prints no RPM; the engine drill RPM cap ({cap_lo:.0}-{cap_hi:.0} RPM \
                 at D <= {tier_hi:?} mm, a repo rule) sets it"
            ),
        };
        let detail = format!(
            "{}; row {} ({}), {column}; valid {lo:?}-{hi:?} mm, 2 or 3 flutes; Amana names the \
             column \"Ramp Down\" and ruling B5 reads it as a straight plunge; the chip is held \
             per tooth; {rpm}; {DRILL_PECK_TEXT}; one witness ({})",
            self.rule, self.side_row, self.source_id, self.witness
        );
        (headline, detail)
    }
}

/// The drill basis of one matched row for one query.
#[derive(Debug, Clone, PartialEq)]
pub enum DrillBasis {
    /// No drill rule reads the row: the row answers as printed.
    Printed,
    /// The drill rule reads the row's side chip as a plunge chip.
    Transferred(Box<DrillClaim>),
}

impl DrillBasis {
    /// The claim, when a drill rule reads the row.
    #[must_use]
    pub fn claim(&self) -> Option<&DrillClaim> {
        match self {
            Self::Printed => None,
            Self::Transferred(claim) => Some(claim),
        }
    }

    /// The short name of the basis, for the FM1 columns.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Printed => "Printed",
            Self::Transferred(_) => "Transferred",
        }
    }

    /// The scale on the row's chip: 1.0, or `1 / flutes` for a claim.
    #[must_use]
    pub fn scale(&self) -> f64 {
        match self {
            Self::Printed => 1.0,
            Self::Transferred(claim) => claim.scale,
        }
    }
}

/// The drill basis of the row `obs` for `query`. A pure function of the
/// two, like `family_basis`; both resolvers call it in `build_result`.
#[must_use]
pub fn drill_basis(query: &LookupQuery, obs: &VendorObservation) -> DrillBasis {
    let Some(rule) = drill_rule(query, obs) else {
        return DrillBasis::Printed;
    };
    // `drill_rule` accepts only the rule's flute counts (2 or 3), so the
    // divisor is never zero.
    let flutes = query.flute_count;
    DrillBasis::Transferred(Box::new(DrillClaim {
        gap: Gap::Drill,
        rule: DRILL_RULE_TEXT,
        scale: 1.0 / f64::from(flutes),
        flutes,
        side_row: obs.observation_id.clone(),
        source_id: obs.source_id.clone(),
        home: rule.home,
        material_label: obs.material_label.clone(),
        diameter_mm: query.diameter_mm,
        range_mm: rule.range_mm.0..=rule.range_mm.1,
        printed_rpm: obs.rpm_nominal,
        witness: rule.witness,
        confidence: ClaimConfidence::OneWitness,
    }))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::feeds::vendor_lut::{HardnessKind, MaterialFamily, VendorLut};

    fn row(lut: &VendorLut, id: &str) -> VendorObservation {
        lut.observations
            .iter()
            .find(|o| o.observation_id == id)
            .unwrap_or_else(|| panic!("row {id} is not in the embedded LUT"))
            .clone()
    }

    fn query(tool_family: ToolFamily, diameter_mm: f64, flute_count: u32) -> LookupQuery {
        LookupQuery {
            tool_family,
            tool_subfamily: None,
            diameter_mm,
            flute_count,
            material_family: MaterialFamily::Hardwood,
            hardness_kind: Some(HardnessKind::Janka),
            hardness_value: Some(1450.0),
            operation_family: LutOperationFamily::Drill,
            pass_role: LutPassRole::Roughing,
        }
    }

    const HOME_6: &str = "amana-flat-hardwood-pocket-6000-2f-spektra";

    /// The rule reads the Spektra pocket row for a 2- or 3-flute flat
    /// plunge inside 3.0-12.7 mm, and for no other query.
    #[test]
    fn the_rule_reads_only_a_flat_plunge_inside_its_range() {
        let lut = VendorLut::embedded();
        let home = row(&lut, HOME_6);
        for (d, z) in [
            (6.0, 2),
            (6.0, 3),
            (3.175, 2),
            (4.0, 3),
            (3.0, 2),
            (6.35, 2),
            (9.525, 3),
            (12.7, 3),
        ] {
            assert!(
                drill_rule(&query(ToolFamily::FlatEnd, d, z), &home).is_some(),
                "{d} mm {z}F"
            );
        }
        for (d, z) in [(2.0, 2), (15.875, 2), (6.0, 1), (6.0, 4)] {
            assert!(
                drill_rule(&query(ToolFamily::FlatEnd, d, z), &home).is_none(),
                "{d} mm {z}F"
            );
        }
        for family in [
            ToolFamily::BullNose,
            ToolFamily::BallNose,
            ToolFamily::TaperedBallNose,
            ToolFamily::ChamferVbit,
        ] {
            assert!(
                drill_rule(&query(family, 6.0, 2), &home).is_none(),
                "{family:?}"
            );
        }
        // A pocket query reads the row as printed.
        let mut pocket = query(ToolFamily::FlatEnd, 6.0, 2);
        pocket.operation_family = LutOperationFamily::Pocket;
        assert!(drill_rule(&pocket, &home).is_none());
        // The 1.5 mm row (below the range; ruling item 1: "the 1.5 mm rows
        // are side rows only") is not read, even though the query is
        // in range.
        let too_small = row(&lut, "amana-flat-hardwood-pocket-1500-2f-spektra");
        assert!(drill_rule(&query(ToolFamily::FlatEnd, 6.0, 2), &too_small).is_none());
        let mut adaptive = home;
        adaptive.operation_family = LutOperationFamily::Adaptive;
        assert!(drill_rule(&query(ToolFamily::FlatEnd, 6.0, 2), &adaptive).is_none());
    }

    /// The claim scales the chip by 1 / Z and states the rule, the RPM cap
    /// and the peck rule.
    #[test]
    fn the_claim_is_the_side_chip_over_z_and_states_the_rpm_cap() {
        let lut = VendorLut::embedded();
        let home = row(&lut, HOME_6);
        assert_eq!(
            drill_basis(&query(ToolFamily::BullNose, 6.0, 2), &home),
            DrillBasis::Printed
        );
        let basis = drill_basis(&query(ToolFamily::FlatEnd, 6.0, 2), &home);
        assert_eq!(basis.name(), "Transferred");
        assert!((basis.scale() - 0.5).abs() < 1e-15);
        let claim = basis.claim().expect("a claimed row carries a claim");
        assert_eq!(claim.gap, Gap::Drill);
        assert_eq!(claim.side_row, HOME_6);
        assert_eq!(claim.printed_rpm, Some(18_000.0));
        let (headline, detail) = claim.card_text();
        assert_eq!(
            headline,
            "vendor row, plunge claim (G6 drill): the side chip of the pocket row / 2 flutes \
             (x0.50)"
        );
        assert!(detail.starts_with(DRILL_RULE_TEXT), "{detail}");
        for needle in [
            "held per tooth",
            "the chart's 18000 RPM is replaced by the engine drill RPM cap (8000-14000 RPM at \
             D <= 6.0 mm, a repo rule)",
            "about x0.78 of the printed Ramp Down",
            "derived grade b",
            "straight plunge",
            DRILL_PECK_TEXT,
        ] {
            assert!(detail.contains(needle), "{needle:?} missing from {detail}");
        }
        let three = drill_basis(&query(ToolFamily::FlatEnd, 6.0, 3), &home);
        assert!((three.scale() - 1.0 / 3.0).abs() < 1e-15);
    }
}
