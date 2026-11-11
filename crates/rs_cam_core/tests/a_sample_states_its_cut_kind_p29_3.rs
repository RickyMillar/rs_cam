//! Feeds contract P29-3, commit C
//! (`planning/feeds_contract_2026-11-03/phases/P29-3_DESIGN.md` section 3):
//! a simulation sample states its cut kind, and the kind tells which
//! samples a gate judges today.
//!
//! Four small fixtures, generated through the session and simulated: a
//! contour Pocket with a ramp entry (the slot ring), the same Pocket with
//! dogbones (the dressup relief), a Profile with two tabs and leads (no
//! entry dressup, so the tab re-plunges stay plunges), and the Geodesic
//! Adaptive 3D Rough of the D65 shelf (capped steps, the waterline cleanup,
//! a planner ramp entry).
//!
//! - Claim 1: `Cut::of_sample` gives `None` for an air sample and only for
//!   one (`SimulationCutSample::is_air`).
//! - Claim 2: for every other sample,
//!   `is_steady_state_for_gate(sample, lookup) == cut.kind.is_judged_today()`,
//!   with the span lookup of the gates (ruling Q10: the span filter states
//!   today's behaviour).
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
//!   that is not air; the row `Q10-LinkingInEntrySpan` has samples on the
//!   shelf; the row `Q4-SlotRingInferred` has samples on the Pocket; the row
//!   `Q4-CappedStepNoTrace` has samples when the shelf is read with no
//!   semantic trace.
//! - The row `Q10-EntryPlungeInNoSpan` has no sample in these fixtures (the
//!   tab re-plunges of the Profile lie in an entry span). A synthetic sample
//!   holds the class: the stamper flags an entry plunge `in_transit_span`
//!   by its intent, so the filter drops it with no entry span.
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
use std::sync::atomic::AtomicBool;

use common::meshes::height_field;
use common::op_fixtures::{flat_stock, outline_model};
use common::session::{mesh_model, single_op_session_with, stock_over};
use common::tools::endmill_tool_config;

use rs_cam_core::adaptive3d::FeedCapSource;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{DogboneParams, DressupEntryStyle, LeadParams};
use rs_cam_core::compute::cutter::build_cutter;
use rs_cam_core::compute::operation_configs::{
    Adaptive3dConfig, Adaptive3dEntryStyle, ClearingStrategy, PocketConfig, PocketPattern,
    ProfileConfig,
};
use rs_cam_core::feeds::engagement::Engagement;
use rs_cam_core::limits::{
    CapTable, CleanupKind, Cut, CutEngagement, CutFacts, CutKind, CutSource, EntryKind,
    NoEngagementModel, SampleContext, SampleDepth,
};
use rs_cam_core::session::{ProjectSession, SimulationOptions, SimulationResolution};
use rs_cam_core::stock::simulation_cut::{Engagement as SampleEngagement, SimulationCutSample};
use rs_cam_core::tool::ToolDefinition;
use rs_cam_core::tool_load::locality::{SpanLookup, is_configured_entry, is_steady_state_for_gate};
use rs_cam_core::toolpath::{MoveIntent, Toolpath};
use rs_cam_core::trace::semantic_trace::{
    SemanticKey, ToolpathSemanticKind, ToolpathSemanticTrace,
};
use rs_cam_core::trace::toolpath_spans::{Span, SpanId, SpanKind};

// ── The fixtures ────────────────────────────────────────────────────────

/// The simulation cell of the flat fixtures (mm): the cell of
/// `common::op_fixtures::SIM_CELL_MM`.
const FLAT_CELL: f64 = common::op_fixtures::SIM_CELL_MM;
/// The tool diameter of every fixture (mm).
const TOOL_D: f64 = 6.0;

fn generate_and_simulate(session: &mut ProjectSession, cell: f64) {
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("the op generates");
    session
        .run_simulation(
            &SimulationOptions {
                resolution: SimulationResolution::Fixed(cell),
                adaptive_feed_modulation: false,
                ..SimulationOptions::default()
            },
            &cancel,
        )
        .expect("the session simulates");
}

