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

// ── 6-view composite renderer ────────────────────────────────────────────
//
// Five defects in this renderer were catalogued in
// `planning/airrun_2026-08-19/RUN_LOG.md` ("Renderer defects the frame bug was
// sitting behind") and are fixed here. They mattered because this project's
// working rule is "never gate on an aggregate without rendering the surface",
// which is worthless if the render itself lies — and these five let a real
// setup-frame bug hide for weeks.

/// Screen basis for one composite panel, in world coordinates.
///
/// The camera is parameterised by an **azimuth** measured from world +Y toward
/// world +X (clockwise seen from above) and an **elevation** above the XY
/// plane. From those, the eye direction — the unit vector pointing from the
/// scene *toward* the viewer — is
///
/// ```text
/// n = ( sin(az)·cos(el),  cos(az)·cos(el),  sin(el) )
/// ```
///
/// and the only screen basis consistent with it is
///
/// ```text
/// r = normalize(Ẑ × n) = ( -cos(az),  sin(az),  0 )                      (right)
/// u = n × r            = ( -sin(az)·sin(el), -cos(az)·sin(el), cos(el) ) (up)
/// ```
///
/// which satisfies `r × u == n` — the condition for a right-handed, i.e.
/// **not mirrored**, image.
///
/// The pre-2026-08-21 renderer used exactly these `u` and `n` but wrote
/// `r = ( cos(az), -sin(az), 0 )`, the negation, so `r × u == -n`: every panel
/// of every composite this repo has ever produced was a mirror image. The
/// RUN_LOG saw it as "the two middle panels are mirror-inconsistent"; the
/// cause is one sign shared by all six panels, and it is fixed here rather
/// than special-cased on the two that made it visible.
///
/// ## Why Top and Bottom differ by a Y flip, not an X flip
///
/// Both polar panels take azimuth π. At el = ±90° the azimuth no longer
/// chooses a viewing direction — only the in-plane roll — and π is the value
/// that puts world +X to the right in *both*, leaving the elevation sign to
/// flip `u` from +Y (Top) to −Y (Bottom).
///
/// That is not an arbitrary pick. It is the flip this CAM system actually
/// models: `compute::transform::FaceUp::Bottom` is documented "Flip 180 deg on
/// X axis" and implemented as `(x, D−y, H−z)` — X survives, Y and Z invert. So
/// the Bottom panel shows the workpiece as it lies after the operator performs
/// the flip the setup describes, and a feature at world (x, y) in Top appears
/// mirrored-in-Y in Bottom. Read edge-for-edge, the pair *is* the real flip.
///
/// (The RUN_LOG's guess that a correct pair "differs by an X flip" describes
/// the other tumble — 180° about Y — which is not the one `FaceUp::Bottom`
/// performs. If `FaceUp::Bottom` ever changes, this convention and the axis
/// captions drawn on the panels must change with it.)
#[derive(Debug, Clone, Copy)]
struct ViewBasis {
    rx: f64,
    ry: f64,
    rz: f64,
    ux: f64,
    uy: f64,
    uz: f64,
    nx: f64,
    ny: f64,
    nz: f64,
    sin_az: f64,
    cos_az: f64,
}

impl ViewBasis {
    fn new(azimuth: f64, elevation: f64) -> Self {
        let sa = azimuth.sin();
        let ca = azimuth.cos();
        let se = elevation.sin();
        let ce = elevation.cos();
        Self {
            rx: -ca,
            ry: sa,
            rz: 0.0,
            ux: -sa * se,
            uy: -ca * se,
            uz: ce,
            nx: sa * ce,
            ny: ca * ce,
            nz: se,
            sin_az: sa,
            cos_az: ca,
        }
    }

    /// Screen right coordinate of a camera-relative world point.
    fn right(&self, x: f64, y: f64, z: f64) -> f64 {
        x * self.rx + y * self.ry + z * self.rz
    }

    /// Screen up coordinate of a camera-relative world point.
    fn up(&self, x: f64, y: f64, z: f64) -> f64 {
        x * self.ux + y * self.uy + z * self.uz
    }

    /// Depth toward the viewer. Larger is nearer.
    fn depth(&self, x: f64, y: f64, z: f64) -> f64 {
        x * self.nx + y * self.ny + z * self.nz
    }
}

/// Elevation of the four isometric corner panels.
const COMPOSITE_ISO_ELEVATION: f64 = std::f64::consts::PI / 6.0;

/// One panel of the 6-view composite.
struct CompositeView {
    /// Caption naming where the **camera** is, in machine terms: +X is the
    /// table's right, +Y runs away from the operator, so −Y is "front".
    ///
    /// The old names were wrong in both axes — "Front-Left" had its eye at
    /// (+X, +Y), i.e. rear-right. Nothing drew them, so nothing caught it.
    label: &'static str,
    /// Second caption line: what this panel's screen axes mean. Drawn so the
    /// convention above is legible on the image itself, not only in source.
    axes: &'static str,
    azimuth: f64,
    elevation: f64,
    col: usize,
    row: usize,
}

