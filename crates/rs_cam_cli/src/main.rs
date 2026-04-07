#![deny(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
#![allow(clippy::print_stderr)] // CLI uses eprintln! for user-facing diagnostic output

mod helpers;
mod job;
#[allow(dead_code)] // Serde structs parse more fields than are actively used
mod project;
mod sweep;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand, ValueEnum};
use rs_cam_core::{
    adaptive::{AdaptiveParams, adaptive_toolpath},
    adaptive3d::{Adaptive3dParams, EntryStyle3d, RegionOrdering, adaptive_3d_toolpath_annotated},
    arcfit::fit_arcs,
    depth::{DepthStepping, depth_stepped_toolpath},
    dexel_stock::{StockCutDirection, TriDexelStock},
    dressup::{
        LinkMoveParams, apply_dogbones, apply_entry, apply_link_moves, apply_tabs, even_tabs,
    },
    dropcutter::batch_drop_cutter,
    gcode::{GcodePhase, emit_gcode, emit_gcode_phased, get_post_processor},
    geo::BoundingBox3,
    inlay::{InlayParams, inlay_toolpaths},
    mesh::{SpatialIndex, TriangleMesh},
    pencil::{PencilParams, pencil_toolpath},
    pocket::{PocketParams, pocket_toolpath},
    profile::{ProfileParams, ProfileSide, profile_toolpath},
    ramp_finish::{CutDirection, RampFinishParams, ramp_finish_toolpath},
    rest::{RestParams, rest_machining_toolpath},
    scallop::{ScallopDirection, ScallopParams, scallop_toolpath},
    simulation_cut::{SimulationCutArtifact, SimulationCutIssueKind, SimulationCutTrace},
    steep_shallow::{SteepShallowParams, steep_shallow_toolpath},
    tool::{
        BallEndmill, BullNoseEndmill, FlatEndmill, MillingCutter, TaperedBallEndmill, VBitEndmill,
    },
    toolpath::{Toolpath, raster_toolpath_from_grid},
    vcarve::{VCarveParams, vcarve_toolpath},
    waterline::{WaterlineParams, waterline_toolpath},
    zigzag::{ZigzagParams, zigzag_toolpath},
};
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// Default holder length (mm) used for collision checks when the holder
/// geometry is not explicitly specified.
const DEFAULT_HOLDER_LENGTH_MM: f64 = 40.0;

