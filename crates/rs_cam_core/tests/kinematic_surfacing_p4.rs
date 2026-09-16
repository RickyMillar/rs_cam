//! Phase 4 sentries — the plunge-class backstop and its triage surfacing.
//!
//! What these pin is the RULE, not the kernel: given a
//! [`ToolpathKinematicUtilization`], when does `project.plunge_class_load`
//! speak, at what severity, and does the triage builder carry it to the
//! `actions` list. Every reading here is a hand-built literal, so the
//! sentries stay valid whatever `analyse_toolpath` measures.
//!
//! The two failure modes they exist for:
//!
//! - **A gate handed an empty population passes and looks healthy.** A
//!   utilization with `plunge.population == 0` must produce NO finding, even
//!   when every other field looks alarming. An absent reading is not a clean
//!   one.
//! - **A finding nothing surfaces is not a finding.** The rule firing in
//!   isolation proves nothing until `SimulationTriage::build` puts it in
//!   `actions`.
//!
//! NOTE: this file depends on the sibling `kinematic_utilization` module
//! (Phase 2 kernel) for its types. It calls no kernel function — no
//! `analyse_toolpath`, no measurement — so it exercises only the surfacing
//! layer.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::ToolpathId;
use rs_cam_core::diagnostics::{Severity, ids};
use rs_cam_core::machine::kinematic_utilization::{
    BindingFractions, PlungeClassObservation, RampObservation, ToolpathKinematicUtilization,
};
use rs_cam_core::machine::kinematics::MachineKinematics;
use rs_cam_core::stock::sim_measurability::MeasurabilityReport;
use rs_cam_core::stock::sim_triage::{SimulationTriage, TriageInputs, plunge_class_finding};
use rs_cam_core::stock::simulation_cut::SimulationCutTrace;
use std::collections::BTreeMap;

/// One hand-built reading. `peak_ratio` and `population` are the two dials
/// the sentries turn; every other field is held at a plausible, HEALTHY
/// value, so a finding can only come from the dials.
fn utilization(population: usize, peak_ratio: Option<f64>) -> ToolpathKinematicUtilization {
    ToolpathKinematicUtilization {
        toolpath_id: ToolpathId(7),
        moves_total: 1200,
        fed_moves: 900,
        fed_time_s: 240.0,
        time_weighted_commanded_mm_min: Some(2400.0),
        time_weighted_achieved_mm_min: Some(1872.0),
        utilization: Some(0.78),
        bindings: Some(BindingFractions {
            feed_bound: 0.52,
            accel_bound: 0.31,
            rate_bound: 0.12,
            junction_bound: 0.05,
            machine_bound: 0.48,
        }),
        rate_bound_axis_time_fraction: [0.01, 0.01, 0.10],
        plunge: PlungeClassObservation {
            population,
            peak_ratio,
            over_1x: 14,
            over_2x: 3,
            worst_move_index: Some(431),
            worst_position: Some([112.5, 64.25, -8.125]),
            worst_achieved_z_rate_mm_min: Some(1807.0),
            plunge_rate_mm_min: 512.0,
        },
        ramp: RampObservation {
            population: 60,
            steepest_deg_from_horizontal: Some(11.5),
            worst_move_index: Some(88),
        },
        // The surfacing layer reads no move, so the list stays empty. The
        // machine model rides along because `headroom_estimate` re-solves
        // every move against it; with no moves there is nothing to re-solve.
        moves: Vec::new(),
        kinematics: MachineKinematics::generic_wood_router(),
        max_feed_mm_min: 10_000.0,
        // The PRECOMPUTED headroom every display surface reads. It is a
        // stored reading, not a derivation of `moves`, which is exactly why
        // this literal can carry one with an empty move list — and why a
        // value that arrived over the wire still has a headroom to show.
        headroom_at_1_30: Some(0.12),
        // Phase 3 (2026-09-07): the surfacing layer reads this only to
        // qualify its own wording; the finding does not branch on it, so
        // the healthy reading is the emitted one.
        feeds_provenance: rs_cam_core::machine::kinematic_utilization::FeedsProvenance::Emitted,
    }
}

