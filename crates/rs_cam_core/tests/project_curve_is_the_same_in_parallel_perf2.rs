//! P2 (planning/perf_sweep_2026-11-03/PLAN.md): `project_curve` projects
//! its points on rayon. The toolpath must not change.
//!
//! This sentry pins each case's toolpath with a hash of every move: the
//! target bits, the move type and its feed bits, and the intent. The
//! values come from a run of the serial code before P2 (see
//! planning/perf_sweep_2026-11-03/B_REPORT.md).
//!
//! The fixture is a wavy heightfield with a square hole. The curves run
//! over the hole and past the mesh edge, so some points have no surface
//! under them and the chain logic breaks the path there. The spacing is
//! fine, so each ring has thousands of points and spans many chunks.

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::cell::Cell;

use rs_cam_core::geo::{P2, P3};
use rs_cam_core::interrupt::{CancelCheck, NeverCancel};
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::ops::project_curve::{
    ProjectCurveParams, ProjectDirection, ProjectProjection, ProjectSide,
    project_curve_toolpath_with_cancel,
};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::tool::{BallEndmill, MillingCutter};
use rs_cam_core::toolpath::{MoveType, Toolpath};

/// Half the side of the square heightfield (mm).
const HALF: f64 = 30.0;
/// Cells per side (1 mm cells).
const CELLS: u32 = 60;

fn surface_z(x: f64, y: f64) -> f64 {
    5.0 + 2.0 * (x / 7.0).sin() * (y / 5.0).cos()
}

/// True for a cell inside the hole: x in [10, 20), y in [-5, 5).
fn cell_in_hole(x0: f64, y0: f64) -> bool {
    (10.0..20.0).contains(&x0) && (-5.0..5.0).contains(&y0)
}

fn mesh_with_hole() -> TriangleMesh {
    let mut vertices = Vec::new();
    for j in 0..=CELLS {
        for i in 0..=CELLS {
            let x = -HALF + f64::from(i);
            let y = -HALF + f64::from(j);
            vertices.push(P3::new(x, y, surface_z(x, y)));
        }
    }
    let mut triangles = Vec::new();
    for j in 0..CELLS {
        for i in 0..CELLS {
            if cell_in_hole(-HALF + f64::from(i), -HALF + f64::from(j)) {
                continue;
            }
            let v00 = j * (CELLS + 1) + i;
            let v10 = v00 + 1;
            let v01 = v00 + CELLS + 1;
            let v11 = v01 + 1;
            triangles.push([v00, v10, v11]);
            triangles.push([v00, v11, v01]);
        }
    }
    TriangleMesh::from_raw(vertices, triangles)
}

/// A 64-gon of radius `r` about (`cx`, `cy`). It starts at 90 degrees, so
/// the gaps at +X do not touch the start of the ring.
fn ngon(cx: f64, cy: f64, r: f64) -> Vec<P2> {
    (0..64)
        .map(|k| {
            let a = std::f64::consts::FRAC_PI_2 + std::f64::consts::TAU * f64::from(k) / 64.0;
            P2::new(cx + r * a.cos(), cy + r * a.sin())
        })
        .collect()
}

/// A closed ring that crosses the hole and the +X mesh edge, with one
/// hole ring (a closed inner ring that also crosses the hole).
fn closed_curve() -> Polygon2 {
    Polygon2::with_holes(ngon(8.0, 0.0, 26.0), vec![ngon(10.0, 0.0, 8.0)])
}

/// An open zigzag that crosses the hole and leaves the mesh at -X.
fn open_curve() -> Polygon2 {
    let pts: Vec<P2> = (0..=16)
        .map(|k| {
            let x = -36.0 + 3.5 * f64::from(k);
            let y = if k % 2 == 0 { -6.0 } else { 6.0 };
            P2::new(x, y)
        })
        .collect();
    Polygon2::open_path(pts)
}

fn params(
    projection: ProjectProjection,
    direction: ProjectDirection,
    side: ProjectSide,
) -> ProjectCurveParams {
    ProjectCurveParams {
        depth: 0.5,
        feed_rate: 900.0,
        plunge_rate: 300.0,
        safe_z: 20.0,
        point_spacing: 0.02,
        direction,
        tool_radius: 3.0,
        side,
        projection,
        setup_z_flipped: false,
    }
}

/// FNV-1a over 64-bit words.
struct Fnv(u64);

