//! F-037 — Smoke baseline runner.
//!
//! Iterates `planning/toolpath_acceptance/cases_agent_smoke.csv`, loads each
//! row's project template, applies the row's baseline params to a fresh
//! toolpath, generates + simulates, and writes per-toolpath verdicts to a
//! baseline CSV. The output is the regression net for the Feed Modulation
//! workstream — any future PR touching simulator-adjacent code should
//! diff its smoke run against this baseline and fail on regression.
//!
//! Best-effort: cases that fail to load / generate / simulate write a row
//! with `status` set to the failure class. The auditor consumes the CSV;
//! `harness_error` rows surface as findings in the next loop round.

#![allow(clippy::print_stdout)] // CLI surface

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use tracing::info;

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::feeds::embedded_vendor_lut;
use rs_cam_core::feeds::suggest::{StockContext, SuggestParamsInput, suggest_params};
use rs_cam_core::material::{AluminumAlloy, Material, PlywoodGrade, SheetGoodKind, WoodSpecies};
use rs_cam_core::session::{ProjectSession, SimulationOptions, ToolpathConfig};
use rs_cam_core::tool_load::drill_gates::DrillGatesVerdict;
use rs_cam_core::tool_load::verdict::ChipSide;
use rs_cam_core::tool_load::{
    ChiploadVerdict, DeflectionVerdict, PowerVerdict, ToolpathLoadVerdict,
};

// ── CSV row types ───────────────────────────────────────────────────────

/// Row in `cases_agent_smoke.csv`. Only some columns are consumed by the
/// runner (case_id, operation_kind, project_template, tool_name,
/// material_family, baseline_params); the rest are retained for schema
/// fidelity and future extension.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct SmokeCase {
    case_id: String,
    #[serde(default)]
    case_group: String,
    operation_kind: String,
    project_template: String,
    #[serde(default)]
    fixture: String,
    #[serde(default)]
    geometry_kind: String,
    #[serde(default)]
    setup_face: String,
    #[serde(default)]
    tool_name: String,
    #[serde(default)]
    tool_type: String,
    #[serde(default)]
    tool_family: String,
    #[serde(default)]
    tool_diameter_mm: String,
    #[serde(default)]
    flute_count: String,
    #[serde(default)]
    material_family: String,
    #[serde(default)]
    goal_id: String,
    #[serde(default)]
    goal_type: String,
    #[serde(default)]
    quality_tier: String,
    #[serde(default)]
    baseline_params: String,
    #[serde(default)]
    variants: String,
    #[serde(default)]
    run_optimizer: String,
    #[serde(default)]
    notes: String,
    /// Comma-separated list of case_ids whose toolpaths must run first
    /// against this same project so the measured toolpath cuts from
    /// already-roughed stock (round-10 STATE.md methodology — AS015
    /// finishing needs an AS013 roughing pass first or its measured
    /// deflection reflects fresh-stock contact, not residual-stock
    /// contact).
    ///
    /// When non-empty, the runner adds each prior toolpath with
    /// `StockSource::Fresh`, generates them, then materializes the
    /// measured case with `StockSource::FromRemainingStock`. All run
    /// in a single `run_simulation` call so the dexel state chains.
    /// Prior-pass verdicts are NOT written to the baseline; only the
    /// measured case's verdict is.
    #[serde(default)]
    prior_passes: String,
}

/// One row of `baseline.csv` — one per case (one toolpath per case).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineRow {
    pub case_id: String,
    pub op_kind: String,
    pub status: String,
    pub chipload_kind: String,
    pub chipload_observed_mm_tooth: String,
    pub deflection_kind: String,
    pub deflection_peak_mm: String,
    pub power_kind: String,
    pub power_peak_kw: String,
    pub rapid_collision_count: String,
    pub avg_engagement: String,
    pub peak_axial_doc_mm: String,
    pub drill_chip_welding_kind: String,
    pub drill_chip_welding_observed: String,
    pub drill_peck_kind: String,
    pub drill_plunge_kind: String,
    pub notes: String,
}

