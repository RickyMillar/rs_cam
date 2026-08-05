//! H4 — re-earning the strategy-comparison campaign the broken instruments
//! voided.
//!
//! # Why this exists
//!
//! Wave 15 (H4) of `planning/review_2026-07-29/TECH_DEBT_RESEARCH_AND_FIX_PLAN.md`.
//! Four stacked instrument defects were fixed by earlier waves on this
//! branch: the classification grid was 6× too coarse, the finish-planner
//! dials were 6×/36× too large, rest-routing radius read the SHAFT instead
//! of the TIP, and `claims_reference` defaulted to the analytic `self_probe`
//! instead of the machined-stock reference (A/M6). Every one of those
//! defects sat directly underneath a shipped strategy-comparison campaign,
//! which means its conclusions — "scallop wins every time", "contour and
//! pencil have nothing to do on this part", the mm²/s efficiency table
//! reading "Op B (unified rest) 0.476 vs D (all-over) 0.938" — were measured
//! through instruments now known to have been wrong. This harness
//! re-measures them on the corrected instruments. It does **not** re-assert
//! them: see "Verdict discipline" below.
//!
//! # Why this harness builds its own fixtures
//!
//! It does **not** load `planning/airrun_2026-06-01/wanaka.toml` — that file
//! is a live, user-modified project; reading it as a dependency makes a gate
//! whose verdict changes when somebody drags a slider (`classification_
//! columns_ab_m3.rs`'s module doc names the same reason). Fixture 1 is the
//! same committed `terrain.stl` M3 uses. Fixture 2 is new to this wave — see
//! below.
//!
//! # The two fixtures, and why both are mandatory
//!
//! **Fixture 1 (`terrain.stl`, `H4_WINDOW_MM` window)** is a convex,
//! naturally-varying heightfield. It has real band mix, but it cannot by
//! itself adjudicate anything about *concave* rest-routing behaviour — the
//! defect class (radius = shaft, not tip) that this programme's H-series
//! fixed lives specifically in re-entrant/pocketed territory a convex
//! terrain never presents.
//!
//! **Fixture 2 (`grooved_block`)** is a straight trapezoidal groove cut into
//! a flat block: flat rim (0°), straight walls at a chosen angle, flat
//! floor. Picked at `wall_deg` ≈ 70° so the wall sits BETWEEN the shipped
//! 45°/75° thresholds — cleanly mid-steep scallop territory — while rim and
//! floor sit cleanly in shallow-raster territory below 45°. That gives two
//! **homogeneous, cleanly separable** bands by construction, rather than the
//! continuously-varying slope terrain.stl presents. The acceptance gate for
//! this wave is explicit that "a convex fixture cannot adjudicate concave
//! defects" — this is the concave half.
//!
//! # Arms
//!
//! - **D — all-over Scallop.** `OperationConfig::Scallop`, one op, whole
//!   surface. The claimed universal winner.
//! - **B — UnifiedFinish band-mix.** `OperationConfig::UnifiedFinish`, one
//!   op, the SHIPPED `classification_sampler` default (not pinned to
//!   `DropCutterProbe`). The band-mix / strategy-mix arm.
//! - **C — cascade.** Two `UnifiedFinish` ops in one session: an all-over
//!   pass, then a second pass reading `StockSource::FromRemainingStock` with
//!   the claims pipeline's SHIPPED `Auto` reference (never hard-pinned to
//!   `self_probe` — that is exactly the A/M6 footgun). Mandatory ordering
//!   (`claims_reference_cascade_am6.rs`'s `run_cascade`): generate(op0) →
//!   simulate → generate(op1) → simulate. A rest op generated without an
//!   intervening simulation is rest-BLIND; this file asserts that did not
//!   happen (op1 non-empty, and its cutting distance materially less than
//!   op0's) and fails loudly, naming the mechanism, if it did.
//!
//! All three arms share the same tool (the project's own Ø1-tip / 7° /
//! Ø6-shaft taper — `envelope_radius_mm()` 3.0 mm, `cusp_radius_mm()`
//! 0.5 mm), the same stock, and the same measurement resolution, so the only
//! variable between them is the STRATEGY.
//!
//! # Metrics — every arm reports all of these
//!
//! 1. **COLUMNS quality** (`SimulationResult::column_deviations`), scored on
//!    the column population common to **all three arms** — never on an
//!    arm's own population (`classification_columns_ab_m3.rs`'s rule,
//!    extended from two arms to three). Indexed by `(row, col)`, never XY
//!    (`MEMORY.md`, P2.g Task 1). `!sim.resolution_clamped` is asserted for
//!    every arm.
//! 2. **Honest swept-footprint mm²/s** (`measurement::swept_footprint_area`
//!    / `swept_footprint_mm2_per_s`), radius = the cutter's
//!    `envelope_radius_mm()` (NOT the tip/cusp radius — the module doc is
//!    emphatic that a footprint is a swept-EXTENT measure). For the cascade
//!    arm, per-op figures are reported plus a whole-arm figure built from
//!    SUMMED area over SUMMED time — summing areas double-counts any ground
//!    both ops crossed, so the whole-arm cascade figure is an UPPER BOUND on
//!    mm²/s, stated as such at the print site.
//! 3. **Retract trip counts** (`ToolpathStats::retract_trip_measurement`).
//!    `in_node`/`between_nodes` are `None` unless `spans_valid` — reported as
//!    `None`, never fabricated as zero.
//! 4. **Role-based mix table**: band × strategy × regions × moves × XY area
//!    (`classification_columns_ab_m3.rs`'s `region_mix`), PLUS a cutting-
//!    DISTANCE column this wave adds — move COUNT is not a cutting measure
//!    (a region's moves are a mix of rapids, links and cuts), and the
//!    strategy-mix claims this file re-measures were expressed as a share of
//!    CUTTING, so distance is the column that can actually answer them.
//! 5. Rapid collisions, generation wall seconds, per-op and whole-arm
//!    runtime seconds, `stats.truncated_core_mm2` (report "not measured"
//!    on `None`, never fabricate `0.0` — A/M9's X-19 rule).
//!
//! # Verdict discipline — the point of this wave
//!
//! **This harness does not assert a winner.** A re-measurement that gates on
//! its own preferred outcome cannot re-earn a verdict — it just launders the
//! old one through new instruments. Its assertions are SAFETY and
//! NON-VACUITY only:
//!
//! - rapid collisions must not exceed a stated bound (see the pre-registered
//!   thresholds below);
//! - every arm must produce a non-empty toolpath;
//! - the common column population (all three arms) must be non-empty;
//! - the terrain fixture's Arm B mix must contain ≥ 2 band/strategy
//!   combinations (otherwise the classifier had nothing to decide and the
//!   comparison is vacuous by construction — the same rule
//!   `classification_columns_ab_m3.rs` applies to its own fixture);
//! - the cascade's rest op must be non-blind (see Arm C above).
//!
//! The speed/quality COMPARISON across arms is printed as a table for a
//! human to read. **Render before verdicts**: every arm's deviation map and
//! the pairwise differences go to `target/h4_strategy/` before any assertion
//! runs — the standing rule from the v3 campaign's closure is "never gate on
//! an aggregate without rendering the surface"; that gate once ranked an
//! operation above one that had left a 28 mm block of material standing.
//!
//! # Measured, wave 15, 2026-08-04 — DEBUG build, `H4_SIM_MM` 0.1
//!
//! Full write-up, renders and the void claims each of these bears on:
//! `planning/review_2026-07-29/SUPERSEDED_CONCLUSIONS.md` §3.
//!
//! `grooved_block(2.5, 70°, 1.2)` — 96 641 columns common to all three arms:
//!
//! | arm | runtime s | mm²/s | trips | p50 µm | p90 µm | ±25 µm | worst overcut µm |
//! |---|---|---|---|---|---|---|---|
//! | D scallop | 105.1 | 13.3802 | 2 | 0.0 | 1.7 | 97.65% | −34.1 |
//! | B unified | 96.9 | 14.6296 | 10 | 0.0 | 0.2 | 96.93% | −235.0 |
//! | C cascade | 140.8 | 17.6511 † | 58 | 0.0 | 0.2 | 96.93% | −235.0 |
//!
//! `terrain.stl` @ 20 mm — 38 770 columns common to all three arms:
//!
//! | arm | runtime s | mm²/s | trips | p50 µm | p90 µm | ±25 µm | worst overcut µm |
//! |---|---|---|---|---|---|---|---|
//! | D scallop | 462.1 | 2.2201 | 13 | 67.4 | 291.1 | 26.26% | −1654.3 |
//! | B unified | 157.3 | 4.6982 | 34 | 62.5 | 265.6 | 25.33% | −528.4 |
//! | C cascade | 232.8 | 6.1866 † | 385 | 61.4 | 261.0 | 25.75% | −528.4 |
//!
//! † whole-arm cascade footprint is an UPPER BOUND (summed areas
//! double-count overlap). Zero rapid collisions and no resolution clamping
//! on every arm of both fixtures.
//!
//! Four things worth stating rather than leaving in the table:
//!
//! * **Waterline takes 41.4% of terrain's cutting**, in one coherent
//!   VerySteep region. The campaign this harness re-measures reported
//!   waterline at **0.0%** and concluded "the fixture has no very-steep
//!   band". It has one.
//! * **Neither arm "wins every time"** on either fixture, in either
//!   direction.
//! * **The terrain fixture cannot adjudicate fine quality.** Every arm
//!   misses the 22.5 µm cusp the dials ask for by ~3× at p50, and the arms
//!   differ from each other in unstructured speckle. Read terrain's *time*,
//!   *mix* and *trip* columns; do not read its µm columns as a verdict.
//! * **The rest pass removed nothing at all on the groove** (its difference
//!   map is uniformly zero over 96 641 columns) and **real material on
//!   terrain**. Both results are honest; running one fixture would have
//!   produced a confident wrong answer either way.
//!
//! ```text
//! cargo test -p rs_cam_core --test strategy_comparison_h4 --release \
//!     -- --ignored --nocapture --test-threads=1
//! ```
//!
//! Knobs: `H4_WINDOW_MM` (mm, default 20 — smaller than M3's 30: this
//! harness runs THREE-to-FOUR generations plus their simulations per
//! fixture, in a DEBUG build, not release, so the default must stay cheap),
//! `H4_SIM_MM` (mm, default 0.1 — the tip-matched measurement grid; a Ø1 tip
//! has a 0.5 mm tip radius, and `feedback_rest_measurement_prerequisites`
//! requires the sim cell to sit well under the TIP radius).
//!
//! # SCHEDULED (Checkpoint E, 2026-08-05, Q3/E6)
//!
//! Ruling: `planning/review_2026-08-04/MEGA_HARNESS_POLICY.md` §5, §7 E6;
//! `planning/review_2026-07-29/ORCHESTRATION_LOG.md:23`. Unlike its two
//! siblings (`p2c_headless_ab_wanaka.rs`, `v3_cascade_ab.rs` — both
//! ARCHIVED the same day, see `tests/ARCHIVED_HARNESSES.md`), this harness
//! is **SCHEDULED**, not archived: it reads no mutable file, hard-codes no
//! machine-local path, and asserts no winner.
//!
//! - **Owner**: the finishing/quality lane (W8).
//! - **Cadence**: on demand, before any strategy-comparison question is
//!   asked in that lane — PLUS its non-ignored CI sentry on every run (see
//!   below). Not a periodic re-run schedule; the ignored arms are
//!   expensive (several generations + simulations per fixture) and are
//!   meant to be invoked deliberately, not swept up by a blanket `cargo
//!   test`.
//! - **`h4_harness_arithmetic_sentry`** (line 1368 as of this edit —
//!   verified by reading this file, not preceded by `#[ignore]`) **stays
//!   in the normal `cargo test -p rs_cam_core` suite.** This is
//!   deliberate, not an oversight to fix: it is the cheap arithmetic
//!   guard (runs against `tiny_groove`, no full fixture, no `--ignored`)
//!   that keeps the expensive ignored arms honest between the on-demand
//!   runs above. (The original Checkpoint E census cited "~line 1322" —
//!   that was correct pre-edit; this SCHEDULED block and the `quantile`
//!   import fix below it shifted line numbers, so it is re-quoted here
//!   at its current value rather than left to drift silently.)
//!
//! **Known debt, recorded and left unfixed except where noted:**
//!
//! - The fifth copy of the `out_dir()` pattern (line 901 as of this edit)
//!   — a `common::out_dir(name)` helper does not exist yet.
//! - ~138 lines shared byte-identically with
//!   `classification_columns_ab_m3.rs` (`render()`, the `terrain()` crop,
//!   …) — not yet extracted to `common/columns.rs`.
//!   `checkpoint_b_resolution_ab.rs` carries its own third copy of at
//!   least part of this and would also want it.
//! - **FIXED, 2026-08-05, as the one permitted exception**: the local
//!   `quantile` was a byte-identical duplicate of
//!   `common::scallop_oracle::quantile` despite this file already having
//!   `mod common;` — it now imports the shared one instead. Recorded here
//!   for the record, not as something still open.
//!
//! Per the C6 migration policy (`tests/common/mod.rs`), the remaining
//! debt above is intentionally left in place: **opportunistic, never in
//! bulk.** Move it when this file is next edited for a substantive
//! reason, not as a standalone tidy-up.
//!
//! **Caveat on the result tables above** (§ "Measured, wave 15,
//! 2026-08-04"): those numbers are prose in a test file and will rot the
//! same way the archived harnesses' pinned constants did, if nobody
//! re-runs them. The on-demand cadence above exists specifically to stop
//! that — re-run before citing this table, don't assume it still holds.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

