//! Parameter sweep runner.
//!
//! Takes a base TOML job file, varies one parameter across specified values,
//! runs each variant through the full job pipeline (dressups, depth stepping,
//! simulation, G-code), and produces structured JSON output for agent analysis.
#![allow(clippy::print_stdout, clippy::indexing_slicing)]

use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use tracing::info;

use rs_cam_core::fingerprint::{
    ParameterSweepResult, StockFingerprint, SweepArtifacts, SweepVariant, ToolpathFingerprint,
    diff_fingerprints,
};

use crate::job;

/// Run a parameter sweep on a TOML job file.
///
/// Modifies one field in the first `[[operation]]` block across multiple values,
/// re-executes the full job pipeline for each, and writes structured results.
pub fn run_sweep(
    job_path: &Path,
    param_name: &str,
    values_str: &str,
    output_dir: &Path,
    simulate: bool,
) -> Result<()> {
    let job_dir = job_path.parent().unwrap_or(Path::new("."));

    // Parse base job
    let base_job = job::parse_job_file(job_path)?;
    if base_job.operation.is_empty() {
        bail!("Job file has no operations");
    }

    // Parse sweep values (comma-separated)
    let values: Vec<String> = values_str.split(',').map(|s| s.trim().to_owned()).collect();
    if values.is_empty() {
        bail!("No sweep values provided");
    }

    std::fs::create_dir_all(output_dir)
        .context(format!("Creating output dir: {}", output_dir.display()))?;

    // Run baseline
    info!("Running baseline...");
    let base_result = job::execute_job(&base_job, job_dir, false)?;
    let base_tp = &base_result.combined;
    let base_fp = ToolpathFingerprint::from_toolpath(base_tp);

    // Get baseline value of the parameter being swept
    let base_value = get_op_field(&base_job.operation[0], param_name)
        .unwrap_or_else(|| serde_json::Value::String("default".to_owned()));

    // Write baseline artifacts
    write_json(&output_dir.join("baseline.json"), &base_fp)?;
    write_toolpath_svg(&output_dir.join("baseline.svg"), base_tp);
    write_gcode(
        &output_dir.join("baseline.nc"),
        base_tp,
        &base_job,
        &base_result,
    )?;

    // Simulate baseline if requested
    let base_stock_fp = if simulate {
        let sfp = simulate_and_export(&base_result, &base_job, job_dir, output_dir, "baseline")?;
        Some(sfp)
    } else {
        None
    };

    if let Some(ref sfp) = base_stock_fp {
        write_json(&output_dir.join("baseline_stock.json"), sfp)?;
    }

    // Run variants
    let mut sweep_variants = Vec::new();

    for val_str in &values {
        info!(
            param = param_name,
            value = val_str.as_str(),
            "Running variant..."
        );

        // Patch the job file with the new parameter value
        let patched_job = patch_job_param(&base_job, param_name, val_str)?;

        // Execute
        let var_result = job::execute_job(&patched_job, job_dir, false)
            .context(format!("Executing variant {param_name}={val_str}"))?;

        let var_tp = &var_result.combined;
        let var_fp = ToolpathFingerprint::from_toolpath(var_tp);
        let diff = diff_fingerprints(&base_fp, &var_fp);

        // Write variant artifacts
        let safe_val = sanitize_filename(val_str);
        write_json(
            &output_dir.join(format!("variant_{safe_val}.json")),
            &var_fp,
        )?;
        write_json(
            &output_dir.join(format!("variant_{safe_val}_diff.json")),
            &diff,
        )?;
        write_toolpath_svg(&output_dir.join(format!("variant_{safe_val}.svg")), var_tp);
        write_gcode(
            &output_dir.join(format!("variant_{safe_val}.nc")),
            var_tp,
            &patched_job,
            &var_result,
        )?;

        // Simulate variant
        if simulate {
            let sfp = simulate_and_export(
                &var_result,
                &patched_job,
                job_dir,
                output_dir,
                &format!("variant_{safe_val}"),
            )?;
            write_json(
                &output_dir.join(format!("variant_{safe_val}_stock.json")),
                &sfp,
            )?;
        }

        let arts = SweepArtifacts::generate(var_tp);

        let json_val = parse_value_to_json(val_str);
        sweep_variants.push(SweepVariant {
            value: json_val,
            fingerprint: var_fp,
            diff,
            artifacts: Some(arts),
        });
    }

    // Write sweep summary
    let sweep_result = ParameterSweepResult {
        operation: base_job.operation[0].op_type.clone(),
        parameter_name: param_name.to_owned(),
        base_value,
        base_fingerprint: base_fp,
        variants: sweep_variants,
    };
    write_json(&output_dir.join("sweep_result.json"), &sweep_result)?;

    // Print summary
    println!("Sweep complete: {param_name}");
    println!("  Baseline + {} variants", values.len());
    println!("  Output: {}", output_dir.display());
    for (i, v) in sweep_result.variants.iter().enumerate() {
        let changed = v.diff.changed_fields.len();
        let unchanged = v.diff.unchanged_fields.len();
        println!(
            "  [{i}] {param_name}={}: {changed} fields changed, {unchanged} unchanged",
            values[i]
        );
    }

    Ok(())
}