impl BaselineRow {
    fn failure(case_id: &str, op_kind: &str, status: &str, note: &str) -> Self {
        Self {
            case_id: case_id.to_owned(),
            op_kind: op_kind.to_owned(),
            status: status.to_owned(),
            chipload_kind: String::new(),
            chipload_observed_mm_tooth: String::new(),
            deflection_kind: String::new(),
            deflection_peak_mm: String::new(),
            power_kind: String::new(),
            power_peak_kw: String::new(),
            rapid_collision_count: String::new(),
            avg_engagement: String::new(),
            peak_axial_doc_mm: String::new(),
            drill_chip_welding_kind: String::new(),
            drill_chip_welding_observed: String::new(),
            drill_peck_kind: String::new(),
            drill_plunge_kind: String::new(),
            notes: note.to_owned(),
        }
    }
}

// ── Main entry points ───────────────────────────────────────────────────

/// Run the full smoke suite and write `output` CSV.
pub fn run_smoke(input_csv: &Path, output: &Path, resolution: f64) -> Result<()> {
    let cases = load_cases(input_csv)?;
    info!(count = cases.len(), "Loaded smoke cases");

    let mut rows = Vec::with_capacity(cases.len());
    for case in &cases {
        info!(case_id = %case.case_id, op = %case.operation_kind, "Running smoke case");
        let row = run_single_case(case, &cases, resolution);
        info!(
            case_id = %case.case_id,
            status = %row.status,
            chipload = %row.chipload_kind,
            deflection = %row.deflection_kind,
            "Smoke case done"
        );
        rows.push(row);
    }

    write_baseline(output, &rows)?;
    info!(path = %output.display(), rows = rows.len(), "Wrote baseline CSV");
    Ok(())
}

/// Diff two baseline CSVs. Returns Ok(true) if any regression detected.
pub fn run_diff(baseline_path: &Path, current_path: &Path) -> Result<bool> {
    let baseline = read_baseline(baseline_path)?;
    let current = read_baseline(current_path)?;

    let base_by_case: BTreeMap<String, &BaselineRow> =
        baseline.iter().map(|r| (r.case_id.clone(), r)).collect();
    let curr_by_case: BTreeMap<String, &BaselineRow> =
        current.iter().map(|r| (r.case_id.clone(), r)).collect();

    let mut regressions: Vec<String> = Vec::new();

    for (case_id, base_row) in &base_by_case {
        let Some(curr_row) = curr_by_case.get(case_id) else {
            regressions.push(format!("{case_id}: missing from current run"));
            continue;
        };
        // Status regression: anything → harness_error/generation_failed/simulation_failed
        if base_row.status == "ok" && curr_row.status != "ok" {
            regressions.push(format!(
                "{case_id}: status {} → {}",
                base_row.status, curr_row.status
            ));
            continue;
        }
        // Per-criterion verdict regression: Within → Exceeds
        if is_within(&base_row.chipload_kind) && is_exceeds(&curr_row.chipload_kind) {
            regressions.push(format!(
                "{case_id}: chipload {} → {}",
                base_row.chipload_kind, curr_row.chipload_kind
            ));
        }
        if is_within(&base_row.deflection_kind) && is_exceeds(&curr_row.deflection_kind) {
            regressions.push(format!(
                "{case_id}: deflection {} → {}",
                base_row.deflection_kind, curr_row.deflection_kind
            ));
        }
        if is_within(&base_row.power_kind) && is_exceeds(&curr_row.power_kind) {
            regressions.push(format!(
                "{case_id}: power {} → {}",
                base_row.power_kind, curr_row.power_kind
            ));
        }
        // Rapid collisions: anything → more collisions
        let base_collisions: u32 = base_row.rapid_collision_count.parse().unwrap_or(0);
        let curr_collisions: u32 = curr_row.rapid_collision_count.parse().unwrap_or(0);
        if curr_collisions > base_collisions {
            regressions.push(format!(
                "{case_id}: rapid_collisions {base_collisions} → {curr_collisions}"
            ));
        }
    }

    // Cases new to current run are informational, not regressions.
    if regressions.is_empty() {
        println!(
            "smoke-diff: no regressions ({} cases checked)",
            baseline.len()
        );
        Ok(false)
    } else {
        println!("smoke-diff: {} regression(s)", regressions.len());
        for r in &regressions {
            println!("  - {r}");
        }
        Ok(true)
    }
}

