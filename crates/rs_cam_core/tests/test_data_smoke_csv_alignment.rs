//! F-018 regression: assert that every row in
//! `planning/toolpath_acceptance/cases_agent_smoke.csv` references a
//! `test_data/ux_*.toml` template that
//!
//! 1. exists on disk,
//! 2. parses through `ProjectSession::load` (the canonical core
//!    loader — F-004 calls out the rs_cam_viz loader as divergent),
//! 3. contains the case's `tool_name`, and
//! 4. has a stock `material` whose key matches the case's
//!    `material_family` (Generic Hardwood ↔ "hardwood", Generic
//!    Softwood ↔ "softwood", Mdf ↔ "mdf", etc.).
//!
//! Additionally asserts the case's `fixture` file exists at the
//! repo-relative path declared in the CSV.
//!
//! Some pre-existing case rows have intentional CSV-vs-template
//! mismatches that F-018 does NOT regenerate (auditor restricted CSV
//! edits to AS002/AS004/AS005 only). Those cases are listed in
//! `KNOWN_MATERIAL_MISMATCHES` with the divergence we tolerate;
//! flipping them is left to a follow-up finding.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

use rs_cam_core::material::Material;
use rs_cam_core::session::ProjectSession;

/// Cases whose `material_family` in the CSV is intentionally not
/// matched by their `project_template` material today. These are
/// out-of-scope for F-018 (auditor restricted CSV regen to AS002 /
/// AS004 / AS005) and tracked as residual mismatches for a follow-up.
const KNOWN_MATERIAL_MISMATCHES: &[(&str, &str, &str)] = &[
    // (case_id, csv_material_family, template_material_key)
    ("AS011", "softwood", "hardwood"), // pocket template stays hardwood for AS001/AS006
    ("AS012", "softwood", "hardwood"), // pocket template stays hardwood
    ("AS013", "softwood", "hardwood"), // terrain template stays hardwood for AS014-18
    ("AS017", "mdf", "hardwood"),      // stepped template stays hardwood
];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root resolves")
}

fn csv_path() -> PathBuf {
    repo_root().join("planning/toolpath_acceptance/cases_agent_smoke.csv")
}

/// Minimal CSV row representation — just the columns the test needs.
struct CaseRow {
    case_id: String,
    project_template: String,
    fixture: String,
    tool_name: String,
    material_family: String,
}

/// Split a single CSV line into fields, honouring `"..."` quoting.
fn split_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if in_quotes => {
                if chars.peek() == Some(&'"') {
                    // Escaped quote inside a quoted field.
                    current.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            }
            '"' => {
                in_quotes = true;
            }
            ',' if !in_quotes => {
                fields.push(std::mem::take(&mut current));
            }
            _ => current.push(c),
        }
    }
    fields.push(current);
    fields
}

fn load_cases() -> Vec<CaseRow> {
    let content = std::fs::read_to_string(csv_path()).expect("cases_agent_smoke.csv readable");
    let mut lines = content.lines();
    let header = lines.next().expect("CSV header present");
    let header_fields: Vec<&str> = header.split(',').collect();
    let idx_case_id = header_fields
        .iter()
        .position(|h| *h == "case_id")
        .expect("case_id column");
    let idx_project = header_fields
        .iter()
        .position(|h| *h == "project_template")
        .expect("project_template column");
    let idx_fixture = header_fields
        .iter()
        .position(|h| *h == "fixture")
        .expect("fixture column");
    let idx_tool = header_fields
        .iter()
        .position(|h| *h == "tool_name")
        .expect("tool_name column");
    let idx_material = header_fields
        .iter()
        .position(|h| *h == "material_family")
        .expect("material_family column");

    let mut rows = Vec::new();
    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        let fields = split_csv_line(line);
        if fields.len() <= idx_material {
            panic!("CSV row has fewer fields than header: {line}");
        }
        rows.push(CaseRow {
            case_id: fields[idx_case_id].clone(),
            project_template: fields[idx_project].clone(),
            fixture: fields[idx_fixture].clone(),
            tool_name: fields[idx_tool].clone(),
            material_family: fields[idx_material].clone(),
        });
    }
    rows
}

