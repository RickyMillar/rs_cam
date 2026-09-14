//! Why — the optional detail behind one operation's recommendation.
//!
//! Provenance, rationale, the chipload breakdown, the derates and the
//! warnings. DC5a's plan makes this an EXPAND from the Feeds tab rather than
//! a window, because optional detail is what a disclosure is for (Rule A).
//! That move is a later package; this file is the split only.
//!
//! Every quantity here is a **commanded** advance per tooth,
//! `feed / (rpm · flutes)`. These surfaces run before a simulation and have
//! no measured value to show. The achieved figure lives on the properties
//! panel's operating-point card, after a sim (Checkpoint H2, 2026-08-08).

use rs_cam_core::feeds::FeedsExplain;
use rs_cam_core::feeds::rationale::{RationaleEntry, SuggestRationale};

use super::shared::{CurrentValues, engaged_diameter_context, vendor_band};
use crate::ui::{AppEvent, theme};
use crate::ui_command::{NoArgs, UiCommand};

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
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Engaged ⌀:")
                .small()
                .color(theme::TEXT_DIM),
        );
        let icon = if flag { "⚠ " } else { "" };
        ui.label(
            egui::RichText::new(format!(
                "{icon}{engaged:.2} mm at DOC {doc:.2} mm (vs {kind} ⌀ {tip_dia:.2} mm)"
            ))
            .small()
            .color(if flag {
                theme::WARNING_MILD
            } else {
                theme::TEXT_STRONG
            }),
        )
        .on_hover_text(
            "The cone shoulder does most of the cutting at depth. Advance/tooth \
             bounds in this modal apply to this engaged diameter, computed \
             at this DOC — not the published tool tip size.",
        );
    });
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
    ui.label(
        egui::RichText::new(
            "⚠ Commanded advance/tooth below the vendor band minimum — risk of \
             rubbing or burning. Feed too slow, RPM too high, or both. Raise feed \
             or drop RPM.",
        )
        .small()
        .color(theme::WARNING_MILD),
    );
}

/// S4 — engaged-diameter chipload attestation. One italic line, for
/// tapered-ball / V-bit tools, telling the operator the chipload number
/// is computed at the engaged diameter (the correct thing) rather than
/// the tool tip — so they can trust it.
pub(crate) fn draw_chipload_engaged_attestation(
    ui: &mut egui::Ui,
    current: &CurrentValues,
    explain: &FeedsExplain,
) {
    let Some((doc, engaged, _tip, _kind)) = engaged_diameter_context(current, explain) else {
        return;
    };
    ui.label(
        egui::RichText::new(format!(
            "Advance/tooth computed at engaged diameter ⌀ {engaged:.2} mm (DOC \
             {doc:.2} mm), not the tool tip."
        ))
        .small()
        .italics()
        .color(theme::TEXT_DIM),
    );
}

// compare_row / format_optional / format_delta / draw_power_bar / power_color
// / draw_mrr_row were lifted to `ui::components::compare` (CL 4/4) so the
// optimizer rollup and the Feeds Details drawer share one implementation.

// ── Provenance disclosure ───────────────────────────────────────────

