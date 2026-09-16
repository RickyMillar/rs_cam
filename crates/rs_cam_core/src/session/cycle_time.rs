//! Cycle time, and the basis it was measured on.
//!
//! WP9 moves this decision from `rs_cam_viz::ui::readiness` into core
//! (G-TIMEEST, 2026-08-22). Five viz surfaces once each carried their own
//! `cutting_distance / feed`, and on a real job they agreed with each other
//! at 25 min while the machine took roughly 3 h. [`toolpath_cycle_time`] is
//! the one decision; a caller that wants a different *population* sums it
//! differently, it does not compute time differently.
//!
//! Two viz-only pieces stay in `rs_cam_viz::ui::readiness`, as a viz-side
//! extension of [`CycleTimeBasis`]:
//!
//! - `remedy()` names a GUI affordance ("Machine properties ▸ Kinematics").
//! - `status()` returns `CheckStatus`, a viz-only tier type.
//!
//! Core cannot depend on either, so viz adds them through an extension
//! trait rather than an inherent method (Rust also forbids an inherent
//! `impl` on a type from another crate). `format_cycle_time` also stays in
//! viz — it renders a `CycleTime`, it does not measure one.

use crate::ids::ToolpathId;
use crate::stock::simulation_cut::SimulationCutTrace;

/// What a cycle-time number measures.
///
/// A machine at 500 mm/s² needs `v²/2a` of runway to reach speed. On a
/// 0.40 mm segment commanded at F3000 (50 mm/s), that runway is 2.5 mm —
/// six times the segment length — so the machine spends the whole segment
/// accelerating and never reaches the commanded feed. `distance / feed` is
/// therefore not an approximation of cycle time on a corner-heavy path; it
/// is a different quantity, and this type names which one a number is.
///
/// Variants are ordered best-modelled first; [`CycleTimeBasis::worse`]
/// folds a mixed project down to its weakest contributor.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CycleTimeBasis {
    /// The trapezoidal integrator's answer (`machine_kinematics::
    /// compute_cycle_time`): accel/decel ramps, junction-deviation
    /// cornering, rapids, and the emitted per-move feed. The only basis
    /// that is a wall-clock prediction.
    MachineModel,
    /// The simulator's dexel-sample sum: every move, rapids included, at
    /// its emitted feed. Real distance and real per-move feeds, but no
    /// acceleration model, because the active machine profile carries no
    /// kinematics. Reads optimistic by a factor that grows as segments
    /// shorten.
    SimulatedNoAccel,
    /// No simulation covers this toolpath: cutting distance divided by the
    /// operation's nominal feed. No rapids, no acceleration, no feed
    /// modulation — the weakest basis, kept so a never-simulated project
    /// still shows a number, and labelled as what it is.
    CuttingOnly,
}

impl CycleTimeBasis {
    /// Keep the weaker of two bases.
    ///
    /// A project total is only as trustworthy as its least-modelled
    /// contributor, so one un-simulated op drags the whole figure down to
    /// `CuttingOnly` and says so.
    pub fn worse(self, other: Self) -> Self {
        match (self, other) {
            (CycleTimeBasis::CuttingOnly, _) | (_, CycleTimeBasis::CuttingOnly) => {
                CycleTimeBasis::CuttingOnly
            }
            (CycleTimeBasis::SimulatedNoAccel, _) | (_, CycleTimeBasis::SimulatedNoAccel) => {
                CycleTimeBasis::SimulatedNoAccel
            }
            _ => CycleTimeBasis::MachineModel,
        }
    }

    /// Short parenthetical for a row title — the signal that the number
    /// changed meaning.
    pub fn qualifier(self) -> &'static str {
        match self {
            CycleTimeBasis::MachineModel => "wall clock",
            CycleTimeBasis::SimulatedNoAccel => "no accel",
            CycleTimeBasis::CuttingOnly => "cutting only, no accel",
        }
    }

    /// One sentence naming what the figure omits, and which way it errs.
    pub fn caveat(self) -> &'static str {
        match self {
            CycleTimeBasis::MachineModel => {
                "Machine-model wall clock: accel/decel ramps, cornering, rapids and emitted feeds."
            }
            CycleTimeBasis::SimulatedNoAccel => {
                "Simulated path ÷ emitted feed, rapids included — but this machine profile has no \
                 acceleration limits, so spool-up and cornering are not modelled. The real cut \
                 will take LONGER."
            }
            CycleTimeBasis::CuttingOnly => {
                "Cutting distance ÷ nominal feed. Excludes rapids, acceleration and per-move feed \
                 changes. On a corner-heavy 3D finish this reads SEVERAL TIMES faster than the \
                 machine — a measured job read 25 min against 3 h."
            }
        }
    }
}

