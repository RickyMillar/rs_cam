//! R1 — the silhouette MACHINING BOUNDARY has no holes.
//!
//! `planning/corne_case_analysis_2026-09-18/ANALYSIS.md` §3, §5, R1.
//!
//! `model_silhouette` keeps every inner loop as a hole. adaptive3d's
//! pre-clip clears every hole cell to the stock bottom, so a through-hole
//! becomes an island of "no material" at every Z level, and the
//! post-generation clip turns every move inside it into a rapid. On the
//! Corne case the roughing wove around 42 hexes it never cut.
//!
//! The ruling: for a machining boundary a through-hole is material. A
//! keep-out is the mechanism for "do not cut here". The fix lives in ONE
//! helper, `geometry::boundary::silhouette_machining_outline`, which the
//! session's `resolve_containment_polygon` calls for the `ModelSilhouette`
//! source. Both boundary consumers (adaptive3d's pre-clip and the
//! post-generation clip) resolve through that one function.
//!
//! The raw `model_silhouette` STILL reports the hole. This file pins both
//! halves, so a change to either is visible:
//!
//! 1. `model_silhouette` on a plate with a square through-hole has one
//!    polygon with one hole, and `contains_point` is `false` at the hole
//!    centre.
//! 2. `silhouette_machining_outline` of that silhouette has zero holes,
//!    `contains_point` is `true` at the hole centre and on the plate, and
//!    `false` outside the plate.
//! 3. The rank is by EXTERIOR area: a plate with a large hole outranks a
//!    smaller solid polygon whose NET area is larger.
//! 4. The resolution site calls the helper. `resolve_containment_polygon`
//!    is `pub(crate)`, so a source scan of `session/compute/generation.rs`
//!    pins the call, anchored on the `cached_silhouette` read beside it.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::geo::{P2, P3};
use rs_cam_core::geometry::boundary::{model_silhouette, silhouette_machining_outline};
use rs_cam_core::mesh::TriangleMesh;
use rs_cam_core::polygon::Polygon2;

/// Outer half-width of the plate (mm).
const OUTER: f64 = 20.0;
/// Half-width of the square through-hole (mm).
const INNER: f64 = 8.0;
/// Silhouette cell (mm). The hole is 16 cells wide, so the raster sees it.
const CELL: f64 = 1.0;

/// A flat square plate at Z = 0 with a square through-hole, centred at the
/// origin: a ring of four trapezoids, two triangles each. Every triangle
/// winds CCW seen from +Z.
fn plate_with_square_hole() -> TriangleMesh {
    let o = OUTER;
    let i = INNER;
    let vertices = vec![
        // 0..4: outer corners, CCW from (-o, -o).
        P3::new(-o, -o, 0.0),
        P3::new(o, -o, 0.0),
        P3::new(o, o, 0.0),
        P3::new(-o, o, 0.0),
        // 4..8: inner corners, CCW from (-i, -i).
        P3::new(-i, -i, 0.0),
        P3::new(i, -i, 0.0),
        P3::new(i, i, 0.0),
        P3::new(-i, i, 0.0),
    ];
    let triangles = vec![
        // South band.
        [0, 1, 5],
        [0, 5, 4],
        // East band.
        [1, 2, 6],
        [1, 6, 5],
        // North band.
        [2, 3, 7],
        [2, 7, 6],
        // West band.
        [3, 0, 4],
        [3, 4, 7],
    ];
    TriangleMesh::from_raw(vertices, triangles)
}

fn raw_silhouette() -> Vec<Polygon2> {
    model_silhouette(&plate_with_square_hole(), Some(CELL))
}

/// The plate the raster produced, before the rank and the strip.
fn raw_plate(polys: &[Polygon2]) -> &Polygon2 {
    polys
        .iter()
        .max_by(|a, b| {
            a.signed_area()
                .abs()
                .partial_cmp(&b.signed_area().abs())
                .unwrap()
        })
        .expect("the silhouette of an 8-triangle plate is not empty")
}

/// Half 1. The raw silhouette keeps the hole. This is the pre-R1 boundary
/// behaviour, and it stays true of `model_silhouette` itself: R1 moved the
/// rule to the resolution site, not into the raster.
#[test]
fn model_silhouette_still_reports_the_through_hole() {
    let polys = raw_silhouette();
    let plate = raw_plate(&polys);
    assert_eq!(
        plate.holes.len(),
        1,
        "the raw silhouette of a plate with one through-hole carries one hole; \
         got {} holes over {} polygons",
        plate.holes.len(),
        polys.len()
    );
    assert!(
        !plate.contains_point(&P2::new(0.0, 0.0)),
        "the raw silhouette excludes the hole centre"
    );
    // The hole is roughly (2·INNER)² and the exterior roughly (2·OUTER)².
    let hole_area = plate.signed_area().abs() - plate.area();
    let expect = (2.0 * INNER) * (2.0 * INNER);
    assert!(
        (hole_area - expect).abs() < 0.25 * expect,
        "hole area {hole_area:.1} is far from the modelled {expect:.1}"
    );
}

