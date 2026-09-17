//! The toolpath tab strip and the diagnostic rows under it: the per-tab
//! badge counts, the row tiers the ribbon merges, and the Geometry tab's
//! wiring block.

use super::ToolpathTab;
use crate::state::toolpath::{HeightContext, OperationConfig, StockSource, ToolpathEntry};

/// Per-tab badge state for the tab bar.
pub(super) struct TabBadges {
    feeds_badge: Option<egui::Color32>,
    heights_badge: Option<egui::Color32>,
    dressup_badge: Option<egui::Color32>,
}

impl TabBadges {
    fn for_tab(&self, tab: ToolpathTab) -> Option<egui::Color32> {
        match tab {
            ToolpathTab::FeedsSpeeds => self.feeds_badge,
            ToolpathTab::Heights => self.heights_badge,
            ToolpathTab::Dressup => self.dressup_badge,
            ToolpathTab::Geometry | ToolpathTab::Linking => None,
        }
    }
}

pub(super) fn compute_tab_badges(
    entry: &ToolpathEntry,
    diagnostics: &[rs_cam_core::diagnostics::Diagnostic],
    height_ctx: Option<&HeightContext>,
) -> TabBadges {
    use rs_cam_core::diagnostics::{Category, DiagnosticState, Severity};

    // Heights: badge if any height warning exists
    let heights_badge = if let Some(hctx) = height_ctx {
        let h = entry.heights.resolve(hctx);
        if h.bottom_z > h.top_z || h.clearance_z < h.retract_z {
            Some(crate::ui::tokens::DANGER) // red
        } else if h.feed_z < h.top_z || h.retract_z < h.feed_z {
            Some(crate::ui::tokens::CAUTION) // yellow
        } else {
            None
        }
    } else {
        None
    };

    // Feeds: badge if any feed warning from core
    let feeds_badge = entry.feeds_result.as_ref().and_then(|r| {
        if r.warnings.is_empty() {
            None
        } else if r.power_limited {
            Some(crate::ui::tokens::CAUTION)
        } else {
            Some(crate::ui::tokens::INFO)
        }
    });

    // Mods: badge derived from actionable Current diagnostics in
    // Safety / Geometry / ToolLoad / Quality categories. Pre-sim hints
    // and State (workflow) notices stay quiet so a healthy op doesn't
    // flash a yellow tab.
    let mut has_critical = false;
    let mut has_caution = false;
    for d in diagnostics {
        if d.state != DiagnosticState::Current {
            continue;
        }
        if matches!(d.category, Category::State | Category::Efficiency) {
            continue;
        }
        match d.severity {
            Severity::Blocking | Severity::Critical => has_critical = true,
            Severity::Caution => has_caution = true,
            _ => {}
        }
    }
    let dressup_badge = if has_critical {
        Some(crate::ui::tokens::DANGER)
    } else if has_caution {
        Some(crate::ui::tokens::CAUTION)
    } else {
        None
    };

    TabBadges {
        feeds_badge,
        heights_badge,
        dressup_badge,
    }
}

/// One small inspector line that carries a SENTENCE, wrapped explicitly.
///
/// G-REACHWRAP (UX-R09-001). `ui.label` inherits its wrap mode from the
/// enclosing layout — `Wrap` under a vertical or main-wrapped one, `Extend`
/// under a plain `ui.horizontal` — so whether a caveat survived at 1400×900
/// depended on which row it happened to be written into. A sentence the
/// operator has to read in full states its own wrapping instead, and does not
/// change if the panel is later restructured.
///
/// Layout only. It sets no width and alters no text; the caller still owns
/// the colour, because these lines are colour-coded by severity.
pub(super) fn wrapped_small_label(ui: &mut egui::Ui, text: String, color: egui::Color32) {
    ui.add(egui::Label::new(egui::RichText::new(text).small().color(color)).wrap());
}

/// Tier of a diagnostic row in the params panel. Drives the colour
/// scheme + collapse behaviour without polluting the core schema.
#[derive(Debug, Clone, Copy)]
pub(super) enum RowTier {
    Actionable,
    Stateful,
    Hint,
}

