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

    let leading_ok =
        first_non_blank.is_some_and(|i| lines.get(i).is_some_and(|l| l.trim() == "%"));
    let trailing_ok =
        last_non_blank.is_some_and(|i| lines.get(i).is_some_and(|l| l.trim() == "%"));

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
    let wcs_present = (54..=59)
        .any(|n| lines.iter().take(first_motion).any(|l| has_word_int(l, 'G', n)));

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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
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
            count_kind(&validate(bad, PostFormat::LinuxCnc), FindingKind::MissingG91_1),
            1
        );
        assert_eq!(
            count_kind(&validate(good, PostFormat::LinuxCnc), FindingKind::MissingG91_1),
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
}
