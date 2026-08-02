//! The post-generation transform provenance contract (Addendum C, item C1).
//!
//! # The defect class this closes
//!
//! Every transform that runs *after* a toolpath has been generated and
//! annotated — the dressups, the boundary clip, the entry-descent splitter —
//! moves move indices around. Several channels store move indices next to the
//! toolpath: `AnnotatedToolpath::spans`, and the semantic trace's per-item
//! move links. Before this module, each transform built a
//! [`MoveRemap`](crate::toolpath_spans::MoveRemap) *privately*, applied it to
//! the spans, and dropped it. The semantic trace was only kept honest because
//! ae10cb2's `SemanticLinkCarrier` smuggled its links through the span vector
//! as fake spans. That is a convention: the next channel, or the next
//! transform, silently drifts again — which is exactly what ae10cb2 was
//! fixing after A/M8 had already "fixed" it once.
//!
//! # The contract
//!
//! A transform that moves indices returns a [`Transformed<Unreconciled>`].
//! There is **no accessor** that yields the toolpath inside it. The only
//! route is [`Transformed::reconcile`], which takes a [`ReconcileSet`] — the
//! registry of index-carrying channels the call site owns — and hands back a
//! [`Transformed<Reconciled>`], which does have
//! [`into_inner`](Transformed::into_inner).
//!
//! So:
//!
//! * **Skipping reconcile is a compile error**, not drift: `into_inner` does
//!   not exist on the unreconciled state, and the type is `#[must_use]` so
//!   dropping it on the floor is a denied warning too.
//! * **Forgetting a channel is a compile error**: [`ReconcileSet::new`] takes
//!   one argument per registered channel. Registering a new channel changes
//!   that arity and breaks every construction site until its author has
//!   decided what that site owns. A site that owns nothing says so out loud
//!   with [`ReconcileSet::empty`].
//! * **A channel the contract has never heard of cannot exist**:
//!   [`RemapConsumer`] is sealed, so a new index-carrying channel has to be
//!   added *here*, in one place, next to the constructor it breaks.
//!
//! # What is NOT in the `ReconcileSet`, and why
//!
//! `AnnotatedToolpath::spans` are remapped by the transform itself and ride
//! home inside the payload. That is not a loophole, it is a necessity: a
//! transform does not merely translate span indices, it *emits* spans
//! (`Entry`, `DressupArtifact`, `LinkBridge`), and the reorder transform
//! additionally has to drop spans whose contents a permutation interleaved
//! with foreign moves ([`crate::tsp`]'s foreign-intrusion filter). No generic
//! consumer can do that. Spans therefore cannot be forgotten for a different
//! structural reason: they are inside the value the transform returns, and
//! the only way to get that value out is to reconcile everything else.
//!
//! # Adding a channel
//!
//! 1. Add a `RemapConsumer` implementation in this module (the trait is
//!    sealed; it will not compile anywhere else).
//! 2. Add a field to [`ReconcileSet`] and a parameter to
//!    [`ReconcileSet::new`].
//! 3. Fix the compile errors. Each one is a call site deciding, on the
//!    record, whether it owns that channel.

use std::marker::PhantomData;
use std::ops::Range;

use crate::semantic_trace::ToolpathSemanticRecorder;
use crate::toolpath::Toolpath;
use crate::toolpath_spans::{AnnotatedToolpath, MoveRemap};

mod sealed {
    pub trait Sealed {}
}

// ── Typestates ──────────────────────────────────────────────────────────

/// Typestate: the transform's provenance has not yet reached the
/// index-carrying channels. The payload is unreachable in this state.
#[derive(Debug)]
pub enum Unreconciled {}

/// Typestate: every registered channel has consumed the provenance, so the
/// payload and the channels agree on what each move index means.
#[derive(Debug)]
pub enum Reconciled {}

impl sealed::Sealed for Unreconciled {}
impl sealed::Sealed for Reconciled {}

/// Sealed marker for the two [`Transformed`] states.
pub trait ReconcileState: sealed::Sealed {}
impl ReconcileState for Unreconciled {}
impl ReconcileState for Reconciled {}

// ── MoveProvenance ──────────────────────────────────────────────────────

