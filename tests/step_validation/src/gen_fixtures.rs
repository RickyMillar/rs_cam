/// Generate STEP test fixtures for GUI testing.
///
/// Run from workspace root: cargo run --manifest-path tests/step_validation/Cargo.toml --bin gen_fixtures
use truck_modeling::*;

fn main() {
    let out_dir = std::path::Path::new("fixtures/gui_step");
    std::fs::create_dir_all(out_dir).expect("create output dir");

    // 1. Simple plate: 100x60x10mm — 6 faces, top face is ideal for face op
    write_step(
        &make_box(100.0, 60.0, 10.0),
        &out_dir.join("plate_100x60x10.step"),
    );

    // 2. Tall block: 40x40x60mm — vertical faces for testing non-horizontal selection
    write_step(
        &make_box(40.0, 40.0, 60.0),
        &out_dir.join("block_40x40x60.step"),
    );

    // 3. L-bracket: two-level shape with horizontal faces at different Z heights
    write_step(&make_l_bracket(), &out_dir.join("l_bracket.step"));

    // 4. Stepped block: staircase shape with 3 horizontal faces at different heights
    write_step(
        &make_stepped_block(),
        &out_dir.join("stepped_block.step"),
    );

    println!("Generated 4 STEP fixtures in {}", out_dir.display());
    for entry in std::fs::read_dir(out_dir).unwrap() {
        let entry = entry.unwrap();
        let meta = entry.metadata().unwrap();
        println!(
            "  {} ({:.1} KB)",
            entry.file_name().to_string_lossy(),
            meta.len() as f64 / 1024.0
        );
    }
}

fn make_box(width: f64, depth: f64, height: f64) -> Solid {
    let v = builder::vertex(Point3::new(0.0, 0.0, 0.0));
    let edge = builder::tsweep(&v, Vector3::new(width, 0.0, 0.0));
    let face = builder::tsweep(&edge, Vector3::new(0.0, depth, 0.0));
    builder::tsweep(&face, Vector3::new(0.0, 0.0, height))
}

fn make_l_bracket() -> Solid {
    // L-shaped cross section extruded along Y
    let pts = vec![
        Point3::new(0.0, 0.0, 0.0),
        Point3::new(60.0, 0.0, 0.0),
        Point3::new(60.0, 0.0, 10.0),
        Point3::new(10.0, 0.0, 10.0),
        Point3::new(10.0, 0.0, 40.0),
        Point3::new(0.0, 0.0, 40.0),
    ];
    extrude_profile(&pts, Vector3::new(0.0, 40.0, 0.0))
}

fn make_stepped_block() -> Solid {
    // Staircase cross-section with 3 step levels
    let pts = vec![
        Point3::new(0.0, 0.0, 0.0),
        Point3::new(60.0, 0.0, 0.0),
        Point3::new(60.0, 0.0, 10.0),
        Point3::new(40.0, 0.0, 10.0),
        Point3::new(40.0, 0.0, 20.0),
        Point3::new(20.0, 0.0, 20.0),
        Point3::new(20.0, 0.0, 30.0),
        Point3::new(0.0, 0.0, 30.0),
    ];
    extrude_profile(&pts, Vector3::new(0.0, 40.0, 0.0))
}

/// Build a solid by creating a closed wire from points, attaching a plane, and extruding.
fn extrude_profile(points: &[Point3], direction: Vector3) -> Solid {
    let verts: Vec<Vertex> = points.iter().map(|p| builder::vertex(*p)).collect();
    let edges: Vec<Edge> = (0..verts.len())
        .map(|i| builder::line(&verts[i], &verts[(i + 1) % verts.len()]))
        .collect();
    let wire = Wire::from(edges);
    let face = builder::try_attach_plane(&[wire]).expect("attach plane to profile");
    builder::tsweep(&face, direction)
}

fn write_step(solid: &Solid, path: &std::path::Path) {
    use std::fmt::Write;
    use truck_stepio::out::{CompleteStepDisplay, StepHeaderDescriptor, StepModels};

    let compressed = solid.compress();
    let mut models = StepModels::default();
    models.push_solid(&compressed);

    let header = StepHeaderDescriptor {
        file_name: path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned(),
        ..Default::default()
    };

    let display = CompleteStepDisplay::new(models, header);
    let mut step_string = String::new();
    write!(&mut step_string, "{display}").expect("format STEP");
    std::fs::write(path, step_string)
        .unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
    println!("  wrote {}", path.display());
}
