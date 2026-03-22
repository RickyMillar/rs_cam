//! Visualization output.
//!
//! - SVG: 2D top-down toolpath preview
//! - HTML: Interactive 3D viewer with mesh + toolpaths (three.js)
//!
//! Note: `let _ = write!(...)` is used throughout this module for writing to `String`.
//! Writing to `String` is infallible (only fails on OOM, which panics regardless),
//! so discarding the `Result` with `let _ =` is safe.

use crate::arc_util::linearize_arc;
use crate::dexel_mesh::dexel_stock_to_mesh;
use crate::dexel_stock::TriDexelStock;
use crate::geo::BoundingBox3;
use crate::mesh::TriangleMesh;
use crate::tool::MillingCutter;
use crate::toolpath::{MoveType, Toolpath};
use std::fmt::Write;

/// Generate an SVG showing the toolpath from a top-down (XY) view.
/// Z is encoded as color: deeper = darker blue, higher = lighter/warmer.
pub fn toolpath_to_svg(toolpath: &Toolpath, width: f64, height: f64) -> String {
    if toolpath.moves.is_empty() {
        return String::from("<svg xmlns='http://www.w3.org/2000/svg'/>");
    }

    // Find XY bounds
    let mut bbox = BoundingBox3::empty();
    for m in &toolpath.moves {
        bbox.expand_to(m.target);
    }

    let margin = 10.0;
    let data_w = bbox.max.x - bbox.min.x;
    let data_h = bbox.max.y - bbox.min.y;
    if data_w < 1e-10 || data_h < 1e-10 {
        return String::from("<svg xmlns='http://www.w3.org/2000/svg'/>");
    }

    let scale = ((width - 2.0 * margin) / data_w).min((height - 2.0 * margin) / data_h);
    let z_min = bbox.min.z;
    let z_range = (bbox.max.z - bbox.min.z).max(1e-6);

    let mut svg = String::new();
    let _ = writeln!(
        svg,
        "<svg xmlns='http://www.w3.org/2000/svg' width='{width}' height='{height}' viewBox='0 0 {width} {height}'>"
    );
    let _ = writeln!(
        svg,
        "<rect width='{width}' height='{height}' fill='#1a1a2e'/>"
    );

    // Draw rapids as thin gray dashed lines
    // Draw feed moves as colored lines (Z-based color)
    for i in 1..toolpath.moves.len() {
        let from = &toolpath.moves[i - 1].target;
        let to = &toolpath.moves[i].target;

        let x1 = margin + (from.x - bbox.min.x) * scale;
        let y1 = height - margin - (from.y - bbox.min.y) * scale; // flip Y
        let x2 = margin + (to.x - bbox.min.x) * scale;
        let y2 = height - margin - (to.y - bbox.min.y) * scale;

        match toolpath.moves[i].move_type {
            MoveType::Rapid => {
                let _ = writeln!(
                    svg,
                    "<line x1='{x1:.1}' y1='{y1:.1}' x2='{x2:.1}' y2='{y2:.1}' stroke='#333' stroke-width='0.3' stroke-dasharray='2,2'/>"
                );
            }
            MoveType::Linear { .. } | MoveType::ArcCW { .. } | MoveType::ArcCCW { .. } => {
                // Color by Z: low=deep blue, high=bright cyan/white
                let t = ((to.z - z_min) / z_range).clamp(0.0, 1.0);
                let r = (t * 100.0) as u8;
                let g = (80.0 + t * 175.0) as u8;
                let b = (180.0 + t * 75.0) as u8;
                let _ = writeln!(
                    svg,
                    "<line x1='{x1:.1}' y1='{y1:.1}' x2='{x2:.1}' y2='{y2:.1}' stroke='#{r:02x}{g:02x}{b:02x}' stroke-width='0.5'/>"
                );
            }
        }
    }

    // Add legend
    let _ = writeln!(
        svg,
        "<text x='5' y='15' fill='white' font-size='10' font-family='monospace'>Z: {:.2} to {:.2} mm</text>",
        z_min, bbox.max.z
    );
    let _ = writeln!(
        svg,
        "<text x='5' y='27' fill='white' font-size='10' font-family='monospace'>{} moves, {:.0}mm cutting</text>",
        toolpath.moves.len(),
        toolpath.total_cutting_distance()
    );

    let _ = writeln!(svg, "</svg>");
    svg
}

/// Generate a self-contained HTML file with an interactive 3D viewer.
///
/// Shows the mesh surface + toolpath lines using three.js (loaded from CDN).
/// Open the resulting file in any modern browser to orbit/zoom/pan.
pub fn toolpath_to_3d_html(mesh: &TriangleMesh, toolpath: &Toolpath) -> String {
    let mut html = String::with_capacity(1024 * 1024);

    // Compute mesh center for camera target
    let center = mesh.bbox.center();
    let extent = (mesh.bbox.max.x - mesh.bbox.min.x)
        .max(mesh.bbox.max.y - mesh.bbox.min.y)
        .max(mesh.bbox.max.z - mesh.bbox.min.z);
    let cam_dist = extent * 1.5;

    // Serialize mesh vertices as flat f32 array [x0,y0,z0, x1,y1,z1, ...]
    let mut mesh_verts = String::new();
    for v in &mesh.vertices {
        let _ = write!(mesh_verts, "{:.4},{:.4},{:.4},", v.x, v.y, v.z);
    }

    // Serialize mesh triangle indices
    let mut mesh_indices = String::new();
    for tri in &mesh.triangles {
        let _ = write!(mesh_indices, "{},{},{},", tri[0], tri[1], tri[2]);
    }

    // Serialize cutting path vertices + colors, and rapid path vertices separately
    let mut cut_verts = String::new();
    let mut cut_colors = String::new();
    let mut rapid_verts = String::new();

    // Compute Z range for coloring
    let mut z_min = f64::INFINITY;
    let mut z_max = f64::NEG_INFINITY;
    for m in &toolpath.moves {
        if let MoveType::Linear { .. } = m.move_type {
            z_min = z_min.min(m.target.z);
            z_max = z_max.max(m.target.z);
        }
    }
    let z_range = (z_max - z_min).max(1e-6);

    for i in 1..toolpath.moves.len() {
        let from = &toolpath.moves[i - 1].target;
        let to = &toolpath.moves[i].target;

        match toolpath.moves[i].move_type {
            MoveType::Rapid => {
                let _ = write!(
                    rapid_verts,
                    "{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},",
                    from.x, from.y, from.z, to.x, to.y, to.z
                );
            }
            MoveType::Linear { .. } | MoveType::ArcCW { .. } | MoveType::ArcCCW { .. } => {
                let _ = write!(
                    cut_verts,
                    "{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},",
                    from.x, from.y, from.z, to.x, to.y, to.z
                );
                // Color both endpoints by their Z
                for z in [from.z, to.z] {
                    let t = ((z - z_min) / z_range).clamp(0.0, 1.0) as f32;
                    // Low Z = blue (0.1, 0.3, 0.9), high Z = cyan (0.2, 0.9, 1.0)
                    let r = 0.1 + t * 0.1;
                    let g = 0.3 + t * 0.6;
                    let b = 0.9 + t * 0.1;
                    let _ = write!(cut_colors, "{:.3},{:.3},{:.3},", r, g, b);
                }
            }
        }
    }

    let _ = write!(
        html,
        r##"<!DOCTYPE html>
<html><head>
<meta charset="utf-8">
<title>rs_cam 3D Toolpath Viewer</title>
<style>
  body {{ margin: 0; overflow: hidden; background: #1a1a2e; }}
  #info {{
    position: absolute; top: 10px; left: 10px; color: #ccc;
    font: 13px monospace; background: rgba(0,0,0,0.6); padding: 8px 12px;
    border-radius: 4px; pointer-events: none;
  }}
  #legend {{
    position: absolute; bottom: 10px; left: 10px; color: #aaa;
    font: 12px monospace; background: rgba(0,0,0,0.6); padding: 8px 12px;
    border-radius: 4px;
  }}
</style>
</head><body>
<div id="info">
  Mesh: {mesh_verts} verts, {mesh_tris} tris<br>
  Toolpath: {tp_moves} moves, {tp_cut:.0}mm cutting, {tp_rapid:.0}mm rapid<br>
  Z range: {z_min:.2} to {z_max:.2} mm
</div>
<div id="legend">
  <span style="color:#3388ff">&#9632;</span> Cutting &nbsp;
  <span style="color:#ff4444">&#9632;</span> Rapid &nbsp;
  <span style="color:#88aa88">&#9632;</span> Mesh &nbsp;
  Mouse: orbit | Scroll: zoom | Right-click: pan
</div>

<script type="importmap">
{{
  "imports": {{
    "three": "https://cdn.jsdelivr.net/npm/three@0.170.0/build/three.module.js",
    "three/addons/": "https://cdn.jsdelivr.net/npm/three@0.170.0/examples/jsm/"
  }}
}}
</script>

<script type="module">
import * as THREE from 'three';
import {{ OrbitControls }} from 'three/addons/controls/OrbitControls.js';

const scene = new THREE.Scene();
scene.background = new THREE.Color(0x1a1a2e);

const camera = new THREE.PerspectiveCamera(50, window.innerWidth / window.innerHeight, 0.1, 100000);
camera.position.set({cx:.3} + {cd:.3} * 0.5, {cy:.3} - {cd:.3} * 0.8, {cz:.3} + {cd:.3} * 1.0);

const renderer = new THREE.WebGLRenderer({{ antialias: true }});
renderer.setSize(window.innerWidth, window.innerHeight);
renderer.setPixelRatio(window.devicePixelRatio);
document.body.appendChild(renderer.domElement);

const controls = new OrbitControls(camera, renderer.domElement);
controls.target.set({cx:.3}, {cy:.3}, {cz:.3});
controls.enableDamping = true;