pub(crate) fn draw_provenance_disclosure(
    ui: &mut egui::Ui,
    explain: &FeedsExplain,
    show: bool,
    events: &mut Vec<AppEvent>,
) {
    ui.horizontal(|ui| {
        let arrow = if show { "▼" } else { "▶" };
        if ui
            .small_button(format!("{arrow} How is this calculated?"))
            .clicked()
        {
            events.push(AppEvent::Ui(UiCommand::ToggleFeedsProvenance(NoArgs)));
        }
    });
    if !show {
        return;
    }
    egui::Frame::group(ui.style()).show(ui, |ui| match &explain.matched_row {
        Some(row) => {
            egui::Grid::new("feeds_modal_prov")
                .num_columns(2)
                .spacing([crate::ui::tokens::SPACE_3, crate::ui::tokens::SPACE_2])
                .min_row_height(crate::ui::tokens::ROW_DENSE)
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Row").small().color(theme::TEXT_DIM));
                    ui.label(egui::RichText::new(&row.observation_id).small());
                    ui.end_row();
                    ui.label(egui::RichText::new("Vendor").small().color(theme::TEXT_DIM));
                    ui.label(egui::RichText::new(row.source_vendor.to_string()).small());
                    ui.end_row();
                    ui.label(
                        egui::RichText::new("Calibrated for")
                            .small()
                            .color(theme::TEXT_DIM),
                    );
                    ui.label(
                        egui::RichText::new(format!(
                            "{:.2} mm tool, {} flute",
                            row.row_diameter_mm, explain.flute_count
                        ))
                        .small(),
                    );
                    ui.end_row();
                    ui.label(
                        egui::RichText::new("Scaling")
                            .small()
                            .color(theme::TEXT_DIM),
                    );
                    let scaling_color = if row.is_extrapolated {
                        theme::WARNING_MILD
                    } else {
                        theme::SUCCESS
                    };
                    let scaling_label = if (row.chipload_diameter_scale - 1.0).abs() < 1e-3
                        && (row.chipload_hardness_scale - 1.0).abs() < 1e-3
                    {
                        "none (direct match)".to_owned()
                    } else {
                        format!(
                            "diameter ×{:.2} · hardness ×{:.2}{}",
                            row.chipload_diameter_scale,
                            row.chipload_hardness_scale,
                            if row.is_extrapolated {
                                " (approximate)"
                            } else {
                                ""
                            }
                        )
                    };
                    ui.label(
                        egui::RichText::new(scaling_label)
                            .small()
                            .color(scaling_color),
                    );
                    ui.end_row();
                    if let (Some(min), Some(max)) = (row.chip_load_min_mm, row.chip_load_max_mm) {
                        ui.label(
                            egui::RichText::new("Scaled band")
                                .small()
                                .color(theme::TEXT_DIM),
                        );
                        ui.label(
                            egui::RichText::new(format!("{min:.4}–{max:.4} mm/tooth")).small(),
                        );
                        ui.end_row();
                    }
                    if let (Some(lo), Some(hi)) = (row.rpm_min, row.rpm_max) {
                        ui.label(
                            egui::RichText::new("Vendor RPM range")
                                .small()
                                .color(theme::TEXT_DIM),
                        );
                        ui.label(egui::RichText::new(format!("{lo:.0}–{hi:.0}")).small());
                        ui.end_row();
                    } else if let Some(nom) = row.rpm_nominal {
                        ui.label(
                            egui::RichText::new("Vendor RPM (nominal)")
                                .small()
                                .color(theme::TEXT_DIM),
                        );
                        ui.label(egui::RichText::new(format!("{nom:.0}")).small());
                        ui.end_row();
                    }
                });
        }
        None => {
            ui.label(
                egui::RichText::new(
                    "No vendor LUT row matched. Recommendation is from the \
                     empirical formula. Re-check against vendor data before use.",
                )
                .small()
                .color(theme::WARNING_MILD),
            );
        }
    });
}

// ── Warnings ────────────────────────────────────────────────────────