fn is_within(kind: &str) -> bool {
    kind == "within"
}

fn is_exceeds(kind: &str) -> bool {
    kind == "exceeds"
}

// ── Case execution ──────────────────────────────────────────────────────

/// Materializes one case's toolpath into an existing session: resolves
/// op + tool, calls `suggest_params`, adds the `ToolpathConfig` with the
/// caller-supplied `stock_source`, applies the case's baseline_params,
/// and generates the toolpath. Used by `run_single_case` for both prior
/// passes (`StockSource::Fresh`) and the measured case
/// (`StockSource::FromRemainingStock` when chaining, else `Fresh`).
///
/// Returns `(tp_idx, op_type, param_warnings)` on success, or a
/// `BaselineRow` describing the failure class so callers can short-
/// circuit with it.
#[allow(clippy::result_large_err)] // tp_idx tuple is small; failure path is the rare branch
fn materialize_case_toolpath(
    session: &mut ProjectSession,
    case: &SmokeCase,
    stock_source: rs_cam_core::compute::config::StockSource,
    name_suffix: &str,
) -> Result<(usize, OperationType, Vec<String>), BaselineRow> {
    let Some(op_type) = parse_op_type(&case.operation_kind) else {
        return Err(BaselineRow::failure(
            &case.case_id,
            &case.operation_kind,
            "harness_error",
            &format!("unknown operation_kind {}", case.operation_kind),
        ));
    };

    let Some(tool_id) = pick_tool(session, &case.tool_name) else {
        return Err(BaselineRow::failure(
            &case.case_id,
            op_type.kind_str(),
            "harness_error",
            "no tool available in template",
        ));
    };

    let stock_ctx =
        StockContext::from_stock_bbox(session.stock_bbox(), session.stock_config().padding);
    let tool = match session.tools().iter().find(|t| t.id.0 == tool_id) {
        Some(t) => t.clone(),
        None => {
            return Err(BaselineRow::failure(
                &case.case_id,
                op_type.kind_str(),
                "harness_error",
                "tool resolution mismatch",
            ));
        }
    };
    let machine = session.machine().clone();
    let material = session.stock_config().material.clone();
    let workholding = session.stock_config().workholding_rigidity;
    let operation = match suggest_params(SuggestParamsInput {
        op_type,
        tool: &tool,
        machine: &machine,
        material: &material,
        workholding,
        lut: embedded_vendor_lut(),
        stock_ctx: &stock_ctx,
        spindle_strategy: rs_cam_core::feeds::SpindleStrategy::default(),
        context: rs_cam_core::feeds::suggest::SuggestContext::default(),
    }) {
        Ok(s) => s.operation,
        Err(e) => {
            return Err(BaselineRow::failure(
                &case.case_id,
                op_type.kind_str(),
                "suggest_refused",
                &format!("{e}"),
            ));
        }
    };

    let model_id = session.models().first().map(|m| m.id).unwrap_or(0);

    let tc = ToolpathConfig {
        id: 0,
        name: format!("{} {name_suffix}", op_type.kind_str()),
        enabled: true,
        operation,
        dressups: rs_cam_core::compute::config::DressupConfig::for_op(op_type),
        heights: rs_cam_core::compute::config::HeightsConfig::default(),
        tool_id,
        model_id,
        pre_gcode: None,
        post_gcode: None,
        boundary: rs_cam_core::compute::config::BoundaryConfig::default(),
        boundary_inherit: true,
        stock_source,
        coolant: rs_cam_core::gcode::CoolantMode::Off,
        face_selection: None,
        debug_options: rs_cam_core::debug_trace::ToolpathDebugOptions::default(),
        feeds_provenance: rs_cam_core::feeds::FeedsProvenance::default(),
    };

    let tp_idx = match session.add_toolpath(0, tc) {
        Ok(i) => i,
        Err(e) => {
            return Err(BaselineRow::failure(
                &case.case_id,
                op_type.kind_str(),
                "harness_error",
                &format!("add_toolpath failed: {e}"),
            ));
        }
    };

    let params = parse_baseline_params(&case.baseline_params);
    let mut param_warnings = Vec::new();
    for (key, value) in &params {
        // stock_* keys are routed to session.stock_mut() up front via
        // apply_stock_overrides; skip them here so they don't pollute
        // param_warnings with "unknown parameter" noise.
        if key.starts_with("stock_") {
            continue;
        }
        let json_value = serde_json::Value::String(value.clone());
        if let Err(e) = session.set_toolpath_param(tp_idx, key, json_value) {
            param_warnings.push(format!("{key}={value}: {e}"));
        }
    }

    let cancel = AtomicBool::new(false);
    if let Err(e) = session.generate_toolpath(tp_idx, &cancel) {
        return Err(BaselineRow::failure(
            &case.case_id,
            op_type.kind_str(),
            "generation_failed",
            &format!("{e}; param_warnings={}", param_warnings.join("|")),
        ));
    }

    Ok((tp_idx, op_type, param_warnings))
}

