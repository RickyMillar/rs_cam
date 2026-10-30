//! The Geometry tab (R2) of the toolpath inspector: the Input row, the op
//! selection, the Machining Boundary section and the Reach row.

use crate::state::toolpath::{BoundaryContainment, BoundarySource};
use crate::ui::AppEvent;
use crate::ui::components::{PropRow, Role};
use crate::ui::properties::tab_badges::{draw_input_row, rest_region_pathology_line};
use crate::ui::properties::{ReachPanelSummary, ToolpathPanelInputs, boundary_summary_line};

/// G-C (§2 d): the act on the Containment row that writes `Inside`.
pub(crate) const USE_INSIDE: &str = "Use Inside";

/// The Geometry tab (R2): the Input row, the op selection and the floor
/// (`geometry_tab.rs`, events only, L-1), the Machining Boundary section
/// and its selection list, then the Reach row of a reach-capable op.
/// `properties::tests` renders it (D3, G-C, G-D, R7a).
pub(in crate::ui::properties) fn draw_geometry_tab(
    ui: &mut egui::Ui,
    entry: &mut crate::state::toolpath::ToolpathEntry,
    inputs: &ToolpathPanelInputs,
    selection_ui: &mut crate::state::pick::SelectionUi,
    containment_autocorrect: &mut crate::state::panels::ContainmentAutoCorrect,
    reach_view: &mut ReachView,
    events: &mut Vec<AppEvent>,
) {
    ui.add_space(4.0);
    draw_input_row(ui, entry, inputs);
    crate::ui::properties::geometry_tab::draw_geometry_tab(ui, entry, inputs, selection_ui, events);
    ui.add_space(6.0);
    draw_machining_boundary(ui, entry, inputs, containment_autocorrect);
    crate::ui::properties::geometry_tab::draw_boundary_selection(
        ui,
        entry,
        inputs,
        selection_ui,
        events,
    );
    if entry.operation.op_type().supports_reach_map() {
        ui.add_space(6.0);
        if draw_reach_row(ui, &inputs.reach, reach_view) {
            reach_view.toggle = true;
        }
    }
}

