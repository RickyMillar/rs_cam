//! H2 (`planning/op_architecture_2026-10-14/PLAN.md`, item H): the GUI view
//! of the core shoe plan.
//!
//! Core owns the plan (`ProjectSession::shoe_plan`) and its words
//! (`ResolvedShoe::label`, `ShoePlanRow::gap_too_low_verdict`). The MCP
//! tool `inspect_machine` prints the same rows as `shoe_plan`. This file
//! holds no second rule. It holds:
//!
//! - [`ShoePlanMemo`]: the plan runs the shoe solver over every move, so a
//!   panel that draws every frame reads it through this memo. The memo key
//!   is every input the plan reads.
//! - [`run_lines`]: the run sheet. One line per run (a tool change, a
//!   setup start, or a change of the shoe setting).
//! - [`auto_text`]: the text of the toolpath panel's Auto choice.

use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{Arc, Mutex};

use rs_cam_core::machine::head::ShoeSetting;
use rs_cam_core::session::{ProjectSession, ShoePlanRow};

/// The shoe plan of the project, as the GUI reads it.
#[derive(Debug, Clone, PartialEq)]
pub struct ShoePlanView {
    /// The machine head has a dust shoe.
    pub has_shoe: bool,
    /// The plan rows in run order, or the core refusal text.
    pub rows: Result<Vec<ShoePlanRow>, String>,
}

impl ShoePlanView {
    /// The plan rows, or an empty list when the plan failed.
    #[must_use]
    pub fn rows(&self) -> &[ShoePlanRow] {
        self.rows.as_deref().unwrap_or(&[])
    }

    /// The row of toolpath `index`, if the toolpath is in the run order.
    #[must_use]
    pub fn row(&self, index: usize) -> Option<&ShoePlanRow> {
        self.rows().iter().find(|row| row.toolpath_index == index)
    }
}

/// The memo of [`ShoePlanView`]. One build per change of an input.
#[derive(Debug, Default)]
pub struct ShoePlanMemo {
    entry: Mutex<Option<(u64, Arc<ShoePlanView>)>>,
}

impl ShoePlanMemo {
    /// The shoe plan of `session` as it is now.
    pub fn plan(&self, session: &ProjectSession) -> Arc<ShoePlanView> {
        let key = plan_key(session);
        let mut guard = match self.entry.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some((stored, view)) = guard.as_ref()
            && *stored == key
        {
            return Arc::clone(view);
        }
        let view = Arc::new(ShoePlanView {
            has_shoe: session.machine().head.shoe.is_some(),
            rows: session.shoe_plan().map_err(|e| e.to_string()),
        });
        *guard = Some((key, Arc::clone(&view)));
        view
    }
}

/// A digest of every input of `ProjectSession::shoe_plan`: the head, the
/// setups (order, obstacles, stock changes, frame), the stock, the tools,
/// each toolpath's run key and result, and each model's geometry (an
/// obstacle can read a model).
fn plan_key(session: &ProjectSession) -> u64 {
    let mut hasher = DefaultHasher::new();
    format!(
        "{:?}",
        (
            &session.machine().head,
            session.list_setups(),
            session.stock_config(),
            session.tools(),
        )
    )
    .hash(&mut hasher);
    for (index, tc) in session.toolpath_configs().iter().enumerate() {
        (tc.id.raw(), tc.enabled, tc.tool_id, &tc.name).hash(&mut hasher);
        format!("{:?}", tc.shoe).hash(&mut hasher);
        session
            .result_at(index)
            .map(|result| Arc::as_ptr(result.annotated()) as usize)
            .hash(&mut hasher);
    }
    for model in session.models() {
        model.id.hash(&mut hasher);
        model
            .mesh
            .as_ref()
            .map(|mesh| Arc::as_ptr(mesh) as usize)
            .hash(&mut hasher);
        (Arc::as_ptr(&model.items) as usize).hash(&mut hasher);
    }
    hasher.finish()
}

/// One line of the run sheet: one run of toolpaths with one tool and one
/// shoe setting, in run order.
#[derive(Debug, Clone, PartialEq)]
pub struct RunLine {
    /// The setup name.
    pub setup: String,
    /// The tool id.
    pub tool_id: rs_cam_core::ids::ToolId,
    /// The tool name.
    pub tool: String,
    /// The toolpath names of the run, in order.
    pub toolpaths: Vec<String>,
    /// The shoe text of the run (`ResolvedShoe::label`), or `None` for a
    /// machine with no shoe.
    pub shoe: Option<String>,
    /// The setting of the run (every toolpath of a run has one setting).
    pub setting: ShoeSetting,
    /// The solver's lowest clear gap (mm) for the run.
    pub needed_gap_mm: Option<f64>,
    /// The stated gap is below the solver gap.
    pub gap_too_low: bool,
}