#[allow(clippy::too_many_arguments)]
fn run_collision_check(
    toolpath: &rs_cam_core::toolpath::Toolpath,
    cutter: &dyn rs_cam_core::tool::MillingCutter,
    mesh: &rs_cam_core::mesh::TriangleMesh,
    index: &rs_cam_core::mesh::SpatialIndex,
    holder_diameter: f64,
    shank_diameter: f64,
    shank_length: f64,
    stickout: f64,
) {
    if holder_diameter <= 0.0 {
        return;
    }
    let assembly = rs_cam_core::collision::ToolAssembly {
        cutter_radius: cutter.radius(),
        cutter_length: if stickout > 0.0 {
            stickout - shank_length
        } else {
            cutter.length()
        },
        shank_diameter: if shank_diameter > 0.0 {
            shank_diameter
        } else {
            cutter.diameter()
        },
        shank_length,
        holder_diameter,
        holder_length: DEFAULT_HOLDER_LENGTH_MM,
    };
    let report = rs_cam_core::collision::check_collisions_interpolated(
        toolpath, &assembly, mesh, index, 2.0,
    );
    if report.is_clear() {
        info!("Collision check: CLEAR");
    } else {
        eprintln!(
            "WARNING: {} holder/shank collisions detected!",
            report.collisions.len()
        );
        eprintln!(
            "  Min safe stickout: {:.1}mm (current: {:.1}mm)",
            report.min_safe_stickout,
            assembly.stickout()
        );
        for c in report.collisions.iter().take(5) {
            eprintln!(
                "  Move {}: {} at ({:.1}, {:.1}, {:.1}), penetration {:.2}mm",
                c.move_idx,
                c.segment,
                c.position.x,
                c.position.y,
                c.position.z,
                c.penetration_depth
            );
        }
    }
}

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
    /// Run a TOML job file with multiple tools and operations
    Job {
        /// Path to the .toml job file
        input: PathBuf,

        /// Enable cutting metrics analysis (requires simulate = true in job file)
        #[arg(long)]
        diagnostics: bool,

        /// Write diagnostics JSON artifact to this path
        #[arg(long)]
        diagnostics_json: Option<PathBuf>,

        /// Write toolpath debug trace JSON artifacts to this directory
        #[arg(long)]
        debug_trace: Option<PathBuf>,
    },

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

        /// Enable material removal simulation in viewer (requires --view)
        #[arg(long)]
        simulate: bool,

        /// Simulation grid resolution in mm (default 0.25)
        #[arg(long, default_value = "0.25")]
        sim_resolution: f64,

        /// Replace short retracts with direct feed moves (max link distance in mm, 0 to disable)
        #[arg(long, default_value = "0.0")]
        link_moves: f64,

        /// Holder diameter for collision check (mm, 0 to disable)
        #[arg(long, default_value = "0.0")]
        holder_diameter: f64,

        /// Shank diameter (mm)
        #[arg(long, default_value = "0.0")]
        shank_diameter: f64,

        /// Shank length above flutes (mm)
        #[arg(long, default_value = "0.0")]
        shank_length: f64,

        /// Tool stickout length (mm)
        #[arg(long, default_value = "0.0")]
        stickout: f64,
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

        /// Add dogbone overcuts at inside corners
        #[arg(long)]
        dogbone: bool,

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

        /// Enable material removal simulation in viewer (requires --view)
        #[arg(long)]
        simulate: bool,

        /// Simulation grid resolution in mm (default 0.25)
        #[arg(long, default_value = "0.25")]
        sim_resolution: f64,
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

        /// Add dogbone overcuts at inside corners
        #[arg(long)]
        dogbone: bool,

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

        /// Enable material removal simulation in viewer (requires --view)
        #[arg(long)]
        simulate: bool,

        /// Simulation grid resolution in mm (default 0.25)
        #[arg(long, default_value = "0.25")]
        sim_resolution: f64,
    },

    /// Adaptive clearing with constant engagement from SVG or DXF boundary
    Adaptive {
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

        /// Path tolerance in mm
        #[arg(long, default_value = "0.1")]
        tolerance: f64,

        /// Enable slot clearing: cut center slot before adaptive spiral
        #[arg(long)]
        slot_clearing: bool,

        /// Minimum cutting radius: blend sharp corners with arcs (mm, 0=disabled)
        #[arg(long, default_value = "0.0")]
        min_cutting_radius: f64,

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

        /// Enable material removal simulation in viewer (requires --view)
        #[arg(long)]
        simulate: bool,

        /// Simulation grid resolution in mm (default 0.25)
        #[arg(long, default_value = "0.25")]
        sim_resolution: f64,
    },

    /// V-carve engraving from SVG or DXF boundary
    Vcarve {
        /// Input file (SVG or DXF)
        input: PathBuf,

        /// Tool specification: vbit:diameter:included_angle (e.g., vbit:6.35:90)
        #[arg(long)]
        tool: String,

        /// Maximum cut depth in mm (0 = full cone depth)
        #[arg(long, default_value = "0.0")]
        max_depth: f64,

        /// Step-over distance between scan lines in mm
        #[arg(long, default_value = "0.5")]
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

        /// Path tolerance / sampling interval in mm
        #[arg(long, default_value = "0.1")]
        tolerance: f64,

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

        /// Enable material removal simulation in viewer (requires --view)
        #[arg(long)]
        simulate: bool,

        /// Simulation grid resolution in mm (default 0.25)
        #[arg(long, default_value = "0.25")]
        sim_resolution: f64,
    },

    /// Rest machining: clean up corners/channels a larger tool missed
    Rest {
        /// Input file (SVG or DXF)
        input: PathBuf,

        /// Current (smaller) tool specification: type:diameter (e.g., flat:3.175)
        #[arg(long)]
        tool: String,

        /// Previous (larger) tool specification: type:diameter (e.g., flat:6.35)
        #[arg(long)]
        prev_tool: String,

        /// Step-over distance in mm
        #[arg(long, default_value = "1.0")]
        stepover: f64,

        /// Total depth in mm (positive, e.g. 6.0)
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

        /// Scan line angle in degrees
        #[arg(long, default_value = "0.0")]
        angle: f64,

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

        /// Enable material removal simulation in viewer (requires --view)
        #[arg(long)]
        simulate: bool,

        /// Simulation grid resolution in mm (default 0.25)
        #[arg(long, default_value = "0.25")]
        sim_resolution: f64,

        /// Replace short retracts with direct feed moves (max link distance in mm, 0 to disable)
        #[arg(long, default_value = "0.0")]
        link_moves: f64,
    },

    /// 3D adaptive clearing (constant engagement rough machining from STL)
    Adaptive3d {
        /// Input STL file
        input: PathBuf,

        /// STL units: mm (default), m, cm, inch
        #[arg(long, default_value = "mm")]
        units: String,

        /// Custom scale factor (overrides --units if set)
        #[arg(long)]
        scale: Option<f64>,

        /// Tool specification: type:diameter (e.g., flat:6.35)
        #[arg(long)]
        tool: String,

        /// Step-over distance in mm
        #[arg(long, default_value = "2.0")]
        stepover: f64,

        /// Maximum depth per pass in mm
        #[arg(long, default_value = "3.0")]
        depth_per_pass: f64,

        /// Stock top Z (flat stock height, default: mesh max Z + 5)
        #[arg(long)]
        stock_top_z: Option<f64>,

        /// Material to leave above mesh surface (mm)
        #[arg(long, default_value = "0.5")]
        stock_to_leave: f64,

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

        /// Path tolerance in mm
        #[arg(long, default_value = "0.1")]
        tolerance: f64,

        /// Minimum cutting radius (mm, 0=disabled)
        #[arg(long, default_value = "0.0")]
        min_cutting_radius: f64,

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

        /// Entry style: plunge, helix, ramp (default: plunge)
        #[arg(long, default_value = "plunge")]
        entry: String,

        /// Fine stepdown interval in mm (default: disabled)
        #[arg(long)]
        fine_stepdown: Option<f64>,

        /// Detect flat areas in mesh and add Z levels at shelf heights
        #[arg(long)]
        detect_flat_areas: bool,

        /// Maximum stay-down distance between passes in mm (default: tool_radius * 6)
        #[arg(long)]
        max_stay_down_dist: Option<f64>,

        /// Region ordering: global (default) or by-area
        #[arg(long, default_value = "global")]
        order_by: String,

        /// Enable material removal simulation in viewer (requires --view)
        #[arg(long)]
        simulate: bool,

        /// Simulation grid resolution in mm (default 0.25)
        #[arg(long, default_value = "0.25")]
        sim_resolution: f64,
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

        /// Enable material removal simulation in viewer (requires --view)
        #[arg(long)]
        simulate: bool,

        /// Simulation grid resolution in mm (default 0.25)
        #[arg(long, default_value = "0.25")]
        sim_resolution: f64,

        /// Replace short retracts with direct feed moves (max link distance in mm, 0 to disable)
        #[arg(long, default_value = "0.0")]
        link_moves: f64,

        /// Holder diameter for collision check (mm, 0 to disable)
        #[arg(long, default_value = "0.0")]
        holder_diameter: f64,

        /// Shank diameter (mm)
        #[arg(long, default_value = "0.0")]
        shank_diameter: f64,

        /// Shank length above flutes (mm)
        #[arg(long, default_value = "0.0")]
        shank_length: f64,

        /// Tool stickout length (mm)
        #[arg(long, default_value = "0.0")]
        stickout: f64,
    },

    /// Ramp finishing — continuous descent on steep walls (no Z-level witness marks)
    #[command(name = "ramp-finish")]
    RampFinish {
        input: PathBuf,
        #[arg(long, default_value = "mm")]
        units: String,
        #[arg(long)]
        scale: Option<f64>,
        #[arg(long)]
        tool: String,
        /// Maximum Z stepdown per revolution (mm)
        #[arg(long, default_value = "1.0")]
        max_stepdown: f64,
        /// Only machine slopes steeper than this (degrees from horizontal)
        #[arg(long, default_value = "0.0")]
        slope_from: f64,
        /// Only machine slopes shallower than this (degrees from horizontal)
        #[arg(long, default_value = "90.0")]
        slope_to: f64,
        /// Cutting direction: climb, conventional, both
        #[arg(long, default_value = "climb")]
        direction: String,
        /// Order passes bottom-up instead of top-down
        #[arg(long)]
        bottom_up: bool,
        #[arg(long, default_value = "1000.0")]
        feed_rate: f64,
        #[arg(long, default_value = "500.0")]
        plunge_rate: f64,
        #[arg(long, default_value = "18000")]
        spindle_speed: u32,
        #[arg(long, default_value = "10.0")]
        safe_z: f64,
        /// Fiber sampling spacing (mm)
        #[arg(long, default_value = "1.0")]
        sampling: f64,
        #[arg(long, default_value = "0.0")]
        stock_to_leave: f64,
        #[arg(long, default_value = "0.05")]
        tolerance: f64,
        #[arg(long, default_value = "grbl")]
        post: String,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        svg: Option<PathBuf>,
        #[arg(long)]
        view: Option<PathBuf>,
        #[arg(long)]
        simulate: bool,
        #[arg(long, default_value = "0.25")]
        sim_resolution: f64,

        /// Replace short retracts with direct feed moves (max link distance in mm, 0 to disable)
        #[arg(long, default_value = "0.0")]
        link_moves: f64,

        /// Holder diameter for collision check (mm, 0 to disable)
        #[arg(long, default_value = "0.0")]
        holder_diameter: f64,

        /// Shank diameter (mm)
        #[arg(long, default_value = "0.0")]
        shank_diameter: f64,

        /// Shank length above flutes (mm)
        #[arg(long, default_value = "0.0")]
        shank_length: f64,

        /// Tool stickout length (mm)
        #[arg(long, default_value = "0.0")]
        stickout: f64,
    },

    /// Steep and Shallow finishing — hybrid waterline + parallel for mixed terrain
    #[command(name = "steep-shallow")]
    SteepShallow {
        input: PathBuf,
        #[arg(long, default_value = "mm")]
        units: String,
        #[arg(long)]
        scale: Option<f64>,
        #[arg(long)]
        tool: String,
        /// Threshold angle (degrees from horizontal, default 40)
        #[arg(long, default_value = "40.0")]
        threshold_angle: f64,
        /// Overlap distance between steep/shallow regions (mm)
        #[arg(long, default_value = "4.0")]
        overlap_distance: f64,
        /// Shallow passes stay this far from steep walls (mm)
        #[arg(long, default_value = "2.0")]
        wall_clearance: f64,
        /// Machine steep regions before shallow
        #[arg(long)]
        steep_first: bool,
        /// Stepover for parallel passes in shallow regions (mm)
        #[arg(long, default_value = "1.0")]
        stepover: f64,
        /// Z step for waterline passes in steep regions (mm)
        #[arg(long, default_value = "1.0")]
        z_step: f64,
        /// Fiber sampling spacing (mm)
        #[arg(long, default_value = "1.0")]
        sampling: f64,
        #[arg(long, default_value = "0.0")]
        stock_to_leave: f64,
        #[arg(long, default_value = "0.05")]
        tolerance: f64,
        #[arg(long, default_value = "1000.0")]
        feed_rate: f64,
        #[arg(long, default_value = "500.0")]
        plunge_rate: f64,
        #[arg(long, default_value = "18000")]
        spindle_speed: u32,
        #[arg(long, default_value = "10.0")]
        safe_z: f64,
        #[arg(long, default_value = "grbl")]
        post: String,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        svg: Option<PathBuf>,
        #[arg(long)]
        view: Option<PathBuf>,
        #[arg(long)]
        simulate: bool,
        #[arg(long, default_value = "0.25")]
        sim_resolution: f64,

        /// Replace short retracts with direct feed moves (max link distance in mm, 0 to disable)
        #[arg(long, default_value = "0.0")]
        link_moves: f64,

        /// Holder diameter for collision check (mm, 0 to disable)
        #[arg(long, default_value = "0.0")]
        holder_diameter: f64,

        /// Shank diameter (mm)
        #[arg(long, default_value = "0.0")]
        shank_diameter: f64,

        /// Shank length above flutes (mm)
        #[arg(long, default_value = "0.0")]
        shank_length: f64,

        /// Tool stickout length (mm)
        #[arg(long, default_value = "0.0")]
        stickout: f64,
    },

    /// Inlay operations — generate male and female V-carve toolpaths
    #[command(name = "inlay")]
    Inlay {
        input: PathBuf,
        #[arg(long)]
        tool: String,
        /// V-bit half-angle in degrees (e.g. 45 for 90° V-bit)
        #[arg(long, default_value = "45.0")]
        half_angle: f64,
        /// Female pocket depth (mm)
        #[arg(long, default_value = "3.0")]
        pocket_depth: f64,
        /// Glue gap between mating surfaces (mm)
        #[arg(long, default_value = "0.1")]
        glue_gap: f64,
        /// Additional male depth below start surface (mm)
        #[arg(long, default_value = "0.5")]
        flat_depth: f64,
        /// Margin around plug boundary (mm)
        #[arg(long, default_value = "2.0")]
        boundary_offset: f64,
        /// Scan line spacing (mm)
        #[arg(long, default_value = "0.5")]
        stepover: f64,
        /// Tool radius for flat area clearing (0 = skip)
        #[arg(long, default_value = "0.0")]
        flat_tool_radius: f64,
        #[arg(long, default_value = "1000.0")]
        feed_rate: f64,
        #[arg(long, default_value = "500.0")]
        plunge_rate: f64,
        #[arg(long, default_value = "18000")]
        spindle_speed: u32,
        #[arg(long, default_value = "10.0")]
        safe_z: f64,
        #[arg(long, default_value = "grbl")]
        post: String,
        /// Output file for female pocket G-code
        #[arg(short, long)]
        output: PathBuf,
        /// Output file for male plug G-code (defaults to <output>_male.nc)
        #[arg(long)]
        male_output: Option<PathBuf>,
        #[arg(long)]
        svg: Option<PathBuf>,
    },

    /// Pencil finishing — trace concave edges (creases) on mesh surfaces
    #[command(name = "pencil")]
    Pencil {
        input: PathBuf,
        #[arg(long, default_value = "mm")]
        units: String,
        #[arg(long)]
        scale: Option<f64>,
        #[arg(long)]
        tool: String,
        /// Dihedral angle threshold (degrees). Edges below this are creases.
        #[arg(long, default_value = "160.0")]
        bitangency_angle: f64,
        /// Minimum chain length to keep (mm)
        #[arg(long, default_value = "5.0")]
        min_cut_length: f64,
        /// Number of offset passes on each side of centerline (0 = center only)
        #[arg(long, default_value = "0")]
        offset_passes: usize,
        /// Offset stepover between parallel passes (mm)
        #[arg(long, default_value = "1.5")]
        offset_stepover: f64,
        /// Point spacing along paths (mm)
        #[arg(long, default_value = "0.5")]
        sampling: f64,
        #[arg(long, default_value = "0.0")]
        stock_to_leave: f64,
        #[arg(long, default_value = "1000.0")]
        feed_rate: f64,
        #[arg(long, default_value = "500.0")]
        plunge_rate: f64,
        #[arg(long, default_value = "18000")]
        spindle_speed: u32,
        #[arg(long, default_value = "10.0")]
        safe_z: f64,
        #[arg(long, default_value = "grbl")]
        post: String,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        svg: Option<PathBuf>,
        #[arg(long)]
        view: Option<PathBuf>,
        #[arg(long)]
        simulate: bool,
        #[arg(long, default_value = "0.25")]
        sim_resolution: f64,

        /// Replace short retracts with direct feed moves (max link distance in mm, 0 to disable)
        #[arg(long, default_value = "0.0")]
        link_moves: f64,

        /// Holder diameter for collision check (mm, 0 to disable)
        #[arg(long, default_value = "0.0")]
        holder_diameter: f64,

        /// Shank diameter (mm)
        #[arg(long, default_value = "0.0")]
        shank_diameter: f64,

        /// Shank length above flutes (mm)
        #[arg(long, default_value = "0.0")]
        shank_length: f64,

        /// Tool stickout length (mm)
        #[arg(long, default_value = "0.0")]
        stickout: f64,
    },

    /// Scallop finishing — constant scallop height with variable stepover
    #[command(name = "scallop")]
    Scallop {
        input: PathBuf,
        #[arg(long, default_value = "mm")]
        units: String,
        #[arg(long)]
        scale: Option<f64>,
        #[arg(long)]
        tool: String,
        /// Target scallop height (mm) — PRIMARY parameter
        #[arg(long, default_value = "0.01")]
        scallop_height: f64,
        /// Direction: outside-in or inside-out
        #[arg(long, default_value = "outside-in")]
        direction: String,
        /// Connect rings into continuous spiral (fewer retracts)
        #[arg(long)]
        continuous: bool,
        /// Only machine slopes steeper than this (degrees)
        #[arg(long, default_value = "0.0")]
        slope_from: f64,
        /// Only machine slopes shallower than this (degrees)
        #[arg(long, default_value = "90.0")]
        slope_to: f64,
        #[arg(long, default_value = "0.0")]
        stock_to_leave: f64,
        #[arg(long, default_value = "0.05")]
        tolerance: f64,
        #[arg(long, default_value = "1000.0")]
        feed_rate: f64,
        #[arg(long, default_value = "500.0")]
        plunge_rate: f64,
        #[arg(long, default_value = "18000")]
        spindle_speed: u32,
        #[arg(long, default_value = "10.0")]
        safe_z: f64,
        #[arg(long, default_value = "grbl")]
        post: String,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        svg: Option<PathBuf>,
        #[arg(long)]
        view: Option<PathBuf>,
        #[arg(long)]
        simulate: bool,
        #[arg(long, default_value = "0.25")]
        sim_resolution: f64,

        /// Replace short retracts with direct feed moves (max link distance in mm, 0 to disable)
        #[arg(long, default_value = "0.0")]
        link_moves: f64,

        /// Holder diameter for collision check (mm, 0 to disable)
        #[arg(long, default_value = "0.0")]
        holder_diameter: f64,

        /// Shank diameter (mm)
        #[arg(long, default_value = "0.0")]
        shank_diameter: f64,

        /// Shank length above flutes (mm)
        #[arg(long, default_value = "0.0")]
        shank_length: f64,

        /// Tool stickout length (mm)
        #[arg(long, default_value = "0.0")]
        stickout: f64,
    },

    /// Run a parameter sweep on a TOML job file
    ///
    /// Varies one parameter across multiple values, running the full job pipeline
    /// for each, and produces JSON fingerprints, diffs, SVGs, and G-code.
    Sweep {
        /// Base TOML job file to sweep over
        input: PathBuf,

        /// Parameter name to vary (e.g. stepover, depth, feed_rate)
        #[arg(long)]
        param: String,

        /// Comma-separated values to sweep (e.g. "0.5,1.0,2.0,4.0")
        #[arg(long)]
        values: String,

        /// Output directory for sweep results
        #[arg(long)]
        output_dir: PathBuf,

        /// Run simulation and produce stock heightmap SVGs
        #[arg(long)]
        simulate: bool,
    },

    /// Analyze a GUI project file with full diagnostics
    ///
    /// Loads the GUI project TOML format (format_version=3), executes all
    /// enabled toolpaths through the core algorithms with full debug and
    /// semantic tracing, runs tri-dexel simulation with cut metrics, checks
    /// collisions, and writes structured JSON diagnostics.
    Project {
        /// Path to the project .toml file (GUI format, format_version=3)
        input: PathBuf,

        /// Output directory for diagnostic artifacts
        #[arg(long, default_value = "diagnostics")]
        output_dir: PathBuf,

        /// Run only this setup (by name or ID)
        #[arg(long)]
        setup: Option<String>,

        /// Skip these toolpath IDs (comma-separated)
        #[arg(long)]
        skip: Option<String>,

        /// Simulation resolution in mm
        #[arg(long, default_value = "0.5")]
        resolution: f64,

        /// Print human-readable summary to stderr
        #[arg(long)]
        summary: bool,
    },
}