/// How one transform maps pre-transform move ranges onto post-transform
/// ones — the report a transform is now obliged to hand back.
///
/// The variants are not cosmetic: they carry the *drop rules* of the
/// transform family, which is precisely the part a caller cannot infer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MoveProvenance {
    /// Per-old-move output ranges. Covers insertion, deletion, fan-out
    /// (one old move becomes an entry ramp) and collapse (arc fitting,
    /// segment merge). Ranges keep their input order.
    Remap(MoveRemap),
    /// A reorder. Same per-move ranges as [`Self::Remap`], plus the
    /// foreign-intrusion rule a permutation needs: if the bounding range a
    /// claim maps onto also contains moves from outside the claim, the claim
    /// is DROPPED rather than silently widened to cover strangers. Mirrors
    /// [`crate::tsp`]'s span filter, on the same predicate
    /// ([`MoveRemap::foreign_intrusion`]).
    Permutation(MoveRemap),
    /// Insertion-only boundary mapping: `m[i]` is the first output index
    /// produced from input move `i`, and `m[n]` is the output move count.
    /// The shape the boundary clip and the entry-descent splitter already
    /// hand out, and what [`crate::toolpath_spans::Span::remap`] consumes.
    Mapping(Vec<usize>),
}

impl MoveProvenance {
    /// The provenance of a transform that did not move a single index —
    /// it rewrote move *content* (feed rates) only.
    #[must_use]
    pub fn index_preserving(n_moves: usize) -> Self {
        Self::Mapping((0..=n_moves).collect())
    }

    /// True when this provenance leaves every index where it found it.
    #[must_use]
    pub fn is_index_preserving(&self) -> bool {
        match self {
            Self::Mapping(m) => m.iter().enumerate().all(|(i, &v)| i == v),
            Self::Remap(r) | Self::Permutation(r) => {
                r.old_to_new.iter().enumerate().all(|(i, slot)| {
                    slot.as_ref()
                        .is_some_and(|s| s.start == i && s.end == i + 1)
                })
            }
        }
    }

    /// Remap a half-open pre-transform move range onto the post-transform
    /// move list.
    ///
    /// `None` means the range did not survive: every move in it was deleted,
    /// or (for [`Self::Permutation`]) the reorder scattered it so the
    /// bounding range would be a lie. That is the UNLINK case every
    /// index-carrying channel must handle — see
    /// [`crate::semantic_trace::ToolpathSemanticItem::move_end`].
    #[must_use]
    pub fn remap_range(
        &self,
        start: usize,
        end: usize,
        new_n_moves: usize,
    ) -> Option<Range<usize>> {
        match self {
            Self::Mapping(mapping) => {
                // Byte-for-byte the rule `Span::remap` applies, so a channel
                // and the spans beside it cannot disagree: out-of-range
                // indices clamp to the sentinel rather than vanishing.
                let last = mapping.last().copied().unwrap_or(0);
                let new_start = mapping.get(start).copied().unwrap_or(last);
                let new_end = mapping.get(end).copied().unwrap_or(last);
                Some(new_start..new_end)
            }
            Self::Remap(remap) => remap.remap_range(start, end),
            Self::Permutation(remap) => {
                let bounds = remap.remap_range(start, end)?;
                if remap.foreign_intrusion(start, end, &bounds).is_some() {
                    return None;
                }
                let _ = new_n_moves;
                Some(bounds)
            }
        }
    }

