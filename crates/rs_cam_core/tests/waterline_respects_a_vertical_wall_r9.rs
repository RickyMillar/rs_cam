//! R9 — the waterline keeps the cutter radius from a vertical wall.
//!
//! The Corne case (`planning/corne_case_analysis_2026-09-18/ANALYSIS.md`
//! §4.5): a 6 mm flat end mill waterline on a tray with 3 mm vertical walls
//! ran along the outer face at the right offset, and at a 7 mm pitch notched
//! 2 mm INTO the wall, on every level from 18 to 0. The mesh has 4 triangles
//! on that face; the wall edges are 55.26 mm long.
//!
//! The mechanism is in `surface/pushcutter.rs::edge_push_single`. It sampled
//! the edge at nine coarse points over the WHOLE edge and bisected only
//! between two coarse samples that disagreed. An edge that crosses the fiber
//! touches the cutter over a window about `2·w` long (6 mm); the coarse pitch
//! on a 55.26 mm edge is 6.9 mm, so on every fiber row at that pitch the
//! window fell between two samples, no sample made contact, no bisection
//! ran, and the edge added nothing. Every wall edge shares the same Y span,
//! so on those rows the fiber passed clean through the wall. The weave's
//! midpoint fallback put the notch at `row + 0.25`, which is where the
//! G-code shows it. §4.5's stated root cause (`facet_push` skips vertical
//! facets) is a real but latent second hole on that part: the wall faces
//! span Z 2..16.5, inside the 25 mm slab, so their edges would have blocked
//! had the sampler found them. It bites when a wall stands taller than the
//! cutting length, which the second case below pins.
//!
//! The fixture is a closed, open-top tray: an outer box with a cavity 3 mm
//! inside it and a 3 mm floor, so every horizontal face is within one cutter
//! radius of an edge. A SOLID box is not usable here: a horizontal facet
//! above the level contributes nothing through `facet_push`, so the interior
//! of a solid reads free and the waterline emits a loop inside the material.
//! That is a separate finding, not this file's claim.
//!
//! Levels: both cases run inside the cavity height, so the expected result is
//! exactly two loops — the outer offset and the cavity offset.
//!
//! The outer loop needs the fiber grid to reach past the outer CL. The
//! waterline lays its fibers over `bbox ± r`, so the outer CL of a
//! bbox-face wall sits exactly at the fiber end, and the weave has no free
//! node outside it to place a crossing. The old sampler's bisection left the
//! blocked interval a hair short of the fiber end, which hid this; the exact
//! window ends expose it, and `waterline_contours` now pads the grid by one
//! sampling cell on every side. The two-loop assertion below pins that pad
//! as well as the sampler.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::Path;

use rs_cam_core::geo::P3;
use rs_cam_core::geometry::fiber::Fiber;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::ops::waterline::waterline_contours;
use rs_cam_core::surface::pushcutter::batch_push_cutter;
use rs_cam_core::tool::{FlatEndmill, MillingCutter};

/// Distance tolerance on a CL point, mm.
const TOL: f64 = 0.05;

/// A closed, open-top tray.
///
/// The outer box is `±hx × ±hy`, Z `0..height`. The cavity is `wall` inside
/// the outer walls and starts at Z `floor`. Every wall face is one quad of
/// two triangles with a long diagonal, the way an STL exporter writes a
/// planar face.
struct Tray {
    hx: f64,
    hy: f64,
    height: f64,
    wall: f64,
    floor: f64,
}

