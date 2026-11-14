//! Feeds contract P29-10: THE one door from a toolpath to its limit rows
//! (`ProjectSession::limit_rows`). The GUI, MCP and the CLI read it, as
//! `suggest_slots` is the one door to the Suggest context.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use super::{ProjectSession, ToolpathConfig};
use crate::feeds::FeedsError;
use crate::limits::{CheckRow, rows_of_rpm_choice, rows_of_verdict, rows_without_a_verdict};
use crate::stock::simulation_cut::SimulationCutTrace;

/// The limit rows of one toolpath. Each part is built when a surface asks
/// for it, so a surface that shows one part pays for that part only.
#[derive(Clone, Copy)]
pub struct LimitRows<'a> {
    session: &'a ProjectSession,
    index: usize,
    toolpath: &'a ToolpathConfig,
    trace: Option<&'a Arc<SimulationCutTrace>>,
}

impl LimitRows<'_> {
    /// The rows of the safe envelope: one planned row per declared cut and
    /// planned limit at the proposal, then the stated point against each
    /// limit (`SafeEnvelope::rows`). `None` with no tool; `Some(Err)` for a
    /// refused pairing.
    #[must_use]
    pub fn envelope(&self) -> Option<Result<Vec<CheckRow>, FeedsError>> {
        self.session
            .safe_envelope(self.toolpath)
            .map(|env| env.map(|env| env.rows().to_vec()))
    }

    /// The rows of the gate verdict after the simulation
    /// (`limits::rows_of_verdict`), from the session's memo of the load
    /// report on the trace the caller holds. Never empty (lead ruling 6): a
    /// toolpath that the report leaves out (a disabled op) gets the rows of
    /// its verdict with no trace, the no-simulation rows; a toolpath with
    /// no load context (no tool, a tool outside its domain) gets one
    /// `NoNumber(NoToolBody)` row per gate and cut group.
    #[must_use]
    pub fn verdict(&self) -> Vec<CheckRow> {
        let report = self.session.tool_load_report_for(self.trace);
        if let Some(v) = report
            .per_toolpath
            .iter()
            .find(|v| v.toolpath_id == self.toolpath.id)
        {
            return rows_of_verdict(v);
        }
        match self.session.load_context_parts(self.index) {
            Some(parts) => rows_of_verdict(&crate::tool_load::evaluate_toolpath(
                &parts.ctx(),
                None,
                Some(self.session.machine()),
                &crate::tool_load::ToleranceBands::default(),
            )),
            None => rows_without_a_verdict(crate::limits::NoNumberReason::NoToolBody),
        }
    }

    /// The rows of the RPM choice (`limits::rows_of_rpm_choice`). Empty
    /// when Suggest made no choice (`MatchChart`) or refused.
    #[must_use]
    pub fn rpm(&self) -> Vec<CheckRow> {
        self.session
            .cutter_op_profile(self.toolpath)
            .and_then(|profile| {
                profile
                    .feeds
                    .as_ref()?
                    .rpm_choice
                    .as_ref()
                    .map(rows_of_rpm_choice)
            })
            .unwrap_or_default()
    }
}