/// The Machining Boundary section of the Geometry tab (R2: it was at the
/// foot of the Cut tab). It writes the draft entry: the enable box, the
/// source, the holes option, the rest source, the containment and the
/// offset. The rows keep their pre-R2 form; W3a moves them onto `PropRow`.
fn draw_machining_boundary(
    ui: &mut egui::Ui,
    entry: &mut crate::state::toolpath::ToolpathEntry,
    inputs: &ToolpathPanelInputs,
    containment_autocorrect: &mut crate::state::panels::ContainmentAutoCorrect,
) {
    containment_autocorrect.scope(entry.id);
    let tools = inputs.tools.as_slice();
    let models = inputs.models.as_slice();
    let boundary_source_candidates = inputs.boundary_source_candidates.as_slice();
    // W3.2: the boundary defines *what region to cut*, so it lives
    // under Geometry (R2: it was at the foot of the Cut tab).
    // D3: an op that no boundary moves (`TerritoryXy::SetupMap`, Pin
    // Drill: its holes are absolute setup-map positions) draws no
    // boundary controls. The panel reads the capability, not the op type.
    if entry
        .operation
        .capabilities()
        .territory_xy
        .session_clip_applies()
    {
        crate::ui::components::SectionHeader::new("Machining Boundary").show(ui);
        ui.checkbox(&mut entry.boundary.enabled, "Enable boundary")
            .on_hover_text(
                "Restrict toolpath to a boundary polygon. \
                 Moves outside the boundary are converted to rapids at safe Z.",
            );
        if entry.boundary.enabled {
            // R7a: the source buttons write the offset of THE source-change
            // rule (`BoundaryConfig::with_source`), which reads the tool
            // diameter of the op.
            let tool_diameter = entry
                .tool_id
                .and_then(|tool_id| tools.iter().find(|(id, _, _)| *id == tool_id))
                .map(|(_, _, diameter)| *diameter);
            // UX-R03-009 (G-BOUNDARYINHERIT): the stored source is the
            // one generation uses (`session/compute.rs` clones
            // `tc.boundary` unconditionally). G-D (ruling D-1): its name is
            // the hover of the Source combo, not a row above it; the row
            // repeated the combo. The controller assigns the model
            // silhouette on add for 3D ops on a mesh; that provenance is
            // not stored, so the hover never claims "(auto)".
            let source_summary = boundary_summary_line(&entry.boundary, false, models);
            // Source selector
            ui.horizontal(|ui| {
                ui.label("Source:");
                egui::ComboBox::from_id_salt("boundary_source")
                    .truncate()
                    .selected_text(entry.boundary.source.label())
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(
                                matches!(entry.boundary.source, BoundarySource::Stock),
                                "Stock",
                            )
                            .on_hover_text("Use the stock bounding rectangle")
                            .clicked()
                        {
                            entry.boundary.source = BoundarySource::Stock;
                        }
                        if ui
                            .selectable_label(
                                matches!(entry.boundary.source, BoundarySource::ModelSilhouette),
                                "Model Silhouette",
                            )
                            .on_hover_text(
                                "XY projection of the 3D model. Only machines \
                                 where the model actually is.",
                            )
                            .clicked()
                        {
                            entry.boundary = entry
                                .boundary
                                .with_source(BoundarySource::ModelSilhouette, tool_diameter);
                        }
                        // P4.3: the one door to a `Selection` boundary. It
                        // is enabled when a model holds a drawing item or a
                        // face item, so a STEP-only project reaches it.
                        let selection_enabled = inputs.boundary_selection_enabled;
                        if ui
                            .add_enabled(
                                selection_enabled,
                                egui::Button::selectable(
                                    matches!(
                                        entry.boundary.source,
                                        BoundarySource::Selection { .. }
                                    ),
                                    "Selection",
                                ),
                            )
                            .on_hover_text(if selection_enabled {
                                "Use a selection of items as the boundary: the closed \
                                 shapes of a drawing, or faces picked in the viewport. \
                                 The items can come from another model. Holes in the \
                                 shapes stay uncut. Edit the selection below."
                            } else {
                                "No model in this project has a drawing item or a face. \
                                 Import a DXF, an SVG or a STEP model to use this source."
                            })
                            .clicked()
                        {
                            // Keep the current selection when the source is
                            // already a selection; else write the value that a
                            // boundary Reset restores: the closed shapes of the
                            // first model with a closed shape, else no item.
                            // The source change writes offset 0 (R7a).
                            if !matches!(entry.boundary.source, BoundarySource::Selection { .. }) {
                                entry.boundary = entry.boundary.with_source(
                                    BoundarySource::Selection {
                                        selection: inputs.boundary_default.clone(),
                                        holes: false,
                                    },
                                    tool_diameter,
                                );
                            }
                        }
                        let has_rest_candidates = !boundary_source_candidates.is_empty();
                        if ui
                            .add_enabled(
                                has_rest_candidates,
                                egui::Button::selectable(
                                    matches!(
                                        entry.boundary.source,
                                        BoundarySource::DerivedRestRegions { .. }
                                    ),
                                    "Rest Regions",
                                ),
                            )
                            .on_hover_text(if has_rest_candidates {
                                "The regions another toolpath leaves: the pieces a \
                                 Geodesic Adaptive rough did not cut, or another op's \
                                 rest analysis. Pick the source below."
                            } else {
                                "No other toolpaths in this project yet — add \
                                 one and generate it to use as the source. Picking it here \
                                 switches its rest analysis on."
                            })
                            .clicked()
                        {
                            let default_source = boundary_source_candidates
                                .first()
                                .map(|(candidate_id, _, _, _, _, _)| *candidate_id)
                                .unwrap_or(entry.id);
                            entry.boundary.source = BoundarySource::DerivedRestRegions {
                                source_toolpath_id: default_source,
                            };
                            // G-C (D-2): the pick may set `Inside`.
                            pick_sets_inside(
                                &mut entry.boundary.containment,
                                inputs.rest_pick.as_ref(),
                                default_source,
                                containment_autocorrect,
                            );
                        }
                    })
                    .response
                    .on_hover_text(source_summary);
            });

            // GT-Q7: the boundary selection list follows this section on
            // the same tab. This section keeps the source and the holes
            // option.
            if let BoundarySource::Selection { holes, .. } = &mut entry.boundary.source {
                // The holes option inverts the regions: the toolpath cuts
                // inside the holes of the shapes and stays off the shapes.
                ui.checkbox(holes, "Machine the holes").on_hover_text(
                    "The toolpath cuts inside the holes of the shapes, for example \
                     the area inside an edge band, and stays off the shapes.",
                );
            }

            // Rest-regions source-toolpath picker (P2.2) — only shown
            // when `Source` above is set to `DerivedRestRegions`.
            // Candidates are every other toolpath in the session;
            // ones with a cached result whose rest regions are
            // already non-empty are labelled "· N regions" (G-C) and
            // sorted first, but a not-yet-generated toolpath is
            // still selectable — generation fails hard with a clear
            // message if the source turns out unusable.
            if let BoundarySource::DerivedRestRegions { source_toolpath_id } =
                &mut entry.boundary.source
            {
                ui.horizontal(|ui| {
                    ui.label("Rest source:");
                    let current = boundary_source_candidates
                        .iter()
                        .find(|(candidate_id, _, _, _, _, _)| candidate_id == source_toolpath_id);
                    let current_label = current.map_or_else(
                        || "(toolpath not found)".to_owned(),
                        |(_, name, _, regions, _, _)| rest_source_label(name, regions.as_deref()),
                    );
                    // G-C (§6 b): the top of the regions first, then the
                    // combo's own hover.
                    let hover = match current.and_then(|(_, _, _, _, _, top_z)| *top_z) {
                        Some(z) => format!(
                            "Top of the regions: Z {z:.2}.\nThe toolpath whose regions bound \
                             this one. Picking it switches its rest analysis on."
                        ),
                        None => "The toolpath whose regions bound this one. Picking it \
                                 switches its rest analysis on."
                            .to_owned(),
                    };
                    let mut sorted = boundary_source_candidates.to_vec();
                    sorted.sort_by_key(|(_, _, ready, _, _, _)| !*ready);
                    egui::ComboBox::from_id_salt("boundary_rest_source")
                        .truncate()
                        .selected_text(current_label)
                        .show_ui(ui, |ui| {
                            for (candidate_id, name, _, regions, _, _) in &sorted {
                                let label = rest_source_label(name, regions.as_deref());
                                let selected = *source_toolpath_id == *candidate_id;
                                if ui.selectable_label(selected, label).clicked() {
                                    *source_toolpath_id = *candidate_id;
                                }
                            }
                        })
                        .response
                        .on_hover_text(hover);
                });

                // Sliver-storm / giant-region caption (2026-07-06
                // incident): the SOURCE toolpath is who suffers the
                // per-island generation explosion or the "barely
                // restricts anything" giant-region case, so classify
                // ITS cached regions (captured in
                // `boundary_source_candidates` alongside `ready`),
                // not this consumer's own (this toolpath has none —
                // it's the one consuming the boundary).
                //
                // LH-2: the denominator is the SOURCE toolpath's own
                // rest-grid footprint (captured alongside its
                // regions), not this model's bounding rectangle.
                let selected = boundary_source_candidates
                    .iter()
                    .find(|(candidate_id, _, _, _, _, _)| candidate_id == source_toolpath_id);
                let selected_regions =
                    selected.and_then(|(_, _, _, regions, _, _)| regions.as_ref());
                // `None` twice over: no candidate selected, or the
                // selected one has no rest grid. Both are "not
                // measured", and `classify_rest_regions` stays silent.
                let source_footprint_area = selected.and_then(|(_, _, _, _, area, _)| *area);
                if let Some(regions) = selected_regions
                    && let Some(pathology) = rs_cam_core::surface::rest_field::classify_rest_regions(
                        regions,
                        source_footprint_area,
                    )
                {
                    ui.add(rest_region_pathology_line(pathology));
                }
            }

            // Containment mode. G-C (§2 c-e): for a rest-regions source
            // whose tool is smaller than this one, `Center` and `Outside`
            // are grey (they refuse, D53) with the sizes on hover; while the
            // draft holds one of them, `[Use Inside]` sits at the right of
            // the row, unless `Inside` would collapse the boundary. The `⟲`
            // badge marks a containment the `Rest Regions` pick set (D-2).
            let draft_source = match entry.boundary.source {
                BoundarySource::DerivedRestRegions { source_toolpath_id } => {
                    Some(source_toolpath_id)
                }
                _ => None,
            };
            let refused = inputs
                .rest_containment
                .as_ref()
                .filter(|check| Some(check.larger.source_toolpath_id) == draft_source);
            let before = entry.boundary.containment;
            ui.horizontal(|ui| {
                ui.label("Containment:");
                egui::ComboBox::from_id_salt("boundary_contain")
                    .truncate()
                    .selected_text(match entry.boundary.containment {
                        BoundaryContainment::Center => "Center",
                        BoundaryContainment::Inside => "Inside",
                        BoundaryContainment::Outside => "Outside",
                    })
                    .show_ui(ui, |ui| {
                        let mut option = |ui: &mut egui::Ui,
                                          value: BoundaryContainment,
                                          label: &str,
                                          hover: &str| {
                            let enabled = refused.is_none() || value == BoundaryContainment::Inside;
                            let response = ui
                                .add_enabled(
                                    enabled,
                                    egui::Button::selectable(
                                        entry.boundary.containment == value,
                                        label,
                                    ),
                                )
                                .on_hover_text(hover);
                            let response = match refused {
                                Some(check) => response.on_disabled_hover_text(check.hover()),
                                None => response,
                            };
                            if response.clicked() {
                                entry.boundary.containment = value;
                            }
                        };
                        option(
                            ui,
                            BoundaryContainment::Center,
                            "Center",
                            "Tool center stays inside boundary",
                        );
                        option(
                            ui,
                            BoundaryContainment::Inside,
                            "Inside",
                            "Entire tool stays inside boundary (shrinks by tool radius)",
                        );
                        option(
                            ui,
                            BoundaryContainment::Outside,
                            "Outside",
                            "Tool edge can extend outside boundary",
                        );
                    });
                if let Some(hover) = containment_autocorrect.hover() {
                    ui.add(
                        crate::ui::components::ProvenanceBadge::new(
                            crate::ui::components::ProvKind::AutoCorrect,
                        )
                        .compact(),
                    )
                    .on_hover_text(hover);
                }
                if let Some(check) = refused
                    && entry.boundary.containment != BoundaryContainment::Inside
                {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let keeps = check.inside == crate::state::runtime::InsideAnswer::Keeps;
                        let response = ui
                            .add_enabled(keeps, egui::Button::new(USE_INSIDE).small())
                            .on_hover_text(check.hover())
                            .on_disabled_hover_text(check.hover());
                        if response.clicked() {
                            entry.boundary.containment = BoundaryContainment::Inside;
                        }
                    });
                }
            });
            // The badge stands until the operator edits the containment.
            if entry.boundary.containment != before {
                containment_autocorrect.clear();
            }

            // Offset
            ui.horizontal(|ui| {
                ui.label("Offset:");
                ui.add(
                    egui::DragValue::new(&mut entry.boundary.offset)
                        .speed(0.1)
                        .suffix(" mm"),
                )
                .on_hover_text(
                    "Expand (positive) or shrink (negative) the boundary. \
                     Applied before tool-radius containment.",
                );
            });
        }
    }
}

