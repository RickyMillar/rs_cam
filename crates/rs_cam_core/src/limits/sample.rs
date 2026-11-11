//! Feeds contract P29-3 (commit C): the cut of one simulation sample.
//!
//! [`Cut::of_sample`] states the kind of a measured sample and wraps the
//! engagement that the gates build today (`Engagement::from_sample`). The
//! sample does not grow (its size is pinned, G-SIMMEM): the kind comes from
//! side tables that [`SampleContext`] holds, by `sample.move_index`, from
//! `sample.source_intent` and from the span lookup of the gates.
//!
//! [`CutKind::is_judged_today`] states which kinds a gate judges today. The
//! gates still read the span filter (`tool_load::locality::
//! is_steady_state_for_gate`); the sentry
//! `tests/a_sample_states_its_cut_kind_p29_3.rs` holds the two equal on
//! every sample that is not air. P29-6 moves the gates onto the kind.
//!
//! The per-sample cost: one index into the cap table, one walk of the span
//! path (2 to 4 ids, as the gates walk it), one `match`. No allocation.

use crate::adaptive3d::FeedCapSource;
use crate::feeds::engagement::Engagement;
use crate::stock::simulation_cut::SimulationCutSample;
use crate::tool::ToolDefinition;
use crate::toolpath::{MoveIntent, Toolpath};
use crate::trace::semantic_trace::{SemanticKey, ToolpathSemanticKind, ToolpathSemanticTrace};
use crate::trace::toolpath_spans::{Span, SpanKind};

use super::{
    CleanupKind, Cut, CutEngagement, CutFacts, CutKind, CutSource, EntryKind, NoEngagementModel,
    SampleDepth,
};

/// The feed cap of one move.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveCap {
    /// The move has no `Move::feed_cap`.
    Uncapped,
    /// A `FeedCap` item of the semantic trace covers the capped move and
    /// names the rule that capped it.
    Source(FeedCapSource),
    /// The move is capped and no `FeedCap` item covers it.
    NoItem,
}

/// The feed cap of each move of one toolpath: `Move::feed_cap` and the
/// `FeedCap` items of the semantic trace (P22a-2). Built once per toolpath.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapTable {
    moves: Vec<MoveCap>,
    traced: bool,
}

impl CapTable {
    /// The cap table of `toolpath`. `semantic` is the semantic trace of the
    /// result, when the result holds one. A `FeedCap` item covers its moves
    /// from `move_start` to `move_end`, both inclusive; it names only the
    /// capped moves in that range.
    #[must_use]
    pub fn of_toolpath(toolpath: &Toolpath, semantic: Option<&ToolpathSemanticTrace>) -> Self {
        let mut moves: Vec<MoveCap> = toolpath
            .moves
            .iter()
            .map(|m| {
                if m.feed_cap.is_some() {
                    MoveCap::NoItem
                } else {
                    MoveCap::Uncapped
                }
            })
            .collect();
        for item in semantic.map_or(&[][..], |t| t.items.as_slice()) {
            if item.kind != ToolpathSemanticKind::FeedCap {
                continue;
            }
            let Some(source) = item
                .params
                .get(SemanticKey::CapSource)
                .and_then(|v| v.as_str())
                .and_then(FeedCapSource::from_key)
            else {
                continue;
            };
            let (Some(start), Some(end)) = (item.move_start, item.move_end) else {
                continue;
            };
            for cap in moves.iter_mut().take(end.saturating_add(1)).skip(start) {
                if *cap != MoveCap::Uncapped {
                    *cap = MoveCap::Source(source);
                }
            }
        }
        Self {
            moves,
            traced: semantic.is_some(),
        }
    }

    /// The cap of move `index`; [`MoveCap::Uncapped`] past the end.
    #[must_use]
    pub fn cap_of(&self, index: usize) -> MoveCap {
        self.moves.get(index).copied().unwrap_or(MoveCap::Uncapped)
    }

    /// True when the result held a semantic trace.
    #[must_use]
    pub fn has_trace(&self) -> bool {
        self.traced
    }
}

/// What [`Cut::of_sample`] reads beside the sample. Built once per toolpath,
/// with the lifetime of the gate context.
#[derive(Clone)]
pub struct SampleContext<'a> {
    tool: &'a ToolDefinition,
    spans: Option<&'a [Span]>,
    caps: CapTable,
    facts: CutFacts,
}

