//! G-SIMMEM (2026-09-30) — no second copy of the cut trace at the peak.
//!
//! Besides the trace's own size (`sim_trace_sample_rate_g_simmem.rs`), three
//! transients multiplied it on the operator's machine
//! (`planning/sim_memory_2026-09-30/RESULTS.md`):
//!
//! 1. the S5 prefix memo cloned the whole running state — every cut sample —
//!    and only THEN checked its size ceiling, so a snapshot too big to keep
//!    was still built;
//! 2. `Vec::append` copied the finishing entry's samples into the run's
//!    (empty or smaller) buffer;
//! 3. the cut-trace artifact the GUI worker and `cli project` write was a
//!    deep `clone()` of the trace, serialised by `to_vec_pretty` into ONE
//!    in-memory document about four times the trace's size.
//!
//! This binary holds ONE test so no other test allocates while it measures.
//! It resets the kernel's peak-RSS mark (`/proc/self/clear_refs` = 5) before
//! each phase and reads `VmHWM` after it. The bounds are derived from the
//! retained trace itself: a simulation may peak at the trace it returns plus
//! half of it (grids and meshes of this small stock are a few MB), and
//! writing the artifact may add a quarter of it (a streamed writer holds one
//! buffer; a document in memory is ~4x). Red before the fix: phase 1 peaked
//! near 3x the trace and phase 2 near 5x.

#![cfg(target_os = "linux")]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::sim_prefix::{SimMemo, SimPrefixCache};
use rs_cam_core::compute::simulate::{
    SimGroupEntry, SimToolpathEntry, SimulationRequest, run_simulation_memoized,
};
use rs_cam_core::compute::tool_config::ToolMaterial;
use rs_cam_core::dexel_stock::StockCutDirection;
use rs_cam_core::geo::{BoundingBox3, P3};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::stock::simulation_cut::{
    SimulationCutArtifact, SimulationCutSample, write_simulation_cut_artifact_to,
};
use rs_cam_core::tool::{FlatEndmill, ToolDefinition};
use rs_cam_core::toolpath::Toolpath;
use rs_cam_core::trace::toolpath_spans::AnnotatedToolpath;

fn status_kb(field: &str) -> usize {
    let status = std::fs::read_to_string("/proc/self/status").expect("/proc/self/status");
    let line = status
        .lines()
        .find(|l| l.starts_with(field))
        .unwrap_or_else(|| panic!("no {field} in /proc/self/status"));
    line.split_whitespace().nth(1).unwrap().parse().unwrap()
}

/// Reset the peak-RSS mark and return the current RSS, in bytes.
fn reset_peak() -> usize {
    std::fs::write("/proc/self/clear_refs", "5")
        .expect("resetting VmHWM needs /proc/self/clear_refs (Linux >= 4.0)");
    let rss = status_kb("VmRSS:");
    assert!(
        status_kb("VmHWM:") <= rss + 1024,
        "clear_refs did not reset the peak mark"
    );
    rss * 1024
}

fn peak_bytes() -> usize {
    status_kb("VmHWM:") * 1024
}

/// A sample-heavy entry that stamps almost nothing: long rapids above a
/// small stock are sampled at the step and never stamped, so the trace is
/// ~200 k samples while grids and meshes stay a few MB.
fn rapid_heavy_entry() -> SimToolpathEntry {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, 5.0));
    for lap in 0..50 {
        let y = (lap % 20) as f64;
        tp.rapid_to(P3::new(500.0, y, 5.0));
        tp.rapid_to(P3::new(0.0, y + 0.5, 5.0));
    }
    tp.feed_to(P3::new(0.0, 1.0, -1.0), 600.0);
    tp.feed_to(P3::new(30.0, 1.0, -1.0), 600.0);
    SimToolpathEntry {
        id: ToolpathId(1),
        name: "rapids".to_owned(),
        annotated: Arc::new(AnnotatedToolpath::new(tp)),
        tool: Arc::new(ToolDefinition::new(
            Box::new(FlatEndmill::new(6.0, 25.0)),
            6.0,
            20.0,
            25.0,
            45.0,
            2,
            ToolMaterial::Carbide,
        )),
        flute_count: 2,
        tool_summary: "6mm Flat".to_owned(),
        semantic_trace: None,
        spindle_rpm: None,
        metrics_not_applicable: false,
        drill_op: None,
        operation_config_hash: 1,
    }
}

fn request() -> SimulationRequest {
    SimulationRequest {
        groups: vec![SimGroupEntry {
            toolpaths: vec![rapid_heavy_entry()],
            direction: StockCutDirection::FromTop,
            local_stock_bbox: None,
            local_to_global: None,
            phantom_prior_stock: None,
        }],
        stock_bbox: BoundingBox3 {
            min: P3::new(0.0, 0.0, -6.0),
            max: P3::new(40.0, 24.0, 0.0),
        },
        stock_top_z: 0.0,
        resolution: 0.25,
        spindle_rpm: 18_000,
        rapid_feed_mm_min: 5000.0,
        model_mesh: None,
        kinematics: None,
        display_stride: 1,
    }
}

fn trace_bytes(samples: &[SimulationCutSample]) -> usize {
    std::mem::size_of_val(samples)
        + samples
            .iter()
            .map(|s| s.span_path.capacity() * std::mem::size_of::<u32>())
            .sum::<usize>()
}

#[test]
fn a_simulation_and_its_artifact_hold_the_trace_once() {
    let req = request();

    // Phase 1: a memoised simulation whose snapshot is refused (the ceiling
    // is 0 bytes), the shape of every project over the memo ceiling.
    let mut cache = SimPrefixCache::new();
    cache.max_bytes = 0;
    let base = reset_peak();
    let result = run_simulation_memoized(
        &req,
        &AtomicBool::new(false),
        |_| {},
        Some(SimMemo {
            cache: &mut cache,
            store: true,
        }),
    )
    .expect("simulation");
    let sim_peak = peak_bytes().saturating_sub(base);
    let trace = result.cut_trace.as_ref().expect("metrics on");
    let retained = trace_bytes(&trace.samples);
    assert!(
        trace.samples.len() > 150_000,
        "the fixture must be sample-heavy, got {}",
        trace.samples.len()
    );
    assert_eq!(cache.stats().size_refusals, 1, "the memo must refuse");
    assert!(!cache.is_populated());
    assert!(
        sim_peak < retained + retained / 2,
        "simulation peaked {} MB above its start for a {} MB trace: a second copy of \
         the trace was built",
        sim_peak >> 20,
        retained >> 20
    );

    // Phase 2: the artifact write, as the GUI worker and `cli project` do it.
    let artifact = SimulationCutArtifact::new(
        req.resolution,
        trace.sample_step_mm,
        [0.0; 3],
        [40.0, 24.0, 6.0],
        vec![ToolpathId(1)],
        serde_json::json!({ "fixture": "g_simmem" }),
        Arc::clone(trace),
    );
    let base = reset_peak();
    write_simulation_cut_artifact_to(std::path::Path::new("/dev/null"), &artifact).expect("write");
    let write_peak = peak_bytes().saturating_sub(base);
    assert!(
        write_peak < retained / 4,
        "writing the artifact peaked {} MB above its start for a {} MB trace: the \
         document or the trace was copied into memory",
        write_peak >> 20,
        retained >> 20
    );
}
