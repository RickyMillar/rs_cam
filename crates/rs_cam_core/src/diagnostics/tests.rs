//! Unit tests for the diagnostics module. Each adapter and the
//! reducer are tested in isolation; integration with `ProjectSession`
//! lands in PR-3 once consumers cut over.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use super::*;
use crate::diagnostics::adapters;
use crate::ids::ToolpathId;

fn make_diag(
    id: &str,
    state: DiagnosticState,
    supersedes: Vec<&str>,
    severity: Severity,
) -> Diagnostic {
    Diagnostic {
        id: DiagnosticId::from(id),
        scope: Scope::Toolpath { id: ToolpathId(0) },
        category: Category::ToolLoad,
        severity,
        confidence: Confidence::Verified,
        state,
        source: Source::ToolLoad,
        message: format!("test {id}"),
        evidence: None,
        fix: None,
        supersedes: supersedes.into_iter().map(DiagnosticId::from).collect(),
        suppressed_diagnostics: vec![],
    }
}

// ── supersession reducer ────────────────────────────────────────────

#[test]
fn all_diagnostic_ids_are_unique() {
    use std::collections::HashSet;
    let mut seen = HashSet::new();
    for id in ids::ALL {
        assert!(
            seen.insert(*id),
            "duplicate diagnostic id in `ids::ALL`: {id}"
        );
    }
}

/// F162: every id is a row of the `diag_ids!` macro, so `ALL` holds it.
/// A `const NAME: &str` written outside the macro block would miss `ALL`;
/// this scan counts every such const in `ids.rs` and compares.
#[test]
fn every_id_const_in_ids_rs_is_in_all() {
    let src = include_str!("ids.rs");
    let consts = src
        .lines()
        .map(str::trim_start)
        .filter(|l| {
            (l.starts_with("pub const ") || l.starts_with("pub(crate) const "))
                && l.contains(": &str")
        })
        .count();
    // Non-vacuity: the scan sees the ids (102 on 2026-10-20, and only more later).
    assert!(consts >= 100, "the scan found {consts} id consts");
    assert_eq!(
        consts,
        ids::ALL.len(),
        "an id const in ids.rs sits outside the diag_ids! block"
    );
    assert!(ids::ALL.contains(&ids::GEOM_CLIPPED_BAND));
    assert!(ids::ALL.contains(&ids::CONFIG_CLAIMS_REFERENCE));
}

/// Sanity check: every constant string used as a diagnostic ID must be
/// referenced *somewhere* in the crate source outside `ids.rs`. Catches
/// orphaned constants (lint policy denies dead_code, but a constant
/// can still drift out of every emitter). The id list is
/// `ids::NAMED`, which the `diag_ids!` macro writes, so a new id is
/// checked with no edit here (F162).
#[test]
fn all_diagnostic_ids_reachable_in_adapter_source() {
    // Read every file under `src/` and assert each ID appears at least
    // once outside `ids.rs` and the `tests.rs` files. A reachable ID need
    // not be emitted under every scenario — we just need to know it's
    // wired. Some ids are emitted outside `diagnostics/` (for example
    // `stock/sim_triage.rs`), so the scan reads the whole crate source.
    use std::fs::read_to_string;
    use std::path::Path;

    fn collect_rs_files(root: &Path, out: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(root).into_iter().flatten().flatten() {
            let p = entry.path();
            if p.is_dir() {
                collect_rs_files(&p, out);
            } else if p.extension().and_then(|e| e.to_str()) == Some("rs")
                && p.file_name().and_then(|n| n.to_str()) != Some("ids.rs")
                && p.file_name().and_then(|n| n.to_str()) != Some("tests.rs")
            {
                out.push(p);
            }
        }
    }
    let mut files = Vec::new();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    collect_rs_files(&root, &mut files);
    // Non-vacuity: the scan read the adapters, and the list holds every id.
    assert!(
        files
            .iter()
            .any(|p| p.ends_with("diagnostics/adapters/from_generation.rs")),
        "the source scan did not find the adapters"
    );
    assert_eq!(ids::NAMED.len(), ids::ALL.len());
    let merged: String = files
        .iter()
        .filter_map(|p| read_to_string(p).ok())
        .collect::<Vec<_>>()
        .join("\n");

    // IDs that are only emitted via a centralised helper get their
    // own constant referenced indirectly — accept either the symbol
    // name (`LOAD_CHIPLOAD_HIGH`) or the literal string
    // (`"load.chipload.high"`).
    //
    // KNOWN ORPHAN: no emitter raises `PROJECT_KINEMATIC_UTILIZATION`
    // (dead `pub(crate)` const since 69e71151; W7 found it when this scan
    // started to read the whole id list). The list holds it so the scan
    // stays green; the check below fails the day an emitter lands, and
    // then this entry goes.
    const KNOWN_ORPHANS: &[&str] = &["PROJECT_KINEMATIC_UTILIZATION"];
    let mut missing = Vec::new();
    for (symbol, literal) in ids::NAMED {
        let reached = merged.contains(symbol) || merged.contains(&format!("\"{literal}\""));
        let orphan = KNOWN_ORPHANS.contains(symbol);
        assert!(
            !(reached && orphan),
            "{symbol} now has an emitter: remove it from KNOWN_ORPHANS"
        );
        if !reached && !orphan {
            missing.push(format!("{symbol} (= {literal:?})"));
        }
    }
    assert!(
        missing.is_empty(),
        "diagnostic IDs declared in `ids.rs` but never referenced in the source: {missing:#?}"
    );
}

