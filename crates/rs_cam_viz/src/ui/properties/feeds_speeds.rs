//! The Feeds & Speeds tab: the LUT calculator card, the manual speed
//! controls, the operating-point and advance-per-tooth readings, the vendor
//! LUT viewer and the entry-style preview diagram.

use crate::state::toolpath::{
    DressupConfig, DressupEntryStyle, HeightContext, HeightsConfig, OperationConfig, ToolpathEntry,
};
use crate::ui::AppEvent;
use crate::ui::components::{PrecedenceField, ProvKind, UiExt, ValueRow};
use crate::ui::feeds::shared::{material_family_label, tool_family_label};
use crate::ui::theme;

/// Map OperationConfig variant to (OperationFamily, PassRole) for the feeds calculator.
/// Run the LUT calculator (read-only), cache the result on the entry,
/// and draw the feeds card. The calculator never writes to the operation
/// here (Roadmap F.5). The canonical comparison's `Apply all` is the
/// only path that pushes calculated values into the operation.
#[allow(clippy::too_many_arguments)]
pub(super) fn calculate_and_apply_feeds(
    ui: &mut egui::Ui,
    entry: &mut ToolpathEntry,
    tool: &crate::state::job::ToolConfig,
    material: &rs_cam_core::material::Material,
    machine: &rs_cam_core::machine::MachineProfile,
    workholding: rs_cam_core::feeds::WorkholdingRigidity,
    spindle_strategy: rs_cam_core::feeds::SpindleStrategy,
    project_default_rpm: u32,
    load_verdict: Option<&rs_cam_core::tool_load::ToolpathLoadVerdict>,
    // Q1: the bbox of the model this toolpath machines, from
    // `ToolpathPanelSnapshot`. The card's Suggest call reads it.
    model_bbox: Option<&rs_cam_core::geo::BoundingBox3>,
    events: &mut Vec<AppEvent>,
) {
    match rs_cam_core::feeds::suggest::feeds_result_for_operation(
        &entry.operation,
        tool,
        material,
        machine,
        workholding,
        rs_cam_core::feeds::embedded_vendor_lut(),
        spindle_strategy,
    ) {
        Ok(result) => {
            entry.feeds_result = Some(result);
            draw_feeds_card(
                ui,
                entry,
                tool,
                machine,
                material,
                project_default_rpm,
                load_verdict,
                workholding,
                spindle_strategy,
                model_bbox,
                events,
            );
        }
        Err(e) => {
            entry.feeds_result = None;
            ui.add_space(8.0);
            ui.colored_label(crate::ui::tokens::DANGER, format!("Feeds unavailable: {e}"));
        }
    }
}

/// W1 — the operator sets the feed, the plunge and the RPM here.
///
/// # Why this exists again
///
/// `d323cabb` (UR4) deleted the `SPEED — how fast` section from this tab on
/// its way past. Feed and plunge had ALREADY been moved off the Geometry
/// panel into this section by W3.2, and the spindle override by W3.1, so
/// the deletion left the product with no way to set a feed rate at all —
/// only `⚡ Apply all`, which overwrites the whole recipe at once. The
/// operator found it the same day.
///
/// # What it is not
///
/// This block does not consult the calculator. It sets what the operation
/// runs. `⚡ Apply all` remains the one route from a recommendation into an
/// operation (Checkpoint I), and nothing here reads `FeedsResult`.
///
/// The write idiom is the panel's own: edit the scratch `ToolpathEntry` and
/// stamp `stale_since`. The panel's write-back turns that into a `Command`,
/// so no draw site touches the session (WP6).
pub(super) fn draw_speed_controls(
    ui: &mut egui::Ui,
    entry: &mut ToolpathEntry,
    project_default_rpm: u32,
) {
    // A drill is Z-only: its plunge rate IS its feed rate. A second field
    // there is a duplicate that writes nothing (the W3.2 rule).
    let z_only = matches!(
        entry.operation,
        OperationConfig::Drill(_) | OperationConfig::AlignmentPinDrill(_)
    );
    ui.named_section("Speed \u{2014} what this operation runs", |ui| {
        ui.param_grid("feeds_speed_controls", |ui| {
            let mut feed = entry.operation.feed_rate();
            if ValueRow::new("Feed:", &mut feed, " mm/min", 50.0, 1.0..=50_000.0)
                .show(ui)
                .edited
            {
                entry.operation.set_feed_rate(feed);
                entry.stale_since = Some(std::time::Instant::now());
            }

            if z_only {
                ui.label("Plunge:");
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(format!("{feed:.0} mm/min"))
                            .color(crate::ui::tokens::TEXT_FAINT),
                    )
                    .wrap(),
                )
                .on_hover_text(
                    "A drilling operation moves in Z only, so its plunge rate IS \
                         its feed rate. Set the feed above.",
                );
                ui.end_row();
            } else {
                let mut plunge = entry.operation.plunge_rate();
                if ValueRow::new("Plunge:", &mut plunge, " mm/min", 10.0, 1.0..=10_000.0)
                    .show(ui)
                    .edited
                {
                    entry.operation.set_plunge_rate(plunge);
                    entry.stale_since = Some(std::time::Instant::now());
                }
            }

            // The project default stays visible beside the override, so
            // the operator can see which value actually runs. The old
            // widget hid it behind a hardcoded 18 000 (P1-005/P2-006).
            let mut spindle = entry.operation.spindle_rpm();
            if PrecedenceField::new("Spindle:", &mut spindle, project_default_rpm)
                .suffix(" RPM")
                .speed(100.0)
                .range(1_000..=60_000)
                .tooltip(
                    "Override the project default spindle speed for this \
                         operation. Leave it unchecked to follow the post-config \
                         spindle speed.",
                )
                .show(ui)
            {
                entry.operation.set_spindle_rpm(spindle);
                entry.stale_since = Some(std::time::Instant::now());
            }
        });
    });
}