/// A contour Pocket of the outline, with a ramp entry. `dogbone`: the
/// dogbone dressup at 90 degrees cuts a relief at each inside corner.
fn pocket_session(dogbone: bool) -> ProjectSession {
    let op = OperationConfig::Pocket(PocketConfig {
        pattern: PocketPattern::Contour,
        ..PocketConfig::default()
    });
    let mut session = single_op_session_with(
        flat_stock(),
        endmill_tool_config(TOOL_D),
        outline_model(),
        "pocket",
        op,
        |tc| {
            tc.dressups.entry_style = DressupEntryStyle::Ramp;
            if dogbone {
                tc.dressups.dogbone = Some(DogboneParams { angle: 90.0 });
            }
        },
    );
    generate_and_simulate(&mut session, FLAT_CELL);
    session
}

/// An outside Profile of the outline with tabs and leads. The entry
/// dressup is off: a ramp entry turns the tab re-plunges into ramps.
fn profile_session() -> ProjectSession {
    let op = OperationConfig::Profile(ProfileConfig {
        tab_count: 4,
        ..ProfileConfig::default()
    });
    let mut session = single_op_session_with(
        flat_stock(),
        endmill_tool_config(TOOL_D),
        outline_model(),
        "profile",
        op,
        |tc| {
            tc.dressups.entry_style = DressupEntryStyle::None;
            tc.dressups.lead_in_out = Some(LeadParams {
                radius: TOOL_D / 3.0,
                in_feed_rate: None,
                out_feed_rate: None,
            });
        },
    );
    generate_and_simulate(&mut session, FLAT_CELL);
    session
}

// The shelf pocket of the D65 sentry
// (`a_geodesic_step_over_the_ceiling_is_cut_capped_d65`), as P22a builds it
// (`every_wide_step_has_a_named_source_p22a`).
const TOP_Z: f64 = 10.0;
const FLOOR_Z: f64 = 4.0;
const MESH_HALF: f64 = 40.0;
const MESH_STEP: f64 = 0.5;
const POCKET_HALF_X: f64 = 34.0;
const POCKET_HALF_Y: f64 = 24.0;
const ISLAND_RADIUS: f64 = 5.0;
const SHELF_Z: f64 = 6.5;
const SHELF_STEPOVER: f64 = 2.0;
/// The simulation cell of the shelf (mm), P22a arm 1.
const SHELF_CELL: f64 = 0.25;

fn shelf_session() -> ProjectSession {
    let mesh = height_field(MESH_HALF, MESH_STEP, |x, y| {
        if x.hypot(y) <= ISLAND_RADIUS {
            TOP_Z
        } else if (12.0..=30.0).contains(&x) && y.abs() <= 12.0 {
            SHELF_Z
        } else if x.abs() <= POCKET_HALF_X && y.abs() <= POCKET_HALF_Y {
            FLOOR_Z
        } else {
            TOP_Z
        }
    });
    let op = OperationConfig::Adaptive3d(Adaptive3dConfig {
        stepover: Some(SHELF_STEPOVER),
        depth_per_pass: Some(TOP_Z - FLOOR_Z),
        clearing_strategy: ClearingStrategy::GeodesicAdaptive,
        entry_style: Adaptive3dEntryStyle::Ramp,
        ..Adaptive3dConfig::default()
    });
    let mut session = single_op_session_with(
        stock_over(MESH_HALF, TOP_Z),
        endmill_tool_config(TOOL_D),
        mesh_model(mesh, "shelf"),
        "shelf rough",
        op,
        |_| {},
    );
    generate_and_simulate(&mut session, SHELF_CELL);
    session
}

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
    air: usize,
    /// The samples that are not air, by kind.
    kinds: BTreeMap<String, usize>,
    /// Air samples that the span filter calls steady (each gate adds its
    /// own air test).
    air_steady: usize,
    /// Row `Q10-LinkingInEntrySpan`: `Linking` samples, not air, with an
    /// entry span.
    linking_in_entry_span: usize,
    /// Row `Q10-EntryPlungeInNoSpan`: entry plunges, not air, in no entry
    /// span.
    entry_plunge_in_no_span: usize,
    /// Row `Q4-SlotRingInferred`.
    slot_ring: usize,
    /// Row `Q4-CappedStepNoTrace`: capped samples, not air, read as the body
    /// with no trace.
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
            "\n## {name}: {} samples, {} air ({} of them steady by the filter)",
            self.samples, self.air, self.air_steady
        );
        for (k, n) in &self.kinds {
            eprintln!("{n:>7}  {k}");
        }
        eprintln!(
            "Q10-LinkingInEntrySpan {}, Q10-EntryPlungeInNoSpan {}, Q4-SlotRingInferred {}, \
             Q4-CappedStepNoTrace {}",
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
            "{name}: the kind and the span filter disagree (claim 2). A new class goes to the \
             lead before any arm: {:?}",
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
                "{name}: non-vacuity: no sample of kind {kind:?} that is not air: {:?}",
                self.kinds
            );
        }
    }
}

