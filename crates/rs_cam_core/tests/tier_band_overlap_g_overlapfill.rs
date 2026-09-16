//! G-OVERLAPFILL — the overlap band, measured instead of redesigned.
//!
//! # What the band does, and why that is not a defect
//!
//! [`rs_cam_core::maps::tier_islands`] grows every fine tier's islands by
//! `overlap_mm` to make the MACHINING copy. The band's stated purpose is to
//! reach into the coarser tier's territory so the fine tool's first pass
//! lands on ground the coarse tool already cut, blending two cusp patterns
//! instead of butting them (module doc, "Ownership is a partition; overlap
//! bands are not"). A hole narrower than `2 · overlap_mm` closes under that
//! dilation — by definition, not by accident. Growing the outline while
//! eroding the holes is a no-op for exactly the same reason.
//!
//! What was missing is that nobody could SEE the consequence. On the wanaka
//! board at tolerance 0.05 the fine tier OWNS 12 224 mm² in 10 islands and
//! MACHINES 29 954 mm², 75 % of a 40 000 mm² board, because the coarse tool's
//! territory inside the valley network arrives as ~1 329 slivers with a
//! median area of 3.6 mm² and a 1.25 mm band on each side closes any gap
//! under 2.5 mm (`planning/island_clip_2026-09-09/SPEC.md` §5, study doc
//! §2.7a). Every island trial therefore cut near whole-board distances.
//!
//! So this file pins the MEASUREMENT and the ADVISORY, never a redesign:
//!
//! - the fixture-scale sentries run in the gate: a ring island with holes on
//!   both sides of the `2 · overlap` width, asserting which survive, what the
//!   counts read, and that the advisory fires at a fat band and stays silent
//!   at a thin one;
//! - the `#[ignore]`d wanaka instrument prints owned vs machining area at
//!   three tolerances at the planner's own dials, which is the evidence
//!   either way.
//!
//! Instrument:
//! `cargo test -p rs_cam_core --test tier_band_overlap_g_overlapfill -- --ignored --nocapture`

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use rs_cam_core::maps::tier_islands::{
    BAND_RATIO_ADVISORY_BOUND, TierIslandParams, extract_tier_islands,
};
use rs_cam_core::maps::tier_map::{ResidualTreatment, TierMap};

// ── Fixture ─────────────────────────────────────────────────────────────

const NX: usize = 120;
const NY: usize = 120;
const CELL: f64 = 0.5;

/// A centred tier-1 square with square tier-0 holes punched through it, on a
/// two-tier map whose WHOLE grid is covered.
///
/// The coverage matters: `region_polygons_from_mask_reported` clamps the
/// dilation to the covered set, so an island floating in uncovered space
/// would grow only inwards and the fixture would measure half the band. Here
/// the square sits in tier-0 territory, which is the real arrangement — the
/// band's whole purpose is to reach into the coarser tier.
///
/// `side_cells` is the square's side and `hole_cells` the hole sides, both in
/// CELLS: a hole of `w` cells is `w · CELL` mm wide, so the band closes it
/// when `w · CELL` is under `2 · overlap`. Holes lie in one row across the
/// square's middle with a four-cell wall between them, so no two merge.
fn ring_island_map(side_cells: usize, hole_cells: &[usize]) -> TierMap {
    // Tier 0 owns everything; the extractor drops the grid-edge ring itself.
    let mut labels = vec![0u8; NX * NY];

    let r0 = (NY - side_cells) / 2;
    let c0 = (NX - side_cells) / 2;
    for r in r0..r0 + side_cells {
        for c in c0..c0 + side_cells {
            labels[r * NX + c] = 1;
        }
    }

    let r_start = r0 + side_cells / 2 - 1;
    let mut c = c0 + 4;
    for &w in hole_cells {
        for r in r_start..r_start + w {
            for cc in c..c + w {
                labels[r * NX + cc] = 0;
            }
        }
        c += w + 4;
    }
    assert!(
        c < c0 + side_cells,
        "the holes must fit inside the square: ran to column {c}"
    );

    TierMap {
        nx: NX,
        ny: NY,
        origin_x: 0.0,
        origin_y: 0.0,
        cell_mm: CELL,
        labels,
        finest_z: vec![-1.0f32; NX * NY],
        tier_count: 2,
        tolerance_mm: 0.05,
        treatment: ResidualTreatment::SlopeCompensated,
    }
}

