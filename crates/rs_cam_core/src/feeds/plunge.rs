//! G10: the plunge resolver (`planning/extrapolation_2026-09-24/G10_PLAN.md`
//! §3 A2).
//!
//! The plunge has two parts, like the G6 ramp:
//!
//! 1. [`PlungeBasis`] is the part that does not use the feed F. `calculate`
//!    sets it on `FeedsResult::plunge` (step 8).
//! 2. [`resolve_plunge`] gives the number at a feed. `calculate` runs it at
//!    the calculator's F. The apply funnel (`suggest::apply`) runs it again
//!    at the feed that ships, because pass 9, pass 10 and an explored feed
//!    can move F.
//!
//! The value is `min(rule, tip cap, axial chip, F)`, rounded down to 1
//! mm/min:
//!
//! - the rule is the G10 claim (`extrapolation::plunge`, fraction x F), or
//!   the named repo rule (the material base) when no claim covers the tool;
//! - the tip cap is a named repo rule for a ball or a tapered ball
//!   (`tool_load::plunge_stress`, the one producer);
//! - the axial chip (feeds contract P8, ruling Q8) bounds the plunge of an
//!   op whose entry contract is `StraightPlunge { feed: AxialChip }`
//!   (Project Curve, lead decision Q8a) where a G6 claim covers the tool:
//!   `g6_chip x rpm x Z` (report 02 F-c1);
//! - a plunge never exceeds the feed.
//!
//! A drill cycle has one feed. Its plunge is the drill feed (calculator
//! step 9c), so the resolver gives `None` for it.

use super::extrapolation::plunge::flutes_text;
use super::extrapolation::{PlungeClaim, PlungeRefusal, plunge_rule};
use super::provenance::ValueProvenance;
use super::{FeedsInput, OperationFamily, ToolGeometryHint};
use crate::tool_load::plunge_stress;

/// The named repo rule when no G10 claim covers the tool. The headline
/// says "no source; repo rule", so this detail text starts at the rule
/// itself and does not say it again (audit WRONG #11, 2026-10-02).
pub const PLUNGE_BASE_RULE_TEXT: &str = "material base 1000/h (wood), \
     900/h (plywood, sheet) at 6 mm, x clamp(D/6, 0.25, 3), h = (Janka/600)^0.4 (a port of the \
     Shapeoko reference calculator)";

/// The named repo rule of the ball and tapered-ball tip cap.
pub const TIP_CAP_RULE_TEXT: &str = "no stored source; repo rule 150 mm/min per mm of tip (a \
     code comment cites FSWizard / GWizard 100-300 mm/min; not stored)";

/// The ruling Q12 sentence. Every plunge detail that is not a drill cycle
/// ends with it.
pub const DOWN_CUT_PLUNGE_TEXT: &str = "The engine does not read the cut direction: a down-cut \
     or compression tool must ramp, not plunge (Vortex, AXYZ, grade c; ruling Q12, future work).";

/// The ball and tapered-ball tip cap.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TipCap {
    /// The effective tip diameter (mm).
    pub tip_d_mm: f64,
    /// The cap (mm/min).
    pub cap_mm_min: f64,
}

impl TipCap {
    /// The tip cap of a tool, from the one producer
    /// ([`plunge_stress::safe_plunge_cap_mm_min`]). `None` for a flat, a
    /// bull nose and a V-bit.
    #[must_use]
    pub fn for_tool(geometry: ToolGeometryHint, diameter_mm: f64) -> Option<Self> {
        let cap_mm_min = plunge_stress::safe_plunge_cap_mm_min(geometry, diameter_mm)?;
        let tip_d_mm = plunge_stress::effective_tip_diameter_mm(geometry, diameter_mm)?;
        Some(Self {
            tip_d_mm,
            cap_mm_min,
        })
    }

    /// The cap in words, for the detail line.
    #[must_use]
    pub fn text(&self) -> String {
        format!(
            "tip cap {:.0} mm/min at a tip of {:?} mm: {TIP_CAP_RULE_TEXT}",
            self.cap_mm_min, self.tip_d_mm
        )
    }
}

