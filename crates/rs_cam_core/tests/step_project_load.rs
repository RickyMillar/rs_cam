//! Regression test for Roadmap D — STEP/BREP loader.
//!
//! `ProjectSession::load` goes through `project_file::load_model_geometry`,
//! which previously had a Step arm that downgraded to a flat `TriangleMesh`
//! (`Ok(LoadedGeometry::Mesh((*enriched.mesh).clone()))`) and set
//! `enriched_mesh: None` on the resulting `LoadedModel`. That silently broke
//! `inspect_brep_faces` and the GUI face picker for any project loaded via
//! the session loader. The parallel `io::load_model_file` loader has always
//! preserved the BREP — this test pins the project loader to the same
//! contract.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg(feature = "step")]

use rs_cam_core::session::ProjectSession;
use std::path::Path;

#[test]
fn project_session_load_preserves_step_brep_topology() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let project_path = manifest.join("tests/fixtures/step_project_loader.toml");
    let step_path = manifest.join("tests/fixtures/step/occt-cube.step");
    assert!(step_path.exists(), "fixture STEP missing: {step_path:?}");

    // Build a minimal TOML in a temp dir so the relative model path resolves.
    let tmp = tempdir_for_test("step_project_load");
    let model_dst = tmp.join("occt-cube.step");
    std::fs::copy(&step_path, &model_dst).expect("copy step fixture");
    let toml_dst = tmp.join("project.toml");
    std::fs::write(&toml_dst, project_toml_template()).expect("write project toml");
    drop(project_path); // silence unused

    let session = ProjectSession::load(&toml_dst).expect("session load");
    let model = session.models().first().expect("at least one model");
    assert_eq!(model.name, "Cube");
    let enriched = model
        .enriched_mesh
        .as_ref()
        .expect("STEP load must populate enriched_mesh — Roadmap D regression");
    assert!(
        enriched.face_count() > 0,
        "enriched mesh should carry BREP face groups (got 0)"
    );
    // Cube has 6 faces; assert we got the expected geometry, not just a
    // stub enriched mesh.
    assert_eq!(enriched.face_count(), 6, "cube should have 6 BREP faces");
}

fn tempdir_for_test(name: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("rs_cam_step_loader_{name}_{nanos}"));
    std::fs::create_dir_all(&dir).expect("mkdir tempdir");
    dir
}

fn project_toml_template() -> &'static str {
    r#"
format_version = 3

[job]
name = "step loader regression"

[job.stock]
x = 100.0
y = 100.0
z = 50.0
origin_x = -50.0
origin_y = -50.0
origin_z = -50.0
padding = 5.0
auto_from_model = false

[job.stock.material.SolidWood]
species = "GenericHardwood"

[job.machine]
name = "Generic Wood Router"
max_feed_mm_min = 4000.0
max_shank_mm = 6.35
safety_factor = 0.75

[job.machine.spindle.Variable]
min_rpm = 8000.0
max_rpm = 24000.0

[job.machine.power.ConstantPower]
power_kw = 0.8

[job.machine.chip_load]
k0 = 0.024
p = 0.61
q = 1.26

[job.machine.rigidity]
doc_roughing_factor = 0.2
doc_finishing_factor = 0.08
woc_roughing_factor = 0.7
woc_roughing_max_mm = 5.0
woc_finishing_mm = 0.5
adaptive_doc_factor = 1.5
adaptive_woc_factor = 0.2

[job.post]
format = "grbl"
spindle_speed = 18000
safe_z = 10.0
high_feedrate_mode = false
high_feedrate = 5000.0

[[models]]
id = 1
path = "occt-cube.step"
name = "Cube"
kind = "step"

[models.units]
kind = "millimeters"
"#
}
