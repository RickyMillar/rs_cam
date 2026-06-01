//! GUI project file (format_version=3) diagnostic executor.
//!
//! Loads the project TOML via [`ProjectSession`], executes all enabled
//! toolpaths, runs tri-dexel simulation with cut metrics, checks
//! collisions, and writes structured JSON diagnostics.

use anyhow::{Context, Result};
use serde::Serialize;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use tracing::{debug, info, warn};

use rs_cam_core::session::{ProjectSession, SimulationOptions};
use rs_cam_core::simulation_cut::SimulationCutArtifact;

// ── JSON output types ───────────────────────────────────────────────────

#[derive(Serialize)]
struct ToolpathDiagnostic {
    toolpath_id: usize,
    toolpath_name: String,
    operation_type: String,
    tool: String,
    move_count: usize,
    cutting_distance_mm: f64,
    rapid_distance_mm: f64,
    debug_trace: Option<rs_cam_core::debug_trace::ToolpathDebugTrace>,
    semantic_trace: Option<rs_cam_core::semantic_trace::ToolpathSemanticTrace>,
    collision_count: usize,
    rapid_collision_count: usize,
    min_safe_stickout: Option<f64>,
}

#[derive(Serialize)]
struct ToolpathSummaryEntry {
    id: usize,
    name: String,
    operation: String,
    status: String,
    move_count: usize,
    collision_count: usize,
}

#[derive(Serialize)]
struct ProjectSummary {
    project: String,
    setup_count: usize,
    toolpath_count: usize,
    total_cutting_distance_mm: f64,
    total_rapid_distance_mm: f64,
    total_runtime_s: f64,
    air_cut_percentage: f64,
    average_engagement: f64,
    collision_count: usize,
    rapid_collision_count: usize,
    per_toolpath: Vec<ToolpathSummaryEntry>,
    verdict: String,
}

