//! S5 sentries — fixpoint prefix memoization (`PERF_REVIEW.md` S5,
//! `rs_cam_core::compute::sim_prefix`).
//!
//! Two properties, and neither is worth anything without the other:
//!
//! 1. **Bit-identity.** A run that resumes from a cached prefix must be
//!    indistinguishable from a full replay — same mesh, same checkpoints, same
//!    cut trace, same collisions, same `prior_stocks`, same everything a caller
//!    can read off `SimulationResult`. The comparison here is a bit-pattern
//!    fingerprint over the *whole* result with no tolerance anywhere, so a
//!    forgotten accumulator shows up as a hash mismatch rather than as a
//!    plausible-looking number.
//!
//! 2. **Non-vacuity.** A memo that never hits is a silent no-op. `DELTA_sim_w2.md`
//!    §2e is the precedent: S2's first `REFRESH_VISIT_MULTIPLIER` made the
//!    whole-stamp early-out fire **zero** times across the entire suite, and
//!    every test stayed green because a stale mip is *sound*. Every test below
//!    that expects a hit asserts on `SimPrefixStats`, so an inert cache fails
//!    red instead of passing quietly.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::sim_prefix::{SimMemo, SimPrefixCache};
use rs_cam_core::compute::simulate::{
    SimGroupEntry, SimToolpathEntry, SimulationRequest, SimulationResult, run_simulation,
    run_simulation_memoized,
};
use rs_cam_core::compute::tool_config::ToolMaterial;
use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
use rs_cam_core::geo::{BoundingBox3, P3};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::simulation_cut::SimulationMetricOptions;
use rs_cam_core::tool::{BallEndmill, FlatEndmill, ToolDefinition, VBitEndmill};
use rs_cam_core::toolpath::Toolpath;
use rs_cam_core::toolpath_spans::AnnotatedToolpath;

// ── Fixture ─────────────────────────────────────────────────────────────

fn stock() -> BoundingBox3 {
    BoundingBox3 {
        min: P3::new(0.0, 0.0, -6.0),
        max: P3::new(40.0, 24.0, 0.0),
    }
}

fn tool() -> ToolDefinition {
    ToolDefinition::new(
        Box::new(FlatEndmill::new(6.0, 25.0)),
        6.0,
        20.0,
        25.0,
        45.0,
        2,
        ToolMaterial::Carbide,
    )
}

/// A raster pass at depth `-z_depth`, offset in Y so each op cuts fresh
/// material as well as re-passing its predecessor's ground.
fn pass(pass_index: usize) -> Arc<AnnotatedToolpath> {
    let y0 = 4.0 + 3.0 * pass_index as f64;
    let depth = -1.0 - 0.8 * pass_index as f64;
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(3.0, y0, 5.0));
    for lane in 0..3 {
        let y = y0 + 2.0 * lane as f64;
        let (x_a, x_b) = if lane % 2 == 0 {
            (3.0, 37.0)
        } else {
            (37.0, 3.0)
        };
        tp.feed_to(P3::new(x_a, y, depth), 900.0);
        tp.feed_to(P3::new(x_b, y, depth), 900.0);
    }
    tp.rapid_to(P3::new(37.0, y0, 5.0));
    Arc::new(AnnotatedToolpath::new(tp))
}

fn entry(index: usize, annotated: &Arc<AnnotatedToolpath>) -> SimToolpathEntry {
    SimToolpathEntry {
        id: ToolpathId(index + 1),
        name: format!("Pass{index}"),
        annotated: Arc::clone(annotated),
        tool: tool(),
        flute_count: 2,
        tool_summary: "6mm Flat".to_owned(),
        semantic_trace: None,
        spindle_rpm: None,
        metrics_not_applicable: false,
        drill_op: None,
        operation_config_hash: index as u64,
    }
}

