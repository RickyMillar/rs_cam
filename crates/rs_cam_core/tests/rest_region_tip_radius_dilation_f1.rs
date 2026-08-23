//! F1 sentry — the rest-region dilation and the boundary moat are sized off
//! the cutter's TIP, not its shank.
//!
//! `rest_field::detect_rest_valleys` produces the polygons that
//! `BoundarySource::DerivedRestRegions` turns into the territory a fine-tool
//! operation is allowed to cut. Two radii feed that pipeline and both used to
//! read `MillingCutter::radius()` — the ENVELOPE, which on a tapered ball is
//! the SHAFT:
//!
//! * the mask -> polygon dilation, `radius() + region_margin_mm`;
//! * the boundary trust-region erosion (the "moat"),
//!   `max(pencil.radius(), reference.erosion_radius())`.
//!
//! On the 1 mm-tip / 6 mm-shank taper this project finishes with, that is
//! `3.0 + 0.5 = 3.5 mm` of dilation — enough to bridge a 7 mm gap — and a
//! 3 mm blanked rim. The welding half is the same number `unified_finish`'s
//! `territory_clip` measured and deliberately refused to use ("a 3.5 mm
//! dilation welds a dendritic keep-mask into full coverage"); the rim half is
//! T1 finding G6. Both are `planning/multitool_2026-08-23/T1_FINDINGS.md` and
//! `T2_FINDINGS.md` section 2.5.
//!
//! The fix is `cusp_radius()`, which is the tip sphere on a tapered ball and
//! **identical to `radius()` on every other shape** — so this file pins the
//! taper arm as the thing that moves and a same-tip-scale BALL as the control
//! that does not.
//!
//! # The fixture
//!
//! A 40x40 mm plate carrying four straight V-grooves at x = -12, -4, +4, +12
//! — 0.9 mm rim half-width, 2.0 mm deep, invariant in Y. The 12 mm ball
//! reference cannot enter any of them (it would need a 4 mm half-width to dip
//! 2 mm), so every groove is rest material and the mask is four parallel
//! ribbons about 1.1 mm half-wide, separated by roughly 5.8 mm of clean gap.
//!
//! That gap is the discriminator, and it sits between the two dilations
//! rather than near either: a 3.5 mm dilation bridges up to 7 mm and welds
//! all four into one region; a 1.0 mm dilation (0.5 tip + 0.5 margin) bridges
//! 2 mm and leaves four.
//!
//! Nothing here is a gate — the regions are report / boundary artifacts.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::rest_field::{RestFieldParams, RestReference, detect_rest_valleys};
use rs_cam_core::tool::{BallEndmill, BullNoseEndmill, FlatEndmill, MillingCutter, VBitEndmill};

mod common;

use common::meshes::height_field_grid;
use common::tools::wanaka_taper;

/// Plate half-extent (mm). The plate is `2 * HALF` square.
const HALF: f64 = 20.0;
/// Groove centre X positions (mm) — 8 mm apart, all well inside the moat.
const GROOVE_CENTRES: [f64; 4] = [-12.0, -4.0, 4.0, 12.0];
/// Groove rim half-width (mm).
const GROOVE_HALF_WIDTH: f64 = 0.9;
/// Groove depth (mm).
const GROOVE_DEPTH: f64 = 2.0;
/// Rest-field cell (mm). Seven cells across a groove.
const CELL_MM: f64 = 0.25;

/// The plate's own XY area (mm²) — the denominator every share below uses.
fn plate_area_mm2() -> f64 {
    (2.0 * HALF) * (2.0 * HALF)
}

/// Surface height at `x`: flat at 0 except inside a groove, where it dips
/// linearly to `-GROOVE_DEPTH` at the centre.
fn groove_z(x: f64) -> f64 {
    let mut z = 0.0_f64;
    for c in GROOVE_CENTRES {
        let d = (x - c).abs();
        if d < GROOVE_HALF_WIDTH {
            z = z.min(-GROOVE_DEPTH * (1.0 - d / GROOVE_HALF_WIDTH));
        }
    }
    z
}

