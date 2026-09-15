//! Why — the optional detail behind one operation's recommendation.
//!
//! Provenance, rationale, the chipload breakdown, the derates and the
//! warnings. The Feeds tab owns the disclosure these render inside.
//!
//! Every quantity here is a **commanded** advance per tooth,
//! `feed / (rpm · flutes)`. These surfaces run before a simulation and have
//! no measured value to show. The achieved figure lives on the properties
//! panel's operating-point card, after a sim (Checkpoint H2, 2026-08-08).
//!
//! # The declutter rule for this file (2026-09-15)
//!
//! The operator's report: the disclosure is "horribly text-heavy". It was.
//! Opened, it drew about twenty-five lines, and most of them were prose
//! captions repeating what the line above already said — a derate row wrote
//! `×0.900` and then wrote a sentence under it, six times over.
//!
//! **A line carries the fact. Its hover carries the explanation.** Every
//! line that has an explanation is marked with [`tokens::GLYPH_DETAIL`], so
//! the mark itself teaches the operator where the detail lives. A caption
//! under a line is now the exception, not the pattern.
//!
//! Two further rules follow from it:
//!
//! - A derate at unity did not change the number. Those rows collapse into
//!   one counted line, with the names on hover.
//! - The arithmetic that PROVES the result is not the result. One line
//!   states the advance per tooth; the three-step derivation is its hover.

use rs_cam_core::feeds::FeedsExplain;
use rs_cam_core::feeds::rationale::{RationaleEntry, SuggestRationale};

use super::shared::{CurrentValues, engaged_diameter_context, vendor_band};
use crate::ui::{theme, tokens};

/// A derate whose factor is within this of 1.0 changed nothing.
const UNITY_TOLERANCE: f64 = 1e-3;

/// One wrapped line, marked and hoverable, carrying its explanation.
///
/// This is the declutter primitive. The caller passes the fact and the
/// explanation; the explanation never takes a line of its own.
fn detail_line(ui: &mut egui::Ui, text: impl AsRef<str>, color: egui::Color32, hover: &str) {
    let marked = format!("{} {}", text.as_ref(), tokens::GLYPH_DETAIL);
    ui.add(egui::Label::new(egui::RichText::new(marked).small().color(color)).wrap())
        .on_hover_text(hover.to_owned());
}

/// One wrapped line with nothing further to say. No mark, no hover.
fn plain_line(ui: &mut egui::Ui, text: impl Into<String>, color: egui::Color32) {
    ui.add(egui::Label::new(egui::RichText::new(text.into()).small().color(color)).wrap());
}

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
    // The attestation that used to follow this row as its own italic line
    // said the same thing this hover says, for the same tools, under the
    // same guard. One of the two was clutter.
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
    ui.add_space(2.0);
    detail_line(
        ui,
        "⚠ Advance/tooth is below the vendor band minimum",
        theme::WARNING_MILD,
        "The tool rubs instead of cutting, which burns the work and the \
         cutting edge. The feed is too slow, the RPM is too high, or both. \
         Raise the feed or drop the RPM.",
    );
}

// compare_row / format_optional / format_delta / draw_power_bar / power_color
// / draw_mrr_row were lifted to `ui::components::compare` (CL 4/4) so the
// optimizer rollup and the Feeds Details drawer share one implementation.

// ── Provenance ──────────────────────────────────────────────────────

