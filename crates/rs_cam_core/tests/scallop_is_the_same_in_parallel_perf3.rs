//! P3 (`planning/perf_sweep_2026-11-03/PLAN.md`): the scallop lifts its
//! rings in parallel, and the toolpath stays byte-identical.
//!
//! The ring lift (`finish/scallop/ring_generation.rs`, `ring_to_3d`) runs
//! one drop-cutter probe per ring point, and the iso-field branch lifts
//! independent rings. P3 puts both on rayon with an ordered collect. This
//! sentry pins the output of both ring sources on one fixture:
//!
//! - a height field with two bumps, so chord refinement inserts points;
//! - a region that runs 2 mm past the mesh edge on -X, so some ring points
//!   are not covered and the run-splitter retracts around them;
//! - enough rings to fill several chunks of the ring loop.
//!
//! Each arm runs in a 1-thread, a 2-thread and a 5-thread rayon pool. The
//! chunk size of the ring loop follows the pool size, so the three pools
//! cut the ring list at different places. Every run must give the pinned
//! move count and the pinned FNV-1a hash of the move list
//! (`common::fingerprint`; `Debug` on `f64` prints every bit).
//!
//! The pins were taken on HEAD `40cf411a7` before the P3 change.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test scallop_is_the_same_in_parallel_perf3
//! ```

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]
#![expect(
    clippy::print_stderr,
    reason = "the test prints its measurements for the reader"
)]

mod common;

use common::fingerprint::move_fingerprint;
use common::meshes::height_field;
use rs_cam_core::finish::scallop::{
    ScallopParams, scallop_toolpath_iso_field_with_cancel,
    scallop_toolpath_structured_annotated_with_cancel,
};
use rs_cam_core::geo::P2;
use rs_cam_core::geometry::region_set::RegionSet;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::tool::BallEndmill;
use rs_cam_core::toolpath::{MoveType, Toolpath};

/// The mesh covers `[-HALF, HALF]` in X and Y.
const HALF: f64 = 12.0;
const STEP: f64 = 0.25;
/// The region starts this far past the mesh edge on -X.
const OVERHANG: f64 = 2.0;
const BALL_D: f64 = 2.0;
const SCALLOP_H: f64 = 0.01;
const TOL: f64 = 0.05;

/// The fewest ring annotations an arm must make. The ring loop lifts
/// `4 x threads` rings per chunk, so in the 2-thread pool this is three
/// chunks.
const MIN_RINGS: usize = 24;

/// The pool sizes each arm runs in.
const POOLS: [usize; 3] = [1, 2, 5];

/// `(moves, hash)` of the cascade arm at HEAD `40cf411a7`.
const PIN_CASCADE: (usize, u64) = (5703, 0x8977_a998_c5af_5090);
/// `(moves, hash)` of the iso-field arm at HEAD `40cf411a7`.
const PIN_ISO_FIELD: (usize, u64) = (6124, 0xf2ec_aec5_50b9_bf91);

fn bumps() -> TriangleMesh {
    height_field(HALF, STEP, |x, y| {
        1.5 * (-((x - 3.0).powi(2) + (y - 2.0).powi(2)) / 8.0).exp()
            + 1.0 * (-((x + 4.0).powi(2) + (y + 3.0).powi(2)) / 5.0).exp()
    })
}

fn region() -> RegionSet<'static> {
    let (x0, x1, y0, y1) = (-HALF - OVERHANG, 10.0, -10.0, 10.0);
    RegionSet::new(vec![Polygon2::new(vec![
        P2::new(x0, y0),
        P2::new(x1, y0),
        P2::new(x1, y1),
        P2::new(x0, y1),
    ])])
}

fn params() -> ScallopParams {
    ScallopParams {
        scallop_height: SCALLOP_H,
        tolerance: TOL,
        ..ScallopParams::default()
    }
}

#[derive(Clone, Copy, Debug)]
enum Arm {
    Cascade,
    IsoField,
}

fn generate(arm: Arm) -> (Toolpath, usize) {
    let mesh = bumps();
    let index = SpatialIndex::build_auto(&mesh);
    let cutter = BallEndmill::new(BALL_D, 25.0);
    let params = params();
    let region = region();
    let (tp, anns, _) = rs_cam_core::interrupt::run_uncancellable(|cancel| match arm {
        Arm::Cascade => scallop_toolpath_structured_annotated_with_cancel(
            &mesh,
            &index,
            &cutter,
            &params,
            None,
            Some(&region),
            cancel,
        ),
        Arm::IsoField => scallop_toolpath_iso_field_with_cancel(
            &mesh,
            &index,
            &cutter,
            &params,
            None,
            Some(&region),
            None,
            cancel,
        ),
    });
    (tp, anns.len())
}

/// Fed moves whose target is past the mesh edge: the coverage test must
/// keep this at zero, while the region overhang makes it possible.
fn fed_off_mesh(tp: &Toolpath) -> usize {
    tp.moves
        .iter()
        .filter(|m| !matches!(m.move_type, MoveType::Rapid))
        .filter(|m| m.target.x < -HALF - 1e-9)
        .count()
}

fn check(arm: Arm, pin: (usize, u64)) {
    let mut seen: Vec<(usize, (usize, u64))> = Vec::new();
    for threads in POOLS {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .expect("a rayon pool");
        let (tp, rings) = pool.install(|| generate(arm));
        let fp = move_fingerprint(&tp);
        let rapids = tp
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, MoveType::Rapid))
            .count();
        eprintln!(
            "{arm:?} @ {threads} threads: {rings} ring annotations, {rapids} rapids, \
             {} fed off-mesh, fingerprint ({}, {:#018x})",
            fed_off_mesh(&tp),
            fp.0,
            fp.1
        );
        assert!(
            rings >= MIN_RINGS,
            "non-vacuity: {rings} rings do not fill several chunks"
        );
        assert_eq!(fed_off_mesh(&tp), 0, "a fed move left the mesh");
        seen.push((threads, fp));
    }
    for (threads, fp) in &seen {
        assert_eq!(
            *fp, pin,
            "{arm:?} @ {threads} threads moved off the pinned toolpath"
        );
    }
}

#[test]
fn the_cascade_scallop_is_the_same_in_every_pool() {
    check(Arm::Cascade, PIN_CASCADE);
}

#[test]
fn the_iso_field_scallop_is_the_same_in_every_pool() {
    check(Arm::IsoField, PIN_ISO_FIELD);
}
