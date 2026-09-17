//! F3 sentry — `MAX_REST_REGIONS` may truncate a region list, but it may no
//! longer do it silently.
//!
//! `region_mask::MAX_REST_REGIONS` keeps the largest 64 regions by area and
//! drops the rest. Until F3 the only trace was a `tracing::warn!`, in a
//! process that usually installs no subscriber — so a 64-long region list was
//! indistinguishable from a complete one at every consumer, including the
//! `BoundarySource::DerivedRestRegions` cascade that turns those polygons
//! into the territory a downstream operation is *allowed to cut*
//! (`planning/multitool_2026-08-23/T2_FINDINGS.md` §3.4).
//!
//! What this pins:
//!
//! (a) the true PRE-CAP count and the truncation flag come back from the
//!     extractor, not just out of a log line;
//! (b) the cap's behaviour is unchanged — the same largest-64-by-area, in
//!     the same order, as the plain `Vec<Polygon2>` name returns;
//! (c) a healthy set reports `truncated() == false` — a measurement, not a
//!     silence;
//! (d) the report reaches an operator surface: `ToolpathStats::region_cap`
//!     renders a `geom.region_cap_truncated` diagnostic carrying the true
//!     count, and is silent on both the clean and the not-measured states.
//!
//! Report-only throughout: no gate consumes any of this and no verdict moves
//! on it.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::toolpath_stats::ToolpathStats;
use rs_cam_core::diagnostics::adapters::from_generation::diagnostics_from_generation;
use rs_cam_core::diagnostics::{DiagnosticId, Scope, ids};
use rs_cam_core::geometry::grid2::Grid2;
use rs_cam_core::geometry::region_mask::{
    MAX_REST_REGIONS, RegionCapReport, region_polygons_from_mask,
    region_polygons_from_mask_reported,
};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::polygon::Polygon2;

/// `count` well-separated 3×3 blocks plus one clearly-largest 15×15 block.
///
/// Same construction as `region_mask.rs`'s own cap test, including the `+1`
/// offset on every coordinate: marching squares cannot close a loop that
/// touches the grid boundary, so blocks flush against row/col 0 silently
/// trace to nothing (the failure that fixture first shipped with).
fn island_storm(small_blocks: usize) -> Grid2<bool> {
    let block = 3usize;
    let step = 6usize; // 3-cell gap: never touches, even diagonally
    let cols = 10usize;
    let rows = small_blocks.div_ceil(cols);
    let small_ny = (rows.max(1) - 1) * step + block;
    let nx = (cols - 1) * step + block + 2;
    let giant = 15usize;
    let giant_row0 = small_ny + 6;
    let ny = giant_row0 + giant + 2;

    let mut mask = Grid2::new_fill(nx, ny, false);
    for i in 0..small_blocks {
        let gr = i / cols;
        let gc = i % cols;
        for r in 0..block {
            for c in 0..block {
                mask.set(gr * step + r + 1, gc * step + c + 1, true);
            }
        }
    }
    for r in 0..giant {
        for c in 0..giant {
            mask.set(giant_row0 + r, c + 1, true);
        }
    }
    mask
}

