//! Vendor LUT lookup with scoring algorithm.
//!
//! Filters observations by must-match criteria, scores remaining candidates,
//! and returns the best match with chipload midpoint.

use super::extrapolation::{
    DrillBasis, Extrapolation, FamilyBasis, HardnessBasis, SizeBasis, SizeLaw, drill_basis,
    drill_rule, family_basis, hardness_basis, transfer_rule,
};
use super::vendor_lut::{
    EvidenceGrade, HardnessKind, LutOperationFamily, LutPassRole, MaterialFamily, ObservationKind,
    ToolFamily, VendorLut, VendorObservation,
};

/// Query parameters for LUT lookup.
#[derive(Debug, Clone)]
pub struct LookupQuery {
    pub tool_family: ToolFamily,
    /// The tool's series key (`feeds::series::ToolSeries::key`), or a row
    /// subfamily. `None` is a generic tool: the lookup reads the standard
    /// rows first (operator ruling 2026-10-31).
    pub tool_subfamily: Option<String>,
    pub diameter_mm: f64,
    pub flute_count: u32,
    pub material_family: MaterialFamily,
    pub hardness_kind: Option<HardnessKind>,
    pub hardness_value: Option<f64>,
    pub operation_family: LutOperationFamily,
    pub pass_role: LutPassRole,
}

pub type LookupCriteria = LookupQuery;
pub type MatchedRow = LookupResult;

/// Result of a successful LUT lookup.
///
/// Chipload fields are **scaled** to the query's diameter and hardness (and,
/// on a G6 drill claim, by 1 / Z: [`Self::drill_basis`]). The
/// query diameter is the lookup key (`feeds::geometry::lut_key_diameter_mm`):
/// the tip of a tapered ball (ruling A1, 2026-09-24), and the nominal
/// diameter otherwise, a V-bit included (ruling B4, 2026-09-25). A V-bit row
/// that prints no diameter is keyed at its printed angle
/// (`SizeBasis::AngleKey`) and is not scaled. When `is_extrapolated` is true the scaling
/// factors diverge from 1.0 by more than ±40 %, and downstream consumers
/// must demote verdict confidence to `Approximate` with a detail describing
/// the scaling.
#[derive(Debug, Clone)]
pub struct LookupResult {
    /// Scaled chipload midpoint (mm/tooth).
    pub chip_load_mm: f64,
    /// Scaled lower bound of the observation's chipload range, if reported.
    /// Below this is rubbing/burning territory. A row that prints one value
    /// (`min == max`) gets `None` here (A2): read [`Self::printed_chipload`].
    pub chip_load_min_mm: Option<f64>,
    /// Scaled upper bound of the observation's chipload range, if reported.
    /// Above this is breakage territory.
    pub chip_load_max_mm: Option<f64>,
    pub rpm_nominal: Option<f64>,
    pub rpm_min: Option<f64>,
    pub rpm_max: Option<f64>,
    pub ap_min_mm: Option<f64>,
    pub ap_max_mm: Option<f64>,
    /// Diameter-scaling lower bound for axial DOC (× cutter diameter).
    /// Carried through from `VendorObservation::ap_min_factor`. Combined
    /// with `ap_min_mm` in `feeds::cutter_constraints` via
    /// `max(factor × diameter, absolute_mm)` for the tighter floor.
    pub ap_min_factor: Option<f64>,
    /// Diameter-scaling upper bound for axial DOC (× cutter diameter).
    /// Carried through from `VendorObservation::ap_max_factor`. Combined
    /// with `ap_max_mm` via `min(factor × diameter, absolute_mm)`.
    pub ap_max_factor: Option<f64>,
    pub ae_min_mm: Option<f64>,
    pub ae_max_mm: Option<f64>,
    pub observation_id: String,
    /// R6: typed vendor identity — format with `Display` at UI edges.
    pub source_vendor: crate::feeds::vendor_lut::Vendor,
    pub score: i64,
    /// Diameter-proximity component of the composite score (0–200).
    /// 200 = exact diameter match; lower = more derated. Surfaced so
    /// downstream consumers (e.g. the F&S suggest module) can flag a
    /// match that won on tie-breakers despite a poor diameter fit.
    pub diameter_match_score: i64,
    /// The matched observation's calibrated diameter (mm). Useful for
    /// debug/UI to show how far the query's diameter diverged from the
    /// row's calibration.
    pub row_diameter_mm: f64,
    /// Scale factor **applied** to the chipload bounds for diameter. For a
    /// G1 size claim ([`Self::size_basis`] is `SizeBasis::Claim`) it is the
    /// claim's scale (form A, B or C, `feeds::extrapolation::size`).
    /// Otherwise it is
    /// `(query.diameter_mm / row.diameter_mm) ^ CHIPLOAD_DIAMETER_EXPONENT`,
    /// which is 1.0 (or within 6e-4 of it) on an exact row. **Diverges from
    /// [`Self::chipload_diameter_ratio_raw`] since 2026-08-06**, when the
    /// exponent moved off 1.0; the two are stored separately precisely so
    /// they cannot be conflated now that they differ.
    pub chipload_diameter_scale: f64,
    /// Scale factor **applied** to the chipload bounds for hardness:
    /// `(row.hardness_value / query.hardness_value) ^ CHIPLOAD_HARDNESS_EXPONENT`
    /// when kinds match, 1.0 otherwise. Softer query → larger factor →
    /// higher chipload. The factor is 1.0 for a composite-board query
    /// (plywood, MDF, HDF, particleboard). Inside solid wood the factor
    /// stops at the soft/hard cap of the row's tool family (ball 1.50, flat
    /// 1.43, ...). See [`Self::hardness_basis`].
    pub chipload_hardness_scale: f64,
    /// The **raw** diameter transfer ratio `query.diameter_mm /
    /// row.diameter_mm`, before any scaling law is applied (clamped only by
    /// [`SCALE_CLAMP_LO`]/[`SCALE_CLAMP_HI`]). This — not the applied scale
    /// — is what [`Self::is_extrapolated`] measures, because "how far is
    /// this row from the query" is a property of the row and the query, not
    /// of the exponent the crate currently believes relates them.
    pub chipload_diameter_ratio_raw: f64,
    /// The **raw** hardness transfer ratio `row.hardness_value /
    /// query.hardness_value`, before any scaling law. A solid-wood row with
    /// no Janka reads its value from the query table
    /// (`WoodSpecies::GenericSoftwood` or `GenericHardwood`). A
    /// composite-board query gives 1.0. See
    /// [`Self::chipload_diameter_ratio_raw`].
    pub chipload_hardness_ratio_raw: f64,
    /// True when the combined **raw** transfer ratio diverges from 1.0 by
    /// more than ±40 % (`|ln(raw_diameter × raw_hardness)| > ln(1.4)`).
    /// Verdicts derived from this row must be reported with
    /// `Confidence::Approximate`.
    ///
    /// **Read on the raw ratios deliberately** (the B-lit §4.1 rider,
    /// 2026-08-06). Computing it on the *applied* scales lets a softened
    /// exponent shrink the product back inside the threshold and silently
    /// un-flag a row that is exactly as far from the query as it was —
    /// which downgrades `Confidence::Approximate` to `Validated` and, via
    /// `ChipBoundsSource::low_side_is_advisory`, converts a burn advisory
    /// into a hard `Exceeds(Low)` trip. Pinned by
    /// `tests/chipload_extrapolation_flag_rider.rs`.
    pub is_extrapolated: bool,
    /// The matched observation's OWN pass role, which is **not**
    /// necessarily the one that was queried: pass role is a scoring
    /// term (`score_observation`, −25 points), not a hard filter, so a
    /// `semi_finish` row can and does win a `finish` query. Carried so
    /// a report can disclose that substitution instead of presenting
    /// the row as if it answered the question asked.
    ///
    /// Census T1.6 (2026-08-04); whether pass role should become a hard
    /// filter is Checkpoint B item T4.4 and is **not** decided here.
    ///
    /// A G3 family transfer ([`Self::family_basis`]) also gives a row role
    /// that differs from the query: the row's role is its home role.
    pub row_pass_role: crate::feeds::vendor_lut::LutPassRole,
    /// The G1 size basis of this row for this query
    /// (`feeds::extrapolation::SizeLaw`, extrapolation P1 step 3). A
    /// `Refused` basis publishes no band: `chip_load_mm` is 0 and both
    /// bounds are `None`, so no consumer can judge or ship against it.
    pub size_basis: SizeBasis,
    /// The G2 hardness basis of this row for this query
    /// (`feeds::extrapolation::hardness_basis`, extrapolation P2 step 4).
    /// [`Self::chipload_hardness_ratio_raw`] and
    /// [`Self::chipload_hardness_scale`] are its `ratio_raw()` and
    /// `scale()`. A `Capped` basis applies the soft/hard cap of the row's
    /// tool family and keeps the raw ratio.
    pub hardness_basis: HardnessBasis,
    /// The G3 family basis of this row for this query
    /// (`feeds::extrapolation::family_basis`, A3). `Transferred` when a
    /// family rule carries the row from its home operation family into the
    /// queried family; the band is the home row's band (copy semantics).
    pub family_basis: FamilyBasis,
    /// The G6 drill basis of this row for this query
    /// (`feeds::extrapolation::drill_basis`, ruling B5). `Transferred` when
    /// the drill rule reads a flat end mill's printed side row for a
    /// plunge: the chip fields carry the side chip / Z
    /// ([`DrillBasis::scale`]), and a printed point stays a point.
    pub drill_basis: DrillBasis,
    /// The matched row's printed material label (the A4 channel: a
    /// "Wood, MDF, Sign-Foam" row that serves hardwood says so here).
    pub material_label: String,
    /// The matched row's evidence grade.
    pub evidence_grade: EvidenceGrade,
    /// The matched row's kind (exact, derived or fallback).
    pub row_kind: ObservationKind,
    /// Feeds contract P16 item 8 (operator ruling D2): the vendor band of
    /// this query. This row is the band's shipped row. The band replaces
    /// the Q3 tie field (`tied_with`): a tie is a band of two. See
    /// [`VendorBand`].
    pub band: VendorBand,
}

/// Feeds contract P16 item 8 (operator ruling D2, 2026-11-06): the vendor
/// band of one query. The printed rows that match one tool, size and wood
/// form a band from the lowest to the highest chip. Suggest ships the
/// lowest; the surfaces name every member, so the operator can state a
/// higher chip inside the band (the stated-chip door).
///
/// A member is a candidate row of the lookup that passes every rule:
///
/// 1. The hard filters of the lookup (tool family, material category,
///    operation family or a stated transfer, the diameter sanity range).
///
///    Rule 1b, the tool family of the query first (feeds contract P18,
///    review D16, lead ruling option (a)): when a row of the query's own
///    tool family passes rule 4 (a printed size or a form A interpolation),
///    the band holds only rows of that family (flat with flat, bull with
///    bull). A row of another compatible family (rule 1,
///    `tool_family_compatible`) joins only when no row of the query family
///    passes rule 4. Such a band names the family of the shipped row
///    ([`VendorBand::cross_family`]).
/// 2. The same material family as the query. The D2 first half (one
///    reading of a missing hardness in the score) is held back (lead,
///    2026-11-06): the score reads the row's own hardness, so the FM1
///    chips do not rise.
/// 3. The same operation family and pass role as the score winner, after
///    the routes of the query (`vendor_normalize::lut_query_for`,
///    `LutRoute::FlatAsRoughPocket`, the G3 family rules). A row that a G3
///    family rule or the G6 drill rule transfers counts as its copy in the
///    queried family and role. A printed winner's band holds only printed
///    rows and a transferred winner's band only transferred rows: a row
///    printed in the family beats a transfer (A3), so a transfer does not
///    ship over it.
/// 4. The size: a printed row at the query size (G1 `SizeBasis::Exact`), or
///    a form A interpolation between two printed sizes of one chart series
///    (one member per series, at the interpolated value). Forms B and C
///    (extrapolation), an angle key and a row with no diameter give no
///    member.
/// 5. The same printed flute count as the query.
/// 6. A source of the vendor class: a row of a hobby feed chart
///    (`SourceClass::HobbyFeedChart`) never enters.
/// 7. A grade c row or a fallback row joins only when the band has no
///    grade a or grade b member.
/// 8. The same series rank as the score winner (`series::series_rank`: the
///    tool's own series, the standard rows, the other specialty rows; lead
///    provisional ruling 2026-11-06, option B). The series ruling of
///    2026-10-31 picks the winner's rank; the band does not cross it.
///
/// A member also prints a chip. Each member carries its chip after its own
/// size, hardness and drill scales: the chip each consumer reads for that
/// row. The members are in ascending order of the chip middle.
///
/// The shipped row (the matched row) is the member with the lowest chip
/// middle ([`BandRule::Lowest`]). The no-rise rule: when the score winner is
/// not a member and no member is strictly below its chip, or the score
/// winner prints no chip or its size is refused, the score winner ships
/// ([`BandRule::WinnerShips`]). When the winner is a member, the lowest
/// member is at or below it by the order.
/// When no row is a member, the score winner ships alone
/// ([`BandRule::NoMember`], the single-row rule before P16).
#[derive(Debug, Clone, PartialEq)]
pub struct VendorBand {
    /// The members, lowest chip middle first.
    pub members: Vec<BandMember>,
    /// The row that the score and the series rule pick.
    pub score_winner: crate::feeds::sourced::RowRef,
    /// The scaled chip middle of the score winner, or `None` when the
    /// winner prints no chip or its size is refused. The shipped chip is at
    /// or below it (the no-rise rule).
    pub score_winner_chip_mm: Option<f64>,
    /// Which rule picked the shipped row.
    pub rule: BandRule,
    /// Why the score winner is not a member, or `None` when it is one.
    pub winner_outside: Option<BandExclusion>,
    /// P18 (review D16): the tool family of the shipped row and of the
    /// query, when the two differ (rule 1b: no row of the query family
    /// passes the size rule). `None` when the shipped row is of the query
    /// family.
    pub cross_family: Option<CrossFamily>,
}

