//! G-code invariant validator.
//!
//! Runs after `emit_gcode_*` and produces `Finding`s for machine-safety
//! issues that the emitter currently does (or could) introduce. The
//! validator is the safety net under the eventual data-driven post:
//! even if a `PostDefinition` TOML is wrong, the validator catches the
//! known-dangerous emissions before they reach the operator.
//!
//! # Phase 1 scope
//!
//! Five rules are implemented now, each tied to a confirmed or
//! spec-identified bug from `planning/gcode_gap_report.md`:
//!
//! - [`FindingKind::UnsupportedM6`] — Grbl 1.1 doesn't implement M6.
//!   Confirmed via `grbl-sim`'s `gvalidate` rejecting our F5 capture.
//! - [`FindingKind::MissingG91_1`] — LinuxCNC defaults to absolute IJK
//!   for arcs; without `G91.1` in the preamble, `G2/G3 I.. J..` blocks
//!   may be interpreted as absolute centers (latent crash risk).
//! - [`FindingKind::WrongProgramEndCode`] — LinuxCNC uses `M30` (with
//!   modal reset), not `M2` (without). Modal state pinned across
//!   restarts on some controllers when `M2` is used.
//! - [`FindingKind::MissingProgramBrackets`] — LinuxCNC requires `%`
//!   tape brackets at first and last line for many streamers.
//! - [`FindingKind::MissingWcs`] — every program should explicitly
//!   select a work coordinate system (G54-G59) before the first
//!   cutting move; relying on the controller's last-used WCS is a
//!   subtle wrong-origin trap.
//!
//! # Out of scope for Phase 1
//!
//! Modal-state tracking rules (M6 must be preceded by spindle stop +
//! safe-Z; G0 must be preceded by Z lift to safe-Z; first cut after
//! M3 must dwell) are deferred. They need a proper modal-state
//! machine which slots in cleanly with the Phase 2 IR refactor.
//! Encoding-style rules (feed decimals, R-format arc radius) wait for
//! the data-driven post (Phase 3) since they reference per-post
//! configuration.

use crate::gcode::PostFormat;

/// Severity of a validator finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Cosmetic deviation; no effect on machine behavior.
    Info,
    /// Suspicious but not necessarily dangerous; operator should review.
    Warning,
    /// Confirmed safety / correctness issue; do not run without fixing.
    Error,
}

/// What the rule that produced this finding was checking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindingKind {
    /// `M6` block emitted on a post that doesn't support tool changes.
    UnsupportedM6,
    /// `G91.1` (incremental IJK) absent on a post that requires it.
    MissingG91_1,
    /// Program end M-code doesn't match the post's required code
    /// (e.g. emitting `M2` where `M30` is required).
    WrongProgramEndCode,
    /// Program tape brackets (`%`) missing on a post that requires them.
    MissingProgramBrackets,
    /// No `G54`-`G59` block before the first cutting move.
    MissingWcs,

    // ── Phase 2: machine-safety modal rules (see `validate_machine_safety`).
    /// A rapid (`G0`) repositioned in X/Y while below the clearance
    /// plane — the tool traverses through the part / fixturing.
    RapidBelowClearance,
    /// A cutting move (`G1`/`G2`/`G3`) is reached with the spindle off.
    SpindleNotRunningAtCut,
    /// A commanded Z goes below the program's allowed depth floor
    /// (runaway plunge past the planned bottom).
    ZBelowProgramFloor,
    /// A feed (`F`) word exceeds the machine's maximum feed.
    FeedExceedsMax,
    /// The program ends without stopping the spindle (`M5`).
    SpindleLeftRunning,
    /// The program ends with the tool below the clearance plane
    /// (no final retract).
    ProgramEndsBelowClearance,
}

/// One validator finding tied to a specific line of g-code.
#[derive(Debug, Clone)]
pub struct Finding {
    pub severity: Severity,
    pub kind: FindingKind,
    /// 1-indexed line number in the input g-code.
    pub line: usize,
    pub message: String,
}

