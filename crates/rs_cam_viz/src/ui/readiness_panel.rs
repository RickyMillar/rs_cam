//! Readiness workspace panel (W3.8) — the single "is this safe to cut?" view.
//!
//! Folds signals that were already computed but scattered (op generation,
//! simulation freshness, rapid + holder collisions, tool-load verdicts, cycle
//! time) into one summary-first page. It adds no capability: thresholds come
//! from the shared [`crate::ui::readiness`] module (the same source the export
//! pre-flight modal uses, so the two can't disagree), and it is built entirely
//! from the component layer (`CountPill`, `FreshnessGate`). The persistent
//! dashboard twin of the pre-flight gate; "Export G-code…" opens that gate.

use egui_plot::{Line, MarkerShape, Plot, PlotPoints, Points};
use rs_cam_core::feeds::FeedsExplain;

use super::AppEvent;
use crate::state::{AppState, ProjectFeedsSort, ProjectFeedsState, Workspace};
use crate::ui::components::compare;
use crate::ui::components::{CountPill, FreshnessGate};
use crate::ui::feeds::compare::{compute_preview, read_current_values};
use crate::ui::feeds::shared::{CurrentValues, draw_machine_envelope, speedup, wash};
use crate::ui::readiness::{self, CheckStatus, CycleTimeBasisExt};
use crate::ui::{theme, tokens};
use crate::ui_command::{NoArgs, UiCommand};

