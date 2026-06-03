//! Per-cell evaluator and matrix-level driver.
//!
//! Loads cells + sources from TOML, runs each cell through the shim,
//! computes sub-verdicts via the primitives, rolls up to a cell verdict,
//! and emits both text + JSON to stdout. Fails the test only when an
//! overall verdict reaches `major` or `critical` (per plan §"CI policy").

use super::cell::{AntiPattern, Band, CellsFile, ExpectedBands, Invariant, LiteratureCell};
use super::freshness::{
    audit_citations, build_freshness_report, decay_fail_enabled, render_citation_audit_text,
    render_freshness_json, render_freshness_text,
};
use super::invariant::{
    BandMode, SubVerdict, SubVerdictDetail, anti_pattern_check, band_check, convex_hull_check,
    expr_check,
};
use super::report::{render_json, render_text};
use super::shim::{self, ShimSnapshot};
use super::verdict::{CellVerdict, Severity, SubVerdictRow, rollup};
use std::collections::BTreeMap;
use std::path::Path;

const EDGE_FRACTION: f64 = 0.10;

pub fn run(cells_path: &Path, sources_path: &Path) {
    let cells_src = std::fs::read_to_string(cells_path)
        .unwrap_or_else(|e| panic!("read cells.toml at {}: {e}", cells_path.display()));
    let parsed: CellsFile =
        toml::from_str(&cells_src).unwrap_or_else(|e| panic!("parse cells.toml: {e}"));

    // Phase 5 (2026-06-03): the sources map now feeds both the
    // freshness report and the citation audit below.
    let sources_src = std::fs::read_to_string(sources_path)
        .unwrap_or_else(|e| panic!("read sources.toml at {}: {e}", sources_path.display()));
    let sources: BTreeMap<String, toml::Value> =
        toml::from_str(&sources_src).unwrap_or_else(|e| panic!("parse sources.toml: {e}"));

    println!("=== literature_matrix: {} cell(s) ===", parsed.cells.len());

    let mut blocking: Vec<String> = Vec::new();
    for cell in &parsed.cells {
        let (verdict, snapshot) = evaluate_cell(cell);
        println!("{}", render_text(&verdict, snapshot.as_ref()));
        println!(
            "--- JSON ({}) ---\n{}\n",
            cell.id,
            render_json(&verdict, snapshot.as_ref())
        );
        if verdict.overall.blocks_ci() {
            blocking.push(format!("{}: {}", cell.id, verdict.summary));
        }
    }

    // --- Phase 5: source freshness + citation audit ---
    let freshness = build_freshness_report(&sources);
    println!("{}", render_freshness_text(&freshness));
    println!(
        "--- JSON (freshness) ---\n{}\n",
        render_freshness_json(&freshness)
    );

    let citation = audit_citations(&parsed, &sources);
    println!("{}", render_citation_audit_text(&citation));

    let mut maintenance_failures: Vec<String> = Vec::new();
    if !citation.missing.is_empty() {
        maintenance_failures.push(format!(
            "{} cell citation(s) reference unknown sources",
            citation.missing.len()
        ));
    }
    let stale = freshness.stale_keys();
    if !stale.is_empty() && decay_fail_enabled() {
        maintenance_failures.push(format!(
            "{} stale source(s) (LIT_MATRIX_DECAY_FAIL=1): {}",
            stale.len(),
            stale.join(", "),
        ));
    }

    if !blocking.is_empty() {
        panic!(
            "literature_matrix: {} cell(s) reached major+ severity:\n  - {}",
            blocking.len(),
            blocking.join("\n  - "),
        );
    }

    if !maintenance_failures.is_empty() {
        panic!(
            "literature_matrix: maintenance check failed:\n  - {}",
            maintenance_failures.join("\n  - "),
        );
    }
}

