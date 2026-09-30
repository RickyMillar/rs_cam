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

use rs_cam_core::maps::grid::GridSpec;
use rs_cam_core::maps::tier_islands::{
    BAND_RATIO_ADVISORY_BOUND, MAX_CLOSE_RAISES, TierIslandParams, extract_tier_islands,
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
        grid: GridSpec {
            nx: NX,
            ny: NY,
            origin_x: 0.0,
            origin_y: 0.0,
            cell_mm: CELL,
        },
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
        // The morphology is off (close radius 0), so no raise can merge
        // anything; the bound is the default only to keep the struct exact.
        max_close_raises: MAX_CLOSE_RAISES,
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
/// (`tests/fixtures/t3b_r10_scallop_islands_2026-09-08_e4d817e9.toml`, a
/// byte-identical copy of the peer's
/// `planning/deep_doc_modulation_2026-09-08/T3b_r10_scallop_islands_relink3.toml`):
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

    // FIN-06: a byte-identical copy of the peer's measurement fixture
    // (sha256 prefix `e4d817e9`). `planning/` holds the evidence, not the
    // fixture a test loads.
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("t3b_r10_scallop_islands_2026-09-08_e4d817e9.toml");
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

// ── Tiered-finish Step 0 instrument (rivmap100, tolerance 0.15) ─────────

/// The rivmap100 tiered-finish benchmark project. `planning/fixtures/` holds
/// the benchmark projects (root `CLAUDE.md`); the README there says how this
/// one was built.
fn rivmap100_tiered_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../planning/fixtures/rivmap100/rivmap100_tiered_finish.toml")
}

/// The ladder the fixture plans: the R2.0 taper (tool 12, copied from the
/// wanaka library in `tests/fixtures/t3b_r10_scallop_islands_*.toml`) over
/// the project's R1.0 taper (tool 6). The operator's own 500 x 500 ladder is
/// not in the repo; this is the proxy the plan names.
const RIVMAP_LADDER: [usize; 2] = [12, 6];
/// The operator's plan tolerance (2026-09-28 symptom).
const RIVMAP_TOLERANCE_MM: f64 = 0.15;

/// Writes the tier chain into the fixture: `plan_multitool_finishing` at the
/// planner's own dials, tolerance 0.15, then `save`. Run once to (re)build
/// the fixture; the Step 0 instrument below reads it.
#[test]
#[ignore = "rewrites planning/fixtures/rivmap100/rivmap100_tiered_finish.toml"]
fn write_rivmap100_tiered_finish_fixture() {
    use rs_cam_core::session::{MultitoolPlanSpec, ProjectSession};

    let path = rivmap100_tiered_path();
    let mut session = ProjectSession::load(&path).expect("load the rivmap100 tiered project");
    let spec = MultitoolPlanSpec {
        setup_index: 0,
        model_id: 1,
        tool_ids: RIVMAP_LADDER.to_vec(),
        tolerance_mm: RIVMAP_TOLERANCE_MM,
        ..MultitoolPlanSpec::default()
    };
    let outcome = session
        .plan_multitool_finishing(&spec)
        .expect("the ladder plans");
    eprintln!(
        "planned {} tier op(s), replaced {}",
        outcome.toolpath_ids.len(),
        outcome.replaced.len()
    );
    session.save(&path).expect("save the fixture");
}