/// A request over the first `count` passes of `chain`.
fn request(chain: &[Arc<AnnotatedToolpath>], count: usize) -> SimulationRequest {
    SimulationRequest {
        groups: vec![SimGroupEntry {
            toolpaths: chain
                .iter()
                .take(count)
                .enumerate()
                .map(|(i, tp)| entry(i, tp))
                .collect(),
            direction: StockCutDirection::FromTop,
            local_stock_bbox: None,
            local_to_global: None,
            phantom_prior_stock: None,
        }],
        stock_bbox: stock(),
        stock_top_z: 0.0,
        resolution: 0.5,
        metric_options: SimulationMetricOptions {
            enabled: true,
            capture_arc_engagement: true,
        },
        spindle_rpm: 18_000,
        rapid_feed_mm_min: 5000.0,
        model_mesh: None,
        kinematics: None,
    }
}

fn chain(len: usize) -> Vec<Arc<AnnotatedToolpath>> {
    (0..len).map(pass).collect()
}

fn run(req: &SimulationRequest) -> SimulationResult {
    run_simulation(req, &AtomicBool::new(false)).expect("simulation")
}

fn run_memo(req: &SimulationRequest, cache: &mut SimPrefixCache, store: bool) -> SimulationResult {
    run_simulation_memoized(
        req,
        &AtomicBool::new(false),
        |_phase| {},
        Some(SimMemo { cache, store }),
    )
    .expect("simulation")
}

// ── Fingerprint ─────────────────────────────────────────────────────────

/// FNV-1a-style bit fingerprint over **everything** a `SimulationResult`
/// exposes. Floats go in as `to_bits`, so `-0.0 != 0.0` and a last-ULP
/// divergence cannot hide — the failure mode `DELTA_gen_w1`'s G3 correction
/// was caught by and a value comparison would have missed.
fn fingerprint(result: &SimulationResult) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    result.total_moves.hash(&mut h);
    result.resolution_clamped.hash(&mut h);
    result.column_grid_cell_mm.to_bits().hash(&mut h);
    hash_mesh(&mut h, &result.mesh);

    match &result.deviations {
        Some(d) => {
            d.len().hash(&mut h);
            for v in d {
                v.to_bits().hash(&mut h);
            }
        }
        None => 0_u8.hash(&mut h),
    }
    match &result.column_deviations {
        Some(d) => {
            d.len().hash(&mut h);
            for c in d {
                c.x.to_bits().hash(&mut h);
                c.y.to_bits().hash(&mut h);
                c.dev.to_bits().hash(&mut h);
                c.top_z.to_bits().hash(&mut h);
                c.group.hash(&mut h);
                c.row.hash(&mut h);
                c.col.hash(&mut h);
            }
        }
        None => 0_u8.hash(&mut h),
    }

    result.boundaries.len().hash(&mut h);
    for b in &result.boundaries {
        b.id.hash(&mut h);
        b.name.hash(&mut h);
        b.tool_name.hash(&mut h);
        b.start_move.hash(&mut h);
        b.end_move.hash(&mut h);
        format!("{:?}", b.direction).hash(&mut h);
    }

    result.checkpoints.len().hash(&mut h);
    for cp in &result.checkpoints {
        cp.boundary_index.hash(&mut h);
        hash_mesh(&mut h, &cp.mesh);
        hash_stock(&mut h, &cp.stock);
    }

    result.rapid_collisions.len().hash(&mut h);
    for rc in &result.rapid_collisions {
        format!("{rc:?}").hash(&mut h);
    }
    result.rapid_collision_move_indices.hash(&mut h);

    // `prior_stocks` is a HashMap; sort by id so iteration order cannot leak
    // into the fingerprint.
    let mut ids: Vec<ToolpathId> = result.prior_stocks.keys().copied().collect();
    ids.sort_unstable();
    ids.len().hash(&mut h);
    for id in ids {
        id.hash(&mut h);
        hash_stock(&mut h, &result.prior_stocks[&id]);
    }

    match &result.cut_trace {
        Some(trace) => {
            // The trace is `Serialize` (it is written to disk as an artifact),
            // so serialising it is the most complete comparison available and
            // it cannot silently skip a field added later.
            serde_json::to_vec(trace.as_ref())
                .expect("trace serialises")
                .hash(&mut h);
        }
        None => 0_u8.hash(&mut h),
    }
    h.finish()
}

