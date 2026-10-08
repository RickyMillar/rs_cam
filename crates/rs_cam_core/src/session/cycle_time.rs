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
//!
//! # Why a weak estimate is weak (U3, 2026-10-01)
//!
//! A basis says WHAT a number measures. It does not say WHY the number is
//! not a better one. On a real project the GUI showed "cutting only, no
//! accel" and told the operator to "Run a simulation" directly after a
//! simulation ran. The two real causes were a run with no cut trace (the
//! GUI capture checkbox, deleted 2026-10-02, was off) and a machine profile
//! with no kinematics.
//! [`CycleTimeBasis::worse`] folds both into one basis, so the basis alone
//! cannot name them. [`MissingInputs`] records each cause beside the
//! number, and [`CycleTime::with_evidence`] fills it from
//! [`CycleTimeEvidence`]. The per-toolpath decision
//! [`toolpath_cycle_time`] does not fill it, because it sees only the trace
//! and cannot tell "never simulated" from "simulated with no trace".

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

/// One input that a cycle-time estimate did not have, and that would make
/// it a better estimate.
///
/// Each variant is one cause with one remedy. The variants are in display
/// order: the causes that need a simulation run come first, and the cause
/// that needs a machine-profile edit comes last.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MissingInput {
    /// No simulation result exists for the project.
    NoSimulation,
    /// A simulation result with a cut trace exists, but the trace does not
    /// include this toolpath. The toolpath was added, or enabled, after
    /// the run.
    NotInSimulation,
    /// A simulation result exists, but it holds no cut trace, and the cut
    /// trace is where the simulator writes the time it measures. Every
    /// simulation captures the trace (operator ruling 2026-10-02, "always
    /// capture"), so this marks evidence that a caller built without one.
    NoCutTrace,
    /// The machine profile has kinematics now, but the simulation ran
    /// before they were set, so the run has no machine-model time.
    KinematicsNotApplied,
    /// The machine profile has no kinematics, so no run can integrate
    /// acceleration and cornering.
    NoMachineKinematics,
}

impl MissingInput {
    /// Every variant, in display order.
    pub const ALL: [Self; 5] = [
        MissingInput::NoSimulation,
        MissingInput::NotInSimulation,
        MissingInput::NoCutTrace,
        MissingInput::KinematicsNotApplied,
        MissingInput::NoMachineKinematics,
    ];

    /// A short phrase for the row label beside the time.
    pub fn short_label(self) -> &'static str {
        match self {
            MissingInput::NoSimulation => "not simulated",
            MissingInput::NotInSimulation => "some toolpaths not simulated",
            MissingInput::NoCutTrace => "metrics not captured",
            MissingInput::KinematicsNotApplied => "simulated before kinematics were set",
            MissingInput::NoMachineKinematics => "no machine kinematics",
        }
    }

    fn bit(self) -> u8 {
        match self {
            MissingInput::NoSimulation => 1,
            MissingInput::NotInSimulation => 1 << 1,
            MissingInput::NoCutTrace => 1 << 2,
            MissingInput::KinematicsNotApplied => 1 << 3,
            MissingInput::NoMachineKinematics => 1 << 4,
        }
    }
}

/// The set of [`MissingInput`] causes behind one estimate.
///
/// An empty set on an estimate that is not `MachineModel` means the caller
/// did not supply evidence ([`CycleTime::with_evidence`]). It does not mean
/// that the estimate is complete.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct MissingInputs {
    bits: u8,
}

impl MissingInputs {
    /// No cause.
    pub const EMPTY: Self = Self { bits: 0 };

    /// Add one cause.
    pub fn insert(&mut self, input: MissingInput) {
        self.bits |= input.bit();
    }

    /// True when the set holds `input`.
    pub fn contains(self, input: MissingInput) -> bool {
        self.bits & input.bit() != 0
    }

    /// True when the set holds no cause.
    pub fn is_empty(self) -> bool {
        self.bits == 0
    }

