//! The toolpath tab strip and its dots, the header Tool row, and the first
//! rows of the Geometry and Passes tabs. W2b: the dots of the findings come
//! from `placement::place` (the anchor names the field, the field names its
//! tab); the footer rows are gone.

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

/// The dot of each tab. W2b: a field finding dots the tab that draws its
/// field (`placement::field_tab`, through `Placement::dots`); a tool or an
/// operation finding dots no tab. The plane order, a pinned height and the
/// feeds warnings keep their own rules below.
pub(super) fn compute_tab_badges(
    entry: &ToolpathEntry,
    placement: &super::placement::Placement<'_>,
    toolpath_heights: Option<&rs_cam_core::session::ToolpathHeights>,
) -> TabBadges {
    let mut badges = TabBadges {
        dots: [None; ToolpathTab::ALL.len()],
    };
    for (tab, role) in &placement.dots {
        badges.raise(*tab, role.text());
    }

    // Heights: the plane order (F033) has its own rule.
    if let Some(color) = toolpath_heights
        .and_then(|answer| height_order_badge(answer, &entry.heights, &entry.operation))
    {
        badges.raise(ToolpathTab::Heights, color);
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

    // The feeds rows (P5b: on the Passes tab): the warnings of the feeds
    // result.
    if let Some(r) = entry.feeds_result.as_ref()
        && !r.warnings.is_empty()
    {
        badges.raise(
            ToolpathTab::Passes,
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
        Severity::Severe | Severity::Critical => Some(crate::ui::tokens::DANGER),
        Severity::Caution => Some(crate::ui::tokens::CAUTION),
        Severity::Info | Severity::Hint => None,
    }
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
pub const REST_MACHINING_LABEL: &str = "Rest machining";

/// The text of the checkbox of the rest machining row (W3b: the label
/// column holds [`REST_MACHINING_LABEL`]).
pub const REST_MACHINING_CHECK: &str = "From previous ops";

/// The rest machining row: ONE checkbox that writes `source` (G-STARTFROM:
/// the one writer of `stock_source`). The response reports `changed` on a
/// click; the caller stamps the toolpath stale on that report. IC1: the
/// row is one line; in a narrow rail the label truncates, and the hover
/// gives the full text.
pub fn rest_machining_row(ui: &mut egui::Ui, source: &mut StockSource) -> egui::Response {
    let mut on = *source == StockSource::FromRemainingStock;
    // W3g: the checkbox form of the row: the box and its text, no well.
    let response = crate::ui::components::PropRow::new(REST_MACHINING_LABEL)
        .hover(Some(START_FROM_HOVER))
        .checkbox(ui, &mut on, REST_MACHINING_CHECK)
        .inner;
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
/// W2b: `findings` are the findings `placement::place` anchors to the tool
/// (`Anchor::Tool`): they set the row state, and their faces are its hover.
pub(super) fn draw_tool_row(
    ui: &mut egui::Ui,
    entry: &mut ToolpathEntry,
    inputs: &super::ToolpathPanelInputs,
    findings: &[super::placement::Finding<'_>],
    events: &mut Vec<crate::ui::AppEvent>,
) {
    let tools = inputs.tools.as_slice();
    let current = tools
        .iter()
        .find(|(id, _, _)| Some(*id) == entry.tool_id)
        .map(|(_, name, _)| name.clone());
    let state = if current.is_none() {
        Some(crate::ui::components::Role::Danger)
    } else {
        super::placement::tool_role(findings)
    };
    let detail = match (
        &inputs.no_tool_state,
        super::placement::tool_hover(findings),
    ) {
        (Some(no_tool), _) => no_tool.clone(),
        (None, Some(hover)) => hover,
        (None, None) => CHOOSE_A_TOOL.to_owned(),
    };
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
    // W3g: the closed dropdown shows the short label of the tool (core
    // `ToolConfig::short_label`, the type and the size that sets the cut);
    // the list and the hover show the full name.
    let short = inputs
        .tool_configs
        .iter()
        .find(|(id, _)| Some(*id) == entry.tool_id)
        .map(|(_, tc)| tc.short_label());
    let out = row.show(ui, |ui| {
        let selected = match (short.as_deref(), current.as_deref()) {
            (Some(text), _) | (None, Some(text)) => egui::RichText::new(text),
            (None, None) => egui::RichText::new(CHOOSE_A_TOOL).color(crate::ui::tokens::DANGER),
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
    let hover = match current.as_deref() {
        Some(name) => format!("{name}\n\n{}", out.hover),
        None => out.hover,
    };
    let _ = out.inner.on_hover_text(hover);
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
    // W3b: one read-only `PropRow`: the two cycle times, and the quiet
    // action that asks for the comparison.
    if entry.stock_source == StockSource::FromRemainingStock {
        let value = rest_fresh.map_or_else(
            || "Not compared".to_owned(),
            |times| {
                format!(
                    "rest {:.1} s \u{00B7} fresh {:.1} s",
                    times.rest_s, times.fresh_s
                )
            },
        );
        let out = crate::ui::components::PropRow::new("Fresh stock")
            .hover(Some(COMPARE_WITH_FRESH_HOVER))
            .action("Compare")
            .show_read_only(ui, value);
        if out.action_clicked {
            events.push(crate::ui::AppEvent::CompareRestWithFresh(entry.id));
        }
    }
}

/// The hover of the "Compare with fresh" button.
const COMPARE_WITH_FRESH_HOVER: &str = "Generate this operation once more on fresh stock and \
    compare the two cycle times (the simulator's model, at the planned feeds). Rest can take \
    longer: each island needs its own entry. The answer stays until the operation changes.";