/// (a) An EMPTY plunge-class population produces no finding, however bad
/// every other field looks.
///
/// This is the "a gate handed an empty population passes and looks healthy"
/// defect, inverted: the reading must be ABSENT, not clean. A ratio of 9.0
/// over zero moves is not a 9x exceedance; it is nothing at all.
#[test]
fn an_empty_plunge_population_produces_no_finding() {
    let util = utilization(0, Some(9.0));
    assert!(
        plunge_class_finding(&util).is_none(),
        "population 0 must produce no finding, whatever peak_ratio says"
    );
}

/// (b) The severity bands: `Caution` above 1x, `Critical` above 2x, silence
/// at or below 1x.
#[test]
fn severity_bands_follow_the_ratio() {
    let caution = plunge_class_finding(&utilization(40, Some(1.5)))
        .expect("ratio 1.5 over a real population must fire");
    assert_eq!(caution.diagnostic.severity, Severity::Caution);

    let critical = plunge_class_finding(&utilization(40, Some(2.5)))
        .expect("ratio 2.5 over a real population must fire");
    assert_eq!(critical.diagnostic.severity, Severity::Critical);

    assert!(
        plunge_class_finding(&utilization(40, Some(0.9))).is_none(),
        "a descent inside its own plunge rate is not a finding"
    );
    assert!(
        plunge_class_finding(&utilization(40, None)).is_none(),
        "an unmeasured peak ratio must not be read as zero, or as an exceedance"
    );
}

/// (c) The finding is NON-BLOCKING and wears the Phase 4 id.
///
/// `fix: None` is load-bearing: nothing in export reads this rule, and a
/// populated `fix` is what an escalation to a blocking gate would look like.
/// That escalation is its own later decision.
#[test]
fn the_finding_is_non_blocking_and_carries_its_own_id() {
    let f = plunge_class_finding(&utilization(40, Some(3.53)))
        .expect("a 3.53x peak over a real population must fire");
    assert_eq!(
        f.diagnostic.id.as_str(),
        ids::PROJECT_PLUNGE_CLASS_LOAD,
        "the backstop must not borrow the static plunge_stress id"
    );
    assert!(
        f.diagnostic.fix.is_none(),
        "the plunge-class backstop is non-blocking; a fix would make it a gate"
    );
    assert!(
        f.diagnostic.supersedes.is_empty(),
        "the backstop supersedes nothing — it is complementary to plunge_stress"
    );
}

/// (d) The triage builder carries the finding to `actions`, and an empty map
/// surfaces nothing.
///
/// Built through `SimulationTriage::build` — the no-rest-context form — on
/// purpose: the plunge-class rule is NOT gated on `rest_driven`, so it must
/// appear even when the caller knows of no rest-driven toolpath.
#[test]
fn the_triage_surfaces_the_finding_and_an_empty_map_surfaces_nothing() {
    let trace = SimulationCutTrace::from_samples(0.5, Vec::new());
    let measurability = MeasurabilityReport::default();
    let diameters: BTreeMap<ToolpathId, f64> = BTreeMap::new();

    let mut populated: BTreeMap<ToolpathId, ToolpathKinematicUtilization> = BTreeMap::new();
    let util = utilization(40, Some(3.53));
    populated.insert(util.toolpath_id, util);

    let with_reading = SimulationTriage::build(&TriageInputs {
        trace: &trace,
        measurability: &measurability,
        diagnostics: &[],
        rapid_collisions: &[],
        holder_collisions: &[],
        tool_diameters_mm: &diameters,
        kinematic_utilization: &populated,
        region_of: None,
    });
    assert!(
        with_reading
            .actions
            .iter()
            .any(|f| f.diagnostic.id.as_str() == ids::PROJECT_PLUNGE_CLASS_LOAD),
        "a measured over-1x reading must reach the actions list"
    );

    let empty: BTreeMap<ToolpathId, ToolpathKinematicUtilization> = BTreeMap::new();
    let without_reading = SimulationTriage::build(&TriageInputs {
        trace: &trace,
        measurability: &measurability,
        diagnostics: &[],
        rapid_collisions: &[],
        holder_collisions: &[],
        tool_diameters_mm: &diameters,
        kinematic_utilization: &empty,
        region_of: None,
    });
    assert!(
        without_reading
            .actions
            .iter()
            .all(|f| f.diagnostic.id.as_str() != ids::PROJECT_PLUNGE_CLASS_LOAD),
        "a caller that measured nothing must produce no plunge-class finding"
    );
}