pub fn draw(ui: &mut egui::Ui, state: &AppState, events: &mut Vec<AppEvent>) {
    let sim = &state.simulation;
    let stale = sim.has_results() && sim.is_stale(state.gui.edit_counter);

    // Compute every check up front (cheap reads of existing producers) so the
    // headline verdict is the worst of them.
    let (ops_status, computed, enabled) = readiness::operations_check(state);
    let sim_status = readiness::simulation_check(state);
    let rapid_status = readiness::rapid_collision_check(state);
    let holder_status = readiness::holder_clearance_check(state);
    let report = readiness::load_report(state);
    let load_status = readiness::tool_load_check(&report);
    let summary = report.summary(|_| None);

    // DC5a. The rollup's rows feed BOTH the check row below and the detail
    // window, so the feeds maths runs once per frame, not once per surface.
    let feeds_rows = project_rows(state);
    let feeds_status = project_feeds_status(&feeds_rows);

    let worst = ops_status
        .worse(sim_status)
        .worse(rapid_status)
        .worse(holder_status)
        .worse(load_status)
        .worse(feeds_status);

    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.add_space(8.0);
        draw_verdict_banner(ui, worst);
        ui.add_space(12.0);

        // Detail rows gated by freshness: a stale sim dims + banners the whole
        // readout rather than asserting fresh verdicts on stale evidence.
        FreshnessGate::new(stale).show(ui, |ui| {
            // F2.2 — "current", not "computed". An operation edited after
            // generation WAS computed; what it is not is the answer to the
            // configuration now in the project, and the count here is the
            // one the export gate will read. Naming the edited ones
            // separately is what distinguishes "never generated" from
            // "generated, then changed" — the same distinction the card
            // chip draws, on the surface that predicts the export.
            let (stale_ops, _) = readiness::freshness_counts(state);
            let ops_detail = if stale_ops > 0 {
                format!("{computed}/{enabled} current \u{2014} {stale_ops} edited since generation")
            } else {
                format!("{computed}/{enabled} current")
            };
            check_row(
                ui,
                ops_status,
                "Operations",
                &ops_detail,
                (ops_status != CheckStatus::Pass).then_some((
                    "Toolpaths",
                    AppEvent::Ui(UiCommand::SwitchWorkspace(Workspace::Toolpaths)),
                )),
                events,
            );

            let sim_detail = if !sim.has_results() {
                "Not run"
            } else if stale {
                "Stale — parameters changed"
            } else {
                "Up to date"
            };
            let sim_action = (sim_status != CheckStatus::Pass
                && readiness::simulation_request_is_buildable(&state.session, &state.gui))
            .then_some(("Run sim", AppEvent::RunSimulation));
            check_row(ui, sim_status, "Simulation", sim_detail, sim_action, events);

            let rapid_detail = if !sim.has_results() {
                "Run simulation first".to_owned()
            } else if sim.checks.rapid_collisions.is_empty() {
                "None detected".to_owned()
            } else {
                format!("{} detected", sim.checks.rapid_collisions.len())
            };
            check_row(
                ui,
                rapid_status,
                "Rapid collisions",
                &rapid_detail,
                (rapid_status == CheckStatus::Fail).then_some((
                    "Simulation",
                    AppEvent::Ui(UiCommand::SwitchWorkspace(Workspace::Simulation)),
                )),
                events,
            );

            // F2.12 — one derivation, shared with the pre-flight gate. This
            // arm used to read `min_safe_stickout`, which the drain writes
            // only on a FAILING check, so a clean current check printed "Not
            // checked" and a stale verdict printed as if it were current.
            //
            // F2.13 — the title is the job's, and the check reads ONE
            // toolpath. Where the two differ the detail names the operation
            // the check examined and how many it did not, and the tier
            // refuses `Pass`.
            let holder_detail = readiness::holder_clearance_detail(state);
            check_row(
                ui,
                holder_status,
                "Holder clearance",
                &holder_detail,
                (holder_status != CheckStatus::Pass)
                    .then_some(("Re-check", AppEvent::RunCollisionCheck)),
                events,
            );

            // Tool load — verdict-family CountPills, one universe via the /T
            // denominator (the canonical `ToolLoadReportSummary` producer).
            check_row(ui, load_status, "Tool load", "", None, events);
            if summary.total_toolpaths > 0 {
                ui.horizontal(|ui| {
                    ui.add_space(28.0);
                    ui.add(
                        CountPill::verdict("within", summary.within)
                            .denom(summary.total_toolpaths)
                            .color(theme::SUCCESS),
                    );
                    if summary.exceeds > 0 {
                        ui.add(
                            CountPill::verdict("exceeds", summary.exceeds)
                                .denom(summary.total_toolpaths)
                                .color(theme::ERROR),
                        );
                    }
                    if summary.fully_unmodeled > 0 {
                        ui.add(
                            CountPill::verdict("unmodeled", summary.fully_unmodeled)
                                .denom(summary.total_toolpaths)
                                .color(theme::WARNING)
                                .hover(
                                    "Criteria that couldn't be evaluated — run/refresh the \
                                     simulation or supply tool data.",
                                ),
                        );
                    }
                });
            }

            // Cycle time — information only, never gates the verdict. The row
            // now names its own basis (G-TIMEEST): the same word used to cover
            // a machine-model wall clock and a cutting-only figure ~7× smaller,
            // and this panel is read immediately before starting a cut.
            let cycle = readiness::estimate_total_time(state);
            let changes = readiness::count_tool_changes(state);
            let (title, detail, status) = match cycle.basis {
                Some(basis) => (
                    format!("Est. cycle time ({})", basis.qualifier()),
                    format!(
                        "{}  ({changes} tool changes)",
                        readiness::format_cycle_time(cycle.seconds)
                    ),
                    basis.status(),
                ),
                // No estimate at all — a dash, never 0:00.
                None => (
                    "Est. cycle time".to_owned(),
                    format!("\u{2014}  ({changes} tool changes)"),
                    CheckStatus::Warning,
                ),
            };
            // A basis with a remedy gets the same jump affordance every other
            // row on this panel offers. Only the un-simulated case has a
            // single-event fix; the no-kinematics case is a properties edit,
            // so it gets named in text instead of a fake button.
            let action = (matches!(cycle.basis, Some(readiness::CycleTimeBasis::CuttingOnly))
                && readiness::simulation_request_is_buildable(&state.session, &state.gui))
            .then_some(("Run sim", AppEvent::RunSimulation));
            check_row(ui, status, &title, &detail, action, events);
            if let Some(basis) = cycle.basis
                && basis != readiness::CycleTimeBasis::MachineModel
            {
                // Inline, not a hover: an operator planning a shift around
                // this number must not have to discover the caveat.
                caveat_line(ui, basis.caveat(), theme::WARNING);
                if let Some(remedy) = basis.remedy() {
                    caveat_line(ui, remedy, theme::TEXT_MUTED);
                }
            }
        });

        ui.add_space(12.0);
        ui.separator();
        ui.add_space(crate::ui::tokens::SPACE_3);

        // Primary action: open the export gate. The pre-flight modal is the
        // confirm/override surface; this dashboard is its persistent twin.
        ui.horizontal(|ui| {
            // UP3, §4.5: ONE Primary per screen. This dashboard exists to
            // answer "is this safe to cut?", so the export gate is the action
            // it is FOR. "Run simulation" is a step along the way and takes
            // the default weight. Before this the two were identical and the
            // screen could not say which was which.
            if ui
                .add(crate::ui::components::Button::primary(
                    "Export G-code\u{2026}",
                ))
                .on_hover_text("Open the export readiness gate")
                .clicked()
            {
                events.push(AppEvent::Ui(UiCommand::ExportGcode(NoArgs)));
            }
            if !sim.has_results()
                && ui
                    .add(
                        crate::ui::components::Button::new("Run simulation").enabled(
                            readiness::simulation_request_is_buildable(&state.session, &state.gui),
                        ),
                    )
                    .on_disabled_hover_text(
                        "Generate at least one enabled toolpath with a valid tool first",
                    )
                    .clicked()
            {
                events.push(AppEvent::RunSimulation);
            }
        });

        // It is ONE row, in the shape of its neighbours. The table, the
        // scatter and the machine envelope open FROM it: a 960-point chart
        // grid does not belong in a 280-point rail.
        draw_project_feeds_row(ui, feeds_status, &feeds_rows, events);
    });

    // DC5a. The project feeds rollup asks a PROJECT-wide question, so it
    // answers on the project-wide workspace. It used to sit behind a mode
    // flip inside the per-operation feeds modal, reachable only by selecting
    // one toolpath and opening its window.
    //
    // The window is context level, so it draws outside the scroll area.
    draw_project_feeds_window(ui.ctx(), state, feeds_rows, events);
}

