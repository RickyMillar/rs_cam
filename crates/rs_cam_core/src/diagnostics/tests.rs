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

fn make_diag(
    id: &str,
    state: DiagnosticState,
    supersedes: Vec<&str>,
    severity: Severity,
) -> Diagnostic {
    Diagnostic {
        id: DiagnosticId::from(id),
        scope: Scope::Toolpath { id: 0 },
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

/// Sanity check: every constant string used as a diagnostic ID must be
/// referenced *somewhere* in the adapter / orchestrator code. Catches
/// orphaned constants (lint policy denies dead_code, but a constant
/// can still drift out of every emitter).
#[test]
fn all_diagnostic_ids_reachable_in_adapter_source() {
    // Read every file under `diagnostics/` and assert each ID appears
    // at least once outside `ids.rs` itself. A reachable ID need not
    // be emitted under every scenario — we just need to know it's wired.
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
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/diagnostics");
    collect_rs_files(&root, &mut files);
    let merged: String = files
        .iter()
        .filter_map(|p| read_to_string(p).ok())
        .collect::<Vec<_>>()
        .join("\n");

    // IDs that are only emitted via a centralised helper get their
    // own constant referenced indirectly — accept either the symbol
    // name (`LOAD_CHIPLOAD_HIGH`) or the literal string
    // (`"load.chipload.high"`).
    let consts: &[(&str, &str)] = &[
        ("LOAD_CHIPLOAD_HIGH", ids::LOAD_CHIPLOAD_HIGH),
        ("LOAD_CHIPLOAD_LOW", ids::LOAD_CHIPLOAD_LOW),
        ("LOAD_CHIPLOAD_WITHIN", ids::LOAD_CHIPLOAD_WITHIN),
        ("LOAD_POWER_EXCEEDS", ids::LOAD_POWER_EXCEEDS),
        ("LOAD_POWER_WITHIN", ids::LOAD_POWER_WITHIN),
        ("LOAD_DEFLECTION_EXCEEDS", ids::LOAD_DEFLECTION_EXCEEDS),
        ("LOAD_DEFLECTION_WITHIN", ids::LOAD_DEFLECTION_WITHIN),
        ("DRILL_CHIP_WELDING", ids::DRILL_CHIP_WELDING),
        ("DRILL_PECK_ADEQUACY", ids::DRILL_PECK_ADEQUACY),
        ("DRILL_PLUNGE_FEED", ids::DRILL_PLUNGE_FEED),
        ("PROJECT_HOLDER_COLLISION", ids::PROJECT_HOLDER_COLLISION),
        ("PROJECT_RAPID_COLLISION", ids::PROJECT_RAPID_COLLISION),
        ("PROJECT_PLUNGE_STRESS", ids::PROJECT_PLUNGE_STRESS),
        ("PROJECT_AIR_CUT_HIGH", ids::PROJECT_AIR_CUT_HIGH),
        ("PROJECT_GENERATED_EMPTY", ids::PROJECT_GENERATED_EMPTY),
        ("FEEDS_FEED_CLAMPED", ids::FEEDS_FEED_CLAMPED),
        ("FEEDS_POWER_LIMITED", ids::FEEDS_POWER_LIMITED),
        ("FEEDS_SHANK_TOO_LARGE", ids::FEEDS_SHANK_TOO_LARGE),
        ("FEEDS_DOC_EXCEEDS_FLUTE", ids::FEEDS_DOC_EXCEEDS_FLUTE),
        ("FEEDS_SLOTTING_DETECTED", ids::FEEDS_SLOTTING_DETECTED),
        ("FEEDS_SCALLOP_INVALID", ids::FEEDS_SCALLOP_INVALID),
        ("FEEDS_FEED_VS_LUT_HIGH", ids::FEEDS_FEED_VS_LUT_HIGH),
        ("FEEDS_FEED_VS_LUT_LOW", ids::FEEDS_FEED_VS_LUT_LOW),
        ("FEEDS_STEPOVER_VS_LUT", ids::FEEDS_STEPOVER_VS_LUT),
        ("FEEDS_DPP_VS_LUT", ids::FEEDS_DPP_VS_LUT),
        (
            "GEOM_STEPOVER_EXCEEDS_DIAMETER",
            ids::GEOM_STEPOVER_EXCEEDS_DIAMETER,
        ),
        (
            "GEOM_DPP_EXCEEDS_CUTTING_LENGTH",
            ids::GEOM_DPP_EXCEEDS_CUTTING_LENGTH,
        ),
        (
            "GEOM_DPP_OVER_1_5X_DIAMETER",
            ids::GEOM_DPP_OVER_1_5X_DIAMETER,
        ),
        ("GEOM_BOTTOM_ABOVE_TOP_Z", ids::GEOM_BOTTOM_ABOVE_TOP_Z),
        ("GEOM_FEED_Z_BELOW_TOP_Z", ids::GEOM_FEED_Z_BELOW_TOP_Z),
        (
            "GEOM_RETRACT_Z_BELOW_FEED_Z",
            ids::GEOM_RETRACT_Z_BELOW_FEED_Z,
        ),
        (
            "GEOM_CLEARANCE_Z_BELOW_RETRACT_Z",
            ids::GEOM_CLEARANCE_Z_BELOW_RETRACT_Z,
        ),
        ("GEOM_PLUNGE_EXCEEDS_FEED", ids::GEOM_PLUNGE_EXCEEDS_FEED),
        (
            "COMPAT_END_MILL_SCALLOP_PENCIL",
            ids::COMPAT_END_MILL_SCALLOP_PENCIL,
        ),
        (
            "COMPAT_BALL_NOSE_FLAT_CLEARING",
            ids::COMPAT_BALL_NOSE_FLAT_CLEARING,
        ),
        (
            "QUALITY_STEPOVER_OVER_80_PCT",
            ids::QUALITY_STEPOVER_OVER_80_PCT,
        ),
        (
            "QUALITY_FINISH_STEPOVER_OVER_50_PCT",
            ids::QUALITY_FINISH_STEPOVER_OVER_50_PCT,
        ),
        (
            "QUALITY_BALL_SCALLOP_HEIGHT",
            ids::QUALITY_BALL_SCALLOP_HEIGHT,
        ),
        (
            "EFFICIENCY_VERY_FINE_STEPOVER",
            ids::EFFICIENCY_VERY_FINE_STEPOVER,
        ),
        ("STALE_DROP_CUTTER_MIN_Z", ids::STALE_DROP_CUTTER_MIN_Z),
        ("STALE_TAPERED_BALL_PLUNGE", ids::STALE_TAPERED_BALL_PLUNGE),
        (
            "STALE_WOOD_ADAPTIVE_STEPOVER",
            ids::STALE_WOOD_ADAPTIVE_STEPOVER,
        ),
        (
            "STALE_PROJECT_CURVE_NEGATIVE_DEPTH",
            ids::STALE_PROJECT_CURVE_NEGATIVE_DEPTH,
        ),
        ("PRECOND_REST_NO_PRIOR", ids::PRECOND_REST_NO_PRIOR),
        (
            "PRECOND_REST_PREV_TOOL_MISSING",
            ids::PRECOND_REST_PREV_TOOL_MISSING,
        ),
        (
            "PRECOND_REST_PREV_TOOL_NOT_LARGER",
            ids::PRECOND_REST_PREV_TOOL_NOT_LARGER,
        ),
        ("PRECOND_DRILL_NO_HOLES", ids::PRECOND_DRILL_NO_HOLES),
        (
            "PRECOND_ALIGNMENT_PIN_DRILL_NO_HOLES",
            ids::PRECOND_ALIGNMENT_PIN_DRILL_NO_HOLES,
        ),
        (
            "PRECOND_PROJECT_CURVE_NO_CURVE",
            ids::PRECOND_PROJECT_CURVE_NO_CURVE,
        ),
        (
            "PRECOND_PROJECT_CURVE_NO_SURFACE",
            ids::PRECOND_PROJECT_CURVE_NO_SURFACE,
        ),
        ("REF_MODEL_MISSING", ids::REF_MODEL_MISSING),
    ];
    let mut missing = Vec::new();
    for (symbol, literal) in consts {
        if !merged.contains(symbol) && !merged.contains(literal) {
            missing.push(format!("{symbol} (= {literal:?})"));
        }
    }
    assert!(
        missing.is_empty(),
        "diagnostic IDs declared in `ids.rs` but never referenced in adapters: {missing:#?}"
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
            vec![ids::GEOM_DPP_OVER_1_5X_DIAMETER],
            Severity::Info,
        ),
        make_diag(
            ids::GEOM_DPP_OVER_1_5X_DIAMETER,
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
        toolpath_id: 0,
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
        drill_gates: Some(DrillGatesVerdict {
            chip_welding: DrillGateOutcome::Within {
                observed: 4.0,
                threshold: 8.0,
            },
            peck_adequacy: DrillGateOutcome::Within {
                observed: 0.5,
                threshold: 2.0,
            },
            plunge_feed: DrillGateOutcome::Within {
                observed: 100.0,
                threshold: 100.0,
            },
        }),
        modulation_summary: None,
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
        },
        bounds: ChipBounds {
            min_mm_per_tooth: Some(0.0305),
            max_mm_per_tooth: 0.0508,
            source: ChipBoundsSource::VendorLutExtrapolated,
        },
    };
    let verdict = ToolpathLoadVerdict {
        toolpath_id: 12,
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
        drill_gates: None,
        modulation_summary: None,    };
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
        toolpath_id: 4,
        chipload: ChiploadVerdict::Unmodeled {
            reason: UnmodeledReason::StaleSimulation,
        },
        power: PowerVerdict::Unmodeled {
            reason: UnmodeledReason::StaleSimulation,
        },
        deflection: DeflectionVerdict::Unmodeled {
            reason: UnmodeledReason::StaleSimulation,
        },
        drill_gates: None,
        modulation_summary: None,    };
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
        toolpath_id: 7,
        chipload: ChiploadVerdict::Unmodeled {
            reason: UnmodeledReason::SimulationRequired,
        },
        power: PowerVerdict::Unmodeled {
            reason: UnmodeledReason::SimulationRequired,
        },
        deflection: DeflectionVerdict::Unmodeled {
            reason: UnmodeledReason::SimulationRequired,
        },
        drill_gates: None,
        modulation_summary: None,    };
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
        toolpath_id: 12,
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
            assert_eq!(*toolpath_id, 12);
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
        ramp_feed_mm_min: 500.0,
        axial_depth_mm: 4.0,
        radial_width_mm: 1.0,
        power_kw: 0.3,
        available_power_kw: 0.6,
        power_limited: false,
        mrr_mm3_min: 1200.0,
        warnings: vec![],
        vendor_source: None,
        chipload_source: ChiploadSource::FormulaFallback,
        chipload_bounds: None,
        matched_lut_row: None,
        effective_diameter_mm: 6.0,
        derates: crate::feeds::FeedsDerates::default(),
    };
    let diags = adapters::from_feeds::heuristic_hints_from_recommendation(
        7, 2500.0, // commanded feed = 2.5× recommended
        None, None, &result,
    );
    assert_eq!(diags.len(), 1);
    let d = &diags[0];
    assert_eq!(d.id.as_str(), ids::FEEDS_FEED_VS_LUT_HIGH);
    assert_eq!(d.severity, Severity::Hint);
    assert_eq!(d.confidence, Confidence::Heuristic);
}
