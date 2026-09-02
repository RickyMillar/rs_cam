//! M1 — the union-coverage audit's first consumer (Track M, G-UNIONCOV).
//!
//! Runs the whole generate → simulate fixpoint headless on two project
//! files and judges each final stock with
//! `rs_cam_core::metrology::union_coverage::audit_stock_vs_model`:
//!
//! * ARM PRODUCTION — `planning/multitool_2026-08-23/wanaka200_mt2.toml`
//!   (the production two-tool tier chain, overlap 2.0 mm).
//! * ARM REJECTED — `wanaka200_mt2_overlap02.toml` (overlap 0.2 mm,
//!   region floors 100/50 mm²) — the variant that read clean on every
//!   per-op wire while leaving standing patches
//!   (`planning/finishing_status_2026-09-01.md` §12).
//!
//! Pre-registered expectations and dials:
//! `planning/metrology_2026-09-02/FINDINGS.md` §M-5, written before the
//! first run. The variant MUST fail `assert_within(500 mm²)`; production
//! is characterized, not gated, with the registered discriminator that
//! the variant's above-spec area exceeds production's by at least 3×.
//!
//! Evidence run:
//! ```text
//! cargo test --release -p rs_cam_core --test union_coverage_m1 -- --ignored --nocapture
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
#![allow(clippy::print_stderr)]

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use rs_cam_core::geo::P3;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::metrology::union_coverage::{
    UnionCoverageParams, UnionCoverageReport, audit_stock_vs_model,
};
use rs_cam_core::session::{ProjectSession, SimulationOptions};

/// Pinned dials — registered in FINDINGS §M-5 before the first run.
const RESOLUTION_MM: f64 = 0.3;
const SPEC_TOLERANCE_MM: f64 = 0.35;
const GOUGE_TOLERANCE_MM: f64 = 0.35;
const VARIANT_ALLOWED_MM2: f64 = 500.0;
const DISCRIMINATOR_MIN_RATIO: f64 = 3.0;
const MAX_FIXPOINT_ROUNDS: usize = 6;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .to_path_buf()
}

/// Translate a mesh — the same `-stock_origin` map `run_simulation`
/// applies to `SimulationRequest::model_mesh`, restated here because the
/// session helper is private.
fn translated(mesh: &TriangleMesh, dx: f64, dy: f64, dz: f64) -> TriangleMesh {
    let vertices: Vec<P3> = mesh
        .vertices
        .iter()
        .map(|v| P3::new(v.x + dx, v.y + dy, v.z + dz))
        .collect();
    TriangleMesh::from_raw(vertices, mesh.triangles.clone())
}