#[test]
fn reducer_records_suppressed_ids_on_surviving_authoritative() {
    let diagnostics = vec![
        make_diag(
            ids::LOAD_CHIPLOAD_WITHIN,
            DiagnosticState::Current,
            vec![ids::FEEDS_FEED_VS_LUT_HIGH, ids::FEEDS_STEPOVER_VS_LUT],
            Severity::Info,
        ),
        make_diag(
            ids::FEEDS_FEED_VS_LUT_HIGH,
            DiagnosticState::Current,
            vec![],
            Severity::Hint,
        ),
    ];
    let reduced = apply_supersession(diagnostics);
    assert_eq!(reduced.len(), 1);
    let surviving = &reduced[0];
    assert_eq!(surviving.id.as_str(), ids::LOAD_CHIPLOAD_WITHIN);
    // Only the id present in the input gets recorded — the
    // `FEEDS_STEPOVER_VS_LUT` entry in the supersedes list never
    // appeared so we don't claim to have suppressed it.
    let actually_suppressed: Vec<_> = surviving
        .suppressed_diagnostics
        .iter()
        .map(|d| d.0.clone())
        .collect();
    assert_eq!(actually_suppressed, vec![ids::FEEDS_FEED_VS_LUT_HIGH]);
}

#[test]
fn reducer_drops_heuristic_when_authoritative_is_current() {
    let diagnostics = vec![
        make_diag(
            ids::LOAD_CHIPLOAD_WITHIN,
            DiagnosticState::Current,
            vec![ids::FEEDS_FEED_VS_LUT_HIGH],
            Severity::Info,
        ),
        make_diag(
            ids::FEEDS_FEED_VS_LUT_HIGH,
            DiagnosticState::Current,
            vec![],
            Severity::Hint,
        ),
    ];
    let reduced = apply_supersession(diagnostics);
    assert_eq!(reduced.len(), 1);
    assert_eq!(reduced[0].id.as_str(), ids::LOAD_CHIPLOAD_WITHIN);
}

#[test]
fn reducer_keeps_heuristic_when_authoritative_is_needs_simulation() {
    let diagnostics = vec![
        make_diag(
            ids::LOAD_CHIPLOAD_WITHIN,
            DiagnosticState::NeedsSimulation,
            vec![ids::FEEDS_FEED_VS_LUT_HIGH],
            Severity::Info,
        ),
        make_diag(
            ids::FEEDS_FEED_VS_LUT_HIGH,
            DiagnosticState::Current,
            vec![],
            Severity::Hint,
        ),
    ];
    let reduced = apply_supersession(diagnostics);
    // Both survive — the authoritative isn't current yet, so it
    // can't silence the pre-sim heuristic.
    assert_eq!(reduced.len(), 2);
}