fn hash_mesh(
    h: &mut std::collections::hash_map::DefaultHasher,
    mesh: &rs_cam_core::stock_mesh::StockMesh,
) {
    mesh.vertices.len().hash(h);
    for v in &mesh.vertices {
        v.to_bits().hash(h);
    }
    mesh.indices.hash(h);
    for c in &mesh.colors {
        c.to_bits().hash(h);
    }
}

fn hash_stock(h: &mut std::collections::hash_map::DefaultHasher, stock: &TriDexelStock) {
    let g = &stock.z_grid;
    g.rows.hash(h);
    g.cols.hash(h);
    g.cell_size.to_bits().hash(h);
    g.rays.len().hash(h);
    for ray in &g.rays {
        ray.len().hash(h);
        for seg in ray.iter() {
            seg.enter.to_bits().hash(h);
            seg.exit.to_bits().hash(h);
        }
    }
    for t in &g.conservative_top {
        t.to_bits().hash(h);
    }
}

// ── Sentries ────────────────────────────────────────────────────────────

/// The load-bearing one. Simulate 3 passes with the memo, then simulate 5
/// passes reusing the same three `Arc<AnnotatedToolpath>`s — the shape a
/// fixpoint round produces — and require the resumed result to be **bit-for-bit**
/// what a from-scratch replay of the same 5-pass request produces.
#[test]
fn resumed_run_is_bit_identical_to_a_full_replay() {
    let chain = chain(5);
    let mut cache = SimPrefixCache::new();

    // Round 1: seeds the memo.
    let round1 = request(&chain, 3);
    let _ = run_memo(&round1, &mut cache, true);
    assert_eq!(cache.stats().hits, 0, "nothing to hit on the first round");
    assert!(cache.is_populated(), "round 1 must leave a snapshot");

    // Round 2: resumes.
    let round2 = request(&chain, 5);
    let resumed = run_memo(&round2, &mut cache, true);
    let stats = cache.stats();
    assert_eq!(stats.hits, 1, "round 2 must resume from the snapshot");
    assert_eq!(
        stats.entries_reused, 3,
        "all three round-1 toolpaths must be skipped, not re-simulated"
    );

    // The oracle: the identical request, no memo at all.
    let fresh = run(&request(&chain, 5));

    assert_eq!(
        fingerprint(&resumed),
        fingerprint(&fresh),
        "a resumed simulation must be indistinguishable from a full replay"
    );
    // Guard the fingerprint itself: a fixture that produced nothing would make
    // the equality above vacuous.
    assert!(resumed.total_moves > 20, "fixture must actually move");
    assert_eq!(resumed.checkpoints.len(), 5);
    let trace = resumed.cut_trace.as_ref().expect("metrics on");
    assert!(
        trace.samples.len() > 500,
        "fixture must actually cut: {} samples",
        trace.samples.len()
    );
    assert!(
        trace.summary.total_removed_volume_est_mm3 > 100.0,
        "fixture must remove material: {} mm3",
        trace.summary.total_removed_volume_est_mm3
    );
}

/// Resuming twice in a row — the three-round ladder wanaka actually runs.
/// Each round must re-snapshot at its own new depth.
#[test]
fn a_three_round_ladder_resumes_at_increasing_depth() {
    let chain = chain(6);
    let mut cache = SimPrefixCache::new();

    let _ = run_memo(&request(&chain, 2), &mut cache, true);
    let _ = run_memo(&request(&chain, 4), &mut cache, true);
    let final_result = run_memo(&request(&chain, 6), &mut cache, true);

    let stats = cache.stats();
    assert_eq!(stats.hits, 2, "rounds 2 and 3 both resume");
    assert_eq!(
        stats.entries_reused,
        2 + 4,
        "round 2 reuses 2 entries and round 3 reuses 4"
    );
    assert_eq!(stats.snapshots_stored, 3);

    let fresh = run(&request(&chain, 6));
    assert_eq!(fingerprint(&final_result), fingerprint(&fresh));
}

