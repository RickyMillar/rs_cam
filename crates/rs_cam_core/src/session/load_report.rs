//! The memo behind [`ProjectSession::tool_load_report_for`].
//!
//! [`crate::gcode::project_load_report`] runs the load gates and the
//! kinematic reading for every enabled toolpath, and the freshness check
//! hashes every move of every toolpath. On a project with a 515 146-move
//! finish that is far too slow for a frame, and the GUI asked for it on
//! every frame from several panels (2026-10-02 profile).
//!
//! The memo keeps the last report and the inputs it read. The key holds
//! each input the builder reads, compared by value or by allocation
//! identity. No key part is a counter, so no mutation path can forget to
//! move it:
//!
//! - each toolpath row: id, `enabled`, tool id, and the serialized
//!   operation (feed, plunge rate, operation kind and family);
//! - each stored result, by `Weak` identity of its annotated toolpath and
//!   its drill operation;
//! - the tool library, the stock material and the post configuration, by
//!   value;
//! - the serialized machine profile (kinematics, feed caps, power model);
//! - the simulation trace, by `Weak` identity.
//!
//! The key is `O(toolpaths)` to build and compare. It never reads a move.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};

use crate::compute::tool_config::ToolConfig;
use crate::material::Material;
use crate::ops::drill_op::DrillOp;
use crate::stock::simulation_cut::SimulationCutTrace;
use crate::tool_load::ToolLoadReport;
use crate::trace::toolpath_spans::AnnotatedToolpath;

use super::ProjectSession;

/// Liveness-checked allocation identity.
///
/// A live `Weak` keeps the allocation reserved, so no other `Arc` can get
/// the same address while the key exists (no ABA). `Arc::make_mut` on an
/// `Arc` with only `Weak` peers moves the value to a new allocation, so an
/// in-place rewrite of the trace also reads as a new identity.
fn same_allocation<T>(stored: Option<&Weak<T>>, live: Option<&Arc<T>>) -> bool {
    match (stored, live) {
        (None, None) => true,
        (Some(weak), Some(arc)) => weak.upgrade().is_some_and(|up| Arc::ptr_eq(&up, arc)),
        _ => false,
    }
}

/// The per-toolpath part of the key.
struct RowKey {
    id: crate::ids::ToolpathId,
    enabled: bool,
    tool_id: usize,
    operation: Vec<u8>,
    annotated: Option<Weak<AnnotatedToolpath>>,
    drill_op: Option<Weak<DrillOp>>,
}

/// Every input [`crate::gcode::project_load_report`] reads.
struct ReportKey {
    rows: Vec<RowKey>,
    tools: Vec<ToolConfig>,
    material: Material,
    post: crate::gcode::PostConfig,
    machine: Vec<u8>,
    trace: Option<Weak<SimulationCutTrace>>,
}

/// The serialized form of a key part, or `None` when serde refuses it.
/// A refused part disables the memo for that call; it never matches.
fn serialized<T: serde::Serialize>(value: &T) -> Option<Vec<u8>> {
    serde_json::to_vec(value).ok()
}

impl ReportKey {
    /// Does this key describe `session` and `trace` as they are now?
    fn matches(&self, session: &ProjectSession, trace: Option<&Arc<SimulationCutTrace>>) -> bool {
        if !same_allocation(self.trace.as_ref(), trace) {
            return false;
        }
        if self.material != session.stock.material
            || self.post != session.post
            || self.tools != session.tools
        {
            return false;
        }
        let configs = &session.toolpath_configs;
        if self.rows.len() != configs.len() {
            return false;
        }
        for (index, (row, tc)) in self.rows.iter().zip(configs).enumerate() {
            if row.id != tc.id || row.enabled != tc.enabled || row.tool_id != tc.tool_id {
                return false;
            }
            let result = session.results.get(&index);
            if !same_allocation(row.annotated.as_ref(), result.map(|r| r.annotated())) {
                return false;
            }
            if !same_allocation(row.drill_op.as_ref(), result.and_then(|r| r.drill_op())) {
                return false;
            }
            if serialized(&tc.operation).as_deref() != Some(row.operation.as_slice()) {
                return false;
            }
        }
        serialized(&session.machine).as_deref() == Some(self.machine.as_slice())
    }

