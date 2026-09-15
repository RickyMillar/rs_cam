//! Why — the sentences that explain one recommended number.
//!
//! This file used to draw a panel: a `Why is the recommendation here?`
//! disclosure holding provenance, then rationale, then a derate chain, then
//! a result, then warnings. The operator's verdict on 2026-09-15 was that it
//! is "all just too much", and that what they actually want is: *if a
//! recommended value differs from mine, let me hover it and see why.*
//!
//! They were right, and the reason is structural rather than a matter of
//! length. The disclosure explained **the recommendation as a whole**. The
//! operator reads the card **one row at a time**, and the question that
//! arises is always about one row — *why is my DOC being tripled?* A
//! whole-recipe explanation cannot answer a per-row question however short
//! it is, and the declutter earlier that day (44 painted runs down to 13)
//! proved it: shorter, still not an answer.
//!
//! So this is no longer a panel. It is a library of sentences, one per row
//! of the comparison card, and `compare.rs` hangs each one on the row it
//! explains. See `planning/feeds_rework_2026-09-15/PLAN.md` W2.
//!
//! Two things deliberately stay on the page rather than moving to a hover:
//!
//! - **Warnings.** A warning behind a hover is a warning that was deleted.
//! - **The engaged-diameter row**, for tapered and V tools, where the
//!   published tip size understates what is actually cutting.
//!
//! Every quantity here is a **commanded** advance per tooth,
//! `feed / (rpm · flutes)`. This surface runs before a simulation and has no
//! measured value to show. The achieved figure lives on the properties
//! panel's operating-point card, after a sim (Checkpoint H2, 2026-08-08).

use rs_cam_core::feeds::FeedsExplain;
use rs_cam_core::feeds::rationale::SuggestRationale;

use super::shared::{CurrentValues, engaged_diameter_context, vendor_band};
use crate::ui::{theme, tokens};

/// A factor within this of 1.0 changed nothing.
const UNITY_TOLERANCE: f64 = 1e-3;

/// One row of the comparison card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RecipeRow {
    Rpm,
    Feed,
    Plunge,
    Doc,
    Woc,
    Advance,
}

impl RecipeRow {
    /// The keywords that route a rationale entry to this row.
    ///
    /// The Suggest pass emits its own account of what it did — "adaptive3d
    /// depth_per_pass clamped 9.00 → 4.20 mm (vendor_ap)". That sentence is
    /// about ONE row, and it belongs on that row. An entry that matches no
    /// row is recipe-level and lands on [`RecipeRow::Advance`], which is the
    /// row that describes the recipe itself.
    fn rationale_keywords(self) -> &'static [&'static str] {
        match self {
            Self::Rpm => &["rpm", "spindle"],
            Self::Feed => &["feed"],
            Self::Plunge => &["plunge"],
            Self::Doc => &["depth_per_pass", "dpp", "axial", "doc", "depth"],
            Self::Woc => &["stepover", "radial", "woc", "scallop"],
            Self::Advance => &[],
        }
    }
}

/// One wrapped line, marked and hoverable, carrying its explanation.
fn detail_line(ui: &mut egui::Ui, text: impl AsRef<str>, color: egui::Color32, hover: &str) {
    let marked = format!("{} {}", text.as_ref(), tokens::GLYPH_DETAIL);
    ui.add(egui::Label::new(egui::RichText::new(marked).small().color(color)).wrap())
        .on_hover_text(hover.to_owned());
}

/// One wrapped line with nothing further to say.
fn plain_line(ui: &mut egui::Ui, text: impl Into<String>, color: egui::Color32) {
    ui.add(egui::Label::new(egui::RichText::new(text.into()).small().color(color)).wrap());
}

// ── The per-row explanations ─────────────────────────────────────────