impl ProjectSession {
    /// P29-11 11-6: **the depth rows that a narration reads**: the
    /// force-budget row of each cut group of the toolpath at `index`, from
    /// the one door (`limit_rows(..).verdict()`, the memo of the load
    /// report on `trace`; the gate rows do not read the run modulation).
    /// Both narration doors (the core `narrate_toolpath` and the GUI MCP
    /// door) call it, so the narration judges no depth of its own. Empty
    /// when no toolpath has the index.
    #[must_use]
    pub fn narration_depth_rows(
        &self,
        index: usize,
        trace: Option<&Arc<SimulationCutTrace>>,
    ) -> Vec<CheckRow> {
        self.limit_rows(index, trace)
            .map(|rows| {
                rows.verdict()
                    .into_iter()
                    .filter(|row| row.check().limit == crate::limits::LimitId::ForceBudget)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// **THE door to the force plane of the toolpath at `index`** (P29-10
    /// 10g, lead rulings O4, R1, R9): the force field of its body cut at
    /// its steady feed over its effective spindle speed (the project
    /// default when the op states none), over `[0, diameter_mm]` x
    /// `[0, ap_top_mm]`, with the level lines of the machine force budget
    /// (`tool_load::cut_force::force_plane`). The caller states the depth
    /// top: the top of the chart that draws the field. `None` with no load
    /// context or no force field (a drill, no force line). A surface builds
    /// it once per load report, never per frame.
    #[must_use]
    pub fn force_plane(
        &self,
        index: usize,
        diameter_mm: f64,
        ap_top_mm: f64,
    ) -> Option<crate::tool_load::force_field::ForcePlane> {
        let tc = self.toolpath_config_at(index)?;
        let parts = self.load_context_parts(index)?;
        let rpm = f64::from(crate::compute::catalog::effective_spindle_rpm(
            &tc.operation,
            self.post_config().spindle_speed,
        ));
        crate::tool_load::cut_force::force_plane(
            &parts.ctx(),
            self.machine(),
            rpm,
            diameter_mm,
            ap_top_mm,
        )
    }

    /// **THE door to the limit rows of the toolpath at `index`** (P29-10,
    /// lead ruling L7). `trace` is the simulation the caller reads, as the
    /// `Arc` that holds it (the GUI keeps its trace beside the session; the
    /// CLI passes the session's own). P30-2: the envelope and RPM rows read
    /// the feed modulation of the toolpath's stored result
    /// (`ProjectSession::run_modulation_of`); no caller states it, so GUI,
    /// MCP and the CLI give the same rows for the same session state.
    /// `None` when no toolpath has the index.
    #[must_use]
    pub fn limit_rows<'a>(
        &'a self,
        index: usize,
        trace: Option<&'a Arc<SimulationCutTrace>>,
    ) -> Option<LimitRows<'a>> {
        let toolpath = self.toolpath_config_at(index)?;
        Some(LimitRows {
            session: self,
            index,
            toolpath,
            trace,
        })
    }
}

/// The session memo of [`ProjectSession::stated_cut_over_a_limit`] (P30-2,
/// review 2026-11-15 F4).
/// The GUI pre-flight modal and the readiness panel read the export
/// warnings on each frame; an envelope costs about 0.2 s, so the face of
/// each toolpath is kept against the `Debug` text of every input of its
/// envelope (the `FeedsKey` idiom of the GUI memo: the text can only
/// over-key). A clone of the session starts empty.
#[derive(Default)]
pub(crate) struct UnmodulatedRowMemo {
    entries: Mutex<HashMap<crate::ids::ToolpathId, (String, GatingRows)>>,
}

/// The memo value of one toolpath: its stated rows with the gating tone,
/// worst first, or `None` when the toolpath has no tool or the envelope
/// refuses.
type GatingRows = Option<Vec<GatingRow>>;

/// One stated row with the gating tone, in the memo: its limit and its
/// face. The memo keeps them worst first (`CheckRow::rank_cmp`).
#[derive(Debug, Clone)]
struct GatingRow {
    limit: crate::limits::LimitId,
    face: String,
}

/// Review 2026-11-15 R8: what judges the cut of an op after its
/// simulation, so which stated rows the export warning keeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimulationCover {
    /// No simulation is fresh for the op: the stated rows are the only
    /// judgement of the cut, so every limit counts.
    NoFreshSimulation,
    /// A simulation is fresh for the op: its sample gates judge the cut.
    /// Only a limit with no sample gate counts
    /// (`tool_load::groups::judged_only_before_the_simulation`, the row
    /// `S-NoSampleGate`).
    FreshSimulation,
}