mod common;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{HeightsConfig, RetractTripCount, StockSource};
use rs_cam_core::compute::operation_configs::{ScallopConfig, UnifiedFinishConfig};
use rs_cam_core::compute::simulate::ColumnDeviation;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::geo::P3;
use rs_cam_core::measurement::{
    DEFAULT_FOOTPRINT_CELL_MM, MeasurementProvenance, ProjectedXyAreaMm2, swept_footprint_area,
    swept_footprint_mm2_per_s,
};
use rs_cam_core::mesh::TriangleMesh;
use rs_cam_core::semantic_trace::{SemanticKey, ToolpathSemanticKind, ToolpathSemanticTrace};
use rs_cam_core::session::{LoadedModel, ProjectSession, SimulationOptions};
use rs_cam_core::tool::MillingCutter;
use rs_cam_core::toolpath::Move;

use common::meshes::grooved_block;
use common::scallop_oracle::quantile;
use common::session::{mesh_model, pinned_heights, single_op_session_with, toolpath_config};
use common::tools::tapered_cutter;

// ── Shared tool ─────────────────────────────────────────────────────────
//
// One tool for every arm on every fixture: the project's own Ø1-tip / 7° /
// Ø6-shaft taper. `envelope_radius_mm()` = 3.0 mm (the shaft), `cusp_radius_
// mm()` = 0.5 mm (the tip) — the 6× split H4's own programme exists because
// of (rest-routing radius read the shaft where it should have read the tip).
// Holding the tool fixed across arms is what makes this a strategy
// comparison rather than a tool comparison.
const TIP_DIAMETER_MM: f64 = 1.0;
const TAPER_HALF_ANGLE_DEG: f64 = 7.0;
const SHANK_DIAMETER_MM: f64 = 6.0;

fn session_tool() -> ToolConfig {
    ToolConfig {
        diameter: TIP_DIAMETER_MM,
        taper_half_angle: TAPER_HALF_ANGLE_DEG,
        shaft_diameter: SHANK_DIAMETER_MM,
        ..ToolConfig::new_default(ToolId(0), ToolType::TaperedBallNose)
    }
}

/// `envelope_radius_mm()` for the shared tool — the footprint radius per the
/// `swept_footprint_area` doc ("the cutter's XY envelope, NOT the tip/cusp
/// radius").
fn envelope_radius_mm() -> f64 {
    tapered_cutter(TIP_DIAMETER_MM, TAPER_HALF_ANGLE_DEG, SHANK_DIAMETER_MM).envelope_radius_mm()
}

/// `0.3² / (8 · 0.5)`: the flat-surface cusp a 0.3 mm stepover leaves on this
/// tip's 0.5 mm tip radius. Spelled as arithmetic (copied from
/// `classification_columns_ab_m3.rs::ab_config`) so the parity is checkable,
/// and used as the finish-quality dial for every finishing arm so a
/// difference between arms is not also a quality-target difference.
fn cusp_mm() -> f64 {
    0.3 * 0.3 / (8.0 * 0.5 * TIP_DIAMETER_MM)
}

fn env_f64(key: &str, fallback: f64) -> f64 {
    match std::env::var(key) {
        Ok(raw) => raw
            .parse()
            .unwrap_or_else(|e| panic!("{key} must be a number, got {raw:?}: {e}")),
        Err(_) => fallback,
    }
}

// ── Fixture 1: terrain.stl ─────────────────────────────────────────────

const DEFAULT_WINDOW_MM: f64 = 20.0;
const DEFAULT_SIM_MM: f64 = 0.1;

fn terrain_path() -> PathBuf {
    let path = common::repo_root().join("crates/rs_cam_core/tests/fixtures/terrain.stl");
    assert!(
        path.exists(),
        "the H4 strategy comparison needs the committed terrain fixture at {}",
        path.display()
    );
    path
}

/// Load terrain.stl and, when `window_mm > 0`, crop it to a centred
/// `window_mm` square. A triangle is kept only when all three vertices fall
/// inside the window (copied verbatim from `classification_columns_ab_m3
/// .rs::terrain` — same crop rule, same reason: no partial facet at the
/// boundary).
fn terrain(window_mm: f64) -> TriangleMesh {
    let full = TriangleMesh::from_stl(&terrain_path()).expect("load terrain.stl");
    if window_mm <= 0.0 {
        return full;
    }
    let cx = 0.5 * (full.bbox.min.x + full.bbox.max.x);
    let cy = 0.5 * (full.bbox.min.y + full.bbox.max.y);
    let half = 0.5 * window_mm;
    let inside = |p: &P3| (p.x - cx).abs() <= half && (p.y - cy).abs() <= half;

    let mut remap: HashMap<u32, u32> = HashMap::new();
    let mut vertices: Vec<P3> = Vec::new();
    let mut triangles: Vec<[u32; 3]> = Vec::new();
    for tri in &full.triangles {
        if !tri.iter().all(|&i| inside(&full.vertices[i as usize])) {
            continue;
        }
        let mut out = [0u32; 3];
        for (slot, &src) in out.iter_mut().zip(tri.iter()) {
            *slot = *remap.entry(src).or_insert_with(|| {
                vertices.push(full.vertices[src as usize]);
                (vertices.len() - 1) as u32
            });
        }
        triangles.push(out);
    }
    assert!(
        triangles.len() > 500,
        "the {window_mm} mm crop kept only {} triangle(s) — window too small for the terrain's \
         facet spacing",
        triangles.len()
    );
    TriangleMesh::from_raw(vertices, triangles)
}

