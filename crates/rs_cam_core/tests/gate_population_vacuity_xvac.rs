//! **X-VAC** — a gate handed an empty population passes and looks healthy.
//!
//! Ledger row: `planning/review_2026-08-04/TECH_DEBT_2_CLOSEOUT.md` §4
//! (**X-VAC**); census and design:
//! `planning/review_2026-08-08/XVAC_CENSUS.md`; scheduled as TD3 sweep
//! item S-1. Programme rule §0.4: *"Populations at source intent; a gate
//! handed an empty population is vacuous until its
//! `sample_count`/`sample_range` is checked."*
//!
//! # The pre-fix reproduction (kept permanently, per §0.1)
//!
//! Measured 2026-08-05 on the arc-fit instance: three gates returned
//! `Within` with `sample_range 0..0`, no locality and `available_kw
//! 0.0` — **indistinguishable on every surface from a measured clean
//! cut**, and one of them was suppressing a burn advisory. The arc-fit
//! *instance* was fixed; the *class* was not.
//!
//! [`the_2026_08_05_shape_still_reproduces`] rebuilds that exact shape
//! from a synthetic trace and asserts every part of it that is still
//! true today: the state is `Within`, the range is `0..0`, the headline
//! numbers are zero, and the deflection gate's confidence tier reads
//! **`Validated`** — the most trustworthy label the gate can print, on a
//! verdict resting on nothing.
//!
//! # The bar (a POPULATION bar, not a verdict bar)
//!
//! The ledger row says so in as many words: *"Note that a verdict bar
//! tests this vacuously — the bar must be a **population** bar."* A test
//! asserting `state == Within` on the vacuous fixture passes before and
//! after any fix. Every assertion below that constitutes the bar reads
//! `population.contributing`, or a rendered string derived from it.
//!
//! # What is deliberately NOT asserted
//!
//! No verdict flips. `LoadState`, `ExceededCriterion`, `exceeded_criteria`
//! and the export gate are byte-identical across this change; a vacuous
//! `Within` is still `Within`. This is report-tier
//! (`gate_outcome_is_untouched_by_the_marker` pins it).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::tool_config::ToolMaterial;
use rs_cam_core::drill_metrics::{build_drill_toolpath_summary, emit_drill_samples};
use rs_cam_core::drill_op::{DrillHole, DrillOp, HoleSource, ToolProfile};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::Material;
use rs_cam_core::simulation_cut::{
    CutKinematics, Engagement, SimulationCutSample, SimulationCutSummary, SimulationCutTrace,
};
use rs_cam_core::tool::{FlatEndmill, ToolDefinition};
use rs_cam_core::tool_load::verdict::{
    CriterionKind, DeflectionVerdict, LoadState, PopulationUnit, PowerVerdict, ToolpathLoadVerdict,
};
use rs_cam_core::tool_load::{GateEnv, ToleranceBands, ToolpathLoadContext};

const TP: ToolpathId = ToolpathId(0);

fn tool() -> ToolDefinition {
    ToolDefinition::new(
        Box::new(FlatEndmill::new(6.35, 20.0)),
        6.35,
        30.0,
        20.0,
        30.0,
        2,
        ToolMaterial::Carbide,
    )
}

/// One cutting sample with real engagement, real arc and real feed. The
/// only thing that varies between the two fixtures is `in_transit_span`.
fn cutting_sample(idx: usize, in_transit: bool) -> SimulationCutSample {
    let feed = 2000.0;
    SimulationCutSample {
        toolpath_id: TP,
        move_index: idx,
        sample_index: idx,
        position: [0.0, 0.0, -1.0],
        cumulative_time_s: 0.1 * idx as f64,
        segment_time_s: 0.1,
        is_cutting: true,
        cut_kinematics: CutKinematics::Linear,
        feed_rate_mm_min: feed,
        spindle_rpm: 18_000,
        flute_count: 2,
        axial_doc_mm: 1.0,
        axial_engagement_mm: 1.0,
        arc_engagement_radians: Some(std::f64::consts::FRAC_PI_2),
        chipload_mm_per_tooth: feed / (18_000.0 * 2.0),
        effective_chip_thickness_mm: Some(feed / (18_000.0 * 2.0)),
        engagement: Engagement::with_radial_woc(0.5),
        removed_volume_est_mm3: 0.1,
        mrr_mm3_s: 1.0,
        // THE emptying predicate. With `spans: None` the gate cannot
        // split configured entries from phantom transit, so
        // `locality::is_phantom_transit` degrades to this bare flag and
        // drops every sample carrying it (`locality.rs:219`).
        in_transit_span: in_transit,
        ..SimulationCutSample::test_fixture()
    }
}

