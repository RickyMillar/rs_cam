//! Feeds contract P29-last
//! (`planning/feeds_contract_2026-11-03/phases/P29-11_DESIGN.md` section 5,
//! lead rulings L3 and L7): no limit reads the op type.
//!
//! A limit sees the cut, the tool, the stock and the machine. A fact of a
//! cut is declared once, in `OperationConfig::cuts` (`compute/catalog/cuts.rs`)
//! or in the registry `Capabilities`, and every limit reads the fact. A
//! limit that reads the op type, the family or the pass role makes a second
//! rule, and a new mechanism (a force, a cap) then reaches some moves and not
//! others. The types hold most of this (`limits::CutFacts` has private
//! fields and one constructor; `feeds::RowKey` is opaque; `PlanFacts::NONE`
//! is private). This scan holds the rest.
//!
//! - Claim 1: each read of a token of `op_type_scan::TOKENS` in a guarded
//!   file of `src/` (section 5.1) has an entry of [`ALLOW`], and each
//!   entry has a hit. A dispatch entry (classes V,
//!   W, D, G, Q, T) names a file and a function, with no count: a new op
//!   adds one arm there, and the compiler asks for it. An X entry names the
//!   rows of `limits::EXEMPTIONS` that hold the reads, and the exact count;
//!   each row names the file. The sum of the X counts equals [`X_TOTAL`],
//!   which only goes down.
//! - Claim 2: no entry names a file with zero tolerance (section 5.2: the
//!   gate files of `tool_load/`, `limits/`, the limit driver and the load
//!   model of `feeds/`, the calculator, the feed modulator, the door of the
//!   limit rows), and such a file has no hit. P29-11 S10 closed the last
//!   reads there: a gate reads the facts of its group, the envelope the
//!   facts of its ceiling cuts, through `RigidityProfile::force_budget_of`.
//! - Claim 3 (non-vacuity): the declaration file holds hits; a planted read
//!   is found in a fixture string; a comment, a string, a test module, an
//!   import and a definition are not reads; the scan reads the guarded tree.
//! - Claim 4: only `compute/catalog/cuts.rs` calls `CutFacts::declared`.
//!
//! The viz display files and MCP are scanned by the twin
//! `crates/rs_cam_viz/tests/no_limit_reads_the_op_type_p29_last.rs`, the CLI
//! by `crates/rs_cam_cli/tests/no_limit_reads_the_op_type_p29_last.rs`.
//!
//! `cargo test -p rs_cam_core -q --test no_limit_reads_the_op_type_p29_last`

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: an expect is how a test reports a failure; the shared source scans index their own buffers"
)]

#[path = "common/source_scan.rs"]
mod source_scan;

#[path = "common/op_type_scan.rs"]
mod op_type_scan;

use std::path::{Path, PathBuf};

use op_type_scan::{Allow, Class};

/// The guarded files of core (section 5.1), relative to `src/`.
fn guarded(rel: &str) -> bool {
    rel.starts_with("feeds/")
        || rel.starts_with("tool_load/")
        || rel.starts_with("limits/")
        || rel.starts_with("diagnostics/adapters/")
        || matches!(
            rel,
            "dressup/feed_modulation.rs"
                | "session/limit_rows.rs"
                | "session/load_report.rs"
                | "session/compute/diagnostics.rs"
                | "trace/narrate.rs"
                | "machine/mod.rs"
                | "compute/simulate.rs"
        )
}

/// The files with zero tolerance (section 5.2): a limit is computed here.
fn zero_tolerance(rel: &str) -> bool {
    rel.starts_with("tool_load/")
        || rel.starts_with("limits/")
        || rel.starts_with("feeds/calculator/")
        || matches!(
            rel,
            "feeds/planned.rs"
                | "feeds/cut_load.rs"
                | "feeds/engagement.rs"
                | "dressup/feed_modulation.rs"
                | "session/limit_rows.rs"
        )
}