/// Step 0 of the tiered-finish plan: per tier raw / owned / machining area,
/// the cap report, hole counts, the F3c flute-reach reading, then the
/// holder/shank check on the generated fine-tier op.
///
/// Measurement only. Prints; asserts nothing but that the pieces ran.
#[test]
#[ignore = "walks the rivmap100 tier map and generates the fine tier; run with --ignored --nocapture"]
fn rivmap100_tiered_finish_step0() {
    use std::sync::atomic::AtomicBool;

    use rs_cam_core::compute::config::{BoundarySource, StockSource};
    use rs_cam_core::compute::cutter::build_cutter;
    use rs_cam_core::mesh::SpatialIndex;
    use rs_cam_core::session::{Command, MultitoolPlanSpec, ProjectSession, SetStockSourceArgs};
    use rs_cam_core::stock::collision::{
        AssemblySegment, BODY_STRIKE_THRESHOLD_MM, body_segment_penetration_mm, check_collisions,
    };
    use rs_cam_core::toolpath::MoveType;

    let mut session =
        ProjectSession::load(&rivmap100_tiered_path()).expect("load the rivmap100 tiered project");
    let cancel = AtomicBool::new(false);

    // The recipe as stored on the fine tier's boundary — the preview reads
    // the same dials the generation will.
    let (fine_index, recipe) = session
        .toolpath_configs()
        .iter()
        .enumerate()
        .find_map(|(i, tc)| match (&tc.planner_origin, &tc.boundary.source) {
            (Some(o), BoundarySource::PlannedTierRegions { .. }) if o.tier == 1 => {
                Some((i, tc.boundary.source.clone()))
            }
            _ => None,
        })
        .expect("the fixture carries a planned fine tier; run the writer first");
    let BoundarySource::PlannedTierRegions {
        tool_ids,
        cell_mm,
        tolerance_mm,
        margin_mm,
        treatment,
        islands,
        ..
    } = recipe
    else {
        unreachable!("matched above")
    };
    let spec = |islands| MultitoolPlanSpec {
        setup_index: 0,
        model_id: 1,
        tool_ids: tool_ids.clone(),
        cell_mm,
        tolerance_mm,
        margin_mm,
        treatment,
        islands,
        ..MultitoolPlanSpec::default()
    };

    let stored = session
        .preview_multitool_plan(&spec(islands), &cancel)
        .expect("the stored recipe previews");
    print_step0_preview("stored recipe (current default raise bound)", &stored);
    let dial0 = session
        .preview_multitool_plan(
            &spec(TierIslandParams {
                max_close_raises: 0,
                ..islands
            }),
            &cancel,
        )
        .expect("the tier map is cached; only the morphology re-runs");
    print_step0_preview("F1 arm: max_close_raises 0", &dial0);
    let no_close = session
        .preview_multitool_plan(
            &spec(TierIslandParams {
                close_radius_mm: Some(0.0),
                max_close_raises: 0,
                ..islands
            }),
            &cancel,
        )
        .expect("the tier map is cached; only the morphology re-runs");
    print_step0_preview("reference: close off, no raise", &no_close);

    // ── The holder/shank check on the generated fine tier ────────────────
    // Stock source Fresh: the remaining-stock chain needs the rough and
    // tier 0 simulated first. A finishing path follows the mesh either way;
    // Fresh changes which air moves the air-cut filter trims.
    let _ = session
        .apply(Command::SetStockSource(SetStockSourceArgs {
            index: fine_index,
            source: StockSource::Fresh,
        }))
        .expect("set the fine tier's stock source");
    let t0 = std::time::Instant::now();
    session
        .generate_toolpath(fine_index, &cancel)
        .expect("the fine tier generates");
    eprintln!(
        "── fine tier generated in {:.1} s ──",
        t0.elapsed().as_secs_f64()
    );
    let tp = session.get_result(fine_index).unwrap().toolpath().clone();
    let tc = session.get_toolpath_config(fine_index).unwrap().clone();
    let tool = session
        .tools()
        .iter()
        .find(|t| t.id.0 == tc.tool_id)
        .unwrap()
        .clone();
    let def = build_cutter(&tool);
    let assembly = def.to_assembly();
    let mesh = session
        .models()
        .iter()
        .find(|m| m.id == tc.model_id)
        .and_then(|m| m.mesh.clone())
        .expect("the terrain mesh");
    let index = SpatialIndex::build_auto(&mesh);

    let cutting: Vec<usize> = tp
        .moves
        .iter()
        .enumerate()
        .filter(|(_, m)| !matches!(m.move_type, MoveType::Rapid))
        .map(|(i, _)| i)
        .collect();
    eprintln!(
        "  moves {}, cutting moves {}, tool '{}' L_c {} mm, shank r {} mm, holder r {} mm, \
         cutter envelope r {} mm",
        tp.moves.len(),
        cutting.len(),
        tool.name,
        assembly.cutter_length,
        assembly.shank_diameter / 2.0,
        assembly.holder_diameter / 2.0,
        assembly.cutter_radius,
    );

    let report = check_collisions(&tp, &assembly, &mesh, &index);
    let shank = report
        .collisions
        .iter()
        .filter(|c| c.segment == AssemblySegment::Shank)
        .count();
    let holder = report.collisions.len() - shank;
    eprintln!(
        "  check_collisions (endpoints): {} strikes — shank {shank}, holder {holder}; \
         min safe stickout {:.2} mm",
        report.collisions.len(),
        report.min_safe_stickout,
    );
    let session_check = session
        .collision_check(fine_index, &cancel)
        .expect("the session check runs");
    eprintln!(
        "  session collision_check (1 mm samples): {} strikes",
        session_check.collision_report.collisions.len()
    );

    // The same rule with no envelope skip: the body directly above the
    // flutes, at every cutting-move endpoint.
    let (z_off, body_r) = assembly.non_fluted_body().expect("a body above the flutes");
    let mut body_strikes = 0usize;
    let mut worst = 0.0f64;
    for &i in &cutting {
        let tip = tp.moves[i].target;
        if let Some(p) = body_segment_penetration_mm(tip, z_off, body_r, &mesh, &index)
            && p > BODY_STRIKE_THRESHOLD_MM
        {
            body_strikes += 1;
            worst = worst.max(p);
        }
    }
    eprintln!(
        "  body above the flutes (r {body_r} mm at {z_off} mm, no envelope skip): \
         {body_strikes} of {} cutting endpoints strike, worst {worst:.3} mm",
        cutting.len()
    );
}