    /// The key for `session` and `trace` as they are now, or `None` when a
    /// part does not serialize.
    fn capture(session: &ProjectSession, trace: Option<&Arc<SimulationCutTrace>>) -> Option<Self> {
        let mut rows = Vec::with_capacity(session.toolpath_configs.len());
        for (index, tc) in session.toolpath_configs.iter().enumerate() {
            let result = session.results.get(&index);
            rows.push(RowKey {
                id: tc.id,
                enabled: tc.enabled,
                tool_id: tc.tool_id,
                operation: serialized(&tc.operation)?,
                annotated: result.map(|r| Arc::downgrade(r.annotated())),
                drill_op: result.and_then(|r| r.drill_op()).map(Arc::downgrade),
            });
        }
        Some(Self {
            rows,
            tools: session.tools.clone(),
            material: session.stock.material.clone(),
            post: session.post.clone(),
            machine: serialized(&session.machine)?,
            trace: trace.map(Arc::downgrade),
        })
    }
}

/// The stored report and the key it answers.
struct Entry {
    key: ReportKey,
    report: Arc<ToolLoadReport>,
}

/// The session's tool-load report memo.
///
/// A clone of the session starts with an empty memo. The memo is derived
/// data, and a what-if copy diverges from its source at once.
#[derive(Default)]
pub(crate) struct LoadReportMemo {
    entry: Mutex<Option<Entry>>,
    /// How many times the memo called the builder. Read by the sentries
    /// through [`ProjectSession::tool_load_report_builds`].
    builds: AtomicU64,
}

impl Clone for LoadReportMemo {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl ProjectSession {
    /// **The project tool-load report, built once per change.**
    ///
    /// The answer equals [`crate::gcode::project_load_report`] for the same
    /// session and trace. The difference is the cost: a call whose inputs
    /// did not change since the last call returns the stored report and
    /// does no analysis. See the module documentation for the key.
    ///
    /// `trace` is the simulation the caller reads, as the `Arc` that holds
    /// it. The memo keys on its identity, so pass the same `Arc` on every
    /// call; a copy of the trace in a new `Arc` rebuilds the report.
    ///
    /// Every surface that shows the report (inspector, readiness, triage,
    /// MCP) reads it here, so every surface shows the same numbers.
    pub fn tool_load_report_for(
        &self,
        trace: Option<&Arc<SimulationCutTrace>>,
    ) -> Arc<ToolLoadReport> {
        let mut guard = match self.load_report_memo.entry.lock() {
            Ok(guard) => guard,
            // A panic during an earlier build leaves no partial entry: the
            // entry is written only after the build returns. The stored
            // value is therefore still whole.
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some(entry) = guard.as_ref()
            && entry.key.matches(self, trace)
        {
            return Arc::clone(&entry.report);
        }
        self.load_report_memo.builds.fetch_add(1, Ordering::Relaxed);
        let start = std::time::Instant::now();
        let report = Arc::new(crate::gcode::project_load_report(
            self,
            trace.map(Arc::as_ref),
        ));
        let elapsed = start.elapsed();
        // The log gives the build cost on a real project. A build above
        // one frame (8 ms) is the signal to move it off the frame loop.
        if elapsed > std::time::Duration::from_millis(8) {
            tracing::debug!(
                elapsed_ms = elapsed.as_secs_f64() * 1000.0,
                rows = report.per_toolpath.len(),
                "slow tool-load report build"
            );
        }
        *guard = ReportKey::capture(self, trace).map(|key| Entry {
            key,
            report: Arc::clone(&report),
        });
        report
    }

    /// How many times [`Self::tool_load_report_for`] called the builder on this
    /// session. A test reads it to prove that an unchanged call does no
    /// analysis work and that a relevant change does.
    pub fn tool_load_report_builds(&self) -> u64 {
        self.load_report_memo.builds.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )]

    use std::collections::BTreeMap;
    use std::sync::Arc;

    use crate::compute::catalog::OperationConfig;
    use crate::compute::config::{BoundaryConfig, DressupConfig, HeightsConfig};
    use crate::compute::operation_configs::PocketConfig;
    use crate::compute::tool_config::{ToolConfig, ToolId, ToolType};
    use crate::compute::toolpath_stats::ToolpathStats;
    use crate::gcode::CoolantMode;
    use crate::geo::P3;
    use crate::ids::ToolpathId;
    use crate::session::{
        Command, ProjectSession, SetMachineArgs, SetToolpathEnabledArgs, ToolpathComputeResult,
        ToolpathConfig,
    };
    use crate::stock::simulation_cut::{SimulationCutTrace, SimulationProvenance};
    use crate::toolpath::Toolpath;
    use crate::trace::debug_trace::ToolpathDebugOptions;
    use crate::trace::toolpath_spans::AnnotatedToolpath;

