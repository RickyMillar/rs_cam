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

use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
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

/// **G-STEPUNITS** — the project loader DROPS a STEP model's unit scale.
///
/// `ModelUnits` is the only unit conversion a STEP file gets in this
/// product. `truck-stepio`'s reader performs none: the crate's only
/// `LENGTH_UNIT` / `SI_UNIT` handling is in its WRITER, which emits a
/// hardcoded millimetre header. `step_input::load_step` adds none of its
/// own.
///
/// `load_model_geometry` binds `let scale = model.units…scale_factor()` and
/// passes it to the STL arm, the DXF arm and the SVG arm. The STEP arm never
/// reads it. The interactive door `io::load_model_file` DOES apply it.
///
/// This is the FOURTH divergence found in this loader pair. G-UNITSRELOAD
/// closed the third (SVG and DXF) and left this one; that sentry has no STEP
/// coverage at all.
///
/// Reachable on three doors, none of them the GUI alone — `import_step_path`
/// hardcodes a scale of 1.0 and `rescale_model` refuses STEP outright. The
/// plainest is `rs_cam_cli run --units inches part.step`, which cuts the
/// program on geometry 25.4 times too small.
///
/// The test asserts the two doors AGREE. It does not assert a magic size,
/// for the reason G-UNITSRELOAD gives: a size assertion pins one door's
/// answer and hides which door is wrong.
#[test]
fn both_doors_apply_a_step_models_declared_units() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let step_path = manifest.join("tests/fixtures/step/occt-cube.step");
    assert!(step_path.exists(), "fixture STEP missing: {step_path:?}");

    // NON-VACUITY: the interactive door must actually move the geometry when
    // the units change. Without this the agreement assertion below could
    // pass because neither door scales anything.
    let as_mm =
        rs_cam_core::io::load_model_file(&step_path, 1, ModelKind::Step, ModelUnits::Millimeters)
            .expect("interactive load, mm");
    let as_inches =
        rs_cam_core::io::load_model_file(&step_path, 1, ModelKind::Step, ModelUnits::Inches)
            .expect("interactive load, inches");
    let mm_bbox = as_mm.bbox().expect("mm bbox");
    let inch_bbox = as_inches.bbox().expect("inch bbox");
    let mm_x = mm_bbox.max.x - mm_bbox.min.x;
    let inch_x = inch_bbox.max.x - inch_bbox.min.x;
    assert!(
        (inch_x - mm_x * 25.4).abs() < 1e-6,
        "non-vacuity: the interactive door must scale a STEP model by its \
         declared units. mm span {mm_x}, inch span {inch_x}"
    );

    // The project door, same file, same declared units.
    let tmp = tempdir_for_test("step_units");
    let model_dst = tmp.join("occt-cube.step");
    std::fs::copy(&step_path, &model_dst).expect("copy step fixture");
    let toml_dst = tmp.join("project.toml");
    std::fs::write(&toml_dst, project_toml_with_units("inches")).expect("write project toml");

    let session = ProjectSession::load(&toml_dst).expect("session load");
    let model = session.models().first().expect("at least one model");
    let project_bbox = model.bbox().expect("project-door bbox");
    let project_x = project_bbox.max.x - project_bbox.min.x;

    assert!(
        (project_x - inch_x).abs() < 1e-6,
        "the two doors must agree on a STEP model's declared units. The \
         interactive door gives a span of {inch_x} mm and the project door \
         gives {project_x} mm. The project door drops the scale, so a saved \
         project re-imports an inch-authored STEP model 25.4 times too small."
    );
}

/// The same template as [`project_toml_template`], with the declared units
/// as a parameter.
fn project_toml_with_units(units_kind: &str) -> String {
    project_toml_template().replace(
        "kind = \"millimeters\"",
        &format!("kind = \"{units_kind}\""),
    )
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
