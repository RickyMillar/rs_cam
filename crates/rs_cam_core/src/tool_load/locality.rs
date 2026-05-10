//! Sample locality classifier — short operator-facing labels for the
//! per-gate triggering sample's local cut conditions.
//!
//! G17 A3 of `planning/OPTIMIZE_EXPLAINABILITY_AND_PEAK_FINDING.md`:
//! the per-gate evaluators (chipload / power / deflection) record a
//! triggering sample index in `SampleEvidence.sample_range`. A bare
//! sample index is opaque to the operator. This module classifies the
//! sample by its kinematics + arc engagement so the modal can render
//! "chipload 0.0707 (+29%) — slot section" instead of the bare
//! reading.
//!
//! Lightweight by design: the classifier only inspects fields already
//! on `SimulationCutSample` (no `AnnotatedToolpath` plumbing). Trade-off
//! is precision — we say "slot section" but not "near corner X". A
//! future pass could enrich with span-path lookup if value warrants
//! the threading cost.

use std::f64::consts::{FRAC_PI_2, PI};

use crate::simulation_cut::{CutKinematics, SimulationCutSample};

/// Classify a sample by kinematics + arc engagement. Returns an
/// operator-facing label or `None` for plain steady-state cuts.
///
/// Order of checks matters: kinematics overrides engagement, since a
/// plunge by definition is a vertical move with full envelope contact
/// and the operator-meaningful word for that cut is "plunge", not
/// "slot section".
pub fn classify_sample_locality(sample: &SimulationCutSample) -> Option<String> {
    match sample.cut_kinematics {
        CutKinematics::Plunge => return Some("plunge entry".to_owned()),
        CutKinematics::Helix => return Some("helix entry".to_owned()),
        CutKinematics::Linear | CutKinematics::Arc | CutKinematics::Rapid => {}
    }
    let arc = sample.arc_engagement_radians?;
    if arc >= PI {
        Some("slot section".to_owned())
    } else if arc >= FRAC_PI_2 {
        Some("heavy engagement".to_owned())
    } else {
        None
    }
}

/// G17 C1 — should this sample participate in the gate-trip decision?
///
/// Returns `false` for transient entry kinematics (Helix / Plunge).
/// Returns `true` for everything else (Linear / Arc / Rapid). The
/// per-gate evaluators use this to exclude entry-sample spikes from
/// driving the trip decision — entry transients are surfaced as
/// informational advisories via [`crate::tool_load::verdict::EntrySpike`]
/// (G17 C2) instead of flipping the verdict to `Exceeds`.
///
/// **Why exclude entries:** the wanaka MCP smoke (2026-05-10) showed
/// every gate violation across both TPs sat in a helical entry move,
/// not steady-state cutting. Treating entry spikes as bulk-cut
/// failures led the optimizer to reject candidates whose bulk cut
/// was actually fine. The advisory carries the entry reading so
/// genuinely-bad entries are still visible — just not blocking.
///
/// **What this does NOT exclude:** air-cut samples and rapids are
/// already filtered upstream in `steady_state_samples_for_toolpath`.
/// This predicate runs *after* that filter; it only narrows the
/// surviving "in-cut, at op-feed" set down to cut-kinematics that
/// represent steady-state material removal.
pub fn is_steady_state_for_gate(sample: &SimulationCutSample) -> bool {
    !matches!(
        sample.cut_kinematics,
        CutKinematics::Helix | CutKinematics::Plunge
    )
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;

    fn sample(kinematics: CutKinematics, arc: Option<f64>) -> SimulationCutSample {
        SimulationCutSample {
            toolpath_id: 0,
            move_index: 0,
            sample_index: 0,
            position: [0.0, 0.0, 0.0],
            cumulative_time_s: 0.0,
            segment_time_s: 0.0,
            is_cutting: true,
            cut_kinematics: kinematics,
            feed_rate_mm_min: 1000.0,
            spindle_rpm: 18000,
            flute_count: 2,
            axial_doc_mm: 1.0,
            radial_engagement: 0.5,
            arc_engagement_radians: arc,
            chipload_mm_per_tooth: 0.03,
            effective_chip_thickness_mm: None,
            removed_volume_est_mm3: 0.0,
            mrr_mm3_s: 0.0,
            semantic_item_id: None,
            span_path: Vec::new(),
        }
    }

    #[test]
    fn plunge_kinematic_dominates_arc_engagement() {
        // Even with full slot arc engagement, a plunge sample reads as
        // "plunge entry" — that's the operator-meaningful descriptor.
        let s = sample(CutKinematics::Plunge, Some(PI));
        assert_eq!(classify_sample_locality(&s).as_deref(), Some("plunge entry"));
    }

    #[test]
    fn helix_kinematic_classifies_as_helix_entry() {
        let s = sample(CutKinematics::Helix, Some(FRAC_PI_2));
        assert_eq!(classify_sample_locality(&s).as_deref(), Some("helix entry"));
    }

    #[test]
    fn linear_with_full_slot_arc_classifies_as_slot_section() {
        // Wanaka TP 1 case: peak chipload sample sits in a full-slot
        // engagement region.
        let s = sample(CutKinematics::Linear, Some(PI));
        assert_eq!(
            classify_sample_locality(&s).as_deref(),
            Some("slot section")
        );
    }

    #[test]
    fn linear_with_heavy_arc_classifies_as_heavy_engagement() {
        let s = sample(CutKinematics::Linear, Some(2.5)); // < π but > π/2
        assert_eq!(
            classify_sample_locality(&s).as_deref(),
            Some("heavy engagement")
        );
    }

    #[test]
    fn linear_with_low_arc_returns_none() {
        let s = sample(CutKinematics::Linear, Some(0.5));
        assert!(classify_sample_locality(&s).is_none());
    }

    #[test]
    fn missing_arc_engagement_returns_none() {
        let s = sample(CutKinematics::Linear, None);
        assert!(classify_sample_locality(&s).is_none());
    }
}
