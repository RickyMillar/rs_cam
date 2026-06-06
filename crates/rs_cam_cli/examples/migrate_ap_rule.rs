//! One-shot: populate `ap_min_factor` / `ap_max_factor` on every vendor LUT
//! observation row that carries a recognised `ap_rule` prose string.
//!
//! Phase 1 of `planning/cutter_axial_constraints_2026-06-06.md`. The mapping
//! table below is §3.2 of that doc verbatim — 46 unique `ap_rule` strings
//! across 252 rows in `crates/rs_cam_core/data/vendor_lut/observations/`.
//!
//! Behaviour:
//! - Reads each `*.json` in the observations directory.
//! - For every row whose `ap_rule` matches a mapping entry with non-None
//!   factor fields, writes the structured fields back. Idempotent: rows
//!   that already carry `ap_min_factor` / `ap_max_factor` are left alone.
//! - Label-only / TBD entries in the mapping table (both factors `None`)
//!   are skipped silently — the `ap_rule` string stays as source citation.
//!
//! Run once after the schema change lands, commit the result.
//!
//! Usage:
//!   cargo run -p rs_cam_cli --example migrate_ap_rule -- \
//!     crates/rs_cam_core/data/vendor_lut/observations

#![allow(clippy::print_stdout, clippy::print_stderr)] // CLI example surface

use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// §3.2 mapping: (`ap_rule` exact string, `ap_min_factor`, `ap_max_factor`).
/// Both `None` → label-only / TBD / absolute-mm-only row → skip.
const AP_RULE_MAPPING: &[(&str, Option<f64>, Option<f64>)] = &[
    // High-volume "1xD per chart with progressive derating" family — cap at 1×D.
    (
        "cut depth per pass = cutting edge diameter (1xD); 2xD reduce chip load 25%; 3xD reduce chip load 50%",
        None,
        Some(1.0),
    ),
    (
        "1xD use recommended feed rate; 2xD reduce 25%; 3xD reduce 50%",
        None,
        Some(1.0),
    ),
    (
        "1xD use recommended chip load; 2xD reduce chip load 25%; 3xD reduce 50%",
        None,
        Some(1.0),
    ),
    (
        "depth of cut equal to bit diameter; if 2xD reduce chipload by at least 25%; if 3xD reduce by at least 50%",
        None,
        Some(1.0),
    ),
    (
        "1xD use recommended feed rate; 2xD reduce feed rate 25%; 3xD reduce 50%",
        None,
        Some(1.0),
    ),
    (
        "1xD per chart; 3D-finish DOC unspecified by source",
        None,
        Some(1.0),
    ),
    ("Profiling Axial = 1xD", None, Some(1.0)),
    ("Traditional ADOC = 0.500 in = 100% x D", None, Some(1.0)),
    // HEM-class rows (2×D).
    (
        "Profiling Axial = 2xD (HEM); side milling up to 2xD",
        None,
        Some(2.0),
    ),
    ("HEM ADOC = 1.000 in = 200% x D", None, Some(2.0)),
    // Slotting-specific.
    (
        "Axial = .5xD (slotting); Pocket/Slot up to 1xD",
        None,
        Some(1.0),
    ),
    ("Axial = .5xD (slotting)", None, Some(0.5)),
    // Plunge-only (20% D).
    (
        "20% of diameter for basic engagement parameters; drop feed ~50% when plunging into solid",
        None,
        Some(0.2),
    ),
    // Explicit `Nxd to Mxd` ranges.
    ("0.3xD to 0.7xD", Some(0.3), Some(0.7)),
    ("0.25xD to 0.65xD", Some(0.25), Some(0.65)),
    ("0.5xD to 1xD", Some(0.5), Some(1.0)),
    ("0.5xD to 1.5xD", Some(0.5), Some(1.5)),
    ("0.4xD to 1.2xD", Some(0.4), Some(1.2)),
    ("0.4xD to 1.25xD", Some(0.4), Some(1.25)),
    ("0.35xD to 0.8xD", Some(0.35), Some(0.8)),
    ("0.35xD to 0.9xD", Some(0.35), Some(0.9)),
    ("0.35xD to 1xD", Some(0.35), Some(1.0)),
    ("0.35xD to 1.2xD", Some(0.35), Some(1.2)),
    ("0.3xD to 0.75xD", Some(0.3), Some(0.75)),
    ("0.3xD to 0.85xD", Some(0.3), Some(0.85)),
    ("0.25xD to 0.6xD", Some(0.25), Some(0.6)),
    ("0.25xD to 0.7xD", Some(0.25), Some(0.7)),
    ("0.25xD to 0.9xD", Some(0.25), Some(0.9)),
    ("0.2xD to 0.45xD", Some(0.2), Some(0.45)),
    ("0.2xD to 0.5xD", Some(0.2), Some(0.5)),
    ("0.2xD to 0.65xD", Some(0.2), Some(0.65)),
    ("0.15xD to 0.45xD", Some(0.15), Some(0.45)),
    ("0.15xD to 0.55xD", Some(0.15), Some(0.55)),
    ("0.12xD to 0.42xD", Some(0.12), Some(0.42)),
    ("0.08xD to 0.25xD", Some(0.08), Some(0.25)),
    // Label-only / absolute-mm-only / TBD — both None means skip (no factor write).
    // The migration still recognises them so the validation test can confirm
    // every distinct ap_rule string is accounted for.
    ("tip depth dependent", None, None),
    ("3d profiling finish", None, None),
    ("3d finishing", None, None),
    ("max 1mm", None, None),
    ("max 0.8mm", None, None),
    ("max 1.2mm", None, None),
    ("light finishing", None, None),
    ("3d profiling semi", None, None),
    ("semi-finish", None, None),
    ("Finishing Axial = Max LOC", None, None),
    (
        "side-entry or ramp entry required (no straight plunge); upcut O-flute for chip evacuation. Phase 5 promotion (2026-06-01) — diameter_mm omitted; article publishes one chipload window across the upcut O-flute line.",
        None,
        None,
    ),
];