/// The rollup's verdict.
///
/// `Warning` only when the project is leaving speed on the table. 1.05x is
/// the band the bottleneck callout already treats as worth acting on; below
/// it the rollup has nothing to offer and the check passes. An empty project
/// passes too — there is nothing to roll up.
fn project_feeds_status(rows: &[ProjectFeedsRow]) -> CheckStatus {
    if rows.is_empty() || aggregate_speedup(rows) <= 1.05 {
        CheckStatus::Pass
    } else {
        CheckStatus::Warning
    }
}

/// The rollup, as one readiness check row.
///
/// The row states the aggregate speedup and opens the detail. It is a
/// `Warning` only when the project is leaving speed on the table; a project
/// already at or above its recommendation passes.
fn draw_project_feeds_row(
    ui: &mut egui::Ui,
    status: CheckStatus,
    rows: &[ProjectFeedsRow],
    events: &mut Vec<AppEvent>,
) {
    if rows.is_empty() {
        return;
    }
    let agg = aggregate_speedup(rows);
    let detail = if agg > 1.0 {
        format!("{agg:.2}\u{00D7} available across {} toolpaths", rows.len())
    } else {
        format!("{} toolpaths at or above recommendation", rows.len())
    };
    check_row(
        ui,
        status,
        "Feeds",
        &detail,
        Some((
            "Review\u{2026}",
            AppEvent::Ui(UiCommand::SetProjectFeedsOpen(true)),
        )),
        events,
    );
}

/// The rollup detail, as a window the operator opens, reads and closes.
///
/// Ruling R31's shape: a thing you open and close, not a floating panel that
/// can be left over the workspace tabs.
fn draw_project_feeds_window(
    ctx: &egui::Context,
    state: &AppState,
    rows: Vec<ProjectFeedsRow>,
    events: &mut Vec<AppEvent>,
) {
    if !state.project_feeds.open {
        return;
    }
    let mut still_open = true;
    egui::Window::new("Feeds & Speeds \u{2014} all toolpaths")
        .collapsible(false)
        .resizable(true)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .default_width(960.0)
        .default_height(620.0)
        .open(&mut still_open)
        .show(ctx, |ui| {
            draw_project_rollup(ui, &state.project_feeds, rows, events);
        });
    if !still_open {
        events.push(AppEvent::Ui(UiCommand::SetProjectFeedsOpen(false)));
    }
}