/// The six panels, laid out 3 columns × 2 rows.
///
/// ```text
///  ┌────────────┬────────────┬────────────┐
///  │  Rear-Left │    Top     │ Rear-Right │
///  ├────────────┼────────────┼────────────┤
///  │ Front-Left │   Bottom   │ Front-Right│
///  └────────────┴────────────┴────────────┘
/// ```
///
/// Rear views sit above front views to agree with the Top panel, where +Y
/// (away from the operator) is up.
const COMPOSITE_VIEWS: [CompositeView; 6] = [
    CompositeView {
        label: "REAR-LEFT",
        axes: "EYE -X +Y ABOVE",
        azimuth: -std::f64::consts::FRAC_PI_4,
        elevation: COMPOSITE_ISO_ELEVATION,
        col: 0,
        row: 0,
    },
    CompositeView {
        label: "TOP",
        axes: "+X RIGHT +Y UP",
        azimuth: std::f64::consts::PI,
        elevation: std::f64::consts::FRAC_PI_2,
        col: 1,
        row: 0,
    },
    CompositeView {
        label: "REAR-RIGHT",
        axes: "EYE +X +Y ABOVE",
        azimuth: std::f64::consts::FRAC_PI_4,
        elevation: COMPOSITE_ISO_ELEVATION,
        col: 2,
        row: 0,
    },
    CompositeView {
        label: "FRONT-LEFT",
        axes: "EYE -X -Y ABOVE",
        azimuth: 5.0 * std::f64::consts::FRAC_PI_4,
        elevation: COMPOSITE_ISO_ELEVATION,
        col: 0,
        row: 1,
    },
    CompositeView {
        label: "BOTTOM",
        axes: "+X RIGHT +Y DOWN",
        azimuth: std::f64::consts::PI,
        elevation: -std::f64::consts::FRAC_PI_2,
        col: 1,
        row: 1,
    },
    CompositeView {
        label: "FRONT-RIGHT",
        axes: "EYE +X -Y ABOVE",
        azimuth: 3.0 * std::f64::consts::FRAC_PI_4,
        elevation: COMPOSITE_ISO_ELEVATION,
        col: 2,
        row: 1,
    },
];

/// World-frame camera shared by all six panels.
///
/// Anchoring is the point. The old renderer re-centred **each panel** on the
/// mesh's own centroid and auto-fitted **each panel** to that panel's own
/// projected extents, with two consequences:
///
/// * a mesh translated by `StockConfig::origin` rendered pixel-identical to
///   one at the world origin, so a setup drawn in the wrong frame looked
///   right (RUN_LOG defect 1);
/// * no two panels — and no two renders — shared a mm/px, so nothing in the
///   image could be measured or compared (RUN_LOG defect 5).
///
/// One world centre and one scale for all six panels fixes both: geometry
/// that sits off-centre in the frame is drawn off-centre in the panel, which
/// is exactly the signal that was missing.
#[derive(Debug, Clone, Copy)]
struct CompositeCamera {
    cx: f64,
    cy: f64,
    cz: f64,
    /// Pixels per millimetre. Identical for every panel.
    scale: f64,
}

/// Margin, in output pixels, between a panel's edge and the camera frame.
const COMPOSITE_MARGIN_PX: f64 = 6.0;

/// Render a composite 6-view PNG of the stock (4 iso corners + top + bottom).
///
/// Returns raw RGBA pixel buffer and dimensions. Use the `image` crate to
/// encode to PNG/JPEG in test code, or [`save_mesh_composite_png`].
///
/// The camera is anchored to `stock.stock_bbox`. **For a mixed-setup project
/// that is not enough**: `session::compute` hands non-identity setups a
/// zero-rooted effective bbox while identity setups keep the world-frame one
/// (the F-024 note in `CLAUDE.md`), so two setups' stocks carry two different
/// frames and each renders centred in its own. Callers that know the
/// project-level stock bbox should use [`render_stock_composite_in_frame`] and
/// pass it, which draws both setups in one frame and makes the offset visible.
pub fn render_stock_composite(
    stock: &crate::dexel_stock::TriDexelStock,
    width: u32,
    height: u32,
) -> Vec<u8> {
    render_stock_composite_in_frame(stock, &stock.stock_bbox, width, height)
}