/// P18 (review D16): a shipped row of another tool family than the tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CrossFamily {
    /// The tool family of the shipped row.
    pub shipped: ToolFamily,
    /// The tool family of the query (the tool).
    pub tool: ToolFamily,
}

/// One member of a [`VendorBand`]. The chip fields are scaled like
/// [`LookupResult::chip_load_mm`] and its bounds.
#[derive(Debug, Clone, PartialEq)]
pub struct BandMember {
    pub row: crate::feeds::sourced::RowRef,
    pub vendor: super::vendor_lut::Vendor,
    /// The scaled chip middle (mm/tooth).
    pub chip_mid_mm: f64,
    /// The scaled lower bound, `None` for a printed point (A2).
    pub chip_min_mm: Option<f64>,
    /// The scaled upper bound.
    pub chip_max_mm: Option<f64>,
    /// The G1 size basis of the member: `Exact` or a form A claim.
    pub size_basis: SizeBasis,
}

/// The rule that picked the shipped row of a [`VendorBand`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BandRule {
    /// The member with the lowest chip ships.
    Lowest,
    /// The score winner is not a member and no member is strictly below its
    /// chip (or the winner has no chip to compare), so the score winner
    /// ships. No chip rises.
    WinnerShips,
    /// No row is a member: the score winner ships alone.
    NoMember,
}

impl BandRule {
    /// The short name, for the FM1 column and the JSON record.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Lowest => "lowest",
            Self::WinnerShips => "winner_ships",
            Self::NoMember => "no_member",
        }
    }
}

/// The first band rule that a row fails, in the order of the
/// [`VendorBand`] rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BandExclusion {
    /// Rule 1b: another tool family than the query, while a row of the
    /// query family is a member (P18).
    ToolFamily,
    /// Rule 2: another material family.
    Material,
    /// Rule 3: another operation family or pass role than the winner.
    Operation,
    /// Rule 4: the size basis is not `Exact` or form A.
    Size,
    /// Rule 5: another flute count.
    Flutes,
    /// Rule 6: a hobby feed chart.
    Hobby,
    /// Rule 7: grade c or fallback while a grade a or b member exists.
    Grade,
    /// The row prints no chip, or its size is refused.
    NoChip,
}

impl BandExclusion {
    /// The short name, for the FM1 column and the JSON record.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::ToolFamily => "tool_family",
            Self::Material => "material",
            Self::Operation => "operation",
            Self::Size => "size",
            Self::Flutes => "flutes",
            Self::Hobby => "hobby",
            Self::Grade => "grade",
            Self::NoChip => "no_chip",
        }
    }
}

impl VendorBand {
    /// The band of a row with no member: the row ships alone
    /// ([`BandRule::NoMember`]).
    #[must_use]
    pub fn alone(observation_id: &str) -> Self {
        Self {
            members: Vec::new(),
            score_winner: crate::feeds::sourced::RowRef(observation_id.to_owned()),
            score_winner_chip_mm: None,
            rule: BandRule::NoMember,
            winner_outside: None,
            cross_family: None,
        }
    }

    /// The member with the lowest chip middle.
    #[must_use]
    pub fn lowest(&self) -> Option<&BandMember> {
        self.members.first()
    }

    /// The member with the highest chip middle.
    #[must_use]
    pub fn highest(&self) -> Option<&BandMember> {
        self.members.last()
    }

    /// The one JSON record of the band. CLI `suggest --json`
    /// (`profile.vendor_band`) and MCP `get_suggest_rationale`
    /// (`basis.vendor_band`) print it.
    #[must_use]
    pub fn to_json(&self) -> serde_json::Value {
        let member = |m: &BandMember| {
            serde_json::json!({
                "row": m.row.id(),
                "vendor": m.vendor.to_string(),
                "chip_mid_mm": m.chip_mid_mm,
                "chip_min_mm": m.chip_min_mm,
                "chip_max_mm": m.chip_max_mm,
                "size_basis": m.size_basis.name(),
            })
        };
        serde_json::json!({
            "members": self.members.iter().map(member).collect::<Vec<_>>(),
            "lowest": self.lowest().map(member),
            "highest": self.highest().map(member),
            "score_winner": self.score_winner.id(),
            "score_winner_chip_mm": self.score_winner_chip_mm,
            "rule": self.rule.name(),
            "winner_outside": self.winner_outside.map(BandExclusion::name),
            "cross_family": self.cross_family.map(|c| serde_json::json!({
                "shipped": c.shipped.label(),
                "tool": c.tool.label(),
            })),
        })
    }

    /// The card line: "Vendors: Amana 0.1270, Freud 0.2413 mm/tooth (ships
    /// the lowest)". `None` for a band of fewer than two members.
    #[must_use]
    pub fn card_text(&self) -> Option<String> {
        // P18 (review D16): a shipped row of another tool family is named
        // on the card, also with fewer than two members.
        let family = self.cross_family.map(|c| {
            format!(
                "; the row is a {} row: no {} row prints this size",
                c.shipped.label(),
                c.tool.label()
            )
        });
        if self.members.len() < 2 {
            return family.map(|f| format!("Vendor row{f}"));
        }
        let names: Vec<String> = self
            .members
            .iter()
            .map(|m| format!("{} {:.4}", m.vendor, m.chip_mid_mm))
            .collect();
        let tail = match self.rule {
            BandRule::Lowest => "ships the lowest",
            BandRule::WinnerShips => "ships the score winner, which is not a member",
            BandRule::NoMember => "ships the score winner",
        };
        Some(format!(
            "Vendors: {} mm/tooth ({tail}){}",
            names.join(", "),
            family.unwrap_or_default()
        ))
    }
}

/// The printed chipload of a matched row, classified (A2, point mode).
///
/// A vendor that prints one value gives a point, not a band. No consumer
/// derives a band from a point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PrintedChipload {
    /// Two printed limits with `min_mm < max_mm` (scaled).
    Band { min_mm: f64, max_mm: f64 },
    /// One printed value (scaled): a maximum only, a minimum only, or
    /// two equal limits.
    Point { value_mm: f64 },
    /// No printed chipload (or a refused size basis).
    Unpublished,
}

impl PrintedChipload {
    /// The point value, or `None` for a band or no print.
    pub fn point_mm(self) -> Option<f64> {
        match self {
            Self::Point { value_mm } => Some(value_mm),
            Self::Band { .. } | Self::Unpublished => None,
        }
    }

    /// The band limits `(min, max)`, or `None` for a point or no print.
    pub fn band_mm(self) -> Option<(f64, f64)> {
        match self {
            Self::Band { min_mm, max_mm } => Some((min_mm, max_mm)),
            Self::Point { .. } | Self::Unpublished => None,
        }
    }
}

impl LookupResult {
    /// Classify the two stored limits as a band, a point or no print.
    ///
    /// - Both limits with `lo < hi` give `Band`.
    /// - One limit, or `lo == hi`, gives `Point`.
    /// - Anything else gives `Unpublished`.
    pub fn printed_chipload(&self) -> PrintedChipload {
        let finite_pos = |v: f64| v.is_finite() && v > 0.0;
        match (
            self.chip_load_min_mm.filter(|v| finite_pos(*v)),
            self.chip_load_max_mm.filter(|v| finite_pos(*v)),
        ) {
            (Some(lo), Some(hi)) if lo < hi => PrintedChipload::Band {
                min_mm: lo,
                max_mm: hi,
            },
            (Some(lo), Some(hi)) if lo == hi => PrintedChipload::Point { value_mm: hi },
            (None, Some(v)) | (Some(v), None) => PrintedChipload::Point { value_mm: v },
            _ => PrintedChipload::Unpublished,
        }
    }
}

/// The card line that names the key a V-bit row is read at (ruling B4,
/// G5). `None` for every other tool family, and for a V-bit row that
/// prints neither a diameter nor an angle.
///
/// - An angle-keyed row (`SizeBasis::AngleKey`): "keyed at the printed
///   angle 60 deg (the chart prints no cutting diameter)".
/// - A row with a printed diameter: "keyed at the printed cutting diameter
///   25.4 mm". The G1 size claim (or refusal) of that row states how the
///   band moves to the tool's nominal diameter.
#[must_use]
pub fn vbit_key_text(tool: ToolFamily, row: &LookupResult) -> Option<String> {
    if tool != ToolFamily::ChamferVbit {
        return None;
    }
    if let SizeBasis::AngleKey { angle_deg } = row.size_basis {
        return Some(format!(
            "keyed at the printed angle {angle_deg} deg (the chart prints no cutting diameter)"
        ));
    }
    (row.row_diameter_mm.is_finite() && row.row_diameter_mm > 0.0).then(|| {
        format!(
            "keyed at the printed cutting diameter {} mm",
            row.row_diameter_mm
        )
    })
}

/// Diameter + hardness-scored LUT lookup for non-angle-aware cutters.
///
/// Thin wrapper around `lookup_best`; kept as a named entry point so
/// call sites read as "find the best vendor row" rather than "look up
/// best". Angle-aware (V-bit / chamfer) queries should use
/// [`find_best_vbit_row`] or the canonical [`find_best_row_for_geometry`]
/// which routes by cutter class.
pub fn find_best_row(lut: &VendorLut, criteria: &LookupCriteria) -> Option<MatchedRow> {
    lookup_best(lut, criteria)
}

/// Maximum V-bit included-angle mismatch (degrees) tolerated before a
/// vendor row is rejected for a cone cutter. A 90° bit must not be matched
/// against a 120° V-groove row — the chip geometry and engaged width at a
/// given depth differ too much for a chipload envelope to carry over. Rows
/// that don't record an angle (legacy data) are exempt from the gate and
/// scored neutrally, so V-bit lookups never regress below `find_best_row`.
const VBIT_ANGLE_TOLERANCE_DEG: f64 = 20.0;

/// Bonus for an exact V-bit angle match, decaying linearly to 0 at the
/// tolerance edge. Sized to dominate the diameter-proximity term (≤ 200)
/// so a same-angle row outranks a closer-diameter wrong-angle row.
const VBIT_ANGLE_MATCH_BONUS: f64 = 200.0;

/// Angle-aware variant of [`find_best_row`] for V-bit / chamfer cutters.
///
/// When both the query and a candidate row carry an included angle, rows
/// whose angle differs by more than [`VBIT_ANGLE_TOLERANCE_DEG`] are
/// rejected and closer-angle rows score higher. When either side lacks an
/// angle the row passes through with neutral scoring, so this never does
/// worse than `find_best_row` on angle-less legacy data. This is the slot
/// incoming V-groove datasets plug into: tag a row with `included_angle_deg`
/// and it becomes selectable only for cutters of that cone angle.
pub fn find_best_vbit_row(
    lut: &VendorLut,
    criteria: &LookupCriteria,
    query_included_angle_deg: Option<f64>,
) -> Option<MatchedRow> {
    find_best_vbit_row_where(lut, criteria, query_included_angle_deg, |_| true)
}

fn find_best_vbit_row_where(
    lut: &VendorLut,
    criteria: &LookupCriteria,
    query_included_angle_deg: Option<f64>,
    extra: impl Fn(&VendorObservation) -> bool,
) -> Option<MatchedRow> {
    find_best_vbit_row_in(lut, criteria, query_included_angle_deg, extra)
}

fn find_best_vbit_row_in(
    lut: &VendorLut,
    criteria: &LookupCriteria,
    query_included_angle_deg: Option<f64>,
    extra: impl Fn(&VendorObservation) -> bool,
) -> Option<MatchedRow> {
    let mut candidates: Vec<Candidate> = Vec::new();
    for (i, obs) in lut.observations.iter().enumerate() {
        let transferred = is_transferred(criteria, obs);
        if !passes_must_match(criteria, obs, transferred) || !extra(obs) {
            continue;
        }
        let angle_bonus = match (query_included_angle_deg, obs.included_angle_deg) {
            (Some(q), Some(o)) => {
                let diff = (q - o).abs();
                if diff > VBIT_ANGLE_TOLERANCE_DEG {
                    continue;
                }
                ((1.0 - diff / VBIT_ANGLE_TOLERANCE_DEG) * VBIT_ANGLE_MATCH_BONUS) as i64
            }
            _ => 0,
        };
        let (base_score, diam_score) = score_observation(criteria, obs, transferred);
        // Ruling B4: a V-bit is keyed at its nominal diameter, so it takes
        // the nearer-diameter tie-break of every other family
        // (`best_by_score`).
        candidates.push((i, base_score + angle_bonus, diam_score, transferred));
    }
    let best = best_by_score(lut, criteria, candidates.iter().copied())?;
    let chosen = prefer_series(lut, criteria, &candidates, best);
    Some(with_band(lut, criteria, &candidates, chosen))
}