/// The headline "is this safe to cut?" banner — worst-of-checks verdict.
fn draw_verdict_banner(ui: &mut egui::Ui, status: CheckStatus) {
    let (text, fill, stroke) = match status {
        CheckStatus::Pass => (
            "\u{2713}  READY TO CUT",
            crate::ui::tokens::TINT_OK,
            theme::SUCCESS,
        ),
        CheckStatus::Warning => (
            "\u{26A0}  REVIEW BEFORE CUTTING",
            crate::ui::tokens::TINT_CAUTION,
            theme::WARNING,
        ),
        CheckStatus::Fail => (
            "\u{2717}  NOT READY — resolve issues first",
            crate::ui::tokens::TINT_DANGER,
            theme::ERROR,
        ),
    };
    egui::Frame::default()
        .fill(fill)
        .stroke(egui::Stroke::new(1.5_f32, stroke))
        .inner_margin(10.0)
        .corner_radius(6)
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).heading().strong().color(stroke));
        });
}

/// A wrapped, indented qualifier under a check row — for the case where the
/// row's number is real but does not mean what its name suggests. Colour
/// separates the caveat (what is wrong) from the remedy (what to do).
fn caveat_line(ui: &mut egui::Ui, text: &str, color: egui::Color32) {
    ui.horizontal(|ui| {
        ui.add_space(28.0);
        ui.add(egui::Label::new(egui::RichText::new(text).small().color(color)).wrap());
    });
    ui.add_space(4.0);
}

/// One readiness check row: status glyph, label, detail, optional jump action.
fn check_row(
    ui: &mut egui::Ui,
    status: CheckStatus,
    label: &str,
    detail: &str,
    action: Option<(&str, AppEvent)>,
    events: &mut Vec<AppEvent>,
) {
    let (icon, color) = match status {
        CheckStatus::Pass => ("\u{2713}", theme::SUCCESS),
        CheckStatus::Warning => ("\u{26A0}", theme::WARNING),
        CheckStatus::Fail => ("\u{2717}", theme::ERROR),
    };
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(icon).color(color));
        ui.label(
            egui::RichText::new(format!("{label}:"))
                .strong()
                .color(theme::TEXT_HEADING),
        );
        if !detail.is_empty() {
            ui.label(egui::RichText::new(detail).color(color));
        }
        if let Some((action_label, event)) = action {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .small_button(action_label)
                    .on_hover_text("Jump to where you can fix this")
                    .clicked()
                {
                    events.push(event);
                }
            });
        }
    });
    ui.add_space(4.0);
}

// ────────────────────────────────────────────────────────────────────
// Project rollup (Phase 4)
// ────────────────────────────────────────────────────────────────────

/// The rollup's rows, one per ENABLED toolpath, in project order.
///
/// The Readiness check row and the rollup window both read this, so the
/// feeds maths runs ONCE per frame rather than once per surface.
fn project_rows(state: &AppState) -> Vec<ProjectFeedsRow> {
    state
        .session
        .toolpath_configs()
        .iter()
        .filter(|tc| tc.enabled)
        .filter_map(|tc| {
            let current = read_current_values(state, tc.id)?;
            let preview = compute_preview(state, tc.id)?;
            let refusal = preview.refusal().map(ToString::to_string);
            Some(ProjectFeedsRow {
                id: tc.id,
                name: tc.name.clone(),
                current,
                explain: preview.explain().clone(),
                refusal,
            })
        })
        .collect()
}

