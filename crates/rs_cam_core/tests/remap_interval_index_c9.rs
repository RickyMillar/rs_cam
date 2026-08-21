//! C9 — interval index for `MoveRemap` span remapping.
//!
//! `MoveRemap::foreign_intrusion` used to linearly scan the ENTIRE
//! `old_to_new` vector for every span, and `MoveRemap::remap_range` made two
//! O(k) passes per span. On a wanaka-class job `old_to_new` is ~200k entries
//! with dozens-to-hundreds of spans, so `foreign_intrusion` alone was
//! O(spans × moves). `RemapIndex` (`toolpath_spans.rs`) builds a sparse
//! table (for `remap_range`) and a necessary-condition segment tree (for
//! `foreign_intrusion`) once per remap and answers both queries without
//! rescanning.
//!
//! This file is the property test (§1) and the wanaka-scale bench-as-test
//! (§2) against a hand-rolled xorshift64 PRNG oracle — `rs_cam_core` has no
//! `proptest`/`rand` dev-dependency (checked `Cargo.toml` before writing
//! this), so no dependency was added. §3 (identical-output regression
//! through the private `tsp::remap_spans`) lives in `tsp.rs`'s own
//! `mod tests`, since that function is private to the crate.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

use std::ops::Range;

use rs_cam_core::toolpath_spans::{MoveRemap, RemapIndex, Span, SpanKind};

// ── hand-rolled PRNG ────────────────────────────────────────────────────

/// xorshift64 — deterministic, dependency-free. Good enough for generating
/// diverse test fixtures; not a cryptographic or statistical RNG.
struct Xorshift64(u64);

