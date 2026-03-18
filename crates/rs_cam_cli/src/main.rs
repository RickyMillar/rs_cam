use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand, ValueEnum};
use rs_cam_core::{
    arcfit::fit_arcs,
    depth::{DepthStepping, depth_stepped_toolpath},
    dressup::{EntryStyle, apply_entry, apply_tabs, even_tabs},
    dropcutter::batch_drop_cutter,
    gcode::{emit_gcode, get_post_processor},
    mesh::{SpatialIndex, TriangleMesh},
    pocket::{PocketParams, pocket_toolpath},
    polygon::Polygon2,
    profile::{ProfileParams, ProfileSide, profile_toolpath},
    tool::{BallEndmill, BullNoseEndmill, FlatEndmill, MillingCutter, TaperedBallEndmill, VBitEndmill},
    toolpath::{Toolpath, raster_toolpath_from_grid},
    waterline::{WaterlineParams, waterline_toolpath},
    zigzag::{ZigzagParams, zigzag_toolpath},
};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "rs_cam", about = "3-axis wood router CAM toolpath generator")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Clone, ValueEnum)]
enum ClearingPattern {
    /// Contour-parallel (concentric offset) pattern
    Contour,
    /// Zigzag/raster pattern
    Zigzag,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate 3D finishing toolpath using drop-cutter algorithm
    #[command(name = "drop-cutter")]
    DropCutter {
        /// Input STL file
        input: PathBuf,

        /// STL units: mm (default), m, cm, inch. Scales vertices to mm.
        #[arg(long, default_value = "mm")]
        units: String,

        /// Custom scale factor (overrides --units if set)
        #[arg(long)]
        scale: Option<f64>,

        /// Tool specification: type:diameter (e.g., ball:6.35, flat:6.35)
        #[arg(long)]
        tool: String,

        /// Step-over distance in mm
        #[arg(long, default_value = "1.0")]
        stepover: f64,

        /// Feed rate in mm/min
        #[arg(long, default_value = "1000.0")]
        feed_rate: f64,

        /// Plunge rate in mm/min
        #[arg(long, default_value = "500.0")]
        plunge_rate: f64,

        /// Spindle speed in RPM
        #[arg(long, default_value = "18000")]
        spindle_speed: u32,

        /// Safe Z height for rapid moves
        #[arg(long, default_value = "10.0")]
        safe_z: f64,

        /// Minimum Z (won't cut below this)
        #[arg(long, default_value = "-100.0")]
        min_z: f64,

        /// Post-processor: grbl, linuxcnc
        #[arg(long, default_value = "grbl")]
        post: String,

        /// Output G-code file
        #[arg(short, long)]
        output: PathBuf,

        /// Optional SVG preview output (top-down toolpath visualization)
        #[arg(long)]
        svg: Option<PathBuf>,

        /// Optional 3D HTML viewer output (mesh + toolpath, opens in browser)
        #[arg(long)]
        view: Option<PathBuf>,
    },

    /// Clear a 2D pocket from SVG or DXF boundary
    Pocket {
        /// Input file (SVG or DXF)
        input: PathBuf,

        /// Tool specification: type:diameter (e.g., flat:6.35)
        #[arg(long)]
        tool: String,

        /// Step-over distance in mm
        #[arg(long, default_value = "2.0")]
        stepover: f64,

        /// Total depth in mm (positive, e.g. 12.0)
        #[arg(long)]
        depth: f64,

        /// Maximum depth per pass in mm
        #[arg(long, default_value = "3.0")]
        depth_per_pass: f64,

        /// Feed rate in mm/min
        #[arg(long, default_value = "1000.0")]
        feed_rate: f64,

        /// Plunge rate in mm/min
        #[arg(long, default_value = "500.0")]
        plunge_rate: f64,

        /// Spindle speed in RPM
        #[arg(long, default_value = "18000")]
        spindle_speed: u32,

        /// Safe Z height for rapid moves
        #[arg(long, default_value = "10.0")]
        safe_z: f64,

        /// Clearing pattern
        #[arg(long, value_enum, default_value = "contour")]
        pattern: ClearingPattern,

        /// Zigzag angle in degrees (only for zigzag pattern)
        #[arg(long, default_value = "0.0")]
        angle: f64,

        /// Use climb milling (CW direction)
        #[arg(long)]
        climb: bool,

        /// Entry style: plunge, ramp, helix
        #[arg(long, default_value = "plunge")]
        entry: String,

        /// Post-processor: grbl, linuxcnc
        #[arg(long, default_value = "grbl")]
        post: String,

        /// Output G-code file
        #[arg(short, long)]
        output: PathBuf,

        /// Optional SVG preview output
        #[arg(long)]
        svg: Option<PathBuf>,

        /// Optional 3D HTML viewer output
        #[arg(long)]
        view: Option<PathBuf>,
    },

