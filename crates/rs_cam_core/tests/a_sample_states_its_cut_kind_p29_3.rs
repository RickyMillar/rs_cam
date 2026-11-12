//! Feeds contract P29-3, commit C
//! (`planning/feeds_contract_2026-11-03/phases/P29-3_DESIGN.md` section 3):
//! a simulation sample states its cut kind, and the kind tells the group
//! in which every gate judges it (P29-6 6d).
//!
//! Five small fixtures, generated through the session and simulated: a
//! contour Pocket with a ramp entry (the slot ring), the same Pocket with
//! dogbones (the dressup relief), a Profile with two tabs and leads (no
//! entry dressup, so the tab re-plunges stay plunges), the Geodesic
//! Adaptive 3D Rough of the D65 shelf (capped steps, the waterline cleanup,
//! a planner ramp entry), and a 2D Adaptive with its default helix entry
//! (P29-6 ruling Q6: the helix entry).
//!
//! - Claim 1: `Cut::of_sample` gives `None` exactly for a sample that
//!   removes no material (`SimulationCutSample::removes_material`, P29-6
//!   ruling Q2).
//! - Claim 2: for every sample, `locality::judged_group` equals the group of
//!   the cut's kind (`CutKind::group`), with the span lookup and the cap
//!   table of the gates; `None` exactly when the sample removes nothing.
//!   The body rule of a reader with no span table
//!   (`locality::is_body_without_spans`, the triage) equals the body group
//!   on every cutting sample with a depth that removes material (and keeps
//!   out every sample that removes nothing, which is in no group).
//! - Claim 3: the kind of every sample follows the side tables: the intent,
//!   the dressup span, then the waterline cleanup span, then
//!   `Move::feed_cap` and the `FeedCap` items of
//!   the semantic trace. The test rebuilds the kind from those tables on its
//!   own (as `every_wide_step_has_a_named_source_p22a` reads the items), so
//!   a broken cap table goes red here: claim 2 cannot see it (a body, a
//!   capped step and a slot ring are all judged).
//! - Claim 4: the cut wraps the engagement of the gates
//!   (`Engagement::from_sample`) and the body facts of the op.
//! - Non-vacuity: each kind that a fixture holds has at least one sample
//!   that removes material; a `Linking` sample in an entry span has samples
//!   on the shelf (a link: the intent wins); the inferred slot ring has
//!   samples on the Pocket; a capped step read with no semantic trace has
//!   samples on the shelf (the body).
//! - An entry plunge in no entry span has no sample in these fixtures (the
//!   tab re-plunges of the Profile lie in an entry span). A synthetic sample
//!   holds the class: the intent makes it an entry, judged in the Entry
//!   group.
//!
//! The test prints the count of each kind (`--nocapture`).
//!
//! `cargo test -p rs_cam_core -q --test a_sample_states_its_cut_kind_p29_3 -- --nocapture`

#![expect(
    clippy::expect_used,
    clippy::print_stderr,
    reason = "test code: a panic is how a test reports a failure; the test prints its counts for the reader"
)]

mod common;

use std::collections::BTreeMap;

use common::tools::endmill_tool_config;

use rs_cam_core::adaptive3d::FeedCapSource;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::cutter::build_cutter;
use rs_cam_core::compute::operation_configs::ProfileConfig;
use rs_cam_core::feeds::engagement::Engagement;
use rs_cam_core::limits::{
    CapTable, CleanupKind, Cut, CutEngagement, CutFacts, CutGroup, CutKind, CutSource, EntryKind,
    NoEngagementModel, SampleContext, SampleDepth,
};
use rs_cam_core::session::ProjectSession;
use rs_cam_core::stock::simulation_cut::{Engagement as SampleEngagement, SimulationCutSample};
use rs_cam_core::tool::ToolDefinition;
use rs_cam_core::tool_load::locality::{is_body_without_spans, judged_group};
use rs_cam_core::toolpath::{MoveIntent, Toolpath};
use rs_cam_core::trace::semantic_trace::{
    SemanticKey, ToolpathSemanticKind, ToolpathSemanticTrace,
};
use rs_cam_core::trace::toolpath_spans::{Span, SpanId, SpanKind};

// ── The fixtures ────────────────────────────────────────────────────────

use common::kind_fixtures::{
    TOOL_D, adaptive_session, pocket_session, profile_session, shelf_session,
};

