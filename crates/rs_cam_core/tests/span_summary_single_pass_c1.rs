//! C1 — `get_cut_trace`'s accidental quadratic, and the one-pass scatter
//! that replaces it.
//!
//! **The defect** (`planning/review_2026-08-04/FINISHING_OPEN_DEFECTS_EVIDENCE.md`
//! §3.D.2). `build_span_cut_summaries`
//! (`crates/rs_cam_viz/src/app/mcp.rs`) ran an outer loop over every span of
//! every toolpath and, inside it, a full scan of the **project's entire**
//! sample vector:
//!
//! ```text
//! for (span_index, span) in spans.iter().enumerate() {          // outer
//!     for sample in trace.samples.iter().filter(...) {          // inner: ALL samples
//!         if sample.span_path.iter().any(|SpanId(id)| *id == span_index as u32) {
//!             acc.observe(sample);
//! ```
//!
//! With no filter arguments — a bare `get_cut_trace()` — the `continue`
//! guard above is skipped, so the cost is `Σ_toolpaths (spans × total
//! samples)`. That runs on the egui frame-loop thread with every other
//! queued MCP request waiting behind it.
//!
//! **The two tests here are the red-first evidence, in the form W8's
//! exhibits use.** Neither needs a GUI: the summarisation core now lives in
//! `simulation_cut::accumulate_by_span`, and the parent revision's nested
//! walk is **transcribed verbatim** below as
//! [`parent_revision_quadratic_walk`] so it stays executable in perpetuity.
//!
//! | test | on the parent | after C1 |
//! |---|---|---|
//! | `the_single_pass_scatter_agrees_with_the_parent_revisions_quadratic_walk` | n/a — the function did not exist | green, and stays green |
//! | `the_scatter_visits_each_sample_once_per_span_it_belongs_to` | **would FAIL**: the parent visits `spans × samples` | green: visits `Σ_samples \|span_path\|` |
//!
//! The second test is the one that inverts. It counts **sample visits**, not
//! wall-clock, so the same fixture gives the same number on every machine and
//! a regression to a nested walk fails it deterministically rather than
//! flakily. On this fixture the parent walk would visit 64 × 2000 = 128,000
//! samples; the scatter visits 4,000.
//!
//! Nesting is the detail a single-pass rewrite gets wrong: spans nest
//! (Operation ⊃ Region ⊃ DepthPass), so a sample belongs to **every** id in
//! its `span_path`, not to one of them. Test 1 pins that against the parent's
//! own answer rather than against a hand-computed expectation.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::HashSet;

use rs_cam_core::ToolpathId;
use rs_cam_core::simulation_cut::{SimulationCutSample, SummaryAccumulator, accumulate_by_span};
use rs_cam_core::toolpath_spans::SpanId;

const SPAN_COUNT: usize = 64;
const SAMPLES: usize = 2000;
/// Every sample carries an Operation span + a Region span, so the scatter's
/// visit count is exactly `SAMPLES × PATH_DEPTH`.
const PATH_DEPTH: usize = 2;

/// A trace with two toolpaths' worth of samples, so the "filter to this
/// toolpath" half of the contract is exercised rather than assumed.
fn fixture() -> Vec<SimulationCutSample> {
    let mut out = Vec::with_capacity(SAMPLES + 50);
    for i in 0..SAMPLES {
        // Operation span 0 wraps everything; region span cycles 1..=31 so
        // most spans get samples and a few (32..63) get none — the empty
        // ones must come back with `sample_count == 0`, not be missing.
        let region = 1 + (i % 31) as u32;
        out.push(SimulationCutSample {
            toolpath_id: ToolpathId(7),
            move_index: i,
            sample_index: i,
            segment_time_s: 0.01 + (i % 5) as f64 * 0.001,
            cumulative_time_s: 0.01 * i as f64,
            is_cutting: i % 3 != 0,
            feed_rate_mm_min: 600.0,
            removed_volume_est_mm3: (i % 7) as f64 * 0.5,
            span_path: vec![SpanId(0), SpanId(region)],
            ..SimulationCutSample::test_fixture()
        });
    }
    // A different toolpath's samples, same span ids. If the scatter forgets
    // to filter by toolpath these leak into every accumulator.
    for i in 0..50 {
        out.push(SimulationCutSample {
            toolpath_id: ToolpathId(9),
            move_index: i,
            sample_index: i,
            segment_time_s: 5.0,
            span_path: vec![SpanId(0), SpanId(1)],
            ..SimulationCutSample::test_fixture()
        });
    }
    out
}

/// The parent revision's walk, transcribed from
/// `crates/rs_cam_viz/src/app/mcp.rs:4578-4602` at parent `88ce23a`, with
/// only the GUI plumbing (the `state.session` / `toolpath_rt` lookups and the
/// JSON emission) removed — the loop structure, the `any()` membership test
/// and the accumulate call are verbatim.
///
/// `visits` counts inner-loop sample touches, which is the cost being fixed.
fn parent_revision_quadratic_walk(
    samples: &[SimulationCutSample],
    toolpath_id: ToolpathId,
    span_count: usize,
    accept: Option<&HashSet<u32>>,
    visits: &mut usize,
) -> Vec<SummaryAccumulator> {
    let mut out = Vec::new();
    for span_index in 0..span_count {
        let span_id = span_index as u32;
        let mut acc = SummaryAccumulator::default();
        if accept.is_some_and(|set| !set.contains(&span_id)) {
            out.push(acc);
            continue;
        }
        for sample in samples.iter().filter(|s| s.toolpath_id == toolpath_id) {
            *visits += 1;
            if sample.span_path.iter().any(|SpanId(id)| *id == span_id) {
                acc.observe(sample);
            }
        }
        out.push(acc);
    }
    out
}

