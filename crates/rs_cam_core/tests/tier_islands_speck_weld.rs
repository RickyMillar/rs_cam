//! Tiered-finish plan F1 (`planning/tiered_finish_2026-09-30/PLAN.md`): the
//! per-tier island cap re-closes the RAW mask at a raised radius, so a field
//! of specks that pass 0 drops under the minimum island area comes back
//! WELDED into one blob, and "keep the largest" then prefers the blob.
//!
//! # The fixture
//!
//! A synthetic two-tier map at 0.4 mm cells, cusp radii `[2.0, 1.0]`:
//!
//! - 30 real islands, 13 × 13 cells each, 10-cell (4.0 mm) gaps;
//! - a field of 3 × 3-cell specks at 4-cell (1.6 mm) gaps, 14 × 14 of them.
//!
//! Each speck is under the derived minimum island area, so pass 0 drops the
//! field. 30 islands are over the default cap of 24, so the loop raises the
//! close radius.
//!
//! # No hand figures
//!
//! Every radius comes from the public derivation
//! ([`TierIslandParams::derived_close_radius_mm`],
//! [`CAP_CLOSE_RAISE_FACTOR`]). The pass at which the field welds is NOT
//! assumed: [`weld_pass`] measures it by closing the field alone through
//! [`extract_tier_islands`] (which calls `morphological_close`) at each
//! radius the cap loop can reach. The plan's own fixture (2 × 2-cell specks
//! at 2.0 mm gaps) does not weld at any of those radii: the exact Euclidean
//! close keeps the lattice's centre open until the radius reaches the
//! centre's distance to the nearest speck, √18 = 4.24 cells, and the last
//! raise stops at 4.22 cells. This fixture is the nearest one that welds.
//!
//! # The two arms
//!
//! - Green (the default, [`DEFAULT_MAX_CLOSE_RAISES`] = 0, since the
//!   operator's approval of 2026-10-01): the 24 largest real islands are
//!   kept, 6 are dropped with their area reported, and the field owns no
//!   cell.
//! - Red (`max_close_raises` = [`MAX_CLOSE_RAISES`], the default until
//!   2026-10-01, and still the value of a project saved with it): the field
//!   welds, the blob is kept, and real islands are dropped instead. The
//!   owned area in the field is a PINNED MEASUREMENT of the defect.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::maps::grid::GridSpec;
use rs_cam_core::maps::tier_islands::{
    CAP_CLOSE_RAISE_FACTOR, DEFAULT_MAX_CLOSE_RAISES, DEFAULT_MAX_REGIONS_PER_TIER,
    MAX_CLOSE_RAISES, TierIslandParams, TierIslandSet, extract_tier_islands,
};
use rs_cam_core::maps::tier_map::{NO_TIER, ResidualTreatment, TierMap};

const CELL: f64 = 0.4;
const CUSP_RADII: [f64; 2] = [2.0, 1.0];
const FINE_CUSP: f64 = 1.0;

const ISLAND_CELLS: usize = 13;
const ISLAND_GAP_CELLS: usize = 10;
const ISLAND_COLS: usize = 6;
const ISLAND_ROWS: usize = 5;
const ISLAND_COUNT: usize = ISLAND_COLS * ISLAND_ROWS;

const SPECK_CELLS: usize = 3;
const SPECK_GAP_CELLS: usize = 4;
const SPECKS_PER_SIDE: usize = 14;

/// First row / column of the island block and the field.
const ORIGIN: usize = 6;
/// Cells between the island block and the field: wider than the island gap.
const BLOCK_TO_FIELD_CELLS: usize = 12;

const fn island_pitch() -> usize {
    ISLAND_CELLS + ISLAND_GAP_CELLS
}
const fn speck_pitch() -> usize {
    SPECK_CELLS + SPECK_GAP_CELLS
}
const fn island_block_cols() -> usize {
    ISLAND_COLS * island_pitch() - ISLAND_GAP_CELLS
}
const fn island_block_rows() -> usize {
    ISLAND_ROWS * island_pitch() - ISLAND_GAP_CELLS
}
const fn field_side() -> usize {
    SPECKS_PER_SIDE * speck_pitch() - SPECK_GAP_CELLS
}
const fn field_col0() -> usize {
    ORIGIN + island_block_cols() + BLOCK_TO_FIELD_CELLS
}
const fn nx() -> usize {
    field_col0() + field_side() + ORIGIN
}
const fn ny() -> usize {
    let tallest = if island_block_rows() > field_side() {
        island_block_rows()
    } else {
        field_side()
    };
    ORIGIN + tallest + ORIGIN
}

#[derive(Clone, Copy)]
struct Content {
    islands: bool,
    field: bool,
}

