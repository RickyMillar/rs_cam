//! The toolpath tab strip and the diagnostic rows under it: the per-tab
//! badge counts, the row tiers the ribbon merges, and the Geometry tab's
//! wiring block.

use super::ToolpathTab;
use crate::state::toolpath::{HeightContext, StockSource, ToolpathEntry};
use crate::ui::components::{TabBadge, TabStrip};

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
    shoe_gap_too_low: bool,
) -> TabBadges {
    use rs_cam_core::diagnostics::{Category, DiagnosticState, Severity};

    // Heights: badge if any height warning exists. A warning outranks the
    // pin mark below.
    let heights_warning = height_ctx.and_then(|hctx| {
        let h = entry.heights.resolve(hctx);
        if h.bottom_z > h.top_z || h.clearance_z < h.retract_z {
            Some(crate::ui::tokens::DANGER) // red
        } else if h.feed_z < h.top_z || h.retract_z < h.feed_z {
            Some(crate::ui::tokens::CAUTION) // yellow
        } else {
            None
        }
    });
    // R5 (2026-09-18): a pinned height is a fact the tab states before it
    // opens. A diagram drag pinned Top at −1.37 and nothing on the tab strip
    // said so (`planning/corne_case_analysis_2026-09-18/ANALYSIS.md` §4.2).
    // INFO is the badge's neutral role; the Feeds badge uses the same one.
    let hs = &entry.heights;
    let any_pinned = [
        &hs.clearance_z,
        &hs.retract_z,
        &hs.feed_z,
        &hs.top_z,
        &hs.bottom_z,
    ]
    .into_iter()
    .any(|mode| !mode.is_auto());
    // H2: a stated shoe gap below the solver gap is a Caution about the
    // shoe row, which this tab holds.
    let heights_badge = heights_warning
        .or(shoe_gap_too_low.then_some(crate::ui::tokens::CAUTION))
        .or(any_pinned.then_some(crate::ui::tokens::INFO));

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

    let prefix = if matches!(d.category, Category::State) && matches!(tier, RowTier::Stateful) {
        String::new()
    } else {
        format!("{category_label}: ")
    };
    // IC1 (2026-10-13): one line per finding. A long message truncates on
    // the face, and the hover gives the whole message, the confidence and
    // the evidence. G-REACHWRAP made this line wrap so a caveat was never
    // cut; the hover now carries it, and the row keeps its role colour.
    let mut hover = format!("{prefix}{}", d.message);
    if matches!(tier, RowTier::Actionable) {
        let chip_text = confidence_chip_label(d.confidence);
        if !chip_text.is_empty() {
            hover.push_str(&format!("\nConfidence: {chip_text}"));
        }
    }
    if let Some(line) = evidence_hover {
        hover.push_str(&format!("\n{line}"));
    }

    // Stale-default fix button — looks up the matching defect by rule_id
    // (the adapter only carries the id; the canonical payload lives in
    // `stale_default_defects`).
    let fix = match &d.fix {
        Some(DiagnosticFix::ApplyStaleDefault {
            rule_id, new_value, ..
        }) => stale_default_defects
            .iter()
            .find(|defect| defect.rule_id.id() == rule_id.as_str())
            .map(|defect| (defect, rule_id.as_str(), *new_value)),
        _ => None,
    };
    ui.horizontal(|ui| {
        // The Fix button sits at the right edge; the message takes the rest.
        let fix_width = if fix.is_some() {
            crate::ui::tokens::FIX_BUTTON_WIDTH
        } else {
            0.0
        };
        let line_width = (ui.available_width() - fix_width).max(0.0);
        ui.allocate_ui_with_layout(
            egui::vec2(line_width, crate::ui::tokens::ROW_DENSE),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.set_width(line_width);
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(format!("{prefix}{}", d.message))
                            .small()
                            .color(color),
                    )
                    .truncate()
                    .show_tooltip_when_elided(false),
                )
                .on_hover_text(hover);
            },
        );
        if let Some((defect, rule_id, new_value)) = fix
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
        C::Verified => "verified",
        C::Approximate => "approximate",
        C::Static => "",
        C::Heuristic => "heuristic",
    }
}

pub(super) fn draw_toolpath_tabs(ui: &mut egui::Ui, active: &mut ToolpathTab, badges: &TabBadges) {
    // The strip is the shared `components::TabStrip` (inspector clean-up
    // round 3); the Simulation inspector draws the same one. It wraps in a
    // narrow rail, so no tab paints past the panel edge (UR1).
    let strip = ToolpathTab::ALL
        .iter()
        .fold(TabStrip::new(active), |strip, &tab| {
            strip.tab_badge(tab, tab.label(), badges.for_tab(tab).map(TabBadge::Dot))
        });
    let _ = strip.show(ui);
}

