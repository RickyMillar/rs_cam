//! Parameter sweep runner.
//!
//! Takes a base TOML job file, varies one parameter across specified values,
//! runs each variant through the full job pipeline (dressups, depth stepping,
//! simulation, G-code), and produces structured JSON output for agent analysis.
#![allow(clippy::print_stdout)]

use anyhow::{Context, Result, bail};
use std::path::Path;
use tracing::info;

use rs_cam_core::export::fingerprint::{
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

    // Read the baseline before any work: a field name the parser does not
    // know refuses here, not after the baseline run has written artifacts.
    let first_probe = values
        .first()
        .context("the sweep has no values to probe the field with")?;
    let base_value = resolve_base_value(&base_job, param_name, first_probe)?;
    let base_op_type = base_job
        .operation
        .first()
        .context("the job file declares no [[operation]] block to sweep")?
        .op_type
        .clone();

    std::fs::create_dir_all(output_dir)
        .context(format!("Creating output dir: {}", output_dir.display()))?;

    // Run baseline
    info!("Running baseline...");
    let base_result = job::execute_job(&base_job, job_dir, false)?;
    let base_tp = &base_result.combined;
    let base_fp = ToolpathFingerprint::from_toolpath(base_tp);

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

        let json_val = crate::job::param_value_from_str(val_str);
        sweep_variants.push(SweepVariant {
            value: json_val,
            fingerprint: var_fp,
            diff,
            artifacts: Some(arts),
        });
    }

    // Write sweep summary
    let sweep_result = ParameterSweepResult {
        operation: base_op_type,
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
    for (i, (v, value)) in sweep_result.variants.iter().zip(values).enumerate() {
        let changed = v.diff.changed_fields.len();
        let unchanged = v.diff.unchanged_fields.len();
        println!("  [{i}] {param_name}={value}: {changed} fields changed, {unchanged} unchanged");
    }

    Ok(())
}

// ── Helpers ─────────────────────────────────────────────────────────────

/// The JSON object the job parser itself makes of one operation.
///
/// The keys are the TOML keys of the job file, so the sweep reads a field by
/// the same name it patches. `OperationDef` skips an unset option, so a field
/// the job file does not set is absent from the map.
fn op_field_map(op: &job::OperationDef) -> Result<serde_json::Map<String, serde_json::Value>> {
    match serde_json::to_value(op).context("Serializing the swept operation")? {
        serde_json::Value::Object(map) => Ok(map),
        other => bail!("the operation serialized as {other}, which is not a table"),
    }
}

/// The value the job file gives the swept field, before the sweep patches it.
///
/// An agent reads `base_value` to learn what the sweep varied FROM, so the
/// sweep reports only what it read:
///
/// - the field's value, exactly as the parsed job file carries it;
/// - JSON `null` when the job file leaves the field unset, which states "the
///   job file sets nothing here; the operation ran on the planner default";
/// - a refusal that names the field when `OperationDef` has no such key.
///
/// The refusal matters: the parser drops an unknown key, so such a sweep
/// patches nothing and every variant repeats the baseline.
fn resolve_base_value(
    base: &job::JobFile,
    field: &str,
    probe_value: &str,
) -> Result<serde_json::Value> {
    // The sweep patches the FIRST `[[operation]]` block, so a job file with
    // none has nothing to sweep and the run stops here with that sentence.
    let first_op = base
        .operation
        .first()
        .context("the job file declares no [[operation]] block to sweep")?;
    if let Some(value) = op_field_map(first_op)?.get(field) {
        return Ok(value.clone());
    }

    // The job file does not set the field. Patch it with the first sweep
    // value and read the parsed result back: a key `OperationDef` knows
    // survives the round trip, an unknown key does not.
    let probe_job = patch_job_param(base, field, probe_value)?;
    let probe_op = probe_job
        .operation
        .first()
        .context("the patched job file lost its [[operation]] block")?;
    if op_field_map(probe_op)?.contains_key(field) {
        Ok(serde_json::Value::Null)
    } else {
        let set_fields = op_field_map(first_op)?;
        let known: Vec<&str> = set_fields.keys().map(String::as_str).collect();
        bail!(
            "the job file's [[operation]] has no field `{field}`; the parser drops that key, so every variant would repeat the baseline. The first operation sets: {}",
            known.join(", ")
        )
    }
}

