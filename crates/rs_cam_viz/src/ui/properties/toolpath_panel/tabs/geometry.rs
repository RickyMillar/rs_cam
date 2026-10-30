//! The Geometry tab (R2) of the toolpath inspector: the Input row, the op
//! selection, the Boundary rows and the Reach row. W3a: every row is a
//! `PropRow`; the tab has no section header.

use crate::state::toolpath::{BoundaryContainment, BoundarySource};
use crate::ui::AppEvent;
use crate::ui::components::{PropRow, Role};
use crate::ui::properties::tab_badges::rest_region_pathology_line;
use crate::ui::properties::{ReachPanelSummary, ToolpathPanelInputs, boundary_summary_line};

/// G-C (§2 d): the act on the Containment row that writes `Inside`.
pub(crate) const USE_INSIDE: &str = "Use Inside";

/// The value of the Boundary row while the boundary is off.
pub(crate) const BOUNDARY_OFF: &str = "Off";

/// The Geometry tab (R2): the Input row, the op selection and the floor
/// (`geometry_tab.rs`, events only, L-1), the Boundary rows and the
/// boundary selection list directly under them, then the Reach row of a
/// reach-capable op. `selection_findings` are the findings that
/// `placement::place` anchors to the `selection` field: the state mark of
/// the selection list header (W3h). `properties::tests` renders it (D3, G-C, G-D,
/// R7a).
#[expect(
    clippy::too_many_arguments,
    reason = "the tab takes the GUI state of its rows one by one, as the panel host holds them"
)]
pub(in crate::ui::properties) fn draw_geometry_tab(
    ui: &mut egui::Ui,
    entry: &mut crate::state::toolpath::ToolpathEntry,
    inputs: &ToolpathPanelInputs,
    selection_ui: &mut crate::state::pick::SelectionUi,
    containment_autocorrect: &mut crate::state::panels::ContainmentAutoCorrect,
    reach_view: &mut ReachView,
    selection_findings: &[crate::ui::properties::placement::Finding<'_>],
    events: &mut Vec<AppEvent>,
) {
    ui.add_space(4.0);
    draw_input_row(ui, entry, inputs);
    crate::ui::properties::geometry_tab::draw_geometry_tab(
        ui,
        entry,
        inputs,
        selection_ui,
        selection_findings,
        events,
    );
    draw_machining_boundary(ui, entry, inputs, containment_autocorrect);
    crate::ui::properties::geometry_tab::draw_boundary_selection(
        ui,
        entry,
        inputs,
        selection_ui,
        events,
    );
    if entry.operation.op_type().supports_reach_map()
        && draw_reach_row(ui, &inputs.reach, reach_view)
    {
        reach_view.toggle = true;
    }
}

/// The Input row of the Geometry tab (R2: it was on the Cut tab; W3a: a
/// `PropRow`): the model that the op reads. A STEP model that loaded with
/// no BREP topology is the Caution state of the row, the reason on the
/// hover.
fn draw_input_row(
    ui: &mut egui::Ui,
    entry: &mut crate::state::toolpath::ToolpathEntry,
    inputs: &ToolpathPanelInputs,
) {
    let models = inputs.models.as_slice();
    // BREP-not-loaded: a STEP model loaded without its enriched mesh
    // (older project files written before the BREP round-trip fix, or an
    // unexpected loader regression).
    let brep = inputs.model_is_step_missing_brep;
    let model_label = models
        .iter()
        .find(|(id, _)| *id == entry.model_id)
        .map_or("(none)", |(_, s)| s.as_str());
    let out = PropRow::new("Input")
        .state(brep.then_some(Role::Caution))
        .hover(Some(
            "The model that this operation reads. The selection rows below name its items.",
        ))
        .detail(brep.then_some(
            "BREP topology not loaded: the face picker is unavailable. Reload the model.",
        ))
        .show(ui, |ui| {
            egui::ComboBox::from_id_salt("tp_model")
                .truncate()
                .width(ui.available_width())
                .selected_text(model_label)
                .show_ui(ui, |ui| {
                    for (id, name) in models {
                        ui.selectable_value(&mut entry.model_id, *id, name.as_str());
                    }
                })
                .response
        });
    let _ = out.inner.on_hover_text(&out.hover);
}

/// The text of a boundary source in the Boundary combo (sentence case; core
/// `BoundarySource::label` is the MCP spelling).
fn source_text(source: &BoundarySource) -> &'static str {
    match source {
        BoundarySource::Stock => "Stock",
        BoundarySource::ModelSilhouette => "Model silhouette",
        BoundarySource::Selection { .. } => "Selection",
        BoundarySource::DerivedRestRegions { .. } => "Rest regions",
        BoundarySource::PlannedTierRegions { .. } => "Planned tier regions",
    }
}