#[test]
fn reducer_keeps_heuristic_when_authoritative_is_stale() {
    let diagnostics = vec![
        make_diag(
            ids::LOAD_DEFLECTION_WITHIN,
            DiagnosticState::StaleEvidence,
            vec![ids::FEEDS_DPP_VS_LUT],
            Severity::Info,
        ),
        make_diag(
            ids::FEEDS_DPP_VS_LUT,
            DiagnosticState::Current,
            vec![],
            Severity::Hint,
        ),
    ];
    let reduced = apply_supersession(diagnostics);
    assert_eq!(reduced.len(), 2);
}

#[test]
fn reducer_is_idempotent() {
    let diagnostics = vec![
        make_diag(
            ids::LOAD_CHIPLOAD_WITHIN,
            DiagnosticState::Current,
            vec![ids::FEEDS_FEED_VS_LUT_HIGH, ids::FEEDS_STEPOVER_VS_LUT],
            Severity::Info,
        ),
        make_diag(
            ids::FEEDS_FEED_VS_LUT_HIGH,
            DiagnosticState::Current,
            vec![],
            Severity::Hint,
        ),
        make_diag(
            ids::FEEDS_STEPOVER_VS_LUT,
            DiagnosticState::Current,
            vec![],
            Severity::Hint,
        ),
    ];
    let once = apply_supersession(diagnostics);
    let ids_once: Vec<_> = once.iter().map(|d| d.id.clone()).collect();
    let twice = apply_supersession(once);
    let ids_twice: Vec<_> = twice.iter().map(|d| d.id.clone()).collect();
    assert_eq!(ids_once, ids_twice);
}

#[test]
fn reducer_preserves_unrelated_diagnostics() {
    let diagnostics = vec![
        make_diag(
            ids::LOAD_CHIPLOAD_WITHIN,
            DiagnosticState::Current,
            vec![ids::FEEDS_FEED_VS_LUT_HIGH],
            Severity::Info,
        ),
        make_diag(
            ids::PROJECT_RAPID_COLLISION,
            DiagnosticState::Current,
            vec![],
            Severity::Critical,
        ),
    ];
    let reduced = apply_supersession(diagnostics);
    assert_eq!(reduced.len(), 2);
    assert!(
        reduced
            .iter()
            .any(|d| d.id.as_str() == ids::PROJECT_RAPID_COLLISION)
    );
}

// ── severity ladder ─────────────────────────────────────────────────

#[test]
fn severity_rank_orders_correctly() {
    assert!(Severity::Blocking.rank() > Severity::Critical.rank());
    assert!(Severity::Critical.rank() > Severity::Caution.rank());
    assert!(Severity::Caution.rank() > Severity::Hint.rank());
    assert!(Severity::Hint.rank() > Severity::Info.rank());
}

#[test]
fn severity_is_actionable() {
    assert!(Severity::Blocking.is_actionable());
    assert!(Severity::Critical.is_actionable());
    assert!(Severity::Caution.is_actionable());
    assert!(!Severity::Hint.is_actionable());
    assert!(!Severity::Info.is_actionable());
}

// ── tool-load adapter ───────────────────────────────────────────────

