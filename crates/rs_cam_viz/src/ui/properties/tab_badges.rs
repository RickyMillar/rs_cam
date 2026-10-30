//! The toolpath tab strip and the diagnostic rows under it: the per-tab
//! badge counts, the row tiers the ribbon merges, and the Geometry tab's
//! wiring block.

use super::ToolpathTab;
use crate::state::toolpath::{StockSource, ToolpathEntry};
use crate::ui::components::{TabBadge, TabStrip};

/// Per-tab badge state for the tab bar: one dot colour per tab, or none.
pub(super) struct TabBadges {
    dots: [Option<egui::Color32>; ToolpathTab::ALL.len()],
}

impl TabBadges {
    pub(super) fn for_tab(&self, tab: ToolpathTab) -> Option<egui::Color32> {
        ToolpathTab::ALL
            .iter()
            .position(|t| *t == tab)
            .and_then(|at| self.dots.get(at).copied().flatten())
    }

    /// No dot on any tab (the D7 strip test).
    #[cfg(test)]
    pub(super) fn none() -> Self {
        Self {
            dots: [None; ToolpathTab::ALL.len()],
        }
    }

    /// Raise the dot of `tab` to `color` when `color` ranks higher.
    pub(super) fn raise(&mut self, tab: ToolpathTab, color: egui::Color32) {
        if let Some(at) = ToolpathTab::ALL.iter().position(|t| *t == tab)
            && let Some(dot) = self.dots.get_mut(at)
        {
            *dot = Some(worse(*dot, color));
        }
    }
}

/// The rank of a dot colour: DANGER over CAUTION over INFO.
fn dot_rank(color: egui::Color32) -> u8 {
    if color == crate::ui::tokens::DANGER {
        3
    } else if color == crate::ui::tokens::CAUTION {
        2
    } else {
        1
    }
}

/// The higher-ranked of a dot and a new colour.
fn worse(dot: Option<egui::Color32>, color: egui::Color32) -> egui::Color32 {
    match dot {
        Some(old) if dot_rank(old) >= dot_rank(color) => old,
        Some(_) | None => color,
    }
}