    /// Cut along a 2D profile from SVG or DXF boundary
    Profile {
        /// Input file (SVG or DXF)
        input: PathBuf,

        /// Tool specification: type:diameter (e.g., flat:6.35)
        #[arg(long)]
        tool: String,

        /// Total depth in mm (positive, e.g. 12.0)
        #[arg(long)]
        depth: f64,

        /// Maximum depth per pass in mm
        #[arg(long, default_value = "3.0")]
        depth_per_pass: f64,

        /// Cut side: inside or outside
        #[arg(long, default_value = "outside")]
        side: String,

        /// Feed rate in mm/min
        #[arg(long, default_value = "1000.0")]
        feed_rate: f64,

        /// Plunge rate in mm/min
        #[arg(long, default_value = "500.0")]
        plunge_rate: f64,

        /// Spindle speed in RPM
        #[arg(long, default_value = "18000")]
        spindle_speed: u32,

        /// Safe Z height for rapid moves
        #[arg(long, default_value = "10.0")]
        safe_z: f64,

        /// Use climb milling (CW direction)
        #[arg(long)]
        climb: bool,

        /// Number of holding tabs (0 to disable)
        #[arg(long, default_value = "0")]
        tabs: usize,

        /// Tab width in mm
        #[arg(long, default_value = "5.0")]
        tab_width: f64,

        /// Tab height in mm
        #[arg(long, default_value = "2.0")]
        tab_height: f64,

        /// Entry style: plunge, ramp, helix
        #[arg(long, default_value = "plunge")]
        entry: String,

        /// Post-processor: grbl, linuxcnc
        #[arg(long, default_value = "grbl")]
        post: String,

        /// Output G-code file
        #[arg(short, long)]
        output: PathBuf,

        /// Optional SVG preview output
        #[arg(long)]
        svg: Option<PathBuf>,

        /// Optional 3D HTML viewer output
        #[arg(long)]
        view: Option<PathBuf>,
    },

    /// Generate 3D waterline (constant-Z contour) toolpath from STL
    Waterline {
        /// Input STL file
        input: PathBuf,

        /// STL units: mm (default), m, cm, inch
        #[arg(long, default_value = "mm")]
        units: String,

        /// Custom scale factor (overrides --units if set)
        #[arg(long)]
        scale: Option<f64>,

        /// Tool specification: type:diameter (e.g., ball:6.35)
        #[arg(long)]
        tool: String,

        /// Z step between waterline passes (mm)
        #[arg(long, default_value = "1.0")]
        z_step: f64,

        /// Fiber sampling spacing (mm, smaller = more accurate)
        #[arg(long, default_value = "1.0")]
        sampling: f64,

        /// Start Z height (top of cut)
        #[arg(long)]
        start_z: Option<f64>,

        /// Final Z height (bottom of cut)
        #[arg(long)]
        final_z: Option<f64>,

        /// Feed rate in mm/min
        #[arg(long, default_value = "1000.0")]
        feed_rate: f64,

        /// Plunge rate in mm/min
        #[arg(long, default_value = "500.0")]
        plunge_rate: f64,

        /// Spindle speed in RPM
        #[arg(long, default_value = "18000")]
        spindle_speed: u32,

        /// Safe Z height for rapid moves
        #[arg(long, default_value = "10.0")]
        safe_z: f64,

        /// Fit G2/G3 arcs (tolerance in mm, 0 to disable)
        #[arg(long, default_value = "0.0")]
        arc_tolerance: f64,

        /// Post-processor: grbl, linuxcnc, mach3
        #[arg(long, default_value = "grbl")]
        post: String,

        /// Output G-code file
        #[arg(short, long)]
        output: PathBuf,

        /// Optional SVG preview output
        #[arg(long)]
        svg: Option<PathBuf>,

        /// Optional 3D HTML viewer output
        #[arg(long)]
        view: Option<PathBuf>,
    },
}