impl<'a> SampleContext<'a> {
    /// The context of one toolpath.
    ///
    /// - `tool`: the built tool.
    /// - `spans`: the spans that the gates read; `None` when the spans are
    ///   not valid (`AnnotatedToolpath::spans_valid`), as the gates pass.
    /// - `caps`: [`CapTable::of_toolpath`].
    /// - `facts`: the facts of the body cut of the op. The caller fills them
    ///   from the cut list of the op (`compute::catalog`); this folder reads
    ///   no op.
    #[must_use]
    pub fn new(
        tool: &'a ToolDefinition,
        spans: Option<&'a [Span]>,
        caps: CapTable,
        facts: CutFacts,
    ) -> Self {
        Self {
            tool,
            spans,
            caps,
            facts,
        }
    }

    /// The span kind of the sample's ancestry that names its cut: a
    /// `DressupArtifact` span before a `WaterlineCleanup` span. One walk of
    /// the span path (2 to 4 ids). `None` with no valid spans.
    fn cut_span(&self, sample: &SimulationCutSample) -> Option<SpanKind> {
        let spans = self.spans?;
        let mut cleanup = false;
        for id in &sample.span_path {
            match spans.get(id.0 as usize).map(|s| s.kind) {
                Some(SpanKind::DressupArtifact) => return Some(SpanKind::DressupArtifact),
                Some(SpanKind::WaterlineCleanup) => cleanup = true,
                Some(
                    SpanKind::Operation
                    | SpanKind::DepthPass
                    | SpanKind::Region
                    | SpanKind::Entry
                    | SpanKind::LeadOut
                    | SpanKind::LinkBridge
                    | SpanKind::GeometryRefit
                    | SpanKind::RapidOrderBarrier,
                )
                | None => {}
            }
        }
        cleanup.then_some(SpanKind::WaterlineCleanup)
    }

    /// The kind of a sample whose intent is a cut (or no intent). The order:
    /// a dressup relief by its span, a waterline cleanup by its span, a
    /// capped step or a slot ring by the cap table, else the body.
    fn body_kind(&self, sample: &SimulationCutSample) -> CutKind {
        match self.cut_span(sample) {
            // EXEMPT(P28-23)
            Some(SpanKind::DressupArtifact) => return CutKind::DressupRelief,
            // EXEMPT(P28-22)
            Some(SpanKind::WaterlineCleanup) => {
                return CutKind::Cleanup(CleanupKind::WaterlineSweep);
            }
            Some(
                SpanKind::Operation
                | SpanKind::DepthPass
                | SpanKind::Region
                | SpanKind::Entry
                | SpanKind::LeadOut
                | SpanKind::LinkBridge
                | SpanKind::GeometryRefit
                | SpanKind::RapidOrderBarrier,
            )
            | None => {}
        }
        match self.caps.cap_of(sample.move_index) {
            MoveCap::Uncapped => CutKind::Body,
            MoveCap::Source(source) => CutKind::CappedStep(source),
            // A capped move that no item names. With a semantic trace it is
            // the slot ring of a contour pocket (ruling Q4: inferred, not
            // stated). With no trace, no rule tells a slot ring from a
            // capped step: the sample is the body.
            MoveCap::NoItem => {
                if self.caps.has_trace() {
                    // EXEMPT(Q4-SlotRingInferred)
                    CutKind::SlotRing
                } else {
                    // EXEMPT(Q4-CappedStepNoTrace)
                    CutKind::Body
                }
            }
        }
    }
}

