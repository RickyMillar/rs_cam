//! Phase I sentries — `tier_islands::extract_tier_islands`.
//!
//! The measured motivation (session of 2026-08-26, real wanaka mesh @ 0.3 mm
//! cell, tolerance 0.05, R2.0 → R1.0 ladder): the slope-compensated tier map
//! hands 22.0% / 8,815 mm² to the fine tier as **~566 raw islands**, and the
//! shipped polygon extractor silently kept the largest 64 of them
//! (`region_mask::MAX_REST_REGIONS`). This module is the layer that turns that
//! storm into an operator-approvable island set, so every sentry here is about
//! a *count*, a *containment* or a *partition* — never about a toolpath.
//!
//! Fixtures are synthetic [`TierMap`]s built by hand rather than by
//! `compute_tier_map`: the storm this filters is a property of the LABEL GRID,
//! and a hand-built grid states the island geometry the assertion is about
//! instead of hoping a mesh happens to produce it. `tests/tier_map_walk_t1.rs`
//! is where the walk that produces real labels is pinned.
//!
//! Note on areas: `marching_squares_bool_grid` treats grid cells as CORNERS
//! and cuts at edge midpoints, so an `n`-cell-wide solid block extracts as an
//! `n*cell` polygon (a 20-cell block at 0.5 mm is a 10 mm square). Area
//! assertions below are written against that, with slack for the corner
//! rounding a Euclidean dilation leaves. The min-area FILTER, by contrast,
//! counts CELLS — it is a population test on the mask, not a polygon test.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![allow(clippy::indexing_slicing)]

use rs_cam_core::geo::P2;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::tier_islands::{
    CAP_CLOSE_RAISE_FACTOR, MAX_CLOSE_RAISES, TierIslandParams, extract_tier_islands,
};
use rs_cam_core::tier_map::{NO_TIER, ResidualTreatment, TierMap};

// ── Fixture construction ────────────────────────────────────────────────

/// A `nx`×`ny` tier map at `cell_mm`, origin (0,0), every cell labelled
/// `base` (0 = the coarse tier). `finest_z` is a flat plane — nothing in this
/// module reads it, but it is part of the type's contract that it is
/// non-`NaN` wherever the label is not [`NO_TIER`].
fn flat_map(nx: usize, ny: usize, cell_mm: f64, tier_count: usize, base: u8) -> TierMap {
    TierMap {
        nx,
        ny,
        origin_x: 0.0,
        origin_y: 0.0,
        cell_mm,
        labels: vec![base; nx * ny],
        finest_z: vec![0.0f32; nx * ny],
        tier_count,
        tolerance_mm: 0.05,
        treatment: ResidualTreatment::Raw,
    }
}

/// Paint a `w`×`h` cell block of `tier` with its top-left at `(r0, c0)`.
fn paint(map: &mut TierMap, r0: usize, c0: usize, w: usize, h: usize, tier: u8) {
    let nx = map.nx;
    for r in r0..(r0 + h) {
        for c in c0..(c0 + w) {
            if r < map.ny && c < nx {
                map.labels[r * nx + c] = tier;
            }
        }
    }
}

/// Ring the grid with [`NO_TIER`] so no island can touch the grid edge —
/// exactly what `TierMapParams::margin_mm` guarantees on a real map.
fn ring_uncovered(map: &mut TierMap, width: usize) {
    let nx = map.nx;
    for r in 0..map.ny {
        for c in 0..nx {
            if r < width || c < width || r + width >= map.ny || c + width >= nx {
                map.labels[r * nx + c] = NO_TIER;
            }
        }
    }
}

/// Neutral params (derived dials in force) with an explicit overlap.
fn neutral(overlap_mm: f64) -> TierIslandParams {
    TierIslandParams {
        overlap_mm,
        ..TierIslandParams::default()
    }
}