/// **The RECIPE resolver.** Answers "which vendor row best describes
/// this cut?" for the feeds-and-speeds calculator, and **every** row
/// competes — including the RPM-only anchors that publish
/// `rpm_nominal` / `rpm_max` and no chipload column at all.
///
/// Routes V-bit / chamfer queries to the angle-aware matcher, everything
/// else to the diameter+hardness matcher, so a 30° V-bit query never
/// silently matches a 90° row. (Pre-Phase-1E only
/// `tool_load::chipload::evaluate` did that routing inline; the Suggest
/// path and the GUI feeds modal called `find_best_row` directly and lost
/// the angle constraint. Centralising the dispatch here keeps that
/// contract in one place.)
///
/// # Its purpose, declared — Checkpoint K (a3), 2026-08-13
///
/// This resolver and [`find_best_chip_envelope_row`] are **deliberately
/// two functions with two purposes**, not a duplication awaiting a
/// merge. A-6's census
/// (`planning/review_2026-08-08/LUT_BOUNDARY_EVIDENCE.md` §1) swept
/// 18 144 queries through both:
///
/// | outcome | count |
/// |---|---:|
/// | no LUT coverage at all | 13 653 |
/// | both resolvers, same row | 4 350 |
/// | Suggest on an RPM-only row, gate on a banded one | **141** |
/// | two *different bands* | **0** |
///
/// The envelope candidate set is a strict **subset** of this one, so
/// this resolver can never miss where that one matches, and the two can
/// differ **only** when an RPM-only row outscores every chipload-bearing
/// row. In those 141 cases the caller gets a row whose
/// `chip_load_mm` is `0.0` — the RPM anchor is real and wanted (standing
/// preference: material and hardness *dial* parameters, they do not
/// hard-reject rows), but the caller must then fall back to the formula
/// chipload and has **no band**. `feeds::calculate` does exactly that and
/// discloses it as
/// [`crate::feeds::FeedsWarning::VendorRowPublishesNoChipload`].
///
/// **Do not** call this where a chipload *envelope* is required; call
/// [`find_best_chip_envelope_row`], which is the contract every gate,
/// the optimizer and the viewport are written against.
///
/// # The V-bit exception to Checkpoint K — ruling B4, 2026-09-25
///
/// A V-bit query does not let every row compete at once. The chip rows
/// (`has_chipload`) compete first; an RPM-only row answers a V-bit only
/// when no chip row matches. So for a V-bit this resolver returns the same
/// row as [`find_best_chip_envelope_row`] whenever that resolver matches,
/// and Suggest and the gate read one row by construction. Before B4 the
/// Whiteside 1540 / 1550 RPM anchors (22 000 / 20 000 rpm, no chipload)
/// outscored the printed Amana AMS-159 point for a 60 degree V-bit in
/// solid wood, and Suggest shipped the formula chipload while the gate
/// judged the Amana row. The other families keep the rule above. After B4
/// no RPM-only row wins a query on the embedded LUT, so the census
/// (`tests/lut_resolver_census_a6.rs`, `f_lut2_divergence_surface_is_pinned`)
/// reads no divergent query.
pub fn find_best_row_for_geometry(
    lut: &VendorLut,
    criteria: &LookupCriteria,
    geometry: &crate::feeds::ToolGeometryHint,
) -> Option<MatchedRow> {
    match geometry {
        crate::feeds::ToolGeometryHint::VBit { included_angle, .. } => {
            find_best_vbit_row_where(lut, criteria, Some(*included_angle), has_chipload)
                .or_else(|| find_best_vbit_row(lut, criteria, Some(*included_angle)))
        }
        crate::feeds::ToolGeometryHint::Flat
        | crate::feeds::ToolGeometryHint::Ball
        | crate::feeds::ToolGeometryHint::Bull { .. }
        | crate::feeds::ToolGeometryHint::TaperedBall { .. } => find_best_row(lut, criteria),
    }
}

/// **The ENVELOPE resolver.** Answers "which vendor row publishes a
/// band I may judge this cut against?" — like
/// [`find_best_row_for_geometry`] but only rows publishing at least one
/// chipload bound compete.
///
/// F3.4 (defect class C5/A1): RPM-only rows (e.g.
/// `whiteside_rpm_assorted.json`) are legitimate RPM anchors for the
/// feeds calculator, but when one outranks a chipload-bearing row it
/// forces the chipload gate to `Unmodeled(NoVendorData)` even though
/// usable rows exist underneath (A1 blocker). Every consumer that
/// needs a chipload envelope — the gate, the optimizer context, the
/// viewport envelope map — must resolve rows through this entry point.
///
/// # Its purpose, declared — Checkpoint K (a3), 2026-08-13
///
/// The extra filter here is the **only** difference from the recipe
/// resolver: same scorer, same tie-break, same angle-aware dispatch. For a
/// V-bit the recipe resolver runs this filter first (ruling B4), so the two
/// give one row whenever this one matches.
/// So the two are not redundant and neither is "the right one" — a
/// recommendation may legitimately rest on an RPM anchor, and a verdict
/// may not. Pinned by
/// `envelope_resolver_never_matches_where_geometry_resolver_does_not`.
///
/// Note that this function is **not** where the two consumers historically
/// diverged in a way that moved numbers. That was the *query*, not the
/// resolver: the gate side routes `Adaptive3d` / `ProjectCurve` through
/// [`crate::feeds::vendor_normalize::lut_query_for`] and, until Checkpoint
/// K (a4), the Suggest side did not.
pub fn find_best_chip_envelope_row(
    lut: &VendorLut,
    criteria: &LookupCriteria,
    geometry: &crate::feeds::ToolGeometryHint,
) -> Option<MatchedRow> {
    match geometry {
        crate::feeds::ToolGeometryHint::VBit { included_angle, .. } => {
            find_best_vbit_row_where(lut, criteria, Some(*included_angle), has_chipload)
        }
        crate::feeds::ToolGeometryHint::Flat
        | crate::feeds::ToolGeometryHint::Ball
        | crate::feeds::ToolGeometryHint::Bull { .. }
        | crate::feeds::ToolGeometryHint::TaperedBall { .. } => {
            lookup_best_where(lut, criteria, has_chipload)
        }
    }
}

/// The envelope filter: the row prints at least one chipload bound. The
/// envelope resolver admits only these rows, and the recipe resolver tries
/// them first for a V-bit (ruling B4).
fn has_chipload(obs: &VendorObservation) -> bool {
    obs.chipload_min_mm_tooth.is_some() || obs.chipload_max_mm_tooth.is_some()
}

/// Cap on extrapolation scaling so a wildly mismatched row can't return
/// physically absurd chipload bounds. Linear scaling outside this band is
/// clamped before being applied — the lookup still succeeds, the
/// `is_extrapolated` flag is set, and the verdict is downgraded to
/// `Approximate`. 0.1× / 10× covers a 100× diameter span which is well past
/// any real wood-tool extrapolation we want to do.
pub const SCALE_CLAMP_LO: f64 = 0.1;
pub const SCALE_CLAMP_HI: f64 = 10.0;

/// Threshold past which a scaled row is reported as extrapolated. ±40 %
/// (≈ ln 1.4) on the *combined* raw diameter × hardness transfer ratio.
/// Picked to span the LUT's existing diameter-row spacing (3.175 mm →
/// 6.0 mm = 1.89×) so neighbouring rows don't trip Approximate, but a 1 mm
/// tip against a 3.175 mm row (0.31× = ln 1.16) does.
pub const CHIPLOAD_EXTRAPOLATION_LN_THRESHOLD: f64 = 0.336_472_236_621_213_07; // f64::ln(1.4)

/// Exponent on the raw diameter transfer ratio when carrying a vendor
/// chipload band from the row's calibrated diameter to the query's.
///
/// **SHIPPED VALUE: 0.61** — adopted 2026-08-06, replacing a linear
/// `^1.0` carry-over that no vendor family in the shipped LUT supports
/// except Freud. `CHIPLOAD_LITERATURE_VERDICT.md` §4.1 regressed the
/// shipped vendor charts (Onsrud's own Hard/Soft Wood PDFs, all 252 LUT
/// observations, grouped by source × subfamily × material × flutes ×
/// role so series differences cannot masquerade as a diameter effect)
/// and found per-family exponents spanning **0.23–1.25**, row-weighted
/// centre 0.52, unweighted cross-family median 0.40.
///
/// 0.61 rather than 0.52 because it is already
/// `machine::ChipLoadFormula::default().p`, so the LUT path adopts the
/// formula path and only **one** implementation moves; because it is
/// what the widest-span series measures (Amana Spektra, Ø0.79–12.7 mm,
/// 16× span, 0.586 MDF / 0.619 softwood at r² 0.89–0.97, against ≤ 6×
/// for every other series); and because it sits inside the physical
/// bracket `[0, 1]` set by the deflection-limited and strength-limited
/// ends. Piecewise was tested and rejected — one series in one material
/// in one chart supports a small-tool break and the same chart's other
/// material does not reproduce it.
///
/// **It is a fitted summary of seven vendors who disagree, not a
/// physical constant, and must never be cited as one.**
///
/// Magnitudes, measured before adoption over 11 712 (query, row) pairs:
/// pivot at Ø6.35 (median ×1.000), median **×2.056 at Ø1** and
/// **×0.780 at Ø12**; extremes ×2.455 / ×0.407, all clamp-bound and all
/// already `is_extrapolated`. See
/// `planning/review_2026-08-04/LAW_MAGNITUDE_TABLES.md` §2 and
/// `tests/law_magnitude_measurement.rs`.
///
/// The exponent exists as a named constant — rather than being implicit
/// in a bare division — precisely so the raw ratio and the applied scale
/// stay distinguishable when the value moves. See
/// [`LookupResult::is_extrapolated`], whose rule reads the **raw**
/// ratios for exactly that reason.
pub const CHIPLOAD_DIAMETER_EXPONENT: f64 = 0.61;

/// Exponent on the raw hardness transfer ratio
/// (`row_hardness / query_hardness`, so `chipload ∝ Janka^-0.5` is
/// expressed here as `+0.5`).
///
/// **SHIPPED VALUE: 0.5** — adopted 2026-08-06, replacing `^1.0`.
/// Bracketed `[0.48, 0.67]` by USDA Forest Service *Wood Handbook*
/// GTR-190 (2010) Ch. 5 Table 5–11a: side hardness scales as `G^1.49`
/// (softwoods) / `G^2.09` (hardwoods) while the strength axes a cutting
/// edge actually loads — compression and shear parallel — scale as
/// `G^0.85–1.13 ≈ G^1.0`; constant force per tooth then gives
/// `f_z ∝ Janka^(-1/2.09) … Janka^(-1/1.49)`. 0.5 is the conservative
/// edge of that bracket.
///
/// It also reproduces the **formula** path to ≤ 0.71 % across the whole
/// calibrated Janka band, so only one implementation moves. The census's
/// C-7 read the formula path's exponent as 1.26; B-lit §4.2 corrected
/// that — `(1/feed_scale)^1.26` with `feed_scale = (janka/600)^0.4`
/// composes to `(janka/600)^-0.504`, so the true disagreement was
/// `^1.0` vs `^0.504`, not `^1.0` vs `^1.26`.
///
/// **Honest uncertainty.** The vendors' own charts derate *less* than
/// this: 125 matched Onsrud hardwood/softwood pairs give a geometric-mean
/// ratio of 0.930, implying `q ≈ 0.08`, and four V-groove families
/// publish `q = 0` exactly. 0.5 sits deliberately between the vendor
/// evidence and the FPL physics, closer to the physics, because this
/// exponent transfers a row onto materials it was never measured on —
/// including Ipe at Janka 3510, 2.7× beyond any chart's "hard wood".
/// A transfer law should be more conservative than an in-domain chart.
/// Anyone who benches permissible chipload against Janka should expect
/// to move this number.
///
/// Magnitudes: median **×0.845 into softwood** (conservative — bands
/// drop) and **×1.650 into Ipe** (permissive), worst ×2.650 on a
/// softwood-anchored row. `LAW_MAGNITUDE_TABLES.md` §3.
pub const CHIPLOAD_HARDNESS_EXPONENT: f64 = 0.5;

/// Apply a chipload transfer law to a raw ratio.
///
/// Separated from the ratio itself so that `is_extrapolated` can read the
/// ratio while the band reads the scale. At `exponent == 1.0` this is
/// bit-identical to the input (IEEE-754 `pow(x, 1) == x`), which
/// `tests/chipload_extrapolation_flag_rider.rs` still asserts — that
/// property is what made the seam free to introduce in the wave before
/// the exponents moved, and it stays pinned so a future rollback to
/// `^1.0` is provably a no-op.
#[must_use]
pub fn apply_chipload_law(raw_ratio: f64, exponent: f64) -> f64 {
    raw_ratio.powf(exponent)
}

/// **The extrapolation rule**, on the RAW transfer ratios.
///
/// `|ln(raw_diameter_ratio × raw_hardness_ratio)| > ln 1.4`. Exposed so a
/// consumer (and the rider sentry) can ask the question without
/// re-deriving the threshold, and so the answer cannot drift from the one
/// [`LookupResult::is_extrapolated`] carries.
#[must_use]
pub fn is_extrapolated_for_ratios(raw_diameter_ratio: f64, raw_hardness_ratio: f64) -> bool {
    (raw_diameter_ratio * raw_hardness_ratio).ln().abs() > CHIPLOAD_EXTRAPOLATION_LN_THRESHOLD
}

/// The RAW diameter transfer ratio `query / row`, clamped only against
/// absurd extrapolation. No law is applied here — see
/// [`apply_chipload_law`].
fn diameter_ratio_raw(query_d: f64, row_d: Option<f64>) -> f64 {
    match row_d {
        Some(rd) if rd > 0.0 && query_d > 0.0 => {
            (query_d / rd).clamp(SCALE_CLAMP_LO, SCALE_CLAMP_HI)
        }
        // No anchor diameter on the row (v-bit / diameter-window
        // article): chipload bounds carry through unscaled.
        _ => 1.0,
    }
}

