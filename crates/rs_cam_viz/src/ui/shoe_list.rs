//! H2 (`planning/op_architecture_2026-10-14/PLAN.md`, item H): the run
//! sheet. One line per tool change or shoe change, in run order: the setup,
//! the tool, the toolpaths and the shoe setting.
//!
//! Operator ruling 2026-10-16: the G-code carries no shoe prompt (gSender
//! strips comments), so this list is where the operator reads the shoe
//! setting for each tool change. The lines come from the core shoe plan
//! through `state::shoe_plan`; `inspect_machine` prints the same plan as
//! `shoe_plan`. The Readiness page and the export wizard draw this one
//! function.

use crate::state::AppState;
use crate::state::shoe_plan::{RunLine, run_lines, unplanned_toolpaths};
use crate::ui::components::{Banner, DataRow, DataTable, OneLine, Role, SectionHeader};
use crate::ui::tokens;

const SECTION_HOVER: &str = "One line per tool change or shoe change, in run order. The \
     G-code carries no shoe prompt: set the dust shoe from this list at each tool change. \
     Set a toolpath's shoe in its Heights tab.";

/// The value text of one line: the shoe setting in core's words.
#[must_use]
pub fn line_value(line: &RunLine) -> String {
    line.shoe.clone().unwrap_or_else(|| "no shoe".to_owned())
}

/// The name text of line `n` (from 1): the setup and the tool.
#[must_use]
pub fn line_name(n: usize, line: &RunLine) -> String {
    if line.setup.is_empty() {
        format!("{n}. {}", line.tool)
    } else {
        format!("{n}. {} \u{00B7} {}", line.setup, line.tool)
    }
}

/// The hover of one line: the toolpaths of the run and the solver gap.
fn line_hover(line: &RunLine) -> String {
    let mut text = format!("Toolpaths: {}", line.toolpaths.join(", "));
    if let Some(needed) = line.needed_gap_mm {
        text.push_str(&format!("\nSolver gap: {needed:.0} mm above the tip"));
    }
    if line.gap_too_low {
        text.push_str("\nThe stated gap is below the solver gap.");
    }
    text
}

/// Draw the run sheet of the project.
pub fn draw_run_sheet(ui: &mut egui::Ui, state: &AppState) {
    let session = &state.session;
    let plan = state.gui.shoe_plan.plan(session);
    SectionHeader::new("Run sheet")
        .hover(SECTION_HOVER)
        .show(ui);
    if let Err(error) = &plan.rows {
        let _ = Banner::new(Role::Danger, format!("The shoe plan failed: {error}")).show(ui);
        return;
    }
    let rows = plan.rows();
    let lines = run_lines(session, rows);
    if lines.is_empty() {
        ui.add(
            OneLine::new("No toolpath has a result yet.")
                .color(tokens::TEXT_MUTED)
                .detail("Generate the toolpaths. The run sheet lists each run in order."),
        );
    } else {
        // One table row per run (the setup, the tool, the shoe), then the
        // toolpaths of the run on a muted line under it.
        for (k, line) in lines.iter().enumerate() {
            let row =
                DataRow::value(line_name(k + 1, line), line_value(line)).hover(line_hover(line));
            let row = if line.gap_too_low {
                row.status(Role::Caution, "low")
            } else {
                row
            };
            let _ = DataTable::new(("run_sheet", k))
                .name_share(0.6)
                .row(row)
                .show(ui);
            ui.add(
                OneLine::new(line.toolpaths.join(", "))
                    .color(tokens::TEXT_MUTED)
                    .detail(line_hover(line)),
            );
        }
    }
    let missing = unplanned_toolpaths(session, rows);
    if !missing.is_empty() {
        ui.add(
            OneLine::new(format!("{} not in the run sheet", missing.len()))
                .color(tokens::CAUTION)
                .detail(format!(
                    "These enabled toolpaths have no result yet: {}",
                    missing.join(", ")
                )),
        );
    }
}
