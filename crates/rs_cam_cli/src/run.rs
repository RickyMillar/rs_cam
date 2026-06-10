//! Generic registry-driven single-operation runner (T9, plan §7.1).
//!
//! ONE subcommand replaces the ~14 hand-rolled per-op clap subcommands:
//! the operation's settable parameters come from the registry's
//! `ParamDef` table (`--list-params`), values are applied through the
//! same serde round-trip the GUI/MCP use (`set_toolpath_param`), and
//! execution routes through `ProjectSession::generate_toolpath` →
//! `execute_operation_annotated` — the same single execution path as
//! every other production surface. Operation #24 appears here with
//! ZERO new CLI code.

use anyhow::{Context, Result, bail};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use tracing::info;

use rs_cam_core::compute::ModelUnits;
use rs_cam_core::compute::catalog::{OpCategory, OperationConfig, OperationType};
use rs_cam_core::compute::config::{BoundaryConfig, DressupConfig, HeightsConfig, StockSource};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::session::{LoadedModel, ProjectSession, ToolpathConfig};

/// CLI arguments for `run`, mirrored from the clap variant in `main.rs`.
#[allow(clippy::struct_excessive_bools)]
pub struct RunArgs {
    pub op: Option<String>,
    pub list_ops: bool,
    pub list_params: bool,
    pub input: Option<PathBuf>,
    pub units: String,
    pub tool: Option<String>,
    pub tool_set: Vec<String>,
    pub set: Vec<String>,
    pub output: Option<PathBuf>,
    pub svg: Option<PathBuf>,
    pub post: String,
    pub safe_z: f64,
    pub spindle_speed: u32,
}

pub fn run_generic(args: &RunArgs) -> Result<()> {
    if args.list_ops {
        print_ops();
        return Ok(());
    }

    let Some(op_token) = args.op.as_deref() else {
        bail!("Missing operation. Usage: run <op> … (see `run --list-ops`)");
    };
    let op_type = parse_op_type(op_token)?;

    if args.list_params {
        print_params(op_type);
        return Ok(());
    }

    // ── Validate inputs ────────────────────────────────────────────
    let input = args
        .input
        .as_deref()
        .context("Missing --input <model file>")?;
    let output = args
        .output
        .as_deref()
        .context("Missing --output <gcode file>")?;
    let tool_spec = args.tool.as_deref().context(
        "Missing --tool <type:diameter> (e.g. --tool end_mill:6.35; \
         types: end_mill, ball_nose, bull_nose, v_bit, tapered_ball_nose)",
    )?;
    let units = parse_units(&args.units)?;

    // ── Assemble the session (same path as GUI/MCP/project) ──────
    let mut session = ProjectSession::new_empty();

    let base_dir = std::env::current_dir().context("cannot resolve current directory")?;
    let model_name = input
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "model".to_owned());
    let model = LoadedModel::from_file(0, &model_name, input, None, Some(units), &base_dir)
        .with_context(|| format!("loading model '{}'", input.display()))?;
    session.add_model(model);

    let (tool_type, diameter) = parse_tool_spec(tool_spec)?;
    let mut tool = ToolConfig::new_default(ToolId(0), tool_type);
    tool.name = format!("{} {:.3}mm", tool_type.label(), diameter);
    tool.diameter = diameter;
    let tool_idx = session.add_tool(tool);
    for kv in &args.tool_set {
        let (key, value) = split_kv(kv)?;
        session
            .set_tool_param(tool_idx, key, &parse_json_value(value))
            .map_err(|e| anyhow::anyhow!("--tool-set {key}: {e}"))?;
    }

    let tool_id = session
        .tools()
        .get(tool_idx)
        .context("tool index out of bounds after add_tool")?
        .id
        .0;
    let tp_index = session
        .add_toolpath(
            0,
            ToolpathConfig {
                id: rs_cam_core::ToolpathId(0),
                name: format!("{} (cli run)", op_type.label()),
                enabled: true,
                operation: OperationConfig::new_default(op_type),
                dressups: DressupConfig::default(),
                heights: HeightsConfig::default(),
                tool_id,
                model_id: 0,
                pre_gcode: None,
                post_gcode: None,
                boundary: BoundaryConfig::default(),
                boundary_inherit: true,
                stock_source: StockSource::default(),
                coolant: CoolantMode::Off,
                face_selection: None,
                debug_options: ToolpathDebugOptions::default(),
                feeds_provenance: rs_cam_core::feeds::FeedsProvenance::default(),
            },
        )
        .map_err(|e| anyhow::anyhow!("adding toolpath: {e}"))?;

    // Registry-driven parameter application: the SAME serde round-trip
    // the GUI/MCP use, so type coercion, validation, and unknown-param
    // messages (listing valid names) are identical across surfaces.
    for kv in &args.set {
        let (key, value) = split_kv(kv)?;
        session
            .set_toolpath_param(tp_index, key, parse_json_value(value))
            .map_err(|e| anyhow::anyhow!("--set {key}: {e}"))?;
    }

    let post = session.post_mut();
    post.format = args.post.clone();
    post.safe_z = args.safe_z;
    post.spindle_speed = args.spindle_speed;

    // ── Generate + export ─────────────────────────────────────────
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(tp_index, &cancel)
        .map_err(|e| anyhow::anyhow!("generating {}: {e}", op_type.kind_str()))?;

    if let Some(result) = session.get_result(tp_index) {
        let toolpath = result.toolpath();
        info!(
            moves = toolpath.moves.len(),
            cutting_mm = format!("{:.1}", toolpath.total_cutting_distance()),
            rapid_mm = format!("{:.1}", toolpath.total_rapid_distance()),
            "Generated toolpath"
        );
        if let Some(svg_path) = &args.svg {
            let svg = rs_cam_core::viz::toolpath_to_svg(toolpath, 800.0, 600.0);
            std::fs::write(svg_path, svg)
                .with_context(|| format!("writing SVG to {}", svg_path.display()))?;
            info!(path = %svg_path.display(), "Wrote toolpath SVG");
        }
    }

    // `run` never simulates, so every tool-load gate reads
    // `SimulationRequired` (= unmodeled). Accept that tier — refusing
    // would make the command unusable — but keep `accept_exceeded`
    // false (unreachable without a sim trace anyway).
    session
        .export_gcode_with_policy(
            output,
            None,
            rs_cam_core::gcode::ToolLoadExportPolicy {
                accept_unmodeled: true,
                accept_exceeded: false,
            },
        )
        .map_err(|e| anyhow::anyhow!("exporting gcode: {e}"))?;
    info!(path = %output.display(), "Wrote G-code");
    Ok(())
}