/// The sentences that explain one recommended value.
///
/// Returns the hover text for `row`. Always non-empty: a row that did not
/// move still answers "why is this NOT moving", which is the same question
/// inverted and is asked just as often.
pub(crate) fn row_explanation(
    row: RecipeRow,
    current: &CurrentValues,
    explain: &FeedsExplain,
    rationale: Option<&SuggestRationale>,
) -> String {
    let mut out = String::new();
    match row {
        RecipeRow::Rpm => explain_rpm(&mut out, explain),
        RecipeRow::Feed => explain_feed(&mut out, current, explain),
        RecipeRow::Plunge => explain_plunge(&mut out, explain),
        RecipeRow::Doc => explain_doc(&mut out, explain),
        RecipeRow::Woc => explain_woc(&mut out, current, explain),
        RecipeRow::Advance => explain_advance(&mut out, explain),
    }
    append_rationale(&mut out, row, rationale);
    out
}

fn explain_rpm(out: &mut String, explain: &FeedsExplain) {
    out.push_str(&format!("{:.0} RPM.\n", explain.recommended.rpm));
    match &explain.matched_row {
        Some(row) => match row.rpm_min.zip(row.rpm_max) {
            Some((lo, hi)) => out.push_str(&format!(
                "Vendor row {} publishes {lo:.0}–{hi:.0} RPM.\n",
                row.observation_id
            )),
            None => match row.rpm_nominal {
                Some(nom) => out.push_str(&format!(
                    "Vendor row {} publishes {nom:.0} RPM nominal.\n",
                    row.observation_id
                )),
                None => out.push_str("The vendor row publishes no RPM; this is the formula's.\n"),
            },
        },
        None => out.push_str("No vendor row matched. This is the empirical formula's RPM.\n"),
    }
    let speedup = explain.recommended.derates.spindle_speedup;
    if (speedup - 1.0).abs() > UNITY_TOLERANCE {
        out.push_str(&format!(
            "Spindle policy MaxSpeed lifted it ×{speedup:.3} toward the spindle \
             ceiling, and scaled the feed with it to hold the advance/tooth \
             constant.\n"
        ));
    } else {
        out.push_str("Spindle policy is Match chart, so the RPM follows the vendor row.\n");
    }
}

fn explain_feed(out: &mut String, current: &CurrentValues, explain: &FeedsExplain) {
    let r = &explain.recommended;
    let flutes = current.flute_count.max(1);
    let advance = if r.rpm > 0.0 {
        r.feed_rate_mm_min / (r.rpm * f64::from(flutes))
    } else {
        0.0
    };
    out.push_str(&format!("{:.0} mm/min.\n", r.feed_rate_mm_min));
    out.push_str(&format!(
        "feed = advance/tooth × RPM × flutes = {advance:.4} × {:.0} × {flutes}.\n",
        r.rpm
    ));
    let d = &r.derates;
    if d.power_limit < 0.999 {
        out.push_str(&format!(
            "The spindle could not deliver the power, so the feed was cut ×{:.3}.\n",
            d.power_limit
        ));
    }
    if d.feed_clamp < 0.999 {
        out.push_str(&format!(
            "The feed hit machine.max_feed_mm_min and was clamped ×{:.3}.\n",
            d.feed_clamp
        ));
    }
    out.push_str("Change the advance/tooth to change this; see that row.\n");
}

fn explain_plunge(out: &mut String, explain: &FeedsExplain) {
    out.push_str(&format!(
        "{:.0} mm/min.\n",
        explain.recommended.plunge_rate_mm_min
    ));
    out.push_str(
        "The plunge baseline is derived from the cutting feed and the tool \
         diameter: a small cutter plunges slower than a large one at the same \
         feed.\n",
    );
}

fn explain_doc(out: &mut String, explain: &FeedsExplain) {
    out.push_str(&format!("{:.2} mm.\n", explain.recommended.axial_depth_mm));
    out.push_str(CALCULATOR_CAVEAT);
    let tier = explain.recommended.derates.depth_tier;
    if tier < 0.999 {
        out.push_str(&format!(
            "\nThis depth is deep enough to derate the feed ×{tier:.3}, to limit \
             deflection."
        ));
    }
}

fn explain_woc(out: &mut String, current: &CurrentValues, explain: &FeedsExplain) {
    out.push_str(&format!("{:.2} mm.\n", explain.recommended.radial_width_mm));
    if current.supports_scallop_override && current.scallop_height.is_some() {
        out.push_str(
            "This stepover is derived from your target scallop height and the \
             tool's ball-tip radius, not from the vendor row.\n",
        );
    }
    out.push_str(CALCULATOR_CAVEAT);
}