/// A hole `w` cells wide survives a band of `overlap_mm` when its centre is
/// further than the band from the nearest tier-1 cell. On a square hole the
/// deepest interior cell sits at `min(i+1, w−i)` cells from the wall, so the
/// survival test is `ceil(w / 2) · CELL > overlap_mm`, and the survivor is a
/// block of `(w − 2·round(overlap/CELL))` cells a side.
///
/// Stated here rather than left implicit because every width in this file is
/// chosen against it, and a survivor under one cell of area is dropped by the
/// extractor as a degenerate loop — which would read as a closed hole.
const fn survivor_cells(w: usize, overlap_cells: usize) -> usize {
    w.saturating_sub(2 * overlap_cells)
}

/// Island dials with the morphology turned OFF, so the fixture measures the
/// BAND and nothing else. A close radius would merge the holes' walls and a
/// min-area floor would drop the island itself; both are separately sentried
/// in `tier_islands_i1.rs`.
fn band_only_params(overlap_mm: f64) -> TierIslandParams {
    TierIslandParams {
        close_radius_mm: Some(0.0),
        min_region_area_mm2: Some(0.0),
        coarseness: 1.0,
        overlap_mm,
        max_regions_per_tier: 24,
        rim_erosion_mm: 0.0,
    }
}

// ── Fixture sentries ────────────────────────────────────────────────────

/// The load-bearing claim: a hole WIDER than `2 · overlap` survives the band,
/// a hole NARROWER than it does not, and the set says how many of each.
///
/// Overlap 1.0 mm is two cells, so it closes anything up to 2.0 mm wide. The
/// fixture punches holes of 2 and 3 cells (1.0 / 1.5 mm — both closed) and 8,
/// 10, 12 cells (4 / 5 / 6 mm — all kept, with 4- to 8-cell survivors so none
/// is lost as a degenerate loop).
///
/// A ONE-cell hole is deliberately absent. Measured here: it never reaches
/// `owned_hole_count` at all, because marching squares traces a single true
/// corner to half a cell of area and `region_mask` drops any loop under one
/// cell as degenerate. A fixture that punched one would be asserting the
/// band closed a hole the extractor had already thrown away.
#[test]
fn the_band_closes_the_narrow_holes_and_keeps_the_wide_ones() {
    assert_eq!(survivor_cells(3, 2), 0, "a 1.5 mm hole cannot survive 1 mm");
    assert_eq!(survivor_cells(8, 2), 4, "a 4 mm hole keeps a 2 mm core");

    let map = ring_island_map(90, &[2, 3, 8, 10, 12]);
    let params = band_only_params(1.0);
    let islands = extract_tier_islands(&map, &params, &[2.0, 1.0]).unwrap();

    let set = islands
        .set_for_tier(1)
        .expect("the fine tier keeps its island");
    assert_eq!(set.islands, 1, "one connected tier-1 body");
    assert_eq!(
        set.owned_hole_count, 5,
        "all five punched holes are owned holes before the band"
    );
    assert_eq!(
        set.machining_hole_count, 3,
        "the three holes wider than 2 x overlap survive the band"
    );
    assert_eq!(set.net_holes_closed_by_band(), 2);

    // The band both grows the outline and fills the narrow holes, so the
    // machining copy is strictly the larger.
    assert!(
        set.machining_area_mm2 > set.owned_area_mm2,
        "machining {:.1} must exceed owned {:.1}",
        set.machining_area_mm2,
        set.owned_area_mm2
    );
    let ratio = set
        .machining_to_owned_ratio()
        .expect("owned area is positive");
    assert!(
        (ratio - set.machining_area_mm2 / set.owned_area_mm2).abs() < 1e-12,
        "the ratio is the two published areas and nothing else"
    );

    // The band is a stated dial, carried on the set so a consumer that holds
    // the set but not the params can still name it.
    assert!((set.overlap_mm - 1.0).abs() < 1e-12);
}