fn build_mapping() -> HashMap<&'static str, (Option<f64>, Option<f64>)> {
    AP_RULE_MAPPING
        .iter()
        .map(|(rule, lo, hi)| (*rule, (*lo, *hi)))
        .collect()
}

#[derive(Default, Debug)]
struct FileStats {
    rows_total: usize,
    rows_with_rule: usize,
    rows_factored: usize,
    rows_already_populated: usize,
    rows_unrecognised: Vec<String>,
}

fn migrate_file(
    path: &Path,
    mapping: &HashMap<&'static str, (Option<f64>, Option<f64>)>,
) -> Result<FileStats, String> {
    let contents =
        std::fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let mut root: Value =
        serde_json::from_str(&contents).map_err(|e| format!("parse {}: {e}", path.display()))?;
    let mut stats = FileStats::default();

    let obs_array = root
        .get_mut("observations")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| format!("no observations array in {}", path.display()))?;

    for obs in obs_array.iter_mut() {
        stats.rows_total += 1;
        let Some(obj) = obs.as_object_mut() else {
            continue;
        };

        let Some(ap_rule) = obj
            .get("ap_rule")
            .and_then(Value::as_str)
            .map(str::to_owned)
        else {
            continue;
        };
        stats.rows_with_rule += 1;

        let already_populated =
            obj.contains_key("ap_min_factor") || obj.contains_key("ap_max_factor");
        if already_populated {
            stats.rows_already_populated += 1;
            continue;
        }

        let (lo, hi) = match mapping.get(ap_rule.as_str()) {
            Some(pair) => *pair,
            None => {
                stats.rows_unrecognised.push(ap_rule);
                continue;
            }
        };

        // Skip label-only entries (both None).
        if lo.is_none() && hi.is_none() {
            continue;
        }

        // Insert factor fields immediately after `ap_max_mm` if present,
        // otherwise after `ap_min_mm`, otherwise append at the object's end.
        // `serde_json` with `preserve_order` keeps insertion order, so we
        // rebuild the object to control where the new keys land.
        let entries: Vec<(String, Value)> =
            obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        let has_max = entries.iter().any(|(k, _)| k == "ap_max_mm");
        let anchor: &str = if has_max { "ap_max_mm" } else { "ap_min_mm" };
        obj.clear();
        let mut inserted = false;
        for (k, v) in entries {
            obj.insert(k.clone(), v);
            if !inserted && k == anchor {
                if let Some(min_f) = lo {
                    obj.insert("ap_min_factor".to_owned(), serde_json::json!(min_f));
                }
                if let Some(max_f) = hi {
                    obj.insert("ap_max_factor".to_owned(), serde_json::json!(max_f));
                }
                inserted = true;
            }
        }
        if !inserted {
            // No ap_*_mm field present — fall back to appending before any
            // closing trailer; `serde_json` with `preserve_order` keeps
            // these at the end, which is fine for parsing.
            if let Some(min_f) = lo {
                obj.insert("ap_min_factor".to_owned(), serde_json::json!(min_f));
            }
            if let Some(max_f) = hi {
                obj.insert("ap_max_factor".to_owned(), serde_json::json!(max_f));
            }
        }
        stats.rows_factored += 1;
    }

    let serialised = serde_json::to_string_pretty(&root)
        .map_err(|e| format!("serialise {}: {e}", path.display()))?;
    // Match existing files' trailing newline.
    let mut out = serialised;
    if !out.ends_with('\n') {
        out.push('\n');
    }
    std::fs::write(path, out).map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok(stats)
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir: PathBuf = match args.first() {
        Some(p) => PathBuf::from(p),
        None => PathBuf::from("crates/rs_cam_core/data/vendor_lut/observations"),
    };
    if !dir.is_dir() {
        return Err(format!("not a directory: {}", dir.display()));
    }

    let mapping = build_mapping();
    let mut grand = FileStats::default();
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map_err(|e| format!("read_dir {}: {e}", dir.display()))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("json"))
        .collect();
    files.sort();

    for path in &files {
        let stats = migrate_file(path, &mapping)?;
        println!(
            "{}: total={}, with_rule={}, factored={}, already={}, unrecognised={}",
            path.file_name().and_then(|s| s.to_str()).unwrap_or("?"),
            stats.rows_total,
            stats.rows_with_rule,
            stats.rows_factored,
            stats.rows_already_populated,
            stats.rows_unrecognised.len(),
        );
        for rule in &stats.rows_unrecognised {
            println!("  UNRECOGNISED: {rule:?}");
        }
        grand.rows_total += stats.rows_total;
        grand.rows_with_rule += stats.rows_with_rule;
        grand.rows_factored += stats.rows_factored;
        grand.rows_already_populated += stats.rows_already_populated;
        grand.rows_unrecognised.extend(stats.rows_unrecognised);
    }

    println!(
        "\nTOTAL: rows={}, with_rule={}, factored={}, already_populated={}, unrecognised={}",
        grand.rows_total,
        grand.rows_with_rule,
        grand.rows_factored,
        grand.rows_already_populated,
        grand.rows_unrecognised.len(),
    );
    if !grand.rows_unrecognised.is_empty() {
        return Err(format!(
            "{} rows had ap_rule strings not in the §3.2 mapping table — \
             add them to AP_RULE_MAPPING before re-running",
            grand.rows_unrecognised.len()
        ));
    }
    Ok(())
}
