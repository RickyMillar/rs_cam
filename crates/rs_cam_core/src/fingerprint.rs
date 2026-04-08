//! Toolpath fingerprinting and diffing for parameter validation.
//!
//! A `ToolpathFingerprint` captures structured metrics from a `Toolpath` in a single
//! pass. Two fingerprints can be diffed to determine exactly what changed when a
//! parameter was modified — the primary mechanism for AI-driven parameter sweeps.

use crate::toolpath::{MoveType, Toolpath};
use serde::{Deserialize, Serialize};

/// Structured metric snapshot of a toolpath.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolpathFingerprint {
    // Move counts
    pub move_count: usize,
    pub rapid_count: usize,
    pub linear_count: usize,
    pub arc_cw_count: usize,
    pub arc_ccw_count: usize,

    // Distances
    pub cutting_distance_mm: f64,
    pub rapid_distance_mm: f64,

    // Z-level analysis
    pub z_levels: Vec<f64>,
    pub z_level_count: usize,
    pub min_z: f64,
    pub max_z: f64,

    // Feed rate analysis
    pub feed_rates: Vec<f64>,
    pub feed_rate_count: usize,
    pub min_feed_rate: f64,
    pub max_feed_rate: f64,

    // Bounding box
    pub bbox_min: [f64; 3],
    pub bbox_max: [f64; 3],

    // Proportions
    pub rapid_fraction: f64,
    pub cutting_fraction: f64,
}

impl ToolpathFingerprint {
    /// Extract a fingerprint from a toolpath in a single pass.
    pub fn from_toolpath(tp: &Toolpath) -> Self {
        let mut rapid_count = 0usize;
        let mut linear_count = 0usize;
        let mut arc_cw_count = 0usize;
        let mut arc_ccw_count = 0usize;

        for m in &tp.moves {
            match m.move_type {
                MoveType::Rapid => rapid_count += 1,
                MoveType::Linear { .. } => linear_count += 1,
                MoveType::ArcCW { .. } => arc_cw_count += 1,
                MoveType::ArcCCW { .. } => arc_ccw_count += 1,
            }
        }

        let move_count = tp.moves.len();
        let cutting_distance_mm = tp.total_cutting_distance();
        let rapid_distance_mm = tp.total_rapid_distance();
        let total_dist = cutting_distance_mm + rapid_distance_mm;

        let z_levels = tp.z_levels(0.001);
        let feed_rates = tp.feed_rates(0.1);

        let (bbox_min, bbox_max) = tp.bounding_box();

        let min_z = z_levels.first().copied().unwrap_or(0.0);
        let max_z = z_levels.last().copied().unwrap_or(0.0);
        let min_feed_rate = feed_rates.first().copied().unwrap_or(0.0);
        let max_feed_rate = feed_rates.last().copied().unwrap_or(0.0);

        Self {
            move_count,
            rapid_count,
            linear_count,
            arc_cw_count,
            arc_ccw_count,
            cutting_distance_mm,
            rapid_distance_mm,
            z_levels: z_levels.clone(),
            z_level_count: z_levels.len(),
            min_z,
            max_z,
            feed_rates: feed_rates.clone(),
            feed_rate_count: feed_rates.len(),
            min_feed_rate,
            max_feed_rate,
            bbox_min,
            bbox_max,
            rapid_fraction: if total_dist > 0.0 {
                rapid_distance_mm / total_dist
            } else {
                0.0
            },
            cutting_fraction: if total_dist > 0.0 {
                cutting_distance_mm / total_dist
            } else {
                0.0
            },
        }
    }
}

/// A single field change between two fingerprints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldChange {
    pub field: String,
    pub before: serde_json::Value,
    pub after: serde_json::Value,
    /// Percentage change for numeric fields, None for non-numeric.
    pub delta_percent: Option<f64>,
}

/// Result of comparing two fingerprints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FingerprintDiff {
    pub changed_fields: Vec<FieldChange>,
    pub unchanged_fields: Vec<String>,
}

impl FingerprintDiff {
    /// True if at least one field changed.
    pub fn has_changes(&self) -> bool {
        !self.changed_fields.is_empty()
    }

    /// Get the change for a specific field, if it changed.
    pub fn field_change(&self, name: &str) -> Option<&FieldChange> {
        self.changed_fields.iter().find(|c| c.field == name)
    }
}