/// The one place a matched row becomes a `LookupResult`. Both resolvers
/// (recipe and envelope) pass through here, so the G1 size claim below is
/// the one number that every consumer reads (extrapolation P1 step 3; the
/// claim runs on every off-size match, not only when no row matches:
/// orchestrator decision 8).
fn build_result(
    lut: &VendorLut,
    obs: &VendorObservation,
    query: &LookupQuery,
    score: i64,
    diameter_match_score: i64,
) -> LookupResult {
    // The rider (B-lit §4.1 ⚠): the extrapolation flag reads the RAW
    // transfer ratios; only the band reads the applied scales. While both
    // exponents are 1.0 these are the same numbers, which is exactly why
    // the separation has to be made now rather than at the moment an
    // exponent moves.
    let diameter_ratio_raw = diameter_ratio_raw(query.diameter_mm, obs.diameter_mm);
    // The G2 hardness basis: the raw ratio and the applied scale come from
    // one place. A capped basis (the soft/hard cap, P2 step 4) keeps its raw
    // ratio, so the flag below still reads it.
    let hardness_basis = hardness_basis(query, obs);
    let hardness_ratio_raw = hardness_basis.ratio_raw();
    let is_extrapolated = is_extrapolated_for_ratios(diameter_ratio_raw, hardness_ratio_raw);
    // The G1 size claim. A claim replaces the generic law with its own
    // scale (one scalar on min, mid and max). Every other basis keeps the
    // generic law, so an exact row moves by no number.
    let size_basis = SizeLaw.basis(lut, query, obs);
    let diameter_scale = match size_basis.claim() {
        Some(claim) => claim.scale,
        None => apply_chipload_law(diameter_ratio_raw, CHIPLOAD_DIAMETER_EXPONENT),
    };
    let hardness_scale = hardness_basis.scale();
    // The G6 drill basis (ruling B5): a flat end mill plunge reads the side
    // chip / Z. One more scalar on min, mid and max, so a printed point
    // stays a point.
    let drill_basis = drill_basis(query, obs);
    let total_scale = diameter_scale * hardness_scale * drill_basis.scale();
    // The G3 family basis: a mark only. The band is the row's own band.
    let family_basis = family_basis(query, obs);
    // A refused basis publishes no band, so no consumer can use it.
    let refused = size_basis.is_refused();

    LookupResult {
        chip_load_mm: if refused {
            0.0
        } else {
            chipload_midpoint(obs) * total_scale
        },
        // A2: a row that prints one value (min == max) is a point. The
        // minimum becomes `None`, so a point always reaches consumers as
        // "max present, min absent" (`LookupResult::printed_chipload`).
        chip_load_min_mm: obs
            .chipload_min_mm_tooth
            .filter(|_| !refused)
            .filter(|&lo| obs.chipload_max_mm_tooth != Some(lo))
            .map(|v| v * total_scale),
        chip_load_max_mm: obs
            .chipload_max_mm_tooth
            .filter(|_| !refused)
            .map(|v| v * total_scale),
        rpm_nominal: obs.rpm_nominal,
        rpm_min: obs.rpm_min,
        rpm_max: obs.rpm_max,
        ap_min_mm: obs.ap_min_mm,
        ap_max_mm: obs.ap_max_mm,
        ap_min_factor: obs.ap_min_factor,
        ap_max_factor: obs.ap_max_factor,
        ae_min_mm: obs.ae_min_mm,
        ae_max_mm: obs.ae_max_mm,
        observation_id: obs.observation_id.clone(),
        source_vendor: obs.source_vendor,
        score,
        diameter_match_score,
        // 0.0 is a safe sentinel for "no diameter anchor" because real
        // cutter diameters are always > 0; downstream consumers that
        // care about the no-anchor case can check `< f64::EPSILON`.
        row_diameter_mm: obs.diameter_mm.unwrap_or(0.0),
        chipload_diameter_scale: diameter_scale,
        chipload_hardness_scale: hardness_scale,
        chipload_diameter_ratio_raw: diameter_ratio_raw,
        chipload_hardness_ratio_raw: hardness_ratio_raw,
        is_extrapolated,
        row_pass_role: obs.pass_role,
        size_basis,
        hardness_basis,
        family_basis,
        drill_basis,
        material_label: obs.material_label.clone(),
        evidence_grade: obs.evidence_grade,
        row_kind: obs.row_kind,
        band: VendorBand::alone(&obs.observation_id),
    }
}

/// F3.2 rule 8 — deterministic tie-break: on equal composite score the
/// lexicographically smallest `observation_id` wins. Pre-F3.2 ties were
/// broken by `EMBEDDED_FILES` iteration order, so reordering the
/// include list (or loading an external directory) could silently flip
/// which vendor row drives a verdict.
///
/// A3 (G3): each side is `(score, distance, transferred, chip, id)`. On an
/// equal score, a row printed in the queried family beats a row that a
/// family rule transfers (`extrapolation::transfer_rule`).
///
/// Ruling Q3 (feeds contract P5, lead decision D2): after the series rule
/// ([`prefer_series`]) and before the id, the lower printed chip wins. A
/// row with no printed chip does not take part in this rule. The id
/// decides last. P16 item 8 keeps this order: it picks the score winner,
/// and the band rules read the winner's operation and role
/// ([`VendorBand`]). Without it a tie can pick a winner whose band does not
/// hold the lower tied row, and the chip can rise.
fn beats(candidate: TieKey<'_>, best: Option<TieKey<'_>>) -> bool {
    let (candidate_score, candidate_distance, candidate_transferred, candidate_chip, candidate_id) =
        candidate;
    match best {
        None => true,
        Some((best_score, best_distance, best_transferred, best_chip, best_id)) => {
            if candidate_score != best_score {
                return candidate_score > best_score;
            }
            // Extrapolation A3 step 4 (2026-09-24): the diameter term of the
            // score saturates at 2x, so a row at 0.5x and a row at 0.25x of the
            // query score the same. On an equal score the row nearer in
            // diameter (smaller |ln(query / row)|) wins before the id. The
            // test applies only when both rows carry a diameter.
            if let (Some(c), Some(b)) = (candidate_distance, best_distance)
                && (c - b).abs() > 1e-12
            {
                return c < b;
            }
            if candidate_transferred != best_transferred {
                return !candidate_transferred;
            }
            if let (Some(c), Some(b)) = (candidate_chip, best_chip)
                && (c - b).abs() > 1e-12
            {
                return c < b;
            }
            candidate_id < best_id
        }
    }
}

/// The order key of [`beats`]: score, diameter distance, transferred,
/// printed chip (Q3), id.
type TieKey<'a> = (i64, Option<f64>, bool, Option<f64>, &'a str);

/// The printed chip that the Q3 rule compares: the row's chip midpoint,
/// or `None` when the row prints no chip. A band member prints a chip.
fn printed_chip(obs: &VendorObservation) -> Option<f64> {
    let chip = chipload_midpoint(obs);
    (chip.is_finite() && chip > 0.0).then_some(chip)
}

/// The order key of one candidate.
#[expect(
    clippy::indexing_slicing,
    reason = "every candidate index was taken from an iteration over `lut.observations`"
)]
fn tie_key<'a>(lut: &'a VendorLut, query: &LookupQuery, c: Candidate) -> TieKey<'a> {
    let (i, score, _, transferred) = c;
    let obs = &lut.observations[i];
    (
        score,
        diameter_distance(query, obs),
        transferred,
        printed_chip(obs),
        obs.observation_id.as_str(),
    )
}

/// `|ln(query / row)|` for a row with a diameter, else `None`. The tie-break
/// of [`beats`] reads it.
fn diameter_distance(query: &LookupQuery, obs: &VendorObservation) -> Option<f64> {
    match obs.diameter_mm {
        Some(d) if d > 0.0 && query.diameter_mm > 0.0 => Some((query.diameter_mm / d).ln().abs()),
        _ => None,
    }
}

/// Find the best matching observation for a query.
pub fn lookup_best(lut: &VendorLut, query: &LookupQuery) -> Option<LookupResult> {
    lookup_best_where(lut, query, |_| true)
}

fn lookup_best_where(
    lut: &VendorLut,
    query: &LookupQuery,
    extra: impl Fn(&VendorObservation) -> bool,
) -> Option<LookupResult> {
    lookup_best_in(lut, query, extra)
}

/// One candidate of a lookup pass: its index in `lut.observations`, its
/// score, its diameter-proximity score and whether a rule transfers it.
type Candidate = (usize, i64, i64, bool);

/// The best candidate by the score order of [`beats`].
fn best_by_score(
    lut: &VendorLut,
    query: &LookupQuery,
    candidates: impl Iterator<Item = Candidate>,
) -> Option<Candidate> {
    let mut best: Option<Candidate> = None;
    for c in candidates {
        if beats(tie_key(lut, query, c), best.map(|b| tie_key(lut, query, b))) {
            best = Some(c);
        }
    }
    best
}

/// The series rule (operator and lead rulings 2026-10-31,
/// `feeds::series`): the series decides only between rows that tie on
/// material and size. The score picks `best`. When another candidate is
/// printed for the same material family at the same size and ranks better
/// in series (`series::series_rank`: the tool's own series, then the
/// standard rows, then the other specialty rows), the best of the
/// better-ranked rows takes its place. So a generic tool reads the standard
/// row where the ZrN carving row and the Spektra row both print 6 mm
/// softwood, and a specialty row still ships where no row of a better rank
/// prints its material and size. Every other ranking rule stays the score.
#[expect(
    clippy::indexing_slicing,
    reason = "every candidate index was taken from an iteration over `lut.observations`"
)]
fn prefer_series(
    lut: &VendorLut,
    query: &LookupQuery,
    candidates: &[Candidate],
    best: Candidate,
) -> Candidate {
    let key = query.tool_subfamily.as_deref();
    let rank = |i: usize| super::series::series_rank(key, &lut.observations[i]);
    let best_obs = &lut.observations[best.0];
    let ties = |i: usize| {
        let obs = &lut.observations[i];
        obs.material_family == best_obs.material_family
            && match (obs.diameter_mm, best_obs.diameter_mm) {
                (Some(a), Some(b)) => (a - b).abs() < 1e-9,
                (None, None) => true,
                _ => false,
            }
    };
    let best_rank = rank(best.0);
    let Some(top) = candidates
        .iter()
        .filter(|c| ties(c.0) && rank(c.0) < best_rank)
        .map(|c| rank(c.0))
        .min()
    else {
        return best;
    };
    best_by_score(
        lut,
        query,
        candidates
            .iter()
            .copied()
            .filter(|c| ties(c.0) && rank(c.0) == top),
    )
    .unwrap_or(best)
}

fn lookup_best_in(
    lut: &VendorLut,
    query: &LookupQuery,
    extra: impl Fn(&VendorObservation) -> bool,
) -> Option<LookupResult> {
    let mut candidates: Vec<Candidate> = Vec::new();
    for (i, obs) in lut.observations.iter().enumerate() {
        let transferred = is_transferred(query, obs);
        if !passes_must_match(query, obs, transferred) || !extra(obs) {
            continue;
        }
        let (score, diam_score) = score_observation(query, obs, transferred);
        candidates.push((i, score, diam_score, transferred));
    }
    let best = best_by_score(lut, query, candidates.iter().copied())?;
    let chosen = prefer_series(lut, query, &candidates, best);
    Some(with_band(lut, query, &candidates, chosen))
}

/// Two band chips closer than this fraction of the larger chip are equal:
/// the order then keeps the score winner. The value is a floating-point
/// tolerance (a printed chip has at most 5 significant digits, and two
/// scale paths to one printed value differ only by rounding), not a cutting
/// figure.
const CHIP_EQUAL_REL: f64 = 1e-9;

/// True for a grade c row or a fallback row (band rule 7).
fn is_weak_evidence(obs: &VendorObservation) -> bool {
    obs.evidence_grade == EvidenceGrade::C || obs.row_kind == ObservationKind::Fallback
}

/// The band rules 2, 3, 5 and 6 and the printed chip, which read only the
/// row, the query and the winner (no size claim). The first rule that
/// `obs` fails, or `None`.
fn cheap_exclusion(
    query: &LookupQuery,
    winner: (&VendorObservation, bool),
    obs: (&VendorObservation, bool),
) -> Option<BandExclusion> {
    let (winner, winner_transferred) = winner;
    let (obs, transferred) = obs;
    if obs.material_family != query.material_family {
        return Some(BandExclusion::Material);
    }
    if transferred != winner_transferred
        || effective_operation(query, obs, transferred)
            != effective_operation(query, winner, winner_transferred)
    {
        return Some(BandExclusion::Operation);
    }
    if obs.flute_count != query.flute_count {
        return Some(BandExclusion::Flutes);
    }
    match obs.source_class {
        super::vendor_lut::SourceClass::HobbyFeedChart => return Some(BandExclusion::Hobby),
        super::vendor_lut::SourceClass::Vendor => {}
    }
    printed_chip(obs).is_none().then_some(BandExclusion::NoChip)
}

/// Band rule 3 (with the transfer status, see `cheap_exclusion`): the
/// operation family and pass role of a row after the routes. A row that a stated rule transfers (`is_transferred`: a G3
/// family rule or the G6 drill rule) serves as its copy in the queried
/// family and role (copy semantics, A3); every other row keeps its own.
fn effective_operation(
    query: &LookupQuery,
    obs: &VendorObservation,
    transferred: bool,
) -> (LutOperationFamily, LutPassRole) {
    if transferred {
        (query.operation_family, query.pass_role)
    } else {
        (obs.operation_family, obs.pass_role)
    }
}

/// Band rule 4: the size basis of a built row. `Exact`, or a form A claim.
fn size_admits(basis: &SizeBasis) -> bool {
    match basis {
        SizeBasis::Exact => true,
        SizeBasis::Claim(claim) => matches!(
            claim.form,
            super::extrapolation::SizeForm::Interpolated { .. }
        ),
        SizeBasis::NoDiameterAnchor
        | SizeBasis::NoChipload
        | SizeBasis::AngleKey { .. }
        | SizeBasis::Refused { .. } => false,
    }
}