/// The allow-list (section 5.2) as it stands after commits L-1 and L-2. A
/// dispatch entry names a file and a function; an X entry also names its row
/// and its exact count. The list only shrinks: an entry with no hit fails.
#[rustfmt::skip]
const ALLOW: &[Allow] = &[
    // V: the vendor row key (row `M6`, P25 / P27).
    Allow { file: "feeds/extrapolation/drill.rs", function: "drill_rule", class: Class::V },
    Allow { file: "feeds/extrapolation/family.rs", function: "family_basis", class: Class::V },
    Allow { file: "feeds/extrapolation/family.rs", function: "transfer_rule", class: Class::V },
    Allow { file: "feeds/extrapolation/size.rs", function: "anchor_series", class: Class::V },
    Allow { file: "feeds/extrapolation/size.rs", function: "cross_chart_bracket", class: Class::V },
    Allow { file: "feeds/mod.rs", function: "validate_tool_for_operation", class: Class::V },
    Allow { file: "feeds/ramp.rs", function: "ramp_basis", class: Class::V },
    Allow { file: "feeds/row_key.rs", function: "RowKey", class: Class::V },
    Allow { file: "feeds/row_key.rs", function: "feeds_style", class: Class::V },
    Allow { file: "feeds/row_key.rs", function: "lut_route", class: Class::V },
    Allow { file: "feeds/row_key.rs", function: "of_operation", class: Class::V },
    Allow { file: "feeds/row_key.rs", function: "op_type", class: Class::V },
    Allow { file: "feeds/support.rs", function: "formula_backing", class: Class::V },
    Allow { file: "feeds/support.rs", function: "formula_source_for_input", class: Class::V },
    Allow { file: "feeds/support.rs", function: "support_for_lookup", class: Class::V },
    Allow { file: "feeds/vendor_lookup.rs", function: "build_result", class: Class::V },
    Allow { file: "feeds/vendor_lookup.rs", function: "effective_operation", class: Class::V },
    Allow { file: "feeds/vendor_lookup.rs", function: "score_observation", class: Class::V },
    Allow { file: "feeds/vendor_normalize.rs", function: "declared_lut_pass_role", class: Class::V },
    Allow { file: "feeds/vendor_normalize.rs", function: "from", class: Class::V },
    Allow { file: "feeds/vendor_normalize.rs", function: "impl From<OperationFamily> for LutOperationFamily", class: Class::V },
    Allow { file: "feeds/vendor_normalize.rs", function: "impl From<PassRole> for LutPassRole", class: Class::V },
    Allow { file: "feeds/vendor_normalize.rs", function: "lut_query_for", class: Class::V },
    Allow { file: "feeds/vendor_normalize.rs", function: "op_family_to_lut", class: Class::V },
    Allow { file: "feeds/vendor_normalize.rs", function: "ramp_drill_query", class: Class::V },
    Allow { file: "feeds/vendor_normalize.rs", function: "routed_row_of", class: Class::V },
    Allow { file: "feeds/vendor_normalize.rs", function: "to_lookup_query", class: Class::V },
    Allow { file: "feeds/vendor_normalize.rs", function: "to_lookup_query_unrouted", class: Class::V },
    // W: the write funnel and the op draft.
    Allow { file: "feeds/suggest.rs", function: "SuggestParamsInput", class: Class::W },
    Allow { file: "feeds/suggest.rs", function: "default_operation", class: Class::W },
    Allow { file: "feeds/suggest.rs", function: "default_operation_for_tool", class: Class::W },
    Allow { file: "feeds/suggest.rs", function: "of_new_op", class: Class::W },
    Allow { file: "feeds/suggest.rs", function: "suggest_for_operation", class: Class::W },
    Allow { file: "feeds/suggest/apply.rs", function: "ApplyContext", class: Class::W },
    Allow { file: "feeds/suggest/apply.rs", function: "apply", class: Class::W },
    Allow { file: "feeds/suggest/apply.rs", function: "apply_drill_defaults", class: Class::W },
    Allow { file: "feeds/suggest/apply.rs", function: "apply_feeds_subset", class: Class::W },
    Allow { file: "feeds/suggest/apply.rs", function: "apply_speeds_to_op", class: Class::W },
    Allow { file: "feeds/suggest/apply.rs", function: "apply_stock_defaults", class: Class::W },
    Allow { file: "feeds/suggest/apply.rs", function: "apply_to_operation", class: Class::W },
    Allow { file: "feeds/suggest/apply.rs", function: "peck_for_hole", class: Class::W },
    Allow { file: "feeds/suggest/apply.rs", function: "write_pick", class: Class::W },
    // D: the declaration (the registry `Capabilities`, the entry rule, the move model, the precondition dispatch, the narration wording).
    Allow { file: "diagnostics/adapters/from_preconditions.rs", function: "diagnostics_from_preconditions", class: Class::D },
    Allow { file: "diagnostics/adapters/from_preconditions.rs", function: "not_set", class: Class::D },
    Allow { file: "diagnostics/adapters/from_static_checks.rs", function: "depth_beyond_stock", class: Class::D },
    Allow { file: "diagnostics/adapters/from_static_checks.rs", function: "depth_beyond_stock_applies", class: Class::D },
    Allow { file: "diagnostics/adapters/from_static_checks.rs", function: "height_order_findings", class: Class::D },
    Allow { file: "diagnostics/adapters/from_static_checks.rs", function: "tool_op_compat_checks", class: Class::D },
    Allow { file: "feeds/entry_rule.rs", function: "of_operation", class: Class::D },
    Allow { file: "feeds/entry_rule.rs", function: "of_planned_op", class: Class::D },
    Allow { file: "feeds/entry_rule.rs", function: "planned_feed_mm_min", class: Class::D },
    Allow { file: "feeds/envelope.rs", function: "runtime_limit", class: Class::D },
    Allow { file: "feeds/input.rs", function: "depth_after_simulation", class: Class::D },
    Allow { file: "feeds/predict.rs", function: "predict_move_count", class: Class::D },
    Allow { file: "feeds/ramp.rs", function: "resolved_entry", class: Class::D },
    Allow { file: "feeds/suggest/axial_envelope.rs", function: "axial_envelope_for_operation", class: Class::D },
    // P29-11 S10: the machine numbers of the depth limit of one cut's facts (the one door).
    Allow { file: "machine/mod.rs", function: "force_budget_of", class: Class::D },
    Allow { file: "session/compute/diagnostics.rs", function: "air_cut_offenders_for_toolpaths", class: Class::D },
    Allow { file: "session/compute/diagnostics.rs", function: "triage_from_parts", class: Class::D },
    Allow { file: "trace/narrate.rs", function: "append_air_cut_anomaly", class: Class::D },
    Allow { file: "trace/narrate.rs", function: "append_clipped_band", class: Class::D },
    Allow { file: "trace/narrate.rs", function: "append_dropped_band", class: Class::D },
    Allow { file: "trace/narrate.rs", function: "append_peak_doc_anomaly", class: Class::D },
    Allow { file: "trace/narrate.rs", function: "append_ramp_reach_clamp", class: Class::D },
    Allow { file: "trace/narrate.rs", function: "append_relink_totals", class: Class::D },
    Allow { file: "trace/narrate.rs", function: "is_drill_cycle_for_narration", class: Class::D },
    // G: generation planning (the strategy pick, the dowel holes).
    Allow { file: "feeds/geometry_class.rs", function: "classify", class: Class::G },
    Allow { file: "feeds/suggest/adaptive_entry.rs", function: "pick_adaptive3d_clearing_strategy", class: Class::G },
    Allow { file: "session/compute/diagnostics.rs", function: "dowel_hole_ops", class: Class::G },
    // Q: a quality hint by role, not a limit.
    Allow { file: "diagnostics/adapters/from_static_checks.rs", function: "stepover_checks", class: Class::Q },
    // T: a label or a report field.
    Allow { file: "diagnostics/adapters/from_static_checks.rs", function: "diagnostics_from_static_checks", class: Class::T },
    Allow { file: "feeds/envelope.rs", function: "fmt", class: Class::T },
    Allow { file: "feeds/mod.rs", function: "FeedsError", class: Class::T },
    Allow { file: "feeds/mod.rs", function: "default_profile_row", class: Class::T },
    Allow { file: "feeds/mod.rs", function: "formula_constants", class: Class::T },
    Allow { file: "feeds/mod.rs", function: "fmt", class: Class::T },
    Allow { file: "feeds/mod.rs", function: "profile_row", class: Class::T },
    Allow { file: "feeds/mod.rs", function: "profile_row_source", class: Class::T },
    Allow { file: "feeds/predict.rs", function: "DeflectionUnmodeled", class: Class::T },
    Allow { file: "feeds/predict.rs", function: "as_unmodeled_reason", class: Class::T },
    Allow { file: "feeds/profile.rs", function: "CutterOpProfile", class: Class::T },
    Allow { file: "feeds/profile.rs", function: "for_combo", class: Class::T },
    Allow { file: "feeds/suggest/adaptive_entry.rs", function: "adaptive_plunge_entry_label", class: Class::T },
    Allow { file: "session/compute/diagnostics.rs", function: "diagnostics_with_evidence", class: Class::T },
    Allow { file: "session/compute/diagnostics.rs", function: "narrate_toolpath", class: Class::T },
    Allow { file: "trace/narrate.rs", function: "ToolpathNarrationContext", class: Class::T },
    // X: a scope read that a row of `EXEMPTIONS` holds, with its exact count.
    Allow { file: "feeds/envelope.rs", function: "for_cut", class: Class::X { rows: &["P28-05"], count: 2 } },
    Allow { file: "feeds/mod.rs", function: "default_engagement", class: Class::X { rows: &["P28-31", "P28-08"], count: 16 } },
    Allow { file: "feeds/mod.rs", function: "default_profile_of", class: Class::X { rows: &["P28-31"], count: 1 } },
    Allow { file: "feeds/mod.rs", function: "operation_default_profile", class: Class::X { rows: &["P28-31"], count: 45 } },
    Allow { file: "feeds/predict.rs", function: "predict_peak_deflection_um", class: Class::X { rows: &["P28-17", "P28-08"], count: 9 } },
    Allow { file: "feeds/ramp.rs", function: "entry_notes", class: Class::X { rows: &["P28-28"], count: 3 } },
    Allow { file: "feeds/suggest/adaptive_entry.rs", function: "check_plunge_entry_stability", class: Class::X { rows: &["P28-28"], count: 7 } },
    Allow { file: "feeds/suggest/invariants.rs", function: "enforce_invariants_at_fixed_geometry", class: Class::X { rows: &["P28-28"], count: 3 } },
];