/// Compare two fingerprints and report what changed.
///
/// Numeric fields are considered changed if the absolute delta exceeds 0.001
/// AND the relative delta exceeds 0.1%. Array fields use set comparison.
pub fn diff_fingerprints(a: &ToolpathFingerprint, b: &ToolpathFingerprint) -> FingerprintDiff {
    let mut changed = Vec::new();
    let mut unchanged = Vec::new();

    // Integer fields
    diff_usize(
        "move_count",
        a.move_count,
        b.move_count,
        &mut changed,
        &mut unchanged,
    );
    diff_usize(
        "rapid_count",
        a.rapid_count,
        b.rapid_count,
        &mut changed,
        &mut unchanged,
    );
    diff_usize(
        "linear_count",
        a.linear_count,
        b.linear_count,
        &mut changed,
        &mut unchanged,
    );
    diff_usize(
        "arc_cw_count",
        a.arc_cw_count,
        b.arc_cw_count,
        &mut changed,
        &mut unchanged,
    );
    diff_usize(
        "arc_ccw_count",
        a.arc_ccw_count,
        b.arc_ccw_count,
        &mut changed,
        &mut unchanged,
    );
    diff_usize(
        "z_level_count",
        a.z_level_count,
        b.z_level_count,
        &mut changed,
        &mut unchanged,
    );
    diff_usize(
        "feed_rate_count",
        a.feed_rate_count,
        b.feed_rate_count,
        &mut changed,
        &mut unchanged,
    );

    // Float fields
    diff_f64(
        "cutting_distance_mm",
        a.cutting_distance_mm,
        b.cutting_distance_mm,
        &mut changed,
        &mut unchanged,
    );
    diff_f64(
        "rapid_distance_mm",
        a.rapid_distance_mm,
        b.rapid_distance_mm,
        &mut changed,
        &mut unchanged,
    );
    diff_f64("min_z", a.min_z, b.min_z, &mut changed, &mut unchanged);
    diff_f64("max_z", a.max_z, b.max_z, &mut changed, &mut unchanged);
    diff_f64(
        "min_feed_rate",
        a.min_feed_rate,
        b.min_feed_rate,
        &mut changed,
        &mut unchanged,
    );
    diff_f64(
        "max_feed_rate",
        a.max_feed_rate,
        b.max_feed_rate,
        &mut changed,
        &mut unchanged,
    );
    diff_f64(
        "rapid_fraction",
        a.rapid_fraction,
        b.rapid_fraction,
        &mut changed,
        &mut unchanged,
    );
    diff_f64(
        "cutting_fraction",
        a.cutting_fraction,
        b.cutting_fraction,
        &mut changed,
        &mut unchanged,
    );

    // Bounding box (compare each component)
    diff_f64(
        "bbox_min_x",
        a.bbox_min[0],
        b.bbox_min[0],
        &mut changed,
        &mut unchanged,
    );
    diff_f64(
        "bbox_min_y",
        a.bbox_min[1],
        b.bbox_min[1],
        &mut changed,
        &mut unchanged,
    );
    diff_f64(
        "bbox_min_z",
        a.bbox_min[2],
        b.bbox_min[2],
        &mut changed,
        &mut unchanged,
    );
    diff_f64(
        "bbox_max_x",
        a.bbox_max[0],
        b.bbox_max[0],
        &mut changed,
        &mut unchanged,
    );
    diff_f64(
        "bbox_max_y",
        a.bbox_max[1],
        b.bbox_max[1],
        &mut changed,
        &mut unchanged,
    );
    diff_f64(
        "bbox_max_z",
        a.bbox_max[2],
        b.bbox_max[2],
        &mut changed,
        &mut unchanged,
    );

    // Array fields (z_levels, feed_rates)
    diff_f64_vec(
        "z_levels",
        &a.z_levels,
        &b.z_levels,
        0.001,
        &mut changed,
        &mut unchanged,
    );
    diff_f64_vec(
        "feed_rates",
        &a.feed_rates,
        &b.feed_rates,
        0.1,
        &mut changed,
        &mut unchanged,
    );

    FingerprintDiff {
        changed_fields: changed,
        unchanged_fields: unchanged,
    }
}

fn diff_usize(
    name: &str,
    a: usize,
    b: usize,
    changed: &mut Vec<FieldChange>,
    unchanged: &mut Vec<String>,
) {
    if a != b {
        let delta_pct = if a > 0 {
            Some(((b as f64 - a as f64) / a as f64) * 100.0)
        } else if b > 0 {
            Some(f64::INFINITY)
        } else {
            Some(0.0)
        };
        changed.push(FieldChange {
            field: name.to_owned(),
            before: serde_json::Value::from(a as u64),
            after: serde_json::Value::from(b as u64),
            delta_percent: delta_pct,
        });
    } else {
        unchanged.push(name.to_owned());
    }
}

fn diff_f64(
    name: &str,
    a: f64,
    b: f64,
    changed: &mut Vec<FieldChange>,
    unchanged: &mut Vec<String>,
) {
    let abs_delta = (b - a).abs();
    let rel_delta = if a.abs() > 1e-10 {
        abs_delta / a.abs()
    } else if abs_delta > 1e-10 {
        f64::INFINITY
    } else {
        0.0
    };

    // Changed if absolute delta > 0.001 AND relative delta > 0.1%
    if abs_delta > 0.001 && rel_delta > 0.001 {
        changed.push(FieldChange {
            field: name.to_owned(),
            before: serde_json::json!(a),
            after: serde_json::json!(b),
            delta_percent: Some(rel_delta * 100.0),
        });
    } else {
        unchanged.push(name.to_owned());
    }
}