/// Every cell centre of an ownership mask, as world points.
fn owned_points(map: &TierMap, mask: &[bool]) -> Vec<P2> {
    let mut out = Vec::new();
    for (i, &on) in mask.iter().enumerate() {
        if !on {
            continue;
        }
        let (r, c) = (i / map.nx, i % map.nx);
        if let Some((x, y)) = map.cell_center(r, c) {
            out.push(P2::new(x, y));
        }
    }
    out
}

fn in_any(polys: &[Polygon2], p: &P2) -> bool {
    polys.iter().any(|poly| poly.contains_point(p))
}

fn total_area(polys: &[Polygon2]) -> f64 {
    polys.iter().map(Polygon2::area).sum()
}

fn biggest_area(polys: &[Polygon2]) -> f64 {
    polys.iter().map(Polygon2::area).fold(0.0f64, f64::max)
}

// ── I1.1 — the storm collapses ──────────────────────────────────────────

/// A dense cluster of small fine-tier specks, three isolated specks, and one
/// big island. The default dials must MERGE the cluster (morphological close),
/// ABSORB the isolated specks (min-area), and leave the big island alone.
///
/// The two mechanisms are asserted separately, because they fail differently:
/// `cap.islands_after_close` is the close's answer, `islands` is the filter's.
#[test]
fn dense_speck_cluster_merges_isolated_specks_absorb_big_island_survives() {
    // 0.5 mm cells, fine-tier cusp radius 2.0 → derived close radius 1.0 mm
    // (2 cells) and min island area 64 mm² (256 cells).
    let cell = 0.5;
    let mut map = flat_map(120, 120, cell, 2, 0);

    // 25 2x2 specks on a 4-cell pitch: 25 raw islands, each 1 mm², every gap
    // (2 cells) inside the close radius, so the close bridges the lot.
    for gr in 0..5 {
        for gc in 0..5 {
            paint(&mut map, 20 + gr * 4, 20 + gc * 4, 2, 2, 1);
        }
    }
    // Three isolated single cells, 6 cells apart — too far for a 2-cell
    // dilation to bridge (the discs do not even touch) and far under the floor.
    for k in 0..3 {
        paint(&mut map, 60, 10 + k * 6, 1, 1, 1);
    }
    // One unmistakably real island: 20x20 cells = 100 mm² of territory.
    paint(&mut map, 70, 70, 20, 20, 1);
    ring_uncovered(&mut map, 2);

    let islands = extract_tier_islands(&map, &neutral(0.0), &[4.0, 2.0]).unwrap();
    let set = islands.set_for_tier(1).expect("tier 1 must be present");

    assert_eq!(
        set.raw_island_count, 29,
        "fixture states 25 cluster specks + 3 isolated + 1 big island"
    );
    assert!(
        set.cap.islands_after_close <= 6,
        "the close must collapse the 25-speck cluster to one blob: {} islands after close",
        set.cap.islands_after_close
    );
    assert_eq!(
        set.islands, 2,
        "min-area keeps the merged cluster and the big island, drops the three specks"
    );
    assert_eq!(set.owned.len(), 2, "one polygon per kept island");

    // The big island must still be the ~9.5 x 9.5 mm block it started as.
    assert!(
        biggest_area(set.owned.as_slice()) > 80.0,
        "the 100 mm2 island must survive: biggest kept = {} mm2",
        biggest_area(set.owned.as_slice())
    );
    // Nothing under the floor survived.
    for poly in set.owned.as_slice() {
        assert!(
            poly.area() > 20.0,
            "a {} mm2 speck survived a {} mm2 floor",
            poly.area(),
            set.min_region_area_mm2
        );
    }
}