// The card reads the tool, the machine, the material, the project RPM, the
// accepted load verdict and two session policies to answer one question. Its
// caller `calculate_and_apply_feeds` carries the same allow for the same
// reason: bundling these into a struct would build a second data model of
// what the session already holds.
#[allow(clippy::too_many_arguments)]
fn draw_feeds_card(
    ui: &mut egui::Ui,
    entry: &mut ToolpathEntry,
    tool: &crate::state::job::ToolConfig,
    machine: &rs_cam_core::machine::MachineProfile,
    material: &rs_cam_core::material::Material,
    project_default_rpm: u32,
    load_verdict: Option<&rs_cam_core::tool_load::ToolpathLoadVerdict>,
    workholding: rs_cam_core::feeds::WorkholdingRigidity,
    spindle_strategy: rs_cam_core::feeds::SpindleStrategy,
    model_bbox: Option<&rs_cam_core::geo::BoundingBox3>,
    events: &mut Vec<AppEvent>,
) {
    ui.add_space(8.0);
    let preview = crate::ui::feeds::compare::compute_preview_for_operation(
        &entry.operation,
        tool,
        material,
        machine,
        workholding,
        spindle_strategy,
    );
    let current = crate::ui::feeds::compare::current_values_for_operation(
        &entry.operation,
        tool,
        project_default_rpm,
    );
    // `FeedsPreview` carries calculator warnings; the rows also quote the
    // invariant pass's own account of what it did. Re-run the read-only
    // canonical Suggest path; its recommendation is the preview's validated
    // recipe. The warnings stay: the stages that move a number paint a line
    // of their own below the card (ruling R4, 2026-09-24).
    let suggest_warnings = rs_cam_core::feeds::suggest::suggest_for_operation(
        rs_cam_core::feeds::suggest::SuggestForOperationInput {
            operation: &entry.operation,
            tool,
            machine,
            material,
            workholding,
            lut: rs_cam_core::feeds::embedded_vendor_lut(),
            spindle_strategy,
            // Q1: the box the runtime-sanity stepover back-off reads.
            // This site passed a default context before, so the card
            // quoted a rationale the controller and the MCP surfaces
            // would not have produced.
            context: rs_cam_core::feeds::suggest::SuggestContext {
                model_bbox,
                ..rs_cam_core::feeds::suggest::SuggestContext::default()
            },
        },
    )
    .ok()
    .map(|suggested| suggested.warnings);
    // The rationale gives the depth-per-pass and the stepover rows one entry
    // each for the aggressiveness record, so each row's hover carries the
    // dial's account of its own number.
    let rationale = suggest_warnings
        .as_deref()
        .map(rs_cam_core::feeds::rationale::SuggestRationale::from_warnings);

    crate::ui::feeds::compare::draw_inspector_comparison(
        ui,
        &current,
        &preview,
        rationale.as_ref(),
        &entry.operation,
        tool,
        material,
        machine,
        entry.id,
        events,
    );

    // W2 deleted the `Why is the recommendation here?` disclosure. Every row
    // of the card above now explains its own number on hover, which is the
    // question an operator actually asks. What stays HERE stays because a
    // hover is the wrong home for it: a warning behind a hover is a warning
    // that was deleted.
    let explain = preview.explain();
    crate::ui::feeds::why::draw_engaged_diameter_row(ui, &current, explain);
    crate::ui::feeds::why::draw_chipload_min_warning(ui, &current, explain);
    crate::ui::feeds::why::draw_warnings(ui, explain);
    let suggest_warnings = suggest_warnings.as_deref().unwrap_or_default();
    crate::ui::feeds::why::draw_suggest_lines(ui, suggest_warnings);

    // This is accepted simulation evidence, not a recommendation. Keep it
    // outside and below the recommendation disclosure so planned and measured
    // values never read as interchangeable.
    if let Some(v) = load_verdict
        && (v.feed_explanation.is_some() || v.modulation_summary.is_some())
    {
        draw_operating_point(ui, v);
    }
}

