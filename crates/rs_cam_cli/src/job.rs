//! TOML job file parsing and execution.
//!
//! A job file defines tools, operations, and output settings in a single
//! TOML file. This replaces long CLI invocations with a declarative config.
//!
//! Example:
//! ```toml
//! [job]
//! output = "part.nc"
//! post = "grbl"
//! spindle_speed = 18000
//! safe_z = 10.0
//!
//! [tools.flat_6mm]
//! type = "flat"
//! diameter = 6.35
//!
//! [[operation]]
//! type = "pocket"
//! input = "design.svg"
//! tool = "flat_6mm"
//! stepover = 2.0
//! depth = 6.0
//! depth_per_pass = 3.0
//! feed_rate = 1000
//! plunge_rate = 500
//! pattern = "zigzag"
//! ```

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rs_cam_core::{
    adaptive::{AdaptiveParams, adaptive_toolpath},
    depth::{DepthStepping, depth_stepped_toolpath},
    dressup::{EntryStyle, apply_dogbones, apply_entry, apply_tabs, even_tabs},
    gcode::get_post_processor,
    pocket::{PocketParams, pocket_toolpath},
    polygon::Polygon2,
    profile::{ProfileParams, ProfileSide, profile_toolpath},
    toolpath::Toolpath,
    zigzag::{ZigzagParams, zigzag_toolpath},
};

// ── TOML types ─────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct JobFile {
    pub job: JobConfig,
    #[serde(default)]
    pub tools: HashMap<String, ToolDef>,
    #[serde(default)]
    pub operation: Vec<OperationDef>,
}

#[derive(Deserialize)]
pub struct JobConfig {
    pub output: PathBuf,
    #[serde(default = "default_post")]
    pub post: String,
    #[serde(default = "default_spindle_speed")]
    pub spindle_speed: u32,
    #[serde(default = "default_safe_z")]
    pub safe_z: f64,
    pub view: Option<PathBuf>,
    pub svg: Option<PathBuf>,
    #[serde(default)]
    pub simulate: bool,
    #[serde(default = "default_sim_resolution")]
    pub sim_resolution: f64,
}

fn default_post() -> String { "grbl".into() }
fn default_spindle_speed() -> u32 { 18000 }
fn default_safe_z() -> f64 { 10.0 }
fn default_sim_resolution() -> f64 { 0.25 }

#[derive(Deserialize)]
pub struct ToolDef {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub diameter: f64,
    /// Corner radius for bull nose
    pub corner_radius: Option<f64>,
    /// Included angle in degrees for V-bit
    pub included_angle: Option<f64>,
    /// Taper half-angle for tapered ball
    pub taper_angle: Option<f64>,
    /// Shaft diameter for tapered ball
    pub shaft_diameter: Option<f64>,
}

#[derive(Deserialize)]
pub struct OperationDef {
    #[serde(rename = "type")]
    pub op_type: String,
    pub input: PathBuf,
    pub tool: String,

    // Common parameters (override job defaults if present)
    pub stepover: Option<f64>,
    pub depth: Option<f64>,
    pub depth_per_pass: Option<f64>,
    pub feed_rate: Option<f64>,
    pub plunge_rate: Option<f64>,
    pub safe_z: Option<f64>,
    pub spindle_speed: Option<u32>,

    // Pocket-specific
    pub pattern: Option<String>,
    pub angle: Option<f64>,
    pub climb: Option<bool>,
    pub entry: Option<String>,

    // Profile-specific
    pub side: Option<String>,
    pub tabs: Option<usize>,
    pub tab_width: Option<f64>,
    pub tab_height: Option<f64>,

    // Dogbone
    pub dogbone: Option<bool>,

    // Adaptive-specific
    pub tolerance: Option<f64>,
}

// ── Parsing ────────────────────────────────────────────────────────────

pub fn parse_job_file(path: &Path) -> Result<JobFile> {
    let content = std::fs::read_to_string(path)
        .context(format!("Failed to read job file: {}", path.display()))?;
    let job: JobFile = toml::from_str(&content)
        .context("Failed to parse TOML job file")?;

    if job.operation.is_empty() {
        bail!("Job file has no [[operation]] entries");
    }
    // Validate all tools referenced by operations exist
    for (i, op) in job.operation.iter().enumerate() {
        if !job.tools.contains_key(&op.tool) {
            bail!(
                "Operation {} references unknown tool '{}'. Available: {:?}",
                i, op.tool, job.tools.keys().collect::<Vec<_>>()
            );
        }
    }

    Ok(job)
}