/// With the band OFF the machining copy IS the owned copy: same area, same
/// holes, ratio exactly 1, no advisory. This is the arm that proves the
/// measurement reads the band and not something else about the extraction.
#[test]
fn a_zero_band_machines_exactly_what_it_owns() {
    let map = ring_island_map(90, &[2, 3, 8, 10, 12]);
    let islands = extract_tier_islands(&map, &band_only_params(0.0), &[2.0, 1.0]).unwrap();
    let set = islands.set_for_tier(1).unwrap();

    assert_eq!(set.owned_hole_count, 5);
    assert_eq!(set.machining_hole_count, 5, "no hole closes with no band");
    assert_eq!(set.net_holes_closed_by_band(), 0);
    assert!((set.machining_area_mm2 - set.owned_area_mm2).abs() < 1e-9);
    assert!((set.machining_to_owned_ratio().unwrap() - 1.0).abs() < 1e-12);
    assert!(
        set.band_advisory().is_none(),
        "a band that changes nothing has nothing to advise about"
    );
}

/// The advisory is a READING with a stated bound, not a refusal. A band fat
/// enough to close every hole and inflate the outline crosses
/// [`BAND_RATIO_ADVISORY_BOUND`]; a band that only blends the seam does not.
/// Nothing refuses in either arm.
#[test]
fn the_advisory_fires_on_a_fat_band_and_names_both_dials() {
    // A small island with small holes, the wanaka shape in miniature: a
    // 4 mm band closes every hole AND grows a 20 mm square to 28 mm.
    let map = ring_island_map(40, &[2, 2, 2, 2]);
    let islands = extract_tier_islands(&map, &band_only_params(4.0), &[2.0, 1.0]).unwrap();
    let set = islands.set_for_tier(1).unwrap();

    let advisory = set
        .band_advisory()
        .expect("a 4 mm band on a 20 mm island is over the bound");
    assert!(advisory.ratio > BAND_RATIO_ADVISORY_BOUND);
    assert!((advisory.bound - BAND_RATIO_ADVISORY_BOUND).abs() < 1e-12);
    assert_eq!(advisory.tier, 1);
    assert_eq!(advisory.owned_hole_count, 4);
    assert_eq!(advisory.net_holes_closed_by_band, 4);
    assert_eq!(advisory.machining_hole_count, 0);
    assert!((advisory.overlap_mm - 4.0).abs() < 1e-12);

    // Both levers are DIALS, and the text has to say which two.
    let text = advisory.to_string();
    assert!(text.contains("overlap_mm"), "text: {text}");
    assert!(text.contains("tolerance_mm"), "text: {text}");
    assert!(text.contains("holes from 4 to 0"), "text: {text}");

    // The median hole is a real hole's area, and the suggested overlap is
    // half its nominal width.
    let median = advisory
        .median_owned_hole_area_mm2
        .expect("this tier owns holes");
    assert!(
        median > 0.0 && median <= 4.0,
        "a 2-cell hole at 0.5 mm cells is of order 1 mm^2, got {median}"
    );
    let keep = advisory
        .overlap_that_keeps_the_median_hole_mm()
        .expect("a median implies a width");
    assert!((keep - median.sqrt() * 0.5).abs() < 1e-12);

    // The same map with a seam-scale band stays quiet. Nothing refused in
    // either arm: both extractions returned a set.
    let thin = extract_tier_islands(&map, &band_only_params(0.25), &[2.0, 1.0]).unwrap();
    let thin_set = thin.set_for_tier(1).unwrap();
    assert!(
        thin_set.band_advisory().is_none(),
        "a 0.25 mm band on this island reads {:?}",
        thin_set.machining_to_owned_ratio()
    );
    assert_eq!(thin.band_advisories().count(), 0);
}