/// Patch one parameter in the first operation of a job file.
/// Returns a new JobFile with the modification applied.
fn patch_job_param(base: &job::JobFile, field: &str, value: &str) -> Result<job::JobFile> {
    // Re-serialize the job to TOML, patch the field, and re-parse.
    // This is the safest way to handle all field types without manual cloning.
    //
    // `job::JobFile` derives both directions, so the sweep baseline keeps
    // every field the parser knows (I10 pair 4: a hand-written mirror dropped
    // the shank and holder geometry).
    let base_toml = toml::to_string(base).context("Serializing base job to TOML")?;

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
        if !in_operation {
            // Every line before the first `[[operation]]` header belongs to
            // another table.
            if line.trim() == "[[operation]]" {
                in_operation = true;
            }
            continue;
        }
        if patched {
            continue;
        }
        // Check if this line sets our field
        if line.trim_start().starts_with(&format!("{field} "))
            || line.trim_start().starts_with(&format!("{field}="))
        {
            *line = format_toml_field(field, value);
            patched = true;
            continue;
        }
        // The first operation ends at the next header, `[[operation]]` for
        // the second operation included. Insert the field before it. The
        // header test used to run before this one, so a second operation
        // re-armed the scan instead of closing the first: the field was
        // appended to the LAST operation, and the sweep varied an operation
        // it does not report.
        if line.starts_with('[') {
            let insert = format_toml_field(field, value);
            *line = format!("{insert}\n{line}");
            patched = true;
            continue;
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
    let svg = rs_cam_core::export::viz::toolpath_to_svg(tp, 800.0, 600.0);
    let _ = std::fs::write(path, svg);
}

#[allow(clippy::needless_pass_by_value)]
fn write_gcode(
    path: &Path,
    tp: &rs_cam_core::toolpath::Toolpath,
    job: &job::JobFile,
    _result: &job::JobResult,
) -> Result<()> {
    let post = rs_cam_core::gcode::get_post_definition(&job.job.post)
        .unwrap_or_else(|| rs_cam_core::gcode::PostFormat::Grbl.definition());
    let gcode = rs_cam_core::gcode::emit_gcode(tp, post, job.job.spindle_speed);
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
    let ([min_x, min_y, min_z], [max_x, max_y, max_z]) = tp.bounding_box();
    let margin = 5.0;
    let stock_bbox = BoundingBox3 {
        min: rs_cam_core::geo::P3::new(min_x - margin, min_y - margin, min_z - margin),
        max: rs_cam_core::geo::P3::new(max_x + margin, max_y + margin, max_z + margin),
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
    let pixels = rs_cam_core::export::fingerprint::render_stock_composite(&stock, w, h);
    if let Some(img) = image::RgbaImage::from_raw(w, h, pixels) {
        let _ = img.save(output_dir.join(format!("{prefix}_stock.png")));
    }

    Ok(StockFingerprint::from_stock(&stock))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::resolve_base_value;
    use crate::job::{CliToolType, JobFile};

    /// A job file with one adaptive3d operation. `min_region_cut_length_mm`
    /// is one of the fields the old hand-written reader did not cover.
    fn sweepable_job() -> JobFile {
        let source = r#"
[job]
output = "part.nc"

[tools.flat_6mm]
type = "flat"
diameter = 6.35

[[operation]]
type = "adaptive3d"
input = "design.stl"
tool = "flat_6mm"
stepover = 2.0
min_region_cut_length_mm = 4.0
"#;
        toml::from_str(source).unwrap()
    }

    /// CLI-05 sentry: the sweep report states the field's real baseline.
    ///
    /// The reader used to name 22 of the 42 job fields and answer the string
    /// `"default"` for the rest. An agent reads `base_value` to learn what
    /// the sweep varied FROM, so that string was made-up evidence.
    #[test]
    fn a_swept_field_reports_its_real_baseline_not_the_word_default() {
        let job = sweepable_job();

        // An uncovered field the job file sets: the real value, not "default".
        let covered = resolve_base_value(&job, "min_region_cut_length_mm", "9.0").unwrap();
        assert_eq!(covered, serde_json::json!(4.0));
        assert_ne!(covered, serde_json::json!("default"));

        // A field the reader already named stays exact.
        assert_eq!(
            resolve_base_value(&job, "stepover", "3.0").unwrap(),
            serde_json::json!(2.0)
        );
    }

    /// A field the job file leaves unset reports `null`, which states "the
    /// job file sets nothing here", not a value the sweep never read.
    #[test]
    fn an_unset_field_reports_null_not_a_made_up_value() {
        let job = sweepable_job();
        let value = resolve_base_value(&job, "stock_to_leave", "0.3").unwrap();
        assert_eq!(value, serde_json::Value::Null);
        assert_ne!(value, serde_json::json!("default"));
    }

    /// The sweep patches the FIRST operation, so a field the job file leaves
    /// unset must land there. The header test used to re-arm the scan on the
    /// second `[[operation]]`, which appended the field to the LAST operation:
    /// the sweep then varied one operation and reported another.
    #[test]
    fn an_unset_field_lands_in_the_first_operation_not_the_last() {
        let source = r#"
[job]
output = "part.nc"

[tools.flat_6mm]
type = "flat"
diameter = 6.35

[[operation]]
type = "adaptive"
input = "rough.svg"
tool = "flat_6mm"

[[operation]]
type = "profile"
input = "finish.svg"
tool = "flat_6mm"
"#;
        let base: JobFile = toml::from_str(source).unwrap();
        let patched = super::patch_job_param(&base, "min_region_cut_length_mm", "3.0").unwrap();
        assert_eq!(patched.operation[0].min_region_cut_length_mm, Some(3.0));
        assert_eq!(patched.operation[1].min_region_cut_length_mm, None);

        // The baseline reads the same operation the patch writes.
        assert_eq!(
            resolve_base_value(&base, "min_region_cut_length_mm", "3.0").unwrap(),
            serde_json::Value::Null
        );
    }

    /// A field name `OperationDef` does not know refuses and names the field.
    /// The parser drops the key, so the sweep would repeat the baseline.
    #[test]
    fn a_field_the_parser_does_not_know_refuses_the_sweep() {
        let job = sweepable_job();
        let err = resolve_base_value(&job, "not_a_field", "1.0").unwrap_err();
        assert!(
            err.to_string().contains("not_a_field"),
            "the refusal must name the field, got: {err}"
        );
    }

    /// I10 pair 4: the sweep baseline is a serialize-then-reparse round trip.
    /// Every field the parser knows must survive it. The hand-written mirror
    /// this replaced dropped the shank and holder geometry, so deflection and
    /// reach modelling read different tools in a sweep than in the base job.
    #[test]
    fn sweep_baseline_round_trip_keeps_every_job_field() {
        let source = r#"
[job]
output = "part.nc"
post = "grbl"
spindle_speed = 16000
safe_z = 12.0
view = "view.png"
svg = "part.svg"
simulate = true
sim_resolution = 0.35
diagnostics = true
diagnostics_json = "diag.json"

[tools.flat_6mm]
type = "flat"
number = 3
diameter = 6.35
flute_count = 3
corner_radius = 0.5
included_angle = 60.0
taper_angle = 4.0
shaft_diameter = 3.0
shank_diameter = 6.0
shank_length = 20.0
holder_diameter = 40.0
holder_length = 35.0

[[setup]]
name = "front"
output = "front.nc"

[[operation]]
type = "adaptive3d"
input = "design.stl"
tool = "flat_6mm"
setup = "front"
stepover = 2.0
depth = 6.0
depth_per_pass = 3.0
feed_rate = 1000.0
plunge_rate = 500.0
safe_z = 11.0
spindle_speed = 17000
coolant = "mist"
pattern = "zigzag"
angle = 45.0
climb = true
entry = "ramp"
side = "outside"
tabs = 4
tab_width = 6.0
tab_height = 1.5
dogbone = true
tolerance = 0.05
slot_clearing = true
min_cutting_radius = 1.2
z_blend = true
prev_tool = "flat_6mm"
scale = 2.0
stock_top_z = 1.0
stock_to_leave = 0.3
entry_3d = "helix"
fine_stepdown = 0.4
detect_flat_areas = true
max_stay_down_dist = 30.0
order_by = "depth"
strategy = "contour"
mill_shallow_areas = true
shallow_angle_deg = 25.0
shallow_stepdown = 0.6
min_region_cut_length_mm = 4.0
max_stay_down_distance_mm = 18.0
stay_down_clearance_mm = 0.75
"#;
        let base: JobFile = toml::from_str(source).unwrap();
        // The sweep serializes the parsed job exactly this way.
        let emitted = toml::to_string(&base).unwrap();
        let back: JobFile = toml::from_str(&emitted).unwrap();

        let tool = back.tools.get("flat_6mm").unwrap();
        assert_eq!(tool.shank_diameter, Some(6.0));
        assert_eq!(tool.shank_length, Some(20.0));
        assert_eq!(tool.holder_diameter, Some(40.0));
        assert_eq!(tool.holder_length, Some(35.0));
        assert_eq!(tool.number, Some(3));
        assert_eq!(tool.flute_count, Some(3));
        assert_eq!(tool.corner_radius, Some(0.5));
        assert_eq!(tool.included_angle, Some(60.0));
        assert_eq!(tool.taper_angle, Some(4.0));
        assert_eq!(tool.shaft_diameter, Some(3.0));
        assert!(matches!(tool.tool_type, CliToolType::Flat));
        assert_eq!(tool.diameter, 6.35);

        assert_eq!(back.job.view, base.job.view);
        assert_eq!(back.job.svg, base.job.svg);
        assert_eq!(back.job.diagnostics_json, base.job.diagnostics_json);
        assert_eq!(back.job.spindle_speed, 16000);
        assert_eq!(back.setup.len(), 1);
        assert_eq!(back.setup[0].name, "front");

        let op = &back.operation[0];
        assert_eq!(op.coolant, rs_cam_core::gcode::CoolantMode::Mist);
        assert_eq!(op.mill_shallow_areas, Some(true));
        assert_eq!(op.shallow_angle_deg, Some(25.0));
        assert_eq!(op.shallow_stepdown, Some(0.6));
        assert_eq!(op.min_region_cut_length_mm, Some(4.0));
        assert_eq!(op.max_stay_down_distance_mm, Some(18.0));
        assert_eq!(op.stay_down_clearance_mm, Some(0.75));
        assert_eq!(op.entry_3d.as_deref(), Some("helix"));
        assert_eq!(op.tabs, Some(4));
        assert_eq!(op.prev_tool.as_deref(), Some("flat_6mm"));
        assert_eq!(op.strategy.as_deref(), Some("contour"));
        assert_eq!(op.order_by.as_deref(), Some("depth"));
    }

    /// The emitted tool-type strings must re-parse as the same variants.
    #[test]
    fn sweep_baseline_round_trip_keeps_every_tool_type() {
        let source = r#"
[job]
output = "part.nc"

[tools.a]
type = "flat"
diameter = 6.0

[tools.b]
type = "ball"
diameter = 6.0

[tools.c]
type = "bullnose"
diameter = 6.0

[tools.d]
type = "vbit"
diameter = 6.0

[tools.e]
type = "tapered_ball"
diameter = 6.0

[[operation]]
type = "pocket"
input = "design.svg"
tool = "a"
"#;
        let base: JobFile = toml::from_str(source).unwrap();
        let emitted = toml::to_string(&base).unwrap();
        let back: JobFile = toml::from_str(&emitted).unwrap();
        for (name, tool) in &base.tools {
            let round = back.tools.get(name).unwrap();
            assert_eq!(
                tool.tool_type.to_string(),
                round.tool_type.to_string(),
                "tool {name} changed type through the round trip"
            );
        }
        assert!(matches!(
            back.tools.get("e").unwrap().tool_type,
            CliToolType::TaperedBall
        ));
    }
}