pub(crate) fn draw_warnings(ui: &mut egui::Ui, explain: &FeedsExplain) {
    if explain.recommended.warnings.is_empty() {
        return;
    }
    ui.label(
        egui::RichText::new("Warnings")
            .small()
            .strong()
            .color(theme::WARNING_MILD),
    );
    for w in &explain.recommended.warnings {
        let text = match w {
            rs_cam_core::feeds::FeedsWarning::FeedRateClamped { requested, actual } => {
                format!("Feed clamped: {requested:.0} → {actual:.0} mm/min (machine limit)")
            }
            rs_cam_core::feeds::FeedsWarning::PowerLimited {
                required_kw,
                available_kw,
            } => {
                format!("Power limited: {required_kw:.2} kW needed, {available_kw:.2} kW available")
            }
            rs_cam_core::feeds::FeedsWarning::DocExceedsFlute { requested, capped } => {
                format!("DOC capped: {requested:.1} → {capped:.1} mm (flute guard)")
            }
            rs_cam_core::feeds::FeedsWarning::SlottingDetected { doc_reduced_to } => {
                format!("Slotting detected: DOC reduced to {doc_reduced_to:.1} mm")
            }
            rs_cam_core::feeds::FeedsWarning::ScallopInvalid {
                target,
                max_possible,
            } => format!("Invalid scallop: {target:.3} mm (max {max_possible:.1} mm)"),
            rs_cam_core::feeds::FeedsWarning::ShankTooLarge { shank_mm, max_mm } => {
                format!("Shank {shank_mm:.1} mm exceeds max {max_mm:.1} mm")
            }
            rs_cam_core::feeds::FeedsWarning::ChiploadClampedToFloor {
                requested,
                floor,
                band_capped_from,
            } => match band_capped_from {
                None => {
                    format!(
                        "Commanded advance/tooth below rubbing floor: \
                         {requested:.3} → {floor:.3} mm/tooth"
                    )
                }
                Some(global) => format!(
                    "Commanded advance/tooth raised to vendor band ceiling: \
                     {requested:.3} → {floor:.3} mm/tooth \
                     (whole band is below the {global:.3} rubbing floor — expect burnishing)"
                ),
            },
            // Checkpoint K (a3) — RPM-anchor row: the number above it in
            // this modal is the empirical formula's, and the modal's band
            // chart has nothing to draw for it.
            // Checkpoint K (a4) — the routing refused; there is no vendor
            // row behind any number on this surface.
            rs_cam_core::feeds::FeedsWarning::NoVendorRowsForRoutedOperation {
                operation_kind,
                tool_family,
                missing_rows,
            } => format!(
                "No vendor data for {operation_kind} on a {tool_family} cutter \
                 — formula-derived, no band ({missing_rows})"
            ),
            rs_cam_core::feeds::FeedsWarning::VendorRowPublishesNoChipload {
                observation_id,
                formula_chipload_mm,
                floor_band_from,
            } => match floor_band_from {
                Some(row) => format!(
                    "Vendor row {observation_id} publishes RPM only — the \
                     {formula_chipload_mm:.4} mm/tooth shown is the empirical formula's, \
                     and this recommendation carries no vendor band. The rubbing floor \
                     was taken from {row} instead, which is the chipload-bearing row the \
                     post-sim gate also resolves"
                ),
                None => format!(
                    "Vendor row {observation_id} publishes RPM only — the \
                     {formula_chipload_mm:.4} mm/tooth shown is the empirical formula's, \
                     and this recommendation carries no vendor band, and no \
                     chipload-bearing row matched either"
                ),
            },
            rs_cam_core::feeds::FeedsWarning::DrillFeedClampedToEnvelope {
                requested,
                actual,
                envelope_lo,
                envelope_hi,
            } => format!(
                "Drill feed clamped: {requested:.0} → {actual:.0} mm/min (envelope {envelope_lo:.0}–{envelope_hi:.0})"
            ),
        };
        ui.label(
            egui::RichText::new(format!("⚠ {text}"))
                .small()
                .color(theme::WARNING_MILD),
        );
    }
}

// ── Suggest rationale (v3.1) ─────────────────────────────────────────

/// Render the rationale tree the combined-Suggest orchestrator
/// produces alongside its parameter writes. Each entry corresponds to
/// one [`SuggestWarning`] emitted by `enforce_invariants`. Renders
/// nothing when the rationale is empty (Suggest made no rewrites
/// worth surfacing).
pub(crate) fn draw_rationale(ui: &mut egui::Ui, rationale: &SuggestRationale) {
    if rationale.is_empty() {
        return;
    }
    ui.label(
        egui::RichText::new("Why these values?")
            .small()
            .strong()
            .color(theme::TEXT_STRONG),
    );
    for entry in &rationale.entries {
        draw_rationale_entry(ui, entry);
    }
}

fn draw_rationale_entry(ui: &mut egui::Ui, entry: &RationaleEntry) {
    ui.label(
        egui::RichText::new(format!("• {}", entry.headline))
            .small()
            .color(theme::TEXT_STRONG),
    );
    if let Some(detail) = &entry.detail {
        ui.label(
            egui::RichText::new(format!("    {detail}"))
                .small()
                .color(theme::TEXT_DIM),
        );
    }
}

// ────────────────────────────────────────────────────────────────────
// Chipload-math breakdown
// ────────────────────────────────────────────────────────────────────