#[test]
fn tool_load_adapter_drops_milling_na_on_drill_with_drill_gates() {
    use crate::tool_load::drill_gates::{DrillGateOutcome, DrillGatesVerdict};
    use crate::tool_load::verdict::*;

    let verdict = ToolpathLoadVerdict {
        toolpath_id: ToolpathId(0),
        chipload: ChiploadVerdict::Unmodeled {
            reason: UnmodeledReason::NotApplicableForOp(
                "drill cycle — no continuous engagement".to_owned(),
            ),
        },
        power: PowerVerdict::Unmodeled {
            reason: UnmodeledReason::NotApplicableForOp(
                "drill cycle — no continuous engagement".to_owned(),
            ),
        },
        deflection: DeflectionVerdict::Unmodeled {
            reason: UnmodeledReason::NotApplicableForOp(
                "drill cycle — no continuous engagement".to_owned(),
            ),
        },
        // S3: the depth row takes the same arm as the three milling
        // gates beside it, so the drill partition is unchanged.
        depth: DepthVerdict::Unmodeled {
            reason: UnmodeledReason::NotApplicableForOp(
                "drill cycle — no continuous engagement".to_owned(),
            ),
        },
        drill_gates: Some(DrillGatesVerdict {
            chip_welding: DrillGateOutcome::Within {
                observed: 4.0,
                threshold: 8.0,
                envelope_lo: None,
                envelope_hi: None,
            },
            peck_adequacy: DrillGateOutcome::Within {
                observed: 0.5,
                threshold: 2.0,
                envelope_lo: None,
                envelope_hi: None,
            },
            plunge_feed: DrillGateOutcome::Within {
                observed: 100.0,
                threshold: 100.0,
                envelope_lo: Some(50.0),
                envelope_hi: Some(400.0),
            },
            // R-7: no hole attribution in a hand-built verdict.
            worst_hole_id: None,
            // X-VAC: not stated by this fixture — it exercises the
            // gate wording, not the population marker. `None` reads as
            // "not stated", never as an empty gate.
            population: None,
            cycle: crate::tool_load::drill_gates::DrillCycleKind::Peck,
        }),
        modulation_summary: None,
        feed_explanation: None,
        kinematic_utilization: None,
        material_split: None,
    };
    let diags = adapters::from_tool_load::diagnostics_from_load_verdict(&verdict);
    assert_eq!(diags.len(), 3, "drill gates only, no milling N/A noise");
    let ids_seen: Vec<&str> = diags.iter().map(|d| d.id.as_str()).collect();
    assert!(ids_seen.contains(&ids::DRILL_CHIP_WELDING));
    assert!(ids_seen.contains(&ids::DRILL_PECK_ADEQUACY));
    assert!(ids_seen.contains(&ids::DRILL_PLUNGE_FEED));
    assert!(!ids_seen.contains(&ids::LOAD_CHIPLOAD_WITHIN));
}

#[test]
fn tool_load_adapter_emits_chipload_exceeds_with_evidence() {
    use crate::tool_load::verdict::*;

    let triggering = ChiploadMetric {
        observed_mm_per_tooth: 0.0011,
        statistic: ChiploadStatistic::MedianLow,
        evidence: SampleEvidence {
            sample_range: 188118..188119,
            statistic: Some(ChiploadStatistic::MedianLow),
            locality: Some("arc-fit".to_owned()),
            // X-VAC: this fixture reproduces the 2026-08-05 arc-fit
            // reading, which had a real triggering sample. Not vacuous.
            population: None,
        },
        bounds: ChipBounds {
            min_mm_per_tooth: Some(0.0305),
            max_mm_per_tooth: 0.0508,
            source: ChipBoundsSource::VendorLutExtrapolated,
        },
    };
    let verdict = ToolpathLoadVerdict {
        toolpath_id: ToolpathId(12),
        chipload: ChiploadVerdict::Exceeds {
            side: ChipSide::Low,
            triggering,
            confidence: Confidence::Approximate(
                "extrapolated from row amana-flat-hardwood-contour-3175-2f".to_owned(),
            ),
        },
        power: PowerVerdict::Unmodeled {
            reason: UnmodeledReason::NotImplemented("phase 1b".to_owned()),
        },
        deflection: DeflectionVerdict::Unmodeled {
            reason: UnmodeledReason::SimulationRequired,
        },
        // S3: this fixture supplies no trace, so the depth gate has
        // nothing to measure either.
        depth: DepthVerdict::Unmodeled {
            reason: UnmodeledReason::SimulationRequired,
        },
        drill_gates: None,
        modulation_summary: None,
        feed_explanation: None,
        kinematic_utilization: None,
        material_split: None,
    };
    let diags = adapters::from_tool_load::diagnostics_from_load_verdict(&verdict);
    let chip = diags
        .iter()
        .find(|d| d.id.as_str() == ids::LOAD_CHIPLOAD_LOW)
        .expect("chipload low diagnostic present");
    assert_eq!(chip.severity, Severity::Caution);
    assert_eq!(chip.state, DiagnosticState::Current);
    assert!(matches!(
        chip.evidence,
        Some(DiagnosticEvidence::SampleRange { .. })
    ));
    assert!(
        chip.supersedes
            .iter()
            .any(|id| id.as_str() == ids::FEEDS_FEED_VS_LUT_LOW)
    );
}

