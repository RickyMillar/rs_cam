//! Feeds contract P29-2a
//! (`planning/feeds_contract_2026-11-03/phases/P29-2_DESIGN.md`): one name
//! for each limit and one record for each check.
//!
//! - Claim 1: every variant of the five old limit lists
//!   (`LimitKind`, `CriterionKind`, `BindingConstraint`, `RpmLimit`,
//!   `AxialBindingConstraint`) maps to a `LimitId`, and every `LimitId` has
//!   at least one old source. (The mapping functions match with no
//!   wildcard, so a new old variant does not compile without its member.)
//! - Claim 2: the `Check` of a gate row equals the row on the value, the
//!   bound, the state and the population. A chip row that exceeds on the
//!   low side reads the FLOOR of the band as its bound.
//! - Claim 3: the checks of a planned point on an envelope hold the margin,
//!   the state and the binding limit of `SafeEnvelope::placement_of`.
//! - Claim 4: the checks of an RPM choice hold its limits and its binding.
//! - Claim 5: the basis join keeps `setting` and `gates_export` of each of
//!   the 6 bound sources.
//! - Claim 6: no code of `src/limits/` names an operation type, an
//!   operation family, a pass role, `compute::` or `session::`.
//! - Claim 7 (P29-3) is deleted (P29-4, ruling Q4): `DepthRule` and
//!   `Limit::depth_rule` are gone; `Rigidity` and `DepthOfCut` name the
//!   force budget only.
//!
//! Inject check (2026-11-11): with `of_criterion` giving the ceiling as the
//! bound of a low-side chip row, claim 2 goes red (bound 0.12 against
//! 0.05); with `of_envelope` setting `binding` on the limits that do not
//! bind, claim 3 goes red.

#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use std::collections::BTreeSet;

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::feeds::cutter_constraints::AxialBindingConstraint;
use rs_cam_core::feeds::envelope::{CutPoint, EnvelopeInput, LimitKind, Placement, SafeEnvelope};
use rs_cam_core::feeds::suggest::{NoProject, SuggestContext};
use rs_cam_core::feeds::{RpmChoice, RpmLimit, RpmLimitValue, SpindleStrategy};
use rs_cam_core::limits::{self, BoundSide, Check, CheckState, Geometry, LimitId, NoNumberReason};
use rs_cam_core::machine::{MachineProfile, RigidityDepthCap};
use rs_cam_core::material::Material;
use rs_cam_core::tool_load::verdict::{
    BindingConstraint, BoundSource, ChipBounds, ChipBoundsSource, ChipSide, ChiploadMetric,
    ChiploadStatistic, ChiploadVerdict, Confidence, CriterionKind, DeflectionBasis,
    DeflectionBounds, DeflectionVerdict, DepthVerdict, GatePopulation, LoadState, PopulationUnit,
    PowerVerdict, SampleEvidence, UnmodeledReason,
};

#[test]
fn every_old_limit_name_maps_and_every_limit_id_has_a_source() {
    let mut seen = BTreeSet::new();
    for kind in LimitKind::ALL {
        seen.insert(LimitId::of_limit_kind(kind));
    }
    for side in [ChipSide::High, ChipSide::Low] {
        for kind in CriterionKind::ALL {
            seen.insert(LimitId::of_criterion(kind, side));
        }
    }
    for binding in BindingConstraint::ALL {
        seen.insert(LimitId::of_binding(binding));
    }
    for limit in RpmLimit::ALL {
        seen.insert(LimitId::of_rpm_limit(limit));
    }
    // The RPM list holds each wire id (7 variants, 3 ceiling sources).
    let wire: BTreeSet<_> = RpmLimit::ALL.iter().map(|l| l.wire_id()).collect();
    assert_eq!(wire.len(), 7, "RpmLimit::ALL names every variant");
    let mut states = 0;
    for binding in AxialBindingConstraint::ALL {
        match LimitId::of_axial_binding(binding) {
            Some(id) => {
                seen.insert(id);
            }
            None => states += 1,
        }
    }
    assert_eq!(
        states, 2,
        "Unbounded and SafeBandEmpty are states, not limits"
    );
    let missing: Vec<_> = LimitId::ALL
        .iter()
        .filter(|id| !seen.contains(id))
        .collect();
    assert!(
        missing.is_empty(),
        "LimitId members with no old source: {missing:?}"
    );
    assert_eq!(LimitId::ALL.len(), 28);
}