/// A changed toolpath invalidates its own position and everything after it.
/// This is the case that makes the memo safe to leave switched on: a
/// regenerated op produces a NEW `Arc<AnnotatedToolpath>`, so the pointer key
/// misses.
#[test]
fn a_regenerated_prefix_toolpath_misses_and_the_result_still_matches() {
    let chain = chain(4);
    let mut cache = SimPrefixCache::new();
    let _ = run_memo(&request(&chain, 3), &mut cache, true);

    // Regenerate pass 1 (a different depth, a different Arc) and add a pass.
    // NOTE: the replacement must stay inside the stock. A stamp whose whole
    // bbox falls outside the grid panics in debug in `stamping.rs`'s S2 mip
    // block (`row_hi + 1 - row_lo` underflows when `row_lo > row_hi`) — a
    // pre-existing defect, unrelated to S5, recorded in `DELTA_sim_w3.md`.
    let mut edited = chain;
    edited[1] = pass(3);
    let req = request(&edited, 4);
    let resumed = run_memo(&req, &mut cache, true);

    assert_eq!(
        cache.stats().hits,
        0,
        "a changed toolpath inside the prefix must miss"
    );
    assert_eq!(fingerprint(&resumed), fingerprint(&run(&req)));
}

/// Changing a *request-global* input the prefix depends on must miss too,
/// even though every toolpath `Arc` is unchanged. Resolution is the sharpest
/// case: it sets the grid the whole prefix was carved into.
#[test]
fn a_changed_resolution_or_stock_misses() {
    let chain = chain(3);

    for mutate in [
        (|r: &mut SimulationRequest| r.resolution = 0.75) as fn(&mut SimulationRequest),
        |r: &mut SimulationRequest| r.stock_bbox.max.z = 1.0,
        |r: &mut SimulationRequest| r.stock_top_z = 1.0,
        |r: &mut SimulationRequest| r.spindle_rpm = 12_000,
        |r: &mut SimulationRequest| r.rapid_feed_mm_min = 9000.0,
        |r: &mut SimulationRequest| r.metric_options.capture_arc_engagement = false,
        |r: &mut SimulationRequest| r.metric_options.enabled = false,
    ] {
        let mut cache = SimPrefixCache::new();
        let _ = run_memo(&request(&chain, 2), &mut cache, true);
        let mut req = request(&chain, 3);
        mutate(&mut req);
        let resumed = run_memo(&req, &mut cache, true);
        assert_eq!(
            cache.stats().hits,
            0,
            "a changed global input must invalidate the prefix"
        );
        assert_eq!(fingerprint(&resumed), fingerprint(&run(&req)));
    }
}

/// The tool has no stable object identity, so its key is derived from the
/// trait's observable surface (`sim_prefix::hash_tool`). Every shipped cutter
/// shape must key differently — otherwise a tool change inside the prefix
/// would resume onto a grid carved by the wrong cutter.
#[test]
fn tool_key_separates_every_shipped_shape() {
    let chain = chain(3);
    let variants: Vec<(&str, ToolDefinition)> = vec![
        (
            "flat6",
            ToolDefinition::new(
                Box::new(FlatEndmill::new(6.0, 25.0)),
                6.0,
                20.0,
                25.0,
                45.0,
                2,
                ToolMaterial::Carbide,
            ),
        ),
        (
            "ball6",
            ToolDefinition::new(
                Box::new(BallEndmill::new(6.0, 25.0)),
                6.0,
                20.0,
                25.0,
                45.0,
                2,
                ToolMaterial::Carbide,
            ),
        ),
        (
            "vbit",
            ToolDefinition::new(
                Box::new(VBitEndmill::new(6.0, 90.0, 25.0)),
                6.0,
                20.0,
                25.0,
                45.0,
                2,
                ToolMaterial::Carbide,
            ),
        ),
        (
            "flat6_long_stickout",
            ToolDefinition::new(
                Box::new(FlatEndmill::new(6.0, 25.0)),
                6.0,
                20.0,
                25.0,
                60.0,
                2,
                ToolMaterial::Carbide,
            ),
        ),
        (
            "flat6_three_flute",
            ToolDefinition::new(
                Box::new(FlatEndmill::new(6.0, 25.0)),
                6.0,
                20.0,
                25.0,
                45.0,
                3,
                ToolMaterial::Carbide,
            ),
        ),
    ];

    // Baseline: identical tools must HIT (otherwise this test would pass by
    // the cache never hitting at all — the vacuity trap).
    let mut cache = SimPrefixCache::new();
    let _ = run_memo(&request(&chain, 2), &mut cache, true);
    let _ = run_memo(&request(&chain, 3), &mut cache, true);
    assert_eq!(
        cache.stats().hits,
        1,
        "identical tools must hit, or the misses below prove nothing"
    );

    for (label, replacement) in variants.iter().skip(1) {
        let mut cache = SimPrefixCache::new();
        let _ = run_memo(&request(&chain, 2), &mut cache, true);
        let mut req = request(&chain, 3);
        req.groups[0].toolpaths[0].tool = clone_tool(replacement);
        let resumed = run_memo(&req, &mut cache, true);
        assert_eq!(
            cache.stats().hits,
            0,
            "{label} must not key equal to the flat end mill it replaced"
        );
        assert_eq!(fingerprint(&resumed), fingerprint(&run(&req)), "{label}");
    }
}

