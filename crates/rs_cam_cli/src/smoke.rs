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
    /// Dexel cell size (mm) the row was measured at — the `--resolution`
    /// argument of the run that produced it. Collision counts and
    /// engagement both move with it, so a row cut at 0.5 and a row cut at
    /// 0.25 are not comparable; without this column `run_diff` had no way
    /// to know. `#[serde(default)]` keeps pre-2026-08-21 baselines
    /// readable, where it deserialises empty = "cell size unrecorded".
    #[serde(default)]
    pub resolution_mm: String,
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
            // Stamped by `run_single_case` on the way out, so every exit
            // path records the cell size it ran at.
            resolution_mm: String::new(),
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

/// Outcome of a baseline diff. `regressions` drives the exit status;
/// `changes` is the non-failing channel — movement that is legitimate (or
/// at least not a regression) but must not pass unremarked.
#[derive(Debug, Default)]
pub struct DiffOutcome {
    pub regressions: Vec<String>,
    pub changes: Vec<String>,
}

/// Diff two baseline CSVs. Returns Ok(true) if any regression detected.
pub fn run_diff(baseline_path: &Path, current_path: &Path) -> Result<bool> {
    let baseline = read_baseline(baseline_path)?;
    let current = read_baseline(current_path)?;

    let outcome = diff_rows(&baseline, &current);

    // Cases new to current run are informational, not regressions.
    if !outcome.changes.is_empty() {
        println!(
            "smoke-diff: {} change(s) [not regressions]",
            outcome.changes.len()
        );
        for c in &outcome.changes {
            println!("  ~ {c}");
        }
    }
    if outcome.regressions.is_empty() {
        println!(
            "smoke-diff: no regressions ({} cases checked)",
            baseline.len()
        );
        Ok(false)
    } else {
        println!("smoke-diff: {} regression(s)", outcome.regressions.len());
        for r in &outcome.regressions {
            println!("  - {r}");
        }
        Ok(true)
    }
}