// ── Tool construction ──────────────────────────────────────────────────

pub fn build_tool_pub(def: &ToolDef) -> Result<Box<dyn rs_cam_core::tool::MillingCutter>> {
    build_tool(def)
}

fn build_tool(def: &ToolDef) -> Result<Box<dyn rs_cam_core::tool::MillingCutter>> {
    use rs_cam_core::tool::*;
    let d = def.diameter;
    let cl = d * 4.0;
    match def.tool_type.as_str() {
        "flat" | "endmill" => Ok(Box::new(FlatEndmill::new(d, cl))),
        "ball" | "ballnose" => Ok(Box::new(BallEndmill::new(d, cl))),
        "bullnose" => {
            let cr = def.corner_radius
                .context("Bull nose tool requires 'corner_radius'")?;
            Ok(Box::new(BullNoseEndmill::new(d, cr, cl)))
        }
        "vbit" => {
            let angle = def.included_angle
                .context("V-bit tool requires 'included_angle'")?;
            Ok(Box::new(VBitEndmill::new(d, angle, cl)))
        }
        "tapered_ball" => {
            let taper = def.taper_angle
                .context("Tapered ball requires 'taper_angle'")?;
            let shaft = def.shaft_diameter
                .context("Tapered ball requires 'shaft_diameter'")?;
            Ok(Box::new(TaperedBallEndmill::new(d, taper, shaft, cl)))
        }
        _ => bail!("Unknown tool type '{}'. Supported: flat, ball, bullnose, vbit, tapered_ball", def.tool_type),
    }
}

// ── Polygon loading ────────────────────────────────────────────────────

fn load_polygons_from(path: &Path) -> Result<Vec<Polygon2>> {
    let ext = path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "svg" => {
            let polys = rs_cam_core::svg_input::load_svg(path, 0.1)
                .context("Failed to load SVG")?;
            if polys.is_empty() { bail!("No closed paths found in SVG file"); }
            Ok(polys)
        }
        "dxf" => {
            let polys = rs_cam_core::dxf_input::load_dxf(path, 5.0)
                .context("Failed to load DXF")?;
            if polys.is_empty() { bail!("No closed entities found in DXF file"); }
            Ok(polys)
        }
        _ => bail!("Unsupported input format '{}'. Supported: .svg, .dxf", ext),
    }
}

// ── Operation execution ────────────────────────────────────────────────