fn stock_for_terrain(mesh: &TriangleMesh) -> StockConfig {
    let b = &mesh.bbox;
    StockConfig {
        x: (b.max.x - b.min.x) + 4.0,
        y: (b.max.y - b.min.y) + 4.0,
        z: b.max.z - b.min.z,
        origin_x: b.min.x - 2.0,
        origin_y: b.min.y - 2.0,
        origin_z: b.min.z,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

// ── Fixture 2: grooved_block ────────────────────────────────────────────
//
// `common::meshes::grooved_block(rim_half_width, wall_deg, depth)`: flat rim
// at z=0, straight walls at `wall_deg` from horizontal, flat floor at
// `z = -depth`. `wall_deg = 70` sits between the shipped 45°/75° thresholds
// (mid-steep scallop territory); rim and floor sit at 0° (shallow raster
// territory). Parameters copied verbatim from the already-exercised
// `grooved_block(2.5, 70.0, 1.2)` fixture (`coverage_routing_pr5.rs`,
// `common_fixtures_smoke_c6.rs`) rather than invented, so this is a known
// mesh, not a novel one. `GroovedBlock`'s fixed footprint is `x ∈ [-20, 20]`,
// `y ∈ [-12, 12]` regardless of `rim_half_width` (no builder setter for
// those extents), so the stock below is sized to that footprint, not to
// `rim_half_width`.
const GROOVE_RIM_HALF_WIDTH_MM: f64 = 2.5;
const GROOVE_WALL_DEG: f64 = 70.0;
const GROOVE_DEPTH_MM: f64 = 1.2;
const GROOVE_X_EXTENT_MM: f64 = 20.0;
const GROOVE_Y_HALF_MM: f64 = 12.0;

fn grooved_fixture() -> TriangleMesh {
    grooved_block(GROOVE_RIM_HALF_WIDTH_MM, GROOVE_WALL_DEG, GROOVE_DEPTH_MM)
}

fn stock_for_groove() -> StockConfig {
    StockConfig {
        x: 2.0 * GROOVE_X_EXTENT_MM + 4.0,
        y: 2.0 * GROOVE_Y_HALF_MM + 4.0,
        z: GROOVE_DEPTH_MM + 2.0,
        origin_x: -GROOVE_X_EXTENT_MM - 2.0,
        origin_y: -GROOVE_Y_HALF_MM - 2.0,
        origin_z: -(GROOVE_DEPTH_MM + 2.0),
        auto_from_model: false,
        ..StockConfig::default()
    }
}

// ── Operation dials ─────────────────────────────────────────────────────

/// Arm B / Arm C's shared UnifiedFinish dial set. `classification_sampler`
/// is left at the type default — the SHIPPED sampler — never pinned, per the
/// module doc.
fn unified_arm_config() -> UnifiedFinishConfig {
    UnifiedFinishConfig {
        steep_threshold_deg: 45.0,
        waterline_threshold_deg: 75.0,
        overlap_mm: 2.0,
        scallop_height: cusp_mm(),
        tolerance: 0.05,
        raster_stepover: 0.3,
        z_step: 0.3,
        sampling: 0.5,
        stock_to_leave: 0.0,
        feed_rate: 3000.0,
        plunge_rate: 150.0,
        spindle_rpm: Some(21000),
        pencil_claims: false,
        territory_clip: false,
        ..UnifiedFinishConfig::default()
    }
}

/// Arm C, op0: the all-over pass. Same dials as Arm B — the cascade's first
/// stage is not a different quality target, only a different pipeline
/// shape.
fn cascade_op0_config() -> UnifiedFinishConfig {
    unified_arm_config()
}

/// Arm C, op1: the rest pass. `claims_reference` is left at its type
/// default — `ClaimsReference::Auto`, the SHIPPED A/M6 default — never
/// hard-pinned to `self_probe`. `pencil_claims` and `territory_clip` turn on
/// the claims pipeline and the rest-territory confinement it drives.
fn cascade_op1_config() -> UnifiedFinishConfig {
    UnifiedFinishConfig {
        pencil_claims: true,
        territory_clip: true,
        ..unified_arm_config()
    }
}

/// Arm D: all-over Scallop, same feed/plunge/cusp target as Arm B so the
/// comparison is strategy-only, not also a quality-target difference.
fn scallop_arm_config() -> ScallopConfig {
    ScallopConfig {
        scallop_height: cusp_mm(),
        tolerance: 0.05,
        feed_rate: 3000.0,
        plunge_rate: 150.0,
        stock_to_leave: 0.0,
        spindle_rpm: Some(21000),
        ..ScallopConfig::default()
    }
}

// ── Distance-aware region mix (the M3 table plus a cutting-distance column) ─

#[derive(Debug, Clone)]
struct MixRow {
    band: String,
    strategy: String,
    regions: usize,
    moves: usize,
    area_mm2: f64,
    cutting_distance_mm: f64,
}

/// Euclidean distance between two toolpath targets (3D — the same measure
/// `geo::polyline_length` sums over a whole polyline, inlined here for a
/// single segment).
fn p3_distance(a: P3, b: P3) -> f64 {
    ((b.x - a.x).powi(2) + (b.y - a.y).powi(2) + (b.z - a.z).powi(2)).sqrt()
}

/// Cutting distance (mm) over `moves[start..=end]` (inclusive, the same
/// convention `ToolpathSemanticItem::move_end` uses): for each index in
/// range whose move is a CUTTING move, add the segment from the PREVIOUS
/// move's target (which may sit just outside the range — a region's first
/// move is swept from whatever preceded it, exactly as `swept_footprint_
/// area` sweeps every move from its predecessor). A move count is not a
/// cutting measure — `move_end - move_start + 1` mixes rapids, links and
/// cuts — which is why this file adds a distance column rather than reusing
/// the M3 harness's move count for the strategy-mix claims.
fn cutting_distance_over_range(moves: &[Move], start: usize, end: usize) -> f64 {
    if moves.is_empty() {
        return 0.0;
    }
    let hi = end.min(moves.len() - 1);
    let lo = start.max(1);
    let mut total = 0.0;
    for i in lo..=hi {
        let Some(cur) = moves.get(i) else { continue };
        if !cur.move_type.is_cutting() {
            continue;
        }
        let Some(prev) = moves.get(i - 1) else {
            continue;
        };
        total += p3_distance(prev.target, cur.target);
    }
    total
}

/// (band, strategy) → (regions, moves, XY area mm², cutting distance mm),
/// from the op's own semantic trace. Copied in shape from `classification_
/// columns_ab_m3.rs::region_mix`; the distance column is new (see `cutting_
/// distance_over_range`'s doc for why it's needed).
fn region_mix_with_distance(trace: Option<&ToolpathSemanticTrace>, moves: &[Move]) -> Vec<MixRow> {
    let Some(trace) = trace else {
        return Vec::new();
    };
    let mut acc: HashMap<(String, String), (usize, usize, f64, f64)> = HashMap::new();
    for item in trace
        .items
        .iter()
        .filter(|i| i.kind == ToolpathSemanticKind::Region)
    {
        let text = |key: SemanticKey| {
            item.params
                .get(key)
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_owned()
        };
        let area = item
            .params
            .get(SemanticKey::AreaMm2)
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0);
        let (mv, dist) = match item.move_start.zip(item.move_end) {
            Some((s, e)) => (
                e.saturating_sub(s) + 1,
                cutting_distance_over_range(moves, s, e),
            ),
            None => (0, 0.0),
        };
        let entry = acc
            .entry((text(SemanticKey::Band), text(SemanticKey::Strategy)))
            .or_default();
        entry.0 += 1;
        entry.1 += mv;
        entry.2 += area;
        entry.3 += dist;
    }
    let mut rows: Vec<MixRow> = acc
        .into_iter()
        .map(
            |((band, strategy), (regions, moves, area_mm2, cutting_distance_mm))| MixRow {
                band,
                strategy,
                regions,
                moves,
                area_mm2,
                cutting_distance_mm,
            },
        )
        .collect();
    rows.sort_by(|a, b| b.cutting_distance_mm.total_cmp(&a.cutting_distance_mm));
    rows
}

// ── COLUMNS scoring (indexed by (row, col), never XY — MEMORY.md) ──────────

#[derive(Debug, Clone, Copy)]
struct Columns {
    n: usize,
    p50_abs_um: f64,
    p90_abs_um: f64,
    max_abs_um: f64,
    on_size_10um: f64,
    on_size_25um: f64,
    worst_overcut_um: f64,
    worst_leftover_um: f64,
}

fn score(devs: &[f64]) -> Columns {
    let mut abs: Vec<f64> = devs.iter().map(|d| d.abs() * 1000.0).collect();
    abs.sort_by(f64::total_cmp);
    let n = devs.len();
    let within =
        |limit_um: f64| abs.iter().filter(|&&a| a <= limit_um).count() as f64 / n.max(1) as f64;
    Columns {
        n,
        p50_abs_um: quantile(&abs, 0.50),
        p90_abs_um: quantile(&abs, 0.90),
        max_abs_um: abs.last().copied().unwrap_or(f64::NAN),
        on_size_10um: within(10.0),
        on_size_25um: within(25.0),
        worst_overcut_um: devs.iter().copied().fold(0.0, f64::min) * 1000.0,
        worst_leftover_um: devs.iter().copied().fold(0.0, f64::max) * 1000.0,
    }
}

fn by_cell(cols: &[ColumnDeviation]) -> HashMap<(usize, usize), f64> {
    cols.iter()
        .map(|c| ((c.row, c.col), f64::from(c.dev)))
        .collect()
}

// ── Per-op and per-arm measurement ──────────────────────────────────────

#[derive(Debug, Clone)]
struct OpReport {
    label: String,
    gen_s: f64,
    runtime_s: f64,
    footprint_mm2: f64,
    footprint_provenance: MeasurementProvenance,
    mm2_per_s: f64,
    retract_trips: Option<RetractTripCount>,
    retract_provenance: Option<MeasurementProvenance>,
    mix: Vec<MixRow>,
    truncated_core_mm2: Option<f64>,
    cutting_distance_mm: f64,
    move_count: usize,
}

/// Measure one already-generated toolpath at `index`. `runtime_s` is
/// supplied by the caller (from `ProjectSession::diagnostics`, isolated per
/// op for a cascade — see `run_cascade_arm`).
fn measure_op(
    session: &ProjectSession,
    index: usize,
    label: &str,
    gen_s: f64,
    runtime_s: f64,
) -> OpReport {
    let result = session
        .get_result(index)
        .unwrap_or_else(|| panic!("{label}: no generated result at index {index}"));
    let moves = &result.toolpath().moves;

    let (footprint, footprint_provenance) =
        swept_footprint_area(moves, envelope_radius_mm(), DEFAULT_FOOTPRINT_CELL_MM);
    let mm2_per_s = swept_footprint_mm2_per_s(footprint, runtime_s);

    let (retract_trips, retract_provenance) = match result.stats.retract_trip_measurement() {
        Some((t, p)) => (Some(t), Some(p)),
        None => (None, None),
    };

    let mix = region_mix_with_distance(result.semantic_trace.as_ref(), moves);

    OpReport {
        label: label.to_owned(),
        gen_s,
        runtime_s,
        footprint_mm2: footprint.mm2(),
        footprint_provenance,
        mm2_per_s,
        retract_trips,
        retract_provenance,
        mix,
        truncated_core_mm2: result.stats.truncated_core_mm2,
        cutting_distance_mm: result.stats.cutting_distance,
        move_count: result.stats.move_count,
    }
}

#[derive(Debug, Clone)]
struct ArmResult {
    label: String,
    columns: Vec<ColumnDeviation>,
    collisions: usize,
    resolution_clamped: bool,
    ops: Vec<OpReport>,
    /// Whole-arm footprint (mm²). For the cascade arm this is `op0 + op1`
    /// and is an UPPER BOUND — see `whole_arm_footprint_is_upper_bound`.
    whole_arm_footprint_mm2: f64,
    whole_arm_time_s: f64,
    whole_arm_mm2_per_s: f64,
    whole_arm_footprint_is_upper_bound: bool,
    whole_trips_total: Option<usize>,
}

/// Arm D or Arm B: one op, one session.
fn run_single_op_arm(
    label: &str,
    stock: StockConfig,
    model: LoadedModel,
    op: OperationConfig,
    heights: HeightsConfig,
    sim_mm: f64,
) -> ArmResult {
    let mut session = single_op_session_with(stock, session_tool(), model, label, op, |cfg| {
        cfg.heights = heights;
    });
    let cancel = AtomicBool::new(false);

    let t0 = Instant::now();
    session
        .generate_toolpath(0, &cancel)
        .unwrap_or_else(|e| panic!("{label}: generation failed: {e:?}"));
    let gen_s = t0.elapsed().as_secs_f64();

    let opts = SimulationOptions {
        resolution: sim_mm,
        ..Default::default()
    };
    session
        .run_simulation(&opts, &cancel)
        .unwrap_or_else(|e| panic!("{label}: measurement simulation failed: {e:?}"));

    let sim = session.simulation_result().expect("simulation result");
    let resolution_clamped = sim.resolution_clamped;
    let collisions = sim.rapid_collisions.len();
    let columns = sim
        .column_deviations
        .as_ref()
        .unwrap_or_else(|| panic!("{label}: no column_deviations (no reference model mesh?)"))
        .clone();

    let runtime_s = session.diagnostics().total_runtime_s;
    let op_report = measure_op(&session, 0, label, gen_s, runtime_s);

    ArmResult {
        label: label.to_owned(),
        columns,
        collisions,
        resolution_clamped,
        whole_arm_footprint_mm2: op_report.footprint_mm2,
        whole_arm_time_s: runtime_s,
        whole_arm_mm2_per_s: op_report.mm2_per_s,
        whole_arm_footprint_is_upper_bound: false,
        whole_trips_total: op_report.retract_trips.map(|t| t.total),
        ops: vec![op_report],
    }
}

/// Arm C: two `UnifiedFinish` ops, mandatory ordering copied from
/// `claims_reference_cascade_am6.rs::run_cascade` — generate(op0) →
/// simulate → generate(op1) → simulate. The intervening simulation is what
/// gives op1 a machined-stock snapshot to reference; skipping it makes op1
/// rest-BLIND (`AwaitingPriorStock`'s whole point).
fn run_cascade_arm(
    label: &str,
    stock: StockConfig,
    model: LoadedModel,
    heights: HeightsConfig,
    sim_mm: f64,
) -> ArmResult {
    let mut session = ProjectSession::new_empty();
    session.set_stock_config(stock);
    let tool_idx = session.add_tool(session_tool());
    let tool_id = session.tools()[tool_idx].id.0;
    let model_id = session.add_model(model);

    let mut op0 = toolpath_config(
        &format!("{label} op0 (all-over)"),
        OperationConfig::UnifiedFinish(cascade_op0_config()),
        tool_id,
        model_id,
    );
    op0.heights = heights.clone();
    session.add_toolpath(0, op0).expect("add cascade op0");

    let mut op1 = toolpath_config(
        &format!("{label} op1 (rest)"),
        OperationConfig::UnifiedFinish(cascade_op1_config()),
        tool_id,
        model_id,
    );
    op1.heights = heights;
    // Without this op1 reads FRESH stock and there is no machined prior for
    // ANY claims reference to use — the cascade would not be a cascade.
    op1.stock_source = StockSource::FromRemainingStock;
    session.add_toolpath(0, op1).expect("add cascade op1");

    let cancel = AtomicBool::new(false);
    let opts = SimulationOptions {
        resolution: sim_mm,
        ..Default::default()
    };

    // op0: generate, then simulate. The simulation is what op1 will read as
    // its machined-stock prior.
    let t0 = Instant::now();
    session
        .generate_toolpath(0, &cancel)
        .unwrap_or_else(|e| panic!("{label}: op0 generation failed: {e:?}"));
    let gen0_s = t0.elapsed().as_secs_f64();
    session
        .run_simulation(&opts, &cancel)
        .unwrap_or_else(|e| panic!("{label}: op0 simulation failed: {e:?}"));

    // At this point only op0 has a result, so this diagnostics snapshot's
    // `total_runtime_s` is op0's runtime ALONE — the isolation trick this
    // file uses instead of reaching for `machine_kinematics::compute_cycle_
    // time` directly (`ProjectDiagnostics::total_runtime_s` already sums
    // per-toolpath kinematics-integrated time from the production path;
    // re-deriving it a second way would be the kind of unchecked-arithmetic
    // risk this wave exists to close).
    let op0_runtime_s = session.diagnostics().total_runtime_s;
    let op0_report = measure_op(&session, 0, &format!("{label} op0"), gen0_s, op0_runtime_s);

    // op1: generated AFTER op0's simulation snapshot exists — the mandatory
    // ordering. Then simulate again for the FINAL (post-both-ops) surface.
    let t1 = Instant::now();
    session
        .generate_toolpath(1, &cancel)
        .unwrap_or_else(|e| panic!("{label}: op1 generation failed: {e:?}"));
    let gen1_s = t1.elapsed().as_secs_f64();
    session
        .run_simulation(&opts, &cancel)
        .unwrap_or_else(|e| panic!("{label}: final simulation failed: {e:?}"));

    let whole_arm_time_s = session.diagnostics().total_runtime_s;
    let op1_runtime_s = (whole_arm_time_s - op0_runtime_s).max(0.0);
    let op1_report = measure_op(
        &session,
        1,
        &format!("{label} op1(rest)"),
        gen1_s,
        op1_runtime_s,
    );

    // ── Non-blindness: the whole point of the mandatory ordering ────────
    assert!(
        op1_report.move_count > 0,
        "{label}: the rest op (op1) generated ZERO moves — either the fixture gives it \
         nothing to do or the cascade wiring is broken; either way this arm proves nothing. \
         Check that op0 actually left material and that op1's `stock_source` is \
         `FromRemainingStock`."
    );
    let cascade_bound = CASCADE_NON_BLIND_MAX_FRACTION * op0_report.cutting_distance_mm;
    assert!(
        op1_report.cutting_distance_mm < cascade_bound,
        "{label}: REST-BLIND CASCADE — op1 cut {:.1} mm against op0's {:.1} mm ({:.1}%), which \
         is not \"materially less\". A rest pass that re-cuts most or all of the finish pass it \
         follows is the exact A/M6 defect this wave exists to have fixed (self_probe silently \
         dropping territory_clip's confinement). Since claims_reference here is the SHIPPED Auto \
         default (never pinned to self_probe), a failure here means either the cascade skipped \
         its intervening simulation (rest-blind by construction) or the A/M6 fix itself has \
         regressed.",
        op1_report.cutting_distance_mm,
        op0_report.cutting_distance_mm,
        100.0 * op1_report.cutting_distance_mm / op0_report.cutting_distance_mm.max(1e-9),
    );

    let sim = session
        .simulation_result()
        .expect("final simulation result");
    let resolution_clamped = sim.resolution_clamped;
    let collisions = sim.rapid_collisions.len();
    let columns = sim
        .column_deviations
        .as_ref()
        .unwrap_or_else(|| panic!("{label}: no column_deviations on the final cascade sim"))
        .clone();

    // Summing areas across two ops double-counts any ground both operations
    // crossed (`swept_footprint_area`'s doc: a footprint has no notion of
    // "already finished"), so this whole-arm figure is an UPPER BOUND on the
    // cascade's true footprint throughput, not a point estimate.
    let whole_arm_footprint_mm2 = op0_report.footprint_mm2 + op1_report.footprint_mm2;
    let whole_arm_mm2_per_s = swept_footprint_mm2_per_s(
        ProjectedXyAreaMm2::new(whole_arm_footprint_mm2),
        whole_arm_time_s,
    );
    let whole_trips_total = match (op0_report.retract_trips, op1_report.retract_trips) {
        (Some(a), Some(b)) => Some(a.total + b.total),
        _ => None,
    };

    ArmResult {
        label: label.to_owned(),
        columns,
        collisions,
        resolution_clamped,
        whole_arm_footprint_mm2,
        whole_arm_time_s,
        whole_arm_mm2_per_s,
        whole_arm_footprint_is_upper_bound: true,
        whole_trips_total,
        ops: vec![op0_report, op1_report],
    }
}

// ── Rendering (v3 rule: never gate on an aggregate without rendering) ────

fn out_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("h4_strategy");
    std::fs::create_dir_all(&dir).expect("create target/h4_strategy");
    dir
}