/// FR-2 F24: THE map from a diagnostic id to the tab that holds its fix.
/// `None`: no tab of the toolpath holds the fix (a setup pin, the tool
/// editor, the material, an obstacle), so no tab gets a dot; the footer
/// row is the one owner of the finding. The table holds every id of
/// core `diagnostics::ids::ALL`; `owner_tab_map_covers_every_id_f24`
/// fails on an id with no row.
///
/// R2: the Dressup tab is deleted (its rows are on Linking); the Cut tab is
/// now Passes. The tool is the header Tool row, on no tab, so a tool
/// finding gives no dot: the row outline carries it.
pub(crate) const OWNER_TAB: &[(&str, Option<ToolpathTab>)] = {
    use rs_cam_core::diagnostics::ids as i;
    const G: Option<ToolpathTab> = Some(ToolpathTab::Geometry);
    const C: Option<ToolpathTab> = Some(ToolpathTab::Passes);
    const F: Option<ToolpathTab> = Some(ToolpathTab::FeedsSpeeds);
    const L: Option<ToolpathTab> = Some(ToolpathTab::Linking);
    const H: Option<ToolpathTab> = Some(ToolpathTab::Heights);
    const NO: Option<ToolpathTab> = None;
    &[
        // The load gates: the feed, the speed and the tool load.
        (i::LOAD_CHIPLOAD_HIGH, F),
        (i::LOAD_CHIPLOAD_LOW, F),
        (i::LOAD_CHIPLOAD_WITHIN, F),
        (i::LOAD_CHIPLOAD_UNMODELED, F),
        (i::LOAD_CHIPLOAD_COMMANDED_ABOVE_BAND, F),
        (i::LOAD_POWER_EXCEEDS, F),
        (i::LOAD_POWER_WITHIN, F),
        (i::LOAD_POWER_UNMODELED, F),
        (i::LOAD_DEFLECTION_EXCEEDS, F),
        (i::LOAD_DEFLECTION_WITHIN, F),
        (i::LOAD_DEFLECTION_UNMODELED, F),
        // The depth of cut is an op parameter.
        (i::LOAD_DEPTH_WITHIN, C),
        (i::LOAD_DEPTH_EXCEEDS, C),
        (i::LOAD_DEPTH_REPORTED, C),
        (i::LOAD_DEPTH_UNMODELED, C),
        (i::LOAD_MATERIAL_NOT_JUDGED, NO),
        (i::DRILL_CHIP_WELDING, F),
        (i::DRILL_PECK_ADEQUACY, C),
        (i::DRILL_PLUNGE_FEED, F),
        // Project findings.
        (i::PROJECT_HOLDER_COLLISION, NO),
        (i::PROJECT_HOLDER_CHECK_FAILED, NO),
        (i::PROJECT_RAPID_COLLISION, H),
        (i::PROJECT_OBSTACLE_COLLISION, NO),
        (i::PROJECT_PLUNGE_STRESS, L),
        (i::PROJECT_AIR_CUT_HIGH, L),
        (i::PROJECT_GENERATED_EMPTY, G),
        (i::PROJECT_ALIGNMENT_PINS_UNKEYED, NO),
        (i::PROJECT_PIN_HOLES_DRILLED_TWICE, NO),
        (i::PROJECT_DOWEL_HOLES_DISAGREE, NO),
        (i::PROJECT_PIN_WALL_THIN, NO),
        (i::PROJECT_PIN_TOOL_MISMATCH, NO),
        (i::PROJECT_SHOE_GAP_TOO_LOW, H),
        (i::PROJECT_MEASURABILITY_ABSTAINED, NO),
        (i::PROJECT_CROSSES_STANDING_MATERIAL, L),
        (i::PROJECT_ENTRY_LOAD, L),
        (i::PROJECT_LINK_CUTS_STOCK, L),
        (i::PROJECT_PLUNGE_CLASS_LOAD, L),
        // G-ROUGHCOV: the Linking tab, as the other stock-reading rules above.
        (i::PROJECT_ROUGH_LEAVES_ISLANDS, L),
        (i::PROJECT_REST_SLOWER_THAN_FRESH, C),
        // Core keeps this id `pub(crate)`, so the row names its text.
        ("project.kinematic_utilization", NO),
        // The feeds calculator.
        (i::FEEDS_FEED_CLAMPED, F),
        (i::FEEDS_POWER_LIMITED, F),
        (i::FEEDS_POWER_LADDER_REDUCED_CUT, F),
        (i::FEEDS_SHANK_TOO_LARGE, F),
        (i::FEEDS_DOC_EXCEEDS_FLUTE, F),
        (i::FEEDS_SLOTTING_DETECTED, F),
        (i::FEEDS_SCALLOP_INVALID, F),
        (i::FEEDS_CHIPLOAD_BELOW_FLOOR, F),
        (i::FEEDS_LONG_TOOL_DERATE, F),
        (i::FEEDS_AGGRESSIVENESS_ENGAGEMENT, F),
        (i::FEEDS_RPM_LOWERED_FOR_CEILING, F),
        (i::FEEDS_DRILL_FEED_CLAMPED_TO_ENVELOPE, F),
        (i::FEEDS_VENDOR_ROW_PUBLISHES_NO_CHIPLOAD, F),
        (i::FEEDS_NO_VENDOR_ROWS_FOR_ROUTED_OPERATION, F),
        (i::FEEDS_DEPTH_BEYOND_PUBLISHED_TABLE, F),
        (i::FEEDS_FEED_VS_LUT_HIGH, F),
        (i::FEEDS_FEED_VS_LUT_LOW, F),
        (i::FEEDS_STEPOVER_VS_LUT, F),
        (i::FEEDS_DPP_VS_LUT, F),
        // Geometry of the op and its heights.
        (i::GEOM_STEPOVER_EXCEEDS_DIAMETER, C),
        (i::GEOM_DPP_EXCEEDS_CUTTING_LENGTH, C),
        (i::GEOM_BOTTOM_ABOVE_TOP_Z, H),
        (i::GEOM_FEED_Z_BELOW_TOP_Z, H),
        (i::GEOM_RETRACT_Z_BELOW_FEED_Z, H),
        (i::GEOM_CLEARANCE_Z_BELOW_RETRACT_Z, H),
        (i::GEOM_RETRACT_Z_BELOW_TOP_Z, H),
        (i::GEOM_PLUNGE_EXCEEDS_FEED, F),
        // The floor row of an op is on the Cut tab.
        (i::GEOM_DEPTH_BEYOND_STOCK, C),
        (i::GEOM_STANDING_MATERIAL, C),
        (i::GEOM_UNMACHINED_BAND, C),
        (i::GEOM_CLIPPED_BAND, G),
        (i::GEOM_TIP_FLOAT, C),
        (i::GEOM_RAMP_REACH_CLAMP, L),
        (i::GEOM_ZERO_REMOVAL, C),
        (i::GEOM_BURIED_CURVE, G),
        (i::GEOM_SELECTION_NOTE, G),
        (i::GEOM_TAB_PLACEMENT, C),
        (i::GEOM_OFFSET_LIBRARY_FAILURE, NO),
        (i::GEOM_BOUNDARY_CLIP_DROPPED, G),
        (i::GEOM_WATERLINE_LEVELS_BELOW_STOCK, C),
        (i::GEOM_WATERLINE_LADDER_EMPTY, C),
        // W4: the fix is a tabbed Profile after the Waterline; no tab of the
        // Waterline holds it.
        (i::GEOM_WATERLINE_PART_UNHELD, NO),
        (i::GEOM_REGION_CAP_TRUNCATED, C),
        (i::GEOM_REST_LEVELS_FALLBACK, C),
        (i::CONFIG_DERIVED_STEPOVER, C),
        (i::CONFIG_CLAIMS_REFERENCE, C),
        (i::CONFIG_INERT_CLAIMS_DIAL, C),
        // R2: the tool is the header Tool row, on no tab.
        (i::COMPAT_TOOL_KIND_REFUSED, NO),
        // R2: an op with no tool; the fix is the header Tool row.
        (i::PRECOND_NO_TOOL, NO),
        (i::COMPAT_BALL_NOSE_FLAT_CLEARING, C),
        (i::COMPAT_PROJECT_CURVE_POINT_PROJECTION, G),
        (i::TOOL_NAME_SIZE_MISMATCH, NO),
        (i::QUALITY_STEPOVER_OVER_80_PCT, C),
        (i::QUALITY_FINISH_STEPOVER_OVER_50_PCT, C),
        (i::QUALITY_BALL_SCALLOP_HEIGHT, C),
        (i::EFFICIENCY_VERY_FINE_STEPOVER, C),
        (i::STALE_DROP_CUTTER_MIN_Z, C),
        (i::STALE_TAPERED_BALL_PLUNGE, C),
        (i::STALE_WOOD_ADAPTIVE_STEPOVER, C),
        (i::STALE_PROJECT_CURVE_NEGATIVE_DEPTH, C),
        // A selection refusal and a model gouge: the Geometry tab.
        (i::PRECOND_DRILL_NO_HOLES, G),
        (i::PRECOND_ALIGNMENT_PIN_DRILL_NO_HOLES, NO),
        (i::PRECOND_SELECTION_REFUSED, G),
        (i::PRECOND_BURIED_FACE, G),
        // S11: an unset floor or depth is a Cut row; the text names the
        // field (the feeds rows are on the Feeds tab).
        (i::PRECOND_NOT_SET, C),
        (i::SAFETY_CUTS_INTO_MODEL, G),
        (i::PRECOND_PROJECT_CURVE_NO_CURVE, G),
        // R2: the fix is the Input row, on the Geometry tab.
        (i::PRECOND_PROJECT_CURVE_NO_SURFACE, G),
        // R2: the Input row is on the Geometry tab.
        (i::REF_MODEL_MISSING, G),
    ]
};