// ── Helpers ─────────────────────────────────────────────────────────────

/// Get a field value from an OperationDef as JSON.
fn get_op_field(op: &job::OperationDef, field: &str) -> Option<serde_json::Value> {
    match field {
        "stepover" => op.stepover.map(|v| serde_json::json!(v)),
        "depth" => op.depth.map(|v| serde_json::json!(v)),
        "depth_per_pass" => op.depth_per_pass.map(|v| serde_json::json!(v)),
        "feed_rate" => op.feed_rate.map(|v| serde_json::json!(v)),
        "plunge_rate" => op.plunge_rate.map(|v| serde_json::json!(v)),
        "safe_z" => op.safe_z.map(|v| serde_json::json!(v)),
        "tolerance" => op.tolerance.map(|v| serde_json::json!(v)),
        "angle" => op.angle.map(|v| serde_json::json!(v)),
        "stock_to_leave" => op.stock_to_leave.map(|v| serde_json::json!(v)),
        "stock_top_z" => op.stock_top_z.map(|v| serde_json::json!(v)),
        "fine_stepdown" => op.fine_stepdown.map(|v| serde_json::json!(v)),
        "min_cutting_radius" => op.min_cutting_radius.map(|v| serde_json::json!(v)),
        "side" => op.side.as_ref().map(|s| serde_json::json!(s)),
        "pattern" => op.pattern.as_ref().map(|s| serde_json::json!(s)),
        "climb" => op.climb.map(|v| serde_json::json!(v)),
        "slot_clearing" => op.slot_clearing.map(|v| serde_json::json!(v)),
        "z_blend" => op.z_blend.map(|v| serde_json::json!(v)),
        "detect_flat_areas" => op.detect_flat_areas.map(|v| serde_json::json!(v)),
        "dogbone" => op.dogbone.map(|v| serde_json::json!(v)),
        "entry" => op.entry.as_ref().map(|s| serde_json::json!(s)),
        "strategy" => op.strategy.as_ref().map(|s| serde_json::json!(s)),
        "order_by" => op.order_by.as_ref().map(|s| serde_json::json!(s)),
        _ => None,
    }
}

/// Patch one parameter in the first operation of a job file.
/// Returns a new JobFile with the modification applied.
fn patch_job_param(base: &job::JobFile, field: &str, value: &str) -> Result<job::JobFile> {
    // Re-serialize the job to TOML, patch the field, and re-parse.
    // This is the safest way to handle all field types without manual cloning.
    //
    // Since JobFile uses Deserialize but not Serialize, we work with the raw
    // TOML string instead.
    let base_toml = toml::to_string(&SerializableJobFile::from(base))
        .context("Serializing base job to TOML")?;

    // Find the first [[operation]] table and patch the field
    let patched = patch_toml_field(&base_toml, field, value)?;

    let patched_job: job::JobFile =
        toml::from_str(&patched).context(format!("Re-parsing patched TOML for {field}={value}"))?;

    Ok(patched_job)
}