/// Top-down render of a per-cell signed value. Red = overcut, blue =
/// leftover, grey = on-size, black = no column. Copied from
/// `classification_columns_ab_m3.rs::render`.
fn render(cells: &HashMap<(usize, usize), f64>, scale_um: f64, name: &str) {
    render_into(&out_dir(), cells, scale_um, name);
}

/// [`render`] with an explicit output directory, so a probe can write
/// its exhibits next to the evidence document that cites them.
fn render_into(dir: &Path, cells: &HashMap<(usize, usize), f64>, scale_um: f64, name: &str) {
    let (rows, cols) = cells.keys().fold((0usize, 0usize), |(r, c), &(rr, cc)| {
        (r.max(rr + 1), c.max(cc + 1))
    });
    if rows == 0 || cols == 0 {
        return;
    }
    let mut px = vec![0u8; rows * cols * 4];
    for (&(r, c), &dev) in cells {
        let um = dev * 1000.0;
        let t = (um.abs() / scale_um).clamp(0.0, 1.0);
        let (rr, gg, bb) = if um.abs() * 1000.0 < 1e-9 {
            (110u8, 110u8, 110u8)
        } else if um < 0.0 {
            (150 + (105.0 * t) as u8, (110.0 * (1.0 - t)) as u8, 60)
        } else {
            (60, (110.0 * (1.0 - t)) as u8, 150 + (105.0 * t) as u8)
        };
        let i = ((rows - 1 - r) * cols + c) * 4;
        px[i] = rr;
        px[i + 1] = gg;
        px[i + 2] = bb;
        px[i + 3] = 255;
    }
    let path = dir.join(format!("{name}.png"));
    image::save_buffer(
        &path,
        &px,
        cols as u32,
        rows as u32,
        image::ColorType::Rgba8,
    )
    .expect("save png");
    eprintln!("  rendered {}", path.display());
}

