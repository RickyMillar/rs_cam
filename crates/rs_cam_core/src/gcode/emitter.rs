//! Render a `Program` IR to bytes using a data-driven `PostDefinition`.
//!
//! This is the Phase 3 successor to the per-impl `PostProcessor` trait:
//! a single emit function walks `Statement`s and formats each line with
//! the post's `Decimals`/`CommentStyle`/template strings. Move-line
//! shape (`G0 X{x} Y{y} Z{z}`, etc.) is identical across the three
//! shipped posts, so it is hard-coded here and parameterized by
//! `decimals.{xyz,feed,ijk}` rather than templated. Multi-line blocks
//! (preamble, postamble, program-pause) come from `PostDefinition`
//! templates with `{spindle_rpm}` / `{message_comment}` substitution.
//!
//! Byte-parity vs the legacy `PostProcessor` trait is enforced by the
//! `gcode_emitter_parity` integration test.

use std::fmt::Write;

use super::ir::{Program, Statement};
use super::post::PostDefinition;

/// Render a `Program` to g-code text using the given `PostDefinition`.
pub fn emit_program(program: &Program, post: &PostDefinition) -> String {
    let mut output = String::new();
    for statement in &program.statements {
        emit_statement(&mut output, statement, post);
    }
    output
}

/// Clamp `requested` to `max` if `max` is set and `requested > max`.
/// On clamp, append a comment line documenting the substitution so
/// operators can spot the post-modified value before running the file.
fn clamp_rpm(output: &mut String, post: &PostDefinition, requested: u32) -> u32 {
    if let Some(max) = post.limits.max_rpm
        && requested > max.get()
    {
        let line = post.render_comment(&format!(
            "WARNING: requested S{requested} clamped to S{} ({} max_rpm)",
            max.get(),
            post.name
        ));
        output.push_str(&line);
        return max.get();
    }
    requested
}

fn clamp_feed(output: &mut String, post: &PostDefinition, requested: f64) -> f64 {
    if let Some(max) = post.limits.max_feed
        && requested > max.get()
    {
        let line = post.render_comment(&format!(
            "WARNING: requested F{requested:.1} clamped to F{:.1} ({} max_feed)",
            max.get(),
            post.name
        ));
        output.push_str(&line);
        return max.get();
    }
    requested
}

/// True if an arc with incremental centre `(i, j)` should be emitted
/// as a chord instead of `G2`/`G3`. Radius computed from the IJK
/// offset since I/J are start-relative.
fn should_linearize_arc(post: &PostDefinition, i: f64, j: f64) -> bool {
    if !post.arc_linearize.enabled {
        return false;
    }
    let r = (i * i + j * j).sqrt();
    r < post.arc_linearize.threshold_mm
}

/// Scan a single line of `Statement::Raw` text and return `Some(n)` if
/// it issues an M-code listed in `post.unsupported_mcodes`.
fn unsupported_mcode_in_line(line: &str, denylist: &[u32]) -> Option<u32> {
    if denylist.is_empty() {
        return None;
    }
    for token in line.split_whitespace() {
        if let Some(rest) = token.strip_prefix('M').or_else(|| token.strip_prefix('m'))
            && let Ok(n) = rest.parse::<u32>()
            && denylist.contains(&n)
        {
            return Some(n);
        }
    }
    None
}

/// True if the line issues a cutter-comp word (G40/G41/G42).
fn line_uses_cutter_comp(line: &str) -> bool {
    for token in line.split_whitespace() {
        let upper = token.to_ascii_uppercase();
        if upper == "G40" || upper == "G41" || upper == "G42" {
            return true;
        }
        // Tool-comp variant `G41 D<n>` shows up as one token "G41" so
        // the simple equality above catches it. `G41.1`/`G42.1` (dynamic
        // comp) would also reject here — that's the correct behaviour
        // for a post that says supports_cutter_comp = false.
        if upper.starts_with("G41.") || upper.starts_with("G42.") {
            return true;
        }
    }
    false
}