/// The tab that holds the fix of `id` ([`OWNER_TAB`]), or `None`.
pub(crate) fn owner_tab(id: &str) -> Option<ToolpathTab> {
    OWNER_TAB
        .iter()
        .find(|(row, _)| *row == id)
        .and_then(|(_, tab)| *tab)
}

pub(super) fn compute_tab_badges(
    entry: &ToolpathEntry,
    diagnostics: &[rs_cam_core::diagnostics::Diagnostic],
    toolpath_heights: Option<&rs_cam_core::session::ToolpathHeights>,
    shoe_gap_too_low: bool,
) -> TabBadges {
    use rs_cam_core::diagnostics::{Category, DiagnosticState, Severity};

    let mut badges = TabBadges {
        dots: [None; ToolpathTab::ALL.len()],
    };

    // F24: each actionable Current finding raises the dot of the tab that
    // holds its fix ([`owner_tab`]). Pre-sim hints and State (workflow)
    // notices stay quiet, so a healthy op shows no dot.
    for d in diagnostics {
        if d.state != DiagnosticState::Current
            || matches!(d.category, Category::State | Category::Efficiency)
        {
            continue;
        }
        let color = match d.severity {
            Severity::Blocking | Severity::Critical => crate::ui::tokens::DANGER,
            Severity::Caution => crate::ui::tokens::CAUTION,
            Severity::Info | Severity::Hint => continue,
        };
        if let Some(tab) = owner_tab(d.id.as_str()) {
            badges.raise(tab, color);
        }
    }

    // Heights: the plane order (F033) has its own rule.
    if let Some(color) = toolpath_heights
        .and_then(|answer| height_order_badge(answer, &entry.heights, &entry.operation))
    {
        badges.raise(ToolpathTab::Heights, color);
    }
    // H2: a stated shoe gap below the solver gap is a Caution about the
    // shoe row, which this tab holds.
    if shoe_gap_too_low {
        badges.raise(ToolpathTab::Heights, crate::ui::tokens::CAUTION);
    }
    // R5 (2026-09-18): a pinned height is a fact the tab states before it
    // opens. A diagram drag pinned Top at −1.37 and nothing on the tab strip
    // said so (`planning/corne_case_analysis_2026-09-18/ANALYSIS.md` §4.2).
    // INFO is the badge's neutral role; a warning outranks it.
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
    if let Some(color) = any_pinned.then_some(crate::ui::tokens::INFO) {
        badges.raise(ToolpathTab::Heights, color);
    }

    // Feeds: the warnings of the feeds result.
    if let Some(r) = entry.feeds_result.as_ref()
        && !r.warnings.is_empty()
    {
        badges.raise(
            ToolpathTab::FeedsSpeeds,
            if r.power_limited {
                crate::ui::tokens::CAUTION
            } else {
                crate::ui::tokens::INFO
            },
        );
    }

    badges
}