fn run_arm(label: &str, project: &Path) -> UnionCoverageReport {
    eprintln!("\n════════ ARM {label}: {} ════════", project.display());
    let mut session = ProjectSession::load(project).expect("load project");
    let cancel = AtomicBool::new(false);
    let opts = SimulationOptions {
        resolution: RESOLUTION_MM,
        metrics_enabled: false,
        adaptive_feed_modulation: false,
        ..SimulationOptions::default()
    };

    // Generate → simulate fixpoint, so FromRemainingStock ops resolve.
    let enabled: Vec<usize> = session
        .toolpath_configs()
        .iter()
        .enumerate()
        .filter(|(_, tc)| tc.enabled)
        .map(|(i, _)| i)
        .collect();
    for round in 0..MAX_FIXPOINT_ROUNDS {
        let start = std::time::Instant::now();
        session.generate_all(&[], &cancel).expect("generate_all");
        session.run_simulation(&opts, &cancel).expect("simulate");
        let pending: Vec<usize> = enabled
            .iter()
            .copied()
            .filter(|&i| session.get_result(i).is_none())
            .collect();
        eprintln!(
            "  round {round}: {} of {} enabled generated ({:.0} s)",
            enabled.len() - pending.len(),
            enabled.len(),
            start.elapsed().as_secs_f64()
        );
        if pending.is_empty() {
            break;
        }
        assert!(
            round + 1 < MAX_FIXPOINT_ROUNDS,
            "fixpoint did not converge; still pending: {pending:?}"
        );
    }

    let sim = session.simulation_result().expect("simulation result");
    eprintln!(
        "  simulated at cell {:.3} mm (clamped: {})",
        sim.column_grid_cell_mm, sim.resolution_clamped
    );
    let last = sim.checkpoints.last().expect("final checkpoint");
    assert!(
        last.stock_local_to_global.is_none(),
        "final checkpoint is not in the global frame — lateral setup?"
    );

    // The model, in the stock's zero-rooted global frame.
    let model = session
        .models()
        .iter()
        .find_map(|m| m.mesh.clone())
        .expect("project has a mesh model");
    let stock = session.stock_config();
    let shifted = translated(&model, -stock.origin_x, -stock.origin_y, -stock.origin_z);
    let index = SpatialIndex::build_auto(&shifted);

    let params = UnionCoverageParams {
        stock_to_leave_mm: 0.0,
        spec_tolerance_mm: SPEC_TOLERANCE_MM,
        gouge_tolerance_mm: GOUGE_TOLERANCE_MM,
        max_hotspots: 12,
    };
    let report = audit_stock_vs_model(&last.stock, &shifted, &index, &params);
    eprintln!(
        "  UNION AUDIT ({label}): compared {:.0} mm2 ({} cells, {} off-model), cell {:.3} mm",
        report.compared_area_mm2, report.compared_cells, report.off_model_cells, report.cell_mm
    );
    eprintln!(
        "    above spec (> {SPEC_TOLERANCE_MM} mm): {:.1} mm2 = {:.3} % in {} patches; \
         max standing {:.3} mm, p99 {:.3} mm",
        report.above_spec_area_mm2,
        100.0 * report.above_spec_fraction,
        report.hotspot_count,
        report.max_standing_mm,
        report.p99_standing_mm
    );
    eprintln!(
        "    gouged (< -{GOUGE_TOLERANCE_MM} mm): {:.1} mm2; max undercut {:.3} mm; \
         cut-through columns {}",
        report.gouged_area_mm2, report.max_undercut_mm, report.cut_through_cells
    );
    for h in &report.hotspots {
        eprintln!(
            "    hotspot {:.1} mm2 at ({:.1}, {:.1}) bbox [{:.1}, {:.1}]..[{:.1}, {:.1}] \
             max standing {:.3} mm",
            h.area_mm2,
            h.centroid.x,
            h.centroid.y,
            h.bbox[0],
            h.bbox[1],
            h.bbox[2],
            h.bbox[3],
            h.max_standing_mm
        );
    }
    report
}

#[test]
#[ignore = "evidence run — full wanaka200 chains, release mode, tens of minutes per arm"]
fn union_coverage_judges_the_rejected_overlap_variant() {
    let root = repo_root().join("planning/multitool_2026-08-23");
    let production = run_arm("PRODUCTION", &root.join("wanaka200_mt2.toml"));
    let rejected = run_arm(
        "REJECTED overlap-0.2",
        &root.join("wanaka200_mt2_overlap02.toml"),
    );

    eprintln!("\n════════ VERDICT ════════");
    eprintln!(
        "  production above-spec {:.1} mm2 vs rejected {:.1} mm2 (ratio {:.2})",
        production.above_spec_area_mm2,
        rejected.above_spec_area_mm2,
        rejected.above_spec_area_mm2 / production.above_spec_area_mm2.max(1e-9)
    );

    // Pre-registered expectation 1: the rejected variant FAILS the audit.
    let failure = rejected
        .assert_within(VARIANT_ALLOWED_MM2)
        .expect_err("PRE-REGISTRATION 1 VIOLATED: the rejected variant passed the union audit");
    eprintln!("\n{failure}");

    // Pre-registered expectation 2: the discriminator.
    assert!(
        rejected.above_spec_area_mm2
            >= DISCRIMINATOR_MIN_RATIO * production.above_spec_area_mm2.max(1.0),
        "PRE-REGISTRATION 2 VIOLATED: rejected {:.1} mm2 is not {}x production {:.1} mm2",
        rejected.above_spec_area_mm2,
        DISCRIMINATOR_MIN_RATIO,
        production.above_spec_area_mm2
    );
}