// SAFETY: `parts.len() < 2` guard above ensures indices 0 and 1 are in-bounds.
#[allow(clippy::indexing_slicing)]
fn parse_tool(spec: &str) -> Result<Box<dyn MillingCutter>> {
    let parts: Vec<&str> = spec.split(':').collect();
    if parts.len() < 2 {
        bail!(
            "Tool spec must be type:diameter[:params] (e.g., ball:6.35, \
             bullnose:10:2, vbit:10:90, tapered_ball:6:10:12)"
        );
    }

    let diameter: f64 = parts[1].parse().context("Invalid tool diameter")?;

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
            Ok(Box::new(BullNoseEndmill::new(
                diameter,
                corner_radius,
                cutting_length,
            )))
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
                diameter,
                taper_angle,
                shaft_diameter,
                cutting_length,
            )))
        }
        _ => bail!(
            "Unknown tool type '{}'. Supported: ball, flat, bullnose, vbit, tapered_ball",
            parts[0]
        ),
    }
}

/// Parse STL scale factor from --scale override or --units string.
fn parse_scale_factor(scale: Option<f64>, units: &str) -> Result<f64> {
    match scale {
        Some(s) => Ok(s),
        None => match units.to_lowercase().as_str() {
            "mm" => Ok(1.0),
            "m" => Ok(1000.0),
            "cm" => Ok(10.0),
            "inch" | "in" => Ok(25.4),
            "ft" | "foot" | "feet" => Ok(304.8),
            _ => bail!("Unknown unit '{}'. Supported: mm, m, cm, inch, ft", units),
        },
    }
}

/// Load an STL mesh with scale and build a spatial index for a given cutter.
fn load_stl_with_index(
    path: &Path,
    scale: f64,
    cutter: &dyn MillingCutter,
) -> Result<(TriangleMesh, SpatialIndex)> {
    let mesh = TriangleMesh::from_stl_scaled(path, scale).context("Failed to load STL")?;
    debug!(
        vertices = mesh.vertices.len(),
        triangles = mesh.faces.len(),
        "Loaded mesh"
    );
    debug!(
        min_x = mesh.bbox.min.x,
        min_y = mesh.bbox.min.y,
        min_z = mesh.bbox.min.z,
        max_x = mesh.bbox.max.x,
        max_y = mesh.bbox.max.y,
        max_z = mesh.bbox.max.z,
        "Bounding box"
    );
    debug!("Building spatial index...");
    let cell_size = cutter.diameter() * 2.0;
    let index = SpatialIndex::build(&mesh, cell_size);
    Ok((mesh, index))
}

/// Write an optional 3D HTML viewer for mesh-based (3D) operations.
fn write_3d_view(
    view: &Option<PathBuf>,
    tp: &Toolpath,
    mesh: &TriangleMesh,
    cutter: &dyn MillingCutter,
    simulate: bool,
    sim_res: f64,
    stock_top: f64,
) -> Result<()> {
    if let Some(view_path) = view {
        let html = if simulate {
            debug!(resolution_mm = sim_res, "Running simulation");
            let mut stock = TriDexelStock::from_stock(
                mesh.bbox.min.x - cutter.radius(),
                mesh.bbox.min.y - cutter.radius(),
                mesh.bbox.max.x + cutter.radius(),
                mesh.bbox.max.y + cutter.radius(),
                mesh.bbox.min.z,
                stock_top,
                sim_res,
            );
            stock.simulate_toolpath(tp, cutter, StockCutDirection::FromTop);
            debug!(
                cols = stock.z_grid.cols,
                rows = stock.z_grid.rows,
                "Simulation stock generated"
            );
            rs_cam_core::viz::simulation_3d_html(&stock, tp, Some(mesh), cutter, &[])
        } else {
            rs_cam_core::viz::toolpath_to_3d_html(mesh, tp)
        };
        std::fs::write(view_path, &html).context("Failed to write 3D viewer file")?;
        info!(path = %view_path.display(), size_mb = html.len() as f64 / 1_048_576.0, "Wrote 3D viewer");
    }
    Ok(())
}

/// Write an optional 3D HTML viewer for 2.5D operations (no source mesh).
fn write_2d_view(
    view: &Option<PathBuf>,
    tp: &Toolpath,
    cutter: &dyn MillingCutter,
    simulate: bool,
    sim_res: f64,
) -> Result<()> {
    if let Some(view_path) = view {
        let html = if simulate {
            debug!(resolution_mm = sim_res, "Running simulation");
            let tp_bbox = toolpath_bbox(tp);
            let margin = cutter.radius();
            let sim_bbox = BoundingBox3 {
                min: rs_cam_core::geo::P3::new(
                    tp_bbox.min.x - margin,
                    tp_bbox.min.y - margin,
                    tp_bbox.min.z,
                ),
                max: rs_cam_core::geo::P3::new(tp_bbox.max.x + margin, tp_bbox.max.y + margin, 0.0),
            };
            let mut stock = TriDexelStock::from_bounds(&sim_bbox, sim_res);
            stock.simulate_toolpath(tp, cutter, StockCutDirection::FromTop);
            debug!(
                cols = stock.z_grid.cols,
                rows = stock.z_grid.rows,
                "Simulation stock generated"
            );
            rs_cam_core::viz::simulation_3d_html(&stock, tp, None, cutter, &[])
        } else {
            rs_cam_core::viz::toolpath_standalone_3d_html(tp, None)
        };
        std::fs::write(view_path, &html).context("Failed to write 3D viewer file")?;
        info!(path = %view_path.display(), "Wrote 3D viewer");
    }
    Ok(())
}

