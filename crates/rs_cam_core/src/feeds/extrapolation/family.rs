//! G3: the family transfer. A row exists for the tool, but the vendor files
//! it under another operation family
//! (`planning/extrapolation_2026-09-24/A3_PLAN.md`).
//!
//! Some charts print one chip load per tool and material, and name no
//! operation. The LUT files each such row under one home family, for
//! example (Pocket, Roughing). A [`FamilyRule`] states which rows are of
//! this kind and which operation families the home row serves. The rule is
//! [`FAMILY_RULE_TEXT`]: the pass role changes the stepover and the depth,
//! not the chip load.
//!
//! The rule works in three places, all in `vendor_lookup`:
//!
//! 1. `passes_must_match` lets a row from another operation family through
//!    only when [`transfer_rule`] returns a rule.
//! 2. `score_observation` scores a transferred row as its copy in the
//!    queried family would score (copy semantics): the role term gives +45.
//! 3. `beats`: on an equal score, a row printed in the queried family beats
//!    a transferred row; after that the id decides.
//!
//! [`family_basis`] runs in `build_result` beside the size claim and the
//! hardness basis, so every consumer of a `LookupResult` reads one mark.
//! The transfer moves no number: the band is the home row's band, and the
//! size claim and the hardness basis act on it as they act on any row.

use std::ops::RangeInclusive;

use super::{ClaimConfidence, Gap};
use crate::feeds::vendor_lookup::LookupQuery;
use crate::feeds::vendor_lut::{LutOperationFamily, LutPassRole, ToolFamily, VendorObservation};

/// The family rule in words, for the detail line.
pub const FAMILY_RULE_TEXT: &str =
    "vendor prints one value per tool; role changes the stepover and depth, not the chip load";

/// One family rule: the rows it covers and the families it serves.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FamilyRule {
    /// The tool family of the rows. The query must have the same family,
    /// so a rule never transfers through a tool-family fallback.
    pub tool_family: ToolFamily,
    /// The `source_id` values of the charts that print one value per tool.
    pub source_ids: &'static [&'static str],
    /// The `tool_subfamily` of the rows.
    pub tool_subfamily: &'static str,
    /// The (operation family, pass role) that the LUT files the rows under.
    pub home: (LutOperationFamily, LutPassRole),
    /// The operation families that the home row serves. The home family
    /// is not in this list: a home query reads the row as printed.
    pub serves: &'static [LutOperationFamily],
    /// The smallest and the largest printed diameter (mm) of the rows.
    pub printed_mm: (f64, f64),
    /// What supports the rule.
    pub witness: &'static str,
}

/// The four Onsrud cutting-data sheets that print the 77-100 taper tool.
const ONSRUD_77_100_SOURCES: &[&str] = &[
    "onsrud_hard_wood_cutting_data",
    "onsrud_soft_wood_cutting_data",
    "onsrud_mdf_cutting_data",
    "onsrud_hard_plywood_cutting_data",
];

/// The families that a roughing home row serves: every milling family that
/// a tapered ball or a bull nose cuts, except Face and Drill.
const SERVES_MILLING: &[LutOperationFamily] = &[
    LutOperationFamily::Adaptive,
    LutOperationFamily::Contour,
    LutOperationFamily::Parallel,
    LutOperationFamily::Scallop,
    LutOperationFamily::Trace,
];

/// The Amana corner-radius chart (46460 and 46462) that prints the bull
/// nose rows.
const AMANA_CORNER_RADIUS_SOURCES: &[&str] = &["amana_corner_radius_spiral_plunge_2f"];

/// The family rules (A3): the tapered ball (step 2) and the bull nose
/// (step 4). There is no ball-nose rule: the 42 ball-nose finish cells stay
/// separate until ruling B3 (orchestrator decision 6).
pub const FAMILY_RULES: &[FamilyRule] = &[
    FamilyRule {
        tool_family: ToolFamily::TaperedBallNose,
        source_ids: ONSRUD_77_100_SOURCES,
        tool_subfamily: "77_100_series",
        home: (LutOperationFamily::Pocket, LutPassRole::Roughing),
        serves: SERVES_MILLING,
        printed_mm: (3.175, 6.35),
        witness: "vendor structure: the Onsrud sheets print one 77-100 chip load per tip \
                  diameter and material, and name no operation",
    },
    FamilyRule {
        tool_family: ToolFamily::BullNose,
        source_ids: AMANA_CORNER_RADIUS_SOURCES,
        tool_subfamily: "corner_radius",
        home: (LutOperationFamily::Pocket, LutPassRole::Roughing),
        serves: SERVES_MILLING,
        printed_mm: (6.35, 12.7),
        witness: "vendor structure: the Amana corner-radius chart prints one chip load per \
                  diameter and material, and names no operation",
    },
];

