//! **Phase F1 step 0 — capture the Wanaka region-1 boundary as an artifact.**
//! (`planning/conformal_finish_2026-08-28/PROGRAMME.md` §F1 step 0, and F0
//! finding 2: *"the captured Wanaka region-1 boundary this contract names does
//! not exist as an artifact"*.)
//!
//! # Why this exists
//!
//! Every F1 candidate is scored against one bounded region — "region 1" — and
//! against the two reference times already published for it (**875.9 s** PCA
//! cell, **1053.7 s** 0° raster; `planning/thin_organic_2026-08-27/FINDINGS.md`).
//! Until now that region existed only *transiently*: it is recomputed inside
//! the `#[ignore]`d evidence test `thin_organic_island_widths.rs` from a
//! machine-local 661k-triangle STL that is not in the repo. Nothing downstream
//! could load it, and nothing could prove that a later run derived the SAME
//! region the reference tables were measured on.
//!
//! This file does two things and nothing else:
//!
//! * [`capture_wanaka_region_1_boundary`] runs **only** the region-derivation
//!   chain and serialises the region-1 `Polygon2` + every pinned parameter to
//!   `test_data/wanaka_region1_boundary_f1.json`. No costing, no toolpath
//!   generation, no SVG.
//! * [`wanaka_region_1_capture_matches_recomputation`] is the **drift sentry**:
//!   it re-runs the same chain and asserts the recomputation still equals the
//!   committed capture.
//!
//! # The derivation chain is FROZEN — do not "clean it up"
//!
//! Region 1's identity comes from this exact chain, including the two
//! **tapered-ball** cutters. A ball-end derivation would produce a different
//! region and silently destroy comparability with every prior evidence table.
//! Ball-end cutters enter later F1 phases for candidate **paths** only, never
//! for region derivation. If the sentry goes red, the chain has drifted and
//! the capture — plus all downstream F1 evidence — must be re-cut deliberately.
//!
//! Restated verbatim from `thin_organic_island_widths.rs`
//! (`wanaka_monotone_cells_kept_retracts` setup, ~lines 556–637):
//!
//! ```text
//! TriangleMesh::from_stl_scaled(WANAKA_MESH, 1.0)
//!   → SpatialIndex::build_auto
//!   → TaperedBallEndmill::new(3.0, 2.8, 6.0, 30.5)   // R1.5, tier 0
//!     TaperedBallEndmill::new(2.0, 5.7, 6.0, 20.0)   // R1.0, tier 1
//!   → TierLadder::new(&[r15, r10])
//!   → compute_tier_map { cell 0.3, tol 0.05, margin 0.5, SlopeCompensated }
//!   → extract_tier_islands { coarseness 1.0, overlap 2.0, max 24, ..default }
//!   → tier 1 island set (`per_tier` where `tier == 1`)
//!   → build_classification_surface_with_sampler_and_cancel(
//!         mesh, index, r10,
//!         unified_finish_classification_resolution(r10, 0.05),
//!         ClassificationSampler::PRODUCTION)
//!   → covered mask = heightmap.covered_flags() ∧ fine.machining.contains(cell)
//!   → FinishPlannerParams::for_tool(cusp_radii[1]) with overlap_mm = 2.0
//!   → decompose(&surface.slope_map, &covered, &[], &planner)
//!   → regions filtered to FinishBand::Shallow, sorted by area DESC, .first()
//! ```
//!
//! # JSON schema (`test_data/wanaka_region1_boundary_f1.json`)
//!
//! Deliberately dead simple, because integration tests cannot import from each
//! other — a later F1 evidence test is expected to **restate** the ~20-line
//! loader rather than depend on this file. Keys, compact JSON, one object:
//!
//! ```text
//! {
//!   "schema":        "wanaka_region1_boundary_f1/1",
//!   "capture_date":  "2026-08-29",
//!   "selection_rule": "<prose: how region 1 was picked>",
//!   "closed":        true,                        // Polygon2::closed
//!   "exterior":      [[x, y], ...],               // Polygon2::exterior
//!   "holes":         [ [[x, y], ...], ... ],      // Polygon2::holes
//!   "integrity": {
//!     "area_mm2":                f64,   // Polygon2::area() — exterior minus holes
//!     "signed_area_mm2":         f64,   // exterior ring only, sign = winding
//!     "hole_count":              usize,
//!     "exterior_vertex_count":   usize,
//!     "hole_vertex_counts":      [usize, ...]
//!   },
//!   "params": { ... every pinned input of the chain, see `provenance_json` ... }
//! }
//! ```
//!
//! Rings are `[[x, y], …]` and coordinates are emitted through `serde_json`'s
//! shortest-round-trip float formatting, so a read-back is bit-identical — the
//! same `{exterior, holes, closed}` convention as
//! `test_data/m5_terrain_mid_steep_polygon.json`, plus the `params` and
//! `integrity` blocks this capture needs and that one does not have.
//!
//! # Running it
//!
//! ```text
//! # Cut the capture (writes test_data/wanaka_region1_boundary_f1.json):
//! cargo test -p rs_cam_core --test wanaka_region_capture_f1 \
//!   capture_wanaka_region_1_boundary -- --ignored --nocapture
//!
//! # Drift sentry (needs BOTH the mesh and the committed capture):
//! cargo test -p rs_cam_core --test wanaka_region_capture_f1 \
//!   wanaka_region_1_capture_matches_recomputation -- --ignored --nocapture
//! ```
//!
//! `#[ignore]` because both need the operator's wanaka mesh, which is not in
//! the repo — the same meaning `#[ignore]` carries everywhere else here. Both
//! SKIP rather than fail when an input is absent.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use std::path::{Path, PathBuf};