// ── The independent kind (claim 3) ──────────────────────────────────────

/// The `FeedCap` source of each capped move, read from the items on their
/// own (the reader of P22a, `exact_sources`).
fn item_sources(
    tp: &Toolpath,
    trace: Option<&ToolpathSemanticTrace>,
) -> Vec<Option<FeedCapSource>> {
    let mut out = vec![None; tp.moves.len()];
    for item in trace.map_or(&[][..], |t| t.items.as_slice()) {
        if item.kind != ToolpathSemanticKind::FeedCap {
            continue;
        }
        let source = item
            .params
            .get(SemanticKey::CapSource)
            .and_then(|v| v.as_str())
            .and_then(FeedCapSource::from_key)
            .expect("a FeedCap item names its source");
        let (Some(a), Some(b)) = (item.move_start, item.move_end) else {
            continue;
        };
        for (k, slot) in out.iter_mut().enumerate().take(b + 1).skip(a) {
            if tp.moves.get(k).is_some_and(|m| m.feed_cap.is_some()) {
                *slot = Some(source);
            }
        }
    }
    out
}

/// True when a span of `kind` holds the sample's move (the raw spans, not
/// the lookup).
fn in_span(spans: Option<&[Span]>, path: &[SpanId], kind: SpanKind) -> bool {
    spans.is_some_and(|spans| {
        path.iter()
            .any(|id| spans.get(id.0 as usize).is_some_and(|s| s.kind == kind))
    })
}

/// The kind that the side tables state for `s`, rebuilt by the test.
fn expected_kind(
    s: &SimulationCutSample,
    tp: &Toolpath,
    spans: Option<&[Span]>,
    sources: &[Option<FeedCapSource>],
    traced: bool,
) -> CutKind {
    let cut = || {
        if in_span(spans, &s.span_path, SpanKind::DressupArtifact) {
            return CutKind::DressupRelief;
        }
        if in_span(spans, &s.span_path, SpanKind::WaterlineCleanup) {
            return CutKind::Cleanup(CleanupKind::WaterlineSweep);
        }
        let capped = tp
            .moves
            .get(s.move_index)
            .is_some_and(|m| m.feed_cap.is_some());
        match (capped, sources.get(s.move_index).copied().flatten(), traced) {
            (false, _, _) | (true, None, false) => CutKind::Body,
            (true, Some(source), _) => CutKind::CappedStep(source),
            (true, None, true) => CutKind::SlotRing,
        }
    };
    match s.source_intent {
        Some(MoveIntent::EntryPlunge) => CutKind::Entry(EntryKind::Plunge),
        Some(MoveIntent::EntryRamp) => CutKind::Entry(EntryKind::Ramp),
        Some(MoveIntent::EntryHelix) => CutKind::Entry(EntryKind::Helix),
        Some(MoveIntent::EntryApproach) => CutKind::EntryApproach,
        Some(MoveIntent::LeadIn) => CutKind::LeadIn,
        Some(MoveIntent::LeadOut) => CutKind::LeadOut,
        Some(MoveIntent::Linking | MoveIntent::Retract | MoveIntent::ObstacleClearance) => {
            CutKind::Link
        }
        Some(MoveIntent::Drilling) => CutKind::Drill,
        Some(MoveIntent::ClearingCut | MoveIntent::FinishingCut | MoveIntent::Unknown) | None => {
            cut()
        }
    }
}

// ── The readout ─────────────────────────────────────────────────────────

/// What one fixture gives.
#[derive(Debug, Default)]
struct Readout {
    samples: usize,
    /// The samples that remove no material (no cut).
    no_removal: usize,
    /// The samples that remove material, by kind.
    kinds: BTreeMap<String, usize>,
    /// The samples that remove material, by group.
    groups: BTreeMap<String, usize>,
    /// `Linking` samples that remove material, in an entry span (a link:
    /// the intent wins).
    linking_in_entry_span: usize,
    /// Entry plunges that remove material, in no entry span.
    entry_plunge_in_no_span: usize,
    /// The inferred slot ring (a body-group label).
    slot_ring: usize,
    /// Capped samples that remove material, read as the body with no
    /// trace.
    capped_no_trace: usize,
    /// Claim 2 failures: the count and one example of each class.
    mismatches: BTreeMap<String, (usize, String)>,
    /// Claim 3 failures.
    wrong_kind: BTreeMap<String, (usize, String)>,
    /// Claim 1 and claim 4 failures.
    wrong_cut: Vec<String>,
}

