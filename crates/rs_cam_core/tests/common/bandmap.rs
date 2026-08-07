//! The finish-planner band-territory map + deviation-histogram instrument,
//! extracted from the now-archived `p2c_headless_ab_wanaka.rs` and
//! `v3_cascade_ab.rs` mega harnesses (Checkpoint E, 2026-08-05,
//! `planning/review_2026-08-04/MEGA_HARNESS_POLICY.md` §2/§4; archived
//! copies live at `planning/archive/mega_harnesses_2026-08-05/`).
//!
//! **This is the stock-mesh-vertex-deviation + `BandMap` instrument
//! lineage (v3/p2c).** It is deliberately kept separate from the
//! `SimulationResult::column_deviations`-pointwise-COLUMNS instrument
//! lineage `strategy_comparison_h4.rs` / `classification_columns_ab_m3.rs`
//! use. Per §4 of the policy: the two measure different things, and the
//! v3 campaign's own closing lesson was that conflating instruments is
//! how a gate ranks a 28 mm uncut block first. Do not merge them into one
//! module.
//!
//! **BIT-IDENTITY.** Everything below is copied verbatim from ONE donor,
//! **`p2c_headless_ab_wanaka.rs`** (pre-archive line numbers cited per
//! item), with only the changes required to make it a shared module —
//! visibility, `use` paths, and (for [`build_band_map`] only) the two
//! named, documented deviations below. `v3_cascade_ab.rs` carried
//! byte-for-byte identical copies of the `BandMap` struct/`code_at`
//! (`:1087-1109`), the DEV_EDGES/labels/`BandAcc` block (`:1269-1287`),
//! `band_shares` (`:1452-1491`) and `outer_region_spans` (`:1616-1637`) —
//! all verified by direct diff against the p2c donor, not just
//! read-through.
//!
//! # `build_band_map`'s two donor deltas
//!
//! v3's comment at its own copy (`:1073-1082`) names exactly two intended
//! differences from the p2c original: the output directory the caller
//! writes PNGs/dumps under (`target/v3_scaled/` vs `target/p2f_fidelity/`
//! — see [`ensure_target_subdir`]), and the classification tool radius
//! (`FinishPlannerParams::for_tool(0.5)` vs p2c's `for_tool(3.0)`, driving
//! a `BallEndmill` diameter of `1.0` vs `6.0` — always exactly
//! `2 * tool_radius` in both donors). [`build_band_map`] takes
//! `tool_radius: f64` and derives the ball diameter from it, so both
//! donor call sites become `build_band_map(s, 3.0)` (p2c) and
//! `build_band_map(s, 0.5)` (v3).
//!
//! **What did NOT make it into this shared function**, because it is
//! genuinely NOT shared (only in the un-extracted, byte-verified-identical
//! sense — v3's copy had already diverged past that point): v3's own
//! `build_band_map` (`:1140-1265`) appends an LH-4 coverage-accounting
//! block (`:1208-1255`, using `rs_cam_core::measurement::*` provenance
//! types) between the shared rasterization loop and its own `BandMap`
//! return, which p2c's version never had. That block stays behind in the
//! archived `v3_cascade_ab.rs.archived` — it was never a duplicate to
//! begin with, so extracting it here would be inventing a shared
//! function that never existed.
//!
//! One more cosmetic (non-arithmetic) deviation: the donor's
//! `.expect("wanaka terrain mesh")` becomes `.expect("terrain mesh")`
//! here, since a generic helper cannot know which fixture (wanaka,
//! v3-scaled-wanaka, or a future third caller) it is running against.

#![allow(dead_code)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use std::path::PathBuf;

use rs_cam_core::session::ProjectSession;
use rs_cam_core::toolpath_spans::{Span, SpanKind};

/// Resolve (and create) `target/<subdir>/` under the workspace root. The
/// donors' two copies of this pattern (`p2c_headless_ab_wanaka.rs`'s
/// `p2f_output_dir()` → `"p2f_fidelity"`, `v3_cascade_ab.rs`'s `v3_dir()`
/// → `"v3_scaled"`) differed only in the subdir name and in `.expect` vs
/// `.unwrap_or_else(|e| panic!(...))` error style; this takes the p2c
/// donor's `.expect` style (the crate's dominant idiom under the
/// `expect_used` allow already in force here) and the subdir as a
/// parameter.
pub fn ensure_target_subdir(subdir: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join(subdir);
    std::fs::create_dir_all(&dir).expect("create target subdir");
    dir
}

/// Per-cell band ownership rasterized from the planner's conditioned
/// regions onto the classification grid. Code 0 = no region (off-model or
/// unclassified), 1 = Shallow, 2 = MidSteep, 3 = VerySteep. Band-overlap
/// cells attribute to the STEEPER band (rasterized in ascending
/// steepness, later overwrites).
///
/// Donor: `p2c_headless_ab_wanaka.rs:255-278`.
pub struct BandMap {
    pub origin_x: f64,
    pub origin_y: f64,
    pub cell: f64,
    pub rows: usize,
    pub cols: usize,
    pub codes: Vec<u8>,
}