// ── Main entry point ────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub fn run_project_command(
    input: &Path,
    output_dir: &Path,
    setup_filter: Option<&str>,
    skip_ids: &[usize],
    resolution: f64,
    summary: bool,
    emit_gcode: Option<&Path>,
    adaptive_feed_modulation: bool,
    modulation_strategy: rs_cam_core::feed_modulation::ModulationStrategy,
    modulation_aggressiveness: f64,
    inject_shapeoko_kinematics: bool,
    apply_suggest: bool,
    spindle_strategy_override: Option<rs_cam_core::feeds::SpindleStrategy>,
) -> Result<()> {
    // 1. Load project into a session
    let project_path = input
        .canonicalize()
        .context(format!("Project file not found: {}", input.display()))?;
    let mut session =
        ProjectSession::load(&project_path).context("Failed to load project session")?;

    // F-036c calibration helper: project TOMLs predating F-034 carry
    // no `kinematics` block, so the modulator + kinematic integrator
    // are both no-ops. Inject the Shapeoko XXL preset so the user can
    // compare wall-clock against the integrator's prediction and
    // exercise modulation end-to-end.
    if inject_shapeoko_kinematics {
        session.machine_mut().kinematics =
            Some(rs_cam_core::machine_kinematics::MachineKinematics::shapeoko_xxl_stock());
        info!("Injected Shapeoko XXL stock kinematics into MachineProfile");
    }

    if let Some(strategy) = spindle_strategy_override {
        let prev = session.post_config().spindle_strategy;
        session.post_mut().spindle_strategy = strategy;
        info!(
            previous = ?prev,
            applied = ?strategy,
            "Overrode project spindle policy from CLI flag"
        );
    }

    info!(
        name = %session.name(),
        tools = session.list_tools().len(),
        models_loaded = true,
        setups = session.setup_count(),
        "Loaded project"
    );

    // 2. Map setup_filter to additional skip IDs
    let mut combined_skip: Vec<usize> = skip_ids.to_vec();
    if let Some(filter) = setup_filter {
        for setup in session.list_setups() {
            let matches = setup.name == filter || setup.id.to_string() == filter;
            if !matches {
                // Gather toolpath IDs from non-matching setups
                for &tp_idx in &setup.toolpath_indices {
                    if let Some(tc) = session.get_toolpath_config(tp_idx) {
                        combined_skip.push(tc.id);
                    }
                }
                debug!(setup = %setup.name, "Skipping setup (filter)");
            }
        }
    }

    // 2b. Optionally apply LUT-suggested feeds/speeds to every
    // enabled toolpath before generation. Replaces feed_rate /
    // plunge_rate / stepover / depth_per_pass via
    // `apply_feeds_result_to_op` and writes spindle_rpm from the
    // suggest result. Mutates the in-memory session only; the
    // project TOML on disk stays unchanged.
    if apply_suggest {
        apply_suggested_feeds_to_session(&mut session)?;
    }

    // 3. Generate all toolpaths
    let cancel = AtomicBool::new(false);
    session.generate_all(&combined_skip, &cancel)?;

    // 4. Run simulation
    let sim_opts = SimulationOptions {
        resolution,
        skip_ids: combined_skip.clone(),
        metrics_enabled: true,
        auto_resolution: false,
        // F-035: predicted-feed plumbing off by default for CLI runs;
        // protects the smoke baseline from spurious verdict drift.
        use_predicted_feed_in_gates: false,
        adaptive_feed_modulation,
        modulation_strategy,
        modulation_aggressiveness,
    };
    session.run_simulation(&sim_opts, &cancel)?;

    // 5. Run collision checks per toolpath and collect results
    let tp_count = session.toolpath_count();
    let mut collision_reports: std::collections::HashMap<
        usize,
        rs_cam_core::collision::CollisionReport,
    > = std::collections::HashMap::new();

    for idx in 0..tp_count {
        if session.get_result(idx).is_none() {
            continue;
        }
        let tp_id = session
            .get_toolpath_config(idx)
            .map(|tc| tc.id)
            .unwrap_or(idx);
        match session.collision_check(idx, &cancel) {
            Ok(check) => {
                if !check.collision_report.is_clear() {
                    collision_reports.insert(tp_id, check.collision_report);
                }
            }
            Err(rs_cam_core::session::SessionError::MissingGeometry(_)) => {
                // 2D ops don't have meshes for collision checking — that's expected
            }
            Err(e) => {
                warn!(index = idx, error = %e, "Collision check failed");
            }
        }
    }

    // 6. Create output directory
    std::fs::create_dir_all(output_dir).context(format!(
        "Failed to create output dir: {}",
        output_dir.display()
    ))?;

    // 7. Write per-toolpath JSON
    let stock_bbox = session.stock_bbox();
    let diag = session.diagnostics();

    for idx in 0..tp_count {
        let Some(result) = session.get_result(idx) else {
            continue;
        };
        let Some(tc) = session.get_toolpath_config(idx) else {
            continue;
        };

        let tool_name = session
            .get_tool(rs_cam_core::compute::tool_config::ToolId(tc.tool_id))
            .map(|t| t.name.clone())
            .unwrap_or_default();

        let col_report = collision_reports.get(&tc.id);
        let collision_count = col_report.map(|r| r.collisions.len()).unwrap_or(0);
        let min_safe = col_report.map(|r| r.min_safe_stickout);

        let rapid_count = diag
            .per_toolpath
            .iter()
            .find(|d| d.toolpath_id == tc.id)
            .map_or(0, |d| d.rapid_collision_count);

        let diagnostic = ToolpathDiagnostic {
            toolpath_id: tc.id,
            toolpath_name: tc.name.clone(),
            operation_type: tc.operation.label().to_owned(),
            tool: tool_name,
            move_count: result.stats.move_count,
            cutting_distance_mm: result.stats.cutting_distance,
            rapid_distance_mm: result.stats.rapid_distance,
            debug_trace: result.debug_trace.clone(),
            semantic_trace: result.semantic_trace.clone(),
            collision_count,
            rapid_collision_count: rapid_count,
            min_safe_stickout: min_safe,
        };

        let file_name = format!(
            "tp_{}_{}.json",
            tc.id,
            tc.name.replace(
                |c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '_',
                "_"
            )
        );
        let file_path = output_dir.join(file_name);
        let json = serde_json::to_string_pretty(&diagnostic)
            .context("Failed to serialize toolpath diagnostic")?;
        std::fs::write(&file_path, json)
            .context(format!("Failed to write {}", file_path.display()))?;
        debug!(path = %file_path.display(), "Wrote toolpath diagnostic");
    }

    // 8. Write simulation.json
    if let Some(sim_result) = session.simulation_result()
        && let Some(trace) = &sim_result.cut_trace
    {
        let included_ids: Vec<usize> = (0..tp_count)
            .filter(|idx| session.get_result(*idx).is_some())
            .filter_map(|idx| session.get_toolpath_config(idx).map(|tc| tc.id))
            .collect();

        let sim_artifact = SimulationCutArtifact::new(
            resolution,
            resolution.max(0.25),
            [stock_bbox.min.x, stock_bbox.min.y, stock_bbox.min.z],
            [stock_bbox.max.x, stock_bbox.max.y, stock_bbox.max.z],
            included_ids,
            serde_json::json!({ "project": session.name() }),
            trace.as_ref().clone(),
        );

        let sim_path = output_dir.join("simulation.json");
        let sim_json = serde_json::to_string_pretty(&sim_artifact)
            .context("Failed to serialize simulation")?;
        std::fs::write(&sim_path, sim_json)
            .context(format!("Failed to write {}", sim_path.display()))?;
        info!(path = %sim_path.display(), "Wrote simulation artifact");
    }

    // 9. Write summary.json
    let total_cutting: f64 = diag
        .per_toolpath
        .iter()
        .map(|d| d.cutting_distance_mm)
        .sum();
    let total_rapid: f64 = diag.per_toolpath.iter().map(|d| d.rapid_distance_mm).sum();

    // Gather collision counts including holder checks
    let total_collision_count: usize = collision_reports.values().map(|r| r.collisions.len()).sum();

    let per_toolpath: Vec<ToolpathSummaryEntry> = diag
        .per_toolpath
        .iter()
        .map(|d| {
            let holder_collisions = collision_reports
                .get(&d.toolpath_id)
                .map(|r| r.collisions.len())
                .unwrap_or(0);
            let total_collisions = holder_collisions + d.rapid_collision_count;
            let status = if total_collisions > 0 { "error" } else { "ok" };
            ToolpathSummaryEntry {
                id: d.toolpath_id,
                name: d.name.clone(),
                operation: d.operation_type.clone(),
                status: status.to_owned(),
                move_count: d.move_count,
                collision_count: total_collisions,
            }
        })
        .collect();

    let verdict = if total_collision_count > 0 {
        format!(
            "ERROR: {} holder/shank collisions detected",
            total_collision_count
        )
    } else if diag.rapid_collision_count > 0 {
        format!(
            "WARNING: {} rapid-through-stock collisions",
            diag.rapid_collision_count
        )
    } else if diag.air_cut_percentage > 40.0 {
        format!("WARNING: {:.1}% air cutting", diag.air_cut_percentage)
    } else {
        "OK".to_owned()
    };

    let project_summary = ProjectSummary {
        project: session.name().to_owned(),
        setup_count: session.setup_count().max(1),
        toolpath_count: diag.per_toolpath.len(),
        total_cutting_distance_mm: total_cutting,
        total_rapid_distance_mm: total_rapid,
        total_runtime_s: diag.total_runtime_s,
        air_cut_percentage: diag.air_cut_percentage,
        average_engagement: diag.average_engagement,
        collision_count: total_collision_count,
        rapid_collision_count: diag.rapid_collision_count,
        per_toolpath,
        verdict: verdict.clone(),
    };

    let summary_path = output_dir.join("summary.json");
    let summary_json =
        serde_json::to_string_pretty(&project_summary).context("Failed to serialize summary")?;
    std::fs::write(&summary_path, summary_json)
        .context(format!("Failed to write {}", summary_path.display()))?;
    info!(path = %summary_path.display(), "Wrote project summary");

    // 10. Optional G-code emit (F-036c calibration helper)
    if let Some(gcode_path) = emit_gcode {
        let trace = session
            .simulation_result()
            .and_then(|s| s.cut_trace.as_deref());
        let policy = rs_cam_core::gcode::ToolLoadExportPolicy {
            accept_unmodeled: true,
            accept_exceeded: true,
        };
        let gcode = rs_cam_core::gcode::export_gcode_checked(&session, trace, policy)
            .context("Emit G-code from session")?;
        std::fs::write(gcode_path, &gcode)
            .context(format!("Failed to write {}", gcode_path.display()))?;
        info!(
            path = %gcode_path.display(),
            bytes = gcode.len(),
            modulated = adaptive_feed_modulation,
            kinematics_injected = inject_shapeoko_kinematics,
            "Wrote G-code"
        );
    }

    // 11. Print human-readable summary
    if summary {
        eprintln!("\n=== Project Diagnostics: {} ===", session.name());
        eprintln!(
            "Toolpaths: {}  |  Cutting: {:.0}mm  |  Rapid: {:.0}mm  |  Time: {:.0}s",
            diag.per_toolpath.len(),
            total_cutting,
            total_rapid,
            diag.total_runtime_s,
        );

        // Print engagement + peak chipload from simulation trace
        if let Some(sim_result) = session.simulation_result()
            && let Some(trace) = &sim_result.cut_trace
        {
            eprintln!(
                "Air cutting: {:.1}%  |  Avg engagement: {:.2}  |  Peak chipload: {:.3} mm/tooth",
                diag.air_cut_percentage,
                diag.average_engagement,
                trace.summary.peak_chipload_mm_per_tooth,
            );
        }

        for entry in &project_summary.per_toolpath {
            let status_icon = if entry.status == "ok" { " " } else { "!" };
            eprintln!(
                "  [{status_icon}] #{} {} ({}) — {} moves, {} collisions",
                entry.id, entry.name, entry.operation, entry.move_count, entry.collision_count,
            );
        }
        eprintln!("Verdict: {verdict}");
        eprintln!("Output: {}", output_dir.display());
    }

    Ok(())
}