/// Q8 (feeds contract P8): the G6 axial-chip bound of a straight plunge.
/// The vertical rate is `axial_chip_mm x rpm x flutes`. Only an op whose
/// entry contract is `StraightPlunge { feed: PlungeFeedRule::AxialChip }`
/// carries it, and only where a G6 claim covers the tool
/// (`RampBasis::Sourced`).
#[derive(Debug, Clone, PartialEq)]
pub struct AxialChipBound {
    /// The G6 axial chip per tooth (mm): the side row's chip / Z, with the
    /// hardness scale (`FeedsResult::ramp`).
    pub axial_chip_mm: f64,
    /// The flute count Z of the claim.
    pub flutes: u32,
    /// The side row that the G6 drill claim reads.
    pub side_row: String,
}

impl AxialChipBound {
    /// The vertical rate at `rpm` (mm/min).
    #[must_use]
    pub fn at_rpm(&self, rpm: f64) -> f64 {
        self.axial_chip_mm * rpm * f64::from(self.flutes)
    }

    /// The bound in words.
    #[must_use]
    pub fn text(&self) -> String {
        format!(
            "G6 axial chip {:.4} mm/tooth x rpm x {} (row {})",
            self.axial_chip_mm, self.flutes, self.side_row
        )
    }
}

/// The part of the plunge that does not use the feed.
#[derive(Debug, Clone, PartialEq)]
pub enum PlungeBasis {
    /// A G10 claim gives the plunge as a fraction of the feed.
    Claimed {
        claim: Box<PlungeClaim>,
        tip_cap: Option<TipCap>,
        /// Q8: the G6 axial-chip bound of a Project Curve plunge.
        axial: Option<AxialChipBound>,
    },
    /// No claim covers the tool. The named repo rule (the material base)
    /// gives the plunge.
    MaterialBase {
        reason: PlungeRefusal,
        base_mm_min: f64,
        tip_cap: Option<TipCap>,
        /// Q8: the G6 axial-chip bound of a Project Curve plunge.
        axial: Option<AxialChipBound>,
    },
    /// A drill cycle: the plunge is the drill feed.
    DrillCycle,
}

/// Which term sets the plunge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlungeBinding {
    /// The G10 claim or the material base.
    Rule,
    /// The ball or tapered-ball tip cap.
    TipCap,
    /// Q8: the G6 axial chip x rpm x Z (Project Curve).
    AxialChip,
    /// The feed: a plunge never exceeds the feed.
    Feed,
}

impl PlungeBinding {
    /// The term in words.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Rule => "the rule",
            Self::TipCap => "the tip cap",
            Self::AxialChip => "the G6 axial chip",
            Self::Feed => "the feed",
        }
    }
}