pub fn execute_job(job: &JobFile, job_dir: &Path) -> Result<Toolpath> {
    let mut combined = Toolpath::new();

    for (i, op) in job.operation.iter().enumerate() {
        eprintln!("=== Operation {} ({}) ===", i, op.op_type);

        let tool_def = &job.tools[&op.tool];
        let cutter = build_tool(tool_def)
            .context(format!("Building tool '{}' for operation {}", op.tool, i))?;
        let tool_radius = cutter.diameter() / 2.0;
        eprintln!("  Tool: {} ({}mm {})", op.tool, tool_def.diameter, tool_def.tool_type);

        let safe_z = op.safe_z.unwrap_or(job.job.safe_z);
        let feed_rate = op.feed_rate.unwrap_or(1000.0);
        let plunge_rate = op.plunge_rate.unwrap_or(500.0);

        // Resolve input path relative to job file directory
        let input_path = if op.input.is_absolute() {
            op.input.clone()
        } else {
            job_dir.join(&op.input)
        };

        let tp = match op.op_type.as_str() {
            "pocket" => {
                let polygons = load_polygons_from(&input_path)?;
                let depth = op.depth.context("Pocket requires 'depth'")?;
                let depth_per_pass = op.depth_per_pass.unwrap_or(3.0);
                let stepover = op.stepover.unwrap_or(2.0);
                let stepping = DepthStepping::new(0.0, -depth, depth_per_pass);
                let climb = op.climb.unwrap_or(false);
                let angle = op.angle.unwrap_or(0.0);
                let pattern = op.pattern.as_deref().unwrap_or("contour");

                eprintln!("  {} polygon(s), depth={:.1}mm, pattern={}", polygons.len(), depth, pattern);

                let mut tp = Toolpath::new();
                for poly in &polygons {
                    let poly_tp = depth_stepped_toolpath(&stepping, safe_z, |z| {
                        match pattern {
                            "zigzag" => zigzag_toolpath(poly, &ZigzagParams {
                                tool_radius, stepover, cut_depth: z,
                                feed_rate, plunge_rate, safe_z, angle,
                            }),
                            _ => pocket_toolpath(poly, &PocketParams {
                                tool_radius, stepover, cut_depth: z,
                                feed_rate, plunge_rate, safe_z, climb,
                            }),
                        }
                    });
                    tp.moves.extend(poly_tp.moves);
                }

                // Entry dressup
                if let Some(entry) = &op.entry {
                    if let Some(style) = parse_entry(entry)? {
                        tp = apply_entry(&tp, style, plunge_rate);
                    }
                }
                if op.dogbone.unwrap_or(false) {
                    tp = apply_dogbones(&tp, tool_radius, 170.0);
                }
                tp
            }

            "profile" => {
                let polygons = load_polygons_from(&input_path)?;
                let depth = op.depth.context("Profile requires 'depth'")?;
                let depth_per_pass = op.depth_per_pass.unwrap_or(3.0);
                let stepping = DepthStepping::new(0.0, -depth, depth_per_pass);
                let climb = op.climb.unwrap_or(false);
                let side = match op.side.as_deref().unwrap_or("outside") {
                    "inside" | "in" => ProfileSide::Inside,
                    _ => ProfileSide::Outside,
                };

                eprintln!("  {} polygon(s), depth={:.1}mm, side={:?}", polygons.len(), depth, side);

                let mut tp = Toolpath::new();
                for poly in &polygons {
                    let poly_tp = depth_stepped_toolpath(&stepping, safe_z, |z| {
                        profile_toolpath(poly, &ProfileParams {
                            tool_radius, side, cut_depth: z,
                            feed_rate, plunge_rate, safe_z, climb,
                        })
                    });
                    tp.moves.extend(poly_tp.moves);
                }

                // Entry dressup
                if let Some(entry) = &op.entry {
                    if let Some(style) = parse_entry(entry)? {
                        tp = apply_entry(&tp, style, plunge_rate);
                    }
                }

                // Tabs
                let num_tabs = op.tabs.unwrap_or(0);
                if num_tabs > 0 {
                    let tw = op.tab_width.unwrap_or(5.0);
                    let th = op.tab_height.unwrap_or(2.0);
                    let tab_list = even_tabs(num_tabs, tw, th);
                    tp = apply_tabs(&tp, &tab_list, -depth);
                }
                if op.dogbone.unwrap_or(false) {
                    tp = apply_dogbones(&tp, tool_radius, 170.0);
                }
                tp
            }

            "adaptive" => {
                let polygons = load_polygons_from(&input_path)?;
                let depth = op.depth.context("Adaptive requires 'depth'")?;
                let depth_per_pass = op.depth_per_pass.unwrap_or(3.0);
                let stepover = op.stepover.unwrap_or(2.0);
                let tolerance = op.tolerance.unwrap_or(0.1);
                let stepping = DepthStepping::new(0.0, -depth, depth_per_pass);

                eprintln!("  {} polygon(s), depth={:.1}mm, stepover={:.1}mm", polygons.len(), depth, stepover);

                let mut tp = Toolpath::new();
                for poly in &polygons {
                    let poly_tp = depth_stepped_toolpath(&stepping, safe_z, |z| {
                        adaptive_toolpath(poly, &AdaptiveParams {
                            tool_radius, stepover, cut_depth: z,
                            feed_rate, plunge_rate, safe_z, tolerance,
                        })
                    });
                    tp.moves.extend(poly_tp.moves);
                }
                tp
            }

            _ => bail!("Unknown operation type '{}'. Supported: pocket, profile, adaptive", op.op_type),
        };

        eprintln!(
            "  {} moves, cutting={:.1}mm, rapid={:.1}mm",
            tp.moves.len(), tp.total_cutting_distance(), tp.total_rapid_distance()
        );

        combined.moves.extend(tp.moves);
    }

    Ok(combined)
}

fn parse_entry(entry: &str) -> Result<Option<EntryStyle>> {
    match entry {
        "plunge" => Ok(None),
        "ramp" => Ok(Some(EntryStyle::Ramp { max_angle_deg: 3.0 })),
        "helix" => Ok(Some(EntryStyle::Helix { radius: 2.0, pitch: 1.0 })),
        _ => bail!("Unknown entry style '{}'. Supported: plunge, ramp, helix", entry),
    }
}