/// The overlap the islands were grown by, read off the first fine tier.
fn islands_overlap(p: &rs_cam_core::session::MultitoolPreview) -> f64 {
    p.islands.per_tier.first().map_or(0.0, |s| s.overlap_mm)
}

/// One Step 0 preview, printed: per tier raw / owned / machining area, the
/// cap report, the holes and the F3c flute-reach reading.
fn print_step0_preview(label: &str, p: &rs_cam_core::session::MultitoolPreview) {
    eprintln!("── {label} ──");
    eprintln!(
        "  ladder {:?} cusp radii {:?}, cell {} mm, tolerance {} mm, overlap {} mm, \
         raise bound {}",
        p.tool_names,
        p.cusp_radii_mm,
        p.map.grid.cell_mm,
        p.map.tolerance_mm,
        islands_overlap(p),
        p.islands
            .per_tier
            .first()
            .map_or(0, |s| s.cap.max_close_raises),
    );
    for k in 0..p.map.tier_count {
        eprintln!(
            "  tier {k}: raw (labelled) area {:.0} mm2",
            p.map.tier_area_mm2(k)
        );
    }
    for set in &p.islands.per_tier {
        let raw = p.map.tier_area_mm2(usize::from(set.tier));
        eprintln!(
            "  tier {}: raw islands {}, raw {:.0} mm2 -> owned {:.0} mm2 ({:.2}x raw) -> \
             machining {:.0} mm2 ({:.2}x owned, {:.2}x raw)",
            set.tier,
            set.raw_island_count,
            raw,
            set.owned_area_mm2,
            set.owned_area_mm2 / raw,
            set.machining_area_mm2,
            set.machining_to_owned_ratio().unwrap_or(f64::NAN),
            set.machining_area_mm2 / raw,
        );
        eprintln!(
            "    cap: after close {}, after min area {}, kept {} (cap {}), raises {} of {}, \
             radius {:.4} -> {:.4} mm, dropped {} ({:.1} mm2), min island {:.1} mm2",
            set.cap.islands_after_close,
            set.cap.islands_after_min_area,
            set.cap.kept,
            set.cap.cap,
            set.cap.close_raises,
            set.cap.max_close_raises,
            set.cap.first_close_radius_mm,
            set.cap.final_close_radius_mm,
            set.cap.dropped(),
            set.cap.dropped_area_mm2,
            set.min_region_area_mm2,
        );
        eprintln!(
            "    holes: owned {}, machining {}, median owned hole {}",
            set.owned_hole_count,
            set.machining_hole_count,
            set.median_owned_hole_area_mm2
                .map_or_else(|| "-".to_owned(), |m| format!("{m:.2} mm2")),
        );
    }
    for r in &p.flute_reach {
        eprintln!(
            "  flute reach tier {}: {} of {} checked owned cells bind ({:.1} mm2), body r {:?} \
             at {:?} mm",
            r.tier,
            r.binding_cells,
            r.checked_cells,
            r.binding_area_mm2,
            r.body_radius_mm,
            r.body_z_offset_mm,
        );
    }
    for a in p.islands.band_advisories() {
        eprintln!("  ADVISORY {a}");
    }
}

