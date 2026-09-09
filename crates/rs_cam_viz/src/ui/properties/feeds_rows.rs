//! Row model for the read-only rows of the Feeds & Speeds card, and the
//! labels the Feeds modal shares with it (G-FEEDSLABEL, UX-R03-005).
//!
//! **Why this module exists.** Until 2026-09-10 the card printed the
//! calculator's recommendation under labels that read as the configured
//! operation. On a fresh Pocket (feed 750, RPM 15000, two flutes, depth
//! per pass 1.2) it printed `Commanded advance/tooth: 0.0435 mm/tooth` and
//! `DOC: 4.20 mm`. The stored operation gives 750 ÷ (15000 × 2) = 0.025
//! mm/tooth and cuts 1.2 mm per pass. The `Apply cut geometry` button
//! under the card writes 1.2, not 4.2, because the apply funnel caps the
//! depth to the machine rigidity. So the button was honest and the label
//! was not (`planning/ui_review_2026-09-09/results/R03/REPORT.md`,
//! UX-R03-005; `planning/ui_fix_2026-09-09/research/R0.5.md` §2.3).
//!
//! The rows are built here as plain strings so a test can read them
//! without an `egui` context. The card renders the strings, nothing more.
//!
//! **Roles.** Every row names two values:
//!
//! - `recommended` — what the calculator proposes now. DOC and WOC are the
//!   RAW calculator values and carry the "(calculator)" note: the apply
//!   funnel (`feeds::suggest::apply_feeds_subset` → `enforce_invariants`)
//!   can lower them, and this surface does not repeat that clamp logic.
//!   The advance row is `feed ÷ (RPM × flutes)` at the recommended feed —
//!   the quantity its label names — not the pre-derate target chipload,
//!   which the hover reports as evidence.
//! - `configured` — the value stored on the operation, which Generate
//!   reads. The advance row derives it from the stored feed and the
//!   effective spindle speed (override, else project default).
//!
//! The word "Commanded" is reserved for the OPERATING POINT card, where it
//! is the gate's commanded figure (`COMMANDED_ADVANCE_PER_TOOTH`).
//!
//! A full two-column card is D4; this is the honest label.

/// Em dash printed when a value cannot be formed (no flutes, zero RPM).
const ABSENT: &str = "\u{2014}";

/// The note appended to a raw calculator value the apply funnel may lower.
/// The Feeds modal hands it to `CompareRow::recommended_note` so both
/// surfaces print one string.
pub const CALCULATOR_NOTE: &str = "(calculator)";

/// Card label for the advance-per-tooth row.
pub const ADVANCE_ROW_LABEL: &str = "Recommended advance/tooth:";
/// Card label for the depth-per-pass row.
pub const DOC_ROW_LABEL: &str = "Recommended DOC:";
/// Card label for the stepover row.
pub const WOC_ROW_LABEL: &str = "Recommended WOC:";

/// Feeds modal row label for advance per tooth. The modal's grid already
/// has a `Current` and a `Recommended` column, so the row label names the
/// quantity only.
pub const MODAL_ADVANCE_ROW_LABEL: &str = "Advance/tooth";

/// One read-only row on the card.
#[derive(Debug, Clone, PartialEq)]
pub struct FeedsCardRow {
    /// Row label, ends with a colon.
    pub label: &'static str,
    /// The calculator's value, formatted with its unit.
    pub recommended: String,
    /// The stored operation's value, formatted as `configured <value>`.
    /// `None` only when the operation has no such field.
    pub configured: Option<String>,
    /// Hover text that says what each number is.
    pub hover: String,
}

/// The values the card reads: the calculator result on one side, the
/// stored operation (and the effective spindle speed) on the other.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FeedsCardInputs {
    /// `FeedsResult::feed_rate_mm_min`.
    pub recommended_feed_mm_min: f64,
    /// `FeedsResult::rpm`.
    pub recommended_rpm: f64,
    /// `FeedsResult::chip_load_mm` — the pre-derate target chipload.
    pub target_chip_load_mm: f64,
    /// `FeedsResult::axial_depth_mm`, `None` when the operation has no
    /// depth per pass.
    pub recommended_axial_depth_mm: Option<f64>,
    /// `FeedsResult::radial_width_mm`, `None` when the operation has no
    /// stepover.
    pub recommended_radial_width_mm: Option<f64>,
    /// `OperationConfig::feed_rate()`.
    pub configured_feed_mm_min: f64,
    /// The effective spindle speed: the operation override, else the
    /// project default.
    pub configured_rpm: u32,
    /// `ToolConfig::flute_count`.
    pub flute_count: u32,
    /// `OperationConfig::depth_per_pass()`.
    pub configured_depth_per_pass_mm: Option<f64>,
    /// `OperationConfig::stepover()`.
    pub configured_stepover_mm: Option<f64>,
}