/// A tier made only of specks keeps NO islands: the cells fall back to the
/// coarse tier's complement rather than becoming 16 one-cell "regions".
#[test]
fn a_tier_made_only_of_specks_keeps_no_islands() {
    let cell = 0.5;
    let mut map = flat_map(80, 80, cell, 2, 0);
    for gr in 0..4 {
        for gc in 0..4 {
            paint(&mut map, 20 + gr * 6, 20 + gc * 6, 1, 1, 1);
        }
    }
    ring_uncovered(&mut map, 2);

    let islands = extract_tier_islands(&map, &neutral(0.0), &[2.0, 1.0]).unwrap();
    let set = islands.set_for_tier(1).expect("tier 1 present");
    assert_eq!(set.raw_island_count, 16);
    assert_eq!(
        set.islands, 0,
        "specks 6 cells apart are too far to close together and too small to keep"
    );
    assert!(set.owned.is_empty());
    assert!(set.machining.is_empty());
    assert_eq!(set.owned_cells, 0);
    assert_eq!(set.owned_area_mm2, 0.0);
    assert!(set.owned_mask.iter().all(|&on| !on));
}

// ── I1.2 — the coarseness slider ────────────────────────────────────────

/// The operator's one knob. Monotone by construction: coarseness scales the
/// close radius linearly and the min-island area quadratically (ONE length
/// scale, expressed once as a radius and once as its square), so a coarser
/// setting can only merge more and drop more.
#[test]
fn coarseness_is_monotone_in_island_count() {
    let cell = 0.4;
    let mut map = flat_map(150, 150, cell, 2, 0);
    // Square blocks of deliberately spread sizes, none of them a tie against
    // any of the three floors this test exercises.
    let blocks: [(usize, usize, usize); 8] = [
        (20, 20, 7),
        (20, 34, 7),
        (20, 48, 5),
        (40, 20, 8),
        (40, 40, 4),
        (60, 60, 10),
        (90, 30, 13),
        (100, 90, 15),
    ];
    for (r0, c0, w) in blocks {
        paint(&mut map, r0, c0, w, w, 1);
    }
    ring_uncovered(&mut map, 2);

    let count_at = |coarseness: f64| -> usize {
        let params = TierIslandParams {
            coarseness,
            overlap_mm: 0.0,
            ..TierIslandParams::default()
        };
        extract_tier_islands(&map, &params, &[2.0, 0.6])
            .unwrap()
            .set_for_tier(1)
            .expect("tier 1 present")
            .islands
    };

    let fine = count_at(0.5);
    let neutral_count = count_at(1.0);
    let coarse = count_at(2.0);

    assert!(
        coarse <= neutral_count,
        "coarseness 2.0 must not produce MORE islands than 1.0: {coarse} vs {neutral_count}"
    );
    assert!(
        fine >= neutral_count,
        "coarseness 0.5 must not produce FEWER islands than 1.0: {fine} vs {neutral_count}"
    );
    assert!(
        fine > coarse,
        "the dial must actually move on this fixture: 0.5 -> {fine}, 2.0 -> {coarse}"
    );
}

/// `None` dials at coarseness 1.0 are identical to spelling the derived values
/// out by hand — the derivation IS the default, not a separate code path.
#[test]
fn explicit_dials_equal_to_the_derivation_reproduce_the_default_exactly() {
    let cell = 0.4;
    let mut map = flat_map(120, 120, cell, 2, 0);
    for (r0, c0, w) in [(20usize, 20usize, 7usize), (20, 34, 7), (60, 60, 10)] {
        paint(&mut map, r0, c0, w, w, 1);
    }
    ring_uncovered(&mut map, 2);

    let cusp = 0.6f64;
    let derived = extract_tier_islands(&map, &neutral(1.0), &[2.0, cusp]).unwrap();
    let set_d = derived.set_for_tier(1).expect("tier 1 present");

    let explicit_params = TierIslandParams {
        close_radius_mm: Some(TierIslandParams::derived_close_radius_mm(cusp, 1.0)),
        min_region_area_mm2: Some(TierIslandParams::derived_min_region_area_mm2(cusp, 1.0)),
        overlap_mm: 1.0,
        ..TierIslandParams::default()
    };
    let explicit = extract_tier_islands(&map, &explicit_params, &[2.0, cusp]).unwrap();
    let set_e = explicit.set_for_tier(1).expect("tier 1 present");

    assert_eq!(set_d.islands, set_e.islands);
    assert_eq!(set_d.owned_cells, set_e.owned_cells);
    assert_eq!(set_d.owned_mask, set_e.owned_mask);
    assert_eq!(set_d.owned.len(), set_e.owned.len());
    assert!((set_d.close_radius_mm - set_e.close_radius_mm).abs() < 1e-12);
    assert!((set_d.min_region_area_mm2 - set_e.min_region_area_mm2).abs() < 1e-12);
    for (a, b) in set_d.owned.as_slice().iter().zip(set_e.owned.as_slice()) {
        assert!(
            (a.area() - b.area()).abs() < 1e-12,
            "same polygons, in order"
        );
    }
}