/// Patch a field in the first [[operation]] of a TOML string.
fn patch_toml_field(toml_str: &str, field: &str, value: &str) -> Result<String> {
    let mut lines: Vec<String> = toml_str.lines().map(String::from).collect();
    let mut in_operation = false;
    let mut patched = false;

    for line in &mut lines {
        if line.trim() == "[[operation]]" {
            in_operation = true;
            continue;
        }
        if in_operation && !patched {
            // Check if this line sets our field
            if line.trim_start().starts_with(&format!("{field} "))
                || line.trim_start().starts_with(&format!("{field}="))
            {
                *line = format_toml_field(field, value);
                patched = true;
                continue;
            }
            // If we hit the next section without finding the field, insert it
            if line.starts_with('[') || line.starts_with("[[") {
                let insert = format_toml_field(field, value);
                *line = format!("{insert}\n{line}");
                patched = true;
                continue;
            }
        }
    }

    // If we never found a place to insert, append to end of first operation
    if !patched && in_operation {
        lines.push(format_toml_field(field, value));
    }

    Ok(lines.join("\n"))
}

fn format_toml_field(field: &str, value: &str) -> String {
    // Try to parse as number/bool, otherwise quote as string
    if value.parse::<f64>().is_ok() || value.parse::<bool>().is_ok() {
        format!("{field} = {value}")
    } else {
        format!("{field} = \"{value}\"")
    }
}

fn parse_value_to_json(s: &str) -> serde_json::Value {
    if let Ok(n) = s.parse::<f64>() {
        serde_json::json!(n)
    } else if let Ok(b) = s.parse::<bool>() {
        serde_json::json!(b)
    } else {
        serde_json::json!(s)
    }
}