/// The whole diff policy, file-free so it can be unit-tested against
/// hand-built rows. Every column of `BaselineRow` is either compared here
/// or explicitly excluded (`notes` — `param_warnings` churn is noise).
fn diff_rows(baseline: &[BaselineRow], current: &[BaselineRow]) -> DiffOutcome {
    let base_by_case: BTreeMap<String, &BaselineRow> =
        baseline.iter().map(|r| (r.case_id.clone(), r)).collect();
    let curr_by_case: BTreeMap<String, &BaselineRow> =
        current.iter().map(|r| (r.case_id.clone(), r)).collect();

    let mut regressions: Vec<String> = Vec::new();
    // Non-failing channel: movement that is not a regression but must not
    // pass unremarked. Exit status is driven by `regressions` only.
    let mut changes: Vec<String> = Vec::new();

    for (case_id, base_row) in &base_by_case {
        let Some(curr_row) = curr_by_case.get(case_id) else {
            regressions.push(format!("{case_id}: missing from current run"));
            continue;
        };
        // Cell size is a property of the INSTRUMENT, not the machining.
        // Collision counts and engagement both move with it, so a row cut
        // at 0.5 and a row cut at 0.25 are not comparable — and before the
        // column existed there was no way to notice. Reported loudly, but
        // not as a regression: changing the cell size is a deliberate act.
        match (base_row.resolution_mm.trim(), curr_row.resolution_mm.trim()) {
            (b, c) if !b.is_empty() && !c.is_empty() && b != c => changes.push(format!(
                "{case_id}: !! RESOLUTION MISMATCH {b} mm → {c} mm — \
                 these rows were measured on different grids"
            )),
            ("", c) if !c.is_empty() => changes.push(format!(
                "{case_id}: baseline predates the resolution column; \
                 current run was {c} mm — comparability is ASSUMED, not checked"
            )),
            _ => {}
        }
        // The join key is `case_id`; if the operation family behind it moved,
        // every remaining column is comparing two different measurements.
        if base_row.op_kind != curr_row.op_kind {
            regressions.push(format!(
                "{case_id}: op_kind {} → {} (rows are not comparable; re-cut the baseline)",
                base_row.op_kind, curr_row.op_kind
            ));
        }
        // Status regression: ok → anything else.
        if base_row.status == "ok" && curr_row.status != "ok" {
            regressions.push(format!(
                "{case_id}: status {} → {}",
                base_row.status, curr_row.status
            ));
            continue;
        }
        // A slide between two failure classes (generation_failed →
        // harness_error) is not a regression but is worth saying out loud.
        if base_row.status != "ok" && base_row.status != curr_row.status {
            changes.push(format!(
                "{case_id}: status {} → {}",
                base_row.status, curr_row.status
            ));
        }

        // ── Verdict columns ────────────────────────────────────────────
        // One table, every emitter. `is_exceeds` covers the bare spelling
        // (deflection, power), `exceeds_low`/`exceeds_high` (chipload) and
        // `exceeds_elevated`/`exceeds_critical` (drill gates).
        for (label, base_kind, curr_kind) in verdict_columns(base_row, curr_row) {
            if is_within(base_kind) && is_exceeds(curr_kind) {
                regressions.push(format!("{case_id}: {label} {base_kind} → {curr_kind}"));
            } else if went_blind(base_kind, curr_kind) {
                regressions.push(format!(
                    "{case_id}: {label} STOPPED BEING MEASURED {base_kind} → {curr_kind}"
                ));
            } else if base_kind != curr_kind {
                changes.push(format!("{case_id}: {label} {base_kind} → {curr_kind}"));
            }
        }

        // ── Rapid collisions ───────────────────────────────────────────
        // An increase is a regression; a decrease is an improvement that
        // still has to be reported, because "211 → 0" passing unremarked is
        // exactly how a baseline drifts away from its instrument.
        match (
            parse_count(&base_row.rapid_collision_count),
            parse_count(&curr_row.rapid_collision_count),
        ) {
            (Some(base_collisions), Some(curr_collisions)) => {
                if curr_collisions > base_collisions {
                    regressions.push(format!(
                        "{case_id}: rapid_collisions {base_collisions} → {curr_collisions}"
                    ));
                } else if curr_collisions < base_collisions {
                    changes.push(format!(
                        "{case_id}: rapid_collisions {base_collisions} → {curr_collisions} (decrease)"
                    ));
                }
            }
            (Some(base_collisions), None) => {
                // Was counted, no longer is: the count went blind. The old
                // `parse().unwrap_or(0)` read this as "zero collisions".
                regressions.push(format!(
                    "{case_id}: rapid_collisions STOPPED BEING COUNTED {base_collisions} → {:?}",
                    curr_row.rapid_collision_count
                ));
            }
            (None, _) => {}
        }

        // ── Numeric drift (informational) ──────────────────────────────
        for (label, base_val, curr_val) in numeric_columns(base_row, curr_row) {
            if let Some(msg) = numeric_drift(label, base_val, curr_val) {
                changes.push(format!("{case_id}: {msg}"));
            }
        }
    }

    DiffOutcome {
        regressions,
        changes,
    }
}

