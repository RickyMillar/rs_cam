//! Extrapolation claims: what the engine says when no vendor row prints
//! the queried cell exactly.
//!
//! The extrapolation programme (`planning/extrapolation_2026-09-24/`)
//! splits the LUT's gaps into groups G1-G9. Each group gets one
//! implementation of [`Extrapolation`]. The implementation reads the
//! matched row (the anchor) and the LUT, and returns a [`SizeBasis`]:
//!
//! - the anchor answers the query as printed (`Exact`, `NoDiameterAnchor`,
//!   `NoChipload`);
//! - a [`Claim`] states the rule, the range where the rule is valid, the
//!   residual, and the scale it applies to the anchor's band;
//! - `Refused` states why no rule may answer.
//!
//! G1 (size) is the first group: [`SizeLaw`] in [`size`]. The claim runs
//! inside `vendor_lookup::build_result`, so every consumer of a
//! `LookupResult` (Suggest, the gate, the modulator, the advisor, the
//! viewport, explain) reads one claimed band.
//!
//! G2 (hardness) is not an [`Extrapolation`] impl: [`hardness_basis`] in
//! [`hardness`] is a typed clamp on the Janka law (the soft/hard cap). It
//! runs in `build_result` too, beside the size claim (P2 step 4).
//!
//! G3 (family) is not an [`Extrapolation`] impl either: [`family_basis`] in
//! [`family`] marks a row that a stated family rule carries from its home
//! operation family into the queried family. The rule table
//! ([`FAMILY_RULES`]) widens the lookup's must-match filter, and the row's
//! band moves by no number (A3, copy semantics).
//!
//! G6 (drill) is not an [`Extrapolation`] impl either: [`drill_basis`] in
//! [`drill`] marks a flat end mill plunge that the Amana Spektra rule
//! ([`DRILL_RULES`]) serves from the tool's printed side row. The rule
//! widens the must-match filter as a family rule does, and it moves one
//! number: the chip is the side chip / Z (ruling B5).

pub mod drill;
pub mod family;
pub mod hardness;
pub mod size;

pub use drill::{
    DRILL_PECK_TEXT, DRILL_RULE_TEXT, DRILL_RULES, DrillBasis, DrillClaim, DrillRule, drill_basis,
    drill_rule,
};

pub use family::{
    FAMILY_RULE_TEXT, FAMILY_RULES, FamilyBasis, FamilyClaim, FamilyRule, family_basis,
    transfer_rule,
};

pub use hardness::{
    HardnessBasis, JankaFrom, SOFT_OVER_HARD_PRINTED_MAX, SoftHardCap, hardness_basis,
    soft_hard_cap,
};

pub use size::{
    EXACT_DIAMETER_TOLERANCE, FLAT_END_SLOPE_SPREAD, SIZE_WINDOW_EDGE_TOLERANCE, SIZE_WINDOW_MAX,
    SIZE_WINDOW_MIN, SizeLaw, TAPERED_SLOPE_SPREAD,
};

use super::vendor_lookup::LookupQuery;
use super::vendor_lut::{VendorLut, VendorObservation};

/// The gap group that a claim or a refusal belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Gap {
    /// G1: a row exists for the family, but not at this diameter.
    Size,
    /// G2: a row exists in the material category, but at another hardness
    /// (the Janka law and its soft/hard cap). Only the card uses it.
    Hardness,
    /// G3: a row exists for the tool, but it is filed under another
    /// operation family (a family rule carries it, `family`).
    Family,
    /// G6: no row is filed under the drill family; a flat end mill plunge
    /// reads the tool's printed side row through the drill rule (`drill`,
    /// ruling B5).
    Drill,
}

impl Gap {
    /// The programme's group tag ("G1").
    #[must_use]
    pub const fn group(self) -> &'static str {
        match self {
            Self::Size => "G1",
            Self::Hardness => "G2",
            Self::Family => "G3",
            Self::Drill => "G6",
        }
    }

    /// The name that a card prints for the gap.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Size => "size",
            Self::Hardness => "hardness",
            Self::Family => "family",
            Self::Drill => "drill",
        }
    }
}

/// How many independent witnesses support a claim. P1 uses only
/// `OneWitness`: a second vendor at the same size is a data check, not a
/// second witness for a law.
///
/// Not called `Confidence`: `tool_load::verdict` and `diagnostics` already
/// own that name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimConfidence {
    OneWitness,
    TwoWitnesses,
}

/// The form of a G1 size claim (EXTRAPOLATION_G1 §3.1).
#[derive(Debug, Clone, PartialEq)]
pub enum SizeForm {
    /// Form A: log-log interpolation between two adjacent printed sizes of
    /// one chart series. `lo_mm` and `hi_mm` are the two printed sizes.
    Interpolated { lo_mm: f64, hi_mm: f64 },
    /// Form B: the anchor series' own power law (an OLS fit of the log
    /// band mids on the log diameters), one printed step past the span.
    SeriesSlope { slope: f64, r2: f64, sizes: usize },
    /// Form C: the generic size law `(d / d_row)^exponent` inside the
    /// 0.5x-2x window of the row. A V-bit takes no claim in P1
    /// ([`SizeBasis::VBitExempt`]).
    GenericFallback { exponent: f64 },
}

