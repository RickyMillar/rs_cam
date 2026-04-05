use std::fmt::Write;

use crate::state::job::{JobState, ToolConfig, ToolId};
use crate::state::toolpath::{OperationConfig, ToolpathEntry};

/// Extract feed rate (mm/min) from an operation config.
fn feed_rate_of(op: &OperationConfig) -> f64 {
    op.feed_rate()
}

/// Extract depth (mm) from an operation config, if applicable.
fn depth_of(op: &OperationConfig) -> Option<f64> {
    match op.depth_semantics() {
        crate::state::toolpath::DepthSemantics::Explicit(value)
        | crate::state::toolpath::DepthSemantics::DerivedStockTop(value) => Some(value),
        crate::state::toolpath::DepthSemantics::None => None,
    }
}

/// Look up a tool by id, returning None if not found.
fn find_tool(tools: &[ToolConfig], id: ToolId) -> Option<&ToolConfig> {
    tools.iter().find(|t| t.id == id)
}

/// Format a duration in seconds as "Xm Ys".
fn format_time(seconds: f64) -> String {
    if seconds < 0.0 || !seconds.is_finite() {
        return "N/A".to_owned();
    }
    let total_secs = seconds.round() as u64;
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    if mins > 0 {
        format!("{}m {}s", mins, secs)
    } else {
        format!("{}s", secs)
    }
}

/// Estimate machining time for a single toolpath entry (cutting only).
/// Returns seconds, or None if no result or zero feed rate.
fn estimate_time(tp: &ToolpathEntry) -> Option<f64> {
    let result = tp.result.as_ref()?;
    let feed = feed_rate_of(&tp.operation);
    if feed <= 0.0 {
        return None;
    }
    // feed_rate is mm/min, cutting_distance is mm => time in minutes
    let minutes = result.stats.cutting_distance / feed;
    Some(minutes * 60.0)
}