/// Render a stock composite in an explicit world frame.
///
/// `frame` is the camera volume: normally the **project** stock bbox, in world
/// coordinates, including `StockConfig::origin`. Every panel is centred on it
/// and scaled by it, so two composites taken with the same `frame` are
/// directly comparable — same mm/px, same world centre, same screen position
/// for the same world point.
///
/// The frame is only ever *expanded* to cover the mesh, never shrunk:
/// anchoring must not silently hide geometry, which is the class of bug this
/// whole change is about.
pub fn render_stock_composite_in_frame(
    stock: &crate::dexel_stock::TriDexelStock,
    frame: &crate::geo::BoundingBox3,
    width: u32,
    height: u32,
) -> Vec<u8> {
    use crate::dexel_mesh::dexel_stock_to_mesh;

    let mut mesh = dexel_stock_to_mesh(stock);
    mesh.apply_height_gradient();
    render_mesh_composite_in_frame(&mesh, Some(frame), width, height)
}

/// Render a toolpath as a 6-view composite PNG, optionally with stock background.
///
/// Converts toolpath moves to a tube mesh and renders through the same
/// pipeline as stock composites. If `background_mesh` is provided, it is
/// rendered dimmed as spatial context with full z-buffer interaction.
///
/// Cutting segments are coloured by
/// [`crate::toolpath_spans::AnnotatedToolpath::classify_span_path`] — the same
/// decision the live 3D viewport makes, not a second copy of it (X-1). Entry /
/// LeadOut / LinkBridge / DressupArtifact get distinct colours; GeometryRefit
/// does not, because an arc-fitted move is ordinary cutting geometry; ordinary
/// cuts carry the per-`DepthPass` lightness shift. Toolpaths without spans fall
/// back to the plain green/orange cut/rapid scheme.
pub fn render_toolpath_composite(
    annotated: &crate::toolpath_spans::AnnotatedToolpath,
    background_mesh: Option<&crate::stock_mesh::StockMesh>,
    width: u32,
    height: u32,
    include_rapids: bool,
) -> Vec<u8> {
    render_toolpath_composite_in_frame(
        annotated,
        background_mesh,
        None,
        width,
        height,
        include_rapids,
    )
}

/// [`render_toolpath_composite`] with an explicit world camera frame.
///
/// Pass the project stock bbox to keep a series of per-toolpath composites on
/// one scale and one origin. Retract heights above the stock still show: the
/// frame expands to cover the mesh, it never clips it away.
pub fn render_toolpath_composite_in_frame(
    annotated: &crate::toolpath_spans::AnnotatedToolpath,
    background_mesh: Option<&crate::stock_mesh::StockMesh>,
    frame: Option<&crate::geo::BoundingBox3>,
    width: u32,
    height: u32,
    include_rapids: bool,
) -> Vec<u8> {
    use crate::stock_mesh::{auto_ribbon_radius, toolpath_to_tube_mesh_with_spans};

    let radius = auto_ribbon_radius(&annotated.toolpath);
    let tp_mesh = toolpath_to_tube_mesh_with_spans(annotated, radius, include_rapids);

    let combined = if let Some(bg) = background_mesh {
        let mut m = bg.with_dimmed_colors(0.35);
        m.append(&tp_mesh);
        m
    } else {
        tp_mesh
    };

    render_mesh_composite_in_frame(&combined, frame, width, height)
}