/// `ToolDefinition` is not `Clone` (its cutter is a trait object), so the
/// variants above are rebuilt rather than cloned.
fn clone_tool(src: &ToolDefinition) -> ToolDefinition {
    use rs_cam_core::feeds::ToolGeometryHint;
    use rs_cam_core::tool::MillingCutter;
    let cutter: Box<dyn MillingCutter> = match src.geometry_hint() {
        ToolGeometryHint::Ball => Box::new(BallEndmill::new(src.diameter(), src.length())),
        ToolGeometryHint::VBit { included_angle, .. } => Box::new(VBitEndmill::new(
            src.diameter(),
            included_angle,
            src.length(),
        )),
        _ => Box::new(FlatEndmill::new(src.diameter(), src.length())),
    };
    ToolDefinition::new(
        cutter,
        src.shank_diameter,
        src.shank_length,
        src.holder_diameter,
        src.stickout,
        src.flute_count,
        src.tool_material,
    )
}

/// `store: false` still *uses* a snapshot (that is free) but must leave the
/// cache empty. This is what bounds the memo's memory for ordinary user
/// simulations: they consume and release rather than retain.
#[test]
fn a_non_storing_run_consumes_the_snapshot_and_leaves_nothing() {
    let chain = chain(3);
    let mut cache = SimPrefixCache::new();
    let _ = run_memo(&request(&chain, 2), &mut cache, true);
    assert!(cache.is_populated());

    let req = request(&chain, 3);
    let resumed = run_memo(&req, &mut cache, false);
    assert_eq!(cache.stats().hits, 1, "a non-storing run still resumes");
    assert!(
        !cache.is_populated(),
        "a non-storing run must not retain a snapshot"
    );
    assert_eq!(cache.stats().held_bytes, 0);
    assert_eq!(fingerprint(&resumed), fingerprint(&run(&req)));
}

/// The size ceiling is a real refusal, not a comment.
#[test]
fn the_size_ceiling_refuses_rather_than_growing() {
    let chain = chain(3);
    let mut cache = SimPrefixCache::new();
    cache.max_bytes = 1;
    let _ = run_memo(&request(&chain, 2), &mut cache, true);
    assert!(!cache.is_populated());
    assert_eq!(cache.stats().size_refusals, 1);
    assert_eq!(cache.stats().snapshots_stored, 0);

    // And a refused snapshot costs correctness nothing.
    let req = request(&chain, 3);
    let resumed = run_memo(&req, &mut cache, true);
    assert_eq!(cache.stats().hits, 0);
    assert_eq!(fingerprint(&resumed), fingerprint(&run(&req)));
}

