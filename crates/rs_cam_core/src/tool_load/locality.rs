//! Sample locality classifier — short operator-facing labels for the
//! per-gate triggering sample's local cut conditions.
//!
//! G17 D6 of `planning/STRUCTURAL_ENTRY_SPANS_AND_LOCALITY.md`: the
//! per-gate evaluators (chipload / power / deflection) record a
//! triggering sample index in `SampleEvidence.sample_range`. A bare
//! sample index is opaque to the operator. This module classifies the
//! sample by its **structural span ancestry** (Entry / DressupArtifact /
//! LinkBridge) and, for steady-state cuts, by arc engagement, so the
//! modal can render "chipload 0.0707 (+29%) — slot section" instead of
//! the bare reading.
//!
//! D6 dropped the kinematics-derived "helix entry" / "plunge entry"
//! labels: D0 research showed `CutKinematics::Helix` is dominated by
//! terrain-following sloped cuts on adaptive3d, not configured entries.
//! The structural Entry spans laid down by D4 / D5 are now the
//! authoritative signal. When no span data is plumbed (legacy traces,
//! synthetic test samples without span_path), the classifier falls back
//! to engagement-only labels.

use std::f64::consts::{FRAC_PI_2, PI};

use crate::simulation_cut::SimulationCutSample;
use crate::toolpath_spans::{Span, SpanId, SpanKind};

/// Resolves [`SpanId`]s recorded on a sample's `span_path` back to their
/// owning [`Span`]s. Constructed from
/// [`crate::toolpath_spans::AnnotatedToolpath::spans`] — typically
/// `SpanLookup::new(&annotated.spans)`.
///
/// Stays a borrow of the original spans slice; cheap to build per
/// gate-evaluation call.
#[derive(Debug, Clone, Copy)]
pub struct SpanLookup<'a> {
    spans: &'a [Span],
}

impl<'a> SpanLookup<'a> {
    pub fn new(spans: &'a [Span]) -> Self {
        Self { spans }
    }

    /// True if any span in `path` has the given [`SpanKind`].
    pub fn ancestors_contain_kind(&self, path: &[SpanId], kind: SpanKind) -> bool {
        path.iter().any(|id| {
            self.spans
                .get(id.0 as usize)
                .is_some_and(|s| s.kind == kind)
        })
    }

    /// First span of the given kind in `path`, walked outermost-first
    /// (matching `AnnotatedToolpath::span_path_at`'s emission order).
    pub fn first_span_of_kind(&self, path: &[SpanId], kind: SpanKind) -> Option<&Span> {
        path.iter()
            .filter_map(|id| self.spans.get(id.0 as usize))
            .find(|s| s.kind == kind)
    }
}

