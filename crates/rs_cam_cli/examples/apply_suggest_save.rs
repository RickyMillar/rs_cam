//! One-shot: load project, apply LUT-suggested feeds/speeds, save to a new path.
//! Used to prepare a "suggest-applied" project file for MCP validation.

#![allow(clippy::print_stdout, clippy::print_stderr)] // CLI example surface

use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use rs_cam_core::feeds::{
    SpindleStrategy, embedded_vendor_lut,
    suggest::{
        StockContext, SuggestContext, SuggestForOperationInput, SuggestPolicy,
        suggest_for_operation,
    },
};
use rs_cam_core::session::ProjectSession;

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let input = PathBuf::from(
        args.next()
            .ok_or_else(|| anyhow!("usage: apply_suggest_save <in.toml> <out.toml>"))?,
    );
    let output = PathBuf::from(
        args.next()
            .ok_or_else(|| anyhow!("usage: apply_suggest_save <in.toml> <out.toml>"))?,
    );

    let input = input.canonicalize().context("input path")?;
    let mut session = ProjectSession::load(&input).context("load project")?;

    let tools_snapshot: Vec<_> = session.tools().to_vec();
    let machine = session.machine().clone();
    let material = session.stock_config().material.clone();
    let workholding = session.stock_config().workholding_rigidity;
    let lut = embedded_vendor_lut();
    let stock_ctx =
        StockContext::from_stock_bbox(session.stock_bbox(), session.stock_config().padding);
    let model_bboxes = session.collect_model_bboxes();

    println!(
        "{:<3} {:<32} {:>14} {:>14} {:>12} {:>12} {:>12}",
        "id", "name", "feed", "plunge", "stepover", "dpp", "rpm"
    );

    for tc in session.toolpath_configs_mut().iter_mut() {
        if !tc.enabled {
            continue;
        }
        let Some(tool) = tools_snapshot.iter().find(|t| t.id.0 == tc.tool_id) else {
            eprintln!("skip {} (tool {} missing)", tc.id, tc.tool_id);
            continue;
        };

        let feed_before = tc.operation.feed_rate();
        let plunge_before = tc.operation.plunge_rate();
        let stepover_before = tc.operation.stepover();
        let dpp_before = tc.operation.depth_per_pass();
        let rpm_before = tc.operation.spindle_rpm();

        let model_bbox = model_bboxes
            .iter()
            .find(|(id, _)| *id == tc.model_id)
            .map(|(_, b)| b);
        let context = SuggestContext {
            model_bbox,
            stock: Some(&stock_ctx),
            upstream_leftover_stock_mm: None,
            neighboring_strategy_hint: None,
            chipload_bounds: None,
            matched_lut_row: None,
            effective_diameter_mm: 0.0,
            policy: SuggestPolicy::default(),
        };
        let suggested = match suggest_for_operation(SuggestForOperationInput {
            operation: &tc.operation,
            tool,
            machine: &machine,
            material: &material,
            workholding,
            lut,
            spindle_strategy: SpindleStrategy::default(),
            context,
        }) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("skip {} (suggest refused: {})", tc.id, e);
                continue;
            }
        };

        tc.operation = suggested.operation;
        // v3.0d: spindle_rpm is now written by apply_feeds_result_to_op
        // itself, so no manual fixup needed here.

        let feed_after = tc.operation.feed_rate();
        let plunge_after = tc.operation.plunge_rate();
        let stepover_after = tc.operation.stepover();
        let dpp_after = tc.operation.depth_per_pass();
        let rpm_after = tc.operation.spindle_rpm();

        println!(
            "{:<3} {:<32} {:>14} {:>14} {:>12} {:>12} {:>12}",
            tc.id,
            truncate(&tc.name, 32),
            format!("{feed_before:.0} -> {feed_after:.0}"),
            format!("{plunge_before:.0} -> {plunge_after:.0}"),
            format!("{} -> {}", fmt_opt(stepover_before), fmt_opt(stepover_after)),
            format!("{} -> {}", fmt_opt(dpp_before), fmt_opt(dpp_after)),
            format!(
                "{} -> {}",
                fmt_opt_u32(rpm_before),
                fmt_opt_u32(rpm_after)
            ),
        );
    }

    session.save(&output).context("save project")?;
    println!("\nwrote {}", output.display());
    Ok(())
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_owned()
    } else {
        let cut: String = s.chars().take(n.saturating_sub(1)).collect();
        format!("{cut}...")
    }
}

fn fmt_opt(v: Option<f64>) -> String {
    match v {
        Some(x) => format!("{x:.2}"),
        None => "-".to_owned(),
    }
}

fn fmt_opt_u32(v: Option<u32>) -> String {
    match v {
        Some(x) => x.to_string(),
        None => "-".to_owned(),
    }
}