/// The sum of the X counts. It only goes down: a closed X read lowers it in
/// the same commit.
const X_TOTAL: usize = 86;

fn src_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

#[test]
fn no_limit_reads_the_op_type_p29_last() {
    let root = src_root();
    let table = op_type_scan::counts(&root, guarded);
    if let Ok(out) = std::env::var("P29_GUARD_TABLE") {
        std::fs::write(out, op_type_scan::table_text(&table)).expect("write the table");
    }
    assert!(
        table.len() > 50,
        "non-vacuity: the scan finds the guarded reads ({} functions)",
        table.len()
    );
    let mut failures = op_type_scan::failures(&table, ALLOW, zero_tolerance, X_TOTAL);
    failures.extend(x_rows_name_their_files());
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Each X entry names rows of `limits::EXEMPTIONS`, and each row names the
/// file of the entry (a row and its reads close together).
fn x_rows_name_their_files() -> Vec<String> {
    let mut out = Vec::new();
    for entry in ALLOW {
        let Class::X { rows, .. } = entry.class else {
            continue;
        };
        if rows.is_empty() {
            out.push(format!(
                "{} `{}`: the X entry names no row",
                entry.file, entry.function
            ));
        }
        let file = format!("src/{}", entry.file);
        for &row in rows {
            match rs_cam_core::limits::EXEMPTIONS.iter().find(|e| e.id == row) {
                None => out.push(format!(
                    "{} `{}`: the X entry names the row {row}, which is not in EXEMPTIONS",
                    entry.file, entry.function
                )),
                Some(e) if !e.where_.contains(&file.as_str()) => out.push(format!(
                    "{} `{}`: the row {row} does not name {file}",
                    entry.file, entry.function
                )),
                Some(_) => {}
            }
        }
    }
    out
}

/// Claim 3: the declaration file holds the reads that the limits do not.
#[test]
fn the_declaration_holds_the_reads_p29_last() {
    let table = op_type_scan::counts(&src_root(), |rel| rel == "compute/catalog/cuts.rs");
    let hits: usize = table.values().map(Vec::len).sum();
    assert!(
        hits > 10,
        "non-vacuity: `compute/catalog/cuts.rs` reads the op type ({hits} hits)"
    );
    for file in [
        "tool_load/chipload.rs",
        "feeds/calculator/feed.rs",
        "limits/cut.rs",
    ] {
        assert!(
            zero_tolerance(file) && guarded(file) && src_root().join(file).is_file(),
            "the zero-tolerance file {file} is scanned"
        );
    }
}

/// Claim 3: a planted read is found and named by its function; a comment, a
/// string, a test module, an import and a definition are not reads.
#[test]
fn the_scan_finds_a_planted_read_p29_last() {
    let planted =
        "fn judge(op: &OperationConfig) -> bool {\n    op.op_type() == OperationType::Drill\n}\n";
    let hits = op_type_scan::hits(planted);
    assert_eq!(hits.len(), 2, "two tokens: {hits:?}");
    assert!(hits.iter().all(|h| h.function == "judge"), "{hits:?}");
    let variant = "fn f(op: &OperationConfig) {\n    if let OperationConfig::Drill(_) = op {}\n}\n";
    assert_eq!(op_type_scan::hits(variant).len(), 1, "a variant path");
    let quiet = concat!(
        "use crate::feeds::PassRole;\n",
        "// op.op_type() in a comment\n",
        "fn depth_rule(&self) -> DepthLimit {\n    self.depth_rule\n}\n",
        "struct S {\n    depth_rule: DepthLimit,\n}\n",
        "fn label() -> &'static str {\n    \"op_type() in a string\"\n}\n",
        "#[cfg(test)]\nmod tests {\n    fn t() { let _ = op.op_type(); }\n}\n",
    );
    let hits = op_type_scan::hits(quiet);
    assert!(hits.is_empty(), "not reads: {hits:?}");
}

/// Claim 4: the facts of a cut have one constructor, and only the
/// declaration calls it.
#[test]
fn only_the_declaration_builds_the_facts_of_a_cut_p29_last() {
    let mut callers = Vec::new();
    for path in source_scan::rust_files(&src_root()) {
        let src = std::fs::read_to_string(&path).expect("a source file reads");
        if op_type_scan::code_of(&src).contains("CutFacts::declared(") {
            callers.push(
                path.strip_prefix(src_root())
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
    }
    assert_eq!(
        callers,
        ["compute/catalog/cuts.rs"],
        "only the declaration builds `CutFacts`; declare the fact in `OperationConfig::cuts`"
    );
}
