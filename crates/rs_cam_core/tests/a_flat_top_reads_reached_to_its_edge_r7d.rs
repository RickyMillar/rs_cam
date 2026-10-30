//! R7d — the reach map measures a flat top up to the part outline.
//!
//! # The claim
//!
//! Operator ruling, 2026-10-30, on `fixtures/gui_step/stepped_block.step`
//! with a Ø6 ball: the reach overlay left a grey band one envelope radius
//! wide inside the part outline (the old `rim_erosion_mm`, 3.0 mm), although
//! a ball reaches every point of a flat top. "yes. the toolpath hits those
//! areas so why wouldn't it measure it?"
//! (`planning/viewport_parity_2026-10-28/FOLLOW_UPS.md` §13.)
//!
//! 1. Every measured cell on a top face within one tip radius `R` of the
//!    OUTER outline, and at least `R` from the foot of a higher wall, reads
//!    REACHED: a gap at or under the tolerance. A `None` there is a failure.
//! 2. At the foot of a higher wall the ball leaves its own radius in the
//!    inside corner. A ball of radius `R` that touches a vertical wall has
//!    its centre `R` from the wall, so at a distance `d < R` from the wall
//!    the ball surface stands
//!
//!    ```text
//!    gap(d) = R − sqrt(R² − (R − d)²)
//!    ```
//!
//!    above the lower face, provided the wall is at least `gap(0) = R` high
//!    and the cell is at least `R` from the outline (nearer the outline the
//!    wall ends, and the ball reaches past its end).
//!    The map can only over-state a gap (its minimum is over a sampled CL
//!    set), so each cell reads at or above `gap(d)`, past the bar where
//!    `gap(d)` is, and never above `gap(0) = R`. The over-read inside that
//!    cap is a known grid limit at the wall stand-off; the corner test
//!    states it and prints the table.
//!
//! # The fixture
//!
//! The stepped block: X 0..60, Y 0..40, three top faces at Z 30, 20 and 10,
//! two walls 10 mm high at X 20 and X 40. The test reads the tops, the
//! walls and the outline from the mesh; it does not type them. The tool
//! diameter is the operator's live case (Ø6, §13). The cell comes from the
//! shipped rule, `ReachMapParams::for_cutter`.
//!
//! # Teeth
//!
//! Before R7d the map abstained one envelope radius inside the outline
//! (`rim_keep_mask`), and claim 1 failed on the first edge cell with a
//! `None`.
//!
//! ```text
//! cargo test -p rs_cam_core -q --features step,test-support --test a_flat_top_reads_reached_to_its_edge_r7d
//! ```

#![cfg(feature = "step")]
#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]
#![expect(
    clippy::print_stderr,
    reason = "the edge band and the corner table are printed so a re-run shows the numbers the report quotes"
)]

use std::path::PathBuf;

use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
use rs_cam_core::geo::Triangle;
use rs_cam_core::ids::ModelId;
use rs_cam_core::interrupt::NeverCancel;
use rs_cam_core::maps::reach_map::{
    DEFAULT_REACH_TOLERANCE_MM, ReachMap, ReachMapParams, compute_reach_map, surface_z_at,
};
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::tool::{BallEndmill, MillingCutter};

const STEPPED: &str = "../../fixtures/gui_step/stepped_block.step";

/// The operator's live tool (FOLLOW_UPS §13): a Ø6 ball.
const LIVE_BALL_DIAMETER_MM: f64 = 6.0;

/// A face counts as flat and up when its normal is within this of +Z.
/// The STEP tops are exact planes, so any small number separates them from
/// the walls (`n.z = 0`).
const FLAT_UP_NZ: f64 = 0.999;

/// Two heights closer than this are one face height.
const SAME_Z_MM: f64 = 1e-6;

