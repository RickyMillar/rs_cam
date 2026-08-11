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

/// The CLI's per-toolpath JSON record — a serde VIEW over
/// [`rs_cam_core::session::ToolpathDiagnostic`], not a second copy of it.
///
/// It cannot simply BE the core struct for two reasons that are not going
/// away: it carries the full debug + semantic traces and the collision-check
/// stickout, which the core diagnostic (a GUI/MCP summary) does not; and its
/// key names (`toolpath_name`, `tool`) differ from the core struct's (`name`,
/// `tool_name`) and are read by existing scripts, so `#[serde(flatten)]`
/// would move the wire.
///
/// What it must NOT do is *diverge on the fields both have*. Wave D3 caught
/// the first instance: the A/M9 standing-material and Wave-D1 dropped-band /
/// tip-float channels were published to the GUI and MCP but silently missing
/// here, so a CLI batch run could not see a finding the same session's GUI
/// would show. D3 fixed those five fields by hand; the struct itself stayed
/// a parallel implementation, which is what
/// `ANTIPATTERNS_BACKLOG.md` P3 logged.
///
/// C3 (2026-08-02) closes it structurally. [`Self::from_core`] EXHAUSTIVELY
/// DESTRUCTURES the core diagnostic, so adding a field there is a compile
/// error here until the CLI decides whether to publish it. That is the
/// mechanism — not vigilance, and not a doc comment asking for it. The wire
/// is pinned byte-for-byte by `tests::the_per_toolpath_json_is_byte_stable`,
/// so the derivation could be rebuilt underneath without the JSON moving.
#[derive(Serialize)]
struct ToolpathDiagnostic<'a> {
    toolpath_id: rs_cam_core::ToolpathId,
    toolpath_name: &'a str,
    operation_type: &'a str,
    /// Stable op-kind tag, mirroring
    /// [`rs_cam_core::session::ToolpathDiagnostic::op_kind`]. `Option` only
    /// because it has always serialised as `null` when the session published
    /// no diagnostic; see [`Self::from_core`].
    op_kind: Option<&'a str>,
    tool: &'a str,
    move_count: usize,
    cutting_distance_mm: f64,
    rapid_distance_mm: f64,
    debug_trace: Option<&'a rs_cam_core::debug_trace::ToolpathDebugTrace>,
    semantic_trace: Option<&'a rs_cam_core::semantic_trace::ToolpathSemanticTrace>,
    /// CLI-LOCAL, deliberately. The core diagnostic's `collision_count` is
    /// whatever holder-collision evidence its caller supplied; the CLI runs
    /// its own per-toolpath [`rs_cam_core::session::ProjectSession::collision_check`]
    /// and reports that. Better data, not a second opinion on the same data.
    collision_count: usize,
    rapid_collision_count: usize,
    /// CLI-local for the same reason: it comes off the collision report the
    /// core diagnostic never sees.
    min_safe_stickout: Option<f64>,
    /// A/M9, renamed by wave 16 (Checkpoint E ruling A6 — the old name said
    /// *standing*, the number means *untouched*). `null` = **not measured**
    /// (this operation runs no ring cascade), never "nothing left uncut".
    truncated_core_mm2: Option<f64>,
    /// A6 compatibility duplicate: the SAME value as
    /// [`Self::truncated_core_mm2`] under the pre-rename key, kept so
    /// existing scripts reading this report keep working. Deprecated; it
    /// carries no independent meaning and will not gain one.
    standing_material_mm2: Option<f64>,
    /// B8: hole-aware sibling of [`Self::truncated_core_mm2`] — the same
    /// truncated core with islands netted out. `null` = not measured.
    untouched_material_mm2: Option<f64>,
    /// B8: area the cascade reached and then dropped every point on — the
    /// oracle's *standing*. An ESTIMATOR of a DIFFERENT quantity from the
    /// core above; never sum or compare the two. `null` = not measured.
    reached_uncut_estimate_mm2: Option<f64>,
    /// Wave D1. `null` = nothing dropped, or nothing that plans bands ran.
    unmachined_band_area_mm2: Option<f64>,
    /// Wave D1. `null` = the operation emits no centrelines (not measured);
    /// `0` = measured and clean.
    tip_float_points: Option<usize>,
    /// Wave D1. `null` under exactly the same condition as
    /// [`Self::tip_float_points`].
    max_tip_float_mm: Option<f64>,
}