/// `feed ÷ (RPM × flutes)`. `None` when the quotient cannot be formed.
#[must_use]
pub fn advance_per_tooth_mm(feed_mm_min: f64, rpm: f64, flutes: u32) -> Option<f64> {
    if flutes == 0 || rpm <= 0.0 || !rpm.is_finite() {
        return None;
    }
    Some(feed_mm_min / (rpm * f64::from(flutes)))
}

/// Append the calculator note to a formatted value: `4.20 mm (calculator)`.
#[must_use]
pub fn with_calculator_note(formatted: &str) -> String {
    format!("{formatted} {CALCULATOR_NOTE}")
}

fn format_advance(value: Option<f64>) -> String {
    value.map_or_else(|| ABSENT.to_owned(), |v| format!("{v:.4} mm/tooth"))
}

fn configured(formatted: &str) -> String {
    format!("configured {formatted}")
}

/// Build the read-only rows of the card in display order: advance per
/// tooth, then DOC and WOC when the operation has those fields.
#[must_use]
pub fn feeds_card_rows(inputs: &FeedsCardInputs) -> Vec<FeedsCardRow> {
    let recommended_advance = advance_per_tooth_mm(
        inputs.recommended_feed_mm_min,
        inputs.recommended_rpm,
        inputs.flute_count,
    );
    let configured_advance = advance_per_tooth_mm(
        inputs.configured_feed_mm_min,
        f64::from(inputs.configured_rpm),
        inputs.flute_count,
    );
    let mut rows = vec![FeedsCardRow {
        label: ADVANCE_ROW_LABEL,
        recommended: format_advance(recommended_advance),
        configured: Some(configured(&format_advance(configured_advance))),
        hover: format!(
            "feed \u{00f7} (RPM \u{00d7} flutes) at the recommended feed \
             ({:.0} mm/min, {:.0} RPM, {} flutes). The calculator's target \
             chipload before derates is {:.4} mm/tooth. \"configured\" is the \
             same quantity from the stored feed ({:.0} mm/min) and the \
             effective spindle speed ({} RPM). The measured counterpart is on \
             the OPERATING POINT card below, after a simulation.",
            inputs.recommended_feed_mm_min,
            inputs.recommended_rpm,
            inputs.flute_count,
            inputs.target_chip_load_mm,
            inputs.configured_feed_mm_min,
            inputs.configured_rpm,
        ),
    }];
    if let Some(doc) = inputs.recommended_axial_depth_mm {
        rows.push(FeedsCardRow {
            label: DOC_ROW_LABEL,
            recommended: with_calculator_note(&format!("{doc:.2} mm")),
            configured: Some(configured(&format_mm(inputs.configured_depth_per_pass_mm))),
            hover: "Raw calculator depth per pass. \"Apply cut geometry\" passes it \
                    through the invariant funnel (machine rigidity cap, flute-length \
                    guard), so the written value can be lower. \"configured\" is the \
                    stored depth per pass, which Generate reads."
                .to_owned(),
        });
    }
    if let Some(woc) = inputs.recommended_radial_width_mm {
        rows.push(FeedsCardRow {
            label: WOC_ROW_LABEL,
            recommended: with_calculator_note(&format!("{woc:.2} mm")),
            configured: Some(configured(&format_mm(inputs.configured_stepover_mm))),
            hover: "Raw calculator stepover. \"Apply cut geometry\" passes it through \
                    the invariant funnel (stepover-to-diameter clamp), so the written \
                    value can be lower. \"configured\" is the stored stepover, which \
                    Generate reads."
                .to_owned(),
        });
    }
    rows
}

fn format_mm(value: Option<f64>) -> String {
    value.map_or_else(|| ABSENT.to_owned(), |v| format!("{v:.2} mm"))
}