/// A thin strip is what a hole becomes when the band eats most of it, and a
/// strip is what the scallop offset library refuses to build rings in. The
/// surviving holes must still be polygons with real area — never a collapsed
/// ring — so the downstream `pre_boundary_regions` consumer gets geometry it
/// can offset.
#[test]
fn a_surviving_hole_is_a_real_polygon_not_a_collapsed_ring() {
    // 8 cells = 4 mm wide, against a 1.0 mm band: a 2 mm core survives.
    assert_eq!(survivor_cells(8, 2), 4);
    let map = ring_island_map(90, &[8, 8, 8]);
    let islands = extract_tier_islands(&map, &band_only_params(1.0), &[2.0, 1.0]).unwrap();
    let set = islands.set_for_tier(1).unwrap();

    assert_eq!(set.machining_hole_count, 3, "all three survive as strips");
    let cell_area = CELL * CELL;
    for poly in set.machining.as_slice() {
        for hole in &poly.holes {
            assert!(
                hole.len() >= 4,
                "a surviving hole must be a closed loop, got {} vertices",
                hole.len()
            );
            let area = rs_cam_core::polygon::shoelace_area(hole).abs();
            assert!(
                area >= cell_area,
                "a surviving hole must carry at least one cell of area, got {area:.4} mm^2 \
                 — anything under that is the degenerate loop the extractor drops"
            );
        }
    }
}

// ── Wanaka instrument ───────────────────────────────────────────────────