impl CutKind {
    /// The kind of `sample`, air or not. The intent of the move decides
    /// the entry, lead, link and drill kinds; a cut intent goes to
    /// [`SampleContext::body_kind`].
    ///
    /// The order for a cut intent: the `DressupArtifact` span (a dogbone
    /// relief), then the `WaterlineCleanup` span, then the cap table. So a
    /// dogbone on the capped first ring of a contour pocket is a
    /// [`Self::DressupRelief`], not a [`Self::SlotRing`]: the span wins. When
    /// the spans are not valid, no span is read: the kind is the kind of
    /// the intent and the cap table (a relief or a cleanup move is then the
    /// body), as the gates then read only the transit flag.
    ///
    /// The intent wins over the span (`P29-3_DESIGN.md` section 3): a
    /// `Linking` move inside an entry span is a link (row
    /// `Q10-LinkingInEntrySpan`), and an `EntryPlunge` move in no entry span
    /// is an entry (row `Q10-EntryPlungeInNoSpan`). A `Retract` feed and the
    /// obstacle rapid join two cuts: they are links. A peck ladder has no
    /// intent of its own: its plunges are plunges.
    #[must_use]
    pub fn of_sample(sample: &SimulationCutSample, ctx: &SampleContext<'_>) -> Self {
        match sample.source_intent {
            Some(MoveIntent::EntryPlunge) => Self::Entry(EntryKind::Plunge),
            Some(MoveIntent::EntryRamp) => Self::Entry(EntryKind::Ramp),
            Some(MoveIntent::EntryHelix) => Self::Entry(EntryKind::Helix),
            Some(MoveIntent::EntryApproach) => Self::EntryApproach,
            Some(MoveIntent::LeadIn) => Self::LeadIn,
            Some(MoveIntent::LeadOut) => Self::LeadOut,
            Some(MoveIntent::Linking | MoveIntent::Retract | MoveIntent::ObstacleClearance) => {
                Self::Link
            }
            Some(MoveIntent::Drilling) => Self::Drill,
            Some(MoveIntent::ClearingCut | MoveIntent::FinishingCut | MoveIntent::Unknown)
            | None => ctx.body_kind(sample),
        }
    }

    /// True when a gate judges a cut of this kind today: the kind form of
    /// the span filter `tool_load::locality::is_steady_state_for_gate`. The
    /// sentry holds the two equal on every sample that is not air.
    ///
    /// The span filter states today's behaviour (ruling Q10). The stamper
    /// flags each move of an entry, lead or link intent `in_transit_span`
    /// (`AnnotatedToolpath::transit_moves_bitmap_from_intents`), so the
    /// filter drops each such sample, in an entry span or not.
    #[must_use]
    pub fn is_judged_today(self) -> bool {
        match self {
            Self::Body
            | Self::FirstPass(_)
            | Self::SpringPass
            | Self::CappedStep(_)
            | Self::SlotRing
            | Self::Drill => true,
            // The configured entries: `is_configured_entry`. An entry
            // plunge in no entry span is dropped as phantom transit by its
            // flag, so it is not judged either.
            // EXEMPT(P28-21, Q10-EntryPlungeInNoSpan)
            Self::Entry(_) => false,
            // The phantom transit of `is_phantom_transit`. A `Linking`
            // sample inside an entry span is dropped as a configured entry.
            // EXEMPT(P28-22, P28-23, Q10-LinkingInEntrySpan)
            Self::Cleanup(_) | Self::LeadOut | Self::Link => false,
            // A dogbone relief: its `DressupArtifact` span is phantom
            // transit.
            // EXEMPT(P28-23)
            Self::DressupRelief => false,
            // A feed through air above an entry, and a lead into the cut:
            // the flag of the intent drops them.
            // EXEMPT(C-LeadInApproach)
            Self::EntryApproach | Self::LeadIn => false,
        }
    }
}

impl Cut {
    /// The cut of one simulation sample; `None` for an air sample
    /// (`SimulationCutSample::is_air`).
    ///
    /// - The kind: [`CutKind::of_sample`].
    /// - The engagement: the door of the gates, `Engagement::from_sample`
    ///   (the removed layer, `SampleDepth::RemovedLayer`). A sample with no
    ///   measured arc has no model ([`NoEngagementModel::NoMeasuredArc`]).
    /// - The point: `None`. Each gate builds its own point from the sample
    ///   and its feed (`feeds::cut_load::OperatingPoint`, one door each).
    /// - The facts: the facts of the body cut of the op.
    #[must_use]
    pub fn of_sample(sample: &SimulationCutSample, ctx: &SampleContext<'_>) -> Option<Self> {
        if sample.is_air() {
            return None;
        }
        let engagement = Engagement::from_sample(sample, ctx.tool).map_or(
            CutEngagement::NoModel(NoEngagementModel::NoMeasuredArc),
            CutEngagement::Side,
        );
        Some(Self {
            kind: CutKind::of_sample(sample, ctx),
            engagement,
            point: None,
            source: CutSource::Measured {
                sample_index: u32::try_from(sample.sample_index).unwrap_or(u32::MAX),
                move_index: u32::try_from(sample.move_index).unwrap_or(u32::MAX),
                depth: SampleDepth::RemovedLayer,
            },
            facts: ctx.facts,
        })
    }
}