// Lighting
scene.add(new THREE.AmbientLight(0x555555));
const dir1 = new THREE.DirectionalLight(0xffffff, 0.8);
dir1.position.set(1, 1, 2);
scene.add(dir1);
const dir2 = new THREE.DirectionalLight(0x4488aa, 0.4);
dir2.position.set(-1, -0.5, 0.5);
scene.add(dir2);

// --- Mesh ---
const meshVerts = new Float32Array([{mesh_verts_data}]);
const meshIdx = new Uint32Array([{mesh_idx_data}]);
const meshGeo = new THREE.BufferGeometry();
meshGeo.setAttribute('position', new THREE.BufferAttribute(meshVerts, 3));
meshGeo.setIndex(new THREE.BufferAttribute(meshIdx, 1));
meshGeo.computeVertexNormals();

const meshMat = new THREE.MeshPhongMaterial({{
  color: 0x88aa88,
  transparent: true,
  opacity: 0.6,
  side: THREE.DoubleSide,
  depthWrite: true,
}});
scene.add(new THREE.Mesh(meshGeo, meshMat));

// Wireframe overlay
const wireMat = new THREE.MeshBasicMaterial({{
  color: 0x445544,
  wireframe: true,
  transparent: true,
  opacity: 0.15,
}});
scene.add(new THREE.Mesh(meshGeo, wireMat));

// --- Cutting toolpath ---
const cutVerts = new Float32Array([{cut_verts_data}]);
const cutColors = new Float32Array([{cut_colors_data}]);
if (cutVerts.length > 0) {{
  const cutGeo = new THREE.BufferGeometry();
  cutGeo.setAttribute('position', new THREE.BufferAttribute(cutVerts, 3));
  cutGeo.setAttribute('color', new THREE.BufferAttribute(cutColors, 3));
  const cutMat = new THREE.LineBasicMaterial({{ vertexColors: true, linewidth: 1 }});
  scene.add(new THREE.LineSegments(cutGeo, cutMat));
}}

// --- Rapid toolpath ---
const rapidVerts = new Float32Array([{rapid_verts_data}]);
if (rapidVerts.length > 0) {{
  const rapidGeo = new THREE.BufferGeometry();
  rapidGeo.setAttribute('position', new THREE.BufferAttribute(rapidVerts, 3));
  const rapidMat = new THREE.LineBasicMaterial({{ color: 0xff4444, linewidth: 1, transparent: true, opacity: 0.5 }});
  scene.add(new THREE.LineSegments(rapidGeo, rapidMat));
}}

// --- Grid helper ---
const gridSize = {grid_size:.0};
const grid = new THREE.GridHelper(gridSize, 20, 0x333355, 0x222244);
grid.rotation.x = Math.PI / 2;
grid.position.set({cx:.3}, {cy:.3}, {grid_z:.3});
scene.add(grid);

// --- Axes ---
const axes = new THREE.AxesHelper(gridSize * 0.3);
axes.position.set({bbox_min_x:.3}, {bbox_min_y:.3}, {bbox_min_z:.3});
scene.add(axes);

window.addEventListener('resize', () => {{
  camera.aspect = window.innerWidth / window.innerHeight;
  camera.updateProjectionMatrix();
  renderer.setSize(window.innerWidth, window.innerHeight);
}});

function animate() {{
  requestAnimationFrame(animate);
  controls.update();
  renderer.render(scene, camera);
}}
animate();
</script>
</body></html>"##,
        mesh_verts = mesh.vertices.len(),
        mesh_tris = mesh.faces.len(),
        tp_moves = toolpath.moves.len(),
        tp_cut = toolpath.total_cutting_distance(),
        tp_rapid = toolpath.total_rapid_distance(),
        z_min = z_min,
        z_max = z_max,
        cx = center.x,
        cy = center.y,
        cz = center.z,
        cd = cam_dist,
        mesh_verts_data = mesh_verts.trim_end_matches(','),
        mesh_idx_data = mesh_indices.trim_end_matches(','),
        cut_verts_data = cut_verts.trim_end_matches(','),
        cut_colors_data = cut_colors.trim_end_matches(','),
        rapid_verts_data = rapid_verts.trim_end_matches(','),
        grid_size = extent * 1.2,
        grid_z = mesh.bbox.min.z - 0.1,
        bbox_min_x = mesh.bbox.min.x,
        bbox_min_y = mesh.bbox.min.y,
        bbox_min_z = mesh.bbox.min.z,
    );

    html
}

/// Generate a 3D HTML viewer for a toolpath without a mesh.
///
/// Shows the toolpath with a wireframe stock outline box.
/// Useful for 2.5D operations (pocket, profile) where there's no STL mesh.
pub fn toolpath_standalone_3d_html(toolpath: &Toolpath, stock_bounds: Option<[f64; 6]>) -> String {
    let mut html = String::with_capacity(512 * 1024);

    // Compute toolpath bounds
    let mut tp_bbox = BoundingBox3::empty();
    for m in &toolpath.moves {
        tp_bbox.expand_to(m.target);
    }

    // Stock bounds: [x_min, y_min, z_min, x_max, y_max, z_max] or auto from toolpath
    let (sx_min, sy_min, sz_min, sx_max, sy_max, sz_max) = match stock_bounds {
        Some(b) => (b[0], b[1], b[2], b[3], b[4], b[5]),
        None => {
            let margin = 5.0;
            (
                tp_bbox.min.x - margin,
                tp_bbox.min.y - margin,
                tp_bbox.min.z,
                tp_bbox.max.x + margin,
                tp_bbox.max.y + margin,
                0.0,
            )
        }
    };

    let center_x = (sx_min + sx_max) / 2.0;
    let center_y = (sy_min + sy_max) / 2.0;
    let center_z = (sz_min + sz_max) / 2.0;
    let extent = (sx_max - sx_min)
        .max(sy_max - sy_min)
        .max((sz_max - sz_min).abs());
    let cam_dist = extent * 1.8;

    // Serialize toolpath data
    let mut cut_verts = String::new();
    let mut cut_colors = String::new();
    let mut rapid_verts = String::new();
    let mut plunge_verts = String::new();

    let mut z_min = f64::INFINITY;
    let mut z_max = f64::NEG_INFINITY;
    for m in &toolpath.moves {
        if let MoveType::Linear { .. } = m.move_type {
            z_min = z_min.min(m.target.z);
            z_max = z_max.max(m.target.z);
        }
    }
    let z_range = (z_max - z_min).max(1e-6);

    for i in 1..toolpath.moves.len() {
        let from = &toolpath.moves[i - 1].target;
        let to = &toolpath.moves[i].target;

        match toolpath.moves[i].move_type {
            MoveType::Rapid => {
                let _ = write!(
                    rapid_verts,
                    "{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},",
                    from.x, from.y, from.z, to.x, to.y, to.z
                );
            }
            MoveType::Linear { .. } | MoveType::ArcCW { .. } | MoveType::ArcCCW { .. } => {
                let dz = (to.z - from.z).abs();
                let vdx = to.x - from.x;
                let vdy = to.y - from.y;
                let dxy = (vdx * vdx + vdy * vdy).sqrt();

                // Classify as plunge (mostly vertical) vs cutting (mostly horizontal)
                if dz > 0.1 && dxy < 0.1 {
                    let _ = write!(
                        plunge_verts,
                        "{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},",
                        from.x, from.y, from.z, to.x, to.y, to.z
                    );
                } else {
                    let _ = write!(
                        cut_verts,
                        "{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},",
                        from.x, from.y, from.z, to.x, to.y, to.z
                    );
                    for z in [from.z, to.z] {
                        let t = ((z - z_min) / z_range).clamp(0.0, 1.0) as f32;
                        let r = 0.1 + t * 0.15;
                        let g = 0.3 + t * 0.6;
                        let b = 0.85 + t * 0.15;
                        let _ = write!(cut_colors, "{:.3},{:.3},{:.3},", r, g, b);
                    }
                }
            }
        }
    }

    let _ = write!(
        html,
        r##"<!DOCTYPE html>
<html><head>
<meta charset="utf-8">
<title>rs_cam Toolpath Viewer</title>
<style>
  body {{ margin: 0; overflow: hidden; background: #1a1a2e; }}
  #info {{
    position: absolute; top: 10px; left: 10px; color: #ccc;
    font: 13px monospace; background: rgba(0,0,0,0.6); padding: 8px 12px;
    border-radius: 4px; pointer-events: none;
  }}
  #legend {{
    position: absolute; bottom: 10px; left: 10px; color: #aaa;
    font: 12px monospace; background: rgba(0,0,0,0.6); padding: 8px 12px;
    border-radius: 4px;
  }}
</style>
</head><body>
<div id="info">
  Toolpath: {tp_moves} moves, {tp_cut:.0}mm cutting, {tp_rapid:.0}mm rapid<br>
  Z range: {z_min:.2} to {z_max:.2} mm<br>
  Stock: {sw:.1} x {sh:.1} x {sd:.1} mm
</div>
<div id="legend">
  <span style="color:#3388ff">&#9632;</span> Cutting &nbsp;
  <span style="color:#ff4444">&#9632;</span> Rapid &nbsp;
  <span style="color:#ffaa22">&#9632;</span> Plunge &nbsp;
  <span style="color:#556655">&#9632;</span> Stock &nbsp;
  Mouse: orbit | Scroll: zoom | Right-click: pan
</div>

<script type="importmap">
{{
  "imports": {{
    "three": "https://cdn.jsdelivr.net/npm/three@0.170.0/build/three.module.js",
    "three/addons/": "https://cdn.jsdelivr.net/npm/three@0.170.0/examples/jsm/"
  }}
}}
</script>

<script type="module">
import * as THREE from 'three';
import {{ OrbitControls }} from 'three/addons/controls/OrbitControls.js';

const scene = new THREE.Scene();
scene.background = new THREE.Color(0x1a1a2e);

const camera = new THREE.PerspectiveCamera(50, window.innerWidth / window.innerHeight, 0.1, 100000);
camera.position.set({cx:.3} + {cd:.3} * 0.3, {cy:.3} - {cd:.3} * 0.6, {cz:.3} + {cd:.3} * 1.2);