use rs_cam_core::classify_probe::ClassificationSampler;
use rs_cam_core::finish_planner::{FinishBand, FinishPlannerParams, decompose};
use rs_cam_core::finish_setup::build_classification_surface_with_sampler_and_cancel;
use rs_cam_core::geo::P2;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::tier_islands::{TierIslandParams, extract_tier_islands};
use rs_cam_core::tier_map::{ResidualTreatment, TierLadder, TierMapParams, compute_tier_map};
use rs_cam_core::tool::{MillingCutter, TaperedBallEndmill};
use rs_cam_core::unified_finish::unified_finish_classification_resolution;

// ── the frozen chain's constants ────────────────────────────────────────
//
// Copied verbatim from `thin_organic_island_widths.rs`, which in turn copied
// them from `planning/multitool_2026-08-23/wanaka200_mt2.toml` (UNTRACKED —
// see that file's note; it describes an R1.5 → R1.0 ladder at a 30 µm cusp).
// They are pinned HERE on purpose: the capture must be reproducible from the
// repo alone even if the operator's play-file moves again.

/// The operator's wanaka board. Absolute, outside the repo, by nature.
const WANAKA_MESH: &str = "/home/ricky/Downloads/wanaka200/rivmap_export/terrain.stl";

/// Tier-map grid cell (mm).
const CELL_MM: f64 = 0.3;
/// Tier-map tolerance (mm).
const TOLERANCE_MM: f64 = 0.05;
/// Tier-map margin (mm).
const MARGIN_MM: f64 = 0.5;
/// Tier-island coarseness.
const COARSENESS: f64 = 1.0;
/// Tier-island / finish-planner band dilation (mm).
const OVERLAP_MM: f64 = 2.0;
/// Tier-island region cap per tier.
const MAX_REGIONS_PER_TIER: usize = 24;
/// `scallop_height` on both tier ops — the cusp target the equal-cusp
/// stepovers are derived from.
const CUSP_HEIGHT_MM: f64 = 0.03;
/// The unified op's own path tolerance, which sizes its classification grid.
const OP_TOLERANCE_MM: f64 = 0.05;

// Argument order is `TaperedBallEndmill::new(ball_diameter,
// taper_half_angle_deg, shaft_diameter, cutting_length)` — checked against
// `tool/tapered_ball.rs`, NOT inferred from the call site. Note the second
// slot is a HALF-angle from the tool axis, and the fourth is cutting length,
// not stickout.