fn explain_advance(out: &mut String, explain: &FeedsExplain) {
    let r = &explain.recommended;
    let d = &r.derates;
    let flutes = explain.flute_count.max(1) as f64;
    let effective = if r.rpm > 0.0 {
        r.feed_rate_mm_min / (r.rpm * flutes)
    } else {
        0.0
    };
    out.push_str(&format!("{effective:.4} mm/tooth.\n"));

    // Where the target came from.
    match (&r.chipload_source, &d.formula) {
        (rs_cam_core::feeds::ChiploadSource::VendorLut { observation_id }, _) => {
            out.push_str(&format!(
                "Target {:.4} mm/tooth — the midpoint of vendor row {observation_id}.\n",
                d.target_chip_load_mm
            ));
        }
        (rs_cam_core::feeds::ChiploadSource::FormulaFallback, Some(f))
        | (rs_cam_core::feeds::ChiploadSource::EdgeRadiusFloor, Some(f)) => {
            out.push_str(&format!(
                "Target {:.4} mm/tooth — no vendor row matched, so the empirical \
                 formula set it: fz = K₀·D^p·(1/H)^q = {:.4}·{:.2}^{:.2}·(1/{:.1})^{:.2}.\n",
                f.result_mm_tooth, f.k0, f.diameter_mm, f.p, f.feed_scale_factor, f.q
            ));
        }
        _ => {
            out.push_str(&format!(
                "Target {:.4} mm/tooth before the factors below.\n",
                d.target_chip_load_mm
            ));
        }
    }

    // What moved it.
    let combined = d.combined_factor();
    let pct = ((1.0 - combined) * 100.0).max(0.0);
    let mut moved: Vec<String> = Vec::new();
    for (label, value) in [
        ("depth tier", d.depth_tier),
        ("L/D overhang", d.ld_overhang),
        ("workholding rigidity", d.workholding),
        ("power limit", d.power_limit),
        ("feed cap", d.feed_clamp),
        ("machine safety factor", d.safety_factor),
    ] {
        if (value - 1.0).abs() > UNITY_TOLERANCE {
            moved.push(format!("{label} ×{value:.3}"));
        }
    }
    if moved.is_empty() {
        out.push_str("Nothing derated it.\n");
    } else {
        out.push_str(&format!(
            "Derated ×{combined:.3} ({pct:.0}% reduction) by {}.\n",
            moved.join(", ")
        ));
    }

    // Chip thinning is MEASURED, NOT APPLIED since 2026-08-19
    // (G-CHIPTHIN-HALFFIX). It is reported because the geometric condition
    // is real, and it must not read as one of the multipliers above,
    // because it no longer is one.
    if d.observed_combined_chip_thinning > 1.001 {
        out.push_str(&format!(
            "Chip thinning ×{:.3} is OBSERVED, NOT APPLIED: the chip is thinner \
             per pass at this stepover and DOC, but the vendor chipload column \
             states no radial condition to correct from.\n",
            d.observed_combined_chip_thinning
        ));
    }

    // Where it landed.
    if let Some((lo, hi)) = explain
        .matched_row
        .as_ref()
        .and_then(|r| r.chip_load_min_mm.zip(r.chip_load_max_mm))
        && effective > 0.0
    {
        out.push_str(&format!(
            "Vendor band {lo:.4}–{hi:.4}; this sits at {:.0}% of the maximum.\n",
            (effective / hi) * 100.0
        ));
    }
}

/// The Suggest pass's own account of what it did, routed to its row.
fn append_rationale(out: &mut String, row: RecipeRow, rationale: Option<&SuggestRationale>) {
    let Some(rationale) = rationale else {
        return;
    };
    for entry in &rationale.entries {
        let headline = entry.headline.to_lowercase();
        let mine = if row == RecipeRow::Advance {
            // The catch-all: an entry that names no row is recipe-level.
            !RECIPE_ROWS
                .iter()
                .filter(|candidate| **candidate != RecipeRow::Advance)
                .any(|candidate| {
                    candidate
                        .rationale_keywords()
                        .iter()
                        .any(|k| headline.contains(k))
                })
        } else {
            row.rationale_keywords()
                .iter()
                .any(|k| headline.contains(k))
        };
        if !mine {
            continue;
        }
        out.push_str(&format!("\n• {}", entry.headline));
        if let Some(detail) = &entry.detail {
            out.push_str(&format!("\n  {detail}"));
        }
    }
}

