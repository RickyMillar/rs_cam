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

use rs_cam_core::feeds::rationale::{
    AGGRESSIVENESS_ABOVE_BASE_TEXT, PLUNGE_AT_MATERIAL_BASE_TEXT, SuggestRationale,
};
use rs_cam_core::feeds::suggest::{AggressivenessShortfall, FeedRecalibrationCap, SuggestWarning};
use rs_cam_core::feeds::{FeedsExplain, SpindleScaleReason};

use super::shared::{CurrentValues, engaged_diameter_context, vendor_band, vendor_single_value};
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
    // The sentence comes from the RECORDED reason, never from the factor.
    // Until 2026-09-16 this read the factor alone and said "MaxSpeed lifted
    // it" for anything that was not 1.0 — which was true only while MaxSpeed
    // was the one thing that could move it. The power ladder walks the same
    // line downward, so a factor-only reading would name the wrong cause.
    let scale = explain.recommended.derates.spindle_scale;
    let reason = explain.recommended.derates.spindle_scale_reason;
    match reason {
        SpindleScaleReason::MaxSpeedPolicy => out.push_str(&format!(
            "Spindle policy MaxSpeed lifted it ×{scale:.3} toward the spindle \
             ceiling, and scaled the feed with it to hold the advance/tooth \
             constant.\n"
        )),
        SpindleScaleReason::PowerLimit => out.push_str(&format!(
            "The cut was over the spindle's power budget, so the RPM came DOWN \
             ×{scale:.3} and the feed came down with it. The advance per tooth \
             is unchanged — the cut is the same shape, just slower.\n"
        )),
        SpindleScaleReason::FeedCeiling => out.push_str(&format!(
            "The feed hit the machine's cutting-feed ceiling, so the RPM came \
             DOWN ×{scale:.3} and the feed came down with it. The advance per \
             tooth stays at the band value (ruling R4 Q10).\n"
        )),
        SpindleScaleReason::Unchanged => {
            if (scale - 1.0).abs() > UNITY_TOLERANCE {
                // A factor that moved with no reason recorded is a bug in the
                // calculator, not something to narrate over.
                out.push_str(&format!(
                    "The RPM was scaled ×{scale:.3} and the engine did not \
                     record why. Treat this recommendation as unexplained.\n"
                ));
            } else {
                out.push_str("Spindle policy is Match chart, so the RPM follows the vendor row.\n");
            }
        }
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
        "The plunge is the material's base rate for the tool diameter: a \
         small cutter plunges slower than a large one.\n",
    );
    // Ruling R4 Q5 (2026-09-24): the dial has no lever on a plunge.
    out.push_str(PLUNGE_AT_MATERIAL_BASE_TEXT);
    out.push('\n');
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
    // Ruling R4 (2026-09-24): the machine safety factor is gone, and the
    // long-tool share is a load target, not a feed factor (Q7). Neither is
    // in this list; the long-tool line and the aggressiveness line on the
    // card state them. The workholding factor is gone (Q8).
    for (label, value) in [
        ("depth tier", d.depth_tier),
        ("power limit", d.power_limit),
        ("feed cap", d.feed_clamp),
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

    if d.ld_overhang < 1.0 - UNITY_TOLERANCE {
        out.push_str(&format!(
            "The long-tool share ×{:.2} does not change the feed. It lowers the \
             aggressiveness load target; the depth and the stepover carry it.\n",
            d.ld_overhang
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

    // Where it landed. A band only when the row publishes both limits
    // (G-CHARTLINES); a one-value row names its one value and claims no band.
    if effective > 0.0 {
        if let Some((lo, hi)) = vendor_band(explain) {
            out.push_str(&format!(
                "Vendor band {lo:.4}–{hi:.4}; this sits at {:.0}% of the maximum.\n",
                (effective / hi) * 100.0
            ));
        } else if let Some(value) = vendor_single_value(explain) {
            out.push_str(&format!(
                "Vendor value {value:.4} (the row publishes one value); this sits \
                 at {:.0}% of it.\n",
                (effective / value) * 100.0
            ));
        }
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
///
/// The warning needs a PUBLISHED minimum. A max-only row or a one-point row
/// has none, so this function draws nothing for it. Until 2026-09-23 it
/// compared against an invented `0.7 × max` floor (G-CHARTLINES).
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
        FeedsWarning::PowerLadderReducedCut {
            rpm_from,
            rpm_to,
            axial_from,
            axial_to,
            radial_from,
            radial_to,
            feed_factor,
            required_kw_before,
            required_kw_after,
            available_kw,
        } => {
            let mut moved: Vec<String> = Vec::new();
            if let (Some(a), Some(b)) = (rpm_from, rpm_to) {
                moved.push(format!("RPM {a:.0} → {b:.0}"));
            }
            if let (Some(a), Some(b)) = (axial_from, axial_to) {
                moved.push(format!("depth {a:.2} → {b:.2} mm"));
            }
            if let (Some(a), Some(b)) = (radial_from, radial_to) {
                moved.push(format!("width {a:.2} → {b:.2} mm"));
            }
            if let Some(f) = feed_factor {
                moved.push(format!("feed ×{f:.3} (last resort)"));
            }
            let cleared = required_kw_after <= available_kw;
            (
                format!(
                    "Cut reduced to fit spindle power: {}{}",
                    moved.join(", "),
                    if cleared { "" } else { " — still over" }
                ),
                Some(format!(
                    "The spindle could not turn the cut you asked for. The engine made the \
                     cut SMALLER rather than slower: {required_kw_before:.2} kW → \
                     {required_kw_after:.2} kW against {available_kw:.2} kW available.\n\
                     {}{}",
                    if feed_factor.is_some() {
                        "The chip was thinned as a LAST resort, after the RPM and the cut \
                         size had already been reduced. The ploughing part of the load \
                         carries no feed term, so a slower feed sheds only part of it — \
                         which is why it is the last dial tried, not the first."
                    } else {
                        "The advance per tooth is unchanged. The cut got smaller, not \
                         thinner."
                    },
                    if cleared {
                        ""
                    } else {
                        "\nIt is STILL over budget. Take a shallower or narrower cut, or \
                         use a machine with more spindle power."
                    }
                )),
            )
        }
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
        //
        // Ruling R4 WP2a (2026-09-23): the engine does not raise the feed.
        // The face line names the consequence and the two levers, because
        // the operator now owns the fix.
        FeedsWarning::ChiploadBelowRubbingFloor {
            commanded,
            floor,
            source,
            ..
        } => (
            format!(
                "Advance per tooth below the rubbing floor: {commanded:.4} mm/tooth \
                 (floor {floor:.4}, {}). The feed is not raised. The tool can \
                 rub and burn the work: raise the feed or lower the RPM.",
                source.describe()
            ),
            None,
        ),
        // Ruling R4 WP1 (2026-09-23): the long-tool de-rate is a repo rule
        // with no source. It fires on most default tools (stickout 45 mm), so
        // it is one short line with the numbers, on the face. Since ruling R4
        // Q7 (2026-09-24) it scales the load target, not the feed.
        FeedsWarning::LongToolDerate {
            stickout_mm,
            diameter_mm,
            ratio,
            factor,
        } => (
            format!(
                "Long tool: load target ×{factor:.2} (stickout {stickout_mm:.0} mm is \
                 {ratio:.1} × Ø{diameter_mm} mm; repo rule, unsourced)"
            ),
            None,
        ),
        // Ruling R4 Q10 (2026-09-24): the RPM follows the feed ceiling down
        // to hold the chip. Core owns the one text.
        FeedsWarning::RpmLoweredForFeedCeiling {
            rpm_from,
            rpm_to,
            feed_ceiling_mm_min,
            rpm_floor,
            floor_source,
            held,
        } => (
            rs_cam_core::feeds::rpm_lowered_text(
                *rpm_from,
                *rpm_to,
                *feed_ceiling_mm_min,
                *rpm_floor,
                *floor_source,
                *held,
            ),
            None,
        ),
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

// ── The Suggest stages that move a number ────────────────────────────

/// The face line of one Suggest record, and whether it is a Caution.
///
/// The operator's standing rule (ruling R4, 2026-09-24): every stage that
/// moves a number is one line on the card, with its source status. The
/// match is exhaustive, so a new record must choose here. A record that
/// returns `None` either moves no number, or its row hover carries it (the
/// declutter ruling of 2026-09-15); the sentry
/// `every_stage_that_moves_a_number_is_on_the_card_g_visible` holds the
/// list.
fn suggest_line(warning: &SuggestWarning) -> Option<(String, bool)> {
    match warning {
        SuggestWarning::EngagementReducedForAggressiveness {
            aggressiveness,
            ld_factor,
            target_share,
            dpp_from,
            dpp_to,
            stepover_from,
            stepover_to,
            force_n_before,
            force_n_after,
            power_kw_before,
            power_kw_after,
            section_mm2_before,
            section_mm2_after,
            target_met,
            shortfall,
            applied,
            ..
        } => {
            let pct = target_share * 100.0;
            let head = if (ld_factor - 1.0).abs() > UNITY_TOLERANCE {
                format!(
                    "Aggressiveness {aggressiveness:.2} (× {ld_factor:.2} long tool = {pct:.0} %)"
                )
            } else {
                format!("Aggressiveness {aggressiveness:.2} (= {pct:.0} %)")
            };
            let mut cut = Vec::new();
            if let (Some(a), Some(b)) = (dpp_from, dpp_to) {
                cut.push(format!("depth {a:.2} → {b:.2} mm"));
            }
            if let (Some(a), Some(b)) = (stepover_from, stepover_to) {
                cut.push(format!("stepover {a:.2} → {b:.2} mm"));
            }
            let mut load = Vec::new();
            if let (Some(a), Some(b)) = (force_n_before, force_n_after) {
                load.push(format!("force {a:.0} → {b:.0} N"));
            }
            if let (Some(a), Some(b)) = (power_kw_before, power_kw_after) {
                load.push(format!("power {a:.2} → {b:.2} kW"));
            }
            if let (Some(a), Some(b)) = (section_mm2_before, section_mm2_after) {
                load.push(format!(
                    "chip section {a:.2} → {b:.2} mm² (proxy, no primary-source Kc)"
                ));
            }
            let mut line = head;
            if !cut.is_empty() {
                line.push_str(&format!(": {}", cut.join(", ")));
            }
            if !load.is_empty() {
                line.push_str(&format!("; {}", load.join(", ")));
            }
            let above_base = *target_share > 1.0;
            if above_base {
                line.push_str(&format!(". Caution, {AGGRESSIVENESS_ABOVE_BASE_TEXT}"));
            }
            if !target_met {
                let reason = match shortfall {
                    Some(AggressivenessShortfall::LeverFloor) | None => {
                        "the depth and the stepover are at their floors; the feed is not cut"
                    }
                    Some(AggressivenessShortfall::EngagementCap) => {
                        "the depth and the stepover are at their caps"
                    }
                    Some(AggressivenessShortfall::NoLever) => {
                        "this operation has no depth or stepover to change"
                    }
                };
                line.push_str(&format!("; target not met: {reason}"));
            }
            if !applied {
                line.push_str(&format!(
                    ". Apply the cut geometry to hold the load at {pct:.0} %"
                ));
            }
            Some((line, above_base || !target_met))
        }
        SuggestWarning::AggressivenessNotApplied {
            aggressiveness,
            reason,
        } => Some((
            format!("Aggressiveness {aggressiveness:.2}: {}", reason.card_text()),
            false,
        )),
        SuggestWarning::FeedRescaledToFinalGeometry {
            requested_mm_per_min,
            rescaled_mm_per_min,
            factor_at_calculator,
            factor_at_final,
            cap_hit,
        } => Some((
            format!(
                "Feed re-derived at the final depth: {requested_mm_per_min:.0} → \
                 {rescaled_mm_per_min:.0} mm/min (depth ladder ×{factor_at_calculator:.2} → \
                 ×{factor_at_final:.2}, published charts){}",
                match cap_hit {
                    Some(FeedRecalibrationCap::MaxFeed) => "; held at the machine feed ceiling",
                    Some(FeedRecalibrationCap::DeflectionThreshold) => {
                        "; held by the deflection budget"
                    }
                    None => "",
                }
            ),
            cap_hit.is_some(),
        )),
        // Ruling R4 Q10: this record mirrors the calculator's
        // `FeedsWarning::RpmLoweredForFeedCeiling`, which `draw_warnings`
        // paints on the face with the same core text. A second face line
        // would repeat it; the rationale row carries it on the RPM hover.
        SuggestWarning::RpmLoweredForFeedCeiling { .. } => None,
        // These records reach the card through the rationale rows, on the
        // hover of the row whose number they move (`append_rationale`), or
        // they move no number.
        SuggestWarning::PlungeClampedToFeed { .. }
        | SuggestWarning::StepoverClampedToToolDiameter { .. }
        | SuggestWarning::RoughingDepthClampedToRigidity { .. }
        | SuggestWarning::DepthClampedToCuttingLength { .. }
        | SuggestWarning::PlungeEntryUnstableAtDpp { .. }
        | SuggestWarning::DppCappedByDeflection { .. }
        | SuggestWarning::DeflectionBackoffUnmodeled { .. }
        | SuggestWarning::DeflectionBackoffFigureIsAFloor { .. }
        | SuggestWarning::StepoverRaisedForRuntime { .. }
        | SuggestWarning::FeedRaisedForChipload { .. }
        | SuggestWarning::ChiploadStillLowAfterRecalibration { .. }
        | SuggestWarning::StrategyRewrote { .. }
        | SuggestWarning::StrategyRecommendedNotApplied { .. }
        | SuggestWarning::AxialEnvelopeSafeBandEmpty { .. }
        | SuggestWarning::AxialDocClampedByEnvelope { .. }
        | SuggestWarning::AxialDocBelowBurnFloor { .. }
        | SuggestWarning::ProjectCurveDepthInfeasible { .. }
        | SuggestWarning::FinishEnvelopeAdvisory { .. }
        | SuggestWarning::FeedClampedToChiploadFloor { .. }
        | SuggestWarning::PowerRecheckedAfterRescale { .. }
        | SuggestWarning::CutGeometryFieldNotHeld { .. } => None,
    }
}

/// The Suggest stages that move a number, one line each, on the face.
///
/// Ruling R4 (2026-09-24): the aggressiveness dial and the feed re-derive
/// at the final depth change the recipe. A hover is not enough for them.
/// A line that states a Caution (a target above the base, or a target not
/// met) carries the warning mark and the Caution tone.
pub(crate) fn draw_suggest_lines(ui: &mut egui::Ui, warnings: &[SuggestWarning]) {
    for warning in warnings {
        let Some((line, caution)) = suggest_line(warning) else {
            continue;
        };
        if caution {
            plain_line(ui, format!("⚠ {line}"), theme::WARNING_MILD);
        } else {
            plain_line(ui, line, tokens::TEXT_MUTED);
        }
    }
}