/// Per-post invariant configuration. Eventually becomes a field of
/// `PostDefinition` (Phase 3); for Phase 1 it's a hardcoded lookup.
#[derive(Debug, Clone, Copy)]
struct PostInvariants {
    /// Whether the post may emit `M6` for tool changes. Grbl 1.1
    /// doesn't support M6 in its parser; LinuxCNC and Mach3 do.
    supports_m6: bool,
    /// Whether `G91.1` (incremental IJK) must appear in the preamble.
    /// LinuxCNC requires this; Grbl and Mach3 default to incremental.
    requires_g91_1: bool,
    /// Whether the program must be wrapped in `%` tape brackets.
    /// LinuxCNC requires this for many streamers.
    requires_percent_brackets: bool,
    /// The numeric M-code that ends the program (M30 = end with
    /// modal reset; M2 = end without). All three Fusion posts use 30.
    program_end_code: u32,
    /// Whether a WCS code (G54-G59) must appear before the first
    /// cutting move. All shipped posts: yes.
    requires_wcs: bool,
}

const fn invariants_for(post: PostFormat) -> PostInvariants {
    match post {
        PostFormat::Grbl => PostInvariants {
            supports_m6: false,
            requires_g91_1: false,
            requires_percent_brackets: false,
            program_end_code: 30,
            requires_wcs: true,
        },
        PostFormat::LinuxCnc => PostInvariants {
            supports_m6: true,
            requires_g91_1: true,
            requires_percent_brackets: true,
            program_end_code: 30,
            requires_wcs: true,
        },
        PostFormat::Mach3 => PostInvariants {
            supports_m6: true,
            requires_g91_1: false,
            requires_percent_brackets: false,
            program_end_code: 30,
            requires_wcs: true,
        },
        // grblHAL is a strict superset of Grbl 1.1 with full M6 ATC
        // support. The grblhal post emits an explicit G54 (validator
        // still requires WCS).
        PostFormat::GrblHal => PostInvariants {
            supports_m6: true,
            requires_g91_1: false,
            requires_percent_brackets: false,
            program_end_code: 30,
            requires_wcs: true,
        },
    }
}

/// Validate `gcode` against the invariants of `post`. Returns all
/// findings (potentially empty if the program is clean).
pub fn validate(gcode: &str, post: PostFormat) -> Vec<Finding> {
    let inv = invariants_for(post);
    let lines: Vec<&str> = gcode.lines().collect();
    let mut findings = Vec::new();

    rule_unsupported_m6(&inv, &lines, post, &mut findings);
    rule_missing_g91_1(&inv, &lines, post, &mut findings);
    rule_wrong_program_end_code(&inv, &lines, post, &mut findings);
    rule_missing_program_brackets(&inv, &lines, post, &mut findings);
    rule_missing_wcs(&inv, &lines, post, &mut findings);

    findings
}

// ── helpers ──────────────────────────────────────────────────────────

/// Strip `(...)` comment ranges (and trailing `;` line comments)
/// before scanning for codes. G-code comments must not trigger rules.
fn strip_comments(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut depth: i32 = 0;
    for ch in line.chars() {
        match ch {
            '(' => depth += 1,
            ')' if depth > 0 => depth -= 1,
            ';' if depth == 0 => break,
            _ if depth == 0 => out.push(ch),
            _ => {}
        }
    }
    out
}

/// Returns true if `line` contains the word `<letter><number>` as a
/// distinct token (not a substring of a longer word like G91.1).
/// Comparison is integer; trailing decimal extensions like `91.1` are
/// distinct from `91`.
fn has_word_int(line: &str, letter: char, number: u32) -> bool {
    let cleaned = strip_comments(line).to_uppercase();
    let target = letter.to_ascii_uppercase();
    let bytes = cleaned.as_bytes();
    let mut i = 0;
    while let Some(&byte) = bytes.get(i) {
        if byte as char == target {
            // Read the integer part of the following digits.
            let mut j = i + 1;
            let start = j;
            while bytes.get(j).is_some_and(u8::is_ascii_digit) {
                j += 1;
            }
            if j > start {
                let digits = cleaned.get(start..j).unwrap_or("");
                let int_part: u32 = digits.parse().unwrap_or(u32::MAX);
                // Ensure there's no decimal extension (e.g. 91.1).
                let has_decimal = bytes.get(j).copied() == Some(b'.');
                if int_part == number && !has_decimal {
                    return true;
                }
            }
            i = j;
        } else {
            i += 1;
        }
    }
    false
}