/// Half 2, the R1 rule. The machining outline drops the hole and contains
/// its centre; the plate and the outside are unchanged.
#[test]
fn machining_outline_has_no_holes_and_contains_the_hole_centre() {
    let polys = raw_silhouette();
    let outline = silhouette_machining_outline(&polys).expect("a plate has an outline");

    assert!(
        outline.holes.is_empty(),
        "R1: the machining outline carries no holes; got {}",
        outline.holes.len()
    );
    assert!(outline.closed, "the outline is a closed loop");
    assert!(
        outline.has_correct_winding(),
        "the outline keeps the CCW exterior"
    );
    assert!(
        outline.contains_point(&P2::new(0.0, 0.0)),
        "R1: the hole centre is INSIDE the machining boundary (a through-hole is material)"
    );
    assert!(
        outline.contains_point(&P2::new(OUTER - 3.0, 0.0)),
        "a point on the plate stays inside"
    );
    assert!(
        !outline.contains_point(&P2::new(OUTER + 5.0, 0.0)),
        "a point outside the plate stays outside"
    );

    // The exterior is the raw plate's exterior, vertex for vertex.
    let plate = raw_plate(&polys);
    assert_eq!(
        outline.exterior.len(),
        plate.exterior.len(),
        "the outline is the raw exterior with nothing added"
    );
    assert!(
        (outline.signed_area() - plate.signed_area()).abs() < 1e-9,
        "the outline's area is the raw exterior area"
    );
}

/// Half 3. The rank is by exterior area. `largest_by_area` ranks by NET
/// area (exterior minus holes) and would pick the small solid square here;
/// under R1 the holes are material, so the plate wins.
#[test]
fn the_rank_is_by_exterior_area_not_net_area() {
    // A 10×10 plate with a 9×9 hole: exterior 100, net 19.
    let plate = Polygon2::with_holes(
        vec![
            P2::new(0.0, 0.0),
            P2::new(10.0, 0.0),
            P2::new(10.0, 10.0),
            P2::new(0.0, 10.0),
        ],
        vec![vec![
            P2::new(0.5, 0.5),
            P2::new(0.5, 9.5),
            P2::new(9.5, 9.5),
            P2::new(9.5, 0.5),
        ]],
    );
    // A 6×6 solid square: exterior 36, net 36.
    let solid = Polygon2::rectangle(20.0, 20.0, 26.0, 26.0);
    assert!(
        plate.area() < solid.area(),
        "the fixture must make net area and exterior area disagree"
    );
    let pair = [plate.clone(), solid.clone()];
    let net_pick = rs_cam_core::polygon::largest_by_area(&pair).expect("two polygons");
    assert!(
        (net_pick.signed_area() - solid.signed_area()).abs() < 1e-9,
        "largest_by_area picks the solid square by net area; the fixture is not vacuous"
    );

    let outline = silhouette_machining_outline(&[plate.clone(), solid]).expect("two polygons");
    assert!(
        (outline.signed_area() - plate.signed_area()).abs() < 1e-9,
        "R1 ranks by exterior area, so the plate wins; got area {}",
        outline.signed_area()
    );
    assert!(outline.holes.is_empty());
}

/// An empty silhouette has no outline. The caller decides the fallback (the
/// session takes the stock rectangle).
#[test]
fn an_empty_silhouette_has_no_outline() {
    assert!(silhouette_machining_outline(&[]).is_none());
}

/// Half 4. The `ModelSilhouette` arm of `resolve_containment_polygon`
/// routes the cached silhouette through the helper, and no longer picks
/// the raw polygon with `largest_by_area`. The three tests above stay
/// green with the site reverted; this one goes red.
#[test]
fn the_resolution_site_calls_the_helper() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/session/compute/generation.rs");
    let text = std::fs::read_to_string(&path).expect("read the resolution site");
    let code_lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.starts_with("//"))
        .collect();
    // Non-vacuity anchor: the silhouette arm still reads the memo.
    assert!(
        code_lines
            .iter()
            .any(|l| l.contains("cached_silhouette(m)")),
        "the ModelSilhouette arm no longer reads `cached_silhouette`; the scan is vacuous"
    );
    assert!(
        code_lines
            .iter()
            .any(|l| l.contains("silhouette_machining_outline(&silhouettes)")),
        "R1: the ModelSilhouette arm must resolve through `silhouette_machining_outline`"
    );
    assert!(
        !code_lines
            .iter()
            .any(|l| l.contains("largest_by_area(&silhouettes)")),
        "R1: the ModelSilhouette arm still picks the raw polygon with `largest_by_area`"
    );
}