/// Tier-0 cutter: R1.5 tapered ball —
/// `(ball_diameter, taper_half_angle_deg, shaft_diameter, cutting_length)`.
const R15_TUPLE: [f64; 4] = [3.0, 2.8, 6.0, 30.5];
/// Tier-1 cutter: R1.0 tapered ball —
/// `(ball_diameter, taper_half_angle_deg, shaft_diameter, cutting_length)`.
const R10_TUPLE: [f64; 4] = [2.0, 5.7, 6.0, 20.0];

/// Hardcoded per the F1 step-0 brief, so the artifact records when the chain
/// was frozen rather than when someone happened to re-run the capture.
const CAPTURE_DATE: &str = "2026-08-29";

/// Schema tag, so a later reader can tell a v1 capture from a re-cut one.
const SCHEMA_TAG: &str = "wanaka_region1_boundary_f1/1";

/// How region 1 is selected, recorded in the artifact so the rule travels with
/// the data rather than living only in this file.
const SELECTION_RULE: &str = "planned.regions filtered to FinishBand::Shallow, \
     sorted by Polygon2::area() descending, first element \
     (== `region 1` in thin_organic_island_widths.rs stage_i / stage_k)";

/// `s = 2·√(2Rh − h²)` — the equal-cusp law. One site in production
/// (`session::multitool`); restated here because an instrument should show its
/// own arithmetic. Provenance only: the stepover does NOT feed the boundary.
fn equal_cusp_stepover_mm(cusp_radius_mm: f64, h: f64) -> f64 {
    if h <= 0.0 || h >= cusp_radius_mm {
        return 0.0;
    }
    2.0 * (2.0 * cusp_radius_mm * h - h * h).sqrt()
}

/// Absolute path to the workspace root (two levels up from the crate manifest
/// dir), canonicalized. Restated from `tests/common/mod.rs` — an integration
/// test cannot import another one, and this file is deliberately standalone.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root resolves")
}

/// Where the committed capture lives.
fn capture_path() -> PathBuf {
    repo_root().join("test_data/wanaka_region1_boundary_f1.json")
}

// ── the derivation ──────────────────────────────────────────────────────

/// Everything the chain produces that the artifact records. One struct, one
/// producer — the capture and the sentry MUST NOT restate the chain
/// separately, or the sentry would only ever compare a copy against itself.
struct Derivation {
    /// Region 1: the largest-area Shallow band polygon.
    boundary: Polygon2,
    mesh_triangle_count: usize,
    /// The two cutters' cusp radii, in ladder order: `[R1.5, R1.0]`.
    cusp_radii_mm: [f64; 2],
    /// The resolved classification grid cell (mm) — the one chain input that is
    /// otherwise invisible, since it comes out of a library policy rather than
    /// a literal here.
    classification_cell_mm: f64,
    /// Tier-1 equal-cusp stepover (mm). Provenance only.
    stepover_mm: f64,
    planned_region_count: usize,
    shallow_region_count: usize,
}