/// Print a human-readable simulation diagnostics report to stderr.
fn print_diagnostics_report(trace: &SimulationCutTrace, toolpath_labels: &[String]) {
    eprintln!();
    eprintln!("=== Simulation Diagnostics ===");
    eprintln!();

    for ts in &trace.toolpath_summaries {
        let label = toolpath_labels
            .get(ts.toolpath_id)
            .map(|s| s.as_str())
            .unwrap_or("unknown");
        let air_runtime = trace.summary.total_runtime_s - ts.cutting_runtime_s - ts.rapid_runtime_s;
        let air_pct = if ts.total_runtime_s > 1e-9 {
            ts.air_cut_time_s / ts.total_runtime_s * 100.0
        } else {
            0.0
        };

        eprintln!("Toolpath: {}", label);
        eprintln!(
            "  Runtime: {:.1}s (cutting: {:.1}s, rapid: {:.1}s, air: {:.1}s)",
            ts.total_runtime_s,
            ts.cutting_runtime_s,
            ts.rapid_runtime_s,
            air_runtime.max(0.0),
        );
        eprintln!("  Air cut: {:.1}% of runtime", air_pct);
        eprintln!("  Avg engagement: {:.2}", ts.average_engagement);
        eprintln!(
            "  Peak chipload: {:.3} mm/tooth",
            ts.peak_chipload_mm_per_tooth
        );
        eprintln!("  Peak DOC: {:.1} mm", ts.peak_axial_doc_mm);
        eprintln!("  MRR avg: {:.1} mm3/s", ts.average_mrr_mm3_s);

        // Count issues for this toolpath
        let air_issues = trace
            .issues
            .iter()
            .filter(|i| i.toolpath_id == ts.toolpath_id && i.kind == SimulationCutIssueKind::AirCut)
            .count();
        let low_eng_issues = trace
            .issues
            .iter()
            .filter(|i| {
                i.toolpath_id == ts.toolpath_id && i.kind == SimulationCutIssueKind::LowEngagement
            })
            .count();
        if air_issues > 0 || low_eng_issues > 0 {
            let mut parts = Vec::new();
            if air_issues > 0 {
                parts.push(format!("{} air cuts", air_issues));
            }
            if low_eng_issues > 0 {
                parts.push(format!("{} low engagement", low_eng_issues));
            }
            eprintln!("  Issues: {}", parts.join(", "));
        }
        eprintln!();
    }

    // Top hotspots by wasted time
    if !trace.hotspots.is_empty() {
        eprintln!("Top issues by wasted time:");
        for (i, hs) in trace.hotspots.iter().take(10).enumerate() {
            let kind_label = if hs.air_cut_time_s > hs.low_engagement_time_s {
                "AirCut"
            } else {
                "LowEngagement"
            };
            let [x, y, z] = hs.representative_position;
            eprintln!(
                "  {}. {} at ({:.1}, {:.1}, {:.1}) -- {:.1}s wasted, engagement {:.2}",
                i + 1,
                kind_label,
                x,
                y,
                z,
                hs.wasted_runtime_s,
                hs.average_engagement,
            );
        }
        eprintln!();
    }
}

fn toolpath_bbox(toolpath: &Toolpath) -> BoundingBox3 {
    let mut bbox = BoundingBox3::empty();
    for m in &toolpath.moves {
        bbox.expand_to(m.target);
    }
    bbox
}

