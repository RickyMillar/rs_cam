//! Unit tests for the diagnostics module. Each adapter and the
//! reducer are tested in isolation; integration with `ProjectSession`
//! lands in PR-3 once consumers cut over.

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
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
        scope: Scope::Toolpath {
            id: ToolpathId::from_raw(0),
        },
        category: Category::ToolLoad,
        severity,
        confidence: Confidence::Verified,
        state,
        source: Source::ToolLoad,
        face: crate::diagnostics::presentation::face(id),
        anchor: crate::diagnostics::presentation::anchor(id),
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

/// CONTRACT K6: the macro writes one column entry per id, in the order
/// of `ALL`.
#[test]
fn every_id_states_its_consequence_k6() {
    assert_eq!(ids::CONSEQUENCES.len(), ids::ALL.len());
    for ((id, _), all) in ids::CONSEQUENCES.iter().zip(ids::ALL) {
        assert_eq!(id, all);
    }
    assert!(ids::row_kind(ids::LOAD_POWER_EXCEEDS).is_some());
    assert!(ids::row_kind("no.such.id").is_none());
}

/// CONTRACT K6 (ruling Q6): no row refuses. Each id of today's refusals
/// warns, and the missing inputs are `NotReady`, not findings (Q10).
#[test]
fn every_former_refusal_warns_k6() {
    use crate::diagnostics::{Consequence, Danger, RowKind};
    for (id, danger) in [
        (ids::GEOM_CLEARANCE_Z_BELOW_RETRACT_Z, Danger::Machine),
        (ids::GEOM_RETRACT_Z_BELOW_TOP_Z, Danger::Machine),
        (ids::GEOM_BOTTOM_ABOVE_TOP_Z, Danger::Machine),
        (ids::SAFETY_CUTS_INTO_MODEL, Danger::Part),
        (ids::SAFETY_NON_CENTRE_CUTTING_PLUNGE, Danger::Tool),
        (ids::SAFETY_NON_CENTRE_CUTTING_DESCENT, Danger::Tool),
        (ids::LOAD_CHIPLOAD_HIGH, Danger::Tool),
        (ids::LOAD_POWER_EXCEEDS, Danger::Machine),
        (ids::LOAD_CHIPLOAD_UNMODELED, Danger::Tool),
        (ids::DRILL_CHIP_WELDING, Danger::Tool),
        (ids::EXPORT_OP_LEFT_OUT, Danger::Part),
        (ids::PROJECT_HOLDER_COLLISION, Danger::Machine),
        (ids::PRECOND_BURIED_FACE, Danger::Part),
        (ids::COMPAT_TOOL_KIND_REFUSED, Danger::Part),
    ] {
        assert_eq!(
            ids::row_kind(id),
            Some(RowKind::Finding(Consequence::Warn(danger))),
            "{id}"
        );
    }
    for id in [
        ids::PRECOND_NOT_SET,
        ids::PRECOND_NO_TOOL,
        ids::REF_MODEL_MISSING,
    ] {
        assert_eq!(ids::row_kind(id), Some(RowKind::NotReady), "{id}");
    }
    // Non-vacuity: the column holds each kind of row.
    assert!(
        ids::CONSEQUENCES
            .iter()
            .any(|(_, row)| *row == RowKind::Finding(Consequence::Info))
    );
}