fn diff_f64_vec(
    name: &str,
    a: &[f64],
    b: &[f64],
    epsilon: f64,
    changed: &mut Vec<FieldChange>,
    unchanged: &mut Vec<String>,
) {
    let same = a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| (x - y).abs() < epsilon);

    if !same {
        changed.push(FieldChange {
            field: name.to_owned(),
            before: serde_json::json!(a),
            after: serde_json::json!(b),
            delta_percent: None,
        });
    } else {
        unchanged.push(name.to_owned());
    }
}

/// Visual and data artifacts produced alongside a fingerprint.
///
/// These are generated per sweep variant so agents can inspect both numeric diffs
/// AND visual output for artifacts that metrics alone wouldn't catch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SweepArtifacts {
    /// Top-down toolpath SVG (Z encoded as color).
    pub toolpath_svg: Option<String>,
    /// Structural summary of the toolpath SVG for quick diff.
    pub svg_summary: Option<SvgSummary>,
}

/// Structural summary extracted from a toolpath SVG.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SvgSummary {
    /// Number of cutting move line elements in the SVG.
    pub cutting_line_count: usize,
    /// Number of rapid move line elements (dashed gray).
    pub rapid_line_count: usize,
    /// Unique stroke colors used (hex strings).
    pub unique_colors: Vec<String>,
}

impl SweepArtifacts {
    /// Generate artifacts for a toolpath.
    pub fn generate(tp: &Toolpath) -> Self {
        let toolpath_svg = if tp.moves.is_empty() {
            None
        } else {
            Some(crate::viz::toolpath_to_svg(tp, 800.0, 600.0))
        };

        let svg_summary = toolpath_svg.as_ref().map(|svg| extract_svg_summary(svg));

        Self {
            toolpath_svg,
            svg_summary,
        }
    }
}

/// Render a composite 6-view PNG of the stock (4 iso corners + top + bottom).
///
/// Returns raw RGBA pixel buffer and dimensions. Use the `image` crate to encode
/// to PNG/JPEG in test code.
///
/// Layout (3 columns x 2 rows):
/// ```text
///  ┌────────────┬────────────┬────────────┐
///  │ Front-Left │    Top     │ Front-Right│
///  ├────────────┼────────────┼────────────┤
///  │ Back-Left  │   Bottom   │ Back-Right │
///  └────────────┴────────────┴────────────┘
/// ```
#[allow(clippy::indexing_slicing)] // bounded by mesh indices
pub fn render_stock_composite(
    stock: &crate::dexel_stock::TriDexelStock,
    width: u32,
    height: u32,
) -> Vec<u8> {
    use crate::dexel_mesh::dexel_stock_to_mesh;

    let mesh = dexel_stock_to_mesh(stock);
    render_mesh_composite(&mesh, width, height)
}

/// Render a composite 6-view PNG from an existing `StockMesh`.
///
/// Same 6-view layout as [`render_stock_composite`] but skips the
/// dexel-to-mesh conversion — use this when you already have the mesh
/// (e.g. from `SimulationResult::mesh`).
#[allow(clippy::indexing_slicing)] // bounded by mesh indices
pub fn render_mesh_composite(
    mesh: &crate::stock_mesh::StockMesh,
    width: u32,
    height: u32,
) -> Vec<u8> {
    let w = width as usize;
    let h = height as usize;
    // RGBA buffer, dark gray background
    let mut pixels = vec![0u8; w * h * 4];
    for i in 0..w * h {
        pixels[i * 4] = 42; // R
        pixels[i * 4 + 1] = 42; // G
        pixels[i * 4 + 2] = 42; // B
        pixels[i * 4 + 3] = 255; // A
    }

    if mesh.vertices.is_empty() || mesh.indices.is_empty() {
        return pixels;
    }

    // Centroid
    let vert_count = mesh.vertices.len() / 3;
    let mut cx = 0.0f64;
    let mut cy = 0.0f64;
    let mut cz = 0.0f64;
    for i in 0..vert_count {
        cx += f64::from(mesh.vertices[i * 3]);
        cy += f64::from(mesh.vertices[i * 3 + 1]);
        cz += f64::from(mesh.vertices[i * 3 + 2]);
    }
    let n = vert_count.max(1) as f64;
    cx /= n;
    cy /= n;
    cz /= n;

    let pi = std::f64::consts::PI;
    let deg30 = pi / 6.0;
    let deg90 = pi / 2.0;

    let cell_w = width / 3;
    let cell_h = height / 2;

    let views: &[(f64, f64, u32, u32)] = &[
        (pi / 4.0, deg30, 0, 0),                     // Front-Left
        (0.0, deg90, cell_w, 0),                     // Top
        (7.0 * pi / 4.0, deg30, cell_w * 2, 0),      // Front-Right
        (3.0 * pi / 4.0, deg30, 0, cell_h),          // Back-Left
        (0.0, -deg90, cell_w, cell_h),               // Bottom
        (5.0 * pi / 4.0, deg30, cell_w * 2, cell_h), // Back-Right
    ];

    for &(az, el, vx, vy) in views {
        render_view_to_pixels(
            &mut pixels,
            w,
            h,
            mesh,
            vert_count,
            cx,
            cy,
            cz,
            az,
            el,
            vx as usize,
            vy as usize,
            cell_w as usize,
            cell_h as usize,
        );
    }

    pixels
}