impl Readout {
    fn has(&self, kind: CutKind) -> bool {
        self.kinds.get(&format!("{kind:?}")).is_some_and(|&n| n > 0)
    }

    fn print(&self, name: &str) {
        eprintln!(
            "\n## {name}: {} samples, {} remove no material",
            self.samples, self.no_removal
        );
        for (k, n) in &self.kinds {
            eprintln!("{n:>7}  {k}");
        }
        for (g, n) in &self.groups {
            eprintln!("{n:>7}  group {g}");
        }
        eprintln!(
            "linking in an entry span {}, entry plunge in no span {}, inferred slot ring {}, \
             capped step with no trace {}",
            self.linking_in_entry_span,
            self.entry_plunge_in_no_span,
            self.slot_ring,
            self.capped_no_trace
        );
    }

    /// Claims 1 to 4 hold on every sample.
    fn assert_claims(&self, name: &str) {
        assert!(
            self.wrong_cut.is_empty(),
            "{name}: the cut of a sample (claims 1, 4): {:?}",
            self.wrong_cut
        );
        assert!(
            self.mismatches.is_empty(),
            "{name}: the judged group and the kind's group disagree (claim 2): {:?}",
            self.mismatches
        );
        assert!(
            self.wrong_kind.is_empty(),
            "{name}: the kind does not follow the side tables (claim 3): {:?}",
            self.wrong_kind
        );
    }

    fn assert_holds(&self, name: &str, kinds: &[CutKind]) {
        for kind in kinds {
            assert!(
                self.has(*kind),
                "{name}: non-vacuity: no sample of kind {kind:?} that removes material: {:?}",
                self.kinds
            );
        }
    }
}

/// The readout of toolpath 0 of a simulated session. `with_trace: false`
/// reads the result as if it held no semantic trace (row
/// a capped step with no trace).
fn readout(session: &ProjectSession, with_trace: bool) -> Readout {
    let tc = session.toolpath_config_at(0).expect("the op");
    let tool_cfg = session
        .tool_by_id(tc.tool_id.expect("a tool"))
        .expect("the tool");
    let tool = build_cutter(tool_cfg).expect("the cutter builds");
    let result = session.result_at(0).expect("a result");
    let annotated = result.annotated();
    let trace = session
        .simulation_result()
        .and_then(|s| s.cut_trace.as_deref())
        .expect("a cut trace");
    let semantic = if with_trace {
        result.semantic_trace.as_deref()
    } else {
        None
    };
    // The spans of the gates (`ProjectSession::load_context_parts`).
    let spans = annotated.spans_valid.then_some(annotated.spans.as_slice());
    let facts = tc.operation.cuts().body_facts();
    let caps = CapTable::of_toolpath(&annotated.toolpath, semantic);
    let ctx = SampleContext::new(&tool, spans, &caps, facts);
    let sources = item_sources(&annotated.toolpath, semantic);
    let mut out = Readout::default();
    for s in &trace.samples {
        if s.toolpath_id != tc.id {
            continue;
        }
        out.samples += 1;
        let group = judged_group(s, &ctx);
        let kind = CutKind::of_sample(s, &ctx);
        let expected = expected_kind(s, &annotated.toolpath, spans, &sources, semantic.is_some());
        if kind != expected {
            let e = out
                .wrong_kind
                .entry(format!("{kind:?} != {expected:?}"))
                .or_insert_with(|| (0, example(s)));
            e.0 += 1;
        }
        let cut = Cut::of_sample(s, &ctx);
        if cut.is_some() != s.removes_material() {
            out.wrong_cut.push(format!(
                "removes material {} but cut {:?}: {}",
                s.removes_material(),
                cut.is_some(),
                example(s)
            ));
        }
        if group != cut.map(|c| c.kind.group()) {
            let e = out
                .mismatches
                .entry(format!("{kind:?}: judged {group:?}"))
                .or_insert_with(|| (0, example(s)));
            e.0 += 1;
        }
        // P29-6 6d (M8): the body rule of a reader with no span table (the
        // triage) equals the body group on every cutting sample with a depth.
        if s.is_cutting
            && s.axial_engagement_mm > 0.0
            && is_body_without_spans(s) != (group == Some(CutGroup::Body))
            && (s.removes_material() || !is_body_without_spans(s))
        {
            let e = out
                .mismatches
                .entry(format!(
                    "{kind:?}: no-span body {} but group {group:?}",
                    is_body_without_spans(s)
                ))
                .or_insert_with(|| (0, example(s)));
            e.0 += 1;
        }
        let Some(cut) = cut else {
            out.no_removal += 1;
            continue;
        };
        check_cut(&cut, s, &tool, kind, facts, &mut out.wrong_cut);
        *out.kinds.entry(format!("{kind:?}")).or_default() += 1;
        *out.groups.entry(format!("{:?}", kind.group())).or_default() += 1;
        let entry_span = in_span(spans, &s.span_path, SpanKind::Entry);
        if s.source_intent == Some(MoveIntent::Linking) && entry_span {
            out.linking_in_entry_span += 1;
        }
        if s.source_intent == Some(MoveIntent::EntryPlunge) && !entry_span {
            out.entry_plunge_in_no_span += 1;
        }
        if kind == CutKind::SlotRing {
            out.slot_ring += 1;
        }
        let capped = annotated
            .toolpath
            .moves
            .get(s.move_index)
            .is_some_and(|m| m.feed_cap.is_some());
        if semantic.is_none() && capped && kind == CutKind::Body {
            out.capped_no_trace += 1;
        }
    }
    out
}