    /// The causes of both sets.
    pub fn union(self, other: Self) -> Self {
        Self {
            bits: self.bits | other.bits,
        }
    }

    /// The causes in the set, in display order.
    pub fn iter(self) -> impl Iterator<Item = MissingInput> {
        MissingInput::ALL
            .into_iter()
            .filter(move |input| self.contains(*input))
    }
}

/// The project facts that [`CycleTime::with_evidence`] reads to name the
/// causes of a weak estimate.
///
/// The per-toolpath decision sees only the cut trace. These three facts
/// live on the session and on the machine profile, so the caller supplies
/// them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CycleTimeEvidence {
    /// A simulation result exists, with or without a cut trace.
    pub simulation_present: bool,
    /// The simulation result carries a cut trace.
    pub cut_trace_present: bool,
    /// The active machine profile has kinematics.
    pub machine_kinematics_present: bool,
}

impl CycleTimeEvidence {
    /// The causes that make an estimate on `basis` weaker than
    /// `MachineModel`.
    ///
    /// A cut trace comes only from a simulation, so `cut_trace_present`
    /// alone also counts as a simulation.
    pub fn missing_for(self, basis: CycleTimeBasis) -> MissingInputs {
        let simulated = self.simulation_present || self.cut_trace_present;
        let mut missing = MissingInputs::EMPTY;
        match basis {
            CycleTimeBasis::MachineModel => return missing,
            // The run kept a trace but no integrator time. With no
            // kinematics that is expected. With kinematics, the run is
            // older than the kinematics.
            CycleTimeBasis::SimulatedNoAccel => {
                if self.machine_kinematics_present {
                    missing.insert(MissingInput::KinematicsNotApplied);
                }
            }
            CycleTimeBasis::CuttingOnly => {
                missing.insert(if !simulated {
                    MissingInput::NoSimulation
                } else if !self.cut_trace_present {
                    MissingInput::NoCutTrace
                } else {
                    MissingInput::NotInSimulation
                });
            }
        }
        if !self.machine_kinematics_present {
            missing.insert(MissingInput::NoMachineKinematics);
        }
        missing
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
    /// Why the estimate is weaker than `MachineModel`: one cause for each
    /// missing input. It stays empty until [`Self::with_evidence`] fills it.
    pub missing: MissingInputs,
}

impl CycleTime {
    /// Nothing to estimate.
    pub const NONE: Self = Self {
        seconds: 0.0,
        basis: None,
        missing: MissingInputs::EMPTY,
    };

    /// An estimate of `seconds` on `basis`, with no cause recorded.
    pub fn of(seconds: f64, basis: CycleTimeBasis) -> Self {
        Self {
            seconds,
            basis: Some(basis),
            missing: MissingInputs::EMPTY,
        }
    }

    /// Record the causes that `evidence` gives for this estimate's basis.
    ///
    /// Apply it to each toolpath's estimate before the fold, so that the
    /// project total keeps the causes of every contributor. `NONE` stays
    /// `NONE`.
    #[must_use]
    pub fn with_evidence(mut self, evidence: CycleTimeEvidence) -> Self {
        if let Some(basis) = self.basis {
            self.missing = self.missing.union(evidence.missing_for(basis));
        }
        self
    }

    /// Accumulate another op's time, degrading the basis to the weaker of
    /// the two and keeping the causes of both.
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
        self.missing = self.missing.union(other.missing);
    }