fn trace(samples: Vec<SimulationCutSample>) -> SimulationCutTrace {
    SimulationCutTrace {
        sample_step_mm: 1.0,
        summary: SimulationCutSummary {
            sample_count: samples.len(),
            toolpath_count: 1,
            issue_count: 0,
            hotspot_count: 0,
            total_runtime_s: 1.0,
            cutting_runtime_s: 1.0,
            rapid_runtime_s: 0.0,
            air_cut_time_s: 0.0,
            low_engagement_time_s: 0.0,
            average_engagement: 0.5,
            peak_chipload_mm_per_tooth: 0.05,
            peak_axial_doc_mm: 1.0,
            peak_plunge_descent_mm: 0.0,
            total_removed_volume_est_mm3: 1.0,
            average_mrr_mm3_s: 1.0,
            per_kinematics: std::collections::BTreeMap::new(),
            runtime_by_intent: None,
        },
        samples,
        ..SimulationCutTrace::test_fixture()
    }
}

/// 12 cutting samples, all of them phantom transit. The population the
/// gates are *offered* is 12; the population that *contributes* is 0.
fn vacuous_trace() -> SimulationCutTrace {
    trace((0..12).map(|i| cutting_sample(i, true)).collect())
}

/// Same 12 samples, none in transit — a genuinely measured cut, the
/// control arm every vacuity assertion below is contrasted against.
fn measured_trace() -> SimulationCutTrace {
    trace((0..12).map(|i| cutting_sample(i, false)).collect())
}

fn verdicts(t: &SimulationCutTrace) -> (PowerVerdict, DeflectionVerdict) {
    let tool = tool();
    let material = Material::default();
    let machine = MachineProfile::shapeoko_makita();
    let tolerance = ToleranceBands::default();
    let ctx = ToolpathLoadContext {
        toolpath_id: TP,
        tool: &tool,
        material: &material,
        operation_family: rs_cam_core::feeds::vendor_lut::LutOperationFamily::Pocket,
        pass_role: rs_cam_core::feeds::vendor_lut::LutPassRole::Roughing,
        operation_feed_rate_mm_min: 2000.0,
        operation_kind: OperationType::Pocket,
        spans: None,
        drill_op: None,
    };
    let env = GateEnv {
        sim_trace: Some(t),
        machine: Some(&machine),
        tolerance: &tolerance,
    };
    (
        rs_cam_core::tool_load::power::evaluate(&ctx, &env),
        rs_cam_core::tool_load::deflection::evaluate(&ctx, &env),
    )
}

fn verdict_for(t: &SimulationCutTrace) -> ToolpathLoadVerdict {
    let (power, deflection) = verdicts(t);
    ToolpathLoadVerdict {
        toolpath_id: TP,
        chipload: rs_cam_core::tool_load::verdict::ChiploadVerdict::Unmodeled {
            reason: rs_cam_core::tool_load::verdict::UnmodeledReason::NoVendorData,
        },
        power,
        deflection,
        drill_gates: None,
        modulation_summary: None,
        feed_explanation: None,
    }
}

// ── 1. the pre-fix reproduction, kept permanently ───────────────────

/// **The 2026-08-05 shape, reproduced.** Everything asserted here was
/// true before the marker existed and is still true after it: the fix
/// added a field, it did not move a verdict. This is the reason a
/// verdict bar cannot test X-VAC.
#[test]
fn the_2026_08_05_shape_still_reproduces() {
    let t = vacuous_trace();
    let (power, deflection) = verdicts(&t);

    // Power: `Within`, `available_kw 0.0`, empty range, no locality.
    match &power {
        PowerVerdict::Within {
            peak_kw,
            available_kw,
            evidence,
            ..
        } => {
            assert_eq!(*peak_kw, 0.0, "the ledger's `peak` on a vacuous pass");
            assert_eq!(*available_kw, 0.0, "the ledger's `available_kw 0.0`");
            assert_eq!(
                evidence.sample_range,
                0..0,
                "the ledger's `sample_range 0..0`"
            );
            assert!(evidence.locality.is_none(), "the ledger's `no locality`");
        }
        other => panic!("expected the vacuous Within shape, got {other:?}"),
    }

    // Deflection: the same, and worse — a zero peak with no slot sample
    // resolves the confidence tier to `Validated`, the most trustworthy
    // label this gate can print.
    match &deflection {
        DeflectionVerdict::Within {
            peak_mm,
            evidence,
            confidence,
            ..
        } => {
            assert_eq!(*peak_mm, 0.0);
            assert_eq!(evidence.sample_range, 0..0);
            assert!(
                matches!(
                    confidence,
                    rs_cam_core::tool_load::verdict::Confidence::Validated
                ),
                "an empty population still reads Validated: {confidence:?}"
            );
        }
        other => panic!("expected the vacuous Within shape, got {other:?}"),
    }

    // And the coarse state — the thing a verdict bar would test — is
    // identical to the measured arm's. This assertion is the point:
    // it passes on BOTH fixtures, which is why it is not the bar.
    let measured = verdict_for(&measured_trace());
    let vacuous = verdict_for(&t);
    for (a, b) in vacuous.criteria().iter().zip(measured.criteria().iter()) {
        assert_eq!(a.state, b.state, "{:?} state differs — it must not", a.kind);
    }
}