/// Every verdict column on the row, paired base-vs-current. Adding a gate
/// to `BaselineRow` means adding it here — that is the point of the table.
fn verdict_columns<'a>(
    base: &'a BaselineRow,
    curr: &'a BaselineRow,
) -> [(&'static str, &'a str, &'a str); 6] {
    [
        ("chipload", &base.chipload_kind, &curr.chipload_kind),
        ("deflection", &base.deflection_kind, &curr.deflection_kind),
        ("power", &base.power_kind, &curr.power_kind),
        (
            "drill_chip_welding",
            &base.drill_chip_welding_kind,
            &curr.drill_chip_welding_kind,
        ),
        ("drill_peck", &base.drill_peck_kind, &curr.drill_peck_kind),
        (
            "drill_plunge",
            &base.drill_plunge_kind,
            &curr.drill_plunge_kind,
        ),
    ]
}

/// Every measured numeric on the row. These feed the non-failing drift
/// channel: they move for legitimate reasons (a shortened rapid, a
/// re-framed grid) far too often to gate CI on, but silence is worse.
fn numeric_columns<'a>(
    base: &'a BaselineRow,
    curr: &'a BaselineRow,
) -> [(&'static str, &'a str, &'a str); 6] {
    [
        (
            "chipload_observed_mm_tooth",
            &base.chipload_observed_mm_tooth,
            &curr.chipload_observed_mm_tooth,
        ),
        (
            "deflection_peak_mm",
            &base.deflection_peak_mm,
            &curr.deflection_peak_mm,
        ),
        ("power_peak_kw", &base.power_peak_kw, &curr.power_peak_kw),
        ("avg_engagement", &base.avg_engagement, &curr.avg_engagement),
        (
            "peak_axial_doc_mm",
            &base.peak_axial_doc_mm,
            &curr.peak_axial_doc_mm,
        ),
        (
            "drill_chip_welding_observed",
            &base.drill_chip_welding_observed,
            &curr.drill_chip_welding_observed,
        ),
    ]
}

/// Relative tolerance for the numeric drift channel. 5 % is loose enough
/// that formatting noise on a 4-decimal column stays quiet.
const NUMERIC_DRIFT_REL_TOL: f64 = 0.05;

/// Describe how one numeric column moved, or `None` if it did not move
/// meaningfully. Empty ↔ populated transitions are reported: a column that
/// stops being written is the numeric face of `went_blind`.
fn numeric_drift(label: &str, base: &str, curr: &str) -> Option<String> {
    let b = base.trim();
    let c = curr.trim();
    match (b.parse::<f64>().ok(), c.parse::<f64>().ok()) {
        (Some(bv), Some(cv)) => {
            let moved = if bv == 0.0 {
                cv != 0.0
            } else {
                ((cv - bv) / bv).abs() > NUMERIC_DRIFT_REL_TOL
            };
            if !moved {
                return None;
            }
            let factor = if bv == 0.0 {
                String::new()
            } else {
                format!(" ({:.2}×)", cv / bv)
            };
            Some(format!("{label} {bv} → {cv}{factor}"))
        }
        (Some(bv), None) if c.is_empty() => Some(format!("{label} {bv} → <empty> (not written)")),
        (None, Some(cv)) if b.is_empty() => Some(format!("{label} <empty> → {cv} (now written)")),
        _ => None,
    }
}

/// `rapid_collision_count` as a number, or `None` when the cell is empty
/// or unparseable. The distinction matters: the pre-2026-08-21 diff used
/// `parse().unwrap_or(0)`, which read an unwritten cell as a clean zero.
fn parse_count(s: &str) -> Option<u32> {
    s.trim().parse::<u32>().ok()
}

fn is_within(kind: &str) -> bool {
    kind == "within"
}

/// All four verdict emitters spell exceedance differently: bare `exceeds`
/// (deflection, power), `exceeds_low`/`exceeds_high` (chipload,
/// `build_ok_row`), `exceeds_elevated`/`exceeds_critical` (drill gates,
/// `extract_drill_gates`). One prefix covers the set.
fn is_exceeds(kind: &str) -> bool {
    kind.starts_with("exceeds")
}

/// A verdict that stopped being measured. `within → unmodeled_* / missing`
/// is a regression of the INSTRUMENT even when the machining is fine — the
/// "empty population passes and looks healthy" class.
fn went_blind(base: &str, curr: &str) -> bool {
    is_within(base) && (curr.starts_with("unmodeled") || curr == "missing" || curr.is_empty())
}

// ── Case execution ──────────────────────────────────────────────────────

/// Materializes one case's toolpath into an existing session: resolves
/// op + tool, calls `suggest_params`, adds the `ToolpathConfig` with the
/// caller-supplied `stock_source`, and applies the case's baseline_params.
/// Used by `run_single_case` for both prior passes (`StockSource::Fresh`)
/// and the measured case (`StockSource::FromRemainingStock` when chaining,
/// else `Fresh`).
///
/// `generate` controls whether the toolpath is generated in the same call.
/// It must be `false` for a `FromRemainingStock` measured case: that op
/// cannot generate until a simulation has keyed a `prior_stocks` snapshot
/// to its id, and the simulation cannot key one until the op EXISTS. See
/// `run_single_case` for the add → simulate → generate order that resolves
/// the catch-22.
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
    generate: bool,
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
        id: rs_cam_core::ToolpathId(0),
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
        rest_analysis: rs_cam_core::compute::config::RestAnalysisConfig::default(),
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

    if generate {
        let cancel = AtomicBool::new(false);
        if let Err(e) = session.generate_toolpath(tp_idx, &cancel) {
            return Err(BaselineRow::failure(
                &case.case_id,
                op_type.kind_str(),
                "generation_failed",
                &format!("{e}; param_warnings={}", param_warnings.join("|")),
            ));
        }
    }

    Ok((tp_idx, op_type, param_warnings))
}