    /// Bounding POST-transform range of the moves this transform actually
    /// restructured — inserted, deleted, collapsed, fanned out or reordered.
    ///
    /// `None` when the transform preserved every index one-to-one, i.e. it
    /// has no structural claim at all and the caller must say what it means
    /// explicitly rather than defaulting to "the whole path" in silence
    /// (the per-dressup `0..len` attribution defect, C1 item 4a).
    #[must_use]
    pub fn touched_new_range(&self, new_n_moves: usize) -> Option<Range<usize>> {
        if new_n_moves == 0 {
            return None;
        }
        match self {
            // A reorder rearranges the whole group it is given; there is no
            // honest sub-range to claim.
            Self::Permutation(_) => Some(0..new_n_moves),
            Self::Mapping(mapping) => {
                let mut lo = usize::MAX;
                let mut hi = 0usize;
                for (a, b) in mapping.iter().zip(mapping.iter().skip(1)) {
                    let (a, b) = (*a, *b);
                    if b > a + 1 {
                        // Moves were inserted between old move `i` and the
                        // next one.
                        lo = lo.min(a);
                        hi = hi.max(b);
                    }
                }
                (lo != usize::MAX).then_some(lo..hi)
            }
            Self::Remap(remap) => {
                let mut lo = usize::MAX;
                let mut hi = 0usize;
                let mut covered = vec![0u8; new_n_moves];
                let mut note = |r: &Range<usize>| {
                    lo = lo.min(r.start);
                    hi = hi.max(r.end);
                };
                for slot in &remap.old_to_new {
                    match slot {
                        // Deleted: its neighbours' ranges bracket the hole,
                        // which the fan-out/collapse checks below pick up
                        // whenever anything else moved. A pure deletion in
                        // the middle is caught by the coverage pass.
                        None => {}
                        Some(r) => {
                            if r.end.saturating_sub(r.start) != 1 {
                                note(r);
                            }
                            for idx in r.start..r.end.min(new_n_moves) {
                                if let Some(c) = covered.get_mut(idx) {
                                    *c = c.saturating_add(1);
                                }
                            }
                        }
                    }
                }
                // Any new move claimed by no old move is an insertion; any
                // new move claimed by two or more is a collapse.
                for (idx, &c) in covered.iter().enumerate() {
                    if c != 1 {
                        lo = lo.min(idx);
                        hi = hi.max(idx + 1);
                    }
                }
                (lo != usize::MAX).then_some(lo..hi.min(new_n_moves))
            }
        }
    }
}

// ── Transformed ─────────────────────────────────────────────────────────

/// A toolpath a post-generation transform has just rewritten, together with
/// the provenance needed to bring every index-carrying channel along.
///
/// `Transformed<Unreconciled>` is deliberately a dead end: it has no
/// accessor for its payload. Reconcile it, or you cannot use it.
///
/// ```
/// use rs_cam_core::toolpath::Toolpath;
/// use rs_cam_core::toolpath_spans::AnnotatedToolpath;
/// use rs_cam_core::transform_provenance::{ReconcileSet, Transformed};
///
/// let t = Transformed::index_preserving(AnnotatedToolpath::new(Toolpath::new()));
/// // A site with no index-carrying channels says so out loud.
/// let at = t.reconcile(&mut ReconcileSet::empty()).into_inner();
/// assert!(at.toolpath.moves.is_empty());
/// ```
///
/// Skipping the reconcile step does not compile — `into_inner` exists only
/// on the reconciled state:
///
/// ```compile_fail
/// use rs_cam_core::toolpath::Toolpath;
/// use rs_cam_core::toolpath_spans::AnnotatedToolpath;
/// use rs_cam_core::transform_provenance::Transformed;
///
/// let t = Transformed::index_preserving(AnnotatedToolpath::new(Toolpath::new()));
/// let at = t.into_inner(); // no method `into_inner` on Transformed<Unreconciled>
/// ```
///
/// Neither does quietly dropping it:
///
/// ```compile_fail
/// #![deny(unused_must_use)]
/// use rs_cam_core::toolpath::Toolpath;
/// use rs_cam_core::toolpath_spans::AnnotatedToolpath;
/// use rs_cam_core::transform_provenance::Transformed;
///
/// fn transform(at: AnnotatedToolpath) -> Transformed { Transformed::index_preserving(at) }
/// transform(AnnotatedToolpath::new(Toolpath::new()));
/// ```
#[must_use = "a transformed toolpath carries a provenance map that the index-carrying \
              channels have not seen yet — call .reconcile(&mut ReconcileSet) before using it"]
pub struct Transformed<S: ReconcileState = Unreconciled> {
    annotated: AnnotatedToolpath,
    provenance: MoveProvenance,
    _state: PhantomData<S>,
}

impl Transformed<Unreconciled> {
    /// Report a transform that inserted, deleted, collapsed or fanned out
    /// moves.
    pub fn from_remap(annotated: AnnotatedToolpath, remap: MoveRemap) -> Self {
        Self::new(annotated, MoveProvenance::Remap(remap))
    }

    /// Report a transform that REORDERED moves — carries the extra drop rule
    /// a permutation needs (see [`MoveProvenance::Permutation`]).
    pub fn from_permutation(annotated: AnnotatedToolpath, remap: MoveRemap) -> Self {
        Self::new(annotated, MoveProvenance::Permutation(remap))
    }