/// A two-tier map with the requested content, the grid-edge ring uncovered.
fn map_with(content: Content) -> TierMap {
    let (nx, ny) = (nx(), ny());
    let mut labels = vec![0u8; nx * ny];
    let mut paint = |r0: usize, c0: usize, side: usize| {
        for r in r0..r0 + side {
            for c in c0..c0 + side {
                labels[r * nx + c] = 1;
            }
        }
    };
    if content.islands {
        for gr in 0..ISLAND_ROWS {
            for gc in 0..ISLAND_COLS {
                paint(
                    ORIGIN + gr * island_pitch(),
                    ORIGIN + gc * island_pitch(),
                    ISLAND_CELLS,
                );
            }
        }
    }
    if content.field {
        for gr in 0..SPECKS_PER_SIDE {
            for gc in 0..SPECKS_PER_SIDE {
                paint(
                    ORIGIN + gr * speck_pitch(),
                    field_col0() + gc * speck_pitch(),
                    SPECK_CELLS,
                );
            }
        }
    }
    for r in 0..ny {
        for c in 0..nx {
            if r < 2 || c < 2 || r + 2 >= ny || c + 2 >= nx {
                labels[r * nx + c] = NO_TIER;
            }
        }
    }
    TierMap {
        grid: GridSpec {
            nx,
            ny,
            origin_x: 0.0,
            origin_y: 0.0,
            cell_mm: CELL,
        },
        labels,
        finest_z: vec![0.0f32; nx * ny],
        tier_count: 2,
        tolerance_mm: 0.05,
        treatment: ResidualTreatment::Raw,
    }
}

fn fine_set(map: &TierMap, params: &TierIslandParams) -> TierIslandSet {
    extract_tier_islands(map, params, &CUSP_RADII)
        .unwrap()
        .set_for_tier(1)
        .expect("tier 1 present")
        .clone()
}

/// The close radius (mm) the cap loop uses at `pass`.
fn pass_radius_mm(pass: usize) -> f64 {
    TierIslandParams::derived_close_radius_mm(FINE_CUSP, 1.0)
        * CAP_CLOSE_RAISE_FACTOR.powi(i32::try_from(pass).unwrap())
}

/// Close `map` at `pass`'s radius with no filter and no cap, and count the
/// islands. This is `morphological_close` through the public door.
fn islands_after_close_at(map: &TierMap, pass: usize) -> usize {
    let params = TierIslandParams {
        close_radius_mm: Some(pass_radius_mm(pass)),
        min_region_area_mm2: Some(0.0),
        overlap_mm: 0.0,
        max_regions_per_tier: usize::MAX,
        max_close_raises: 0,
        ..TierIslandParams::default()
    };
    fine_set(map, &params).islands
}

/// The first pass at which the field closes into ONE island, or `None` when
/// no pass up to [`MAX_CLOSE_RAISES`] welds it.
fn weld_pass() -> Option<usize> {
    let field = map_with(Content {
        islands: false,
        field: true,
    });
    (0..=MAX_CLOSE_RAISES).find(|&pass| islands_after_close_at(&field, pass) == 1)
}

/// Is cell `i` inside the field's bounding box?
fn in_field(i: usize) -> bool {
    let (r, c) = (i / nx(), i % nx());
    (ORIGIN..ORIGIN + field_side()).contains(&r)
        && (field_col0()..field_col0() + field_side()).contains(&c)
}

fn island_area_mm2() -> f64 {
    (ISLAND_CELLS * ISLAND_CELLS) as f64 * CELL * CELL
}

fn field_raw_cells() -> usize {
    SPECKS_PER_SIDE * SPECKS_PER_SIDE * SPECK_CELLS * SPECK_CELLS
}

/// The derivations the two arms rest on, measured rather than assumed.
#[test]
fn the_fixture_welds_the_field_and_never_the_islands() {
    let min_area = TierIslandParams::derived_min_region_area_mm2(FINE_CUSP, 1.0);
    let speck_area = (SPECK_CELLS * SPECK_CELLS) as f64 * CELL * CELL;
    assert!(
        speck_area < min_area,
        "a speck ({speck_area} mm2) must be under the minimum island area ({min_area} mm2)"
    );
    assert!(
        island_area_mm2() >= min_area,
        "a real island ({} mm2) must pass the minimum island area ({min_area} mm2)",
        island_area_mm2()
    );
    const { assert!(ISLAND_COUNT > DEFAULT_MAX_REGIONS_PER_TIER) };

    let pass = weld_pass().expect("the field welds within the bounded raise loop");
    assert!(
        pass > 0,
        "the field must NOT weld at the configured radius, or dial 0 would weld it too"
    );
    // Measured 2026-09-30: pass 2, a 1.125 mm radius. Moves only with the
    // close rule, the raise factor or the fixture.
    assert_eq!(pass, 2, "the field's weld pass moved");

    let islands = map_with(Content {
        islands: true,
        field: false,
    });
    assert_eq!(
        islands_after_close_at(&islands, MAX_CLOSE_RAISES),
        ISLAND_COUNT,
        "the real islands must stay apart at the largest radius the loop reaches"
    );
}