fn population() -> GatePopulation {
    GatePopulation::new(40, 100, PopulationUnit::Samples)
}

fn evidence() -> SampleEvidence {
    SampleEvidence::empty().with_population(population())
}

fn metric(
    observed: f64,
    min: Option<f64>,
    max: f64,
    statistic: ChiploadStatistic,
) -> ChiploadMetric {
    ChiploadMetric {
        observed_mm_per_tooth: observed,
        statistic,
        evidence: evidence(),
        bounds: ChipBounds {
            min_mm_per_tooth: min,
            max_mm_per_tooth: max,
            source: ChipBoundsSource::VendorLut,
        },
    }
}

/// The gate row against its check: the value, the population, and the
/// expected limit, bound and state.
fn row_agrees(
    status: &rs_cam_core::tool_load::verdict::CriterionStatus<'_>,
    limit: LimitId,
    bound: Option<f64>,
    state: CheckState,
) -> Check {
    let check = limits::of_criterion(status);
    assert_eq!(check.limit, limit, "{:?}", status.kind);
    assert_eq!(check.value, status.display_peak, "{:?} value", status.kind);
    assert_eq!(check.bound, bound, "{:?} bound", status.kind);
    assert_eq!(check.state, state, "{:?} state", status.kind);
    assert_eq!(
        check.population, status.population,
        "{:?} population",
        status.kind
    );
    assert_eq!(check.unit, status.unit);
    assert!(!check.binding, "a gate row is never binding");
    // The basis join keeps the two policies of the old source.
    match (&check.basis, &status.bound_source) {
        (Some(basis), Some(source)) => {
            assert_eq!(basis.setting(), source.setting());
            assert_eq!(basis.gates_export(), source.gates_export());
        }
        (None, None) => {}
        (basis, source) => panic!("basis {basis:?} against source {source:?}"),
    }
    // The old state and the new state agree.
    match (status.state, check.state) {
        (LoadState::Exceeds, CheckState::Exceeds)
        | (LoadState::Within, CheckState::Within)
        | (LoadState::Within, CheckState::NoNumber(NoNumberReason::NoBound))
        | (LoadState::Unmodeled, CheckState::NoNumber(_) | CheckState::NotApplicable(_)) => {}
        (old, new) => panic!("old state {old:?} against {new:?}"),
    }
    check
}