/// Iterate every enabled toolpath in the session, run
/// `feeds::suggest_for_operation`, and replace the operation's
/// feed_rate / plunge_rate / stepover / depth_per_pass / spindle_rpm
/// with the suggested values. Mutates the session in place; does NOT
/// write back to the project file. Prints a before→after table to
/// stderr so the operator can see what shifted.
fn apply_suggested_feeds_to_session(session: &mut ProjectSession) -> Result<()> {
    use rs_cam_core::feeds::{embedded_vendor_lut, suggest::{SuggestForOperationInput, suggest_for_operation}};

    // Snapshot inputs needed for suggest. We need to borrow `tools()`,
    // `machine()`, `stock_config()` immutably while we mutate
    // `toolpath_configs_mut()` — so clone the immutable view up front.
    let tools_snapshot: Vec<_> = session.tools().to_vec();
    let machine = session.machine().clone();
    let material = session.stock_config().material.clone();
    let workholding = session.stock_config().workholding_rigidity;
    let lut = embedded_vendor_lut();

    eprintln!("\n=== Applying LUT-suggested feeds/speeds ===");
    eprintln!(
        "{:<3} {:<32} {:>10} {:>10} {:>10} {:>10} {:>8}",
        "id", "name", "feed", "plunge", "stepover", "dpp", "rpm"
    );

    for tc in session.toolpath_configs_mut().iter_mut() {
        if !tc.enabled {
            continue;
        }
        let Some(tool) = tools_snapshot.iter().find(|t| t.id.0 == tc.tool_id) else {
            warn!(toolpath_id = tc.id, tool_id = tc.tool_id, "Tool not found, skipping suggest");
            continue;
        };

        // Capture before values for the table.
        let feed_before = tc.operation.feed_rate();
        let plunge_before = tc.operation.plunge_rate();
        let stepover_before = tc.operation.stepover();
        let dpp_before = tc.operation.depth_per_pass();
        let rpm_before = tc.operation.spindle_rpm();

        let suggested = suggest_for_operation(SuggestForOperationInput {
            operation: &tc.operation,
            tool,
            machine: &machine,
            material: &material,
            workholding,
            lut,
            spindle_strategy: rs_cam_core::feeds::SpindleStrategy::default(),
        });

        // Replace operation with the suggested one (feed/plunge/
        // stepover/dpp already written by apply_feeds_result_to_op).
        tc.operation = suggested.operation;
        // Suggest doesn't write spindle_rpm into the operation; the
        // RPM lives in `feeds_result.rpm`. Apply it explicitly so the
        // emitted M3 line matches the calculator's recommendation.
        let suggested_rpm = suggested.feeds_result.rpm;
        if suggested_rpm.is_finite() && suggested_rpm > 0.0 {
            tc.operation.set_spindle_rpm(Some(suggested_rpm.round() as u32));
        }

        let feed_after = tc.operation.feed_rate();
        let plunge_after = tc.operation.plunge_rate();
        let stepover_after = tc.operation.stepover();
        let dpp_after = tc.operation.depth_per_pass();
        let rpm_after = tc.operation.spindle_rpm();

        eprintln!(
            "{:<3} {:<32} {:>10} {:>10} {:>10} {:>10} {:>8}",
            tc.id,
            truncate(&tc.name, 32),
            format!("{:.0}→{:.0}", feed_before, feed_after),
            format!("{:.0}→{:.0}", plunge_before, plunge_after),
            format!(
                "{}→{}",
                fmt_opt(stepover_before, 2),
                fmt_opt(stepover_after, 2)
            ),
            format!(
                "{}→{}",
                fmt_opt(dpp_before, 2),
                fmt_opt(dpp_after, 2)
            ),
            format!(
                "{}→{}",
                fmt_opt_u32(rpm_before),
                fmt_opt_u32(rpm_after)
            ),
        );

        // No cache invalidation needed — apply runs before
        // generate_all, which always computes from current configs.
    }
    eprintln!();
    Ok(())
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_owned()
    } else {
        let cut: String = s.chars().take(n.saturating_sub(1)).collect();
        format!("{cut}…")
    }
}

fn fmt_opt(v: Option<f64>, p: usize) -> String {
    match v {
        Some(x) => format!("{:.*}", p, x),
        None => "-".to_owned(),
    }
}

fn fmt_opt_u32(v: Option<u32>) -> String {
    match v {
        Some(x) => x.to_string(),
        None => "-".to_owned(),
    }
}
