//! Readiness workspace panel (W3.8) — the single "is this safe to cut?" view.
//!
//! Folds signals that were already computed but scattered (op generation,
//! simulation freshness, rapid + holder collisions, tool-load verdicts, cycle
//! time) into one summary-first page. It adds no capability: thresholds come
//! from the shared [`crate::ui::readiness`] module (the same source the export
//! pre-flight modal uses, so the two can't disagree), and it is built entirely
//! from the component layer (`CountPill`, `FreshnessGate`). The persistent
//! dashboard twin of the pre-flight gate; "Export G-code…" opens that gate.

use super::AppEvent;
use crate::state::{AppState, Workspace};
use crate::ui::components::{CountPill, FreshnessGate};
use crate::ui::readiness::{self, CheckStatus, CycleTimeBasisExt};
use crate::ui::theme;
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

    let worst = ops_status
        .worse(sim_status)
        .worse(rapid_status)
        .worse(holder_status)
        .worse(load_status);

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
            check_row(
                ui,
                sim_status,
                "Simulation",
                sim_detail,
                (sim_status != CheckStatus::Pass).then_some(("Run sim", AppEvent::RunSimulation)),
                events,
            );

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
            let action = matches!(cycle.basis, Some(readiness::CycleTimeBasis::CuttingOnly))
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
                    .add(crate::ui::components::Button::new("Run simulation"))
                    .clicked()
            {
                events.push(AppEvent::RunSimulation);
            }
        });
    });
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