/// Run the frozen chain. `Err` is a SKIP reason, not a failure.
fn derive_region_1(mesh_path: &Path) -> Result<Derivation, &'static str> {
    let mesh = TriangleMesh::from_stl_scaled(mesh_path, 1.0).expect("load wanaka terrain");
    let index = SpatialIndex::build_auto(&mesh);
    // Built from the tuple consts rather than re-typed literals, so the
    // cutters the chain runs and the cutters the artifact records cannot drift
    // apart. Equivalent to the source instrument's
    // `TaperedBallEndmill::new(3.0, 2.8, 6.0, 30.5)` / `(2.0, 5.7, 6.0, 20.0)`.
    let r15 = TaperedBallEndmill::new(R15_TUPLE[0], R15_TUPLE[1], R15_TUPLE[2], R15_TUPLE[3]);
    let r10 = TaperedBallEndmill::new(R10_TUPLE[0], R10_TUPLE[1], R10_TUPLE[2], R10_TUPLE[3]);
    let tools: [&dyn MillingCutter; 2] = [&r15, &r10];
    let ladder = TierLadder::new(&tools).expect("ladder");
    let never_cancel = || false;
    let map = compute_tier_map(
        &mesh,
        &index,
        &ladder,
        &TierMapParams {
            cell_mm: CELL_MM,
            tolerance_mm: TOLERANCE_MM,
            margin_mm: MARGIN_MM,
            treatment: ResidualTreatment::SlopeCompensated,
        },
        &never_cancel,
    )
    .expect("tier map");
    let cusp_radii: Vec<f64> = tools.iter().map(|tool| tool.cusp_radius_mm()).collect();
    let islands = extract_tier_islands(
        &map,
        &TierIslandParams {
            coarseness: COARSENESS,
            overlap_mm: OVERLAP_MM,
            max_regions_per_tier: MAX_REGIONS_PER_TIER,
            ..TierIslandParams::default()
        },
        &cusp_radii,
    )
    .expect("islands");
    let Some(fine) = islands.per_tier.iter().find(|set| set.tier == 1) else {
        return Err("no tier 1 machining region");
    };
    if fine.machining.is_empty() {
        return Err("tier 1 machining region is empty");
    }

    let resolution = unified_finish_classification_resolution(&r10, OP_TOLERANCE_MM);
    let surface = build_classification_surface_with_sampler_and_cancel(
        &mesh,
        &index,
        &r10,
        resolution,
        ClassificationSampler::PRODUCTION,
        &never_cancel,
    )
    .expect("classification surface");
    let heightmap = &surface.heightmap;
    let covered: Vec<bool> = heightmap
        .covered_flags()
        .iter()
        .enumerate()
        .map(|(i, &covered)| {
            if !covered {
                return false;
            }
            let row = i / heightmap.cols;
            let col = i % heightmap.cols;
            fine.machining.contains(&P2::new(
                heightmap.origin_x + col as f64 * heightmap.cell_size,
                heightmap.origin_y + row as f64 * heightmap.cell_size,
            ))
        })
        .collect();
    let mut planner = FinishPlannerParams::for_tool(cusp_radii[1]);
    planner.overlap_mm = OVERLAP_MM;
    let planned = decompose(&surface.slope_map, &covered, &[], &planner);

    // `PlannedRegions` documents its regions as already largest-area-first
    // within a band. The sort is restated anyway, exactly as stage_i does it,
    // so "region 1" is defined by this file's own arithmetic and cannot be
    // redefined by a change in the planner's emission order.
    let mut shallow: Vec<&Polygon2> = planned
        .regions
        .iter()
        .filter(|region| region.band == FinishBand::Shallow)
        .map(|region| &region.polygon)
        .collect();
    shallow.sort_by(|a, b| b.area().total_cmp(&a.area()));
    let Some(&boundary) = shallow.first() else {
        return Err("no Shallow band region");
    };

    Ok(Derivation {
        boundary: boundary.clone(),
        mesh_triangle_count: mesh.triangles.len(),
        cusp_radii_mm: [cusp_radii[0], cusp_radii[1]],
        classification_cell_mm: resolution.cell_mm(),
        stepover_mm: equal_cusp_stepover_mm(cusp_radii[1], CUSP_HEIGHT_MM),
        planned_region_count: planned.regions.len(),
        shallow_region_count: shallow.len(),
    })
}

// ── serialisation ───────────────────────────────────────────────────────

/// One ring as `[[x, y], …]`. Coordinates go through `serde_json`'s
/// shortest-round-trip float formatting — never a manual `format!`, which is
/// what would cost the bit-identical read-back the sentry's 1e-9 epsilon
/// assumes.
fn ring_json(ring: &[P2]) -> serde_json::Value {
    serde_json::Value::Array(ring.iter().map(|p| serde_json::json!([p.x, p.y])).collect())
}