/// Claim 4: the engagement is the gates' door, the source is the sample,
/// the facts are the op's body facts, the point is not set.
fn check_cut(
    cut: &Cut,
    s: &SimulationCutSample,
    tool: &ToolDefinition,
    kind: CutKind,
    facts: CutFacts,
    wrong: &mut Vec<String>,
) {
    let engagement = Engagement::from_sample(s, tool).map_or(
        CutEngagement::NoModel(NoEngagementModel::NoMeasuredArc),
        CutEngagement::Side,
    );
    let source = CutSource::Measured {
        sample_index: u32::try_from(s.sample_index).expect("a small trace"),
        move_index: u32::try_from(s.move_index).expect("a small toolpath"),
        depth: SampleDepth::RemovedLayer,
    };
    if cut.kind != kind
        || cut.engagement != engagement
        || cut.source != source
        || cut.facts != facts
        || cut.point.is_some()
    {
        wrong.push(format!("{cut:?} for {}", example(s)));
    }
}

fn example(s: &SimulationCutSample) -> String {
    format!(
        "sample {} move {} intent {:?} span_path {:?} in_transit {} at {:?}",
        s.sample_index, s.move_index, s.source_intent, s.span_path, s.in_transit_span, s.position
    )
}

// ── The tests ───────────────────────────────────────────────────────────

#[test]
fn a_pocket_sample_states_its_cut_kind() {
    let r = readout(&pocket_session(false), true);
    r.print("contour Pocket, ramp entry");
    r.assert_claims("pocket");
    r.assert_holds(
        "pocket",
        &[
            CutKind::Body,
            CutKind::SlotRing,
            CutKind::Entry(EntryKind::Ramp),
            CutKind::EntryApproach,
        ],
    );
    assert!(r.slot_ring > 0, "non-vacuity: the inferred slot ring");
    assert!(
        !r.has(CutKind::DressupRelief),
        "no dogbone dressup: no relief"
    );
}

/// The fourth fixture (lead ruling 1, 2026-11-11): the Pocket with dogbones.
/// A relief move keeps the cut intent; its `DressupArtifact` span names it.
/// On the capped first ring the span wins over the slot ring inference.
#[test]
fn a_dogbone_sample_states_its_cut_kind() {
    let r = readout(&pocket_session(true), true);
    r.print("contour Pocket, ramp entry, dogbones");
    r.assert_claims("pocket with dogbones");
    r.assert_holds(
        "pocket with dogbones",
        &[
            CutKind::Body,
            CutKind::SlotRing,
            CutKind::DressupRelief,
            CutKind::Entry(EntryKind::Ramp),
        ],
    );
}

#[test]
fn a_profile_sample_states_its_cut_kind() {
    let r = readout(&profile_session(), true);
    r.print("outside Profile, tabs and leads, no entry dressup");
    r.assert_claims("profile");
    r.assert_holds(
        "profile",
        &[
            CutKind::Body,
            CutKind::Entry(EntryKind::Plunge),
            CutKind::LeadIn,
            CutKind::LeadOut,
        ],
    );
    assert_eq!(r.slot_ring, 0, "a Profile has no slot ring");
}