// ── 2. the marker, on the verdict ───────────────────────────────────

/// **The population bar.** The vacuous arm states a population of zero;
/// the measured arm states a positive one. Nothing else distinguishes
/// them, which is exactly what X-VAC filed.
#[test]
fn every_per_sample_gate_states_its_population() {
    let vacuous = verdict_for(&vacuous_trace());
    let measured = verdict_for(&measured_trace());

    for status in vacuous.criteria() {
        if status.state == LoadState::Unmodeled {
            continue; // the chipload arm here has no row; it refuses.
        }
        let p = status
            .population
            .unwrap_or_else(|| panic!("{:?} stated no population", status.kind));
        assert_eq!(
            p.contributing, 0,
            "{:?} must report an empty gate",
            status.kind
        );
        assert_eq!(
            p.offered, 12,
            "{:?} must report what it was handed",
            status.kind
        );
        assert_eq!(p.unit, PopulationUnit::Samples);
        assert_eq!(p.filtered_out(), 12);
        assert!(status.is_vacuous(), "{:?} must read vacuous", status.kind);
    }

    for status in measured.criteria() {
        if status.state == LoadState::Unmodeled {
            continue;
        }
        let p = status
            .population
            .unwrap_or_else(|| panic!("{:?} stated no population", status.kind));
        assert_eq!(
            p.contributing, 12,
            "{:?} measured the whole cut",
            status.kind
        );
        assert!(
            !status.is_vacuous(),
            "{:?} measured 12 samples and must not read vacuous",
            status.kind
        );
    }
}

/// An **unstated** population is not vacuous. The repo's standing
/// contract — `None` means *not measured*, never *measured zero* —
/// applies to this field too, so a pre-2026-08-14 wire payload that
/// carries no `population` key must not start reading as an empty gate.
#[test]
fn an_unstated_population_is_unknown_not_empty() {
    let measured = verdict_for(&measured_trace());
    let mut json = serde_json::to_value(&measured.power).expect("power verdict serialises");
    let removed = json
        .get_mut("evidence")
        .and_then(|e| e.as_object_mut())
        .and_then(|e| e.remove("population"));
    assert!(
        removed.is_some(),
        "the marker must be on the wire to remove"
    );

    let legacy: PowerVerdict =
        serde_json::from_value(json).expect("a payload without the key must still deserialise");
    let status = legacy.as_criterion_status();
    assert!(
        status.population.is_none(),
        "absent key reads as not stated"
    );
    assert!(
        !status.is_vacuous(),
        "not stated must never be mistaken for empty"
    );
    assert_eq!(status.vacuity_clause(), "", "and renders no claim at all");
}

// ── 3. the marker, on every surface ─────────────────────────────────

/// Surface 1 — **the diagnostics adapter**, which is the single
/// construction site the CLI `project` report, the GUI diagnostics
/// panel, MCP `get_diagnostics` and the narration list all share.
#[test]
fn the_diagnostics_surface_says_the_pass_is_vacuous() {
    use rs_cam_core::diagnostics::adapters::from_tool_load::diagnostics_from_load_verdict;

    let vacuous = diagnostics_from_load_verdict(&verdict_for(&vacuous_trace()));
    let measured = diagnostics_from_load_verdict(&verdict_for(&measured_trace()));

    let vacuous_msgs: Vec<&str> = vacuous.iter().map(|d| d.message.as_str()).collect();
    assert!(
        vacuous_msgs
            .iter()
            .any(|m| m.contains("Power within budget")),
        "the pre-fix message must still be there: {vacuous_msgs:?}"
    );
    // The chipload arm of this fixture refuses (no vendor row), and a
    // refusal has no population to report — that row is `Unmodeled`, not
    // vacuous, and must stay untouched.
    let gate_rows: Vec<&&str> = vacuous_msgs
        .iter()
        .filter(|m| !m.starts_with("Chipload:"))
        .collect();
    assert_eq!(gate_rows.len(), 2, "power + deflection: {vacuous_msgs:?}");
    for m in gate_rows {
        assert!(
            m.contains("VACUOUS") && m.contains("0 of 12 samples"),
            "every vacuous gate row must say so, got: {m}"
        );
    }
    for d in &measured {
        assert!(
            !d.message.contains("VACUOUS"),
            "a measured gate must not be marked vacuous: {}",
            d.message
        );
    }

    // Report-tier: same ids, same severities, same count.
    assert_eq!(vacuous.len(), measured.len());
    for (a, b) in vacuous.iter().zip(measured.iter()) {
        assert_eq!(a.id, b.id);
        assert_eq!(a.severity, b.severity);
        assert_eq!(a.state, b.state);
    }
}