#[test]
fn all_smoke_cases_reference_existing_templates_and_fixtures() {
    let root = repo_root();
    let rows = load_cases();
    assert!(!rows.is_empty(), "loaded at least one smoke case row");
    let mut missing: Vec<String> = Vec::new();
    for row in &rows {
        let template = root.join(&row.project_template);
        if !template.exists() {
            missing.push(format!(
                "{}: project_template {} not found at {}",
                row.case_id,
                row.project_template,
                template.display()
            ));
        }
        // Skip non-file fixtures like the alignment-pin sentinel.
        if !row.fixture.starts_with("fixtures/") {
            continue;
        }
        let fixture = root.join(&row.fixture);
        if !fixture.exists() {
            missing.push(format!(
                "{}: fixture {} not found at {}",
                row.case_id,
                row.fixture,
                fixture.display()
            ));
        }
    }
    assert!(
        missing.is_empty(),
        "smoke CSV references non-existent paths:\n{}",
        missing.join("\n")
    );
}

#[test]
fn all_smoke_cases_load_via_project_session() {
    let root = repo_root();
    let rows = load_cases();
    let mut errors: Vec<String> = Vec::new();
    for row in &rows {
        let template = root.join(&row.project_template);
        if !template.exists() {
            continue; // surfaced by the path-existence test
        }
        match ProjectSession::load(&template) {
            Ok(_) => {}
            Err(e) => errors.push(format!("{}: {} → {}", row.case_id, row.project_template, e)),
        }
    }
    assert!(
        errors.is_empty(),
        "templates referenced by smoke CSV failed to load via ProjectSession::load:\n{}",
        errors.join("\n")
    );
}

#[test]
fn all_smoke_cases_have_required_tool_in_template() {
    let root = repo_root();
    let rows = load_cases();
    let mut missing: Vec<String> = Vec::new();
    for row in &rows {
        let template = root.join(&row.project_template);
        let Ok(session) = ProjectSession::load(&template) else {
            continue; // surfaced by the loader test
        };
        let has_tool = session.list_tools().iter().any(|t| t.name == row.tool_name);
        if !has_tool {
            let available: Vec<String> = session
                .list_tools()
                .iter()
                .map(|t| t.name.clone())
                .collect();
            missing.push(format!(
                "{}: template {} is missing tool {:?} (available: {:?})",
                row.case_id, row.project_template, row.tool_name, available
            ));
        }
    }
    assert!(
        missing.is_empty(),
        "smoke CSV cases reference tools absent from their templates:\n{}",
        missing.join("\n")
    );
}

#[test]
fn all_smoke_cases_have_matching_material_family() {
    let root = repo_root();
    let rows = load_cases();
    let mut mismatches: Vec<String> = Vec::new();
    for row in &rows {
        let template = root.join(&row.project_template);
        let Ok(session) = ProjectSession::load(&template) else {
            continue; // surfaced by the loader test
        };
        let template_key = Material::to_key(&session.stock_config().material);
        if template_key == row.material_family {
            continue;
        }
        // Tolerate documented residual mismatches.
        let is_known = KNOWN_MATERIAL_MISMATCHES
            .iter()
            .any(|(case, csv_mat, tpl_mat)| {
                *case == row.case_id
                    && *csv_mat == row.material_family.as_str()
                    && *tpl_mat == template_key.as_str()
            });
        if is_known {
            continue;
        }
        mismatches.push(format!(
            "{}: CSV material_family={:?} but template {} stock material key is {:?}",
            row.case_id, row.material_family, row.project_template, template_key
        ));
    }
    assert!(
        mismatches.is_empty(),
        "smoke CSV cases have material_family mismatches with their templates:\n{}",
        mismatches.join("\n")
    );
}
