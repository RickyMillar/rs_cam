//! Integration tests for STEP file import via truck.

#![cfg(feature = "step")]

use rs_cam_core::enriched_mesh::SurfaceType;
use rs_cam_core::step_input::load_step;
use std::path::Path;

fn fixtures_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/step")
}

#[test]
fn test_load_step_cube() {
    let path = fixtures_dir().join("occt-cube.step");
    let enriched = load_step(&path, 0.1).expect("Failed to load cube STEP");

    assert_eq!(enriched.face_count(), 6, "Cube should have 6 faces");
    assert!(
        enriched.as_mesh().triangles.len() >= 12,
        "Cube should have at least 12 triangles, got {}",
        enriched.as_mesh().triangles.len()
    );

    // All faces should be planes
    for group in &enriched.face_groups {
        assert_eq!(
            group.surface_type,
            SurfaceType::Plane,
            "Cube face {} should be a plane",
            group.id.0
        );
    }
}

#[test]
fn test_load_step_cylinder() {
    let path = fixtures_dir().join("occt-cylinder.step");
    let enriched = load_step(&path, 0.1).expect("Failed to load cylinder STEP");

    assert_eq!(enriched.face_count(), 3, "Cylinder should have 3 faces");
    assert!(
        enriched.as_mesh().triangles.len() > 0,
        "Cylinder should have triangles"
    );

    // Should have 2 planar faces (top + bottom) and 1 non-planar (side)
    let plane_count = enriched
        .face_groups
        .iter()
        .filter(|g| g.surface_type == SurfaceType::Plane)
        .count();
    assert!(
        plane_count >= 2,
        "Cylinder should have at least 2 planar faces (top/bottom), got {}",
        plane_count
    );
}

#[test]
fn test_load_step_cone() {
    let path = fixtures_dir().join("occt-cone.step");
    let enriched = load_step(&path, 0.1).expect("Failed to load cone STEP");

    assert_eq!(enriched.face_count(), 3, "Cone should have 3 faces");
    assert!(enriched.as_mesh().triangles.len() > 0);
}

#[test]
fn test_load_step_sphere() {
    let path = fixtures_dir().join("occt-sphere.step");
    let enriched = load_step(&path, 0.1).expect("Failed to load sphere STEP");

    assert!(enriched.face_count() >= 1, "Sphere should have at least 1 face");
    assert!(enriched.as_mesh().triangles.len() > 0);
}

#[test]
fn test_triangle_to_face_coverage() {
    let path = fixtures_dir().join("occt-cube.step");
    let enriched = load_step(&path, 0.1).unwrap();

    // Every triangle should map to a valid face group
    let tri_count = enriched.as_mesh().triangles.len();
    assert_eq!(enriched.triangle_to_face.len(), tri_count);

    for (i, &face_idx) in enriched.triangle_to_face.iter().enumerate() {
        assert!(
            (face_idx as usize) < enriched.face_count(),
            "Triangle {} maps to invalid face group {}",
            i,
            face_idx
        );
    }
}

#[test]
fn test_face_groups_have_contiguous_triangles() {
    let path = fixtures_dir().join("occt-cube.step");
    let enriched = load_step(&path, 0.1).unwrap();

    for group in &enriched.face_groups {
        assert!(
            !group.triangle_range.is_empty(),
            "Face group {} has empty triangle range",
            group.id.0
        );
        assert!(
            group.triangle_range.end <= enriched.as_mesh().triangles.len(),
            "Face group {} triangle range exceeds mesh",
            group.id.0
        );
    }
}

#[test]
fn test_cube_horizontal_faces_produce_polygons() {
    let path = fixtures_dir().join("occt-cube.step");
    let enriched = load_step(&path, 0.1).unwrap();

    // At least 2 faces of a cube should be horizontal planes producing 2D polygons
    let polygon_count = enriched
        .face_groups
        .iter()
        .filter(|g| enriched.face_boundary_as_polygon(g.id).is_some())
        .count();

    assert!(
        polygon_count >= 2,
        "Cube should have at least 2 horizontal faces producing polygons, got {}",
        polygon_count
    );
}

#[test]
fn test_cube_adjacency() {
    let path = fixtures_dir().join("occt-cube.step");
    let enriched = load_step(&path, 0.1).unwrap();

    // A cube has 12 edges, each shared by 2 faces = 12 adjacency pairs
    assert!(
        enriched.adjacency.len() >= 6,
        "Cube should have at least 6 adjacency pairs, got {}",
        enriched.adjacency.len()
    );
}