/// Render one diagnostic row in the params panel ribbon. Surfaces
/// evidence + confidence inline. When the diagnostic carries an
/// `ApplyStaleDefault` fix, the fix button is rendered alongside
/// (uses the same direct-apply path the validator banner uses).
/// `SetToolpathParam` fixes are advisory for now — the schema
/// supports them but no adapter emits them yet.
/// Merge load-gate stateful rows that carry the same status text into one
/// "Gates: …" row (density pass V3). Pre-merge, every gate printed its own
/// copy of "<gate>: simulation stale — re-run to verify" — three identical
/// sentences for chipload/power/deflection on every stale toolpath. Gate
/// rows with a unique status text (and all non-gate rows) pass through
/// unchanged in the second slot.
pub(super) fn merge_stateful_gate_rows<'a>(
    stateful: &[&'a rs_cam_core::diagnostics::Diagnostic],
) -> (
    Vec<rs_cam_core::diagnostics::Diagnostic>,
    Vec<&'a rs_cam_core::diagnostics::Diagnostic>,
) {
    let mut merged: Vec<rs_cam_core::diagnostics::Diagnostic> = Vec::new();
    let mut rest: Vec<&'a rs_cam_core::diagnostics::Diagnostic> = Vec::new();
    let mut gate_groups: Vec<(&str, Vec<&'a rs_cam_core::diagnostics::Diagnostic>)> = Vec::new();
    for &d in stateful {
        let suffix = (d.source == rs_cam_core::diagnostics::Source::ToolLoad)
            .then(|| d.message.split_once(": ").map(|(_, s)| s))
            .flatten();
        match suffix {
            Some(suffix) => match gate_groups.iter_mut().find(|(s, _)| *s == suffix) {
                Some((_, group)) => group.push(d),
                None => gate_groups.push((suffix, vec![d])),
            },
            None => rest.push(d),
        }
    }
    for (suffix, group) in gate_groups {
        if let &[single] = group.as_slice() {
            rest.push(single);
        } else if let Some(first) = group.first() {
            let mut row = (*first).clone();
            row.message = format!("Gates: {suffix}");
            merged.push(row);
        }
    }
    (merged, rest)
}

pub(super) fn render_diagnostic_row(
    ui: &mut egui::Ui,
    d: &rs_cam_core::diagnostics::Diagnostic,
    tier: RowTier,
    entry: &mut ToolpathEntry,
    stale_default_defects: &[rs_cam_core::compute::validate::StaleDefault],
) {
    use rs_cam_core::diagnostics::{Category, DiagnosticFix, Severity};

    let category_label = match d.category {
        Category::Safety => "Safety",
        Category::Geometry => "Geometry",
        Category::ToolLoad => "Tool load",
        Category::Quality => "Quality",
        Category::Efficiency => "Efficiency",
        Category::State => "State",
    };
    let color = match tier {
        RowTier::Actionable => match d.severity {
            Severity::Blocking | Severity::Critical => crate::ui::tokens::DANGER,
            Severity::Caution => crate::ui::tokens::CAUTION,
            _ => crate::ui::tokens::INFO,
        },
        RowTier::Stateful => crate::ui::tokens::TEXT_MUTED,
        RowTier::Hint => crate::ui::tokens::TEXT_MUTED,
    };

    // Evidence (sample ranges, observed-vs-threshold, locality tags) is
    // expert payload: it rides on hover instead of an inline line below the
    // row (density pass 2026-06-11; same pattern as
    // ENGAGEMENT_PROVENANCE_HOVER in sim_diagnostics).
    let evidence_hover = if matches!(tier, RowTier::Hint) {
        None
    } else {
        d.evidence.as_ref().and_then(evidence_line)
    };

    ui.horizontal_wrapped(|ui| {
        let prefix = if matches!(d.category, Category::State) && matches!(tier, RowTier::Stateful) {
            String::new()
        } else {
            format!("{category_label}: ")
        };
        // Wrapped EXPLICITLY, not by inheritance from `horizontal_wrapped`
        // (G-REACHWRAP). A diagnostic message is a whole sentence — the
        // chipload-clamp caution names the guarantee that is NOT met and
        // then the alternatives — and the review's 1400×900 capture shows
        // it cut at "The whole derat…", which is the worst half to lose:
        // a truncated caveat reads as an unqualified result.
        let row_label = ui.add(
            egui::Label::new(
                egui::RichText::new(format!("{prefix}{}", d.message))
                    .small()
                    .color(color),
            )
            .wrap(),
        );
        if let Some(line) = evidence_hover {
            row_label.on_hover_text(line);
        }
        if matches!(tier, RowTier::Actionable) {
            let chip_text = confidence_chip_label(d.confidence);
            if !chip_text.is_empty() {
                ui.label(
                    egui::RichText::new(chip_text)
                        .small()
                        .italics()
                        .color(crate::ui::tokens::TEXT_FAINT),
                );
            }
        }

        // Stale-default fix button — looks up the matching defect by
        // rule_id (the adapter only carries the id; the canonical
        // payload lives in `stale_default_defects`). Mirrors the
        // existing validator-banner Fix button so behaviour is
        // identical to clicking that.
        if let Some(DiagnosticFix::ApplyStaleDefault {
            rule_id, new_value, ..
        }) = &d.fix
            && let Some(defect) = stale_default_defects
                .iter()
                .find(|defect| defect.rule_id.id() == rule_id.as_str())
            && ui
                .small_button(format!("\u{2713} Fix ({new_value:.3})"))
                .on_hover_text(format!(
                    "Apply validator rule `{rule_id}` to this toolpath."
                ))
                .clicked()
        {
            rs_cam_core::compute::validate::apply_stale_default_to_op(
                &mut entry.operation,
                &mut entry.feeds_provenance,
                defect,
            );
            entry.stale_since = Some(std::time::Instant::now());
        }
    });
}

/// One-line evidence summary for the panel. Returns `None` when the
/// evidence variant has nothing meaningful to render in the ribbon.
fn evidence_line(ev: &rs_cam_core::diagnostics::DiagnosticEvidence) -> Option<String> {
    use rs_cam_core::diagnostics::DiagnosticEvidence as E;
    Some(match ev {
        E::SampleRange {
            sample_start,
            sample_end,
            observed,
            threshold,
            unit,
            locality,
            ..
        } => {
            let thr = threshold
                .map(|t| format!(", threshold {t:.3}"))
                .unwrap_or_default();
            let loc = if locality.is_empty() {
                String::new()
            } else {
                format!(" [{}]", locality.as_str())
            };
            format!(
                "at samples {sample_start}-{sample_end}: observed {observed:.3} {unit}{thr}{loc}"
            )
        }
        E::LutCitation {
            row_id,
            min,
            max,
            observed,
            unit,
            extrapolated,
        } => {
            let min_s = min
                .map(|v| format!("min {v:.3}"))
                .unwrap_or_else(|| "min —".to_owned());
            let max_s = max.map(|m| format!(", max {m:.3}")).unwrap_or_default();
            let ext = if *extrapolated { " (extrapolated)" } else { "" };
            format!("LUT `{row_id}`: observed {observed:.3} {unit} ({min_s}{max_s}){ext}")
        }
        E::GeometryCompare {
            lhs_label,
            lhs_value,
            rhs_label,
            rhs_value,
            unit,
        } => format!("{lhs_label} ({lhs_value:.3} {unit}) vs {rhs_label} ({rhs_value:.3} {unit})"),
        E::Move {
            move_index,
            position,
            ..
        } => {
            if let Some([x, y, z]) = position {
                format!("at move {move_index} ({x:.2}, {y:.2}, {z:.2})")
            } else {
                format!("at move {move_index}")
            }
        }
        E::Counts {
            count,
            offender_toolpath_ids,
        } if *count > 0 => {
            if offender_toolpath_ids.is_empty() {
                format!("{count} occurrences")
            } else {
                let ids: Vec<String> = offender_toolpath_ids
                    .iter()
                    .map(|i| i.to_string())
                    .collect();
                format!("{count} occurrences across toolpaths {}", ids.join(", "))
            }
        }
        E::Counts { .. } => return None,
    })
}

fn confidence_chip_label(c: rs_cam_core::diagnostics::Confidence) -> &'static str {
    use rs_cam_core::diagnostics::Confidence as C;
    match c {
        C::Verified => "(verified)",
        C::Approximate => "(approximate)",
        C::Static => "",
        C::Heuristic => "(heuristic)",
    }
}