/// The key that makes one form A member per chart series: the series
/// source and the two bracketing rows. `None` for an `Exact` member.
fn series_key(basis: &SizeBasis) -> Option<(String, Vec<String>)> {
    basis.claim().map(|claim| {
        let mut rows = claim.source_rows.clone();
        rows.sort();
        (claim.series_source_id.clone(), rows)
    })
}

/// Feeds contract P16 item 8: build the [`VendorBand`] of `chosen` (the
/// score winner after the series rule) over `candidates`, and return the
/// shipped row with the band set. The rules are in the [`VendorBand`] doc.
#[expect(
    clippy::indexing_slicing,
    reason = "every candidate index was taken from an iteration over `lut.observations`"
)]
fn with_band(
    lut: &VendorLut,
    query: &LookupQuery,
    candidates: &[Candidate],
    chosen: Candidate,
) -> LookupResult {
    let winner_obs = &lut.observations[chosen.0];
    let winner = build_result(lut, winner_obs, query, chosen.1, chosen.2);
    // Rules 2-6: the members before the grade rule, each with its built
    // result. The winner comes first, then the other rows by id, so a tie
    // of equal rows keeps the winner.
    let mut order: Vec<Candidate> = candidates.to_vec();
    order.sort_by(|a, b| {
        (a.0 != chosen.0).cmp(&(b.0 != chosen.0)).then_with(|| {
            lut.observations[a.0]
                .observation_id
                .cmp(&lut.observations[b.0].observation_id)
        })
    });
    let key = query.tool_subfamily.as_deref();
    let winner_rank = super::series::series_rank(key, winner_obs);
    let mut admitted: Vec<(Candidate, LookupResult)> = Vec::new();
    for c in order {
        let obs = &lut.observations[c.0];
        if super::series::series_rank(key, obs) != winner_rank {
            continue;
        }
        if cheap_exclusion(query, (winner_obs, chosen.3), (obs, c.3)).is_some() {
            continue;
        }
        let result = if c.0 == chosen.0 {
            winner.clone()
        } else {
            build_result(lut, obs, query, c.1, c.2)
        };
        if size_admits(&result.size_basis) && result.chip_load_mm > 0.0 {
            admitted.push((c, result));
        }
    }
    // Rule 1b (P18, review D16, option (a)): the query's tool family
    // first. When a row of that family passes rule 4, rows of another
    // family leave the band.
    let family_first = admitted
        .iter()
        .any(|(c, _)| lut.observations[c.0].tool_family == query.tool_family);
    if family_first {
        admitted.retain(|(c, _)| lut.observations[c.0].tool_family == query.tool_family);
    }
    // Rule 4, one member per chart series: every `Exact` row is a member. A
    // form A row joins only when no member already stands for its series:
    // no `Exact` member is one of its bracketing rows, and no earlier form
    // A member has the same series and bracketing rows. (At a printed size
    // the form A claim of a neighbour row gives the printed row's value.)
    let exact_ids: Vec<String> = admitted
        .iter()
        .filter(|(_, r)| r.size_basis.claim().is_none())
        .map(|(_, r)| r.observation_id.clone())
        .collect();
    // A form A row that is one of its own bracketing rows stands for the
    // series first, so the member names a printed size next to the query.
    // The sort is stable: the winner and the id order stay inside each part.
    let brackets_itself = |r: &LookupResult| {
        r.size_basis
            .claim()
            .is_some_and(|claim| claim.source_rows.contains(&r.observation_id))
    };
    admitted.sort_by_key(|(_, r)| !brackets_itself(r));
    let mut seen_series: Vec<(String, Vec<String>)> = Vec::new();
    let mut built: Vec<(Candidate, LookupResult)> = Vec::new();
    for (c, result) in admitted {
        if let Some(claim) = result.size_basis.claim() {
            if claim.source_rows.iter().any(|row| exact_ids.contains(row)) {
                continue;
            }
            if let Some(key) = series_key(&result.size_basis) {
                if seen_series.contains(&key) {
                    continue;
                }
                seen_series.push(key);
            }
        }
        built.push((c, result));
    }
    // Rule 7: a grade c or fallback row joins only when no grade a or b
    // member exists.
    let has_strong = built
        .iter()
        .any(|(c, _)| !is_weak_evidence(&lut.observations[c.0]));
    if has_strong {
        built.retain(|(c, _)| !is_weak_evidence(&lut.observations[c.0]));
    }
    // Ascending chip middle; on an equal chip (within `CHIP_EQUAL_REL`) the
    // winner first, then the id.
    built.sort_by(|(ca, a), (cb, b)| {
        let equal = (a.chip_load_mm - b.chip_load_mm).abs()
            <= CHIP_EQUAL_REL * a.chip_load_mm.max(b.chip_load_mm);
        let by_chip = if equal {
            std::cmp::Ordering::Equal
        } else {
            a.chip_load_mm.total_cmp(&b.chip_load_mm)
        };
        by_chip
            .then_with(|| (ca.0 != chosen.0).cmp(&(cb.0 != chosen.0)))
            .then_with(|| a.observation_id.cmp(&b.observation_id))
    });
    let winner_outside = if built.iter().any(|(c, _)| c.0 == chosen.0) {
        None
    } else {
        Some(winner_exclusion(
            query,
            (winner_obs, chosen.3),
            &winner,
            has_strong,
            family_first,
        ))
    };
    let members: Vec<BandMember> = built
        .iter()
        .map(|(_, r)| BandMember {
            row: crate::feeds::sourced::RowRef(r.observation_id.clone()),
            vendor: r.source_vendor,
            chip_mid_mm: r.chip_load_mm,
            chip_min_mm: r.chip_load_min_mm,
            chip_max_mm: r.chip_load_max_mm,
            size_basis: r.size_basis.clone(),
        })
        .collect();
    // The no-rise rule: the winner's chip, when it has one to compare.
    let winner_chip = (!winner.size_basis.is_refused() && winner.chip_load_mm > 0.0)
        .then_some(winner.chip_load_mm);
    // When the winner is a member, the lowest member is at or below it (the
    // sort puts the winner first on an equal chip). When the winner is not a
    // member, a member ships only when its chip is strictly lower: an equal
    // chip moves no number, so the winner keeps its row (its RPM and depth
    // fields).
    let winner_is_member = winner_outside.is_none();
    let (rule, shipped): (BandRule, LookupResult) = match (built.first(), winner_chip) {
        (None, _) => (BandRule::NoMember, winner),
        (Some((_, lowest)), Some(chip))
            if winner_is_member || lowest.chip_load_mm < chip * (1.0 - CHIP_EQUAL_REL) =>
        {
            (BandRule::Lowest, lowest.clone())
        }
        (Some(_), Some(_) | None) => (BandRule::WinnerShips, winner),
    };
    let shipped_family = lut
        .observations
        .iter()
        .find(|o| o.observation_id == shipped.observation_id)
        .map_or(winner_obs.tool_family, |o| o.tool_family);
    let cross_family = (shipped_family != query.tool_family).then_some(CrossFamily {
        shipped: shipped_family,
        tool: query.tool_family,
    });
    LookupResult {
        band: VendorBand {
            members,
            score_winner: crate::feeds::sourced::RowRef(winner_obs.observation_id.clone()),
            score_winner_chip_mm: winner_chip,
            rule,
            winner_outside,
            cross_family,
        },
        ..shipped
    }
}

/// Why the score winner is not a band member: the first rule it fails.
fn winner_exclusion(
    query: &LookupQuery,
    winner_obs: (&VendorObservation, bool),
    winner: &LookupResult,
    has_strong: bool,
    family_first: bool,
) -> BandExclusion {
    if let Some(e) = cheap_exclusion(query, winner_obs, winner_obs) {
        return e;
    }
    // Rule 1b (P18): a winner of another tool family while the query
    // family has a member.
    if family_first && winner_obs.0.tool_family != query.tool_family {
        return BandExclusion::ToolFamily;
    }
    if !size_admits(&winner.size_basis) || winner.chip_load_mm <= 0.0 {
        return if winner.size_basis.is_refused() || winner.chip_load_mm <= 0.0 {
            BandExclusion::NoChip
        } else {
            BandExclusion::Size
        };
    }
    if has_strong && is_weak_evidence(winner_obs.0) {
        return BandExclusion::Grade;
    }
    // A form A winner whose series already gave a member.
    BandExclusion::Size
}

/// Group `MaterialFamily` into the categories a row may serve across.
/// Within a category we allow score-only matching, with chipload bounds
/// scaled by the hardness ratio. The composite categories 5 and 6 get no
/// hardness scale (`extrapolation::hardness_basis`). Across categories the lookup
/// hard-rejects:
/// cutting wood and cutting aluminum live in entirely different chipload
/// regimes and linear extrapolation between them isn't meaningful.
///
/// **Operator ruling 2026-09-24 (the wood charts), as refined the same day.**
/// Every vendor prints softwood, hardwood, plywood and MDF/HDF/particleboard
/// as separate tables. The probe that raised it: a Ø1 flat pocket in
/// HARDWOOD matched `amana-flat-mdf-pocket-0794-2f-spektra`, an MDF row.
///
/// - Softwood and hardwood are ONE solid-wood category. The 2026-05
///   hardness ruling models exactly their difference through the Janka
///   scale, and the card prints the row id and the "x hardness" factor, so
///   that substitution is visible.
/// - Plywood (both grades) is its own category, and MDF, HDF and
///   particleboard are one more. They are glue boards with no Janka mapping,
///   so no hardness scale can carry a solid-wood row onto them or back.
///   A cross-category row is a non-match; the cell falls to the formula and
///   the R1 judgement.
pub(crate) fn material_category(family: MaterialFamily) -> u8 {
    match family {
        MaterialFamily::Softwood | MaterialFamily::Hardwood => 0,
        MaterialFamily::PlywoodSoftwood | MaterialFamily::PlywoodHardwood => 5,
        MaterialFamily::Mdf | MaterialFamily::Hdf | MaterialFamily::Particleboard => 6,
        MaterialFamily::Acrylic
        | MaterialFamily::Hdpe
        | MaterialFamily::Polycarbonate
        | MaterialFamily::Delrin => 1,
        MaterialFamily::Aluminum => 2,
        // Fiberglass / fiber-reinforced composites — separate category.
        // Abrasive + fiber-reinforced cutting behaviour doesn't map onto
        // polymers or metals; never extrapolate across.
        MaterialFamily::Fiberglass => 3,
    }
}

fn materials_compatible(query: MaterialFamily, obs: MaterialFamily) -> bool {
    material_category(query) == material_category(obs)
}

/// True when a stated rule carries `obs` from its home operation family
/// into the queried family: a G3 family rule (`extrapolation::transfer_rule`,
/// A3) or the G6 drill rule (`extrapolation::drill_rule`, ruling B5). Both
/// resolvers call this one predicate, so Suggest and the gate read one row.
fn is_transferred(query: &LookupQuery, obs: &VendorObservation) -> bool {
    transfer_rule(query, obs).is_some() || drill_rule(query, obs).is_some()
}

/// `transferred` is [`is_transferred`]: a row from another operation family
/// passes only when a G3 family rule (A3) or the G6 drill rule (B5) carries
/// it into the queried family. Every other filter applies to a transferred
/// row too.
fn passes_must_match(query: &LookupQuery, obs: &VendorObservation, transferred: bool) -> bool {
    if obs.operation_family != query.operation_family && !transferred {
        return false;
    }
    // Material category is a hard filter; exact family is a score
    // contributor in `score_observation`. Since the 2026-09-24 ruling the
    // solid woods, the plywoods and the MDF-class boards are separate
    // categories (see `material_category`), so a hardwood query never
    // borrows an MDF or plywood row. Within a
    // category, chipload bounds are scaled by the hardness ratio, except in
    // the composite categories 5 and 6. See
    // `LookupResult::chipload_hardness_scale`.
    if !materials_compatible(query.material_family, obs.material_family) {
        return false;
    }
    if !tool_family_compatible(query.tool_family, obs.tool_family) {
        return false;
    }
    // Diameter ratio gate intentionally relaxed to a sanity floor only.
    // The query diameter is the lookup key (`lut_key_diameter_mm`; the tip
    // of a tapered ball since ruling A1). Chipload bounds get scaled to
    // the query's diameter in `build_result`
    // and the verdict is downgraded to `Approximate` past ±40 % divergence.
    // Without the relax a 1 mm tapered-ball tip can never reach a 3.175 mm
    // calibrated row even though the LUT trends are well-behaved.
    if query.diameter_mm <= 0.0 {
        return false;
    }
    // V-bit rows and diameter-window articles intentionally carry
    // `diameter_mm = None` — they're matched by angle / material /
    // operation rather than by diameter. Let them through the gate;
    // scoring + build_result handle the no-anchor case (no diameter
    // contribution to score, scale factor pinned to 1.0).
    if let Some(obs_d) = obs.diameter_mm {
        if obs_d <= 0.0 {
            return false;
        }
        let ratio = query.diameter_mm / obs_d;
        if !(SCALE_CLAMP_LO..=SCALE_CLAMP_HI).contains(&ratio) {
            return false;
        }
    }
    true
}