impl Tray {
    fn mesh(&self) -> TriangleMesh {
        let (hx, hy, h) = (self.hx, self.hy, self.height);
        let (ix, iy) = (hx - self.wall, hy - self.wall);
        let mut vertices: Vec<P3> = Vec::new();
        let mut triangles: Vec<[u32; 3]> = Vec::new();
        let mut v = |x: f64, y: f64, z: f64| -> u32 {
            vertices.push(P3::new(x, y, z));
            (vertices.len() - 1) as u32
        };
        let ob = [
            v(-hx, -hy, 0.0),
            v(hx, -hy, 0.0),
            v(hx, hy, 0.0),
            v(-hx, hy, 0.0),
        ];
        let ot = [v(-hx, -hy, h), v(hx, -hy, h), v(hx, hy, h), v(-hx, hy, h)];
        let fl = self.floor;
        let ib = [
            v(-ix, -iy, fl),
            v(ix, -iy, fl),
            v(ix, iy, fl),
            v(-ix, iy, fl),
        ];
        let it = [v(-ix, -iy, h), v(ix, -iy, h), v(ix, iy, h), v(-ix, iy, h)];
        let mut quad = |a: u32, b: u32, c: u32, d: u32| {
            triangles.push([a, b, c]);
            triangles.push([a, c, d]);
        };
        // Bottom, facing down; floor, facing up.
        quad(ob[0], ob[3], ob[2], ob[1]);
        quad(ib[0], ib[1], ib[2], ib[3]);
        for k in 0..4 {
            let n = (k + 1) % 4;
            // Outer wall, facing out; inner wall, facing into the cavity;
            // rim, facing up.
            quad(ob[k], ob[n], ot[n], ot[k]);
            quad(ib[n], ib[k], it[k], it[n]);
            quad(ot[k], ot[n], it[n], it[k]);
        }
        TriangleMesh::from_raw(vertices, triangles)
    }

    /// Outer face rectangle, half extents.
    fn outer(&self) -> (f64, f64) {
        (self.hx, self.hy)
    }

    /// Cavity rectangle, half extents.
    fn cavity(&self) -> (f64, f64) {
        (self.hx - self.wall, self.hy - self.wall)
    }
}

/// Is `p` inside the axis-aligned rectangle of half extents `(hx, hy)`?
fn inside_rect(p: &P3, hx: f64, hy: f64) -> bool {
    p.x.abs() <= hx && p.y.abs() <= hy
}

/// Distance from `p` to the axis-aligned rectangle of half extents
/// `(hx, hy)`; zero inside. The outward offset of a rectangle has rounded
/// corners, so the outer loop is checked by distance, not by a larger
/// rectangle.
fn dist_to_rect(p: &P3, hx: f64, hy: f64) -> f64 {
    let dx = (p.x.abs() - hx).max(0.0);
    let dy = (p.y.abs() - hy).max(0.0);
    (dx * dx + dy * dy).sqrt()
}

/// The tray claim at one level: exactly two loops; every outer-loop point
/// is at least `r - TOL` outside the outer faces; every inner-loop point is
/// at least `r - TOL` inside the cavity faces; and both loops reach their
/// full offset so that the claim is not vacuous.
fn assert_tray_level(tray: &Tray, cutter: &dyn MillingCutter, z: f64, sampling: f64) {
    let mesh = tray.mesh();
    let index = SpatialIndex::build(&mesh, 5.0);
    let r = cutter.radius();
    let contours = waterline_contours(&mesh, &index, cutter, z, sampling);
    assert_eq!(
        contours.len(),
        2,
        "z={z}: expected the outer loop and the cavity loop, got {} loops",
        contours.len()
    );

    let (ox, oy) = tray.outer();
    let (cx, cy) = tray.cavity();
    let mut saw_outer = false;
    let mut saw_inner = false;
    for contour in &contours {
        // The weave returns an open chain as well as a closed one; a closed
        // loop ends within a cell of where it started.
        let first = contour[0];
        let last = contour[contour.len() - 1];
        let gap = ((first.x - last.x).powi(2) + (first.y - last.y).powi(2)).sqrt();
        assert!(
            gap <= 2.0 * sampling,
            "z={z}: a loop of {} points does not close (gap {gap:.3} mm)",
            contour.len()
        );
        let max_x = contour.iter().map(|p| p.x.abs()).fold(0.0, f64::max);
        let max_y = contour.iter().map(|p| p.y.abs()).fold(0.0, f64::max);
        if max_x > ox {
            // The outer loop: nothing inside the outer faces inflated by r.
            saw_outer = true;
            for p in contour {
                assert!(
                    dist_to_rect(p, ox, oy) >= r - TOL,
                    "z={z}: outer-loop CL ({:.3}, {:.3}) is closer than {r} to an outer wall",
                    p.x,
                    p.y
                );
            }
            assert!(
                max_x >= ox + r - TOL && max_y >= oy + r - TOL,
                "z={z}: outer loop does not reach its full offset (max |x| {max_x:.3}, max |y| {max_y:.3})"
            );
        } else {
            // The cavity loop: nothing outside the cavity faces deflated by r.
            saw_inner = true;
            for p in contour {
                assert!(
                    inside_rect(p, cx - r + TOL, cy - r + TOL),
                    "z={z}: cavity-loop CL ({:.3}, {:.3}) is closer than {r} to a cavity wall",
                    p.x,
                    p.y
                );
            }
            assert!(
                max_x >= cx - r - TOL && max_y >= cy - r - TOL,
                "z={z}: cavity loop does not reach its full offset (max |x| {max_x:.3}, max |y| {max_y:.3})"
            );
        }
    }
    assert!(
        saw_outer && saw_inner,
        "z={z}: one loop of each kind expected"
    );
}