fn draw_project_rollup(
    ui: &mut egui::Ui,
    rollup: &ProjectFeedsState,
    rows: Vec<ProjectFeedsRow>,
    events: &mut Vec<AppEvent>,
) {
    let mut rows = rows;
    let sort = rollup.sort;
    match sort {
        ProjectFeedsSort::Index => {}
        ProjectFeedsSort::Speedup => {
            rows.sort_by(|a, b| {
                speedup(&b.current, &b.explain).total_cmp(&speedup(&a.current, &a.explain))
            });
        }
        ProjectFeedsSort::Name => {
            rows.sort_by(|a, b| a.name.cmp(&b.name));
        }
    }

    if rows.is_empty() {
        ui.label(
            egui::RichText::new("No enabled toolpaths.")
                .small()
                .color(theme::TEXT_DIM),
        );
        return;
    }

    // ── Bottleneck callout ──────────────────────────────────────────
    draw_bottleneck_callout(ui, &rows);
    ui.add_space(6.0);

    // ── Project scatter overlay ────────────────────────────────────
    ui.horizontal(|ui| {
        let mut show_scatter = rollup.show_scatter;
        if ui
            .checkbox(&mut show_scatter, "Show feed-RPM scatter")
            .on_hover_text(
                "Overlay every toolpath's current→recommended move on a single feed-RPM chart.",
            )
            .changed()
        {
            events.push(AppEvent::Ui(UiCommand::SetFeedsProjectScatter(
                show_scatter,
            )));
        }
    });
    if rollup.show_scatter {
        draw_project_scatter(ui, &rows);
        ui.add_space(6.0);
    }

    // ── Toolbar: sort + select-all + apply ─────────────────────────
    let selected_count = rollup.selected.len();
    let total = rows.len();
    ui.horizontal(|ui| {
        let all_selected = selected_count == total && total > 0;
        let any_selected = selected_count > 0;
        if ui
            .checkbox(
                &mut { all_selected },
                format!("Select all ({selected_count}/{total})"),
            )
            .clicked()
        {
            events.push(AppEvent::Ui(UiCommand::SetFeedsProjectSelectAll(
                !all_selected,
            )));
        }
        ui.separator();
        let mut apply_btn = ui.add_enabled(
            any_selected,
            egui::Button::new("⚡ Apply selected — changes the cut"),
        );
        apply_btn = apply_btn.on_hover_text(
            "Apply Feeds recommendations to every checked toolpath. \
             CHANGES THE CUT: DOC and WOC move as well as the speeds. \
             Rows whose tool cannot run their operation are skipped and reported.",
        );
        if apply_btn.clicked() {
            events.push(AppEvent::ApplyFeedsProjectSelected);
        }
        if ui
            .button("⚡⚡ Apply all toolpaths — changes the cut")
            .on_hover_text(
                "Apply to every enabled toolpath regardless of selection. \
                 CHANGES THE CUT: DOC and WOC move as well as the speeds. \
                 Rows whose tool cannot run their operation are skipped and reported.",
            )
            .clicked()
        {
            events.push(AppEvent::ApplyFeedsProject);
        }
        ui.separator();
        ui.label(egui::RichText::new("Sort:").small().color(theme::TEXT_DIM));
        if ui
            .selectable_label(sort == ProjectFeedsSort::Index, "Order")
            .clicked()
        {
            events.push(AppEvent::Ui(UiCommand::SetFeedsProjectSort(
                ProjectFeedsSort::Index,
            )));
        }
        if ui
            .selectable_label(sort == ProjectFeedsSort::Speedup, "Speedup")
            .clicked()
        {
            events.push(AppEvent::Ui(UiCommand::SetFeedsProjectSort(
                ProjectFeedsSort::Speedup,
            )));
        }
        if ui
            .selectable_label(sort == ProjectFeedsSort::Name, "Name")
            .clicked()
        {
            events.push(AppEvent::Ui(UiCommand::SetFeedsProjectSort(
                ProjectFeedsSort::Name,
            )));
        }
    });

    ui.add_space(4.0);
    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new("feeds_modal_project_table")
            .num_columns(9)
            .spacing([crate::ui::tokens::SPACE_3, crate::ui::tokens::SPACE_2])
            .min_row_height(crate::ui::tokens::ROW_DENSE)
            .striped(true)
            .show(ui, |ui| {
                for h in [
                    "", "Toolpath", "Feed", "Rec feed", "Δ", "DOC", "Rec DOC", "WOC", "Apply",
                ] {
                    ui.label(
                        egui::RichText::new(h)
                            .small()
                            .strong()
                            .color(theme::TEXT_DIM),
                    );
                }
                ui.end_row();

                for r in &rows {
                    let mut checked = rollup.selected.contains(&r.id);
                    if ui.checkbox(&mut checked, "").changed() {
                        events.push(AppEvent::Ui(UiCommand::ToggleFeedsProjectRow(r.id)));
                    }
                    match &r.refusal {
                        Some(why) => {
                            ui.label(
                                egui::RichText::new(format!("{} ⚠", r.name))
                                    .small()
                                    .color(theme::WARNING),
                            )
                            .on_hover_text(format!(
                                "This tool cannot run this operation, so no apply — batch or \
                                 single — will write to it.\n{why}"
                            ));
                        }
                        None => {
                            ui.label(egui::RichText::new(&r.name).small());
                        }
                    }
                    ui.label(format!("{:.0}", r.current.feed_rate_mm_min));
                    ui.label(format!("{:.0}", r.explain.recommended.feed_rate_mm_min));
                    ui.label(compare::delta_tag(
                        Some(r.current.feed_rate_mm_min),
                        Some(r.explain.recommended.feed_rate_mm_min),
                    ));
                    ui.label(compare::format_optional(r.current.depth_per_pass, "", 0.01));
                    ui.label(format!("{:.2}", r.explain.recommended.axial_depth_mm));
                    ui.label(compare::format_optional(r.current.stepover, "", 0.01));
                    match &r.refusal {
                        Some(why) => {
                            ui.label(egui::RichText::new("refused").small().color(theme::ERROR))
                                .on_hover_text(why);
                        }
                        None => {
                            if ui
                                .small_button("Apply")
                                .on_hover_text(
                                    "Apply the recommendation to this toolpath. \
                                     CHANGES THE CUT (DOC/WOC) as well as the speeds.",
                                )
                                .clicked()
                            {
                                events.push(AppEvent::ApplyFeedsAll(r.id));
                            }
                        }
                    }
                    ui.end_row();
                }
            });
    });
}