/// The run sheet of `rows`: one line per run start.
#[must_use]
pub fn run_lines(session: &ProjectSession, rows: &[ShoePlanRow]) -> Vec<RunLine> {
    let mut lines: Vec<RunLine> = Vec::new();
    for row in rows {
        if row.run_start || lines.is_empty() {
            let setup = session
                .list_setups()
                .iter()
                .find(|setup| setup.toolpath_indices().contains(&row.toolpath_index))
                .map_or_else(String::new, |setup| setup.name.clone());
            let tool = session
                .tools()
                .iter()
                .find(|tool| tool.id == row.tool_id)
                .map_or_else(|| format!("tool {}", row.tool_id), |tool| tool.name.clone());
            lines.push(RunLine {
                setup,
                tool_id: row.tool_id,
                tool,
                toolpaths: Vec::new(),
                shoe: row.resolved.label(),
                setting: row.setting,
                needed_gap_mm: row.needed_gap_mm,
                gap_too_low: false,
            });
        }
        if let Some(line) = lines.last_mut() {
            line.toolpaths.push(row.toolpath_name.clone());
            line.gap_too_low |= row.gap_too_low();
        }
    }
    lines
}

/// The enabled toolpaths that are not in the run order: they have no
/// result yet, so the solver has no moves to read.
#[must_use]
pub fn unplanned_toolpaths(session: &ProjectSession, rows: &[ShoePlanRow]) -> Vec<String> {
    session
        .toolpath_configs()
        .iter()
        .enumerate()
        .filter(|(index, tc)| tc.enabled && !rows.iter().any(|row| row.toolpath_index == *index))
        .map(|(_, tc)| tc.name.clone())
        .collect()
}

/// The text of the Auto choice for toolpath `index`: the solver's answer
/// for its run, in core's words. A toolpath with no row (disabled, or no
/// result) has no answer yet.
#[must_use]
pub fn auto_text(view: &ShoePlanView, index: usize) -> String {
    if let Err(error) = &view.rows {
        return format!("Auto: the shoe plan failed ({error})");
    }
    match view.row(index) {
        Some(row) if row.setting == ShoeSetting::Auto => match row.resolved.label() {
            Some(label) => format!("Auto: {label}"),
            None => "Auto: no shoe".to_owned(),
        },
        Some(row) => match row.needed_gap_mm {
            Some(needed) => format!("Auto: brush {needed:.0} mm above tip"),
            None => "Auto: not solved for this setting".to_owned(),
        },
        None => "Auto: generate to solve".to_owned(),
    }
}

#[cfg(test)]
#[allow(
    // SAFETY: test code; a failed lookup is a failed test.
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use rs_cam_core::ToolpathId;
    use rs_cam_core::machine::head::ResolvedShoe;

    fn row(index: usize, tool: usize, run_start: bool, resolved: ResolvedShoe) -> ShoePlanRow {
        ShoePlanRow {
            toolpath_index: index,
            toolpath_id: ToolpathId::from_raw(index),
            toolpath_name: format!("op {index}"),
            tool_id: rs_cam_core::ids::ToolId::from_raw(tool),
            run_start,
            setting: ShoeSetting::Auto,
            resolved,
            needed_gap_mm: Some(25.0),
        }
    }

    #[test]
    fn the_auto_text_reads_core_label_and_names_a_missing_row_h2() {
        let view = ShoePlanView {
            has_shoe: true,
            rows: Ok(vec![
                row(0, 1, true, ResolvedShoe::Gap { gap_mm: 25.0 }),
                row(1, 2, true, ResolvedShoe::Raised),
            ]),
        };
        assert_eq!(auto_text(&view, 0), "Auto: brush 25 mm above tip");
        assert_eq!(auto_text(&view, 1), "Auto: raise clear");
        assert_eq!(
            auto_text(&view, 2),
            "Auto: generate to solve",
            "a toolpath with no row has no solver answer; it must not read Raised"
        );
    }
}