const RECIPE_ROWS: [RecipeRow; 6] = [
    RecipeRow::Rpm,
    RecipeRow::Feed,
    RecipeRow::Plunge,
    RecipeRow::Doc,
    RecipeRow::Woc,
    RecipeRow::Advance,
];

/// Why the DOC and WOC rows are not what the operation will end up with.
const CALCULATOR_CAVEAT: &str = "This is the raw calculator value. `⚡ Apply all` passes it through \
     the invariant funnel, which can lower it — the rigidity cap put 1.2 mm \
     where this row read 4.2 mm on the R03 pocket.";

// ── What stays on the page ───────────────────────────────────────────

/// S2 — engaged-diameter-at-DOC annotation row. Renders just below the
/// context chip for tapered-ball / V-bit tools, where the published tip
/// size badly understates what's actually cutting at depth.
pub(crate) fn draw_engaged_diameter_row(
    ui: &mut egui::Ui,
    current: &CurrentValues,
    explain: &FeedsExplain,
) {
    let Some((doc, engaged, tip_dia, kind)) = engaged_diameter_context(current, explain) else {
        return;
    };
    // Flag when the engaged diameter has hit the shank, or differs from
    // the published tip by more than 50 % — i.e. the cone shoulder is
    // doing most of the work and the tip spec is misleading.
    let flag = engaged >= explain.shank_diameter_mm - 1e-6
        || (tip_dia > 1e-6 && (engaged - tip_dia).abs() / tip_dia > 0.5);
    let icon = if flag { "⚠ " } else { "" };
    let color = if flag {
        theme::WARNING_MILD
    } else {
        theme::TEXT_STRONG
    };
    detail_line(
        ui,
        format!("{icon}Engaged ⌀ {engaged:.2} mm at DOC {doc:.2} mm"),
        color,
        &format!(
            "The published {kind} ⌀ is {tip_dia:.2} mm, but the cone shoulder \
             does most of the cutting at this depth. Every advance/tooth \
             figure on this surface is computed at the engaged diameter, not \
             at the tool tip."
        ),
    );
}

/// S3 — chipload-min warning. For finishing operations a chipload below
/// the vendor band minimum is the common failure mode (operator slows
/// feed for surface quality → rubbing / burning). Surface it loudly.
pub(crate) fn draw_chipload_min_warning(
    ui: &mut egui::Ui,
    current: &CurrentValues,
    explain: &FeedsExplain,
) {
    if current.pass_role != rs_cam_core::feeds::PassRole::Finish {
        return;
    }
    let cl = current.chipload_mm();
    if cl <= 0.0 {
        return;
    }
    let Some((lo, _hi)) = vendor_band(explain) else {
        return;
    };
    if cl >= lo {
        return;
    }
    detail_line(
        ui,
        "⚠ Advance/tooth is below the vendor band minimum",
        theme::WARNING_MILD,
        "The tool rubs instead of cutting, which burns the work and the \
         cutting edge. The feed is too slow, the RPM is too high, or both. \
         Raise the feed or drop the RPM.",
    );
}