/// Render the full chipload → feed pipeline so the user can see *why*
/// the recommended diamond sits where it does on the nomogram. The
/// "target" chipload comes from either a vendor LUT row (midpoint) or
/// the empirical formula `K₀ × D^p × (1/H)^q`. Each derate is then
/// listed with its multiplier, and the final effective chipload
/// matches `feed / (RPM × flutes)` — the chart marker's location.
pub(crate) fn draw_chipload_breakdown(ui: &mut egui::Ui, explain: &FeedsExplain) {
    let d = &explain.recommended.derates;
    let flutes = explain.flute_count.max(1) as f64;
    let effective = if explain.recommended.rpm > 0.0 {
        explain.recommended.feed_rate_mm_min / (explain.recommended.rpm * flutes)
    } else {
        0.0
    };

    egui::CollapsingHeader::new(
        egui::RichText::new("Why is the recommendation here?")
            .strong()
            .color(theme::TEXT_STRONG),
    )
    .default_open(false)
    .show(ui, |ui| {
        // ── Step 1: target chipload ──────────────────────────────────
        ui.label(
            egui::RichText::new("Target advance/tooth")
                .small()
                .strong()
                .color(theme::TEXT_DIM),
        );
        match (&explain.recommended.chipload_source, &d.formula) {
            (rs_cam_core::feeds::ChiploadSource::VendorLut { observation_id }, _) => {
                let lut_band = match &explain.matched_row {
                    Some(row) => match (row.chip_load_min_mm, row.chip_load_max_mm) {
                        (Some(lo), Some(hi)) => {
                            format!(" (band {lo:.4}–{hi:.4})")
                        }
                        _ => String::new(),
                    },
                    None => String::new(),
                };
                ui.label(
                    egui::RichText::new(format!(
                        "Vendor LUT midpoint: {:.4} mm/tooth{lut_band}",
                        d.target_chip_load_mm
                    ))
                    .small(),
                );
                ui.label(
                    egui::RichText::new(format!("from row: {observation_id}"))
                        .small()
                        .color(theme::TEXT_DIM),
                );
            }
            (rs_cam_core::feeds::ChiploadSource::FormulaFallback, Some(f))
            | (rs_cam_core::feeds::ChiploadSource::EdgeRadiusFloor, Some(f)) => {
                ui.label(
                    egui::RichText::new(
                        "No vendor LUT matched — using empirical formula:",
                    )
                    .small()
                    .color(theme::WARNING_MILD),
                );
                ui.label(
                    // ui-string-columns: a monospace formula block; the runs
                    // align the two continuation lines' `=` under the first.
                    egui::RichText::new(format!(
                        "  fz = K₀ · D^p · (1/H)^q\n     = {:.4} · {:.2}^{:.2} · (1/{:.1})^{:.2}\n     = {:.4} mm/tooth",
                        f.k0, f.diameter_mm, f.p, f.feed_scale_factor, f.q, f.result_mm_tooth
                    ))
                    .small()
                    .monospace(),
                );
                ui.label(
                    egui::RichText::new(
                        "K₀/p/q come from MachineProfile.chip_load; D is your tool diameter; \
                         H is the material hardness index.",
                    )
                    .small()
                    .color(theme::TEXT_DIM),
                );
            }
            _ => {
                ui.label(
                    egui::RichText::new(format!(
                        "Starting advance/tooth: {:.4} mm/tooth",
                        d.target_chip_load_mm
                    ))
                    .small(),
                );
            }
        }

        ui.add_space(6.0);

        // ── Step 2: derate chain ─────────────────────────────────────
        ui.label(
            egui::RichText::new("Derates applied")
                .small()
                .strong()
                .color(theme::TEXT_DIM),
        );
        egui::Grid::new("feeds_modal_derates")
            .num_columns(3)
            .spacing([crate::ui::tokens::SPACE_3, crate::ui::tokens::SPACE_2])
            .min_row_height(crate::ui::tokens::ROW_DENSE)
            .show(ui, |ui| {
                // Header
                ui.label(
                    egui::RichText::new("factor")
                        .small()
                        .color(theme::TEXT_DIM),
                );
                ui.label(
                    egui::RichText::new("×")
                        .small()
                        .color(theme::TEXT_DIM),
                );
                ui.label(
                    egui::RichText::new("note")
                        .small()
                        .color(theme::TEXT_DIM),
                );
                ui.end_row();

                // Chip thinning is MEASURED, NOT APPLIED since 2026-08-19
                // (G-CHIPTHIN-HALFFIX). It is still shown, because the
                // geometric condition is real and an operator should see it —
                // but it must not read as one of the multipliers that produced
                // the feed, because it no longer is one. The old rows said
                // "feed faster" and sat in the same column as the derates that
                // do multiply; that wording is what a reader would have cited.
                if d.observed_combined_chip_thinning > 1.001 {
                    derate_row(
                        ui,
                        "chip-thinning (observed, NOT applied)",
                        d.observed_combined_chip_thinning,
                        "chip is thinner per pass at this stepover / DOC — reported only; \
                         the vendor chipload column states no radial condition to correct from",
                    );
                }
                derate_row(
                    ui,
                    "depth-tier feed derate",
                    d.depth_tier,
                    if d.depth_tier < 0.999 {
                        "deep cut — slow feed to limit deflection"
                    } else {
                        "shallow / nominal depth"
                    },
                );
                derate_row(
                    ui,
                    "L/D overhang",
                    d.ld_overhang,
                    if d.ld_overhang < 0.999 {
                        "long tool — back off to limit deflection"
                    } else {
                        "stickout reasonable for tool diameter"
                    },
                );
                derate_row(
                    ui,
                    "workholding rigidity",
                    d.workholding,
                    match d.workholding {
                        x if x < 0.99 => "Low rigidity (tape / vacuum) — back off",
                        x if x > 1.01 => "High rigidity (vise / bolted) — push up",
                        _ => "Medium — no adjustment",
                    },
                );
                if d.power_limit < 0.999 {
                    derate_row(
                        ui,
                        "power limit",
                        d.power_limit,
                        "spindle can't deliver more power — feed reduced",
                    );
                }
                if d.feed_clamp < 0.999 {
                    derate_row(
                        ui,
                        "feed-cap clamp",
                        d.feed_clamp,
                        "hit machine.max_feed_mm_min",
                    );
                }
                derate_row(
                    ui,
                    "machine safety factor",
                    d.safety_factor,
                    "extra margin so the recommendation is comfortably safe",
                );
                // Spindle speedup is the only ≥ 1.0 "derate" in the
                // chain — it walks the constant-chipload line up the
                // speed axis when SpindleStrategy::MaxSpeed is on.
                // Hidden when at unity (the default MatchChart state)
                // to avoid clutter on every recommendation.
                if (d.spindle_speedup - 1.0).abs() > 1e-3 {
                    derate_row(
                        ui,
                        "spindle speedup",
                        d.spindle_speedup,
                        "MaxSpeed policy — RPM lifted toward spindle ceiling, feed scaled to keep the commanded advance/tooth constant",
                    );
                }
            });

        ui.add_space(6.0);

        // ── Step 3: effective chipload ───────────────────────────────
        let combined = d.combined_factor();
        let derate_pct = ((1.0 - combined) * 100.0).max(0.0);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Advance/tooth at recommendation:")
                    .small()
                    .strong()
                    .color(theme::TEXT_STRONG),
            );
            ui.label(
                egui::RichText::new(format!("{effective:.4} mm/tooth"))
                    .small()
                    .strong()
                    .color(theme::SUCCESS),
            );
        });
        ui.label(
            egui::RichText::new(format!(
                "= target {:.4} × {:.3} (combined derate, {derate_pct:.0}% reduction)",
                d.target_chip_load_mm, combined
            ))
            .small()
            .color(theme::TEXT_DIM),
        );
        ui.label(
            egui::RichText::new(format!(
                "= feed {:.0} mm/min ÷ ({:.0} RPM × {} flutes)",
                explain.recommended.feed_rate_mm_min, explain.recommended.rpm, explain.flute_count
            ))
            .small()
            .color(theme::TEXT_DIM),
        );
        if let Some((_, hi)) = explain.matched_row.as_ref().and_then(|r| {
            r.chip_load_min_mm.zip(r.chip_load_max_mm)
        }) && effective > 0.0
        {
            let pct_of_max = (effective / hi) * 100.0;
            ui.label(
                egui::RichText::new(format!(
                    "= {pct_of_max:.0}% of vendor band max ({hi:.4} mm/tooth)"
                ))
                .small()
                .color(theme::TEXT_DIM),
            );
        }
    });
}

fn derate_row(ui: &mut egui::Ui, label: &str, value: f64, note: &str) {
    let color = if value < 0.999 {
        theme::WARNING_MILD
    } else if value > 1.001 {
        theme::SUCCESS
    } else {
        theme::TEXT_DIM
    };
    ui.label(egui::RichText::new(label).small());
    ui.label(
        egui::RichText::new(format!("{value:.3}"))
            .small()
            .color(color)
            .monospace(),
    );
    ui.label(egui::RichText::new(note).small().color(theme::TEXT_DIM));
    ui.end_row();
}