/// The Reach row of the Geometry tab (R3): one read-only `PropRow`. The
/// value is the unreachable share of the reach overlay (CAUTION above
/// zero), or "computing…". The quiet action Show/Hide asks the caller to
/// switch the overlay; the dock and the legend own the rest. True when the
/// action was clicked.
fn draw_reach_row(ui: &mut egui::Ui, reach: &ReachPanelSummary, view: &ReachView) -> bool {
    let (value, state, detail): (String, Option<Role>, Option<String>) = match reach {
        ReachPanelSummary::Measured {
            unreachable_pct,
            max_gap_mm,
            grid_line,
            area_basis_note,
            over_statement_note,
            tolerance_below_floor,
        } => {
            // The base comes from `ReachMap::area_basis_note`; the bias
            // sentence is `ReachMap::over_statement_note`, quoted on the
            // hover only when the bar is under the grid floor.
            let mut detail =
                format!("Worst gap {max_gap_mm:.2} mm. {area_basis_note}\n{grid_line}");
            if *tolerance_below_floor {
                detail.push('\n');
                detail.push_str(over_statement_note);
            }
            (
                format!("Unreachable {unreachable_pct:.0} %"),
                (*unreachable_pct > 0.0).then_some(Role::Caution),
                Some(detail),
            )
        }
        ReachPanelSummary::Computing => ("computing\u{2026}".to_owned(), None, None),
        ReachPanelSummary::NotMeasured => (
            "not measured".to_owned(),
            Some(Role::Caution),
            Some("The reach walk found no upward surface to judge.".to_owned()),
        ),
        ReachPanelSummary::Failed(message) => (
            "failed".to_owned(),
            Some(Role::Caution),
            Some(message.clone()),
        ),
        ReachPanelSummary::Idle => {
            let value = if view.on { "waiting" } else { "off" };
            (value.to_owned(), None, None)
        }
    };
    let action = if view.on { "Hide" } else { "Show" };
    let row = PropRow::new("Reach")
        .state(state)
        .hover(Some(REACH_ROW_HOVER))
        .detail(detail.as_deref());
    // A refused switch has no action; the reason is the hover.
    let row = match (&view.refusal, view.on) {
        (Some(_), false) => row,
        _ => row.action(action),
    };
    let out = row.show_read_only(ui, value);
    if let Some(reason) = view.refusal.as_ref().filter(|_| !view.on) {
        let _ = out
            .response
            .on_hover_text(format!("Show is not available: {reason}."));
    }
    out.action_clicked
}