/// Map a chipload provenance source into the shared UI vocabulary.
///
/// The per-field suggestion builder lives in `pills.rs` and still uses this
/// mapping even though UR4 removes the duplicate pills from the Feeds tab.
pub(super) fn prov_from_chipload(
    source: &rs_cam_core::feeds::ChiploadSource,
) -> (ProvKind, Option<&str>) {
    use rs_cam_core::feeds::ChiploadSource;

    match source {
        ChiploadSource::VendorLut { observation_id } => {
            (ProvKind::VendorLut, Some(observation_id.as_str()))
        }
        ChiploadSource::FormulaFallback => (ProvKind::Formula, None),
        ChiploadSource::EdgeRadiusFloor => (ProvKind::EdgeRadiusFloor, None),
    }
}

/// F-039 — read-only "solved operating point" for this toolpath: the single
/// constraint that bound feed across the most cuts, how far modulation moved
/// the feed off the commanded value, and how much of the path it touched. The
/// *measured* counterpart to the Suggest-predicted "Derived" rollup; the data
/// is the per-toolpath modulation summary captured during simulation
/// (`planning/UNIFIED_LOAD_MODEL_2026-06-18.md` §10.6). Display-only — it is
/// the optimizer's result, not a field to edit.
fn draw_operating_point(ui: &mut egui::Ui, verdict: &rs_cam_core::tool_load::ToolpathLoadVerdict) {
    use rs_cam_core::tool_load::ModulationStrategyTag;
    let summary = verdict.modulation_summary.as_ref();
    ui.named_section("OPERATING POINT \u{2014} measured", |ui| {
        // Checkpoint H2/H4.4 — the four-line card. Before it existed, this
        // section showed a feed RATIO and never printed an advance per
        // tooth at all, so commanded and achieved were nowhere on screen
        // together (A-1 census row P2, classified a gap rather than a
        // mislabel). Every number below is read off the gate's own
        // `FeedExplanation` stages — nothing is recomputed here, so the
        // card cannot drift from the verdict it sits beside.
        draw_advance_per_tooth_card(ui, verdict);

        // Hero line: the one constraint that bound feed on the most cuts —
        // the "why" behind these feeds.
        let Some(summary) = summary else {
            return;
        };
        if let Some((binding, frac)) = summary
            .binding_constraint_distribution
            .iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        {
            ui.label(
                egui::RichText::new(format!(
                    "Limited by {} ({:.0}% of cuts)",
                    binding.label(),
                    frac * 100.0
                ))
                .strong(),
            );
        }
        ui.param_grid("feeds_card_operating_point", |ui| {
            ui.label("Feed vs commanded:");
            let d = summary.median_feed_delta_pct;
            let sign = if d >= 0.0 { "+" } else { "" };
            ui.label(format!("{sign}{d:.0}% median"));
            ui.end_row();

            ui.label("Modulated:");
            ui.label(format!(
                "{} / {} cuts",
                summary.moves_touched, summary.moves_total
            ));
            ui.end_row();

            ui.label("Strategy:");
            let strat = match summary.strategy {
                ModulationStrategyTag::ConstrainedMax => "constrained-max",
                ModulationStrategyTag::BandMid => "band-mid",
            };
            ui.label(format!(
                "{strat} \u{00b7} feed scale {:.1}",
                summary.feed_scale
            ));
            ui.end_row();
        });
        // Full per-constraint breakdown, collapsed by default — only when more
        // than one constraint actually bound somewhere on the path.
        if summary.binding_constraint_distribution.len() > 1 {
            egui::CollapsingHeader::new("Constraint breakdown")
                .default_open(false)
                .show(ui, |ui| {
                    for (binding, frac) in &summary.binding_constraint_distribution {
                        ui.label(
                            egui::RichText::new(format!(
                                "{}: {:.0}%",
                                binding.label(),
                                frac * 100.0
                            ))
                            .small(),
                        );
                    }
                });
        }
    });
}