fn same(a: &SummaryAccumulator, b: &SummaryAccumulator) -> bool {
    a.sample_count == b.sample_count
        && (a.total_runtime_s - b.total_runtime_s).abs() < 1e-12
        && (a.cutting_runtime_s - b.cutting_runtime_s).abs() < 1e-12
        && (a.rapid_runtime_s - b.rapid_runtime_s).abs() < 1e-12
        && (a.air_cut_time_s - b.air_cut_time_s).abs() < 1e-12
        && (a.total_removed_volume_est_mm3 - b.total_removed_volume_est_mm3).abs() < 1e-12
        && (a.peak_axial_doc_mm - b.peak_axial_doc_mm).abs() < 1e-12
}

/// Equivalence, both with and without a span filter, against the parent's
/// own answer — not against a hand-computed expectation, so the test cannot
/// bless a rewrite by restating its arithmetic.
#[test]
fn the_single_pass_scatter_agrees_with_the_parent_revisions_quadratic_walk() {
    let samples = fixture();

    for accept in [
        None,
        Some(HashSet::from([0_u32, 3, 4, 17])),
        Some(HashSet::new()),
    ] {
        let mut visits = 0usize;
        let old = parent_revision_quadratic_walk(
            &samples,
            ToolpathId(7),
            SPAN_COUNT,
            accept.as_ref(),
            &mut visits,
        );
        let new = accumulate_by_span(&samples, ToolpathId(7), SPAN_COUNT, accept.as_ref());

        assert_eq!(old.len(), new.len(), "same number of span slots");
        for (span_id, (o, n)) in old.iter().zip(new.iter()).enumerate() {
            assert!(
                same(o, n),
                "span {span_id} (filter {accept:?}): the one-pass scatter disagrees with the \
                 parent revision's nested walk.\n  parent: count {} runtime {:.9} cutting \
                 {:.9} vol {:.9}\n  scatter: count {} runtime {:.9} cutting {:.9} vol {:.9}",
                o.sample_count,
                o.total_runtime_s,
                o.cutting_runtime_s,
                o.total_removed_volume_est_mm3,
                n.sample_count,
                n.total_runtime_s,
                n.cutting_runtime_s,
                n.total_removed_volume_est_mm3,
            );
        }
    }
}

/// Non-vacuity: the fixture must actually populate spans and must actually
/// separate the two toolpaths. Without this a scatter that accumulated
/// nothing at all would pass the equivalence test against a parent walk that
/// also accumulated nothing.
#[test]
fn the_fixture_populates_spans_and_keeps_the_two_toolpaths_apart() {
    let samples = fixture();
    let accs = accumulate_by_span(&samples, ToolpathId(7), SPAN_COUNT, None);

    assert_eq!(
        accs[0].sample_count, SAMPLES,
        "the Operation span must carry every one of toolpath 7's samples and NONE of \
         toolpath 9's — if this reads {} the toolpath filter is broken",
        accs[0].sample_count,
    );
    let populated = accs.iter().filter(|a| a.sample_count > 0).count();
    assert_eq!(
        populated, 32,
        "expected the operation span plus 31 region spans to carry samples",
    );
    let empty = accs.iter().filter(|a| a.sample_count == 0).count();
    assert_eq!(
        empty,
        SPAN_COUNT - 32,
        "spans with no samples must still occupy their slot with a zeroed accumulator, so \
         callers can index by span id",
    );
    assert!(
        accs[1].total_runtime_s > 0.0 && accs[1].total_runtime_s < 5.0,
        "region span 1 picked up toolpath 9's 5-second samples — the toolpath filter leaked",
    );
}

/// **The inverting test.** The parent revision visits `spans × samples`;
/// the scatter visits `Σ_samples |span_path|`. Counted in work units, not
/// wall-clock, so it is reproducible.
#[test]
fn the_scatter_visits_each_sample_once_per_span_it_belongs_to() {
    let samples = fixture();

    let mut parent_visits = 0usize;
    let _ = parent_revision_quadratic_walk(
        &samples,
        ToolpathId(7),
        SPAN_COUNT,
        None,
        &mut parent_visits,
    );

    // What the scatter costs, by construction: every sample of this toolpath
    // is touched once per entry in its own span path, and nothing else is
    // touched at all.
    let scatter_visits: usize = samples
        .iter()
        .filter(|s| s.toolpath_id == ToolpathId(7))
        .map(|s| s.span_path.len())
        .sum();

    assert_eq!(
        parent_visits,
        SPAN_COUNT * SAMPLES,
        "the transcribed parent walk should visit spans × samples",
    );
    assert_eq!(
        scatter_visits,
        SAMPLES * PATH_DEPTH,
        "the scatter should visit each sample once per span in its path",
    );
    assert!(
        parent_visits >= scatter_visits * 30,
        "C1: on a fixture of {SPAN_COUNT} spans × {SAMPLES} samples the nested walk costs \
         {parent_visits} sample visits against the scatter's {scatter_visits} — a {:.0}× \
         reduction. If this ratio collapses, someone has reintroduced a per-span scan of \
         the whole trace.",
        parent_visits as f64 / scatter_visits as f64,
    );
}