impl PlungeBasis {
    /// The short name of the basis, for the FM1 columns.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Claimed { .. } => "Claimed",
            Self::MaterialBase { .. } => "MaterialBase",
            Self::DrillCycle => "DrillCycle",
        }
    }

    /// The claim, when a G10 claim covers the tool.
    #[must_use]
    pub fn claim(&self) -> Option<&PlungeClaim> {
        match self {
            Self::Claimed { claim, .. } => Some(claim),
            Self::MaterialBase { .. } | Self::DrillCycle => None,
        }
    }

    /// The tip cap, when the tool has one.
    #[must_use]
    pub fn tip_cap(&self) -> Option<TipCap> {
        match self {
            Self::Claimed { tip_cap, .. } | Self::MaterialBase { tip_cap, .. } => *tip_cap,
            Self::DrillCycle => None,
        }
    }

    /// Q8: the G6 axial-chip bound, when the op states it and a G6 claim
    /// covers the tool.
    #[must_use]
    pub fn axial(&self) -> Option<&AxialChipBound> {
        match self {
            Self::Claimed { axial, .. } | Self::MaterialBase { axial, .. } => axial.as_ref(),
            Self::DrillCycle => None,
        }
    }

    /// The card text: a headline and a detail line.
    ///
    /// Example headlines:
    /// - "Plunge = feed / 2 (G10 plunge claim: flat end mill, Amana Spektra
    ///   rule; Sienci, IDC, 3.0-12.7 mm, 2-3 flutes)";
    /// - "Plunge = min(0.50 x feed, tip cap 900 mm/min) (G10 plunge claim:
    ///   ball nose, Sienci, Amana, 3.175-25.4 mm, 2 flutes)";
    /// - "Plunge: no source; repo rule material base 1000 mm/min (bull nose;
    ///   nearest printed: Amana corner-radius Ramp Down = feed / flutes, no
    ///   feed-paired witness)".
    ///
    /// Every detail that is not a drill cycle ends with
    /// [`DOWN_CUT_PLUNGE_TEXT`] (ruling Q12).
    #[must_use]
    pub fn card_text(&self) -> (String, String) {
        match self {
            Self::Claimed {
                claim,
                tip_cap,
                axial,
            } => {
                let rule = claim.rule;
                let fraction = claim.fraction_text();
                let mut terms = vec![fraction];
                if let Some(cap) = tip_cap {
                    terms.push(format!("tip cap {:.0} mm/min", cap.cap_mm_min));
                }
                if let Some(bound) = axial {
                    terms.push(format!(
                        "{:.4} x rpm x {}",
                        bound.axial_chip_mm, bound.flutes
                    ));
                }
                let value = if terms.len() == 1 {
                    terms.remove(0)
                } else {
                    format!("min({})", terms.join(", "))
                };
                let headline = format!(
                    "Plunge = {value} ({} {} claim: {}, {}, {}, {})",
                    claim.gap.group(),
                    claim.gap.label(),
                    rule.tool_family.label(),
                    rule.vendors,
                    rule.key.text(),
                    flutes_text(rule.flutes)
                );
                let (_, claim_detail) = claim.card_text();
                let cap = tip_cap.map_or_else(String::new, |c| format!("; {}", c.text()));
                let ax = axial
                    .as_ref()
                    .map_or_else(String::new, |b| format!("; {}", b.text()));
                let detail = format!("{claim_detail}{cap}{ax}. {DOWN_CUT_PLUNGE_TEXT}");
                (headline, detail)
            }
            Self::MaterialBase {
                reason,
                base_mm_min,
                tip_cap,
                axial,
            } => {
                let mut cap = tip_cap.map_or_else(String::new, |c| {
                    format!(", tip cap {:.0} mm/min", c.cap_mm_min)
                });
                if let Some(bound) = axial {
                    cap.push_str(&format!(", {}", bound.text()));
                }
                let headline = format!(
                    "Plunge: no source; repo rule material base {base_mm_min:.0} mm/min{cap} ({})",
                    reason.short()
                );
                let cap = tip_cap.map_or_else(String::new, |c| format!(" The {}.", c.text()));
                let detail = format!(
                    "The rule: {PLUNGE_BASE_RULE_TEXT}. {}{cap} {DOWN_CUT_PLUNGE_TEXT}",
                    reason.text()
                );
                (headline, detail)
            }
            Self::DrillCycle => (
                "Plunge: the drill feed".to_owned(),
                "A drill cycle has one feed: the plunge is the drill feed.".to_owned(),
            ),
        }
    }

    /// The card text at a feed and an RPM. The headline names the term
    /// that binds.
    #[must_use]
    pub fn card_text_at(&self, feed_mm_min: f64, rpm: f64) -> (String, String) {
        let (headline, detail) = self.card_text();
        match resolve_plunge(self, feed_mm_min, rpm) {
            Some((value, binding)) => (
                format!("{headline}; {value:.0} mm/min, set by {}", binding.label()),
                detail,
            ),
            None => (headline, detail),
        }
    }

    /// The provenance of the plunge that `binding` set.
    ///
    /// - A claim that binds stamps `PublishedRule` with the rule id.
    /// - The material base that binds stamps `RepoRule`
    ///   ("material_plunge_base").
    /// - The tip cap stamps `RepoRule` ("ball_tip_cap").
    /// - The feed that binds, and a drill cycle, copy the feed stamp.
    #[must_use]
    pub fn provenance(&self, binding: PlungeBinding, feed: ValueProvenance) -> ValueProvenance {
        match (self, binding) {
            (Self::DrillCycle, _) | (_, PlungeBinding::Feed) => feed,
            (_, PlungeBinding::TipCap) => ValueProvenance::repo_rule("ball_tip_cap"),
            (
                Self::Claimed { axial, .. } | Self::MaterialBase { axial, .. },
                PlungeBinding::AxialChip,
            ) => axial
                .as_ref()
                .map_or(feed, |b| ValueProvenance::vendor_lut(b.side_row.clone())),
            (Self::Claimed { claim, .. }, PlungeBinding::Rule) => {
                ValueProvenance::published_rule(claim.rule.id)
            }
            (Self::MaterialBase { .. }, PlungeBinding::Rule) => {
                ValueProvenance::repo_rule("material_plunge_base")
            }
        }
    }
}