pub const BAND_NAMES: [&str; 4] = ["off-region", "shallow", "mid-steep", "very-steep"];

impl BandMap {
    pub fn code_at(&self, x: f64, y: f64) -> u8 {
        let col = ((x - self.origin_x) / self.cell).round();
        let row = ((y - self.origin_y) / self.cell).round();
        if col < 0.0 || row < 0.0 || col >= self.cols as f64 || row >= self.rows as f64 {
            return 0;
        }
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        let idx = row as usize * self.cols + col as usize;
        self.codes[idx]
    }
}

/// Build the band map with the classification + decomposition dials a
/// caller's finishing op uses (`BallEndmill` diameter `2 * tool_radius`,
/// `FinishPlannerParams::for_tool(tool_radius)`, `overlap_mm = 2.0`,
/// locked 45/75 thresholds), so deviations can be attributed to the
/// regions that op actually routed. Score every branch under comparison
/// against the SAME map (same `tool_radius`) — the comparison question is
/// "what did each strategy's territory look like", so the territory
/// definition must be identical across branches.
///
/// Donor: `p2c_headless_ab_wanaka.rs:295-368` (shared rasterization body
/// verified byte-identical against `v3_cascade_ab.rs:1140-1207` — see the
/// module doc for what v3 appended past that point and why it is not
/// here).
pub fn build_band_map(s: &ProjectSession, tool_radius: f64) -> BandMap {
    use rs_cam_core::finish_planner::{FinishBand, FinishPlannerParams, decompose};
    use rs_cam_core::finish_setup::build_classification_surface_with_cancel;
    use rs_cam_core::geo::P2;
    use rs_cam_core::mesh::SpatialIndex;
    use rs_cam_core::region_set::RegionSet;
    use rs_cam_core::tool::BallEndmill;

    let mesh = s
        .models()
        .iter()
        .find_map(|m| m.mesh.clone())
        .expect("terrain mesh");
    let index = SpatialIndex::build(&mesh, 10.0);
    let cutter = BallEndmill::new(2.0 * tool_radius, 25.0);
    let never = || false;
    let surface = build_classification_surface_with_cancel(&mesh, &index, &cutter, 0.05, &never)
        .expect("classification surface");
    let mut planner = FinishPlannerParams::for_tool(tool_radius);
    planner.overlap_mm = 2.0;
    let planned = decompose(
        &surface.slope_map,
        surface.heightmap.covered_flags(),
        &[],
        &planner,
    );

    let hm = &surface.heightmap;
    let (rows, cols, cell) = (hm.rows, hm.cols, hm.cell_size);
    let mut codes = vec![0u8; rows * cols];
    for (band, code) in [
        (FinishBand::Shallow, 1u8),
        (FinishBand::MidSteep, 2u8),
        (FinishBand::VerySteep, 3u8),
    ] {
        let polys: Vec<_> = planned
            .regions
            .iter()
            .filter(|r| r.band == band)
            .map(|r| r.polygon.clone())
            .collect();
        if polys.is_empty() {
            continue;
        }
        let rs = RegionSet::new(polys);
        for r in 0..rows {
            for c in 0..cols {
                let x = hm.origin_x + c as f64 * cell;
                let y = hm.origin_y + r as f64 * cell;
                if rs.contains(&P2::new(x, y)) {
                    codes[r * cols + c] = code;
                }
            }
        }
    }

    let counts = codes.iter().fold([0usize; 4], |mut acc, &c| {
        acc[c as usize] += 1;
        acc
    });
    eprintln!(
        "BAND MAP {rows}x{cols} @ {cell:.3}mm: off-region={} shallow={} mid-steep={} very-steep={}",
        counts[0], counts[1], counts[2], counts[3]
    );

    BandMap {
        origin_x: hm.origin_x,
        origin_y: hm.origin_y,
        cell,
        rows,
        cols,
        codes,
    }
}

/// Save the band map itself as a PNG (top-down; gray = off-region, green =
/// shallow, orange = mid-steep, red = very-steep) so deviation maps can be
/// read against the territory that produced them.
///
/// Donor: `p2c_headless_ab_wanaka.rs:373-402` (v3 had no equivalent — it
/// never rendered its own territory map, only the deviation PNG in its
/// own `fidelity_report`, which is NOT extracted here; see "what did not
/// make it" below).
pub fn save_band_map_png(bm: &BandMap, path: &std::path::Path) {
    let mut px = vec![0u8; bm.rows * bm.cols * 4];
    for r in 0..bm.rows {
        for c in 0..bm.cols {
            let code = bm.codes[r * bm.cols + c];
            let (rr, gg, bb) = match code {
                1 => (40u8, 140u8, 60u8),
                2 => (220u8, 140u8, 30u8),
                3 => (200u8, 40u8, 40u8),
                _ => (60u8, 60u8, 60u8),
            };
            // Image row 0 is the TOP of the picture; grid row 0 is min-Y.
            let ir = bm.rows - 1 - r;
            let i = (ir * bm.cols + c) * 4;
            px[i] = rr;
            px[i + 1] = gg;
            px[i + 2] = bb;
            px[i + 3] = 255;
        }
    }
    image::save_buffer(
        path,
        &px,
        bm.cols as u32,
        bm.rows as u32,
        image::ColorType::Rgba8,
    )
    .expect("save band map png");
    eprintln!("band map png: {}", path.display());
}

