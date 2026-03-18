//! Visualization output.
//!
//! - SVG: 2D top-down toolpath preview
//! - HTML: Interactive 3D viewer with mesh + toolpaths (three.js)

use crate::geo::BoundingBox3;
use crate::mesh::TriangleMesh;
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
    writeln!(svg, "<svg xmlns='http://www.w3.org/2000/svg' width='{width}' height='{height}' viewBox='0 0 {width} {height}'>").unwrap();
    writeln!(svg, "<rect width='{width}' height='{height}' fill='#1a1a2e'/>").unwrap();

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
                writeln!(svg, "<line x1='{x1:.1}' y1='{y1:.1}' x2='{x2:.1}' y2='{y2:.1}' stroke='#333' stroke-width='0.3' stroke-dasharray='2,2'/>").unwrap();
            }
            MoveType::Linear { .. } | MoveType::ArcCW { .. } | MoveType::ArcCCW { .. } => {
                // Color by Z: low=deep blue, high=bright cyan/white
                let t = ((to.z - z_min) / z_range).clamp(0.0, 1.0);
                let r = (t * 100.0) as u8;
                let g = (80.0 + t * 175.0) as u8;
                let b = (180.0 + t * 75.0) as u8;
                writeln!(svg, "<line x1='{x1:.1}' y1='{y1:.1}' x2='{x2:.1}' y2='{y2:.1}' stroke='#{r:02x}{g:02x}{b:02x}' stroke-width='0.5'/>").unwrap();
            }
        }
    }

    // Add legend
    writeln!(svg, "<text x='5' y='15' fill='white' font-size='10' font-family='monospace'>Z: {:.2} to {:.2} mm</text>", z_min, bbox.max.z).unwrap();
    writeln!(svg, "<text x='5' y='27' fill='white' font-size='10' font-family='monospace'>{} moves, {:.0}mm cutting</text>", toolpath.moves.len(), toolpath.total_cutting_distance()).unwrap();

    writeln!(svg, "</svg>").unwrap();
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
        write!(mesh_verts, "{:.4},{:.4},{:.4},", v.x, v.y, v.z).unwrap();
    }

    // Serialize mesh triangle indices
    let mut mesh_indices = String::new();
    for tri in &mesh.triangles {
        write!(mesh_indices, "{},{},{},", tri[0], tri[1], tri[2]).unwrap();
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
                write!(rapid_verts, "{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},",
                    from.x, from.y, from.z, to.x, to.y, to.z).unwrap();
            }
            MoveType::Linear { .. } | MoveType::ArcCW { .. } | MoveType::ArcCCW { .. } => {
                write!(cut_verts, "{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},",
                    from.x, from.y, from.z, to.x, to.y, to.z).unwrap();
                // Color both endpoints by their Z
                for z in [from.z, to.z] {
                    let t = ((z - z_min) / z_range).clamp(0.0, 1.0) as f32;
                    // Low Z = blue (0.1, 0.3, 0.9), high Z = cyan (0.2, 0.9, 1.0)
                    let r = 0.1 + t * 0.1;
                    let g = 0.3 + t * 0.6;
                    let b = 0.9 + t * 0.1;
                    write!(cut_colors, "{:.3},{:.3},{:.3},", r, g, b).unwrap();
                }
            }
        }
    }

    write!(html, r##"<!DOCTYPE html>
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
    ).unwrap();

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
                tp_bbox.min.x - margin, tp_bbox.min.y - margin, tp_bbox.min.z,
                tp_bbox.max.x + margin, tp_bbox.max.y + margin, 0.0,
            )
        }
    };

    let center_x = (sx_min + sx_max) / 2.0;
    let center_y = (sy_min + sy_max) / 2.0;
    let center_z = (sz_min + sz_max) / 2.0;
    let extent = (sx_max - sx_min).max(sy_max - sy_min).max((sz_max - sz_min).abs());
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
                write!(rapid_verts, "{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},",
                    from.x, from.y, from.z, to.x, to.y, to.z).unwrap();
            }
            MoveType::Linear { .. } | MoveType::ArcCW { .. } | MoveType::ArcCCW { .. } => {
                let dz = (to.z - from.z).abs();
                let dxy = ((to.x - from.x).powi(2) + (to.y - from.y).powi(2)).sqrt();

                // Classify as plunge (mostly vertical) vs cutting (mostly horizontal)
                if dz > 0.1 && dxy < 0.1 {
                    write!(plunge_verts, "{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},",
                        from.x, from.y, from.z, to.x, to.y, to.z).unwrap();
                } else {
                    write!(cut_verts, "{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},",
                        from.x, from.y, from.z, to.x, to.y, to.z).unwrap();
                    for z in [from.z, to.z] {
                        let t = ((z - z_min) / z_range).clamp(0.0, 1.0) as f32;
                        let r = 0.1 + t * 0.15;
                        let g = 0.3 + t * 0.6;
                        let b = 0.85 + t * 0.15;
                        write!(cut_colors, "{:.3},{:.3},{:.3},", r, g, b).unwrap();
                    }
                }
            }
        }
    }

    write!(html, r##"<!DOCTYPE html>
<html><head>
<meta charset="utf-8">
<title>rs_cam 2.5D Toolpath Viewer</title>
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
    ).unwrap();

    html
}