/// The slider moves the DERIVATION, never a number the operator typed. An
/// explicit dial is taken verbatim at every coarseness — otherwise the value
/// showing in the advanced flyout would not be the value in force.
#[test]
fn coarseness_does_not_scale_explicit_dials() {
    let cell = 0.4;
    let mut map = flat_map(120, 120, cell, 2, 0);
    for (r0, c0, w) in [(20usize, 20usize, 7usize), (20, 34, 7), (60, 60, 10)] {
        paint(&mut map, r0, c0, w, w, 1);
    }
    ring_uncovered(&mut map, 2);

    let with = |coarseness: f64| {
        let params = TierIslandParams {
            close_radius_mm: Some(0.3),
            min_region_area_mm2: Some(2.0),
            coarseness,
            overlap_mm: 0.0,
            ..TierIslandParams::default()
        };
        let out = extract_tier_islands(&map, &params, &[2.0, 0.6]).unwrap();
        let set = out.set_for_tier(1).expect("tier 1 present");
        (set.islands, set.owned_cells, set.close_radius_mm.to_bits())
    };

    assert_eq!(with(0.25), with(1.0), "explicit dials ignore the slider");
    assert_eq!(with(4.0), with(1.0), "explicit dials ignore the slider");
}

// ── I1.3 — the cap backstop ─────────────────────────────────────────────

/// Engineered to exceed `max_regions_per_tier`: 60 real islands, each well
/// over the min-area floor, spaced too far for the bounded auto-raise loop to
/// bridge. The result must be ≤ the cap, and the module must SAY what it did
/// in a typed report — not in a `tracing::warn!` nobody installs a subscriber
/// for (the F3 lesson, `region_mask::RegionCapReport`).
#[test]
fn cap_backstop_raises_close_radius_then_truncates_and_reports_it() {
    let cell = 0.5;
    let block = 8usize;
    let pitch = 14usize; // 6-cell (3 mm) gaps
    let cols = 10usize;
    let rows = 6usize;
    let nx = cols * pitch + 8;
    let ny = rows * pitch + 8;
    let mut map = flat_map(nx, ny, cell, 2, 0);
    for gr in 0..rows {
        for gc in 0..cols {
            paint(&mut map, 4 + gr * pitch, 4 + gc * pitch, block, block, 1);
        }
    }
    ring_uncovered(&mut map, 2);

    let params = TierIslandParams {
        max_regions_per_tier: 12,
        overlap_mm: 0.0,
        ..TierIslandParams::default()
    };
    // Cusp radius 0.3 → derived close radius 0.15 mm, well under a cell: the
    // raise loop is the only thing that could merge anything here.
    let islands = extract_tier_islands(&map, &params, &[2.0, 0.3]).unwrap();
    let set = islands.set_for_tier(1).expect("tier 1 present");

    assert_eq!(set.raw_island_count, 60, "fixture states 60 raw islands");
    assert!(
        set.islands <= 12,
        "the cap is a backstop, not a suggestion: {} islands survived a cap of 12",
        set.islands
    );
    assert_eq!(set.owned.len(), set.islands, "one polygon per kept island");
    assert!(
        set.cap.acted(),
        "something had to give on a 60-island fixture capped at 12; report = {:?}",
        set.cap
    );
    assert!(
        set.cap.close_raises > 0,
        "the auto-raise loop must fire before the truncation: {:?}",
        set.cap
    );
    assert!(
        set.cap.close_raises <= MAX_CLOSE_RAISES,
        "the loop is BOUNDED: {} raises",
        set.cap.close_raises
    );
    assert!(
        set.cap.truncated(),
        "3 raises of 1.5x cannot bridge a 6-cell gap, so the truncation is what \
         finally holds the cap: {:?}",
        set.cap
    );
    assert_eq!(set.cap.cap, 12, "the report carries the cap in force");
    assert_eq!(
        set.cap.islands_after_min_area, 60,
        "nothing was under the floor"
    );
    assert_eq!(set.cap.kept, set.islands);

    // The reported final radius is the one actually used, and the set agrees.
    let raises = i32::try_from(set.cap.close_raises).unwrap();
    let expected = set.cap.first_close_radius_mm * CAP_CLOSE_RAISE_FACTOR.powi(raises);
    assert!(
        (set.cap.final_close_radius_mm - expected).abs() < 1e-9,
        "final radius {} must be first {} raised {} times by {CAP_CLOSE_RAISE_FACTOR}",
        set.cap.final_close_radius_mm,
        set.cap.first_close_radius_mm,
        set.cap.close_raises
    );
    assert!((set.close_radius_mm - set.cap.final_close_radius_mm).abs() < 1e-12);

    // Truncation keeps the LARGEST by cell count — every kept island here is a
    // full 8x8 block, so they are all the same size and none is degenerate.
    for poly in set.owned.as_slice() {
        assert!(poly.area() > 5.0, "a degenerate sliver survived truncation");
    }
}