/// Every pinned input of the chain, so the artifact is self-describing.
fn provenance_json(derived: &Derivation) -> serde_json::Value {
    serde_json::json!({
        "mesh_path": WANAKA_MESH,
        "mesh_scale": 1.0,
        "mesh_triangle_count": derived.mesh_triangle_count,
        "spatial_index": "SpatialIndex::build_auto",
        "cutters": {
            "note": "TaperedBallEndmill::new(ball_diameter_mm, \
                     taper_half_angle_deg, shaft_diameter_mm, \
                     cutting_length_mm). FROZEN — a ball-end substitution \
                     would derive a DIFFERENT region.",
            "tier_0_r15": R15_TUPLE,
            "tier_1_r10": R10_TUPLE,
            "cusp_radii_mm": derived.cusp_radii_mm,
        },
        "tier_map": {
            "cell_mm": CELL_MM,
            "tolerance_mm": TOLERANCE_MM,
            "margin_mm": MARGIN_MM,
            "treatment": "ResidualTreatment::SlopeCompensated",
        },
        "tier_islands": {
            "coarseness": COARSENESS,
            "overlap_mm": OVERLAP_MM,
            "max_regions_per_tier": MAX_REGIONS_PER_TIER,
            "other_fields": "TierIslandParams::default()",
            "tier_selected": 1,
        },
        "classification_surface": {
            "cutter": "tier_1_r10",
            "resolution_policy": "unified_finish_classification_resolution(r10, op_tolerance_mm)",
            "resolved_cell_mm": derived.classification_cell_mm,
            "sampler": "ClassificationSampler::PRODUCTION",
            "covered_mask": "heightmap.covered_flags() AND \
                             tier1.machining.contains(origin + index * cell_size)",
        },
        "finish_planner": {
            "base": "FinishPlannerParams::for_tool(cusp_radii[1])",
            "overlap_mm": OVERLAP_MM,
            "call": "decompose(&surface.slope_map, &covered, &[], &planner)",
            "planned_region_count": derived.planned_region_count,
            "shallow_region_count": derived.shallow_region_count,
        },
        "cusp_height_mm": CUSP_HEIGHT_MM,
        "op_tolerance_mm": OP_TOLERANCE_MM,
        "stepover_mm": derived.stepover_mm,
        "stepover_note": "s = 2*sqrt(2*R*h - h^2) at R = cusp_radii[1], h = cusp_height_mm. \
                          PROVENANCE ONLY — the stepover does not feed the boundary.",
        "source_instrument": "crates/rs_cam_core/tests/thin_organic_island_widths.rs \
                              (wanaka_monotone_cells_kept_retracts setup)",
        "reference_times_s": {
            "note": "planning/thin_organic_2026-08-27/FINDINGS.md — the numbers \
                     any F1 candidate is scored against on THIS region.",
            "pca_cell": 875.9,
            "raster_0deg": 1053.7,
        },
    })
}

/// The full artifact document.
fn capture_json(derived: &Derivation) -> String {
    let poly = &derived.boundary;
    let hole_vertex_counts: Vec<usize> = poly.holes.iter().map(Vec::len).collect();
    serde_json::json!({
        "schema": SCHEMA_TAG,
        "capture_date": CAPTURE_DATE,
        "selection_rule": SELECTION_RULE,
        "closed": poly.closed,
        "exterior": ring_json(&poly.exterior),
        "holes": serde_json::Value::Array(poly.holes.iter().map(|h| ring_json(h)).collect()),
        "integrity": {
            "area_mm2": poly.area(),
            "signed_area_mm2": poly.signed_area(),
            "hole_count": poly.holes.len(),
            "exterior_vertex_count": poly.exterior.len(),
            "hole_vertex_counts": hole_vertex_counts,
        },
        "params": provenance_json(derived),
    })
    .to_string()
}

// ── the loader ──────────────────────────────────────────────────────────

/// A capture read back off disk.
///
/// The polygon is rebuilt with [`Polygon2::with_holes_closed`] and **no**
/// `ensure_winding` — the captured vertex order is preserved exactly, so the
/// sentry compares against untransformed data.
struct CapturedRegion {
    polygon: Polygon2,
    area_mm2: f64,
    hole_count: usize,
    exterior_vertex_count: usize,
    hole_vertex_counts: Vec<usize>,
}

