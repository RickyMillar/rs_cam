use super::AppEvent;
use crate::state::AppState;
use crate::state::Workspace;
use crate::ui::automation;
use crate::ui::components::Role;
use crate::ui::tokens;
use crate::ui_command::{NoArgs, UiCommand};

/// The badge dot's radius. Three quarters of the grid step puts the dot at 6
/// points across, which keeps it on the token scale like every other mark.
const BADGE_DOT_RADIUS: f32 = tokens::GRID * 0.75;

/// Draw the workspace switcher bar. Sits below the menu bar, always visible.
///
/// DC3, Pattern E. A tab strip is a STRIP. One hairline runs the full width
/// of the bar along the seam the tabs share with the panel below, and the
/// active tab breaks that hairline with its own `ACCENT` segment. The result
/// is a continuous baseline with one tab joined through it. Before DC3 each
/// tab drew alone, so the tabs read as pills that float above the panel.
///
/// The hairline reserves its shape index HERE, before the first tab draws,
/// because the paint order decides which line wins at the seam. The seam is
/// only known after the row is laid out, so the shape is set at the end.
pub fn draw(ui: &mut egui::Ui, state: &AppState, events: &mut Vec<AppEvent>) {
    let seam = ui.painter().add(egui::Shape::Noop);
    let bar = ui.max_rect();
    let mut seam_y = None;

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;

        let current = state.workspace;

        // One list for the bar, the Workspace menu and the sentry
        // (G-WSMENU) — see `Workspace::ALL`. The badge stays a per-tab
        // decision because each reads different state; the aggregate
        // export-readiness chip lives on its named tab (W3.8), not
        // interleaved onto Setup.
        for target in Workspace::ALL {
            let badge = match target {
                Workspace::Setup => None,
                Workspace::Toolpaths => toolpath_badge(state),
                Workspace::Simulation => simulation_badge(state),
                Workspace::Readiness => readiness_badge(state),
            };
            // Every tab shares one baseline, so the last one answers for
            // all four.
            let bottom = workspace_tab(ui, target.label(), target, current, badge, events);
            seam_y = Some(bottom);
        }

        // Right-aligned workspace context info. DC3 deleted the hint strip
        // that sat here (Pattern C): it explained the workspace the operator
        // had already chosen, and nobody reads it twice. The Optimize row
        // stays, because a run in flight is real state and this bar is the
        // one surface that reports it in every workspace.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = tokens::SPACE_3;
            optimize_progress_row(ui, state, events);
        });
    });

    if let Some(y) = seam_y {
        let stroke = egui::Stroke::new(1.0_f32, tokens::HAIRLINE);
        ui.painter()
            .set(seam, egui::Shape::hline(bar.x_range(), y, stroke));
    }
}

/// The Optimize progress row (WP24), or nothing when no run is in flight.
///
/// This bar is where the row belongs, because `draw` runs for EVERY
/// workspace: the status bar has three call sites and none of them is the
/// Simulation layout, and the viewport overlay has one and never renders on
/// Readiness. With the full-screen placeholder deleted, this row is the one
/// surface that tells the operator a run is under way in every workspace.
///
/// It reports the run label, the rung of the search ladder, the candidate
/// inside that rung and the elapsed seconds (WP29, plan §33). The sentence
/// is built by [`crate::state::OptimizeRun::progress_text`], so the row and
/// the in-crate sentry read ONE sentence and the draw stays a draw. A run
/// that walks no candidate ladder — the rollup and the tier-map preview —
/// keeps the WP24 label-and-seconds text.
///
/// It reports NO time left. The whole-run candidate total is not known up
/// front, and a refine candidate costs more than a grid candidate, so one
/// mean second per candidate mixes two populations.
fn optimize_progress_row(ui: &mut egui::Ui, state: &AppState, events: &mut Vec<AppEvent>) {
    let Some(run) = state.optimize_run.as_ref() else {
        return;
    };
    let text = run.progress_text(&state.session);

    // The cancel arms THIS submit's flag and closes no window. None of the
    // three older cancels fits: `CancelCompute` cancels every lane, which
    // kills an MCP caller's queued job, and both Close arms close a window
    // the operator may not have open.
    let cancel =
        ui.add(crate::ui::components::Button::quiet("Cancel").enabled(!run.cancel_requested));
    automation::record(ui, "workspace_bar_cancel_optimize", &cancel, "Cancel");
    if cancel.clicked() {
        events.push(AppEvent::Ui(UiCommand::CancelOptimizeRun(NoArgs)));
    }
    ui.label(crate::ui::components::text::caption(text).color(tokens::CAUTION));
    ui.spinner();
    ui.separator();
}

