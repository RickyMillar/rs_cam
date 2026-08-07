//! **The decisive M3 gate**: does switching the classification sampler make
//! the MACHINED SURFACE worse?
//!
//! `CLASSIFICATION_PERF_STUDY.md` §8 is explicit that M3's switch is not a
//! performance change — it moves 1.8–4.3% of classification cells across a
//! band boundary, which changes which operation owns which territory, which
//! changes toolpaths. §9.3 gate 7 names the only measurement that can
//! adjudicate that: an end-to-end `UnifiedFinish` A/B on the terrain fixture,
//! **scored on `SimulationResult::column_deviations`** (COLUMNS) rather than
//! on cell counts. Cell counts say the labels moved; only the machined
//! surface says whether that was an improvement.
//!
//! Branch A pins [`ClassificationSampler::DropCutterProbe`] — the pre-wave-7b
//! production classifier. Branch B pins
//! [`ClassificationSampler::TileRaster`] — what production runs now. Same
//! mesh, same tool, same stock, same dials, same simulation resolution, same
//! process, one after the other.
//!
//! # Why this harness builds its own fixture
//!
//! It does **not** load `planning/airrun_2026-06-01/wanaka.toml`. That file is
//! a live, user-modified project; reading it as a dependency makes a gate
//! whose verdict changes when somebody drags a slider (see
//! `wanaka_suggest_baseline`, permanently red for exactly that reason). The
//! fixture here is `tests/fixtures/terrain.stl` — the same wanaka-class
//! export the M3 study measured, committed to the repo — and the operation
//! config is built in-test from the `common::session` helpers.
//!
//! # The window, and why the footprint is cropped
//!
//! The classifier decision lives at the PRODUCTION cell size: `cusp/4` =
//! 0.125 mm for the shipped Ø1-tip taper, which is the 849² grid the study's
//! headline rows use. Everything that makes the switch discriminating — the
//! probe's 10–25 µm offset against a 0.125 mm cell — is a property of that
//! scale, so the tool and the cell must NOT be coarsened to make the run
//! cheaper.
//!
//! What can be reduced without touching the scale under test is the
//! FOOTPRINT. `M3_AB_WINDOW_MM` (default 30 mm, `0` = the full 100 mm
//! terrain) crops the mesh to a centred window, keeping the tool, the cell,
//! the slope statistics and the band mix, and cutting the generation +
//! simulation cost by the area ratio. The mix table this harness prints is
//! what proves the cropped fixture still exercises every band; a run whose
//! mix has collapsed to one strategy is reported as vacuous rather than
//! passed.
//!
//! ```text
//! cargo test -p rs_cam_core --test classification_columns_ab_m3 --release \
//!     -- --ignored --nocapture --test-threads=1
//! ```
//!
//! Knobs: `M3_AB_WINDOW_MM` (mm, default 30), `M3_AB_SIM_MM` (mm, default
//! 0.1 — the tip-matched measurement grid; a Ø1 tip has a 0.5 mm tip radius,
//! and `feedback_rest_measurement_prerequisites` requires the sim cell to sit
//! well under the TIP radius or the instrument aliases away what it is
//! measuring).
//!
//! # Measured, wave 7b, 2026-08-02 — ADOPT on both windows
//!
//! | | 30 mm window | full 100 mm terrain |
//! |---|---|---|
//! | common columns | 86 637 | 1 001 987 |
//! | p50 \|dev\| A→B | 62.39 → 61.74 µm (−0.65) | 28.59 → 28.69 µm (**+0.11**) |
//! | p90 \|dev\| A→B | 302.01 → 300.53 µm (−1.48) | 219.03 → 223.47 µm (**+4.44**) |
//! | on-size ±10 µm | 13.030 → 12.892% (−0.139 pp) | 26.665 → 26.570% (−0.095 pp) |
//! | on-size ±25 µm | 27.155 → 27.154% (−0.001 pp) | 47.596 → 47.514% (−0.082 pp) |
//! | max \|dev\| (reported) | 2150.7 → 2491.9 µm | 3870.2 → 3990.4 µm |
//! | rapid collisions | 0 → 0 | 0 → 0 |
//! | VerySteep territory | 647.8 → 688.8 mm² (+6.3%) | 3343.1 → 3427.6 mm² (+2.5%) |
//! | generation wall | 5.9 → 5.7 s | **249.0 → 192.9 s (−22.5%)** |
//! | cutting time | 1124.1 → 1096.5 s (−2.5%) | 7188.8 → 7588.5 s (**+5.6%**) |
//!
//! Every gated statistic clears its tolerance by an order of magnitude on
//! both windows, in both directions — this is a wash on quality, not a win.
//! Three things are worth stating rather than burying:
//!
//! * **The classifier's own 187× shows up as −22.5% of whole-op generation
//!   wall on the full terrain** (249.0 s → 192.9 s). That is the M3 speed
//!   claim landing where a user can feel it.
//! * **B costs +5.6% cutting time on the full terrain.** It finds 84.5 mm²
//!   more very-steep territory (§9.2 risk 2, predicted), and waterline is the
//!   most expensive strategy per area. Quality did not pay for it; the clock
//!   did.
//! * **`max` got worse on both windows** (+341 µm, +120 µm). It is one column
//!   out of a million, it is leftover rather than gouge, and it is not gated
//!   — but it is not hidden either.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