/// Provenance content for the inspector's single Why disclosure.
///
/// This is deliberately a body renderer: the outer disclosure belongs to the
/// Feeds tab, so the old independent "How is this calculated?" button would
/// create a second, competing expansion state.
///
/// Six labelled rows in a group frame became at most three lines. `Row` and
/// `Vendor` name one thing and now share a line; `Calibrated for` is the
/// hover on that line; the band and the RPM range share the next; and
/// `Scaling` only appears when there IS scaling — "none (direct match)" is
/// the default case and states nothing.
pub(crate) fn draw_provenance(ui: &mut egui::Ui, explain: &FeedsExplain) {
    let Some(row) = &explain.matched_row else {
        detail_line(
            ui,
            "No vendor row matched — formula-derived",
            theme::WARNING_MILD,
            "No vendor lookup-table row matched this tool, material and \
             operation, so the recommendation comes from the empirical \
             formula. Re-check it against vendor data before you cut.",
        );
        return;
    };

    detail_line(
        ui,
        format!("{} · {}", row.source_vendor, row.observation_id),
        tokens::TEXT_BODY,
        &format!(
            "The vendor row behind this recommendation. It is calibrated for \
             a {:.2} mm tool at {} flute.",
            row.row_diameter_mm, explain.flute_count
        ),
    );

    let band = row
        .chip_load_min_mm
        .zip(row.chip_load_max_mm)
        .map(|(lo, hi)| format!("band {lo:.4}–{hi:.4} mm/tooth"));
    let rpm = match row.rpm_min.zip(row.rpm_max) {
        Some((lo, hi)) => Some(format!("RPM {lo:.0}–{hi:.0}")),
        None => row.rpm_nominal.map(|nom| format!("RPM {nom:.0} nominal")),
    };
    let published: Vec<String> = band.into_iter().chain(rpm).collect();
    if !published.is_empty() {
        detail_line(
            ui,
            published.join(" · "),
            tokens::TEXT_BODY,
            "What the vendor row publishes, after it was scaled to this tool \
             and this material. The recommendation sits inside these.",
        );
    }

    let scaled = (row.chipload_diameter_scale - 1.0).abs() >= UNITY_TOLERANCE
        || (row.chipload_hardness_scale - 1.0).abs() >= UNITY_TOLERANCE;
    if scaled {
        let approximate = if row.is_extrapolated {
            " (approximate)"
        } else {
            ""
        };
        let color = if row.is_extrapolated {
            theme::WARNING_MILD
        } else {
            theme::TEXT_DIM
        };
        detail_line(
            ui,
            format!(
                "scaled ×{:.2} diameter · ×{:.2} hardness{approximate}",
                row.chipload_diameter_scale, row.chipload_hardness_scale
            ),
            color,
            "The vendor row was measured on a different tool diameter or a \
             different material hardness, so its chipload was scaled to \
             yours. An approximate scaling reached outside the measured \
             range.",
        );
    }
}

// ── Warnings ────────────────────────────────────────────────────────

/// One warning as a headline and, where it has one, the detail behind it.
///
/// Four of these warnings used to ship as paragraphs. A paragraph in a list
/// is unreadable as a list, and the operator needs to SCAN the warnings
/// before reading any one of them.
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
        // Checkpoint K (a3) — RPM-anchor row: the number above it in
        // this surface is the empirical formula's, and the nomogram's
        // band chart has nothing to draw for it.
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
/// The "Warnings" heading is deleted: every line already carries ⚠ in the
/// caution colour, so the heading was a third channel saying what two
/// channels had said.
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

// ── Suggest rationale ────────────────────────────────────────────────

/// Render the rationale tree emitted by the canonical Suggest pass.
///
/// The "Why these values?" heading is deleted: the disclosure this renders
/// inside is titled "Why is the recommendation here?", so the heading
/// answered a question its own container had already asked.
pub(crate) fn draw_rationale(ui: &mut egui::Ui, rationale: &SuggestRationale) {
    for entry in &rationale.entries {
        draw_rationale_entry(ui, entry);
    }
}

fn draw_rationale_entry(ui: &mut egui::Ui, entry: &RationaleEntry) {
    let headline = format!("• {}", entry.headline);
    match &entry.detail {
        Some(detail) => detail_line(ui, headline, theme::TEXT_STRONG, detail),
        None => plain_line(ui, headline, theme::TEXT_STRONG),
    }
}

// ────────────────────────────────────────────────────────────────────
// Chipload-math breakdown
// ────────────────────────────────────────────────────────────────────