    fn pocket_config(tool_id: usize) -> ToolpathConfig {
        ToolpathConfig {
            id: ToolpathId(0),
            name: "memo".to_owned(),
            enabled: true,
            operation: OperationConfig::Pocket(PocketConfig::default()),
            dressups: DressupConfig::default(),
            heights: HeightsConfig::default(),
            tool_id,
            model_id: 0,
            pre_gcode: None,
            post_gcode: None,
            boundary: BoundaryConfig::default(),
            boundary_inherit: true,
            stock_source: crate::session::StockSource::Fresh,
            coolant: CoolantMode::Off,
            face_selection: None,
            debug_options: ToolpathDebugOptions::default(),
            feeds_provenance: crate::feeds::FeedsProvenance::default(),
            rest_analysis: crate::compute::config::RestAnalysisConfig::default(),
            planner_origin: None,
        }
    }

    /// A zig-zag with plunges, so the kinematic reading has real work.
    fn zigzag(feed: f64) -> Toolpath {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 5.0));
        tp.feed_to(P3::new(0.0, 0.0, -1.0), feed * 0.3);
        for row in 0..40 {
            let y = f64::from(row) * 2.0;
            let x = if row % 2 == 0 { 50.0 } else { 0.0 };
            tp.feed_to(P3::new(x, y, -1.0), feed);
            tp.feed_to(P3::new(x, y + 2.0, -1.0), feed);
        }
        tp.rapid_to(P3::new(0.0, 0.0, 5.0));
        tp
    }

    fn result_of(toolpath: Toolpath) -> ToolpathComputeResult {
        ToolpathComputeResult {
            op_data: crate::ops::drill_op::OpData::Toolpath(Arc::new(AnnotatedToolpath::new(
                toolpath,
            ))),
            stats: ToolpathStats::default(),
            debug_trace: None,
            semantic_trace: None,
        }
    }

    /// A trace whose provenance names the stored toolpath, so the report
    /// reads the row as fresh.
    fn fresh_trace(session: &ProjectSession) -> SimulationCutTrace {
        let tc = &session.toolpath_configs()[0];
        let toolpath = session.get_result(0).unwrap().toolpath();
        SimulationCutTrace {
            provenance: Some(SimulationProvenance {
                trace_schema_version: 0,
                captured_arc_engagement: false,
                toolpath_hashes: BTreeMap::from([(
                    tc.id,
                    crate::compute::simulate::hash_toolpath(toolpath),
                )]),
                tool_hashes: BTreeMap::new(),
                operation_config_hashes: BTreeMap::new(),
                stock_hash: 0,
                machine_hash: 0,
            }),
            ..SimulationCutTrace::test_fixture()
        }
    }

    fn session() -> ProjectSession {
        let mut s = ProjectSession::new_empty();
        let _ = s.add_tool(ToolConfig::new_default(ToolId(0), ToolType::EndMill));
        let tool_id = s.tools()[0].id.0;
        let _ = s.add_toolpath(0, pocket_config(tool_id)).unwrap();
        s.insert_result(0, result_of(zigzag(1500.0))).unwrap();
        s
    }

    /// The memo answer and the raw builder answer, as text. Debug covers
    /// every field, the kinematic move rows included.
    fn assert_same_as_fresh(s: &ProjectSession, trace: Option<&Arc<SimulationCutTrace>>) {
        let memo = s.tool_load_report_for(trace);
        let fresh = crate::gcode::project_load_report(s, trace.map(Arc::as_ref));
        assert_eq!(format!("{:?}", *memo), format!("{fresh:?}"));
    }

    #[test]
    fn an_unchanged_call_does_no_work_and_a_change_rebuilds() {
        let mut s = session();
        let first = s.tool_load_report_for(None);
        assert_eq!(s.tool_load_report_builds(), 1);
        assert_eq!(first.per_toolpath.len(), 1, "the fixture has one row");
        assert!(
            first.per_toolpath[0].kinematic_utilization.is_some(),
            "the row carries the kinematic reading the memo exists to keep"
        );
        let second = s.tool_load_report_for(None);
        assert_eq!(s.tool_load_report_builds(), 1, "an unchanged call rebuilt");
        assert!(Arc::ptr_eq(&first, &second));

        // A new result with no simulation and no edit. A GUI edit counter
        // does not move on an adopt; this key must.
        s.insert_result(0, result_of(zigzag(900.0))).unwrap();
        let _ = s.tool_load_report_for(None);
        assert_eq!(s.tool_load_report_builds(), 2, "a new result was missed");
        assert_same_as_fresh(&s, None);
        assert_eq!(s.tool_load_report_builds(), 2);

        // Disable, then enable: rows leave and return.
        let disable = SetToolpathEnabledArgs {
            index: 0,
            enabled: false,
        };
        let _ = s.apply(Command::SetToolpathEnabled(disable)).unwrap();
        assert!(s.tool_load_report_for(None).per_toolpath.is_empty());
        assert_eq!(s.tool_load_report_builds(), 3);
        let enable = SetToolpathEnabledArgs {
            index: 0,
            enabled: true,
        };
        let _ = s.apply(Command::SetToolpathEnabled(enable)).unwrap();
        let _ = s.tool_load_report_for(None);
        assert_eq!(s.tool_load_report_builds(), 4);

        // Machine kinematics.
        let mut machine = s.machine().clone();
        machine.max_feed_mm_min *= 0.5;
        let _ = s
            .apply(Command::SetMachine(SetMachineArgs {
                machine: Box::new(machine),
            }))
            .unwrap();
        let _ = s.tool_load_report_for(None);
        assert_eq!(s.tool_load_report_builds(), 5, "a machine edit was missed");
        assert_same_as_fresh(&s, None);

        // The tool library and the stock material, written directly so the
        // test does not lean on an invalidation path.
        s.tools[0].diameter *= 2.0;
        let _ = s.tool_load_report_for(None);
        assert_eq!(s.tool_load_report_builds(), 6, "a tool edit was missed");
        s.stock.material = crate::material::Material::SolidWood {
            species: crate::material::WoodSpecies::WhiteOak,
        };
        let _ = s.tool_load_report_for(None);
        assert_eq!(s.tool_load_report_builds(), 7, "a material edit was missed");
        let after_material = s.tool_load_report_builds();

        // A parameter edit (feed rate) on the operation.
        if let OperationConfig::Pocket(cfg) = &mut s.toolpath_configs[0].operation {
            cfg.feed_rate += 100.0;
        }
        let _ = s.tool_load_report_for(None);
        assert_eq!(
            s.tool_load_report_builds(),
            after_material + 1,
            "an operation edit was missed"
        );
    }

    #[test]
    fn the_trace_is_keyed_by_identity_and_an_in_place_rewrite_reads_as_new() {
        let s = session();
        let trace = Arc::new(fresh_trace(&s));
        let _ = s.tool_load_report_for(Some(&trace));
        let _ = s.tool_load_report_for(Some(&trace));
        assert_eq!(s.tool_load_report_builds(), 1);
        assert_same_as_fresh(&s, Some(&trace));
        assert_eq!(s.tool_load_report_builds(), 1);

        // The same content in a new `Arc` is a new trace.
        let copy = Arc::new((*trace).clone());
        let _ = s.tool_load_report_for(Some(&copy));
        assert_eq!(s.tool_load_report_builds(), 2);

        // `Arc::make_mut` on the only strong `Arc`, as the GUI's feed
        // modulation pass does. The memo holds a `Weak`, so the rewrite
        // moves the trace and the key misses.
        let mut unique = copy;
        drop(trace);
        Arc::make_mut(&mut unique).sample_step_mm += 1.0;
        let _ = s.tool_load_report_for(Some(&unique));
        assert_eq!(
            s.tool_load_report_builds(),
            3,
            "an in-place rewrite was missed"
        );

        // No trace after a trace.
        let _ = s.tool_load_report_for(None);
        assert_eq!(s.tool_load_report_builds(), 4);
        assert_same_as_fresh(&s, None);
    }

    #[test]
    fn a_clone_starts_with_an_empty_memo() {
        let s = session();
        let _ = s.tool_load_report_for(None);
        let copy = s.clone();
        assert_eq!(copy.tool_load_report_builds(), 0);
        let _ = copy.tool_load_report_for(None);
        assert_eq!(copy.tool_load_report_builds(), 1);
        assert_eq!(s.tool_load_report_builds(), 1);
    }
}
