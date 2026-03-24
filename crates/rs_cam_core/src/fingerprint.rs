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
    diff_usize("move_count", a.move_count, b.move_count, &mut changed, &mut unchanged);
    diff_usize("rapid_count", a.rapid_count, b.rapid_count, &mut changed, &mut unchanged);
    diff_usize("linear_count", a.linear_count, b.linear_count, &mut changed, &mut unchanged);
    diff_usize("arc_cw_count", a.arc_cw_count, b.arc_cw_count, &mut changed, &mut unchanged);
    diff_usize("arc_ccw_count", a.arc_ccw_count, b.arc_ccw_count, &mut changed, &mut unchanged);
    diff_usize("z_level_count", a.z_level_count, b.z_level_count, &mut changed, &mut unchanged);
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
    diff_f64_vec("z_levels", &a.z_levels, &b.z_levels, 0.001, &mut changed, &mut unchanged);
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
            field: name.to_string(),
            before: serde_json::Value::from(a as u64),
            after: serde_json::Value::from(b as u64),
            delta_percent: delta_pct,
        });
    } else {
        unchanged.push(name.to_string());
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
            field: name.to_string(),
            before: serde_json::json!(a),
            after: serde_json::json!(b),
            delta_percent: Some(rel_delta * 100.0),
        });
    } else {
        unchanged.push(name.to_string());
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
    let same = a.len() == b.len()
        && a.iter()
            .zip(b.iter())
            .all(|(x, y)| (x - y).abs() < epsilon);

    if !same {
        changed.push(FieldChange {
            field: name.to_string(),
            before: serde_json::json!(a),
            after: serde_json::json!(b),
            delta_percent: None,
        });
    } else {
        unchanged.push(name.to_string());
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
    /// Isometric 3D stock SVG rendered from the dexel mesh.
    pub stock_iso_svg: Option<String>,
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
    /// Generate artifacts for a toolpath. Pass stock for isometric stock render.
    pub fn generate(
        tp: &Toolpath,
        stock: Option<&crate::dexel_stock::TriDexelStock>,
    ) -> Self {
        let toolpath_svg = if tp.moves.is_empty() {
            None
        } else {
            Some(crate::viz::toolpath_to_svg(tp, 800.0, 600.0))
        };

        let svg_summary = toolpath_svg.as_ref().map(|svg| extract_svg_summary(svg));

        let stock_iso_svg = stock.map(|s| stock_isometric_svg(s, 800.0, 600.0));

        Self {
            toolpath_svg,
            stock_iso_svg,
            svg_summary,
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
                colors.insert(color.to_string());
            }
        }
    }

    SvgSummary {
        cutting_line_count: cutting_lines,
        rapid_line_count: rapid_lines,
        unique_colors: colors.into_iter().collect(),
    }
}