/// The memory claim, tested rather than asserted in prose: a held snapshot
/// SHARES the result's checkpoints instead of copying them.
///
/// A checkpoint is a marching-cubes mesh plus a full dexel-grid clone, one per
/// toolpath — the heaviest artifact a simulation produces. `strong_count == 2`
/// while the snapshot is held (the result and the snapshot), falling to 1 when
/// it is cleared, is exactly what says the snapshot added a refcount and not a
/// second copy. The `held_bytes` bound is the same claim in the size estimate:
/// the snapshot's own footprint must sit well under the checkpoint bytes it
/// points at.
#[test]
fn the_snapshot_shares_checkpoints_rather_than_copying_them() {
    let chain = chain(3);
    let mut cache = SimPrefixCache::new();
    let result = run_memo(&request(&chain, 3), &mut cache, true);
    assert!(cache.is_populated());

    for cp in &result.checkpoints {
        assert_eq!(
            Arc::strong_count(cp),
            2,
            "the snapshot must share the result's checkpoint, not clone it"
        );
    }

    let checkpoint_bytes: usize = result
        .checkpoints
        .iter()
        .map(|cp| {
            cp.mesh.vertices.len() * 4
                + cp.mesh.indices.len() * 4
                + cp.mesh.colors.len() * 4
                + cp.stock.z_grid.rays.len() * 32
        })
        .sum();
    let held = cache.stats().held_bytes;
    assert!(
        held < checkpoint_bytes,
        "snapshot footprint {held} B should stay under the {checkpoint_bytes} B of \
         checkpoints it shares"
    );
    // Recorded rather than printed: the measured figures for this fixture are
    // in `DELTA_sim_w3.md` §4. A lower bound keeps the estimate honest in the
    // other direction — a snapshot that reported ~nothing would satisfy the
    // upper bound while measuring the wrong thing.
    assert!(
        held > 100_000,
        "the snapshot really does own the sample stream and both grids: {held} B"
    );

    cache.clear();
    for cp in &result.checkpoints {
        assert_eq!(Arc::strong_count(cp), 1, "clear() must release the share");
    }
}

/// `clear()` drops the snapshot — the GUI's release point when the fixpoint
/// ladder settles.
#[test]
fn clear_releases_the_snapshot() {
    let chain = chain(3);
    let mut cache = SimPrefixCache::new();
    let _ = run_memo(&request(&chain, 2), &mut cache, true);
    assert!(cache.is_populated());
    assert!(cache.stats().held_bytes > 0);
    cache.clear();
    assert!(!cache.is_populated());
    assert_eq!(cache.stats().held_bytes, 0);

    let req = request(&chain, 3);
    let resumed = run_memo(&req, &mut cache, true);
    assert_eq!(cache.stats().hits, 0, "a cleared cache cannot hit");
    assert_eq!(fingerprint(&resumed), fingerprint(&run(&req)));
}

/// `phantom_prior_stock` is the one per-group input deliberately left out of
/// the key, because it moves down the group on every fixpoint round. Its
/// effect on `prior_stocks` must be re-derived from the live request — an
/// entry the previous round produced must not survive, and one the current
/// round asks for must appear.
#[test]
fn phantom_prior_stock_is_rederived_not_restored() {
    let chain = chain(4);
    let mut cache = SimPrefixCache::new();

    // Round 1: three generated ops, with the pending rest op's phantom
    // snapshot taken before op index 1.
    let mut round1 = request(&chain, 3);
    round1.groups[0].phantom_prior_stock = Some((1, ToolpathId(900)));
    let r1 = run_memo(&round1, &mut cache, true);
    assert!(r1.prior_stocks.contains_key(&ToolpathId(900)));

    // Round 2: the phantom has moved to a different position AND a different
    // id — exactly what a fixpoint round does.
    let mut round2 = request(&chain, 4);
    round2.groups[0].phantom_prior_stock = Some((2, ToolpathId(901)));
    let resumed = run_memo(&round2, &mut cache, true);
    assert_eq!(cache.stats().hits, 1, "the phantom must not block the hit");
    assert!(
        !resumed.prior_stocks.contains_key(&ToolpathId(900)),
        "the previous round's phantom must not survive into this result"
    );
    assert!(resumed.prior_stocks.contains_key(&ToolpathId(901)));
    assert_eq!(fingerprint(&resumed), fingerprint(&run(&round2)));

    // And the tail position, which is where the fixpoint ladder actually puts
    // it: after every generated op in the group.
    let mut cache = SimPrefixCache::new();
    let mut round1 = request(&chain, 2);
    round1.groups[0].phantom_prior_stock = Some((2, ToolpathId(910)));
    let _ = run_memo(&round1, &mut cache, true);
    let mut round2 = request(&chain, 3);
    round2.groups[0].phantom_prior_stock = Some((3, ToolpathId(911)));
    let resumed = run_memo(&round2, &mut cache, true);
    assert_eq!(cache.stats().hits, 1);
    assert!(!resumed.prior_stocks.contains_key(&ToolpathId(910)));
    assert!(resumed.prior_stocks.contains_key(&ToolpathId(911)));
    assert_eq!(fingerprint(&resumed), fingerprint(&run(&round2)));
}

