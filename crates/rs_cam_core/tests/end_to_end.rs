//! End-to-end integration test: STL -> drop-cutter -> G-code

use rs_cam_core::{
    dropcutter::batch_drop_cutter,
    gcode::{GrblPost, emit_gcode},
    mesh::{SpatialIndex, TriangleMesh},
    tool::{BallEndmill, MillingCutter},
    toolpath::raster_toolpath_from_grid,
};
use std::path::Path;

#[test]
fn test_terrain_stl_to_gcode() {
    let stl_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join("terrain_small.stl");

    if !stl_path.exists() {
        eprintln!("Skipping: fixture not found at {:?}", stl_path);
        return;
    }

    // Load
    let mesh = TriangleMesh::from_stl(&stl_path).expect("Failed to load STL");
    assert!(
        mesh.faces.len() > 1000,
        "Expected a real mesh, got {} triangles",
        mesh.faces.len()
    );

    // Index
    let tool = BallEndmill::new(6.35, 25.0);
    let index = SpatialIndex::build(&mesh, tool.diameter() * 2.0);

    // Drop-cutter
    let grid = batch_drop_cutter(&mesh, &index, &tool, 2.0, 0.0, -50.0);
    assert!(grid.rows > 5);
    assert!(grid.cols > 5);

    // Verify Z values are within mesh bounds (with some margin for the cutter radius)
    let margin = tool.diameter();
    for cl in &grid.points {
        assert!(cl.z >= -50.0 - 1e-6, "CL.z {} below min_z", cl.z);
        assert!(
            cl.z <= mesh.bbox.max.z + margin,
            "CL.z {} above mesh max {} + margin {}",
            cl.z,
            mesh.bbox.max.z,
            margin
        );
    }

    // Generate toolpath
    let toolpath = raster_toolpath_from_grid(&grid, 1000.0, 500.0, 10.0);
    assert!(toolpath.moves.len() > 100);
    assert!(toolpath.total_cutting_distance() > 0.0);

    // Emit G-code
    let gcode = emit_gcode(&toolpath, &GrblPost, 18000);
    assert!(gcode.contains("G17 G21 G90"));
    assert!(gcode.contains("M3 S18000"));
    assert!(gcode.contains("G0"));
    assert!(gcode.contains("G1"));
    assert!(gcode.contains("M30"));
    assert!(gcode.len() > 1000);
}

#[test]
fn test_programmatic_hemisphere_to_gcode() {
    use rs_cam_core::mesh::make_test_hemisphere;

    let mesh = make_test_hemisphere(20.0, 16);
    let tool = BallEndmill::new(6.35, 25.0);
    let index = SpatialIndex::build(&mesh, tool.diameter() * 2.0);

    let grid = batch_drop_cutter(&mesh, &index, &tool, 1.0, 0.0, -30.0);

    // Center point should be at approximately z = hemisphere_radius
    let center_row = grid.rows / 2;
    let center_col = grid.cols / 2;
    let center_cl = grid.get(center_row, center_col);
    assert!(
        (center_cl.z - 20.0).abs() < 1.0,
        "Center CL.z = {}, expected ~20.0",
        center_cl.z
    );

    let toolpath = raster_toolpath_from_grid(&grid, 1000.0, 500.0, 25.0);
    let gcode = emit_gcode(&toolpath, &GrblPost, 18000);
    assert!(gcode.len() > 500);
}