/// One value cell of a feeds grid that WRAPS instead of extending the `Ui`.
///
/// F-3, and `AUDIT.md` D-16 before it. A grid cell's default wrap mode is
/// `Extend`, which sets an INFINITE max width. One long value — "0.1313
/// mm/tooth (clamped)", or "CLAMPED to band ceiling \u{2014} not exceeded"
/// beside its label — then grows the inspector past its 280 point panel. The
/// panel clamps to its own maximum and draws the over-wide content
/// right-aligned, which puts the left end outside the clip rect and slides
/// the whole tab off its own left edge.
///
/// Do NOT fix this with a global `style.wrap_mode`: UP1 tried that and it
/// wrapped short labels mid-word ("Spoilb / oard:"). `tokens::apply_to_style`
/// leaves `wrap_mode` as `None` on purpose, so each long cell opts in here.
fn wrapped_cell(ui: &mut egui::Ui, text: impl Into<egui::WidgetText>) -> egui::Response {
    ui.add(egui::Label::new(text).wrap_mode(egui::TextWrapMode::Wrap))
}

/// Checkpoint H4.4 — the operating-point card: **Commanded advance/tooth
/// / Achieved advance/tooth / Vendor band / Gate verdict**, four lines,
/// read-only.
///
/// The point of putting them adjacent is that the interesting number is
/// the *gap*: commanded is what the operator typed, achieved is what the
/// machine reaches after the F-035 kinematics substitution, and the band
/// is the only one of the three published by a vendor. A recommendation
/// accepted against the commanded figure alone can be well outside the
/// band by the time the cutter is in the wood.
///
/// Display-only. This is the Feeds tab, and the standing rule is that it
/// never auto-locks a numeric field — there is no input affordance here,
/// only labels.
fn draw_advance_per_tooth_card(
    ui: &mut egui::Ui,
    verdict: &rs_cam_core::tool_load::ToolpathLoadVerdict,
) {
    use rs_cam_core::feeds::{ACHIEVED_ADVANCE_PER_TOOTH, COMMANDED_ADVANCE_PER_TOOTH};
    let Some(explain) = verdict.feed_explanation.as_deref() else {
        return;
    };
    ui.param_grid("feeds_card_advance_per_tooth", |ui| {
        ui.label(format!("{COMMANDED_ADVANCE_PER_TOOTH}:"));
        // Checkpoint K (d2) — renderer 2 of 3. When this number sits
        // exactly on the rubbing floor, say so on the face of the row, not
        // only in the hover. Since ruling R4 WP2a (2026-09-23) no engine
        // step puts it there; WP2b deletes `clamped_to`.
        let commanded_text = match explain.commanded.clamped_to {
            Some(_) => egui::RichText::new(format!(
                "{:.4} mm/tooth (on the floor)",
                explain.commanded.feed_per_tooth_mm
            ))
            .color(theme::WARNING_MILD),
            None => egui::RichText::new(format!(
                "{:.4} mm/tooth",
                explain.commanded.feed_per_tooth_mm
            )),
        };
        let commanded_hover = format!(
            "feed {:.0} mm/min \u{00f7} ({} RPM \u{00d7} {} flutes){}",
            explain.commanded.feed_rate_mm_min,
            explain.commanded.spindle_rpm,
            explain.commanded.flute_count,
            explain
                .commanded
                .clamped_to
                .map(|c| format!("\n\nThe advance {}.", c.label()))
                .unwrap_or_default(),
        );
        wrapped_cell(ui, commanded_text).on_hover_text(commanded_hover);
        ui.end_row();

        ui.label(format!("{ACHIEVED_ADVANCE_PER_TOOTH}:"));
        let achieved = egui::RichText::new(format!("{:.4} mm/tooth", explain.gate.value_mm));
        let ratio_note = match explain.achieved_feed.median_ratio {
            Some(r) => format!(
                "effective feed \u{00f7} (RPM \u{00d7} flutes), over the {} \
                     {}. The machine reaches {:.0}% of the commanded feed \
                     (median) on this path.",
                explain.gate.sample_count,
                explain.gate.statistic.label(),
                r * 100.0,
            ),
            // No predicted-feed map: `effective_feed_for_sample` returns
            // the commanded feed, so this row IS the commanded value and
            // must say so rather than implying a measurement.
            None => format!(
                "No kinematics prediction on this trace, so the effective \
                     feed falls back to the commanded feed \u{2014} this row is \
                     not independent evidence. Over the {} {}.",
                explain.gate.sample_count,
                explain.gate.statistic.label(),
            ),
        };
        wrapped_cell(ui, achieved).on_hover_text(ratio_note);
        ui.end_row();

        ui.label("Vendor band:");
        let band = match explain.band.min_mm_per_tooth {
            Some(lo) => format!(
                "{lo:.4}\u{2013}{:.4} mm/tooth",
                explain.band.max_mm_per_tooth
            ),
            None => format!("\u{2264} {:.4} mm/tooth", explain.band.max_mm_per_tooth),
        };
        let band_hover = format!(
            "Vendor chipload column from row {} (calibrated d={:.3} mm), \
                 after DOC derate. Published as a linear advance per tooth \
                 \u{2014} the same quantity as the two rows above.",
            explain.band.observation_id, explain.band.row_diameter_mm,
        );
        wrapped_cell(ui, band).on_hover_text(band_hover);
        ui.end_row();

        ui.label("Gate verdict:");
        let (text, color) = advance_gate_verdict_text(&verdict.chipload);
        // The worst row on this tab: "CLAMPED to band ceiling — not
        // exceeded" beside "Gate verdict:" is wider than the panel.
        wrapped_cell(ui, egui::RichText::new(text).color(color));
        ui.end_row();
    });
    ui.add_space(4.0);
}