#[test]
fn tool_load_adapter_marks_stale_simulation_as_stale_evidence_state() {
    use crate::tool_load::verdict::*;

    let verdict = ToolpathLoadVerdict {
        toolpath_id: ToolpathId(4),
        chipload: ChiploadVerdict::Unmodeled {
            reason: UnmodeledReason::StaleSimulation,
        },
        power: PowerVerdict::Unmodeled {
            reason: UnmodeledReason::StaleSimulation,
        },
        deflection: DeflectionVerdict::Unmodeled {
            reason: UnmodeledReason::StaleSimulation,
        },
        // S3: every row is stale, including this one.
        depth: DepthVerdict::Unmodeled {
            reason: UnmodeledReason::StaleSimulation,
        },
        drill_gates: None,
        modulation_summary: None,
        feed_explanation: None,
        kinematic_utilization: None,
        material_split: None,
    };
    let diags = adapters::from_tool_load::diagnostics_from_load_verdict(&verdict);
    assert!(!diags.is_empty());
    for d in &diags {
        assert_eq!(
            d.state,
            DiagnosticState::StaleEvidence,
            "{} should be stale, got {:?}",
            d.id.as_str(),
            d.state
        );
        assert_eq!(d.category, Category::State);
        // Stale evidence is neutral, not actionable.
        assert!(!d.severity.is_actionable());
    }
}

#[test]
fn tool_load_adapter_marks_sim_required_as_needs_simulation_state() {
    use crate::tool_load::verdict::*;

    let verdict = ToolpathLoadVerdict {
        toolpath_id: ToolpathId(7),
        chipload: ChiploadVerdict::Unmodeled {
            reason: UnmodeledReason::SimulationRequired,
        },
        power: PowerVerdict::Unmodeled {
            reason: UnmodeledReason::SimulationRequired,
        },
        deflection: DeflectionVerdict::Unmodeled {
            reason: UnmodeledReason::SimulationRequired,
        },
        // S3: no simulation, so no measured depth either.
        depth: DepthVerdict::Unmodeled {
            reason: UnmodeledReason::SimulationRequired,
        },
        drill_gates: None,
        modulation_summary: None,
        feed_explanation: None,
        kinematic_utilization: None,
        material_split: None,
    };
    let diags = adapters::from_tool_load::diagnostics_from_load_verdict(&verdict);
    assert!(!diags.is_empty());
    for d in &diags {
        assert_eq!(
            d.state,
            DiagnosticState::NeedsSimulation,
            "{}",
            d.id.as_str()
        );
        assert_eq!(d.category, Category::State);
        assert_eq!(d.severity, Severity::Info);
    }
}

// ── stale-default adapter ───────────────────────────────────────────

#[test]
fn stale_default_adapter_carries_fix() {
    use crate::compute::validate::{StaleDefault, StaleDefaultRule};

    let defects = vec![StaleDefault {
        rule_id: StaleDefaultRule::ProjectCurveNegativeDepth,
        toolpath_id: ToolpathId(12),
        toolpath_name: "Rivers (back) (copy)".to_owned(),
        title: "Project-curve depth is negative".to_owned(),
        detail: "depth is -2.0".to_owned(),
        new_value: 2.0,
        current_value: -2.0,
    }];
    let diags = adapters::from_stale_default::diagnostics_from_stale_defaults(&defects);
    assert_eq!(diags.len(), 1);
    let d = &diags[0];
    assert_eq!(d.id.as_str(), ids::STALE_PROJECT_CURVE_NEGATIVE_DEPTH);
    assert_eq!(d.severity, Severity::Caution);
    assert_eq!(d.category, Category::Safety);
    match &d.fix {
        Some(DiagnosticFix::ApplyStaleDefault {
            rule_id,
            toolpath_id,
            new_value,
        }) => {
            assert_eq!(rule_id, "project_curve_negative_depth");
            assert_eq!(*toolpath_id, ToolpathId(12));
            assert!((new_value - 2.0).abs() < 1e-9);
        }
        other => panic!("expected ApplyStaleDefault fix, got {other:?}"),
    }
}