/// RED ARM — the defect at the raise bound [`MAX_CLOSE_RAISES`], the
/// default until 2026-10-01 (a project saved with it keeps it). The field
/// welds into one blob, the blob is kept as the largest island, and real
/// islands are dropped in its place.
#[test]
fn at_three_raises_the_specks_weld_into_a_blob_that_displaces_real_islands() {
    let map = map_with(Content {
        islands: true,
        field: true,
    });
    let params = TierIslandParams {
        overlap_mm: 0.0,
        max_close_raises: MAX_CLOSE_RAISES,
        ..TierIslandParams::default()
    };
    let set = fine_set(&map, &params);

    assert_eq!(set.cap.max_close_raises, MAX_CLOSE_RAISES);
    assert_eq!(
        set.cap.close_raises, MAX_CLOSE_RAISES,
        "30 islands and a blob stay over the cap at every raise"
    );
    assert_eq!(
        set.cap.islands_after_min_area,
        ISLAND_COUNT + 1,
        "the real islands plus the welded field"
    );
    assert_eq!(set.cap.kept, DEFAULT_MAX_REGIONS_PER_TIER);
    let dropped = ISLAND_COUNT + 1 - DEFAULT_MAX_REGIONS_PER_TIER;
    assert_eq!(
        set.cap.dropped(),
        dropped,
        "real islands dropped for the blob"
    );
    assert!(
        (set.cap.dropped_area_mm2 - dropped as f64 * island_area_mm2()).abs() < 1e-9,
        "dropped area {} is the dropped islands' cells",
        set.cap.dropped_area_mm2
    );

    let field_owned = set
        .owned_mask
        .iter()
        .enumerate()
        .filter(|&(i, &on)| on && in_field(i))
        .count();
    // The pinned measurement of the defect (2026-09-30): the weld owns this
    // many cells in the field (1 380.5 mm2), against the field's raw 1 764
    // speck cells (282.2 mm2): 4.9x.
    // It moves only with the close rule, the raise factor or the fixture.
    assert_eq!(
        field_owned,
        WELDED_FIELD_CELLS,
        "measured weld: {field_owned} owned cells in the field ({:.1} mm2) against {} raw",
        field_owned as f64 * CELL * CELL,
        field_raw_cells()
    );
    assert!(
        field_owned >= 4 * field_raw_cells(),
        "the weld owns at least 4x the field's raw cells"
    );
}

/// The pinned weld measurement. See the red arm.
const WELDED_FIELD_CELLS: usize = 8_628;

/// GREEN ARM — the default dials (the raise bound
/// [`DEFAULT_MAX_CLOSE_RAISES`] = 0). The cap keeps the 24 largest islands at
/// the configured radius, reports the 6 it drops and their area, and the
/// field owns nothing.
#[test]
fn at_the_default_zero_raises_the_specks_stay_coarse_and_the_drop_is_reported() {
    let map = map_with(Content {
        islands: true,
        field: true,
    });
    // The raise bound is NOT set here: this arm reads the default.
    let params = TierIslandParams {
        overlap_mm: 0.0,
        ..TierIslandParams::default()
    };
    let set = fine_set(&map, &params);

    assert_eq!(set.cap.max_close_raises, DEFAULT_MAX_CLOSE_RAISES);
    assert_eq!(set.cap.max_close_raises, 0);
    assert_eq!(set.cap.close_raises, 0, "no raise");
    assert!((set.cap.final_close_radius_mm - pass_radius_mm(0)).abs() < 1e-12);
    assert_eq!(set.cap.islands_after_min_area, ISLAND_COUNT);
    assert_eq!(set.islands, DEFAULT_MAX_REGIONS_PER_TIER);
    let dropped = ISLAND_COUNT - DEFAULT_MAX_REGIONS_PER_TIER;
    assert_eq!(set.cap.dropped(), dropped);
    assert!(
        (set.cap.dropped_area_mm2 - dropped as f64 * island_area_mm2()).abs() < 1e-9,
        "dropped area {} must be {dropped} islands of {} mm2",
        set.cap.dropped_area_mm2,
        island_area_mm2()
    );

    assert!(
        !set.owned_mask
            .iter()
            .enumerate()
            .any(|(i, &on)| on && in_field(i)),
        "with no raise, no field cell is owned"
    );

    // Owned area: 24 islands, each a solid square that marching squares
    // traces to exactly its cell extent, within perimeter x cell.
    let kept = DEFAULT_MAX_REGIONS_PER_TIER as f64;
    let side_mm = ISLAND_CELLS as f64 * CELL;
    let slack = kept * 4.0 * side_mm * CELL;
    assert!(
        (set.owned_area_mm2 - kept * island_area_mm2()).abs() <= slack,
        "owned {:.2} mm2 against {:.2} mm2 +- {slack:.2}",
        set.owned_area_mm2,
        kept * island_area_mm2()
    );

    // A raise only merges: with none, ownership is a subset of the close at
    // the configured radius.
    let closed = fine_set(
        &map,
        &TierIslandParams {
            close_radius_mm: Some(pass_radius_mm(0)),
            min_region_area_mm2: Some(0.0),
            overlap_mm: 0.0,
            max_regions_per_tier: usize::MAX,
            max_close_raises: 0,
            ..TierIslandParams::default()
        },
    );
    assert!(
        set.owned_mask
            .iter()
            .zip(closed.owned_mask.iter())
            .all(|(&owned, &in_close)| !owned || in_close),
        "owned must be a subset of close(raw, r0)"
    );
}