const renderer = new THREE.WebGLRenderer({{ antialias: true }});
renderer.setSize(window.innerWidth, window.innerHeight);
renderer.setPixelRatio(window.devicePixelRatio);
document.body.appendChild(renderer.domElement);

const controls = new OrbitControls(camera, renderer.domElement);
controls.target.set({cx:.3}, {cy:.3}, {cz:.3});
controls.enableDamping = true;

scene.add(new THREE.AmbientLight(0x555555));
const dir1 = new THREE.DirectionalLight(0xffffff, 0.8);
dir1.position.set(1, 1, 2);
scene.add(dir1);

// --- Stock wireframe box ---
const stockGeo = new THREE.BoxGeometry(
  {sx_max:.3} - {sx_min:.3},
  {sy_max:.3} - {sy_min:.3},
  {sz_max:.3} - {sz_min:.3}
);
const stockEdges = new THREE.EdgesGeometry(stockGeo);
const stockLine = new THREE.LineSegments(stockEdges,
  new THREE.LineBasicMaterial({{ color: 0x556655, transparent: true, opacity: 0.6 }}));
stockLine.position.set(
  ({sx_min:.3} + {sx_max:.3}) / 2,
  ({sy_min:.3} + {sy_max:.3}) / 2,
  ({sz_min:.3} + {sz_max:.3}) / 2
);
scene.add(stockLine);

// Semi-transparent stock top face
const topGeo = new THREE.PlaneGeometry({sx_max:.3} - {sx_min:.3}, {sy_max:.3} - {sy_min:.3});
const topMat = new THREE.MeshBasicMaterial({{
  color: 0x556655, transparent: true, opacity: 0.15, side: THREE.DoubleSide
}});
const topMesh = new THREE.Mesh(topGeo, topMat);
topMesh.position.set(
  ({sx_min:.3} + {sx_max:.3}) / 2,
  ({sy_min:.3} + {sy_max:.3}) / 2,
  {sz_max:.3}
);
scene.add(topMesh);

// --- Cutting toolpath ---
const cutVerts = new Float32Array([{cut_verts_data}]);
const cutColors = new Float32Array([{cut_colors_data}]);
if (cutVerts.length > 0) {{
  const cutGeo = new THREE.BufferGeometry();
  cutGeo.setAttribute('position', new THREE.BufferAttribute(cutVerts, 3));
  cutGeo.setAttribute('color', new THREE.BufferAttribute(cutColors, 3));
  const cutMat = new THREE.LineBasicMaterial({{ vertexColors: true, linewidth: 1 }});
  scene.add(new THREE.LineSegments(cutGeo, cutMat));
}}

// --- Plunge moves ---
const plungeVerts = new Float32Array([{plunge_verts_data}]);
if (plungeVerts.length > 0) {{
  const plungeGeo = new THREE.BufferGeometry();
  plungeGeo.setAttribute('position', new THREE.BufferAttribute(plungeVerts, 3));
  const plungeMat = new THREE.LineBasicMaterial({{ color: 0xffaa22, linewidth: 1, transparent: true, opacity: 0.7 }});
  scene.add(new THREE.LineSegments(plungeGeo, plungeMat));
}}

// --- Rapid toolpath ---
const rapidVerts = new Float32Array([{rapid_verts_data}]);
if (rapidVerts.length > 0) {{
  const rapidGeo = new THREE.BufferGeometry();
  rapidGeo.setAttribute('position', new THREE.BufferAttribute(rapidVerts, 3));
  const rapidMat = new THREE.LineBasicMaterial({{ color: 0xff4444, linewidth: 1, transparent: true, opacity: 0.3 }});
  scene.add(new THREE.LineSegments(rapidGeo, rapidMat));
}}

// --- Grid ---
const gridSize = {grid_size:.0};
const grid = new THREE.GridHelper(gridSize, 20, 0x333355, 0x222244);
grid.rotation.x = Math.PI / 2;
grid.position.set({cx:.3}, {cy:.3}, {sz_min:.3} - 0.1);
scene.add(grid);

// --- Axes ---
const axes = new THREE.AxesHelper(gridSize * 0.3);
axes.position.set({sx_min:.3}, {sy_min:.3}, {sz_min:.3});
scene.add(axes);

window.addEventListener('resize', () => {{
  camera.aspect = window.innerWidth / window.innerHeight;
  camera.updateProjectionMatrix();
  renderer.setSize(window.innerWidth, window.innerHeight);
}});

function animate() {{
  requestAnimationFrame(animate);
  controls.update();
  renderer.render(scene, camera);
}}
animate();
</script>
</body></html>"##,
        tp_moves = toolpath.moves.len(),
        tp_cut = toolpath.total_cutting_distance(),
        tp_rapid = toolpath.total_rapid_distance(),
        z_min = z_min,
        z_max = z_max,
        sw = sx_max - sx_min,
        sh = sy_max - sy_min,
        sd = (sz_max - sz_min).abs(),
        cx = center_x,
        cy = center_y,
        cz = center_z,
        cd = cam_dist,
        sx_min = sx_min,
        sy_min = sy_min,
        sz_min = sz_min,
        sx_max = sx_max,
        sy_max = sy_max,
        sz_max = sz_max,
        cut_verts_data = cut_verts.trim_end_matches(','),
        cut_colors_data = cut_colors.trim_end_matches(','),
        plunge_verts_data = plunge_verts.trim_end_matches(','),
        rapid_verts_data = rapid_verts.trim_end_matches(','),
        grid_size = extent * 1.2,
    );

    html
}

