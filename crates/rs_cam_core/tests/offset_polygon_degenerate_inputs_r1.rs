//! R1 (tech-debt review 2026-06-10) — `offset_polygon` must survive
//! degenerate inputs that crash cavalier_contours 0.7.0's offset stack
//! in debug builds (`debug_assert!`s) and silently violate its
//! documented invariants in release.
//!
//! Two captured classes:
//! 1. WANAKA Back Rough terrain slice (86-vertex exterior, 13 holes,
//!    inward 5.53 mm) — `Shape::parallel_offset` slice stitching trips
//!    "start index should be less than or equal to end index" in
//!    `pline_view.rs`. Captured live from the optimizer grid search;
//!    asset at `test_data/cavalier_panic_polygon_r1.json`. Contained by
//!    the `catch_unwind` chokepoint in `offset_polygon`.
//! 2. Repeat-position vertexes fed back into `parallel_offset` by
//!    chained offsets (`pocket_offsets` at the 0.05 mm search floor;
//!    that function was retired at Checkpoint D and the case is now
//!    driven through its replacement, `OffsetRingSet`) —
//!    "input assumed to not have repeat position vertexes" in
//!    `pline_offset.rs`. Root-fixed by `remove_repeat_pos` dedupe
//!    before every cavalier offset call.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;
use common::repo_root;

use rs_cam_core::geo::P2;
use rs_cam_core::polygon::{FlattenPolicy, OffsetRingSet, Polygon2, offset_polygon};

fn load_captured_polygon() -> (Polygon2, f64) {
    let raw = std::fs::read_to_string(repo_root().join("test_data/cavalier_panic_polygon_r1.json"))
        .expect("read captured cavalier panic input");
    let v: serde_json::Value = serde_json::from_str(&raw).expect("parse capture");
    let distance = v["distance"].as_f64().expect("distance");
    let ring = |val: &serde_json::Value| -> Vec<P2> {
        val.as_array()
            .expect("ring array")
            .iter()
            .map(|p| P2::new(p[0].as_f64().unwrap(), p[1].as_f64().unwrap()))
            .collect()
    };
    let mut poly = Polygon2::new(ring(&v["exterior"]));
    poly.closed = v["closed"].as_bool().unwrap_or(true);
    poly.holes = v["holes"]
        .as_array()
        .expect("holes array")
        .iter()
        .map(ring)
        .collect();
    (poly, distance)
}

#[test]
fn captured_wanaka_terrain_slice_offsets_without_panicking() {
    let (poly, distance) = load_captured_polygon();
    assert_eq!(poly.exterior.len(), 86, "capture asset changed shape");
    assert_eq!(poly.holes.len(), 13, "capture asset changed shape");
    // The bar is "no panic". The offset may legitimately come back
    // empty (containment maps a cavalier panic to a collapsed offset).
    let result = offset_polygon(&poly, distance);
    // Nearby distances walk different code paths through cavalier's
    // slice stitching — cover a band around the captured crash point.
    for delta in [-0.5, -0.1, 0.1, 0.5] {
        let _ = offset_polygon(&poly, distance + delta);
    }
    drop(result);
}

#[test]
fn repeat_position_vertexes_are_deduped_before_offsetting() {
    // 20 mm square with every corner doubled and a sub-epsilon sliver
    // vertex — the shape `pocket_offsets` feeds back into cavalier at
    // tiny stepovers.
    let mut verts = Vec::new();
    for &(x, y) in &[(0.0, 0.0), (20.0, 0.0), (20.0, 20.0), (0.0, 20.0)] {
        verts.push(P2::new(x, y));
        verts.push(P2::new(x, y)); // exact repeat
        verts.push(P2::new(x + 4.0e-6, y)); // within cavalier's 1e-5 eps
    }
    let poly = Polygon2::new(verts);
    let result = offset_polygon(&poly, 0.05);
    assert!(
        !result.is_empty(),
        "a healthy 20 mm square inset by 0.05 mm must survive"
    );

    // Same contract through the chained-offset path that originally
    // crashed: tiny-stepover pocket rings re-offset cavalier output.
    //
    // Wave 14: that path was `polygon::pocket_offsets`, retired at
    // Checkpoint D (no production caller; it duplicated
    // `pocket_contours_with_cancel`'s loop without its cancel hook, and it is
    // the function that ran 13 minutes to 386 MB in the M5 study). The
    // cascade it is replaced by is `OffsetRingSet`, and the R1 contract is
    // unchanged and re-asserted verbatim below: chained tiny-stepover offsets
    // of a repeat-vertex square must neither panic nor collapse early.
    //
    // The dedupe R1 root-fixed still runs — `OffsetRingSet::from_polygon`
    // enters through the same `remove_repeat_pos` as `offset_polygon` — so
    // this exercises the fix rather than merely re-testing that a square is
    // offsettable.
    let mut rings = OffsetRingSet::from_polygon(&poly);
    let mut layers: Vec<Vec<Polygon2>> = Vec::new();
    for _ in 0..40 {
        rings = rings.offset(0.05);
        if rings.is_empty() {
            break;
        }
        layers.push(rings.to_polygons(FlattenPolicy::untoleranced()));
    }
    assert!(
        layers.len() > 10,
        "expected many 0.05 mm rings from a 20 mm square, got {}",
        layers.len()
    );
}