fn stepped_mesh() -> TriangleMesh {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(STEPPED);
    let model = rs_cam_core::io::load_model_file(
        &path,
        ModelId::from_raw(0),
        ModelKind::Step,
        ModelUnits::Millimeters,
        Default::default(),
    )
    .unwrap();
    model
        .mesh
        .expect("the STEP door gives a mesh")
        .as_ref()
        .clone()
}

/// XY distance from `(x, y)` to the projection of `tri`; `0.0` inside.
fn xy_distance_to_triangle(tri: &Triangle, x: f64, y: f64) -> f64 {
    let p = |i: usize| (tri.v[i].x, tri.v[i].y);
    let (a, b, c) = (p(0), p(1), p(2));
    let cross = |o: (f64, f64), u: (f64, f64), v: (f64, f64)| {
        (u.0 - o.0) * (v.1 - o.1) - (u.1 - o.1) * (v.0 - o.0)
    };
    let q = (x, y);
    let (d1, d2, d3) = (cross(a, b, q), cross(b, c, q), cross(c, a, q));
    let has_neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let has_pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    if !(has_neg && has_pos) {
        return 0.0;
    }
    let seg = |u: (f64, f64), v: (f64, f64)| {
        let (dx, dy) = (v.0 - u.0, v.1 - u.1);
        let len2 = dx * dx + dy * dy;
        let t = if len2 > 0.0 {
            (((q.0 - u.0) * dx + (q.1 - u.1) * dy) / len2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        ((q.0 - u.0 - t * dx).powi(2) + (q.1 - u.1 - t * dy).powi(2)).sqrt()
    };
    seg(a, b).min(seg(b, c)).min(seg(c, a))
}

/// The closed form for a ball of radius `r` at a distance `d` from a wall.
fn corner_gap_mm(r: f64, d: f64) -> f64 {
    let lean = r - d;
    r - (r * r - lean * lean).max(0.0).sqrt()
}

/// One measured top cell, with its two distances.
struct TopCell {
    x: f64,
    y: f64,
    z: f64,
    gap: Option<f32>,
    floor: f32,
    /// Distance to the outer outline (the footprint is the bbox rectangle;
    /// the test checks that).
    to_outline: f64,
    /// Distance to the nearest higher flat top, i.e. to the foot of the wall
    /// that rises to it; `INFINITY` on the highest face.
    to_wall: f64,
    /// Height of that wall above this face.
    wall_height: f64,
}

fn top_cells(mesh: &TriangleMesh, index: &SpatialIndex, map: &ReachMap) -> Vec<TopCell> {
    let tops: Vec<&Triangle> = mesh
        .faces
        .iter()
        .filter(|t| t.normal.z >= FLAT_UP_NZ)
        .collect();
    let bbox = &mesh.bbox;
    let mut cells = Vec::new();
    for row in 0..map.grid.ny {
        for col in 0..map.grid.nx {
            let (x, y) = (map.grid.x_of(col), map.grid.y_of(row));
            let Some(z) = surface_z_at(x, y, mesh, index) else {
                continue;
            };
            let on_flat_top = tops.iter().any(|t| {
                (t.v[0].z - z).abs() < SAME_Z_MM && xy_distance_to_triangle(t, x, y) == 0.0
            });
            if !on_flat_top {
                continue;
            }
            let mut to_wall = f64::INFINITY;
            let mut wall_height = 0.0;
            for t in &tops {
                let higher = t.v[0].z - z;
                if higher <= SAME_Z_MM {
                    continue;
                }
                let d = xy_distance_to_triangle(t, x, y);
                if d < to_wall {
                    to_wall = d;
                    wall_height = higher;
                }
            }
            let to_outline = (x - bbox.min.x)
                .min(bbox.max.x - x)
                .min(y - bbox.min.y)
                .min(bbox.max.y - y);
            let i = row * map.grid.nx + col;
            cells.push(TopCell {
                x,
                y,
                z,
                gap: map.cells[i],
                floor: map.cell_floor_mm.get(i).copied().unwrap_or(f32::NAN),
                to_outline,
                to_wall,
                wall_height,
            });
        }
    }
    cells
}

fn build() -> (TriangleMesh, SpatialIndex, ReachMap, f64) {
    let mesh = stepped_mesh();
    let index = SpatialIndex::build_auto(&mesh);
    let ball = BallEndmill::new(LIVE_BALL_DIAMETER_MM, 25.0);
    let params = ReachMapParams::for_cutter(&ball, DEFAULT_REACH_TOLERANCE_MM);
    let map = compute_reach_map(&mesh, &index, &ball, &params, &NeverCancel).unwrap();
    (mesh, index, map, ball.envelope_radius_mm())
}

/// Claim 1: the flat tops read reached up to the outer outline.
#[test]
fn every_top_cell_within_one_radius_of_the_outline_reads_reached() {
    let (mesh, index, map, r) = build();
    let cells = top_cells(&mesh, &index, &map);

    // The fixture precondition: the footprint is the bbox rectangle, so the
    // distance to the outline is the distance to the nearest bbox side.
    let bbox = &mesh.bbox;
    let inset = map.grid.cell_mm / 2.0;
    for (x, y) in [
        (bbox.min.x + inset, bbox.min.y + inset),
        (bbox.max.x - inset, bbox.min.y + inset),
        (bbox.min.x + inset, bbox.max.y - inset),
        (bbox.max.x - inset, bbox.max.y - inset),
    ] {
        assert!(
            surface_z_at(x, y, &mesh, &index).is_some(),
            "the stepped block footprint must fill its bbox; no surface at ({x:.2}, {y:.2})"
        );
    }

    let band: Vec<&TopCell> = cells
        .iter()
        .filter(|c| c.to_outline <= r && c.to_wall >= r)
        .collect();
    let distinct_tops = {
        let mut zs: Vec<f64> = band.iter().map(|c| c.z).collect();
        zs.sort_by(f64::total_cmp);
        zs.dedup_by(|a, b| (*a - *b).abs() < SAME_Z_MM);
        zs
    };
    eprintln!(
        "R7d edge band: cell {:.3} mm, R {r:.3} mm, tol {:.3} mm, floor {:.4} mm \
         (profile {:.4}, curvature p95 {:.4}); {} band cells on tops {:?}",
        map.grid.cell_mm,
        map.tolerance_mm,
        map.discretisation_floor_mm,
        map.profile_floor_mm,
        map.curvature_floor_p95_mm,
        band.len(),
        distinct_tops
    );
    // The published floor states the surface as a whole, so the outline
    // ring R7d added must not move it: 1.0621 mm is what this map published
    // at ce32777f3, before R7d (its wall-foot jumps). Slack: the profile
    // floor at this cell.
    let before = 1.0621;
    assert!(
        (map.discretisation_floor_mm - before).abs() <= map.profile_floor_mm,
        "the published floor moved: {:.4} mm against {before:.4} mm before R7d",
        map.discretisation_floor_mm
    );
    assert_eq!(
        distinct_tops.len(),
        3,
        "the edge band must touch all three top faces of the stepped block"
    );

    let mut worst = 0.0f32;
    for c in &band {
        let Some(gap) = c.gap else {
            panic!(
                "top cell ({:.2}, {:.2}) at Z {:.1} is {:.2} mm from the outline and \
                 {:.2} mm from any wall, and the map does not measure it. A ball reaches \
                 every point of a flat top (R7d).",
                c.x, c.y, c.z, c.to_outline, c.to_wall
            );
        };
        worst = worst.max(gap);
        assert!(
            f64::from(gap) <= map.tolerance_mm,
            "top cell ({:.2}, {:.2}) at Z {:.1}, {:.2} mm from the outline, reads a \
             gap of {gap:.4} mm against a {:.3} mm bar (cell floor {:.4} mm)",
            c.x,
            c.y,
            c.z,
            c.to_outline,
            map.tolerance_mm,
            c.floor
        );
    }
    eprintln!("R7d edge band: worst gap {worst:.5} mm");
}

/// Claim 2: the inside corner at a wall foot keeps the ball's radius.
#[test]
fn the_inside_corner_at_a_wall_foot_keeps_the_ball_radius() {
    let (mesh, index, map, r) = build();
    let cells = top_cells(&mesh, &index, &map);
    let corner: Vec<&TopCell> = cells
        .iter()
        // Within R of the outline the wall ends and the ball can come in
        // from past the outline beside it, so the closed form does not hold
        // there; those cells are claim 1's business only where they are also
        // R from the wall.
        .filter(|c| c.to_wall > 0.0 && c.to_wall < r && c.to_outline >= r)
        .collect();
    assert!(
        !corner.is_empty(),
        "the stepped block must give cells within one radius of a wall foot"
    );

    // One row per distance, so the table reads as the closed form.
    let mut rows: Vec<(f64, f64, f32, f32, f32)> = Vec::new();
    for c in &corner {
        assert!(
            c.wall_height >= r,
            "the closed form needs a wall at least R high; {:.2} mm at ({:.2}, {:.2})",
            c.wall_height,
            c.x,
            c.y
        );
        let expected = corner_gap_mm(r, c.to_wall);
        let Some(gap) = c.gap else {
            panic!(
                "corner cell ({:.2}, {:.2}) {:.2} mm from the wall is not measured",
                c.x, c.y, c.to_wall
            );
        };
        let excess = f64::from(gap) - expected;
        // A sampled minimum never under-states the gap. The f32 cell store
        // rounds; 1e-5 mm is far under any figure here.
        assert!(
            excess >= -1e-5,
            "corner cell ({:.2}, {:.2}) {:.3} mm from the wall reads {gap:.4} mm, under \
             the closed form {expected:.4} mm",
            c.x,
            c.y,
            c.to_wall
        );
        // The band keeps its gap: a cell whose true gap clears the bar
        // reads past the bar, never reached.
        if expected > map.tolerance_mm {
            assert!(
                f64::from(gap) > map.tolerance_mm,
                "corner cell ({:.2}, {:.2}) {:.3} mm from the wall reads {gap:.4} mm, \
                 reached, where the ball leaves {expected:.4} mm",
                c.x,
                c.y,
                c.to_wall
            );
        }
        // No reading passes the gap at the wall itself, `gap(0) = R`.
        //
        // The over-read inside that cap is NOT bounded by the cell floor
        // here. `tip_z` JUMPS at the ball's wall stand-off (`d = R`): one
        // side the ball rests on the lower face, the other it rides the top
        // edge of the wall, `R − sqrt(R² − d²)` under that edge. Bilinear
        // reading across the jump over-reads by up to 1.39 mm at d = 0.4 mm
        // (0.6 mm cell), above the published floor of 1.06 mm. That is the
        // kink regime `curvature_floor_plane` documents as uncovered; it is
        // not the rim, and R7d does not change it.
        assert!(
            f64::from(gap) <= r + 1e-5,
            "corner cell ({:.2}, {:.2}) {:.3} mm from the wall reads {gap:.4} mm, more \
             than the gap at the wall itself ({r:.4} mm)",
            c.x,
            c.y,
            c.to_wall
        );
        if let Some(row) = rows.iter_mut().find(|row| (row.0 - c.to_wall).abs() < 1e-9) {
            row.2 = row.2.min(gap);
            row.3 = row.3.max(gap);
            row.4 = row.4.max(c.floor);
        } else {
            rows.push((c.to_wall, expected, gap, gap, c.floor));
        }
    }
    rows.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (d, expected, lo, hi, floor) in rows {
        eprintln!(
            "R7d corner: d {d:.3} mm, closed form {expected:.4} mm, map {lo:.4}..{hi:.4} mm, \
             cell floor up to {floor:.4} mm"
        );
    }
}