/// Render a composite 6-view PNG from an existing `StockMesh`.
///
/// Same 6-view layout as [`render_stock_composite`] but skips the
/// dexel-to-mesh conversion — use this when you already have the mesh
/// (e.g. from `SimulationResult::mesh`).
///
/// With no explicit frame the camera falls back to the mesh's own world
/// bounding box — computed **once and shared by all six panels**, so panels
/// are mutually comparable even though two renders of different meshes are
/// not. Prefer [`render_mesh_composite_in_frame`] when a stock bbox is known.
pub fn render_mesh_composite(
    mesh: &crate::stock_mesh::StockMesh,
    width: u32,
    height: u32,
) -> Vec<u8> {
    render_mesh_composite_in_frame(mesh, None, width, height)
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

/// Core of the composite renderer.
///
/// Pipeline, in order: pick a world camera frame → derive one shared camera →
/// rasterize the six panels into a **supersampled** buffer → box-downsample to
/// the requested size → draw panel captions and the frame/scale footer.
///
/// The supersample step is RUN_LOG defect 3. `dexel_stock_to_mesh` emits one
/// wall quad per dexel column — ~0.4 mm apart — while a composite panel
/// resolves ~0.9 mm/px, so the wall quads beat against the pixel grid and
/// produce "vertical striping" that reads as standing material and isn't. The
/// measured stripe pitch tracked the render size (2.91 mm at 1400 px,
/// 1.62 mm at 1600 px), never the geometry. There is no sample rate at these
/// panel sizes that resolves a 0.4 mm feature, so the honest picture is the
/// **average** over the pixel footprint, which is what box-downsampling a
/// supersampled render gives. Do not chase the stripes as geometry.
pub fn render_mesh_composite_in_frame(
    mesh: &crate::stock_mesh::StockMesh,
    frame: Option<&crate::geo::BoundingBox3>,
    width: u32,
    height: u32,
) -> Vec<u8> {
    let w = width as usize;
    let h = height as usize;
    let mut out = vec![0u8; w.saturating_mul(h).saturating_mul(4)];
    fill_background(&mut out);

    let Some(layout) = composite_layout(w, h) else {
        return out;
    };
    let panel_w = layout.panel_w;
    let panel_h = layout.panel_h;
    let footer_scale = layout.footer_scale;

    let Some(cam_frame) = composite_camera_frame(mesh, frame) else {
        return out;
    };
    let scale = composite_scale(&cam_frame, panel_w as f64, panel_h as f64);
    let centre = cam_frame.center();
    let cam = CompositeCamera {
        cx: centre.x,
        cy: centre.y,
        cz: centre.z,
        scale,
    };

    if !mesh.vertices.is_empty() && !mesh.indices.is_empty() {
        let ss = composite_supersample(w, h);
        let cell_w = panel_w * ss;
        let cell_h = panel_h * ss;
        let hi_w = cell_w * 3;
        let hi_h = cell_h * 2;
        let mut hi = vec![0u8; hi_w * hi_h * 4];
        fill_background(&mut hi);

        // The camera is in millimetres; only the pixel scale changes with the
        // supersample factor, so the downsampled image is the same framing.
        let hi_cam = CompositeCamera {
            scale: cam.scale * ss as f64,
            ..cam
        };
        let vert_count = mesh.vertices.len() / 3;
        for view in &COMPOSITE_VIEWS {
            let basis = ViewBasis::new(view.azimuth, view.elevation);
            render_view_to_pixels(
                &mut hi,
                hi_w,
                mesh,
                vert_count,
                &hi_cam,
                &basis,
                view.col * cell_w,
                view.row * cell_h,
                cell_w,
                cell_h,
            );
        }

        downsample_into(&hi, hi_w, &mut out, w, panel_w * 3, panel_h * 2, ss);
    }

    draw_panel_frames(&mut out, w, h, panel_w, panel_h);
    draw_composite_footer(&mut out, w, h, panel_h, footer_scale, &cam_frame, scale);
    out
}

/// Pixel geometry of a composite at a given output size.
#[derive(Debug, Clone, Copy)]
struct CompositeLayout {
    panel_w: usize,
    panel_h: usize,
    /// Glyph scale of the reserved footer strip; also fixes its height.
    footer_scale: usize,
}

/// Panel grid plus reserved footer strip, or `None` if the requested size is
/// too small to lay out.
///
/// The footer is **reserved**, not overlaid: an overlay would hide geometry,
/// which is the class of bug this whole change is about.
fn composite_layout(w: usize, h: usize) -> Option<CompositeLayout> {
    if w < 3 || h < 2 {
        return None;
    }
    let footer_scale = (w / 700).clamp(1, 2);
    let footer_h = (GLYPH_H + 4) * footer_scale;
    let panel_w = w / 3;
    let panel_h = if h > footer_h * 3 {
        (h - footer_h) / 2
    } else {
        h / 2
    };
    if panel_w == 0 || panel_h == 0 {
        return None;
    }
    Some(CompositeLayout {
        panel_w,
        panel_h,
        footer_scale,
    })
}

/// One panel of a composite, located in the output image.
#[derive(Debug, Clone, Copy)]
pub struct CompositePanel {
    /// Where the camera is, in machine terms (+X table right, +Y away from
    /// the operator). Matches the caption drawn on the panel.
    pub label: &'static str,
    /// What this panel's screen axes mean.
    pub axes: &'static str,
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

/// Where each panel of a composite lands in the output image.
///
/// Exposed so a caller — or a sentry test — can crop or annotate one panel
/// without re-deriving the layout arithmetic and silently drifting from it.
/// Order matches the drawn layout: rear row left-to-right, then front row.
/// Empty when the requested size is too small to lay out.
pub fn composite_panel_layout(width: u32, height: u32) -> Vec<CompositePanel> {
    let Some(l) = composite_layout(width as usize, height as usize) else {
        return Vec::new();
    };
    COMPOSITE_VIEWS
        .iter()
        .map(|v| CompositePanel {
            label: v.label,
            axes: v.axes,
            x: v.col * l.panel_w,
            y: v.row * l.panel_h,
            width: l.panel_w,
            height: l.panel_h,
        })
        .collect()
}

/// Dark-gray background, opaque.
fn fill_background(buf: &mut [u8]) {
    for px in buf.chunks_exact_mut(4) {
        px.fill(42);
        if let Some(a) = px.get_mut(3) {
            *a = 255;
        }
    }
}

/// Camera volume: the supplied frame, expanded to cover the mesh.
///
/// Expand-only is deliberate — see [`render_stock_composite_in_frame`].
fn composite_camera_frame(
    mesh: &crate::stock_mesh::StockMesh,
    frame: Option<&crate::geo::BoundingBox3>,
) -> Option<crate::geo::BoundingBox3> {
    use crate::geo::{BoundingBox3, P3};

    let mut bb = BoundingBox3::empty();
    let mut any = false;

    if let Some(f) = frame
        && f.min.x.is_finite()
        && f.min.y.is_finite()
        && f.min.z.is_finite()
        && f.max.x >= f.min.x
        && f.max.y >= f.min.y
        && f.max.z >= f.min.z
    {
        bb.expand_to(f.min);
        bb.expand_to(f.max);
        any = true;
    }

    for v in mesh.vertices.chunks_exact(3) {
        let (Some(&x), Some(&y), Some(&z)) = (v.first(), v.get(1), v.get(2)) else {
            continue;
        };
        if !(x.is_finite() && y.is_finite() && z.is_finite()) {
            continue;
        }
        bb.expand_to(P3::new(f64::from(x), f64::from(y), f64::from(z)));
        any = true;
    }

    if any { Some(bb) } else { None }
}

/// One mm→px scale for every panel: the largest that fits the camera volume
/// inside the tightest of the six projections.
///
/// The projected half-extent of an axis-aligned box is the box's support
/// function along the screen axis, so no corner enumeration is needed.
fn composite_scale(frame: &crate::geo::BoundingBox3, panel_w: f64, panel_h: f64) -> f64 {
    let hx = ((frame.max.x - frame.min.x) * 0.5).max(1e-6);
    let hy = ((frame.max.y - frame.min.y) * 0.5).max(1e-6);
    let hz = ((frame.max.z - frame.min.z) * 0.5).max(1e-6);

    let usable_w = (panel_w - 2.0 * COMPOSITE_MARGIN_PX).max(1.0);
    let usable_h = (panel_h - 2.0 * COMPOSITE_MARGIN_PX).max(1.0);

    let mut scale = f64::MAX;
    for view in &COMPOSITE_VIEWS {
        let b = ViewBasis::new(view.azimuth, view.elevation);
        let ex = (hx * b.rx.abs() + hy * b.ry.abs() + hz * b.rz.abs()).max(1e-6);
        let ey = (hx * b.ux.abs() + hy * b.uy.abs() + hz * b.uz.abs()).max(1e-6);
        let s = (usable_w / (2.0 * ex)).min(usable_h / (2.0 * ey));
        if s < scale {
            scale = s;
        }
    }
    if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    }
}

/// Supersample factor, capped so the intermediate buffer stays near 64 MB.
fn composite_supersample(w: usize, h: usize) -> usize {
    let px = w.saturating_mul(h);
    if px <= 1_800_000 {
        3
    } else if px <= 4_000_000 {
        2
    } else {
        1
    }
}

/// Box-filter the supersampled buffer down into the output buffer.
// SAFETY: `src` is `src_w * region_h * ss` rows of `src_w` RGBA pixels and
// `dst` is at least `dst_w * region_h` RGBA pixels; every index below is
// `(row_in_range * width + col_in_range) * 4 + {0..3}`, bounded by the loop
// ranges that were used to size both buffers.
#[allow(clippy::indexing_slicing, clippy::too_many_arguments)]
fn downsample_into(
    src: &[u8],
    src_w: usize,
    dst: &mut [u8],
    dst_w: usize,
    region_w: usize,
    region_h: usize,
    ss: usize,
) {
    if ss == 0 {
        return;
    }
    let inv = 1.0f32 / (ss * ss) as f32;
    for oy in 0..region_h {
        for ox in 0..region_w {
            let mut r = 0.0f32;
            let mut g = 0.0f32;
            let mut b = 0.0f32;
            for sy in 0..ss {
                let row = (oy * ss + sy) * src_w;
                for sx in 0..ss {
                    let si = (row + ox * ss + sx) * 4;
                    if si + 2 >= src.len() {
                        continue;
                    }
                    r += f32::from(src[si]);
                    g += f32::from(src[si + 1]);
                    b += f32::from(src[si + 2]);
                }
            }
            let di = (oy * dst_w + ox) * 4;
            if di + 3 >= dst.len() {
                continue;
            }
            dst[di] = (r * inv) as u8;
            dst[di + 1] = (g * inv) as u8;
            dst[di + 2] = (b * inv) as u8;
            dst[di + 3] = 255;
        }
    }
}

/// Rasterize one view of the mesh into an RGBA pixel buffer.
// SAFETY: vertex/colour reads are `i * 3 + {0,1,2}` with `i < vert_count`,
// and `vert_count` is `vertices.len() / 3`; triangle indices are range-checked
// against `vert_count` before use.
#[allow(clippy::indexing_slicing, clippy::too_many_arguments)]
fn render_view_to_pixels(
    pixels: &mut [u8],
    buf_w: usize,
    mesh: &crate::stock_mesh::StockMesh,
    vert_count: usize,
    cam: &CompositeCamera,
    basis: &ViewBasis,
    vx: usize,
    vy: usize,
    vw: usize,
    vh: usize,
) {
    if vw == 0 || vh == 0 || vert_count == 0 {
        return;
    }
    if mesh.colors.len() < vert_count * 3 {
        return;
    }

    // World-space lamp, fixed relative to the panel's azimuth (over the
    // viewer's shoulder). Independent of the screen basis, so the 2026-08-21
    // mirror fix leaves the shading exactly where it was.
    let light_x = basis.sin_az * 0.4 + basis.cos_az * 0.3;
    let light_y = basis.cos_az * 0.4 - basis.sin_az * 0.3;
    let light_z: f64 = 0.866;

    let half_w = vw as f64 * 0.5;
    let half_h = vh as f64 * 0.5;

    let mut projected: Vec<[f64; 2]> = Vec::with_capacity(vert_count);
    let mut depths: Vec<f64> = Vec::with_capacity(vert_count);

    for i in 0..vert_count {
        let x = f64::from(mesh.vertices[i * 3]) - cam.cx;
        let y = f64::from(mesh.vertices[i * 3 + 1]) - cam.cy;
        let z = f64::from(mesh.vertices[i * 3 + 2]) - cam.cz;

        // Fixed camera: the panel centre is the frame centre, always. No
        // per-panel auto-fit — that was RUN_LOG defect 5.
        let px = half_w + basis.right(x, y, z) * cam.scale;
        let py = half_h - basis.up(x, y, z) * cam.scale;

        projected.push([px, py]);
        depths.push(basis.depth(x, y, z));
    }

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

        let [px0, py0] = projected[i0];
        let [px1, py1] = projected[i1];
        let [px2, py2] = projected[i2];

        // Per-face normal for Phong shading
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
        let shade = 0.35 + 0.65 * dot;

        // Per-vertex colors for interpolation
        let c0 = [
            f64::from(mesh.colors[i0 * 3]),
            f64::from(mesh.colors[i0 * 3 + 1]),
            f64::from(mesh.colors[i0 * 3 + 2]),
        ];
        let c1 = [
            f64::from(mesh.colors[i1 * 3]),
            f64::from(mesh.colors[i1 * 3 + 1]),
            f64::from(mesh.colors[i1 * 3 + 2]),
        ];
        let c2 = [
            f64::from(mesh.colors[i2 * 3]),
            f64::from(mesh.colors[i2 * 3 + 1]),
            f64::from(mesh.colors[i2 * 3 + 2]),
        ];

        // Rasterize with per-pixel color interpolation
        rasterize_triangle(
            pixels, &mut zbuf, buf_w, vx, vy, vw, vh, px0, py0, px1, py1, px2, py2, tri_depth,
            shade, c0, c1, c2,
        );
    }
}