/// Generate a 3D HTML viewer showing the simulated heightmap surface + toolpath lines,
/// with animated replay support.
///
/// The heightmap mesh is rendered with vertex colors (wood tones) and the toolpath
/// is overlaid as colored lines. A "Replay" button lets the user watch the tool
/// cut the material in real-time with a 3D tool model. An optional ghost source mesh
/// can be shown for 3D operations (drop-cutter, waterline).
/// `annotations`: optional `(move_index, label)` pairs shown during replay.
/// The label for the highest move_index <= current moveIdx is displayed.
pub fn simulation_3d_html(
    stock: &TriDexelStock,
    toolpath: &Toolpath,
    source_mesh: Option<&TriangleMesh>,
    cutter: &dyn MillingCutter,
    annotations: &[(usize, String)],
) -> String {
    let hm_mesh = dexel_stock_to_mesh(stock);
    let hm_bbox = stock.stock_bbox;

    let center = hm_bbox.center();
    let extent = (hm_bbox.max.x - hm_bbox.min.x)
        .max(hm_bbox.max.y - hm_bbox.min.y)
        .max((hm_bbox.max.z - hm_bbox.min.z).abs().max(1.0));
    let cam_dist = extent * 1.5;

    // Serialize heightmap mesh (final state)
    let mut hm_verts = String::new();
    for v in hm_mesh.vertices.chunks(3) {
        let _ = write!(hm_verts, "{},{},{},", v[0], v[1], v[2]);
    }
    let mut hm_colors = String::new();
    for c in hm_mesh.colors.chunks(3) {
        let _ = write!(hm_colors, "{:.3},{:.3},{:.3},", c[0], c[1], c[2]);
    }
    let mut hm_indices = String::new();
    for idx in &hm_mesh.indices {
        let _ = write!(hm_indices, "{},", idx);
    }

    // Serialize toolpath lines for display
    let mut cut_verts = String::new();
    let mut cut_colors = String::new();
    let mut rapid_verts = String::new();

    let mut z_min = f64::INFINITY;
    let mut z_max = f64::NEG_INFINITY;
    for m in &toolpath.moves {
        match m.move_type {
            MoveType::Linear { .. } | MoveType::ArcCW { .. } | MoveType::ArcCCW { .. } => {
                z_min = z_min.min(m.target.z);
                z_max = z_max.max(m.target.z);
            }
            _ => {}
        }
    }
    let z_range = (z_max - z_min).max(1e-6);

    for i in 1..toolpath.moves.len() {
        let from = &toolpath.moves[i - 1].target;
        let to = &toolpath.moves[i].target;

        match toolpath.moves[i].move_type {
            MoveType::Rapid => {
                let _ = write!(
                    rapid_verts,
                    "{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},",
                    from.x, from.y, from.z, to.x, to.y, to.z
                );
            }
            MoveType::Linear { .. } | MoveType::ArcCW { .. } | MoveType::ArcCCW { .. } => {
                let _ = write!(
                    cut_verts,
                    "{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},",
                    from.x, from.y, from.z, to.x, to.y, to.z
                );
                for z in [from.z, to.z] {
                    let t = ((z - z_min) / z_range).clamp(0.0, 1.0) as f32;
                    let r = 0.1 + t * 0.1;
                    let g = 0.3 + t * 0.6;
                    let b = 0.9 + t * 0.1;
                    let _ = write!(cut_colors, "{:.3},{:.3},{:.3},", r, g, b);
                }
            }
        }
    }

    // Optional source mesh (ghost)
    let mut src_mesh_verts = String::new();
    let mut src_mesh_indices = String::new();
    let has_source_mesh = source_mesh.is_some();
    if let Some(mesh) = source_mesh {
        for v in &mesh.vertices {
            let _ = write!(src_mesh_verts, "{:.4},{:.4},{:.4},", v.x, v.y, v.z);
        }
        for tri in &mesh.triangles {
            let _ = write!(src_mesh_indices, "{},{},{},", tri[0], tri[1], tri[2]);
        }
    }

    // Serialize tool profile as lookup table (sampled at 50 radii)
    let num_profile_samples = 50;
    let tool_radius = cutter.radius();
    let mut tool_profile = String::new();
    for i in 0..=num_profile_samples {
        let r = (i as f64 / num_profile_samples as f64) * tool_radius;
        let h = cutter.height_at_radius(r).unwrap_or(-1.0);
        let _ = write!(tool_profile, "{:.4},", h);
    }

    // Serialize linearized toolpath for animation (arcs pre-linearized)
    // Format: flat array [x, y, z, type, x, y, z, type, ...]
    // type: 0 = rapid, 1 = cutting
    let mut anim_tp = String::new();
    let mut anim_move_count: usize = 0;
    if !toolpath.moves.is_empty() {
        let first = &toolpath.moves[0].target;
        let _ = write!(anim_tp, "{:.3},{:.3},{:.3},0,", first.x, first.y, first.z);
        anim_move_count += 1;

        for i in 1..toolpath.moves.len() {
            let start = toolpath.moves[i - 1].target;
            let end = toolpath.moves[i].target;

            match toolpath.moves[i].move_type {
                MoveType::Rapid => {
                    let _ = write!(anim_tp, "{:.3},{:.3},{:.3},0,", end.x, end.y, end.z);
                    anim_move_count += 1;
                }
                MoveType::Linear { .. } => {
                    let _ = write!(anim_tp, "{:.3},{:.3},{:.3},1,", end.x, end.y, end.z);
                    anim_move_count += 1;
                }
                MoveType::ArcCW { i, j, .. } => {
                    let points = linearize_arc(start, end, i, j, true, stock.z_grid.cell_size);
                    for p in points.iter().skip(1) {
                        let _ = write!(anim_tp, "{:.3},{:.3},{:.3},1,", p.x, p.y, p.z);
                        anim_move_count += 1;
                    }
                }
                MoveType::ArcCCW { i, j, .. } => {
                    let points = linearize_arc(start, end, i, j, false, stock.z_grid.cell_size);
                    for p in points.iter().skip(1) {
                        let _ = write!(anim_tp, "{:.3},{:.3},{:.3},1,", p.x, p.y, p.z);
                        anim_move_count += 1;
                    }
                }
            }
        }
    }

    // Serialize annotations as JS array: [[moveIdx, "label"], ...]
    let mut annotations_js = String::new();
    if !annotations.is_empty() {
        for (idx, label) in annotations {
            let escaped = label.replace('\\', "\\\\").replace('\"', "\\\"");
            let _ = write!(annotations_js, "[{idx},\"{escaped}\"],");
        }
        // Trim trailing comma
        annotations_js = annotations_js.trim_end_matches(',').to_string();
    }

    // Build Three.js LatheGeometry profile points for the tool model
    let mut lathe_profile = String::new();
    // Profile curve from center outward
    for i in 0..=num_profile_samples {
        let r = (i as f64 / num_profile_samples as f64) * tool_radius;
        if let Some(h) = cutter.height_at_radius(r) {
            let _ = write!(lathe_profile, "new THREE.Vector2({:.3},{:.4}),", r, h);
        }
    }
    // Shaft extending upward
    let shaft_h = tool_radius * 4.0;
    let _ = write!(
        lathe_profile,
        "new THREE.Vector2({:.3},{:.3}),",
        tool_radius, shaft_h
    );
    let _ = write!(lathe_profile, "new THREE.Vector2(0,{:.3}),", shaft_h);

    let mut html = String::with_capacity(2 * 1024 * 1024);

    let _ = write!(
        html,
        r##"<!DOCTYPE html>
<html><head>
<meta charset="utf-8">
<title>rs_cam Simulation Viewer</title>
<style>
  body {{ margin: 0; overflow: hidden; background: #1a1a2e; }}
  #info {{
    position: absolute; top: 10px; left: 10px; color: #ccc;
    font: 13px monospace; background: rgba(0,0,0,0.6); padding: 8px 12px;
    border-radius: 4px; pointer-events: none;
  }}
  #legend {{
    position: absolute; bottom: 10px; left: 10px; color: #aaa;
    font: 12px monospace; background: rgba(0,0,0,0.6); padding: 8px 12px;
    border-radius: 4px;
  }}
  #controls {{
    position: absolute; bottom: 50px; left: 50%; transform: translateX(-50%);
    color: #ccc; font: 13px monospace; background: rgba(0,0,0,0.75);
    padding: 10px 18px; border-radius: 6px; display: flex; align-items: center; gap: 14px;
    user-select: none;
  }}
  #controls button {{
    background: #445; border: 1px solid #667; color: #ddd; padding: 5px 14px;
    border-radius: 4px; cursor: pointer; font: 13px monospace;
  }}
  #controls button:hover {{ background: #556; }}
  #controls button.active {{ background: #664; border-color: #aa8; }}
  #speedRange {{ width: 100px; cursor: pointer; }}
  #progressBar {{
    width: 200px; height: 6px; background: #333; border-radius: 3px;
    cursor: pointer; position: relative;
  }}
  #progressFill {{
    height: 100%; background: #6a6; border-radius: 3px; width: 100%;
    pointer-events: none;
  }}
</style>
</head><body>
<div id="info">
  Simulation: {hm_rows}x{hm_cols} heightmap ({cell_size:.2}mm resolution)<br>
  Toolpath: {tp_moves} moves ({anim_moves} linearized), {tp_cut:.0}mm cutting<br>
  Z range: {z_min:.2} to {z_max:.2} mm
</div>
<div id="controls">
  <button id="replayBtn">&#9654; Replay</button>
  <button id="skipBtn">&#9646;&#9646; End</button>
  <label>Speed: <input type="range" id="speedRange" min="0" max="100" value="40" step="1">
  <span id="speedVal">1.0</span>x</label>
  <div id="progressBar"><div id="progressFill"></div></div>
  <span id="moveInfo">Complete</span>
  <span id="annotLabel" style="margin-left:12px;color:#aef;font-weight:bold"></span>
</div>
<div id="legend">
  <span style="color:#c49a6c">&#9632;</span> Uncut &nbsp;
  <span style="color:#73401a">&#9632;</span> Cut &nbsp;
  <span style="color:#3388ff">&#9632;</span> Toolpath &nbsp;
  <span style="color:#ff4444">&#9632;</span> Rapid &nbsp;
  <span style="color:#aabbcc">&#9632;</span> Tool &nbsp;
  {ghost_legend}
  Mouse: orbit | Scroll: zoom | Right-click: pan
</div>

<script type="importmap">
{{
  "imports": {{
    "three": "https://cdn.jsdelivr.net/npm/three@0.170.0/build/three.module.js",
    "three/addons/": "https://cdn.jsdelivr.net/npm/three@0.170.0/examples/jsm/"
  }}
}}
</script>

<script type="module">
import * as THREE from 'three';
import {{ OrbitControls }} from 'three/addons/controls/OrbitControls.js';

const scene = new THREE.Scene();
scene.background = new THREE.Color(0x1a1a2e);

const camera = new THREE.PerspectiveCamera(50, window.innerWidth / window.innerHeight, 0.1, 100000);
camera.position.set({cx:.3} + {cd:.3} * 0.5, {cy:.3} - {cd:.3} * 0.8, {cz:.3} + {cd:.3} * 1.0);

const renderer = new THREE.WebGLRenderer({{ antialias: true }});
renderer.setSize(window.innerWidth, window.innerHeight);
renderer.setPixelRatio(window.devicePixelRatio);
document.body.appendChild(renderer.domElement);

const controls = new OrbitControls(camera, renderer.domElement);
controls.target.set({cx:.3}, {cy:.3}, {cz:.3});
controls.enableDamping = true;

// Lighting
scene.add(new THREE.AmbientLight(0x666666));
const dir1 = new THREE.DirectionalLight(0xffffff, 0.9);
dir1.position.set(1, 1, 2);
scene.add(dir1);
const dir2 = new THREE.DirectionalLight(0x4488aa, 0.3);
dir2.position.set(-1, -0.5, 0.5);
scene.add(dir2);

// --- Final heightmap mesh data (for restoring after animation) ---
const finalVerts = new Float32Array([{hm_verts_data}]);
const finalColors = new Float32Array([{hm_colors_data}]);

// --- Heightmap surface ---
const hmVerts = new Float32Array(finalVerts);
const hmColors = new Float32Array(finalColors);
const hmIdx = new Uint32Array([{hm_idx_data}]);
const hmGeo = new THREE.BufferGeometry();
hmGeo.setAttribute('position', new THREE.BufferAttribute(hmVerts, 3));
hmGeo.setAttribute('color', new THREE.BufferAttribute(hmColors, 3));
hmGeo.setIndex(new THREE.BufferAttribute(hmIdx, 1));
hmGeo.computeVertexNormals();

const hmMat = new THREE.MeshPhongMaterial({{
  vertexColors: true,
  side: THREE.DoubleSide,
  flatShading: false,
  shininess: 20,
}});
scene.add(new THREE.Mesh(hmGeo, hmMat));

// --- Ghost source mesh ---
{ghost_mesh_js}

// --- Cutting toolpath ---
const cutVerts = new Float32Array([{cut_verts_data}]);
const cutColors = new Float32Array([{cut_colors_data}]);
if (cutVerts.length > 0) {{
  const cutGeo = new THREE.BufferGeometry();
  cutGeo.setAttribute('position', new THREE.BufferAttribute(cutVerts, 3));
  cutGeo.setAttribute('color', new THREE.BufferAttribute(cutColors, 3));
  const cutMat = new THREE.LineBasicMaterial({{ vertexColors: true, linewidth: 1, transparent: true, opacity: 0.4 }});
  scene.add(new THREE.LineSegments(cutGeo, cutMat));
}}

// --- Rapid toolpath ---
const rapidVerts = new Float32Array([{rapid_verts_data}]);
if (rapidVerts.length > 0) {{
  const rapidGeo = new THREE.BufferGeometry();
  rapidGeo.setAttribute('position', new THREE.BufferAttribute(rapidVerts, 3));
  const rapidMat = new THREE.LineBasicMaterial({{ color: 0xff4444, linewidth: 1, transparent: true, opacity: 0.2 }});
  scene.add(new THREE.LineSegments(rapidGeo, rapidMat));
}}