/// A healthy tier reports a NEUTRAL cap record — measured, not absent, and not
/// "acted". The distinction a bare 64-long `Vec` could never make.
#[test]
fn a_healthy_tier_reports_a_neutral_cap_record() {
    let cell = 0.5;
    let mut map = flat_map(100, 100, cell, 2, 0);
    paint(&mut map, 20, 20, 20, 20, 1);
    paint(&mut map, 60, 60, 20, 20, 1);
    ring_uncovered(&mut map, 2);

    let islands = extract_tier_islands(&map, &neutral(0.0), &[2.0, 1.0]).unwrap();
    let set = islands.set_for_tier(1).expect("tier 1 present");
    assert_eq!(set.islands, 2);
    assert!(
        !set.cap.acted(),
        "two islands is not a storm: {:?}",
        set.cap
    );
    assert_eq!(set.cap.close_raises, 0);
    assert!(!set.cap.truncated());
    assert_eq!(set.cap.kept, 2);
    assert_eq!(set.cap.islands_after_min_area, 2);
}

// ── I1.4 — the overlap band ─────────────────────────────────────────────

/// Overlap is applied PER ISLAND, after filtering, so it is a pure geometric
/// growth: the count cannot change, every owned island is contained in its
/// overlapped twin, and the total area grows by a plausible band.
#[test]
fn overlap_contains_the_owned_islands_and_leaves_the_count_alone() {
    let cell = 0.5;
    let mut map = flat_map(140, 140, cell, 2, 0);
    // Two islands 40 cells (20 mm) apart — a 2 mm band cannot merge them.
    paint(&mut map, 20, 20, 20, 20, 1);
    paint(&mut map, 80, 80, 24, 24, 1);
    ring_uncovered(&mut map, 2);

    let overlap_mm = 2.0;
    let islands = extract_tier_islands(&map, &neutral(overlap_mm), &[2.0, 1.0]).unwrap();
    let set = islands.set_for_tier(1).expect("tier 1 present");

    assert_eq!(set.islands, 2);
    assert_eq!(
        set.owned.len(),
        set.machining.len(),
        "overlap grows islands; it does not create, merge or drop them"
    );

    // Containment, both ways of asking: every owned vertex and every owned
    // cell centre lies inside the overlap set.
    for poly in set.owned.as_slice() {
        for v in &poly.exterior {
            assert!(
                in_any(set.machining.as_slice(), v),
                "owned vertex ({}, {}) fell outside the overlap band",
                v.x,
                v.y
            );
        }
    }
    for p in owned_points(&map, &set.owned_mask) {
        assert!(
            in_any(set.machining.as_slice(), &p),
            "owned cell centre ({}, {}) fell outside the overlap band",
            p.x,
            p.y
        );
    }

    let owned_area = total_area(set.owned.as_slice());
    let over_area = total_area(set.machining.as_slice());
    assert!(
        over_area > owned_area,
        "a 2 mm overlap must grow the area: {owned_area} -> {over_area}"
    );
    // The blocks trace to 10 and 12 mm squares; a `d` band on every side
    // predicts `(s + 2d)^2` before corner rounding. Assert the band exists and
    // is the right order, not a discretisation to three decimals.
    let expect = (10.0 + 2.0 * overlap_mm).powi(2) + (12.0 + 2.0 * overlap_mm).powi(2);
    assert!(
        (over_area - expect).abs() < 0.25 * expect,
        "overlap area {over_area} is not within 25% of the {expect} mm2 a 2 mm band predicts"
    );
}