/// Multi-group: the resume point can be inside the last group while earlier
/// groups sit entirely in the restored state, including their end-of-group
/// composite-mesh append.
#[test]
fn a_multi_group_prefix_resumes_inside_the_last_group() {
    let chain = chain(4);
    let two_groups = |counts: [usize; 2]| SimulationRequest {
        groups: (0..2)
            .map(|g| SimGroupEntry {
                toolpaths: (0..counts[g])
                    .map(|i| {
                        let index = g * 2 + i;
                        entry(index, &chain[index])
                    })
                    .collect(),
                direction: StockCutDirection::FromTop,
                local_stock_bbox: None,
                local_to_global: None,
                phantom_prior_stock: None,
            })
            .collect(),
        stock_bbox: stock(),
        stock_top_z: 0.0,
        resolution: 0.5,
        metric_options: SimulationMetricOptions {
            enabled: true,
            capture_arc_engagement: true,
        },
        spindle_rpm: 18_000,
        rapid_feed_mm_min: 5000.0,
        model_mesh: None,
        kinematics: None,
    };

    let mut cache = SimPrefixCache::new();
    let _ = run_memo(&two_groups([2, 1]), &mut cache, true);
    let req = two_groups([2, 2]);
    let resumed = run_memo(&req, &mut cache, true);
    assert_eq!(cache.stats().hits, 1);
    assert_eq!(cache.stats().entries_reused, 3);
    assert_eq!(fingerprint(&resumed), fingerprint(&run(&req)));
}

/// A group *before* the resume group carrying a tail-position phantom is the
/// one shape the cache cannot reconstruct (its snapshot is that group's fully
/// carved stock, which the run drops when it moves on). It must refuse — and
/// the refusal must be counted, not silent.
#[test]
fn a_tail_phantom_on_an_earlier_group_refuses_the_hit() {
    let chain = chain(4);
    let two_groups = |counts: [usize; 2], phantom: Option<(usize, ToolpathId)>| SimulationRequest {
        groups: (0..2)
            .map(|g| SimGroupEntry {
                toolpaths: (0..counts[g])
                    .map(|i| {
                        let index = g * 2 + i;
                        entry(index, &chain[index])
                    })
                    .collect(),
                direction: StockCutDirection::FromTop,
                local_stock_bbox: None,
                local_to_global: None,
                phantom_prior_stock: if g == 0 { phantom } else { None },
            })
            .collect(),
        stock_bbox: stock(),
        stock_top_z: 0.0,
        resolution: 0.5,
        metric_options: SimulationMetricOptions {
            enabled: true,
            capture_arc_engagement: true,
        },
        spindle_rpm: 18_000,
        rapid_feed_mm_min: 5000.0,
        model_mesh: None,
        kinematics: None,
    };

    let mut cache = SimPrefixCache::new();
    let _ = run_memo(&two_groups([2, 1], None), &mut cache, true);
    let req = two_groups([2, 2], Some((2, ToolpathId(900))));
    let resumed = run_memo(&req, &mut cache, true);
    assert_eq!(cache.stats().hits, 0);
    assert_eq!(cache.stats().phantom_refusals, 1);
    assert_eq!(fingerprint(&resumed), fingerprint(&run(&req)));

    // The recoverable interior case on the same shape must still hit, so the
    // refusal above is a rule rather than a blanket.
    let mut cache = SimPrefixCache::new();
    let _ = run_memo(&two_groups([2, 1], None), &mut cache, true);
    let req = two_groups([2, 2], Some((1, ToolpathId(901))));
    let resumed = run_memo(&req, &mut cache, true);
    assert_eq!(cache.stats().hits, 1);
    assert!(resumed.prior_stocks.contains_key(&ToolpathId(901)));
    assert_eq!(fingerprint(&resumed), fingerprint(&run(&req)));
}