// ── Reporting ────────────────────────────────────────────────────────────

fn report_op(op: &OpReport) {
    eprintln!(
        "  -- {} -- gen {:.1}s | runtime {:.1}s | footprint {:.1} mm² ({}) | {:.4} mm²/s",
        op.label,
        op.gen_s,
        op.runtime_s,
        op.footprint_mm2,
        op.footprint_provenance.describe(),
        op.mm2_per_s,
    );
    match (op.retract_trips, &op.retract_provenance) {
        (Some(t), Some(p)) => {
            eprintln!(
                "     retract trips: total {} | in-node {:?} | between-nodes {:?} | {}",
                t.total,
                t.in_node,
                t.between_nodes,
                p.describe()
            );
        }
        _ => eprintln!("     retract trips: not measured"),
    }
    match op.truncated_core_mm2 {
        Some(a) => eprintln!("     standing material: {a:.1} mm²"),
        None => eprintln!("     standing material: not measured"),
    }
    eprintln!(
        "     moves {} | cutting distance {:.1} mm",
        op.move_count, op.cutting_distance_mm
    );
    if op.mix.is_empty() {
        eprintln!("     mix: (no Region items — single-strategy op or none classified)");
    } else {
        eprintln!("     | band | strategy | regions | moves | XY area mm² | cutting mm |");
        eprintln!("     |---|---|---|---|---|---|");
        for row in &op.mix {
            eprintln!(
                "     | {} | {} | {} | {} | {:.1} | {:.1} |",
                row.band,
                row.strategy,
                row.regions,
                row.moves,
                row.area_mm2,
                row.cutting_distance_mm
            );
        }
    }
}

fn report_arm(arm: &ArmResult, columns_on_common: Columns) {
    eprintln!("\n== arm {} ==", arm.label);
    for op in &arm.ops {
        report_op(op);
    }
    let bound_note = if arm.whole_arm_footprint_is_upper_bound {
        " (UPPER BOUND — summed areas double-count overlap between ops)"
    } else {
        ""
    };
    eprintln!(
        "  WHOLE ARM: footprint {:.1} mm²{bound_note} | time {:.1}s | {:.4} mm²/s | trips {:?} | \
         rapid collisions {} | resolution_clamped {}",
        arm.whole_arm_footprint_mm2,
        arm.whole_arm_time_s,
        arm.whole_arm_mm2_per_s,
        arm.whole_trips_total,
        arm.collisions,
        arm.resolution_clamped,
    );
    eprintln!(
        "  COLUMNS (common population) n={} p50 {:.1}µm p90 {:.1}µm max {:.1}µm | on-size ±10µm \
         {:.2}% ±25µm {:.2}% | worst overcut {:.1}µm leftover {:.1}µm",
        columns_on_common.n,
        columns_on_common.p50_abs_um,
        columns_on_common.p90_abs_um,
        columns_on_common.max_abs_um,
        100.0 * columns_on_common.on_size_10um,
        100.0 * columns_on_common.on_size_25um,
        columns_on_common.worst_overcut_um,
        columns_on_common.worst_leftover_um,
    );
}

fn final_summary_table(rows: &[(&ArmResult, Columns)]) {
    eprintln!("\n== H4 SUMMARY (re-measurement only — no verdict is asserted) ==");
    eprintln!(
        "| arm | COLUMNS p50 µm | p90 µm | on-size ±25µm | mm²/s | trips | collisions | \
         runtime s |"
    );
    eprintln!("|---|---|---|---|---|---|---|---|");
    for (arm, cols) in rows {
        eprintln!(
            "| {} | {:.1} | {:.1} | {:.2}% | {:.4} | {:?} | {} | {:.1} |",
            arm.label,
            cols.p50_abs_um,
            cols.p90_abs_um,
            100.0 * cols.on_size_25um,
            arm.whole_arm_mm2_per_s,
            arm.whole_trips_total,
            arm.collisions,
            arm.whole_arm_time_s,
        );
    }
}

// ── Pre-registered thresholds ───────────────────────────────────────────
//
// Written down BEFORE the run. These are SAFETY and NON-VACUITY bounds
// ONLY — nothing here adjudicates which arm is "better"; a re-measurement
// that gates on its own preferred outcome cannot re-earn a verdict (see the
// module doc's "Verdict discipline" section). The speed/quality table is
// printed for a human to read.

/// No arm may produce a rapid collision. These fixtures have no obstacles
/// beyond their own stock, so a genuine rapid collision here means the
/// planner drove a rapid through material — a safety defect regardless of
/// which strategy produced it, not a quality tradeoff between strategies.
const MAX_RAPID_COLLISIONS_PER_ARM: usize = 0;

/// A rest pass (Arm C's op1) must cut LESS than 90% of the finish pass it
/// follows, or it is not resting anything — it is re-cutting the part under
/// a rest pass's name. This is not a quality target: A/M6 measured a
/// genuinely rest-confined pass at 0%-25% of the finish pass on its own
/// fixtures (`claims_reference_cascade_am6.rs`'s 0.35 gate, tighter than
/// this one); 0.90 is loose on purpose because this file's fixtures were
/// not built to reproduce that specific defect shape, only to prove the
/// cascade is wired correctly and not blind.
const CASCADE_NON_BLIND_MAX_FRACTION: f64 = 0.90;

/// On-size bins (±10 µm / ±25 µm), reported not gated: justified against the
/// cusp the shared dials ask for (`cusp_mm()` = 0.3² / (8·0.5) = 22.5 µm) —
/// 10 µm is under half that cusp, 25 µm is just over it. Copied from
/// `classification_columns_ab_m3.rs`'s tolerance block, which used the same
/// justification for the same dial set.
#[allow(dead_code)]
const ON_SIZE_BIN_10_UM: f64 = 10.0;
#[allow(dead_code)]
const ON_SIZE_BIN_25_UM: f64 = 25.0;

/// Non-vacuity: Arm B's mix on the terrain fixture must exercise at least
/// this many band/strategy combinations, or the classifier had nothing to
/// decide and the comparison proves nothing (`classification_columns_ab_m3
/// .rs` applies the identical rule to its own cropped fixture).
const MIN_BAND_STRATEGY_COMBOS: usize = 2;

// ── The comparison, per fixture ─────────────────────────────────────────

struct FixtureRun {
    d: ArmResult,
    b: ArmResult,
    c: ArmResult,
}

fn run_all_arms(
    fixture_label: &str,
    mesh: &TriangleMesh,
    stock: StockConfig,
    heights: HeightsConfig,
    sim_mm: f64,
) -> FixtureRun {
    let d = run_single_op_arm(
        &format!("{fixture_label} D (scallop)"),
        stock.clone(),
        mesh_model(mesh.clone(), &format!("{fixture_label}_d")),
        OperationConfig::Scallop(scallop_arm_config()),
        heights.clone(),
        sim_mm,
    );
    let b = run_single_op_arm(
        &format!("{fixture_label} B (unified)"),
        stock.clone(),
        mesh_model(mesh.clone(), &format!("{fixture_label}_b")),
        OperationConfig::UnifiedFinish(unified_arm_config()),
        heights.clone(),
        sim_mm,
    );
    let c = run_cascade_arm(
        &format!("{fixture_label} C (cascade)"),
        stock,
        mesh_model(mesh.clone(), &format!("{fixture_label}_c")),
        heights,
        sim_mm,
    );
    FixtureRun { d, b, c }
}