/// The Corne-shaped case: wall edges much longer than eight cutter
/// diameters, every wall edge inside the slab. Only the edge sampler stands
/// between the fiber and the wall.
#[test]
fn tray_levels_keep_the_cutter_radius_from_every_wall() {
    let tray = Tray {
        hx: 60.3,
        hy: 30.3,
        height: 20.0,
        wall: 3.0,
        floor: 3.0,
    };
    let cutter = FlatEndmill::new(6.0, 25.0);
    for z in [15.0, 5.0] {
        assert_tray_level(&tray, &cutter, z, 0.5);
    }
}

/// The R9 hole proper: the wall stands taller than the cutting length, so
/// the top edges leave the slab and only the vertical-facet arm can block
/// the middle of a wall triangle.
#[test]
fn tall_wall_beyond_the_cutting_length_still_blocks_the_fiber() {
    let tray = Tray {
        hx: 60.3,
        hy: 30.3,
        height: 30.0,
        wall: 3.0,
        floor: 3.0,
    };
    let cutter = FlatEndmill::new(6.0, 12.0);
    assert_tray_level(&tray, &cutter, 5.0, 0.5);
}

/// The fiber-level form of the first case, without the weave: on every row
/// that crosses the long walls, the X-fiber is blocked one radius short of
/// each wall face and free one radius past it.
#[test]
fn every_x_fiber_row_is_blocked_across_both_long_walls() {
    let tray = Tray {
        hx: 60.3,
        hy: 30.3,
        height: 20.0,
        wall: 3.0,
        floor: 3.0,
    };
    let mesh = tray.mesh();
    let index = SpatialIndex::build(&mesh, 5.0);
    let cutter = FlatEndmill::new(6.0, 25.0);
    let r = cutter.radius();
    let z = 15.0;
    let sampling = 0.5;
    let (ox, _) = tray.outer();
    let (cx, cy) = tray.cavity();

    // The waterline's own fiber layout.
    let x_min = mesh.bbox.min.x - r;
    let x_max = mesh.bbox.max.x + r;
    let y_min = mesh.bbox.min.y - r;
    let y_max = mesh.bbox.max.y + r;
    let ny = ((y_max - y_min) / sampling).ceil() as usize + 1;
    let mut fibers: Vec<Fiber> = (0..ny)
        .map(|i| Fiber::new_x(y_min + i as f64 * sampling, z, x_min, x_max))
        .collect();
    batch_push_cutter(&mut fibers, &mesh, &index, &cutter);

    let mut rows_checked = 0usize;
    for fiber in &fibers {
        let y = fiber.p1.y;
        // Rows across the cavity, away from the short walls' corner arcs.
        if y.abs() > cy - r - TOL {
            continue;
        }
        rows_checked += 1;
        let probe = |x: f64| fiber.is_blocked(fiber.tval(&P3::new(x, y, z)));
        for sign in [-1.0, 1.0] {
            let outer_face = sign * ox;
            let cavity_face = sign * cx;
            assert!(
                probe(outer_face + sign * (r - TOL)),
                "row y={y:.3}: free {r} short of the outer face at x={outer_face}"
            );
            assert!(
                !probe(outer_face + sign * (r + TOL)),
                "row y={y:.3}: blocked past the outer face at x={outer_face}"
            );
            assert!(
                probe(cavity_face - sign * (r - TOL)),
                "row y={y:.3}: free {r} short of the cavity face at x={cavity_face}"
            );
            assert!(
                !probe(cavity_face - sign * (r + TOL)),
                "row y={y:.3}: blocked past the cavity face at x={cavity_face}"
            );
        }
        assert!(!probe(0.0), "row y={y:.3}: the cavity centre is blocked");
    }
    assert!(
        rows_checked > 50,
        "only {rows_checked} rows checked; fixture too small"
    );
}