// --- Grid helper ---
const gridSize = {grid_size:.0};
const grid = new THREE.GridHelper(gridSize, 20, 0x333355, 0x222244);
grid.rotation.x = Math.PI / 2;
grid.position.set({cx:.3}, {cy:.3}, {grid_z:.3});
scene.add(grid);

// --- Axes ---
const axes = new THREE.AxesHelper(gridSize * 0.3);
axes.position.set({bbox_min_x:.3}, {bbox_min_y:.3}, {bbox_min_z:.3});
scene.add(axes);

// ====== ANIMATION ENGINE ======

// Tool profile lookup table (sampled at equal radii from 0 to toolRadius)
const toolProfile = new Float32Array([{tool_profile_data}]);
const toolRadius = {tool_radius:.4};
const numProfileSamples = {num_profile_samples};

// Heightmap grid params
const hmRows = {hm_rows};
const hmCols = {hm_cols};
const originX = {origin_x:.4};
const originY = {origin_y:.4};
const cellSize = {cell_size_val:.4};
const stockTopZ = {stock_top_z:.4};
const zRange = {z_range_val:.6};

// Working heightmap grid (for animation stamping)
const hmGrid = new Float64Array(hmRows * hmCols);

// Linearized toolpath: flat [x,y,z,type, x,y,z,type, ...]
// type: 0=rapid, 1=cutting
const animTP = new Float32Array([{anim_tp_data}]);
const animMoveCount = {anim_move_count};

// Annotations: [moveIdx, label] pairs sorted by moveIdx ascending
const annotations = [{annotations_js}];

// Tool 3D model (LatheGeometry from profile)
const profilePts = [{lathe_profile_data}];
const toolGeo = new THREE.LatheGeometry(profilePts, 24);
toolGeo.rotateX(Math.PI / 2);
const toolMat = new THREE.MeshPhongMaterial({{
  color: 0xaabbcc, transparent: true, opacity: 0.7,
  side: THREE.DoubleSide, shininess: 60,
}});
const toolMesh = new THREE.Mesh(toolGeo, toolMat);
toolMesh.visible = false;
scene.add(toolMesh);

// Animation state
let playing = false;
let moveIdx = 1;
let lastTime = 0;

function heightAtRadius(r) {{
  if (r > toolRadius) return -1;
  const t = (r / toolRadius) * numProfileSamples;
  const i = Math.min(Math.floor(t), numProfileSamples - 1);
  const f = t - i;
  const h0 = toolProfile[i], h1 = toolProfile[i + 1];
  if (h0 < 0 || h1 < 0) return -1;
  return h0 * (1 - f) + h1 * f;
}}

function stampToolAt(cx, cy, tipZ) {{
  const rSq = toolRadius * toolRadius;
  const colMin = Math.max(0, Math.floor((cx - toolRadius - originX) / cellSize));
  const colMax = Math.min(hmCols - 1, Math.ceil((cx + toolRadius - originX) / cellSize));
  const rowMin = Math.max(0, Math.floor((cy - toolRadius - originY) / cellSize));
  const rowMax = Math.min(hmRows - 1, Math.ceil((cy + toolRadius - originY) / cellSize));

  for (let row = rowMin; row <= rowMax; row++) {{
    const cellY = originY + row * cellSize;
    const dy = cellY - cy;
    const dySq = dy * dy;
    if (dySq > rSq) continue;
    for (let col = colMin; col <= colMax; col++) {{
      const cellX = originX + col * cellSize;
      const dx = cellX - cx;
      const distSq = dx * dx + dySq;
      if (distSq > rSq) continue;
      const h = heightAtRadius(Math.sqrt(distSq));
      if (h >= 0) {{
        const idx = row * hmCols + col;
        const cutZ = tipZ + h;
        if (cutZ < hmGrid[idx]) hmGrid[idx] = cutZ;
      }}
    }}
  }}
}}

function stampSegment(x0, y0, z0, x1, y1, z1) {{
  const dx = x1-x0, dy = y1-y0, dz = z1-z0;
  const len = Math.sqrt(dx*dx + dy*dy + dz*dz);
  const samples = Math.max(1, Math.ceil(len / cellSize));
  for (let i = 0; i <= samples; i++) {{
    const t = i / samples;
    stampToolAt(x0 + t*dx, y0 + t*dy, z0 + t*dz);
  }}
}}

function updateMeshFromGrid() {{
  const pos = hmGeo.attributes.position.array;
  const col = hmGeo.attributes.color.array;
  for (let row = 0; row < hmRows; row++) {{
    for (let c = 0; c < hmCols; c++) {{
      const idx = row * hmCols + c;
      const z = hmGrid[idx];
      pos[idx * 3 + 2] = z;
      const depthT = Math.max(0, Math.min(1, (stockTopZ - z) / zRange));
      col[idx * 3]     = 0.76 + (0.45 - 0.76) * depthT;
      col[idx * 3 + 1] = 0.60 + (0.25 - 0.60) * depthT;
      col[idx * 3 + 2] = 0.42 + (0.10 - 0.42) * depthT;
    }}
  }}
  hmGeo.attributes.position.needsUpdate = true;
  hmGeo.attributes.color.needsUpdate = true;
  hmGeo.computeVertexNormals();
}}

function restoreFinalState() {{
  hmGeo.attributes.position.array.set(finalVerts);
  hmGeo.attributes.color.array.set(finalColors);
  hmGeo.attributes.position.needsUpdate = true;
  hmGeo.attributes.color.needsUpdate = true;
  hmGeo.computeVertexNormals();
  toolMesh.visible = false;
  moveIdx = animMoveCount;
  updateUI();
}}

function resetToStock() {{
  hmGrid.fill(stockTopZ);
  for (let i = 0; i < hmRows * hmCols; i++) {{
    hmGeo.attributes.position.array[i * 3 + 2] = stockTopZ;
    hmGeo.attributes.color.array[i * 3]     = 0.76;
    hmGeo.attributes.color.array[i * 3 + 1] = 0.60;
    hmGeo.attributes.color.array[i * 3 + 2] = 0.42;
  }}
  hmGeo.attributes.position.needsUpdate = true;
  hmGeo.attributes.color.needsUpdate = true;
  hmGeo.computeVertexNormals();
  moveIdx = 1;
}}

// Compute total path distance for speed calibration
let totalDist = 0;
for (let i = 1; i < animMoveCount; i++) {{
  const b = i * 4;
  const pb = (i-1) * 4;
  const dx = animTP[b] - animTP[pb], dy = animTP[b+1] - animTP[pb+1], dz = animTP[b+2] - animTP[pb+2];
  totalDist += Math.sqrt(dx*dx + dy*dy + dz*dz);
}}
const baseSpeed = Math.max(totalDist / 20, 10); // complete in ~20s at 1x

function processFrame(dt) {{
  const sliderVal = parseFloat(document.getElementById('speedRange').value);
  const speedMult = Math.pow(10, (sliderVal - 50) / 30); // 0→0.04x, 50→1x, 100→215x
  const speed = baseSpeed * speedMult;
  let budget = speed * dt; // mm to advance this frame
  let meshDirty = false;

  while (budget > 0 && moveIdx < animMoveCount) {{
    const b = moveIdx * 4;
    const pb = (moveIdx - 1) * 4;
    const x0 = animTP[pb], y0 = animTP[pb+1], z0 = animTP[pb+2];
    const x1 = animTP[b], y1 = animTP[b+1], z1 = animTP[b+2];
    const isRapid = animTP[b+3] === 0;

    const dx = x1-x0, dy = y1-y0, dz = z1-z0;
    const segLen = Math.sqrt(dx*dx + dy*dy + dz*dz);

    if (!isRapid && segLen > 0.001) {{
      stampSegment(x0, y0, z0, x1, y1, z1);
      meshDirty = true;
    }}

    // Position tool at end of this move
    toolMesh.position.set(x1, y1, z1);
    toolMesh.visible = true;

    budget -= isRapid ? segLen * 0.1 : segLen; // rapids are fast
    moveIdx++;
  }}

  if (meshDirty) updateMeshFromGrid();

  if (moveIdx >= animMoveCount) {{
    playing = false;
    restoreFinalState();
    document.getElementById('replayBtn').textContent = '\u25B6 Replay';
  }}
  updateUI();
}}

function currentAnnotation() {{
  if (typeof annotations === 'undefined' || annotations.length === 0) return '';
  let label = '';
  for (let i = 0; i < annotations.length; i++) {{
    if (annotations[i][0] <= moveIdx) label = annotations[i][1];
    else break;
  }}
  return label;
}}

function updateUI() {{
  const pct = animMoveCount > 1 ? ((moveIdx - 1) / (animMoveCount - 1)) * 100 : 100;
  document.getElementById('progressFill').style.width = pct + '%';
  document.getElementById('moveInfo').textContent =
    playing ? (moveIdx + ' / ' + animMoveCount) : 'Complete';
  const al = document.getElementById('annotLabel');
  if (al) al.textContent = playing ? currentAnnotation() : '';
}}

// UI event handlers
document.getElementById('replayBtn').addEventListener('click', () => {{
  if (playing) {{
    playing = false;
    document.getElementById('replayBtn').textContent = '\u25B6 Resume';
  }} else {{
    if (moveIdx >= animMoveCount) {{
      resetToStock();
    }}
    playing = true;
    lastTime = performance.now() / 1000;
    document.getElementById('replayBtn').textContent = '\u23F8 Pause';
  }}
}});

document.getElementById('skipBtn').addEventListener('click', () => {{
  playing = false;
  restoreFinalState();
  document.getElementById('replayBtn').textContent = '\u25B6 Replay';
}});

document.getElementById('speedRange').addEventListener('input', (e) => {{
  const mult = Math.pow(10, (parseFloat(e.target.value) - 50) / 30);
  document.getElementById('speedVal').textContent = mult < 1 ? mult.toFixed(2) : mult.toFixed(0);
}});
// Set initial label
document.getElementById('speedVal').textContent = '1.0';