    /// Report a transform that only inserted moves and handed out the
    /// boundary mapping (`m[i]` = first output index of input move `i`).
    pub fn from_mapping(annotated: AnnotatedToolpath, mapping: Vec<usize>) -> Self {
        Self::new(annotated, MoveProvenance::Mapping(mapping))
    }

    /// Report a transform that rewrote move CONTENT without moving a single
    /// index (feed-rate optimisation). Still has to be reconciled — the
    /// point is that the claim is made explicitly rather than by omission.
    pub fn index_preserving(annotated: AnnotatedToolpath) -> Self {
        let n = annotated.toolpath.moves.len();
        Self::new(annotated, MoveProvenance::index_preserving(n))
    }

    pub fn new(annotated: AnnotatedToolpath, provenance: MoveProvenance) -> Self {
        Self {
            annotated,
            provenance,
            _state: PhantomData,
        }
    }

    /// Push this transform's provenance into every registered channel.
    ///
    /// The single door out of [`Unreconciled`].
    pub fn reconcile(self, channels: &mut ReconcileSet<'_>) -> Transformed<Reconciled> {
        channels.consume(&self.provenance, &self.annotated.toolpath);
        Transformed {
            annotated: self.annotated,
            provenance: self.provenance,
            _state: PhantomData,
        }
    }
}

impl Transformed<Reconciled> {
    /// The transformed toolpath. Reachable only once the channels agree
    /// with it.
    #[must_use]
    pub fn into_inner(self) -> AnnotatedToolpath {
        self.annotated
    }

    /// The toolpath plus the provenance that produced it, for a caller that
    /// wants to attribute its own work (see
    /// [`MoveProvenance::touched_new_range`]).
    #[must_use]
    pub fn into_parts(self) -> (AnnotatedToolpath, MoveProvenance) {
        (self.annotated, self.provenance)
    }
}

impl<S: ReconcileState> Transformed<S> {
    /// The provenance report, readable in either state — it names no moves,
    /// so exposing it cannot leak an unreconciled payload.
    #[must_use]
    pub fn provenance(&self) -> &MoveProvenance {
        &self.provenance
    }

    /// Post-transform move count. Safe in either state for the same reason.
    #[must_use]
    pub fn move_count(&self) -> usize {
        self.annotated.toolpath.moves.len()
    }
}

// ── Channels ────────────────────────────────────────────────────────────

/// An index-carrying channel that must be told about every post-generation
/// transform.
///
/// SEALED on purpose. A channel type that is not implemented in this module
/// does not exist as far as the contract is concerned, so a new one cannot
/// be added without also touching [`ReconcileSet::new`] — whose arity is the
/// registry, and whose change is what makes every call site re-decide.
///
/// ```compile_fail
/// use rs_cam_core::toolpath::Toolpath;
/// use rs_cam_core::transform_provenance::{MoveProvenance, RemapConsumer};
///
/// struct MyChannel;
/// impl RemapConsumer for MyChannel {
///     fn consume_provenance(&mut self, _p: &MoveProvenance, _tp: &Toolpath) {}
/// }
/// ```
pub trait RemapConsumer: sealed::Sealed {
    /// Rewrite this channel's move indices through `provenance`. `toolpath`
    /// is the POST-transform toolpath — the one the new indices index.
    fn consume_provenance(&mut self, provenance: &MoveProvenance, toolpath: &Toolpath);
}

/// The semantic trace's per-item move links
/// ([`crate::semantic_trace::ToolpathSemanticItem::move_start`] /
/// `move_end`).
///
/// Replaces ae10cb2's `SemanticLinkCarrier`: instead of smuggling each link
/// through the span vector as a fake span so it would be remapped by
/// whatever the transform did to real spans, the link is remapped by the
/// transform's own reported provenance. The observable rules are unchanged —
/// same UNLINK policy for deleted moves, same foreign-intrusion drop for
/// reordered ones, same post-transform geometry re-derivation.
pub struct SemanticLinkChannel<'a>(&'a ToolpathSemanticRecorder);

impl<'a> SemanticLinkChannel<'a> {
    pub fn new(recorder: &'a ToolpathSemanticRecorder) -> Self {
        Self(recorder)
    }
}