/// Render a 6-view composite and save as PNG.
///
/// Convenience wrapper around [`render_mesh_composite`] that encodes the
/// RGBA pixels to a PNG file on disk.
pub fn save_mesh_composite_png(
    mesh: &crate::stock_mesh::StockMesh,
    path: &std::path::Path,
    width: u32,
    height: u32,
) -> Result<(), String> {
    let pixels = render_mesh_composite(mesh, width, height);
    image::save_buffer(path, &pixels, width, height, image::ColorType::Rgba8)
        .map_err(|e| format!("PNG save failed: {e}"))
}

/// Rasterize one view of the stock mesh into an RGBA pixel buffer.
#[allow(clippy::indexing_slicing, clippy::too_many_arguments)]
fn render_view_to_pixels(
    pixels: &mut [u8],
    buf_w: usize,
    _buf_h: usize,
    mesh: &crate::stock_mesh::StockMesh,
    vert_count: usize,
    cx: f64,
    cy: f64,
    cz: f64,
    azimuth: f64,
    elevation: f64,
    vx: usize,
    vy: usize,
    vw: usize,
    vh: usize,
) {
    let cos_az = azimuth.cos();
    let sin_az = azimuth.sin();
    let cos_el = elevation.cos();
    let sin_el = elevation.sin();

    let light_x = sin_az * 0.4 + cos_az * 0.3;
    let light_y = cos_az * 0.4 - sin_az * 0.3;
    let light_z: f64 = 0.866;

    // Project vertices
    let mut projected: Vec<[f64; 2]> = Vec::with_capacity(vert_count);
    let mut depths: Vec<f64> = Vec::with_capacity(vert_count);
    let mut sx_min = f64::MAX;
    let mut sx_max = f64::MIN;
    let mut sy_min = f64::MAX;
    let mut sy_max = f64::MIN;

    for i in 0..vert_count {
        let x = f64::from(mesh.vertices[i * 3]) - cx;
        let y = f64::from(mesh.vertices[i * 3 + 1]) - cy;
        let z = f64::from(mesh.vertices[i * 3 + 2]) - cz;

        let sx = x * cos_az - y * sin_az;
        let sy = -(x * sin_az * sin_el) - (y * cos_az * sin_el) + z * cos_el;
        let depth = x * sin_az * cos_el + y * cos_az * cos_el + z * sin_el;

        projected.push([sx, sy]);
        depths.push(depth);

        if sx < sx_min {
            sx_min = sx;
        }
        if sx > sx_max {
            sx_max = sx;
        }
        if sy < sy_min {
            sy_min = sy;
        }
        if sy > sy_max {
            sy_max = sy;
        }
    }

    let margin = 4.0;
    let data_w = (sx_max - sx_min).max(1e-6);
    let data_h = (sy_max - sy_min).max(1e-6);
    let fw = vw as f64;
    let fh = vh as f64;
    let scale = ((fw - 2.0 * margin) / data_w).min((fh - 2.0 * margin) / data_h);
    let proj_w = data_w * scale;
    let proj_h = data_h * scale;
    let off_x = margin + (fw - 2.0 * margin - proj_w) / 2.0;
    let off_y = margin + (fh - 2.0 * margin - proj_h) / 2.0;

    // Sort triangles back-to-front
    let tri_count = mesh.indices.len() / 3;
    let mut tris: Vec<(f64, usize)> = Vec::with_capacity(tri_count);
    for t in 0..tri_count {
        let i0 = mesh.indices[t * 3] as usize;
        let i1 = mesh.indices[t * 3 + 1] as usize;
        let i2 = mesh.indices[t * 3 + 2] as usize;
        if i0 >= vert_count || i1 >= vert_count || i2 >= vert_count {
            continue;
        }
        tris.push(((depths[i0] + depths[i1] + depths[i2]) / 3.0, t));
    }
    tris.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    // Z-buffer per pixel in this viewport (f64::MIN = no triangle yet)
    let mut zbuf = vec![f64::MIN; vw * vh];

    for &(tri_depth, t) in &tris {
        let i0 = mesh.indices[t * 3] as usize;
        let i1 = mesh.indices[t * 3 + 1] as usize;
        let i2 = mesh.indices[t * 3 + 2] as usize;

        let px0 = off_x + (projected[i0][0] - sx_min) * scale;
        let py0 = off_y + proj_h - (projected[i0][1] - sy_min) * scale;
        let px1 = off_x + (projected[i1][0] - sx_min) * scale;
        let py1 = off_y + proj_h - (projected[i1][1] - sy_min) * scale;
        let px2 = off_x + (projected[i2][0] - sx_min) * scale;
        let py2 = off_y + proj_h - (projected[i2][1] - sy_min) * scale;

        // Compute shaded color once per triangle
        let ax = f64::from(mesh.vertices[i1 * 3]) - f64::from(mesh.vertices[i0 * 3]);
        let ay = f64::from(mesh.vertices[i1 * 3 + 1]) - f64::from(mesh.vertices[i0 * 3 + 1]);
        let az_v = f64::from(mesh.vertices[i1 * 3 + 2]) - f64::from(mesh.vertices[i0 * 3 + 2]);
        let bx = f64::from(mesh.vertices[i2 * 3]) - f64::from(mesh.vertices[i0 * 3]);
        let by = f64::from(mesh.vertices[i2 * 3 + 1]) - f64::from(mesh.vertices[i0 * 3 + 1]);
        let bz = f64::from(mesh.vertices[i2 * 3 + 2]) - f64::from(mesh.vertices[i0 * 3 + 2]);
        let nx = ay * bz - az_v * by;
        let ny = az_v * bx - ax * bz;
        let nz = ax * by - ay * bx;
        let nlen = (nx * nx + ny * ny + nz * nz).sqrt().max(1e-10);
        let dot =
            ((nx / nlen) * light_x + (ny / nlen) * light_y + (nz / nlen) * light_z).clamp(0.0, 1.0);
        let shade = 0.3 + 0.7 * dot;

        let base_r = (f64::from(mesh.colors[i0 * 3])
            + f64::from(mesh.colors[i1 * 3])
            + f64::from(mesh.colors[i2 * 3]))
            / 3.0;
        let base_g = (f64::from(mesh.colors[i0 * 3 + 1])
            + f64::from(mesh.colors[i1 * 3 + 1])
            + f64::from(mesh.colors[i2 * 3 + 1]))
            / 3.0;
        let base_b = (f64::from(mesh.colors[i0 * 3 + 2])
            + f64::from(mesh.colors[i1 * 3 + 2])
            + f64::from(mesh.colors[i2 * 3 + 2]))
            / 3.0;

        let cr = (base_r * shade * 255.0).clamp(0.0, 255.0) as u8;
        let cg = (base_g * shade * 255.0).clamp(0.0, 255.0) as u8;
        let cb = (base_b * shade * 255.0).clamp(0.0, 255.0) as u8;

        // Rasterize triangle with scanline fill
        rasterize_triangle(
            pixels, &mut zbuf, buf_w, vx, vy, vw, vh, px0, py0, px1, py1, px2, py2, tri_depth, cr,
            cg, cb,
        );
    }
}