#[test]
fn a_gate_row_and_its_check_agree() {
    // Power, within and over.
    let power = |peak_kw: f64| {
        let source = Some(BoundSource::MachinePowerCurve { rpm: 18_000.0 });
        if peak_kw <= 1.5 {
            PowerVerdict::Within {
                peak_kw,
                available_kw: 1.5,
                evidence: evidence(),
                confidence: Confidence::Validated,
                bound_source: source,
            }
        } else {
            PowerVerdict::Exceeds {
                peak_kw,
                available_kw: 1.5,
                evidence: evidence(),
                confidence: Confidence::Validated,
                bound_source: source,
            }
        }
    };
    let within = power(0.6);
    let check = row_agrees(
        &within.as_criterion_status(),
        LimitId::SpindlePower,
        Some(1.5),
        CheckState::Within,
    );
    assert_eq!(check.side, BoundSide::Ceiling);
    assert_eq!(check.margin, Some((1.5 - 0.6) / 1.5));
    let over = power(2.0);
    let check = row_agrees(
        &over.as_criterion_status(),
        LimitId::SpindlePower,
        Some(1.5),
        CheckState::Exceeds,
    );
    assert!(check.margin.is_some_and(|m| m < 0.0));

    // Deflection on a rough pass with a leave.
    let deflection = DeflectionVerdict::Within {
        peak_mm: 0.02,
        bounds: DeflectionBounds {
            validated_within_mm: 0.05,
            exceeds_mm: 0.3,
            basis: DeflectionBasis::StockToLeave { mm: 0.3 },
        },
        evidence: evidence(),
        confidence: Confidence::Validated,
    };
    let status = deflection.as_criterion_status();
    row_agrees(
        &status,
        LimitId::ToolDeflection,
        status.bound,
        CheckState::Within,
    );

    // Chip: within, over the maximum, under the minimum.
    let chip_within = ChiploadVerdict::Within {
        approach_to_min: None,
        approach_to_max: metric(0.10, Some(0.05), 0.12, ChiploadStatistic::PeakHigh),
        confidence: Confidence::Validated,
        burn_advisory: None,
    };
    row_agrees(
        &chip_within.as_criterion_status(),
        LimitId::ChipMax,
        Some(0.12),
        CheckState::Within,
    );
    let chip_high = ChiploadVerdict::Exceeds {
        side: ChipSide::High,
        triggering: metric(0.20, Some(0.05), 0.12, ChiploadStatistic::PeakHigh),
        confidence: Confidence::Validated,
    };
    row_agrees(
        &chip_high.as_criterion_status(),
        LimitId::ChipMax,
        Some(0.12),
        CheckState::Exceeds,
    );
    let chip_low = ChiploadVerdict::Exceeds {
        side: ChipSide::Low,
        triggering: metric(0.02, Some(0.05), 0.12, ChiploadStatistic::MedianLow),
        confidence: Confidence::Validated,
    };
    let status = chip_low.as_criterion_status();
    assert_eq!(status.bound, Some(0.12), "the old row holds the ceiling");
    let check = row_agrees(&status, LimitId::ChipMin, Some(0.05), CheckState::Exceeds);
    assert_eq!(check.side, BoundSide::Floor);
    assert!(check.margin.is_some_and(|m| m < 0.0));

    // Depth: the force budget, and a reading with no bound.
    let budget = DepthVerdict::Exceeds {
        peak_mm: 6.0,
        bound: RigidityDepthCap::ForceBudget {
            budget_n: 35.3,
            ae_mm: 1.2,
            fz_mm: 0.15,
            depth_mm: 3.9,
            below_floor: false,
        },
        evidence: evidence(),
        confidence: Confidence::Validated,
    };
    let status = budget.as_criterion_status();
    let check = row_agrees(
        &status,
        LimitId::ForceBudget,
        status.bound,
        CheckState::Exceeds,
    );
    assert!(check.basis.is_some_and(|b| !b.gates_export()));
    let reported = DepthVerdict::Reported {
        peak_mm: 2.0,
        evidence: evidence(),
        confidence: Confidence::Validated,
    };
    let status = reported.as_criterion_status();
    assert_eq!(status.state, LoadState::Within, "the old row reads Within");
    let check = row_agrees(
        &status,
        LimitId::ForceBudget,
        None,
        CheckState::NoNumber(NoNumberReason::NoBound),
    );
    assert_eq!(check.value, Some(2.0));
}

#[test]
fn every_gate_refusal_has_a_state() {
    let cases: [(UnmodeledReason, CheckState); 10] = [
        (
            UnmodeledReason::SimulationRequired,
            CheckState::NoNumber(NoNumberReason::NoSimulation),
        ),
        (
            UnmodeledReason::StaleSimulation,
            CheckState::NoNumber(NoNumberReason::StaleSimulation),
        ),
        (
            UnmodeledReason::ArcEngagementNotCaptured,
            CheckState::NoNumber(NoNumberReason::NoArc),
        ),
        (
            UnmodeledReason::NoVendorData,
            CheckState::NoNumber(NoNumberReason::NoVendorRow),
        ),
        (
            UnmodeledReason::MaterialUnvalidated,
            CheckState::NoNumber(NoNumberReason::NoForceLine),
        ),
        (
            UnmodeledReason::CutterModeUnsupported("a".to_owned()),
            CheckState::NoNumber(NoNumberReason::NotModelled),
        ),
        (
            UnmodeledReason::NotImplemented("a".to_owned()),
            CheckState::NoNumber(NoNumberReason::NotModelled),
        ),
        (
            UnmodeledReason::NotApplicableForOp("a".to_owned()),
            CheckState::NotApplicable(Geometry::NoRadialEngagement),
        ),
        (
            UnmodeledReason::AllSamplesAirCutOrRapid,
            CheckState::NoNumber(NoNumberReason::NoMaterialRemoved),
        ),
        // A second detail string gives the same state.
        (
            UnmodeledReason::NotImplemented("b".to_owned()),
            CheckState::NoNumber(NoNumberReason::NotModelled),
        ),
    ];
    for (reason, want) in cases {
        let verdict = PowerVerdict::Unmodeled {
            reason: reason.clone(),
        };
        let check = limits::of_criterion(&verdict.as_criterion_status());
        assert_eq!(check.state, want, "{reason:?}");
        assert_eq!(check.value, None);
        assert_eq!(check.margin, None);
    }
}

fn pocket_envelope() -> SafeEnvelope {
    envelope_of(OperationType::Pocket)
}