/// Surface 2 — **the MCP wire**. `get_tool_load_report` serialises the
/// `ToolLoadReport` whole (`serde_json::to_value(&report)`), so the
/// marker has to survive as JSON an agent can read without knowing
/// which gate produced it.
#[test]
fn the_mcp_wire_carries_the_population() {
    let report = rs_cam_core::tool_load::verdict::ToolLoadReport {
        per_toolpath: vec![verdict_for(&vacuous_trace())],
    };
    let json = serde_json::to_value(&report).expect("report serialises");
    let p = &json["per_toolpath"][0]["power"]["evidence"]["population"];
    assert_eq!(p["contributing"], 0);
    assert_eq!(p["offered"], 12);
    assert_eq!(p["unit"], "samples");

    let d = &json["per_toolpath"][0]["deflection"]["evidence"]["population"];
    assert_eq!(d["contributing"], 0);

    // And the measured arm is a different JSON value at the same path —
    // the distinguishability X-VAC says does not exist today.
    let measured = rs_cam_core::tool_load::verdict::ToolLoadReport {
        per_toolpath: vec![verdict_for(&measured_trace())],
    };
    let mjson = serde_json::to_value(&measured).expect("report serialises");
    assert_ne!(
        json["per_toolpath"][0]["power"]["evidence"], mjson["per_toolpath"][0]["power"]["evidence"],
        "the two must not be byte-identical on the wire"
    );
}

/// Surface 3 — **the shared renderer clause**. GUI badge, GUI status
/// flag, tooltip and CLI all read `CriterionStatus::vacuity_clause`, so
/// the wording is asserted once here rather than four times in two
/// crates. `rs_cam_viz` reads it through `is_vacuous()` /
/// `vacuity_clause()` and never re-derives the condition.
#[test]
fn the_shared_renderer_clause_names_the_population() {
    let vacuous = verdict_for(&vacuous_trace());
    let power = vacuous.power.as_criterion_status();
    let clause = power.vacuity_clause();
    assert!(clause.contains("VACUOUS"), "{clause}");
    assert!(clause.contains("0 of 12 samples"), "{clause}");
    assert!(
        clause.contains("not a measurement of a clean cut"),
        "the clause must refuse the reading a bare `Within` invites: {clause}"
    );

    let measured = verdict_for(&measured_trace());
    assert_eq!(measured.power.as_criterion_status().vacuity_clause(), "");
}

// ── 4. the drill trio ───────────────────────────────────────────────

fn drill_op(holes: Vec<DrillHole>) -> DrillOp {
    DrillOp {
        holes,
        hole_source: HoleSource::ModelDerived,
        tool_profile: ToolProfile::StandardTwist,
        tool_diameter_mm: 6.0,
        cycle: rs_cam_core::drill::DrillCycle::Peck(2.0),
        feed_rate_mm_min: 300.0,
        spindle_rpm: 18_000,
        flute_count: 2,
        material: Material::default(),
        retract_z_mm: 0.0,
    }
}