impl Fnv {
    fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }
    fn word(&mut self, w: u64) {
        for b in w.to_le_bytes() {
            self.0 ^= u64::from(b);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    fn text(&mut self, s: &str) {
        for b in s.bytes() {
            self.0 ^= u64::from(b);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
        self.word(0xff);
    }
}

/// The hash of every move: target bits, move type and feed bits, intent.
fn toolpath_hash(tp: &Toolpath) -> u64 {
    let mut h = Fnv::new();
    for m in &tp.moves {
        h.word(m.target.x.to_bits());
        h.word(m.target.y.to_bits());
        h.word(m.target.z.to_bits());
        match m.move_type {
            MoveType::Rapid => h.word(0),
            MoveType::Linear { feed_rate } => {
                h.word(1);
                h.word(feed_rate.to_bits());
            }
            MoveType::ArcCW { i, j, feed_rate } => {
                h.word(2);
                h.word(i.to_bits());
                h.word(j.to_bits());
                h.word(feed_rate.to_bits());
            }
            MoveType::ArcCCW { i, j, feed_rate } => {
                h.word(3);
                h.word(i.to_bits());
                h.word(j.to_bits());
                h.word(feed_rate.to_bits());
            }
        }
        h.text(&format!("{:?}", m.intent));
    }
    h.0
}

fn rapid_count(tp: &Toolpath) -> usize {
    tp.moves
        .iter()
        .filter(|m| matches!(m.move_type, MoveType::Rapid))
        .count()
}

struct Case {
    name: &'static str,
    curve: fn() -> Polygon2,
    projection: ProjectProjection,
    direction: ProjectDirection,
    side: ProjectSide,
    /// The pinned (move count, rapid count, hash) from the serial code.
    pinned: (usize, usize, u64),
}

const CASES: &[Case] = &[
    Case {
        name: "closed_cutter_center",
        curve: closed_curve,
        projection: ProjectProjection::Cutter,
        direction: ProjectDirection::FromAbove,
        side: ProjectSide::Center,
        pinned: (8692, 8, 0x14df_a405_67f9_ff42),
    },
    Case {
        name: "closed_point_center",
        curve: closed_curve,
        projection: ProjectProjection::Point,
        direction: ProjectDirection::FromAbove,
        side: ProjectSide::Center,
        pinned: (8750, 8, 0x6d35_8162_073b_a45d),
    },
    Case {
        name: "closed_cutter_outside_from_below",
        curve: closed_curve,
        projection: ProjectProjection::Cutter,
        direction: ProjectDirection::FromBelow,
        side: ProjectSide::Outside,
        pinned: (10470, 10, 0x7bd1_70e6_6e2a_e681),
    },
    Case {
        name: "open_cutter",
        curve: open_curve,
        projection: ProjectProjection::Cutter,
        direction: ProjectDirection::FromAbove,
        side: ProjectSide::Center,
        pinned: (7414, 8, 0x614e_1d1f_ac39_7705),
    },
    Case {
        name: "open_point",
        curve: open_curve,
        projection: ProjectProjection::Point,
        direction: ProjectDirection::FromAbove,
        side: ProjectSide::Center,
        pinned: (7414, 8, 0x62e4_e1e5_0675_eb41),
    },
];

fn generate(case: &Case, mesh: &TriangleMesh, index: &SpatialIndex) -> Toolpath {
    let cutter = BallEndmill::new(6.0, 25.0);
    let cutter_dyn: &dyn MillingCutter = &cutter;
    project_curve_toolpath_with_cancel(
        &(case.curve)(),
        mesh,
        index,
        cutter_dyn,
        &params(case.projection, case.direction, case.side),
        &NeverCancel,
    )
    .expect("NeverCancel does not cancel")
}

#[test]
fn project_curve_is_the_same_in_parallel_perf2() {
    let mesh = mesh_with_hole();
    let index = SpatialIndex::build_auto(&mesh);
    let mut report = Vec::new();
    let mut ok = true;
    for case in CASES {
        let tp = generate(case, &mesh, &index);
        let got = (tp.moves.len(), rapid_count(&tp), toolpath_hash(&tp));
        // Each case must break at a gap: a segment has two rapids, and
        // more than two segments means at least one ring breaks.
        assert!(
            got.1 > 4,
            "{}: the fixture must put some points off the surface (rapids = {})",
            case.name,
            got.1
        );
        report.push(format!(
            "{}: ({}, {}, 0x{:016x})",
            case.name, got.0, got.1, got.2
        ));
        ok &= got == case.pinned;
    }
    assert!(ok, "toolpath changed:\n{}", report.join("\n"));
}

/// A cancel that answers `true` from its `n`-th poll on. It is not `Sync`
/// (a `Cell`), like a caller's `&dyn CancelCheck` can be.
struct CancelAfter {
    polls: Cell<usize>,
    n: usize,
}

impl CancelCheck for CancelAfter {
    fn cancelled(&self) -> bool {
        let p = self.polls.get() + 1;
        self.polls.set(p);
        p >= self.n
    }
}

#[test]
fn project_curve_stops_on_cancel_inside_a_ring_perf2() {
    let mesh = mesh_with_hole();
    let index = SpatialIndex::build_auto(&mesh);
    let cutter = BallEndmill::new(6.0, 25.0);
    // Poll 1 is the entry check. Poll 3 falls inside the first ring.
    let cancel = CancelAfter {
        polls: Cell::new(0),
        n: 3,
    };
    let result = project_curve_toolpath_with_cancel(
        &closed_curve(),
        &mesh,
        &index,
        &cutter,
        &params(
            ProjectProjection::Cutter,
            ProjectDirection::FromAbove,
            ProjectSide::Center,
        ),
        &cancel,
    );
    assert!(result.is_err(), "a cancel inside a ring must stop the op");
    assert_eq!(
        cancel.polls.get(),
        3,
        "the op must stop at the first true poll"
    );
}