/// Owned vs machining area at the planner's own dials, on the real board.
///
/// The dials come from the fixture the peer measured with
/// (`planning/deep_doc_modulation_2026-09-08/T3b_r10_scallop_islands_relink3.toml`):
/// tools 4 and 2, cell 0.4, margin 0.5, slope-compensated, **overlap 1.25**
/// — NOT [`rs_cam_core::maps::tier_islands::DEFAULT_OVERLAP_MM`] (2.0). Reference
/// readings from study doc §2.7a, owned / machining mm²:
///
/// | tolerance | owned | machining |
/// |---|---:|---:|
/// | 0.05 | 12 224 | 29 954 |
/// | 0.146 | 4 482 | 17 169 |
/// | 0.30 | 641 | 3 005 |
///
/// Also prints the ratio under a candidate default improvement — the overlap
/// clamped to half a median sliver width — as a MEASUREMENT, not a change:
/// nothing in the shipped path reads it.
#[test]
#[ignore = "loads the wanaka mesh and walks three full tier maps; run with --ignored --nocapture"]
fn wanaka_owned_versus_machining_area_at_three_tolerances() {
    use std::path::PathBuf;
    use std::sync::atomic::AtomicBool;

    use rs_cam_core::session::{MultitoolPlanSpec, ProjectSession};

    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("planning")
        .join("deep_doc_modulation_2026-09-08")
        .join("T3b_r10_scallop_islands_relink3.toml");
    assert!(
        path.exists(),
        "instrument fixture not found at {}",
        path.display()
    );
    let session = ProjectSession::load(&path).expect("load the T3b project");

    let cancel = AtomicBool::new(false);
    let plan = |tolerance_mm: f64, islands: TierIslandParams| MultitoolPlanSpec {
        setup_index: 0,
        model_id: 0,
        tool_ids: vec![4, 2],
        cell_mm: 0.4,
        tolerance_mm,
        margin_mm: 0.5,
        islands,
        ..MultitoolPlanSpec::default()
    };

    eprintln!("── G-OVERLAPFILL: wanaka owned vs machining, overlap 1.25 mm ──");
    eprintln!(
        "{:>6}  {:>5}  {:>10}  {:>10}  {:>7}  {:>7}  {:>7}  {:>9}",
        "tol", "isles", "owned mm2", "mach mm2", "ratio", "owned h", "mach h", "median mm2"
    );

    for tolerance_mm in [0.05, 0.146, 0.30] {
        let band = TierIslandParams {
            overlap_mm: 1.25,
            ..TierIslandParams::default()
        };
        let preview = session
            .preview_multitool_plan(&plan(tolerance_mm, band), &cancel)
            .expect("the tier map walks");

        eprintln!(
            "  ladder {:?} cusp radii {:?}",
            preview.tool_names, preview.cusp_radii_mm
        );
        for set in &preview.islands.per_tier {
            eprintln!(
                "  cap: raises {}, truncated {}, dropped {}, close radius {:.3} mm, \
                 min island {:.1} mm2",
                set.cap.close_raises,
                set.cap.truncated(),
                set.cap.dropped(),
                set.close_radius_mm,
                set.min_region_area_mm2,
            );
            eprintln!(
                "{tolerance_mm:>6.3}  {:>5}  {:>10.0}  {:>10.0}  {:>6.2}x  {:>7}  {:>7}  {:>9}",
                set.islands,
                set.owned_area_mm2,
                set.machining_area_mm2,
                set.machining_to_owned_ratio().unwrap_or(f64::NAN),
                set.owned_hole_count,
                set.machining_hole_count,
                set.median_owned_hole_area_mm2
                    .map_or_else(|| "-".to_owned(), |m| format!("{m:.2}")),
            );
        }

        // Reference arm: the SAME map with the morphological close OFF, so
        // the owned column is the tier's RAW territory. The shipped close
        // radius fills every gap under 2 x its radius, and a dendritic
        // network is mostly gaps — which is why owned area is NOT monotonic
        // in tolerance on the row above.
        let raw = session
            .preview_multitool_plan(
                &plan(
                    tolerance_mm,
                    TierIslandParams {
                        overlap_mm: 1.25,
                        close_radius_mm: Some(0.0),
                        ..TierIslandParams::default()
                    },
                ),
                &cancel,
            )
            .expect("the tier map is cached; only the morphology re-runs");
        for set in &raw.islands.per_tier {
            eprintln!(
                "  no-close reference -> {:>4} islands, owned {:.0}, machining {:.0} ({:.2}x)",
                set.islands,
                set.owned_area_mm2,
                set.machining_area_mm2,
                set.machining_to_owned_ratio().unwrap_or(f64::NAN),
            );
        }
        for advisory in preview.islands.band_advisories() {
            eprintln!("  ADVISORY {advisory}");
        }

        // Candidate default improvement, MEASURED ONLY: clamp the overlap to
        // half a nominal median sliver width. Nothing ships this.
        let candidate = preview
            .islands
            .per_tier
            .first()
            .and_then(|s| s.median_owned_hole_area_mm2)
            .filter(|a| *a > 0.0)
            .map(|a| (a.sqrt() * 0.5).min(1.25));
        if let Some(overlap_mm) = candidate {
            let clamped = session
                .preview_multitool_plan(
                    &plan(
                        tolerance_mm,
                        TierIslandParams {
                            overlap_mm,
                            ..TierIslandParams::default()
                        },
                    ),
                    &cancel,
                )
                .expect("the tier map is cached; only the morphology re-runs");
            for set in &clamped.islands.per_tier {
                eprintln!(
                    "  candidate overlap {overlap_mm:.2} mm -> owned {:.0}, machining {:.0} \
                     ({:.2}x), holes {} -> {}",
                    set.owned_area_mm2,
                    set.machining_area_mm2,
                    set.machining_to_owned_ratio().unwrap_or(f64::NAN),
                    set.owned_hole_count,
                    set.machining_hole_count,
                );
            }
        }
    }
}