pub fn evaluate_cell(cell: &LiteratureCell) -> (CellVerdict, Option<ShimSnapshot>) {
    // Resolve the cell's wrong-tool mode up front. Default is `values`
    // (normal-use) for cells that omit the [cell.expected_behaviour]
    // table entirely. The mode determines how shim errors are scored
    // (in `unusable` / `refuse` modes a refusal is a *pass*, not a
    // failure) and whether the final verdict is capped (in `unadvised`
    // the cell is warn-only).
    let mode = cell
        .expected_behaviour
        .as_ref()
        .map(|b| b.mode.to_lowercase())
        .unwrap_or_else(|| "values".to_owned());

    let snapshot_result = shim::run_cell(cell);

    match (mode.as_str(), snapshot_result) {
        // --- mode = "unusable" | "refuse": engine MUST refuse ---
        ("unusable" | "refuse", Err(e)) => {
            // Engine correctly refused — the cell passes. Record the
            // refusal as a Within row carrying the error message so the
            // operator can see what the refusal looked like. If
            // `expected_refuse_pattern` is set, sanity-check it; mismatch
            // is reported as Edge (informational only — the refusal
            // itself is what matters).
            let mut rows: Vec<SubVerdictRow> = Vec::new();
            let err_text = format!("{e}");
            let pattern_ok = cell
                .expected_behaviour
                .as_ref()
                .and_then(|b| b.expected_refuse_pattern.as_ref())
                .map(|p| substring_match(p, &err_text));
            let reason = format!("engine refused as expected: {err_text}");
            let detail = match pattern_ok {
                Some(true) | None => SubVerdictDetail::within(reason),
                Some(false) => SubVerdictDetail::edge(format!(
                    "{reason} (pattern `{}` did not match)",
                    cell.expected_behaviour
                        .as_ref()
                        .and_then(|b| b.expected_refuse_pattern.as_deref())
                        .unwrap_or("")
                )),
            };
            rows.push(SubVerdictRow {
                label: "wrong_tool.refusal".into(),
                detail,
                severity_on_fail: Severity::Minor,
            });
            let mut v = rollup(&cell.id, rows);
            v.mode = mode;
            (v, None)
        }
        ("unusable" | "refuse", Ok(snapshot)) => {
            // Engine accepted values for an unusable input — this is the
            // bug class the cell exists to catch. Surface a critical row
            // so the cell blocks CI.
            let mut rows: Vec<SubVerdictRow> = Vec::new();
            rows.push(SubVerdictRow {
                label: "wrong_tool.unusable_accepted".into(),
                detail: SubVerdictDetail::outside(
                    "engine accepted values for unusable input (should have refused)",
                ),
                severity_on_fail: Severity::Critical,
            });
            // Still attach the band/invariant rows so the operator can
            // see the values the engine produced.
            eval_bands(&cell.expected, &snapshot, &mut rows);
            eval_invariants(&cell.invariants, &snapshot, &mut rows);
            eval_anti_patterns(&cell.anti_patterns, &snapshot, &mut rows);
            let mut v = rollup(&cell.id, rows);
            v.mode = mode;
            (v, Some(snapshot))
        }
        // --- mode = "values" | "unadvised": engine SHOULD return values ---
        (_, Err(e)) => {
            // Unsupported tool/op/material — produce an NA-only verdict so
            // the runner can still emit a row. Shim coverage gaps surface
            // as NA, not failures.
            let detail = SubVerdictDetail::na(format!("shim: {e}"));
            let rows = vec![SubVerdictRow {
                label: "shim".into(),
                detail,
                severity_on_fail: Severity::Minor,
            }];
            let mut v = rollup(&cell.id, rows);
            v.mode = mode;
            (v, None)
        }
        (m, Ok(snapshot)) => {
            let mut rows: Vec<SubVerdictRow> = Vec::new();
            eval_bands(&cell.expected, &snapshot, &mut rows);
            eval_invariants(&cell.invariants, &snapshot, &mut rows);
            eval_anti_patterns(&cell.anti_patterns, &snapshot, &mut rows);

            // For unadvised cells, optionally pattern-match the engine
            // warnings against `expected_warning_pattern`. Missing
            // warning is informational — DO NOT fail the cell on it in
            // Phase 1 (the engine's warning surface is still maturing).
            if m == "unadvised"
                && let Some(pattern) = cell
                    .expected_behaviour
                    .as_ref()
                    .and_then(|b| b.expected_warning_pattern.as_ref())
            {
                let any_match = snapshot
                    .warnings
                    .iter()
                    .any(|w| substring_match(pattern, w));
                let detail = if any_match {
                    SubVerdictDetail::within(format!(
                        "engine emitted matching warning for `{pattern}`"
                    ))
                } else {
                    // Informational only. Severity Cosmetic so a
                    // missing warning never contributes to rollup.
                    SubVerdictDetail::edge(format!(
                        "no engine warning matched `{pattern}` (informational)"
                    ))
                };
                rows.push(SubVerdictRow {
                    label: "wrong_tool.warning_pattern".into(),
                    detail,
                    severity_on_fail: Severity::Cosmetic,
                });
                // TODO(phase2): if `expected_derate` is present, compare
                // engine output magnitude against (derate × reference)
                // for the matched normal-use cell. Tracked in plan.
            }

            let mut verdict = rollup(&cell.id, rows);
            verdict.mode = m.to_owned();

            // unadvised cells warn but never block CI — cap the overall
            // at Minor so even an Outside band can only ever raise a
            // Minor severity (which doesn't trip `blocks_ci`).
            if m == "unadvised" {
                verdict.cap_overall(Severity::Minor);
            }

            (verdict, Some(snapshot))
        }
    }
}

/// Cheap substring/case-insensitive match. We deliberately do NOT pull
/// in a regex dependency for Phase 1 — `expected_warning_pattern` and
/// `expected_refuse_pattern` are documented as substring patterns at the
/// schema level and Phase-1 cells stay within that.
fn substring_match(pattern: &str, haystack: &str) -> bool {
    let p = pattern.to_lowercase();
    let h = haystack.to_lowercase();
    h.contains(&p)
}

