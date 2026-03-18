use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use rs_cam_core::{
    dropcutter::batch_drop_cutter,
    gcode::{emit_gcode, get_post_processor},
    mesh::{SpatialIndex, TriangleMesh},
    tool::{BallEndmill, FlatEndmill, MillingCutter},
    toolpath::raster_toolpath_from_grid,
};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "rs_cam", about = "3-axis wood router CAM toolpath generator")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
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
}

fn parse_tool(spec: &str) -> Result<Box<dyn MillingCutter>> {
    let parts: Vec<&str> = spec.split(':').collect();
    if parts.len() != 2 {
        bail!("Tool spec must be type:diameter (e.g., ball:6.35)");
    }

    let diameter: f64 = parts[1]
        .parse()
        .context("Invalid tool diameter")?;

    let cutting_length = diameter * 4.0; // reasonable default

    match parts[0] {
        "ball" => Ok(Box::new(BallEndmill::new(diameter, cutting_length))),
        "flat" => Ok(Box::new(FlatEndmill::new(diameter, cutting_length))),
        _ => bail!("Unknown tool type '{}'. Supported: ball, flat", parts[0]),
    }
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
            input,
            units,
            scale,
            tool,
            stepover,
            feed_rate,
            plunge_rate,
            spindle_speed,
            safe_z,
            min_z,
            post,
            output,
            svg,
            view,
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
            eprintln!(
                "  {} vertices, {} triangles",
                mesh.vertices.len(),
                mesh.faces.len()
            );
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
                grid.cols,
                grid.rows,
                grid.points.len(),
                elapsed.as_secs_f64()
            );

            eprintln!("Generating toolpath...");
            let toolpath = raster_toolpath_from_grid(&grid, feed_rate, plunge_rate, safe_z);
            eprintln!(
                "  {} moves, cutting={:.1}mm, rapid={:.1}mm",
                toolpath.moves.len(),
                toolpath.total_cutting_distance(),
                toolpath.total_rapid_distance(),
            );

            let post_proc = get_post_processor(&post)
                .context(format!("Unknown post-processor '{}'. Supported: grbl, linuxcnc", post))?;

            eprintln!("Emitting G-code ({})...", post_proc.name());
            let gcode = emit_gcode(&toolpath, post_proc.as_ref(), spindle_speed);

            std::fs::write(&output, &gcode)
                .context("Failed to write output file")?;
            eprintln!("Wrote {} bytes to {}", gcode.len(), output.display());

            if let Some(svg_path) = svg {
                let svg_content = rs_cam_core::viz::toolpath_to_svg(&toolpath, 800.0, 600.0);
                std::fs::write(&svg_path, &svg_content)
                    .context("Failed to write SVG file")?;
                eprintln!("Wrote SVG preview to {}", svg_path.display());
            }

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
