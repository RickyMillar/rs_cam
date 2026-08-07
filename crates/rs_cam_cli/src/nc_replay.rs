#![allow(clippy::print_stdout)] // CLI surface

//! `nc-time`: parse a G-code .nc file and replay it through F-034's
//! kinematics integrator to get an independent predicted cycle time.
//!
//! Useful as a cross-check after `project --emit-gcode` runs — confirms
//! the emitter didn't drop / add moves on the way from `Toolpath` IR to
//! the .nc the controller will see. Also lets us re-predict time on .nc
//! files we didn't generate (vendor G-code, hand-tweaked, etc.) using
//! the user's calibrated `shapeoko_xxl_ricky_tuned` kinematics.
//!
//! Parser is intentionally minimal — only handles the G-code subset
//! rs_cam emits + common GRBL-friendly bits. Comments, M-codes,
//! S-words, T-words are all silently skipped. Modal X/Y/Z/F are
//! tracked. G0/G1 emit linear moves; G2/G3 emit arcs.

use anyhow::{Context, Result};
use rs_cam_core::geo::P3;
use rs_cam_core::machine_kinematics::{MachineKinematics, compute_cycle_time};
use rs_cam_core::toolpath::Toolpath;
use std::path::{Path, PathBuf};

/// Replay one or more .nc files through F-034 and print the predicted
/// cycle time for each.
pub fn run_nc_time(inputs: &[PathBuf], max_feed: f64, rapid_feed: f64) -> Result<()> {
    if inputs.is_empty() {
        anyhow::bail!("nc-time requires at least one input .nc file");
    }
    let kinematics = MachineKinematics::shapeoko_xxl_ricky_tuned();
    let junction_str = kinematics
        .max_junction_velocity_mm_min
        .map(|v| format!("{v:.0} mm/min"))
        .unwrap_or_else(|| "derived".to_owned());
    let accel_str = match kinematics.acceleration_xyz_mm_s2 {
        Some([ax, ay, az]) => format!("accel X/Y/Z {ax:.0}/{ay:.0}/{az:.0} mm/s²"),
        None => format!("accel {:.0} mm/s²", kinematics.acceleration_mm_s2),
    };
    println!(
        "kinematics: shapeoko_xxl_ricky_tuned ({accel_str}, δ {:.3} mm, junction {})",
        kinematics.junction_deviation_mm, junction_str,
    );
    println!(
        "machine caps: max_feed {:.0} mm/min, rapid_feed {:.0} mm/min",
        max_feed, rapid_feed,
    );
    println!();
    println!(
        "{:>10}  {:>8}  {:>8}  {:<}",
        "time", "moves", "size_kb", "file"
    );
    println!("{}", "-".repeat(60));
    for path in inputs {
        let (toolpath, total_lines) =
            parse_nc(path).with_context(|| format!("failed to parse {}", path.display()))?;
        let time_s = compute_cycle_time(&toolpath, &kinematics, max_feed, rapid_feed);
        let size_kb = path.metadata().map(|m| m.len() / 1024).unwrap_or(0);
        let mins = (time_s / 60.0).floor() as u32;
        let secs = (time_s - f64::from(mins) * 60.0).round() as u32;
        println!(
            "{:>5}m{:>02}s  {:>8}  {:>8}  {:<}",
            mins,
            secs,
            toolpath.moves.len(),
            size_kb,
            path.display(),
        );
        let _ = total_lines; // diagnostic only
    }
    Ok(())
}