/// Load `test_data/wanaka_region1_boundary_f1.json`. `None` when the artifact
/// is absent or unreadable — a later F1 evidence test that cannot `use` this
/// module is expected to restate these ~20 lines rather than depend on it.
fn load_region1_capture() -> Option<CapturedRegion> {
    let raw = std::fs::read_to_string(capture_path()).ok()?;
    let v: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let ring = |val: &serde_json::Value| -> Vec<P2> {
        val.as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|p| Some(P2::new(p[0].as_f64()?, p[1].as_f64()?)))
                    .collect()
            })
            .unwrap_or_default()
    };
    let exterior = ring(&v["exterior"]);
    let holes: Vec<Vec<P2>> = v["holes"]
        .as_array()
        .map(|a| a.iter().map(ring).collect())
        .unwrap_or_default();
    let closed = v["closed"].as_bool().unwrap_or(true);
    let integrity = &v["integrity"];
    let hole_vertex_counts: Vec<usize> = integrity["hole_vertex_counts"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|n| n.as_u64().unwrap_or_default() as usize)
                .collect()
        })
        .unwrap_or_default();
    (exterior.len() >= 3).then(|| CapturedRegion {
        polygon: Polygon2::with_holes_closed(exterior, holes, closed),
        area_mm2: integrity["area_mm2"].as_f64().unwrap_or(f64::NAN),
        hole_count: integrity["hole_count"].as_u64().unwrap_or_default() as usize,
        exterior_vertex_count: integrity["exterior_vertex_count"]
            .as_u64()
            .unwrap_or_default() as usize,
        hole_vertex_counts,
    })
}

// ── tests ───────────────────────────────────────────────────────────────

/// Cut the artifact. Run once; the JSON is committed.
///
/// Region 1's area is expected to be **~3104 mm²**
/// (`planning/thin_organic_2026-08-27/FINDINGS.md` — the 875.9 s / 1053.7 s
/// tables all quote that region). It is reported, deliberately NOT asserted:
/// this test's job is to record what the chain produces, and a hard bar here
/// would turn a legitimate deliberate re-cut into a red capture run. The
/// SENTRY below is where drift is caught.
#[test]
#[ignore = "capture — needs the operator's wanaka mesh (not in repo)"]
fn capture_wanaka_region_1_boundary() {
    let path = Path::new(WANAKA_MESH);
    if !path.exists() {
        eprintln!("SKIP: {WANAKA_MESH} not present on this machine.");
        return;
    }
    let derived = match derive_region_1(path) {
        Ok(derived) => derived,
        Err(reason) => {
            eprintln!("SKIP: {reason}.");
            return;
        }
    };

    let poly = &derived.boundary;
    eprintln!(
        "region 1: {:.1} mm² (expected ~3104 mm² per thin_organic FINDINGS), \
         {} exterior vertices, {} holes",
        poly.area(),
        poly.exterior.len(),
        poly.holes.len()
    );
    eprintln!(
        "chain: {} triangles, classification cell {:.6} mm, tier-1 stepover {:.6} mm, \
         {} planned regions ({} Shallow)",
        derived.mesh_triangle_count,
        derived.classification_cell_mm,
        derived.stepover_mm,
        derived.planned_region_count,
        derived.shallow_region_count
    );

    let out = capture_path();
    std::fs::write(&out, capture_json(&derived)).expect("write capture");
    eprintln!("wrote {}", out.display());
}

