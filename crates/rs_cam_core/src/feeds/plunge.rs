//! G10: the plunge resolver (`planning/extrapolation_2026-09-24/G10_PLAN.md`
//! §3 A2).
//!
//! The plunge has two parts, like the G6 ramp:
//!
//! 1. [`PlungeBasis`] is the part that does not use the feed F. `calculate`
//!    sets it on `FeedsResult::plunge` (step 8).
//! 2. [`resolve_plunge_at`] gives the number at a feed. `calculate` runs it
//!    at the calculator's F. The apply funnel (`suggest::apply`) runs it
//!    again at the feed that ships, because pass 9, pass 10 and an explored
//!    feed can move F.
//!
//! P18 (review D14): F is the feed BEFORE the radial chip-thinning lift
//! ([`UnliftedFeed`]). The Garr lift corrects a narrow radial width; a
//! plunge is axial and has no radial thinning. Until P18 the plunge read
//! the lifted feed: a 6 mm 2F flat at a 0.3 mm width plunged at 2.29 x the
//! axial chip of the same tool at full width.
//!
//! The value is `min(rule, tip cap, axial chip, Z axis rate, F)`, rounded down to 1
//! mm/min:
//!
//! - the rule is the G10 claim (`extrapolation::plunge`, fraction x F), or
//!   the named repo rule (the material base) when no claim covers the tool;
//! - the tip cap is a named repo rule for a ball or a tapered ball
//!   (`tool_load::plunge_stress`, the one producer);
//! - P20b: the one vertical-rate limit ([`VerticalRateLimit`]) bounds the
//!   plunge of every op: the G6 axial chip `g6_chip x rpm x Z` (report 02
//!   F-c1) where a G6 claim covers the tool, and the machine Z axis rate
//!   (GRBL `$112`) where the machine states it. Until P20b the axial chip
//!   bound only an op whose entry contract stated it (Project Curve, lead
//!   decision Q8a), and no producer read the Z axis rate;
//! - a plunge never exceeds the feed.
//!
//! A drill cycle has one feed. Its plunge is the drill feed (calculator
//! step 9c), so the resolver gives `None` for it. Step 9c bounds the drill
//! feed with the same limit ([`PlungeBasis::vertical_limit`]).

use super::extrapolation::plunge::flutes_text;
use super::extrapolation::{PlungeClaim, PlungeRefusal, plunge_rule};
use super::geometry::UnliftedFeed;
use super::provenance::ValueProvenance;
use super::vertical::{VerticalBinding, VerticalRateLimit, ZAxisRate};
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

/// The part of the plunge that does not use the feed.
#[derive(Debug, Clone, PartialEq)]
pub enum PlungeBasis {
    /// A G10 claim gives the plunge as a fraction of the feed.
    Claimed {
        claim: Box<PlungeClaim>,
        tip_cap: Option<TipCap>,
        /// P20b: the vertical-rate limit (the G6 axial chip, the Z axis
        /// rate).
        vertical: VerticalRateLimit,
    },
    /// No claim covers the tool. The named repo rule (the material base)
    /// gives the plunge.
    MaterialBase {
        reason: PlungeRefusal,
        base_mm_min: f64,
        tip_cap: Option<TipCap>,
        /// P20b: the vertical-rate limit (the G6 axial chip, the Z axis
        /// rate).
        vertical: VerticalRateLimit,
    },
    /// A drill cycle: the plunge is the drill feed. Step 9c bounds the
    /// drill feed with `vertical`.
    DrillCycle { vertical: VerticalRateLimit },
}

