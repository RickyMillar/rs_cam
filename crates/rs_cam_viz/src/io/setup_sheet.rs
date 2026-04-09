use crate::state::toolpath::OperationConfig;

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

// ── Session-based setup sheet ────────────────────────────────────────

use crate::state::runtime::GuiState;
use rs_cam_core::session::ProjectSession;

/// Generate an HTML setup sheet from session + GUI state.
pub fn generate_setup_sheet_from_session(session: &ProjectSession, gui: &GuiState) -> String {
    let mut html = String::with_capacity(8192);

    // Compute total estimated time from GUI runtime results.
    let total_seconds: f64 = session
        .toolpath_configs()
        .iter()
        .filter(|tc| tc.enabled)
        .filter_map(|tc| {
            let rt = gui.toolpath_rt.get(&tc.id)?;
            let result = rt.result.as_ref()?;
            let feed = feed_rate_of(&tc.operation);
            if feed <= 0.0 {
                return None;
            }
            Some(result.stats.cutting_distance / feed * 60.0)
        })
        .sum();

    let date = {
        let now = std::time::SystemTime::now();
        let secs = now
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let days = secs / 86400;
        let (year, month, day) = days_to_ymd(days);
        format!("{:04}-{:02}-{:02}", year, month, day)
    };

    let stock = session.stock_config();
    let name = session.name();

    let _ = std::fmt::Write::write_fmt(
        &mut html,
        format_args!(
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
            name = escape_html(name),
        ),
    );

    let _ = std::fmt::Write::write_fmt(
        &mut html,
        format_args!(
            "<h1>Setup Sheet: {}</h1>\n\
             <p class=\"meta\">Generated: {} | Estimated machining time: {}</p>\n",
            escape_html(name),
            escape_html(&date),
            format_time(total_seconds),
        ),
    );

    // Stock
    let _ = std::fmt::Write::write_fmt(
        &mut html,
        format_args!(
            "<h2>Stock</h2>\n\
             <table>\n\
             <tr><th>Dimension</th><th>Value</th></tr>\n\
             <tr><td>Size</td><td>{:.2} x {:.2} x {:.2} mm</td></tr>\n\
             <tr><td>Origin</td><td>({:.2}, {:.2}, {:.2})</td></tr>\n\
             </table>\n",
            stock.x, stock.y, stock.z, stock.origin_x, stock.origin_y, stock.origin_z,
        ),
    );

    // Setups
    let setups = session.list_setups();
    if setups.len() > 1 {
        let _ = std::fmt::Write::write_str(&mut html, "<h2>Setups</h2>\n");
        let mut prev_face = rs_cam_core::compute::transform::FaceUp::Top;
        for (i, setup) in setups.iter().enumerate() {
            let _ = std::fmt::Write::write_fmt(
                &mut html,
                format_args!(
                    "<h3>Setup {}: {}</h3>\n\
                     <p>Orientation: {} up, Z rotation: {}</p>\n",
                    i + 1,
                    escape_html(&setup.name),
                    setup.face_up.label(),
                    setup.z_rotation.label(),
                ),
            );
            if i > 0 && setup.face_up != prev_face {
                let _ = std::fmt::Write::write_fmt(
                    &mut html,
                    format_args!(
                        "<div class=\"flip-instruction\">{}</div>\n",
                        setup.face_up.flip_instruction(),
                    ),
                );
            }
            prev_face = setup.face_up;
        }
    }

    // Tools
    let _ = std::fmt::Write::write_str(
        &mut html,
        "<h2>Tools</h2>\n\
         <table>\n\
         <tr><th>#</th><th>Name</th><th>Type</th><th>Diameter</th><th>Flute Length</th></tr>\n",
    );
    for (i, tool) in session.tools().iter().enumerate() {
        let _ = std::fmt::Write::write_fmt(
            &mut html,
            format_args!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{:.2} mm</td><td>{:.2} mm</td></tr>\n",
                i + 1,
                escape_html(&tool.name),
                tool.tool_type.label(),
                tool.diameter,
                tool.cutting_length,
            ),
        );
    }
    let _ = std::fmt::Write::write_str(&mut html, "</table>\n");

    // Operations
    let _ = std::fmt::Write::write_str(
        &mut html,
        "<h2>Operations</h2>\n\
         <table>\n\
         <tr><th>#</th><th>Name</th><th>Tool</th><th>Type</th><th>Feed Rate</th><th>Depth</th><th>Est. Time</th></tr>\n",
    );
    for (i, tc) in session.toolpath_configs().iter().enumerate() {
        let tool_name = session
            .tools()
            .iter()
            .find(|t| t.id.0 == tc.tool_id)
            .map(|t| t.name.as_str())
            .unwrap_or("(unknown)");
        let feed = feed_rate_of(&tc.operation);
        let depth_str = match depth_of(&tc.operation) {
            Some(d) => format!("{:.2} mm", d),
            None => "-".to_owned(),
        };
        let time_str = gui
            .toolpath_rt
            .get(&tc.id)
            .and_then(|rt| rt.result.as_ref())
            .and_then(|result| {
                if feed <= 0.0 {
                    return None;
                }
                Some(format_time(result.stats.cutting_distance / feed * 60.0))
            })
            .unwrap_or_else(|| "-".to_owned());
        let enabled_marker = if tc.enabled { "" } else { " (disabled)" };

        let _ = std::fmt::Write::write_fmt(
            &mut html,
            format_args!(
                "<tr><td>{}</td><td>{}{}</td><td>{}</td><td>{}</td><td>{:.0} mm/min</td><td>{}</td><td>{}</td></tr>\n",
                i + 1,
                escape_html(&tc.name),
                enabled_marker,
                escape_html(tool_name),
                tc.operation.label(),
                feed,
                depth_str,
                time_str,
            ),
        );
    }
    let _ = std::fmt::Write::write_str(&mut html, "</table>\n");

    // Post info
    let _ = std::fmt::Write::write_fmt(
        &mut html,
        format_args!(
            "<h2>Post-Processor</h2>\n\
             <table>\n\
             <tr><th>Setting</th><th>Value</th></tr>\n\
             <tr><td>Format</td><td>{}</td></tr>\n\
             <tr><td>Spindle Speed</td><td>{} RPM</td></tr>\n\
             <tr><td>Safe Z</td><td>{:.2} mm</td></tr>\n\
             </table>\n",
            gui.post.format.label(),
            gui.post.spindle_speed,
            gui.post.safe_z,
        ),
    );

    let _ = std::fmt::Write::write_str(&mut html, "</body>\n</html>\n");
    html
}

#[cfg(test)]
mod tests {
    use super::*;

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