/// Render a composite 6-view SVG of the stock: 4 isometric corners + top + bottom.
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
fn stock_isometric_svg(
    stock: &crate::dexel_stock::TriDexelStock,
    width: f64,
    height: f64,
) -> String {
    use crate::dexel_mesh::dexel_stock_to_mesh;
    use std::fmt::Write;

    let mesh = dexel_stock_to_mesh(stock);
    if mesh.vertices.is_empty() || mesh.indices.is_empty() {
        return String::from("<svg xmlns='http://www.w3.org/2000/svg'/>");
    }

    // 3x2 grid layout
    let cols = 3.0;
    let rows = 2.0;
    let cell_w = width / cols;
    let cell_h = height / rows;
    let pad = 4.0;

    // Pre-compute centroid once
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

    // 6 views: (azimuth, elevation, label, grid_col, grid_row)
    let views: &[(f64, f64, &str, f64, f64)] = &[
        (pi / 4.0,       deg30,  "Front-Left",  0.0, 0.0),  // 45° az
        (0.0,            deg90,  "Top",          1.0, 0.0),  // straight down
        (7.0 * pi / 4.0, deg30,  "Front-Right", 2.0, 0.0),  // 315° az
        (3.0 * pi / 4.0, deg30,  "Back-Left",   0.0, 1.0),  // 135° az
        (0.0,            -deg90, "Bottom",       1.0, 1.0),  // straight up
        (5.0 * pi / 4.0, deg30,  "Back-Right",  2.0, 1.0),  // 225° az
    ];

    let mut svg = String::with_capacity(mesh.indices.len() / 3 * 120 * 6);
    let _ = writeln!(
        svg,
        "<svg xmlns='http://www.w3.org/2000/svg' width='{width}' height='{height}'>"
    );
    let _ = writeln!(
        svg,
        "<rect width='{width}' height='{height}' fill='#2a2a2a'/>"
    );

    for &(az, el, label, col, row) in views {
        let vx = col * cell_w;
        let vy = row * cell_h;
        let vw = cell_w - pad;
        let vh = cell_h - pad;

        // Clip group to cell
        let _ = writeln!(
            svg,
            "<g transform='translate({vx},{vy})'>"
        );
        let _ = writeln!(
            svg,
            "<rect x='0' y='0' width='{cw}' height='{ch}' fill='#222' rx='3'/>",
            cw = cell_w, ch = cell_h,
        );

        render_view_into(
            &mut svg, &mesh, vert_count, cx, cy, cz,
            az, el, pad / 2.0, pad / 2.0, vw, vh,
        );

        // Label
        let _ = writeln!(
            svg,
            "<text x='4' y='13' fill='#aaa' font-size='10' font-family='monospace'>{label}</text>"
        );
        let _ = writeln!(svg, "</g>");
    }

    // Overall legend at bottom
    let bbox = &stock.stock_bbox;
    let tri_count = mesh.indices.len() / 3;
    let sx = bbox.max.x - bbox.min.x;
    let sy = bbox.max.y - bbox.min.y;
    let sz = bbox.max.z - bbox.min.z;
    let ly = height - 3.0;
    let _ = writeln!(
        svg,
        "<text x='5' y='{ly}' fill='#888' font-size='9' font-family='monospace'>Stock: {sx:.0}x{sy:.0}x{sz:.0} mm  |  {tri_count} triangles</text>",
    );
    let _ = writeln!(svg, "</svg>");
    svg
}