window.addEventListener('resize', () => {{
  camera.aspect = window.innerWidth / window.innerHeight;
  camera.updateProjectionMatrix();
  renderer.setSize(window.innerWidth, window.innerHeight);
}});

const clock = new THREE.Clock();
function animate() {{
  requestAnimationFrame(animate);
  const dt = clock.getDelta();
  if (playing) processFrame(Math.min(dt, 0.05));
  controls.update();
  renderer.render(scene, camera);
}}
animate();
</script>
</body></html>"##,
        hm_rows = stock.z_grid.rows,
        hm_cols = stock.z_grid.cols,
        cell_size = stock.z_grid.cell_size,
        tp_moves = toolpath.moves.len(),
        anim_moves = anim_move_count,
        tp_cut = toolpath.total_cutting_distance(),
        z_min = z_min,
        z_max = z_max,
        ghost_legend = if has_source_mesh {
            "<span style=\"color:#88aa88\">&#9632;</span> Source mesh &nbsp;"
        } else {
            ""
        },
        cx = center.x,
        cy = center.y,
        cz = center.z,
        cd = cam_dist,
        hm_verts_data = hm_verts.trim_end_matches(','),
        hm_colors_data = hm_colors.trim_end_matches(','),
        hm_idx_data = hm_indices.trim_end_matches(','),
        ghost_mesh_js = if has_source_mesh {
            format!(
                r#"const srcVerts = new Float32Array([{}]);
const srcIdx = new Uint32Array([{}]);
const srcGeo = new THREE.BufferGeometry();
srcGeo.setAttribute('position', new THREE.BufferAttribute(srcVerts, 3));
srcGeo.setIndex(new THREE.BufferAttribute(srcIdx, 1));
srcGeo.computeVertexNormals();
const srcMat = new THREE.MeshPhongMaterial({{
  color: 0x88aa88, transparent: true, opacity: 0.2,
  side: THREE.DoubleSide, depthWrite: false,
}});
scene.add(new THREE.Mesh(srcGeo, srcMat));"#,
                src_mesh_verts.trim_end_matches(','),
                src_mesh_indices.trim_end_matches(','),
            )
        } else {
            String::new()
        },
        cut_verts_data = cut_verts.trim_end_matches(','),
        cut_colors_data = cut_colors.trim_end_matches(','),
        rapid_verts_data = rapid_verts.trim_end_matches(','),
        tool_profile_data = tool_profile.trim_end_matches(','),
        tool_radius = tool_radius,
        num_profile_samples = num_profile_samples,
        origin_x = stock.z_grid.origin_u,
        origin_y = stock.z_grid.origin_v,
        cell_size_val = stock.z_grid.cell_size,
        stock_top_z = stock.stock_bbox.max.z,
        z_range_val = (stock.stock_bbox.max.z - hm_bbox.min.z).max(1e-6),
        anim_tp_data = anim_tp.trim_end_matches(','),
        anim_move_count = anim_move_count,
        annotations_js = annotations_js,
        lathe_profile_data = lathe_profile.trim_end_matches(','),
        grid_size = extent * 1.2,
        grid_z = hm_bbox.min.z - 0.1,
        bbox_min_x = hm_bbox.min.x,
        bbox_min_y = hm_bbox.min.y,
        bbox_min_z = hm_bbox.min.z,
    );

    html
}

/// A simulation phase: one toolpath executed with one cutter.
pub struct SimPhase<'a> {
    pub toolpath: &'a Toolpath,
    pub cutter: &'a dyn MillingCutter,
    pub label: String,
}

/// Generate a stacked multi-tool simulation viewer.
///
/// Plays through multiple phases sequentially (e.g. roughing then rest
/// machining), switching tool profile between phases. The heightmap
/// shows the final combined result.
pub fn stacked_simulation_3d_html(
    phases: &[SimPhase],
    stock: &TriDexelStock,
    source_mesh: Option<&TriangleMesh>,
) -> String {
    let hm_mesh = dexel_stock_to_mesh(stock);
    let hm_bbox = stock.stock_bbox;

    let center = hm_bbox.center();
    let extent = (hm_bbox.max.x - hm_bbox.min.x)
        .max(hm_bbox.max.y - hm_bbox.min.y)
        .max((hm_bbox.max.z - hm_bbox.min.z).abs().max(1.0));
    let cam_dist = extent * 1.5;

    // Serialize heightmap mesh (final state)
    let mut hm_verts = String::new();
    for v in hm_mesh.vertices.chunks(3) {
        let _ = write!(hm_verts, "{},{},{},", v[0], v[1], v[2]);
    }
    let mut hm_colors = String::new();
    for c in hm_mesh.colors.chunks(3) {
        let _ = write!(hm_colors, "{:.3},{:.3},{:.3},", c[0], c[1], c[2]);
    }
    let mut hm_indices_str = String::new();
    for idx in &hm_mesh.indices {
        let _ = write!(hm_indices_str, "{},", idx);
    }

    // Serialize combined toolpath lines for display (all phases)
    let mut cut_verts = String::new();
    let mut cut_colors = String::new();
    let mut rapid_verts = String::new();

    let mut global_z_min = f64::INFINITY;
    let mut global_z_max = f64::NEG_INFINITY;
    for phase in phases {
        for m in &phase.toolpath.moves {
            match m.move_type {
                MoveType::Linear { .. } | MoveType::ArcCW { .. } | MoveType::ArcCCW { .. } => {
                    global_z_min = global_z_min.min(m.target.z);
                    global_z_max = global_z_max.max(m.target.z);
                }
                _ => {}
            }
        }
    }
    let z_range_tp = (global_z_max - global_z_min).max(1e-6);

    for phase in phases {
        let tp = phase.toolpath;
        for i in 1..tp.moves.len() {
            let from = &tp.moves[i - 1].target;
            let to = &tp.moves[i].target;
            match tp.moves[i].move_type {
                MoveType::Rapid => {
                    let _ = write!(
                        rapid_verts,
                        "{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},",
                        from.x, from.y, from.z, to.x, to.y, to.z
                    );
                }
                MoveType::Linear { .. } | MoveType::ArcCW { .. } | MoveType::ArcCCW { .. } => {
                    let _ = write!(
                        cut_verts,
                        "{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},",
                        from.x, from.y, from.z, to.x, to.y, to.z
                    );
                    for z in [from.z, to.z] {
                        let t = ((z - global_z_min) / z_range_tp).clamp(0.0, 1.0) as f32;
                        let r = 0.1 + t * 0.1;
                        let g = 0.3 + t * 0.6;
                        let b = 0.9 + t * 0.1;
                        let _ = write!(cut_colors, "{:.3},{:.3},{:.3},", r, g, b);
                    }
                }
            }
        }
    }

    // Optional source mesh (ghost)
    let mut src_mesh_verts = String::new();
    let mut src_mesh_indices_str = String::new();
    let has_source_mesh = source_mesh.is_some();
    if let Some(mesh) = source_mesh {
        for v in &mesh.vertices {
            let _ = write!(src_mesh_verts, "{:.4},{:.4},{:.4},", v.x, v.y, v.z);
        }
        for tri in &mesh.triangles {
            let _ = write!(src_mesh_indices_str, "{},{},{},", tri[0], tri[1], tri[2]);
        }
    }

    // Serialize multiple tool profiles
    let num_profile_samples = 50;
    let mut tool_profiles_js = String::new();
    let mut tool_radii_js = String::new();
    let mut lathe_profiles_js = String::new();
    let mut phase_labels_js = String::new();

    for (ti, phase) in phases.iter().enumerate() {
        let r = phase.cutter.radius();
        let _ = write!(tool_radii_js, "{:.4},", r);

        let mut profile = String::new();
        for i in 0..=num_profile_samples {
            let dist = (i as f64 / num_profile_samples as f64) * r;
            let h = phase.cutter.height_at_radius(dist).unwrap_or(-1.0);
            let _ = write!(profile, "{:.4},", h);
        }
        let _ = write!(
            tool_profiles_js,
            "new Float32Array([{}]),",
            profile.trim_end_matches(',')
        );

        let mut lathe = String::new();
        for i in 0..=num_profile_samples {
            let dist = (i as f64 / num_profile_samples as f64) * r;
            if let Some(h) = phase.cutter.height_at_radius(dist) {
                let _ = write!(lathe, "new THREE.Vector2({:.3},{:.4}),", dist, h);
            }
        }
        let shaft_h = r * 4.0;
        let _ = write!(lathe, "new THREE.Vector2({:.3},{:.3}),", r, shaft_h);
        let _ = write!(lathe, "new THREE.Vector2(0,{:.3}),", shaft_h);
        let _ = write!(lathe_profiles_js, "[{}],", lathe.trim_end_matches(','));

        let _ = write!(phase_labels_js, "\"Phase {}: {}\",", ti + 1, phase.label);
        let _ = ti; // suppress unused warning
    }

    // Serialize animation with tool-change markers.
    // type: 0=rapid, 1=cutting, 3+N = switch to tool N
    let mut anim_tp = String::new();
    let mut anim_move_count: usize = 0;
    let mut total_moves: usize = 0;
    let mut total_cut_dist: f64 = 0.0;

    for (ti, phase) in phases.iter().enumerate() {
        let tp = phase.toolpath;
        total_cut_dist += tp.total_cutting_distance();
        if tp.moves.is_empty() {
            continue;
        }

        // Tool-change marker
        let _ = write!(anim_tp, "0,0,0,{},", 3 + ti);
        anim_move_count += 1;

        let first = &tp.moves[0].target;
        let _ = write!(anim_tp, "{:.3},{:.3},{:.3},0,", first.x, first.y, first.z);
        anim_move_count += 1;

        for i in 1..tp.moves.len() {
            let start = tp.moves[i - 1].target;
            let end = tp.moves[i].target;
            match tp.moves[i].move_type {
                MoveType::Rapid => {
                    let _ = write!(anim_tp, "{:.3},{:.3},{:.3},0,", end.x, end.y, end.z);
                    anim_move_count += 1;
                }
                MoveType::Linear { .. } => {
                    let _ = write!(anim_tp, "{:.3},{:.3},{:.3},1,", end.x, end.y, end.z);
                    anim_move_count += 1;
                }
                MoveType::ArcCW { i: ai, j: aj, .. } => {
                    let points = linearize_arc(start, end, ai, aj, true, stock.z_grid.cell_size);
                    for p in points.iter().skip(1) {
                        let _ = write!(anim_tp, "{:.3},{:.3},{:.3},1,", p.x, p.y, p.z);
                        anim_move_count += 1;
                    }
                }
                MoveType::ArcCCW { i: ai, j: aj, .. } => {
                    let points = linearize_arc(start, end, ai, aj, false, stock.z_grid.cell_size);
                    for p in points.iter().skip(1) {
                        let _ = write!(anim_tp, "{:.3},{:.3},{:.3},1,", p.x, p.y, p.z);
                        anim_move_count += 1;
                    }
                }
            }
        }
        total_moves += tp.moves.len();
    }

    let mut html = String::with_capacity(2 * 1024 * 1024);

    let _ = write!(
        html,
        r##"<!DOCTYPE html>
<html><head>
<meta charset="utf-8">
<title>rs_cam Stacked Simulation</title>
<style>
  body {{ margin: 0; overflow: hidden; background: #1a1a2e; }}
  #info {{
    position: absolute; top: 10px; left: 10px; color: #ccc;
    font: 13px monospace; background: rgba(0,0,0,0.6); padding: 8px 12px;
    border-radius: 4px; pointer-events: none;
  }}
  #legend {{
    position: absolute; bottom: 10px; left: 10px; color: #aaa;
    font: 12px monospace; background: rgba(0,0,0,0.6); padding: 8px 12px;
    border-radius: 4px;
  }}
  #controls {{
    position: absolute; bottom: 50px; left: 50%; transform: translateX(-50%);
    color: #ccc; font: 13px monospace; background: rgba(0,0,0,0.75);
    padding: 10px 18px; border-radius: 6px; display: flex; align-items: center; gap: 14px;
    user-select: none;
  }}
  #controls button {{
    background: #445; border: 1px solid #667; color: #ddd; padding: 5px 14px;
    border-radius: 4px; cursor: pointer; font: 13px monospace;
  }}
  #controls button:hover {{ background: #556; }}
  #progressBar {{
    width: 200px; height: 6px; background: #333; border-radius: 3px;
    cursor: pointer; position: relative;
  }}
  #progressFill {{
    height: 100%; background: #6a6; border-radius: 3px; width: 100%;
    pointer-events: none;
  }}
  #phaseLabel {{
    position: absolute; top: 80px; left: 50%; transform: translateX(-50%);
    color: #ffcc44; font: 16px monospace; background: rgba(0,0,0,0.7);
    padding: 6px 16px; border-radius: 4px; pointer-events: none;
    transition: opacity 0.3s;
  }}