/// Run one case and stamp the cell size it ran at onto whatever row comes
/// back — success or any of the failure classes. A baseline row without its
/// resolution is a measurement without its instrument setting.
fn run_single_case(case: &SmokeCase, all_cases: &[SmokeCase], resolution: f64) -> BaselineRow {
    let mut row = run_single_case_inner(case, all_cases, resolution);
    row.resolution_mm = format!("{resolution}");
    row
}

fn run_single_case_inner(
    case: &SmokeCase,
    all_cases: &[SmokeCase],
    resolution: f64,
) -> BaselineRow {
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
            // Prior passes cut fresh stock: nothing blocks their generate.
            true,
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

    let chained = !prior_ids.is_empty();
    let measured_stock_source = if chained {
        rs_cam_core::compute::config::StockSource::FromRemainingStock
    } else {
        rs_cam_core::compute::config::StockSource::Fresh
    };

    let cancel = AtomicBool::new(false);

    // ONE `SimulationOptions` value serves both the priming pass and the
    // measurement pass, so the two dexel grids are the same grid. Hoisted
    // above the measured-case materialisation for exactly that reason.
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

    // ADD the measured toolpath; generate it here only when it cuts fresh
    // stock. A `FromRemainingStock` op cannot generate yet — see below.
    let (tp_idx, op_type, mut param_warnings) = match materialize_case_toolpath(
        &mut session,
        case,
        measured_stock_source,
        "smoke",
        /* generate = */ !chained,
    ) {
        Ok(triple) => triple,
        Err(row) => return row,
    };

    param_warnings.extend(combined_param_warnings);

    if chained {
        // The catch-22 this order resolves (AS015 had been
        // `generation_failed` since 4b105dab made the fresh-stock fallback
        // fail-hard): `generate_toolpath` refuses a `FromRemainingStock` op
        // unless `simulation.prior_stocks` holds a snapshot keyed to THAT
        // op's id, and `run_simulation` only keys one to an op that already
        // exists in the plan. Simulating before the op is added therefore
        // fixes nothing.
        //
        // The core already has the mechanism: `PhantomPriorStockScan`
        // (`compute/simulate.rs`) locks onto the first
        // enabled-but-ungenerated config in plan order and, when it is
        // `FromRemainingStock`, takes its snapshot at that plan position.
        // So the order must be add → simulate → generate. Chain depth is
        // always 1 here — `prior_passes` is a flat list of ids, never
        // transitive — so one priming pass is sufficient and no fixpoint
        // loop is needed.
        if let Err(e) = session.run_simulation(&sim_opts, &cancel) {
            return BaselineRow::failure(
                &case.case_id,
                op_type.kind_str(),
                "simulation_failed",
                &format!(
                    "priming sim for prior-stock chain: {e}; param_warnings={}",
                    param_warnings.join("|")
                ),
            );
        }
        if let Err(e) = session.generate_toolpath(tp_idx, &cancel) {
            return BaselineRow::failure(
                &case.case_id,
                op_type.kind_str(),
                "generation_failed",
                &format!("{e}; param_warnings={}", param_warnings.join("|")),
            );
        }
    }

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

    // The measurement pass. Simulation runs all enabled toolpaths in
    // sequence; the dexel state carries from each prior toolpath into the
    // measured one because `stock_source: FromRemainingStock` is set on
    // the latter.
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
        .unwrap_or(rs_cam_core::ToolpathId(0));
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
        resolution_mm: String::new(),
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

// ── Diff-policy tests ───────────────────────────────────────────────────
//
// The five rows below are the exact probe from
// `planning/perf_review_2026-08-19/RESEARCH_corpus_instruments.md` §3.2,
// which the pre-2026-08-21 `run_diff` scored **1 of 5**: only X03's bare
// `exceeds` (deflection) matched `is_exceeds`, so a chipload flip, three
// drill gates, a gate that went blind, a collapsed engagement, a 990×
// peak and a changed `op_kind` were all silent.

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;

    /// A row with everything measured and `within`, as the starting point
    /// for injecting one deliberate regression at a time.
    fn clean_row(case_id: &str, op_kind: &str) -> BaselineRow {
        BaselineRow {
            case_id: case_id.to_owned(),
            op_kind: op_kind.to_owned(),
            status: "ok".to_owned(),
            resolution_mm: "0.5".to_owned(),
            chipload_kind: "within".to_owned(),
            chipload_observed_mm_tooth: "0.010000".to_owned(),
            deflection_kind: "within".to_owned(),
            deflection_peak_mm: "0.0100".to_owned(),
            power_kind: "within".to_owned(),
            power_peak_kw: "0.0100".to_owned(),
            rapid_collision_count: "0".to_owned(),
            avg_engagement: "0.3000".to_owned(),
            peak_axial_doc_mm: "2.000".to_owned(),
            drill_chip_welding_kind: "within".to_owned(),
            drill_chip_welding_observed: "4.000".to_owned(),
            drill_peck_kind: "within".to_owned(),
            drill_plunge_kind: "within".to_owned(),
            notes: String::new(),
        }
    }

    fn regressions_for(base: BaselineRow, curr: BaselineRow) -> Vec<String> {
        diff_rows(&[base], &[curr]).regressions
    }

    fn changes_for(base: BaselineRow, curr: BaselineRow) -> Vec<String> {
        diff_rows(&[base], &[curr]).changes
    }

    #[test]
    fn x01_chipload_within_to_exceeds_high_is_a_regression() {
        // The vacuous arm: `is_exceeds` used to be `== "exceeds"`, which
        // `build_ok_row` never emits for chipload.
        let base = clean_row("X01", "pocket");
        let mut curr = clean_row("X01", "pocket");
        curr.chipload_kind = "exceeds_high".to_owned();
        curr.chipload_observed_mm_tooth = "0.500000".to_owned();

        let regressions = regressions_for(base, curr);
        assert!(
            regressions
                .iter()
                .any(|r| r.contains("chipload within → exceeds_high")),
            "chipload flip not flagged: {regressions:?}"
        );
    }

    #[test]
    fn x02_all_three_drill_gates_are_diffed() {
        // Drill gates spell exceedance `exceeds_elevated` /
        // `exceeds_critical` and were not read at all.
        let base = clean_row("X02", "drill");
        let mut curr = clean_row("X02", "drill");
        curr.drill_chip_welding_kind = "exceeds_critical".to_owned();
        curr.drill_chip_welding_observed = "99.000".to_owned();
        curr.drill_peck_kind = "exceeds_critical".to_owned();
        curr.drill_plunge_kind = "exceeds_critical".to_owned();

        let regressions = regressions_for(base, curr);
        for gate in ["drill_chip_welding", "drill_peck", "drill_plunge"] {
            assert!(
                regressions
                    .iter()
                    .any(|r| r.contains(gate) && r.contains("exceeds_critical")),
                "{gate} not flagged: {regressions:?}"
            );
        }
    }

    #[test]
    fn x03_deflection_positive_control_still_flags() {
        // The one transition the old net caught; it must keep working.
        let base = clean_row("X03", "profile");
        let mut curr = clean_row("X03", "profile");
        curr.deflection_kind = "exceeds".to_owned();

        let regressions = regressions_for(base, curr);
        assert!(
            regressions
                .iter()
                .any(|r| r.contains("deflection within → exceeds")),
            "positive control lost: {regressions:?}"
        );
    }

    #[test]
    fn x04_a_gate_that_stops_measuring_is_a_regression() {
        // `within → unmodeled_*` — the "empty population passes and looks
        // healthy" class. Not an exceedance, still an instrument failure.
        let base = clean_row("X04", "zigzag");
        let mut curr = clean_row("X04", "zigzag");
        curr.chipload_kind = "unmodeled_novendordata".to_owned();
        curr.chipload_observed_mm_tooth = String::new();

        let regressions = regressions_for(base, curr);
        assert!(
            regressions
                .iter()
                .any(|r| r.contains("chipload STOPPED BEING MEASURED")),
            "blind chipload gate not flagged: {regressions:?}"
        );
    }

    #[test]
    fn x05_op_kind_change_invalidates_the_join() {
        let base = clean_row("X05", "zigzag");
        let mut curr = clean_row("X05", "pocket");
        curr.deflection_peak_mm = "9.9000".to_owned();
        curr.power_peak_kw = "9.9000".to_owned();

        let outcome = diff_rows(&[base], &[curr]);
        assert!(
            outcome
                .regressions
                .iter()
                .any(|r| r.contains("op_kind zigzag → pocket")),
            "op_kind change not flagged: {:?}",
            outcome.regressions
        );
        // The 990× numeric moves land on the non-failing channel.
        assert!(
            outcome
                .changes
                .iter()
                .any(|c| c.contains("deflection_peak_mm")),
            "deflection drift not reported: {:?}",
            outcome.changes
        );
    }

    #[test]
    fn collision_decrease_is_reported_but_does_not_fail() {
        // The 211 → 0 case: an improvement nobody was told about.
        let mut base = clean_row("AS010", "inlay");
        base.rapid_collision_count = "104".to_owned();
        let curr = clean_row("AS010", "inlay");

        let outcome = diff_rows(&[base], &[curr]);
        assert!(
            outcome.regressions.is_empty(),
            "a collision DECREASE must not fail the diff: {:?}",
            outcome.regressions
        );
        assert!(
            outcome
                .changes
                .iter()
                .any(|c| c.contains("rapid_collisions 104 → 0")),
            "collision decrease not reported: {:?}",
            outcome.changes
        );
    }

    #[test]
    fn collision_increase_is_still_a_regression() {
        let base = clean_row("AS010", "inlay");
        let mut curr = clean_row("AS010", "inlay");
        curr.rapid_collision_count = "7".to_owned();

        let regressions = regressions_for(base, curr);
        assert!(
            regressions
                .iter()
                .any(|r| r.contains("rapid_collisions 0 → 7")),
            "collision increase not flagged: {regressions:?}"
        );
    }

    #[test]
    fn an_uncounted_collision_cell_is_not_read_as_zero() {
        // `parse().unwrap_or(0)` used to turn an empty cell into a clean
        // zero — a count that stopped being taken looked like a fix.
        let mut base = clean_row("AS017", "horizontal_finish");
        base.rapid_collision_count = "100".to_owned();
        let mut curr = clean_row("AS017", "horizontal_finish");
        curr.rapid_collision_count = String::new();

        let regressions = regressions_for(base, curr);
        assert!(
            regressions
                .iter()
                .any(|r| r.contains("STOPPED BEING COUNTED")),
            "unreadable collision cell not flagged: {regressions:?}"
        );
    }

    #[test]
    fn an_unchanged_row_is_silent_on_both_channels() {
        let outcome = diff_rows(
            &[clean_row("AS001", "pocket")],
            &[clean_row("AS001", "pocket")],
        );
        assert!(outcome.regressions.is_empty(), "{:?}", outcome.regressions);
        assert!(outcome.changes.is_empty(), "{:?}", outcome.changes);
    }

    #[test]
    fn numeric_drift_is_relative_and_never_a_regression() {
        // Within tolerance: silent.
        assert!(numeric_drift("x", "1.0000", "1.0200").is_none());
        // Outside it: reported, with the ratio spelled out.
        let msg = numeric_drift("peak_axial_doc_mm", "14.766", "3.743").unwrap();
        assert!(msg.contains("0.25×"), "{msg}");
        // Zero base: any movement is movement.
        assert!(numeric_drift("x", "0.0000", "0.0001").is_some());
        assert!(numeric_drift("x", "0.0000", "0.0000").is_none());
        // Stopped being written.
        assert!(
            numeric_drift("x", "0.3000", "")
                .unwrap()
                .contains("not written")
        );
    }

    #[test]
    fn exceeds_predicate_covers_every_emitted_spelling() {
        for kind in [
            "exceeds",
            "exceeds_low",
            "exceeds_high",
            "exceeds_elevated",
            "exceeds_critical",
        ] {
            assert!(is_exceeds(kind), "{kind} not recognised as an exceedance");
        }
        for kind in ["within", "unmodeled_novendordata", "missing", ""] {
            assert!(!is_exceeds(kind), "{kind} wrongly read as an exceedance");
        }
    }

    #[test]
    fn a_recovered_verdict_is_not_a_regression() {
        // unmodeled → within, and non-ok → ok, are improvements.
        let mut base = clean_row("X06", "pocket");
        base.chipload_kind = "unmodeled_novendordata".to_owned();
        base.status = "generation_failed".to_owned();
        let curr = clean_row("X06", "pocket");

        let regressions = regressions_for(base, curr);
        assert!(regressions.is_empty(), "{regressions:?}");
    }

    #[test]
    fn a_resolution_change_is_reported_but_does_not_fail() {
        let base = clean_row("X08", "pocket");
        let mut curr = clean_row("X08", "pocket");
        curr.resolution_mm = "0.25".to_owned();

        let outcome = diff_rows(&[base], &[curr]);
        assert!(outcome.regressions.is_empty(), "{:?}", outcome.regressions);
        assert!(
            outcome
                .changes
                .iter()
                .any(|c| c.contains("RESOLUTION MISMATCH")),
            "cell-size change not reported: {:?}",
            outcome.changes
        );
    }

    #[test]
    fn a_baseline_without_the_resolution_column_says_so() {
        // Every checked-in baseline through 2026-06-04 predates the column;
        // its cell size is unrecorded, so comparability is assumed.
        let mut base = clean_row("AS001", "pocket");
        base.resolution_mm = String::new();
        let curr = clean_row("AS001", "pocket");

        let outcome = diff_rows(&[base], &[curr]);
        assert!(outcome.regressions.is_empty(), "{:?}", outcome.regressions);
        assert!(
            outcome
                .changes
                .iter()
                .any(|c| c.contains("predates the resolution column")),
            "missing-column case not reported: {:?}",
            outcome.changes
        );
    }

    #[test]
    fn a_failure_class_slide_is_reported_on_the_change_channel() {
        let mut base = clean_row("X07", "scallop");
        base.status = "generation_failed".to_owned();
        let mut curr = clean_row("X07", "scallop");
        curr.status = "harness_error".to_owned();

        let changes = changes_for(base, curr);
        assert!(
            changes
                .iter()
                .any(|c| c.contains("status generation_failed → harness_error")),
            "failure-class slide not reported: {changes:?}"
        );
    }
}