fn run_single_case(case: &SmokeCase, all_cases: &[SmokeCase], resolution: f64) -> BaselineRow {
    let Some(op_type) = parse_op_type(&case.operation_kind) else {
        return BaselineRow::failure(
            &case.case_id,
            &case.operation_kind,
            "harness_error",
            &format!("unknown operation_kind {}", case.operation_kind),
        );
    };

    // Resolve template path relative to CWD (the project root).
    let template_path = PathBuf::from(&case.project_template);
    if !template_path.exists() {
        return BaselineRow::failure(
            &case.case_id,
            op_type.kind_str(),
            "harness_error",
            &format!("project template not found: {}", template_path.display()),
        );
    }

    let mut session = match ProjectSession::load(&template_path) {
        Ok(s) => s,
        Err(e) => {
            return BaselineRow::failure(
                &case.case_id,
                op_type.kind_str(),
                "harness_error",
                &format!("failed to load template: {e}"),
            );
        }
    };

    // Override material on stock to match the measured row's
    // `material_family`. The template's default material may differ
    // from what the smoke row wants to test — the goal_id / vendor LUT
    // lookup keys off material. For chained runs, the measured case
    // drives material so prior passes cut the same physical stock.
    if !case.material_family.is_empty()
        && let Some(material) = material_for_family(&case.material_family)
    {
        session.stock_mut().material = material;
    }

    // Apply any stock_* prefixed params from the measured case's
    // baseline_params to the stock config (rather than the toolpath
    // operation schema, which rejects them as unknown). Round-09's
    // MCP workflow did this implicitly via project setup; round-10+
    // CLI smoke needs to route them here. See planning/phase_5_*.
    // The measured case drives stock geometry for chained runs.
    apply_stock_overrides(&mut session, &case.baseline_params, &case.case_id);

    // Disable any existing toolpaths so simulation only sees what we
    // explicitly add (prior passes + measured case).
    for tc in session.toolpath_configs_mut().iter_mut() {
        tc.enabled = false;
    }

    // Resolve prior_passes: each id must reference a case earlier in
    // the suite. Prior passes share the measured case's project + stock
    // material — they're a residual-stock setup, not standalone runs.
    let prior_ids: Vec<&str> = case
        .prior_passes
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();

    let mut combined_param_warnings: Vec<String> = Vec::new();
    for prior_id in &prior_ids {
        let Some(prior_case) = all_cases.iter().find(|c| c.case_id == *prior_id) else {
            return BaselineRow::failure(
                &case.case_id,
                op_type.kind_str(),
                "harness_error",
                &format!("prior_passes references unknown case_id {prior_id}"),
            );
        };
        match materialize_case_toolpath(
            &mut session,
            prior_case,
            rs_cam_core::compute::config::StockSource::Fresh,
            &format!("prior:{prior_id}"),
        ) {
            Ok((_, _, warns)) => {
                for w in warns {
                    combined_param_warnings.push(format!("prior[{prior_id}]: {w}"));
                }
            }
            Err(mut row) => {
                // Surface prior-pass failures as harness_error against
                // the measured case so the regression diff still sees
                // a stable row for this case_id.
                row.case_id.clone_from(&case.case_id);
                row.op_kind = op_type.kind_str().to_owned();
                row.notes = format!("prior[{prior_id}] failed: {}", row.notes);
                return row;
            }
        }
    }

    let measured_stock_source = if prior_ids.is_empty() {
        rs_cam_core::compute::config::StockSource::Fresh
    } else {
        rs_cam_core::compute::config::StockSource::FromRemainingStock
    };

    let (tp_idx, op_type, mut param_warnings) =
        match materialize_case_toolpath(&mut session, case, measured_stock_source, "smoke") {
            Ok(triple) => triple,
            Err(row) => return row,
        };

    param_warnings.extend(combined_param_warnings);

    // Simulation runs all enabled toolpaths in sequence; the dexel
    // state carries from each prior toolpath into the measured one
    // because `stock_source: FromRemainingStock` is set on the latter.
    let cancel = AtomicBool::new(false);

    // Confirm non-empty toolpath
    let move_count = session
        .get_result(tp_idx)
        .map(|r| r.annotated().toolpath.moves.len())
        .unwrap_or(0);
    if move_count < 2 {
        return BaselineRow::failure(
            &case.case_id,
            op_type.kind_str(),
            "generation_empty",
            &format!(
                "toolpath has {move_count} moves; param_warnings={}",
                param_warnings.join("|")
            ),
        );
    }

    let sim_opts = SimulationOptions {
        resolution,
        skip_ids: Vec::new(),
        metrics_enabled: true,
        auto_resolution: false,
        // F-035: predicted-feed plumbing off — smoke baseline must
        // continue exercising commanded-feed gates so the regression
        // diff stays meaningful.
        use_predicted_feed_in_gates: false,
        adaptive_feed_modulation: false,
        modulation_strategy: rs_cam_core::feed_modulation::ModulationStrategy::ConstrainedMax,
        modulation_aggressiveness: 1.0,
    };
    if let Err(e) = session.run_simulation(&sim_opts, &cancel) {
        return BaselineRow::failure(
            &case.case_id,
            op_type.kind_str(),
            "simulation_failed",
            &format!("{e}; param_warnings={}", param_warnings.join("|")),
        );
    }

    // Extract verdicts.
    let report = session.tool_load_report();
    let toolpath_id_after_add = session
        .get_toolpath_config(tp_idx)
        .map(|tc| tc.id)
        .unwrap_or(0);
    let verdict = report
        .per_toolpath
        .iter()
        .find(|v| v.toolpath_id == toolpath_id_after_add);

    let sim_result = session.simulation_result();
    let summary = sim_result.and_then(|s| s.cut_trace.as_ref()).and_then(|t| {
        t.toolpath_summaries
            .iter()
            .find(|ts| ts.toolpath_id == toolpath_id_after_add)
            .cloned()
    });

    let diag = session.diagnostics();
    let rapid_collision_count = diag
        .per_toolpath
        .iter()
        .find(|d| d.toolpath_id == toolpath_id_after_add)
        .map_or(0_u32, |d| d.rapid_collision_count as u32);

    build_ok_row(
        &case.case_id,
        op_type,
        verdict,
        summary.as_ref(),
        rapid_collision_count,
        &param_warnings,
    )
}