/// Histogram bin edges (mm). Deviations below the first edge land in bin
/// 0; at or above the last edge in the final bin. Negative = OVERCUT
/// (beheaded detail), positive = leftover material.
///
/// Donor: `p2c_headless_ab_wanaka.rs:407-425` (`DEV_EDGES` /
/// `DEV_BIN_COUNT` / `DEV_BIN_LABELS` / `BandAcc`), byte-identical to
/// `v3_cascade_ab.rs:1269-1287`.
pub const DEV_EDGES: [f32; 12] = [
    -0.5, -0.3, -0.2, -0.1, -0.05, -0.01, 0.01, 0.05, 0.1, 0.2, 0.3, 0.5,
];
pub const DEV_BIN_COUNT: usize = DEV_EDGES.len() + 1;
pub const DEV_BIN_LABELS: [&str; DEV_BIN_COUNT] = [
    "<-.5", "-.5", "-.3", "-.2", "-.1", "-.05", "on-size", "+.05", "+.1", "+.2", "+.3", "+.5",
    ">+.5",
];

#[derive(Default, Clone, Copy)]
pub struct BandAcc {
    pub bins: [usize; DEV_BIN_COUNT],
    pub leftover_n: usize,
    pub leftover_sum: f64,
    pub leftover_max: f32,
    pub overcut_n: usize,
    pub overcut_sum: f64,
    pub overcut_min: f32,
}

/// Group-filtered mid-steep on-size / `+.05` shares from
/// `sim.column_deviations` — indexes into `DEV_EDGES`/`DEV_BIN_LABELS`
/// (index 6 = "on-size", index 7 = "+.05"), restricted to `group` (a
/// finish op's own setup group — multi-setup projects sample the same
/// world XY once per group, see `ColumnDeviation`'s own doc) and to cells
/// whose `bm.code_at` equals `band_code`. Returns
/// `(sample_count, on_size_pct, plus05_pct, tail_count)` where `tail_count`
/// is the count in the final (`">+.5"`, big standing leftover) bin.
///
/// Donor: `p2c_headless_ab_wanaka.rs:3659-3698`, byte-identical to
/// `v3_cascade_ab.rs:1452-1491`.
pub fn band_shares(
    s: &ProjectSession,
    bm: &BandMap,
    group: usize,
    band_code: u8,
) -> (usize, f64, f64, usize) {
    let sim = s.simulation_result().expect("sim result");
    let cols = sim
        .column_deviations
        .as_ref()
        .expect("column deviations (sim ran without a reference model mesh?)");
    let bin_of = |d: f32| -> usize {
        DEV_EDGES
            .iter()
            .position(|&e| d < e)
            .unwrap_or(DEV_BIN_COUNT - 1)
    };
    const ON_SIZE_BIN: usize = 6;
    const PLUS_05_BIN: usize = 7;
    const TAIL_BIN: usize = DEV_BIN_COUNT - 1; // ">+.5" — big standing leftover
    let mut total = 0usize;
    let mut on_size = 0usize;
    let mut plus05 = 0usize;
    let mut tail = 0usize;
    for cd in cols.iter().filter(|cd| cd.group == group) {
        if bm.code_at(cd.x, cd.y) != band_code {
            continue;
        }
        total += 1;
        match bin_of(cd.dev) {
            ON_SIZE_BIN => on_size += 1,
            PLUS_05_BIN => plus05 += 1,
            TAIL_BIN => tail += 1,
            _ => {}
        }
    }
    let pct = |n: usize| 100.0 * n as f64 / (total.max(1) as f64);
    (total, pct(on_size), pct(plus05), tail)
}

/// The outermost (node-level) `Region` spans in `spans` — those NOT fully
/// contained inside another `Region` span's `[start_move, end_move]`
/// range. Used to assert a finish op carries at least one node-level
/// Region span (design-doc §2.4's MUST).
///
/// Donor: `p2c_headless_ab_wanaka.rs:3630-3651`, byte-identical to
/// `v3_cascade_ab.rs:1616-1637`.
pub fn outer_region_spans(spans: &[Span]) -> Vec<&Span> {
    let regions: Vec<&Span> = spans
        .iter()
        .filter(|s| s.kind == SpanKind::Region)
        .collect();
    regions
        .iter()
        .copied()
        .filter(|s| {
            !regions.iter().any(|other| {
                !std::ptr::eq(*other, *s)
                    && other.start_move <= s.start_move
                    && other.end_move >= s.end_move
                    && (other.start_move, other.end_move) != (s.start_move, s.end_move)
            })
        })
        .collect()
}