/// Filter a `Statement::Raw` text against the post's denylists. Each
/// line containing an unsupported M-code or (when comp is unsupported)
/// a cutter-comp word is replaced by a warning comment.
fn filter_raw(text: &str, post: &PostDefinition) -> String {
    let needs_mcode_filter = !post.unsupported_mcodes.is_empty();
    let needs_comp_filter = !post.supports_cutter_comp;
    if !needs_mcode_filter && !needs_comp_filter {
        return text.to_owned();
    }
    let trailing_nl = text.ends_with('\n');
    let body = text.strip_suffix('\n').unwrap_or(text);
    let mut out = String::with_capacity(text.len());
    for (idx, line) in body.split('\n').enumerate() {
        if idx > 0 {
            out.push('\n');
        }
        if needs_mcode_filter
            && let Some(n) = unsupported_mcode_in_line(line, &post.unsupported_mcodes)
        {
            // render_comment includes a trailing newline; strip it so the
            // outer split('\n') reassembly stays consistent.
            let comment = post.render_comment(&format!(
                "WARNING: M{n} unsupported on {}; dropped: {}",
                post.name,
                line.trim()
            ));
            out.push_str(comment.trim_end_matches('\n'));
            continue;
        }
        if needs_comp_filter && line_uses_cutter_comp(line) {
            let comment = post.render_comment(&format!(
                "WARNING: cutter compensation unsupported on {}; dropped: {}",
                post.name,
                line.trim()
            ));
            out.push_str(comment.trim_end_matches('\n'));
            continue;
        }
        out.push_str(line);
    }
    if trailing_nl {
        out.push('\n');
    }
    out
}