/// K6: the emit error ids (`gcode::EmitError::id`) are not `diag_ids!`
/// rows. An emit error is not a finding: the program text would be wrong.
#[test]
fn emit_error_ids_are_not_finding_ids_k6() {
    let emit: Vec<&str> = crate::gcode::EmitError::ALL_IDS.to_vec();
    assert_eq!(
        emit.len(),
        3,
        "inch units, bad feed, controller compensation"
    );
    for id in &emit {
        assert!(id.starts_with("emit."), "{id}");
        assert!(!ids::ALL.contains(id), "{id} is a finding id too");
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
    // once outside `ids.rs`, `presentation.rs` and the `tests.rs` files. A reachable ID need
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
                // W1: the presentation table names every id; it emits none.
                && p.file_name().and_then(|n| n.to_str()) != Some("presentation.rs")
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
    assert!(Severity::Severe.rank() > Severity::Critical.rank());
    assert!(Severity::Critical.rank() > Severity::Caution.rank());
    assert!(Severity::Caution.rank() > Severity::Hint.rank());
    assert!(Severity::Hint.rank() > Severity::Info.rank());
}

#[test]
fn severity_is_actionable() {
    assert!(Severity::Severe.is_actionable());
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
        toolpath_id: ToolpathId::from_raw(0),
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
        cut_force: None,
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
        toolpath_id: ToolpathId::from_raw(12),
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
        cut_force: None,
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
        toolpath_id: ToolpathId::from_raw(4),
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
        cut_force: None,
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
        toolpath_id: ToolpathId::from_raw(7),
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
        cut_force: None,
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
        toolpath_id: ToolpathId::from_raw(12),
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
            assert_eq!(*toolpath_id, ToolpathId::from_raw(12));
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
        plunge: crate::feeds::PlungeBasis::DrillCycle {
            vertical: crate::feeds::vertical::VerticalRateLimit::new(
                &crate::feeds::RampBasis::NoChip {
                    reason: crate::feeds::RampFallback::NoLut,
                },
                &crate::machine::MachineProfile::default(),
            ),
        },
        ramp: crate::feeds::RampBasis::NoChip {
            reason: crate::feeds::RampFallback::NoLut,
        },
        axial_depth_mm: 4.0,
        radial_width_mm: 1.0,
        power_kw: Some(0.3),
        power_case: crate::feeds::engagement::PowerCase::Nominal,
        available_power_kw: 0.6,
        power_limited: false,
        mrr_mm3_min: Ok(1200.0),
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
        bases: crate::feeds::PointBases::not_computed(),
        steps: Vec::new(),
        rpm_choice: None,
    };
    let diags = adapters::from_feeds::heuristic_hints_from_recommendation(
        ToolpathId::from_raw(7),
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
        let tool = ToolConfig::test_default(ToolId::from_raw(0), tool_type);
        adapters::from_static_checks::diagnostics_from_static_checks(
            ToolpathId::from_raw(3),
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

// ── W1: the anchor and the face of each rule ────────────────────────

/// W1 (toolpath inspector programme): every id of `ids::ALL` has one row
/// of `presentation::TABLE`, every face is short plain text, and every
/// field anchor names a real field.
#[test]
fn every_id_has_an_anchor_and_a_short_face_w1() {
    use crate::diagnostics::presentation::{self, Anchor, FACE_MAX_CHARS, FieldKey, TABLE};
    use std::collections::HashSet;

    let mut rows = HashSet::new();
    for (id, _, _) in TABLE {
        assert!(rows.insert(*id), "two TABLE rows for {id}");
        assert!(ids::ALL.contains(id), "TABLE row {id} is not in ids::ALL");
    }
    for id in ids::ALL {
        assert!(rows.contains(id), "{id} has no presentation::TABLE row");
        let face = presentation::face(id);
        assert!(!face.is_empty(), "{id}: empty face");
        assert!(
            face.chars().count() <= FACE_MAX_CHARS,
            "{id}: face {face:?} is over {FACE_MAX_CHARS} characters"
        );
        assert!(!face.contains('\n'), "{id}: face {face:?} has two lines");
        assert!(
            face.chars().next().is_some_and(char::is_uppercase),
            "{id}: face {face:?} does not start with a capital"
        );
        assert!(
            !face.ends_with('.'),
            "{id}: face {face:?} is a label, not a sentence"
        );
        if let Anchor::Field { key } = presentation::anchor(id) {
            assert_eq!(
                FieldKey::parse(key.as_str()),
                Some(key),
                "{id}: field anchor `{}` names no op parameter, height or dressup",
                key.as_str()
            );
        }
    }
}

/// W4 (message rewrite): a face says what is wrong or what to do, not only
/// the topic. Each face of `presentation::TABLE` fits the panel, is not its
/// id, has at least three words, and does not end in a bare topic word
/// ("Selection note", "Tab placement", "Rest reference" fail). A face that
/// ends in "height" passes only as a relation ("Feed height below top
/// height"), because the three-word rule stops "Scallop height".
#[test]
fn every_face_says_what_is_wrong_w4() {
    use crate::diagnostics::presentation::{FACE_MAX_CHARS, TABLE};

    const TOPIC_WORDS: &[&str] = &[
        "note",
        "notes",
        "placement",
        "reference",
        "derived",
        "projection",
        "info",
        "issue",
        "warning",
    ];
    let mut bad = Vec::new();
    for (id, _, face) in TABLE {
        let words: Vec<&str> = face.split_whitespace().collect();
        let last = words
            .last()
            .map(|w| {
                w.trim_matches(|c: char| !c.is_alphanumeric())
                    .to_lowercase()
            })
            .unwrap_or_default();
        if face.chars().count() > FACE_MAX_CHARS {
            bad.push(format!(
                "{id}: {face:?} is over {FACE_MAX_CHARS} characters"
            ));
        }
        if face == id || face.eq_ignore_ascii_case(&id.replace(['.', '_'], " ")) {
            bad.push(format!("{id}: {face:?} repeats the id"));
        }
        if words.len() < 3 {
            bad.push(format!(
                "{id}: {face:?} names a topic only (fewer than 3 words)"
            ));
        }
        if TOPIC_WORDS.contains(&last.as_str()) {
            bad.push(format!("{id}: {face:?} ends in the topic word {last:?}"));
        }
    }
    assert!(
        bad.is_empty(),
        "faces that do not say what is wrong:\n{}",
        bad.join("\n")
    );
}

/// The developer phrases that a diagnostic detail (`message`) must not
/// carry. Agent evidence lives in `evidence` or in the bracketed
/// provenance tail, in plain words.
const DETAIL_DENY_LIST: &[&str] = &[
    "repo rule",
    "unsourced",
    "snaps to a step",
    "AT OR BELOW",
    "does not take the dial",
    "pre-sim",
];

/// The production half of a source file: the text before its test module,
/// with the comment lines dropped. What stays in a line is code and string
/// literals, so a backtick in it is in a string literal.
fn production_lines(source: &str) -> Vec<(usize, &str)> {
    let production = source.split("#[cfg(test)]").next().unwrap_or(source);
    production
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim_start().starts_with("//"))
        .map(|(index, line)| (index + 1, line))
        .collect()
}

/// W4 (message rewrite): every adapter detail is free of developer
/// phrases and of backticked identifiers ("`feed_rate`" reads "feed
/// rate"). The adapters and the generation findings they convert build
/// the text, so the check reads their source. A runtime arm covers the
/// text that a source layer writes (the rubbing-floor source, the
/// long-tool share).
#[test]
fn no_adapter_detail_carries_developer_text_w4() {
    let sources: &[(&str, &str)] = &[
        ("from_feeds.rs", include_str!("adapters/from_feeds.rs")),
        (
            "from_generation.rs",
            include_str!("adapters/from_generation.rs"),
        ),
        (
            "from_model_refs.rs",
            include_str!("adapters/from_model_refs.rs"),
        ),
        (
            "from_preconditions.rs",
            include_str!("adapters/from_preconditions.rs"),
        ),
        (
            "from_project_diagnostics.rs",
            include_str!("adapters/from_project_diagnostics.rs"),
        ),
        (
            "from_stale_default.rs",
            include_str!("adapters/from_stale_default.rs"),
        ),
        (
            "from_static_checks.rs",
            include_str!("adapters/from_static_checks.rs"),
        ),
        (
            "from_tool_load.rs",
            include_str!("adapters/from_tool_load.rs"),
        ),
        (
            "toolpath_stats.rs",
            include_str!("../compute/toolpath_stats.rs"),
        ),
        ("validate.rs", include_str!("../compute/validate.rs")),
    ];
    let mut bad = Vec::new();
    for (name, source) in sources {
        for (line_no, line) in production_lines(source) {
            if line.contains('`') {
                bad.push(format!(
                    "{name}:{line_no}: backticked identifier: {}",
                    line.trim()
                ));
            }
            for phrase in DETAIL_DENY_LIST {
                if line.contains(phrase) {
                    bad.push(format!("{name}:{line_no}: {phrase:?}: {}", line.trim()));
                }
            }
        }
    }

    // The runtime arm: the text that `feeds` writes into a detail.
    use crate::feeds::{FeedsWarning, RubbingFloorSource};
    let mut warnings = vec![FeedsWarning::LongToolDerate {
        stickout_mm: 37.0,
        diameter_mm: 6.0,
        ratio: 37.0 / 6.0,
        factor: 0.75,
    }];
    for source in [
        RubbingFloorSource::RepoConstant,
        RubbingFloorSource::BandMinimum,
        RubbingFloorSource::BandMaximum,
        RubbingFloorSource::PrintedPoint,
    ] {
        warnings.push(FeedsWarning::ChiploadBelowRubbingFloor {
            commanded: 0.01,
            floor: 0.025,
            source,
            band_min: None,
            band_max: None,
        });
    }
    for w in &warnings {
        let mut result = fixture_feeds_result();
        result.warnings = vec![w.clone()];
        for d in
            adapters::from_feeds::diagnostics_from_feeds_result(ToolpathId::from_raw(1), &result)
        {
            for phrase in DETAIL_DENY_LIST {
                if d.message.contains(phrase) || d.face.contains(phrase) {
                    bad.push(format!("{}: {phrase:?}: {}", d.id.as_str(), d.message));
                }
            }
            if d.message.contains('`') {
                bad.push(format!("{}: backtick: {}", d.id.as_str(), d.message));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "developer text in a detail:\n{}",
        bad.join("\n")
    );
}

/// W4: the long-tool share reads as a percent on the face and in the
/// detail, with the caveat that the limit has no published source.
#[test]
fn the_long_tool_share_reads_as_a_percent_w4() {
    use crate::feeds::FeedsWarning;
    let mut result = fixture_feeds_result();
    result.warnings = vec![FeedsWarning::LongToolDerate {
        stickout_mm: 37.0,
        diameter_mm: 6.0,
        ratio: 37.0 / 6.0,
        factor: 0.75,
    }];
    let found =
        adapters::from_feeds::diagnostics_from_feeds_result(ToolpathId::from_raw(1), &result);
    let d = found.first().expect("one finding");
    assert_eq!(d.face, "Long tool: load target 75 %");
    assert!(d.message.contains("37.0 mm"), "{}", d.message);
    assert!(d.message.contains("6.2 \u{00D7}"), "{}", d.message);
    assert!(d.message.contains("75 %"), "{}", d.message);
    assert!(d.message.contains("no published source"), "{}", d.message);
    assert!(
        d.message.contains("The feed rate does not change"),
        "{}",
        d.message
    );
}

/// A plain recommendation with no warnings, for the feeds adapter tests.
fn fixture_feeds_result() -> crate::feeds::FeedsResult {
    use crate::feeds::{ChiploadSource, FeedsResult};
    FeedsResult {
        rpm: 18_000.0,
        chip_load_mm: 0.02,
        feed_rate_mm_min: 1000.0,
        plunge_rate_mm_min: 400.0,
        plunge: crate::feeds::PlungeBasis::DrillCycle {
            vertical: crate::feeds::vertical::VerticalRateLimit::new(
                &crate::feeds::RampBasis::NoChip {
                    reason: crate::feeds::RampFallback::NoLut,
                },
                &crate::machine::MachineProfile::default(),
            ),
        },
        ramp: crate::feeds::RampBasis::NoChip {
            reason: crate::feeds::RampFallback::NoLut,
        },
        axial_depth_mm: 4.0,
        radial_width_mm: 1.0,
        power_kw: Some(0.3),
        power_case: crate::feeds::engagement::PowerCase::Nominal,
        available_power_kw: 0.6,
        power_limited: false,
        mrr_mm3_min: Ok(1200.0),
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
        bases: crate::feeds::PointBases::not_computed(),
        steps: Vec::new(),
        rpm_choice: None,
    }
}

/// W1: a field anchor names a field of every op the rule can fire on.
/// The static and feeds rules fire on an op only when the op carries the
/// value (`OperationConfig::stepover` and its siblings are `Some`). Where
/// the op carries a value but publishes no row for it (a Drill plunge rate,
/// a Waterline depth per pass), `fit_anchor` gives the op, never a field
/// the panel cannot draw. Each table key is a field of at least one op.
#[test]
fn a_field_anchor_names_a_param_of_each_op_it_fires_on_w1() {
    use crate::compute::catalog::{OperationConfig, OperationType};
    use crate::compute::fixture_values::fixture_op;
    use crate::diagnostics::presentation::{self, Anchor};

    type Has = fn(&OperationConfig) -> bool;
    let carriers: &[(&str, Has)] = &[
        ("feed_rate", |op| op.feed_rate().is_some()),
        ("plunge_rate", |op| op.plunge_rate().is_some()),
        ("spindle_rpm", |op| op.spindle_rpm().is_some()),
        ("stepover", |op| op.stepover().is_some()),
        ("depth_per_pass", |op| op.depth_per_pass().is_some()),
        ("scallop_height", |op| op.scallop_height().is_some()),
    ];
    let mut bad = Vec::new();
    let mut fitted = 0;
    for op_type in OperationType::ALL {
        let op = fixture_op(*op_type);
        for (key, has) in carriers {
            if !has(&op) {
                continue;
            }
            match presentation::fit_anchor(Anchor::field(key), *op_type) {
                Anchor::Field { key: kept }
                    if !presentation::op_has_field(*op_type, kept.as_str()) =>
                {
                    bad.push(format!(
                        "{op_type:?}: `{key}` kept but not a field of the op"
                    ));
                }
                Anchor::Field { .. } => {}
                Anchor::Tool
                | Anchor::Operation
                | Anchor::Note
                | Anchor::Project
                | Anchor::View => fitted += 1,
            }
        }
        for (id, anchor, _) in presentation::TABLE {
            if let Anchor::Field { key } = presentation::fit_anchor(*anchor, *op_type)
                && !presentation::op_has_field(*op_type, key.as_str())
            {
                bad.push(format!(
                    "{op_type:?} {id}: `{}` is no field of the op",
                    key.as_str()
                ));
            }
        }
    }
    // Non-vacuity: some op carries a value it does not publish (2026-10-30:
    // the Drill and Pin Drill plunge rate, the Waterline and Ramp Finish
    // depth per pass), and the fit gives the op for it.
    assert!(
        fitted > 0,
        "no carried-but-unpublished value: the fit is untested"
    );
    for (id, anchor, _) in presentation::TABLE {
        if let Anchor::Field { key } = anchor
            && !OperationType::ALL
                .iter()
                .any(|op| presentation::op_has_field(*op, key.as_str()))
        {
            bad.push(format!("{id}: `{}` is a field of no op", key.as_str()));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("; "));
}

/// W1: the adapters fill the face and the anchor. The table row is the
/// default; the sites that build their own face (the value that is not set,
/// the tool kind) keep it short and name the field.
#[test]
fn the_adapters_fill_the_face_and_the_anchor_w1() {
    use crate::compute::catalog::{OperationConfig, OperationType};
    use crate::compute::fixture_values::fixture_op;
    use crate::compute::tool_config::{ToolConfig, ToolId, ToolType};
    use crate::diagnostics::adapters::from_preconditions::{
        PreconditionContext, diagnostics_from_preconditions,
    };
    use crate::diagnostics::presentation::{self, Anchor, FACE_MAX_CHARS};

    let tp = ToolpathId::from_raw(1);
    // Every op on every tool kind: each static finding has a short face, and
    // its anchor is the table row (no static site overrides the anchor).
    for op_type in OperationType::ALL {
        let op = fixture_op(*op_type);
        for tool_type in [
            ToolType::EndMill,
            ToolType::BallNose,
            ToolType::BullNose,
            ToolType::VBit,
            ToolType::TaperedBallNose,
        ] {
            let tool = ToolConfig::test_default(ToolId::from_raw(0), tool_type);
            for d in
                adapters::from_static_checks::diagnostics_from_static_checks(tp, &op, &tool, None)
            {
                assert!(
                    !d.face.is_empty() && d.face.chars().count() <= FACE_MAX_CHARS,
                    "{op_type:?}/{tool_type:?} {}: face {:?}",
                    d.id.as_str(),
                    d.face
                );
                // The static sites write the table anchor; the orchestrator
                // fits it to the op.
                assert_eq!(d.anchor, presentation::anchor(d.id.as_str()));
                if let Anchor::Field { key } = presentation::fit_anchor(d.anchor, *op_type) {
                    assert!(presentation::op_has_field(*op_type, key.as_str()));
                }
            }
        }
    }

    // A Scallop Finish on an end mill: the face names the tool it needs.
    let scallop = fixture_op(OperationType::Scallop);
    let flat = ToolConfig::test_default(ToolId::from_raw(0), ToolType::EndMill);
    let refused =
        adapters::from_static_checks::diagnostics_from_static_checks(tp, &scallop, &flat, None)
            .into_iter()
            .find(|d| d.id.as_str() == ids::COMPAT_TOOL_KIND_REFUSED)
            .expect("Scallop refuses an end mill");
    assert_eq!(refused.anchor, Anchor::Tool);
    assert!(
        refused.face.starts_with("Needs a "),
        "face {:?}",
        refused.face
    );

    // A Pocket with no feeds and no tool: "Feed and spindle not set" on the
    // feed field, and "No tool" on the tool.
    let mut pocket = fixture_op(OperationType::Pocket);
    if let OperationConfig::Pocket(cfg) = &mut pocket {
        cfg.feed_rate = None;
        cfg.plunge_rate = None;
        cfg.spindle_rpm = None;
    }
    let ctx = PreconditionContext {
        no_tool_state: Some("No tool fits: needs a flat end mill".to_owned()),
        ..Default::default()
    };
    let found = diagnostics_from_preconditions(tp, &pocket, &ctx);
    let not_set = found
        .iter()
        .find(|d| d.id.as_str() == ids::PRECOND_NOT_SET)
        .expect("unset feeds give precondition.not_set");
    assert_eq!(not_set.face, "Feed and spindle not set");
    assert_eq!(not_set.anchor, Anchor::field("feed_rate"));
    let no_tool = found
        .iter()
        .find(|d| d.id.as_str() == ids::PRECOND_NO_TOOL)
        .expect("no tool gives precondition.no_tool");
    assert_eq!(no_tool.face, "No tool chosen");
    assert_eq!(no_tool.anchor, Anchor::Tool);

    // Each op with its values unset: the not-set anchor names a field the op
    // publishes (or the op, when no field holds it).
    for op_type in OperationType::ALL {
        let op = OperationConfig::new_default(*op_type);
        for d in diagnostics_from_preconditions(tp, &op, &PreconditionContext::default()) {
            if d.id.as_str() != ids::PRECOND_NOT_SET {
                continue;
            }
            assert!(d.face.chars().count() <= FACE_MAX_CHARS);
            if let Anchor::Field { key } = d.anchor {
                assert!(
                    crate::diagnostics::presentation::op_has_field(*op_type, key.as_str()),
                    "{op_type:?}: not-set anchor `{}` is not a field of the op",
                    key.as_str()
                );
            }
        }
    }

    // W3 fix: an unset floor anchors to the Bottom height row (`bottom_z`),
    // THE floor editor of an `OpFloor` op (D6), not to the op. Red proof:
    // the anchor was the unknown key "floor", so it fell back to the op
    // and the GUI painted it twice (issue row + Floor row).
    let pocket = OperationConfig::new_default(OperationType::Pocket);
    assert!(
        pocket.floor().is_some_and(|f| f.is_unset()),
        "fixture: a new Pocket has no floor"
    );
    let found = diagnostics_from_preconditions(tp, &pocket, &PreconditionContext::default());
    let floor = found
        .iter()
        .find(|d| d.face == "Floor not set")
        .expect("an unset floor gives Floor not set");
    assert_eq!(floor.anchor, Anchor::field("bottom_z"));
}

/// W1: the JSON of a diagnostic carries `face` and `anchor`, and a field
/// anchor reads back only when it names a real field.
#[test]
fn the_json_carries_the_face_and_the_anchor_w1() {
    use crate::diagnostics::presentation::Anchor;
    let mut d = make_diag(
        ids::FEEDS_FEED_VS_LUT_HIGH,
        DiagnosticState::Current,
        vec![],
        Severity::Hint,
    );
    d.anchor = Anchor::field("feed_rate");
    let json = serde_json::to_value(&d).expect("serialise");
    assert_eq!(json["face"], "Feed far above the recommended feed");
    assert_eq!(
        json["anchor"],
        serde_json::json!({"kind": "field", "key": "feed_rate"})
    );
    let back: Diagnostic = serde_json::from_value(json.clone()).expect("read back");
    assert_eq!(back.anchor, d.anchor);
    let mut bad = json;
    bad["anchor"]["key"] = serde_json::json!("no_such_field");
    assert!(serde_json::from_value::<Diagnostic>(bad).is_err());
    let tool = serde_json::to_value(Anchor::Tool).expect("serialise");
    assert_eq!(tool, serde_json::json!({"kind": "tool"}));
}