/// HTML-escape a string to prevent injection.
fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Generate an HTML setup sheet from the current job state.
///
/// The returned string is a self-contained HTML document with inline CSS
/// (dark theme) that can be saved to a file and opened in any browser.
/// It documents stock dimensions, tools, operations, and per-operation
/// statistics for use by CNC operators.
pub fn generate_setup_sheet(job: &JobState) -> String {
    let mut html = String::with_capacity(8192);

    // Compute total estimated time across all enabled toolpaths with results.
    let total_seconds: f64 = job
        .all_toolpaths()
        .filter(|tp| tp.enabled)
        .filter_map(estimate_time)
        .sum();

    // Get the current date as YYYY-MM-DD.
    let date = {
        let now = std::time::SystemTime::now();
        let secs = now
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        // Simple date calculation (no chrono dependency).
        let days = secs / 86400;
        let (year, month, day) = days_to_ymd(days);
        format!("{:04}-{:02}-{:02}", year, month, day)
    };

    // --- Document start ---
    let _ = write!(
        html,
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Setup Sheet - {name}</title>
<style>
body {{ background: #1e1e24; color: #c8c8d2; font-family: -apple-system, sans-serif; max-width: 900px; margin: 0 auto; padding: 20px; }}
h1 {{ color: #e0e0ea; border-bottom: 2px solid #3a3a4a; padding-bottom: 8px; }}
h2 {{ color: #b0b0c0; margin-top: 24px; }}
table {{ border-collapse: collapse; width: 100%; margin: 12px 0; }}
th {{ background: #2a2a36; color: #a0a0b0; text-align: left; padding: 8px 12px; border: 1px solid #3a3a4a; }}
td {{ padding: 6px 12px; border: 1px solid #3a3a4a; }}
tr:nth-child(even) {{ background: #24242e; }}
.meta {{ color: #888; font-size: 0.9em; }}
.flip-instruction {{ background: #3a3520; border: 2px solid #d4a020; border-radius: 6px; padding: 12px 16px; margin: 12px 0; color: #f0d060; font-size: 1.1em; font-weight: bold; }}
</style>
</head>
<body>
"#,
        name = escape_html(&job.name),
    );

    // --- Header ---
    let _ = write!(
        html,
        "<h1>Setup Sheet: {}</h1>\n\
         <p class=\"meta\">Generated: {} | Estimated machining time: {}</p>\n",
        escape_html(&job.name),
        escape_html(&date),
        format_time(total_seconds),
    );

    // --- Stock section ---
    let _ = write!(
        html,
        "<h2>Stock</h2>\n\
         <table>\n\
         <tr><th>Dimension</th><th>Value</th></tr>\n\
         <tr><td>Size</td><td>{:.2} x {:.2} x {:.2} mm</td></tr>\n\
         <tr><td>Origin</td><td>({:.2}, {:.2}, {:.2})</td></tr>\n\
         </table>\n",
        job.stock.x,
        job.stock.y,
        job.stock.z,
        job.stock.origin_x,
        job.stock.origin_y,
        job.stock.origin_z,
    );

    // --- Setup orientation section ---
    if job.setups.len() > 1 {
        let _ = writeln!(html, "<h2>Setups</h2>");
        let mut prev_face = crate::state::job::FaceUp::Top;
        for (i, setup) in job.setups.iter().enumerate() {
            let _ = write!(
                html,
                "<h3>Setup {}: {}</h3>\n\
                 <p>Orientation: {} up, Z rotation: {}</p>\n",
                i + 1,
                escape_html(&setup.name),
                setup.face_up.label(),
                setup.z_rotation.label(),
            );
            if i > 0 && setup.face_up != prev_face {
                let _ = writeln!(
                    html,
                    "<div class=\"flip-instruction\">{}</div>",
                    setup.face_up.flip_instruction(),
                );
            }
            prev_face = setup.face_up;
        }
    }

    // --- Workholding section ---
    let has_workholding = job
        .setups
        .iter()
        .any(|setup| !setup.fixtures.is_empty() || !setup.keep_out_zones.is_empty());
    if has_workholding {
        let _ = writeln!(html, "<h2>Workholding</h2>");
        for setup in &job.setups {
            if setup.fixtures.is_empty() && setup.keep_out_zones.is_empty() {
                continue;
            }
            if job.setups.len() > 1 {
                let _ = writeln!(html, "<h3>{}</h3>", escape_html(&setup.name));
            }
            let _ = write!(
                html,
                "<table>\n\
                 <tr><th>Name</th><th>Type</th><th>Position</th><th>Size</th><th>Clearance</th></tr>\n"
            );
            for fixture in &setup.fixtures {
                let _ = writeln!(
                    html,
                    "<tr><td>{}</td><td>{}</td><td>({:.1}, {:.1}, {:.1})</td>\
                     <td>{:.1} x {:.1} x {:.1} mm</td><td>{:.1} mm</td></tr>",
                    escape_html(&fixture.name),
                    fixture.kind.label(),
                    fixture.origin_x,
                    fixture.origin_y,
                    fixture.origin_z,
                    fixture.size_x,
                    fixture.size_y,
                    fixture.size_z,
                    fixture.clearance,
                );
            }
            for keep_out in &setup.keep_out_zones {
                let _ = writeln!(
                    html,
                    "<tr><td>{}</td><td>Keep-Out</td><td>({:.1}, {:.1})</td>\
                     <td>{:.1} x {:.1} mm</td><td>-</td></tr>",
                    escape_html(&keep_out.name),
                    keep_out.origin_x,
                    keep_out.origin_y,
                    keep_out.size_x,
                    keep_out.size_y,
                );
            }
            let _ = writeln!(html, "</table>");
        }
    }

    // --- Datum / Alignment section ---
    let has_datum_info = !job.stock.alignment_pins.is_empty()
        || job.setups.iter().any(|setup| {
            !setup.datum.notes.is_empty()
                || setup.datum.xy_method != crate::state::job::XYDatum::default()
                || setup.datum.z_method != crate::state::job::ZDatum::default()
        });
    if has_datum_info || job.setups.len() > 1 {
        let _ = writeln!(html, "<h2>Datum / Alignment</h2>");

        // Stock-level alignment pins (shared across all setups).
        if !job.stock.alignment_pins.is_empty() {
            let _ = write!(
                html,
                "<h4>Alignment Pins</h4>\n\
                 <table>\n\
                 <tr><th>Pin</th><th>Position</th><th>Diameter</th></tr>\n"
            );
            for (pin_index, pin) in job.stock.alignment_pins.iter().enumerate() {
                let _ = writeln!(
                    html,
                    "<tr><td>{}</td><td>({:.1}, {:.1})</td><td>{:.1} mm</td></tr>",
                    pin_index + 1,
                    pin.x,
                    pin.y,
                    pin.diameter,
                );
            }
            let _ = writeln!(html, "</table>");
        }

        for (i, setup) in job.setups.iter().enumerate() {
            if job.setups.len() > 1 {
                let _ = writeln!(
                    html,
                    "<h3>Setup {}: {}</h3>",
                    i + 1,
                    escape_html(&setup.name)
                );
            }
            let _ = write!(
                html,
                "<table>\n\
                 <tr><td>XY Method</td><td>{}</td></tr>\n\
                 <tr><td>Z Method</td><td>{}</td></tr>\n\
                 </table>\n",
                setup.datum.xy_method.label(),
                setup.datum.z_method.label(),
            );
            if !setup.datum.notes.is_empty() {
                let _ = writeln!(
                    html,
                    "<p class=\"meta\">Notes: {}</p>",
                    escape_html(&setup.datum.notes)
                );
            }
            if i > 0
                && matches!(
                    setup.datum.xy_method,
                    crate::state::job::XYDatum::AlignmentPins
                )
            {
                let _ = writeln!(
                    html,
                    "<div class=\"flip-instruction\">Insert dowels into alignment pin holes. Part self-locates on pins.</div>"
                );
            }
        }
    }

    // --- Tool table ---
    let _ = write!(
        html,
        "<h2>Tools</h2>\n\
         <table>\n\
         <tr><th>#</th><th>Name</th><th>Type</th><th>Diameter</th><th>Flute Length</th></tr>\n"
    );
    for (i, tool) in job.tools.iter().enumerate() {
        let _ = writeln!(
            html,
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{:.2} mm</td><td>{:.2} mm</td></tr>",
            i + 1,
            escape_html(&tool.name),
            tool.tool_type.label(),
            tool.diameter,
            tool.cutting_length,
        );
    }
    let _ = writeln!(html, "</table>");

    // --- Operations table ---
    let _ = write!(
        html,
        "<h2>Operations</h2>\n\
         <table>\n\
         <tr><th>#</th><th>Name</th><th>Tool</th><th>Type</th><th>Feed Rate</th><th>Depth</th><th>Est. Time</th></tr>\n"
    );
    for (i, tp) in job.toolpaths_enumerated() {
        let tool_name = find_tool(&job.tools, tp.tool_id)
            .map(|t| t.name.as_str())
            .unwrap_or("(unknown)");
        let feed = feed_rate_of(&tp.operation);
        let depth_str = match depth_of(&tp.operation) {
            Some(d) => format!("{:.2} mm", d),
            None => "-".to_owned(),
        };
        let time_str = estimate_time(tp)
            .map(format_time)
            .unwrap_or_else(|| "-".to_owned());
        let enabled_marker = if tp.enabled { "" } else { " (disabled)" };

        let _ = writeln!(
            html,
            "<tr><td>{}</td><td>{}{}</td><td>{}</td><td>{}</td><td>{:.0} mm/min</td><td>{}</td><td>{}</td></tr>",
            i + 1,
            escape_html(&tp.name),
            enabled_marker,
            escape_html(tool_name),
            tp.operation.label(),
            feed,
            depth_str,
            time_str,
        );
    }
    let _ = writeln!(html, "</table>");

    // --- Post-processor info ---
    let _ = write!(
        html,
        "<h2>Post-Processor</h2>\n\
         <table>\n\
         <tr><th>Setting</th><th>Value</th></tr>\n\
         <tr><td>Format</td><td>{}</td></tr>\n\
         <tr><td>Spindle Speed</td><td>{} RPM</td></tr>\n\
         <tr><td>Safe Z</td><td>{:.2} mm</td></tr>\n\
         </table>\n",
        job.post.format.label(),
        job.post.spindle_speed,
        job.post.safe_z,
    );

    // --- Per-operation details ---
    let has_details = job
        .all_toolpaths()
        .any(|tp| tp.enabled && tp.result.is_some());

    if has_details {
        let _ = writeln!(html, "<h2>Toolpath Details</h2>");

        for tp in job.all_toolpaths() {
            if !tp.enabled {
                continue;
            }
            let Some(result) = &tp.result else {
                continue;
            };

            let _ = write!(
                html,
                "<h3 style=\"color:#b0b0c0;margin-top:16px\">{}</h3>\n\
                 <table>\n\
                 <tr><th>Metric</th><th>Value</th></tr>\n\
                 <tr><td>Move Count</td><td>{}</td></tr>\n\
                 <tr><td>Cutting Distance</td><td>{:.1} mm</td></tr>\n\
                 <tr><td>Rapid Distance</td><td>{:.1} mm</td></tr>\n\
                 </table>\n",
                escape_html(&tp.name),
                result.stats.move_count,
                result.stats.cutting_distance,
                result.stats.rapid_distance,
            );
        }
    }

    // --- Document end ---
    let _ = write!(html, "</body>\n</html>\n");

    html
}

/// Convert days since Unix epoch to (year, month, day).
///
/// Uses a basic calendar algorithm; no leap-second precision needed for a
/// date stamp on a setup sheet.
fn days_to_ymd(days_since_epoch: u64) -> (u64, u64, u64) {
    // Algorithm from Howard Hinnant's date library (public domain).
    let z = days_since_epoch + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::state::job::{
        AlignmentPin, FaceUp, Fixture, FixtureId, JobState, KeepOutId, KeepOutZone, Setup, SetupId,
        ToolConfig, ToolType, XYDatum, ZDatum, ZRotation,
    };
    use crate::state::toolpath::{
        ComputeStatus, OperationConfig, PocketConfig, ToolpathEntry, ToolpathResult, ToolpathStats,
    };
    use std::sync::Arc;

    fn make_test_job() -> JobState {
        let mut job = JobState::new();
        job.name = "Test Job".to_owned();

        // Add a tool.
        let tool_id = job.next_tool_id();
        job.tools
            .push(ToolConfig::new_default(tool_id, ToolType::EndMill));

        // Add a toolpath with a result.
        let tp_id = job.next_toolpath_id();
        let mut toolpath = ToolpathEntry::from_init(
            crate::state::toolpath::ToolpathEntryInit::from_loaded_state(
                tp_id,
                "Pocket 1".to_owned(),
                tool_id,
                crate::state::job::ModelId(0),
                OperationConfig::Pocket(PocketConfig {
                    feed_rate: 1000.0,
                    ..PocketConfig::default()
                }),
            ),
        );
        toolpath.status = ComputeStatus::Done;
        toolpath.result = Some(ToolpathResult {
            toolpath: Arc::new(rs_cam_core::toolpath::Toolpath::new()),
            stats: ToolpathStats {
                move_count: 150,
                cutting_distance: 5000.0,
                rapid_distance: 200.0,
            },
            debug_trace: None,
            semantic_trace: None,
            debug_trace_path: None,
        });
        job.push_toolpath(toolpath);

        job
    }

    fn make_multi_setup_job() -> JobState {
        let mut job = make_test_job();
        let second_setup_id = SetupId(1);

        {
            let top_setup = &mut job.setups[0];
            top_setup.name = "Top Side".to_owned();
            top_setup.datum.xy_method = XYDatum::AlignmentPins;
            top_setup.datum.notes = "Probe on the pin pair".to_owned();
            job.stock
                .alignment_pins
                .push(AlignmentPin::new(10.0, 20.0, 6.0));

            let mut fixture = Fixture::new_default(FixtureId(0));
            fixture.name = "Toe Clamp".to_owned();
            top_setup.fixtures.push(fixture);

            let mut keep_out = KeepOutZone::new_default(KeepOutId(0));
            keep_out.name = "Vice Travel".to_owned();
            top_setup.keep_out_zones.push(keep_out);
        }

        let mut bottom_setup = Setup::new(second_setup_id, "Bottom Side".to_owned());
        bottom_setup.face_up = FaceUp::Bottom;
        bottom_setup.z_rotation = ZRotation::Deg90;
        bottom_setup.datum.xy_method = XYDatum::AlignmentPins;
        bottom_setup.datum.z_method = ZDatum::MachineTable;
        job.stock
            .alignment_pins
            .push(AlignmentPin::new(12.0, 22.0, 6.0));
        job.setups.push(bottom_setup);
        job.sync_next_ids();

        job
    }

    #[test]
    fn setup_sheet_contains_job_name() {
        let job = make_test_job();
        let html = generate_setup_sheet(&job);
        assert!(html.contains("Test Job"));
    }

    #[test]
    fn setup_sheet_contains_stock_dimensions() {
        let job = make_test_job();
        let html = generate_setup_sheet(&job);
        assert!(html.contains("100.00 x 100.00 x 25.00 mm"));
    }

    #[test]
    fn setup_sheet_contains_tool_info() {
        let job = make_test_job();
        let html = generate_setup_sheet(&job);
        assert!(html.contains("End Mill"));
        assert!(html.contains("6.35 mm"));
    }

    #[test]
    fn setup_sheet_contains_operation_info() {
        let job = make_test_job();
        let html = generate_setup_sheet(&job);
        assert!(html.contains("Pocket 1"));
        assert!(html.contains("1000 mm/min"));
    }

    #[test]
    fn setup_sheet_contains_post_info() {
        let job = make_test_job();
        let html = generate_setup_sheet(&job);
        assert!(html.contains("GRBL"));
        assert!(html.contains("18000 RPM"));
    }

    #[test]
    fn setup_sheet_contains_toolpath_details() {
        let job = make_test_job();
        let html = generate_setup_sheet(&job);
        assert!(html.contains("150")); // move_count
        assert!(html.contains("5000.0 mm")); // cutting_distance
        assert!(html.contains("200.0 mm")); // rapid_distance
    }

    #[test]
    fn setup_sheet_estimated_time() {
        let job = make_test_job();
        let html = generate_setup_sheet(&job);
        // 5000mm / 1000mm/min = 5 min = 300s => "5m 0s"
        assert!(html.contains("5m 0s"));
    }

    #[test]
    fn setup_sheet_contains_setup_orientation_and_workholding() {
        let job = make_multi_setup_job();
        let html = generate_setup_sheet(&job);
        assert!(html.contains("<h2>Setups</h2>"));
        assert!(html.contains("Setup 2: Bottom Side"));
        assert!(html.contains("Orientation: Bottom up, Z rotation: 90 deg"));
        assert!(html.contains("Flip 180 deg on X axis"));
        assert!(html.contains("<h2>Workholding</h2>"));
        assert!(html.contains("Toe Clamp"));
        assert!(html.contains("Vice Travel"));
    }

    #[test]
    fn setup_sheet_contains_datum_and_alignment_sections() {
        let job = make_multi_setup_job();
        let html = generate_setup_sheet(&job);
        assert!(html.contains("<h2>Datum / Alignment</h2>"));
        assert!(html.contains("Alignment Pins"));
        assert!(html.contains("Machine Table"));
        assert!(html.contains("Probe on the pin pair"));
        assert!(html.contains("Insert dowels into alignment pin holes"));
    }

    #[test]
    fn format_time_renders_correctly() {
        assert_eq!(format_time(0.0), "0s");
        assert_eq!(format_time(59.0), "59s");
        assert_eq!(format_time(60.0), "1m 0s");
        assert_eq!(format_time(125.0), "2m 5s");
        assert_eq!(format_time(-1.0), "N/A");
        assert_eq!(format_time(f64::NAN), "N/A");
    }

    #[test]
    fn escape_html_works() {
        assert_eq!(
            escape_html("<b>\"test\" & 'it'</b>"),
            "&lt;b&gt;&quot;test&quot; &amp; &#39;it&#39;&lt;/b&gt;"
        );
    }

    #[test]
    fn setup_sheet_is_valid_html() {
        let job = make_test_job();
        let html = generate_setup_sheet(&job);
        assert!(html.starts_with("<!DOCTYPE html>"));
        assert!(html.contains("</html>"));
    }

    #[test]
    fn days_to_ymd_epoch() {
        let (y, m, d) = days_to_ymd(0);
        assert_eq!((y, m, d), (1970, 1, 1));
    }

    #[test]
    fn days_to_ymd_known_date() {
        // 2024-01-01 is day 19723 since epoch
        let (y, m, d) = days_to_ymd(19723);
        assert_eq!((y, m, d), (2024, 1, 1));
    }
}