/// A cycle time and the basis it was measured on, always travelling
/// together.
///
/// `basis: None` means **no estimate exists**, not "zero seconds" — render
/// it as a dash.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct CycleTime {
    /// The estimate, in seconds. Meaningless when `basis` is `None`.
    pub seconds: f64,
    /// What the estimate measures, or `None` for no estimate.
    pub basis: Option<CycleTimeBasis>,
}

impl CycleTime {
    /// Nothing to estimate.
    pub const NONE: Self = Self {
        seconds: 0.0,
        basis: None,
    };

    /// An estimate of `seconds` on `basis`.
    pub fn of(seconds: f64, basis: CycleTimeBasis) -> Self {
        Self {
            seconds,
            basis: Some(basis),
        }
    }

    /// Accumulate another op's time, degrading the basis to the weaker of
    /// the two.
    ///
    /// Folding `NONE` is a no-op, so an op with no estimate neither adds
    /// time nor claims modelling it does not have.
    pub fn fold(&mut self, other: Self) {
        let Some(other_basis) = other.basis else {
            return;
        };
        self.seconds += other.seconds;
        self.basis = Some(match self.basis {
            Some(mine) => mine.worse(other_basis),
            None => other_basis,
        });
    }
}

/// The cycle-time decision for one toolpath.
///
/// `trace` is the simulation's cut trace the caller measured this toolpath
/// against, if any. Reading `toolpath_runtimes` before `toolpath_summaries`
/// is load-bearing: a drill toolpath sets `metrics_not_applicable` and
/// publishes no `toolpath_summaries` row (it has no engagement metrics), so
/// reading only that list answers "was this toolpath integrated?" wrong for
/// every drill. `toolpath_runtimes` carries that fact on its own, joined by
/// `toolpath_id`, independent of whether an engagement summary exists.
///
/// `pub`, not `pub(crate)`: [`crate::session::ProjectSession::query`] calls
/// it for the `ToolpathCycleTime` row, and an integration test exercises
/// the raw decision directly, independent of the `Query` plumbing.
pub fn toolpath_cycle_time(
    trace: Option<&SimulationCutTrace>,
    id: ToolpathId,
    cutting_distance_mm: f64,
    nominal_feed_mm_min: f64,
) -> CycleTime {
    if let Some(rt) = trace.and_then(|t| t.toolpath_runtimes.iter().find(|r| r.toolpath_id == id)) {
        return CycleTime::of(rt.breakdown.total_s, CycleTimeBasis::MachineModel);
    }
    let summary = trace.and_then(|t| t.toolpath_summaries.iter().find(|s| s.toolpath_id == id));
    if let Some(summary) = summary {
        // Reached when the integrator did not run (no machine kinematics on
        // the profile), so the summary carries the simulator's naive
        // segment timing. `runtime_by_intent` is stamped only by the
        // integrator, in the same pass that fills `toolpath_runtimes`, so
        // in practice this arm is the no-kinematics one.
        let basis = if summary.runtime_by_intent.is_some() {
            CycleTimeBasis::MachineModel
        } else {
            CycleTimeBasis::SimulatedNoAccel
        };
        return CycleTime::of(summary.total_runtime_s, basis);
    }
    // No simulated evidence for this toolpath. A zero-length cut still
    // counts as an estimate, so the project basis records that this op was
    // never simulated rather than silently omitting it.
    if nominal_feed_mm_min > 0.0 {
        return CycleTime::of(
            (cutting_distance_mm / nominal_feed_mm_min) * 60.0,
            CycleTimeBasis::CuttingOnly,
        );
    }
    CycleTime::NONE
}
