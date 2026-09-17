#![allow(clippy::print_stdout)] // CLI surface

//! `nc-time`: parse a G-code .nc file and replay it through F-034's
//! kinematics integrator to get an independent predicted cycle time.
//!
//! Useful as a cross-check after `project --emit-gcode` runs — confirms
//! the emitter didn't drop / add moves on the way from `Toolpath` IR to
//! the .nc the controller will see. Also lets us re-predict time on .nc
//! files we didn't generate (vendor G-code, hand-tweaked, etc.).
//!
//! CLI-06: `--machine <name>` reads the same `io::machine_library` the
//! GUI and MCP `load_machine_from_library` read, so a prediction can be
//! made for any machine in the library. Without the flag the command
//! keeps the built-in `shapeoko_xxl_ricky_tuned` preset it used to
//! hardcode. The feed caps come off the loaded profile through the
//! canonical mapping (`session/compute.rs`'s `LinkKinematics`): the
//! cutting-feed ceiling is the commanded-feed cap, the travel rate is
//! the rapid rate. `--max-feed` and `--rapid-feed` still override.
//!
//! Parser is intentionally minimal — only handles the G-code subset
//! rs_cam emits + common GRBL-friendly bits. Comments, M-codes,
//! S-words, T-words are all silently skipped. Modal X/Y/Z/F are
//! tracked. G0/G1 emit linear moves; G2/G3 emit arcs.

use anyhow::{Context, Result, bail};
use rs_cam_core::geo::P3;
use rs_cam_core::io::machine_library;
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::machine::kinematics::{MachineKinematics, compute_cycle_time};
use rs_cam_core::toolpath::Toolpath;
use std::path::{Path, PathBuf};

/// The machine one `nc-time` run replays against.
///
/// `kinematics_declared` is the honest half: a library profile may carry
/// no `[kinematics]` block at all, and `MachineProfile::
/// effective_kinematics` then answers the generic wood-router default.
/// That is a real answer, but it is not the named machine's answer, so
/// the run says which one it used.
#[derive(Debug)]
pub(crate) struct ReplayMachine {
    pub label: String,
    pub kinematics: MachineKinematics,
    pub max_feed_mm_min: f64,
    pub rapid_feed_mm_min: f64,
    pub kinematics_declared: bool,
}

/// The built-in default when no `--machine` is given: the operator's
/// calibrated preset, with the feed caps this command has always used.
fn builtin_machine() -> ReplayMachine {
    ReplayMachine {
        label: "shapeoko_xxl_ricky_tuned (built-in preset)".to_owned(),
        kinematics: MachineKinematics::shapeoko_xxl_ricky_tuned(),
        max_feed_mm_min: 4000.0,
        rapid_feed_mm_min: 10_000.0,
        kinematics_declared: true,
    }
}

/// Read a machine profile the way the simulator does.
///
/// The mapping is `session/compute.rs`'s `LinkKinematics`, not a second
/// reading of the same struct: `cutting_feed_ceiling_mm_min` is the cap
/// on commanded feeds (F4 — `max_feed_mm_min` is the TRAVEL rate, and
/// reading it as the cutting cap is the defect F4 fixed), and
/// `max_feed_mm_min` is the rapid rate.
pub(crate) fn machine_from_profile(profile: &MachineProfile) -> ReplayMachine {
    ReplayMachine {
        label: profile.name.clone(),
        kinematics: profile.effective_kinematics(),
        max_feed_mm_min: profile.cutting_feed_ceiling_mm_min().max(1.0),
        rapid_feed_mm_min: profile.max_feed_mm_min.max(1.0),
        kinematics_declared: profile.kinematics.is_some(),
    }
}

/// Resolve `--machine` against a library directory.
///
/// `None` keeps the built-in preset. A named machine is loaded from
/// `dir`; a missing directory or a missing name is a refusal that lists
/// what the library does hold, not a silent fall back to the preset —
/// a predicted cycle time attributed to the wrong machine is worse than
/// no prediction.
pub(crate) fn resolve_machine_in(
    dir: Option<&Path>,
    machine: Option<&str>,
) -> Result<ReplayMachine> {
    let Some(name) = machine else {
        return Ok(builtin_machine());
    };
    let Some(dir) = dir else {
        bail!(
            "--machine {name} needs a machine library, and none is set. Set \
             RS_CAM_MACHINE_DIR, XDG_CONFIG_HOME or HOME, or drop the flag to \
             use the built-in shapeoko_xxl_ricky_tuned preset."
        );
    };
    match machine_library::load_from(dir, name) {
        Ok(profile) => Ok(machine_from_profile(&profile)),
        Err(e) => {
            let available = machine_library::list_in(dir);
            let listed = if available.is_empty() {
                "nothing".to_owned()
            } else {
                available.join(", ")
            };
            bail!("{e}. The library at {} holds: {listed}.", dir.display())
        }
    }
}