/// The rule that carries `obs` into the queried operation family, or
/// `None`.
///
/// A rule applies when all of these conditions are true:
///
/// - the row is from another operation family than the query;
/// - the query, the row and the rule have the same tool family;
/// - the row's `source_id` is one of the rule's sources, and its
///   `tool_subfamily` is the rule's subfamily;
/// - the row is filed under the rule's home (family and role);
/// - the rule serves the queried family.
///
/// The caller still applies the material, tool and diameter filters.
#[must_use]
pub fn transfer_rule(query: &LookupQuery, obs: &VendorObservation) -> Option<&'static FamilyRule> {
    if obs.operation_family == query.operation_family {
        return None;
    }
    FAMILY_RULES.iter().find(|rule| {
        rule.tool_family == query.tool_family
            && obs.tool_family == rule.tool_family
            && rule.source_ids.contains(&obs.source_id.as_str())
            && obs.tool_subfamily.as_deref() == Some(rule.tool_subfamily)
            && (obs.operation_family, obs.pass_role) == rule.home
            && rule.home.0 != query.operation_family
            && rule.serves.contains(&query.operation_family)
    })
}

/// One family claim: a stated rule carries the home row into the queried
/// operation family at x1.00.
#[derive(Debug, Clone, PartialEq)]
pub struct FamilyClaim {
    /// Always [`Gap::Family`].
    pub gap: Gap,
    /// The rule in words ([`FAMILY_RULE_TEXT`]).
    pub rule: &'static str,
    /// The (family, role) the row is filed under.
    pub home: (LutOperationFamily, LutPassRole),
    /// The (family, role) of the query.
    pub query: (LutOperationFamily, LutPassRole),
    /// The row that the claim carries.
    pub source_rows: Vec<String>,
    /// The row's `source_id`.
    pub source_id: String,
    /// The families that the rule serves.
    pub serves: &'static [LutOperationFamily],
    /// The printed diameters (mm) of the rule's rows.
    pub printed_mm: RangeInclusive<f64>,
    /// What supports the rule.
    pub witness: &'static str,
    pub confidence: ClaimConfidence,
}

impl FamilyClaim {
    /// The card text: a headline and a detail line, in plain words.
    ///
    /// Example: headline "vendor row, family transferred (G3 family): the
    /// pocket row serves this contour pass"; detail "vendor prints one value
    /// per tool; role changes the stepover and depth, not the chip load; row
    /// onsrud-hardwood-77-100-1_8-pocket (onsrud_hard_wood_cutting_data) is
    /// filed as pocket/roughing and serves adaptive, contour, parallel,
    /// scallop and trace at x1.00; printed 3.175-6.35 mm; one witness
    /// (vendor structure: ...)".
    #[must_use]
    pub fn card_text(&self) -> (String, String) {
        let headline = format!(
            "vendor row, family transferred ({} {}): the {} row serves this {} pass",
            self.gap.group(),
            self.gap.label(),
            self.home.0.label(),
            self.query.0.label()
        );
        let lo = *self.printed_mm.start();
        let hi = *self.printed_mm.end();
        let detail = format!(
            "{}; row {} ({}) is filed as {}/{} and serves {} at x1.00; printed {lo:?}-{hi:?} mm; \
             one witness ({})",
            self.rule,
            self.source_rows.join(", "),
            self.source_id,
            self.home.0.label(),
            self.home.1.label(),
            join_families(self.serves),
            self.witness
        );
        (headline, detail)
    }
}

/// "a, b and c".
fn join_families(families: &[LutOperationFamily]) -> String {
    let names: Vec<&str> = families.iter().map(|f| f.label()).collect();
    match names.split_last() {
        None => String::new(),
        Some((last, [])) => (*last).to_owned(),
        Some((last, rest)) => format!("{} and {last}", rest.join(", ")),
    }
}

/// The family basis of one matched row for one query.
#[derive(Debug, Clone, PartialEq)]
pub enum FamilyBasis {
    /// The row is printed in the queried operation family.
    Printed,
    /// A family rule carries the row from its home family.
    Transferred(Box<FamilyClaim>),
}