mod common;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use rs_cam_core::classify_probe::ClassificationSampler;
use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::UnifiedFinishConfig;
use rs_cam_core::compute::simulate::ColumnDeviation;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::geo::P3;
use rs_cam_core::mesh::TriangleMesh;
use rs_cam_core::semantic_trace::{SemanticKey, ToolpathSemanticKind};
use rs_cam_core::session::{ProjectSession, SimulationOptions};

use common::session::{mesh_model, pinned_heights, single_op_session_with};

// ── Fixture ─────────────────────────────────────────────────────────────

/// The wanaka-class finisher this study measured throughout: Ø1 TIP on a Ø6
/// SHANK, 7° half angle. `cusp_radius()` = 0.5 mm, so the classification cell
/// is `0.5/4` = 0.125 mm — the production scale the switch was decided at.
const TIP_DIAMETER_MM: f64 = 1.0;
const SHANK_DIAMETER_MM: f64 = 6.0;
const TAPER_HALF_ANGLE_DEG: f64 = 7.0;

/// Default cropped window (mm). `M3_AB_WINDOW_MM=0` runs the full terrain.
const DEFAULT_WINDOW_MM: f64 = 30.0;

/// Default measurement resolution (mm) — see the module doc.
const DEFAULT_SIM_MM: f64 = 0.1;

fn env_f64(key: &str, fallback: f64) -> f64 {
    match std::env::var(key) {
        Ok(raw) => raw
            .parse()
            .unwrap_or_else(|e| panic!("{key} must be a number, got {raw:?}: {e}")),
        Err(_) => fallback,
    }
}

fn terrain_path() -> PathBuf {
    let path = common::repo_root().join("crates/rs_cam_core/tests/fixtures/terrain.stl");
    assert!(
        path.exists(),
        "the M3 COLUMNS A/B needs the committed terrain fixture at {}",
        path.display()
    );
    path
}

/// Load the terrain and, when `window_mm > 0`, crop it to a centred
/// `window_mm` square.
///
/// A triangle is kept only when **all three** of its vertices are inside the
/// window, so the crop never produces a partial facet whose plane extends
/// past the boundary — the cropped mesh is a proper closed-in-XY heightfield
/// over a smaller footprint, not the original with holes chewed in its edge.
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
        triangles.len() > 1000,
        "the {window_mm} mm crop kept only {} triangle(s) — the window is smaller than the \
         terrain's facet spacing and the fixture would be a flat plate",
        triangles.len()
    );
    TriangleMesh::from_raw(vertices, triangles)
}

fn tapered_ball_tool() -> ToolConfig {
    ToolConfig {
        diameter: TIP_DIAMETER_MM,
        taper_half_angle: TAPER_HALF_ANGLE_DEG,
        shaft_diameter: SHANK_DIAMETER_MM,
        ..ToolConfig::new_default(ToolId(0), ToolType::TaperedBallNose)
    }
}

