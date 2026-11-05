#![expect(
    clippy::print_stdout,
    reason = "CLI surface: the JSON records and the table go to stdout"
)]

//! `suggest`: print Suggest and its rationale for every enabled toolpath,
//! without generation and without a change to the project.
//!
//! The command reads `ProjectSession::cutter_op_profile`, the one profile
//! that MCP `get_suggest_rationale` and the GUI Suggest card read. It
//! computes no feed of its own. The `basis` text is the core `card_text`
//! of each basis, the strings that the MCP `basis` object carries.
//!
//! `current` is the operation in the project file. `suggested` is
//! `CutterOpProfile::suggested_operation`, the operation that `ApplyFeeds`
//! writes. `shipped_power` is `feeds::power_at_operating_point` on the
//! suggested operation; `calculator` is the `FeedsResult` at the
//! calculator's geometry (see the `FeedsResult::power_kw` note). Where the
//! operation holds no stepover, depth or rpm, `shipped_power` reads the
//! calculator point, as the GUI does, and `point_source` names the source
//! of `ae`, `ap` and the rpm. A Drill and a Pin Drill print `peck_depth`.
//!
//! Feeds contract P5 (K2b): `suggested` holds the speeds for the cut the
//! op keeps (Suggest writes no geometry by itself). `envelope` is the safe
//! envelope of the profile (`CutterOpProfile::envelope`), the JSON that MCP
//! `get_safe_envelope` returns: the axes, the limits with law and basis, the
//! speeds grid, the proposal and the operator's placement. `tied_rows`
//! names both rows of a Q3 tie.
//!
//! Feeds contract P13b: `lineage` is the lineage of the toolpath with no
//! generation (`ProjectSession::lineage_measured`: the stored step records
//! of the project file), and `run_state` names the state of the toolpath.

use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::path::Path;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::feeds::profile::CutterOpProfile;
use rs_cam_core::feeds::rationale::SuggestRationale;
use rs_cam_core::session::ProjectSession;

/// The peck depth of a Drill or a Pin Drill. Other operations have none.
fn peck_depth(op: &OperationConfig) -> Option<f64> {
    if let OperationConfig::Drill(cfg) = op {
        return cfg.peck_depth;
    }
    if let OperationConfig::AlignmentPinDrill(cfg) = op {
        return cfg.peck_depth;
    }
    None
}

fn op_values(op: &OperationConfig) -> Value {
    json!({
        "feed_rate": op.feed_rate(),
        "plunge_rate": op.plunge_rate(),
        "stepover": op.stepover(),
        "depth_per_pass": op.depth_per_pass(),
        "spindle_rpm": op.spindle_rpm(),
        "peck_depth": peck_depth(op),
    })
}

/// Where the power point reads `ae`, `ap` and the rpm: `"operation"` when
/// the operation holds a usable value (finite and above 0, the rule of
/// `power_at_operating_point`), `"calculator"` when the fallback gives it.
/// The calculator point is a default cut, not the cut the operation makes.
fn point_source(op: &OperationConfig) -> Value {
    let source = |v: Option<f64>| {
        if v.is_some_and(|x| x.is_finite() && x > 0.0) {
            "operation"
        } else {
            "calculator"
        }
    };
    json!({
        "ae": source(op.stepover()),
        "ap": source(op.depth_per_pass()),
        "rpm": source(op.spindle_rpm().map(f64::from)),
    })
}

