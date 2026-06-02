//! Per-cell evaluator and matrix-level driver.
//!
//! Loads cells + sources from TOML, runs each cell through the shim,
//! computes sub-verdicts via the primitives, rolls up to a cell verdict,
//! and emits both text + JSON to stdout. Fails the test only when an
//! overall verdict reaches `major` or `critical` (per plan §"CI policy").

use super::cell::{AntiPattern, Band, CellsFile, ExpectedBands, Invariant, LiteratureCell};
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
    let parsed: CellsFile = toml::from_str(&cells_src)
        .unwrap_or_else(|e| panic!("parse cells.toml: {e}"));

    // Sources file is parsed lazily for now (Phase 0 doesn't dereference
    // citations, only validates the file is well-formed TOML).
    let sources_src = std::fs::read_to_string(sources_path)
        .unwrap_or_else(|e| panic!("read sources.toml at {}: {e}", sources_path.display()));
    let _sources: BTreeMap<String, toml::Value> = toml::from_str(&sources_src)
        .unwrap_or_else(|e| panic!("parse sources.toml: {e}"));

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

    if !blocking.is_empty() {
        panic!(
            "literature_matrix: {} cell(s) reached major+ severity:\n  - {}",
            blocking.len(),
            blocking.join("\n  - "),
        );
    }
}

pub fn evaluate_cell(cell: &LiteratureCell) -> (CellVerdict, Option<ShimSnapshot>) {
    let snapshot = match shim::run_cell(cell) {
        Ok(s) => s,
        Err(e) => {
            // Unsupported tool/op/material — produce an NA-only verdict so
            // the runner can still emit a row. This is Phase 0 behaviour;
            // Phase 1 will grow the shim to handle the starter 12.
            let detail = SubVerdictDetail::na(format!("shim: {e}"));
            let rows = vec![SubVerdictRow {
                label: "shim".into(),
                detail,
                severity_on_fail: Severity::Minor,
            }];
            return (rollup(&cell.id, rows), None);
        }
    };

    let mut rows: Vec<SubVerdictRow> = Vec::new();
    eval_bands(&cell.expected, &snapshot, &mut rows);
    eval_invariants(&cell.invariants, &snapshot, &mut rows);
    eval_anti_patterns(&cell.anti_patterns, &snapshot, &mut rows);

    let verdict = rollup(&cell.id, rows);
    (verdict, Some(snapshot))
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

fn eval_invariants(
    invariants: &[Invariant],
    snap: &ShimSnapshot,
    rows: &mut Vec<SubVerdictRow>,
) {
    for inv in invariants {
        let sev = Severity::parse(&inv.severity_on_fail);
        let detail = match inv.r#type.as_deref() {
            Some("convex_hull") => {
                if inv.vars.len() != 2 || inv.vertices.len() < 3 {
                    SubVerdictDetail::na("convex_hull needs 2 vars + ≥3 vertices")
                } else {
                    let (Some(&x), Some(&y)) =
                        (snap.bindings.get(&inv.vars[0]), snap.bindings.get(&inv.vars[1]))
                    else {
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