/// One-line rendering of the chipload gate's verdict for the card above.
fn advance_gate_verdict_text(
    chipload: &rs_cam_core::tool_load::ChiploadVerdict,
) -> (String, egui::Color32) {
    use rs_cam_core::tool_load::ChiploadVerdict;
    use rs_cam_core::tool_load::verdict::ChipSide;
    match chipload {
        // Checkpoint K (c2) — a recipe the rubbing-floor clamp parked on
        // the band ceiling reads CLAMPED, never EXCEEDS: the engine put
        // it there, and the boundary comparison that used to flip it was
        // decided by float noise (G-CHIP-ULP).
        ChiploadVerdict::Within {
            burn_advisory,
            ceiling_advisory,
            ..
        } => match (burn_advisory, ceiling_advisory) {
            (Some(_), _) => (
                "Within band (burn advisory)".to_owned(),
                theme::WARNING_MILD,
            ),
            (None, Some(_)) => (
                "CLAMPED to band ceiling \u{2014} not exceeded".to_owned(),
                theme::WARNING_MILD,
            ),
            (None, None) => ("Within band".to_owned(), theme::SUCCESS),
        },
        ChiploadVerdict::Exceeds { side, .. } => (
            match side {
                ChipSide::Low => "EXCEEDS \u{2014} below band (burn/rubbing)".to_owned(),
                ChipSide::High => "EXCEEDS \u{2014} above band (breakage)".to_owned(),
            },
            theme::ERROR,
        ),
        // A gate that could not evaluate must not render as a pass.
        ChiploadVerdict::Unmodeled { reason } => {
            (format!("not modelled ({reason:?})"), theme::TEXT_DIM)
        }
    }
}

// ── Vendor LUT viewer ──────────────────────────────────────────────────

/// Map `ToolType` to the vendor LUT `ToolFamily` for filtering.
fn tool_type_to_lut_family(
    tt: crate::state::job::ToolType,
) -> rs_cam_core::feeds::vendor_lut::ToolFamily {
    use rs_cam_core::feeds::vendor_lut::ToolFamily;
    match tt {
        crate::state::job::ToolType::EndMill => ToolFamily::FlatEnd,
        crate::state::job::ToolType::BallNose => ToolFamily::BallNose,
        crate::state::job::ToolType::BullNose => ToolFamily::BullNose,
        crate::state::job::ToolType::VBit => ToolFamily::ChamferVbit,
        crate::state::job::ToolType::TaperedBallNose => ToolFamily::TaperedBallNose,
    }
}

/// Human-readable label for an `EvidenceGrade`.
fn evidence_grade_label(g: rs_cam_core::feeds::vendor_lut::EvidenceGrade) -> &'static str {
    use rs_cam_core::feeds::vendor_lut::EvidenceGrade;
    match g {
        EvidenceGrade::A => "A (vendor)",
        EvidenceGrade::B => "B (derived)",
        EvidenceGrade::C => "C (community)",
    }
}