/// Find the toolpath with the largest feed-speedup potential and
/// surface it as a one-line callout. Skips toolpaths whose current
/// feed is already at or above the recommendation.
fn draw_bottleneck_callout(ui: &mut egui::Ui, rows: &[ProjectFeedsRow]) {
    let bottleneck = rows
        .iter()
        .filter_map(|r| {
            let s = speedup(&r.current, &r.explain);
            if s > 1.05 { Some((r, s)) } else { None }
        })
        .max_by(|a, b| a.1.total_cmp(&b.1));

    egui::Frame::group(ui.style()).show(ui, |ui| match bottleneck {
        Some((row, s)) => {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Biggest opportunity:")
                        .small()
                        .strong()
                        .color(theme::TEXT_STRONG),
                );
                ui.label(
                    egui::RichText::new(&row.name)
                        .small()
                        .color(theme::TEXT_STRONG),
                );
                ui.label(
                    egui::RichText::new(format!(
                        "feed {:.0} → {:.0} mm/min ({:.2}× speedup)",
                        row.current.feed_rate_mm_min,
                        row.explain.recommended.feed_rate_mm_min,
                        s
                    ))
                    .small()
                    .color(theme::SUCCESS),
                );
            });
            // Aggregate cycle-time gain estimate — sum of inverse speedups.
            let agg = aggregate_speedup(rows);
            if agg > 1.05 {
                ui.label(
                    egui::RichText::new(format!(
                        "Estimated project speedup if all applied: {agg:.2}×"
                    ))
                    .small()
                    .color(theme::TEXT_DIM),
                );
            }
        }
        None => {
            ui.label(
                egui::RichText::new(
                    "Every toolpath is already at or above its recommendation — no project-wide gain available.",
                )
                .small()
                .color(theme::TEXT_DIM),
            );
        }
    });
}

/// Crude project speedup estimate: weighted by feed-rate inverse
/// (treats faster feeds as proxy for cycle-time saved). Just for
/// the callout — the real number requires a sim.
fn aggregate_speedup(rows: &[ProjectFeedsRow]) -> f64 {
    if rows.is_empty() {
        return 1.0;
    }
    let mut t_current = 0.0;
    let mut t_recommended = 0.0;
    for r in rows {
        if r.current.feed_rate_mm_min > 0.0 {
            t_current += 1.0 / r.current.feed_rate_mm_min;
        }
        if r.explain.recommended.feed_rate_mm_min > 0.0 {
            t_recommended += 1.0 / r.explain.recommended.feed_rate_mm_min;
        }
    }
    if t_recommended > 0.0 {
        t_current / t_recommended
    } else {
        1.0
    }
}