/// Scanline rasterize a triangle with per-pixel color interpolation and z-test.
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
    shade: f64,
    c0: [f64; 3],
    c1: [f64; 3],
    c2: [f64; 3],
) {
    if vw == 0 || vh == 0 {
        return;
    }
    let fmin_x = x0.min(x1).min(x2);
    let fmax_x = x0.max(x1).max(x2);
    let fmin_y = y0.min(y1).min(y2);
    let fmax_y = y0.max(y1).max(y2);
    if !(fmin_x.is_finite() && fmax_x.is_finite() && fmin_y.is_finite() && fmax_y.is_finite()) {
        return;
    }
    // With a fixed camera, geometry outside the frame is genuinely off-panel;
    // reject it up front rather than letting a saturating cast fold it onto
    // pixel 0, which would smear off-screen triangles along the panel edge.
    let last_x = vw.saturating_sub(1) as f64;
    let last_y = vh.saturating_sub(1) as f64;
    if fmax_x < 0.0 || fmax_y < 0.0 || fmin_x > last_x || fmin_y > last_y {
        return;
    }

    // Bounding box clipped to viewport
    let min_x = fmin_x.max(0.0) as usize;
    let max_x = (fmax_x.max(0.0) as usize).min(vw.saturating_sub(1));
    let min_y = fmin_y.max(0.0) as usize;
    let max_y = (fmax_y.max(0.0) as usize).min(vh.saturating_sub(1));

    // Edge function constants
    let dx01 = x1 - x0;
    let dy01 = y1 - y0;
    let dx12 = x2 - x1;
    let dy12 = y2 - y1;
    let dx20 = x0 - x2;
    let dy20 = y0 - y2;

    // Total area (2x) for barycentric normalization
    let area = (x1 - x0) * (y2 - y0) - (x2 - x0) * (y1 - y0);
    let inv_area = if area.abs() < 1e-10 { 0.0 } else { 1.0 / area };

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
                    if zi >= zbuf.len() {
                        continue;
                    }
                    if depth > zbuf[zi] {
                        zbuf[zi] = depth;

                        // Barycentric weights for per-pixel color interpolation
                        let w0 = ((x1 - fx) * (y2 - fy) - (x2 - fx) * (y1 - fy)) * inv_area;
                        let w1 = ((x2 - fx) * (y0 - fy) - (x0 - fx) * (y2 - fy)) * inv_area;
                        let w2 = 1.0 - w0 - w1;

                        let r = ((c0[0] * w0 + c1[0] * w1 + c2[0] * w2) * shade * 255.0)
                            .clamp(0.0, 255.0) as u8;
                        let g = ((c0[1] * w0 + c1[1] * w1 + c2[1] * w2) * shade * 255.0)
                            .clamp(0.0, 255.0) as u8;
                        let b = ((c0[2] * w0 + c1[2] * w1 + c2[2] * w2) * shade * 255.0)
                            .clamp(0.0, 255.0) as u8;

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

// ── Overlay: panel separators, captions, frame/scale footer ──────────────
//
// RUN_LOG defect 4. Nothing was drawn on these panels, so a viewer had no way
// to tell which corner they were looking at — and the internal names were
// wrong anyway. Captions are drawn *after* the downsample so the glyphs stay
// crisp, and the footer strip is reserved rather than overlaid so it cannot
// hide geometry.

const GLYPH_W: usize = 5;
const GLYPH_H: usize = 7;

/// 5×7 bitmap glyph, one `u8` per row, bit `GLYPH_W-1` leftmost.
///
/// Deliberately hand-rolled: the composite is core-library output and adding a
/// font crate to `rs_cam_core` for six captions is not a trade worth making.
/// Unmapped characters render blank.
fn glyph(c: char) -> [u8; GLYPH_H] {
    match c.to_ascii_uppercase() {
        'A' => [0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'B' => [0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E],
        'C' => [0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E],
        'D' => [0x1E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1E],
        'E' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F],
        'F' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10],
        'G' => [0x0E, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0F],
        'H' => [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'I' => [0x0E, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E],
        'J' => [0x07, 0x02, 0x02, 0x02, 0x02, 0x12, 0x0C],
        'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F],
        'M' => [0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11],
        'N' => [0x11, 0x11, 0x19, 0x15, 0x13, 0x11, 0x11],
        'O' => [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'P' => [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10],
        'Q' => [0x0E, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0D],
        'R' => [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11],
        'S' => [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E],
        'T' => [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'V' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0A, 0x04],
        'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x1B, 0x11],
        'X' => [0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11],
        'Y' => [0x11, 0x11, 0x0A, 0x04, 0x04, 0x04, 0x04],
        'Z' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F],
        '0' => [0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E],
        '1' => [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E],
        '2' => [0x0E, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1F],
        '3' => [0x1F, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0E],
        '4' => [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02],
        '5' => [0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E],
        '6' => [0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E],
        '7' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        '8' => [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E],
        '9' => [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C],
        '-' => [0x00, 0x00, 0x00, 0x1F, 0x00, 0x00, 0x00],
        '+' => [0x00, 0x04, 0x04, 0x1F, 0x04, 0x04, 0x00],
        '.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C],
        ',' => [0x00, 0x00, 0x00, 0x00, 0x0C, 0x04, 0x08],
        ':' => [0x00, 0x0C, 0x0C, 0x00, 0x0C, 0x0C, 0x00],
        '/' => [0x01, 0x01, 0x02, 0x04, 0x08, 0x10, 0x10],
        '(' => [0x02, 0x04, 0x08, 0x08, 0x08, 0x04, 0x02],
        ')' => [0x08, 0x04, 0x02, 0x02, 0x02, 0x04, 0x08],
        _ => [0x00; GLYPH_H],
    }
}

/// Width in pixels of `text` rendered at `scale`.
fn text_width_px(text: &str, scale: usize) -> usize {
    text.chars().count() * (GLYPH_W + 1) * scale
}

// SAFETY: every write is guarded by an explicit `i + 3 < pixels.len()` test.
#[allow(clippy::indexing_slicing, clippy::too_many_arguments)]
fn fill_rect(
    pixels: &mut [u8],
    buf_w: usize,
    buf_h: usize,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    color: [u8; 3],
) {
    for py in y..y.saturating_add(h).min(buf_h) {
        for px in x..x.saturating_add(w).min(buf_w) {
            let i = (py * buf_w + px) * 4;
            if i + 3 < pixels.len() {
                pixels[i] = color[0];
                pixels[i + 1] = color[1];
                pixels[i + 2] = color[2];
                pixels[i + 3] = 255;
            }
        }
    }
}

// SAFETY: every write is guarded by an explicit `i + 3 < pixels.len()` test.
#[allow(clippy::indexing_slicing, clippy::too_many_arguments)]
fn draw_text(
    pixels: &mut [u8],
    buf_w: usize,
    buf_h: usize,
    x: usize,
    y: usize,
    text: &str,
    scale: usize,
    color: [u8; 3],
) {
    if scale == 0 {
        return;
    }
    let mut cursor = x;
    for ch in text.chars() {
        if cursor >= buf_w {
            return;
        }
        let rows = glyph(ch);
        for (row, bits) in rows.iter().enumerate() {
            for col in 0..GLYPH_W {
                if bits & (1u8 << (GLYPH_W - 1 - col)) == 0 {
                    continue;
                }
                for dy in 0..scale {
                    for dx in 0..scale {
                        let px = cursor + col * scale + dx;
                        let py = y + row * scale + dy;
                        if px >= buf_w || py >= buf_h {
                            continue;
                        }
                        let i = (py * buf_w + px) * 4;
                        if i + 3 < pixels.len() {
                            pixels[i] = color[0];
                            pixels[i + 1] = color[1];
                            pixels[i + 2] = color[2];
                            pixels[i + 3] = 255;
                        }
                    }
                }
            }
        }
        cursor += (GLYPH_W + 1) * scale;
    }
}

/// Text on a solid plate, so a caption stays readable over light material.
#[allow(clippy::too_many_arguments)]
fn draw_label(
    pixels: &mut [u8],
    buf_w: usize,
    buf_h: usize,
    x: usize,
    y: usize,
    text: &str,
    scale: usize,
    color: [u8; 3],
) {
    let pad = scale;
    fill_rect(
        pixels,
        buf_w,
        buf_h,
        x.saturating_sub(pad),
        y.saturating_sub(pad),
        text_width_px(text, scale) + pad,
        GLYPH_H * scale + 2 * pad,
        [18, 18, 18],
    );
    draw_text(pixels, buf_w, buf_h, x, y, text, scale, color);
}

/// Panel separators plus the two caption lines on each panel.
fn draw_panel_frames(pixels: &mut [u8], w: usize, h: usize, panel_w: usize, panel_h: usize) {
    let grid = [60u8, 60, 60];
    for col in 1..3 {
        fill_rect(pixels, w, h, col * panel_w, 0, 1, panel_h * 2, grid);
    }
    fill_rect(pixels, w, h, 0, panel_h, panel_w * 3, 1, grid);

    let scale = (panel_w / 150).clamp(1, 3);
    let sub_scale = scale.saturating_sub(1).max(1);
    for view in &COMPOSITE_VIEWS {
        let x = view.col * panel_w + 3 * scale;
        let y = view.row * panel_h + 3 * scale;
        draw_label(pixels, w, h, x, y, view.label, scale, [255, 255, 255]);
        draw_label(
            pixels,
            w,
            h,
            x,
            y + (GLYPH_H + 3) * scale,
            view.axes,
            sub_scale,
            [170, 175, 185],
        );
    }
}

/// Footer naming the world frame the composite was drawn in, and its scale.
///
/// This is the readable half of the fix for RUN_LOG defect 1. Two composites
/// of the same project taken in different frames used to be indistinguishable;
/// now the frame is printed on the image, so "these two setups were drawn in
/// different frames" is legible without measuring anything.
fn draw_composite_footer(
    pixels: &mut [u8],
    w: usize,
    h: usize,
    panel_h: usize,
    scale: usize,
    frame: &crate::geo::BoundingBox3,
    px_per_mm: f64,
) {
    let strip_top = panel_h * 2;
    if strip_top >= h {
        return;
    }
    fill_rect(pixels, w, h, 0, strip_top, w, h - strip_top, [18, 18, 18]);

    let mm_per_px = if px_per_mm > 0.0 {
        1.0 / px_per_mm
    } else {
        f64::NAN
    };
    let text = format!(
        "FRAME X {:.1}..{:.1}  Y {:.1}..{:.1}  Z {:.1}..{:.1}  SCALE {:.3} MM/PX",
        frame.min.x, frame.max.x, frame.min.y, frame.max.y, frame.min.z, frame.max.z, mm_per_px
    );
    draw_text(
        pixels,
        w,
        h,
        2 * scale,
        strip_top + 2 * scale,
        &text,
        scale,
        [200, 205, 215],
    );
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