fn parse_tool(spec: &str) -> Result<Box<dyn MillingCutter>> {
    let parts: Vec<&str> = spec.split(':').collect();
    if parts.len() < 2 {
        bail!(
            "Tool spec must be type:diameter[:params] (e.g., ball:6.35, \
             bullnose:10:2, vbit:10:90, tapered_ball:6:10:12)"
        );
    }

    let diameter: f64 = parts[1]
        .parse()
        .context("Invalid tool diameter")?;

    let cutting_length = diameter * 4.0;

    match parts[0] {
        "ball" => Ok(Box::new(BallEndmill::new(diameter, cutting_length))),
        "flat" => Ok(Box::new(FlatEndmill::new(diameter, cutting_length))),
        "bullnose" => {
            // bullnose:diameter:corner_radius
            let corner_radius: f64 = parts
                .get(2)
                .context("Bull nose needs corner radius: bullnose:10:2")?
                .parse()
                .context("Invalid corner radius")?;
            Ok(Box::new(BullNoseEndmill::new(diameter, corner_radius, cutting_length)))
        }
        "vbit" => {
            // vbit:diameter:included_angle_deg
            let angle: f64 = parts
                .get(2)
                .context("V-bit needs included angle: vbit:10:90")?
                .parse()
                .context("Invalid included angle")?;
            Ok(Box::new(VBitEndmill::new(diameter, angle, cutting_length)))
        }
        "tapered_ball" => {
            // tapered_ball:ball_diameter:taper_half_angle:shaft_diameter
            let taper_angle: f64 = parts
                .get(2)
                .context("Tapered ball needs taper angle and shaft diameter: tapered_ball:6:10:12")?
                .parse()
                .context("Invalid taper half-angle")?;
            let shaft_diameter: f64 = parts
                .get(3)
                .context("Tapered ball needs shaft diameter: tapered_ball:6:10:12")?
                .parse()
                .context("Invalid shaft diameter")?;
            Ok(Box::new(TaperedBallEndmill::new(
                diameter, taper_angle, shaft_diameter, cutting_length,
            )))
        }
        _ => bail!(
            "Unknown tool type '{}'. Supported: ball, flat, bullnose, vbit, tapered_ball",
            parts[0]
        ),
    }
}

fn parse_entry_style(entry: &str) -> Result<Option<EntryStyle>> {
    match entry {
        "plunge" => Ok(None),
        "ramp" => Ok(Some(EntryStyle::Ramp { max_angle_deg: 3.0 })),
        "helix" => Ok(Some(EntryStyle::Helix {
            radius: 2.0,
            pitch: 1.0,
        })),
        _ => bail!("Unknown entry style '{}'. Supported: plunge, ramp, helix", entry),
    }
}

fn load_polygons(path: &PathBuf) -> Result<Vec<Polygon2>> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "svg" => {
            let polys = rs_cam_core::svg_input::load_svg(path, 0.1)
                .context("Failed to load SVG")?;
            if polys.is_empty() {
                bail!("No closed paths found in SVG file");
            }
            Ok(polys)
        }
        "dxf" => {
            let polys = rs_cam_core::dxf_input::load_dxf(path, 5.0)
                .context("Failed to load DXF")?;
            if polys.is_empty() {
                bail!("No closed entities found in DXF file");
            }
            Ok(polys)
        }
        _ => bail!("Unsupported input format '{}'. Supported: .svg, .dxf", ext),
    }
}