/// `transferred` marks a row that a G3 family rule carries (see
/// [`passes_must_match`]). It changes only the role term.
fn score_observation(
    query: &LookupQuery,
    obs: &VendorObservation,
    transferred: bool,
) -> (i64, i64) {
    let mut score: i64 = 1000;
    score += tool_family_score(query.tool_family, obs.tool_family);
    score += obs.row_kind.score();
    score += obs.evidence_grade.score();

    let flute_diff = (query.flute_count as i64 - obs.flute_count as i64).unsigned_abs();
    score += match flute_diff {
        0 => 80,
        1 => 30,
        _ => -20,
    };

    // Diameter scoring is skipped for rows without a diameter anchor
    // (v-bit, diameter-window articles). They still compete on the
    // other axes; v-bit angle bonus in find_best_vbit_row provides
    // the per-row specificity.
    let diam_score = match obs.diameter_mm {
        Some(obs_d) if obs_d > 0.0 => {
            let log_ratio = (query.diameter_mm / obs_d).ln().abs();
            ((1.0 - log_ratio / 2.0_f64.ln()) * 200.0).clamp(0.0, 200.0) as i64
        }
        _ => 0,
    };
    score += diam_score;

    if let (Some(qk), Some(qv), Some(ok), Some(ov)) = (
        query.hardness_kind,
        query.hardness_value,
        obs.hardness_kind,
        obs.hardness_value,
    ) && qk == ok
        && ov > 0.0
    {
        let rel_diff = ((qv - ov) / ov).abs();
        let h_score = ((1.0 - rel_diff) * 80.0).clamp(0.0, 80.0) as i64;
        score += h_score;
    }

    // The tool's series key (`feeds::series`): a row of that series, or a
    // row whose subfamily is the key.
    if let Some(qs) = &query.tool_subfamily
        && super::series::row_matches_key(qs, obs)
    {
        score += 50;
    }

    // A3 (G3), copy semantics: a transferred row scores as its copy in the
    // queried family would. The home row's role is a filing label; the
    // vendor printed no role.
    if transferred || query.pass_role == obs.pass_role {
        score += 45;
    } else {
        score -= 25;
    }

    // Material-family exact match is a strong score contributor (was a
    // hard filter pre-relaxation). Within a category, exact-family rows
    // should still win over neighbouring-family rows so a hardwood query
    // prefers a hardwood row over a softwood row when both exist.
    if query.material_family == obs.material_family {
        score += 100;
    }

    (score, diam_score)
}

/// Check if a query tool family is compatible with an observation tool family.
fn tool_family_compatible(query: ToolFamily, obs: ToolFamily) -> bool {
    if query == obs {
        return true;
    }
    // Fallback compatibility
    matches!(
        (query, obs),
        (ToolFamily::BullNose, ToolFamily::FlatEnd)
            | (ToolFamily::TaperedBallNose, ToolFamily::BallNose)
            | (ToolFamily::FlatEnd, ToolFamily::BullNose)
    )
}

/// Score for tool family match quality.
fn tool_family_score(query: ToolFamily, obs: ToolFamily) -> i64 {
    if query == obs {
        return 220;
    }
    match (query, obs) {
        (ToolFamily::BullNose, ToolFamily::FlatEnd) => 120,
        (ToolFamily::TaperedBallNose, ToolFamily::BallNose) => 110,
        (ToolFamily::FlatEnd, ToolFamily::BullNose) => 100,
        _ => 0, // shouldn't reach here due to compatible filter
    }
}

/// Extract chipload as midpoint of min/max. The G1 size claim reads the
/// same mid (`feeds::extrapolation::size`).
pub(crate) fn chipload_midpoint(obs: &VendorObservation) -> f64 {
    match (obs.chipload_min_mm_tooth, obs.chipload_max_mm_tooth) {
        (Some(min), Some(max)) => (min + max) / 2.0,
        (Some(v), None) | (None, Some(v)) => v,
        (None, None) => {
            // Infer from feed/rpm/flutes if available
            if let (Some(rpm), Some(_)) = (obs.rpm_nominal, Some(obs.flute_count)) {
                if rpm > 0.0 && obs.flute_count > 0 {
                    // No feed data in schema, return 0
                    0.0
                } else {
                    0.0
                }
            } else {
                0.0
            }
        }
    }
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    clippy::str_to_string,
    reason = "test code: a panic is how a test reports a failure; test code accepts the style lint str_to_string"
)]
mod tests {
    use super::*;

    fn embedded_lut() -> VendorLut {
        VendorLut::embedded()
    }

    #[test]
    fn find_best_row_matches_legacy_lookup_best() {
        let lut = embedded_lut();
        let query = LookupQuery {
            tool_family: ToolFamily::FlatEnd,
            tool_subfamily: Some("upcut".to_string()),
            diameter_mm: 6.0,
            flute_count: 2,
            material_family: MaterialFamily::Softwood,
            hardness_kind: Some(HardnessKind::Janka),
            hardness_value: Some(600.0),
            operation_family: LutOperationFamily::Adaptive,
            pass_role: LutPassRole::Roughing,
        };
        let via_canonical = find_best_row(&lut, &query).expect("canonical row");
        let via_legacy = lookup_best(&lut, &query).expect("legacy row");
        assert_eq!(via_canonical.observation_id, via_legacy.observation_id);
    }

    #[test]
    fn test_exact_match_6mm_2f_flat_softwood_adaptive() {
        let lut = embedded_lut();
        let query = LookupQuery {
            tool_family: ToolFamily::FlatEnd,
            // The recipe path builds its query with no subfamily
            // (`vendor_normalize::to_lookup_query_unrouted`). The old
            // "upcut" tag matched the derived/c row's own tag and let it
            // outscore the printed row by the subfamily bonus; the test
            // now asks what production asks.
            tool_subfamily: None,
            diameter_mm: 6.0,
            flute_count: 2,
            material_family: MaterialFamily::Softwood,
            hardness_kind: Some(HardnessKind::Janka),
            hardness_value: Some(600.0),
            operation_family: LutOperationFamily::Adaptive,
            pass_role: LutPassRole::Roughing,
        };
        let result = lookup_best(&lut, &query).expect("should find match");
        assert!(
            result.score > 1400,
            "score {} should be > 1400",
            result.score
        );
        // Feeds matrix R5 (2026-09-23): the printed Spektra v24 row wins.
        // Chart line: 2 Flute 6mm (48118-K/48218-K), Wood/Plywood
        // 0.0050 in = 0.127 mm/tooth. The row publishes that one value as
        // its maximum, so the midpoint is 0.127. The old winner
        // amana-flat-softwood-adaptive-6000-2f (0.065-0.11) is not
        // printed on the chart and is now derived/c.
        assert_eq!(
            result.observation_id,
            "amana-flat-softwood-adaptive-6000-2f-spektra"
        );
        assert!(
            (result.chip_load_mm - 0.127).abs() < 0.001,
            "chipload {} should be ~0.127",
            result.chip_load_mm
        );
    }

    /// Feeds matrix R5 item 8 (EVIDENCE 5.2-10). An MDF query for a Ø6
    /// 2-flute end mill in the Adaptive family borrowed the HARDWOOD row,
    /// because no MDF row existed in that family and the hardness term
    /// scores MDF (Janka proxy 1100) nearer hardwood (1450) than softwood
    /// (600). The cause was a missing row, not the scorer: with the
    /// printed Spektra MDF/Laminate row in the family, the MDF row wins
    /// and carries the printed 0.0060 in = 0.1524 mm/tooth unscaled.
    #[test]
    fn mdf_adaptive_query_matches_the_printed_mdf_row() {
        let lut = embedded_lut();
        let query = LookupQuery {
            tool_family: ToolFamily::FlatEnd,
            tool_subfamily: None,
            diameter_mm: 6.0,
            flute_count: 2,
            material_family: MaterialFamily::Mdf,
            hardness_kind: Some(HardnessKind::Janka),
            hardness_value: Some(1100.0),
            operation_family: LutOperationFamily::Adaptive,
            pass_role: LutPassRole::Roughing,
        };
        let result = lookup_best(&lut, &query).expect("MDF adaptive row");
        assert_eq!(
            result.observation_id,
            "amana-flat-mdf-adaptive-6000-2f-spektra"
        );
        assert!(
            (result.chip_load_max_mm.expect("printed value") - 0.1524).abs() < 1e-9,
            "the printed MDF value must carry through unscaled, got {:?}",
            result.chip_load_max_mm
        );
    }

    #[test]
    fn test_near_match_5mm_still_finds_6mm() {
        let lut = embedded_lut();
        let query = LookupQuery {
            tool_family: ToolFamily::FlatEnd,
            tool_subfamily: None,
            diameter_mm: 5.0,
            flute_count: 2,
            material_family: MaterialFamily::Softwood,
            hardness_kind: Some(HardnessKind::Janka),
            hardness_value: Some(600.0),
            operation_family: LutOperationFamily::Adaptive,
            pass_role: LutPassRole::Roughing,
        };
        let result = lookup_best(&lut, &query).expect("should find match");
        // Should match 6mm row with diameter penalty
        assert!(result.chip_load_mm > 0.03, "chipload should be reasonable");
    }

    #[test]
    fn test_ball_nose_lookup() {
        let lut = embedded_lut();
        let query = LookupQuery {
            tool_family: ToolFamily::BallNose,
            tool_subfamily: None,
            diameter_mm: 6.0,
            flute_count: 2,
            material_family: MaterialFamily::Softwood,
            hardness_kind: Some(HardnessKind::Janka),
            hardness_value: Some(600.0),
            operation_family: LutOperationFamily::Parallel,
            pass_role: LutPassRole::Finish,
        };
        let result = lookup_best(&lut, &query).expect("should find ball nose match");
        assert!(result.chip_load_mm > 0.02 && result.chip_load_mm < 0.06);
        assert!(result.observation_id.contains("ball"));
    }

    #[test]
    fn test_no_match_returns_none_when_outside_sanity_floor() {
        // Diameter ratio is gated by a generous sanity floor only
        // (`SCALE_CLAMP_LO..=SCALE_CLAMP_HI`, currently 0.1..=10.0). Past
        // that bound the lookup hard-rejects.
        // 2026-05-30: bumped from 100 mm to 200 mm — the Phase 1C
        // Whiteside RD5218H 12.7 mm row brought the largest hardwood
        // flat-end adaptive-roughing row into the wood category, so the
        // old 100 mm probe (100/12.7 = 7.9×) sat inside the sanity
        // floor. 200 mm restores the >10× margin (200/12.7 = 15.7×).
        let lut = embedded_lut();
        let query = LookupQuery {
            tool_family: ToolFamily::FlatEnd,
            tool_subfamily: None,
            diameter_mm: 200.0,
            flute_count: 2,
            material_family: MaterialFamily::Softwood,
            hardness_kind: None,
            hardness_value: None,
            operation_family: LutOperationFamily::Adaptive,
            pass_role: LutPassRole::Roughing,
        };
        assert!(lookup_best(&lut, &query).is_none());
    }

    #[test]
    fn test_no_match_when_material_category_differs() {
        // Material category is still a hard filter — a wood query must
        // not extrapolate from an aluminum row even though the diameter
        // and op family align.
        let lut = embedded_lut();
        let query = LookupQuery {
            tool_family: ToolFamily::BullNose,
            tool_subfamily: None,
            diameter_mm: 6.0,
            flute_count: 3,
            material_family: MaterialFamily::Aluminum,
            hardness_kind: Some(HardnessKind::Hb),
            hardness_value: Some(95.0),
            operation_family: LutOperationFamily::Parallel,
            pass_role: LutPassRole::Finish,
        };
        // No aluminum + parallel/finish + bull-nose row exists; the
        // wood-category bull-nose rows must not satisfy this query.
        assert!(lookup_best(&lut, &query).is_none());
    }

    #[test]
    fn test_bull_nose_falls_back_to_flat_end() {
        let lut = embedded_lut();
        let query = LookupQuery {
            tool_family: ToolFamily::BullNose,
            tool_subfamily: None,
            diameter_mm: 6.0,
            flute_count: 2,
            material_family: MaterialFamily::Softwood,
            hardness_kind: Some(HardnessKind::Janka),
            hardness_value: Some(600.0),
            operation_family: LutOperationFamily::Adaptive,
            pass_role: LutPassRole::Roughing,
        };
        // Since A3 step 4 (G3) the Amana corner-radius pocket row serves this
        // query through the family rule, ahead of the flat-end fallback.
        let result =
            lookup_best(&lut, &query).expect("bull nose should fallback to flat/bull rows");
        assert!(result.chip_load_mm > 0.03);
    }

    #[test]
    fn test_sub_1mm_tapered_ball_softwood_finish_matches() {
        // Item E coverage gain: a 1mm tapered ball + parallel + finish + softwood
        // must match at least one row after sub-1mm coverage was added (2026-05-02).
        // Source: Amana ZrN 3D Profiling chart sub-1mm rows.
        //
        // Extrapolation P1 step 1 (2026-09-24) files the Amana ZrN v8 cells
        // as `tapered_ball_nose` rows (amana_zrn_tapered_v8.json). The chart
        // keys these cells on the tip, and 46256 in this cell is a 5.4 deg
        // tapered ball. The v8 softwood row is derived/b (ruling A4: the
        // shared "Wood" column). It scores 1825. The kept ball_nose row
        // (for the straight ball 46471) is exact/a but reaches a tapered
        // query only on the family fallback, so it scores 1795. The SpeTool
        // 1.0 mm row also scores 1825, and the smaller id wins the tie.
        let lut = embedded_lut();
        let query = LookupQuery {
            tool_family: ToolFamily::TaperedBallNose,
            tool_subfamily: None,
            diameter_mm: 1.0,
            flute_count: 2,
            material_family: MaterialFamily::Softwood,
            hardness_kind: Some(HardnessKind::Janka),
            hardness_value: Some(600.0),
            operation_family: LutOperationFamily::Parallel,
            pass_role: LutPassRole::Finish,
        };
        let result = lookup_best(&lut, &query).expect(
            "1mm tapered ball + softwood + parallel/finish should match a sub-1mm tapered-ball row",
        );
        // Ruling Q3 (feeds contract P5): the ZrN v8 row and the SpeTool
        // 1.0 mm row tie at 1825. Until P5 the smaller id won (the ZrN
        // row). Now the lower printed chip wins. P16 item 8: the tie is
        // part of the vendor band (both rows print 1.0 mm, 2 flutes,
        // softwood, parallel finish), lowest chip first, and the lowest
        // ships. The straight ball ZrN row (`amana-ball-softwood-parallel-
        // 1000-2f-zrn`, 1795 on the family fallback) prints the same cell,
        // but its series rank (`series::series_rank`) is not the winner's,
        // so band rule 8 keeps it out.
        let members: Vec<&str> = result.band.members.iter().map(|m| m.row.id()).collect();
        assert_eq!(
            members,
            vec![
                "spetool-tapered-softwood-parallel-1000-2f",
                "amana-tapered-softwood-parallel-1000-2f-zrn-v8"
            ],
            "the band names both rows, the lowest first"
        );
        assert_eq!(result.band.rule, BandRule::Lowest);
        // The ZrN row prints midpoint(0.01905, 0.0508); the winner's chip is
        // lower.
        let zrn_mid = (0.01905 + 0.0508) / 2.0;
        assert!(
            result.chip_load_mm < zrn_mid,
            "the winner's chip {} is not below the ZrN chip {zrn_mid}",
            result.chip_load_mm
        );
    }