impl SizeForm {
    /// The short name of the form, for the FM1 columns and the card.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Interpolated { .. } => "A",
            Self::SeriesSlope { .. } => "B",
            Self::GenericFallback { .. } => "C",
        }
    }
}

/// What a claim does not know: the error that it carries.
#[derive(Debug, Clone, PartialEq)]
pub enum ClaimResidual {
    /// Form A: the two printed band mids (mm/tooth) that bracket the
    /// query. The true value lies between them if the chart is monotone.
    Bracket { lo_value: f64, hi_value: f64 },
    /// Form B: the root-mean-square of the fit's log residuals. For small
    /// values this is the fractional error of one printed point.
    FitRms { fraction: f64 },
    /// Form C: the spread of the printed vendor slopes, as the scale range
    /// `r^(slope_lo - p)` to `r^(slope_hi - p)` at this ratio `r`, sorted
    /// so that `lo <= hi`. `slopes` is the slope range of `family`.
    VendorSpread {
        lo: f64,
        hi: f64,
        slopes: (f64, f64),
        family: SpreadFamily,
    },
}

/// Whose printed slopes give a form C spread (decision 2, 2026-09-24).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpreadFamily {
    /// The flat-end series ([`FLAT_END_SLOPE_SPREAD`]).
    FlatEnd,
    /// The tapered-ball series ([`TAPERED_SLOPE_SPREAD`]).
    TaperedBall,
    /// A family with no printed series (ball, bull, V-bit): the flat-end
    /// spread stands in, and the card says so.
    BorrowedFromFlatEnd,
}

/// One claim: a stated rule that carries a vendor band from the anchor
/// row to the queried size.
#[derive(Debug, Clone, PartialEq)]
pub struct Claim {
    pub gap: Gap,
    pub form: SizeForm,
    /// The rule in words, for the detail line.
    pub rule: &'static str,
    /// The scale that the claim applies to the anchor's min, mid and max
    /// (before the hardness scale).
    pub scale: f64,
    /// The anchor row's diameter (mm).
    pub anchor_diameter_mm: f64,
    /// The query's lookup key (mm).
    pub query_diameter_mm: f64,
    /// The rows that support the claim: the two bracketing rows (form A),
    /// the fitted series (form B), or the anchor (form C).
    pub source_rows: Vec<String>,
    /// The `source_id` of the chart whose rows support the claim. It
    /// differs from the anchor's chart only on a cross-chart form A
    /// (a tapered tip under 1.5 mm).
    pub series_source_id: String,
    /// The diameters (mm) where the rule is valid.
    pub range_mm: std::ops::RangeInclusive<f64>,
    pub residual: ClaimResidual,
    pub confidence: ClaimConfidence,
}