impl PlungeBasis {
    /// CONTRACT K2 (feeds contract P5): the basis of the plunge that
    /// `binding` sets at a feed whose basis is `feed`. A G10 claim names its
    /// rule and the row that decides the feed; the material base names its
    /// F201 row; the tip cap is a repo rule over the tool; a plunge at the
    /// feed copies the feed's basis.
    #[must_use]
    pub(crate) fn basis(
        &self,
        binding: PlungeBinding,
        feed: &super::Basis,
        material_row: crate::material::FormulaConstant,
    ) -> super::Basis {
        use super::Basis;
        use super::sourced::{ClaimKind, ClaimRef, RowRef};
        match (self, binding) {
            (Self::DrillCycle, _) | (_, PlungeBinding::Feed) => {
                Basis::derived("plunge = feed", vec![feed.clone()])
            }
            (_, PlungeBinding::TipCap) => Basis::derived(
                "ball tip cap (repo rule, tool_load::plunge_stress)",
                vec![Basis::tool_diameter()],
            ),
            (
                Self::Claimed { axial, .. } | Self::MaterialBase { axial, .. },
                PlungeBinding::AxialChip,
            ) => Basis::derived(
                "Q8 G6 axial chip x rpm x Z",
                axial
                    .iter()
                    .map(|b| {
                        Basis::Claim(ClaimRef {
                            kind: ClaimKind::DrillG6,
                            row: RowRef(b.side_row.clone()),
                            rule: "g6_axial_plunge".to_owned(),
                        })
                    })
                    .chain(std::iter::once(feed.clone()))
                    .collect(),
            ),
            (Self::Claimed { claim, .. }, PlungeBinding::Rule) => {
                let row = feed
                    .deciding_row()
                    .cloned()
                    .unwrap_or_else(|| RowRef(claim.rule.id.to_owned()));
                Basis::derived(
                    "G10 fraction x feed",
                    vec![
                        Basis::Claim(ClaimRef {
                            kind: ClaimKind::PlungeG10,
                            row,
                            rule: claim.rule.id.to_owned(),
                        }),
                        feed.clone(),
                    ],
                )
            }
            (Self::MaterialBase { .. }, PlungeBinding::Rule) => {
                Basis::derived("material plunge base", vec![Basis::Rule(material_row)])
            }
        }
    }
}

/// The plunge basis of one calculator input (`calculate` step 8).
///
/// 1. A drill cycle gives `DrillCycle`.
/// 2. Otherwise the key comes from the tool geometry: the tip of a tapered
///    ball (with the same 0.5 mm floor as the cap), the included angle of a
///    V-bit, else the diameter. The family and the material come from the
///    unrouted LUT query.
/// 3. A G10 claim gives `Claimed`; a refusal gives `MaterialBase` with
///    `material_base_mm_min`.
/// 4. Q8 (P8): an op whose entry contract is `StraightPlunge { feed:
///    AxialChip }` takes the G6 axial-chip bound when `ramp` holds a G6
///    claim (`RampBasis::Sourced`); any other op, or no claim, takes none.
#[must_use]
pub fn plunge_basis(
    input: &FeedsInput<'_>,
    material_base_mm_min: f64,
    ramp: &super::ramp::RampBasis,
) -> PlungeBasis {
    if input.operation == OperationFamily::Drill {
        return PlungeBasis::DrillCycle;
    }
    let axial = axial_chip_bound(input, ramp);
    let tip_cap = TipCap::for_tool(input.tool_geometry, input.tool_diameter);
    let tip_d_mm = match input.tool_geometry {
        ToolGeometryHint::TaperedBall { .. } => {
            plunge_stress::effective_tip_diameter_mm(input.tool_geometry, input.tool_diameter)
        }
        ToolGeometryHint::Flat
        | ToolGeometryHint::Ball
        | ToolGeometryHint::Bull { .. }
        | ToolGeometryHint::VBit { .. } => None,
    };
    let angle_deg = match input.tool_geometry {
        ToolGeometryHint::VBit { included_angle, .. } => Some(included_angle),
        ToolGeometryHint::Flat
        | ToolGeometryHint::Ball
        | ToolGeometryHint::Bull { .. }
        | ToolGeometryHint::TaperedBall { .. } => None,
    };
    let query = super::vendor_normalize::to_lookup_query_unrouted(input);
    match plunge_rule(
        query.tool_family,
        input.tool_diameter,
        tip_d_mm,
        angle_deg,
        input.flute_count,
        query.material_family,
    ) {
        Ok(claim) => PlungeBasis::Claimed {
            claim: Box::new(claim),
            tip_cap,
            axial,
        },
        Err(reason) => PlungeBasis::MaterialBase {
            reason,
            base_mm_min: material_base_mm_min,
            tip_cap,
            axial,
        },
    }
}