// ── feeds heuristic hints ───────────────────────────────────────────

#[test]
fn feeds_hint_emits_high_feed_ratio() {
    use crate::feeds::{ChiploadSource, FeedsResult};

    let result = FeedsResult {
        rpm: 18_000.0,
        chip_load_mm: 0.02,
        feed_rate_mm_min: 1000.0,
        plunge_rate_mm_min: 400.0,
        plunge: crate::feeds::PlungeBasis::DrillCycle,
        ramp: crate::feeds::RampBasis::NoChip {
            reason: crate::feeds::RampFallback::NoLut,
        },
        axial_depth_mm: 4.0,
        radial_width_mm: 1.0,
        power_kw: 0.3,
        available_power_kw: 0.6,
        power_limited: false,
        mrr_mm3_min: 1200.0,
        warnings: vec![],
        vendor_source: None,
        support: crate::feeds::FeedsSupport::FormulaOnly {
            source: crate::feeds::support::MILLING_FORMULA_SOURCE,
        },
        chipload_source: ChiploadSource::FormulaFallback,
        chipload_bounds: None,
        chipload_point_mm: None,
        matched_lut_row: None,
        effective_diameter_mm: 6.0,
        derates: crate::feeds::FeedsDerates::default(),
    };
    let diags = adapters::from_feeds::heuristic_hints_from_recommendation(
        ToolpathId(7),
        2500.0, // commanded feed = 2.5× recommended
        None,
        None,
        &result,
    );
    assert_eq!(diags.len(), 1);
    let d = &diags[0];
    assert_eq!(d.id.as_str(), ids::FEEDS_FEED_VS_LUT_HIGH);
    assert_eq!(d.severity, Severity::Hint);
    assert_eq!(d.confidence, Confidence::Heuristic);
}

// ── Project Curve point projection ──────────────────────────────────

/// Point projection is a deliberate operator choice. The static checks give
/// the caution for Point, with the agreed text, and give nothing for Cutter.
#[test]
fn project_curve_point_projection_gives_the_caution() {
    use crate::compute::catalog::OperationConfig;
    use crate::compute::operation_configs::{
        PROJECT_CURVE_POINT_CAUTION, ProjectCurveConfig, ProjectCurveProjection,
    };
    use crate::compute::tool_config::{ToolConfig, ToolId, ToolType};

    let find = |projection: ProjectCurveProjection, tool_type: ToolType| {
        let op = OperationConfig::ProjectCurve(ProjectCurveConfig {
            projection,
            ..ProjectCurveConfig::default()
        });
        let tool = ToolConfig::new_default(ToolId(0), tool_type);
        adapters::from_static_checks::diagnostics_from_static_checks(
            ToolpathId(3),
            &op,
            &tool,
            None,
        )
        .into_iter()
        .filter(|d| d.id.as_str() == ids::COMPAT_PROJECT_CURVE_POINT_PROJECTION)
        .collect::<Vec<_>>()
    };

    for tool_type in [ToolType::VBit, ToolType::BallNose, ToolType::EndMill] {
        let point = find(ProjectCurveProjection::Point, tool_type);
        assert_eq!(point.len(), 1, "one caution for Point on {tool_type:?}");
        assert_eq!(point[0].severity, Severity::Caution);
        assert_eq!(point[0].message, PROJECT_CURVE_POINT_CAUTION);
        assert_eq!(
            PROJECT_CURVE_POINT_CAUTION,
            "Point projection does not protect the far surface: the tool body can cut it. \
             Use only when the tool cuts from the other side or into material that is \
             removed later."
        );
        assert!(
            find(ProjectCurveProjection::Cutter, tool_type).is_empty(),
            "no caution for Cutter on {tool_type:?}"
        );
    }
}