/// **Drift sentry.** The committed capture must still be what the chain
/// produces.
///
/// Skips (never fails) on either missing input: the mesh is machine-local, and
/// the artifact does not exist until the capture above has been run once.
#[test]
#[ignore = "drift sentry — needs the operator's wanaka mesh (not in repo)"]
fn wanaka_region_1_capture_matches_recomputation() {
    const DRIFT: &str = "The region-1 derivation chain has DRIFTED: the committed capture at \
         test_data/wanaka_region1_boundary_f1.json no longer matches what the \
         chain produces. Region 1's identity is what every F1 comparison and \
         the 875.9 s / 1053.7 s reference tables are anchored to, so this is \
         NOT to be fixed by re-running the capture casually. Find what moved \
         (tier map, tier islands, classification surface, finish planner, or a \
         cutter definition), decide deliberately whether the new region is the \
         one F1 should measure, and if so re-cut the capture AND re-cut every \
         downstream F1 evidence run against it.";
    // Absorbs JSON float round-trip only — the chain is assumed run-to-run
    // deterministic, so anything above this is real drift.
    const EPS: f64 = 1e-9;
    // Relative tolerance on the area cross-check.
    const AREA_REL: f64 = 1e-6;

    let path = Path::new(WANAKA_MESH);
    if !path.exists() {
        eprintln!("SKIP: {WANAKA_MESH} not present on this machine.");
        return;
    }
    let Some(captured) = load_region1_capture() else {
        eprintln!(
            "SKIP: no capture on disk at {} — run capture_wanaka_region_1_boundary first.",
            capture_path().display()
        );
        return;
    };
    // NOT a skip arm. The capture test skips when the chain yields nothing,
    // because there is then nothing to record. Here a capture already exists
    // on disk, which is proof the chain ONCE produced a region — so the chain
    // now producing none is the loudest drift there is, and letting it print
    // SKIP would make maximal drift read as no signal.
    let derived = match derive_region_1(path) {
        Ok(derived) => derived,
        Err(reason) => panic!(
            "the chain no longer produces a region at all ({reason}), yet a \
             capture exists on disk — so it produced one when the capture was \
             cut.\n{DRIFT}"
        ),
    };
    let fresh = &derived.boundary;

    assert_eq!(
        fresh.exterior.len(),
        captured.exterior_vertex_count,
        "exterior vertex count: recomputed {} vs captured {}.\n{DRIFT}",
        fresh.exterior.len(),
        captured.exterior_vertex_count
    );
    assert_eq!(
        fresh.exterior.len(),
        captured.polygon.exterior.len(),
        "captured file is internally inconsistent: integrity.exterior_vertex_count \
         disagrees with the exterior ring it ships.\n{DRIFT}"
    );
    assert_eq!(
        fresh.holes.len(),
        captured.hole_count,
        "hole count: recomputed {} vs captured {}.\n{DRIFT}",
        fresh.holes.len(),
        captured.hole_count
    );
    assert_eq!(
        fresh.holes.len(),
        captured.polygon.holes.len(),
        "captured file is internally inconsistent: integrity.hole_count disagrees \
         with the number of hole rings it ships.\n{DRIFT}"
    );
    assert_eq!(
        fresh.closed, captured.polygon.closed,
        "closed flag: recomputed {} vs captured {}.\n{DRIFT}",
        fresh.closed, captured.polygon.closed
    );

    let exterior_pairs = fresh.exterior.iter().zip(captured.polygon.exterior.iter());
    for (i, (a, b)) in exterior_pairs.enumerate() {
        assert!(
            (a.x - b.x).abs() <= EPS && (a.y - b.y).abs() <= EPS,
            "exterior vertex {i}: recomputed ({}, {}) vs captured ({}, {}), \
             beyond the {EPS:e} round-trip epsilon.\n{DRIFT}",
            a.x,
            a.y,
            b.x,
            b.y
        );
    }

    let hole_pairs = fresh.holes.iter().zip(captured.polygon.holes.iter());
    for (h, (fresh_hole, captured_hole)) in hole_pairs.enumerate() {
        assert_eq!(
            fresh_hole.len(),
            captured_hole.len(),
            "hole {h} vertex count: recomputed {} vs captured {}.\n{DRIFT}",
            fresh_hole.len(),
            captured_hole.len()
        );
        assert_eq!(
            captured.hole_vertex_counts.get(h).copied(),
            Some(fresh_hole.len()),
            "captured file is internally inconsistent at hole {h}: \
             integrity.hole_vertex_counts disagrees with the ring it ships.\n{DRIFT}"
        );
        for (i, (a, b)) in fresh_hole.iter().zip(captured_hole.iter()).enumerate() {
            assert!(
                (a.x - b.x).abs() <= EPS && (a.y - b.y).abs() <= EPS,
                "hole {h} vertex {i}: recomputed ({}, {}) vs captured ({}, {}), \
                 beyond the {EPS:e} round-trip epsilon.\n{DRIFT}",
                a.x,
                a.y,
                b.x,
                b.y
            );
        }
    }

    let fresh_area = fresh.area();
    let denominator = captured.area_mm2.abs().max(1e-12);
    let relative = (fresh_area - captured.area_mm2).abs() / denominator;
    assert!(
        relative <= AREA_REL,
        "area: recomputed {fresh_area:.6} mm² vs captured {:.6} mm² \
         (relative {relative:e} > {AREA_REL:e}).\n{DRIFT}",
        captured.area_mm2
    );

    eprintln!(
        "capture matches: {:.1} mm², {} exterior vertices, {} holes.",
        fresh_area,
        fresh.exterior.len(),
        fresh.holes.len()
    );
}