pub(super) fn draw_toolpath_tabs(ui: &mut egui::Ui, active: &mut ToolpathTab, badges: &TabBadges) {
    // `horizontal_wrapped`: the five tabs' natural width (~350 points) exceeds
    // the Simulation workspace's 240-point rail, and a plain horizontal row
    // would paint the last tabs past the panel edge (cut off, exactly the
    // defect UR1 bans). Wrapping drops the tail tabs to a second row in a
    // narrow rail instead.
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for &tab in ToolpathTab::ALL {
            let is_active = *active == tab;
            // The inspector tab strip takes the same treatment as the
            // workspace tabs: an active tab is a SELECTED thing, so it takes
            // ACCENT_QUIET. It used to carry from_rgb(55, 60, 80), a private
            // blue-violet one step off the workspace bar's own private
            // violet — two tab strips, two invented palettes, neither
            // matching the product.
            let (bg, text_color) = if is_active {
                (
                    crate::ui::tokens::ACCENT_QUIET,
                    crate::ui::tokens::TEXT_STRONG,
                )
            } else {
                (egui::Color32::TRANSPARENT, crate::ui::tokens::TEXT_MUTED)
            };
            let label = if let Some(badge_color) = badges.for_tab(tab) {
                // Prepend a colored dot
                let mut job = egui::text::LayoutJob::default();
                job.append(
                    "\u{25CF} ",
                    0.0,
                    egui::TextFormat {
                        color: badge_color,
                        font_id: egui::FontId::proportional(8.0),
                        ..Default::default()
                    },
                );
                job.append(
                    tab.label(),
                    0.0,
                    egui::TextFormat {
                        color: text_color,
                        font_id: egui::FontId::proportional(13.0),
                        ..Default::default()
                    },
                );
                egui::WidgetText::LayoutJob(std::sync::Arc::new(job))
            } else {
                egui::RichText::new(tab.label())
                    .color(text_color)
                    .strong()
                    .into()
            };
            let button = egui::Button::new(label)
                .fill(bg)
                // §2.2: the radius is on the two TOP corners, because a tab
                // joins the panel below it.
                .corner_radius(egui::CornerRadius {
                    nw: crate::ui::tokens::RADIUS_SM,
                    ne: crate::ui::tokens::RADIUS_SM,
                    sw: 0,
                    se: 0,
                })
                // Was 24, below the 26-point control floor. That is why the
                // "Dressup" tab wrapped to "Dressu / p".
                .min_size(egui::vec2(55.0, crate::ui::tokens::ROW_ACTION));
            let response = ui.add(button);
            if response.clicked() && !is_active {
                *active = tab;
            }
            if is_active {
                let rect = response.rect;
                ui.painter().line_segment(
                    [
                        egui::pos2(rect.min.x, rect.max.y),
                        egui::pos2(rect.max.x, rect.max.y),
                    ],
                    egui::Stroke::new(2.0_f32, crate::ui::tokens::ACCENT),
                );
            }
            ui.add_space(2.0);
        }
    });
}