/// The four-groove comb. Y-invariant, so X is sampled finely (0.1 mm, 18
/// samples across a groove) and Y coarsely — the mesh has to resolve the
/// groove profile, not its length.
fn comb_plate() -> TriangleMesh {
    let x_step = 0.1;
    let y_step = 2.0;
    let nx = ((2.0 * HALF) / x_step).round() as usize + 1;
    let ny = ((2.0 * HALF) / y_step).round() as usize + 1;
    height_field_grid(-HALF, x_step, nx, -HALF, y_step, ny, |x, _y| groove_z(x))
}

fn params() -> RestFieldParams {
    RestFieldParams {
        cell_mm: CELL_MM,
        min_valley_depth: 0.05,
        offset_stepover_mm: 0.25,
        num_offset_passes_cap: 4,
        min_cut_length: 2.0,
        region_margin_mm: 0.5,
    }
}

/// Derived region polygons for `cutter`, measured against a 12 mm ball.
///
/// The reference is deliberately the BIGGER of the two erosion terms
/// (`radius() == 6.0` beats either arm of the pencil's), so the moat is held
/// constant across the two cutters and the only thing that can move the
/// region count is the DILATION. The moat is measured separately, below.
fn derived_regions(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
) -> Vec<Polygon2> {
    let reference = BallEndmill::new(12.0, 25.0);
    let rf = detect_rest_valleys(
        mesh,
        index,
        cutter,
        RestReference::Cutter {
            tool: &reference as &dyn MillingCutter,
            is_surface_probe: false,
        },
        &params(),
    );
    rf.region_polygons
}

fn total_region_area_mm2(regions: &[Polygon2]) -> f64 {
    regions.iter().map(Polygon2::area).sum()
}

/// The headline: a 1 mm-tip / 6 mm-shank taper's derived regions stay
/// DISJOINT.
///
/// RED before F1: the 3.5 mm envelope dilation bridges the ~5.8 mm gaps and
/// returns ONE region covering most of the comb. GREEN after: the 1.0 mm tip
/// dilation returns the four islands the mask actually found.
#[test]
fn a_tapered_ball_keeps_dendritic_rest_islands_disjoint() {
    let mesh = comb_plate();
    let index = SpatialIndex::build_auto(&mesh);
    let taper = wanaka_taper();

    // Fixture precondition: this tool is the envelope/cusp split the test is
    // about. Without it the assertions below prove nothing.
    assert!(
        taper.envelope_radius_mm() > 4.0 * taper.cusp_radius_mm(),
        "fixture tool has no shank/tip split: envelope {} vs cusp {}",
        taper.envelope_radius_mm(),
        taper.cusp_radius_mm()
    );

    let regions = derived_regions(&mesh, &index, &taper);
    let area = total_region_area_mm2(&regions);
    let count = regions.len();

    assert!(
        count >= GROOVE_CENTRES.len(),
        "four separated grooves must yield at least four derived regions; got \
         {count} covering {area:.0} mm². One giant region is the \
         envelope-radius weld this sentry exists for — the dilation must be \
         the TIP radius ({:.2} mm), not the shank ({:.2} mm).",
        taper.cusp_radius_mm(),
        taper.envelope_radius_mm(),
    );
    assert!(
        area < 0.5 * plate_area_mm2(),
        "the derived regions cover {area:.0} mm² of a {:.0} mm² plate — a \
         rest boundary that claims half the part confines nothing. Welded \
         islands are the cause: four 1 mm-wide ribbons dilated by 1 mm are \
         nowhere near this.",
        plate_area_mm2(),
    );
}