impl Xorshift64 {
    fn new(seed: u64) -> Self {
        // xorshift64 is undefined at state 0 (it's a fixed point); nudge
        // away from it so every seed produces a real sequence.
        Self(if seed == 0 {
            0x9E37_79B9_7F4A_7C15
        } else {
            seed
        })
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// Uniform-ish value in `[0, bound)`. `0` for `bound == 0`.
    fn below(&mut self, bound: usize) -> usize {
        if bound == 0 {
            0
        } else {
            (self.next_u64() % bound as u64) as usize
        }
    }

    /// True with probability `pct / 100`.
    fn chance_pct(&mut self, pct: u64) -> bool {
        self.next_u64() % 100 < pct
    }
}

// ── MoveRemap generators ────────────────────────────────────────────────

#[derive(Clone, Copy)]
enum RemapShape {
    Identity,
    Permutation,
    Drops,
    Merges,
    Expansions,
    Empty,
    AllDropped,
    DegenerateMix,
    GenericRandom,
}

const ALL_SHAPES: [RemapShape; 9] = [
    RemapShape::Identity,
    RemapShape::Permutation,
    RemapShape::Drops,
    RemapShape::Merges,
    RemapShape::Expansions,
    RemapShape::Empty,
    RemapShape::AllDropped,
    RemapShape::DegenerateMix,
    RemapShape::GenericRandom,
];

/// Build one `MoveRemap` of the requested shape and (approximate) size `n`.
/// `RemapShape::Empty` always ignores `n` and produces a zero-length remap.
fn gen_remap(rng: &mut Xorshift64, shape: RemapShape, n: usize) -> MoveRemap {
    match shape {
        RemapShape::Empty => MoveRemap {
            old_to_new: Vec::new(),
        },
        RemapShape::Identity => MoveRemap::identity(n),
        RemapShape::AllDropped => MoveRemap {
            old_to_new: vec![None; n],
        },
        RemapShape::Permutation => {
            // Fisher-Yates shuffle of a 1:1 assignment — every old index
            // survives, but scattered across the new index space, which is
            // exactly the shape TSP reordering produces.
            let mut order: Vec<usize> = (0..n).collect();
            for i in (1..n).rev() {
                let j = rng.below(i + 1);
                order.swap(i, j);
            }
            MoveRemap {
                old_to_new: order.into_iter().map(|p| Some(p..p + 1)).collect(),
            }
        }
        RemapShape::Drops => {
            let mut next_new = 0usize;
            let mut out = Vec::with_capacity(n);
            for _ in 0..n {
                if rng.chance_pct(30) {
                    out.push(None);
                } else {
                    out.push(Some(next_new..next_new + 1));
                    next_new += 1;
                }
            }
            MoveRemap { old_to_new: out }
        }
        RemapShape::Merges => {
            // Several consecutive old indices map onto the SAME new range —
            // the shape a collapse (arc-fit, segment-merge) produces.
            let mut out = Vec::with_capacity(n);
            let mut new_pos = 0usize;
            let mut i = 0usize;
            while i < n {
                let block = 1 + rng.below(4);
                let width = 1 + rng.below(3);
                let r = new_pos..new_pos + width;
                for _ in 0..block {
                    if i >= n {
                        break;
                    }
                    out.push(Some(r.clone()));
                    i += 1;
                }
                new_pos += width;
            }
            MoveRemap { old_to_new: out }
        }
        RemapShape::Expansions => {
            // One old index fans out into a multi-move new range — the
            // shape a dogbone / entry-ramp insertion produces.
            let mut out = Vec::with_capacity(n);
            let mut new_pos = 0usize;
            for _ in 0..n {
                let width = 1 + rng.below(5);
                out.push(Some(new_pos..new_pos + width));
                new_pos += width;
            }
            MoveRemap { old_to_new: out }
        }
        RemapShape::DegenerateMix => {
            let mut out = Vec::with_capacity(n);
            let mut new_pos = 0usize;
            for _ in 0..n {
                let roll = rng.below(10);
                if roll < 2 {
                    out.push(None);
                } else if roll < 5 {
                    // Degenerate slot: start == end.
                    out.push(Some(new_pos..new_pos));
                } else {
                    let width = 1 + rng.below(3);
                    out.push(Some(new_pos..new_pos + width));
                    new_pos += width;
                }
            }
            MoveRemap { old_to_new: out }
        }
        RemapShape::GenericRandom => {
            // No structural assumption at all: every slot independently
            // dropped, degenerate, or an arbitrary (possibly overlapping,
            // possibly out-of-order) range. `MoveRemap` carries no
            // invariant that ranges are sorted or monotonic, so the index
            // must be correct against this too.
            let new_bound = (n * 2 + 5).max(1);
            let out = (0..n)
                .map(|_| {
                    if rng.chance_pct(15) {
                        None
                    } else {
                        let a = rng.below(new_bound);
                        let extra = rng.below(4);
                        Some(a..a + extra)
                    }
                })
                .collect();
            MoveRemap { old_to_new: out }
        }
    }
}

fn gen_spans(rng: &mut Xorshift64, n: usize) -> Vec<Span> {
    let kinds = [
        SpanKind::Operation,
        SpanKind::DepthPass,
        SpanKind::Region,
        SpanKind::Entry,
        SpanKind::LeadOut,
    ];
    let count = rng.below(6);
    (0..count)
        .map(|_| {
            let kind = kinds[rng.below(kinds.len())];
            if rng.chance_pct(20) {
                let pos = rng.below(n + 2);
                Span::boundary(pos, kind)
            } else {
                let a = rng.below(n + 2);
                let b = rng.below(n + 2);
                Span::new(a.min(b), a.max(b), kind)
            }
        })
        .collect()
}

// ── brute-force oracle (independent of MoveRemap's own implementation) ──

/// Ground truth for [`RemapIndex::remap_range`], reimplemented directly from
/// [`MoveRemap::remap_range`]'s documented contract rather than by calling
/// it, so a shared bug in the production two-pass scan couldn't hide from
/// this test by also being copied into the oracle.
fn oracle_remap_range(remap: &MoveRemap, start: usize, end: usize) -> Option<Range<usize>> {
    if start > end {
        return None;
    }
    let mut min_start: Option<usize> = None;
    let mut max_end: Option<usize> = None;
    for i in start..end {
        if let Some(Some(r)) = remap.old_to_new.get(i) {
            min_start = Some(min_start.map_or(r.start, |m| m.min(r.start)));
            max_end = Some(max_end.map_or(r.end, |m| m.max(r.end)));
        }
    }
    match (min_start, max_end) {
        (Some(s), Some(e)) => Some(s..e),
        _ => None,
    }
}

/// Ground truth for [`RemapIndex::foreign_intrusion`], reimplemented
/// directly from [`MoveRemap::foreign_intrusion`]'s documented contract.
fn oracle_foreign_intrusion(
    remap: &MoveRemap,
    old_start: usize,
    old_end: usize,
    bounds: &Range<usize>,
) -> Option<(usize, Range<usize>)> {
    for (i, slot) in remap.old_to_new.iter().enumerate() {
        let outside = i < old_start || i >= old_end;
        if let Some(r) = slot
            && outside
            && r.start < r.end
            && r.start < bounds.end
            && r.end > bounds.start
        {
            return Some((i, r.clone()));
        }
    }
    None
}

/// Ground truth for `MoveRemap::remap_spans` — same shape as
/// `MoveRemap::remap_span`, but built on [`oracle_remap_range`] instead of
/// the production method, so the wired-up `remap_spans` (which now builds a
/// `RemapIndex` internally) is checked against an independent computation.
/// `remap_boundary` is reused unmodified: it is out of scope for C9 (see
/// `RemapIndex`'s doc comment) and was not touched by this change.
fn oracle_remap_span(remap: &MoveRemap, span: &Span, new_n_moves: usize) -> Option<Span> {
    let mut new_span = if span.is_boundary() {
        let new_pos = remap.remap_boundary(span.start_move, new_n_moves);
        Span::new(new_pos, new_pos, span.kind)
    } else {
        let r = oracle_remap_range(remap, span.start_move, span.end_move)?;
        Span::new(r.start, r.end, span.kind)
    }
    .with_label(span.label.clone());
    if let Some(p) = span.payload.clone() {
        new_span = new_span.with_payload(p);
    }
    Some(new_span)
}

fn oracle_remap_spans(remap: &MoveRemap, spans: &[Span], new_n_moves: usize) -> Vec<Span> {
    spans
        .iter()
        .filter_map(|s| oracle_remap_span(remap, s, new_n_moves))
        .collect()
}

// ── §1: property test ───────────────────────────────────────────────────

#[test]
fn property_indexed_queries_match_brute_force_oracle() {
    let mut rng = Xorshift64::new(0xC9C9_C9C9_1234_5678);

    for iter in 0..2000usize {
        let shape = ALL_SHAPES[iter % ALL_SHAPES.len()];
        let n = if matches!(shape, RemapShape::Empty) {
            0
        } else {
            rng.below(201)
        };
        let remap = gen_remap(&mut rng, shape, n);
        let index = RemapIndex::build(&remap);
        let new_n = remap
            .old_to_new
            .iter()
            .flatten()
            .map(|r| r.end)
            .max()
            .unwrap_or(0);

        for probe in 0..8usize {
            // remap_range: mostly start<=end queries, occasionally inverted
            // (start>end) to exercise the "None, unconditionally" branch.
            let a = rng.below(n + 5);
            let b = rng.below(n + 5);
            let (lo, hi) = (a.min(b), a.max(b));
            let (qs, qe) = if rng.chance_pct(10) {
                (hi, lo)
            } else {
                (lo, hi)
            };

            let expected = oracle_remap_range(&remap, qs, qe);
            let got = index.remap_range(qs, qe);
            assert_eq!(
                got,
                expected,
                "remap_range mismatch: shape idx={} n={n} iter={iter} probe={probe} \
                 start={qs} end={qe}",
                iter % ALL_SHAPES.len(),
            );

            // foreign_intrusion: random exclusion window + random bounds.
            let old_start = rng.below(n + 3);
            let old_end = rng.below(n + 3);
            let ba = rng.below(new_n + 5);
            let bb = rng.below(new_n + 5);
            let bounds = ba.min(bb)..ba.max(bb);

            let expected_fi = oracle_foreign_intrusion(&remap, old_start, old_end, &bounds);
            let got_fi = index.foreign_intrusion(old_start, old_end, &bounds);
            assert_eq!(
                got_fi,
                expected_fi,
                "foreign_intrusion mismatch: shape idx={} n={n} iter={iter} probe={probe} \
                 old_start={old_start} old_end={old_end} bounds={bounds:?}",
                iter % ALL_SHAPES.len(),
            );
        }

        // Wired-through MoveRemap::remap_spans (now index-backed internally)
        // against the independent oracle recombination.
        let spans = gen_spans(&mut rng, n);
        let expected_spans = oracle_remap_spans(&remap, &spans, new_n);
        let got_spans = remap.remap_spans(&spans, new_n);
        assert_eq!(
            got_spans,
            expected_spans,
            "remap_spans mismatch: shape idx={} n={n} iter={iter}",
            iter % ALL_SHAPES.len(),
        );
    }
}

// ── §2: wanaka-scale bench-as-test ──────────────────────────────────────

/// A `MoveRemap` + span list shaped like TSP's output on a wanaka-class job:
/// 200_000 moves split into 250 contiguous blocks, the block ORDER permuted
/// (intra-block order preserved — TSP reorders segments, not individual
/// moves), one `Operation` span wrapping everything, a boundary span at
/// every 25th block start (cheap in both paths — boundary spans skip the
/// intrusion check entirely), and one `Region` span per block (250 of
/// them) standing in for the region-node spans that pay the O(moves)
/// `foreign_intrusion` scan pre-C9.
///
/// Every block gets a `Region` span rather than "a handful", deliberately:
/// the index's build cost is a FIXED O(n log n) charge paid once per
/// `RemapIndex::build`, independent of how many spans query it afterward.
/// Worked through by hand before writing this (no `cargo bench` available
/// to check empirically under the no-cargo rule this file was written
/// under): at n = 200_000 the one-time build is roughly-comparable in
/// raw operation count to ~10-25 full O(n) linear scans, so with only a
/// "handful" (single digits to ~20) of `foreign_intrusion`-triggering
/// spans the build cost is NOT reliably amortized and a 5x margin is not
/// a safe bet. With all 250 blocks paying the scan, the linear-scan total
/// (250 × O(n)) dwarfs the fixed build cost by roughly an order of
/// magnitude, which is also the more representative shape for a real
/// wanaka job — the codebase's own history notes a single TSP reorder
/// carrying a "200 924-move region node" through this exact check, and
/// `AnnotatedToolpath` doc comments describe wanaka-class span counts as
/// "dozens-to-hundreds", not single digits.
fn build_wanaka_scale_fixture(rng: &mut Xorshift64) -> (MoveRemap, Vec<Span>) {
    const N: usize = 200_000;
    const BLOCKS: usize = 250;
    const BLOCK_SIZE: usize = N / BLOCKS;

    let mut block_order: Vec<usize> = (0..BLOCKS).collect();
    for i in (1..BLOCKS).rev() {
        let j = rng.below(i + 1);
        block_order.swap(i, j);
    }
    // new_start_for_block[old_block] = where that block's content starts in
    // the new (post-reorder) move list.
    let mut new_start_for_block = vec![0usize; BLOCKS];
    let mut cursor = 0usize;
    for &old_block in &block_order {
        new_start_for_block[old_block] = cursor;
        cursor += BLOCK_SIZE;
    }

    let mut old_to_new = Vec::with_capacity(N);
    let mut spans = vec![Span::new(0, N, SpanKind::Operation)];

    for (block, &new_block_start) in new_start_for_block.iter().enumerate().take(BLOCKS) {
        let old_block_start = block * BLOCK_SIZE;
        let old_block_end = old_block_start + BLOCK_SIZE;
        for offset in 0..BLOCK_SIZE {
            let new_idx = new_block_start + offset;
            old_to_new.push(Some(new_idx..new_idx + 1));
        }
        if block % 25 == 0 {
            spans.push(Span::boundary(old_block_start, SpanKind::RapidOrderBarrier));
        }
        spans.push(
            Span::new(old_block_start, old_block_end, SpanKind::Region)
                .with_label(format!("region-{block}")),
        );
    }

    (MoveRemap { old_to_new }, spans)
}

/// Today's un-indexed per-span scan (`MoveRemap::remap_span` +
/// `MoveRemap::foreign_intrusion`, both left byte-for-byte unchanged by
/// C9) vs the `RemapIndex`-backed path, on a wanaka-scale fixture. Not
/// `#[ignore]`d: the deliberately-slow old path (250 spans × one O(n) scan
/// each, repeated `REPS` times per round × `ROUNDS` rounds) is a few
/// seconds at most, nowhere near the ~20s budget; the indexed path is
/// comfortably faster still.
///
/// The wall-clock claim is a **median of `ROUNDS` paired rounds** — see the
/// comment at `ROUNDS` for why, and why it is median-of-ratios. The
/// output-matching half of this test is not timing-dependent and runs on
/// every round.
#[test]
fn wanaka_scale_indexed_path_beats_linear_scan_and_matches_output() {
    let mut rng = Xorshift64::new(0xA5A5_1357_ABCD_EF01);
    let (remap, spans) = build_wanaka_scale_fixture(&mut rng);
    let new_n = remap
        .old_to_new
        .iter()
        .flatten()
        .map(|r| r.end)
        .max()
        .unwrap_or(0);

    // 250 spans × O(n) per span on the linear path is already ~50M checks
    // for a single pass; keep REPS small so the (deliberately slow) old
    // path stays well inside the ~20s budget while still giving a stable
    // timing signal.
    const REPS: u32 = 5;

    // The ≥5× bar below used to compare two SINGLE measurements, and this
    // test flaked on three separate sessions because of it — most recently
    // `DELTA_sim_w6_playback.md` §3e, which recorded 4.66× then 5.3× on an
    // immediate re-run, with the movement entirely in the LINEAR reference
    // arm (158.9 → 190.5 ms) while the indexed arm held at ~35 ms.
    //
    // Change (user-approved 2026-08-21): keep the bar at 5×, take the
    // MEDIAN of 3 full rounds. Median-**of-ratios**, not ratio-of-medians:
    // each round measures both arms back-to-back under the same machine
    // conditions, so a ratio is a paired measurement and the median throws
    // away the whole contended round. Ratio-of-medians would happily divide
    // a contended linear median by a clean indexed median (or the reverse)
    // and manufacture a number no round actually observed. The output
    // matching below is unchanged and still runs every round.
    const ROUNDS: usize = 3;

    // WHAT THE MEDIAN THEN REVEALED, and it is not noise. Measured
    // 2026-08-21 on this fixture, three process runs each:
    //
    // | profile | per-round speedups | median | vs the 5× bar |
    // |---|---|---|---|
    // | debug   | 12.68 – 12.94 | **12.9×** | passes, every run |
    // | release |  3.92 –  4.75 |  **4.3×** | FAILS, every run |
    //
    // The bar was calibrated in DEBUG. The landing commit (`ef4011cc`,
    // 2026-08-03) recorded "linear scan 4.44 s, indexed 324 ms, 13.7x" —
    // seconds-scale timings this same fixture only produces unoptimised,
    // and today's debug build reproduces them (3.08 s / 240 ms / 12.9×).
    // 5× was a margin under 13.7×, and `cargo test -p rs_cam_core` without
    // `--release` is what it was checked against.
    //
    // In release both arms get much faster but the LINEAR reference arm
    // gains far more — a flat scan vectorises; the index's tree descent is
    // pointer-chasing that does not — so the ratio collapses to ~4.3×. The
    // "flake" was never machine noise: it is one bar being read under two
    // build profiles whose honest answers differ by 3×, and the release
    // answer sits just under it. Before the median, release passed
    // occasionally on the tail (1 of 5 runs of the unmodified test today:
    // 4.4, 4.9, 4.7, 5.3, 4.4); after it, release fails deterministically.
    //
    // The bar is DELIBERATELY LEFT AT 5×. Moving it is a decision about
    // what this test claims, and it belongs to the operator, not to the
    // lane that happened to measure it. The options on the table are: lower
    // it to ~4× so one number holds in both profiles; make it
    // profile-aware via `cfg!(debug_assertions)`; or pin the test to one
    // profile. `RemapIndex` itself is NOT regressed — both arms are far
    // faster than they were in August, and the index still wins by 4.3× in
    // the profile that ships.

    // Both arms stay INLINE in the round loop rather than being hoisted
    // into two closures. A first draft used closures and read consistently
    // lower (medians 4.13× vs 4.3×) — not cleanly attributable, since the
    // two drafts were not measured under the same machine load, which is
    // exactly why the number is not quoted as a result. The point is that
    // the timed region should stay the straight-line body the original
    // single-shot test timed, so the duplicated `mut` accumulators are the
    // cheaper price.
    let mut ratios: Vec<f64> = Vec::with_capacity(ROUNDS);
    let mut heap_bytes = 0usize;
    for round in 1..=ROUNDS {
        let mut old_spans = Vec::new();
        let old_start = std::time::Instant::now();
        for _ in 0..REPS {
            old_spans = spans
                .iter()
                .filter_map(|s| {
                    let new_span = remap.remap_span(s, new_n)?;
                    if !s.is_boundary() && s.kind != SpanKind::Operation {
                        let bounds = new_span.start_move..new_span.end_move;
                        if remap
                            .foreign_intrusion(s.start_move, s.end_move, &bounds)
                            .is_some()
                        {
                            return None;
                        }
                    }
                    Some(new_span)
                })
                .collect();
        }
        let old_elapsed = old_start.elapsed();

        let mut new_spans = Vec::new();
        let new_start = std::time::Instant::now();
        for _ in 0..REPS {
            let index = RemapIndex::build(&remap);
            heap_bytes = index.heap_bytes();
            new_spans = spans
                .iter()
                .filter_map(|s| {
                    let new_span = remap.remap_span_with_index(s, new_n, &index)?;
                    if !s.is_boundary() && s.kind != SpanKind::Operation {
                        let bounds = new_span.start_move..new_span.end_move;
                        if index
                            .foreign_intrusion(s.start_move, s.end_move, &bounds)
                            .is_some()
                        {
                            return None;
                        }
                    }
                    Some(new_span)
                })
                .collect();
        }
        let new_elapsed = new_start.elapsed();

        assert_eq!(
            old_spans, new_spans,
            "indexed path must produce byte-for-byte the same spans as the linear scan"
        );

        let speedup = old_elapsed.as_secs_f64() / new_elapsed.as_secs_f64().max(1e-12);
        println!(
            "C9 remap interval index — round {round}/{ROUNDS}: linear scan = {old_elapsed:?}, \
             indexed (incl. {REPS} index builds) = {new_elapsed:?}, speedup = {speedup:.2}x"
        );
        ratios.push(speedup);
    }

    let mut sorted = ratios.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).expect("no NaN speedups"));
    let speedup = sorted[ROUNDS / 2];

    println!(
        "C9 remap interval index — wanaka-scale fixture ({} moves, {} spans, {REPS} reps × \
         {ROUNDS} rounds): per-round speedups = {ratios:?}, MEDIAN speedup = {speedup:.1}x, \
         index heap footprint = {heap_bytes} bytes ({:.2} MB)",
        remap.old_to_new.len(),
        spans.len(),
        heap_bytes as f64 / (1024.0 * 1024.0),
    );

    assert!(
        speedup >= 5.0,
        "indexed path should be at least 5x faster than the linear scan on a wanaka-scale \
         fixture: median-of-{ROUNDS} speedup={speedup:.2}x, per-round speedups={ratios:?}"
    );

    // Memory regression guard: an earlier revision of `RemapIndex` answered
    // `remap_range` with a second, independent O(n log n) sparse table even
    // though the intrusion tree already carried the same (min_start,
    // max_end) aggregate — ~67 MB of transient allocation on this exact
    // fixture. The rewrite (two `AggregateTree`s: an unpadded O(n) tree for
    // `remap_range`'s O(log n) range query, the existing padded O(n) tree
    // for `foreign_intrusion`'s descent, no separate raw slot arrays) should
    // land comfortably under 16 MB at n = 200_000. Assert it stays there so
    // the duplication can't come back silently.
    const MAX_HEAP_BYTES: usize = 16 * 1024 * 1024;
    assert!(
        heap_bytes < MAX_HEAP_BYTES,
        "index heap footprint regressed: {heap_bytes} bytes ({:.2} MB) >= {:.0} MB budget",
        heap_bytes as f64 / (1024.0 * 1024.0),
        MAX_HEAP_BYTES as f64 / (1024.0 * 1024.0),
    );
}