fn sanitize_filename(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn write_json(path: &Path, value: &impl serde::Serialize) -> Result<()> {
    let json = serde_json::to_string_pretty(value)?;
    std::fs::write(path, json).context(format!("Writing {}", path.display()))
}

fn write_toolpath_svg(path: &Path, tp: &rs_cam_core::toolpath::Toolpath) {
    let svg = rs_cam_core::viz::toolpath_to_svg(tp, 800.0, 600.0);
    let _ = std::fs::write(path, svg);
}

#[allow(clippy::needless_pass_by_value)]
fn write_gcode(
    path: &Path,
    tp: &rs_cam_core::toolpath::Toolpath,
    job: &job::JobFile,
    _result: &job::JobResult,
) -> Result<()> {
    let post = rs_cam_core::gcode::get_post_processor(&job.job.post)
        .unwrap_or_else(|| Box::new(rs_cam_core::gcode::GrblPost));
    let gcode = rs_cam_core::gcode::emit_gcode(tp, post.as_ref(), job.job.spindle_speed);
    std::fs::write(path, gcode).context(format!("Writing G-code to {}", path.display()))
}

/// Run simulation on a job result and export stock heightmap SVG.
fn simulate_and_export(
    result: &job::JobResult,
    job: &job::JobFile,
    _job_dir: &Path,
    output_dir: &Path,
    prefix: &str,
) -> Result<StockFingerprint> {
    use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
    use rs_cam_core::geo::BoundingBox3;

    // Build stock from first operation's geometry bounds + margin
    let tp = &result.combined;
    let (bbox_min, bbox_max) = tp.bounding_box();
    let margin = 5.0;
    let stock_bbox = BoundingBox3 {
        min: rs_cam_core::geo::P3::new(
            bbox_min[0] - margin,
            bbox_min[1] - margin,
            bbox_min[2] - margin,
        ),
        max: rs_cam_core::geo::P3::new(
            bbox_max[0] + margin,
            bbox_max[1] + margin,
            bbox_max[2] + margin,
        ),
    };

    let cell_size = job.job.sim_resolution;
    let mut stock = TriDexelStock::from_bounds(&stock_bbox, cell_size);

    // Simulate each phase
    for phase in &result.phases {
        stock.simulate_toolpath(&phase.toolpath, &phase.cutter, StockCutDirection::FromTop);
    }

    // Export composite stock PNG
    let w: u32 = 900;
    let h: u32 = 600;
    let pixels = rs_cam_core::fingerprint::render_stock_composite(&stock, w, h);
    if let Some(img) = image::RgbaImage::from_raw(w, h, pixels) {
        let _ = img.save(output_dir.join(format!("{prefix}_stock.png")));
    }

    Ok(StockFingerprint::from_stock(&stock))
}

// ── Serializable wrapper for JobFile ────────────────────────────────────
//
// JobFile uses Deserialize but not Serialize. We need a serializable mirror
// to generate TOML strings for patching.

use serde::Serialize;

#[derive(Serialize)]
struct SerializableJobFile {
    job: SerializableJobConfig,
    tools: std::collections::HashMap<String, SerializableToolDef>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    setup: Vec<SerializableSetupDef>,
    operation: Vec<SerializableOperationDef>,
}

#[derive(Serialize)]
struct SerializableJobConfig {
    output: PathBuf,
    post: String,
    spindle_speed: u32,
    safe_z: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    view: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    svg: Option<PathBuf>,
    simulate: bool,
    sim_resolution: f64,
    diagnostics: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    diagnostics_json: Option<PathBuf>,
}

#[derive(Serialize)]
struct SerializableToolDef {
    #[serde(rename = "type")]
    tool_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    number: Option<u32>,
    diameter: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    flute_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    corner_radius: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    included_angle: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    taper_angle: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shaft_diameter: Option<f64>,
}

#[derive(Serialize)]
struct SerializableSetupDef {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    output: Option<PathBuf>,
}

#[derive(Serialize)]
struct SerializableOperationDef {
    #[serde(rename = "type")]
    op_type: String,
    input: PathBuf,
    tool: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    setup: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stepover: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    depth: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    depth_per_pass: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    feed_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    plunge_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    safe_z: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    spindle_speed: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pattern: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    angle: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    climb: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    entry: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    side: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tabs: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tab_width: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tab_height: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dogbone: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tolerance: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    slot_clearing: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    min_cutting_radius: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    z_blend: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prev_tool: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scale: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stock_top_z: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stock_to_leave: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    entry_3d: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fine_stepdown: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    detect_flat_areas: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_stay_down_dist: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    order_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    strategy: Option<String>,
}

impl From<&job::JobFile> for SerializableJobFile {
    #[allow(clippy::indexing_slicing)] // bounded by structure
    fn from(j: &job::JobFile) -> Self {
        Self {
            job: SerializableJobConfig {
                output: j.job.output.clone(),
                post: j.job.post.clone(),
                spindle_speed: j.job.spindle_speed,
                safe_z: j.job.safe_z,
                view: j.job.view.clone(),
                svg: j.job.svg.clone(),
                simulate: j.job.simulate,
                sim_resolution: j.job.sim_resolution,
                diagnostics: j.job.diagnostics,
                diagnostics_json: j.job.diagnostics_json.clone(),
            },
            tools: j
                .tools
                .iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        SerializableToolDef {
                            tool_type: v.tool_type.to_string(),
                            number: v.number,
                            diameter: v.diameter,
                            flute_count: v.flute_count,
                            corner_radius: v.corner_radius,
                            included_angle: v.included_angle,
                            taper_angle: v.taper_angle,
                            shaft_diameter: v.shaft_diameter,
                        },
                    )
                })
                .collect(),
            setup: j
                .setup
                .iter()
                .map(|s| SerializableSetupDef {
                    name: s.name.clone(),
                    output: s.output.clone(),
                })
                .collect(),
            operation: j
                .operation
                .iter()
                .map(|o| SerializableOperationDef {
                    op_type: o.op_type.clone(),
                    input: o.input.clone(),
                    tool: o.tool.clone(),
                    setup: o.setup.clone(),
                    stepover: o.stepover,
                    depth: o.depth,
                    depth_per_pass: o.depth_per_pass,
                    feed_rate: o.feed_rate,
                    plunge_rate: o.plunge_rate,
                    safe_z: o.safe_z,
                    spindle_speed: o.spindle_speed,
                    pattern: o.pattern.clone(),
                    angle: o.angle,
                    climb: o.climb,
                    entry: o.entry.clone(),
                    side: o.side.clone(),
                    tabs: o.tabs,
                    tab_width: o.tab_width,
                    tab_height: o.tab_height,
                    dogbone: o.dogbone,
                    tolerance: o.tolerance,
                    slot_clearing: o.slot_clearing,
                    min_cutting_radius: o.min_cutting_radius,
                    z_blend: o.z_blend,
                    prev_tool: o.prev_tool.clone(),
                    scale: o.scale,
                    stock_top_z: o.stock_top_z,
                    stock_to_leave: o.stock_to_leave,
                    entry_3d: o.entry_3d.clone(),
                    fine_stepdown: o.fine_stepdown,
                    detect_flat_areas: o.detect_flat_areas,
                    max_stay_down_dist: o.max_stay_down_dist,
                    order_by: o.order_by.clone(),
                    strategy: o.strategy.clone(),
                })
                .collect(),
        }
    }
}