/// The drill trio's population is holes, not samples — and two of its
/// three gates read `0.0 → Within` on a hole set with no depth, which
/// is the same vacuity in a different unit. `hole_count` existed on the
/// summary and never reached the verdict.
#[test]
fn the_drill_trio_states_its_hole_population() {
    let zero_depth = drill_op(vec![
        DrillHole {
            xy: [0.0, 0.0],
            top_z: 0.0,
            bottom_z: 0.0,
        },
        DrillHole {
            xy: [10.0, 0.0],
            top_z: 0.0,
            bottom_z: 0.0,
        },
    ]);
    let samples = emit_drill_samples(TP, &zero_depth);
    let summary = build_drill_toolpath_summary(TP, &zero_depth, &samples);
    let gates = rs_cam_core::tool_load::drill_gates::evaluate(&zero_depth, &summary);

    // Pre-fix shape: both depth gates pass on nothing, and nothing says so.
    assert!(!gates.chip_welding.is_exceeded());
    assert!(!gates.peck_adequacy.is_exceeded());
    assert_eq!(gates.chip_welding.observed(), 0.0);
    assert!(gates.worst_hole_id.is_none());

    let p = gates
        .population
        .expect("the trio must state its population");
    assert_eq!(p.contributing, 0);
    assert_eq!(p.offered, 2);
    assert_eq!(p.unit, PopulationUnit::Holes);

    let real = drill_op(vec![DrillHole {
        xy: [0.0, 0.0],
        top_z: 0.0,
        bottom_z: -18.0,
    }]);
    let rs = emit_drill_samples(TP, &real);
    let rsum = build_drill_toolpath_summary(TP, &real, &rs);
    let rgates = rs_cam_core::tool_load::drill_gates::evaluate(&real, &rsum);
    let rp = rgates.population.expect("stated");
    assert_eq!(rp.contributing, 1);
    assert!(!rp.is_vacuous());
}

/// The hole population reaches all three drill criteria, and the
/// diagnostics surface renders it — including the plunge-feed gate,
/// whose observation is config-derived but whose verdict still judges a
/// feed that nothing runs at.
#[test]
fn the_drill_criteria_and_diagnostics_carry_the_hole_population() {
    use rs_cam_core::diagnostics::adapters::from_tool_load::diagnostics_from_load_verdict;

    let zero_depth = drill_op(vec![DrillHole {
        xy: [0.0, 0.0],
        top_z: 0.0,
        bottom_z: 0.0,
    }]);
    let samples = emit_drill_samples(TP, &zero_depth);
    let summary = build_drill_toolpath_summary(TP, &zero_depth, &samples);
    let gates = rs_cam_core::tool_load::drill_gates::evaluate(&zero_depth, &summary);

    let v = ToolpathLoadVerdict {
        toolpath_id: TP,
        chipload: rs_cam_core::tool_load::verdict::ChiploadVerdict::Unmodeled {
            reason: rs_cam_core::tool_load::verdict::UnmodeledReason::NotApplicableForOp(
                "drill".to_owned(),
            ),
        },
        power: PowerVerdict::Unmodeled {
            reason: rs_cam_core::tool_load::verdict::UnmodeledReason::NotApplicableForOp(
                "drill".to_owned(),
            ),
        },
        deflection: DeflectionVerdict::Unmodeled {
            reason: rs_cam_core::tool_load::verdict::UnmodeledReason::NotApplicableForOp(
                "drill".to_owned(),
            ),
        },
        drill_gates: Some(gates),
        modulation_summary: None,
        feed_explanation: None,
    };

    let drill_kinds = [
        CriterionKind::DrillChipWelding,
        CriterionKind::DrillPeckAdequacy,
        CriterionKind::DrillPlungeFeed,
    ];
    for status in v.criteria() {
        if !drill_kinds.contains(&status.kind) {
            continue;
        }
        assert!(
            status.is_vacuous(),
            "{:?} judged a drill op with no hole of depth",
            status.kind
        );
        assert!(status.vacuity_clause().contains("0 of 1 holes"));
    }

    for d in diagnostics_from_load_verdict(&v) {
        assert!(
            d.message.contains("VACUOUS") && d.message.contains("holes"),
            "drill diagnostic must carry the marker: {}",
            d.message
        );
    }
}

// ── 5. report-tier guard ────────────────────────────────────────────

/// **No gate outcome moves.** The marker is a report, and the moment it
/// starts deciding an export is the moment it needs a Checkpoint. This
/// pins the boundary: same states, same exceedances, same export gate,
/// on a fixture whose population is empty.
#[test]
fn gate_outcome_is_untouched_by_the_marker() {
    let vacuous = verdict_for(&vacuous_trace());
    assert!(
        !vacuous.any_exceeded(),
        "a vacuous gate must not start tripping"
    );
    assert!(
        vacuous.exceeded_criteria().is_empty(),
        "and must not reach the export gate"
    );
    for status in vacuous.criteria() {
        if status.is_vacuous() {
            assert_eq!(
                status.state,
                LoadState::Within,
                "a vacuous Within stays Within — flipping it is a Checkpoint question"
            );
            assert!(status.exceeded.is_none());
        }
    }
}