/// `overlap_mm = 0` makes the two sets the same geometry — the dial is OFF,
/// not merely small.
#[test]
fn zero_overlap_leaves_the_owned_geometry_untouched() {
    let cell = 0.5;
    let mut map = flat_map(100, 100, cell, 2, 0);
    paint(&mut map, 20, 20, 20, 20, 1);
    ring_uncovered(&mut map, 2);

    let islands = extract_tier_islands(&map, &neutral(0.0), &[2.0, 1.0]).unwrap();
    let set = islands.set_for_tier(1).expect("tier 1 present");
    assert_eq!(set.owned.len(), set.machining.len());
    for (a, b) in set.owned.as_slice().iter().zip(set.machining.as_slice()) {
        assert!((a.area() - b.area()).abs() < 1e-12);
    }
}

/// The overlap band reaches into COARSER territory, and stops at the part
/// edge: it is clamped to the tier map's coverage, never grown off the board.
#[test]
fn the_overlap_band_stays_on_the_part() {
    let cell = 0.5;
    let mut map = flat_map(80, 80, cell, 2, 0);
    // A fine island hard against the covered boundary (rows/cols 2..77 are
    // covered after the ring), with a large overlap that would run off it.
    paint(&mut map, 3, 3, 20, 20, 1);
    ring_uncovered(&mut map, 2);

    let islands = extract_tier_islands(&map, &neutral(4.0), &[2.0, 1.0]).unwrap();
    let set = islands.set_for_tier(1).expect("tier 1 present");
    assert_eq!(set.islands, 1);

    let covered = map.covered_mask();
    for (i, &cov) in covered.iter().enumerate() {
        if cov {
            continue;
        }
        let (r, c) = (i / map.nx, i % map.nx);
        if let Some((x, y)) = map.cell_center(r, c) {
            assert!(
                !in_any(set.machining.as_slice(), &P2::new(x, y)),
                "the overlap band claimed uncovered cell ({r}, {c})"
            );
        }
    }
}

// ── I1.5 — ownership is a partition ─────────────────────────────────────

