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
use crate::ui::readiness::{self, CycleTimeBasis};
use rs_cam_core::session::ProjectSession;
use rs_cam_core::simulation_cut::SimulationCutTrace;

/// Generate an HTML setup sheet from session + GUI state.
///
/// `trace` is the current simulation's cut trace. It is a parameter rather
/// than something this module reaches for because the sheet is printed and
/// carried to the machine: the estimate it prints has to be the SAME quantity
/// the on-screen surfaces show, and the trace is what decides which quantity
/// that is (G-TIMEEST). Passing `None` is legitimate — it means no simulation
/// — and the sheet then says so in words rather than printing a bare number.
pub fn generate_setup_sheet_from_session(
    session: &ProjectSession,
    gui: &GuiState,
    trace: Option<&SimulationCutTrace>,
) -> String {
    let mut html = String::with_capacity(8192);

    // The project total, from the same fold the readiness panel, the pre-flight
    // gate and the export wizard use — not a local re-derivation.
    let cycle = readiness::project_cycle_time(session, gui, trace);

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
.time-basis {{ background: #33301e; border-left: 4px solid #d4a020; padding: 8px 12px; margin: 8px 0; color: #d8c88a; font-size: 0.9em; }}
.time-basis .remedy {{ color: #9a9aa8; }}
</style>
</head>
<body>
"#,
            name = escape_html(name),
        ),
    );

    // The headline time always carries its basis in the same breath — the
    // parenthetical is not decoration, it is the difference between a
    // wall-clock prediction and a cutting-only figure measured 7x optimistic.
    let (time_str, basis_str) = match cycle.basis {
        Some(basis) => (
            readiness::format_cycle_time(cycle.seconds),
            format!(" ({})", basis.qualifier()),
        ),
        None => ("\u{2014}".to_owned(), " (no estimate)".to_owned()),
    };
    let _ = std::fmt::Write::write_fmt(
        &mut html,
        format_args!(
            "<h1>Setup Sheet: {}</h1>\n\
             <p class=\"meta\">Generated: {} | Estimated machining time: {}{}</p>\n",
            escape_html(name),
            escape_html(&date),
            escape_html(&time_str),
            escape_html(&basis_str),
        ),
    );

    // Paper has no hover. A sheet that prints "2:59:41" with no indication of
    // which model produced it is precisely how one word came to cover two
    // quantities, so on this surface the caveat is body text.
    if let Some(basis) = cycle.basis
        && basis != CycleTimeBasis::MachineModel
    {
        let remedy = basis
            .remedy()
            .map(|r| format!(" <span class=\"remedy\">{}</span>", escape_html(r)))
            .unwrap_or_default();
        let _ = std::fmt::Write::write_fmt(
            &mut html,
            format_args!(
                "<p class=\"time-basis\">\u{26A0} {}{}</p>\n",
                escape_html(basis.caveat()),
                remedy,
            ),
        );
    }

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

    // Machine
    {
        let machine = session.machine();
        let (min_rpm, max_rpm) = machine.rpm_range();
        let _ = std::fmt::Write::write_fmt(
            &mut html,
            format_args!(
                "<h2>Machine</h2>\n\
                 <table>\n\
                 <tr><th>Property</th><th>Value</th></tr>\n\
                 <tr><td>Name</td><td>{}</td></tr>\n\
                 <tr><td>RPM Range</td><td>{:.0} – {:.0}</td></tr>\n\
                 <tr><td>Max Feed (travel)</td><td>{:.0} mm/min</td></tr>\n\
                 <tr><td>Max Shank</td><td>{:.1} mm</td></tr>\n",
                escape_html(&machine.name),
                min_rpm,
                max_rpm,
                machine.max_feed_mm_min,
                machine.max_shank_mm,
            ),
        );
        // Kinematics (cycle-time model) — only listed when explicitly
        // configured; the None default means the naive runtime estimate.
        if let Some(kin) = machine.kinematics {
            let accel = match kin.acceleration_xyz_mm_s2 {
                Some(a) => format!("X {:.0} / Y {:.0} / Z {:.0} mm/s²", a[0], a[1], a[2]),
                None => format!("{:.0} mm/s² (isotropic)", kin.acceleration_mm_s2),
            };
            let jerk = match kin.jerk_mm_s3 {
                Some(j) => format!("{j:.0} mm/s³"),
                None => "off (trapezoidal)".to_owned(),
            };
            let _ = std::fmt::Write::write_fmt(
                &mut html,
                format_args!(
                    "<tr><td>Acceleration</td><td>{}</td></tr>\n\
                     <tr><td>Junction deviation ($11)</td><td>{:.3} mm</td></tr>\n\
                     <tr><td>Jerk limit</td><td>{}</td></tr>\n",
                    accel, kin.junction_deviation_mm, jerk,
                ),
            );
        }
        let _ = std::fmt::Write::write_str(&mut html, "</table>\n");
    }

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
            // W9 / P-2: the datum is the operator's zeroing procedure
            // and the setup sheet is the sheet they work from, so it
            // prints here now that the value survives a save. Emitted
            // only when it is non-default, so sheets for projects that
            // never touched the Setup panel are unchanged.
            if !setup.datum.is_default() {
                let _ = std::fmt::Write::write_fmt(
                    &mut html,
                    format_args!(
                        "<p>Datum — XY: {}, Z: {}</p>\n",
                        escape_html(setup.datum.xy_method.label()),
                        escape_html(&setup.datum.z_method.label()),
                    ),
                );
                if !setup.datum.notes.is_empty() {
                    let _ = std::fmt::Write::write_fmt(
                        &mut html,
                        format_args!("<p>Datum notes: {}</p>\n", escape_html(&setup.datum.notes)),
                    );
                }
            }
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
    // The column header names the basis too, so the table stays self-describing
    // if someone photographs it away from the header line above.
    let _ = std::fmt::Write::write_fmt(
        &mut html,
        format_args!(
            "<h2>Operations</h2>\n\
             <table>\n\
             <tr><th>#</th><th>Name</th><th>Tool</th><th>Type</th><th>Feed Rate</th><th>Depth</th><th>Est. Time{}</th></tr>\n",
            escape_html(&basis_str),
        ),
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
            .map(|result| {
                let op = readiness::toolpath_cycle_time(
                    trace,
                    tc.id,
                    result.stats.cutting_distance,
                    feed,
                );
                match op.basis {
                    Some(_) => readiness::format_cycle_time(op.seconds),
                    None => "-".to_owned(),
                }
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

    // This module's private `format_time` is gone (G-TIMEEST): its
    // `"{m}m {s}s"` spelling rendered the three-hour job that motivated the row
    // as "179m 41s", and it was the second of two duration formatters
    // disagreeing about the same seconds. `readiness::format_cycle_time` is now
    // the only one; its coverage lives with it, in
    // `tests/cycle_time_basis_g_timeest.rs`.

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