/// The dial is clamped: it can lower the raise bound, never raise it.
#[test]
fn the_dial_cannot_raise_the_bound_past_the_ceiling() {
    let params = TierIslandParams {
        max_close_raises: MAX_CLOSE_RAISES + 5,
        ..TierIslandParams::default()
    };
    assert_eq!(params.effective_max_close_raises(), MAX_CLOSE_RAISES);
    assert_eq!(
        TierIslandParams::default().max_close_raises,
        DEFAULT_MAX_CLOSE_RAISES,
        "the default is the named constant"
    );
    assert_eq!(
        DEFAULT_MAX_CLOSE_RAISES, 0,
        "plan F1, operator approval 2026-10-01: no raise by default"
    );
}

/// A project file that states the raise bound keeps it; one that does not
/// state it reads the default. The struct-level `#[serde(default)]` is the
/// mechanism; this pins it, so a saved project with `max_close_raises = 3`
/// still plans as it did before the default changed.
#[test]
fn a_saved_raise_bound_survives_the_default_change() {
    let explicit: TierIslandParams =
        toml::from_str("max_close_raises = 3\n").expect("an explicit bound parses");
    assert_eq!(explicit.max_close_raises, MAX_CLOSE_RAISES);
    assert_eq!(explicit.effective_max_close_raises(), MAX_CLOSE_RAISES);

    let absent: TierIslandParams = toml::from_str("coarseness = 1.0\n").expect("parses");
    assert_eq!(absent.max_close_raises, DEFAULT_MAX_CLOSE_RAISES);

    let written = toml::to_string(&TierIslandParams::default()).expect("serializes");
    assert!(
        written.contains("max_close_raises = 0"),
        "a new project writes the bound explicitly: {written}"
    );
}

/// The plan's own fixture (2 x 2-cell specks at 5-cell = 2.0 mm gaps) does
/// NOT weld at any radius the loop reaches: the exact Euclidean close keeps
/// the lattice centre open until the radius reaches its distance to the
/// nearest speck, sqrt(3^2 + 3^2) = 4.24 cells, and the last raise stops
/// below that. This is why the fixture above uses 3 x 3 specks at 1.6 mm.
#[test]
fn the_plans_two_cell_specks_at_two_mm_never_weld() {
    let (speck, gap, per_side) = (2usize, 5usize, 8usize);
    let pitch = speck + gap;
    let side = ORIGIN + per_side * pitch + ORIGIN;
    let mut labels = vec![0u8; side * side];
    for gr in 0..per_side {
        for gc in 0..per_side {
            for r in 0..speck {
                for c in 0..speck {
                    labels[(ORIGIN + gr * pitch + r) * side + ORIGIN + gc * pitch + c] = 1;
                }
            }
        }
    }
    for r in 0..side {
        for c in 0..side {
            if r < 2 || c < 2 || r + 2 >= side || c + 2 >= side {
                labels[r * side + c] = NO_TIER;
            }
        }
    }
    let map = TierMap {
        grid: GridSpec {
            nx: side,
            ny: side,
            origin_x: 0.0,
            origin_y: 0.0,
            cell_mm: CELL,
        },
        labels,
        finest_z: vec![0.0f32; side * side],
        tier_count: 2,
        tolerance_mm: 0.05,
        treatment: ResidualTreatment::Raw,
    };
    let centre_distance_cells = ((gap / 2 + 1).pow(2) as f64 * 2.0).sqrt();
    let last_radius_cells = pass_radius_mm(MAX_CLOSE_RAISES) / CELL;
    assert!(
        last_radius_cells < centre_distance_cells,
        "{last_radius_cells} cells must stay under the lattice centre's {centre_distance_cells}"
    );
    assert_eq!(
        islands_after_close_at(&map, MAX_CLOSE_RAISES),
        per_side * per_side,
        "no speck merges at the largest radius the loop reaches"
    );
}