</style>
</head><body>
<div id="info">
  Stacked simulation: {num_phases} phase(s), {hm_rows}x{hm_cols} heightmap ({cell_size:.2}mm)<br>
  Total: {total_moves} moves ({anim_moves} linearized), {total_cut:.0}mm cutting
</div>
<div id="phaseLabel"></div>
<div id="controls">
  <button id="replayBtn">&#9654; Replay</button>
  <button id="skipBtn">&#9646;&#9646; End</button>
  <label>Speed: <input type="range" id="speedRange" min="0" max="100" value="40" step="1">
  <span id="speedVal">1.0</span>x</label>
  <div id="progressBar"><div id="progressFill"></div></div>
  <span id="moveInfo">Complete</span>
  <span id="annotLabel" style="margin-left:12px;color:#aef;font-weight:bold"></span>
</div>
<div id="legend">
  <span style="color:#c49a6c">&#9632;</span> Uncut &nbsp;
  <span style="color:#73401a">&#9632;</span> Cut &nbsp;
  <span style="color:#3388ff">&#9632;</span> Toolpath &nbsp;
  <span style="color:#ff4444">&#9632;</span> Rapid &nbsp;
  <span style="color:#aabbcc">&#9632;</span> Tool &nbsp;
  {ghost_legend}
  Mouse: orbit | Scroll: zoom | Right-click: pan
</div>

<script type="importmap">
{{
  "imports": {{
    "three": "https://cdn.jsdelivr.net/npm/three@0.170.0/build/three.module.js",
    "three/addons/": "https://cdn.jsdelivr.net/npm/three@0.170.0/examples/jsm/"
  }}
}}
</script>

<script type="module">
import * as THREE from 'three';
import {{ OrbitControls }} from 'three/addons/controls/OrbitControls.js';

const scene = new THREE.Scene();
scene.background = new THREE.Color(0x1a1a2e);
const camera = new THREE.PerspectiveCamera(50, window.innerWidth / window.innerHeight, 0.1, 100000);
camera.position.set({cx:.3} + {cd:.3} * 0.5, {cy:.3} - {cd:.3} * 0.8, {cz:.3} + {cd:.3} * 1.0);
const renderer = new THREE.WebGLRenderer({{ antialias: true }});
renderer.setSize(window.innerWidth, window.innerHeight);
renderer.setPixelRatio(window.devicePixelRatio);
document.body.appendChild(renderer.domElement);
const orbitCtrl = new OrbitControls(camera, renderer.domElement);
orbitCtrl.target.set({cx:.3}, {cy:.3}, {cz:.3});
orbitCtrl.enableDamping = true;

scene.add(new THREE.AmbientLight(0x666666));
const dl1 = new THREE.DirectionalLight(0xffffff, 0.9);
dl1.position.set(1, 1, 2); scene.add(dl1);
const dl2 = new THREE.DirectionalLight(0x4488aa, 0.3);
dl2.position.set(-1, -0.5, 0.5); scene.add(dl2);

const finalVerts = new Float32Array([{hm_verts_data}]);
const finalColors = new Float32Array([{hm_colors_data}]);
const hmVerts = new Float32Array(finalVerts);
const hmColors = new Float32Array(finalColors);
const hmIdx = new Uint32Array([{hm_idx_data}]);
const hmGeo = new THREE.BufferGeometry();
hmGeo.setAttribute('position', new THREE.BufferAttribute(hmVerts, 3));
hmGeo.setAttribute('color', new THREE.BufferAttribute(hmColors, 3));
hmGeo.setIndex(new THREE.BufferAttribute(hmIdx, 1));
hmGeo.computeVertexNormals();
scene.add(new THREE.Mesh(hmGeo, new THREE.MeshPhongMaterial({{
  vertexColors: true, side: THREE.DoubleSide, flatShading: false, shininess: 20,
}})));

{ghost_mesh_js}

const cutVerts = new Float32Array([{cut_verts_data}]);
const cutColors = new Float32Array([{cut_colors_data}]);
if (cutVerts.length > 0) {{
  const g = new THREE.BufferGeometry();
  g.setAttribute('position', new THREE.BufferAttribute(cutVerts, 3));
  g.setAttribute('color', new THREE.BufferAttribute(cutColors, 3));
  scene.add(new THREE.LineSegments(g,
    new THREE.LineBasicMaterial({{ vertexColors: true, transparent: true, opacity: 0.4 }})));
}}
const rapidVerts = new Float32Array([{rapid_verts_data}]);
if (rapidVerts.length > 0) {{
  const g = new THREE.BufferGeometry();
  g.setAttribute('position', new THREE.BufferAttribute(rapidVerts, 3));
  scene.add(new THREE.LineSegments(g,
    new THREE.LineBasicMaterial({{ color: 0xff4444, transparent: true, opacity: 0.2 }})));
}}

const gridSz = {grid_size:.0};
const grd = new THREE.GridHelper(gridSz, 20, 0x333355, 0x222244);
grd.rotation.x = Math.PI / 2;
grd.position.set({cx:.3}, {cy:.3}, {grid_z:.3});
scene.add(grd);
scene.add(new THREE.AxesHelper(gridSz * 0.3).translateX({bbox_min_x:.3}).translateY({bbox_min_y:.3}).translateZ({bbox_min_z:.3}));

// ====== MULTI-TOOL ANIMATION ======
const toolProfiles = [{tool_profiles_js}];
const toolRadii = new Float32Array([{tool_radii_js}]);
const latheProfiles = [{lathe_profiles_js}];
const phaseLabels = [{phase_labels_js}];
const NPS = {num_profile_samples};
const hmRows = {hm_rows}, hmCols = {hm_cols};
const originX = {origin_x:.4}, originY = {origin_y:.4};
const cellSize = {cell_size_val:.4};
const stockTopZ = {stock_top_z:.4};
const zRV = {z_range_val:.6};
const hmGrid = new Float64Array(hmRows * hmCols);
const animTP = new Float32Array([{anim_tp_data}]);
const animMoveCount = {anim_move_count};

let activeTool = 0, activeRadius = toolRadii[0]||1, activeProfile = toolProfiles[0];
const toolMat = new THREE.MeshPhongMaterial({{
  color: 0xaabbcc, transparent: true, opacity: 0.7, side: THREE.DoubleSide, shininess: 60,
}});
let toolMesh = null;

function buildToolMesh(ti) {{
  if (toolMesh) scene.remove(toolMesh);
  const geo = new THREE.LatheGeometry(latheProfiles[ti]||latheProfiles[0], 24);
  geo.rotateX(Math.PI / 2);
  toolMesh = new THREE.Mesh(geo, toolMat);
  toolMesh.visible = false;
  scene.add(toolMesh);
}}
buildToolMesh(0);

function switchTool(ti) {{
  activeTool = ti;
  activeRadius = toolRadii[ti]||1;
  activeProfile = toolProfiles[ti];
  buildToolMesh(ti);
  document.getElementById('phaseLabel').textContent = phaseLabels[ti]||'';
  document.getElementById('phaseLabel').style.opacity = '1';
}}