/// Render the chipload → feed pipeline so the operator can see *why* the
/// recommended diamond sits where it does on the nomogram.
///
/// Three lines in the common case: the target, what moved it, and the
/// result. The formula substitution, the derate notes and the three-step
/// proof of the result all live on hovers.
pub(crate) fn draw_chipload_breakdown(ui: &mut egui::Ui, explain: &FeedsExplain) {
    let flutes = explain.flute_count.max(1) as f64;
    let effective = if explain.recommended.rpm > 0.0 {
        explain.recommended.feed_rate_mm_min / (explain.recommended.rpm * flutes)
    } else {
        0.0
    };

    draw_target_line(ui, explain);
    draw_derate_lines(ui, explain);
    draw_result_line(ui, explain, effective);
}

/// Step 1 — where the target advance per tooth came from.
fn draw_target_line(ui: &mut egui::Ui, explain: &FeedsExplain) {
    let d = &explain.recommended.derates;
    match (&explain.recommended.chipload_source, &d.formula) {
        (rs_cam_core::feeds::ChiploadSource::VendorLut { observation_id }, _) => {
            detail_line(
                ui,
                format!(
                    "Target {:.4} mm/tooth (vendor midpoint)",
                    d.target_chip_load_mm
                ),
                tokens::TEXT_BODY,
                &format!("The midpoint of vendor row {observation_id}."),
            );
        }
        (rs_cam_core::feeds::ChiploadSource::FormulaFallback, Some(f))
        | (rs_cam_core::feeds::ChiploadSource::EdgeRadiusFloor, Some(f)) => {
            let mut hover =
                String::from("No vendor row matched, so the empirical formula set the target.\n");
            hover.push_str("fz = K₀ · D^p · (1/H)^q\n");
            hover.push_str(&format!(
                "= {:.4} · {:.2}^{:.2} · (1/{:.1})^{:.2}\n",
                f.k0, f.diameter_mm, f.p, f.feed_scale_factor, f.q
            ));
            hover.push_str(&format!("= {:.4} mm/tooth\n", f.result_mm_tooth));
            hover.push_str(
                "K₀, p and q come from MachineProfile.chip_load. D is the tool \
                 diameter. H is the material hardness index.",
            );
            detail_line(
                ui,
                format!(
                    "Target {:.4} mm/tooth (empirical formula)",
                    f.result_mm_tooth
                ),
                theme::WARNING_MILD,
                &hover,
            );
        }
        _ => {
            detail_line(
                ui,
                format!("Target {:.4} mm/tooth", d.target_chip_load_mm),
                tokens::TEXT_BODY,
                "The starting advance per tooth, before the factors below.",
            );
        }
    }
}