fn profile_json(profile: &CutterOpProfile<'_>) -> Value {
    let rationale = SuggestRationale::from_warnings(&profile.warnings);
    let Some(feeds) = profile.feeds.as_ref() else {
        return json!({ "rationale": rationale });
    };
    let (support_headline, support_detail) = feeds.support.card_text();
    let (plunge_headline, plunge_detail) = feeds.plunge.card_text();
    let (ramp_headline, ramp_detail) = feeds.ramp.card_text();
    let shipped = profile
        .suggested_operation
        .as_ref()
        .unwrap_or(profile.operation);
    // The fallback is the calculator point, as the GUI builds it
    // (`compare.rs` `PowerReading::at_shipped_point`).
    let fallback = Some(rs_cam_core::feeds::suggest::CalculatorOperatingPoint {
        radial_width_mm: feeds.radial_width_mm,
        axial_depth_mm: feeds.axial_depth_mm,
        feed_rate_mm_min: feeds.feed_rate_mm_min,
        rpm: feeds.rpm,
    });
    let shipped_power = match rs_cam_core::feeds::power_at_operating_point(
        shipped,
        profile.tool_cfg,
        profile.material,
        profile.machine,
        fallback,
    ) {
        Ok(p) => json!({
            "required_kw": p.required_kw,
            "available_kw": p.available_kw,
            "ap_mm": p.ap_mm,
            "ae_mm": p.ae_mm,
            "rpm": p.rpm,
            "feed_mm_min": p.feed_mm_min,
            // K7 (feeds contract P9): the worst case of the pre-sim set.
            "case": p.case.label(),
            "case_ae_mm": p.case_ae_mm,
        }),
        Err(e) => json!({ "unmodeled": format!("{e:?}") }),
    };
    json!({
        "calculator": {
            "rpm": feeds.rpm,
            "chip_load_mm": feeds.chip_load_mm,
            "feed_rate_mm_min": feeds.feed_rate_mm_min,
            "plunge_rate_mm_min": feeds.plunge_rate_mm_min,
            "axial_depth_mm": feeds.axial_depth_mm,
            "radial_width_mm": feeds.radial_width_mm,
            "power_kw": feeds.power_kw,
            "power_case": feeds.power_case.label(),
            "available_power_kw": feeds.available_power_kw,
            "power_limited": feeds.power_limited,
            "mrr_mm3_min": feeds.mrr_mm3_min,
            "vendor_source": feeds.vendor_source,
            "chipload_source": format!("{:?}", feeds.chipload_source),
            "chipload_bounds_mm": feeds
                .chipload_bounds
                .as_ref()
                .map(|b| [b.min_mm_per_tooth, b.max_mm_per_tooth]),
            "chipload_point_mm": feeds.chipload_point_mm,
            "warnings": feeds.warnings.iter().map(|w| format!("{w:?}")).collect::<Vec<_>>(),
        },
        "shipped_power": shipped_power,
        "point_source": point_source(shipped),
        // P11c (D13 item 6): the machine class and how the rigidity values
        // are known; MCP `get_suggest_rationale` `basis.machine` prints the
        // same two texts.
        "machine": {
            "class": profile.machine.class.label(),
            "rigidity_basis": profile.machine.rigidity_basis().text(),
        },
        "basis": {
            "support": { "headline": support_headline, "detail": support_detail },
            "plunge": { "headline": plunge_headline, "detail": plunge_detail },
            "ramp": { "headline": ramp_headline, "detail": ramp_detail },
            "tied_rows": feeds.matched_lut_row.as_ref().and_then(|row| {
                row.tied_with
                    .as_ref()
                    .map(|tie| vec![row.observation_id.clone(), tie.id().to_owned()])
            }),
        },
        "envelope": match &profile.envelope {
            Ok(envelope) => serde_json::to_value(envelope).unwrap_or(Value::Null),
            Err(e) => json!({ "refused": e.to_string() }),
        },
        "rationale": rationale,
    })
}

/// The JSON record of one toolpath: the current operation, the tool, the
/// suggested operation and the profile.
fn record_json(
    session: &ProjectSession,
    index: usize,
    tc: &rs_cam_core::session::ToolpathConfig,
) -> Value {
    let profile = session.cutter_op_profile(tc);
    let mut record = serde_json::Map::new();
    record.insert("index".into(), json!(index));
    record.insert("toolpath_id".into(), json!(tc.id));
    record.insert("toolpath_name".into(), json!(tc.name));
    record.insert("current".into(), op_values(&tc.operation));
    // P13b: the lineage that the project file stores (the step records),
    // the JSON of MCP `get_toolpath_lineage`; this command generates
    // nothing, so the run state names the toolpath's state.
    record.insert(
        "run_state".into(),
        json!(rs_cam_core::session::toolpath_run_state(session, tc.id, false).map(|s| s.label())),
    );
    record.insert(
        "lineage".into(),
        session
            .lineage_measured(index, None)
            .map_or(Value::Null, |l| l.to_json()),
    );
    match profile.as_ref() {
        None => {
            record.insert(
                "error".into(),
                json!("no tool, or the tool is outside its domain"),
            );
        }
        Some(p) => {
            record.insert(
                "tool".into(),
                json!({
                    "name": p.tool_cfg.name,
                    "tool_type": p.tool_cfg.tool_type,
                    "diameter": p.tool_cfg.diameter,
                    "flute_count": p.tool_cfg.flute_count_planning(),
                }),
            );
            match &p.feasibility {
                Err(e) => {
                    record.insert("error".into(), json!(format!("Suggest refused: {e}")));
                }
                Ok(()) => {
                    record.insert(
                        "suggested".into(),
                        p.suggested_operation
                            .as_ref()
                            .map_or(Value::Null, op_values),
                    );
                    record.insert("profile".into(), profile_json(p));
                }
            }
        }
    }
    Value::Object(record)
}