// ── Listing helpers (the registry payoff) ─────────────────────────

fn print_ops() {
    println!("{:<22} {:<26} CATEGORY", "KIND", "LABEL");
    for &op in OperationType::ALL {
        let category = match op.category() {
            OpCategory::Menu2d => "2d",
            OpCategory::Menu3d => "3d",
            OpCategory::SystemOnly => "system-only",
        };
        println!("{:<22} {:<26} {}", op.kind_str(), op.label(), category);
    }
}

fn print_params(op_type: OperationType) {
    let entry = op_type.registry_entry();
    println!("{} ({})", entry.spec.label, op_type.kind_str());
    println!("  {}", entry.spec.description);
    let tc = entry.tool_constraints.to_schema();
    if !tc.required_tool_type.is_empty() {
        println!("  requires tool type: {}", tc.required_tool_type.join(", "));
    }
    if !tc.supports_v_bit {
        println!("  v_bit not supported");
    }
    println!();
    println!(
        "{:<28} {:<16} {:<9} DESCRIPTION",
        "PARAM", "TYPE", "OPTIONAL"
    );
    for def in entry.param_defs {
        println!(
            "{:<28} {:<16} {:<9} {}",
            def.name,
            def.type_name,
            if def.optional { "yes" } else { "no" },
            def.description.unwrap_or("")
        );
    }
}

// ── Parsing helpers ───────────────────────────────────────────────

fn parse_op_type(token: &str) -> Result<OperationType> {
    OperationType::ALL
        .iter()
        .copied()
        .find(|op| op.kind_str() == token)
        .with_context(|| {
            let valid: Vec<&str> = OperationType::ALL.iter().map(|op| op.kind_str()).collect();
            format!(
                "Unknown operation '{token}'. Valid operations: {}",
                valid.join(", ")
            )
        })
}

fn parse_units(s: &str) -> Result<ModelUnits> {
    match s.to_ascii_lowercase().as_str() {
        "mm" | "millimeters" => Ok(ModelUnits::Millimeters),
        "cm" | "centimeters" => Ok(ModelUnits::Centimeters),
        "m" | "meters" => Ok(ModelUnits::Meters),
        "inch" | "in" | "inches" => Ok(ModelUnits::Inches),
        other => match other.parse::<f64>() {
            Ok(scale) if scale > 0.0 => Ok(ModelUnits::Custom(scale)),
            _ => bail!("Unknown units '{s}'. Supported: mm, cm, m, inch, or a numeric scale"),
        },
    }
}

/// Parse `type:diameter` (e.g. `end_mill:6.35`). The type token goes
/// through the unified `ToolType::parse_lenient` vocabulary (T8).
fn parse_tool_spec(spec: &str) -> Result<(ToolType, f64)> {
    let (type_token, diameter_str) = spec
        .split_once(':')
        .context("--tool must be <type>:<diameter>, e.g. end_mill:6.35")?;
    let tool_type = ToolType::parse_lenient(type_token).with_context(|| {
        format!(
            "Unknown tool type '{type_token}'. Valid: end_mill, ball_nose, bull_nose, v_bit, \
             tapered_ball_nose"
        )
    })?;
    let diameter: f64 = diameter_str
        .parse()
        .with_context(|| format!("Invalid tool diameter '{diameter_str}'"))?;
    if !(diameter.is_finite() && diameter > 0.0) {
        bail!("Tool diameter must be positive, got {diameter}");
    }
    Ok((tool_type, diameter))
}

fn split_kv(kv: &str) -> Result<(&str, &str)> {
    kv.split_once('=')
        .with_context(|| format!("Expected key=value, got '{kv}'"))
}

/// Best-effort typed JSON from a CLI string: integer, float, bool,
/// else string. `set_toolpath_param`'s own coercion layer (E.6.a)
/// handles the rest (numeric strings, 0/1 bools, enum tokens).
fn parse_json_value(s: &str) -> serde_json::Value {
    if let Ok(i) = s.parse::<i64>() {
        return serde_json::Value::Number(i.into());
    }
    if let Ok(f) = s.parse::<f64>()
        && let Some(n) = serde_json::Number::from_f64(f)
    {
        return serde_json::Value::Number(n);
    }
    match s {
        "true" => serde_json::Value::Bool(true),
        "false" => serde_json::Value::Bool(false),
        _ => serde_json::Value::String(s.to_owned()),
    }
}