/// Draw one tab. Returns the y of the seam the tab shares with the panel
/// below, so [`draw`] runs ONE hairline across the whole bar at that line.
fn workspace_tab(
    ui: &mut egui::Ui,
    label: &str,
    target: Workspace,
    current: Workspace,
    badge: Option<(String, Role)>,
    events: &mut Vec<AppEvent>,
) -> f32 {
    let is_active = current == target;

    // UP3. The active tab used to carry a private violet-blue that matched
    // nothing else in the product. An active tab is a SELECTED thing, so it
    // takes the same fill a selected row takes.
    let (bg, text_color) = if is_active {
        (tokens::ACCENT_QUIET, tokens::TEXT_STRONG)
    } else {
        (egui::Color32::TRANSPARENT, tokens::TEXT_MUTED)
    };

    // §2.2: the asymmetric radius stays, because a tab really does join the
    // panel below it. It is expressed as RADIUS_SM on the two TOP corners.
    let button = egui::Button::new(
        egui::RichText::new(label)
            .text_style(egui::TextStyle::Button)
            .color(text_color),
    )
    .fill(bg)
    .corner_radius(egui::CornerRadius {
        nw: tokens::RADIUS_SM,
        ne: tokens::RADIUS_SM,
        sw: 0,
        se: 0,
    })
    .min_size(egui::vec2(90.0, tokens::ROW_ACTION));

    let response = ui.add(button);
    let rect = response.rect;
    if response.clicked() && !is_active {
        events.push(AppEvent::Ui(UiCommand::SwitchWorkspace(target)));
    }

    // The active indicator, sitting on the seam the tab shares with the
    // panel below it. DC3: it paints AFTER the bar's hairline, so it breaks
    // that line and joins this one tab to the panel.
    if is_active {
        ui.painter().line_segment(
            [
                egui::pos2(rect.min.x, rect.max.y),
                egui::pos2(rect.max.x, rect.max.y),
            ],
            egui::Stroke::new(2.0_f32, tokens::ACCENT),
        );
    }

    // DC3, Pattern E. A badge sits ON its tab as a 6-point dot in the
    // top-right corner, and its count arrives on hover. That includes
    // Danger: safety remains legible in the Simulation inspector, Readiness
    // banner, status bar and simulation card without turning this navigation
    // strip into a second status surface. The tab's width therefore does not
    // change with its badge, which is what lets the bar read as one strip.
    if let Some((badge_text, badge_role)) = badge {
        let centre = egui::pos2(
            rect.max.x - tokens::SPACE_2 - BADGE_DOT_RADIUS,
            rect.min.y + tokens::SPACE_2 + BADGE_DOT_RADIUS,
        );
        ui.painter()
            .circle_filled(centre, BADGE_DOT_RADIUS, badge_role.text());
        let _ = response.on_hover_text(badge_text.trim());
    }

    ui.add_space(tokens::SPACE_1);
    rect.max.y
}

/// Badge for the Toolpaths tab: stale operations, else pending ones.
///
/// F2.2 (R0.1 §4.4). Stale outranks pending and is reported separately,
/// because the two ask the operator for different things: a pending
/// operation has never produced an answer, while a stale one has produced a
/// WRONG one that every other surface is still drawing and counting. Folding
/// them into one "N pending" would have gone on reading zero on a project
/// where every operation was edited after generation.
pub(crate) fn toolpath_badge(state: &AppState) -> Option<(String, Role)> {
    let (stale, pending) = crate::ui::readiness::freshness_counts(state);
    if stale > 0 {
        Some((format!("{stale} stale"), Role::Caution))
    } else if pending > 0 {
        Some((format!("{pending} pending"), Role::Caution))
    } else {
        None
    }
}

/// Shared collision badge text for the Simulation and Readiness tabs.
///
/// The dot itself is intentionally compact; this tooltip keeps its safety
/// count available without creating another status chip in the tab strip.
fn collision_badge(collision_count: usize) -> (String, Role) {
    (format!("{collision_count} safety"), Role::Danger)
}

/// Badge for the Simulation tab: stale, collisions, or empty.
fn simulation_badge(state: &AppState) -> Option<(String, Role)> {
    let sim = &state.simulation;

    if !sim.has_results() {
        return None;
    }

    // Collisions outrank staleness: a safety error must never hide behind
    // the yellow "stale" warning (SHE-003 — mirror readiness_badge's order).
    let collision_count = sim.checks.total_collision_count();
    if collision_count > 0 {
        return Some(collision_badge(collision_count));
    }

    if sim.is_stale(state.gui.edit_counter) {
        return Some((" stale".to_owned(), Role::Caution));
    }

    Some((" \u{2713}".to_owned(), Role::Ok))
}

/// Badge for the Setup tab: aggregate export readiness.
/// Shows issues that would prevent a clean export.
pub(crate) fn readiness_badge(state: &AppState) -> Option<(String, Role)> {
    let sim = &state.simulation;

    // F2.2 — from the one freshness model. This used to ask
    // `gui.toolpath_rt[..].result.is_none()`, the GUI store, which an edited
    // operation KEEPS so the viewport can still draw it. So the chip counted
    // an operation whose result the core had dropped as computed, and read
    // clean on a project where nothing could be reproduced.
    let (stale_ops, uncomputed) = crate::ui::readiness::freshness_counts(state);

    // Check collisions
    let collisions = sim.checks.total_collision_count();

    // Check simulation staleness
    let stale = sim.has_results() && sim.is_stale(state.gui.edit_counter);

    // Order: collisions, then stale operations, then uncomputed, then a
    // stale simulation. Safety outranks everything (SHE-003), and an edited
    // operation outranks an ungenerated one for the same reason it does on
    // the Toolpaths chip — one of the two is currently showing a wrong
    // answer rather than no answer.
    if collisions > 0 {
        Some(collision_badge(collisions))
    } else if stale_ops > 0 {
        Some((format!("{stale_ops} stale"), Role::Caution))
    } else if uncomputed > 0 {
        Some((format!("{uncomputed} uncomputed"), Role::Caution))
    } else if stale {
        Some(("sim stale".to_owned(), Role::Caution))
    } else if !sim.has_results() && state.session.toolpath_configs().iter().any(|tc| tc.enabled) {
        // "not simulated" is an ABSTENTION, not a caution: nothing was
        // measured, so there is nothing to review.
        Some(("not simulated".to_owned(), Role::Unknown))
    } else {
        None
    }
}