/// Q8: the G6 axial-chip bound of `input`, from its op's entry contract and
/// the G6 ramp basis. `None` unless the contract is `StraightPlunge { feed:
/// AxialChip }` and a G6 claim covers the tool.
fn axial_chip_bound(
    input: &FeedsInput<'_>,
    ramp: &super::ramp::RampBasis,
) -> Option<AxialChipBound> {
    use crate::compute::catalog::{EntryContract, PlungeFeedRule};
    let op = input.operation_kind?;
    let states_axial_chip = match op.registry_entry().entry {
        EntryContract::StraightPlunge { feed, .. } => feed == PlungeFeedRule::AxialChip,
        EntryContract::Dressup { .. }
        | EntryContract::Planner { .. }
        | EntryContract::DescentIsCut => false,
    };
    if !states_axial_chip {
        return None;
    }
    match ramp {
        super::ramp::RampBasis::Sourced {
            axial_chip_mm,
            drill,
            ..
        } => Some(AxialChipBound {
            axial_chip_mm: *axial_chip_mm,
            flutes: drill.flutes,
            side_row: drill.side_row.clone(),
        }),
        super::ramp::RampBasis::NoChip { .. } => None,
    }
}

/// The plunge at `feed_mm_min` and `rpm`, and the term that sets it.
///
/// The value is `min(rule, tip cap, axial chip, feed)`, rounded DOWN to 1
/// mm/min (T-9: the value is bound from above). On a tie the rule binds,
/// then the tip cap, then the axial chip. The RPM reaches only the axial
/// chip (Q8). `None` for a drill cycle, and for a value that is not
/// positive.
#[must_use]
pub fn resolve_plunge(
    basis: &PlungeBasis,
    feed_mm_min: f64,
    rpm: f64,
) -> Option<(f64, PlungeBinding)> {
    let (rule_mm_min, tip_cap, axial) = match basis {
        PlungeBasis::Claimed {
            claim,
            tip_cap,
            axial,
        } => (claim.fraction * feed_mm_min, *tip_cap, axial.as_ref()),
        PlungeBasis::MaterialBase {
            base_mm_min,
            tip_cap,
            axial,
            ..
        } => (*base_mm_min, *tip_cap, axial.as_ref()),
        PlungeBasis::DrillCycle => return None,
    };
    let mut value = rule_mm_min;
    let mut binding = PlungeBinding::Rule;
    if let Some(cap) = tip_cap
        && cap.cap_mm_min < value
    {
        value = cap.cap_mm_min;
        binding = PlungeBinding::TipCap;
    }
    if let Some(bound) = axial {
        let vertical = bound.at_rpm(rpm);
        if vertical.is_finite() && vertical > 0.0 && vertical < value {
            value = vertical;
            binding = PlungeBinding::AxialChip;
        }
    }
    if feed_mm_min.is_finite() && feed_mm_min > 0.0 && feed_mm_min < value {
        value = feed_mm_min;
        binding = PlungeBinding::Feed;
    }
    let value = super::suggest::round_suggestion_value_down(value, 1.0);
    (value.is_finite() && value > 0.0).then_some((value, binding))
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::*;
    use crate::feeds::vendor_lut::{MaterialFamily, ToolFamily};

    fn claimed(family: ToolFamily, d: f64, tip: Option<f64>, z: u32) -> PlungeBasis {
        let claim = plunge_rule(family, d, tip, None, z, MaterialFamily::Hardwood)
            .unwrap_or_else(|e| panic!("{family:?}: {}", e.text()));
        let geometry = match family {
            ToolFamily::BallNose => ToolGeometryHint::Ball,
            ToolFamily::TaperedBallNose => ToolGeometryHint::TaperedBall {
                tip_radius: tip.unwrap_or(1.0) / 2.0,
                taper_angle_deg: 10.0,
            },
            ToolFamily::FlatEnd
            | ToolFamily::BullNose
            | ToolFamily::ChamferVbit
            | ToolFamily::FacingBit => ToolGeometryHint::Flat,
        };
        PlungeBasis::Claimed {
            claim: Box::new(claim),
            tip_cap: TipCap::for_tool(geometry, d),
            axial: None,
        }
    }

    /// The claim gives the fraction of the given feed: F / 2, F / 3 and
    /// 0.5 F.
    #[test]
    fn the_claim_is_the_fraction_of_the_feed() {
        let two = claimed(ToolFamily::FlatEnd, 6.0, None, 2);
        assert_eq!(
            resolve_plunge(&two, 3000.0, 18_000.0),
            Some((1500.0, PlungeBinding::Rule))
        );
        let three = claimed(ToolFamily::FlatEnd, 6.0, None, 3);
        assert_eq!(
            resolve_plunge(&three, 3000.0, 18_000.0),
            Some((1000.0, PlungeBinding::Rule))
        );
        let ball = claimed(ToolFamily::BallNose, 6.0, None, 2);
        assert_eq!(
            resolve_plunge(&ball, 1200.0, 18_000.0),
            Some((600.0, PlungeBinding::Rule))
        );
        // The 6 mm ball cap is 900 mm/min: at F 4000 the cap binds.
        assert_eq!(
            resolve_plunge(&ball, 4000.0, 18_000.0),
            Some((900.0, PlungeBinding::TipCap))
        );
    }

    /// A 1 mm tapered tip: the cap (150 mm/min) binds, under 0.5 F.
    #[test]
    fn the_tip_cap_binds_on_a_one_millimetre_tip() {
        let tapered = claimed(ToolFamily::TaperedBallNose, 6.35, Some(1.0), 2);
        assert_eq!(
            tapered.tip_cap().map(|c| c.cap_mm_min),
            Some(150.0),
            "the cap is 150 mm/min per mm of tip"
        );
        assert_eq!(
            resolve_plunge(&tapered, 2000.0, 18_000.0),
            Some((150.0, PlungeBinding::TipCap))
        );
        let (headline, detail) = tapered.card_text_at(2000.0, 18_000.0);
        assert!(headline.contains("set by the tip cap"), "{headline}");
        assert!(detail.contains(TIP_CAP_RULE_TEXT), "{detail}");
    }

    /// The feed binds when the material base is above it.
    #[test]
    fn the_feed_binds_when_the_base_is_above_it() {
        let base = PlungeBasis::MaterialBase {
            reason: PlungeRefusal::NoRuleForFamily(ToolFamily::BullNose),
            base_mm_min: 1000.0,
            tip_cap: None,
            axial: None,
        };
        assert_eq!(
            resolve_plunge(&base, 800.0, 18_000.0),
            Some((800.0, PlungeBinding::Feed))
        );
        assert_eq!(
            resolve_plunge(&base, 3000.0, 18_000.0),
            Some((1000.0, PlungeBinding::Rule))
        );
        let (headline, detail) = base.card_text();
        assert!(headline.contains("no source; repo rule"), "{headline}");
        assert!(headline.contains("bull nose"), "{headline}");
        assert!(detail.contains(PLUNGE_BASE_RULE_TEXT), "{detail}");
        // Audit WRONG #11: the headline says "no source; repo rule"; the
        // detail must not say it again.
        assert!(!detail.contains("no source; repo rule"), "{detail}");
        assert!(detail.ends_with(DOWN_CUT_PLUNGE_TEXT), "{detail}");
    }

    /// The value rounds down to 1 mm/min, and a drill cycle gives `None`.
    #[test]
    fn the_value_rounds_down() {
        let three = claimed(ToolFamily::FlatEnd, 6.0, None, 3);
        let (value, _) = resolve_plunge(&three, 2999.0, 18_000.0).unwrap();
        assert!((value - 999.0).abs() < 1e-12, "{value}");
        let two = claimed(ToolFamily::FlatEnd, 6.0, None, 2);
        let (value, _) = resolve_plunge(&two, 2001.9, 18_000.0).unwrap();
        assert!((value - 1000.0).abs() < 1e-12, "{value}");
        assert_eq!(
            resolve_plunge(&PlungeBasis::DrillCycle, 2000.0, 18_000.0),
            None
        );
    }

    /// The claim headline names the fraction, the family and the range;
    /// the detail ends with the Q12 sentence.
    #[test]
    fn the_claim_card_names_its_rule() {
        let (headline, detail) = claimed(ToolFamily::FlatEnd, 6.0, None, 2).card_text();
        assert_eq!(
            headline,
            "Plunge = feed / 2 (G10 plunge claim: flat end mill, Amana Spektra rule; Sienci, IDC, \
             3.0-12.7 mm, 2-3 flutes)"
        );
        assert!(detail.ends_with(DOWN_CUT_PLUNGE_TEXT), "{detail}");
    }
}