/// Score all three arms on the column population common to ALL THREE (not
/// just two), render every surface and the pairwise diffs, and run the
/// safety/non-vacuity assertions. `min_mix_combos` gates Arm B's mix size
/// (0 to skip — the sentry-scale fixtures don't need this).
#[allow(clippy::too_many_lines)]
fn adjudicate(fixture_label: &str, run: &FixtureRun, min_mix_combos: usize) {
    let cells_d = by_cell(&run.d.columns);
    let cells_b = by_cell(&run.b.columns);
    let cells_c = by_cell(&run.c.columns);

    let common: Vec<(usize, usize)> = cells_d
        .keys()
        .filter(|k| cells_b.contains_key(*k) && cells_c.contains_key(*k))
        .copied()
        .collect();
    assert!(
        !common.is_empty(),
        "{fixture_label}: the three arms share NO dexel column at all — they were not measured \
         on the same grid, and nothing below means anything. Check that all three arms use the \
         same stock and the same H4_SIM_MM."
    );

    let devs_d: Vec<f64> = common.iter().map(|k| cells_d[k]).collect();
    let devs_b: Vec<f64> = common.iter().map(|k| cells_b[k]).collect();
    let devs_c: Vec<f64> = common.iter().map(|k| cells_c[k]).collect();
    let cols_d = score(&devs_d);
    let cols_b = score(&devs_b);
    let cols_c = score(&devs_c);

    report_arm(&run.d, cols_d);
    report_arm(&run.b, cols_b);
    report_arm(&run.c, cols_c);
    eprintln!(
        "\n{fixture_label} column population: D {} | B {} | C {} | common to all three {}",
        run.d.columns.len(),
        run.b.columns.len(),
        run.c.columns.len(),
        common.len(),
    );

    // ── Render before verdicts (v3 rule) ────────────────────────────────
    render(&cells_d, 100.0, &format!("{fixture_label}_d_dev"));
    render(&cells_b, 100.0, &format!("{fixture_label}_b_dev"));
    render(&cells_c, 100.0, &format!("{fixture_label}_c_dev"));
    let diff_db: HashMap<(usize, usize), f64> = common
        .iter()
        .map(|&k| (k, cells_b[&k] - cells_d[&k]))
        .collect();
    let diff_dc: HashMap<(usize, usize), f64> = common
        .iter()
        .map(|&k| (k, cells_c[&k] - cells_d[&k]))
        .collect();
    let diff_bc: HashMap<(usize, usize), f64> = common
        .iter()
        .map(|&k| (k, cells_c[&k] - cells_b[&k]))
        .collect();
    render(&diff_db, 50.0, &format!("{fixture_label}_b_minus_d"));
    render(&diff_dc, 50.0, &format!("{fixture_label}_c_minus_d"));
    render(&diff_bc, 50.0, &format!("{fixture_label}_c_minus_b"));

    // ── Safety ────────────────────────────────────────────────────────
    for arm in [&run.d, &run.b, &run.c] {
        // SAFETY (lint, not memory): `MAX_RAPID_COLLISIONS_PER_ARM` is a
        // tunable BOUND that currently sits at 0, so `<=` on a `usize` is
        // trivially `== 0` and clippy says so. Kept as `<=` against the named
        // constant on purpose: the assertion should still read correctly, and
        // still hold, if an operator ever raises the bound. Writing `== 0`
        // here would silently decouple the check from the constant.
        #[allow(clippy::absurd_extreme_comparisons)]
        let within_collision_bound = arm.collisions <= MAX_RAPID_COLLISIONS_PER_ARM;
        assert!(
            within_collision_bound,
            "{fixture_label}: SAFETY — arm {} produced {} rapid collision(s), bound is {}",
            arm.label, arm.collisions, MAX_RAPID_COLLISIONS_PER_ARM,
        );
        assert!(
            !arm.resolution_clamped,
            "{fixture_label}: arm {} — the measurement grid was resolution-clamped, so its \
             column population is not at the requested resolution and the cross-arm comparison \
             is unsound",
            arm.label,
        );
    }

    // ── Non-vacuity ──────────────────────────────────────────────────
    for arm in [&run.d, &run.b, &run.c] {
        let total_moves: usize = arm.ops.iter().map(|o| o.move_count).sum();
        assert!(
            total_moves > 0,
            "{fixture_label}: arm {} produced an EMPTY toolpath — this arm proves nothing",
            arm.label,
        );
    }
    if min_mix_combos > 0 {
        let combos = run.b.ops[0].mix.len();
        assert!(
            combos >= min_mix_combos,
            "{fixture_label}: Arm B ran only {combos} band/strategy combination(s) (need >= \
             {min_mix_combos}) — the fixture does not exercise enough of the classifier's \
             territory for the strategy-mix comparison to mean anything",
        );
    }

    // ── The comparison table (printed, not gated) ───────────────────────
    final_summary_table(&[(&run.d, cols_d), (&run.b, cols_b), (&run.c, cols_c)]);
    eprintln!(
        "\n{fixture_label}: this table is a RE-MEASUREMENT, not a verdict. No assertion above \
         adjudicates which arm is faster or higher quality — see the module doc's \"Verdict \
         discipline\" section for why."
    );
}

// ── Evidence tests (minutes-long) ───────────────────────────────────────

#[test]
#[ignore = "H4 strategy comparison (terrain): 3 arms (Scallop, UnifiedFinish, 2-op cascade), \
            each with its own generation + measurement simulation. Run --release, \
            --test-threads=1, on a quiet machine."]
fn h4_strategy_comparison_terrain() {
    let window_mm = env_f64("H4_WINDOW_MM", DEFAULT_WINDOW_MM);
    let sim_mm = env_f64("H4_SIM_MM", DEFAULT_SIM_MM);
    let mesh = terrain(window_mm);
    eprintln!(
        "fixture: terrain.stl cropped to {window_mm} mm ({} triangles), bbox {:?}..{:?}",
        mesh.faces.len(),
        mesh.bbox.min,
        mesh.bbox.max
    );
    let stock = stock_for_terrain(&mesh);
    let heights = pinned_heights(mesh.bbox.max.z, mesh.bbox.min.z);

    let run = run_all_arms("terrain", &mesh, stock, heights, sim_mm);
    adjudicate("terrain", &run, MIN_BAND_STRATEGY_COMBOS);
}

#[test]
#[ignore = "H4 strategy comparison (grooved_block): the concave fixture — terrain.stl alone \
            cannot adjudicate rest-routing behaviour on re-entrant territory. Run --release, \
            --test-threads=1, on a quiet machine."]
fn h4_strategy_comparison_grooved_block() {
    let sim_mm = env_f64("H4_SIM_MM", DEFAULT_SIM_MM);
    let mesh = grooved_fixture();
    eprintln!(
        "fixture: grooved_block(rim={GROOVE_RIM_HALF_WIDTH_MM}, wall={GROOVE_WALL_DEG}deg, \
         depth={GROOVE_DEPTH_MM}) ({} triangles)",
        mesh.faces.len()
    );
    let stock = stock_for_groove();
    let heights = pinned_heights(0.0, -GROOVE_DEPTH_MM);

    let run = run_all_arms("groove", &mesh, stock, heights, sim_mm);
    adjudicate("groove", &run, MIN_BAND_STRATEGY_COMBOS);
}

// ── Cheap sentry: the harness's own arithmetic ──────────────────────────
//
// "A harness whose own arithmetic is unchecked is exactly the failure this
// whole wave exists to correct." This runs in seconds, on a tiny fixture,
// with NO ignore flag, and proves three things about the machinery above
// without running a full 3-arm comparison:
//
// 1. `swept_footprint_area` reads non-zero on a real generated toolpath.
// 2. `retract_trips` is measured (`Some`), not silently absent.
// 3. `region_mix_with_distance`'s per-(band,strategy) distance sum agrees
//    with an INDEPENDENTLY-computed total over the same covered moves — a
//    mark-then-scan algorithm, not the same per-item summation read twice.

/// A tiny trapezoidal groove: rim at `z=0`, floor at `z=-2` over a 1 mm
/// horizontal run (~63° wall — mid-steep scallop territory), 6 mm half-width
/// so the whole fixture is a handful of triangles.
fn tiny_groove() -> TriangleMesh {
    let profile = [
        (-6.0_f64, 0.0_f64),
        (-3.0, 0.0),
        (-2.0, -2.0),
        (2.0, -2.0),
        (3.0, 0.0),
        (6.0, 0.0),
    ];
    common::meshes::extrude_profile(&profile, -6.0, 6.0)
}