/// The explanation of the Boundary row.
const BOUNDARY_HOVER: &str = "Restrict the toolpath to a boundary polygon. Moves outside the \
    boundary become rapids at safe Z. Off cuts the whole territory of the operation.";

/// The Boundary rows of the Geometry tab (R2: it was at the foot of the Cut
/// tab; W3a: `PropRow`s, no section header). ONE "Boundary" row is the
/// switch and the source: Off, Stock, Model silhouette, Selection or Rest
/// regions. While the boundary is on, the rows of the source (holes, rest
/// source), Containment and Offset follow. They write the draft entry.
fn draw_machining_boundary(
    ui: &mut egui::Ui,
    entry: &mut crate::state::toolpath::ToolpathEntry,
    inputs: &ToolpathPanelInputs,
    containment_autocorrect: &mut crate::state::panels::ContainmentAutoCorrect,
) {
    containment_autocorrect.scope(entry.id);
    // D3: an op that no boundary moves (`TerritoryXy::SetupMap`, Pin
    // Drill: its holes are absolute setup-map positions) draws no
    // boundary controls. The panel reads the capability, not the op type.
    if !entry
        .operation
        .capabilities()
        .territory_xy
        .session_clip_applies()
    {
        return;
    }
    draw_boundary_row(ui, entry, inputs, containment_autocorrect);
    if !entry.boundary.enabled {
        return;
    }
    let boundary_source_candidates = inputs.boundary_source_candidates.as_slice();

    // GT-Q7: the boundary selection list follows these rows on the same
    // tab. The holes option inverts the regions: the toolpath cuts inside
    // the holes of the shapes and stays off the shapes.
    if let BoundarySource::Selection { holes, .. } = &mut entry.boundary.source {
        let _ = PropRow::new("Holes")
            .hover(Some(
                "The toolpath cuts inside the holes of the shapes, for example the area \
                 inside an edge band, and stays off the shapes.",
            ))
            .checkbox(ui, holes, "");
    }

    // Rest-regions source-toolpath picker (P2.2) — only shown when the
    // source is `DerivedRestRegions`. Candidates are every other toolpath
    // in the session; ones with a cached result whose rest regions are
    // already non-empty are labelled "· N regions" (G-C) and sorted
    // first, but a not-yet-generated toolpath is still selectable —
    // generation fails hard with a clear message if the source turns out
    // unusable.
    if let BoundarySource::DerivedRestRegions { source_toolpath_id } = &mut entry.boundary.source {
        let current = boundary_source_candidates
            .iter()
            .find(|(candidate_id, _, _, _, _, _)| candidate_id == source_toolpath_id);
        let current_label = current.map_or_else(
            || "(toolpath not found)".to_owned(),
            |(_, name, _, regions, _, _)| rest_source_label(name, regions.as_deref()),
        );
        // G-C (§6 b): the top of the regions first, then the row's own
        // explanation.
        let hover = match current.and_then(|(_, _, _, _, _, top_z)| *top_z) {
            Some(z) => format!(
                "Top of the regions: Z {z:.2}.\nThe toolpath whose regions bound this one. \
                 Picking it switches its rest analysis on."
            ),
            None => "The toolpath whose regions bound this one. Picking it switches its \
                     rest analysis on."
                .to_owned(),
        };
        let mut sorted = boundary_source_candidates.to_vec();
        sorted.sort_by_key(|(_, _, ready, _, _, _)| !*ready);
        let out = PropRow::new("Rest source")
            .state(current.is_none().then_some(Role::Danger))
            .hover(Some(&hover))
            .detail(Some(
                "The source toolpath is not in the project. Pick another.",
            ))
            .show(ui, |ui| {
                egui::ComboBox::from_id_salt("boundary_rest_source")
                    .truncate()
                    .width(ui.available_width())
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
            });
        let _ = out.inner.on_hover_text(&out.hover);

        // Sliver-storm / giant-region caption (2026-07-06 incident): the
        // SOURCE toolpath is who suffers the per-island generation
        // explosion or the "barely restricts anything" giant-region case,
        // so classify ITS cached regions (captured in
        // `boundary_source_candidates` alongside `ready`), not this
        // consumer's own (this toolpath has none — it's the one consuming
        // the boundary).
        //
        // LH-2: the denominator is the SOURCE toolpath's own rest-grid
        // footprint (captured alongside its regions), not this model's
        // bounding rectangle.
        let selected = boundary_source_candidates
            .iter()
            .find(|(candidate_id, _, _, _, _, _)| candidate_id == source_toolpath_id);
        let selected_regions = selected.and_then(|(_, _, _, regions, _, _)| regions.as_ref());
        // `None` twice over: no candidate selected, or the selected one has
        // no rest grid. Both are "not measured", and
        // `classify_rest_regions` stays silent.
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

    draw_containment_row(ui, entry, inputs, containment_autocorrect);

    let _ = PropRow::new("Offset")
        .unit("mm")
        .hover(Some(
            "Expand (positive) or shrink (negative) the boundary. Applied before the \
             tool-radius containment.",
        ))
        .number(
            ui,
            &mut entry.boundary.offset,
            &crate::ui::components::NumberSpec::new(0.1),
        );
}

/// The Boundary row: Off or one source, in one combo (W3a: it replaces the
/// "Enable boundary" checkbox and the Source row). Off writes
/// `enabled = false` and keeps the stored source; a source writes
/// `enabled = true`.
fn draw_boundary_row(
    ui: &mut egui::Ui,
    entry: &mut crate::state::toolpath::ToolpathEntry,
    inputs: &ToolpathPanelInputs,
    containment_autocorrect: &mut crate::state::panels::ContainmentAutoCorrect,
) {
    let boundary_source_candidates = inputs.boundary_source_candidates.as_slice();
    // R7a: a source change writes the offset of THE source-change rule
    // (`BoundaryConfig::with_source`), which reads the tool diameter of
    // the op.
    let tool_diameter = entry
        .tool_id
        .and_then(|tool_id| inputs.tools.iter().find(|(id, _, _)| *id == tool_id))
        .map(|(_, _, diameter)| *diameter);
    // UX-R03-009 (G-BOUNDARYINHERIT): the stored source is the one
    // generation uses (`session/compute.rs` clones `tc.boundary`
    // unconditionally). G-D (ruling D-1): its name is the detail of the
    // row hover, not a row above it. The controller assigns the model
    // silhouette on add for 3D ops on a mesh; that provenance is not
    // stored, so the hover never claims "(auto)".
    let source_summary = boundary_summary_line(&entry.boundary, false, &inputs.models);
    let enabled = entry.boundary.enabled;
    let selected_text = if enabled {
        source_text(&entry.boundary.source)
    } else {
        BOUNDARY_OFF
    };
    let hover = if enabled {
        format!("{BOUNDARY_HOVER}\n{source_summary}")
    } else {
        BOUNDARY_HOVER.to_owned()
    };
    let out = PropRow::new("Boundary").hover(Some(&hover)).show(ui, |ui| {
        egui::ComboBox::from_id_salt("boundary_source")
            .truncate()
            .width(ui.available_width())
            .selected_text(selected_text)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(!enabled, BOUNDARY_OFF)
                    .on_hover_text("No boundary: the operation cuts its whole territory.")
                    .clicked()
                {
                    entry.boundary.enabled = false;
                }
                let on = |source: &BoundarySource, want: fn(&BoundarySource) -> bool| {
                    enabled && want(source)
                };
                if ui
                    .selectable_label(
                        on(&entry.boundary.source, |s| {
                            matches!(s, BoundarySource::Stock)
                        }),
                        "Stock",
                    )
                    .on_hover_text("Use the stock bounding rectangle")
                    .clicked()
                {
                    entry.boundary.enabled = true;
                    entry.boundary.source = BoundarySource::Stock;
                }
                if ui
                    .selectable_label(
                        on(&entry.boundary.source, |s| {
                            matches!(s, BoundarySource::ModelSilhouette)
                        }),
                        "Model silhouette",
                    )
                    .on_hover_text(
                        "XY projection of the 3D model. Only machines where the model \
                         actually is.",
                    )
                    .clicked()
                {
                    entry.boundary.enabled = true;
                    if !matches!(entry.boundary.source, BoundarySource::ModelSilhouette) {
                        entry.boundary = entry
                            .boundary
                            .with_source(BoundarySource::ModelSilhouette, tool_diameter);
                    }
                }
                // P4.3: the one door to a `Selection` boundary. It is
                // enabled when a model holds a drawing item or a face item,
                // so a STEP-only project reaches it.
                let selection_enabled = inputs.boundary_selection_enabled;
                if ui
                    .add_enabled(
                        selection_enabled,
                        egui::Button::selectable(
                            on(&entry.boundary.source, |s| {
                                matches!(s, BoundarySource::Selection { .. })
                            }),
                            "Selection",
                        ),
                    )
                    .on_hover_text(if selection_enabled {
                        "Use a selection of items as the boundary: the closed shapes of a \
                         drawing, or faces picked in the viewport. The items can come from \
                         another model. Holes in the shapes stay uncut. Edit the selection \
                         below."
                    } else {
                        "No model in this project has a drawing item or a face. Import a \
                         DXF, an SVG or a STEP model to use this source."
                    })
                    .clicked()
                {
                    entry.boundary.enabled = true;
                    // Keep the current selection when the source is already
                    // a selection; else write the value that a boundary
                    // Reset restores: the closed shapes of the first model
                    // with a closed shape, else no item. The source change
                    // writes offset 0 (R7a).
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
                            on(&entry.boundary.source, |s| {
                                matches!(s, BoundarySource::DerivedRestRegions { .. })
                            }),
                            "Rest regions",
                        ),
                    )
                    .on_hover_text(if has_rest_candidates {
                        "The regions another toolpath leaves: the pieces a Geodesic \
                         Adaptive rough did not cut, or another op's rest analysis. Pick \
                         the source below."
                    } else {
                        "No other toolpaths in this project yet — add one and generate it \
                         to use as the source. Picking it here switches its rest analysis \
                         on."
                    })
                    .clicked()
                {
                    entry.boundary.enabled = true;
                    if !matches!(
                        entry.boundary.source,
                        BoundarySource::DerivedRestRegions { .. }
                    ) {
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
                }
            })
            .response
    });
    let _ = out.inner.on_hover_text(&out.hover);
}