/// Step 2 — the factors, with the ones that changed nothing counted rather
/// than listed.
fn draw_derate_lines(ui: &mut egui::Ui, explain: &FeedsExplain) {
    let d = &explain.recommended.derates;

    // Chip thinning is MEASURED, NOT APPLIED since 2026-08-19
    // (G-CHIPTHIN-HALFFIX). It is still shown, because the geometric
    // condition is real and an operator should see it — but it must not
    // read as one of the multipliers that produced the feed, because it no
    // longer is one. It therefore keeps its own line ABOVE the derates, and
    // "NOT applied" stays on the face where a reader would cite it.
    if d.observed_combined_chip_thinning > 1.001 {
        detail_line(
            ui,
            format!(
                "chip-thinning ×{:.3} (observed, NOT applied)",
                d.observed_combined_chip_thinning
            ),
            theme::TEXT_DIM,
            "The chip is thinner per pass at this stepover and DOC. This is \
             reported only: the vendor chipload column states no radial \
             condition to correct from, so nothing multiplies the feed by it.",
        );
    }

    let mut applied: Vec<(&str, f64, &str)> = Vec::new();
    let mut unity: Vec<&str> = Vec::new();
    let mut record = |label: &'static str, value: f64, note: &'static str| {
        if (value - 1.0).abs() > UNITY_TOLERANCE {
            applied.push((label, value, note));
        } else {
            unity.push(label);
        }
    };

    record(
        "depth-tier feed derate",
        d.depth_tier,
        "A deep cut runs a slower feed, to limit deflection.",
    );
    record(
        "L/D overhang",
        d.ld_overhang,
        "A long tool backs off, to limit deflection.",
    );
    record(
        "workholding rigidity",
        d.workholding,
        "Low rigidity (tape or vacuum) backs off. High rigidity (a vise or \
         bolted work) pushes up.",
    );
    if d.power_limit < 0.999 {
        record(
            "power limit",
            d.power_limit,
            "The spindle cannot deliver more power, so the feed is reduced.",
        );
    }
    if d.feed_clamp < 0.999 {
        record(
            "feed-cap clamp",
            d.feed_clamp,
            "The feed hit machine.max_feed_mm_min.",
        );
    }
    record(
        "machine safety factor",
        d.safety_factor,
        "Extra margin, so the recommendation is comfortably safe.",
    );
    // Spindle speedup is the only ≥ 1.0 factor in the chain. It walks the
    // constant-chipload line up the speed axis when
    // `SpindleStrategy::MaxSpeed` is on.
    record(
        "spindle speedup",
        d.spindle_speedup,
        "The MaxSpeed policy lifts RPM toward the spindle ceiling and scales \
         the feed to hold the commanded advance/tooth constant.",
    );

    for (label, value, note) in applied {
        derate_line(ui, label, value, note);
    }
    if !unity.is_empty() {
        detail_line(
            ui,
            format!("{} factors at unity", unity.len()),
            theme::TEXT_FAINT,
            &format!("These changed nothing: {}.", unity.join(", ")),
        );
    }
}

/// Step 3 — the result, with its derivation on hover.
fn draw_result_line(ui: &mut egui::Ui, explain: &FeedsExplain, effective: f64) {
    let d = &explain.recommended.derates;
    let combined = d.combined_factor();

    let derate_pct = ((1.0 - combined) * 100.0).max(0.0);
    let mut proof = format!(
        "= target {:.4} × {:.3} combined derate ({derate_pct:.0}% reduction)\n",
        d.target_chip_load_mm, combined
    );
    proof.push_str(&format!(
        "= feed {:.0} mm/min ÷ ({:.0} RPM × {} flutes)",
        explain.recommended.feed_rate_mm_min, explain.recommended.rpm, explain.flute_count
    ));
    if let Some((_, hi)) = explain
        .matched_row
        .as_ref()
        .and_then(|r| r.chip_load_min_mm.zip(r.chip_load_max_mm))
        && effective > 0.0
    {
        let pct_of_max = (effective / hi) * 100.0;
        proof.push_str(&format!(
            "\n= {pct_of_max:.0}% of the vendor band maximum ({hi:.4} mm/tooth)"
        ));
    }
    ui.add_space(2.0);
    let text = format!(
        "Advance/tooth {effective:.4} mm/tooth {}",
        tokens::GLYPH_DETAIL
    );
    ui.add(
        egui::Label::new(
            egui::RichText::new(text)
                .small()
                .strong()
                .color(theme::SUCCESS),
        )
        .wrap(),
    )
    .on_hover_text(proof);
}

/// One factor as `label ×0.850`. The note that used to sit under it is now
/// its hover — six captions were the bulk of this disclosure's text.
fn derate_line(ui: &mut egui::Ui, label: &str, value: f64, note: &str) {
    let color = if value < 0.999 {
        theme::WARNING_MILD
    } else if value > 1.001 {
        theme::SUCCESS
    } else {
        theme::TEXT_DIM
    };
    let text = format!("{label} ×{value:.3} {}", tokens::GLYPH_DETAIL);
    ui.add(egui::Label::new(egui::RichText::new(text).small().color(color)).wrap())
        .on_hover_text(note.to_owned());
}