#[test]
fn a_shelf_sample_states_its_cut_kind() {
    let session = shelf_session();
    let r = readout(&session, true);
    r.print("Geodesic 3D Rough, D65 shelf");
    r.assert_claims("shelf");
    let mut kinds = vec![
        CutKind::Body,
        CutKind::Cleanup(CleanupKind::WaterlineSweep),
        CutKind::Entry(EntryKind::Ramp),
        CutKind::EntryApproach,
        CutKind::Link,
    ];
    // The five sources that this fixture caps (`LevelContour` is off by
    // default).
    kinds.extend(
        FeedCapSource::ALL
            .into_iter()
            .filter(|s| *s != FeedCapSource::LevelContour)
            .map(CutKind::CappedStep),
    );
    r.assert_holds("shelf", &kinds);
    assert_eq!(
        r.slot_ring, 0,
        "every capped move of the shelf lies in a FeedCap item (the P22a pin)"
    );
    assert!(
        r.linking_in_entry_span > 0,
        "non-vacuity: a link in an entry span"
    );

    // With no semantic trace, a capped move is
    // the body, and the equality still holds.
    let bare = readout(&session, false);
    bare.print("Geodesic 3D Rough, D65 shelf, read with no semantic trace");
    bare.assert_claims("shelf, no trace");
    assert!(
        bare.capped_no_trace > 0,
        "non-vacuity: a capped step read with no trace"
    );
    assert_eq!(bare.slot_ring, 0, "no trace: no slot ring is inferred");
}

/// An entry plunge in no entry span (no fixture sample): the intent makes
/// it an entry, judged in the Entry group (P29-6 6d), with or without a
/// span.
#[test]
fn an_entry_plunge_in_no_span_is_judged_in_the_entry_group() {
    let spans = [
        Span::new(0, 4, SpanKind::Operation),
        Span::new(0, 1, SpanKind::Entry),
    ];
    let tool = build_cutter(&endmill_tool_config(TOOL_D)).expect("the cutter builds");
    let tp = Toolpath::new();
    let caps = CapTable::of_toolpath(&tp, None);
    let ctx = SampleContext::new(
        &tool,
        Some(&spans),
        &caps,
        OperationConfig::Profile(ProfileConfig::default())
            .cuts()
            .body_facts(),
    );
    // The stamper rule: a move of the entry plunge intent is transit by its
    // intent, with or without a span.
    let mut plunge = Toolpath::new();
    plunge.rapid_to(rs_cam_core::geo::P3::new(0.0, 0.0, 5.0));
    plunge.feed_to(rs_cam_core::geo::P3::new(0.0, 0.0, 0.0), 500.0);
    plunge.feed_to_with_intent(
        rs_cam_core::geo::P3::new(0.0, 0.0, -1.0),
        500.0,
        MoveIntent::EntryPlunge,
    );
    let flagged = rs_cam_core::trace::toolpath_spans::AnnotatedToolpath::new(plunge)
        .transit_moves_bitmap_from_intents();
    assert_eq!(
        flagged,
        vec![false, false, true],
        "the intent flags the plunge"
    );
    let sample = SimulationCutSample {
        move_index: 2,
        is_cutting: true,
        in_transit_span: true,
        span_path: vec![SpanId(0)],
        source_intent: Some(MoveIntent::EntryPlunge),
        engagement: SampleEngagement::with_radial_woc(0.5),
        removed_volume_est_mm3: 0.1,
        ..SimulationCutSample::test_fixture()
    };
    assert!(
        !in_span(Some(&spans), &sample.span_path, SpanKind::Entry),
        "in no entry span"
    );
    let cut = Cut::of_sample(&sample, &ctx).expect("removes material");
    assert_eq!(cut.kind, CutKind::Entry(EntryKind::Plunge));
    assert_eq!(judged_group(&sample, &ctx), Some(CutGroup::Entry));
}

/// Ruling Q6: the 2D Adaptive default entry, the helix, states its kind.
#[test]
fn an_adaptive_helix_sample_states_its_cut_kind() {
    let r = readout(&adaptive_session(), true);
    r.print("2D Adaptive, default helix entry");
    r.assert_claims("adaptive");
    r.assert_holds(
        "adaptive",
        &[CutKind::Body, CutKind::Entry(EntryKind::Helix)],
    );
}