/// The hover of the Reach row.
const REACH_ROW_HOVER: &str = "The share of the surface that the cutter of this toolpath \
    cannot form within its tolerance. Show colours the model; the legend explains the colours.";

/// R3: the reach overlay as the Reach row reads it, and the Show/Hide
/// request of the row. The caller fills `on` and `refusal` from the overlay
/// registry row `reach_map` and, when `toggle` is set after the draw,
/// switches the overlay through `registry::set_overlay`.
#[derive(Default)]
pub(crate) struct ReachView {
    /// The reach overlay is on.
    pub on: bool,
    /// Why the overlay cannot go on now (the registry precondition).
    pub refusal: Option<String>,
    /// The row asked to switch the overlay this frame.
    pub toggle: bool,
}

/// G-C (D-2, ruled yes): the `Rest Regions` pick of `source` writes
/// `Inside` when this op's tool is larger than the source's (`pick`), the
/// containment is not `Inside` yet, and `Inside` keeps area (the collapse
/// check, G-BOUNDCOLLAPSE). The Containment combo then shows the `⟲` badge
/// with the sizes on its hover. True when it wrote `Inside`.
pub(in crate::ui::properties) fn pick_sets_inside(
    containment: &mut BoundaryContainment,
    pick: Option<&crate::ui::properties::RestContainment>,
    source: crate::state::toolpath::ToolpathId,
    autocorrect: &mut crate::state::panels::ContainmentAutoCorrect,
) -> bool {
    let Some(pick) = pick.filter(|pick| {
        pick.larger.source_toolpath_id == source
            && pick.inside == crate::state::runtime::InsideAnswer::Keeps
    }) else {
        return false;
    };
    if *containment == BoundaryContainment::Inside {
        return false;
    }
    *containment = BoundaryContainment::Inside;
    autocorrect.set(format!(
        "Set to Inside: this tool (\u{00D8}{:.2}) is larger than the tool of '{}' \
         (\u{00D8}{:.2}).",
        2.0 * pick.larger.tool_radius_mm,
        pick.larger.source_name,
        2.0 * pick.larger.source_tool_radius_mm,
    ));
    true
}

/// G-C (§6, review): the item text of a rest source: `{name} · {N} regions`
/// when its result has regions, else the plain name. One phrase for the
/// count, as the source's own disclosure title.
pub(in crate::ui::properties) fn rest_source_label(
    name: &str,
    regions: Option<&Vec<rs_cam_core::polygon::Polygon2>>,
) -> String {
    match regions.map(Vec::len) {
        Some(n) if n > 0 => format!("{name} \u{00B7} {}", regions_phrase(n)),
        _ => name.to_owned(),
    }
}

/// `N regions` (`1 region`).
pub(in crate::ui::properties) fn regions_phrase(n: usize) -> String {
    if n == 1 {
        "1 region".to_owned()
    } else {
        format!("{n} regions")
    }
}