fn build_ok_row(
    case_id: &str,
    op_type: OperationType,
    verdict: Option<&ToolpathLoadVerdict>,
    summary: Option<&rs_cam_core::simulation_cut::SimulationToolpathCutSummary>,
    rapid_collision_count: u32,
    param_warnings: &[String],
) -> BaselineRow {
    let (chipload_kind, chipload_observed) = match verdict.map(|v| &v.chipload) {
        Some(ChiploadVerdict::Within {
            approach_to_max, ..
        }) => (
            "within".to_owned(),
            format!("{:.6}", approach_to_max.observed_mm_per_tooth),
        ),
        Some(ChiploadVerdict::Exceeds {
            triggering, side, ..
        }) => (
            format!("exceeds_{}", side_str(side)),
            format!("{:.6}", triggering.observed_mm_per_tooth),
        ),
        Some(ChiploadVerdict::Unmodeled { reason }) => (
            format!("unmodeled_{:?}", reason).to_lowercase(),
            String::new(),
        ),
        None => ("missing".to_owned(), String::new()),
    };

    let (deflection_kind, deflection_peak) = match verdict.map(|v| &v.deflection) {
        Some(DeflectionVerdict::Within { peak_mm, .. }) => {
            ("within".to_owned(), format!("{:.4}", peak_mm))
        }
        Some(DeflectionVerdict::Exceeds { peak_mm, .. }) => {
            ("exceeds".to_owned(), format!("{:.4}", peak_mm))
        }
        Some(DeflectionVerdict::Unmodeled { reason }) => (
            format!("unmodeled_{:?}", reason).to_lowercase(),
            String::new(),
        ),
        None => ("missing".to_owned(), String::new()),
    };

    let (power_kind, power_peak) = match verdict.map(|v| &v.power) {
        Some(PowerVerdict::Within { peak_kw, .. }) => {
            ("within".to_owned(), format!("{:.4}", peak_kw))
        }
        Some(PowerVerdict::Exceeds { peak_kw, .. }) => {
            ("exceeds".to_owned(), format!("{:.4}", peak_kw))
        }
        Some(PowerVerdict::Unmodeled { reason }) => (
            format!("unmodeled_{:?}", reason).to_lowercase(),
            String::new(),
        ),
        None => ("missing".to_owned(), String::new()),
    };

    let avg_engagement = summary
        .map(|s| format!("{:.4}", s.average_engagement))
        .unwrap_or_default();
    let peak_axial_doc = summary
        .map(|s| format!("{:.3}", s.peak_axial_doc_mm))
        .unwrap_or_default();

    let (drill_chip_welding_kind, drill_chip_welding_observed, drill_peck_kind, drill_plunge_kind) =
        extract_drill_gates(verdict.and_then(|v| v.drill_gates.as_ref()));

    let notes = if param_warnings.is_empty() {
        String::new()
    } else {
        format!("param_warnings={}", param_warnings.join("|"))
    };

    BaselineRow {
        case_id: case_id.to_owned(),
        op_kind: op_type.kind_str().to_owned(),
        status: "ok".to_owned(),
        chipload_kind,
        chipload_observed_mm_tooth: chipload_observed,
        deflection_kind,
        deflection_peak_mm: deflection_peak,
        power_kind,
        power_peak_kw: power_peak,
        rapid_collision_count: rapid_collision_count.to_string(),
        avg_engagement,
        peak_axial_doc_mm: peak_axial_doc,
        drill_chip_welding_kind,
        drill_chip_welding_observed,
        drill_peck_kind,
        drill_plunge_kind,
        notes,
    }
}