/// Which term sets the plunge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlungeBinding {
    /// The G10 claim or the material base.
    Rule,
    /// The ball or tapered-ball tip cap.
    TipCap,
    /// The G6 axial chip x rpm x Z (Q8; every op since P20b).
    AxialChip,
    /// P20b: the machine Z axis rate (GRBL `$112`).
    ZAxisRate,
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
            Self::ZAxisRate => "the machine Z axis rate",
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
            Self::DrillCycle { .. } => "DrillCycle",
        }
    }

    /// The claim, when a G10 claim covers the tool.
    #[must_use]
    pub fn claim(&self) -> Option<&PlungeClaim> {
        match self {
            Self::Claimed { claim, .. } => Some(claim),
            Self::MaterialBase { .. } | Self::DrillCycle { .. } => None,
        }
    }

    /// The tip cap, when the tool has one.
    #[must_use]
    pub fn tip_cap(&self) -> Option<TipCap> {
        match self {
            Self::Claimed { tip_cap, .. } | Self::MaterialBase { tip_cap, .. } => *tip_cap,
            Self::DrillCycle { .. } => None,
        }
    }

    /// P20b: the vertical-rate limit of the cut. The plunge, the drill feed
    /// (Step 9c) and the vertical part of a G6 ramp
    /// (`ramp::resolve_ramp_feed`) read it.
    #[must_use]
    pub const fn vertical_limit(&self) -> &VerticalRateLimit {
        match self {
            Self::Claimed { vertical, .. }
            | Self::MaterialBase { vertical, .. }
            | Self::DrillCycle { vertical } => vertical,
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
                vertical,
            } => {
                let rule = claim.rule;
                let fraction = claim.fraction_text();
                let mut terms = vec![fraction];
                if let Some(cap) = tip_cap {
                    terms.push(format!("tip cap {:.0} mm/min", cap.cap_mm_min));
                }
                terms.extend(vertical_terms(vertical));
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
                let detail = format!(
                    "{claim_detail}{cap}; {}. {DOWN_CUT_PLUNGE_TEXT}",
                    vertical.text()
                );
                (headline, detail)
            }
            Self::MaterialBase {
                reason,
                base_mm_min,
                tip_cap,
                vertical,
            } => {
                let mut cap = tip_cap.map_or_else(String::new, |c| {
                    format!(", tip cap {:.0} mm/min", c.cap_mm_min)
                });
                for term in vertical_terms(vertical) {
                    cap.push_str(&format!(", {term}"));
                }
                let headline = format!(
                    "Plunge: no source; repo rule material base {base_mm_min:.0} mm/min{cap} ({})",
                    reason.short()
                );
                let cap = tip_cap.map_or_else(String::new, |c| format!(" The {}.", c.text()));
                let detail = format!(
                    "The rule: {PLUNGE_BASE_RULE_TEXT}. {}{cap} The {}. {DOWN_CUT_PLUNGE_TEXT}",
                    reason.text(),
                    vertical.text()
                );
                (headline, detail)
            }
            Self::DrillCycle { vertical } => (
                "Plunge: the drill feed".to_owned(),
                format!(
                    "A drill cycle has one feed: the plunge is the drill feed. The {}; the RPM \
                     follows a capped feed down to hold the chip.",
                    vertical.text()
                ),
            ),
        }
    }

    /// The card text at a feed and an RPM. The headline names the term
    /// that binds.
    #[must_use]
    pub fn card_text_at(&self, feed: UnliftedFeed, rpm: f64) -> (String, String) {
        let (headline, detail) = self.card_text();
        match resolve_plunge_at(self, feed, rpm) {
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
    /// - The G6 axial chip stamps the G6 side row; the Z axis rate stamps
    ///   the machine field (P20b).
    /// - The feed that binds, and a drill cycle, copy the feed stamp.
    #[must_use]
    pub fn provenance(&self, binding: PlungeBinding, feed: ValueProvenance) -> ValueProvenance {
        match (self, binding) {
            (Self::DrillCycle { .. }, _) | (_, PlungeBinding::Feed) => feed,
            (_, PlungeBinding::TipCap) => ValueProvenance::repo_rule("ball_tip_cap"),
            (_, PlungeBinding::ZAxisRate) => ValueProvenance::of_basis(&super::Basis::Machine(
                super::sourced::MachineField::ZAxisRate,
            )),
            (
                Self::Claimed { vertical, .. } | Self::MaterialBase { vertical, .. },
                PlungeBinding::AxialChip,
            ) => vertical
                .axial()
                .map_or(feed, |b| ValueProvenance::vendor_lut(b.side_row())),
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
        use super::sourced::{ClaimKind, ClaimRef, MachineField, RowRef};
        match (self, binding) {
            (Self::DrillCycle { .. }, _) | (_, PlungeBinding::Feed) => {
                Basis::derived("plunge = feed", vec![feed.clone()])
            }
            (_, PlungeBinding::ZAxisRate) => Basis::derived(
                "machine Z axis rate",
                vec![Basis::Machine(MachineField::ZAxisRate)],
            ),
            (_, PlungeBinding::TipCap) => Basis::derived(
                "ball tip cap (repo rule, tool_load::plunge_stress)",
                vec![Basis::tool_diameter()],
            ),
            (
                Self::Claimed { vertical, .. } | Self::MaterialBase { vertical, .. },
                PlungeBinding::AxialChip,
            ) => Basis::derived(
                "Q8 G6 axial chip x rpm x Z",
                vertical
                    .axial()
                    .into_iter()
                    .map(|b| {
                        Basis::Claim(ClaimRef {
                            kind: ClaimKind::DrillG6,
                            row: RowRef(b.side_row().to_owned()),
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

/// The headline terms of the vertical-rate limit: the G6 axial chip
/// (`0.0635 x rpm x 2`) and the stated Z axis rate. A machine with no Z rate
/// adds no term; the detail line says that no Z cap applies.
fn vertical_terms(vertical: &VerticalRateLimit) -> Vec<String> {
    let mut terms = Vec::new();
    if let Some(bound) = vertical.axial() {
        terms.push(format!(
            "{:.4} x rpm x {}",
            bound.axial_chip_mm(),
            bound.flutes()
        ));
    }
    match vertical.z_axis() {
        ZAxisRate::Stated(rate) => terms.push(format!("Z axis rate {rate:.0} mm/min")),
        ZAxisRate::NotStated => {}
    }
    terms
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
/// 4. P20b: every basis carries the one vertical-rate limit
///    ([`VerticalRateLimit::new`]): the G6 axial chip when `ramp` holds a G6
///    claim (`RampBasis::Sourced`), and the machine Z axis rate.
#[must_use]
pub fn plunge_basis(
    input: &FeedsInput<'_>,
    material_base_mm_min: f64,
    ramp: &super::ramp::RampBasis,
) -> PlungeBasis {
    let vertical = VerticalRateLimit::new(ramp, input.machine);
    if input.operation == OperationFamily::Drill {
        return PlungeBasis::DrillCycle { vertical };
    }
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
            vertical,
        },
        Err(reason) => PlungeBasis::MaterialBase {
            reason,
            base_mm_min: material_base_mm_min,
            tip_cap,
            vertical,
        },
    }
}

/// P18 (review D14): the plunge at the feed before the lift, `feed`, and
/// `rpm`, and the term that sets it. Every product site calls this door
/// ([`resolve_plunge`] at `feed.mm_min()`). The type holds that no caller
/// passes the lifted feed.
#[must_use]
pub fn resolve_plunge_at(
    basis: &PlungeBasis,
    feed: UnliftedFeed,
    rpm: f64,
) -> Option<(f64, PlungeBinding)> {
    resolve_plunge(basis, feed.mm_min(), rpm)
}

/// The plunge at a raw feed `feed_mm_min` and `rpm`, and the term that sets
/// it. P18: a product site calls [`resolve_plunge_at`]; this raw form stays
/// public only for the FM1 instrument's `plunge_binding` column
/// (`tests/feeds_matrix_instrument_fm1.rs`). The sentry
/// `the_plunge_reads_the_feed_before_the_lift_p18` holds that no source
/// file outside this module calls it.
///
/// The value is `min(rule, tip cap, axial chip, Z axis rate, feed)`,
/// rounded DOWN to 1 mm/min (T-9: the value is bound from above). The
/// axial chip and the Z axis rate come from the one vertical-rate limit
/// ([`VerticalRateLimit::bound`], P20b). On a tie the rule binds, then the
/// tip cap, then the axial chip, then the Z axis rate. The RPM reaches only
/// the axial chip (Q8). `None` for a drill cycle, and for a value that is
/// not positive.
#[must_use]
pub fn resolve_plunge(
    basis: &PlungeBasis,
    feed_mm_min: f64,
    rpm: f64,
) -> Option<(f64, PlungeBinding)> {
    let (rule_mm_min, tip_cap, vertical) = match basis {
        PlungeBasis::Claimed {
            claim,
            tip_cap,
            vertical,
        } => (claim.fraction * feed_mm_min, *tip_cap, vertical),
        PlungeBasis::MaterialBase {
            base_mm_min,
            tip_cap,
            vertical,
            ..
        } => (*base_mm_min, *tip_cap, vertical),
        PlungeBasis::DrillCycle { .. } => return None,
    };
    let mut value = rule_mm_min;
    let mut binding = PlungeBinding::Rule;
    if let Some(cap) = tip_cap
        && cap.cap_mm_min < value
    {
        value = cap.cap_mm_min;
        binding = PlungeBinding::TipCap;
    }
    let bounded = vertical.bound(value, rpm);
    match bounded.binding() {
        VerticalBinding::Candidate => {}
        VerticalBinding::AxialChip => {
            value = bounded.mm_min();
            binding = PlungeBinding::AxialChip;
        }
        VerticalBinding::ZAxisRate => {
            value = bounded.mm_min();
            binding = PlungeBinding::ZAxisRate;
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

    /// No G6 chip and no Z axis rate (`MachineProfile::default()` has no
    /// kinematics).
    fn no_limit() -> VerticalRateLimit {
        VerticalRateLimit::new(
            &crate::feeds::ramp::RampBasis::NoChip {
                reason: crate::feeds::ramp::RampFallback::NoLut,
            },
            &crate::machine::MachineProfile::default(),
        )
    }

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
            vertical: no_limit(),
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
        let (headline, detail) =
            tapered.card_text_at(UnliftedFeed::over_lift(2000.0, 1.0), 18_000.0);
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
            vertical: no_limit(),
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
            resolve_plunge(
                &PlungeBasis::DrillCycle {
                    vertical: no_limit()
                },
                2000.0,
                18_000.0
            ),
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