/// Build a request whose groups take explicit toolpath indices out of `chain`.
fn layout_request(chain: &[Arc<AnnotatedToolpath>], layout: &[&[usize]]) -> SimulationRequest {
    SimulationRequest {
        groups: layout
            .iter()
            .map(|indices| SimGroupEntry {
                toolpaths: indices.iter().map(|&i| entry(i, &chain[i])).collect(),
                direction: StockCutDirection::FromTop,
                local_stock_bbox: None,
                local_to_global: None,
                phantom_prior_stock: None,
            })
            .collect(),
        stock_bbox: stock(),
        stock_top_z: 0.0,
        resolution: 0.5,
        metric_options: SimulationMetricOptions {
            enabled: true,
            capture_arc_engagement: true,
        },
        spindle_rpm: 18_000,
        rapid_feed_mm_min: 5000.0,
        model_mesh: None,
        kinematics: None,
    }
}

/// A group the snapshot replayed **in full** has its end-of-group work
/// (composite-mesh append, column deviations) baked into the restored state.
/// If that group later GAINS a toolpath, the snapshot is unusable — resuming
/// past it would skip the new op entirely and keep a composite mesh built from
/// a stock the op never carved.
///
/// The degenerate form is the sharper one: a group that was **empty** keys as
/// zero entries, and a naive prefix test would let it match a group that has
/// since grown any number of them.
#[test]
fn a_completed_group_that_gains_a_toolpath_invalidates_the_prefix() {
    let chain = chain(5);

    // Setup 0 gains an op between rounds.
    let mut cache = SimPrefixCache::new();
    let _ = run_memo(&layout_request(&chain, &[&[0, 1], &[3]]), &mut cache, true);
    let req = layout_request(&chain, &[&[0, 1, 2], &[3]]);
    let resumed = run_memo(&req, &mut cache, true);
    assert_eq!(
        cache.stats().hits,
        0,
        "a completed group that grew must invalidate the prefix"
    );
    assert_eq!(fingerprint(&resumed), fingerprint(&run(&req)));

    // The degenerate form: an empty leading group that later grows.
    let mut cache = SimPrefixCache::new();
    let _ = run_memo(&layout_request(&chain, &[&[], &[3]]), &mut cache, true);
    let req = layout_request(&chain, &[&[0, 1], &[3]]);
    let resumed = run_memo(&req, &mut cache, true);
    assert_eq!(
        cache.stats().hits,
        0,
        "an empty group is not a wildcard for a populated one"
    );
    assert_eq!(fingerprint(&resumed), fingerprint(&run(&req)));

    // Positive control on the same shape: leave setup 0 alone, extend setup 1,
    // and it must hit — otherwise the two misses above prove nothing.
    let mut cache = SimPrefixCache::new();
    let _ = run_memo(&layout_request(&chain, &[&[0, 1], &[3]]), &mut cache, true);
    let req = layout_request(&chain, &[&[0, 1], &[3, 4]]);
    let resumed = run_memo(&req, &mut cache, true);
    assert_eq!(cache.stats().hits, 1);
    assert_eq!(cache.stats().entries_reused, 3);
    assert_eq!(fingerprint(&resumed), fingerprint(&run(&req)));
}

/// A shorter request than the snapshot cannot resume from it. (The memo only
/// holds state at one depth, so a shallower prefix has nothing to restore.)
#[test]
fn a_shorter_request_misses_cleanly() {
    let chain = chain(4);
    let mut cache = SimPrefixCache::new();
    let _ = run_memo(&request(&chain, 4), &mut cache, true);
    let req = request(&chain, 2);
    let resumed = run_memo(&req, &mut cache, true);
    assert_eq!(cache.stats().hits, 0);
    assert_eq!(fingerprint(&resumed), fingerprint(&run(&req)));
}