fn side_str(side: &ChipSide) -> &'static str {
    match side {
        ChipSide::Low => "low",
        ChipSide::High => "high",
    }
}

fn extract_drill_gates(gates: Option<&DrillGatesVerdict>) -> (String, String, String, String) {
    use rs_cam_core::tool_load::drill_gates::DrillGateOutcome;
    fn outcome_kind(o: &DrillGateOutcome) -> String {
        match o {
            DrillGateOutcome::Within { .. } => "within".to_owned(),
            DrillGateOutcome::Exceeds { severity, .. } => {
                format!("exceeds_{}", format!("{severity:?}").to_lowercase())
            }
        }
    }
    let Some(g) = gates else {
        return (String::new(), String::new(), String::new(), String::new());
    };
    let chip_welding_kind = outcome_kind(&g.chip_welding);
    let chip_welding_observed = format!("{:.3}", g.chip_welding.observed());
    let peck_kind = outcome_kind(&g.peck_adequacy);
    let plunge_kind = outcome_kind(&g.plunge_feed);
    (
        chip_welding_kind,
        chip_welding_observed,
        peck_kind,
        plunge_kind,
    )
}

// ── Helpers ─────────────────────────────────────────────────────────────

fn load_cases(path: &Path) -> Result<Vec<SmokeCase>> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_path(path)
        .context(format!("failed to open {}", path.display()))?;
    let mut out = Vec::new();
    for (idx, result) in rdr.deserialize().enumerate() {
        let case: SmokeCase = result.context(format!("row {idx} parse failed"))?;
        out.push(case);
    }
    Ok(out)
}