    #[test]
    fn diameter_scaling_linearly_scales_chipload_bounds() {
        // Feeds matrix R5 (2026-09-23): the printed Onsrud 77-100 rows
        // (exact/a) now win tapered-ball hardwood parallel/finish; the
        // Amana 6.0 mm row this test used to name is derived/c. The
        // exact-diameter row is the 1/4 in (6.35 mm) Onsrud line
        // `77-100 1xD .005-.007 (1/4)`, so the query asks for 6.35 mm and
        // the diameter scale must be 1.0. Since A3 step 3 the LUT files
        // that line once, under pocket/roughing, and the G3 family rule
        // serves the parallel query from it.
        let lut = embedded_lut();
        let query = LookupQuery {
            tool_family: ToolFamily::TaperedBallNose,
            tool_subfamily: None,
            diameter_mm: 6.35,
            flute_count: 2,
            material_family: MaterialFamily::Hardwood,
            hardness_kind: Some(HardnessKind::Janka),
            hardness_value: Some(1450.0),
            operation_family: LutOperationFamily::Parallel,
            pass_role: LutPassRole::Finish,
        };
        let result = lookup_best(&lut, &query).expect("must match a tapered-ball parallel row");
        // The 6.35 mm hardwood row is an exact-diameter match.
        assert_eq!(result.observation_id, "onsrud-hardwood-77-100-1_4-pocket");
        assert_eq!(result.row_pass_role, LutPassRole::Roughing);
        assert_eq!(result.family_basis.name(), "Transferred");
        assert!(
            (result.row_diameter_mm - 6.35).abs() < 1e-9,
            "expected exact 6.35 mm row, got {} mm",
            result.row_diameter_mm
        );
        assert!((result.chipload_diameter_scale - 1.0).abs() < 1e-6);
        assert!((result.chipload_hardness_scale - 1.0).abs() < 1e-6);
        assert!(!result.is_extrapolated);
    }

    #[test]
    fn hardness_scaling_uses_obs_over_query() {
        // A softwood query should match the softwood parallel/finish row
        // (no extrapolation needed). Hardness scale = obs/query = 600/600
        // = 1.0 because the row's hardness equals the query's.
        let lut = embedded_lut();
        let query = LookupQuery {
            tool_family: ToolFamily::TaperedBallNose,
            tool_subfamily: None,
            diameter_mm: 6.0,
            flute_count: 2,
            material_family: MaterialFamily::Softwood,
            hardness_kind: Some(HardnessKind::Janka),
            hardness_value: Some(600.0),
            operation_family: LutOperationFamily::Parallel,
            pass_role: LutPassRole::Finish,
        };
        let result = lookup_best(&lut, &query).expect("must match softwood row");
        assert!((result.chipload_hardness_scale - 1.0).abs() < 1e-6);
    }

    /// Phase 1C smoke — an aluminum query against the embedded LUT
    /// must now reach a row. Before the 2026-05-30 ingest the LUT
    /// carried zero `MaterialFamily::Aluminum` rows; after Phase 1C the
    /// Helical 6061 + Amana ZrN aluminum + Garr (deferred) sources
    /// land an aluminum-only category at every roughing-class flat-end
    /// query in the 3–13 mm diameter window.
    #[test]
    fn phase_1c_aluminum_query_reaches_a_row() {
        let lut = embedded_lut();
        let query = LookupQuery {
            tool_family: ToolFamily::FlatEnd,
            tool_subfamily: None,
            diameter_mm: 6.35,
            flute_count: 3,
            material_family: MaterialFamily::Aluminum,
            hardness_kind: Some(HardnessKind::Hb),
            hardness_value: Some(95.0),
            operation_family: LutOperationFamily::Adaptive,
            pass_role: LutPassRole::Roughing,
        };
        let result = lookup_best(&lut, &query)
            .expect("Phase 1C wires aluminum rows live; an adaptive/roughing flat-end aluminum query must match");
        assert!(result.chip_load_mm > 0.0);
    }

    #[test]
    fn material_category_hard_filter_blocks_wood_to_metal() {
        // Wood query must not extrapolate from aluminum row even when
        // op family / diameter / pass-role would otherwise permit it.
        let lut = embedded_lut();
        let query = LookupQuery {
            tool_family: ToolFamily::BullNose,
            tool_subfamily: None,
            diameter_mm: 6.0,
            flute_count: 3,
            material_family: MaterialFamily::Hardwood,
            hardness_kind: Some(HardnessKind::Janka),
            hardness_value: Some(1450.0),
            operation_family: LutOperationFamily::Adaptive,
            pass_role: LutPassRole::Roughing,
        };
        let result = lookup_best(&lut, &query)
            .expect("a hardwood bull-nose row serves the adaptive query (G3 transfer)");
        // The aluminum row in `amana_3d_profiling.json` must not be
        // selected — its observation_id contains "aluminum".
        assert!(
            !result.observation_id.contains("aluminum"),
            "expected wood-category row, got {}",
            result.observation_id
        );
    }

    #[test]
    fn test_sub_1mm_tapered_ball_hardwood_finish_matches_the_printed_tip_row() {
        // History. Before 2026-05-08 a [0.5, 2.0] ratio gate refused this
        // query. From 2026-05-08 the lookup scaled a >= 1 mm hardwood row
        // down to 0.5 mm and set `is_extrapolated`.
        //
        // Extrapolation P1 step 1 (2026-09-24) loads the SpeTool 2D/3D
        // tapered chart. It prints a 0.5 mm tip row, and ruling A4 serves
        // hardwood from its shared "Wood" row (derived, grade b). A 0.5 mm
        // query now matches that row at a ratio of 1.0, so the lookup does
        // not scale and does not extrapolate.
        let lut = embedded_lut();
        let query = LookupQuery {
            tool_family: ToolFamily::TaperedBallNose,
            tool_subfamily: None,
            diameter_mm: 0.5,
            flute_count: 2,
            material_family: MaterialFamily::Hardwood,
            hardness_kind: Some(HardnessKind::Janka),
            hardness_value: Some(1450.0),
            operation_family: LutOperationFamily::Parallel,
            pass_role: LutPassRole::Finish,
        };
        let result = lookup_best(&lut, &query)
            .expect("0.5mm hardwood tapered ball should match the SpeTool 0.5 mm tip row");
        assert_eq!(
            result.observation_id,
            "spetool-tapered-hardwood-parallel-0500-2f"
        );
        assert!(
            (result.row_diameter_mm - 0.5).abs() < 1e-9,
            "expected the exact 0.5 mm row, got {} mm",
            result.row_diameter_mm
        );
        assert!(!result.is_extrapolated);
        assert!((result.chipload_diameter_ratio_raw - 1.0).abs() < 1e-12);
        assert!((result.chipload_diameter_scale - 1.0).abs() < 1e-12);
        // Same hardness anchor (natural-wood Janka 1450), so hardness scale is unity.
        assert!((result.chipload_hardness_scale - 1.0).abs() < 1e-6);
        // The chart prints one value (0.0007 in), so the row has no minimum.
        assert!(result.chip_load_min_mm.is_none());
        assert!((result.chip_load_mm - 0.01778).abs() < 1e-9);
        assert!(
            result
                .chip_load_max_mm
                .is_some_and(|max| (max - 0.01778).abs() < 1e-9)
        );
    }

    /// The raw ratio and the applied scale are two fields (RE-PINNED
    /// 2026-08-06). While the diameter law was linear, one field could stand
    /// for both. This test keeps that guard on a query that still
    /// extrapolates: a 0.3 mm tip against the SpeTool 0.5 mm row (ratio
    /// 0.6). This is a raw lookup test. The lookup itself does not refuse a
    /// tip under 0.5 mm; only the Suggest support arm does
    /// (`support::TAPERED_MIN_TIP_MM`, ruling B1, extrapolation P1 step 2).
    /// Step 3 puts the size claim into `build_result` and must re-check
    /// this test.
    #[test]
    fn test_tapered_ball_below_the_smallest_tip_row_scales_on_the_raw_ratio() {
        let lut = embedded_lut();
        let query = LookupQuery {
            tool_family: ToolFamily::TaperedBallNose,
            tool_subfamily: None,
            diameter_mm: 0.3,
            flute_count: 2,
            material_family: MaterialFamily::Hardwood,
            hardness_kind: Some(HardnessKind::Janka),
            hardness_value: Some(1450.0),
            operation_family: LutOperationFamily::Parallel,
            pass_role: LutPassRole::Finish,
        };
        let result = lookup_best(&lut, &query).expect("0.3mm hardwood tapered ball row");
        assert_eq!(
            result.observation_id,
            "spetool-tapered-hardwood-parallel-0500-2f"
        );
        assert!(
            result.is_extrapolated,
            "0.3 mm against the 0.5 mm row must trip extrapolation"
        );
        let expected_raw = 0.3 / result.row_diameter_mm;
        assert!(
            (result.chipload_diameter_ratio_raw - expected_raw).abs() < 1e-6,
            "raw transfer ratio should be query/row = {expected_raw}, got {}",
            result.chipload_diameter_ratio_raw
        );
        let expected_scale = apply_chipload_law(expected_raw, CHIPLOAD_DIAMETER_EXPONENT);
        assert!(
            (result.chipload_diameter_scale - expected_scale).abs() < 1e-6,
            "applied scale should be raw^{CHIPLOAD_DIAMETER_EXPONENT} = {expected_scale}, got {}",
            result.chipload_diameter_scale
        );
        assert!(
            expected_scale > expected_raw,
            "sanity: softening the exponent must RAISE the band on a down-transfer \
             ({expected_raw} → {expected_scale})"
        );
        assert!((result.chipload_hardness_scale - 1.0).abs() < 1e-6);
    }

    /// F3.1 (defect class C1) — A1's score replay: the Whiteside
    /// Fusion360 .tool preset rows (encoded as degenerate min==max
    /// "ranges") used to WIN tapered-ball/hardwood/parallel/finish at
    /// grade-a/exact (1813 vs 1705 for the calibrated Amana row),
    /// returning a Validated chipload ~15× the Amana-scaled value and
    /// hard-blocking the optimizer. Demoted to grade-c + fallback with
    /// the fabricated lower bound dropped, the calibrated row must win.
    #[test]
    fn demoted_fusion360_preset_rows_lose_to_calibrated_amana() {
        let lut = embedded_lut();
        let query = LookupQuery {
            tool_family: ToolFamily::TaperedBallNose,
            tool_subfamily: None,
            diameter_mm: 6.0,
            flute_count: 2,
            material_family: MaterialFamily::Hardwood,
            hardness_kind: Some(HardnessKind::Janka),
            hardness_value: Some(1450.0),
            operation_family: LutOperationFamily::Parallel,
            pass_role: LutPassRole::Finish,
        };
        let result = lookup_best(&lut, &query).expect("tapered-ball hardwood row");
        assert!(
            !result.observation_id.contains("fusion360"),
            "Fusion360 preset row must not win a calibrated query, got {}",
            result.observation_id
        );
        // Feeds matrix R5 (2026-09-23): the printed Onsrud 77-100 1/4 in
        // row (exact/a) now wins this query; the Amana 6.0 mm row is
        // derived/c. Since A3 step 3 the row is the pocket row, which the
        // G3 family rule serves to the parallel query. The claim of this
        // test is unchanged: a preset row never beats a printed vendor row.
        assert_eq!(
            result.observation_id, "onsrud-hardwood-77-100-1_4-pocket",
            "a printed vendor row expected, got {}",
            result.observation_id
        );
        // The demoted rows keep their nominal as an upper reference but
        // publish no fabricated lower bound any more.
        let sc64 = embedded_lut()
            .observations
            .into_iter()
            .find(|o| o.observation_id == "whiteside-sc64-conical-ball-nose-spiral-fusion360")
            .expect("demoted row still present as RPM/nominal reference");
        assert!(sc64.chipload_min_mm_tooth.is_none());
        assert_eq!(
            sc64.evidence_grade,
            crate::feeds::vendor_lut::EvidenceGrade::C
        );
        assert_eq!(
            sc64.row_kind,
            crate::feeds::vendor_lut::ObservationKind::Fallback
        );
    }

    #[test]
    fn test_rpm_nominal_returned() {
        let lut = embedded_lut();
        let query = LookupQuery {
            tool_family: ToolFamily::FlatEnd,
            tool_subfamily: None,
            diameter_mm: 6.0,
            flute_count: 2,
            material_family: MaterialFamily::Softwood,
            hardness_kind: Some(HardnessKind::Janka),
            hardness_value: Some(600.0),
            operation_family: LutOperationFamily::Adaptive,
            pass_role: LutPassRole::Roughing,
        };
        let result = lookup_best(&lut, &query).expect("should find match for 6mm flat softwood");
        assert_eq!(result.rpm_nominal, Some(18000.0));
    }