/// Print one record per enabled toolpath. `json` prints one JSON object
/// per line; otherwise a table of current and suggested values.
pub fn run_suggest(input: &Path, json_out: bool) -> Result<()> {
    let path = input
        .canonicalize()
        .context(format!("Project file not found: {}", input.display()))?;
    let session = ProjectSession::load(&path).context("Failed to load project session")?;

    if !json_out {
        // P11c (D13 item 6): the machine class and how its rigidity values
        // are known, above the table.
        let machine = session.machine();
        println!(
            "machine: {} (rigidity: {})",
            machine.name,
            machine.rigidity_basis().text()
        );
        println!(
            "{:<3} {:<30} {:>13} {:>11} {:>11} {:>11} {:>13} {:>11}",
            "idx", "name", "feed", "plunge", "stepover", "dpp", "rpm", "peck"
        );
    }
    for (index, tc) in session.toolpath_configs().iter().enumerate() {
        if !tc.enabled {
            continue;
        }
        if json_out {
            println!("{}", record_json(&session, index, tc));
        } else {
            let profile = session.cutter_op_profile(tc);
            let sug = profile
                .as_ref()
                .and_then(|p| p.suggested_operation.as_ref());
            let cell = |a: Option<f64>, b: Option<f64>| {
                let f = |v: Option<f64>| v.map_or_else(|| "-".to_owned(), |x| format!("{x:.2}"));
                format!("{}→{}", f(a), f(b))
            };
            let cur = &tc.operation;
            println!(
                "{:<3} {:<30} {:>13} {:>11} {:>11} {:>11} {:>13} {:>11}",
                index,
                tc.name.chars().take(30).collect::<String>(),
                cell(cur.feed_rate(), sug.and_then(OperationConfig::feed_rate)),
                cell(
                    cur.plunge_rate(),
                    sug.and_then(OperationConfig::plunge_rate)
                ),
                cell(cur.stepover(), sug.and_then(OperationConfig::stepover)),
                cell(
                    cur.depth_per_pass(),
                    sug.and_then(OperationConfig::depth_per_pass)
                ),
                cell(
                    cur.spindle_rpm().map(f64::from),
                    sug.and_then(OperationConfig::spindle_rpm).map(f64::from)
                ),
                cell(peck_depth(cur), sug.and_then(peck_depth)),
            );
            // P13c (O3): a computed width states its assumptions, the one
            // core line the GUI card and MCP print.
            if profile.as_ref().is_some_and(|p| {
                p.warnings.iter().any(|w| {
                    matches!(
                        w,
                        rs_cam_core::feeds::suggest::SuggestWarning::ComputedWidthAssumptions
                    )
                })
            }) {
                println!(
                    "    {}",
                    rs_cam_core::feeds::force_budget::computed_width_assumptions_line()
                );
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::*;
    use crate::command::apply_command;
    use rs_cam_core::ToolpathId;
    use rs_cam_core::compute::catalog::OperationType;
    use rs_cam_core::session::{AddToolpathArgs, Command, ToolpathConfig};

    /// The 2D pocket fixture with one toolpath of `op` added through the
    /// session door (the `pocket_session` pattern of `rough_score.rs`).
    fn session_with(op: OperationType) -> ProjectSession {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test_data/ux_2d_pocket.toml");
        let mut session = ProjectSession::load(&path).expect("load ux_2d_pocket.toml");
        let tool_id = session.tools()[0].id.raw();
        let model_id = session.models()[0].id;
        let _ = apply_command(
            &mut session,
            Command::AddToolpath(AddToolpathArgs {
                setup_index: 0,
                config: Box::new(ToolpathConfig {
                    shoe: Default::default(),
                    feeds_lock: Default::default(),
                    id: ToolpathId::from_raw(0),
                    name: "suggest test".to_owned(),
                    enabled: true,
                    operation: rs_cam_core::compute::fixture_values::fixture_op(op),
                    dressups: rs_cam_core::compute::config::DressupConfig::default(),
                    heights: rs_cam_core::compute::config::HeightsConfig::default(),
                    tool_id: Some(rs_cam_core::ids::ToolId::from_raw(tool_id)),
                    model_id,
                    pre_gcode: None,
                    post_gcode: None,
                    boundary: rs_cam_core::compute::config::BoundaryConfig::default(),
                    boundary_inherit: true,
                    rest_analysis: rs_cam_core::compute::config::RestAnalysisConfig::default(),
                    stock_source: rs_cam_core::compute::config::StockSource::default(),
                    coolant: rs_cam_core::gcode::CoolantMode::Off,
                    debug_options: rs_cam_core::trace::debug_trace::ToolpathDebugOptions::default(),
                    feeds_provenance: rs_cam_core::feeds::FeedsProvenance::default(),
                    planner_origin: None,
                }),
            }),
        )
        .expect("add the toolpath");
        session
    }

    fn last_record(session: &ProjectSession) -> Value {
        let configs = session.toolpath_configs();
        let index = configs.len() - 1;
        record_json(session, index, &configs[index])
    }

    /// Feeds contract P13b (surface parity): the record prints the stored
    /// lineage of the toolpath (the core `lineage_measured` with no trace,
    /// the call MCP `get_toolpath_lineage` reads) and its run state. After
    /// an apply the lineage holds the step records. Red at HEAD 255d8db90:
    /// the record has no `lineage` key.
    #[test]
    fn the_suggest_record_carries_the_stored_lineage_p13b() {
        let mut session = session_with(OperationType::Pocket);
        let index = session.toolpath_configs().len() - 1;
        let _ = apply_command(
            &mut session,
            Command::ApplyFeeds(rs_cam_core::session::ApplyFeedsArgs {
                index,
                pick: None,
                explored: None,
            }),
        )
        .expect("the speeds apply");
        let record = last_record(&session);
        let core = session
            .lineage_measured(index, None)
            .expect("a stored lineage")
            .to_json();
        assert_eq!(record["lineage"], core);
        assert!(
            record["lineage"]["steps"]
                .as_array()
                .is_some_and(|s| !s.is_empty()),
            "no step record in {}",
            record["lineage"]
        );
        assert!(record["run_state"].is_string());
    }

    /// FC0 item 2: a Drill record prints its peck depth.
    #[test]
    fn a_drill_record_prints_its_peck_fc0() {
        let session = session_with(OperationType::Drill);
        let record = last_record(&session);
        let peck = record["current"]["peck_depth"].as_f64();
        assert!(peck.is_some_and(|p| p > 0.0), "no peck_depth in {record}");
    }

    /// Feeds contract P11c (D13 item 6): the record names the machine class
    /// and how its rigidity values are known, as MCP
    /// `get_suggest_rationale` `basis.machine` does.
    #[test]
    fn the_suggest_record_names_the_machine_class_p11c() {
        let session = session_with(OperationType::Pocket);
        let record = last_record(&session);
        let machine = &record["profile"]["machine"];
        assert_eq!(
            machine["class"],
            session.machine().class.label(),
            "{record}"
        );
        assert_eq!(
            machine["rigidity_basis"],
            session.machine().rigidity_basis().text(),
            "{record}"
        );
    }

    /// Feeds contract P13c (ruling O3): a roughing Pocket's record names the
    /// assumptions of its computed width in the rationale, the core text
    /// that MCP `get_suggest_rationale` and the GUI card print.
    #[test]
    fn the_suggest_record_states_the_width_assumptions_p13c() {
        let session = session_with(OperationType::Pocket);
        let record = last_record(&session);
        let text = record["profile"]["rationale"].to_string();
        for note in rs_cam_core::feeds::force_budget::COMPUTED_WIDTH_ASSUMPTIONS {
            assert!(text.contains(note), "the record lacks {note:?}: {record}");
        }
        assert!(
            text.contains(&rs_cam_core::feeds::force_budget::computed_width_assumptions_line()),
            "{record}"
        );
    }

    /// FC0 item 2: a Profile has no stepover, so the shipped power reads
    /// the calculator point, as the GUI does, and says so.
    #[test]
    fn a_profile_power_reads_the_calculator_point_fc0() {
        let session = session_with(OperationType::Profile);
        let record = last_record(&session);
        let power = &record["profile"]["shipped_power"];
        assert!(
            power["required_kw"].as_f64().is_some(),
            "no figure: {record}"
        );
        assert_eq!(
            record["profile"]["point_source"]["ae"], "calculator",
            "{record}"
        );
    }
}