/// The Containment row. G-C (§2 c-e): for a rest-regions source whose tool
/// is smaller than this one, `Center` and `Outside` are grey (they refuse,
/// D53) with the sizes on hover. While the draft holds one of them, the
/// row is in the Danger state and its quiet action [`USE_INSIDE`] writes
/// `Inside`, unless `Inside` would collapse the boundary (then no action;
/// the hover says why). The `⟲` badge in the well marks a containment that
/// the `Rest regions` pick set (D-2).
fn draw_containment_row(
    ui: &mut egui::Ui,
    entry: &mut crate::state::toolpath::ToolpathEntry,
    inputs: &ToolpathPanelInputs,
    containment_autocorrect: &mut crate::state::panels::ContainmentAutoCorrect,
) {
    let draft_source = match entry.boundary.source {
        BoundarySource::DerivedRestRegions { source_toolpath_id } => Some(source_toolpath_id),
        _ => None,
    };
    let refused = inputs
        .rest_containment
        .as_ref()
        .filter(|check| Some(check.larger.source_toolpath_id) == draft_source);
    let before = entry.boundary.containment;
    let blocked = refused.filter(|_| entry.boundary.containment != BoundaryContainment::Inside);
    let detail = blocked.map(|check| check.hover());
    let keeps =
        blocked.is_some_and(|check| check.inside == crate::state::runtime::InsideAnswer::Keeps);
    let mut row = PropRow::new("Containment")
        .state(blocked.map(|_| Role::Danger))
        .hover(Some(
            "Where the tool may go against the boundary: its centre, its whole body \
             inside, or its edge outside.",
        ))
        .detail(detail.as_deref());
    if keeps {
        row = row.action(USE_INSIDE);
    }
    let autocorrect_hover = containment_autocorrect.hover().map(str::to_owned);
    let out = row.show(ui, |ui| {
        let badge_w = if autocorrect_hover.is_some() {
            24.0
        } else {
            0.0
        };
        let response = egui::ComboBox::from_id_salt("boundary_contain")
            .truncate()
            .width((ui.available_width() - badge_w).max(0.0))
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
                            egui::Button::selectable(entry.boundary.containment == value, label),
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
            })
            .response;
        if let Some(hover) = &autocorrect_hover {
            ui.add(
                crate::ui::components::ProvenanceBadge::new(
                    crate::ui::components::ProvKind::AutoCorrect,
                )
                .compact(),
            )
            .on_hover_text(hover);
        }
        response
    });
    let _ = out.inner.on_hover_text(&out.hover);
    if out.action_clicked {
        entry.boundary.containment = BoundaryContainment::Inside;
    }
    // The badge stands until the operator edits the containment.
    if entry.boundary.containment != before {
        containment_autocorrect.clear();
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