/// Scanline rasterize a triangle into the pixel buffer with z-test.
#[allow(clippy::too_many_arguments)]
fn rasterize_triangle(
    pixels: &mut [u8],
    zbuf: &mut [f64],
    buf_w: usize,
    vx: usize,
    vy: usize,
    vw: usize,
    vh: usize,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    depth: f64,
    r: u8,
    g: u8,
    b: u8,
) {
    // Bounding box clipped to viewport
    let min_x = x0.min(x1).min(x2).max(0.0) as usize;
    let max_x = (x0.max(x1).max(x2) as usize).min(vw.saturating_sub(1));
    let min_y = y0.min(y1).min(y2).max(0.0) as usize;
    let max_y = (y0.max(y1).max(y2) as usize).min(vh.saturating_sub(1));

    // Edge function constants
    let dx01 = x1 - x0;
    let dy01 = y1 - y0;
    let dx12 = x2 - x1;
    let dy12 = y2 - y1;
    let dx20 = x0 - x2;
    let dy20 = y0 - y2;

    for py in min_y..=max_y {
        for px in min_x..=max_x {
            let fx = px as f64 + 0.5;
            let fy = py as f64 + 0.5;

            // Barycentric edge test
            let e0 = (fx - x0) * dy01 - (fy - y0) * dx01;
            let e1 = (fx - x1) * dy12 - (fy - y1) * dx12;
            let e2 = (fx - x2) * dy20 - (fy - y2) * dx20;

            if (e0 >= 0.0 && e1 >= 0.0 && e2 >= 0.0) || (e0 <= 0.0 && e1 <= 0.0 && e2 <= 0.0) {
                // SAFETY: zi is bounded by min_y..=max_y × vw + min_x..=max_x,
                // which are clamped to the viewport dimensions that size zbuf.
                // pi is bounds-checked explicitly before writing.
                #[allow(clippy::indexing_slicing)]
                {
                    let zi = py * vw + px;
                    if depth > zbuf[zi] {
                        zbuf[zi] = depth;
                        let pi = ((vy + py) * buf_w + (vx + px)) * 4;
                        if pi + 3 < pixels.len() {
                            pixels[pi] = r;
                            pixels[pi + 1] = g;
                            pixels[pi + 2] = b;
                            pixels[pi + 3] = 255;
                        }
                    }
                }
            }
        }
    }
}