fn tiny_groove_stock() -> StockConfig {
    StockConfig {
        x: 16.0,
        y: 16.0,
        z: 4.0,
        origin_x: -8.0,
        origin_y: -8.0,
        origin_z: -4.0,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

/// The independent oracle: mark every move index any Region span covers
/// into a boolean array, then walk the move list ONCE summing cutting-move
/// segments where the CURRENT index is covered — a union-based scan, not a
/// per-item sum. If item ranges are disjoint (the expected case for a
/// planner's own region decomposition) this must agree with `region_mix_
/// with_distance`'s per-item sum to the last bit; if it does not, either the
/// ranges overlap (itself worth knowing) or one of the two algorithms is
/// wrong.
fn total_cutting_distance_over_covered_moves(moves: &[Move], ranges: &[(usize, usize)]) -> f64 {
    if moves.is_empty() {
        return 0.0;
    }
    let mut covered = vec![false; moves.len()];
    for &(s, e) in ranges {
        let hi = e.min(moves.len() - 1);
        for slot in covered.iter_mut().take(hi + 1).skip(s) {
            *slot = true;
        }
    }
    let mut total = 0.0;
    for i in 1..moves.len() {
        if !covered[i] {
            continue;
        }
        let cur = &moves[i];
        if !cur.move_type.is_cutting() {
            continue;
        }
        let prev = &moves[i - 1];
        total += p3_distance(prev.target, cur.target);
    }
    total
}

#[test]
fn h4_harness_arithmetic_sentry() {
    let mesh = tiny_groove();
    let stock = tiny_groove_stock();
    let heights = pinned_heights(0.0, -2.0);
    let tool = ToolConfig {
        diameter: 2.0,
        ..ToolConfig::new_default(ToolId(0), ToolType::BallNose)
    };

    let cfg = UnifiedFinishConfig {
        steep_threshold_deg: 45.0,
        waterline_threshold_deg: 75.0,
        overlap_mm: 1.0,
        scallop_height: 0.05,
        tolerance: 0.05,
        raster_stepover: 1.0,
        z_step: 1.0,
        sampling: 0.5,
        stock_to_leave: 0.0,
        feed_rate: 1500.0,
        plunge_rate: 300.0,
        ..UnifiedFinishConfig::default()
    };

    let mut session = single_op_session_with(
        stock,
        tool,
        mesh_model(mesh, "tiny_groove"),
        "H4 sentry",
        OperationConfig::UnifiedFinish(cfg),
        |c| c.heights = heights,
    );
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("sentry generation must succeed");

    let result = session.get_result(0).expect("generated result");
    let moves = &result.toolpath().moves;

    // (1) swept_footprint_area is non-zero on a real toolpath.
    let (footprint, footprint_provenance) =
        swept_footprint_area(moves, 1.0, DEFAULT_FOOTPRINT_CELL_MM);
    assert!(
        footprint.mm2() > 0.0,
        "a generated finishing toolpath must sweep SOME footprint, got {footprint}"
    );
    assert_eq!(
        footprint_provenance.domain,
        rs_cam_core::measurement::MeasurementDomain::ProjectedXyArea
    );

    // (2) retract_trips is measured.
    assert!(
        result.stats.retract_trips.is_some(),
        "a real generation through generate_toolpath must measure retract trips (Some), not \
         leave the field at its ToolpathStats::default() None"
    );

    // (3) the distance-sum arithmetic agrees with an independent oracle.
    let trace = result.semantic_trace.as_ref().expect("semantic trace");
    let region_items: Vec<_> = trace
        .items
        .iter()
        .filter(|i| i.kind == ToolpathSemanticKind::Region)
        .collect();
    assert!(
        !region_items.is_empty(),
        "the tiny groove must decompose into at least one Region — otherwise the arithmetic \
         check below is vacuous"
    );
    let ranges: Vec<(usize, usize)> = region_items
        .iter()
        .filter_map(|i| i.move_start.zip(i.move_end))
        .collect();
    assert_eq!(
        ranges.len(),
        region_items.len(),
        "every Region item in a completed generation must carry a move range"
    );

    let mix = region_mix_with_distance(Some(trace), moves);
    let sum_from_mix: f64 = mix.iter().map(|r| r.cutting_distance_mm).sum();
    let independent_total = total_cutting_distance_over_covered_moves(moves, &ranges);

    eprintln!(
        "H4 sentry: {} region(s), mix-summed distance {sum_from_mix:.6} mm, independent oracle \
         {independent_total:.6} mm",
        region_items.len()
    );
    assert!(
        (sum_from_mix - independent_total).abs() < 1e-6,
        "the per-(band,strategy) distance sum ({sum_from_mix:.9} mm) must equal the \
         independently-computed total over the same covered moves ({independent_total:.9} mm) — \
         if this fails, either the Region spans overlap (worth knowing) or one of the two \
         distance-summing algorithms in this file is wrong, and nothing this file reports about \
         cutting-distance mix can be trusted until it is fixed"
    );
}

// ── D-16.1 residual locating probe (io-fixes wave, 2026-08-06) ──────────
//
// F23-impl closed D-16.1's MECHANISM claim (`ring_to_3d`'s rounded
// coverage guard: 131 of 3,287 mid-steep cut targets sat outside the part
// footprint, worst distance past the edge 0.375 mm = half a generation
// cell; after the exact point-in-triangle predicate, 0 of 3,133 and
// 0.000 mm). It did NOT close the QUALITY claim: with zero off-footprint
// targets, arm B still overcuts by −201.6 µm, and W8's superposition
// (169 µm rim-riding + 34 µm chord refinement + ~30 µm unaccounted) did
// not survive measurement — removing the 169 µm term moved the total by
// 33 µm.
//
// F23-impl's own "NOT FIXED, STATED" entry names the re-open condition
// verbatim: *"a column-index probe on the grooved fixture that locates
// the worst-overcut column and attributes it — the harness already
// carries `ColumnDeviation`'s row/col, so this is instrumentation, not a
// campaign."* This is that probe. It changes NO finishing geometry: it
// runs the two arms the H4 harness already defines and reads their
// columns.
//
// What it can and cannot do, stated up front:
//
// * It can LOCATE the residual (which columns, where on the groove
//   profile, how many, how concentrated) and it can separate
//   "B-specific" from "hard for everyone" by reading arm D's deviation
//   at the SAME (row, col).
// * It cannot prove a mechanism. Naming the geometry that owns the worst
//   columns is a hypothesis with an address, not an attribution.
//
// ```text
// cargo test -p rs_cam_core --test strategy_comparison_h4 --release \
//     -- --ignored --nocapture --test-threads=1 d16_1_residual
// ```

/// Where the probe's exhibits go — next to the evidence document that
/// cites them, not into `target/`.
fn probe_artifact_dir() -> PathBuf {
    let dir = common::repo_root()
        .join("planning")
        .join("review_2026-08-04")
        .join("artifacts")
        .join("io_probe");
    std::fs::create_dir_all(&dir).expect("create the io_probe artifact dir");
    dir
}

/// Where a column sits on the groove's transverse profile. The profile
/// is a function of x only (`GroovedBlock` is invariant in y), so this
/// is exact rather than sampled.
///
/// The three surface zones are the profile's own faces, partitioned
/// strictly — no "corner" zone competing with them. At the shipped
/// parameters the wall is only `depth / tan(70°)` = 0.437 mm wide in x,
/// so a corner band wide enough to be interesting would swallow it.
/// Proximity to a profile break is reported separately, by
/// [`dist_to_profile_break`], which keeps the partition clean.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum GrooveZone {
    /// Flat rim, |x| >= rim half-width. 0°.
    Rim,
    /// The 70° wall. Mid-steep by construction.
    Wall,
    /// Flat floor, |x| <= floor half-width. 0°.
    Floor,
    /// Outside the mesh footprint entirely (stock margin).
    OffPart,
}

/// Half-width of the groove floor: where the wall meets the flat.
fn groove_floor_half() -> f64 {
    GROOVE_RIM_HALF_WIDTH_MM - GROOVE_DEPTH_MM / GROOVE_WALL_DEG.to_radians().tan()
}

/// Distance (mm) from a column to the nearer of the two profile breaks
/// on its side — floor/wall or wall/rim. Small values are the corners
/// the cutter cannot enter; large values are open faces.
fn dist_to_profile_break(x: f64) -> f64 {
    let ax = x.abs();
    (ax - groove_floor_half())
        .abs()
        .min((ax - GROOVE_RIM_HALF_WIDTH_MM).abs())
}

impl GrooveZone {
    fn label(self) -> &'static str {
        match self {
            GrooveZone::Rim => "rim (flat)",
            GrooveZone::Wall => "wall (70 deg)",
            GrooveZone::Floor => "floor (flat)",
            GrooveZone::OffPart => "off-part",
        }
    }

    /// Classify by |x| against the profile's own breakpoints.
    fn of(x: f64, y: f64) -> Self {
        if x.abs() > GROOVE_X_EXTENT_MM || y.abs() > GROOVE_Y_HALF_MM {
            return GrooveZone::OffPart;
        }
        let ax = x.abs();
        if ax >= GROOVE_RIM_HALF_WIDTH_MM {
            GrooveZone::Rim
        } else if ax > groove_floor_half() {
            GrooveZone::Wall
        } else {
            GrooveZone::Floor
        }
    }
}

/// One overcut column, with everything needed to attribute it.
#[derive(Debug, Clone, Copy)]
struct OvercutColumn {
    /// Dexel grid address — the identity that survives a cross-arm
    /// comparison (MEMORY.md: index by row/col, never inverse-transform
    /// XY).
    cell: (usize, usize),
    /// Arm B deviation, µm. Negative = overcut.
    b_um: f64,
    /// Arm D deviation at the SAME cell, µm.
    d_um: f64,
    x: f64,
    y: f64,
}

/// One arm, kept whole: the probe needs the session (for moves and the
/// semantic trace), not just the scored columns.
struct ProbeArm {
    label: String,
    session: ProjectSession,
    columns: Vec<ColumnDeviation>,
}

fn run_probe_arm(label: &str, op: OperationConfig, sim_mm: f64) -> ProbeArm {
    let mesh = grooved_fixture();
    let heights = pinned_heights(0.0, -GROOVE_DEPTH_MM);
    let mut session = single_op_session_with(
        stock_for_groove(),
        session_tool(),
        mesh_model(mesh, label),
        label,
        op,
        |cfg| {
            cfg.heights = heights;
        },
    );
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .unwrap_or_else(|e| panic!("{label}: generation failed: {e:?}"));
    session
        .run_simulation(
            &SimulationOptions {
                resolution: sim_mm,
                ..Default::default()
            },
            &cancel,
        )
        .unwrap_or_else(|e| panic!("{label}: measurement simulation failed: {e:?}"));
    let columns = session
        .simulation_result()
        .and_then(|s| s.column_deviations.clone())
        .unwrap_or_else(|| panic!("{label}: no column_deviations"));
    ProbeArm {
        label: label.to_owned(),
        session,
        columns,
    }
}

/// Which Region (band + strategy) owns the cutting move nearest a given
/// XY, and how far away that move actually is. Returns `None` when the
/// op has no semantic trace or no cutting moves.
fn nearest_region_to(
    session: &ProjectSession,
    x: f64,
    y: f64,
) -> Option<(String, String, f64, usize)> {
    let result = session.get_result(0)?;
    let moves = &result.toolpath().moves;
    let trace = result.semantic_trace.as_ref()?;

    let mut best: Option<(f64, usize)> = None;
    for (i, mv) in moves.iter().enumerate() {
        if !mv.move_type.is_cutting() {
            continue;
        }
        let d = ((mv.target.x - x).powi(2) + (mv.target.y - y).powi(2)).sqrt();
        match best {
            Some((bd, _)) if bd <= d => {}
            _ => best = Some((d, i)),
        }
    }
    let (dist, idx) = best?;

    for item in trace
        .items
        .iter()
        .filter(|i| i.kind == ToolpathSemanticKind::Region)
    {
        if let Some((s, e)) = item.move_start.zip(item.move_end)
            && idx >= s
            && idx <= e
        {
            let text = |key: SemanticKey| {
                item.params
                    .get(key)
                    .and_then(|v| v.as_str())
                    .unwrap_or("(unlabelled)")
                    .to_owned()
            };
            return Some((
                text(SemanticKey::Band),
                text(SemanticKey::Strategy),
                dist,
                idx,
            ));
        }
    }
    Some(("(no region span)".to_owned(), String::new(), dist, idx))
}

#[test]
#[ignore = "D-16.1 residual locating probe: two arms (UnifiedFinish, Scallop) on the grooved \
            fixture, each with its own generation + 0.1 mm measurement simulation. Research \
            only — it asserts non-vacuity and prints, it does not gate quality. Run --release, \
            --test-threads=1."]