/// Returns true if `line` contains the word `G91.1` (incremental IJK
/// arc-center mode in LinuxCNC).
fn has_g91_1(line: &str) -> bool {
    let cleaned = strip_comments(line).to_uppercase();
    // Walk and look for G91.1 as a full token.
    let bytes = cleaned.as_bytes();
    let mut i = 0;
    while let Some(window) = bytes.get(i..i + 5) {
        if window == b"G91.1" {
            // Make sure no trailing digit extends it (e.g. G91.10).
            let next = bytes.get(i + 5).copied().unwrap_or(b' ');
            if !next.is_ascii_digit() {
                return true;
            }
        }
        i += 1;
    }
    false
}

/// Returns the index of the first cutting move (G1/G2/G3) in `lines`,
/// if any. Returns `None` for a program with no cutting moves.
fn first_cutting_move_index(lines: &[&str]) -> Option<usize> {
    lines.iter().position(|line| {
        has_word_int(line, 'G', 1) || has_word_int(line, 'G', 2) || has_word_int(line, 'G', 3)
    })
}

// ── rules ────────────────────────────────────────────────────────────

fn rule_unsupported_m6(
    inv: &PostInvariants,
    lines: &[&str],
    post: PostFormat,
    findings: &mut Vec<Finding>,
) {
    if inv.supports_m6 {
        return;
    }
    for (i, line) in lines.iter().enumerate() {
        if has_word_int(line, 'M', 6) {
            findings.push(Finding {
                severity: Severity::Error,
                kind: FindingKind::UnsupportedM6,
                line: i + 1,
                message: format!(
                    "{} doesn't implement M6 (tool change). The controller will reject this block. \
                     Either disable tool-change emission for this post or fall back to a T<n> + operator-prompt comment.",
                    post.label()
                ),
            });
        }
    }
}

fn rule_missing_g91_1(
    inv: &PostInvariants,
    lines: &[&str],
    post: PostFormat,
    findings: &mut Vec<Finding>,
) {
    if !inv.requires_g91_1 {
        return;
    }
    let g91_1_idx = lines.iter().position(|l| has_g91_1(l));
    let first_motion = first_cutting_move_index(lines);

    let problem_line = match (g91_1_idx, first_motion) {
        (None, Some(fm)) => Some(fm + 1),
        (Some(g), Some(fm)) if g >= fm => Some(fm + 1),
        _ => None,
    };

    if let Some(line) = problem_line {
        findings.push(Finding {
            severity: Severity::Error,
            kind: FindingKind::MissingG91_1,
            line,
            message: format!(
                "{} requires G91.1 (incremental IJK arc-center mode) before the first cutting move. \
                 Without it, G2/G3 I.. J.. blocks may be interpreted as absolute IJK and the toolpath will deviate \
                 from the intended arc — latent machine-crash risk.",
                post.label()
            ),
        });
    }
}

fn rule_wrong_program_end_code(
    inv: &PostInvariants,
    lines: &[&str],
    post: PostFormat,
    findings: &mut Vec<Finding>,
) {
    // Look at the last non-blank, non-comment-only line for the end code.
    let mut last_meaningful: Option<(usize, &str)> = None;
    for (i, line) in lines.iter().enumerate() {
        let cleaned = strip_comments(line);
        if !cleaned.trim().is_empty() {
            last_meaningful = Some((i, line));
        }
    }
    let Some((idx, line)) = last_meaningful else {
        return;
    };

    // Found end code → check it's the right one.
    let has_m30 = has_word_int(line, 'M', 30);
    let has_m2 = has_word_int(line, 'M', 2);
    if !has_m30 && !has_m2 {
        // No end code at all — separate concern; not what this rule covers.
        return;
    }
    let actual = if has_m30 {
        30
    } else if has_m2 {
        2
    } else {
        return;
    };
    if actual != inv.program_end_code {
        findings.push(Finding {
            severity: Severity::Error,
            kind: FindingKind::WrongProgramEndCode,
            line: idx + 1,
            message: format!(
                "{} should end with M{} (modal reset), not M{}. M2 ends without resetting modal state, \
                 which can cause the next program to inherit unexpected G-codes (G91, G54-relative offsets, etc.).",
                post.label(),
                inv.program_end_code,
                actual
            ),
        });
    }
}

