//! Round 7 (FOLLOW_UPS §12): the draw order of the translucent solid stock
//! and the reach overlay, read back as pixels from the headless frame
//! (`RenderResources::render_headless_callback`).
//!
//! - A line behind the translucent solid stock shows through it, tinted,
//!   and a line in front of it keeps its colour. The stock draws last, with
//!   the read-only depth state. Red proof: the stock drew before the lines
//!   with depth write on, so its top face hid every line below it.
//! - The reach overlay recolours its own model only. A second model draws
//!   as it does without the overlay. Red proof: the overlay arm replaced
//!   the face draw of every model, so the second model was the background.
//!
//! The arms share one device, in a binary of their own. They draw with the
//! colored and the reach pipelines, which the G-PIPESMOKE binary does not
//! draw. In that binary, beside its eight device tests, llvmpipe crashed
//! the process (SIGSEGV) in about one run of eight; alone, it did not.
//!
//! A machine with no adapter prints the reason and passes, as G-PIPESMOKE
//! does. Run it with `-- --nocapture` to see the pixels.

#![expect(
    clippy::panic,
    clippy::print_stderr,
    reason = "test code: a panic is how a test reports a failure; the test prints its measurements for the reader"
)]

#[path = "fixtures/gpu.rs"]
mod gpu;

use egui_wgpu::wgpu;
use gpu::{
    FRAME_HEIGHT, FRAME_WIDTH, PIXEL_FACE_Z, assert_no_uncaptured, headless_device, line_buffer,
    pixel_of, pixel_view, read_pixel, same_colour, uncaptured_sink,
};
use rs_cam_core::geo::P3;
use rs_cam_core::geometry::enriched_mesh::{
    EnrichedMesh, FaceTessellation, SurfaceParams, SurfaceType, build_enriched_mesh,
};
use rs_cam_core::ids::ModelId;
use rs_cam_viz::render::mesh_render::face_mesh_gpu_data;
use rs_cam_viz::render::outline::OutlineUniforms;
use rs_cam_viz::render::{
    EdgeOffset, LineUniforms, LineVertex, LineWidths, MeshUniforms, RenderResources,
};

/// The frame uniforms of the pixel arms: the orthographic top view, the
/// light straight down, one pixel per point.
fn write_pixel_uniforms(gpu: &gpu::HeadlessGpu, resources: &mut RenderResources) -> MeshUniforms {
    let view_proj = pixel_view();
    let mesh = MeshUniforms::new(view_proj, [0.0, 0.0, 1.0], [0.0, 0.0, 100.0]);
    let lines = LineUniforms::new(view_proj, [0.0, 0.0, 1.0, 0.0]);
    let widths = LineWidths {
        viewport_px: [FRAME_WIDTH as f32, FRAME_HEIGHT as f32],
        pixels_per_point: 1.0,
    };
    resources.write_frame_uniforms(
        &gpu.queue,
        &mesh,
        lines,
        EdgeOffset { tol: 0.0, k: 0.0 },
        widths,
        &OutlineUniforms::orthographic(50.0, 1.0, FRAME_HEIGHT as f32),
    );
    mesh
}

/// A line BEHIND the translucent solid stock
/// shows through it, tinted, and a line IN FRONT of it keeps its colour.
/// Red proof: the stock drew before the lines with depth write on, so the
/// stock top hid the line below it and that pixel was the stock colour.
fn a_line_behind_the_translucent_stock_shows_through(gpu: &gpu::HeadlessGpu) {
    use rs_cam_core::geo::BoundingBox3;
    use rs_cam_viz::render::ViewportCallback;
    use rs_cam_viz::render::stock_render::SolidStockGpuData;

    let uncaptured = uncaptured_sink(&gpu.device);
    let mut resources = RenderResources::new(&gpu.device, wgpu::TextureFormat::Rgba8Unorm, 1);
    let mesh = write_pixel_uniforms(gpu, &mut resources);

    // The stock: Z -5 to 10 over the probes. One line under its top (Z 0),
    // one over it (Z 20).
    let stock = BoundingBox3::from_points([P3::new(-0.5, -0.5, -5.0), P3::new(1.2, 1.4, 10.0)]);
    resources.solid_stock_data = Some(SolidStockGpuData::from_bbox(&gpu.device, &stock));
    let magenta = [1.0, 0.0, 1.0];
    let cyan = [0.0, 1.0, 1.0];
    let line = |y: f32, z: f32, color: [f32; 3]| {
        [
            LineVertex {
                position: [-0.4, y, z],
                color,
            },
            LineVertex {
                position: [1.1, y, z],
                color,
            },
        ]
    };
    let mut verts = line(0.25, 0.0, magenta).to_vec();
    verts.extend(line(1.0, 20.0, cyan));
    resources
        .polygon_data
        .push(line_buffer(gpu, "g_pipesmoke_stock_lines", &verts));

    let behind = pixel_of(0.5, 0.25);
    let in_front = pixel_of(0.5, 1.0);
    let stock_only = pixel_of(0.5, 0.6);
    let callback = ViewportCallback {
        mesh_uniforms: mesh,
        show_polygons: true,
        show_solid_stock: true,
        ..ViewportCallback::empty(FRAME_WIDTH, FRAME_HEIGHT)
    };
    resources.render_headless_callback(&gpu.device, &gpu.queue, &callback);
    let behind_px = read_pixel(gpu, &resources, behind);
    let front_px = read_pixel(gpu, &resources, in_front);
    let stock_px = read_pixel(gpu, &resources, stock_only);
    eprintln!("R7 stock: behind {behind_px:?}, front {front_px:?}, stock {stock_px:?}");
    assert_ne!(stock_px, [0, 0, 0, 0], "non-vacuity: the stock draws");
    assert!(
        behind_px[0] > 150 && behind_px[2] > 150 && behind_px[1] < 100,
        "the line behind the stock top shows through, tinted: {behind_px:?} (stock alone {stock_px:?})"
    );
    assert_ne!(behind_px, stock_px, "the line behind the stock is hidden");
    assert!(
        same_colour(front_px, cyan),
        "the line in front of the stock keeps its colour: {front_px:?}"
    );
    assert_no_uncaptured(&uncaptured, "rendering the stock frame");
}