/// Draw a collapsible vendor cutting data viewer, filtered by current tool.
pub(super) fn draw_vendor_lut_viewer(
    ui: &mut egui::Ui,
    tool_type: crate::state::job::ToolType,
    tool_diameter: f64,
) {
    ui.add_space(8.0);

    let header = egui::RichText::new("Vendor Cutting Data")
        .strong()
        .color(crate::ui::tokens::TEXT_STRONG);

    egui::CollapsingHeader::new(header)
        .default_open(false)
        .show(ui, |ui| {
            let lut = rs_cam_core::feeds::embedded_vendor_lut();
            let target_family = tool_type_to_lut_family(tool_type);

            // Filter observations: match tool family, and prefer matching diameter
            // (show all diameters for this family so the user can see the full picture).
            let matching: Vec<&rs_cam_core::feeds::vendor_lut::VendorObservation> = lut
                .observations
                .iter()
                .filter(|obs| obs.tool_family == target_family)
                .collect();

            if matching.is_empty() {
                ui.label(
                    egui::RichText::new("No matching vendor data for this tool type")
                        .small()
                        .color(crate::ui::tokens::CAUTION),
                );
                return;
            }

            ui.label(
                egui::RichText::new(format!(
                    "{} observations for {} tools (current: {:.1} mm)",
                    matching.len(),
                    tool_family_label(target_family),
                    tool_diameter,
                ))
                .small()
                .color(crate::ui::tokens::TEXT_MUTED),
            );
            ui.add_space(4.0);

            // Table header
            let dim = crate::ui::tokens::TEXT_MUTED;
            let val = crate::ui::tokens::TEXT_STRONG;
            let highlight = crate::ui::tokens::OK;
            let header_font = egui::FontId::proportional(9.0);
            let body_font = egui::FontId::proportional(9.0);

            // A seven-column data table cannot fit the inspector rail, and
            // UR1's rule forbids it from widening the panel: wrap it in a
            // horizontal ScrollArea so the table scrolls inside the rail
            // instead of being cut off at the panel edge.
            egui::ScrollArea::horizontal()
                .id_salt("vendor_lut_table_scroll")
                .auto_shrink([false, false])
                .max_width(ui.available_width())
                .show(ui, |ui| {
                    // UI-02: 7 columns and striped, so `param_grid` does not fit.
                    egui::Grid::new("vendor_lut_table")
                        .num_columns(7)
                        .spacing([crate::ui::tokens::SPACE_3, crate::ui::tokens::SPACE_2])
                        .min_row_height(crate::ui::tokens::ROW_DENSE)
                        .striped(true)
                        .show(ui, |ui| {
                            // Column headers
                            for label in [
                                "Material", "Dia (mm)", "Flutes", "RPM", "Chipload", "DOC (mm)",
                                "Grade",
                            ] {
                                ui.label(
                                    egui::RichText::new(label)
                                        .font(header_font.clone())
                                        .strong()
                                        .color(dim),
                                );
                            }
                            ui.end_row();

                            for obs in &matching {
                                // Highlight rows matching the current tool diameter (within 0.1mm).
                                // Rows with `diameter_mm = None` (v-bit charts, diameter-window
                                // articles) are never highlighted as exact-diameter matches —
                                // their match criterion is angle / material, not diameter.
                                let is_diameter_match = match obs.diameter_mm {
                                    Some(d) => (d - tool_diameter).abs() < 0.1,
                                    None => false,
                                };
                                let row_color = if is_diameter_match { highlight } else { val };

                                // Material
                                ui.label(
                                    egui::RichText::new(material_family_label(obs.material_family))
                                        .font(body_font.clone())
                                        .color(row_color),
                                );

                                // Diameter (— if the row has no diameter anchor)
                                let diameter_text = match obs.diameter_mm {
                                    Some(d) => format!("{:.1}", d),
                                    None => "—".to_owned(),
                                };
                                ui.label(
                                    egui::RichText::new(diameter_text)
                                        .font(body_font.clone())
                                        .color(row_color),
                                );

                                // Flutes
                                ui.label(
                                    egui::RichText::new(format!("{}", obs.flute_count))
                                        .font(body_font.clone())
                                        .color(row_color),
                                );

                                // RPM range
                                let rpm_text = match (obs.rpm_min, obs.rpm_max) {
                                    (Some(lo), Some(hi)) => format!("{lo:.0}-{hi:.0}"),
                                    (Some(lo), None) => format!("{lo:.0}"),
                                    (None, Some(hi)) => format!("{hi:.0}"),
                                    (None, None) => {
                                        if let Some(nom) = obs.rpm_nominal {
                                            format!("{nom:.0}")
                                        } else {
                                            "-".to_owned()
                                        }
                                    }
                                };
                                ui.label(
                                    egui::RichText::new(rpm_text)
                                        .font(body_font.clone())
                                        .color(row_color),
                                );

                                // Chipload range (mm/tooth)
                                let chip_text =
                                    match (obs.chipload_min_mm_tooth, obs.chipload_max_mm_tooth) {
                                        (Some(lo), Some(hi)) => format!("{lo:.3}-{hi:.3}"),
                                        (Some(lo), None) => format!("{lo:.3}"),
                                        (None, Some(hi)) => format!("{hi:.3}"),
                                        (None, None) => "-".to_owned(),
                                    };
                                ui.label(
                                    egui::RichText::new(chip_text)
                                        .font(body_font.clone())
                                        .color(row_color),
                                );

                                // DOC range (ap)
                                let doc_text = match (obs.ap_min_mm, obs.ap_max_mm) {
                                    (Some(lo), Some(hi)) => format!("{lo:.1}-{hi:.1}"),
                                    (Some(v), None) | (None, Some(v)) => format!("{v:.1}"),
                                    (None, None) => "-".to_owned(),
                                };
                                ui.label(
                                    egui::RichText::new(doc_text)
                                        .font(body_font.clone())
                                        .color(row_color),
                                );

                                // Evidence grade
                                ui.label(
                                    egui::RichText::new(evidence_grade_label(obs.evidence_grade))
                                        .font(body_font.clone())
                                        .color(row_color),
                                );

                                ui.end_row();
                            }
                        });
                });
        });
}

