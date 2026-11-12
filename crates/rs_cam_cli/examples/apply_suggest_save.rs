//! One-shot: load project, apply LUT-suggested feeds/speeds, save to a new path.
//! Used to prepare a "suggest-applied" project file for MCP validation.

#![expect(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "CLI example surface"
)]

use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use rs_cam_core::feeds::{
    SpindleStrategy, embedded_vendor_lut,
    suggest::{
        NoProject, StockContext, SuggestContext, SuggestForOperationInput, suggest_for_operation,
    },
};
use rs_cam_core::session::{Command, ProjectSession, ReplaceToolpathConfigArgs};

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
    let lut = embedded_vendor_lut();
    let stock_ctx =
        StockContext::from_stock_bbox(session.stock_bbox(), session.stock_config().padding);
    let model_bboxes = session.collect_model_bboxes();

    println!(
        "{:<3} {:<32} {:>14} {:>14} {:>12} {:>12} {:>12}",
        "id", "name", "feed", "plunge", "stepover", "dpp", "rpm"
    );

    // WP7: the session mutates through `ProjectSession::apply`. The loop
    // reads one config, edits a CLONE, and writes the clone back with the
    // whole-config row.
    let toolpath_count = session.toolpath_count();
    for index in 0..toolpath_count {
        let Some(stored) = session.toolpath_config_at(index) else {
            continue;
        };
        if !stored.enabled {
            continue;
        }
        let mut tc = stored.clone();
        let Some(tool) = tools_snapshot.iter().find(|t| Some(t.id) == tc.tool_id) else {
            eprintln!("skip {} (tool {:?} missing)", tc.id, tc.tool_id);
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
        // P29-9: this example holds no session door; it states the model,
        // the stock and the dressups, and no setup frame or earlier leave.
        let context = SuggestContext::no_project(NoProject {
            model_bbox,
            stock: Some(&stock_ctx),
            dressups: Some(&tc.dressups),
            ..NoProject::NONE
        });
        let suggested = match suggest_for_operation(SuggestForOperationInput {
            operation: &tc.operation,
            tool,
            machine: &machine,
            material: &material,
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
        // v3.0d: spindle_rpm is now written by the Suggest speeds write
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
            // S11: a feed can be not set.
            format!("{} -> {}", fmt_opt(feed_before), fmt_opt(feed_after)),
            format!("{} -> {}", fmt_opt(plunge_before), fmt_opt(plunge_after)),
            format!(
                "{} -> {}",
                fmt_opt(stepover_before),
                fmt_opt(stepover_after)
            ),
            format!("{} -> {}", fmt_opt(dpp_before), fmt_opt(dpp_after)),
            format!("{} -> {}", fmt_opt_u32(rpm_before), fmt_opt_u32(rpm_after)),
        );

        let _ = session
            .apply(Command::ReplaceToolpathConfig(ReplaceToolpathConfigArgs {
                index,
                config: Box::new(tc),
            }))
            .map_err(|e| anyhow!("write the suggested operation back: {e}"))?;
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