/// Stock flush with the model's own bbox in Z, padded 2 mm in XY.
///
/// Flush on top is deliberate: an unroughed block whose top sits above the
/// highest peak would make both branches spend the entire run removing
/// material no finish strategy is responsible for, and would put the
/// deviation population somewhere neither classifier decides.
fn stock_for(mesh: &TriangleMesh) -> StockConfig {
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

/// The A/B operation config.
///
/// Representative of the live wanaka finisher, restated for a Ø1 TIP rather
/// than the Ø6 ball the P2 harnesses used: `raster_stepover` 0.3 mm, and
/// `scallop_height` set to the cusp that stepover leaves on a 0.5 mm tip
/// (`0.3² / (8 · 0.5)` = 0.0225 mm), so the two bands ask for the same
/// surface finish and a band boundary moving between them is not also a
/// quality change. Thresholds are the shipped 45/75.
///
/// `pencil_claims` is off: the claims pipeline's own A/B measured it
/// self-defeating for a single-tool op, and it would add a second moving
/// part to a comparison whose only variable must be the classifier.
fn ab_config(sampler: ClassificationSampler) -> UnifiedFinishConfig {
    UnifiedFinishConfig {
        steep_threshold_deg: 45.0,
        waterline_threshold_deg: 75.0,
        overlap_mm: 2.0,
        // 0.3² / (8 · 0.5): the flat-surface cusp `raster_stepover` leaves on
        // this tip. Spelled as the arithmetic so the parity is checkable.
        scallop_height: 0.3 * 0.3 / (8.0 * 0.5 * TIP_DIAMETER_MM),
        tolerance: 0.05,
        raster_stepover: 0.3,
        z_step: 0.3,
        sampling: 0.5,
        stock_to_leave: 0.0,
        feed_rate: 3000.0,
        plunge_rate: 150.0,
        spindle_rpm: Some(21000),
        pencil_claims: false,
        classification_sampler: sampler,
        ..UnifiedFinishConfig::default()
    }
}

// ── Scoring ─────────────────────────────────────────────────────────────

/// COLUMNS quality for one branch, over a stated column population.
#[derive(Debug, Clone, Copy)]
struct Columns {
    n: usize,
    p50_abs_um: f64,
    p90_abs_um: f64,
    max_abs_um: f64,
    /// Share of columns within ±10 µm and ±25 µm of the model.
    on_size_10um: f64,
    on_size_25um: f64,
    /// Signed extremes: the worst overcut (gouge) and the worst leftover.
    worst_overcut_um: f64,
    worst_leftover_um: f64,
}

fn quantile(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let idx = ((sorted.len() - 1) as f64 * q).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
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

/// Index a branch's columns by `(row, col)`.
///
/// **Never by XY.** `ColumnDeviation` carries its own grid indices precisely
/// so a cross-branch comparison does not have to invert a transform to line
/// two populations up (P2.g Task 1's lesson, `MEMORY.md`). The two branches
/// share stock and resolution, so the grids are the same grid.
fn by_cell(cols: &[ColumnDeviation]) -> HashMap<(usize, usize), f64> {
    cols.iter()
        .map(|c| ((c.row, c.col), f64::from(c.dev)))
        .collect()
}

// ── Branch run ──────────────────────────────────────────────────────────

struct Branch {
    label: &'static str,
    sampler: ClassificationSampler,
    columns: Vec<ColumnDeviation>,
    collisions: usize,
    cutting_s: f64,
    gen_s: f64,
    /// (band, strategy) → (region count, move count, XY area mm²).
    mix: Vec<(String, String, usize, usize, f64)>,
    region_nodes: usize,
}

fn run_branch(
    label: &'static str,
    sampler: ClassificationSampler,
    mesh: &TriangleMesh,
    sim_mm: f64,
) -> Branch {
    let top_z = mesh.bbox.max.z;
    let bottom_z = mesh.bbox.min.z;
    let mut session = single_op_session_with(
        stock_for(mesh),
        tapered_ball_tool(),
        mesh_model(mesh.clone(), "terrain"),
        "M3 Unified Finish",
        OperationConfig::UnifiedFinish(ab_config(sampler)),
        |cfg| {
            // Surface ops carry no depth dial: `bottom_z: Auto` would resolve
            // to `top_z - op_depth` and flatten the waterline band's Z range.
            cfg.heights = pinned_heights(top_z, bottom_z);
        },
    );

    let cancel = AtomicBool::new(false);
    let t0 = Instant::now();
    session
        .generate_toolpath(0, &cancel)
        .unwrap_or_else(|e| panic!("branch {label}: UnifiedFinish generation failed: {e:?}"));
    let gen_s = t0.elapsed().as_secs_f64();

    let mix = region_mix(&session);
    let region_nodes = mix.iter().map(|r| r.2).sum();

    let opts = SimulationOptions {
        resolution: sim_mm,
        ..Default::default()
    };
    session
        .run_simulation(&opts, &cancel)
        .unwrap_or_else(|e| panic!("branch {label}: measurement simulation failed: {e:?}"));
    let sim = session.simulation_result().expect("simulation result");
    assert!(
        !sim.resolution_clamped,
        "branch {label}: the {sim_mm} mm measurement grid was clamped to {} mm — the two \
         branches would be scored on different column populations",
        sim.column_grid_cell_mm
    );
    let columns = sim
        .column_deviations
        .as_ref()
        .expect("column_deviations (simulation ran without a reference model mesh?)")
        .clone();
    let collisions = sim.rapid_collisions.len();

    // Project runtime, kinematics-integrated where the profile carries it.
    // Reported beside the quality verdict, never gated on: M3's speed claim
    // is about the CLASSIFIER (§4), not about the toolpath it plans.
    let cutting_s = session.diagnostics().total_runtime_s;

    Branch {
        label,
        sampler,
        columns,
        collisions,
        cutting_s,
        gen_s,
        mix,
        region_nodes,
    }
}

/// Region mix from the op's own semantic trace: which band ran which
/// strategy, over how many regions, moves and XY-projected mm².
///
/// §9.3 gate 7 asks for this table beside the COLUMNS verdict, because the
/// verdict alone cannot say *why* a branch differs. It is also the vacuity
/// check on the cropped fixture: a mix that has collapsed to one strategy
/// would make the classifier switch unobservable by construction.
fn region_mix(session: &ProjectSession) -> Vec<(String, String, usize, usize, f64)> {
    let Some(result) = session.get_result(0) else {
        return Vec::new();
    };
    let Some(trace) = result.semantic_trace.as_ref() else {
        return Vec::new();
    };
    let mut acc: HashMap<(String, String), (usize, usize, f64)> = HashMap::new();
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
        let moves = item
            .move_end
            .zip(item.move_start)
            .map_or(0, |(e, s)| e.saturating_sub(s) + 1);
        let entry = acc
            .entry((text(SemanticKey::Band), text(SemanticKey::Strategy)))
            .or_default();
        entry.0 += 1;
        entry.1 += moves;
        entry.2 += area;
    }
    let mut rows: Vec<_> = acc
        .into_iter()
        .map(|((band, strategy), (regions, moves, area))| (band, strategy, regions, moves, area))
        .collect();
    rows.sort_by(|a, b| b.3.cmp(&a.3));
    rows
}

// ── Rendering (the v3 rule: never gate on an aggregate alone) ────────────

fn out_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("m3_columns_ab");
    std::fs::create_dir_all(&dir).expect("create target/m3_columns_ab");
    dir
}

