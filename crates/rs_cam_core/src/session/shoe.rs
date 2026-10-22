//! H1: the shoe plan of a project, the one door the simulation request,
//! `collision_check`, the diagnostics and the export read.
//!
//! The plan walks every setup in order and every enabled toolpath with a
//! result in setup order: the run order of the G-code. A RUN is a list of
//! consecutive toolpaths with the same tool and the same shoe setting. A
//! tool change starts a run; a change of the shoe setting without a tool
//! change starts one too. The solver (`compute::shoe::solve_gap`) gives
//! each run its lowest clear gap; the toolpath's setting then gives its
//! resolved shoe (`compute::shoe::resolve`).

use crate::compute::obstacle::JoinStart;
use crate::compute::tool_config::ToolId;
use crate::ids::ToolpathId;
use crate::machine::head::{HeadFit, ResolvedShoe, ShoeSetting};

use super::diagnostics_types::{Verdict, VerdictEvidence, VerdictKind, VerdictSeverity};
use super::{ProjectSession, SessionError, SetupEvalContext};

/// The shoe state of one toolpath.
#[derive(Debug, Clone, PartialEq)]
pub struct ShoePlanRow {
    /// The toolpath's index in the session list.
    pub toolpath_index: usize,
    /// The toolpath id.
    pub toolpath_id: ToolpathId,
    /// The toolpath name.
    pub toolpath_name: String,
    /// The raw tool id of the toolpath.
    pub tool_id: ToolId,
    /// The first toolpath of its run: a tool change, a setup start, or a
    /// change of the shoe setting.
    pub run_start: bool,
    /// The setting the operator chose.
    pub setting: ShoeSetting,
    /// The shoe state the toolpath runs with.
    pub resolved: ResolvedShoe,
    /// The solver's lowest clear gap (mm) for the toolpath's run. `None`
    /// for a machine with no shoe and for a run set to Raised or Off.
    pub needed_gap_mm: Option<f64>,
}

impl ShoePlanRow {
    /// A stated gap below the gap the solver needs (H1): the shoe plate or
    /// the shoe shelf strikes the stock or an obstacle.
    #[must_use]
    pub fn gap_too_low(&self) -> bool {
        match (self.setting, self.needed_gap_mm) {
            (ShoeSetting::Gap { gap_mm }, Some(needed)) => gap_mm < needed,
            _ => false,
        }
    }

    /// The `project.shoe_gap_too_low` finding of this row (H1, Caution),
    /// or `None` when the stated gap is not below the solver gap. The
    /// project diagnostics and the GUI toolpath panel read this one text.
    #[must_use]
    pub fn gap_too_low_verdict(&self) -> Option<Verdict> {
        let (ShoeSetting::Gap { gap_mm }, Some(needed)) = (self.setting, self.needed_gap_mm) else {
            return None;
        };
        if gap_mm >= needed {
            return None;
        }
        Some(Verdict {
            severity: VerdictSeverity::Important,
            kind: VerdictKind::ShoeGapTooLow,
            headline: format!(
                "'{}' sets the dust shoe gap to {gap_mm:.0} mm; it needs \
                 {needed:.0} mm to keep the shoe plate clear of the stock \
                 and the obstacles",
                self.toolpath_name
            ),
            offender_toolpath_ids: vec![self.toolpath_id],
            fix_hint: format!(
                "Set the shoe gap to {needed:.0} mm or more, or set the shoe \
                 to Auto, Raised or Off"
            ),
            evidence: VerdictEvidence {
                move_index: None,
                z_value: Some(needed),
                count: None,
            },
        })
    }
}