fn eval_bands(expected: &ExpectedBands, snap: &ShimSnapshot, rows: &mut Vec<SubVerdictRow>) {
    if let Some(band) = &expected.rpm {
        push_band_row("rpm", snap.rpm, band, rows);
    }
    if let Some(band) = &expected.feed_per_tooth {
        push_band_row("fpt", snap.effective_chip_load_mm, band, rows);
    }
    if let Some(band) = &expected.axial_doc {
        push_band_row("axial_doc", snap.axial_doc_mm, band, rows);
    }
    if let Some(band) = &expected.radial_woc {
        push_band_row("radial_woc", snap.radial_woc_mm, band, rows);
    }
    if let Some(band) = &expected.plunge_feed {
        push_plunge_band_row(band, snap, rows);
    }
    // Extras: numeric bands keyed by a custom name. The shim's bindings map
    // provides the value source; if the key is unbound we mark NA.
    for (key, band) in &expected.extras {
        let value = snap.bindings.get(key).copied();
        match value {
            Some(v) => push_band_row(key, v, band, rows),
            None => rows.push(SubVerdictRow {
                label: format!("expected.{key}"),
                detail: SubVerdictDetail::na(format!("no binding for `{key}`")),
                severity_on_fail: Severity::Minor,
            }),
        }
    }
}

fn push_band_row(label: &str, value: f64, band: &Band, rows: &mut Vec<SubVerdictRow>) {
    let mode = BandMode::parse(band.mode.as_deref());
    let detail = band_check(value, band.min, band.max, mode, EDGE_FRACTION);
    rows.push(SubVerdictRow {
        label: label.into(),
        detail,
        severity_on_fail: Severity::Moderate, // band misses are moderate by default
    });
}

fn push_plunge_band_row(band: &Band, snap: &ShimSnapshot, rows: &mut Vec<SubVerdictRow>) {
    // plunge_feed mode = fraction: min_fraction_of_feed / max_fraction_of_feed
    if snap.feed_rate_mm_min.abs() < f64::EPSILON {
        rows.push(SubVerdictRow {
            label: "plunge/feed".into(),
            detail: SubVerdictDetail::na("feed_rate is zero"),
            severity_on_fail: Severity::Minor,
        });
        return;
    }
    let ratio = snap.plunge_rate_mm_min / snap.feed_rate_mm_min;
    let detail = band_check(
        ratio,
        band.min_fraction_of_feed.or(band.min),
        band.max_fraction_of_feed.or(band.max),
        BandMode::Fraction,
        EDGE_FRACTION,
    );
    rows.push(SubVerdictRow {
        label: "plunge/feed".into(),
        detail,
        severity_on_fail: Severity::Minor, // plunge ratio is advisory
    });
}

fn eval_invariants(invariants: &[Invariant], snap: &ShimSnapshot, rows: &mut Vec<SubVerdictRow>) {
    for inv in invariants {
        let sev = Severity::parse(&inv.severity_on_fail);
        let detail = match inv.r#type.as_deref() {
            Some("convex_hull") => {
                if inv.vars.len() != 2 || inv.vertices.len() < 3 {
                    SubVerdictDetail::na("convex_hull needs 2 vars + ≥3 vertices")
                } else {
                    let (Some(&x), Some(&y)) = (
                        snap.bindings.get(&inv.vars[0]),
                        snap.bindings.get(&inv.vars[1]),
                    ) else {
                        rows.push(SubVerdictRow {
                            label: format!("invariant.{}", inv.name),
                            detail: SubVerdictDetail::na(format!(
                                "missing binding for {:?}",
                                inv.vars
                            )),
                            severity_on_fail: sev,
                        });
                        continue;
                    };
                    convex_hull_check((x, y), &inv.vertices)
                }
            }
            _ => {
                let Some(expr) = inv.expr.as_deref() else {
                    rows.push(SubVerdictRow {
                        label: format!("invariant.{}", inv.name),
                        detail: SubVerdictDetail::na("expr invariant missing `expr`"),
                        severity_on_fail: sev,
                    });
                    continue;
                };
                expr_check(expr, &snap.bindings, inv.floor, inv.ceiling, EDGE_FRACTION)
            }
        };
        rows.push(SubVerdictRow {
            label: format!("invariant.{}", inv.name),
            detail,
            severity_on_fail: sev,
        });
    }
}

fn eval_anti_patterns(
    patterns: &[AntiPattern],
    snap: &ShimSnapshot,
    rows: &mut Vec<SubVerdictRow>,
) {
    for ap in patterns {
        let detail = anti_pattern_check(&ap.expr, &snap.bindings);
        rows.push(SubVerdictRow {
            label: format!("anti.{}", ap.name),
            detail,
            severity_on_fail: Severity::parse(&ap.severity),
        });
    }
}

#[allow(dead_code)] // utility for harness_tests / future Phase 1 wiring
pub fn worst_verdict(rows: &[SubVerdictRow]) -> SubVerdict {
    let mut worst = SubVerdict::Within;
    for r in rows {
        worst = match (worst, r.detail.verdict) {
            (_, SubVerdict::Outside) => SubVerdict::Outside,
            (SubVerdict::Outside, _) => SubVerdict::Outside,
            (_, SubVerdict::Edge) | (SubVerdict::Edge, _) => SubVerdict::Edge,
            (SubVerdict::NA, x) => x,
            (x, SubVerdict::NA) => x,
            _ => SubVerdict::Within,
        };
    }
    worst
}