/// Top-down render of a per-cell signed value, red = overcut, blue =
/// leftover, grey = on-size, black = no column.
///
/// The v3 campaign closed with the rule *never gate on an aggregate without
/// rendering the surface* — an on-size percentage ranked a branch above one
/// that had left a 28 mm block standing. These PNGs are how a reader checks
/// the verdict against the thing it claims to measure.
fn render(cells: &HashMap<(usize, usize), f64>, scale_um: f64, name: &str) {
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
        // Image row 0 is the TOP of the picture; grid row 0 is min-Y.
        let i = ((rows - 1 - r) * cols + c) * 4;
        px[i] = rr;
        px[i + 1] = gg;
        px[i + 2] = bb;
        px[i + 3] = 255;
    }
    let path = out_dir().join(format!("{name}.png"));
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

fn report(b: &Branch, s: Columns) {
    eprintln!("\n== branch {} — {} ==", b.label, b.sampler.label());
    eprintln!(
        "  generation {:.1}s | cutting {:.1}s | rapid collisions {} | region nodes {}",
        b.gen_s, b.cutting_s, b.collisions, b.region_nodes
    );
    eprintln!("  | band | strategy | regions | moves | XY area mm² |");
    eprintln!("  |---|---|---|---|---|");
    for (band, strategy, regions, moves, area) in &b.mix {
        eprintln!("  | {band} | {strategy} | {regions} | {moves} | {area:.1} |");
    }
    eprintln!(
        "  COLUMNS n={} p50 {:.1}µm p90 {:.1}µm max {:.1}µm | on-size ±10µm {:.2}% ±25µm {:.2}% \
         | worst overcut {:.1}µm leftover {:.1}µm",
        s.n,
        s.p50_abs_um,
        s.p90_abs_um,
        s.max_abs_um,
        100.0 * s.on_size_10um,
        100.0 * s.on_size_25um,
        s.worst_overcut_um,
        s.worst_leftover_um,
    );
}