/// F033: the Heights tab badge colour of the plane order, or `None`.
///
/// THE core rule (`ToolpathHeights::height_order`, the R7 anchor) and the
/// one severity of each kind (`height_order_severity`, which the
/// diagnostics adapter also reads): `Critical` is DANGER, `Caution` is
/// CAUTION. The badge keeps no comparison of its own.
pub fn height_order_badge(
    answer: &rs_cam_core::session::ToolpathHeights,
    heights: &rs_cam_core::compute::config::HeightsConfig,
    op: &rs_cam_core::compute::catalog::OperationConfig,
) -> Option<egui::Color32> {
    use rs_cam_core::diagnostics::Severity;
    use rs_cam_core::diagnostics::adapters::from_static_checks::height_order_severity;
    let worst = answer
        .height_order(heights, op.op_type())
        .into_iter()
        .map(height_order_severity)
        .max()?;
    match worst {
        Severity::Blocking | Severity::Critical => Some(crate::ui::tokens::DANGER),
        Severity::Caution => Some(crate::ui::tokens::CAUTION),
        Severity::Info | Severity::Hint => None,
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
    // L-2 (geometry selection P4.3): a selection refusal has one owner,
    // the Selection section on the Geometry tab, which holds the text and
    // the fix. The footer row names that section in one line; the hover
    // keeps the whole refusal.
    let face = if is_selection_refusal(d) {
        format!("{prefix}the selection refuses. See the Geometry tab.")
    } else {
        format!("{prefix}{}", d.message)
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
                    egui::Label::new(egui::RichText::new(face).small().color(color))
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

/// R2 / 6.F4: the refusal text of the tool `id` for the op of `inputs`, or
/// `None` when the op accepts it.
pub(crate) fn tool_refusal(
    inputs: &super::ToolpathPanelInputs,
    id: crate::state::job::ToolId,
) -> Option<&str> {
    inputs
        .tool_refusals
        .iter()
        .find(|(tool, _)| *tool == id)
        .map(|(_, text)| text.as_str())
}

/// True when `d` is the refusal of the op selection: the footer row of
/// `precondition.selection_refused`, and its drill form
/// `precondition.drill_no_holes` (both carry the `selection_status`
/// refusal).
fn is_selection_refusal(d: &rs_cam_core::diagnostics::Diagnostic) -> bool {
    use rs_cam_core::diagnostics::ids;
    let id = d.id.as_str();
    id == ids::PRECOND_SELECTION_REFUSED || id == ids::PRECOND_DRILL_NO_HOLES
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

pub(super) fn draw_toolpath_tabs(
    ui: &mut egui::Ui,
    active: &mut ToolpathTab,
    badges: &TabBadges,
    operation: &crate::state::toolpath::OperationConfig,
) {
    // The strip is the shared `components::TabStrip` (inspector clean-up
    // round 3); the Simulation inspector draws the same one. It keeps one
    // row (IC3). R2: a tab with no control for the op is not drawn
    // (`ToolpathTab::applies`).
    let strip = ToolpathTab::ALL
        .iter()
        .filter(|tab| tab.applies(operation))
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
             the fine pass. Raise Min Valley Depth, or turn on \
             Rest machining: from previous ops for a true rest picture."
                .to_owned(),
        ),
    };
    crate::ui::components::OneLine::new(face)
        .color(crate::ui::tokens::CAUTION)
        .detail(detail)
}

/// The label of the rest machining row (L5, operator ruling 2026-10-23:
/// the Fusion form, one checkbox). On = `StockSource::FromRemainingStock`,
/// off = `StockSource::Fresh`. `StockSource` has these two values only.
pub const REST_MACHINING_LABEL: &str = "Rest machining: from previous ops";

/// The rest machining row: ONE checkbox that writes `source` (G-STARTFROM:
/// the one writer of `stock_source`). The response reports `changed` on a
/// click; the caller stamps the toolpath stale on that report. IC1: the
/// row is one line; in a narrow rail the label truncates, and the hover
/// gives the full text.
pub fn rest_machining_row(ui: &mut egui::Ui, source: &mut StockSource) -> egui::Response {
    let mut on = *source == StockSource::FromRemainingStock;
    let response = ui
        .scope(|ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
            ui.checkbox(&mut on, REST_MACHINING_LABEL)
        })
        .inner
        .on_hover_text(START_FROM_HOVER);
    if response.changed() {
        *source = if on {
            StockSource::FromRemainingStock
        } else {
            StockSource::Fresh
        };
    }
    response
}

/// One hover for the row. It names what each option does, because the
/// difference between them is the whole question.
const START_FROM_HOVER: &str = concat!(
    "Stock: the toolpath starts from the raw stock. ",
    "Rest machining: the toolpath starts from the material the earlier ",
    "operations in this setup leave, and needs their simulation first."
);

/// The text of the header Tool row while the op has no tool (R2, P1).
pub(crate) const CHOOSE_A_TOOL: &str = "Choose a tool";

/// The Tool row entry that opens the tool library modal.
pub(crate) const ADD_FROM_LIBRARY: &str = "Add from library\u{2026}";

/// The Tool row entry that adds a new tool of a kind the op takes.
pub(crate) const NEW_TOOL: &str = "New tool\u{2026}";

/// The hover of the header Tool row.
const TOOL_ROW_HOVER: &str = "The cutter this toolpath runs. A grey tool is one the \
    operation refuses; its hover gives the reason.";

/// The header Tool row (R2, P1): the ONE tool control of the inspector, a
/// `PropRow` dropdown under the name.
///
/// The list holds the project tools (a tool the op refuses is grey, with
/// the refusal of core `tool_fit` on hover, 6.F4), a separator, then
/// [`ADD_FROM_LIBRARY`] (the tool library modal, the route of the Setup
/// rail) and [`NEW_TOOL`] (the add-tool route, with the first tool kind
/// the op takes). P3: core `add_tool` binds a new fitting tool to this op
/// while it has no tool. With no tool bound the row reads
/// [`CHOOSE_A_TOOL`] in a DANGER outline; the no-tool state is its hover.
pub(super) fn draw_tool_row(
    ui: &mut egui::Ui,
    entry: &mut ToolpathEntry,
    inputs: &super::ToolpathPanelInputs,
    events: &mut Vec<crate::ui::AppEvent>,
) {
    let tools = inputs.tools.as_slice();
    let current = tools
        .iter()
        .find(|(id, _, _)| Some(*id) == entry.tool_id)
        .map(|(_, name, _)| name.clone());
    let state = current
        .is_none()
        .then_some(crate::ui::components::Role::Danger);
    let detail = inputs
        .no_tool_state
        .clone()
        .unwrap_or_else(|| CHOOSE_A_TOOL.to_owned());
    // The first tool kind the op takes, for "New tool…".
    let constraints = entry.operation.op_type().registry_entry().tool_constraints;
    let new_kind = crate::state::job::ToolType::ALL
        .iter()
        .copied()
        .find(|tt| constraints.allows(tt.cutter_kind()));
    let row = crate::ui::components::PropRow::new("Tool")
        .state(state)
        .hover(Some(TOOL_ROW_HOVER))
        .detail(Some(detail.as_str()));
    let out = row.show(ui, |ui| {
        let selected = match current.as_deref() {
            Some(name) => egui::RichText::new(name),
            None => egui::RichText::new(CHOOSE_A_TOOL).color(crate::ui::tokens::DANGER),
        };
        egui::ComboBox::from_id_salt("tp_tool")
            .truncate()
            .width(ui.available_width())
            .selected_text(selected)
            .show_ui(ui, |ui| {
                for (id, name, _) in tools {
                    // R2 / 6.F4: a tool that the op refuses is grey, with
                    // the refusal of core `tool_fit` (the add's door).
                    let refusal = tool_refusal(inputs, *id);
                    let row = ui.add_enabled_ui(refusal.is_none(), |ui| {
                        ui.selectable_value(&mut entry.tool_id, Some(*id), name.as_str())
                    });
                    if let Some(text) = refusal {
                        row.response.on_disabled_hover_text(text);
                    }
                }
                if !tools.is_empty() {
                    ui.separator();
                }
                if ui
                    .selectable_label(false, ADD_FROM_LIBRARY)
                    .on_hover_text("Open the tool library and add a tool to the project.")
                    .clicked()
                {
                    events.push(crate::ui::AppEvent::Ui(
                        crate::ui_command::UiCommand::OpenToolLibrary(crate::ui_command::NoArgs),
                    ));
                }
                if let Some(kind) = new_kind
                    && ui
                        .selectable_label(false, NEW_TOOL)
                        .on_hover_text(format!(
                            "Add a new {} to the project. This toolpath takes it while it \
                             has no tool.",
                            kind.label().to_lowercase()
                        ))
                        .clicked()
                {
                    events.push(crate::ui::AppEvent::AddTool(kind));
                }
            })
            .response
    });
    let _ = out.inner.on_hover_text(&out.hover);
}

/// The Input row of the Geometry tab (R2: it was on the Cut tab) and the
/// BREP warning of the model. The row keeps its pre-R2 form; W3a moves it
/// onto `PropRow`.
pub(super) fn draw_input_row(
    ui: &mut egui::Ui,
    entry: &mut ToolpathEntry,
    inputs: &super::ToolpathPanelInputs,
) {
    let models = inputs.models.as_slice();
    let model_is_step_missing_brep = inputs.model_is_step_missing_brep;
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

    // Geometry selection P4.3: the face picker block is in the Selection
    // section (`selection_section.rs`). The Geometry tab draws that section
    // directly below this row.
}

/// The rest machining row and its rest comparison: the first rows of the
/// Passes tab (R2).
pub(super) fn draw_start_from(
    ui: &mut egui::Ui,
    entry: &mut ToolpathEntry,
    inputs: &super::ToolpathPanelInputs,
    events: &mut Vec<crate::ui::AppEvent>,
) {
    let rest_fresh = inputs.rest_fresh;
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
    let resp = rest_machining_row(ui, &mut entry.stock_source);
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