/// Parse a .nc file into a `Toolpath`. Returns `(toolpath, raw_line_count)`.
///
/// Modal state: previous X/Y/Z persist across moves; previous F (feed)
/// persists across linear/arc moves. G0/G1 set motion mode. Comments,
/// M-codes, S/T words are skipped.
fn parse_nc(path: &Path) -> Result<(Toolpath, usize)> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let mut tp = Toolpath::new();
    let mut mode_rapid = true;
    let mut x = 0.0_f64;
    let mut y = 0.0_f64;
    let mut z = 0.0_f64;
    let mut f = 1000.0_f64;
    let mut total = 0;

    for raw in content.lines() {
        total += 1;
        // Strip GRBL comments `(...)` and `;...` tail comments.
        let stripped = strip_comments(raw);
        let line = stripped.trim();
        if line.is_empty() {
            continue;
        }
        // Quick reject of M-/T-/S- lines that don't carry motion.
        if line.starts_with('%') {
            continue;
        }

        // Tokenize: words are letter + signed-decimal.
        let words = tokenize_gcode(line);
        if words.is_empty() {
            continue;
        }

        let mut new_x = x;
        let mut new_y = y;
        let mut new_z = z;
        let mut i_off: Option<f64> = None;
        let mut j_off: Option<f64> = None;
        let mut motion_kind: Option<u8> = None; // 0/1/2/3

        for (letter, value) in &words {
            match letter {
                'G' | 'g' => {
                    let g = value.round() as i32;
                    match g {
                        0 => {
                            motion_kind = Some(0);
                            mode_rapid = true;
                        }
                        1 => {
                            motion_kind = Some(1);
                            mode_rapid = false;
                        }
                        2 => motion_kind = Some(2),
                        3 => motion_kind = Some(3),
                        _ => {} // modes we don't care about: G17/21/40/49/80/90...
                    }
                }
                'X' | 'x' => new_x = *value,
                'Y' | 'y' => new_y = *value,
                'Z' | 'z' => new_z = *value,
                'F' | 'f' => f = *value,
                'I' | 'i' => i_off = Some(*value),
                'J' | 'j' => j_off = Some(*value),
                _ => {}
            }
        }

        // No motion word AND no XYZ delta → modal-only update (e.g.
        // standalone `F1500` to set feed). Don't emit a move.
        let xyz_changed =
            (new_x - x).abs() > 1e-9 || (new_y - y).abs() > 1e-9 || (new_z - z).abs() > 1e-9;
        if motion_kind.is_none() && !xyz_changed {
            continue;
        }

        let target = P3::new(new_x, new_y, new_z);
        let resolved_motion = motion_kind.unwrap_or(if mode_rapid { 0 } else { 1 });
        match resolved_motion {
            0 => {
                tp.rapid_to(target);
            }
            1 => {
                tp.feed_to(target, f);
            }
            2 => {
                let i_val = i_off.unwrap_or(0.0);
                let j_val = j_off.unwrap_or(0.0);
                tp.arc_cw_to(target, i_val, j_val, f);
            }
            3 => {
                let i_val = i_off.unwrap_or(0.0);
                let j_val = j_off.unwrap_or(0.0);
                tp.arc_ccw_to(target, i_val, j_val, f);
            }
            _ => {}
        }

        x = new_x;
        y = new_y;
        z = new_z;
    }

    Ok((tp, total))
}

/// Strip GRBL-style `(...)` parenthesized comments and `;...` tail
/// comments. GRBL doesn't support nested parens, so a single pass
/// state machine is fine.
fn strip_comments(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut in_paren = false;
    for c in line.chars() {
        match c {
            '(' => in_paren = true,
            ')' => in_paren = false,
            ';' => break, // semicolon → rest of line is comment
            _ if !in_paren => out.push(c),
            _ => {}
        }
    }
    out
}

/// Tokenize a G-code line into (letter, value) pairs. Returns `[]` for
/// lines with no valid words.
// SAFETY: every `bytes[i]` access is guarded by `i < bytes.len()` either
// directly or by the loop condition. `line[start..i]` slice is safe
// because `start` and `i` come from byte indices the ASCII-only scan
// never advances past a multi-byte boundary.
#[allow(clippy::indexing_slicing)]
fn tokenize_gcode(line: &str) -> Vec<(char, f64)> {
    let mut words = Vec::new();
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if c.is_ascii_alphabetic() {
            let letter = c;
            i += 1;
            let start = i;
            // Read the numeric literal: optional sign + digits + optional dot + digits.
            if i < bytes.len() && (bytes[i] == b'-' || bytes[i] == b'+') {
                i += 1;
            }
            while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                i += 1;
            }
            if i > start
                && let Ok(value) = line[start..i].parse::<f64>()
            {
                words.push((letter, value));
            }
        } else {
            i += 1;
        }
    }
    words
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn strip_comments_handles_paren_and_semicolon() {
        assert_eq!(strip_comments("G1 X10 (some comment)"), "G1 X10 ");
        assert_eq!(strip_comments("G1 X10 ; tail"), "G1 X10 ");
        assert_eq!(strip_comments("(only comment)"), "");
    }

    #[test]
    fn tokenize_extracts_letter_value_pairs() {
        let w = tokenize_gcode("G1 X10.5 Y-3.0 F1500");
        assert_eq!(w, vec![('G', 1.0), ('X', 10.5), ('Y', -3.0), ('F', 1500.0)]);
    }

    fn write_tmp(content: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("nc_replay_test_{}.nc", std::process::id()));
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn parse_modal_feed_persists() {
        let path = write_tmp("G1 X10 F1500\nG1 X20\nG1 X30\n");
        let (tp, _) = parse_nc(&path).unwrap();
        assert_eq!(tp.moves.len(), 3);
        for m in &tp.moves {
            match m.move_type {
                rs_cam_core::toolpath::MoveType::Linear { feed_rate } => {
                    assert!((feed_rate - 1500.0).abs() < 1e-6);
                }
                _ => panic!("expected Linear"),
            }
        }
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn parse_rapid_followed_by_linear() {
        let mut path = std::env::temp_dir();
        path.push(format!("nc_replay_test2_{}.nc", std::process::id()));
        std::fs::write(&path, "G0 X5 Y5 Z10\nG1 Z-2 F300\nG1 X20 F1500\n").unwrap();
        let (tp, _) = parse_nc(&path).unwrap();
        assert_eq!(tp.moves.len(), 3);
        assert!(matches!(
            tp.moves[0].move_type,
            rs_cam_core::toolpath::MoveType::Rapid
        ));
        assert!(matches!(
            tp.moves[1].move_type,
            rs_cam_core::toolpath::MoveType::Linear { .. }
        ));
        let _ = std::fs::remove_file(&path);
    }
}