// ── Pre-registered verdict thresholds ───────────────────────────────────
//
// Written down BEFORE the run, and justified against a physical scale rather
// than against whatever the run produces. The dials above ask for a 22.5 µm
// cusp (`0.3²/(8·0.5)`), so:
//
//  * p50 may not rise by more than 5 µm — under a quarter of the cusp the
//    operation is deliberately leaving;
//  * p90 may not rise by more than 10 µm — under half of it;
//  * the ±25 µm on-size share may not fall by more than 0.5 pp, which on a
//    ~10⁵-column population is thousands of columns, not sampling noise;
//  * rapid collisions may not increase at all.
//
// `max` is REPORTED, not gated: it is one column, and the v3 campaign's
// standing rule is that a single extreme is not a distribution.

const P50_TOLERANCE_UM: f64 = 5.0;
const P90_TOLERANCE_UM: f64 = 10.0;
const ON_SIZE_TOLERANCE_PP: f64 = 0.5;

#[test]
#[ignore = "M3 decisive gate: two UnifiedFinish generations + two tip-matched dexel sims. \
            Run --release, --test-threads=1, on a quiet machine."]
fn m3_columns_ab_shipped_vs_production_classifier() {
    let window_mm = env_f64("M3_AB_WINDOW_MM", DEFAULT_WINDOW_MM);
    let sim_mm = env_f64("M3_AB_SIM_MM", DEFAULT_SIM_MM);
    let mesh = terrain(window_mm);
    eprintln!(
        "fixture: terrain.stl cropped to {window_mm} mm ({} triangles), bbox {:?}..{:?}",
        mesh.faces.len(),
        mesh.bbox.min,
        mesh.bbox.max
    );
    eprintln!(
        "measurement grid: {sim_mm} mm (tip radius {} mm)",
        TIP_DIAMETER_MM / 2.0
    );

    let a = run_branch(
        "A (shipped)",
        ClassificationSampler::DropCutterProbe,
        &mesh,
        sim_mm,
    );
    let b = run_branch(
        "B (production)",
        ClassificationSampler::TileRaster,
        &mesh,
        sim_mm,
    );

    // Score on the COMMON column population. The two branches machine
    // differently, so the relevance filter can admit a column in one run and
    // not the other; comparing p50s over two different populations compares
    // two different questions.
    let cells_a = by_cell(&a.columns);
    let cells_b = by_cell(&b.columns);
    let common: Vec<(usize, usize)> = cells_a
        .keys()
        .filter(|k| cells_b.contains_key(*k))
        .copied()
        .collect();
    assert!(
        !common.is_empty(),
        "the two branches share no dexel column at all — they were not simulated on the \
         same grid, and nothing below means anything"
    );
    let devs_a: Vec<f64> = common.iter().map(|k| cells_a[k]).collect();
    let devs_b: Vec<f64> = common.iter().map(|k| cells_b[k]).collect();
    let sa = score(&devs_a);
    let sb = score(&devs_b);

    report(&a, sa);
    report(&b, sb);
    eprintln!(
        "\ncolumn population: A {}, B {}, common {} ({:.2}% of A)",
        a.columns.len(),
        b.columns.len(),
        common.len(),
        100.0 * common.len() as f64 / a.columns.len().max(1) as f64
    );

    // Render both surfaces and their difference before any verdict is read.
    render(&cells_a, 100.0, "branch_a_shipped_dev");
    render(&cells_b, 100.0, "branch_b_production_dev");
    let diff: HashMap<(usize, usize), f64> = common
        .iter()
        .map(|&k| (k, cells_b[&k] - cells_a[&k]))
        .collect();
    render(&diff, 50.0, "branch_b_minus_a");

    // ── Non-vacuity ────────────────────────────────────────────────────
    // Two ways this comparison could be empty: the classifier could have
    // changed nothing (then there is no product decision to gate), or the
    // fixture could present only one band (then the classifier has nothing
    // to decide).
    let strategies_a: usize = a.mix.len();
    assert!(
        strategies_a >= 2,
        "branch A ran only {strategies_a} band/strategy combination(s) — the cropped fixture \
         does not exercise the band boundary the classifier moves, so this A/B is vacuous. \
         Raise M3_AB_WINDOW_MM."
    );
    let moved = diff.values().filter(|d| d.abs() > 1e-6).count();
    eprintln!(
        "surface difference: {moved} of {} common columns moved ({:.3}%)",
        common.len(),
        100.0 * moved as f64 / common.len() as f64
    );

    // ── Verdict ────────────────────────────────────────────────────────
    let d_p50 = sb.p50_abs_um - sa.p50_abs_um;
    let d_p90 = sb.p90_abs_um - sa.p90_abs_um;
    let d_on25 = 100.0 * (sb.on_size_25um - sa.on_size_25um);
    let d_on10 = 100.0 * (sb.on_size_10um - sa.on_size_10um);
    eprintln!("\n== M3 COLUMNS verdict (B production vs A shipped) ==");
    eprintln!(
        "  p50 |dev|     A {:8.2}µm  B {:8.2}µm  Δ {d_p50:+8.2}µm (tolerance +{P50_TOLERANCE_UM})",
        sa.p50_abs_um, sb.p50_abs_um
    );
    eprintln!(
        "  p90 |dev|     A {:8.2}µm  B {:8.2}µm  Δ {d_p90:+8.2}µm (tolerance +{P90_TOLERANCE_UM})",
        sa.p90_abs_um, sb.p90_abs_um
    );
    eprintln!(
        "  max |dev|     A {:8.2}µm  B {:8.2}µm  (reported, not gated)",
        sa.max_abs_um, sb.max_abs_um
    );
    eprintln!(
        "  on-size ±10µm A {:7.3}%   B {:7.3}%   Δ {d_on10:+7.3} pp",
        100.0 * sa.on_size_10um,
        100.0 * sb.on_size_10um
    );
    eprintln!(
        "  on-size ±25µm A {:7.3}%   B {:7.3}%   Δ {d_on25:+7.3} pp (tolerance −{ON_SIZE_TOLERANCE_PP})",
        100.0 * sa.on_size_25um,
        100.0 * sb.on_size_25um
    );
    eprintln!("  collisions    A {}  B {}", a.collisions, b.collisions);
    eprintln!(
        "  cutting time  A {:.1}s  B {:.1}s ({:+.1}%)",
        a.cutting_s,
        b.cutting_s,
        100.0 * (b.cutting_s - a.cutting_s) / a.cutting_s.max(1e-9)
    );

    assert!(
        b.collisions <= a.collisions,
        "SAFETY: the production classifier produced {} rapid collisions against the shipped \
         classifier's {}",
        b.collisions,
        a.collisions
    );
    assert!(
        d_p50 <= P50_TOLERANCE_UM,
        "COLUMNS REGRESSION: median deviation rose {d_p50:.2} µm (tolerance {P50_TOLERANCE_UM}). \
         Per the checkpoint ruling this means production falls back to the shipped classifier."
    );
    assert!(
        d_p90 <= P90_TOLERANCE_UM,
        "COLUMNS REGRESSION: p90 deviation rose {d_p90:.2} µm (tolerance {P90_TOLERANCE_UM}). \
         Per the checkpoint ruling this means production falls back to the shipped classifier."
    );
    assert!(
        d_on25 >= -ON_SIZE_TOLERANCE_PP,
        "COLUMNS REGRESSION: the ±25 µm on-size share fell {:.3} pp (tolerance \
         {ON_SIZE_TOLERANCE_PP}). Per the checkpoint ruling this means production falls back \
         to the shipped classifier.",
        -d_on25
    );
    eprintln!("\nVERDICT: ADOPT — the production classifier is no worse on COLUMNS.");
}