// ── Engagement diagram ──────────────────────────────────────────────────

// ── Entry style preview diagram ─────────────────────────────────────────

/// Draw a 2D side-view of the entry style geometry (ramp or helix).
pub(super) fn draw_entry_preview_diagram(
    ui: &mut egui::Ui,
    dressups: &DressupConfig,
    height_ctx: &HeightContext,
    heights: &HeightsConfig,
) {
    let resolved = heights.resolve(height_ctx);
    let feed_z = resolved.feed_z;
    let top_z = resolved.top_z;
    let z_drop = feed_z - top_z;
    if z_drop <= 0.0 {
        return;
    }

    let desired_size = egui::vec2(ui.available_width().min(260.0), 140.0);
    let (rect, _) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
    let painter = ui.painter_at(rect);

    // Background
    painter.rect_filled(rect, 4.0, crate::ui::tokens::DIAGRAM_CANVAS);

    // Z range with margin
    let z_min = top_z - z_drop * 0.15;
    let z_max = feed_z + z_drop * 0.25;

    let z_to_y = |z: f64| -> f32 {
        let frac = (z - z_min) / (z_max - z_min);
        rect.bottom() - (frac as f32) * rect.height()
    };

    let feed_y = z_to_y(feed_z);
    let top_y = z_to_y(top_z);
    let dim_color = crate::ui::tokens::TEXT_FAINT;
    let scale_color = crate::ui::tokens::BORDER;

    // Z-axis scale bar on the left edge
    let scale_x = rect.left() + 3.0;
    painter.line_segment(
        [egui::pos2(scale_x, feed_y), egui::pos2(scale_x, top_y)],
        egui::Stroke::new(1.0_f32, scale_color),
    );
    // Ticks + values at feed_z and top_z
    painter.line_segment(
        [
            egui::pos2(scale_x, feed_y),
            egui::pos2(scale_x + 4.0, feed_y),
        ],
        egui::Stroke::new(1.0_f32, scale_color),
    );
    painter.line_segment(
        [egui::pos2(scale_x, top_y), egui::pos2(scale_x + 4.0, top_y)],
        egui::Stroke::new(1.0_f32, scale_color),
    );
    // Z drop distance label
    painter.text(
        egui::pos2(scale_x + 2.0, (feed_y + top_y) / 2.0),
        egui::Align2::LEFT_CENTER,
        format!("{z_drop:.1}"),
        egui::FontId::proportional(8.0),
        scale_color,
    );

    // Dashed horizontal reference lines
    for &(z, label) in &[(feed_z, "Feed Z"), (top_z, "Top Z")] {
        let y = z_to_y(z);
        // Draw dashed line
        let dash_len = 6.0;
        let gap_len = 4.0;
        let mut x = rect.left() + 12.0;
        while x < rect.right() - 50.0 {
            let end_x = (x + dash_len).min(rect.right() - 50.0);
            painter.line_segment(
                [egui::pos2(x, y), egui::pos2(end_x, y)],
                egui::Stroke::new(0.5_f32, dim_color),
            );
            x += dash_len + gap_len;
        }
        painter.text(
            egui::pos2(rect.right() - 4.0, y),
            egui::Align2::RIGHT_CENTER,
            format!("{label} {z:.1}"),
            egui::FontId::proportional(8.0),
            dim_color,
        );
    }

    let entry_color = crate::ui::tokens::DIAGRAM_INK;
    let stroke = egui::Stroke::new(2.0_f32, entry_color);
    let cx = rect.center().x;

    match dressups.entry_style {
        DressupEntryStyle::Ramp => {
            let angle_rad = (dressups.ramp_angle as f32).to_radians();
            let ramp_horiz = z_drop as f32 / angle_rad.tan().max(0.01);

            // Scale horizontal distance to fit canvas
            let available_w = rect.width() * 0.6;
            let h_scale = available_w / ramp_horiz.max(1.0);
            let v_height = (top_y - feed_y).abs();
            let h_pixels = ramp_horiz * h_scale.min(1.0);

            let start_x = cx - h_pixels / 2.0;
            let end_x = cx + h_pixels / 2.0;

            // Ramp line
            painter.add(egui::Shape::line(
                vec![egui::pos2(start_x, feed_y), egui::pos2(end_x, top_y)],
                stroke,
            ));

            // Angle arc annotation
            let arc_r = 20.0_f32;
            let mut arc_pts = Vec::with_capacity(12);
            for i in 0..=10 {
                let t = i as f32 / 10.0;
                let a = -angle_rad * t;
                arc_pts.push(egui::pos2(
                    start_x + arc_r * a.cos(),
                    feed_y - arc_r * a.sin(),
                ));
            }
            painter.add(egui::Shape::line(
                arc_pts,
                egui::Stroke::new(1.0_f32, entry_color),
            ));
            painter.text(
                egui::pos2(start_x + arc_r + 4.0, feed_y - 8.0),
                egui::Align2::LEFT_CENTER,
                format!("{:.1}\u{00B0}", dressups.ramp_angle),
                egui::FontId::proportional(9.0),
                entry_color,
            );

            // Entry point marker
            painter.circle_filled(egui::pos2(end_x, top_y), 3.0, entry_color);

            // Label
            painter.text(
                egui::pos2(rect.left() + 6.0, rect.top() + 8.0),
                egui::Align2::LEFT_TOP,
                "Ramp Entry",
                egui::FontId::proportional(10.0),
                entry_color,
            );

            let _ = v_height;
        }
        DressupEntryStyle::Helix => {
            let radius = dressups.helix_radius;
            let pitch = dressups.helix_pitch;
            let turns = z_drop / pitch.max(0.01);

            // Side view of helix: sinusoidal wave descending
            let total_angle = turns * std::f64::consts::TAU;
            let steps = (turns * 32.0).clamp(32.0, 200.0) as usize;

            // Scale radius to fit canvas
            let available_w = rect.width() * 0.5;
            let r_pixels = (radius as f32 * available_w / (radius as f32 * 2.0).max(1.0))
                .min(available_w / 2.0);

            let mut pts = Vec::with_capacity(steps + 1);
            for i in 0..=steps {
                let t = i as f64 / steps as f64;
                let angle = total_angle * t;
                let z = feed_z - z_drop * t;
                let x_off = (radius * angle.cos()) as f32 * (r_pixels / radius.max(0.01) as f32);
                pts.push(egui::pos2(cx + x_off, z_to_y(z)));
            }
            painter.add(egui::Shape::line(pts, stroke));

            // Entry point marker
            painter.circle_filled(egui::pos2(cx, top_y), 3.0, entry_color);

            // Radius annotation
            painter.line_segment(
                [
                    egui::pos2(cx, z_to_y(feed_z)),
                    egui::pos2(cx + r_pixels, z_to_y(feed_z)),
                ],
                egui::Stroke::new(1.0_f32, dim_color),
            );
            painter.text(
                egui::pos2(cx + r_pixels / 2.0, z_to_y(feed_z) - 8.0),
                egui::Align2::CENTER_BOTTOM,
                format!("r={radius:.1}"),
                egui::FontId::proportional(8.0),
                dim_color,
            );

            // Label
            painter.text(
                egui::pos2(rect.left() + 6.0, rect.top() + 8.0),
                egui::Align2::LEFT_TOP,
                format!("Helix Entry ({turns:.1} turns)"),
                egui::FontId::proportional(10.0),
                entry_color,
            );
        }
        DressupEntryStyle::None => {
            // Vertical plunge arrow
            painter.line_segment([egui::pos2(cx, feed_y), egui::pos2(cx, top_y)], stroke);
            // Arrowhead
            painter.add(egui::Shape::line(
                vec![
                    egui::pos2(cx - 4.0, top_y - 8.0),
                    egui::pos2(cx, top_y),
                    egui::pos2(cx + 4.0, top_y - 8.0),
                ],
                stroke,
            ));
            painter.text(
                egui::pos2(rect.left() + 6.0, rect.top() + 8.0),
                egui::Align2::LEFT_TOP,
                "Direct Plunge",
                egui::FontId::proportional(10.0),
                entry_color,
            );
        }
    }
}