fn envelope_of(op_type: OperationType) -> SafeEnvelope {
    let op = rs_cam_core::compute::fixture_values::fixture_op(op_type);
    let tool = common::tools::endmill_tool_config(6.0);
    let machine = MachineProfile::default();
    let material = Material::default();
    let bbox = rs_cam_core::geo::BoundingBox3 {
        min: rs_cam_core::geo::P3::new(0.0, 0.0, -12.0),
        max: rs_cam_core::geo::P3::new(40.0, 40.0, 0.0),
    };
    SafeEnvelope::for_cut(EnvelopeInput {
        operation: &op,
        tool: &tool,
        machine: &machine,
        material: &material,
        lut: rs_cam_core::feeds::embedded_vendor_lut(),
        spindle_strategy: SpindleStrategy::default(),
        context: SuggestContext::no_project(NoProject {
            model_bbox: Some(&bbox),
            ..NoProject::NONE
        }),
        modulation: rs_cam_core::dressup::feed_modulation::RunModulation::Off,
    })
    .expect("a 6 mm end mill pocket is backed")
}

#[test]
fn the_checks_of_a_planned_point_hold_the_placement() {
    let env = pocket_envelope();
    let stated = env.stated.expect("the fixture pocket states its cut");
    let points = [
        stated,
        // Deep: over the depth ceilings.
        CutPoint {
            ap_mm: 60.0,
            ae_mm: stated.ae_mm,
        },
        // Wide: over the width ceilings.
        CutPoint {
            ap_mm: stated.ap_mm,
            ae_mm: 40.0,
        },
        // Light: inside.
        CutPoint {
            ap_mm: 0.3,
            ae_mm: 1.0,
        },
    ];
    let (mut inside, mut outside) = (0, 0);
    for p in points {
        let placement = env.placement_of(p);
        let checks = limits::of_envelope(&env, p);
        assert!(
            checks.len() >= env.boundary.len(),
            "one check for each limit of the boundary"
        );
        let binding: Vec<&Check> = checks.iter().filter(|c| c.binding).collect();
        let exceeds = checks
            .iter()
            .filter(|c| c.state == CheckState::Exceeds)
            .count();
        match placement {
            Placement::Outside {
                binding: kind,
                margin,
            } => {
                outside += 1;
                assert_eq!(binding.len(), 1, "one binding check at {p:?}");
                let check = binding.first().expect("one");
                assert_eq!(check.state, CheckState::Exceeds);
                assert_eq!(
                    check.margin,
                    Some(margin),
                    "the placement's margin at {p:?}"
                );
                let limit = env
                    .limit(kind)
                    .expect("the binding limit is in the boundary");
                assert_eq!(check.limit, LimitId::of_limit_kind(limit.kind));
                assert!(exceeds >= 1);
            }
            Placement::Inside { margin } => {
                inside += 1;
                assert!(binding.is_empty(), "no binding check inside at {p:?}");
                assert_eq!(exceeds, 0, "no check exceeds inside at {p:?}");
                // P29-11 11-3: a per-point limit (the cutting feed cap) is a
                // value check of the stated cut, not a margin that places
                // the point; the least margin reads the placing checks only.
                let least = checks
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| {
                        env.boundary.get(*i).is_none_or(|l| {
                            !matches!(l.edge, rs_cam_core::feeds::envelope::Edge::PerPoint)
                        })
                    })
                    .map(|(_, c)| c)
                    .filter(|c| c.state == CheckState::Within)
                    .filter_map(|c| c.margin)
                    .fold(f64::INFINITY, f64::min);
                let want = if least.is_finite() { least } else { 1.0 };
                assert_eq!(margin, want, "the least margin at {p:?}");
            }
        }
        // A check with a value and a bound states the margin of the two.
        for check in &checks {
            if let (Some(value), Some(bound), Some(margin)) =
                (check.value, check.bound, check.margin)
                && bound.is_finite()
            {
                let own = check.side.margin(value, bound).expect("a positive bound");
                assert!(
                    (own - margin).abs() <= 1e-12 * own.abs().max(1.0),
                    "{:?} at {p:?}: {own} against {margin}",
                    check.limit
                );
            }
            assert!(check.basis.is_some(), "an envelope limit states its basis");
            assert!(
                check.population.is_none(),
                "a planned cut has no population"
            );
        }
    }
    assert!(
        inside >= 1 && outside >= 2,
        "the fixture has both cases: {inside} in, {outside} out"
    );
}