impl<'a> ToolpathDiagnostic<'a> {
    /// Project a core diagnostic onto the CLI wire, adding the three
    /// CLI-only channels.
    ///
    /// The `let ... = core;` destructure below is load-bearing: it has no
    /// `..`, so a new field on
    /// [`rs_cam_core::session::ToolpathDiagnostic`] breaks this build. Bind
    /// it to `_name_unused`-style names if the CLI genuinely should not
    /// publish it — but make that a decision someone wrote down, which is
    /// exactly what D3 found nobody had.
    fn from_core(
        core: &'a rs_cam_core::session::ToolpathDiagnostic,
        debug_trace: Option<&'a rs_cam_core::debug_trace::ToolpathDebugTrace>,
        semantic_trace: Option<&'a rs_cam_core::semantic_trace::ToolpathSemanticTrace>,
        collision_count: usize,
        min_safe_stickout: Option<f64>,
    ) -> Self {
        let rs_cam_core::session::ToolpathDiagnostic {
            toolpath_id,
            name,
            operation_type,
            op_kind,
            tool_name,
            move_count,
            cutting_distance_mm,
            rapid_distance_mm,
            // Superseded by the CLI's own collision check — see the field doc.
            collision_count: _evidence_collision_count,
            rapid_collision_count,
            truncated_core_mm2,
            untouched_material_mm2,
            reached_uncut_estimate_mm2,
            unmachined_band_area_mm2,
            tip_float_points,
            max_tip_float_mm,
        } = core;

        Self {
            toolpath_id: *toolpath_id,
            toolpath_name: name,
            operation_type,
            op_kind: Some(op_kind),
            tool: tool_name,
            move_count: *move_count,
            cutting_distance_mm: *cutting_distance_mm,
            rapid_distance_mm: *rapid_distance_mm,
            debug_trace,
            semantic_trace,
            collision_count,
            rapid_collision_count: *rapid_collision_count,
            min_safe_stickout,
            truncated_core_mm2: *truncated_core_mm2,
            // A6: same value, pre-rename key, for existing readers.
            standing_material_mm2: *truncated_core_mm2,
            untouched_material_mm2: *untouched_material_mm2,
            reached_uncut_estimate_mm2: *reached_uncut_estimate_mm2,
            unmachined_band_area_mm2: *unmachined_band_area_mm2,
            tip_float_points: *tip_float_points,
            max_tip_float_mm: *max_tip_float_mm,
        }
    }
}

#[derive(Serialize)]
struct ToolpathSummaryEntry {
    id: rs_cam_core::ToolpathId,
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
    /// Legacy key, unchanged value: identical to
    /// `air_cut_pct_of_total_runtime`. Kept so existing scripts reading this
    /// report keep working.
    air_cut_percentage: f64,
    /// LH-1: air cut over TOTAL runtime (cutting + rapids) - the measure
    /// every threshold in the codebase uses. The cutting-time reading of the
    /// same seconds ships beside it so neither travels unnamed.
    air_cut_pct_of_total_runtime: f64,
    air_cut_pct_of_cutting_time: f64,
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
    skip_ids: &[rs_cam_core::ToolpathId],
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
    let mut combined_skip: Vec<rs_cam_core::ToolpathId> = skip_ids.to_vec();
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
        rs_cam_core::ToolpathId,
        rs_cam_core::collision::CollisionReport,
    > = std::collections::HashMap::new();