function hAtR(r) {{
  if (r > activeRadius) return -1;
  const t = (r / activeRadius) * NPS;
  const i = Math.min(Math.floor(t), NPS - 1);
  const f = t - i;
  const h0 = activeProfile[i], h1 = activeProfile[i + 1];
  return (h0 < 0 || h1 < 0) ? -1 : h0*(1-f)+h1*f;
}}

function stampAt(cx, cy, tipZ) {{
  const rSq = activeRadius * activeRadius;
  const c0 = Math.max(0, Math.floor((cx-activeRadius-originX)/cellSize));
  const c1 = Math.min(hmCols-1, Math.ceil((cx+activeRadius-originX)/cellSize));
  const r0 = Math.max(0, Math.floor((cy-activeRadius-originY)/cellSize));
  const r1 = Math.min(hmRows-1, Math.ceil((cy+activeRadius-originY)/cellSize));
  for (let row = r0; row <= r1; row++) {{
    const dy = originY + row*cellSize - cy; const dySq = dy*dy;
    if (dySq > rSq) continue;
    for (let col = c0; col <= c1; col++) {{
      const dx = originX + col*cellSize - cx;
      const dSq = dx*dx+dySq;
      if (dSq > rSq) continue;
      const h = hAtR(Math.sqrt(dSq));
      if (h >= 0) {{
        const idx = row*hmCols+col;
        const cz = tipZ+h;
        if (cz < hmGrid[idx]) hmGrid[idx] = cz;
      }}
    }}
  }}
}}

function stampSeg(x0,y0,z0,x1,y1,z1) {{
  const dx=x1-x0,dy=y1-y0,dz=z1-z0;
  const len=Math.sqrt(dx*dx+dy*dy+dz*dz);
  const n=Math.max(1,Math.ceil(len/cellSize));
  for(let i=0;i<=n;i++){{const t=i/n;stampAt(x0+t*dx,y0+t*dy,z0+t*dz);}}
}}

function updateHmMesh() {{
  const pos=hmGeo.attributes.position.array;
  const col=hmGeo.attributes.color.array;
  for(let r=0;r<hmRows;r++)for(let c=0;c<hmCols;c++){{
    const idx=r*hmCols+c; const z=hmGrid[idx];
    pos[idx*3+2]=z;
    const dt=Math.max(0,Math.min(1,(stockTopZ-z)/zRV));
    col[idx*3]=0.76+(0.45-0.76)*dt;
    col[idx*3+1]=0.60+(0.25-0.60)*dt;
    col[idx*3+2]=0.42+(0.10-0.42)*dt;
  }}
  hmGeo.attributes.position.needsUpdate=true;
  hmGeo.attributes.color.needsUpdate=true;
  hmGeo.computeVertexNormals();
}}

function restoreFinal() {{
  hmGeo.attributes.position.array.set(finalVerts);
  hmGeo.attributes.color.array.set(finalColors);
  hmGeo.attributes.position.needsUpdate=true;
  hmGeo.attributes.color.needsUpdate=true;
  hmGeo.computeVertexNormals();
  if(toolMesh) toolMesh.visible=false;
  moveIdx=animMoveCount;
  document.getElementById('phaseLabel').style.opacity='0';
  updateUI();
}}

function resetToStock() {{
  hmGrid.fill(stockTopZ);
  for(let i=0;i<hmRows*hmCols;i++){{
    hmGeo.attributes.position.array[i*3+2]=stockTopZ;
    hmGeo.attributes.color.array[i*3]=0.76;
    hmGeo.attributes.color.array[i*3+1]=0.60;
    hmGeo.attributes.color.array[i*3+2]=0.42;
  }}
  hmGeo.attributes.position.needsUpdate=true;
  hmGeo.attributes.color.needsUpdate=true;
  hmGeo.computeVertexNormals();
  moveIdx=1; switchTool(0);
}}

let playing=false, moveIdx=1;
let totalDist=0;
for(let i=1;i<animMoveCount;i++){{
  const b=i*4;if(animTP[b+3]>=3)continue;
  const dx=animTP[b]-animTP[b-4],dy=animTP[b+1]-animTP[b-3],dz=animTP[b+2]-animTP[b-2];
  totalDist+=Math.sqrt(dx*dx+dy*dy+dz*dz);
}}
const baseSpeed=Math.max(totalDist/25,10);

function processFrame(dt) {{
  const sv=parseFloat(document.getElementById('speedRange').value);
  const sm=Math.pow(10,(sv-50)/30);
  let budget=baseSpeed*sm*dt; let dirty=false;
  while(budget>0 && moveIdx<animMoveCount){{
    const b=moveIdx*4; const tc=animTP[b+3];
    if(tc>=3){{ switchTool(Math.round(tc-3)); moveIdx++; continue; }}
    const pb=(moveIdx-1)*4;
    const x0=animTP[pb],y0=animTP[pb+1],z0=animTP[pb+2];
    const x1=animTP[b],y1=animTP[b+1],z1=animTP[b+2];
    const isR=tc===0;
    const dx=x1-x0,dy=y1-y0,dz=z1-z0;
    const sl=Math.sqrt(dx*dx+dy*dy+dz*dz);
    if(!isR&&sl>0.001){{ stampSeg(x0,y0,z0,x1,y1,z1); dirty=true; }}
    if(toolMesh){{ toolMesh.position.set(x1,y1,z1); toolMesh.visible=true; }}
    budget-=isR?sl*0.1:sl; moveIdx++;
  }}
  if(dirty)updateHmMesh();
  if(moveIdx>=animMoveCount){{
    playing=false; restoreFinal();
    document.getElementById('replayBtn').textContent='\u25B6 Replay';
  }}
  updateUI();
}}

function updateUI() {{
  const pct=animMoveCount>1?((moveIdx-1)/(animMoveCount-1))*100:100;
  document.getElementById('progressFill').style.width=pct+'%';
  document.getElementById('moveInfo').textContent=playing?(moveIdx+'/'+animMoveCount):'Complete';
}}

document.getElementById('replayBtn').addEventListener('click',()=>{{
  if(playing){{ playing=false; document.getElementById('replayBtn').textContent='\u25B6 Resume'; }}
  else{{ if(moveIdx>=animMoveCount)resetToStock(); playing=true;
    document.getElementById('replayBtn').textContent='\u23F8 Pause'; }}
}});
document.getElementById('skipBtn').addEventListener('click',()=>{{
  playing=false; restoreFinal(); document.getElementById('replayBtn').textContent='\u25B6 Replay';
}});
document.getElementById('speedRange').addEventListener('input',(e)=>{{
  const m=Math.pow(10,(parseFloat(e.target.value)-50)/30);
  document.getElementById('speedVal').textContent=m<1?m.toFixed(2):m.toFixed(0);
}});
document.getElementById('speedVal').textContent='1.0';
window.addEventListener('resize',()=>{{
  camera.aspect=window.innerWidth/window.innerHeight;
  camera.updateProjectionMatrix(); renderer.setSize(window.innerWidth,window.innerHeight);
}});

const clk=new THREE.Clock();
function animate(){{ requestAnimationFrame(animate);
  const dt=clk.getDelta(); if(playing)processFrame(Math.min(dt,0.05));
  orbitCtrl.update(); renderer.render(scene,camera);
}}
animate();
</script>
</body></html>"##,
        num_phases = phases.len(),
        hm_rows = stock.z_grid.rows,
        hm_cols = stock.z_grid.cols,
        cell_size = stock.z_grid.cell_size,
        total_moves = total_moves,
        anim_moves = anim_move_count,
        total_cut = total_cut_dist,
        ghost_legend = if has_source_mesh {
            "<span style=\"color:#88aa88\">&#9632;</span> Source mesh &nbsp;"
        } else {
            ""
        },
        cx = center.x,
        cy = center.y,
        cz = center.z,
        cd = cam_dist,
        hm_verts_data = hm_verts.trim_end_matches(','),
        hm_colors_data = hm_colors.trim_end_matches(','),
        hm_idx_data = hm_indices_str.trim_end_matches(','),
        ghost_mesh_js = if has_source_mesh {
            format!(
                r#"const srcV=new Float32Array([{}]);const srcI=new Uint32Array([{}]);
const srcG=new THREE.BufferGeometry();
srcG.setAttribute('position',new THREE.BufferAttribute(srcV,3));
srcG.setIndex(new THREE.BufferAttribute(srcI,1));srcG.computeVertexNormals();
scene.add(new THREE.Mesh(srcG,new THREE.MeshPhongMaterial({{
  color:0x88aa88,transparent:true,opacity:0.2,side:THREE.DoubleSide,depthWrite:false}})));"#,
                src_mesh_verts.trim_end_matches(','),
                src_mesh_indices_str.trim_end_matches(','),
            )
        } else {
            String::new()
        },
        cut_verts_data = cut_verts.trim_end_matches(','),
        cut_colors_data = cut_colors.trim_end_matches(','),
        rapid_verts_data = rapid_verts.trim_end_matches(','),
        tool_profiles_js = tool_profiles_js.trim_end_matches(','),
        tool_radii_js = tool_radii_js.trim_end_matches(','),
        lathe_profiles_js = lathe_profiles_js.trim_end_matches(','),
        phase_labels_js = phase_labels_js.trim_end_matches(','),
        num_profile_samples = num_profile_samples,
        origin_x = stock.z_grid.origin_u,
        origin_y = stock.z_grid.origin_v,
        cell_size_val = stock.z_grid.cell_size,
        stock_top_z = stock.stock_bbox.max.z,
        z_range_val = (stock.stock_bbox.max.z - hm_bbox.min.z).max(1e-6),
        anim_tp_data = anim_tp.trim_end_matches(','),
        anim_move_count = anim_move_count,
        grid_size = extent * 1.2,
        grid_z = hm_bbox.min.z - 0.1,
        bbox_min_x = hm_bbox.min.x,
        bbox_min_y = hm_bbox.min.y,
        bbox_min_z = hm_bbox.min.z,
    );

    html
}