/// Render one view of a stock mesh into an SVG string, offset to (ox, oy) with size (vw, vh).
#[allow(clippy::indexing_slicing, clippy::too_many_arguments)]
fn render_view_into(
    svg: &mut String,
    mesh: &crate::stock_mesh::StockMesh,
    vert_count: usize,
    cx: f64, cy: f64, cz: f64,
    azimuth: f64,
    elevation: f64,
    ox: f64, oy: f64,
    vw: f64, vh: f64,
) {
    use std::fmt::Write;

    let cos_az = azimuth.cos();
    let sin_az = azimuth.sin();
    let cos_el = elevation.cos();
    let sin_el = elevation.sin();

    // Light direction (rotates with camera so shading is consistent)
    let light_x = sin_az * 0.4 + cos_az * 0.3;
    let light_y = cos_az * 0.4 - sin_az * 0.3;
    let light_z: f64 = 0.866;

    // Project all vertices
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

        if sx < sx_min { sx_min = sx; }
        if sx > sx_max { sx_max = sx; }
        if sy < sy_min { sy_min = sy; }
        if sy > sy_max { sy_max = sy; }
    }

    let margin = 8.0;
    let data_w = (sx_max - sx_min).max(1e-6);
    let data_h = (sy_max - sy_min).max(1e-6);
    let scale = ((vw - 2.0 * margin) / data_w).min((vh - 2.0 * margin) / data_h);

    // Center the projection in the viewport
    let proj_w = data_w * scale;
    let proj_h = data_h * scale;
    let off_x = ox + margin + (vw - 2.0 * margin - proj_w) / 2.0;
    let off_y = oy + margin + (vh - 2.0 * margin - proj_h) / 2.0;

    // Build and sort triangles
    let tri_count = mesh.indices.len() / 3;
    let mut tris: Vec<(f64, usize)> = Vec::with_capacity(tri_count);
    for t in 0..tri_count {
        let i0 = mesh.indices[t * 3] as usize;
        let i1 = mesh.indices[t * 3 + 1] as usize;
        let i2 = mesh.indices[t * 3 + 2] as usize;
        if i0 >= vert_count || i1 >= vert_count || i2 >= vert_count {
            continue;
        }
        let avg_depth = (depths[i0] + depths[i1] + depths[i2]) / 3.0;
        tris.push((avg_depth, t));
    }
    tris.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    // Emit triangles
    for &(_, t) in &tris {
        let i0 = mesh.indices[t * 3] as usize;
        let i1 = mesh.indices[t * 3 + 1] as usize;
        let i2 = mesh.indices[t * 3 + 2] as usize;

        let x0 = off_x + (projected[i0][0] - sx_min) * scale;
        let y0 = off_y + proj_h - (projected[i0][1] - sy_min) * scale;
        let x1 = off_x + (projected[i1][0] - sx_min) * scale;
        let y1 = off_y + proj_h - (projected[i1][1] - sy_min) * scale;
        let x2 = off_x + (projected[i2][0] - sx_min) * scale;
        let y2 = off_y + proj_h - (projected[i2][1] - sy_min) * scale;

        // Face normal for lighting
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

        let dot = ((nx / nlen) * light_x + (ny / nlen) * light_y + (nz / nlen) * light_z)
            .clamp(0.0, 1.0);
        let shade = 0.3 + 0.7 * dot;

        let base_r = (f64::from(mesh.colors[i0 * 3])
            + f64::from(mesh.colors[i1 * 3])
            + f64::from(mesh.colors[i2 * 3])) / 3.0;
        let base_g = (f64::from(mesh.colors[i0 * 3 + 1])
            + f64::from(mesh.colors[i1 * 3 + 1])
            + f64::from(mesh.colors[i2 * 3 + 1])) / 3.0;
        let base_b = (f64::from(mesh.colors[i0 * 3 + 2])
            + f64::from(mesh.colors[i1 * 3 + 2])
            + f64::from(mesh.colors[i2 * 3 + 2])) / 3.0;

        let r = (base_r * shade * 255.0).clamp(0.0, 255.0) as u8;
        let g = (base_g * shade * 255.0).clamp(0.0, 255.0) as u8;
        let b = (base_b * shade * 255.0).clamp(0.0, 255.0) as u8;

        let _ = write!(
            svg,
            "<polygon points='{x0:.1},{y0:.1} {x1:.1},{y1:.1} {x2:.1},{y2:.1}' fill='#{r:02x}{g:02x}{b:02x}' stroke='#{r:02x}{g:02x}{b:02x}' stroke-width='0.3'/>"
        );
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
    clippy::indexing_slicing
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
        let mc = diff.field_change("move_count").expect("move_count should change");
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
        let arts = SweepArtifacts::generate(&tp, None);

        assert!(arts.toolpath_svg.is_some());
        assert!(arts.stock_iso_svg.is_none()); // no stock provided
        assert!(arts.svg_summary.is_some());

        let summary = arts.svg_summary.unwrap();
        assert!(summary.cutting_line_count > 0);
    }

    #[test]
    fn artifacts_empty_toolpath() {
        let tp = Toolpath::new();
        let arts = SweepArtifacts::generate(&tp, None);
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
    fn stock_isometric_svg_renders() {
        use crate::dexel_stock::TriDexelStock;
        use crate::geo::BoundingBox3;

        let bbox = BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(20.0, 20.0, 10.0),
        };
        let stock = TriDexelStock::from_bounds(&bbox, 2.0);
        let svg = stock_isometric_svg(&stock, 400.0, 400.0);

        assert!(svg.contains("<svg"));
        assert!(svg.contains("<polygon"));
        assert!(svg.contains("Stock:"));
    }

    #[test]
    fn stock_isometric_svg_with_cut() {
        use crate::dexel_stock::{StockCutDirection, TriDexelStock};
        use crate::geo::BoundingBox3;
        use crate::tool::FlatEndmill;

        let bbox = BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(30.0, 30.0, 10.0),
        };
        let mut stock = TriDexelStock::from_bounds(&bbox, 1.0);

        // Cut a pocket path through the stock
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

        let svg = stock_isometric_svg(&stock, 600.0, 500.0);
        assert!(svg.contains("<polygon"));
        // Should have more triangles than uncut stock (cut creates new geometry)
        assert!(svg.len() > 1000);
    }
}
