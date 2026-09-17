//! UI-04's sentry: a parameter tooltip is keyed on the registry parameter
//! name, never on the visible label.
//!
//! # What the finding was
//!
//! `ui/properties/linking_dressup.rs::tooltip_for` dispatched 60 arms on
//! `label.trim().trim_end_matches(':')`. `"Stepover:"` from `boundary_2d.rs`
//! reached its help text only because the two spellings matched by hand, and
//! `record_stock_to_leave` decided whether to emit a UI-automation hook by
//! the same trick. Re-word a label and the tooltip, or the hook, disappears
//! with nothing failing.
//!
//! # What this test measures
//!
//! Arm 1 reads every `dv` / `dv_pill` call site in `ui/properties/` out of
//! the source, takes the `(OperationType::X, "param")` pair each one passes,
//! and resolves it against the SAME core registry the GUI reads. A name the
//! registry does not carry fails, and so does a name whose row states no
//! help — so a new parameter row cannot reach the panel with a dead tooltip.
//!
//! Arm 2 reads the `dv_dressup` call sites and resolves them against
//! `DressupConfig::FIELD_DEFS`, the table CMP-17 made the one place the
//! dressup vocabulary is written down.
//!
//! Arm 3 is the ban: no label-keyed lookup survives in `ui/properties/`.
//!
//! Arm 4 is the non-vacuity guard: the scan must find the call sites it
//! claims to check.
//!
//! # NOT MEASURED
//!
//! Whether the help text is CORRECT, and the tooltips of rows that are not
//! `dv` rows (a checkbox or a combo carries its own `on_hover_text`).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::config::DressupConfig;

/// The fewest `dv` / `dv_pill` keys the scan must find before its result is
/// believed. UI-04 converted 111 operation rows.
const MIN_OP_KEYS: usize = 100;

/// The fewest `dv_dressup` keys the scan must find. UI-04 converted 10.
const MIN_DRESSUP_KEYS: usize = 10;

fn properties_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("ui")
        .join("properties")
}

fn rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            rs_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

fn properties_source() -> Vec<(String, String)> {
    let root = properties_root();
    let mut files = Vec::new();
    rs_files(&root, &mut files);
    files.sort();
    files
        .into_iter()
        .map(|p| {
            let rel = p
                .strip_prefix(&root)
                .unwrap_or(&p)
                .to_string_lossy()
                .replace('\\', "/");
            let text = std::fs::read_to_string(&p).unwrap();
            (rel, text)
        })
        .collect()
}

/// `OperationType::Face` by variant name, so the scan compares against the
/// real enum rather than a second copy of the list.
fn op_by_variant(name: &str) -> Option<OperationType> {
    OperationType::ALL
        .iter()
        .copied()
        .find(|op| format!("{op:?}") == name)
}

/// Every `(file, op variant, param name)` a `dv` or `dv_pill` call passes.
///
/// The call is written over several lines by rustfmt, so the scan collapses
/// whitespace first and then reads the two arguments after `ui`.
fn op_keys() -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    for (rel, text) in properties_source() {
        let flat: String = text.split_whitespace().collect();
        let needle = "p(OperationType::";
        let mut from = 0usize;
        while let Some(i) = flat[from..].find(needle) {
            let start = from + i + needle.len();
            let rest = &flat[start..];
            let variant: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if let Some(q) = rest.find('"') {
                let after = &rest[q + 1..];
                if let Some(e) = after.find('"') {
                    out.push((rel.clone(), variant, after[..e].to_owned()));
                }
            }
            from = start;
        }
    }
    out
}

/// Every dressup field name a `dv_dressup` call passes.
fn dressup_keys() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (rel, text) in properties_source() {
        let flat: String = text.split_whitespace().collect();
        let needle = "dv_dressup(ui,\"";
        let mut from = 0usize;
        while let Some(i) = flat[from..].find(needle) {
            let start = from + i + needle.len();
            let rest = &flat[start..];
            if let Some(e) = rest.find('"') {
                out.push((rel.clone(), rest[..e].to_owned()));
            }
            from = start;
        }
    }
    out
}

#[test]
fn the_help_key_is_the_registry_param_name_ui04() {
    let keys = op_keys();
    let mut bad = Vec::new();

    for (file, variant, param) in &keys {
        let Some(op) = op_by_variant(variant) else {
            bad.push(format!("{file}: OperationType::{variant} is not a variant"));
            continue;
        };
        let def = op
            .registry_entry()
            .param_defs
            .iter()
            .find(|d| d.name == param);
        match def {
            None => bad.push(format!(
                "{file}: {variant}.{param} names no ParamDef, so the row \
                 renders with no tooltip"
            )),
            Some(def) if def.help.is_none_or(|h| h.trim().is_empty()) => bad.push(format!(
                "{file}: {variant}.{param} resolves to a ParamDef that states \
                 no help"
            )),
            Some(_) => {}
        }
    }

    assert!(
        bad.is_empty(),
        "these parameter rows key a tooltip on a name the core registry \
         cannot answer. Add the row to `compute/catalog/registry.rs`, or \
         give the existing row a `.with_help(\"…\")` line. Failures: {}",
        bad.join("; ")
    );
}

#[test]
fn the_dressup_help_key_is_the_published_field_ui04() {
    let keys = dressup_keys();
    let mut bad = Vec::new();

    for (file, field) in &keys {
        match DressupConfig::FIELD_DEFS.iter().find(|d| d.name == field) {
            None => bad.push(format!(
                "{file}: `{field}` names no DressupConfig::FIELD_DEFS row"
            )),
            Some(def) if def.description.trim().is_empty() => {
                bad.push(format!("{file}: `{field}` states no description"));
            }
            Some(_) => {}
        }
    }

    assert!(
        bad.is_empty(),
        "these dressup rows key a tooltip on a name the published field \
         table cannot answer: {}",
        bad.join("; ")
    );
}

#[test]
fn no_parameter_row_keys_its_help_on_a_label_ui04() {
    let mut offenders = Vec::new();
    for (rel, text) in properties_source() {
        for (n, line) in text.lines().enumerate() {
            let code = match line.find("//") {
                Some(i) => &line[..i],
                None => line,
            };
            if code.contains("trim_end_matches(':')") {
                offenders.push(format!("{rel}:{}", n + 1));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "a help or automation lookup keyed on the visible label survives at \
         {}. Key on the registry parameter name; a label is what the \
         operator reads, not an identifier.",
        offenders.join(", ")
    );
}

#[test]
fn the_key_scan_is_not_vacuous_ui04() {
    let op = op_keys();
    let dressup = dressup_keys();
    assert!(
        op.len() >= MIN_OP_KEYS,
        "the scan found only {} operation parameter keys, fewer than the \
         {MIN_OP_KEYS} UI-04 converted. Either the call shape changed or the \
         scan broke, and an empty scan passes arm 1 for free.",
        op.len()
    );
    assert!(
        dressup.len() >= MIN_DRESSUP_KEYS,
        "the scan found only {} dressup keys, fewer than the \
         {MIN_DRESSUP_KEYS} UI-04 converted.",
        dressup.len()
    );
}