/// Scatter chart showing every toolpath on the feed-RPM plane.
/// Current = a neutral ●, Recommended = blue ◆, line connecting the two.
fn draw_project_scatter(ui: &mut egui::Ui, rows: &[ProjectFeedsRow]) {
    if rows.is_empty() {
        return;
    }
    let max_rpm = rows
        .iter()
        .filter_map(|r| r.current.spindle_rpm.map(f64::from))
        .chain(rows.iter().map(|r| r.explain.recommended.rpm))
        .chain(rows.iter().map(|r| r.explain.machine.spindle_max_rpm))
        .fold(0.0_f64, f64::max)
        * 1.1;
    let max_feed = rows
        .iter()
        .map(|r| r.current.feed_rate_mm_min)
        .chain(rows.iter().map(|r| r.explain.recommended.feed_rate_mm_min))
        .chain(rows.iter().map(|r| r.explain.machine.max_feed_mm_min))
        .fold(0.0_f64, f64::max)
        * 1.05;

    // The machine envelope is the same for every toolpath (project-wide
    // machine profile), so we take it from the first row.
    let env_opt = rows.first().map(|r| r.explain.machine.clone());

    Plot::new("feeds_modal_project_scatter")
        .height(220.0)
        .include_x(0.0)
        .include_y(0.0)
        .include_x(max_rpm)
        .include_y(max_feed)
        .x_axis_label("RPM")
        .y_axis_label("feed (mm/min)")
        .allow_drag(false)
        .allow_zoom(false)
        .allow_scroll(false)
        .show(ui, |plot_ui| {
            // Machine envelope — forbidden zones + walls, same grammar as Chart C.
            if let Some(env) = env_opt.as_ref() {
                draw_machine_envelope(plot_ui, env, max_rpm, max_feed);
            }
            for r in rows {
                let cur = match r.current.spindle_rpm {
                    Some(rpm) if rpm > 0 => Some([f64::from(rpm), r.current.feed_rate_mm_min]),
                    _ => None,
                };
                let rec = [
                    r.explain.recommended.rpm,
                    r.explain.recommended.feed_rate_mm_min,
                ];
                if let Some(c) = cur {
                    // Connecting arrow line. Construction geometry, so it
                    // takes the diagram's dim rung; it is darker than the
                    // grey it replaces.
                    plot_ui.line(
                        Line::new("", PlotPoints::from(vec![c, rec]))
                            .color(wash(tokens::DIAGRAM_DIM, 140))
                            .width(1.0_f32),
                    );
                    plot_ui.points(
                        Points::new("", vec![c])
                            .shape(MarkerShape::Circle)
                            .filled(true)
                            .radius(4.0_f32)
                            // Ruling R23: "current" is the operator's OWN value, a
                            // "you are here" marker, not a verdict. It wore
                            // DANGER red on what is a CATEGORY — current versus
                            // recommended. TEXT_STRONG is the highest-contrast
                            // neutral and reads as a position in a chart that is
                            // otherwise all blue.
                            .color(crate::ui::tokens::TEXT_STRONG),
                    );
                }
                plot_ui.points(
                    Points::new("", vec![rec])
                        .shape(MarkerShape::Diamond)
                        .filled(true)
                        .radius(4.5_f32)
                        .color(tokens::DIAGRAM_INK),
                );
            }
        });
}

struct ProjectFeedsRow {
    id: rs_cam_core::ToolpathId,
    name: String,
    current: CurrentValues,
    explain: FeedsExplain,
    /// `Some` when `validate_tool_for_operation` refuses this row's tool ×
    /// operation pairing. Pre-fix (A-3 §3.5) the rollup had no idea: the row
    /// rendered a recommendation and an Apply button like any other, and
    /// `⚡⚡ Apply all toolpaths` swept the refused row up **silently**. The
    /// batch handler now skips it and says so; the table marks it.
    refusal: Option<String>,
}