/// The Corne STL itself, when it is on this machine. Skips silently when the
/// file is absent. Run with `-- --ignored`.
///
/// Also prints the X-fiber intervals at the first notched row and the
/// Y-fiber intervals at three columns inside the right wall, for a reading
/// by eye.
#[test]
#[ignore = "needs /home/ricky/Downloads/Corne Case Hex.stl; evidence run, not a gate"]
#[allow(clippy::print_stdout)] // SAFETY: an evidence run whose output is read off the terminal
fn corne_case_hex_at_z10_has_no_point_inside_the_right_wall() {
    let path = Path::new("/home/ricky/Downloads/Corne Case Hex.stl");
    if !path.exists() {
        return;
    }
    let mesh = TriangleMesh::from_stl(path).expect("Corne STL loads");
    let index = SpatialIndex::build_auto(&mesh);
    let cutter = FlatEndmill::new(6.0, 25.0);
    let z = 10.0;
    let sampling = 0.5;

    // Diagnostic: the fiber intervals around the first notched row.
    let r = cutter.radius();
    let x_min = mesh.bbox.min.x - r;
    let x_max = mesh.bbox.max.x + r;
    let y_min = mesh.bbox.min.y - r;
    let y_max = mesh.bbox.max.y + r;
    let ny = ((y_max - y_min) / sampling).ceil() as usize + 1;
    let mut x_fibers: Vec<Fiber> = (0..ny)
        .map(|i| Fiber::new_x(y_min + i as f64 * sampling, z, x_min, x_max))
        .collect();
    let mut y_fibers: Vec<Fiber> = [146.3, 146.8, 147.3]
        .iter()
        .map(|&x| Fiber::new_y(x, z, y_min, y_max))
        .collect();
    batch_push_cutter(&mut x_fibers, &mesh, &index, &cutter);
    batch_push_cutter(&mut y_fibers, &mesh, &index, &cutter);
    for fiber in x_fibers.iter().filter(|f| (f.p1.y + 18.07).abs() < 0.6) {
        let spans: Vec<String> = fiber
            .intervals()
            .iter()
            .map(|iv| {
                format!(
                    "[{:.2}, {:.2}]",
                    fiber.point(iv.lower).x,
                    fiber.point(iv.upper).x
                )
            })
            .collect();
        println!(
            "X-fiber y={:.4} z={z}: blocked x {}",
            fiber.p1.y,
            spans.join(" ")
        );
    }
    for fiber in &y_fibers {
        let spans: Vec<String> = fiber
            .intervals()
            .iter()
            .map(|iv| {
                format!(
                    "[{:.2}, {:.2}]",
                    fiber.point(iv.lower).y,
                    fiber.point(iv.upper).y
                )
            })
            .collect();
        println!(
            "Y-fiber x={:.1} z={z}: blocked y {}",
            fiber.p1.x,
            spans.join(" ")
        );
    }

    let contours = waterline_contours(&mesh, &index, &cutter, z, sampling);
    let inside_wall: Vec<&P3> = contours
        .iter()
        .flatten()
        .filter(|p| p.x > 145.3 && p.x < 147.8 && p.y > -70.0 && p.y < -15.0)
        .collect();
    for p in &inside_wall {
        println!("CL inside the right wall: ({:.3}, {:.3})", p.x, p.y);
    }
    assert!(
        inside_wall.is_empty(),
        "{} CL points inside the right wall at z={z}",
        inside_wall.len()
    );
}
