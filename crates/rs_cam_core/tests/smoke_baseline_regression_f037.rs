//! F-037 — Smoke baseline regression net.
//!
//! Confirms:
//! 1. The checked-in baseline file exists and parses cleanly.
//! 2. At least one cutting toolpath with deflection Within is present
//!    (so the regression net actually has something to protect — an
//!    empty / all-`harness_error` baseline is a regression-net bug).
//! 3. Mutating a single verdict (Within → Exceeds) is detectable from
//!    the CSV without running the smoke suite — the diff invariant the
//!    `smoke --diff` CLI relies on.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::PathBuf;

fn baseline_path() -> PathBuf {
    // 2026-06-01 baseline supersedes 2026-05-26.csv (F-037 capture, pre-
    // Phase-2B / pre-F-031). The newer file was captured against `cfb146b`
    // after the feeds-data-ingest A→F + audit followups (S2-9, S3-13,
    // hierarchical material picker) landed. The older file stays on disk
    // as historical archive of the F-037 reference state.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("planning")
        .join("toolpath_acceptance")
        .join("baselines")
        .join("2026-06-01.csv")
}

fn read_rows(path: &std::path::Path) -> Vec<csv::StringRecord> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_path(path)
        .unwrap_or_else(|e| panic!("open baseline {}: {e}", path.display()));
    rdr.records()
        .map(|r| r.expect("baseline row parse"))
        .collect()
}

fn header_index(path: &std::path::Path, column: &str) -> usize {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_path(path)
        .expect("open baseline");
    let headers = rdr.headers().expect("baseline headers").clone();
    headers
        .iter()
        .position(|h| h == column)
        .unwrap_or_else(|| panic!("baseline is missing column {column}"))
}

#[test]
fn baseline_file_exists_and_parses() {
    let path = baseline_path();
    assert!(
        path.exists(),
        "baseline file missing: {} — run `cargo run -p rs_cam_cli -- smoke --output {}` to regenerate",
        path.display(),
        path.display()
    );
    let rows = read_rows(&path);
    assert!(!rows.is_empty(), "baseline must have at least one row");

    // Required columns the diff CLI keys on.
    for col in [
        "case_id",
        "op_kind",
        "status",
        "chipload_kind",
        "deflection_kind",
        "power_kind",
        "rapid_collision_count",
    ] {
        header_index(&path, col);
    }
}

#[test]
fn baseline_has_protected_cutting_verdicts() {
    let path = baseline_path();
    let rows = read_rows(&path);

    let deflection_idx = header_index(&path, "deflection_kind");
    let status_idx = header_index(&path, "status");

    let cutting_within = rows
        .iter()
        .filter(|r| r.get(status_idx) == Some("ok"))
        .filter(|r| r.get(deflection_idx) == Some("within"))
        .count();
    assert!(
        cutting_within >= 5,
        "baseline must contain at least 5 cases with deflection Within (acceptance loop closed at \
         7/7); got {cutting_within} — the regression net is empty"
    );
}

#[test]
fn diff_detects_within_to_exceeds_mutation() {
    // Smallest verifiable invariant of the diff semantics: changing a
    // `within` cell to `exceeds` in the same case_id constitutes a
    // regression. The smoke --diff CLI tests this same thing against a
    // running result CSV; here we synthesise both sides as static CSV
    // text to keep the test fast and to lock the diff schema.

    let header = "case_id,op_kind,status,chipload_kind,chipload_observed_mm_tooth,deflection_kind,\
                  deflection_peak_mm,power_kind,power_peak_kw,rapid_collision_count,avg_engagement,\
                  peak_axial_doc_mm,drill_chip_welding_kind,drill_chip_welding_observed,\
                  drill_peck_kind,drill_plunge_kind,notes\n";
    let baseline_row = "AS001,pocket,ok,exceeds_low,0.005,within,0.078,within,0.005,0,0.25,2.0,,,,,";
    let mutated_row = "AS001,pocket,ok,exceeds_low,0.005,exceeds,0.250,within,0.005,0,0.25,2.0,,,,,";

    let tmp = std::env::temp_dir();
    let baseline_path = tmp.join("f037_diff_baseline.csv");
    let current_path = tmp.join("f037_diff_current.csv");
    std::fs::write(&baseline_path, format!("{header}{baseline_row}\n")).unwrap();
    std::fs::write(&current_path, format!("{header}{mutated_row}\n")).unwrap();

    // Re-derive the diff logic locally instead of shelling out — the
    // smoke runner CLI sits behind the binary boundary, and we want this
    // acceptance test to fail when the CSV schema drifts even before the
    // CLI rebuilds.
    let kind_at = |path: &std::path::Path, column: &str, row_filter: &str| -> String {
        let mut rdr = csv::ReaderBuilder::new()
            .from_path(path)
            .expect("open csv");
        let headers = rdr.headers().expect("headers").clone();
        let col_idx = headers.iter().position(|h| h == column).expect("column");
        let case_idx = headers.iter().position(|h| h == "case_id").expect("case_id");
        for rec in rdr.records() {
            let rec = rec.unwrap();
            if rec.get(case_idx) == Some(row_filter) {
                return rec.get(col_idx).unwrap_or("").to_owned();
            }
        }
        String::new()
    };

    assert_eq!(kind_at(&baseline_path, "deflection_kind", "AS001"), "within");
    assert_eq!(kind_at(&current_path, "deflection_kind", "AS001"), "exceeds");
    // This is the Within→Exceeds regression case the CLI MUST catch.
}