/// (a) + (b): the storm's true island count survives as a number, and the
/// polygons the reported call returns are byte-for-byte the ones the plain
/// call already returned.
#[test]
fn a_storm_reports_its_true_pre_cap_count_and_keeps_the_same_regions() {
    // 100 small blocks + 1 giant = 101 islands, comfortably past the cap.
    let mask = island_storm(100);
    let cell = 1.0;

    let plain = region_polygons_from_mask(&mask, 0.0, 0.0, cell, 0.0);
    let reported = region_polygons_from_mask_reported(&mask, 0.0, 0.0, cell, 0.0, None);

    assert_eq!(
        reported.cap.total_before_cap, 101,
        "the pre-cap count must be the islands the MASK produced, not the \
         truncated list length"
    );
    assert_eq!(reported.cap.kept, MAX_REST_REGIONS);
    assert_eq!(reported.cap.cap, MAX_REST_REGIONS);
    assert!(
        reported.cap.truncated(),
        "101 islands against a cap of {MAX_REST_REGIONS} is a truncation"
    );
    assert_eq!(reported.cap.dropped(), 101 - MAX_REST_REGIONS);

    // (b) unchanged behaviour: same count, same order, same areas — the cap
    // policy (largest by area first) is not what F3 touches.
    assert_eq!(plain.len(), MAX_REST_REGIONS);
    assert_eq!(plain.len(), reported.polygons.len());
    for (i, (a, b)) in plain.iter().zip(reported.polygons.iter()).enumerate() {
        assert!(
            (a.area() - b.area()).abs() < 1e-9,
            "region {i} differs between the plain and reported calls"
        );
    }
    let max_area = reported
        .polygons
        .iter()
        .map(Polygon2::area)
        .fold(0.0, f64::max);
    assert!(
        max_area > 100.0,
        "the 15x15 giant must still survive the cut: max kept area = {max_area}"
    );
}

/// (c) A healthy set is MEASURED clean, not silent. Distinguishing "5
/// islands, all kept" from "5 of 500" is the whole point of the channel.
#[test]
fn a_healthy_set_reports_measured_clean_rather_than_nothing() {
    let mask = island_storm(4); // 4 small + 1 giant
    let reported = region_polygons_from_mask_reported(&mask, 0.0, 0.0, 1.0, 0.0, None);

    assert_eq!(reported.cap.total_before_cap, 5);
    assert_eq!(reported.cap.kept, 5);
    assert!(!reported.cap.truncated());
    assert_eq!(reported.cap.dropped(), 0);
    assert_eq!(reported.polygons.len(), 5);
}

/// (d) The report reaches an operator surface, and carries the number an
/// operator would act on: how many islands there really were.
#[test]
fn a_truncated_extraction_renders_a_diagnostic_carrying_the_true_count() {
    let stats = ToolpathStats {
        region_cap: Some(RegionCapReport {
            total_before_cap: 312,
            kept: MAX_REST_REGIONS,
            cap: MAX_REST_REGIONS,
        }),
        ..ToolpathStats::default()
    };
    let out = diagnostics_from_generation(ToolpathId(7), &stats);
    let d = out
        .iter()
        .find(|d| d.id == DiagnosticId::from(ids::GEOM_REGION_CAP_TRUNCATED))
        .expect("a truncated extraction must surface a diagnostic");
    assert_eq!(d.scope, Scope::Toolpath { id: ToolpathId(7) });
    assert!(
        d.message.contains("312"),
        "the pre-cap count is the actionable number: {}",
        d.message
    );
    assert!(
        d.message.contains("248"),
        "the dropped count must be stated, not left to arithmetic: {}",
        d.message
    );
}

/// (d, negative arm): the two quiet states. A measured-clean extraction is
/// not a notice, and an operation that ran no extraction at all must not be
/// reported as clean — the X-19 rule, applied to this channel.
#[test]
fn a_clean_or_unmeasured_extraction_says_nothing() {
    let clean = ToolpathStats {
        region_cap: Some(RegionCapReport {
            total_before_cap: 9,
            kept: 9,
            cap: MAX_REST_REGIONS,
        }),
        ..ToolpathStats::default()
    };
    assert!(
        !diagnostics_from_generation(ToolpathId(1), &clean)
            .iter()
            .any(|d| d.id == DiagnosticId::from(ids::GEOM_REGION_CAP_TRUNCATED)),
        "a notice on every healthy rest pass is a notice nobody reads"
    );

    let unmeasured = ToolpathStats::default();
    assert_eq!(
        unmeasured.region_cap, None,
        "the default must read as NOT MEASURED, never as a clean extraction"
    );
    assert!(
        diagnostics_from_generation(ToolpathId(1), &unmeasured).is_empty(),
        "an absent measurement is not a claim"
    );
}
