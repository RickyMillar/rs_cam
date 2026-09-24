//! The cutting-metrics carve and the plain carve leave the SAME stock.
//!
//! Operator ruling 2026-09-24 ("mcp and gui should cut the same"). The
//! simulator has two carve routes: the metric route (the CLI and MCP always
//! take it) and the plain playback route (the GUI takes it when "Capture
//! cutting metrics" is off). `dexel_stock/CLAUDE.md` states that they "must
//! agree on the stamped volume"; the G-RESTRES parity fixture measured that
//! they did not. Metrics must only observe the carve, never change it.
//!
//! The fixture is the parity project: a pocket, then a rest pocket, at a
//! stored 0.5 mm cell. The test carves the pocket both ways and compares
//! the rest pocket's `prior_stocks` snapshot bit for bit.
//!
//! Red before the fix: 15 Z rays differed, by up to 2.2 mm, on the ring the
//! pocket's first plunge leaves at (8, 32). The metric-off group carve took
//! the playback replay route; it now takes the one milling carve, and the
//! samples are dropped instead (`compute/simulate.rs::carve_entry`).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::toolpath_stats::StockSnapshotStamp;
use rs_cam_core::dexel_stock::TriDexelStock;
use rs_cam_core::session::{ProjectSession, SimulationOptions};

fn fixture() -> ProjectSession {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/rest_cascade_g_restres/project.toml");
    let mut session = ProjectSession::load(&path).expect("the fixture loads");
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("the pocket generates");
    session
}

/// The rest pocket's snapshot after a simulation with metrics on or off.
fn snapshot(metrics: bool) -> std::sync::Arc<TriDexelStock> {
    let mut session = fixture();
    let rest_id = session.toolpath_configs()[1].id;
    let cancel = AtomicBool::new(false);
    let opts = SimulationOptions {
        resolution: session.simulation_resolution_mm(),
        metrics_enabled: metrics,
        adaptive_feed_modulation: false,
        ..SimulationOptions::default()
    };
    let sim = session.run_simulation(&opts, &cancel).expect("simulate");
    std::sync::Arc::clone(
        sim.prior_stocks
            .get(&rest_id)
            .expect("the scan gives the rest pocket a snapshot"),
    )
}

/// How many Z rays differ, and the largest difference in any segment
/// bound, in mm. The number that says WHERE to look when the stamps differ.
fn ray_difference(a: &TriDexelStock, b: &TriDexelStock) -> (usize, f64) {
    let mut rays = 0;
    let mut worst = 0.0_f64;
    for (ra, rb) in a.z_grid.rays.iter().zip(&b.z_grid.rays) {
        if ra.len() != rb.len() {
            rays += 1;
            worst = f64::INFINITY;
            continue;
        }
        let mut differs = false;
        for (sa, sb) in ra.iter().zip(rb.iter()) {
            let d = f64::from((sa.enter - sb.enter).abs().max((sa.exit - sb.exit).abs()));
            if d > 0.0 {
                differs = true;
                worst = worst.max(d);
            }
        }
        if differs {
            rays += 1;
        }
    }
    (rays, worst)
}

#[test]
fn the_metric_and_plain_carves_leave_the_same_stock() {
    let metric = snapshot(true);
    let plain = snapshot(false);
    let (rays, worst) = ray_difference(&metric, &plain);
    assert_eq!(
        StockSnapshotStamp::of(&metric),
        StockSnapshotStamp::of(&plain),
        "the two carve routes leave different stock: {rays} Z rays differ, \
         by up to {worst} mm"
    );
}