/// The caution line for a [`rs_cam_core::surface::rest_field::RestRegionPathology`]
/// — shared by the rest analysis (this toolpath's own regions) and the
/// Machining Boundary section (a `DerivedRestRegions` source's regions).
/// See `crates/rs_cam_core/src/surface/rest_field.rs` for the underlying
/// classification (2026-07-06 sliver-storm incident).
///
/// IC1: one line with the count or the share; the cause and the remedy are
/// on the hover.
pub(super) fn rest_region_pathology_line(
    pathology: rs_cam_core::surface::rest_field::RestRegionPathology,
) -> crate::ui::components::OneLine {
    use rs_cam_core::surface::rest_field::RestRegionPathology;
    let (face, detail) = match pathology {
        RestRegionPathology::TooManyIslands { count } => (
            format!("\u{26A0} {count} rest regions"),
            "The threshold is probably below the cusp height of the prior pass. \
             Raise Min Valley Depth."
                .to_owned(),
        ),
        // LH-2: the percentage is of the part's COVERED XY FOOTPRINT (the
        // rest grid's solved cells), not of its bounding rectangle — say so,
        // because the two differ by ~2x on any non-rectangular part.
        RestRegionPathology::SingleGiantRegion {
            part_footprint_fraction,
        } => (
            format!(
                "\u{26A0} One rest region covers {:.0}% of the part",
                part_footprint_fraction * 100.0
            ),
            "The percentage is of the part footprint. The regions barely restrict \
             the fine pass. Raise Min Valley Depth, or set Start from to After \
             previous ops for a true rest picture."
                .to_owned(),
        ),
    };
    crate::ui::components::OneLine::new(face)
        .color(crate::ui::tokens::CAUTION)
        .detail(detail)
}

/// The two ways an operation can start, in the order an operator meets
/// them. `Fresh` is the default, so it reads first.
const STOCK_SOURCES: &[(StockSource, &str)] = &[
    (StockSource::Fresh, "Stock"),
    (StockSource::FromRemainingStock, "After previous ops"),
];

/// One hover for the row. It names what each option does, because the
/// difference between them is the whole question.
const START_FROM_HOVER: &str = concat!(
    "Stock: the toolpath starts from the raw stock. ",
    "After previous ops: the toolpath starts from the material the earlier ",
    "operations in this setup leave, and needs their simulation first."
);

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
    inputs: &super::ToolpathPanelInputs,
    events: &mut Vec<crate::ui::AppEvent>,
) {
    let tools = inputs.tools.as_slice();
    let models = inputs.models.as_slice();
    let model_has_enriched = inputs.model_has_enriched;
    let model_is_step_missing_brep = inputs.model_is_step_missing_brep;
    let rest_fresh = inputs.rest_fresh;
    // Tool selector
    ui.horizontal(|ui| {
        ui.label("Tool:");
        let tool_label = tools
            .iter()
            .find(|(id, _, _)| *id == entry.tool_id)
            .map(|(_, s, _)| s.as_str())
            .unwrap_or("(none)");
        egui::ComboBox::from_id_salt("tp_tool")
            .truncate()
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
            .truncate()
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
        ui.add(
            crate::ui::components::OneLine::new("\u{26A0} BREP topology not loaded")
                .color(crate::ui::tokens::CAUTION)
                .detail("The face picker is unavailable. Reload the model."),
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

    // W2 (G-STARTFROM): ONE row writes `stock_source`, for every
    // operation, pencil included. It used to be two controls that
    // disagreed: this generic checkbox, and a pencil-only "Rest
    // reference" pair. The checkbox won at generation, through
    // `rest_depth_arm`'s stock preference, whatever the pencil pair
    // showed. Pencil keeps its analytic reference picker, which is a
    // different question: what "rest" is measured against when no
    // machined stock is read.
    //
    // W3/W4: hover from the result. The deleted caption said the generator
    // falls back to the reference tool when the simulated stock does not
    // overlap this model's frame. That is resolved at generation, not in
    // the config, so it belongs on this row as a hover read from the
    // generation result once `rest_reference_mode` reaches `ToolpathStats`.
    ui.add_space(8.0);
    let resp = ui.add(
        crate::ui::components::ChoiceRow::new("Start from", &mut entry.stock_source, STOCK_SOURCES)
            .hover(START_FROM_HOVER),
    );
    if resp.changed() {
        entry.stale_since = Some(std::time::Instant::now());
    }

    // Operator ruling 2026-10-15: a rest op compares itself with the same
    // op on fresh stock ON REQUEST. The twin is one more generation, so it
    // never runs on its own.
    if entry.stock_source == StockSource::FromRemainingStock {
        ui.horizontal(|ui| {
            if ui
                .small_button("Compare with fresh")
                .on_hover_text(COMPARE_WITH_FRESH_HOVER)
                .clicked()
            {
                events.push(crate::ui::AppEvent::CompareRestWithFresh(entry.id));
            }
            if let Some(times) = rest_fresh {
                ui.label(
                    egui::RichText::new(format!(
                        "rest {:.1} s \u{00B7} fresh {:.1} s",
                        times.rest_s, times.fresh_s
                    ))
                    .color(crate::ui::tokens::TEXT_FAINT),
                );
            }
        });
    }
}

/// The hover of the "Compare with fresh" button.
const COMPARE_WITH_FRESH_HOVER: &str = "Generate this operation once more on fresh stock and \
    compare the two cycle times (the simulator's model, at the planned feeds). Rest can take \
    longer: each island needs its own entry. The answer stays until the operation changes.";