/// A one-triangle face mesh on `ring` at `z`.
fn face_mesh_on(ring: [(f64, f64); 3], z: f64) -> EnrichedMesh {
    let ring: Vec<P3> = ring.iter().map(|&(x, y)| P3::new(x, y, z)).collect();
    let face = FaceTessellation {
        vertices: ring.clone(),
        triangles: vec![[0, 1, 2]],
        surface_type: SurfaceType::Unknown,
        surface_params: SurfaceParams::Unknown,
        boundary_loops: vec![ring],
    };
    build_enriched_mesh(vec![face], Vec::new(), Vec::new())
        .unwrap_or_else(|e| panic!("the face mesh builds: {e}"))
}

/// The reach overlay recolours ITS model only.
/// Model 1 under the overlay takes the overlay colour; model 2 draws as
/// usual. Red proof: the overlay arm replaced the face draw of every
/// model, so model 2 was the background.
fn the_reach_overlay_recolours_its_own_model_only(gpu: &gpu::HeadlessGpu) {
    use rs_cam_core::mesh::TriangleMesh;
    use rs_cam_core::settings::DisplayMode;
    use rs_cam_viz::render::ViewportCallback;
    use rs_cam_viz::render::mesh_render::colored_mesh_gpu_data;

    let uncaptured = uncaptured_sink(&gpu.device);
    let mut resources = RenderResources::new(&gpu.device, wgpu::TextureFormat::Rgba8Unorm, 1);
    let mesh = write_pixel_uniforms(gpu, &mut resources);

    let z = f64::from(PIXEL_FACE_Z);
    let one = [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)];
    let two = [(1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
    for (wire, ring) in [(1, one), (2, two)] {
        let enriched = face_mesh_on(ring, z);
        let face_mesh = face_mesh_gpu_data(
            &gpu.device,
            &resources.gpu_limits,
            &resources.face_state_bgl,
            &resources.face_palette_buffer,
            ModelId::from_wire(wire),
            &enriched,
            &None,
        )
        .unwrap_or_else(|| panic!("the face mesh fits the device limits"));
        resources.face_mesh_data_list.push(face_mesh);
    }
    let green = [0.0, 1.0, 0.0];
    let reach_mesh = TriangleMesh::from_raw(
        one.iter().map(|&(x, y)| P3::new(x, y, z)).collect(),
        vec![[0, 1, 2]],
    );
    let model_one = ModelId::from_wire(1);
    resources.reach_overlay_data = Some(
        colored_mesh_gpu_data(
            &gpu.device,
            &resources.gpu_limits,
            model_one,
            &reach_mesh,
            &[green; 3],
        )
        .unwrap_or_else(|| panic!("the reach mesh fits the device limits")),
    );

    let in_one = pixel_of(0.25, 0.25);
    let in_two = pixel_of(0.75, 0.75);
    let frame = |reach: bool, resources: &mut RenderResources| {
        let callback = ViewportCallback {
            mesh_uniforms: mesh,
            show_model: true,
            display_mode: DisplayMode::Shaded,
            show_reach_overlay: reach,
            ..ViewportCallback::empty(FRAME_WIDTH, FRAME_HEIGHT)
        };
        resources.render_headless_callback(&gpu.device, &gpu.queue, &callback);
        (
            read_pixel(gpu, resources, in_one),
            read_pixel(gpu, resources, in_two),
        )
    };
    let (plain_one, plain_two) = frame(false, &mut resources);
    let (reach_one, reach_two) = frame(true, &mut resources);
    eprintln!("R7 reach: plain {plain_one:?} {plain_two:?}, reach {reach_one:?} {reach_two:?}");
    assert_ne!(plain_two, [0, 0, 0, 0], "non-vacuity: model 2 draws");
    assert!(
        reach_one[1] > reach_one[0] && reach_one[1] > reach_one[2],
        "model 1 takes the reach colour: {reach_one:?}"
    );
    assert_eq!(
        reach_two, plain_two,
        "model 2 draws as usual under the reach overlay of model 1"
    );
    assert_no_uncaptured(&uncaptured, "rendering the reach frames");
}

/// The two arms on one headless device.
#[test]
fn the_stock_and_the_reach_overlay_draw_in_order_r7() {
    let gpu = match headless_device("r7_draw_order_device") {
        Ok(gpu) => gpu,
        Err(reason) => {
            eprintln!("R7 DRAW ORDER SKIPPED — {reason}");
            return;
        }
    };
    a_line_behind_the_translucent_stock_shows_through(&gpu);
    the_reach_overlay_recolours_its_own_model_only(&gpu);
}