impl sealed::Sealed for SemanticLinkChannel<'_> {}

impl RemapConsumer for SemanticLinkChannel<'_> {
    fn consume_provenance(&mut self, provenance: &MoveProvenance, toolpath: &Toolpath) {
        self.0.consume_provenance(provenance, toolpath);
    }
}

/// Scallop's per-ring runtime annotations
/// ([`crate::scallop::ScallopRuntimeAnnotation::move_index`]).
///
/// Registered in wave 12. Before it, the two call sites of
/// [`crate::surface_link::relink_fragments`] each hand-rolled
/// `annotations[i].move_index = report.move_remap[old]` off a bespoke
/// `Vec<usize>` the relinker built privately — the exact convention this
/// module exists to delete, one transform later.
///
/// Unlike the semantic trace, this channel cannot express "unlinked": a
/// `move_index` is a plain `usize`. An annotation whose move did not
/// survive the transform is therefore DROPPED rather than pointed at a
/// neighbour it does not describe. (The relinker itself maps every input
/// move onto something, so nothing is dropped there; the rule matters for
/// whatever transform is routed through this channel next.)
pub struct ScallopAnnotationChannel<'a>(&'a mut Vec<crate::scallop::ScallopRuntimeAnnotation>);

impl<'a> ScallopAnnotationChannel<'a> {
    pub fn new(annotations: &'a mut Vec<crate::scallop::ScallopRuntimeAnnotation>) -> Self {
        Self(annotations)
    }
}

impl sealed::Sealed for ScallopAnnotationChannel<'_> {}

impl RemapConsumer for ScallopAnnotationChannel<'_> {
    fn consume_provenance(&mut self, provenance: &MoveProvenance, toolpath: &Toolpath) {
        let n = toolpath.moves.len();
        self.0.retain_mut(
            |a| match provenance.remap_range(a.move_index, a.move_index + 1, n) {
                Some(r) => {
                    a.move_index = r.start;
                    true
                }
                None => false,
            },
        );
    }
}

// ── ReconcileSet ────────────────────────────────────────────────────────

/// Every index-carrying channel a call site owns.
///
/// One constructor argument per registered channel — the arity IS the
/// registry, so adding a channel breaks every site until its author says
/// what that site owns. `None` is an explicit "this site has none of that";
/// there is no default and no implicit skip.
///
/// A site that owns nothing at all uses [`Self::empty`], which reads as the
/// claim it is.
///
/// ```compile_fail
/// use rs_cam_core::transform_provenance::ReconcileSet;
/// // The registered channels are not optional to MENTION, only to own.
/// let mut set = ReconcileSet::new();
/// ```
pub struct ReconcileSet<'a> {
    semantic_links: Option<SemanticLinkChannel<'a>>,
    scallop_annotations: Option<ScallopAnnotationChannel<'a>>,
}

impl<'a> ReconcileSet<'a> {
    /// Register the channels this call site owns.
    ///
    /// * `semantic_trace` — the recorder whose items carry move links; the
    ///   generation recorder on the session path, the worker's recorder in
    ///   the GUI, `None` where no trace is being produced.
    /// * `scallop_annotations` — the per-ring runtime annotations a scallop
    ///   or unified-finish generator is carrying alongside its toolpath,
    ///   `None` for a site that holds none.
    pub fn new(
        semantic_trace: Option<&'a ToolpathSemanticRecorder>,
        scallop_annotations: Option<&'a mut Vec<crate::scallop::ScallopRuntimeAnnotation>>,
    ) -> Self {
        Self {
            semantic_links: semantic_trace.map(SemanticLinkChannel::new),
            scallop_annotations: scallop_annotations.map(ScallopAnnotationChannel::new),
        }
    }

    /// This call site owns no index-carrying channel. Distinct from
    /// forgetting one: it is written down.
    pub fn empty() -> Self {
        Self::new(None, None)
    }

