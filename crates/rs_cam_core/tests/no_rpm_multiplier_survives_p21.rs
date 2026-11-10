//! Feeds contract P21c (`planning/feeds_contract_2026-11-03/phases/P21c.md`,
//! `P21_DESIGN.md` section 3): no RPM multiplier survives. The constant-chip
//! RPM stops at a named limit; the speed-up factor, the spindle headroom,
//! the diameter tiers and the legacy limits of P21a are deleted.
//!
//! A source scan of `crates/rs_cam_core/src`, comments removed:
//!
//! - no file holds one of the deleted names;
//! - the RPM-choice door `feeds/calculator/spindle.rs` holds no float
//!   literal other than 0.0, 0.5 (the bisection midpoint) and 1.0 (the
//!   power ratio of a cut that fits), so no factor scales a limit there.
//!
//! The viz crate read two of the names in a hover; the compiler holds that
//! (the constants do not exist), so this scan reads core only.
//!
//! Red: add `const MAX_SPINDLE_SPEEDUP: f64 = 1.5;` to `feeds/mod.rs`, or
//! multiply a limit by `0.95` in `spindle.rs`.

#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure; the shared scan module uses `expect`"
)]

#[path = "common/source_scan.rs"]
mod source_scan;

use std::path::PathBuf;

/// The names that P21c deleted.
const DELETED: [&str; 9] = [
    "MAX_SPINDLE_SPEEDUP",
    "SPINDLE_CEILING_HEADROOM",
    "MILLING_CEILING",
    "milling_rpm_ceiling",
    "step2b_",
    "LegacySpeedup",
    "LegacyHeadroom",
    "LegacyTier",
    "MaxSpeedPolicy",
];

fn src_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn code_of(path: &std::path::Path) -> String {
    let src =
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    source_scan::strip_comments(&src)
}

/// The float literals (`digits.digits`) of comment-free text.
fn float_literals(code: &str) -> Vec<String> {
    let chars: Vec<char> = code.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let starts_number = chars[i].is_ascii_digit()
            && (i == 0 || !(chars[i - 1].is_alphanumeric() || chars[i - 1] == '_'));
        if !starts_number {
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '_') {
            i += 1;
        }
        let has_fraction = i + 1 < chars.len() && chars[i] == '.' && chars[i + 1].is_ascii_digit();
        if has_fraction {
            i += 1;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '_') {
                i += 1;
            }
            out.push(chars[start..i].iter().collect());
        }
    }
    out
}

#[test]
fn no_rpm_multiplier_survives_p21() {
    let files = source_scan::rust_files(&src_dir());
    // Non-vacuity: the scan reads the RPM-choice door and the limit type.
    let door = src_dir().join("feeds/calculator/spindle.rs");
    let door_code = code_of(&door);
    assert!(
        door_code.contains("fn step5c_constant_chip"),
        "the scan does not see the RPM-choice door"
    );
    assert!(
        code_of(&src_dir().join("feeds/mod.rs")).contains("pub enum RpmLimit"),
        "the scan does not see the limit type"
    );
    assert!(
        files.len() > 100,
        "the scan reads too few files: {}",
        files.len()
    );

    let mut hits = Vec::new();
    for path in &files {
        let code = code_of(path);
        for name in DELETED {
            if code.contains(name) {
                hits.push(format!("{}: {name}", path.display()));
            }
        }
    }
    assert!(hits.is_empty(), "a deleted RPM cap survives: {hits:#?}");

    // The door scales no limit: its only float literals are 0.0, 0.5 and
    // 1.0.
    let literals = float_literals(&door_code);
    assert!(
        literals.iter().any(|l| l == "0.5"),
        "non-vacuity: the scan finds the bisection midpoint: {literals:?}"
    );
    let strange: Vec<&String> = literals
        .iter()
        .filter(|l| !matches!(l.as_str(), "0.0" | "0.5" | "1.0"))
        .collect();
    assert!(
        strange.is_empty(),
        "the RPM-choice door holds a factor: {strange:?}"
    );
}