/// Step 0 on the ×3.5 terrain (`rivmap100_memory_repro.toml`: 350 x 350 x
/// 42 mm, the board nearest the operator's 500 x 500 in the repo), preview
/// only: the same ladder, tolerance and dials as the rivmap100 fixture. The
/// R2.0 taper is added in memory from the tiered fixture. No toolpath is
/// generated: at this size the fine tier takes too long in a debug build,
/// and the F3c flute-reach pass is the per-cell reading of the same rule.
#[test]
#[ignore = "walks a 350 x 350 mm tier map; run with --ignored --nocapture"]
fn rivmap350_tiered_finish_step0_preview() {
    use std::sync::atomic::AtomicBool;

    use rs_cam_core::session::{AddToolArgs, Command, MultitoolPlanSpec, ProjectSession};

    let donor =
        ProjectSession::load(&rivmap100_tiered_path()).expect("load the rivmap100 tiered project");
    let coarse = donor
        .tools()
        .iter()
        .find(|t| t.id.0 == RIVMAP_LADDER[0])
        .expect("the R2.0 taper")
        .clone();
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../planning/fixtures/rivmap100/rivmap100_memory_repro.toml");
    let mut session = ProjectSession::load(&path).expect("load the x3.5 project");
    let created = session
        .apply(Command::AddTool(AddToolArgs {
            tool: Box::new(coarse),
        }))
        .expect("add the R2.0 taper")
        .created
        .expect("add_tool reports the new index");
    let coarse_id = session.tools()[created].id.0;

    let cancel = AtomicBool::new(false);
    let spec = |islands: TierIslandParams| MultitoolPlanSpec {
        setup_index: 0,
        model_id: 1,
        tool_ids: vec![coarse_id, RIVMAP_LADDER[1]],
        tolerance_mm: RIVMAP_TOLERANCE_MM,
        islands,
        ..MultitoolPlanSpec::default()
    };
    let t0 = std::time::Instant::now();
    let stored = session
        .preview_multitool_plan(&spec(TierIslandParams::default()), &cancel)
        .expect("the x3.5 terrain previews");
    eprintln!(
        "── walk + islands + flute reach {:.1} s ──",
        t0.elapsed().as_secs_f64()
    );
    print_step0_preview("x3.5: planner defaults (raise bound 3)", &stored);
    let dial0 = session
        .preview_multitool_plan(
            &spec(TierIslandParams {
                max_close_raises: 0,
                ..TierIslandParams::default()
            }),
            &cancel,
        )
        .expect("cached map");
    print_step0_preview("x3.5: F1 arm, max_close_raises 0", &dial0);
    let no_close = session
        .preview_multitool_plan(
            &spec(TierIslandParams {
                close_radius_mm: Some(0.0),
                max_close_raises: 0,
                ..TierIslandParams::default()
            }),
            &cancel,
        )
        .expect("cached map");
    print_step0_preview("x3.5: reference, close off, no raise", &no_close);

    // F3 at the operator's flute length: the operator's R1.0 taper has
    // 15 mm of flute (PROGRESS 2026-09-29/30), the library copy here 20 mm.
    // A clone of tool 6 with 15 mm of flute; nothing else changes.
    let mut short = session
        .tools()
        .iter()
        .find(|t| t.id.0 == RIVMAP_LADDER[1])
        .expect("the R1.0 taper")
        .clone();
    short.cutting_length = 15.0;
    short.name = format!("{} (L_c 15)", short.name);
    let created = session
        .apply(Command::AddTool(AddToolArgs {
            tool: Box::new(short),
        }))
        .expect("add the 15 mm flute copy")
        .created
        .expect("add_tool reports the new index");
    let short_id = session.tools()[created].id.0;
    let lc15 = session
        .preview_multitool_plan(
            &MultitoolPlanSpec {
                tool_ids: vec![coarse_id, short_id],
                ..spec(TierIslandParams::default())
            },
            &cancel,
        )
        .expect("the x3.5 terrain previews with the 15 mm flute copy");
    print_step0_preview("x3.5: F3 arm, fine tool with 15 mm of flute", &lc15);
}