fn rule_missing_program_brackets(
    inv: &PostInvariants,
    lines: &[&str],
    post: PostFormat,
    findings: &mut Vec<Finding>,
) {
    if !inv.requires_percent_brackets {
        return;
    }
    let first_non_blank = lines.iter().position(|l| !l.trim().is_empty());
    let last_non_blank = lines.iter().rposition(|l| !l.trim().is_empty());

    let leading_ok = first_non_blank.is_some_and(|i| lines.get(i).is_some_and(|l| l.trim() == "%"));
    let trailing_ok = last_non_blank.is_some_and(|i| lines.get(i).is_some_and(|l| l.trim() == "%"));

    if !leading_ok {
        findings.push(Finding {
            severity: Severity::Error,
            kind: FindingKind::MissingProgramBrackets,
            line: 1,
            message: format!(
                "{} requires a `%` on the first line of the program (tape begin). \
                 Some streamers refuse the program without it.",
                post.label()
            ),
        });
    }
    if !trailing_ok {
        findings.push(Finding {
            severity: Severity::Error,
            kind: FindingKind::MissingProgramBrackets,
            line: lines.len(),
            message: format!(
                "{} requires a `%` on the last line of the program (tape end). \
                 Without it the controller may continue parsing into garbage memory.",
                post.label()
            ),
        });
    }
}

fn rule_missing_wcs(
    inv: &PostInvariants,
    lines: &[&str],
    post: PostFormat,
    findings: &mut Vec<Finding>,
) {
    if !inv.requires_wcs {
        return;
    }
    let Some(first_motion) = first_cutting_move_index(lines) else {
        return;
    };

    // Check for any WCS code (G54-G59) before the first cutting move.
    let wcs_present = (54..=59).any(|n| {
        lines
            .iter()
            .take(first_motion)
            .any(|l| has_word_int(l, 'G', n))
    });

    if !wcs_present {
        findings.push(Finding {
            severity: Severity::Error,
            kind: FindingKind::MissingWcs,
            line: first_motion + 1,
            message: format!(
                "{} program reaches its first cutting move (line {}) without selecting a WCS (G54-G59). \
                 The controller will use whatever WCS was last active — wrong-origin risk.",
                post.label(),
                first_motion + 1
            ),
        });
    }
}

// ── Phase 2: machine-safety modal pass ───────────────────────────────
//
// These rules need geometric context — the clearance plane, the depth
// floor, the machine's max feed — that the post-format `validate` entry
// doesn't carry, so they live behind a separate entry. The pass walks
// the program once, tracking the active motion mode, tool position,
// spindle state and feed. It is the last automated check on the actual
// bytes that reach the controller, independent of the dexel simulator
// (which validates the toolpath IR, not the emitted text), so it catches
// emitter / post bugs the sim can't see.
//
// Every rule is deliberately conservative: it fires only on an
// unambiguous violation, because a false "all clear" on a first real cut
// is worse than no check at all.

/// Geometric / machine limits the safety pass checks the program
/// against. Supply these from the post + machine profile at export time.
#[derive(Debug, Clone, Copy)]
pub struct MachineSafety {
    /// Clearance plane Z. A rapid (`G0`) that changes X or Y must stay
    /// at or above this height for the whole move — no traversing
    /// through the part. Use the post's safe-Z.
    pub clearance_z: f64,
    /// Deepest Z the program may command (a negative value: stock bottom
    /// minus any allowed spoilboard margin). `None` disables the floor
    /// check.
    pub min_z: Option<f64>,
    /// Maximum feed in mm/min. `F` words above this are flagged (the
    /// firmware will clamp, but a feed exceeding the machine's
    /// `$110`/`$111` indicates a planner/post bug). `None` disables.
    pub max_feed_mm_min: Option<f64>,
}

#[derive(Clone, Copy, PartialEq)]
enum Motion {
    Rapid,
    Feed,
    Arc,
}

/// Parse the signed decimal value of word `<letter>` from a
/// comment-stripped, upper-cased line. Returns the first occurrence.
/// Intended for axis / feed words (X/Y/Z/F/S), not integer G/M codes —
/// use [`has_word_int`] for those.
fn word_value(cleaned_upper: &str, letter: char) -> Option<f64> {
    let target = letter.to_ascii_uppercase();
    let bytes = cleaned_upper.as_bytes();
    let mut i = 0;
    while let Some(&b) = bytes.get(i) {
        if b as char == target {
            let start = i + 1;
            let mut j = start;
            while bytes
                .get(j)
                .is_some_and(|c| c.is_ascii_digit() || matches!(*c, b'.' | b'-' | b'+'))
            {
                j += 1;
            }
            if j > start
                && let Some(tok) = cleaned_upper.get(start..j)
                && let Ok(v) = tok.parse::<f64>()
            {
                return Some(v);
            }
        }
        i += 1;
    }
    None
}