/// Two fine tiers that genuinely contest the same cells: tier 1's close
/// bridges straight across tier 2's strip. Pre-overlap ownership MUST
/// partition — the FINER tier wins, and no cell is owned twice.
#[test]
fn ownership_partitions_pre_overlap_with_the_finer_tier_winning() {
    let cell = 0.5;
    let mut map = flat_map(140, 140, cell, 3, 0);
    // rows 30..79: tier 1 | tier 2 strip | tier 1. The strip is 2 cells wide,
    // so tier 1's 1.5-cell dilation reaches across it from BOTH sides and its
    // close genuinely claims tier 2's cells — the contest this asserts on.
    paint(&mut map, 30, 30, 20, 50, 1);
    paint(&mut map, 30, 50, 2, 50, 2);
    paint(&mut map, 30, 52, 20, 50, 1);
    ring_uncovered(&mut map, 2);

    let islands = extract_tier_islands(&map, &neutral(0.0), &[3.0, 1.5, 1.0]).unwrap();

    let total = map.nx * map.ny;
    let mut seen = vec![0u8; total];
    for set in &islands.per_tier {
        assert_eq!(
            set.owned_mask.len(),
            total,
            "ownership mask covers the grid"
        );
        for (i, &on) in set.owned_mask.iter().enumerate() {
            if on {
                seen[i] += 1;
            }
        }
    }
    assert_eq!(
        seen.iter().filter(|&&n| n > 1).count(),
        0,
        "pre-overlap ownership must be a partition"
    );

    // The finer tier held the contested strip.
    let t2 = islands.set_for_tier(2).expect("tier 2 present");
    assert!(
        t2.owned_cells >= 90,
        "tier 2 kept its 100-cell strip: {} cells",
        t2.owned_cells
    );
    // ...and tier 1 is still TWO islands, not one welded across the strip.
    let t1 = islands.set_for_tier(1).expect("tier 1 present");
    assert_eq!(
        t1.islands, 2,
        "tier 1's close bridged the strip, but ownership clipped it back out"
    );

    // No tier ever owns an uncovered cell.
    let covered = map.covered_mask();
    for set in &islands.per_tier {
        for (i, (&on, &cov)) in set.owned_mask.iter().zip(covered.iter()).enumerate() {
            assert!(!on || cov, "cell {i} is owned but off the part");
        }
    }

    // The overlap bands MAY overlap each other — that is what they are for.
    // Only ownership partitions.
    assert!(
        islands
            .per_tier
            .iter()
            .all(|s| s.machining.len() == s.owned.len())
    );
}

/// The coarsest tier is the COMPLEMENT — it gets no island set, by design. A
/// consumer that finds tier 0 in `per_tier` is reading a different contract
/// than the one this module documents.
#[test]
fn the_coarsest_tier_gets_no_island_set() {
    let cell = 0.5;
    let mut map = flat_map(100, 100, cell, 2, 0);
    paint(&mut map, 20, 20, 20, 20, 1);
    ring_uncovered(&mut map, 2);

    let islands = extract_tier_islands(&map, &neutral(0.0), &[2.0, 1.0]).unwrap();
    assert!(islands.set_for_tier(0).is_none());
    assert_eq!(
        islands.per_tier.len(),
        1,
        "one fine tier on a 2-tool ladder"
    );
    assert_eq!(islands.per_tier[0].tier, 1);
}

// ── I1.6 — determinism ──────────────────────────────────────────────────

/// Two runs, identical output. No `HashMap` iteration order, no floating
/// tie-break without a total fallback, no clock, no RNG.
#[test]
fn extraction_is_deterministic_across_runs() {
    let cell = 0.4;
    let mut map = flat_map(150, 150, cell, 3, 0);
    for gr in 0..6 {
        for gc in 0..6 {
            let tier = if (gr + gc) % 3 == 0 { 2 } else { 1 };
            paint(&mut map, 10 + gr * 20, 10 + gc * 20, 9 + gc, 9 + gr, tier);
        }
    }
    ring_uncovered(&mut map, 2);

    let params = neutral(1.5);
    let a = extract_tier_islands(&map, &params, &[3.0, 1.5, 0.8]).unwrap();
    let b = extract_tier_islands(&map, &params, &[3.0, 1.5, 0.8]).unwrap();

    assert_eq!(a.per_tier.len(), b.per_tier.len());
    assert!(!a.per_tier.is_empty(), "the fixture must produce islands");
    for (sa, sb) in a.per_tier.iter().zip(b.per_tier.iter()) {
        assert_eq!(sa.tier, sb.tier);
        assert_eq!(sa.islands, sb.islands);
        assert_eq!(sa.raw_island_count, sb.raw_island_count);
        assert_eq!(sa.owned_cells, sb.owned_cells);
        assert_eq!(sa.owned_mask, sb.owned_mask);
        assert_eq!(sa.cap, sb.cap);
        assert_eq!(sa.owned.len(), sb.owned.len());
        for (pa, pb) in sa.owned.as_slice().iter().zip(sb.owned.as_slice()) {
            assert_eq!(pa.exterior.len(), pb.exterior.len());
            for (va, vb) in pa.exterior.iter().zip(pb.exterior.iter()) {
                assert!(
                    (va.x - vb.x).abs() < 1e-12 && (va.y - vb.y).abs() < 1e-12,
                    "vertex drift between runs"
                );
            }
        }
        for (pa, pb) in sa.machining.as_slice().iter().zip(sb.machining.as_slice()) {
            assert!((pa.area() - pb.area()).abs() < 1e-12);
        }
    }
}