fn emit_and_write(
    toolpath: &Toolpath,
    post: &str,
    spindle_speed: u32,
    output: &PathBuf,
    svg_path: &Option<PathBuf>,
) -> Result<()> {
    let post_proc = get_post_processor(post)
        .context(format!("Unknown post-processor '{}'. Supported: grbl, linuxcnc", post))?;

    eprintln!("Emitting G-code ({})...", post_proc.name());
    let gcode = emit_gcode(toolpath, post_proc.as_ref(), spindle_speed);

    std::fs::write(output, &gcode)
        .context("Failed to write output file")?;
    eprintln!("Wrote {} bytes to {}", gcode.len(), output.display());

    if let Some(svg_out) = svg_path {
        let svg_content = rs_cam_core::viz::toolpath_to_svg(toolpath, 800.0, 600.0);
        std::fs::write(svg_out, &svg_content)
            .context("Failed to write SVG file")?;
        eprintln!("Wrote SVG preview to {}", svg_out.display());
    }

    Ok(())
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::DropCutter {
            input, units, scale, tool, stepover, feed_rate, plunge_rate,
            spindle_speed, safe_z, min_z, post, output, svg, view,
        } => {
            let scale_factor = match scale {
                Some(s) => s,
                None => match units.to_lowercase().as_str() {
                    "mm" => 1.0,
                    "m" => 1000.0,
                    "cm" => 10.0,
                    "inch" | "in" => 25.4,
                    "ft" | "foot" | "feet" => 304.8,
                    _ => bail!("Unknown unit '{}'. Supported: mm, m, cm, inch, ft", units),
                },
            };
            eprintln!("Loading STL: {} (units: {}, scale: {:.4})", input.display(), units, scale_factor);
            let mesh = TriangleMesh::from_stl_scaled(&input, scale_factor)
                .context("Failed to load STL")?;
            eprintln!("  {} vertices, {} triangles", mesh.vertices.len(), mesh.faces.len());
            eprintln!(
                "  Bounding box: ({:.2}, {:.2}, {:.2}) to ({:.2}, {:.2}, {:.2})",
                mesh.bbox.min.x, mesh.bbox.min.y, mesh.bbox.min.z,
                mesh.bbox.max.x, mesh.bbox.max.y, mesh.bbox.max.z,
            );

            let cutter = parse_tool(&tool)?;
            eprintln!("Tool: {} diameter={:.3}mm", tool, cutter.diameter());

            eprintln!("Building spatial index...");
            let cell_size = cutter.diameter() * 2.0;
            let index = SpatialIndex::build(&mesh, cell_size);

            eprintln!("Running drop-cutter (stepover={:.3}mm)...", stepover);
            let start = std::time::Instant::now();
            let grid = batch_drop_cutter(&mesh, &index, cutter.as_ref(), stepover, 0.0, min_z);
            let elapsed = start.elapsed();
            eprintln!(
                "  {}x{} grid ({} points) in {:.2}s",
                grid.cols, grid.rows, grid.points.len(), elapsed.as_secs_f64()
            );

            let toolpath = raster_toolpath_from_grid(&grid, feed_rate, plunge_rate, safe_z);
            eprintln!(
                "  {} moves, cutting={:.1}mm, rapid={:.1}mm",
                toolpath.moves.len(), toolpath.total_cutting_distance(), toolpath.total_rapid_distance(),
            );

            emit_and_write(&toolpath, &post, spindle_speed, &output, &svg)?;

            if let Some(view_path) = view {
                eprintln!("Generating 3D viewer...");
                let html = rs_cam_core::viz::toolpath_to_3d_html(&mesh, &toolpath);
                std::fs::write(&view_path, &html)
                    .context("Failed to write 3D viewer file")?;
                eprintln!("Wrote 3D viewer to {} ({:.1} MB)", view_path.display(), html.len() as f64 / 1_048_576.0);
            }
        }

        Commands::Pocket {
            input, tool, stepover, depth, depth_per_pass, feed_rate, plunge_rate,
            spindle_speed, safe_z, pattern, angle, climb, entry, post, output, svg, view,
        } => {
            let cutter = parse_tool(&tool)?;
            let tool_radius = cutter.diameter() / 2.0;
            eprintln!("Tool: {} diameter={:.3}mm", tool, cutter.diameter());

            let polygons = load_polygons(&input)?;
            eprintln!("Loaded {} polygon(s) from {}", polygons.len(), input.display());

            let depth_stepping = DepthStepping::new(0.0, -depth, depth_per_pass);
            eprintln!(
                "Depth: {:.1}mm total, {:.1}mm/pass ({} passes)",
                depth, depth_per_pass, depth_stepping.roughing_pass_count()
            );

            let start = std::time::Instant::now();
            let mut toolpath = Toolpath::new();

            for (i, poly) in polygons.iter().enumerate() {
                eprintln!("  Polygon {}: {} vertices, area={:.1}mm²", i, poly.exterior.len(), poly.area());

                let poly_tp = depth_stepped_toolpath(&depth_stepping, safe_z, |z| {
                    match pattern {
                        ClearingPattern::Contour => pocket_toolpath(
                            poly,
                            &PocketParams {
                                tool_radius,
                                stepover,
                                cut_depth: z,
                                feed_rate,
                                plunge_rate,
                                safe_z,
                                climb,
                            },
                        ),
                        ClearingPattern::Zigzag => zigzag_toolpath(
                            poly,
                            &ZigzagParams {
                                tool_radius,
                                stepover,
                                cut_depth: z,
                                feed_rate,
                                plunge_rate,
                                safe_z,
                                angle,
                            },
                        ),
                    }
                });

                toolpath.moves.extend(poly_tp.moves);
            }

            // Apply entry dressup
            if let Some(entry_style) = parse_entry_style(&entry)? {
                eprintln!("Applying {} entry...", entry);
                toolpath = apply_entry(&toolpath, entry_style, plunge_rate);
            }

            let elapsed = start.elapsed();
            eprintln!(
                "Generated {} moves, cutting={:.1}mm, rapid={:.1}mm in {:.2}s",
                toolpath.moves.len(), toolpath.total_cutting_distance(),
                toolpath.total_rapid_distance(), elapsed.as_secs_f64()
            );

            emit_and_write(&toolpath, &post, spindle_speed, &output, &svg)?;

            if let Some(view_path) = view {
                eprintln!("Generating 3D viewer...");
                let html = rs_cam_core::viz::toolpath_standalone_3d_html(&toolpath, None);
                std::fs::write(&view_path, &html)
                    .context("Failed to write 3D viewer file")?;
                eprintln!("Wrote 3D viewer to {}", view_path.display());
            }
        }

        Commands::Profile {
            input, tool, depth, depth_per_pass, side, feed_rate, plunge_rate,
            spindle_speed, safe_z, climb, tabs, tab_width, tab_height,
            entry, post, output, svg, view,
        } => {
            let cutter = parse_tool(&tool)?;
            let tool_radius = cutter.diameter() / 2.0;
            eprintln!("Tool: {} diameter={:.3}mm", tool, cutter.diameter());

            let profile_side = match side.to_lowercase().as_str() {
                "outside" | "out" => ProfileSide::Outside,
                "inside" | "in" => ProfileSide::Inside,
                _ => bail!("Unknown side '{}'. Supported: inside, outside", side),
            };

            let polygons = load_polygons(&input)?;
            eprintln!("Loaded {} polygon(s) from {}", polygons.len(), input.display());

            let depth_stepping = DepthStepping::new(0.0, -depth, depth_per_pass);
            eprintln!(
                "Depth: {:.1}mm total, {:.1}mm/pass ({} passes), side={}",
                depth, depth_per_pass, depth_stepping.roughing_pass_count(), side
            );

            let start = std::time::Instant::now();
            let mut toolpath = Toolpath::new();

            for (i, poly) in polygons.iter().enumerate() {
                eprintln!("  Polygon {}: {} vertices", i, poly.exterior.len());

                let poly_tp = depth_stepped_toolpath(&depth_stepping, safe_z, |z| {
                    profile_toolpath(
                        poly,
                        &ProfileParams {
                            tool_radius,
                            side: profile_side,
                            cut_depth: z,
                            feed_rate,
                            plunge_rate,
                            safe_z,
                            climb,
                        },
                    )
                });

                toolpath.moves.extend(poly_tp.moves);
            }

            // Apply entry dressup
            if let Some(entry_style) = parse_entry_style(&entry)? {
                eprintln!("Applying {} entry...", entry);
                toolpath = apply_entry(&toolpath, entry_style, plunge_rate);
            }

            // Apply tabs (on final depth pass only)
            if tabs > 0 {
                eprintln!("Adding {} tabs ({}mm wide, {}mm high)...", tabs, tab_width, tab_height);
                let tab_list = even_tabs(tabs, tab_width, tab_height);
                toolpath = apply_tabs(&toolpath, &tab_list, -depth);
            }

            let elapsed = start.elapsed();
            eprintln!(
                "Generated {} moves, cutting={:.1}mm, rapid={:.1}mm in {:.2}s",
                toolpath.moves.len(), toolpath.total_cutting_distance(),
                toolpath.total_rapid_distance(), elapsed.as_secs_f64()
            );

            emit_and_write(&toolpath, &post, spindle_speed, &output, &svg)?;

            if let Some(view_path) = view {
                eprintln!("Generating 3D viewer...");
                let html = rs_cam_core::viz::toolpath_standalone_3d_html(&toolpath, None);
                std::fs::write(&view_path, &html)
                    .context("Failed to write 3D viewer file")?;
                eprintln!("Wrote 3D viewer to {}", view_path.display());
            }
        }

        Commands::Waterline {
            input, units, scale, tool, z_step, sampling, start_z, final_z,
            feed_rate, plunge_rate, spindle_speed, safe_z, arc_tolerance,
            post, output, svg, view,
        } => {
            let scale_factor = match scale {
                Some(s) => s,
                None => match units.to_lowercase().as_str() {
                    "mm" => 1.0,
                    "m" => 1000.0,
                    "cm" => 10.0,
                    "inch" | "in" => 25.4,
                    _ => bail!("Unknown unit '{}'. Supported: mm, m, cm, inch", units),
                },
            };

            eprintln!("Loading STL: {} (units: {}, scale: {:.4})", input.display(), units, scale_factor);
            let mesh = TriangleMesh::from_stl_scaled(&input, scale_factor)
                .context("Failed to load STL")?;
            eprintln!("  {} vertices, {} triangles", mesh.vertices.len(), mesh.faces.len());

            let cutter = parse_tool(&tool)?;
            eprintln!("Tool: {} diameter={:.3}mm", tool, cutter.diameter());

            eprintln!("Building spatial index...");
            let cell_size = cutter.diameter() * 2.0;
            let index = SpatialIndex::build(&mesh, cell_size);

            let sz = start_z.unwrap_or(mesh.bbox.max.z);
            let fz = final_z.unwrap_or(mesh.bbox.min.z);
            eprintln!("Waterline: z={:.1} to {:.1}, step={:.1}mm, sampling={:.1}mm", sz, fz, z_step, sampling);

            let params = WaterlineParams {
                sampling,
                feed_rate,
                plunge_rate,
                safe_z,
            };

            let start = std::time::Instant::now();
            let mut toolpath = waterline_toolpath(&mesh, &index, cutter.as_ref(), sz, fz, z_step, &params);
            let elapsed = start.elapsed();

            // Apply arc fitting if requested
            if arc_tolerance > 0.0 {
                let before = toolpath.moves.len();
                toolpath = fit_arcs(&toolpath, arc_tolerance);
                eprintln!("Arc fitting: {} → {} moves (tolerance={:.3}mm)", before, toolpath.moves.len(), arc_tolerance);
            }

            eprintln!(
                "Generated {} moves, cutting={:.1}mm, rapid={:.1}mm in {:.2}s",
                toolpath.moves.len(), toolpath.total_cutting_distance(),
                toolpath.total_rapid_distance(), elapsed.as_secs_f64()
            );

            emit_and_write(&toolpath, &post, spindle_speed, &output, &svg)?;

            if let Some(view_path) = view {
                eprintln!("Generating 3D viewer...");
                let html = rs_cam_core::viz::toolpath_to_3d_html(&mesh, &toolpath);
                std::fs::write(&view_path, &html)
                    .context("Failed to write 3D viewer file")?;
                eprintln!("Wrote 3D viewer to {} ({:.1} MB)", view_path.display(), html.len() as f64 / 1_048_576.0);
            }
        }
    }

    Ok(())
}