/// Run the machine-safety modal pass over `gcode`. Returns all findings
/// (possibly empty). Complements [`validate`]: that checks post-format
/// invariants; this checks motion safety against `cfg`.
pub fn validate_machine_safety(gcode: &str, cfg: MachineSafety) -> Vec<Finding> {
    const EPS: f64 = 1e-6;
    let mut findings = Vec::new();

    let mut motion: Option<Motion> = None;
    let (mut x, mut y, mut z): (Option<f64>, Option<f64>, Option<f64>) = (None, None, None);
    let mut spindle_on = false;
    let mut first_cut_checked = false;
    let mut feed_flagged = false;
    let mut floor_flagged = false;
    let mut last_line_no = 0usize;

    for (idx, raw) in gcode.lines().enumerate() {
        let line_no = idx + 1;
        let cleaned = strip_comments(raw).to_uppercase();
        if cleaned.trim().is_empty() {
            continue;
        }
        last_line_no = line_no;

        // Spindle modal state.
        if has_word_int(raw, 'M', 3) || has_word_int(raw, 'M', 4) {
            spindle_on = true;
        }
        if has_word_int(raw, 'M', 5) {
            spindle_on = false;
        }

        // Motion modal state.
        if has_word_int(raw, 'G', 0) {
            motion = Some(Motion::Rapid);
        } else if has_word_int(raw, 'G', 1) {
            motion = Some(Motion::Feed);
        } else if has_word_int(raw, 'G', 2) || has_word_int(raw, 'G', 3) {
            motion = Some(Motion::Arc);
        }

        let nx = word_value(&cleaned, 'X');
        let ny = word_value(&cleaned, 'Y');
        let nz = word_value(&cleaned, 'Z');
        let nf = word_value(&cleaned, 'F');

        // Feed ceiling (applies to any F word).
        if let (Some(f), Some(max)) = (nf, cfg.max_feed_mm_min)
            && f > max + EPS
            && !feed_flagged
        {
            feed_flagged = true;
            findings.push(Finding {
                severity: Severity::Warning,
                kind: FindingKind::FeedExceedsMax,
                line: line_no,
                message: format!(
                    "Feed F{f:.0} mm/min exceeds the machine maximum {max:.0} mm/min. \
                     The controller will clamp it, but the planner/post emitted a feed the \
                     machine can't reach — the cut will run slower than planned."
                ),
            });
        }

        let has_axis = nx.is_some() || ny.is_some() || nz.is_some();
        if !has_axis {
            continue;
        }

        let pre_z = z;
        let dest_z = nz.or(z);
        let dx_changed = nx.is_some_and(|v| x.is_none_or(|c| (c - v).abs() > EPS));
        let dy_changed = ny.is_some_and(|v| y.is_none_or(|c| (c - v).abs() > EPS));

        // Depth floor.
        if let (Some(cz), Some(min)) = (nz, cfg.min_z)
            && cz < min - EPS
            && !floor_flagged
        {
            floor_flagged = true;
            findings.push(Finding {
                severity: Severity::Error,
                kind: FindingKind::ZBelowProgramFloor,
                line: line_no,
                message: format!(
                    "Commanded Z{cz:.3} is below the program depth floor {min:.3} — \
                     runaway plunge past the planned bottom (into spoilboard / table)."
                ),
            });
        }

        match motion {
            Some(Motion::Rapid) => {
                if dx_changed || dy_changed {
                    // The whole rapid must clear the part: check the
                    // lower of the start and end heights (linear-interp
                    // controllers sweep XY while descending). Unknown
                    // start (program origin) is treated as safe.
                    let lo = match (pre_z, dest_z) {
                        (Some(a), Some(b)) => a.min(b),
                        (None, Some(b)) => b,
                        (Some(a), None) => a,
                        (None, None) => f64::INFINITY,
                    };
                    if lo < cfg.clearance_z - EPS {
                        findings.push(Finding {
                            severity: Severity::Error,
                            kind: FindingKind::RapidBelowClearance,
                            line: line_no,
                            message: format!(
                                "Rapid (G0) repositions in X/Y at Z{lo:.3}, below the clearance \
                                 plane {:.3}. The tool traverses through the part / fixturing — \
                                 retract to clearance before any rapid XY move.",
                                cfg.clearance_z
                            ),
                        });
                    }
                }
            }
            Some(Motion::Feed) | Some(Motion::Arc) => {
                if !first_cut_checked {
                    first_cut_checked = true;
                    if !spindle_on {
                        findings.push(Finding {
                            severity: Severity::Error,
                            kind: FindingKind::SpindleNotRunningAtCut,
                            line: line_no,
                            message: "First cutting move (G1/G2/G3) reached with the spindle off \
                                 (no preceding M3/M4). Cutting with a stopped spindle stalls \
                                 the motor or snaps the bit."
                                .to_owned(),
                        });
                    }
                }
            }
            None => {}
        }

        if let Some(v) = nx {
            x = Some(v);
        }
        if let Some(v) = ny {
            y = Some(v);
        }
        if let Some(v) = nz {
            z = Some(v);
        }
    }

    if spindle_on {
        findings.push(Finding {
            severity: Severity::Error,
            kind: FindingKind::SpindleLeftRunning,
            line: last_line_no,
            message: "Program ends with the spindle still running (no M5).".to_owned(),
        });
    }
    if let Some(final_z) = z
        && final_z < cfg.clearance_z - EPS
    {
        findings.push(Finding {
            severity: Severity::Warning,
            kind: FindingKind::ProgramEndsBelowClearance,
            line: last_line_no,
            message: format!(
                "Program ends with the tool at Z{final_z:.3}, below the clearance plane \
                 {:.3} — no final retract.",
                cfg.clearance_z
            ),
        });
    }

    findings
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;

    fn count_kind(findings: &[Finding], kind: FindingKind) -> usize {
        findings.iter().filter(|f| f.kind == kind).count()
    }

    // ── helper tests ────────────────────────────────────────────────

    #[test]
    fn has_word_int_distinguishes_g91_from_g91_1() {
        assert!(has_word_int("G91", 'G', 91));
        assert!(!has_word_int("G91.1", 'G', 91));
        assert!(!has_word_int("G91", 'G', 9));
        assert!(!has_word_int("G91", 'G', 911));
    }

    #[test]
    fn has_word_int_ignores_comments() {
        assert!(!has_word_int("(M6 in a comment)", 'M', 6));
        assert!(!has_word_int("G1 X10 ; M6 line comment", 'M', 6));
        assert!(has_word_int("M6 (CHANGE TOOL)", 'M', 6));
    }

    #[test]
    fn has_g91_1_recognises_only_full_token() {
        assert!(has_g91_1("G90 G94 G17 G91.1"));
        assert!(has_g91_1("G91.1\n"));
        assert!(!has_g91_1("G91"));
        assert!(!has_g91_1("G91.10"));
        assert!(!has_g91_1("(emit G91.1 here)"));
    }

    // ── rule tests ──────────────────────────────────────────────────

    #[test]
    fn unsupported_m6_flags_grbl_only() {
        let prog = "M6 T1\nG1 X10 F600\nM30\n";
        let f_grbl = validate(prog, PostFormat::Grbl);
        assert_eq!(count_kind(&f_grbl, FindingKind::UnsupportedM6), 1);

        let f_lcnc = validate(prog, PostFormat::LinuxCnc);
        assert_eq!(count_kind(&f_lcnc, FindingKind::UnsupportedM6), 0);

        let f_mach3 = validate(prog, PostFormat::Mach3);
        assert_eq!(count_kind(&f_mach3, FindingKind::UnsupportedM6), 0);
    }

    #[test]
    fn missing_g91_1_only_flagged_for_linuxcnc() {
        let bad = "G90 G21 G17\nG54\nG1 X10 F600\nM30\n";
        let good = "G90 G21 G17 G91.1\nG54\nG1 X10 F600\nM30\n";

        assert_eq!(
            count_kind(
                &validate(bad, PostFormat::LinuxCnc),
                FindingKind::MissingG91_1
            ),
            1
        );
        assert_eq!(
            count_kind(
                &validate(good, PostFormat::LinuxCnc),
                FindingKind::MissingG91_1
            ),
            0
        );
        assert_eq!(
            count_kind(&validate(bad, PostFormat::Grbl), FindingKind::MissingG91_1),
            0
        );
        assert_eq!(
            count_kind(&validate(bad, PostFormat::Mach3), FindingKind::MissingG91_1),
            0
        );
    }

    #[test]
    fn wrong_program_end_code_flags_m2_for_linuxcnc() {
        let bad = "G90 G91.1\nG54\nG1 X10 F600\nM2\n";
        let good = "G90 G91.1\nG54\nG1 X10 F600\nM30\n";
        assert_eq!(
            count_kind(
                &validate(bad, PostFormat::LinuxCnc),
                FindingKind::WrongProgramEndCode
            ),
            1
        );
        assert_eq!(
            count_kind(
                &validate(good, PostFormat::LinuxCnc),
                FindingKind::WrongProgramEndCode
            ),
            0
        );
    }

    #[test]
    fn missing_program_brackets_flags_linuxcnc_without_percent() {
        let bad = "G90 G91.1\nG54\nG1 X10 F600\nM30\n";
        let good = "%\nG90 G91.1\nG54\nG1 X10 F600\nM30\n%\n";
        assert_eq!(
            count_kind(
                &validate(bad, PostFormat::LinuxCnc),
                FindingKind::MissingProgramBrackets
            ),
            2
        );
        assert_eq!(
            count_kind(
                &validate(good, PostFormat::LinuxCnc),
                FindingKind::MissingProgramBrackets
            ),
            0
        );
        // Grbl/Mach3 don't need brackets.
        assert_eq!(
            count_kind(
                &validate(bad, PostFormat::Grbl),
                FindingKind::MissingProgramBrackets
            ),
            0
        );
    }

    #[test]
    fn missing_wcs_flags_program_with_no_g54_before_first_cut() {
        let bad = "G90 G21\nG0 X0 Y0 Z5\nG1 X10 F600\nM30\n";
        let good = "G90 G21\nG54\nG0 X0 Y0 Z5\nG1 X10 F600\nM30\n";
        assert_eq!(
            count_kind(&validate(bad, PostFormat::Grbl), FindingKind::MissingWcs),
            1
        );
        assert_eq!(
            count_kind(&validate(good, PostFormat::Grbl), FindingKind::MissingWcs),
            0
        );
        // G55-G59 should also satisfy.
        let g55 = "G90 G21\nG55\nG1 X10 F600\nM30\n";
        assert_eq!(
            count_kind(&validate(g55, PostFormat::Grbl), FindingKind::MissingWcs),
            0
        );
    }

    #[test]
    fn clean_program_produces_no_findings_on_any_post() {
        // A program that satisfies all five rules across all dialects
        // (using G91.1 and percent brackets which are no-ops on Grbl/Mach3).
        let clean = "%\nG90 G21 G17 G91.1\nG54\nG0 X0 Y0 Z5\nG1 X10 Y0 Z-2 F600\nM30\n%\n";
        for &post in PostFormat::ALL {
            let findings = validate(clean, post);
            assert!(
                findings.is_empty(),
                "expected zero findings on clean program for {:?}, got: {:?}",
                post,
                findings
            );
        }
    }

    // ── machine-safety pass ─────────────────────────────────────────

    fn safety() -> MachineSafety {
        MachineSafety {
            clearance_z: 5.0,
            min_z: Some(-3.0),
            max_feed_mm_min: Some(1000.0),
        }
    }

    /// A well-formed GRBL program: WCS, spindle on, rapid at clearance,
    /// plunge, cut, retract, spindle off, retract, end.
    const SAFE_PROG: &str = "\
(Generated by rs_cam)
G17 G21 G90
G54
M3 S18000
G0 X0.000 Y0.000 Z5.000
G1 Z-2.000 F200
G1 X10.000 Y0.000 F600
G0 Z5.000
M5
G0 Z10.000
M30
";

    #[test]
    fn machine_safety_clean_program_is_silent() {
        let f = validate_machine_safety(SAFE_PROG, safety());
        assert!(f.is_empty(), "expected no findings, got: {f:?}");
    }

    #[test]
    fn flags_rapid_xy_below_clearance() {
        // After cutting at Z-2, a G0 repositions in XY without retracting.
        let prog =
            "G54\nM3 S1000\nG0 X0 Y0 Z5\nG1 Z-2 F200\nG1 X10 F600\nG0 X20 Y20\nG0 Z10\nM5\nM30\n";
        let f = validate_machine_safety(prog, safety());
        assert_eq!(count_kind(&f, FindingKind::RapidBelowClearance), 1);
    }

    #[test]
    fn does_not_flag_z_only_retract_or_initial_approach() {
        // Initial approach (origin unknown → safe) and the Z-only
        // postamble retract must NOT be flagged.
        let prog = "G54\nM3 S1000\nG0 X0 Y0 Z5\nG1 Z-2 F200\nG1 X10 F600\nG0 Z5\nG0 X0 Y0\nM5\nG0 Z10\nM30\n";
        let f = validate_machine_safety(prog, safety());
        assert_eq!(count_kind(&f, FindingKind::RapidBelowClearance), 0);
    }

    #[test]
    fn flags_first_cut_with_spindle_off() {
        let prog = "G54\nG0 X0 Y0 Z5\nG1 Z-2 F200\nG0 Z10\nM30\n";
        let f = validate_machine_safety(prog, safety());
        assert_eq!(count_kind(&f, FindingKind::SpindleNotRunningAtCut), 1);
    }

    #[test]
    fn flags_z_below_floor() {
        let prog = "G54\nM3 S1000\nG0 X0 Y0 Z5\nG1 Z-9 F200\nG0 Z10\nM5\nM30\n";
        let f = validate_machine_safety(prog, safety());
        assert_eq!(count_kind(&f, FindingKind::ZBelowProgramFloor), 1);
    }

    #[test]
    fn flags_feed_over_max() {
        let prog = "G54\nM3 S1000\nG0 X0 Y0 Z5\nG1 Z-2 F200\nG1 X10 F5000\nG0 Z10\nM5\nM30\n";
        let f = validate_machine_safety(prog, safety());
        assert_eq!(count_kind(&f, FindingKind::FeedExceedsMax), 1);
    }

    #[test]
    fn flags_spindle_left_running() {
        let prog = "G54\nM3 S1000\nG0 X0 Y0 Z5\nG1 Z-2 F200\nG0 Z10\nM30\n";
        let f = validate_machine_safety(prog, safety());
        assert_eq!(count_kind(&f, FindingKind::SpindleLeftRunning), 1);
    }

    #[test]
    fn flags_program_ending_below_clearance() {
        // Ends at Z-2 with no retract.
        let prog = "G54\nM3 S1000\nG0 X0 Y0 Z5\nG1 Z-2 F200\nG1 X10 F600\nM5\nM30\n";
        let f = validate_machine_safety(prog, safety());
        assert_eq!(count_kind(&f, FindingKind::ProgramEndsBelowClearance), 1);
    }

    #[test]
    fn word_value_parses_signed_decimals() {
        assert_eq!(word_value("G1 X10.5 Y-2.000 Z-0.25 F600", 'X'), Some(10.5));
        assert_eq!(word_value("G1 X10.5 Y-2.000 Z-0.25 F600", 'Y'), Some(-2.0));
        assert_eq!(word_value("G1 X10.5 Y-2.000 Z-0.25 F600", 'Z'), Some(-0.25));
        assert_eq!(word_value("G1 X10.5 F600", 'Z'), None);
    }

    #[test]
    fn machine_safety_clean_on_real_grbl_capture() {
        // The committed F1 capture (post-G54 fix) must pass the safety
        // pass with a clearance below its Z5 approach.
        let cfg = MachineSafety {
            clearance_z: 5.0,
            min_z: None,
            max_feed_mm_min: None,
        };
        let prog = "(Generated by rs_cam)\nG17 G21 G90 G40 G49 G80\nG54\nM3 S18000\n\
                    G0 X0.000 Y0.000 Z5.000\nG1 X0.000 Y0.000 Z-1.000 F200\n\
                    G1 X10.000 Y0.000 Z-1.000 F400\nG1 X0.000 Y0.000 Z5.000 F1000\n\
                    M5\nG0 Z10.000\nM30\n";
        let f = validate_machine_safety(prog, cfg);
        assert!(f.is_empty(), "expected no findings, got: {f:?}");
    }
}