    /// True when no channel is registered — a transform can skip provenance
    /// work it would only throw away.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.semantic_links.is_none() && self.scallop_annotations.is_none()
    }

    fn consume(&mut self, provenance: &MoveProvenance, toolpath: &Toolpath) {
        // Destructured, not field-accessed: a new channel makes this fail to
        // compile until it is routed, the same trick
        // `AnnotatedToolpath::translated` uses for coordinate-bearing fields.
        let Self {
            semantic_links,
            scallop_annotations,
        } = self;
        if let Some(channel) = semantic_links {
            channel.consume_provenance(provenance, toolpath);
        }
        if let Some(channel) = scallop_annotations {
            channel.consume_provenance(provenance, toolpath);
        }
    }
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
    use crate::geo::P3;

    fn tp_with(n: usize) -> Toolpath {
        let mut tp = Toolpath::new();
        for i in 0..n {
            tp.feed_to(P3::new(i as f64, 0.0, 0.0), 1000.0);
        }
        tp
    }

    #[test]
    fn index_preserving_round_trips_through_an_empty_set() {
        let at = AnnotatedToolpath::new(tp_with(4));
        let t = Transformed::index_preserving(at);
        assert!(t.provenance().is_index_preserving());
        assert_eq!(t.move_count(), 4);
        let out = t.reconcile(&mut ReconcileSet::empty()).into_inner();
        assert_eq!(out.toolpath.moves.len(), 4);
    }

    #[test]
    fn mapping_remap_matches_span_remap() {
        // Two moves inserted after old move 1.
        let mapping = vec![0, 1, 3, 4];
        let prov = MoveProvenance::Mapping(mapping.clone());
        let span = crate::toolpath_spans::Span::new(1, 3, crate::toolpath_spans::SpanKind::Region);
        let remapped = span.remap(&mapping);
        let via_prov = prov.remap_range(1, 3, 4).unwrap();
        assert_eq!(
            (remapped.start_move, remapped.end_move),
            (via_prov.start, via_prov.end)
        );
    }

    #[test]
    fn remap_drops_a_fully_deleted_range() {
        let remap = MoveRemap {
            old_to_new: vec![Some(0..1), None, None, Some(1..2)],
        };
        let prov = MoveProvenance::Remap(remap);
        assert_eq!(prov.remap_range(1, 3, 2), None);
        assert_eq!(prov.remap_range(0, 4, 2), Some(0..2));
    }

    #[test]
    fn permutation_drops_a_scattered_range() {
        // Old 0,1 land at new 0,2 with old 2 interleaved at new 1.
        let remap = MoveRemap {
            old_to_new: vec![Some(0..1), Some(2..3), Some(1..2)],
        };
        let prov = MoveProvenance::Permutation(remap.clone());
        assert_eq!(prov.remap_range(0, 2, 3), None, "foreign move intruded");
        // The same remap under plain `Remap` semantics keeps the bounds —
        // the difference IS the permutation rule.
        assert_eq!(
            MoveProvenance::Remap(remap).remap_range(0, 2, 3),
            Some(0..3)
        );
    }

    #[test]
    fn touched_range_is_none_when_nothing_moved() {
        assert_eq!(
            MoveProvenance::index_preserving(5).touched_new_range(5),
            None
        );
        let identity = MoveRemap::identity(5);
        assert_eq!(MoveProvenance::Remap(identity).touched_new_range(5), None);
    }

    #[test]
    fn touched_range_covers_an_insertion() {
        // Old move 1 fanned out into new 1..4.
        let remap = MoveRemap {
            old_to_new: vec![Some(0..1), Some(1..4), Some(4..5)],
        };
        assert_eq!(
            MoveProvenance::Remap(remap).touched_new_range(5),
            Some(1..4)
        );
        // Mapping form of the same insertion.
        assert_eq!(
            MoveProvenance::Mapping(vec![0, 1, 4, 5]).touched_new_range(5),
            Some(1..4)
        );
    }

    #[test]
    fn touched_range_covers_a_collapse() {
        // Old 1,2,3 collapsed onto the single new move 1.
        let remap = MoveRemap {
            old_to_new: vec![Some(0..1), Some(1..2), Some(1..2), Some(1..2), Some(2..3)],
        };
        assert_eq!(
            MoveProvenance::Remap(remap).touched_new_range(3),
            Some(1..2)
        );
    }

    #[test]
    fn a_permutation_claims_the_whole_path() {
        let remap = MoveRemap {
            old_to_new: vec![Some(1..2), Some(0..1)],
        };
        assert_eq!(
            MoveProvenance::Permutation(remap).touched_new_range(2),
            Some(0..2)
        );
    }
}