    fn vbit_query(diameter_mm: f64, material: MaterialFamily, janka: f64) -> LookupQuery {
        LookupQuery {
            tool_family: ToolFamily::ChamferVbit,
            tool_subfamily: None,
            diameter_mm,
            flute_count: 2,
            material_family: material,
            hardness_kind: Some(HardnessKind::Janka),
            hardness_value: Some(janka),
            operation_family: LutOperationFamily::Trace,
            pass_role: LutPassRole::Finish,
        }
    }

    #[test]
    fn vbit_90deg_matches_a_90_row_not_120() {
        // A 90° V-bit must select a 90° row and never the Whiteside 120°
        // row, whose cone angle is 30° away — past the 20° tolerance.
        // Since 2026-10-01 the 90° hardwood row is the printed Onsrud 37-50
        // 1/4 in row of the Hard Wood sheet (the nearest printed key to
        // 6.0 mm). Before, it was `amana-vbit-hardwood-trace-6000-2f`,
        // retired because it is not on its cited chart (finding G5 D1).
        let lut = embedded_lut();
        let query = vbit_query(6.0, MaterialFamily::Hardwood, 1450.0);
        let result =
            find_best_vbit_row(&lut, &query, Some(90.0)).expect("90° V-bit must match a 90° row");
        assert_eq!(result.observation_id, "onsrud-hardwood-37-50-1_4-trace");
    }

    #[test]
    fn vbit_120deg_matches_120_row_not_90() {
        // A 120° V-bit must select the Whiteside 120° row, not the 90°
        // Amana rows (30° away, rejected by the tolerance gate).
        let lut = embedded_lut();
        let query = vbit_query(12.0, MaterialFamily::Hardwood, 1450.0);
        let result = find_best_vbit_row(&lut, &query, Some(120.0))
            .expect("120° V-bit must match the 120° row");
        assert_eq!(
            result.observation_id,
            "whiteside-vbit-hardwood-trace-12000-2f"
        );
    }

    #[test]
    fn vbit_no_query_angle_falls_back_to_score_only() {
        // With no query angle the gate is inert — every angle-compatible
        // row is eligible and the best-scoring one wins (legacy behaviour,
        // never worse than find_best_row).
        let lut = embedded_lut();
        let query = vbit_query(6.0, MaterialFamily::Hardwood, 1450.0);
        let with_none = find_best_vbit_row(&lut, &query, None).expect("angle-less query matches");
        let via_plain = find_best_row(&lut, &query).expect("plain lookup matches");
        assert_eq!(with_none.observation_id, via_plain.observation_id);
    }

    #[test]
    fn dispatcher_routes_vbit_to_angle_aware_lookup() {
        // S1-2 contract: `find_best_row_for_geometry` must route a
        // V-bit geometry hint to the angle-aware matcher, so a 90°
        // query never silently lands a 120° row. With dispatcher,
        // identical to calling find_best_vbit_row directly; without
        // dispatcher, the plain find_best_row would ignore the angle.
        let lut = embedded_lut();
        let query = vbit_query(6.0, MaterialFamily::Hardwood, 1450.0);
        let geom = crate::feeds::ToolGeometryHint::VBit {
            included_angle: 90.0,
            tip_diameter: 0.0,
        };
        let via_dispatch = find_best_row_for_geometry(&lut, &query, &geom)
            .expect("dispatcher must reach the 90° row");
        let via_direct = find_best_vbit_row(&lut, &query, Some(90.0))
            .expect("direct angle-aware lookup must reach the 90° row");
        assert_eq!(via_dispatch.observation_id, via_direct.observation_id);
    }

    #[test]
    fn dispatcher_routes_flat_to_plain_lookup() {
        // Non-V-bit geometry falls through to find_best_row — angle
        // gate doesn't apply. Identical to calling find_best_row.
        let lut = embedded_lut();
        let query = LookupQuery {
            tool_family: ToolFamily::FlatEnd,
            tool_subfamily: None,
            diameter_mm: 6.0,
            flute_count: 2,
            material_family: MaterialFamily::Softwood,
            hardness_kind: Some(HardnessKind::Janka),
            hardness_value: Some(600.0),
            operation_family: LutOperationFamily::Adaptive,
            pass_role: LutPassRole::Roughing,
        };
        let geom = crate::feeds::ToolGeometryHint::Flat;
        let via_dispatch =
            find_best_row_for_geometry(&lut, &query, &geom).expect("dispatch must match flat");
        let via_direct = find_best_row(&lut, &query).expect("plain must match flat");
        assert_eq!(via_dispatch.observation_id, via_direct.observation_id);
    }

    #[test]
    fn vbit_matches_angle_only_row_without_diameter() {
        // Phase 5 Step 5.2: a 30° Spektra engrave row carrying
        // `diameter_mm = None` must be selectable by an angle-aware
        // V-bit query of the same angle. Pre-relaxation `passes_must_match`
        // rejected any obs with `diameter_mm <= 0.0`; the new row would
        // have been invisible to the lookup.
        let lut = embedded_lut();
        let query = vbit_query(3.175, MaterialFamily::Softwood, 600.0);
        let result = find_best_vbit_row(&lut, &query, Some(30.0))
            .expect("30° angle-only v-bit row must be reachable for a 30° query");
        // Either the Spektra engrave row OR the AMS-159
        // `amana-vgroove-softwood-trace-30deg-1f` row can win on score;
        // both have the same chipload data. Since ruling B4 (2026-09-25)
        // neither row carries a diameter (the charts print none). Assert
        // we land a 30° softwood-class row, not a far-angle row.
        let won_an_angle_only_row = result.row_diameter_mm < f64::EPSILON;
        let won_the_legacy_row = result.observation_id == "amana-vgroove-softwood-trace-30deg-1f";
        assert!(
            won_an_angle_only_row || won_the_legacy_row,
            "expected angle-only or legacy 30° row, got {} (row_diameter={})",
            result.observation_id,
            result.row_diameter_mm
        );
    }

    #[test]
    fn diameter_window_row_matches_flat_query_no_extrapolation() {
        // Phase 5 Step 5.2: the Onsrud polycarbonate article row
        // (diameter_mm = None) is matched on tool_family + material +
        // operation. A polycarbonate contour-finish query against the
        // embedded LUT must reach it and the chipload bounds must
        // carry through unscaled (no diameter extrapolation).
        let lut = embedded_lut();
        let query = LookupQuery {
            tool_family: ToolFamily::FlatEnd,
            tool_subfamily: Some("o_flute_upcut".to_owned()),
            diameter_mm: 6.35,
            flute_count: 1,
            material_family: MaterialFamily::Polycarbonate,
            hardness_kind: None,
            hardness_value: None,
            operation_family: LutOperationFamily::Contour,
            pass_role: LutPassRole::Finish,
        };
        let result =
            lookup_best(&lut, &query).expect("Onsrud polycarbonate window row must be reachable");
        // Anchorless rows return `chipload_diameter_scale = 1.0` regardless
        // of query diameter — diameter extrapolation is intentionally
        // off for these rows.
        if result.row_diameter_mm < f64::EPSILON {
            assert!(
                (result.chipload_diameter_scale - 1.0).abs() < 1e-9,
                "anchorless row must report scale=1.0, got {}",
                result.chipload_diameter_scale,
            );
        }
    }

    #[test]
    fn fiberglass_category_does_not_extrapolate_to_polymer_or_metal() {
        // Phase 5 Step 5.3: MaterialFamily::Fiberglass is its own
        // material_category (3) — a fiberglass query must NOT match
        // a polymer/aluminum row even when all other axes line up.
        let lut = embedded_lut();
        let query = LookupQuery {
            tool_family: ToolFamily::FlatEnd,
            tool_subfamily: None,
            diameter_mm: 6.0,
            flute_count: 2,
            material_family: MaterialFamily::Fiberglass,
            hardness_kind: None,
            hardness_value: None,
            operation_family: LutOperationFamily::Pocket,
            pass_role: LutPassRole::Roughing,
        };
        let result = lookup_best(&lut, &query)
            .expect("Garr GP fiberglass row must be reachable for a fiberglass query");
        assert_eq!(
            result.observation_id, "garr-gp-fiberglass-6000-2f-flat",
            "fiberglass query must select the Garr fiberglass row, not a \
             polymer/aluminum row that would otherwise match on diameter+op"
        );
    }

    /// F3.4 / A1 blocker replay — an RPM-only row (no chipload bounds)
    /// can win the plain lookup (it's a legitimate vendor RPM anchor
    /// for the feeds calculator), but the chipload-ENVELOPE lookup
    /// must skip it and return the best chipload-bearing row instead
    /// of forcing the gate to `Unmodeled(NoVendorData)`. Replay case:
    /// Replay case since feeds matrix R5 (2026-09-23): 6.35 mm 2F V-bit
    /// / hardwood / trace finish — plain winner is the Whiteside 1540
    /// RPM anchor `whiteside-1540-vgroove-60deg-quarter-rpm` (chipload
    /// 0.0, score 1945 against 1825 for the AMS-159 60 deg row). Before
    /// ruling B4 every V-bit trace cell of the matrix resolved to it
    /// (EVIDENCE 4.4-5). Since B4 the recipe door
    /// (`find_best_row_for_geometry`) tries the chip rows first for a
    /// V-bit, so only the plain `find_best_vbit_row` still reaches it.
    /// The earlier plywood adaptive replay lost its RPM-only winner when
    /// R5 added the Onsrud plywood sheets.
    #[test]
    fn chip_envelope_lookup_skips_rpm_only_rows() {
        let lut = embedded_lut();
        let query = LookupQuery {
            tool_family: ToolFamily::ChamferVbit,
            tool_subfamily: None,
            diameter_mm: 6.35,
            flute_count: 2,
            material_family: MaterialFamily::Hardwood,
            hardness_kind: Some(HardnessKind::Janka),
            hardness_value: Some(1450.0),
            operation_family: LutOperationFamily::Trace,
            pass_role: LutPassRole::Finish,
        };
        // The V-bit family resolves through the angle-aware lookup, the
        // door `find_best_row_for_geometry` uses; the flat `lookup_best`
        // ignores the included angle and would pick a 90 degree insert row.
        let plain = find_best_vbit_row(&lut, &query, Some(60.0)).expect("plain lookup matches");
        assert_eq!(
            plain.observation_id, "whiteside-1540-vgroove-60deg-quarter-rpm",
            "fixture drift: the RPM-only row no longer wins the plain lookup — \
             pick a query where it does, or this test loses its teeth"
        );
        assert!(plain.chip_load_mm <= 0.0, "RPM-only row has no chipload");

        let env = find_best_chip_envelope_row(
            &lut,
            &query,
            &crate::feeds::ToolGeometryHint::VBit {
                included_angle: 60.0,
                tip_diameter: 0.0,
            },
        )
        .expect("a chipload-bearing row exists underneath");
        assert!(
            env.chip_load_min_mm.is_some() || env.chip_load_max_mm.is_some(),
            "envelope lookup must return a chipload-bearing row, got {}",
            env.observation_id
        );
        assert!(env.chip_load_mm > 0.0);

        // Ruling B4: the recipe door gives the envelope row, not the RPM
        // anchor, for a V-bit.
        let recipe = find_best_row_for_geometry(
            &lut,
            &query,
            &crate::feeds::ToolGeometryHint::VBit {
                included_angle: 60.0,
                tip_diameter: 0.0,
            },
        )
        .expect("the recipe door matches");
        assert_eq!(recipe.observation_id, env.observation_id);
        assert_eq!(
            recipe.observation_id,
            "amana-vgroove-hardwood-trace-60deg-2f"
        );
    }

    /// F3.2 rule 8 — equal-score ties break on observation_id, not on
    /// insertion order: the same two rows in either order pick the
    /// same winner.
    #[test]
    fn equal_score_tie_breaks_on_observation_id_not_order() {
        let row = |id: &str| {
            let mut o = VendorLut::embedded()
                .observations
                .into_iter()
                .find(|o| o.observation_id == "amana-flat-softwood-adaptive-6000-2f")
                .expect("anchor row exists");
            o.observation_id = id.to_owned();
            o
        };
        let query = LookupQuery {
            tool_family: ToolFamily::FlatEnd,
            tool_subfamily: None,
            diameter_mm: 6.0,
            flute_count: 2,
            material_family: MaterialFamily::Softwood,
            hardness_kind: Some(HardnessKind::Janka),
            hardness_value: Some(600.0),
            operation_family: LutOperationFamily::Adaptive,
            pass_role: LutPassRole::Roughing,
        };
        let forward = VendorLut {
            observations: vec![row("tie-aaa"), row("tie-bbb")],
        };
        let reversed = VendorLut {
            observations: vec![row("tie-bbb"), row("tie-aaa")],
        };
        let a = lookup_best(&forward, &query).expect("match");
        let b = lookup_best(&reversed, &query).expect("match");
        assert_eq!(a.observation_id, "tie-aaa");
        assert_eq!(b.observation_id, "tie-aaa");
    }

    #[test]
    fn vbit_rejects_row_outside_angle_tolerance() {
        // A 150° bit has no row within 20° (the embedded ladder tops out at
        // 120°), so the angle-aware lookup refuses rather than returning a
        // far-angle row. (60° now matches a real 60° V-groove row, so this
        // probe uses an angle beyond the populated range.)
        let lut = embedded_lut();
        let query = vbit_query(6.0, MaterialFamily::Hardwood, 1450.0);
        assert!(
            find_best_vbit_row(&lut, &query, Some(150.0)).is_none(),
            "150° bit must not match any row (nearest is 120°, 30° away)"
        );
    }
}