/// Extract structural summary from a toolpath SVG string.
fn extract_svg_summary(svg: &str) -> SvgSummary {
    let mut cutting_lines = 0usize;
    let mut rapid_lines = 0usize;
    let mut colors = std::collections::BTreeSet::new();

    for line in svg.lines() {
        if !line.contains("<line") {
            continue;
        }
        if line.contains("stroke-dasharray") {
            rapid_lines += 1;
        } else if let Some(pos) = line.find("stroke='#") {
            cutting_lines += 1;
            let color_start = pos + "stroke='".len();
            if let Some(end) = line.get(color_start..).and_then(|s| s.find('\''))
                && let Some(color) = line.get(color_start..color_start + end)
            {
                colors.insert(color.to_owned());
            }
        }
    }

    SvgSummary {
        cutting_line_count: cutting_lines,
        rapid_line_count: rapid_lines,
        unique_colors: colors.into_iter().collect(),
    }
}

/// Numeric fingerprint of stock state after simulation.
///
/// Captures aggregate metrics from the tri-dexel Z-grid so agents can diff
/// stock state between parameter variants without rendering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockFingerprint {
    /// Number of grid cells with material remaining.
    pub cells_with_material: usize,
    /// Number of grid cells fully empty (all material removed).
    pub cells_empty: usize,
    /// Total grid cells.
    pub total_cells: usize,
    /// Highest material Z across all cells.
    pub max_surface_z: f64,
    /// Lowest material Z across all cells (deepest cut).
    pub min_surface_z: f64,
    /// Average top Z across cells with material.
    pub avg_surface_z: f64,
    /// Original stock top Z (uncut level).
    pub stock_top_z: f64,
    /// Fraction of cells that were cut below stock top.
    pub cut_fraction: f64,
}

impl StockFingerprint {
    /// Extract stock metrics from a tri-dexel stock after simulation.
    pub fn from_stock(stock: &crate::dexel_stock::TriDexelStock) -> Self {
        let grid = &stock.z_grid;
        let rows = grid.rows;
        let cols = grid.cols;
        let total_cells = rows * cols;
        let stock_top = f64::from(stock.stock_bbox.max.z as f32);

        let mut cells_with = 0usize;
        let mut cells_empty = 0usize;
        let mut z_sum = 0.0f64;
        let mut z_max = f64::MIN;
        let mut z_min = f64::MAX;

        for row in 0..rows {
            for col in 0..cols {
                if let Some(top) = grid.top_z_at(row, col) {
                    let top = f64::from(top);
                    cells_with += 1;
                    z_sum += top;
                    if top > z_max {
                        z_max = top;
                    }
                    if top < z_min {
                        z_min = top;
                    }
                } else {
                    cells_empty += 1;
                }
            }
        }

        let avg_z = if cells_with > 0 {
            z_sum / cells_with as f64
        } else {
            stock_top
        };

        // Count cells cut below stock top (with small epsilon for float comparison)
        let mut cut_cells = 0usize;
        for row in 0..rows {
            for col in 0..cols {
                if let Some(top) = grid.top_z_at(row, col)
                    && f64::from(top) < stock_top - 0.01
                {
                    cut_cells += 1;
                }
            }
        }

        Self {
            cells_with_material: cells_with,
            cells_empty,
            total_cells,
            max_surface_z: if z_max > f64::MIN { z_max } else { 0.0 },
            min_surface_z: if z_min < f64::MAX { z_min } else { 0.0 },
            avg_surface_z: avg_z,
            stock_top_z: stock_top,
            cut_fraction: if total_cells > 0 {
                cut_cells as f64 / total_cells as f64
            } else {
                0.0
            },
        }
    }
}

/// Result of a single parameter sweep (one param, multiple values).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterSweepResult {
    pub operation: String,
    pub parameter_name: String,
    pub base_value: serde_json::Value,
    pub base_fingerprint: ToolpathFingerprint,
    pub variants: Vec<SweepVariant>,
}

/// One variant in a parameter sweep.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SweepVariant {
    pub value: serde_json::Value,
    pub fingerprint: ToolpathFingerprint,
    pub diff: FingerprintDiff,
    /// Visual artifacts for this variant (SVGs, structural summaries).
    pub artifacts: Option<SweepArtifacts>,
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::str_to_string
)]
mod tests {
    use super::*;
    use crate::geo::P3;

    fn make_test_toolpath() -> Toolpath {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp.rapid_to(P3::new(10.0, 0.0, 10.0));
        tp.feed_to(P3::new(10.0, 0.0, -3.0), 500.0);
        tp.feed_to(P3::new(20.0, 0.0, -3.0), 1000.0);
        tp.feed_to(P3::new(20.0, 10.0, -3.0), 1000.0);
        tp.rapid_to(P3::new(20.0, 10.0, 10.0));
        tp
    }

    fn make_variant_toolpath() -> Toolpath {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp.rapid_to(P3::new(10.0, 0.0, 10.0));
        tp.feed_to(P3::new(10.0, 0.0, -6.0), 500.0); // deeper Z
        tp.feed_to(P3::new(20.0, 0.0, -6.0), 1500.0); // different feed
        tp.feed_to(P3::new(20.0, 10.0, -6.0), 1500.0);
        tp.feed_to(P3::new(20.0, 20.0, -6.0), 1500.0); // extra move
        tp.rapid_to(P3::new(20.0, 20.0, 10.0));
        tp
    }