    /// The short label beside the time: the basis, then each cause.
    ///
    /// Examples: `"wall clock"`, `"no accel: no machine kinematics"`,
    /// `"cutting only: metrics not captured, no machine kinematics"`. With
    /// no cause recorded, the label is the basis qualifier alone. With no
    /// estimate, the label is `"no estimate"`.
    pub fn label(&self) -> String {
        let Some(basis) = self.basis else {
            return "no estimate".to_owned();
        };
        if self.missing.is_empty() {
            return basis.qualifier().to_owned();
        }
        let head = match basis {
            CycleTimeBasis::MachineModel => "wall clock",
            CycleTimeBasis::SimulatedNoAccel => "no accel",
            CycleTimeBasis::CuttingOnly => "cutting only",
        };
        let causes: Vec<&str> = self.missing.iter().map(MissingInput::short_label).collect();
        format!("{head}: {}", causes.join(", "))
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

/// G5: how many times the program starts the spindle: once at the start,
/// then at each tool change, each change of S, and each setup pause.
///
/// The count reads the enabled toolpaths in setup order (index order when
/// the project has one setup), the order the export writes them.
pub fn spindle_starts(session: &super::ProjectSession) -> usize {
    let spindle_default = session.post_config().spindle_speed;
    let mut starts = 0usize;
    for setup in session.list_setups() {
        // A setup pause stops the spindle (M5), so the first op of each
        // setup starts it again.
        let mut last: Option<(usize, u32)> = None;
        for idx in &setup.toolpath_indices {
            let Some(tc) = session.toolpath_configs().get(*idx) else {
                continue;
            };
            if !tc.enabled {
                continue;
            }
            let rpm =
                crate::compute::catalog::effective_spindle_rpm(&tc.operation, spindle_default);
            if last != Some((tc.tool_id, rpm)) {
                starts += 1;
                last = Some((tc.tool_id, rpm));
            }
        }
    }
    starts
}

/// G5: the seconds the controller spends waiting for the spindle over the
/// whole program: the profile's spindle on delay (`$394`) for each spindle
/// start. 0 when the profile has no controller settings.
///
/// grblHAL waits `$394` on every M3 that changes the spindle state
/// (spindle_control.c:828-838, 787-790).
pub fn controller_spindle_wait_s(session: &super::ProjectSession) -> f64 {
    let per_start = session
        .machine()
        .controller
        .as_ref()
        .map_or(0.0, crate::machine::ControllerSettings::spindle_wait_s);
    if per_start <= 0.0 {
        return 0.0;
    }
    per_start * spindle_starts(session) as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    const TP: ToolpathId = ToolpathId(7);

    fn evidence(sim: bool, trace: bool, kin: bool) -> CycleTimeEvidence {
        CycleTimeEvidence {
            simulation_present: sim,
            cut_trace_present: trace,
            machine_kinematics_present: kin,
        }
    }

    fn causes(missing: MissingInputs) -> Vec<MissingInput> {
        missing.iter().collect()
    }

    /// The operator's case (U3): a simulation result with no cut trace, and
    /// the profile has no kinematics. Both causes are named, and
    /// "not simulated" is not one of them.
    #[test]
    fn a_trace_less_run_names_the_trace_and_the_kinematics() {
        let ct = toolpath_cycle_time(None, TP, 60_000.0, 1200.0)
            .with_evidence(evidence(true, false, false));
        assert_eq!(ct.basis, Some(CycleTimeBasis::CuttingOnly));
        assert_eq!(
            causes(ct.missing),
            vec![MissingInput::NoCutTrace, MissingInput::NoMachineKinematics]
        );
        assert!(!ct.missing.contains(MissingInput::NoSimulation));
    }

    #[test]
    fn every_cause_combination_for_a_cutting_only_estimate() {
        use MissingInput::{NoCutTrace, NoMachineKinematics, NoSimulation, NotInSimulation};
        let cases = [
            // (simulation, trace, kinematics) -> causes
            (
                (false, false, false),
                vec![NoSimulation, NoMachineKinematics],
            ),
            ((false, false, true), vec![NoSimulation]),
            ((true, false, false), vec![NoCutTrace, NoMachineKinematics]),
            ((true, false, true), vec![NoCutTrace]),
            (
                (true, true, false),
                vec![NotInSimulation, NoMachineKinematics],
            ),
            ((true, true, true), vec![NotInSimulation]),
            // A trace implies a simulation, even when the session slot is
            // empty.
            ((false, true, true), vec![NotInSimulation]),
        ];
        for ((sim, trace, kin), want) in cases {
            let got = evidence(sim, trace, kin).missing_for(CycleTimeBasis::CuttingOnly);
            assert_eq!(causes(got), want, "sim={sim} trace={trace} kin={kin}");
        }
    }

    #[test]
    fn a_simulated_no_accel_estimate_names_the_kinematics_cause() {
        assert_eq!(
            causes(evidence(true, true, false).missing_for(CycleTimeBasis::SimulatedNoAccel)),
            vec![MissingInput::NoMachineKinematics]
        );
        // Kinematics exist but the run did not use them: the run is older.
        assert_eq!(
            causes(evidence(true, true, true).missing_for(CycleTimeBasis::SimulatedNoAccel)),
            vec![MissingInput::KinematicsNotApplied]
        );
    }

    #[test]
    fn a_machine_model_estimate_has_no_cause() {
        for sim in [false, true] {
            for trace in [false, true] {
                for kin in [false, true] {
                    assert!(
                        evidence(sim, trace, kin)
                            .missing_for(CycleTimeBasis::MachineModel)
                            .is_empty()
                    );
                }
            }
        }
    }

    #[test]
    fn no_estimate_takes_no_cause_and_folds_to_nothing() {
        let none = CycleTime::NONE.with_evidence(evidence(false, false, false));
        assert_eq!(none, CycleTime::NONE);
        let mut total = CycleTime::NONE;
        total.fold(none);
        assert_eq!(total, CycleTime::NONE);
        assert_eq!(total.label(), "no estimate");
    }

    /// The fold keeps the causes of every contributor, not only those of
    /// the weakest basis.
    #[test]
    fn the_fold_keeps_the_union_of_the_causes() {
        let ev = evidence(true, true, false);
        let mut total = CycleTime::NONE;
        total.fold(CycleTime::of(100.0, CycleTimeBasis::SimulatedNoAccel).with_evidence(ev));
        total.fold(CycleTime::of(50.0, CycleTimeBasis::CuttingOnly).with_evidence(ev));
        assert_eq!(total.basis, Some(CycleTimeBasis::CuttingOnly));
        assert!((total.seconds - 150.0).abs() < 1e-9);
        assert_eq!(
            causes(total.missing),
            vec![
                MissingInput::NotInSimulation,
                MissingInput::NoMachineKinematics
            ]
        );
    }

    /// `of` records no cause, so the per-toolpath decision stays equal to
    /// the frozen oracle in `tests/query_cycle_time_one_answer.rs`.
    #[test]
    fn the_per_toolpath_decision_records_no_cause() {
        let ct = toolpath_cycle_time(None, TP, 60_000.0, 1200.0);
        assert_eq!(ct, CycleTime::of(3000.0, CycleTimeBasis::CuttingOnly));
        assert!(ct.missing.is_empty());
    }

    #[test]
    fn the_label_names_the_basis_and_each_cause() {
        let ct = CycleTime::of(29_599.0, CycleTimeBasis::CuttingOnly)
            .with_evidence(evidence(true, false, false));
        assert_eq!(
            ct.label(),
            "cutting only: metrics not captured, no machine kinematics"
        );

        let ct = CycleTime::of(10.0, CycleTimeBasis::CuttingOnly)
            .with_evidence(evidence(false, false, true));
        assert_eq!(ct.label(), "cutting only: not simulated");

        let ct = CycleTime::of(10.0, CycleTimeBasis::SimulatedNoAccel)
            .with_evidence(evidence(true, true, false));
        assert_eq!(ct.label(), "no accel: no machine kinematics");

        let ct = CycleTime::of(10.0, CycleTimeBasis::MachineModel)
            .with_evidence(evidence(true, true, true));
        assert_eq!(ct.label(), "wall clock");

        // No evidence supplied: the basis qualifier alone, as before U3.
        assert_eq!(
            CycleTime::of(10.0, CycleTimeBasis::CuttingOnly).label(),
            CycleTimeBasis::CuttingOnly.qualifier()
        );
    }
}