/// Classify a sample by its structural span ancestry, falling back to
/// arc engagement for steady-state cuts. Returns an operator-facing
/// label or `None` for plain low-engagement samples without distinctive
/// ancestry.
///
/// `span_lookup` is the toolpath's span resolver (from
/// `SpanLookup::new(&annotated.spans)`). Pass `None` when no annotated
/// toolpath is available — the classifier degrades to engagement-only
/// labels rather than fabricating ancestry.
pub fn classify_sample_locality(
    sample: &SimulationCutSample,
    span_lookup: Option<&SpanLookup<'_>>,
) -> Option<String> {
    if let Some(lookup) = span_lookup {
        if let Some(entry) = lookup.first_span_of_kind(&sample.span_path, SpanKind::Entry) {
            let label = entry.label.as_ref();
            return Some(if label.is_empty() {
                "entry".to_owned()
            } else {
                label.to_owned()
            });
        }
        if let Some(dressup) =
            lookup.first_span_of_kind(&sample.span_path, SpanKind::DressupArtifact)
        {
            let label = dressup.label.as_ref();
            if !label.is_empty() {
                return Some(label.to_owned());
            }
        }
        if lookup.ancestors_contain_kind(&sample.span_path, SpanKind::WaterlineCleanup) {
            return Some("waterline cleanup".to_owned());
        }
        if lookup.ancestors_contain_kind(&sample.span_path, SpanKind::LinkBridge) {
            return Some("region join".to_owned());
        }
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

/// Predicate the per-gate trip loops use to decide whether a sample is
/// part of steady-state cutting (and therefore eligible to drive the
/// gate trip) or part of a transient transition (kept out of the
/// trip; surfaced separately as an `EntrySpike` advisory).
///
/// G17 D7 of `planning/STRUCTURAL_ENTRY_SPANS_AND_LOCALITY.md`. The
/// pre-D3 kinematics-based heuristic
/// (`CutKinematics::{Helix, Plunge}` → not steady-state) hid terrain-
/// following Helix samples on adaptive3d 3D-rough cuts (D0 finding).
/// The replacement reads structural span ancestry: a sample is
/// steady-state iff none of its `span_path` ancestors is one of the
/// transient kinds.
///
/// Transient kinds excluded:
/// - [`SpanKind::Entry`] — configured entry transitions (plunge, ramp,
///   helix).
/// - [`SpanKind::WaterlineCleanup`] — adaptive3d's post-pass cleanup
///   sweep. Lateral feeds in cleanup spans can briefly engage
///   uncleared material the lift-bridge crosses; the simulator
///   reports the engagement faithfully but it's not a steady-state
///   condition the operator dialed in via feeds & speeds (Roadmap
///   F.3).
/// - All other transit-style spans tracked by the dexel stamper —
///   `SpanKind::{LinkBridge, LeadOut, DressupArtifact}` (any span the
///   stamper marks `in_transit_span = true`). Dexel readings at these
///   samples report `stock_top − cutter_z` over *neighbouring* stock
///   rather than steady-state engagement, so feeding them into any
///   peak/LUT-lookup metric (e.g. `lookup_axial_doc_mm` for chipload
///   DOC derating) silently inflates the gate's view of cutter
///   engagement (P3_TRANSIT_PEAK_DOC_RCA.md).
///
/// Consulting `sample.in_transit_span` first makes this the canonical
/// gate-side predicate — callers don't need to know whether they have
/// span_lookup plumbed; the dexel-stamper-set flag covers every transit
/// span the simulator marks.
///
/// When `span_lookup` is `None` and the trace predates `in_transit_span`
/// (the field defaults to `false` on deserialize of older traces),
/// the predicate returns `true` — conservatively treating every sample
/// as steady-state preserves the pre-D7 trip behaviour for legacy
/// callers rather than silently dropping samples from the gate.
pub fn is_steady_state_for_gate(
    sample: &SimulationCutSample,
    span_lookup: Option<&SpanLookup<'_>>,
) -> bool {
    !is_phantom_transit(sample, span_lookup) && !is_configured_entry(sample, span_lookup)
}

/// Phantom-transit predicate: returns `true` when the sample sits in a
/// transit-style span whose dexel reading does **not** reflect real
/// cutter engagement.
///
/// Specifically: `LeadOut`, `LinkBridge`, `DressupArtifact`,
/// `WaterlineCleanup`. The dexel reads `stock_top − cutter_z` over
/// *neighbouring* uncleared stock the bridge crosses; any peak /
/// extreme-value gate metric (LUT lookup, entry_spike report, trip
/// decision) must drop these samples entirely — surfacing the inflated
/// value as an `entry_spike` advisory misleads the operator
/// (P3_TRANSIT_PEAK_DOC_RCA.md).
///
/// `Entry` is **not** phantom-transit even though it's also marked
/// `in_transit_span = true`: a configured plunge / ramp / helix entry
/// IS a real engagement transient the operator can dial in, and the
/// entry_spike report on those samples is actionable. Use
/// [`is_configured_entry`] for that branch.
///
/// When `span_lookup` is `None`, the predicate falls back to the
/// `sample.in_transit_span` flag and assumes the worst (returns `true`
/// for any in_transit sample). This is conservative: we'd rather drop
/// a real Entry transient from the spike report than let a phantom
/// reading slip through. Callers that need the precise Entry/phantom
/// split must thread spans into the gate.
pub fn is_phantom_transit(
    sample: &SimulationCutSample,
    span_lookup: Option<&SpanLookup<'_>>,
) -> bool {
    match span_lookup {
        Some(lookup) => {
            // With spans, we can split phantom from configured-entry
            // precisely. A sample is phantom iff any non-Entry transit
            // kind appears in its ancestry — OR the stamper flagged it
            // `in_transit_span` without an Entry ancestor (F2.2: a
            // transit span dropped by TSP leaves its moves without
            // ancestry, but the per-move intent union still marks them
            // in_transit; dropping them beats letting a phantom
            // reading drive the gate).
            lookup.ancestors_contain_kind(&sample.span_path, SpanKind::WaterlineCleanup)
                || lookup.ancestors_contain_kind(&sample.span_path, SpanKind::LinkBridge)
                || lookup.ancestors_contain_kind(&sample.span_path, SpanKind::LeadOut)
                || lookup.ancestors_contain_kind(&sample.span_path, SpanKind::DressupArtifact)
                || (sample.in_transit_span
                    && !lookup.ancestors_contain_kind(&sample.span_path, SpanKind::Entry))
        }
        // No span info → fall back to the conservative
        // `in_transit_span` flag and treat anything in_transit as
        // phantom (Entry transients lose their entry_spike report
        // until spans are wired). Safer than letting phantoms surface.
        None => sample.in_transit_span,
    }
}

/// Configured-entry predicate: returns `true` when the sample sits in
/// an `Entry` span (the configured plunge / ramp / helix entry the
/// operation generator emitted).
///
/// Gates use this to route Entry-ancestry samples to their
/// `entry_spike` reporting track — distinct from steady-state trip
/// decisions (which exclude them) and from phantom transit samples
/// (which are dropped entirely; see [`is_phantom_transit`]).
///
/// When `span_lookup` is `None`, no split is possible — returns
/// `false`. Combined with [`is_phantom_transit`]'s conservative
/// fallback above, this means a no-span trace will route every
/// in_transit sample to "drop", never to entry_spike. Surface fewer
/// false-positive advisories at the cost of a true-positive Entry
/// spike going unreported. Most production traces have spans plumbed.
pub fn is_configured_entry(
    sample: &SimulationCutSample,
    span_lookup: Option<&SpanLookup<'_>>,
) -> bool {
    let Some(lookup) = span_lookup else {
        return false;
    };
    lookup.ancestors_contain_kind(&sample.span_path, SpanKind::Entry)
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
    use crate::ids::ToolpathId;
    use crate::simulation_cut::CutKinematics;
    use std::borrow::Cow;

    fn sample(arc: Option<f64>, span_path: Vec<SpanId>) -> SimulationCutSample {
        SimulationCutSample {
            toolpath_id: ToolpathId(0),
            move_index: 0,
            sample_index: 0,
            position: [0.0, 0.0, 0.0],
            cumulative_time_s: 0.0,
            segment_time_s: 0.0,
            is_cutting: true,
            cut_kinematics: CutKinematics::Linear,
            feed_rate_mm_min: 1000.0,
            spindle_rpm: 18000,
            flute_count: 2,
            axial_doc_mm: 1.0,
            axial_engagement_mm: 1.0,
            plunge_descent_mm: 0.0,
            arc_engagement_radians: arc,
            chipload_mm_per_tooth: 0.03,
            effective_chip_thickness_mm: None,
            engagement: crate::simulation_cut::Engagement::with_radial_woc(0.5),
            removed_volume_est_mm3: 0.0,
            mrr_mm3_s: 0.0,
            semantic_item_id: None,
            span_path,
            in_transit_span: false,
        }
    }

    fn span(start: usize, end: usize, kind: SpanKind, label: &'static str) -> Span {
        Span {
            start_move: start,
            end_move: end,
            kind,
            label: Cow::Borrowed(label),
            payload: None,
        }
    }

    #[test]
    fn entry_ancestor_returns_entry_label() {
        let spans = vec![
            span(0, 10, SpanKind::Operation, "op"),
            span(0, 3, SpanKind::Entry, "plunge entry"),
        ];
        let lookup = SpanLookup::new(&spans);
        // span_path is outermost-first: [Operation, Entry]
        let s = sample(Some(PI), vec![SpanId(0), SpanId(1)]);
        assert_eq!(
            classify_sample_locality(&s, Some(&lookup)).as_deref(),
            Some("plunge entry"),
        );
    }

    #[test]
    fn helix_entry_label_propagates_from_span() {
        let spans = vec![
            span(0, 10, SpanKind::Operation, "op"),
            span(0, 5, SpanKind::Entry, "helix entry"),
        ];
        let lookup = SpanLookup::new(&spans);
        let s = sample(Some(FRAC_PI_2), vec![SpanId(0), SpanId(1)]);
        assert_eq!(
            classify_sample_locality(&s, Some(&lookup)).as_deref(),
            Some("helix entry"),
        );
    }

    #[test]
    fn no_entry_ancestor_falls_back_to_engagement_label() {
        // The D6 regression: terrain-following Helix-kinematics samples
        // *without* an Entry ancestor must NOT be labelled "helix entry"
        // any more — the classifier reads structural ancestry, not
        // CutKinematics. The pre-D6 implementation would have returned
        // "helix entry" for this sample even though it sits in a
        // steady-state cut.
        let spans = vec![span(0, 10, SpanKind::Operation, "op")];
        let lookup = SpanLookup::new(&spans);
        let mut s = sample(Some(PI), vec![SpanId(0)]);
        s.cut_kinematics = CutKinematics::Helix;
        assert_eq!(
            classify_sample_locality(&s, Some(&lookup)).as_deref(),
            Some("slot section"),
        );

        let mut s = sample(Some(2.5), vec![SpanId(0)]);
        s.cut_kinematics = CutKinematics::Plunge;
        assert_eq!(
            classify_sample_locality(&s, Some(&lookup)).as_deref(),
            Some("heavy engagement"),
        );
    }

    #[test]
    fn dressup_artifact_label_propagates() {
        let spans = vec![
            span(0, 10, SpanKind::Operation, "op"),
            span(2, 5, SpanKind::DressupArtifact, "dogbone"),
        ];
        let lookup = SpanLookup::new(&spans);
        let s = sample(Some(FRAC_PI_2), vec![SpanId(0), SpanId(1)]);
        assert_eq!(
            classify_sample_locality(&s, Some(&lookup)).as_deref(),
            Some("dogbone"),
        );
    }

    #[test]
    fn link_bridge_ancestor_returns_region_join() {
        let spans = vec![
            span(0, 10, SpanKind::Operation, "op"),
            span(2, 5, SpanKind::LinkBridge, ""),
        ];
        let lookup = SpanLookup::new(&spans);
        let s = sample(Some(0.3), vec![SpanId(0), SpanId(1)]);
        assert_eq!(
            classify_sample_locality(&s, Some(&lookup)).as_deref(),
            Some("region join"),
        );
    }

    #[test]
    fn waterline_cleanup_ancestor_classifies_as_waterline_cleanup() {
        // Roadmap F.3: post-pass waterline cleanup spans carry transient
        // lift-bridge transitions, not steady-state cuts. Narration must
        // surface them so the operator can tell a peak comes from a
        // transient transition rather than dialed-in cutting.
        let spans = vec![
            span(0, 10, SpanKind::Operation, "op"),
            span(2, 8, SpanKind::WaterlineCleanup, "Waterline cleanup"),
        ];
        let lookup = SpanLookup::new(&spans);
        let s = sample(Some(PI), vec![SpanId(0), SpanId(1)]);
        assert_eq!(
            classify_sample_locality(&s, Some(&lookup)).as_deref(),
            Some("waterline cleanup"),
        );
    }

    #[test]
    fn linear_with_full_slot_arc_classifies_as_slot_section() {
        let s = sample(Some(PI), Vec::new());
        assert_eq!(
            classify_sample_locality(&s, None).as_deref(),
            Some("slot section"),
        );
    }

    #[test]
    fn linear_with_heavy_arc_classifies_as_heavy_engagement() {
        let s = sample(Some(2.5), Vec::new()); // < π but > π/2
        assert_eq!(
            classify_sample_locality(&s, None).as_deref(),
            Some("heavy engagement"),
        );
    }

    #[test]
    fn linear_with_low_arc_returns_none() {
        let s = sample(Some(0.5), Vec::new());
        assert!(classify_sample_locality(&s, None).is_none());
    }

    #[test]
    fn missing_arc_engagement_returns_none() {
        let s = sample(None, Vec::new());
        assert!(classify_sample_locality(&s, None).is_none());
    }

    #[test]
    fn missing_lookup_skips_ancestry_walk() {
        // `span_path` is set but lookup is None — fall back to
        // engagement-only classification (no panic, no fabricated label).
        let s = sample(Some(PI), vec![SpanId(0), SpanId(1)]);
        assert_eq!(
            classify_sample_locality(&s, None).as_deref(),
            Some("slot section"),
        );
    }

    #[test]
    fn is_steady_state_for_gate_excludes_entry_ancestor() {
        let spans = vec![
            span(0, 10, SpanKind::Operation, "op"),
            span(0, 3, SpanKind::Entry, "plunge entry"),
        ];
        let lookup = SpanLookup::new(&spans);
        // Inside the Entry span — not steady-state.
        let entry_sample = sample(Some(PI), vec![SpanId(0), SpanId(1)]);
        assert!(!is_steady_state_for_gate(&entry_sample, Some(&lookup)));
        // Past the Entry — steady-state.
        let cut_sample = sample(Some(PI), vec![SpanId(0)]);
        assert!(is_steady_state_for_gate(&cut_sample, Some(&lookup)));
    }

    #[test]
    fn is_steady_state_for_gate_terrain_helix_is_steady_state() {
        // The D7 regression: pre-D3, Helix-kinematics samples were
        // dropped from the gate by `is_steady_state_for_gate`. With
        // span ancestry as the signal, a terrain-following Helix sample
        // *without* an Entry ancestor counts as steady-state and
        // therefore drives the trip decision.
        let spans = vec![span(0, 10, SpanKind::Operation, "op")];
        let lookup = SpanLookup::new(&spans);
        let mut s = sample(Some(PI), vec![SpanId(0)]);
        s.cut_kinematics = CutKinematics::Helix;
        assert!(is_steady_state_for_gate(&s, Some(&lookup)));
    }

    #[test]
    fn is_steady_state_for_gate_no_lookup_assumes_steady_state() {
        // No span data plumbed → conservatively trip on every sample
        // rather than silently filter (matches pre-D3 behaviour).
        let s = sample(Some(PI), vec![SpanId(0)]);
        assert!(is_steady_state_for_gate(&s, None));
    }

    #[test]
    fn is_steady_state_for_gate_excludes_waterline_cleanup_ancestor() {
        // Roadmap F.3: a lift-bridge sample inside a WaterlineCleanup
        // span has a real peak engagement reported by the simulator
        // (e.g. the dexel says material is in the cutter's footprint),
        // but it's a transient transition, not a steady-state cut. The
        // gate must filter it out the same way it filters Entry
        // transients.
        let spans = vec![
            span(0, 20, SpanKind::Operation, "op"),
            span(10, 20, SpanKind::WaterlineCleanup, "Waterline cleanup"),
        ];
        let lookup = SpanLookup::new(&spans);
        // Inside cleanup → not steady-state.
        let cleanup_sample = sample(Some(PI), vec![SpanId(0), SpanId(1)]);
        assert!(!is_steady_state_for_gate(&cleanup_sample, Some(&lookup)));
        // Inside an earlier depth pass (no cleanup ancestor) → steady-state.
        let cut_sample = sample(Some(PI), vec![SpanId(0)]);
        assert!(is_steady_state_for_gate(&cut_sample, Some(&lookup)));
    }

    /// F2.2 — a sample the stamper flagged `in_transit_span` whose
    /// transit span was dropped (TSP split) has no transit ancestry,
    /// but must still be excluded from the steady-state trip. An
    /// Entry-ancestry sample with the flag keeps routing to the
    /// configured-entry track, not the phantom drop.
    #[test]
    fn in_transit_flag_without_ancestry_is_phantom_even_with_lookup() {
        let spans = vec![
            span(0, 10, SpanKind::Operation, "op"),
            span(0, 3, SpanKind::Entry, "plunge entry"),
        ];
        let lookup = SpanLookup::new(&spans);

        // Flagged transit, ancestry only reaches Operation (its
        // LinkBridge span was dropped by the TSP remap).
        let mut dropped_bridge = sample(Some(PI), vec![SpanId(0)]);
        dropped_bridge.in_transit_span = true;
        assert!(is_phantom_transit(&dropped_bridge, Some(&lookup)));
        assert!(!is_steady_state_for_gate(&dropped_bridge, Some(&lookup)));

        // Flagged transit WITH Entry ancestry → configured entry, not
        // phantom.
        let mut entry = sample(Some(PI), vec![SpanId(0), SpanId(1)]);
        entry.in_transit_span = true;
        assert!(!is_phantom_transit(&entry, Some(&lookup)));
        assert!(is_configured_entry(&entry, Some(&lookup)));

        // Unflagged steady-state sample unaffected.
        let steady = sample(Some(PI), vec![SpanId(0)]);
        assert!(!is_phantom_transit(&steady, Some(&lookup)));
        assert!(is_steady_state_for_gate(&steady, Some(&lookup)));
    }

    #[test]
    fn out_of_range_span_id_is_silently_ignored() {
        let spans = vec![span(0, 10, SpanKind::Operation, "op")];
        let lookup = SpanLookup::new(&spans);
        // SpanId(99) doesn't index any span — must not panic, and must
        // not satisfy any kind check.
        let s = sample(Some(PI), vec![SpanId(99)]);
        assert_eq!(
            classify_sample_locality(&s, Some(&lookup)).as_deref(),
            Some("slot section"),
        );
    }
}
