//! Feeds contract P29-10: THE one door from a toolpath to its limit rows
//! (`ProjectSession::limit_rows`). The GUI, MCP and the CLI read it, as
//! `suggest_slots` is the one door to the Suggest context.

use std::sync::Arc;

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
    /// CLI passes the session's own). `None` when no toolpath has the
    /// index.
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