    #[test]
    fn fingerprint_basic_counts() {
        let tp = make_test_toolpath();
        let fp = ToolpathFingerprint::from_toolpath(&tp);

        assert_eq!(fp.move_count, 6);
        assert_eq!(fp.rapid_count, 3);
        assert_eq!(fp.linear_count, 3);
        assert_eq!(fp.arc_cw_count, 0);
        assert_eq!(fp.arc_ccw_count, 0);
    }

    #[test]
    fn fingerprint_z_levels() {
        let tp = make_test_toolpath();
        let fp = ToolpathFingerprint::from_toolpath(&tp);

        assert_eq!(fp.z_level_count, 2); // -3.0 and 10.0
        assert!((fp.min_z - (-3.0)).abs() < 0.01);
        assert!((fp.max_z - 10.0).abs() < 0.01);
    }

    #[test]
    fn fingerprint_feed_rates() {
        let tp = make_test_toolpath();
        let fp = ToolpathFingerprint::from_toolpath(&tp);

        assert_eq!(fp.feed_rate_count, 2); // 500 and 1000
        assert!((fp.min_feed_rate - 500.0).abs() < 1.0);
        assert!((fp.max_feed_rate - 1000.0).abs() < 1.0);
    }

    #[test]
    fn fingerprint_bbox() {
        let tp = make_test_toolpath();
        let fp = ToolpathFingerprint::from_toolpath(&tp);

        assert!((fp.bbox_min[0] - 0.0).abs() < 0.01);
        assert!((fp.bbox_min[1] - 0.0).abs() < 0.01);
        assert!((fp.bbox_min[2] - (-3.0)).abs() < 0.01);
        assert!((fp.bbox_max[0] - 20.0).abs() < 0.01);
        assert!((fp.bbox_max[1] - 10.0).abs() < 0.01);
        assert!((fp.bbox_max[2] - 10.0).abs() < 0.01);
    }

    #[test]
    fn fingerprint_distances() {
        let tp = make_test_toolpath();
        let fp = ToolpathFingerprint::from_toolpath(&tp);

        assert!(fp.cutting_distance_mm > 0.0);
        assert!(fp.rapid_distance_mm > 0.0);
        assert!(fp.cutting_fraction > 0.0);
        assert!(fp.rapid_fraction > 0.0);
        assert!((fp.cutting_fraction + fp.rapid_fraction - 1.0).abs() < 0.01);
    }

    #[test]
    fn fingerprint_empty_toolpath() {
        let tp = Toolpath::new();
        let fp = ToolpathFingerprint::from_toolpath(&tp);

        assert_eq!(fp.move_count, 0);
        assert_eq!(fp.z_level_count, 0);
        assert_eq!(fp.feed_rate_count, 0);
        assert!((fp.cutting_distance_mm).abs() < 0.001);
        assert!((fp.rapid_distance_mm).abs() < 0.001);
    }

    #[test]
    fn diff_identical_fingerprints() {
        let tp = make_test_toolpath();
        let fp = ToolpathFingerprint::from_toolpath(&tp);
        let diff = diff_fingerprints(&fp, &fp);

        assert!(!diff.has_changes());
        assert!(diff.changed_fields.is_empty());
        assert!(!diff.unchanged_fields.is_empty());
    }

    #[test]
    fn diff_detects_changes() {
        let base = ToolpathFingerprint::from_toolpath(&make_test_toolpath());
        let variant = ToolpathFingerprint::from_toolpath(&make_variant_toolpath());
        let diff = diff_fingerprints(&base, &variant);

        assert!(diff.has_changes());

        // Should detect move_count change (6 → 7)
        let mc = diff
            .field_change("move_count")
            .expect("move_count should change");
        assert_eq!(mc.before, serde_json::json!(6u64));
        assert_eq!(mc.after, serde_json::json!(7u64));

        // Should detect min_z change (-3 → -6)
        let mz = diff.field_change("min_z").expect("min_z should change");
        assert!(mz.delta_percent.unwrap().abs() > 1.0);

        // Should detect feed rate change
        assert!(diff.field_change("max_feed_rate").is_some());

        // Should detect z_levels change
        assert!(diff.field_change("z_levels").is_some());
    }

    #[test]
    fn diff_feed_rate_only_change() {
        let mut tp1 = Toolpath::new();
        tp1.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp1.feed_to(P3::new(10.0, 0.0, -3.0), 1000.0);
        tp1.rapid_to(P3::new(10.0, 0.0, 10.0));

        let mut tp2 = Toolpath::new();
        tp2.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp2.feed_to(P3::new(10.0, 0.0, -3.0), 2000.0);
        tp2.rapid_to(P3::new(10.0, 0.0, 10.0));

        let fp1 = ToolpathFingerprint::from_toolpath(&tp1);
        let fp2 = ToolpathFingerprint::from_toolpath(&tp2);
        let diff = diff_fingerprints(&fp1, &fp2);

        // Feed rate fields should change
        assert!(diff.field_change("min_feed_rate").is_some());
        assert!(diff.field_change("max_feed_rate").is_some());
        assert!(diff.field_change("feed_rates").is_some());

        // Geometry fields should NOT change
        assert!(diff.field_change("move_count").is_none());
        assert!(diff.field_change("min_z").is_none());
        assert!(diff.field_change("bbox_min_x").is_none());
        assert!(diff.field_change("cutting_distance_mm").is_none());
    }