impl Claim {
    /// The card text: a headline and a detail line, in plain words.
    ///
    /// Example (form C on a ball nose):
    /// headline "extrapolated (G1 size): x0.68 from the 6.0 mm row";
    /// detail "scaled x0.68 from the 6.0 mm row by the generic size law
    /// d^0.61; valid 0.5-2x of the row (3.000-12.000 mm); spread borrowed
    /// from flat end mills: flat-end vendors spread x0.80-1.56 at 2x
    /// (x0.67-1.23 at this size)". Diameters print in their shortest exact
    /// form with a decimal point ("6.0", "0.79375").
    #[must_use]
    pub fn card_text(&self) -> (String, String) {
        let anchor = self.anchor_diameter_mm;
        let scale = self.scale;
        let headline = format!(
            "extrapolated ({} {}): x{scale:.2} from the {anchor:?} mm row",
            self.gap.group(),
            self.gap.label()
        );
        let lo = *self.range_mm.start();
        let hi = *self.range_mm.end();
        let detail = match (&self.form, &self.residual) {
            (SizeForm::Interpolated { lo_mm, hi_mm }, residual) => {
                let values = match residual {
                    ClaimResidual::Bracket { lo_value, hi_value } => {
                        format!("; the printed values are {lo_value:.4} and {hi_value:.4} mm/tooth")
                    }
                    ClaimResidual::FitRms { .. } | ClaimResidual::VendorSpread { .. } => {
                        String::new()
                    }
                };
                format!(
                    "interpolated x{scale:.2} from the {anchor:?} mm row between the printed \
                     {lo_mm:?} mm and {hi_mm:?} mm rows of {} (log-log); valid {lo:?}-{hi:?} \
                     mm{values}",
                    self.series_source_id
                )
            }
            (SizeForm::SeriesSlope { slope, r2, sizes }, residual) => {
                let rms = match residual {
                    ClaimResidual::FitRms { fraction } => {
                        format!("; fit rms {:.1} %", fraction * 100.0)
                    }
                    ClaimResidual::Bracket { .. } | ClaimResidual::VendorSpread { .. } => {
                        String::new()
                    }
                };
                format!(
                    "scaled x{scale:.2} from the {anchor:?} mm row by the chart's own size slope \
                     d^{slope:.2} (r2 {r2:.2}, {sizes} printed sizes of {}); valid {lo:.3}-{hi:.3} \
                     mm, one printed step past the chart{rms}",
                    self.series_source_id
                )
            }
            (SizeForm::GenericFallback { exponent }, residual) => {
                let spread = match residual {
                    ClaimResidual::VendorSpread {
                        lo: at_lo,
                        hi: at_hi,
                        slopes: (s_lo, s_hi),
                        family,
                    } => {
                        let double_lo = 2.0_f64.powf(s_lo - exponent);
                        let double_hi = 2.0_f64.powf(s_hi - exponent);
                        let who = match family {
                            SpreadFamily::FlatEnd => "flat-end vendors spread",
                            SpreadFamily::TaperedBall => "tapered-ball vendors spread",
                            SpreadFamily::BorrowedFromFlatEnd => {
                                "spread borrowed from flat end mills: flat-end vendors spread"
                            }
                        };
                        format!(
                            "; {who} x{double_lo:.2}-{double_hi:.2} at 2x (x{at_lo:.2}-{at_hi:.2} \
                             at this size)"
                        )
                    }
                    ClaimResidual::Bracket { .. } | ClaimResidual::FitRms { .. } => String::new(),
                };
                let window = format!("valid {SIZE_WINDOW_MIN}-{SIZE_WINDOW_MAX}x of the row");
                format!(
                    "scaled x{scale:.2} from the {anchor:?} mm row by the generic size law \
                     d^{exponent}; {window} ({lo:.3}-{hi:.3} mm){spread}"
                )
            }
        };
        (headline, detail)
    }
}

/// The basis that one gap implementation gives for one matched row.
#[derive(Debug, Clone, PartialEq)]
pub enum SizeBasis {
    /// The anchor prints the queried size (within
    /// [`EXACT_DIAMETER_TOLERANCE`]).
    Exact,
    /// The anchor has no diameter (a V-bit row, a diameter-window
    /// article). The band carries through unscaled.
    NoDiameterAnchor,
    /// The anchor publishes an RPM and no chipload (an RPM-only anchor).
    /// The RPM carries; the calculator uses the formula chipload.
    NoChipload,
    /// A V-bit takes no size claim and no size window in P1 (P1_PLAN §4
    /// risk 5, until ruling B4). The gate keys a V-bit at the engaged width
    /// at the sample depth, which is under 1.5 mm on a shallow cut, so a
    /// window here would turn judged V-bit verdicts into `Unmodeled`. The
    /// row keeps the generic `(d / d_row)^0.61` scale, and the Suggest
    /// support arm keeps the pre-P1 micro rule on its own key.
    VBitExempt,
    /// A stated claim carries the band to the queried size.
    Claim(Box<Claim>),
    /// No rule may answer. The reason starts with
    /// "no published figure for a ".
    Refused { gap: Gap, reason: String },
}

impl SizeBasis {
    /// The claim, when the basis is a claim.
    #[must_use]
    pub fn claim(&self) -> Option<&Claim> {
        match self {
            Self::Claim(claim) => Some(claim),
            Self::Exact
            | Self::NoDiameterAnchor
            | Self::NoChipload
            | Self::VBitExempt
            | Self::Refused { .. } => None,
        }
    }

    /// True when no rule may answer. A refused row publishes no band.
    #[must_use]
    pub const fn is_refused(&self) -> bool {
        matches!(self, Self::Refused { .. })
    }

    /// The refusal reason, when the basis is a refusal.
    #[must_use]
    pub fn refusal_reason(&self) -> Option<&str> {
        match self {
            Self::Refused { reason, .. } => Some(reason),
            Self::Exact
            | Self::NoDiameterAnchor
            | Self::NoChipload
            | Self::VBitExempt
            | Self::Claim(_) => None,
        }
    }

    /// The short name of the basis, for the FM1 columns.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Exact => "Exact",
            Self::NoDiameterAnchor => "NoDiameterAnchor",
            Self::NoChipload => "NoChipload",
            Self::VBitExempt => "VBitExempt",
            Self::Claim(_) => "Claim",
            Self::Refused { .. } => "Refused",
        }
    }
}

/// One gap group's rule. The implementation reads the matched row (the
/// anchor) and the LUT, and returns the basis for the query.
pub trait Extrapolation {
    /// The gap group this rule serves.
    fn gap(&self) -> Gap;
    /// The basis for `query` on the matched row `anchor`.
    fn basis(&self, lut: &VendorLut, query: &LookupQuery, anchor: &VendorObservation) -> SizeBasis;
}