/// The control: a BALL of the same tip scale, where `cusp_radius()` and
/// `radius()` are the same number, must be unaffected by F1 — four regions
/// before and four after.
///
/// This is what makes the test above a statement about the tapered arm
/// rather than about the fixture. A 1 mm ball has radius 0.5 == cusp 0.5, so
/// it sees the identical 1.0 mm dilation the fixed taper sees, and it saw
/// that same 1.0 mm before the fix too.
#[test]
fn a_ball_of_the_same_tip_scale_is_the_unchanged_control() {
    let mesh = comb_plate();
    let index = SpatialIndex::build_auto(&mesh);
    let ball = BallEndmill::new(1.0, 25.0);

    assert!(
        (ball.cusp_radius_mm() - ball.envelope_radius_mm()).abs() < 1e-12,
        "a ball's two radii must be one number, or it is not a control"
    );

    let regions = derived_regions(&mesh, &index, &ball);
    assert!(
        regions.len() >= GROOVE_CENTRES.len(),
        "the control lost islands ({} of {}) — F1 must not move anything \
         where tip == envelope",
        regions.len(),
        GROOVE_CENTRES.len(),
    );
    assert!(
        total_region_area_mm2(&regions) < 0.5 * plate_area_mm2(),
        "the control's regions welded, which would mean the fixture — not the \
         radius accessor — decides the island count"
    );
}

/// The moat: the boundary erosion is the tip radius too, so a tapered ball
/// stops blanking a 3 mm ring of readings it can make perfectly well (T1 G6).
///
/// Measured as the trusted footprint `RestGrid` publishes. The reference here
/// is the bare-surface PROBE, whose own erosion term is negligible, so the
/// pencil's term is the one in force — the arrangement the shipped pencil
/// cascade uses when no bigger reference tool is configured.
///
/// RED before F1: erosion 3.0 mm leaves ~34.5 x 34.5 mm trusted of a
/// 40 x 40 mm plate (~75%). GREEN after: erosion 0.5 mm leaves ~39.5 x 39.5
/// mm (~99%).
#[test]
fn the_moat_erosion_is_sized_off_the_tip_not_the_shank() {
    let mesh = comb_plate();
    let index = SpatialIndex::build_auto(&mesh);
    let taper = wanaka_taper();
    let probe = BallEndmill::new(0.1, 10.0);

    let rf = detect_rest_valleys(
        &mesh,
        &index,
        &taper,
        RestReference::Cutter {
            tool: &probe as &dyn MillingCutter,
            is_surface_probe: true,
        },
        &params(),
    );

    let trusted = rf.rest_grid.covered_footprint_area_mm2();
    let plate = plate_area_mm2();
    assert!(
        trusted > 0.88 * plate,
        "only {trusted:.0} mm² of a {plate:.0} mm² plate is trusted \
         ({:.0}%). A {:.2} mm shank-radius moat blanks a ring the {:.2} mm \
         TIP reads perfectly well — and it is the part EDGE, where the \
         operator cares.",
        100.0 * trusted / plate,
        taper.envelope_radius_mm(),
        taper.cusp_radius_mm(),
    );
}

/// F1 is an identity transformation for every non-tapered shape, by
/// construction: `cusp_radius()` falls through to `radius()` unless the
/// geometry hint is `TaperedBall`. Stated here as an executable claim so the
/// controls above are not the only thing saying so.
#[test]
fn every_non_tapered_shape_keeps_the_radius_it_had() {
    let flat = FlatEndmill::new(6.0, 25.0);
    let ball = BallEndmill::new(6.0, 25.0);
    let bull = BullNoseEndmill::new(6.0, 1.0, 25.0);
    let vbit = VBitEndmill::new(6.0, 90.0, 25.0);
    let shapes: [(&str, &dyn MillingCutter); 4] = [
        ("flat", &flat),
        ("ball", &ball),
        ("bullnose", &bull),
        ("v-bit", &vbit),
    ];
    for (label, cutter) in shapes {
        assert!(
            (cutter.cusp_radius_mm() - cutter.envelope_radius_mm()).abs() < 1e-12,
            "{label}: cusp {} != envelope {} — F1 would change this shape's \
             dilation and moat, which it must not",
            cutter.cusp_radius_mm(),
            cutter.envelope_radius_mm(),
        );
    }

    // And the split the fix exists for is real on the taper.
    let taper = wanaka_taper();
    assert!((taper.envelope_radius_mm() - 3.0).abs() < 1e-9);
    assert!((taper.cusp_radius_mm() - 0.5).abs() < 1e-9);
}