/// The readout of toolpath 0 of a simulated session. `with_trace: false`
/// reads the result as if it held no semantic trace (row
/// `Q4-CappedStepNoTrace`).
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
    // The lookup of the gates (`ProjectSession::load_context_parts`).
    let spans = annotated.spans_valid.then_some(annotated.spans.as_slice());
    let lookup = spans.map(SpanLookup::new);
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
        let filter = is_steady_state_for_gate(s, lookup.as_ref());
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
        if cut.is_some() == s.is_air() {
            out.wrong_cut.push(format!(
                "air {} but cut {:?}: {}",
                s.is_air(),
                cut.is_some(),
                example(s)
            ));
        }
        let Some(cut) = cut else {
            out.air += 1;
            if filter {
                out.air_steady += 1;
            }
            continue;
        };
        check_cut(&cut, s, &tool, kind, facts, &mut out.wrong_cut);
        *out.kinds.entry(format!("{kind:?}")).or_default() += 1;
        if cut.kind.is_judged_today() != filter {
            let e = out
                .mismatches
                .entry(format!("{kind:?} judged {} filter {filter}", !filter))
                .or_insert_with(|| (0, example(s)));
            e.0 += 1;
        }
        let entry_span = is_configured_entry(s, lookup.as_ref());
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
    assert!(r.slot_ring > 0, "non-vacuity: row Q4-SlotRingInferred");
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
        "non-vacuity: row Q10-LinkingInEntrySpan"
    );

    // Row Q4-CappedStepNoTrace: with no semantic trace, a capped move is
    // the body, and the equality still holds.
    let bare = readout(&session, false);
    bare.print("Geodesic 3D Rough, D65 shelf, read with no semantic trace");
    bare.assert_claims("shelf, no trace");
    assert!(
        bare.capped_no_trace > 0,
        "non-vacuity: row Q4-CappedStepNoTrace"
    );
    assert_eq!(bare.slot_ring, 0, "no trace: no slot ring is inferred");
}

/// Row `Q10-EntryPlungeInNoSpan` (no fixture sample): an entry plunge in no
/// entry span. The stamper flags it in transit by its intent
/// (`AnnotatedToolpath::transit_moves_bitmap_from_intents`), so the filter
/// drops it as phantom transit; the kind is an entry, not judged.
#[test]
fn an_entry_plunge_in_no_span_is_not_judged() {
    let spans = [
        Span::new(0, 4, SpanKind::Operation),
        Span::new(0, 1, SpanKind::Entry),
    ];
    let lookup = SpanLookup::new(&spans);
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
        ..SimulationCutSample::test_fixture()
    };
    assert!(
        !is_configured_entry(&sample, Some(&lookup)),
        "in no entry span"
    );
    let cut = Cut::of_sample(&sample, &ctx).expect("not air");
    assert_eq!(cut.kind, CutKind::Entry(EntryKind::Plunge));
    assert_eq!(
        is_steady_state_for_gate(&sample, Some(&lookup)),
        cut.kind.is_judged_today()
    );
    assert!(!cut.kind.is_judged_today(), "an entry is not judged today");
}