    for idx in 0..tp_count {
        if session.get_result(idx).is_none() {
            continue;
        }
        let tp_id = session
            .get_toolpath_config(idx)
            .map(|tc| tc.id)
            // Defensive fallback mirrors the project-file loader: when a
            // config is somehow absent, the position doubles as the id.
            .unwrap_or(rs_cam_core::ToolpathId(idx));
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

        // The tool name is no longer looked up here: the core diagnostic
        // resolves it from the same `tool_id` through the same session, so
        // the CLI reading it off the core record is one derivation instead of
        // two identical ones (C3).
        let col_report = collision_reports.get(&tc.id);
        let collision_count = col_report.map(|r| r.collisions.len()).unwrap_or(0);
        let min_safe = col_report.map(|r| r.min_safe_stickout);

        // The core per-toolpath diagnostic for this id. EVERY shared field is
        // derived THERE, so the CLI and the GUI/MCP cannot report different
        // numbers for the same run (Wave D3; structural since C3).
        //
        // Both loops are guarded by the same `results` map — the core builds
        // a diagnostic for exactly the toolpaths that have a result, which is
        // the condition this loop already `continue`d on — so a miss is
        // unreachable. It is warned rather than defaulted because a silently
        // half-populated record is the failure mode D3 was cleaning up.
        let Some(core_diag) = diag.per_toolpath.iter().find(|d| d.toolpath_id == tc.id) else {
            warn!(
                toolpath = %tc.name,
                "No core diagnostic for a toolpath that has a result — skipping its JSON record"
            );
            continue;
        };

        let diagnostic = ToolpathDiagnostic::from_core(
            core_diag,
            result.debug_trace.as_ref(),
            result.semantic_trace.as_ref(),
            collision_count,
            min_safe,
        );

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
        let included_ids: Vec<rs_cam_core::ToolpathId> = (0..tp_count)
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

    // The page-one answer, from the SAME `ProjectSession::simulation_triage`
    // the GUI panel, the MCP `get_diagnostics` response and narration read
    // (census §4 acceptance bar). Printed before the verdict line so the
    // reader sees the classes — safety, then actions, then a bounded
    // advisory list that says how much it withheld — rather than a single
    // string plus an unbounded pile of runs.
    print_triage_report(&session.triage());

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
    } else if diag.air_cut_pct_of_total_runtime > 40.0 {
        // LH-1: the 40% band is on the TOTAL-runtime measure; the verdict
        // string says so rather than shipping a bare "air cutting %".
        format!(
            "WARNING: {:.1}% air cutting of total runtime",
            diag.air_cut_pct_of_total_runtime
        )
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
        air_cut_pct_of_total_runtime: diag.air_cut_pct_of_total_runtime,
        air_cut_pct_of_cutting_time: diag.air_cut_pct_of_cutting_time,
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

        // Print engagement + peak COMMANDED advance/tooth from the sim trace.
        // `peak_chipload_mm_per_tooth` is a per-sample peak of the commanded
        // value, not the gate statistic and not a chip thickness (A-1 census
        // row N7). Named accordingly since 2026-08-08; the number is unchanged.
        if let Some(sim_result) = session.simulation_result()
            && let Some(trace) = &sim_result.cut_trace
        {
            eprintln!(
                "Air cutting: {:.1}% of total runtime  |  Avg engagement: {:.2}  |  \
                 Peak commanded advance/tooth: {:.3} mm/tooth",
                diag.air_cut_pct_of_total_runtime,
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
    // T16 — route through the canonical `ProjectSession::cutter_op_profile`
    // (T10) instead of a third hand-rolled `SuggestContext` assembly. The
    // previous CLI copy hardcoded `SpindleStrategy::default()` where the
    // GUI Suggest button and the MCP rationale endpoint read
    // `post_config().spindle_strategy`, so projects with a non-default
    // strategy got different RPM/feed from `--apply-suggest` than from
    // the GUI. That was a bug, not deliberate CLI semantics.
    //
    // Pass 1 (immutable): collect suggestions per enabled toolpath. The
    // profile borrows `session`, so the suggested operations are moved
    // into an owned list before the mutating pass.
    let mut suggestions: Vec<(
        usize,
        rs_cam_core::compute::catalog::OperationConfig,
        f64,
        rs_cam_core::feeds::FeedsProvenance,
    )> = Vec::new();
    for (idx, tc) in session.toolpath_configs().iter().enumerate() {
        if !tc.enabled {
            continue;
        }
        let Some(profile) = session.cutter_op_profile(tc) else {
            warn!(
                toolpath_id = tc.id.0,
                tool_id = tc.tool_id,
                "Tool not found, skipping suggest"
            );
            continue;
        };
        if let Err(e) = &profile.feasibility {
            warn!(
                toolpath_id = tc.id.0,
                tool_id = tc.tool_id,
                error = %e,
                "Suggest refused tool × operation combination, leaving existing values"
            );
            continue;
        }
        // Feasibility Ok ⟺ both Some (`CutterOpProfile::for_combo`).
        let (Some(operation), Some(feeds)) = (profile.suggested_operation, profile.feeds) else {
            continue;
        };
        // W2.1: per-field provenance of the suggested values, derived from the
        // same FeedsResult the calculator produced.
        let rpm_written = feeds.rpm.is_finite() && feeds.rpm > 0.0;
        let mut provenance = rs_cam_core::feeds::FeedsProvenance::default();
        provenance.apply_suggested(&feeds, &operation, rpm_written);
        suggestions.push((idx, operation, feeds.rpm, provenance));
    }

    eprintln!("\n=== Applying LUT-suggested feeds/speeds ===");
    eprintln!(
        "{:<3} {:<32} {:>10} {:>10} {:>10} {:>10} {:>8}",
        "id", "name", "feed", "plunge", "stepover", "dpp", "rpm"
    );

    // Pass 2 (mutable): apply + print the before→after table.
    for (idx, suggested_op, suggested_rpm, provenance) in suggestions {
        let Some(tc) = session.toolpath_configs_mut().get_mut(idx) else {
            continue;
        };

        // Capture before values for the table.
        let feed_before = tc.operation.feed_rate();
        let plunge_before = tc.operation.plunge_rate();
        let stepover_before = tc.operation.stepover();
        let dpp_before = tc.operation.depth_per_pass();
        let rpm_before = tc.operation.spindle_rpm();

        // Replace operation with the suggested one (feed/plunge/
        // stepover/dpp already written by apply_feeds_result_to_op).
        tc.operation = suggested_op;
        // Suggest doesn't write spindle_rpm into the operation; the
        // RPM lives in `feeds_result.rpm`. Apply it explicitly so the
        // emitted M3 line matches the calculator's recommendation.
        if suggested_rpm.is_finite() && suggested_rpm > 0.0 {
            tc.operation
                .set_spindle_rpm(Some(suggested_rpm.round() as u32));
        }
        tc.feeds_provenance = provenance;

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
            format!("{}→{}", fmt_opt(dpp_before, 2), fmt_opt(dpp_after, 2)),
            format!("{}→{}", fmt_opt_u32(rpm_before), fmt_opt_u32(rpm_after)),
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

/// Render the shared [`SimulationTriage`] contract.
///
/// The census found five surfaces each assembling, ranking and truncating
/// the issue channel their own way. This one renders the shared object and
/// adds nothing of its own — including the truncation notice, which comes
/// from `Bounded` rather than from a local `.take(10)` that forgets to say
/// so (R-6's defect, one layer up).
fn print_triage_report(triage: &rs_cam_core::sim_triage::SimulationTriage) {
    use rs_cam_core::sim_measurability::Measurability;

    // Measurability first: it qualifies everything below it.
    let unmeasured: Vec<_> = triage
        .measurability
        .entries
        .iter()
        .filter(|e| matches!(e.measurability, Measurability::NotMeasurable(_)))
        .collect();
    if !unmeasured.is_empty() {
        eprintln!("Measurability:");
        for e in &unmeasured {
            let reason = e
                .measurability
                .reason()
                .map(|r| r.describe())
                .unwrap_or_default();
            eprintln!(
                "  NOT MEASURED  {} on toolpath {} — {reason}",
                e.metric.label(),
                e.toolpath_id.0
            );
        }
        eprintln!();
    }

    if !triage.safety.is_empty() {
        eprintln!("Safety ({}):", triage.safety.len());
        for f in &triage.safety {
            eprintln!("  {}", f.diagnostic.message);
        }
        eprintln!();
    }

    if !triage.actions.is_empty() {
        eprintln!("Act on ({}):", triage.actions.len());
        for f in &triage.actions {
            eprintln!("  {}", f.diagnostic.message);
        }
        eprintln!();
    }

    if !triage.advisories.items.is_empty() {
        eprintln!("Advisories:");
        for f in &triage.advisories.items {
            let seen = if f.occurrences > 1 {
                format!(" (x{})", f.occurrences)
            } else {
                String::new()
            };
            eprintln!("  {}{seen}", f.diagnostic.message);
        }
        if triage.advisories.truncated {
            eprintln!("  ... {} more not shown", triage.advisories.hidden());
        }
        eprintln!();
    }

    // The class-D tallies, demoted to a footer and each named for the
    // population it counts — the 43x gap the census measured is visible here
    // instead of inferable.
    let c = &triage.counts;
    eprintln!(
        "Counts: {} samples | {} flagged-air + {} flagged-low (per SAMPLE) | \
         {} issue runs (COALESCED, the legacy \"issue_count\") | {} hotspots",
        c.samples_total,
        c.flagged_samples_air,
        c.flagged_samples_low,
        c.issue_segments,
        c.hotspots_total
    );
    eprintln!();
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    /// A core diagnostic with every field distinguishable, so a mis-wired
    /// field shows up as a wrong VALUE and not just a wrong key.
    fn core_diagnostic() -> rs_cam_core::session::ToolpathDiagnostic {
        rs_cam_core::session::ToolpathDiagnostic {
            toolpath_id: rs_cam_core::ToolpathId(7),
            name: "Finish pass".to_owned(),
            operation_type: "Waterline".to_owned(),
            op_kind: "waterline".to_owned(),
            tool_name: "Ø1 tapered ball".to_owned(),
            move_count: 1234,
            cutting_distance_mm: 5678.25,
            rapid_distance_mm: 90.5,
            collision_count: 3,
            rapid_collision_count: 2,
            truncated_core_mm2: Some(12.5),
            untouched_material_mm2: Some(9.75),
            reached_uncut_estimate_mm2: Some(1.5),
            unmachined_band_area_mm2: Some(3.25),
            tip_float_points: Some(4),
            max_tip_float_mm: Some(0.125),
        }
    }

    /// C3 byte-stability sentry.
    ///
    /// This record's key names are a compatibility surface — the doc comment
    /// on [`ToolpathDiagnostic`] says existing scripts read `toolpath_name`
    /// and `tool`, which is precisely why it could not simply become the core
    /// struct. Pinning the serialized bytes is what let the derivation be
    /// rebuilt underneath without asking the wire to move.
    ///
    /// Captured from the pre-C3 struct and asserted unchanged after it became
    /// a view. If a future field is added, add it here in the same edit — a
    /// key appearing in output but not in this string is a wire change nobody
    /// declared.
    #[test]
    fn the_per_toolpath_json_is_byte_stable() {
        let core = core_diagnostic();
        let record = ToolpathDiagnostic::from_core(&core, None, None, 3, Some(21.5));
        let json = serde_json::to_string_pretty(&record).unwrap();
        let expected = r#"{
  "toolpath_id": 7,
  "toolpath_name": "Finish pass",
  "operation_type": "Waterline",
  "op_kind": "waterline",
  "tool": "Ø1 tapered ball",
  "move_count": 1234,
  "cutting_distance_mm": 5678.25,
  "rapid_distance_mm": 90.5,
  "debug_trace": null,
  "semantic_trace": null,
  "collision_count": 3,
  "rapid_collision_count": 2,
  "min_safe_stickout": 21.5,
  "truncated_core_mm2": 12.5,
  "standing_material_mm2": 12.5,
  "untouched_material_mm2": 9.75,
  "reached_uncut_estimate_mm2": 1.5,
  "unmachined_band_area_mm2": 3.25,
  "tip_float_points": 4,
  "max_tip_float_mm": 0.125
}"#;
        assert_eq!(json, expected, "CLI per-toolpath JSON wire changed");
    }

    /// The `null`-means-not-measured contract survives the view: an operation
    /// that measured nothing must emit `null`, never `0`.
    #[test]
    fn unmeasured_channels_stay_null() {
        let core = rs_cam_core::session::ToolpathDiagnostic {
            truncated_core_mm2: None,
            untouched_material_mm2: None,
            reached_uncut_estimate_mm2: None,
            unmachined_band_area_mm2: None,
            tip_float_points: None,
            max_tip_float_mm: None,
            ..core_diagnostic()
        };
        let record = ToolpathDiagnostic::from_core(&core, None, None, 0, None);
        let json = serde_json::to_string(&record).unwrap();
        for key in [
            "truncated_core_mm2",
            // A6: the compatibility duplicate obeys the same contract — an
            // unmeasured channel must be `null` under BOTH keys.
            "standing_material_mm2",
            "untouched_material_mm2",
            "reached_uncut_estimate_mm2",
            "unmachined_band_area_mm2",
            "tip_float_points",
            "max_tip_float_mm",
            "min_safe_stickout",
        ] {
            assert!(
                json.contains(&format!("\"{key}\":null")),
                "{key} must serialise as null, not 0:\n{json}"
            );
        }
    }
}