fn emit_and_write(
    toolpath: &Toolpath,
    post: &str,
    spindle_speed: u32,
    output: &PathBuf,
    svg_path: &Option<PathBuf>,
) -> Result<()> {
    let post_proc = get_post_processor(post).context(format!(
        "Unknown post-processor '{}'. Supported: grbl, linuxcnc",
        post
    ))?;

    info!("Emitting G-code ({})...", post_proc.name());
    let gcode = emit_gcode(toolpath, post_proc.as_ref(), spindle_speed);

    std::fs::write(output, &gcode).context("Failed to write output file")?;
    info!(bytes = gcode.len(), path = %output.display(), "Wrote G-code");

    if let Some(svg_out) = svg_path {
        let svg_content = rs_cam_core::viz::toolpath_to_svg(toolpath, 800.0, 600.0);
        std::fs::write(svg_out, &svg_content).context("Failed to write SVG file")?;
        info!(path = %svg_out.display(), "Wrote SVG preview");
    }

    Ok(())
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_timer(tracing_subscriber::fmt::time::uptime())
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Job {
            input,
            diagnostics,
            diagnostics_json,
            debug_trace,
        } => {
            let job_path = input
                .canonicalize()
                .context(format!("Job file not found: {}", input.display()))?;
            let job_dir = job_path.parent().unwrap_or(Path::new("."));
            debug!(path = %job_path.display(), "Loading job file");

            let mut job_file = job::parse_job_file(&job_path)?;
            // CLI flags override TOML config
            if diagnostics {
                job_file.job.diagnostics = true;
            }
            if diagnostics_json.is_some() {
                job_file.job.diagnostics_json = diagnostics_json;
            }
            let debug_trace_dir =
                debug_trace.map(|p| if p.is_absolute() { p } else { job_dir.join(p) });

            info!(
                tools = job_file.tools.len(),
                operations = job_file.operation.len(),
                output = %job_file.job.output.display(),
                "Job loaded"
            );

            let job_result = job::execute_job(&job_file, job_dir, debug_trace_dir.is_some())?;
            let toolpath = &job_result.combined;

            // Export debug trace artifacts if requested
            if let Some(ref trace_dir) = debug_trace_dir {
                for (idx, artifact) in job_result.trace_artifacts.iter().enumerate() {
                    let file_stem = format!(
                        "{}-{}",
                        idx,
                        artifact.toolpath_name.replace(
                            |c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '_',
                            "_",
                        )
                    );
                    match rs_cam_core::semantic_trace::write_toolpath_trace_artifact(
                        trace_dir, &file_stem, artifact,
                    ) {
                        Ok(path) => info!(path = %path.display(), "Wrote debug trace artifact"),
                        Err(e) => {
                            eprintln!(
                                "Warning: failed to write trace artifact for op {}: {}",
                                idx, e
                            );
                        }
                    }
                }
            }

            info!(
                moves = toolpath.moves.len(),
                cutting_mm = format!("{:.1}", toolpath.total_cutting_distance()),
                rapid_mm = format!("{:.1}", toolpath.total_rapid_distance()),
                "Total toolpath"
            );

            let output = if job_file.job.output.is_absolute() {
                job_file.job.output.clone()
            } else {
                job_dir.join(&job_file.job.output)
            };
            let svg = job_file.job.svg.as_ref().map(|p| {
                if p.is_absolute() {
                    p.clone()
                } else {
                    job_dir.join(p)
                }
            });

            // Emit G-code with per-operation spindle speed support
            let post_proc = get_post_processor(&job_file.job.post).context(format!(
                "Unknown post-processor '{}'. Supported: grbl, linuxcnc, mach3",
                job_file.job.post
            ))?;
            if !job_file.setup.is_empty() {
                for setup_def in &job_file.setup {
                    let setup_phases: Vec<GcodePhase<'_>> = job_result
                        .phases
                        .iter()
                        .filter(|phase| phase.setup_name.as_deref() == Some(&setup_def.name))
                        .map(|phase| GcodePhase {
                            toolpath: &phase.toolpath,
                            spindle_rpm: phase.spindle_speed,
                            label: &phase.label,
                            pre_gcode: None,
                            post_gcode: None,
                            tool_number: phase.tool_number,
                            coolant: phase.coolant,
                        })
                        .collect();
                    if setup_phases.is_empty() {
                        continue;
                    }

                    let setup_output = setup_def
                        .output
                        .as_ref()
                        .map(|path| {
                            if path.is_absolute() {
                                path.clone()
                            } else {
                                job_dir.join(path)
                            }
                        })
                        .unwrap_or_else(|| {
                            let name = setup_def.name.replace(' ', "_").to_lowercase();
                            output.with_file_name(format!(
                                "{}_{}.nc",
                                output.file_stem().unwrap_or_default().to_string_lossy(),
                                name
                            ))
                        });

                    let gcode = emit_gcode_phased(&setup_phases, post_proc.as_ref());
                    std::fs::write(&setup_output, &gcode)
                        .context("Failed to write setup output file")?;
                    info!(
                        setup = %setup_def.name,
                        bytes = gcode.len(),
                        path = %setup_output.display(),
                        "Wrote setup G-code"
                    );
                }
            } else {
                let phases: Vec<GcodePhase<'_>> = job_result
                    .phases
                    .iter()
                    .map(|phase| GcodePhase {
                        toolpath: &phase.toolpath,
                        spindle_rpm: phase.spindle_speed,
                        label: &phase.label,
                        pre_gcode: None,
                        post_gcode: None,
                        tool_number: phase.tool_number,
                        coolant: phase.coolant,
                    })
                    .collect();
                info!("Emitting G-code ({})...", post_proc.name());
                let gcode = emit_gcode_phased(&phases, post_proc.as_ref());
                std::fs::write(&output, &gcode).context("Failed to write output file")?;
                info!(bytes = gcode.len(), path = %output.display(), "Wrote G-code");
            }

            if let Some(svg_out) = &svg {
                let svg_content = rs_cam_core::viz::toolpath_to_svg(toolpath, 800.0, 600.0);
                std::fs::write(svg_out, &svg_content).context("Failed to write SVG file")?;
                info!(path = %svg_out.display(), "Wrote SVG preview");
            }

            if let Some(view) = &job_file.job.view {
                let view_path = if view.is_absolute() {
                    view.clone()
                } else {
                    job_dir.join(view)
                };
                let html = if job_file.job.simulate {
                    // Stacked simulation: each operation is a phase with its own cutter
                    let tp_bbox = toolpath_bbox(toolpath);
                    let max_margin = job_result
                        .phases
                        .iter()
                        .map(|p| p.cutter.radius())
                        .fold(0.0_f64, f64::max);
                    // Determine stock top: use stock_top_z from first 3D op, or bbox max + 5
                    let stock_top = job_file
                        .operation
                        .iter()
                        .find_map(|op| op.stock_top_z)
                        .unwrap_or(tp_bbox.max.z + 5.0);
                    let sim_bbox = BoundingBox3 {
                        min: rs_cam_core::geo::P3::new(
                            tp_bbox.min.x - max_margin,
                            tp_bbox.min.y - max_margin,
                            tp_bbox.min.z,
                        ),
                        max: rs_cam_core::geo::P3::new(
                            tp_bbox.max.x + max_margin,
                            tp_bbox.max.y + max_margin,
                            stock_top,
                        ),
                    };
                    let mut stock =
                        TriDexelStock::from_bounds(&sim_bbox, job_file.job.sim_resolution);

                    debug!(
                        phases = job_result.phases.len(),
                        resolution_mm = job_file.job.sim_resolution,
                        "Running stacked simulation"
                    );

                    // Simulate each phase with its own cutter
                    for phase in &job_result.phases {
                        stock.simulate_toolpath(
                            &phase.toolpath,
                            &phase.cutter,
                            StockCutDirection::FromTop,
                        );
                    }
                    debug!(
                        cols = stock.z_grid.cols,
                        rows = stock.z_grid.rows,
                        phases = job_result.phases.len(),
                        "Simulation stock generated"
                    );

                    // Try to load source mesh for overlay (from first STL-based operation)
                    let source_mesh = job_file.operation.iter().find_map(|op| {
                        let p = if op.input.is_absolute() {
                            op.input.clone()
                        } else {
                            job_dir.join(&op.input)
                        };
                        let ext = p.extension()?.to_str()?.to_lowercase();
                        if ext == "stl" {
                            match rs_cam_core::mesh::TriangleMesh::from_stl_scaled(&p, 1.0) {
                                Ok(m) => Some(m),
                                Err(e) => {
                                    warn!("Failed to load overlay mesh {}: {e}", p.display());
                                    None
                                }
                            }
                        } else {
                            None
                        }
                    });

                    use rs_cam_core::viz::SimPhase;
                    let sim_phases: Vec<SimPhase> = job_result
                        .phases
                        .iter()
                        .map(|p| SimPhase {
                            toolpath: &p.toolpath,
                            cutter: &p.cutter,
                            label: p.label.clone(),
                        })
                        .collect();

                    rs_cam_core::viz::stacked_simulation_3d_html(
                        &sim_phases,
                        &stock,
                        source_mesh.as_ref(),
                    )
                } else {
                    rs_cam_core::viz::toolpath_standalone_3d_html(toolpath, None)
                };
                std::fs::write(&view_path, &html).context("Failed to write 3D viewer file")?;
                info!(path = %view_path.display(), "Wrote 3D viewer");
            }

            // Diagnostics: run metric simulation and print report
            if job_file.job.diagnostics && job_file.job.simulate {
                let tp_bbox = toolpath_bbox(toolpath);
                let max_margin = job_result
                    .phases
                    .iter()
                    .map(|p| p.cutter.radius())
                    .fold(0.0_f64, f64::max);
                let stock_top = job_file
                    .operation
                    .iter()
                    .find_map(|op| op.stock_top_z)
                    .unwrap_or(tp_bbox.max.z + 5.0);
                let sim_bbox = BoundingBox3 {
                    min: rs_cam_core::geo::P3::new(
                        tp_bbox.min.x - max_margin,
                        tp_bbox.min.y - max_margin,
                        tp_bbox.min.z,
                    ),
                    max: rs_cam_core::geo::P3::new(
                        tp_bbox.max.x + max_margin,
                        tp_bbox.max.y + max_margin,
                        stock_top,
                    ),
                };
                let resolution = job_file.job.sim_resolution;
                let sample_step = resolution;
                let never_cancel = || false;
                let mut diag_stock = TriDexelStock::from_bounds(&sim_bbox, resolution);

                debug!("Running diagnostics metric simulation");

                let mut all_samples = Vec::new();
                let mut labels = Vec::new();
                for (idx, phase) in job_result.phases.iter().enumerate() {
                    labels.push(phase.label.clone());
                    let rapid_feed = 3000.0_f64; // typical rapid rate for diagnostics
                    match diag_stock.simulate_toolpath_with_metrics_with_cancel(
                        &phase.toolpath,
                        &phase.cutter,
                        StockCutDirection::FromTop,
                        idx,
                        phase.spindle_speed,
                        phase.flute_count,
                        rapid_feed,
                        sample_step,
                        None,
                        &never_cancel,
                    ) {
                        Ok(samples) => all_samples.extend(samples),
                        Err(_) => {
                            eprintln!("Diagnostics simulation cancelled for phase {}", idx);
                        }
                    }
                }

                let trace = SimulationCutTrace::from_samples(sample_step, all_samples);
                print_diagnostics_report(&trace, &labels);

                if let Some(json_path) = &job_file.job.diagnostics_json {
                    let json_out = if json_path.is_absolute() {
                        json_path.clone()
                    } else {
                        job_dir.join(json_path)
                    };
                    let artifact = SimulationCutArtifact::new(
                        resolution,
                        sample_step,
                        [sim_bbox.min.x, sim_bbox.min.y, sim_bbox.min.z],
                        [sim_bbox.max.x, sim_bbox.max.y, sim_bbox.max.z],
                        (0..job_result.phases.len()).collect(),
                        serde_json::json!({"source": "cli_diagnostics"}),
                        trace,
                    );
                    let json = serde_json::to_string_pretty(&artifact)
                        .context("Failed to serialize diagnostics artifact")?;
                    std::fs::write(&json_out, &json).context("Failed to write diagnostics JSON")?;
                    info!(path = %json_out.display(), "Wrote diagnostics JSON");
                }
            } else if job_file.job.diagnostics && !job_file.job.simulate {
                eprintln!("Warning: --diagnostics requires simulate = true in the job file");
            }
        }

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
            simulate,
            sim_resolution,
            link_moves,
            holder_diameter,
            shank_diameter,
            shank_length,
            stickout,
        } => {
            let scale_factor = parse_scale_factor(scale, &units)?;
            debug!(path = %input.display(), units = %units, scale = scale_factor, "Loading STL");

            let cutter = parse_tool(&tool)?;
            debug!(tool = %tool, diameter_mm = cutter.diameter(), "Tool");

            let (mesh, index) = load_stl_with_index(&input, scale_factor, cutter.as_ref())?;

            debug!(stepover_mm = stepover, "Running drop-cutter");
            let start = std::time::Instant::now();
            let grid = batch_drop_cutter(&mesh, &index, cutter.as_ref(), stepover, 0.0, min_z);
            let elapsed = start.elapsed();
            debug!(
                cols = grid.cols,
                rows = grid.rows,
                points = grid.points.len(),
                elapsed_secs = format!("{:.2}", elapsed.as_secs_f64()),
                "Drop-cutter grid"
            );

            let mut toolpath = raster_toolpath_from_grid(&grid, feed_rate, plunge_rate, safe_z);
            info!(
                moves = toolpath.moves.len(),
                cutting_mm = format!("{:.1}", toolpath.total_cutting_distance()),
                rapid_mm = format!("{:.1}", toolpath.total_rapid_distance()),
                "Generated toolpath"
            );

            if link_moves > 0.0 {
                let link_params = LinkMoveParams {
                    max_link_distance: link_moves,
                    link_feed_rate: feed_rate,
                    safe_z_threshold: safe_z,
                };
                let before_rapid = toolpath.total_rapid_distance();
                toolpath = apply_link_moves(toolpath, &link_params);
                info!(
                    before_rapid_mm = format!("{:.1}", before_rapid),
                    after_rapid_mm = format!("{:.1}", toolpath.total_rapid_distance()),
                    "Applied link moves"
                );
            }

            run_collision_check(
                &toolpath,
                cutter.as_ref(),
                &mesh,
                &index,
                holder_diameter,
                shank_diameter,
                shank_length,
                stickout,
            );

            emit_and_write(&toolpath, &post, spindle_speed, &output, &svg)?;

            write_3d_view(
                &view,
                &toolpath,
                &mesh,
                cutter.as_ref(),
                simulate,
                sim_resolution,
                mesh.bbox.max.z,
            )?;
        }

        Commands::Pocket {
            input,
            tool,
            stepover,
            depth,
            depth_per_pass,
            feed_rate,
            plunge_rate,
            spindle_speed,
            safe_z,
            pattern,
            angle,
            climb,
            dogbone,
            entry,
            post,
            output,
            svg,
            view,
            simulate,
            sim_resolution,
        } => {
            let cutter = parse_tool(&tool)?;
            let tool_radius = cutter.diameter() / 2.0;
            debug!(tool = %tool, diameter_mm = cutter.diameter(), "Tool");

            let polygons = helpers::load_polygons(&input)?;
            debug!(count = polygons.len(), path = %input.display(), "Loaded polygons");

            let depth_stepping = DepthStepping::new(0.0, -depth, depth_per_pass);
            debug!(
                total_mm = depth,
                per_pass_mm = depth_per_pass,
                passes = depth_stepping.roughing_pass_count(),
                "Depth stepping"
            );

            let start = std::time::Instant::now();
            let mut toolpath = Toolpath::new();

            for (i, poly) in polygons.iter().enumerate() {
                debug!(
                    index = i,
                    vertices = poly.exterior.len(),
                    area_mm2 = format!("{:.1}", poly.area()),
                    "Polygon"
                );

                let poly_tp = depth_stepped_toolpath(&depth_stepping, safe_z, |z| match pattern {
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
                });

                toolpath.moves.extend(poly_tp.moves);
            }

            // Apply entry dressup
            if let Some(entry_style) = helpers::parse_entry_style(&entry)? {
                debug!("Applying {} entry...", entry);
                toolpath = apply_entry(toolpath, entry_style, plunge_rate);
            }

            // Apply dogbone dressup
            if dogbone {
                debug!("Applying dogbone overcuts...");
                toolpath = apply_dogbones(toolpath, tool_radius, 170.0);
            }

            let elapsed = start.elapsed();
            info!(
                moves = toolpath.moves.len(),
                cutting_mm = format!("{:.1}", toolpath.total_cutting_distance()),
                rapid_mm = format!("{:.1}", toolpath.total_rapid_distance()),
                elapsed_secs = format!("{:.2}", elapsed.as_secs_f64()),
                "Generated toolpath"
            );

            emit_and_write(&toolpath, &post, spindle_speed, &output, &svg)?;

            write_2d_view(&view, &toolpath, cutter.as_ref(), simulate, sim_resolution)?;
        }

        Commands::Profile {
            input,
            tool,
            depth,
            depth_per_pass,
            side,
            feed_rate,
            plunge_rate,
            spindle_speed,
            safe_z,
            climb,
            dogbone,
            tabs,
            tab_width,
            tab_height,
            entry,
            post,
            output,
            svg,
            view,
            simulate,
            sim_resolution,
        } => {
            let cutter = parse_tool(&tool)?;
            let tool_radius = cutter.diameter() / 2.0;
            debug!(tool = %tool, diameter_mm = cutter.diameter(), "Tool");

            let profile_side = match side.to_lowercase().as_str() {
                "outside" | "out" => ProfileSide::Outside,
                "inside" | "in" => ProfileSide::Inside,
                _ => bail!("Unknown side '{}'. Supported: inside, outside", side),
            };

            let polygons = helpers::load_polygons(&input)?;
            debug!(count = polygons.len(), path = %input.display(), "Loaded polygons");

            let depth_stepping = DepthStepping::new(0.0, -depth, depth_per_pass);
            debug!(
                total_mm = depth, per_pass_mm = depth_per_pass,
                passes = depth_stepping.roughing_pass_count(), side = %side,
                "Depth stepping"
            );

            let start = std::time::Instant::now();
            let mut toolpath = Toolpath::new();

            for (i, poly) in polygons.iter().enumerate() {
                debug!(index = i, vertices = poly.exterior.len(), "Polygon");

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
            if let Some(entry_style) = helpers::parse_entry_style(&entry)? {
                debug!("Applying {} entry...", entry);
                toolpath = apply_entry(toolpath, entry_style, plunge_rate);
            }

            // Apply tabs (on final depth pass only)
            if tabs > 0 {
                debug!(
                    count = tabs,
                    width_mm = tab_width,
                    height_mm = tab_height,
                    "Adding tabs"
                );
                let tab_list = even_tabs(tabs, tab_width, tab_height);
                toolpath = apply_tabs(toolpath, &tab_list, -depth);
            }

            // Apply dogbone dressup
            if dogbone {
                debug!("Applying dogbone overcuts...");
                toolpath = apply_dogbones(toolpath, tool_radius, 170.0);
            }

            let elapsed = start.elapsed();
            info!(
                moves = toolpath.moves.len(),
                cutting_mm = format!("{:.1}", toolpath.total_cutting_distance()),
                rapid_mm = format!("{:.1}", toolpath.total_rapid_distance()),
                elapsed_secs = format!("{:.2}", elapsed.as_secs_f64()),
                "Generated toolpath"
            );

            emit_and_write(&toolpath, &post, spindle_speed, &output, &svg)?;

            write_2d_view(&view, &toolpath, cutter.as_ref(), simulate, sim_resolution)?;
        }

        Commands::Adaptive {
            input,
            tool,
            stepover,
            depth,
            depth_per_pass,
            feed_rate,
            plunge_rate,
            spindle_speed,
            safe_z,
            tolerance,
            slot_clearing,
            min_cutting_radius,
            post,
            output,
            svg,
            view,
            simulate,
            sim_resolution,
        } => {
            let cutter = parse_tool(&tool)?;
            let tool_radius = cutter.diameter() / 2.0;
            debug!(tool = %tool, diameter_mm = cutter.diameter(), "Tool");

            let polygons = helpers::load_polygons(&input)?;
            debug!(count = polygons.len(), path = %input.display(), "Loaded polygons");

            let depth_stepping = DepthStepping::new(0.0, -depth, depth_per_pass);
            debug!(
                total_mm = depth,
                per_pass_mm = depth_per_pass,
                passes = depth_stepping.roughing_pass_count(),
                "Depth stepping"
            );

            let start = std::time::Instant::now();
            let mut toolpath = Toolpath::new();

            for (i, poly) in polygons.iter().enumerate() {
                debug!(
                    index = i,
                    vertices = poly.exterior.len(),
                    area_mm2 = format!("{:.1}", poly.area()),
                    "Polygon"
                );

                let poly_tp = depth_stepped_toolpath(&depth_stepping, safe_z, |z| {
                    adaptive_toolpath(
                        poly,
                        &AdaptiveParams {
                            tool_radius,
                            stepover,
                            cut_depth: z,
                            feed_rate,
                            plunge_rate,
                            safe_z,
                            tolerance,
                            slot_clearing,
                            min_cutting_radius,
                            initial_stock: None,
                        },
                    )
                });

                toolpath.moves.extend(poly_tp.moves);
            }

            let elapsed = start.elapsed();
            info!(
                moves = toolpath.moves.len(),
                cutting_mm = format!("{:.1}", toolpath.total_cutting_distance()),
                rapid_mm = format!("{:.1}", toolpath.total_rapid_distance()),
                elapsed_secs = format!("{:.2}", elapsed.as_secs_f64()),
                "Generated toolpath"
            );

            emit_and_write(&toolpath, &post, spindle_speed, &output, &svg)?;

            write_2d_view(&view, &toolpath, cutter.as_ref(), simulate, sim_resolution)?;
        }

        Commands::Vcarve {
            input,
            tool,
            max_depth,
            stepover,
            feed_rate,
            plunge_rate,
            spindle_speed,
            safe_z,
            tolerance,
            post,
            output,
            svg,
            view,
            simulate,
            sim_resolution,
        } => {
            let cutter = parse_tool(&tool)?;
            let tool_radius = cutter.diameter() / 2.0;

            // Extract half-angle from the tool spec string
            let tool_parts: Vec<&str> = tool.split(':').collect();
            if tool_parts.first() != Some(&"vbit") || tool_parts.len() < 3 {
                bail!("V-carve requires a V-bit tool (e.g., --tool vbit:6.35:90)");
            }
            let included_angle_deg: f64 = tool_parts
                .get(2)
                .context("V-carve requires a V-bit tool (e.g., --tool vbit:6.35:90)")?
                .parse()
                .context("Invalid V-bit angle")?;
            let half_angle = (included_angle_deg / 2.0).to_radians();

            debug!(tool = %tool, diameter_mm = cutter.diameter(), half_angle_deg = half_angle.to_degrees(), "Tool");

            let polygons = helpers::load_polygons(&input)?;
            debug!(count = polygons.len(), path = %input.display(), "Loaded polygons");

            let effective_max_depth = if max_depth > 0.0 {
                max_depth
            } else {
                tool_radius / half_angle.tan()
            };
            debug!(
                max_depth_mm = effective_max_depth,
                stepover_mm = stepover,
                "V-carve params"
            );

            let start = std::time::Instant::now();
            let mut toolpath = Toolpath::new();

            for (i, poly) in polygons.iter().enumerate() {
                debug!(index = i, vertices = poly.exterior.len(), "Polygon");

                let poly_tp = vcarve_toolpath(
                    poly,
                    &VCarveParams {
                        half_angle,
                        max_depth: effective_max_depth,
                        stepover,
                        feed_rate,
                        plunge_rate,
                        safe_z,
                        tolerance,
                    },
                );

                toolpath.moves.extend(poly_tp.moves);
            }

            let elapsed = start.elapsed();
            info!(
                moves = toolpath.moves.len(),
                cutting_mm = format!("{:.1}", toolpath.total_cutting_distance()),
                rapid_mm = format!("{:.1}", toolpath.total_rapid_distance()),
                elapsed_secs = format!("{:.2}", elapsed.as_secs_f64()),
                "Generated toolpath"
            );

            emit_and_write(&toolpath, &post, spindle_speed, &output, &svg)?;

            write_2d_view(&view, &toolpath, cutter.as_ref(), simulate, sim_resolution)?;
        }

        Commands::Rest {
            input,
            tool,
            prev_tool,
            stepover,
            depth,
            depth_per_pass,
            feed_rate,
            plunge_rate,
            spindle_speed,
            safe_z,
            angle,
            post,
            output,
            svg,
            view,
            simulate,
            sim_resolution,
            link_moves,
        } => {
            let cutter = parse_tool(&tool)?;
            let tool_radius = cutter.diameter() / 2.0;
            let prev_cutter = parse_tool(&prev_tool)?;
            let prev_tool_radius = prev_cutter.diameter() / 2.0;

            if tool_radius >= prev_tool_radius {
                bail!(
                    "Rest machining tool ({:.2}mm) must be smaller than previous tool ({:.2}mm)",
                    cutter.diameter(),
                    prev_cutter.diameter()
                );
            }

            debug!(tool = %tool, diameter_mm = cutter.diameter(), "Tool");
            debug!(prev_tool = %prev_tool, diameter_mm = prev_cutter.diameter(), "Previous tool");

            let polygons = helpers::load_polygons(&input)?;
            debug!(count = polygons.len(), path = %input.display(), "Loaded polygons");

            let depth_stepping = DepthStepping::new(0.0, -depth, depth_per_pass);
            debug!(
                total_mm = depth,
                per_pass_mm = depth_per_pass,
                passes = depth_stepping.roughing_pass_count(),
                "Depth stepping"
            );

            let start = std::time::Instant::now();
            let mut toolpath = Toolpath::new();

            for (i, poly) in polygons.iter().enumerate() {
                debug!(
                    index = i,
                    vertices = poly.exterior.len(),
                    area_mm2 = format!("{:.1}", poly.area()),
                    "Polygon"
                );

                let poly_tp = depth_stepped_toolpath(&depth_stepping, safe_z, |z| {
                    rest_machining_toolpath(
                        poly,
                        &RestParams {
                            prev_tool_radius,
                            tool_radius,
                            cut_depth: z,
                            stepover,
                            feed_rate,
                            plunge_rate,
                            safe_z,
                            angle,
                        },
                    )
                });

                toolpath.moves.extend(poly_tp.moves);
            }

            let elapsed = start.elapsed();
            info!(
                moves = toolpath.moves.len(),
                cutting_mm = format!("{:.1}", toolpath.total_cutting_distance()),
                rapid_mm = format!("{:.1}", toolpath.total_rapid_distance()),
                elapsed_secs = format!("{:.2}", elapsed.as_secs_f64()),
                "Generated toolpath"
            );

            if link_moves > 0.0 {
                let link_params = LinkMoveParams {
                    max_link_distance: link_moves,
                    link_feed_rate: feed_rate,
                    safe_z_threshold: safe_z,
                };
                let before_rapid = toolpath.total_rapid_distance();
                toolpath = apply_link_moves(toolpath, &link_params);
                info!(
                    before_rapid_mm = format!("{:.1}", before_rapid),
                    after_rapid_mm = format!("{:.1}", toolpath.total_rapid_distance()),
                    "Applied link moves"
                );
            }

            emit_and_write(&toolpath, &post, spindle_speed, &output, &svg)?;

            if let Some(view_path) = view {
                let html = if simulate {
                    debug!(resolution_mm = sim_resolution, "Running stacked simulation");
                    let tp_bbox = toolpath_bbox(&toolpath);
                    let margin = prev_cutter.radius();
                    let sim_bbox = BoundingBox3 {
                        min: rs_cam_core::geo::P3::new(
                            tp_bbox.min.x - margin,
                            tp_bbox.min.y - margin,
                            tp_bbox.min.z,
                        ),
                        max: rs_cam_core::geo::P3::new(
                            tp_bbox.max.x + margin,
                            tp_bbox.max.y + margin,
                            0.0,
                        ),
                    };

                    // Generate the previous (large) tool's pocket toolpath
                    let mut prev_toolpath = Toolpath::new();
                    for poly in &polygons {
                        let prev_tp = depth_stepped_toolpath(&depth_stepping, safe_z, |z| {
                            zigzag_toolpath(
                                poly,
                                &ZigzagParams {
                                    tool_radius: prev_tool_radius,
                                    stepover: prev_cutter.diameter() * 0.4,
                                    cut_depth: z,
                                    feed_rate,
                                    plunge_rate,
                                    safe_z,
                                    angle: 0.0,
                                },
                            )
                        });
                        prev_toolpath.moves.extend(prev_tp.moves);
                    }

                    // Simulate both into the stock for final state
                    let mut stock = TriDexelStock::from_bounds(&sim_bbox, sim_resolution);
                    stock.simulate_toolpath(
                        &prev_toolpath,
                        prev_cutter.as_ref(),
                        StockCutDirection::FromTop,
                    );
                    stock.simulate_toolpath(&toolpath, cutter.as_ref(), StockCutDirection::FromTop);
                    debug!(
                        cols = stock.z_grid.cols,
                        rows = stock.z_grid.rows,
                        phases = 2,
                        "Simulation stock generated"
                    );

                    // Stacked viewer: animates roughing then rest
                    use rs_cam_core::viz::SimPhase;
                    let phases = vec![
                        SimPhase {
                            toolpath: &prev_toolpath,
                            cutter: prev_cutter.as_ref(),
                            label: format!(
                                "Roughing ({:.2}mm {})",
                                prev_cutter.diameter(),
                                prev_tool
                            ),
                        },
                        SimPhase {
                            toolpath: &toolpath,
                            cutter: cutter.as_ref(),
                            label: format!("Rest ({:.2}mm {})", cutter.diameter(), tool),
                        },
                    ];
                    rs_cam_core::viz::stacked_simulation_3d_html(&phases, &stock, None)
                } else {
                    rs_cam_core::viz::toolpath_standalone_3d_html(&toolpath, None)
                };
                std::fs::write(&view_path, &html).context("Failed to write 3D viewer file")?;
                info!(path = %view_path.display(), "Wrote 3D viewer");
            }
        }

        Commands::Adaptive3d {
            input,
            units,
            scale,
            tool,
            stepover,
            depth_per_pass,
            stock_top_z,
            stock_to_leave,
            feed_rate,
            plunge_rate,
            spindle_speed,
            safe_z,
            tolerance,
            min_cutting_radius,
            entry,
            fine_stepdown,
            detect_flat_areas,
            max_stay_down_dist,
            order_by,
            post,
            output,
            svg,
            view,
            simulate,
            sim_resolution,
        } => {
            let scale_factor = parse_scale_factor(scale, &units)?;
            debug!(path = %input.display(), units = %units, scale = scale_factor, "Loading STL");

            let cutter = parse_tool(&tool)?;
            debug!(tool = %tool, diameter_mm = cutter.diameter(), "Tool");

            let (mesh, index) = load_stl_with_index(&input, scale_factor, cutter.as_ref())?;

            let stock_z = stock_top_z.unwrap_or(mesh.bbox.max.z + 5.0);
            debug!(
                stock_top = stock_z,
                depth_per_pass = depth_per_pass,
                stock_to_leave = stock_to_leave,
                stepover = stepover,
                "3D Adaptive params"
            );

            let entry_3d = match entry.to_lowercase().as_str() {
                "helix" => EntryStyle3d::Helix {
                    radius: cutter.radius() * 0.8,
                    pitch: 1.0,
                },
                "ramp" => EntryStyle3d::Ramp { max_angle_deg: 3.0 },
                _ => EntryStyle3d::Plunge,
            };

            let region_ord = match order_by.to_lowercase().as_str() {
                "by-area" | "by_area" | "byarea" => RegionOrdering::ByArea,
                _ => RegionOrdering::Global,
            };

            let params = Adaptive3dParams {
                tool_radius: cutter.radius(),
                stepover,
                depth_per_pass,
                stock_to_leave,
                feed_rate,
                plunge_rate,
                safe_z,
                tolerance,
                min_cutting_radius,
                stock_top_z: stock_z,
                entry_style: entry_3d,
                fine_stepdown,
                detect_flat_areas,
                max_stay_down_dist,
                region_ordering: region_ord,
                initial_stock: None,
                clearing_strategy: rs_cam_core::adaptive3d::ClearingStrategy3d::AgentSearch,
                z_blend: false,
            };

            let start = std::time::Instant::now();
            let (toolpath, annotations) =
                adaptive_3d_toolpath_annotated(&mesh, &index, cutter.as_ref(), &params);
            let elapsed = start.elapsed();

            info!(
                moves = toolpath.moves.len(),
                cutting_mm = format!("{:.1}", toolpath.total_cutting_distance()),
                rapid_mm = format!("{:.1}", toolpath.total_rapid_distance()),
                elapsed_secs = format!("{:.2}", elapsed.as_secs_f64()),
                "Generated toolpath"
            );

            emit_and_write(&toolpath, &post, spindle_speed, &output, &svg)?;

            // Use annotated viewer for adaptive3d
            if let Some(view_path) = &view {
                let html = if simulate {
                    debug!(resolution_mm = sim_resolution, "Running simulation");
                    let mut stock = TriDexelStock::from_stock(
                        mesh.bbox.min.x - cutter.radius(),
                        mesh.bbox.min.y - cutter.radius(),
                        mesh.bbox.max.x + cutter.radius(),
                        mesh.bbox.max.y + cutter.radius(),
                        mesh.bbox.min.z,
                        stock_z,
                        sim_resolution,
                    );
                    stock.simulate_toolpath(&toolpath, cutter.as_ref(), StockCutDirection::FromTop);
                    rs_cam_core::viz::simulation_3d_html(
                        &stock,
                        &toolpath,
                        Some(&mesh),
                        cutter.as_ref(),
                        &annotations,
                    )
                } else {
                    rs_cam_core::viz::toolpath_to_3d_html(&mesh, &toolpath)
                };
                std::fs::write(view_path, &html).context("Failed to write 3D viewer file")?;
                info!(path = %view_path.display(), size_mb = html.len() as f64 / 1_048_576.0, "Wrote 3D viewer");
            }
        }

        Commands::Waterline {
            input,
            units,
            scale,
            tool,
            z_step,
            sampling,
            start_z,
            final_z,
            feed_rate,
            plunge_rate,
            spindle_speed,
            safe_z,
            arc_tolerance,
            post,
            output,
            svg,
            view,
            simulate,
            sim_resolution,
            link_moves,
            holder_diameter,
            shank_diameter,
            shank_length,
            stickout,
        } => {
            let scale_factor = parse_scale_factor(scale, &units)?;
            debug!(path = %input.display(), units = %units, scale = scale_factor, "Loading STL");

            let cutter = parse_tool(&tool)?;
            debug!(tool = %tool, diameter_mm = cutter.diameter(), "Tool");

            let (mesh, index) = load_stl_with_index(&input, scale_factor, cutter.as_ref())?;

            let sz = start_z.unwrap_or(mesh.bbox.max.z);
            let fz = final_z.unwrap_or(mesh.bbox.min.z);
            debug!(
                start_z = sz,
                final_z = fz,
                z_step = z_step,
                sampling = sampling,
                "Waterline params"
            );

            let params = WaterlineParams {
                sampling,
                feed_rate,
                plunge_rate,
                safe_z,
            };

            let start = std::time::Instant::now();
            let mut toolpath =
                waterline_toolpath(&mesh, &index, cutter.as_ref(), sz, fz, z_step, &params);
            let elapsed = start.elapsed();

            // Apply arc fitting if requested
            if arc_tolerance > 0.0 {
                let before = toolpath.moves.len();
                toolpath = fit_arcs(&toolpath, arc_tolerance);
                debug!(
                    before = before,
                    after = toolpath.moves.len(),
                    tolerance_mm = arc_tolerance,
                    "Arc fitting"
                );
            }

            if link_moves > 0.0 {
                let link_params = LinkMoveParams {
                    max_link_distance: link_moves,
                    link_feed_rate: feed_rate,
                    safe_z_threshold: safe_z,
                };
                let before_rapid = toolpath.total_rapid_distance();
                toolpath = apply_link_moves(toolpath, &link_params);
                info!(
                    before_rapid_mm = format!("{:.1}", before_rapid),
                    after_rapid_mm = format!("{:.1}", toolpath.total_rapid_distance()),
                    "Applied link moves"
                );
            }

            info!(
                moves = toolpath.moves.len(),
                cutting_mm = format!("{:.1}", toolpath.total_cutting_distance()),
                rapid_mm = format!("{:.1}", toolpath.total_rapid_distance()),
                elapsed_secs = format!("{:.2}", elapsed.as_secs_f64()),
                "Generated toolpath"
            );

            run_collision_check(
                &toolpath,
                cutter.as_ref(),
                &mesh,
                &index,
                holder_diameter,
                shank_diameter,
                shank_length,
                stickout,
            );

            emit_and_write(&toolpath, &post, spindle_speed, &output, &svg)?;

            write_3d_view(
                &view,
                &toolpath,
                &mesh,
                cutter.as_ref(),
                simulate,
                sim_resolution,
                mesh.bbox.max.z,
            )?;
        }

        Commands::RampFinish {
            input,
            units,
            scale,
            tool,
            max_stepdown,
            slope_from,
            slope_to,
            direction,
            bottom_up,
            feed_rate,
            plunge_rate,
            spindle_speed,
            safe_z,
            sampling,
            stock_to_leave,
            tolerance,
            post,
            output,
            svg,
            view,
            simulate,
            sim_resolution,
            link_moves,
            holder_diameter,
            shank_diameter,
            shank_length,
            stickout,
        } => {
            let scale_factor = parse_scale_factor(scale, &units)?;
            let cutter = parse_tool(&tool)?;
            let (mesh, index) = load_stl_with_index(&input, scale_factor, cutter.as_ref())?;

            let dir = match direction.as_str() {
                "conventional" => CutDirection::Conventional,
                "both" => CutDirection::BothWays,
                _ => CutDirection::Climb,
            };

            let params = RampFinishParams {
                max_stepdown,
                slope_from,
                slope_to,
                direction: dir,
                order_bottom_up: bottom_up,
                feed_rate,
                plunge_rate,
                safe_z,
                sampling,
                stock_to_leave,
                tolerance,
            };

            let start = std::time::Instant::now();
            let mut toolpath = ramp_finish_toolpath(&mesh, &index, cutter.as_ref(), &params);
            info!(
                moves = toolpath.moves.len(),
                cutting_mm = format!("{:.1}", toolpath.total_cutting_distance()),
                rapid_mm = format!("{:.1}", toolpath.total_rapid_distance()),
                elapsed_secs = format!("{:.2}", start.elapsed().as_secs_f64()),
                "Generated ramp finish toolpath"
            );

            if link_moves > 0.0 {
                let link_params = LinkMoveParams {
                    max_link_distance: link_moves,
                    link_feed_rate: feed_rate,
                    safe_z_threshold: safe_z,
                };
                let before_rapid = toolpath.total_rapid_distance();
                toolpath = apply_link_moves(toolpath, &link_params);
                info!(
                    before_rapid_mm = format!("{:.1}", before_rapid),
                    after_rapid_mm = format!("{:.1}", toolpath.total_rapid_distance()),
                    "Applied link moves"
                );
            }

            run_collision_check(
                &toolpath,
                cutter.as_ref(),
                &mesh,
                &index,
                holder_diameter,
                shank_diameter,
                shank_length,
                stickout,
            );

            emit_and_write(&toolpath, &post, spindle_speed, &output, &svg)?;
            write_3d_view(
                &view,
                &toolpath,
                &mesh,
                cutter.as_ref(),
                simulate,
                sim_resolution,
                mesh.bbox.max.z,
            )?;
        }

        Commands::SteepShallow {
            input,
            units,
            scale,
            tool,
            threshold_angle,
            overlap_distance,
            wall_clearance,
            steep_first,
            stepover,
            z_step,
            sampling,
            stock_to_leave,
            tolerance,
            feed_rate,
            plunge_rate,
            spindle_speed,
            safe_z,
            post,
            output,
            svg,
            view,
            simulate,
            sim_resolution,
            link_moves,
            holder_diameter,
            shank_diameter,
            shank_length,
            stickout,
        } => {
            let scale_factor = parse_scale_factor(scale, &units)?;
            let cutter = parse_tool(&tool)?;
            let (mesh, index) = load_stl_with_index(&input, scale_factor, cutter.as_ref())?;

            let params = SteepShallowParams {
                threshold_angle,
                overlap_distance,
                wall_clearance,
                steep_first,
                stepover,
                z_step,
                feed_rate,
                plunge_rate,
                safe_z,
                sampling,
                stock_to_leave,
                tolerance,
            };

            let start = std::time::Instant::now();
            let mut toolpath = steep_shallow_toolpath(&mesh, &index, cutter.as_ref(), &params);
            info!(
                moves = toolpath.moves.len(),
                cutting_mm = format!("{:.1}", toolpath.total_cutting_distance()),
                rapid_mm = format!("{:.1}", toolpath.total_rapid_distance()),
                elapsed_secs = format!("{:.2}", start.elapsed().as_secs_f64()),
                "Generated steep & shallow toolpath"
            );

            if link_moves > 0.0 {
                let link_params = LinkMoveParams {
                    max_link_distance: link_moves,
                    link_feed_rate: feed_rate,
                    safe_z_threshold: safe_z,
                };
                let before_rapid = toolpath.total_rapid_distance();
                toolpath = apply_link_moves(toolpath, &link_params);
                info!(
                    before_rapid_mm = format!("{:.1}", before_rapid),
                    after_rapid_mm = format!("{:.1}", toolpath.total_rapid_distance()),
                    "Applied link moves"
                );
            }

            run_collision_check(
                &toolpath,
                cutter.as_ref(),
                &mesh,
                &index,
                holder_diameter,
                shank_diameter,
                shank_length,
                stickout,
            );

            emit_and_write(&toolpath, &post, spindle_speed, &output, &svg)?;
            write_3d_view(
                &view,
                &toolpath,
                &mesh,
                cutter.as_ref(),
                simulate,
                sim_resolution,
                mesh.bbox.max.z,
            )?;
        }

        Commands::Inlay {
            input,
            tool,
            half_angle,
            pocket_depth,
            glue_gap,
            flat_depth,
            boundary_offset,
            stepover,
            flat_tool_radius,
            feed_rate,
            plunge_rate,
            spindle_speed,
            safe_z,
            post,
            output,
            male_output,
            svg,
        } => {
            let polygons = helpers::load_polygons(&input)?;
            let _cutter = parse_tool(&tool)?;

            let params = InlayParams {
                half_angle: half_angle.to_radians(),
                pocket_depth,
                glue_gap,
                flat_depth,
                boundary_offset,
                stepover,
                flat_tool_radius,
                feed_rate,
                plunge_rate,
                safe_z,
                tolerance: 0.1,
            };

            for (i, poly) in polygons.iter().enumerate() {
                let result = inlay_toolpaths(poly, &params);

                info!(
                    polygon = i,
                    female_moves = result.female.moves.len(),
                    male_moves = result.male.moves.len(),
                    "Inlay toolpaths generated"
                );

                // Write female pocket
                emit_and_write(&result.female, &post, spindle_speed, &output, &svg)?;

                // Write male plug
                let male_path = male_output.clone().unwrap_or_else(|| {
                    let stem = output.file_stem().unwrap_or_default().to_string_lossy();
                    let ext = output.extension().unwrap_or_default().to_string_lossy();
                    output.with_file_name(format!("{}_male.{}", stem, ext))
                });
                emit_and_write(&result.male, &post, spindle_speed, &male_path, &None)?;
                info!(
                    female = %output.display(),
                    male = %male_path.display(),
                    "Wrote inlay G-code files"
                );
            }
        }

        Commands::Pencil {
            input,
            units,
            scale,
            tool,
            bitangency_angle,
            min_cut_length,
            offset_passes,
            offset_stepover,
            sampling,
            stock_to_leave,
            feed_rate,
            plunge_rate,
            spindle_speed,
            safe_z,
            post,
            output,
            svg,
            view,
            simulate,
            sim_resolution,
            link_moves,
            holder_diameter,
            shank_diameter,
            shank_length,
            stickout,
        } => {
            let scale_factor = parse_scale_factor(scale, &units)?;
            let cutter = parse_tool(&tool)?;
            let (mesh, index) = load_stl_with_index(&input, scale_factor, cutter.as_ref())?;

            let params = PencilParams {
                bitangency_angle,
                min_cut_length,
                hookup_distance: cutter.diameter() * 3.0,
                num_offset_passes: offset_passes,
                offset_stepover,
                sampling,
                feed_rate,
                plunge_rate,
                safe_z,
                stock_to_leave,
            };

            let start = std::time::Instant::now();
            let mut toolpath = pencil_toolpath(&mesh, &index, cutter.as_ref(), &params);
            info!(
                moves = toolpath.moves.len(),
                cutting_mm = format!("{:.1}", toolpath.total_cutting_distance()),
                rapid_mm = format!("{:.1}", toolpath.total_rapid_distance()),
                elapsed_secs = format!("{:.2}", start.elapsed().as_secs_f64()),
                "Generated pencil toolpath"
            );

            if link_moves > 0.0 {
                let link_params = LinkMoveParams {
                    max_link_distance: link_moves,
                    link_feed_rate: feed_rate,
                    safe_z_threshold: safe_z,
                };
                let before_rapid = toolpath.total_rapid_distance();
                toolpath = apply_link_moves(toolpath, &link_params);
                info!(
                    before_rapid_mm = format!("{:.1}", before_rapid),
                    after_rapid_mm = format!("{:.1}", toolpath.total_rapid_distance()),
                    "Applied link moves"
                );
            }

            run_collision_check(
                &toolpath,
                cutter.as_ref(),
                &mesh,
                &index,
                holder_diameter,
                shank_diameter,
                shank_length,
                stickout,
            );

            emit_and_write(&toolpath, &post, spindle_speed, &output, &svg)?;
            write_3d_view(
                &view,
                &toolpath,
                &mesh,
                cutter.as_ref(),
                simulate,
                sim_resolution,
                mesh.bbox.max.z,
            )?;
        }

        Commands::Scallop {
            input,
            units,
            scale,
            tool,
            scallop_height,
            direction,
            continuous,
            slope_from,
            slope_to,
            stock_to_leave,
            tolerance,
            feed_rate,
            plunge_rate,
            spindle_speed,
            safe_z,
            post,
            output,
            svg,
            view,
            simulate,
            sim_resolution,
            link_moves,
            holder_diameter,
            shank_diameter,
            shank_length,
            stickout,
        } => {
            let scale_factor = parse_scale_factor(scale, &units)?;
            let cutter = parse_tool(&tool)?;
            let (mesh, index) = load_stl_with_index(&input, scale_factor, cutter.as_ref())?;

            let dir = match direction.as_str() {
                "inside-out" => ScallopDirection::InsideOut,
                _ => ScallopDirection::OutsideIn,
            };

            let params = ScallopParams {
                scallop_height,
                tolerance,
                direction: dir,
                continuous,
                slope_from,
                slope_to,
                feed_rate,
                plunge_rate,
                safe_z,
                stock_to_leave,
            };

            let start = std::time::Instant::now();
            let mut toolpath = scallop_toolpath(&mesh, &index, cutter.as_ref(), &params);
            info!(
                moves = toolpath.moves.len(),
                cutting_mm = format!("{:.1}", toolpath.total_cutting_distance()),
                rapid_mm = format!("{:.1}", toolpath.total_rapid_distance()),
                elapsed_secs = format!("{:.2}", start.elapsed().as_secs_f64()),
                "Generated scallop toolpath"
            );

            if link_moves > 0.0 {
                let link_params = LinkMoveParams {
                    max_link_distance: link_moves,
                    link_feed_rate: feed_rate,
                    safe_z_threshold: safe_z,
                };
                let before_rapid = toolpath.total_rapid_distance();
                toolpath = apply_link_moves(toolpath, &link_params);
                info!(
                    before_rapid_mm = format!("{:.1}", before_rapid),
                    after_rapid_mm = format!("{:.1}", toolpath.total_rapid_distance()),
                    "Applied link moves"
                );
            }

            run_collision_check(
                &toolpath,
                cutter.as_ref(),
                &mesh,
                &index,
                holder_diameter,
                shank_diameter,
                shank_length,
                stickout,
            );

            emit_and_write(&toolpath, &post, spindle_speed, &output, &svg)?;
            write_3d_view(
                &view,
                &toolpath,
                &mesh,
                cutter.as_ref(),
                simulate,
                sim_resolution,
                mesh.bbox.max.z,
            )?;
        }

        Commands::Sweep {
            input,
            param,
            values,
            output_dir,
            simulate,
        } => {
            let job_path = input
                .canonicalize()
                .context(format!("Job file not found: {}", input.display()))?;
            sweep::run_sweep(&job_path, &param, &values, &output_dir, simulate)?;
        }

        Commands::Project {
            input,
            output_dir,
            setup,
            skip,
            resolution,
            summary,
        } => {
            let skip_ids: Vec<usize> = skip
                .as_deref()
                .unwrap_or("")
                .split(',')
                .filter(|s| !s.is_empty())
                .filter_map(|s| s.trim().parse().ok())
                .collect();
            project::run_project_command(
                &input,
                &output_dir,
                setup.as_deref(),
                &skip_ids,
                resolution,
                summary,
            )?;
        }
    }

    Ok(())
}
