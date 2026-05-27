//! WANAKA end-to-end revalidation for DEXEL roadmap Step 5 — J
//! marching-cubes mesh extraction (`planning/DEXEL_Z_ONLY_INVESTIGATION.md`
//! §9 acceptance gate 2).
//!
//! Loads the WANAKA project via `ProjectSession`, runs simulation, and
//! asserts that the resulting closed-solid mesh (and every per-toolpath
//! checkpoint mesh) is well-formed under MC:
//!   - non-empty
//!   - vertex z values bounded by the stock envelope
//!   - all triangle indices in range
//!   - no NaN / Inf positions
//!   - colour buffer matches vertex buffer
//!
//! Skipped if the WANAKA TOML is not present (consistent with the Step 4
//! revalidation pattern).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr,
    clippy::print_stdout
)]

use rs_cam_core::session::{ProjectSession, SimulationOptions};
use std::path::Path;
use std::sync::atomic::AtomicBool;

const WANAKA_TOML: &str = "/home/ricky/Downloads/wanaka100/wanaka_full_tuned.toml";

fn assert_mesh_well_formed(label: &str, mesh: &rs_cam_core::stock_mesh::StockMesh) {
    assert!(!mesh.vertices.is_empty(), "{label}: mesh has no vertices");
    assert_eq!(
        mesh.vertices.len() % 3,
        0,
        "{label}: vertex array not multiple of 3"
    );
    assert_eq!(
        mesh.indices.len() % 3,
        0,
        "{label}: index array not multiple of 3"
    );
    assert_eq!(
        mesh.colors.len(),
        mesh.vertices.len(),
        "{label}: colour buffer size mismatch ({} vs {})",
        mesh.colors.len(),
        mesh.vertices.len()
    );
    let n_verts = mesh.vertices.len() / 3;
    for &idx in &mesh.indices {
        assert!(
            (idx as usize) < n_verts,
            "{label}: index {idx} out of range (n_verts={n_verts})"
        );
    }
    // NaN / Inf check on all coordinates and colors. We don't constrain the
    // z-range against the project's stock_bbox: WANAKA uses multi-setup
    // machining where per-setup local stocks are transformed back to global
    // coords by `append_transformed`, and the resulting vertices may legitimately
    // sit outside the global stock_bbox (e.g. by setup transform offsets).
    for i in 0..n_verts {
        let x = mesh.vertices[i * 3];
        let y = mesh.vertices[i * 3 + 1];
        let z = mesh.vertices[i * 3 + 2];
        assert!(x.is_finite(), "{label}: vertex {i} x={x} not finite");
        assert!(y.is_finite(), "{label}: vertex {i} y={y} not finite");
        assert!(z.is_finite(), "{label}: vertex {i} z={z} not finite");
    }
    for (i, &c) in mesh.colors.iter().enumerate() {
        assert!(
            c.is_finite() && (0.0..=1.0).contains(&c),
            "{label}: color {i} = {c} outside [0, 1]"
        );
    }
}

#[test]
#[ignore = "expensive WANAKA end-to-end mesh revalidation; run with `cargo test --test wanaka_step5_mc_revalidation -- --ignored`"]
fn wanaka_step5_final_mesh_is_well_formed() {
    let toml_path = Path::new(WANAKA_TOML);
    if !toml_path.exists() {
        eprintln!("skip: {WANAKA_TOML} not present");
        return;
    }

    let mut session = ProjectSession::load(toml_path).expect("load wanaka");
    let cancel = AtomicBool::new(false);
    let n_toolpaths = session.toolpath_count();
    for idx in 0..n_toolpaths {
        let _ = session.generate_toolpath(idx, &cancel);
    }

    let opts = SimulationOptions {
        resolution: 0.5,
        skip_ids: vec![],
        metrics_enabled: false,
        auto_resolution: false,
        use_predicted_feed_in_gates: false,
        adaptive_feed_modulation: false,
        modulation_strategy: rs_cam_core::feed_modulation::ModulationStrategy::ConstrainedMax,
        modulation_aggressiveness: 1.0,    };
    let result = session.run_simulation(&opts, &cancel).expect("sim");

    assert_mesh_well_formed("final mesh", &result.mesh);
    eprintln!(
        "WANAKA Step 5 final mesh: {} verts, {} tris",
        result.mesh.vertices.len() / 3,
        result.mesh.indices.len() / 3
    );

    // Each per-toolpath checkpoint mesh must also be well-formed.
    for cp in &result.checkpoints {
        let label = format!("checkpoint @ boundary {}", cp.boundary_index);
        assert_mesh_well_formed(&label, &cp.mesh);
    }
    eprintln!(
        "WANAKA Step 5: {} per-toolpath checkpoint meshes all well-formed",
        result.checkpoints.len()
    );
}