/// Operator-facing caption text for a [`rs_cam_core::surface::rest_field::RestRegionPathology`]
/// — shared by the Rest Analysis section (this toolpath's own regions) and
/// the Machining Boundary section (a `DerivedRestRegions` source's regions).
/// See `crates/rs_cam_core/src/surface/rest_field.rs` for the underlying
/// classification (2026-07-06 sliver-storm incident).
pub(super) fn rest_region_pathology_caption(
    pathology: rs_cam_core::surface::rest_field::RestRegionPathology,
) -> String {
    match pathology {
        rs_cam_core::surface::rest_field::RestRegionPathology::TooManyIslands { count } => format!(
            "⚠ {count} rest regions — threshold likely below the prior pass's cusp height; \
             raise min_valley_depth."
        ),
        rs_cam_core::surface::rest_field::RestRegionPathology::SingleGiantRegion {
            part_footprint_fraction,
        } => {
            // LH-2: the percentage is of the part's COVERED XY FOOTPRINT (the
            // rest grid's solved cells), not of its bounding rectangle — say
            // so, because the two differ by ~2x on any non-rectangular part.
            format!(
                "⚠ Rest region covers {:.0}% of the part footprint — regions barely restrict \
                 the fine pass; raise min_valley_depth, or use the machined-stock reference \
                 (Use remaining stock) for an honest rest picture.",
                part_footprint_fraction * 100.0
            )
        }
    }
}