// ── I1.7 — the rim band ─────────────────────────────────────────────────

/// `tier_map`'s own doc assigns rim erosion to this consumer: within roughly
/// one envelope radius of the part edge a big tool hangs off and reads a
/// false-high residual, so the rim reads as fine-tier territory. The dial is
/// OFF by default (this module receives CUSP radii, not envelope radii, so it
/// cannot derive the band) — but when a caller supplies the band, the rim goes
/// back to the coarse tier and the interior island is untouched.
#[test]
fn rim_erosion_demotes_the_false_high_edge_band() {
    let cell = 0.5;
    let mut map = flat_map(100, 100, cell, 2, 0);
    // A fine-tier ring hugging the covered boundary — what a hanging-off
    // coarse tool fabricates — plus a real interior island far from it.
    paint(&mut map, 3, 3, 94, 94, 1);
    paint(&mut map, 10, 10, 80, 80, 0);
    paint(&mut map, 40, 40, 20, 20, 1);
    ring_uncovered(&mut map, 2);

    let off = extract_tier_islands(&map, &neutral(0.0), &[2.0, 1.0]).unwrap();
    let off_cells = off.set_for_tier(1).expect("tier 1").owned_cells;

    let params = TierIslandParams {
        rim_erosion_mm: 3.0,
        overlap_mm: 0.0,
        ..TierIslandParams::default()
    };
    let on = extract_tier_islands(&map, &params, &[2.0, 1.0]).unwrap();
    let on_set = on.set_for_tier(1).expect("tier 1");

    assert!(
        on_set.owned_cells < off_cells,
        "rim erosion must hand the edge band back: {off_cells} -> {}",
        on_set.owned_cells
    );
    assert!(
        biggest_area(on_set.owned.as_slice()) > 80.0,
        "the interior island is 20 mm from the rim and must survive: {} mm2",
        biggest_area(on_set.owned.as_slice())
    );
}

// ── I1.8 — input contract ───────────────────────────────────────────────

/// A ladder-length mismatch is a caller bug and is REFUSED, not papered over
/// with a borrowed cusp radius — every dial here derives from the tier's OWN
/// tool, so a wrong-length list would silently mis-size the dials.
#[test]
fn a_cusp_radius_list_that_does_not_match_the_ladder_is_refused() {
    let mut map = flat_map(40, 40, 0.5, 3, 0);
    paint(&mut map, 10, 10, 8, 8, 1);
    ring_uncovered(&mut map, 2);
    assert!(extract_tier_islands(&map, &neutral(0.0), &[2.0, 1.0]).is_err());
    assert!(extract_tier_islands(&map, &neutral(0.0), &[]).is_err());
    assert!(extract_tier_islands(&map, &neutral(0.0), &[2.0, 1.5, 1.0]).is_ok());
}

/// A single-tool ladder has no fine tier at all: the whole board is the
/// complement. Empty, not an error.
#[test]
fn a_one_tool_ladder_yields_no_tiers() {
    let map = flat_map(40, 40, 0.5, 1, 0);
    let islands = extract_tier_islands(&map, &neutral(0.0), &[2.0]).unwrap();
    assert!(islands.per_tier.is_empty());
    assert!(islands.is_empty());
    assert_eq!(islands.total_owned_area_mm2(), 0.0);
}