fn read_baseline(path: &Path) -> Result<Vec<BaselineRow>> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_path(path)
        .context(format!("failed to open {}", path.display()))?;
    let mut out = Vec::new();
    for (idx, result) in rdr.deserialize().enumerate() {
        let row: BaselineRow = result.context(format!("baseline row {idx} parse failed"))?;
        out.push(row);
    }
    Ok(out)
}

fn write_baseline(path: &Path, rows: &[BaselineRow]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context(format!("creating {}", parent.display()))?;
    }
    let mut wtr = csv::Writer::from_path(path).context(format!("opening {}", path.display()))?;
    for row in rows {
        wtr.serialize(row).context("CSV serialize")?;
    }
    wtr.flush().context("CSV flush")?;
    Ok(())
}

fn parse_baseline_params(s: &str) -> Vec<(String, String)> {
    s.split(';')
        .filter_map(|kv| {
            let mut parts = kv.splitn(2, '=');
            let k = parts.next()?.trim();
            let v = parts.next()?.trim();
            if k.is_empty() {
                return None;
            }
            Some((k.to_owned(), v.to_owned()))
        })
        .collect()
}

fn parse_op_type(kind: &str) -> Option<OperationType> {
    for op in OperationType::ALL {
        if op.kind_str() == kind {
            return Some(*op);
        }
    }
    None
}

fn pick_tool(session: &ProjectSession, tool_name: &str) -> Option<usize> {
    if !tool_name.is_empty()
        && let Some(t) = session.tools().iter().find(|t| t.name == tool_name)
    {
        return Some(t.id.0);
    }
    session.tools().first().map(|t| t.id.0)
}

/// Route any `stock_*` prefixed params in `baseline_params` to
/// `session.stock_mut()` field mutations. The toolpath operation
/// schema rejects these as unknown params, but the test author's
/// intent is to constrain stock geometry — pre-CLI, round-09 set
/// stock_top_z via the project file directly.
///
/// Currently supports `stock_top_z=N` (sets stock top to absolute Z=N,
/// preserving `origin_z`, by adjusting `stock.z = N - origin_z`).
/// Also disables `auto_from_model` so subsequent re-derivation can't
/// undo the override. Unknown stock_* keys are logged but ignored.
fn apply_stock_overrides(session: &mut ProjectSession, baseline_params: &str, case_id: &str) {
    for (key, value) in parse_baseline_params(baseline_params) {
        if !key.starts_with("stock_") {
            continue;
        }
        match key.as_str() {
            "stock_top_z" => match value.parse::<f64>() {
                Ok(top_z) => {
                    let stock = session.stock_mut();
                    let new_z = top_z - stock.origin_z;
                    if new_z <= 0.0 {
                        info!(
                            case_id,
                            top_z,
                            origin_z = stock.origin_z,
                            "stock_top_z would yield non-positive thickness; ignored"
                        );
                        continue;
                    }
                    stock.z = new_z;
                    stock.auto_from_model = false;
                    info!(case_id, top_z, stock_z = stock.z, "applied stock_top_z");
                }
                Err(e) => {
                    info!(case_id, %value, "stock_top_z parse failed: {e}; ignored");
                }
            },
            other => {
                info!(case_id, key = other, %value, "unknown stock_* param; ignored");
            }
        }
    }
}

fn material_for_family(family: &str) -> Option<Material> {
    match family.trim().to_ascii_lowercase().as_str() {
        "hardwood" => Some(Material::SolidWood {
            species: WoodSpecies::GenericHardwood,
        }),
        "softwood" => Some(Material::SolidWood {
            species: WoodSpecies::GenericSoftwood,
        }),
        "mdf" => Some(Material::SheetGood {
            kind: SheetGoodKind::Mdf,
        }),
        "plywood" => Some(Material::Plywood {
            grade: PlywoodGrade::BalticBirch,
        }),
        "aluminum" | "aluminium" | "6061" | "6061_t6" | "6061-t6" => Some(Material::Aluminum {
            alloy: AluminumAlloy::Alloy6061T6,
        }),
        "7075" | "7075_t6" | "7075-t6" => Some(Material::Aluminum {
            alloy: AluminumAlloy::Alloy7075T6,
        }),
        _ => None,
    }
}