fn emit_statement(output: &mut String, statement: &Statement, post: &PostDefinition) {
    let xyz = post.decimals.xyz;
    let feed_dp = post.decimals.feed;
    let ijk = post.decimals.ijk;

    match *statement {
        Statement::Preamble { spindle_rpm } => {
            let rpm = clamp_rpm(output, post, spindle_rpm);
            output.push_str(&post.render_preamble(rpm));
        }
        Statement::SpindleSet { rpm } => {
            let rpm = clamp_rpm(output, post, rpm);
            let _ = writeln!(output, "M3 S{rpm}");
        }
        Statement::Postamble => output.push_str(&post.render_postamble()),
        Statement::ProgramPause { ref message } => {
            output.push_str(&post.render_program_pause(message));
        }
        Statement::Comment(ref text) => output.push_str(&post.render_comment(text)),
        Statement::Raw(ref text) => output.push_str(&filter_raw(text, post)),
        Statement::Rapid { x, y, z } => {
            let _ = writeln!(output, "G0 X{x:.xyz$} Y{y:.xyz$} Z{z:.xyz$}");
        }
        Statement::Linear { x, y, z, feed } => {
            let feed = clamp_feed(output, post, feed);
            let _ = writeln!(
                output,
                "G1 X{x:.xyz$} Y{y:.xyz$} Z{z:.xyz$} F{feed:.feed_dp$}"
            );
        }
        Statement::LinearModal { x, y, z } => {
            let _ = writeln!(output, "G1 X{x:.xyz$} Y{y:.xyz$} Z{z:.xyz$}");
        }
        Statement::ArcCw { x, y, z, i, j, feed } => {
            let feed = clamp_feed(output, post, feed);
            if should_linearize_arc(post, i, j) {
                // Sub-threshold arc — emit as a chord. Some controllers
                // (Grbl 1.1, rs274ngc) reject sub-mm arcs outright.
                let _ = writeln!(
                    output,
                    "G1 X{x:.xyz$} Y{y:.xyz$} Z{z:.xyz$} F{feed:.feed_dp$}"
                );
            } else {
                let _ = writeln!(
                    output,
                    "G2 X{x:.xyz$} Y{y:.xyz$} Z{z:.xyz$} I{i:.ijk$} J{j:.ijk$} F{feed:.feed_dp$}"
                );
            }
        }
        Statement::ArcCcw { x, y, z, i, j, feed } => {
            let feed = clamp_feed(output, post, feed);
            if should_linearize_arc(post, i, j) {
                let _ = writeln!(
                    output,
                    "G1 X{x:.xyz$} Y{y:.xyz$} Z{z:.xyz$} F{feed:.feed_dp$}"
                );
            } else {
                let _ = writeln!(
                    output,
                    "G3 X{x:.xyz$} Y{y:.xyz$} Z{z:.xyz$} I{i:.ijk$} J{j:.ijk$} F{feed:.feed_dp$}"
                );
            }
        }
        Statement::SafeZRetract { z } => {
            let _ = writeln!(output, "G0 Z{z:.xyz$}");
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::gcode::post;
    use crate::gcode::program_builder;
    use crate::geo::P3;
    use crate::toolpath::Toolpath;

    #[test]
    fn emit_basic_grbl_lines() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp.feed_to(P3::new(10.0, 0.0, 0.0), 1000.0);
        tp.feed_to(P3::new(20.0, 0.0, 0.0), 1000.0);

        let program = program_builder::build_single(&tp, 18_000);
        let gcode = emit_program(&program, post::grbl());

        assert!(gcode.contains("(Generated by rs_cam)"));
        assert!(gcode.contains("M3 S18000"));
        assert!(gcode.contains("G0 X0.000 Y0.000 Z10.000"));
        assert!(gcode.contains("G1 X10.000 Y0.000 Z0.000 F1000"));
        // Modal F-elision on second linear at the same feed.
        assert!(gcode.contains("G1 X20.000 Y0.000 Z0.000\n"));
        assert!(gcode.contains("M30"));
    }

    #[test]
    fn emit_linuxcnc_uses_4_decimals_and_g54() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(1.234567, 0.0, 5.0));
        tp.feed_to(P3::new(1.234567, 1.0, -2.0), 600.0);

        let program = program_builder::build_single(&tp, 18_000);
        let gcode = emit_program(&program, post::linuxcnc());

        assert!(gcode.contains("G54"));
        assert!(gcode.contains("G0 X1.2346 Y0.0000 Z5.0000"));
        assert!(gcode.contains("F600.0"));
        assert!(gcode.contains("G53 G0 Z0"));
        assert!(gcode.contains("M2"));
    }

    #[test]
    fn emit_mach3_dwells_after_spindle() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));

        let program = program_builder::build_single(&tp, 18_000);
        let gcode = emit_program(&program, post::mach3());

        assert!(gcode.contains("G4 P2"));
        assert!(gcode.contains("G28 G91 Z0"));
    }

    /// Custom post with max_rpm + max_feed; emitter must clamp both
    /// the preamble RPM and feed words, prepending a comment that
    /// names the requested vs clamped value.
    #[test]
    fn post_limits_clamp_rpm_and_feed() {
        let toml = r#"
            name = "Capped"
            preamble = """\
M3 S{spindle_rpm}
"""
            postamble = "M30\n"
            program_pause = "M0\n"
            [decimals]
            xyz = 3
            feed = 0
            ijk = 3
            [comment]
            format = "({text})"
            [limits]
            max_rpm = 12000
            max_feed = 800.0
        "#;
        let post = crate::gcode::PostDefinition::from_toml(toml).expect("toml");
        let mut tp = Toolpath::new();
        tp.feed_to(P3::new(1.0, 0.0, 0.0), 1500.0);

        let program = program_builder::build_single(&tp, 24_000);
        let gcode = emit_program(&program, &post);

        // RPM clamp: requested 24000 → 12000, with warning comment.
        assert!(gcode.contains("M3 S12000"), "rpm not clamped: {gcode}");
        assert!(gcode.contains("S24000"), "warning should mention requested: {gcode}");
        assert!(gcode.contains("max_rpm"));
        // Feed clamp: requested 1500 → 800, with warning comment.
        assert!(gcode.contains("F800"), "feed not clamped: {gcode}");
        assert!(gcode.contains("F1500"), "warning should mention requested: {gcode}");
        assert!(gcode.contains("max_feed"));
    }

    /// Limits unset (shipped TOMLs): emitter must NOT alter or annotate
    /// the requested values.
    #[test]
    fn no_limits_means_no_clamp_no_warning() {
        let mut tp = Toolpath::new();
        tp.feed_to(P3::new(1.0, 0.0, 0.0), 9999.0);

        let program = program_builder::build_single(&tp, 30_000);
        let gcode = emit_program(&program, post::grbl());

        assert!(gcode.contains("M3 S30000"));
        assert!(gcode.contains("F9999"));
        assert!(!gcode.contains("WARNING"));
        assert!(!gcode.contains("clamped"));
    }
}