    #[test]
    fn sweep_result_serializes() {
        let tp = make_test_toolpath();
        let fp = ToolpathFingerprint::from_toolpath(&tp);
        let result = ParameterSweepResult {
            operation: "pocket".to_string(),
            parameter_name: "stepover".to_string(),
            base_value: serde_json::json!(2.0),
            base_fingerprint: fp.clone(),
            variants: vec![SweepVariant {
                value: serde_json::json!(1.0),
                fingerprint: fp.clone(),
                diff: diff_fingerprints(&fp, &fp),
                artifacts: None,
            }],
        };

        let json = serde_json::to_string_pretty(&result).unwrap();
        assert!(json.contains("pocket"));
        assert!(json.contains("stepover"));

        // Round-trip
        let parsed: ParameterSweepResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.operation, "pocket");
        assert_eq!(parsed.variants.len(), 1);
    }

    #[test]
    fn svg_summary_extraction() {
        let tp = make_test_toolpath();
        let svg = crate::viz::toolpath_to_svg(&tp, 400.0, 300.0);
        let summary = extract_svg_summary(&svg);

        // Should have both cutting and rapid lines
        assert!(summary.cutting_line_count > 0);
        assert!(summary.rapid_line_count > 0);
        assert!(!summary.unique_colors.is_empty());
    }

    #[test]
    fn artifacts_from_toolpath() {
        let tp = make_test_toolpath();
        let arts = SweepArtifacts::generate(&tp);

        assert!(arts.toolpath_svg.is_some());
        assert!(arts.svg_summary.is_some());
        assert!(arts.svg_summary.is_some());

        let summary = arts.svg_summary.unwrap();
        assert!(summary.cutting_line_count > 0);
    }

    #[test]
    fn artifacts_empty_toolpath() {
        let tp = Toolpath::new();
        let arts = SweepArtifacts::generate(&tp);
        assert!(arts.toolpath_svg.is_none());
        assert!(arts.svg_summary.is_none());
    }

    #[test]
    fn stock_fingerprint_from_fresh_stock() {
        use crate::dexel_stock::TriDexelStock;
        use crate::geo::BoundingBox3;

        let bbox = BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(40.0, 30.0, 25.0),
        };
        let stock = TriDexelStock::from_bounds(&bbox, 1.0);
        let sfp = StockFingerprint::from_stock(&stock);

        assert!(sfp.cells_with_material > 0);
        assert_eq!(sfp.cells_empty, 0);
        assert!(sfp.cut_fraction < 0.01); // fresh stock, nothing cut
        assert!((sfp.max_surface_z - 25.0).abs() < 0.1);
    }

    #[test]
    fn stock_composite_png_renders() {
        use crate::dexel_stock::TriDexelStock;
        use crate::geo::BoundingBox3;

        let bbox = BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(20.0, 20.0, 10.0),
        };
        let stock = TriDexelStock::from_bounds(&bbox, 2.0);
        let pixels = render_stock_composite(&stock, 300, 200);

        // 300*200*4 = 240000 bytes RGBA
        assert_eq!(pixels.len(), 300 * 200 * 4);
        // Should have non-background pixels (not all gray)
        let has_color = pixels
            .chunks(4)
            .any(|p| p[0] != 42 || p[1] != 42 || p[2] != 42);
        assert!(has_color, "Render produced only background pixels");
    }

    #[test]
    fn stock_composite_png_with_cut() {
        use crate::dexel_stock::{StockCutDirection, TriDexelStock};
        use crate::geo::BoundingBox3;
        use crate::tool::FlatEndmill;

        let bbox = BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(30.0, 30.0, 10.0),
        };
        let mut stock = TriDexelStock::from_bounds(&bbox, 1.0);

        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(5.0, 5.0, 12.0));
        tp.feed_to(P3::new(5.0, 5.0, 5.0), 500.0);
        tp.feed_to(P3::new(25.0, 5.0, 5.0), 1000.0);
        tp.feed_to(P3::new(25.0, 25.0, 5.0), 1000.0);
        tp.feed_to(P3::new(5.0, 25.0, 5.0), 1000.0);
        tp.feed_to(P3::new(5.0, 5.0, 5.0), 1000.0);
        tp.rapid_to(P3::new(5.0, 5.0, 12.0));

        let cutter = FlatEndmill::new(6.35, 25.0);
        stock.simulate_toolpath(&tp, &cutter, StockCutDirection::FromTop);

        let pixels = render_stock_composite(&stock, 600, 400);
        assert_eq!(pixels.len(), 600 * 400 * 4);
        // Cut areas should produce darker pixels (walnut color) alongside lighter uncut
        let has_color = pixels
            .chunks(4)
            .any(|p| p[0] != 42 || p[1] != 42 || p[2] != 42);
        assert!(has_color);
    }
}