impl ProjectSession {
    /// The shoe plan: one row per enabled toolpath with a result, in run
    /// order. Empty rows are never left out: a toolpath with no result is
    /// not in the run order, so it has no row.
    ///
    /// # Errors
    /// [`SessionError::Obstacle`] when an obstacle of a setup cannot be
    /// resolved, or an outward offset fails.
    pub fn shoe_plan(&self) -> Result<Vec<ShoePlanRow>, SessionError> {
        let head = &self.machine.head;
        let mut rows = Vec::new();
        for setup in &self.setups {
            let entries: Vec<usize> = setup
                .toolpath_indices
                .iter()
                .copied()
                .filter(|&i| {
                    self.toolpath_configs.get(i).is_some_and(|tc| tc.enabled)
                        && self.results.contains_key(&i)
                })
                .collect();
            if head.shoe.is_none() {
                let mut previous_tool = None;
                rows.extend(entries.iter().filter_map(|&i| {
                    let tc = self.toolpath_configs.get(i)?;
                    let run_start = previous_tool != Some(tc.tool_id);
                    previous_tool = Some(tc.tool_id);
                    Some(ShoePlanRow {
                        toolpath_index: i,
                        toolpath_id: tc.id,
                        toolpath_name: tc.name.clone(),
                        tool_id: tc.tool_id,
                        run_start,
                        setting: tc.shoe,
                        resolved: ResolvedShoe::NoShoe,
                        needed_gap_mm: None,
                    })
                }));
                continue;
            }
            let obstacles = self.obstacle_set_for_setup(setup)?;
            let stock = SetupEvalContext::build_for_setup(self, Some(setup)).heights_stock_bbox;
            // The runs: consecutive entries with one tool and one setting.
            let mut runs: Vec<Vec<usize>> = Vec::new();
            for &i in &entries {
                let key = self.toolpath_configs.get(i).map(|tc| (tc.tool_id, tc.shoe));
                let same = runs
                    .last()
                    .and_then(|run| run.last())
                    .and_then(|&last| self.toolpath_configs.get(last))
                    .map(|tc| (tc.tool_id, tc.shoe));
                match runs.last_mut() {
                    Some(run) if same.is_some() && same == key => run.push(i),
                    _ => runs.push(vec![i]),
                }
            }
            for run in runs {
                let Some(first) = run.first().and_then(|&i| self.toolpath_configs.get(i)) else {
                    continue;
                };
                let setting = first.shoe;
                let tool = self
                    .tool_by_id(first.tool_id)
                    .ok_or(SessionError::ToolNotFound(first.tool_id))?;
                let solution = match setting {
                    ShoeSetting::Raised | ShoeSetting::Off => None,
                    ShoeSetting::Auto | ShoeSetting::Gap { .. } => {
                        let toolpaths: Vec<&crate::toolpath::Toolpath> = run
                            .iter()
                            .filter_map(|i| self.results.get(i).map(|r| r.toolpath()))
                            .collect();
                        // The join of each toolpath: unknown at the run start,
                        // then the previous end. (After a setting change with
                        // no tool change the check reads the previous end;
                        // the solver reads Unknown there, which asks the
                        // first target to clear every ceiling.)
                        let joined: Vec<(&crate::toolpath::Toolpath, JoinStart)> = toolpaths
                            .iter()
                            .enumerate()
                            .map(|(k, tp)| {
                                let join = JoinStart::after_previous(
                                    toolpaths.iter().take(k).map(|t| ((), *t)),
                                    &(),
                                );
                                (*tp, join)
                            })
                            .collect();
                        crate::compute::shoe::solve_gap(
                            head,
                            tool.stickout,
                            &obstacles,
                            &stock,
                            &joined,
                        )?
                    }
                };
                let resolved = crate::compute::shoe::resolve(head, setting, solution);
                for (k, &i) in run.iter().enumerate() {
                    let Some(tc) = self.toolpath_configs.get(i) else {
                        continue;
                    };
                    rows.push(ShoePlanRow {
                        toolpath_index: i,
                        toolpath_id: tc.id,
                        toolpath_name: tc.name.clone(),
                        tool_id: tc.tool_id,
                        run_start: k == 0,
                        setting,
                        resolved,
                        needed_gap_mm: solution.map(|s| s.needed_gap_mm),
                    });
                }
            }
        }
        Ok(rows)
    }

    /// The resolved shoe of every toolpath, for a reader that walks many
    /// toolpaths: one plan, then a lookup. A toolpath with no row reads
    /// [`Self::resolved_shoe_for_toolpath`]'s rule.
    pub fn resolved_shoe_lookup(&self) -> impl Fn(usize) -> ResolvedShoe + '_ {
        let head = &self.machine.head;
        let rows = if head.shoe.is_some() {
            self.shoe_plan().unwrap_or_else(|e| {
                tracing::warn!(error = %e, "the shoe plan failed; the check reads the bare head");
                Vec::new()
            })
        } else {
            Vec::new()
        };
        move |index| {
            if head.shoe.is_none() {
                return ResolvedShoe::NoShoe;
            }
            rows.iter()
                .find(|row| row.toolpath_index == index)
                .map_or(ResolvedShoe::Raised, |row| row.resolved)
        }
    }

    /// The shoe state of toolpath `index` from [`Self::shoe_plan`].
    /// A toolpath outside the run order (disabled, or with no result) reads
    /// its own setting with no solver answer: Auto then reads Raised, the
    /// bare head. A plan that cannot be built also reads Raised.
    #[must_use]
    pub fn resolved_shoe_for_toolpath(&self, index: usize) -> ResolvedShoe {
        let head = &self.machine.head;
        if head.shoe.is_none() {
            return ResolvedShoe::NoShoe;
        }
        let setting = self
            .toolpath_configs
            .get(index)
            .map(|tc| tc.shoe)
            .unwrap_or_default();
        match self.shoe_plan() {
            Ok(rows) => rows
                .iter()
                .find(|row| row.toolpath_index == index)
                .map_or_else(
                    || crate::compute::shoe::resolve(head, setting, None),
                    |row| row.resolved,
                ),
            Err(e) => {
                tracing::warn!(error = %e, "the shoe plan failed; the check reads the bare head");
                ResolvedShoe::Raised
            }
        }
    }

    /// The head fit the checks read for toolpath `index` (H1): the machine
    /// head with the shoe at the toolpath's resolved position.
    #[must_use]
    pub fn head_fit_for_toolpath(&self, index: usize) -> HeadFit {
        crate::compute::simulate::entry_head_fit(
            &self.machine.head,
            self.resolved_shoe_for_toolpath(index),
        )
    }

    /// The fresh stock box of the setup of toolpath `index`, in the
    /// setup frame (the frame the toolpath emits in). The shoe parts and
    /// the shelf floor of the checks read it.
    #[must_use]
    pub fn setup_stock_box_for_toolpath(&self, index: usize) -> crate::geo::BoundingBox3 {
        SetupEvalContext::build_for_setup(self, self.find_setup_for_toolpath_index(index))
            .heights_stock_bbox
    }
}