/// One warning as a headline and, where it has one, the detail behind it.
fn warning_lines(warning: &rs_cam_core::feeds::FeedsWarning) -> (String, Option<String>) {
    use rs_cam_core::feeds::FeedsWarning;
    match warning {
        FeedsWarning::FeedRateClamped { requested, actual } => (
            format!("Feed clamped: {requested:.0} → {actual:.0} mm/min (machine limit)"),
            None,
        ),
        FeedsWarning::PowerLimited {
            required_kw,
            available_kw,
        } => (
            format!("Power limited: {required_kw:.2} kW needed, {available_kw:.2} kW available"),
            None,
        ),
        FeedsWarning::DocExceedsFlute { requested, capped } => (
            format!("DOC capped: {requested:.1} → {capped:.1} mm (flute guard)"),
            None,
        ),
        FeedsWarning::SlottingDetected { doc_reduced_to } => (
            format!("Slotting detected: DOC reduced to {doc_reduced_to:.1} mm"),
            None,
        ),
        FeedsWarning::ScallopInvalid {
            target,
            max_possible,
        } => (
            format!("Invalid scallop: {target:.3} mm (max {max_possible:.1} mm)"),
            None,
        ),
        FeedsWarning::ShankTooLarge { shank_mm, max_mm } => (
            format!("Shank {shank_mm:.1} mm exceeds max {max_mm:.1} mm"),
            None,
        ),
        // The headline below is asserted verbatim by
        // `inspector_width_is_tab_independent_up4.rs`: it is the real
        // warning that sentry renders to prove the tab holds a 240-point
        // rail. Keep the wording on the FACE — a hover is not painted.
        FeedsWarning::ChiploadClampedToFloor {
            requested,
            floor,
            band_capped_from,
        } => match band_capped_from {
            None => (
                format!(
                    "Commanded advance/tooth below rubbing floor: \
                     {requested:.3} → {floor:.3} mm/tooth"
                ),
                None,
            ),
            Some(global) => (
                format!(
                    "Advance/tooth raised to the vendor band ceiling: \
                     {requested:.3} → {floor:.3} mm/tooth"
                ),
                Some(format!(
                    "The whole vendor band sits below the {global:.3} mm/tooth \
                     rubbing floor, so the ceiling is the best this row can \
                     offer. Expect burnishing."
                )),
            ),
        },
        // Checkpoint K (a4) — the routing refused; there is no vendor
        // row behind any number on this surface.
        FeedsWarning::NoVendorRowsForRoutedOperation {
            operation_kind,
            tool_family,
            missing_rows,
        } => (
            format!("No vendor data for {operation_kind} on a {tool_family} cutter"),
            Some(format!(
                "The recommendation is formula-derived and carries no band. \
                 Missing rows: {missing_rows}."
            )),
        ),
        // Checkpoint K (a3) — RPM-anchor row: the number above it is the
        // empirical formula's, and the nomogram's band has nothing to draw.
        FeedsWarning::VendorRowPublishesNoChipload {
            observation_id,
            formula_chipload_mm,
            floor_band_from,
        } => (
            format!("Vendor row {observation_id} publishes RPM only — no band"),
            Some(match floor_band_from {
                Some(row) => format!(
                    "The {formula_chipload_mm:.4} mm/tooth shown is the \
                     empirical formula's, and this recommendation carries no \
                     vendor band. The rubbing floor came from {row} instead, \
                     which is the chipload-bearing row the post-sim gate also \
                     resolves."
                ),
                None => format!(
                    "The {formula_chipload_mm:.4} mm/tooth shown is the \
                     empirical formula's. This recommendation carries no \
                     vendor band, and no chipload-bearing row matched either."
                ),
            }),
        ),
        FeedsWarning::DrillFeedClampedToEnvelope {
            requested,
            actual,
            envelope_lo,
            envelope_hi,
        } => (
            format!("Drill feed clamped: {requested:.0} → {actual:.0} mm/min"),
            Some(format!(
                "The drilling envelope for this tool and material is \
                 {envelope_lo:.0}–{envelope_hi:.0} mm/min."
            )),
        ),
    }
}

/// The recommendation's warnings, one line each.
///
/// These stay on the page. A warning behind a hover is a warning that was
/// deleted, and this surface exists to stop an operator burning a cutter.
pub(crate) fn draw_warnings(ui: &mut egui::Ui, explain: &FeedsExplain) {
    for warning in &explain.recommended.warnings {
        let (headline, detail) = warning_lines(warning);
        match detail {
            Some(detail) => {
                detail_line(ui, format!("⚠ {headline}"), theme::WARNING_MILD, &detail);
            }
            None => plain_line(ui, format!("⚠ {headline}"), theme::WARNING_MILD),
        }
    }
}