fn d16_1_residual_locating_probe() {
    let sim_mm = env_f64("H4_SIM_MM", DEFAULT_SIM_MM);
    let overcut_gate_um = env_f64("D161_OVERCUT_UM", 50.0);

    eprintln!(
        "D-16.1 residual probe — grooved_block(rim={GROOVE_RIM_HALF_WIDTH_MM}, \
         wall={GROOVE_WALL_DEG}deg, depth={GROOVE_DEPTH_MM}), sim {sim_mm} mm"
    );
    let floor_half = groove_floor_half();
    eprintln!(
        "  profile breakpoints: floor |x| <= {floor_half:.4}, wall {floor_half:.4} < |x| < \
         {GROOVE_RIM_HALF_WIDTH_MM}, rim |x| >= {GROOVE_RIM_HALF_WIDTH_MM}; part y in \
         [-{GROOVE_Y_HALF_MM}, {GROOVE_Y_HALF_MM}]"
    );

    let b = run_probe_arm(
        "B unified",
        OperationConfig::UnifiedFinish(unified_arm_config()),
        sim_mm,
    );
    let d = run_probe_arm(
        "D scallop",
        OperationConfig::Scallop(scallop_arm_config()),
        sim_mm,
    );

    let b_cells = by_cell(&b.columns);
    let d_cells = by_cell(&d.columns);
    // XY per (row, col), taken from B (both arms share the grid geometry —
    // same stock, same resolution — which the agreement check below tests
    // rather than assumes).
    let mut xy: HashMap<(usize, usize), (f64, f64)> = HashMap::new();
    for c in &b.columns {
        xy.insert((c.row, c.col), (c.x, c.y));
    }
    let mut grid_disagreements = 0usize;
    for c in &d.columns {
        if let Some(&(bx, by)) = xy.get(&(c.row, c.col))
            && ((bx - c.x).abs() > 1e-6 || (by - c.y).abs() > 1e-6)
        {
            grid_disagreements += 1;
        }
    }
    assert_eq!(
        grid_disagreements, 0,
        "the two arms' (row, col) grids must address the same world XY, or every cross-arm \
         comparison below is meaningless"
    );

    let common: Vec<(usize, usize)> = b_cells
        .keys()
        .filter(|k| d_cells.contains_key(*k))
        .copied()
        .collect();
    assert!(
        common.len() > 10_000,
        "only {} common columns — the probe has no population",
        common.len()
    );
    eprintln!(
        "  columns: B {}, D {}, common {}",
        b_cells.len(),
        d_cells.len(),
        common.len()
    );

    // ── Per-zone table ──────────────────────────────────────────────
    let mut per_zone: HashMap<GrooveZone, (Vec<f64>, Vec<f64>)> = HashMap::new();
    for key in &common {
        let (&bd, &dd) = (
            b_cells.get(key).unwrap_or(&f64::NAN),
            d_cells.get(key).unwrap_or(&f64::NAN),
        );
        let Some(&(x, y)) = xy.get(key) else { continue };
        let entry = per_zone.entry(GrooveZone::of(x, y)).or_default();
        entry.0.push(bd * 1000.0);
        entry.1.push(dd * 1000.0);
    }
    let mut zones: Vec<_> = per_zone.into_iter().collect();
    zones.sort_by_key(|(z, _)| *z);
    eprintln!(
        "\n  {:<16} {:>8} {:>12} {:>12} {:>12} {:>12}",
        "zone", "columns", "B worst um", "D worst um", "B p50 |um|", "D p50 |um|"
    );
    for (zone, (bs, ds)) in &zones {
        let worst = |v: &[f64]| v.iter().copied().fold(0.0_f64, f64::min);
        let p50 = |v: &[f64]| {
            let mut a: Vec<f64> = v.iter().map(|d| d.abs()).collect();
            a.sort_by(f64::total_cmp);
            quantile(&a, 0.50)
        };
        eprintln!(
            "  {:<16} {:>8} {:>12.1} {:>12.1} {:>12.2} {:>12.2}",
            zone.label(),
            bs.len(),
            worst(bs),
            worst(ds),
            p50(bs),
            p50(ds)
        );
    }

    // ── The overcut set ─────────────────────────────────────────────
    let mut overcut: Vec<OvercutColumn> = Vec::new();
    for key in &common {
        let b_um = b_cells.get(key).copied().unwrap_or(0.0) * 1000.0;
        if b_um >= -overcut_gate_um {
            continue;
        }
        let d_um = d_cells.get(key).copied().unwrap_or(0.0) * 1000.0;
        let Some(&(x, y)) = xy.get(key) else { continue };
        overcut.push(OvercutColumn {
            cell: *key,
            b_um,
            d_um,
            x,
            y,
        });
    }
    overcut.sort_by(|a, b| a.b_um.total_cmp(&b.b_um));

    eprintln!(
        "\n  columns overcut worse than {overcut_gate_um:.0} um on B: {} of {} ({:.4}%)",
        overcut.len(),
        common.len(),
        100.0 * overcut.len() as f64 / common.len() as f64
    );
    if overcut.is_empty() {
        eprintln!("  no residual at this gate — nothing to attribute");
        return;
    }

    let xs: Vec<f64> = overcut.iter().map(|o| o.x).collect();
    let ys: Vec<f64> = overcut.iter().map(|o| o.y).collect();
    let min_max = |v: &[f64]| {
        (
            v.iter().copied().fold(f64::INFINITY, f64::min),
            v.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        )
    };
    let (x_lo, x_hi) = min_max(&xs);
    let (y_lo, y_hi) = min_max(&ys);
    let at_ends = ys
        .iter()
        .filter(|y| y.abs() > GROOVE_Y_HALF_MM - 1.0)
        .count();
    eprintln!(
        "  overcut-set extent: x [{x_lo:.3}, {x_hi:.3}], y [{y_lo:.3}, {y_hi:.3}]; \
         {at_ends} of {} ({:.1}%) sit within 1 mm of a longitudinal end",
        overcut.len(),
        100.0 * at_ends as f64 / overcut.len() as f64
    );

    let mut zone_counts: HashMap<GrooveZone, usize> = HashMap::new();
    for o in &overcut {
        *zone_counts.entry(GrooveZone::of(o.x, o.y)).or_default() += 1;
    }
    let mut zc: Vec<_> = zone_counts.into_iter().collect();
    zc.sort_by_key(|(z, _)| *z);
    eprintln!("  overcut set by zone:");
    for (zone, n) in zc {
        eprintln!(
            "    {:<16} {n:>7} ({:.1}%)",
            zone.label(),
            100.0 * n as f64 / overcut.len() as f64
        );
    }

    // Proximity to a profile break, reported separately from the zone
    // partition (see `GrooveZone`'s doc for why the two are not one
    // table). The envelope radius is 3.0 mm and the cusp radius 0.5 mm,
    // so the 0.5 / 1.0 bins are the scales at which "the cutter cannot
    // enter this corner" is a live explanation.
    let mut break_bins = [0usize; 4];
    for o in &overcut {
        let d = dist_to_profile_break(o.x);
        let bin = if d < 0.25 {
            0
        } else if d < 0.5 {
            1
        } else if d < 1.0 {
            2
        } else {
            3
        };
        break_bins[bin] += 1;
    }
    eprintln!("  overcut set by distance to the nearer profile break:");
    for (label, n) in [
        ("< 0.25 mm", break_bins[0]),
        ("0.25-0.5 mm", break_bins[1]),
        ("0.5-1.0 mm", break_bins[2]),
        (">= 1.0 mm", break_bins[3]),
    ] {
        eprintln!(
            "    {label:<16} {n:>7} ({:.1}%)",
            100.0 * n as f64 / overcut.len() as f64
        );
    }

    // ── The worst columns, one line each ────────────────────────────
    eprintln!(
        "\n  {:>4} {:>9} {:>9} {:>11} {:>11} {:>8} {:<16} {:<26} {:>9}",
        "#", "x", "y", "B dev um", "D dev um", "d_break", "zone", "B nearest region", "cut dist"
    );
    for (i, o) in overcut.iter().take(12).enumerate() {
        let (band, strategy, dist, idx) = nearest_region_to(&b.session, o.x, o.y)
            .unwrap_or_else(|| ("(no trace)".to_owned(), String::new(), f64::NAN, 0));
        let region = format!("{band}/{strategy}");
        let (x, y, b_um, d_um) = (o.x, o.y, o.b_um, o.d_um);
        eprintln!(
            "  {:>4} {x:>9.3} {y:>9.3} {b_um:>11.1} {d_um:>11.1} {:>8.3} {:<16} {region:<26} \
             {dist:>9.3}  (row {}, col {}, move {idx})",
            i + 1,
            dist_to_profile_break(x),
            GrooveZone::of(x, y).label(),
            o.cell.0,
            o.cell.1
        );
    }

    // ── Is the residual B-specific? ─────────────────────────────────
    let both_bad = overcut.iter().filter(|o| o.d_um < -overcut_gate_um).count();
    eprintln!(
        "\n  of B's {} overcut columns, {both_bad} ({:.1}%) are ALSO overcut past the gate on \
         arm D — the rest are B-specific",
        overcut.len(),
        100.0 * both_bad as f64 / overcut.len() as f64
    );

    // ── Exhibits ────────────────────────────────────────────────────
    let dir = probe_artifact_dir();
    render_into(&dir, &b_cells, 250.0, "d161_probe_b_dev");
    render_into(&dir, &d_cells, 250.0, "d161_probe_d_dev");
    let diff: HashMap<(usize, usize), f64> = common
        .iter()
        .map(|k| {
            (
                *k,
                b_cells.get(k).copied().unwrap_or(0.0) - d_cells.get(k).copied().unwrap_or(0.0),
            )
        })
        .collect();
    render_into(&dir, &diff, 250.0, "d161_probe_b_minus_d");
    let mask: HashMap<(usize, usize), f64> = common
        .iter()
        .map(|k| {
            let bd = b_cells.get(k).copied().unwrap_or(0.0);
            (
                *k,
                if bd * 1000.0 < -overcut_gate_um {
                    bd
                } else {
                    0.0
                },
            )
        })
        .collect();
    render_into(&dir, &mask, 250.0, "d161_probe_b_overcut_mask");

    // Non-vacuity only. This probe reports; it does not gate.
    assert!(
        b.columns.len() > 10_000 && d.columns.len() > 10_000,
        "both arms must produce a real column population"
    );
    eprintln!(
        "\n  probe complete — arms {} / {}; exhibits in {}",
        b.label,
        d.label,
        dir.display()
    );
}