/// Replay one or more .nc files through F-034 and print the predicted
/// cycle time for each.
pub fn run_nc_time(
    inputs: &[PathBuf],
    machine: Option<&str>,
    max_feed: Option<f64>,
    rapid_feed: Option<f64>,
) -> Result<()> {
    if inputs.is_empty() {
        anyhow::bail!("nc-time requires at least one input .nc file");
    }
    let resolved = resolve_machine_in(machine_library::library_dir().as_deref(), machine)?;
    let kinematics = resolved.kinematics;
    // A flag overrides the profile; the profile overrides nothing else.
    let max_feed = max_feed.unwrap_or(resolved.max_feed_mm_min);
    let rapid_feed = rapid_feed.unwrap_or(resolved.rapid_feed_mm_min);
    let junction_str = kinematics
        .max_junction_velocity_mm_min
        .map(|v| format!("{v:.0} mm/min"))
        .unwrap_or_else(|| "derived".to_owned());
    let accel_str = match kinematics.acceleration_xyz_mm_s2 {
        Some([ax, ay, az]) => format!("accel X/Y/Z {ax:.0}/{ay:.0}/{az:.0} mm/s²"),
        None => format!("accel {:.0} mm/s²", kinematics.acceleration_mm_s2),
    };
    println!(
        "kinematics: {} ({accel_str}, δ {:.3} mm, junction {})",
        resolved.label, kinematics.junction_deviation_mm, junction_str,
    );
    if !resolved.kinematics_declared {
        println!(
            "note: {} declares no kinematics; the generic wood-router \
             default is in use, so this time is NOT that machine's.",
            resolved.label
        );
    }
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

    /// CLI-06 sentry: `nc-time` reads the shared machine library, and a
    /// different machine predicts a different time.
    ///
    /// The command used to read
    /// `MachineKinematics::shapeoko_xxl_ricky_tuned()` with no parameter
    /// to change it, while `io::machine_library` — the library the GUI
    /// and MCP `load_machine_from_library` share — sat unreachable from
    /// this surface. A prediction that can only ever describe one
    /// machine is not a cross-check for any other.
    ///
    /// The directory is passed in, not read from the environment:
    /// `std::env::set_var` is `unsafe` in edition 2024 and the workspace
    /// denies `unsafe_code`.
    #[test]
    fn a_library_machine_changes_the_predicted_time() {
        let dir = std::env::temp_dir().join(format!("nc_time_library_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        let mut slow = rs_cam_core::machine::MachineProfile::generic_wood_router();
        slow.name = "slow_router".to_owned();
        slow.max_feed_mm_min = 6000.0;
        slow.kinematics = Some(MachineKinematics {
            acceleration_mm_s2: 100.0,
            acceleration_xyz_mm_s2: Some([100.0, 100.0, 100.0]),
            ..MachineKinematics::default()
        });
        let mut fast = slow.clone();
        fast.name = "fast_router".to_owned();
        fast.kinematics = Some(MachineKinematics {
            acceleration_mm_s2: 3000.0,
            acceleration_xyz_mm_s2: Some([3000.0, 3000.0, 3000.0]),
            ..MachineKinematics::default()
        });
        rs_cam_core::io::machine_library::save_to(&dir, "slow_router", &slow).unwrap();
        rs_cam_core::io::machine_library::save_to(&dir, "fast_router", &fast).unwrap();

        // A short zig-zag: acceleration decides its time, not the feed.
        let mut tp = Toolpath::new();
        for i in 0..20 {
            let x = f64::from(i % 2) * 10.0;
            tp.feed_to(P3::new(x, f64::from(i), 0.0), 3000.0);
        }

        let slow_run = resolve_machine_in(Some(&dir), Some("slow_router")).unwrap();
        let fast_run = resolve_machine_in(Some(&dir), Some("fast_router")).unwrap();
        assert_eq!(slow_run.label, "slow_router");
        assert!(slow_run.kinematics_declared);

        let slow_s = compute_cycle_time(
            &tp,
            &slow_run.kinematics,
            slow_run.max_feed_mm_min,
            slow_run.rapid_feed_mm_min,
        );
        let fast_s = compute_cycle_time(
            &tp,
            &fast_run.kinematics,
            fast_run.max_feed_mm_min,
            fast_run.rapid_feed_mm_min,
        );
        assert!(
            slow_s > fast_s * 1.5,
            "the two library machines predicted the same time \
             ({slow_s:.3} s vs {fast_s:.3} s); nc-time is not reading the profile"
        );

        // The feed caps come off the profile, not off the old hardcoded
        // flag defaults.
        assert!(
            (slow_run.rapid_feed_mm_min - 6000.0).abs() < 1e-9,
            "the rapid rate must be the profile travel rate, got {}",
            slow_run.rapid_feed_mm_min
        );
        assert!(
            (slow_run.max_feed_mm_min - slow.cutting_feed_ceiling_mm_min()).abs() < 1e-9,
            "the feed cap must be the profile cutting ceiling, got {}",
            slow_run.max_feed_mm_min
        );

        // A name the library does not hold refuses and says what it holds.
        let err = resolve_machine_in(Some(&dir), Some("not_in_the_library")).unwrap_err();
        let text = format!("{err:#}");
        assert!(
            text.contains("not_in_the_library") && text.contains("slow_router"),
            "the refusal must name the machine and list the library, got: {text}"
        );

        // No flag keeps the built-in preset.
        let builtin = resolve_machine_in(Some(&dir), None).unwrap();
        assert!(builtin.label.contains("shapeoko_xxl_ricky_tuned"));

        let _ = std::fs::remove_dir_all(&dir);
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