/// The Geometry tab's WIRING rows: Tool, Input model, Faces, stock source.
///
/// DC5 (Pattern A). These rows used to sit behind a `Geometry` disclosure
/// ABOVE the tab strip, while a `Geometry` TAB held the operation's geometry
/// PARAMETERS. One name carried two different blocks at one level, and the
/// reader could not tell what contained what. The rows now open the Geometry
/// tab and the disclosure is deleted, so the name has one home.
pub(super) fn draw_geometry_wiring(
    ui: &mut egui::Ui,
    entry: &mut ToolpathEntry,
    tools: &[(crate::state::job::ToolId, String, f64)],
    models: &[(crate::state::job::ModelId, String)],
    model_has_enriched: bool,
    model_is_step_missing_brep: bool,
) {
    // Tool selector
    ui.horizontal(|ui| {
        ui.label("Tool:");
        let tool_label = tools
            .iter()
            .find(|(id, _, _)| *id == entry.tool_id)
            .map(|(_, s, _)| s.as_str())
            .unwrap_or("(none)");
        egui::ComboBox::from_id_salt("tp_tool")
            .selected_text(tool_label)
            .show_ui(ui, |ui| {
                for (id, name, _) in tools {
                    ui.selectable_value(&mut entry.tool_id, *id, name.as_str());
                }
            });
    });

    // Model selector
    ui.horizontal(|ui| {
        ui.label("Input:");
        let model_label = models
            .iter()
            .find(|(id, _)| *id == entry.model_id)
            .map(|(_, s)| s.as_str())
            .unwrap_or("(none)");
        egui::ComboBox::from_id_salt("tp_model")
            .selected_text(model_label)
            .show_ui(ui, |ui| {
                for (id, name) in models {
                    ui.selectable_value(&mut entry.model_id, *id, name.as_str());
                }
            });
    });

    // BREP-not-loaded warning: surfaces when a STEP model loaded
    // without its enriched mesh (older project files written before
    // the BREP round-trip fix, or an unexpected loader regression).
    // It stays adjacent to the Input model combo it concerns.
    if model_is_step_missing_brep {
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(
                "⚠ BREP topology not loaded — face picker unavailable. Reload model.",
            )
            .color(crate::ui::tokens::CAUTION)
            .strong(),
        );
    }

    // Face selection (STEP models only)
    if model_has_enriched {
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new("Face Selection")
                .strong()
                .color(crate::ui::tokens::TEXT_STRONG),
        );
        // SHE-005 — one affordance, not two stacked sentences. Zero
        // selected: a single muted placeholder. ≥1 selected: count +
        // Clear, no tip line. The unconditional "Tip:" sentence is
        // removed — the placeholder + the live count updating as the
        // user clicks convey the action.
        let face_count = entry.face_selection.as_ref().map(|f| f.len()).unwrap_or(0);
        if face_count > 0 {
            ui.horizontal(|ui| {
                ui.label(format!(
                    "{} face{} selected",
                    face_count,
                    if face_count == 1 { "" } else { "s" }
                ));
                if ui.small_button("Clear").clicked() {
                    entry.face_selection = None;
                    entry.stale_since = Some(std::time::Instant::now());
                }
            });
        } else {
            ui.label(
                egui::RichText::new("Pick faces in viewport \u{2197}")
                    .color(crate::ui::tokens::TEXT_FAINT),
            );
        }
    }

    // Stock source toggle — hidden for pencil ops. Pencil's own
    // "Rest reference" group on the Geometry tab (see
    // `draw_pencil_params`) now owns `stock_source` directly; showing
    // this generic checkbox too used to give the user two controls
    // that silently disagreed (this one won at generation time via
    // `rest_depth_arm`'s R2 stock preference, regardless of what the
    // reference-tool picker showed). Every other op still shows it.
    if !matches!(entry.operation, OperationConfig::Pencil(_)) {
        ui.add_space(8.0);
        let mut use_remaining = entry.stock_source == StockSource::FromRemainingStock;
        let resp = ui
            .checkbox(&mut use_remaining, "Use remaining stock")
            .on_hover_text(
                "When enabled, prior operations in this setup are simulated to \
                 determine remaining material. The toolpath will skip air cuts and \
                 adapt to the actual stock state.",
            );
        if resp.changed() {
            entry.stock_source = if use_remaining {
                StockSource::FromRemainingStock
            } else {
                StockSource::Fresh
            };
            entry.stale_since = Some(std::time::Instant::now());
        }
    }
}