impl FamilyBasis {
    /// The claim, when the row is transferred.
    #[must_use]
    pub fn claim(&self) -> Option<&FamilyClaim> {
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
}

/// The family basis of the row `obs` for `query`. A pure function of the
/// two, like `hardness_basis`; both resolvers call it in `build_result`.
///
/// A row that no rule carries is `Printed`. The lookup lets a row from
/// another family through only when a rule carries it, so a matched row is
/// `Transferred` exactly when its family differs from the query's.
#[must_use]
pub fn family_basis(query: &LookupQuery, obs: &VendorObservation) -> FamilyBasis {
    let Some(rule) = transfer_rule(query, obs) else {
        return FamilyBasis::Printed;
    };
    FamilyBasis::Transferred(Box::new(FamilyClaim {
        gap: Gap::Family,
        rule: FAMILY_RULE_TEXT,
        home: rule.home,
        query: (query.operation_family, query.pass_role),
        source_rows: vec![obs.observation_id.clone()],
        source_id: obs.source_id.clone(),
        serves: rule.serves,
        printed_mm: rule.printed_mm.0..=rule.printed_mm.1,
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

    fn query(
        tool_family: ToolFamily,
        operation_family: LutOperationFamily,
        pass_role: LutPassRole,
    ) -> LookupQuery {
        LookupQuery {
            tool_family,
            tool_subfamily: None,
            diameter_mm: 3.175,
            flute_count: 2,
            material_family: MaterialFamily::Hardwood,
            hardness_kind: Some(HardnessKind::Janka),
            hardness_value: Some(1450.0),
            operation_family,
            pass_role,
        }
    }

    const POCKET: &str = "onsrud-hardwood-77-100-1_8-pocket";

    /// The tapered rule carries the Onsrud 77-100 home row into each served
    /// family, and into no other family.
    #[test]
    fn the_tapered_rule_serves_only_its_families() {
        use LutOperationFamily::{
            Adaptive, Contour, Drill, Face, Parallel, Pocket, Scallop, Trace,
        };
        let lut = VendorLut::embedded();
        let home = row(&lut, POCKET);
        for family in [Adaptive, Contour, Parallel, Scallop, Trace] {
            for role in [
                LutPassRole::Roughing,
                LutPassRole::SemiFinish,
                LutPassRole::Finish,
            ] {
                let q = query(ToolFamily::TaperedBallNose, family, role);
                let rule = transfer_rule(&q, &home)
                    .unwrap_or_else(|| panic!("{family:?}/{role:?}: the rule must carry the row"));
                assert_eq!(rule.tool_subfamily, "77_100_series");
            }
        }
        // The home family reads the row as printed; Face and Drill are not
        // served.
        for family in [Pocket, Face, Drill] {
            let q = query(ToolFamily::TaperedBallNose, family, LutPassRole::Roughing);
            assert!(transfer_rule(&q, &home).is_none(), "{family:?}");
        }
    }

    /// Only the home rows of the rule's sources and subfamily transfer, and
    /// only for a query of the rule's tool family.
    #[test]
    fn the_tapered_rule_matches_only_the_onsrud_77_100_home_rows() {
        let lut = VendorLut::embedded();
        let home = row(&lut, POCKET);
        let trace = query(
            ToolFamily::TaperedBallNose,
            LutOperationFamily::Trace,
            LutPassRole::Finish,
        );
        // Another tool family: a ball-nose or a flat query never reads a
        // tapered row through the rule.
        for family in [
            ToolFamily::BallNose,
            ToolFamily::FlatEnd,
            ToolFamily::BullNose,
        ] {
            let q = query(family, LutOperationFamily::Trace, LutPassRole::Finish);
            assert!(transfer_rule(&q, &home).is_none(), "{family:?}");
        }
        // A copy that is not filed under the home does not transfer. The
        // LUT holds no copy since A3 step 3, so the test makes one.
        let mut copy = home.clone();
        copy.observation_id = "onsrud-hardwood-77-100-1_8-parallel".to_owned();
        copy.operation_family = LutOperationFamily::Parallel;
        copy.pass_role = LutPassRole::Finish;
        assert!(transfer_rule(&trace, &copy).is_none());
        // Another source or another subfamily does not transfer.
        let mut other_source = home.clone();
        other_source.source_id = "spetool_tapered".to_owned();
        assert!(transfer_rule(&trace, &other_source).is_none());
        let mut other_subfamily = home.clone();
        other_subfamily.tool_subfamily = Some("77_200_series".to_owned());
        assert!(transfer_rule(&trace, &other_subfamily).is_none());
        let mut no_subfamily = home.clone();
        no_subfamily.tool_subfamily = None;
        assert!(transfer_rule(&trace, &no_subfamily).is_none());
        // A row of another tool family with the same source does not
        // transfer.
        let mut ball = home;
        ball.tool_family = ToolFamily::BallNose;
        let ball_query = query(
            ToolFamily::BallNose,
            LutOperationFamily::Trace,
            LutPassRole::Finish,
        );
        assert!(transfer_rule(&ball_query, &ball).is_none());
        // Every home row of the four sheets transfers.
        let homes: Vec<&VendorObservation> = lut
            .observations
            .iter()
            .filter(|o| {
                o.tool_family == ToolFamily::TaperedBallNose
                    && o.tool_subfamily.as_deref() == Some("77_100_series")
                    && o.operation_family == LutOperationFamily::Pocket
            })
            .collect();
        assert_eq!(homes.len(), 8, "4 sheets x 2 tip diameters");
        for obs in homes {
            assert!(
                transfer_rule(&trace, obs).is_some(),
                "{}",
                obs.observation_id
            );
        }
    }

    /// The bull rule carries the Amana corner-radius home row into each
    /// served family. The derived Onsrud bull plywood row is from another
    /// source, so it does not transfer.
    #[test]
    fn the_bull_rule_serves_only_its_families_and_its_source() {
        use LutOperationFamily::{
            Adaptive, Contour, Drill, Face, Parallel, Pocket, Scallop, Trace,
        };
        let lut = VendorLut::embedded();
        let home = row(&lut, "amana-bull-hardwood-pocket-6350-2f-cr");
        for family in [Adaptive, Contour, Parallel, Scallop, Trace] {
            let q = query(ToolFamily::BullNose, family, LutPassRole::Finish);
            let rule = transfer_rule(&q, &home)
                .unwrap_or_else(|| panic!("{family:?}: the rule must carry the row"));
            assert_eq!(rule.tool_subfamily, "corner_radius");
        }
        for family in [Pocket, Face, Drill] {
            let q = query(ToolFamily::BullNose, family, LutPassRole::Roughing);
            assert!(transfer_rule(&q, &home).is_none(), "{family:?}");
        }
        // A flat query never reads a bull row through the rule.
        let flat = query(
            ToolFamily::FlatEnd,
            LutOperationFamily::Parallel,
            LutPassRole::Finish,
        );
        assert!(transfer_rule(&flat, &home).is_none());
        let plywood = row(&lut, "onsrud-bull-plywood-hardwood-pocket-6000-2f");
        let parallel = query(
            ToolFamily::BullNose,
            LutOperationFamily::Parallel,
            LutPassRole::Finish,
        );
        assert!(transfer_rule(&parallel, &plywood).is_none());
    }

    /// The rule's printed range is the range of its home rows.
    #[test]
    fn each_rule_states_the_printed_range_of_its_home_rows() {
        let lut = VendorLut::embedded();
        for rule in FAMILY_RULES {
            let diameters: Vec<f64> = lut
                .observations
                .iter()
                .filter(|o| {
                    o.tool_family == rule.tool_family
                        && rule.source_ids.contains(&o.source_id.as_str())
                        && o.tool_subfamily.as_deref() == Some(rule.tool_subfamily)
                        && (o.operation_family, o.pass_role) == rule.home
                })
                .filter_map(|o| o.diameter_mm)
                .collect();
            assert!(!diameters.is_empty(), "{rule:?} has no home rows");
            let lo = diameters.iter().copied().fold(f64::INFINITY, f64::min);
            let hi = diameters.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            assert_eq!(rule.printed_mm, (lo, hi), "{rule:?}");
            assert!(!rule.serves.contains(&rule.home.0), "{rule:?}");
        }
    }

    /// A transferred row carries a claim that names the rule, the row and
    /// both families; a row in the queried family is printed.
    #[test]
    fn the_basis_marks_a_transferred_row_and_states_the_rule() {
        let lut = VendorLut::embedded();
        let home = row(&lut, POCKET);
        let pocket = query(
            ToolFamily::TaperedBallNose,
            LutOperationFamily::Pocket,
            LutPassRole::Roughing,
        );
        assert_eq!(family_basis(&pocket, &home), FamilyBasis::Printed);
        let waterline = query(
            ToolFamily::TaperedBallNose,
            LutOperationFamily::Contour,
            LutPassRole::SemiFinish,
        );
        let basis = family_basis(&waterline, &home);
        assert_eq!(basis.name(), "Transferred");
        let claim = basis.claim().expect("a transferred row carries a claim");
        assert_eq!(claim.gap, Gap::Family);
        assert_eq!(
            claim.home,
            (LutOperationFamily::Pocket, LutPassRole::Roughing)
        );
        assert_eq!(
            claim.query,
            (LutOperationFamily::Contour, LutPassRole::SemiFinish)
        );
        assert_eq!(claim.source_rows, vec![POCKET.to_owned()]);
        let (headline, detail) = claim.card_text();
        assert_eq!(
            headline,
            "vendor row, family transferred (G3 family): the pocket row serves this contour pass"
        );
        assert!(detail.starts_with(FAMILY_RULE_TEXT), "{detail}");
        assert!(detail.contains(POCKET), "{detail}");
        assert!(
            detail.contains(
                "is filed as pocket/roughing and serves adaptive, contour, parallel, scallop and \
                 trace at x1.00; printed 3.175-6.35 mm"
            ),
            "{detail}"
        );
    }
}