/// Review 2026-11-15 F4: the stated rows of one toolpath that exceed with
/// the gating tone (`Tone::Exceeds`), whatever the limit: the worst row by
/// `CheckRow::rank_cmp` and the count of the others.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatedCutOverLimit {
    /// The face of the worst row.
    pub face: String,
    /// The count of the other stated rows with the gating tone.
    pub others: usize,
    /// What judged the cut after the simulation (review R8).
    pub cover: SimulationCover,
}

impl StatedCutOverLimit {
    /// The warning text after the toolpath name: the worst row, the count
    /// of the others, and why the stated rows judge the cut.
    #[must_use]
    pub fn words(&self) -> String {
        let others = match self.others {
            0 => String::new(),
            1 => "; 1 more stated row over a limit".to_owned(),
            n => format!("; {n} more stated rows over a limit"),
        };
        let why = match self.cover {
            SimulationCover::NoFreshSimulation => "no current simulation measured this op",
            SimulationCover::FreshSimulation => {
                "no sample gate of the simulation judges this limit"
            }
        };
        format!("{}{others}; {why}", self.face)
    }
}

impl Clone for UnmodulatedRowMemo {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl ProjectSession {
    /// **Review 2026-11-15 F4: the stated rows of the toolpath at `index`
    /// that exceed with the gating tone (`Tone::Exceeds`), whatever the
    /// limit**: the face of the worst by `CheckRow::rank_cmp` and the count
    /// of the others, from the one door
    /// ([`Self::limit_rows`]`(..).envelope()`). `None` when no such row
    /// exists, the toolpath has no tool or the envelope refuses. The export
    /// door raises `export.stated_cut_over_limit` from it.
    ///
    /// Review R8: with [`SimulationCover::FreshSimulation`] only the rows of
    /// a limit that no sample gate judges count
    /// ([`crate::tool_load::groups::judged_only_before_the_simulation`]):
    /// the gate rows of the simulation judge every other limit.
    #[must_use]
    pub fn stated_cut_over_a_limit(
        &self,
        index: usize,
        cover: SimulationCover,
    ) -> Option<StatedCutOverLimit> {
        let tc = self.toolpath_config_at(index)?;
        let tool = self.toolpath_tool(tc)?;
        let key = format!(
            "{:?}|{tool:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}",
            tc.operation,
            self.machine(),
            self.stock_config().material,
            self.post_config().spindle_policy(),
            self.suggest_slots(tc),
            tc.dressups,
            self.run_modulation_of(index),
        );
        let mut guard = match self.unmodulated_row_memo.entries.lock() {
            Ok(guard) => guard,
            // An entry is written only after its build returns.
            Err(poisoned) => poisoned.into_inner(),
        };
        let gating = match guard.get(&tc.id) {
            Some((stored, gating)) if *stored == key => gating.clone(),
            Some(_) | None => {
                let gating = self
                    .limit_rows(index, None)
                    .and_then(|rows| rows.envelope())
                    .and_then(Result::ok)
                    .map(|rows| {
                        let mut gating: Vec<&crate::limits::CheckRow> = rows
                            .iter()
                            .filter(|row| {
                                row.is_stated() && row.tone() == crate::limits::Tone::Exceeds
                            })
                            .collect();
                        gating.sort_by(|a, b| a.rank_cmp(b));
                        gating
                            .into_iter()
                            .map(|row| GatingRow {
                                limit: row.check().limit,
                                face: row.face().to_owned(),
                            })
                            .collect::<Vec<_>>()
                    });
                let _ = guard.insert(tc.id, (key, gating.clone()));
                gating
            }
        };
        let kept: Vec<GatingRow> = gating?
            .into_iter()
            .filter(|row| match cover {
                SimulationCover::NoFreshSimulation => true,
                SimulationCover::FreshSimulation => {
                    crate::tool_load::groups::judged_only_before_the_simulation(row.limit)
                }
            })
            .collect();
        kept.first().map(|worst| StatedCutOverLimit {
            face: worst.face.clone(),
            others: kept.len() - 1,
            cover,
        })
    }
}