#[test]
fn the_checks_of_an_rpm_choice_hold_its_limits() {
    let choice = RpmChoice {
        rpm: 18_000.0,
        chart_rpm: Some(16_000.0),
        binding: RpmLimit::OperatorCeiling,
        limits: vec![
            RpmLimitValue {
                limit: RpmLimit::SpindleMax,
                rpm: 24_000.0,
            },
            RpmLimitValue {
                limit: RpmLimit::OperatorCeiling,
                rpm: 18_000.0,
            },
            RpmLimitValue {
                limit: RpmLimit::PowerAtRpm,
                rpm: 15_000.0,
            },
        ],
    };
    let checks = limits::of_rpm_choice(&choice);
    assert_eq!(checks.len(), choice.limits.len());
    for (check, old) in checks.iter().zip(&choice.limits) {
        assert_eq!(check.limit, LimitId::of_rpm_limit(old.limit));
        assert_eq!(check.value, Some(choice.rpm));
        assert_eq!(check.bound, Some(old.rpm));
        assert_eq!(check.binding, old.limit == choice.binding);
    }
    let states: Vec<_> = checks.iter().map(|c| c.state).collect();
    assert_eq!(
        states,
        [CheckState::Within, CheckState::Within, CheckState::Exceeds],
        "a limit that only stops the rise can be below the RPM"
    );
}

#[test]
fn the_basis_join_keeps_the_setting_and_the_export_rule() {
    let sources = [
        BoundSource::MachinePowerCurve { rpm: 18_000.0 },
        BoundSource::VendorChipBand {
            floor_mm_per_tooth: Some(0.05),
            ceiling_mm_per_tooth: 0.12,
            source: ChipBoundsSource::VendorLut,
        },
        BoundSource::DeflectionBudget,
        BoundSource::StockToLeave { mm: 0.3 },
        BoundSource::MachineForceBudget {
            budget_n: 35.3,
            ae_mm: 1.2,
            fz_mm: 0.15,
            depth_mm: 3.9,
        },
        BoundSource::DrillEnvelope,
    ];
    let mut settings = BTreeSet::new();
    for source in &sources {
        let basis = limits::basis_of_bound(source);
        assert_eq!(basis.setting(), source.setting(), "{source:?}");
        assert_eq!(basis.gates_export(), source.gates_export(), "{source:?}");
        settings.insert(basis.setting());
    }
    assert_eq!(
        settings.len(),
        5,
        "the five settings are all reached: {settings:?}"
    );
}

#[test]
fn the_limits_folder_names_no_operation_type() {
    let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/limits");
    let mut files = 0;
    let mut text = String::new();
    for entry in std::fs::read_dir(&folder).expect("the folder reads") {
        let path = entry.expect("an entry").path();
        if path.extension().is_some_and(|e| e == "rs") {
            files += 1;
            let rel = format!(
                "src/limits/{}",
                path.file_name().expect("a name").to_string_lossy()
            );
            text.push_str(&common::wide_samples::core_source(&rel));
        }
    }
    assert!(files >= 3, "the scan reads the folder");
    assert!(text.contains("pub enum LimitId"), "the scan reads the code");
    for needle in [
        "OperationConfig",
        "OperationType",
        "OperationFamily",
        "PassRole",
        "LutPassRole",
        "LutOperationFamily",
        "compute::",
        "session::",
    ] {
        assert!(
            !text.contains(needle),
            "src/limits/ names `{needle}`: a limit sees the cut, the tool, the stock and the \
             machine, never the operation type (P29 design 4.5)"
        );
    }
}

/// P29-10: `LimitId::text` names every limit: one noun and one short word
/// each, no two the same, and each short word fits a face.
#[test]
fn every_limit_has_one_name_p29_10() {
    let mut nouns = std::collections::BTreeSet::new();
    let mut shorts = std::collections::BTreeSet::new();
    for limit in LimitId::ALL {
        let text = limit.text();
        assert!(!text.noun.is_empty() && !text.short.is_empty(), "{limit:?}");
        assert!(
            text.short.chars().count() <= 12,
            "{limit:?}: {}",
            text.short
        );
        assert!(nouns.insert(text.noun), "{limit:?}: the noun repeats");
        assert!(
            shorts.insert(text.short),
            "{limit:?}: the short word repeats"
        );
    }
    assert_eq!(nouns.len(), LimitId::ALL.len());
}
