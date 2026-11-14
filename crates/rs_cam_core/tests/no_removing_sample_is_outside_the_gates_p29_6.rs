//! Feeds contract P29-6 commit 6d
//! (`planning/feeds_contract_2026-11-03/phases/P29-6_DESIGN.md` section 4):
//! no sample that removes material is outside the gates.
//!
//! The five fixtures of the cut-kind sentry (`common::kind_fixtures`), each
//! simulated with modulation off, through the session's own load report.
//!
//! - Claim 1: every sample that removes material lands in exactly one group
//!   (`locality::judged_group`), and the group populations sum to the
//!   removing count of the toolpath. Every gate holds one cell for each
//!   group (`ByGroup`, a fixed record: a dropped group does not compile, so
//!   the injected defect of the design note is a compile error, not a red
//!   test).
//! - Claim 2: a sample that removes nothing is in no group.
//! - Claim 3: the body group of each gate is the headline population: the
//!   headline power, deflection and depth populations count no sample
//!   outside the body group, and each non-body group with samples holds a
//!   judged cell whose population counts no sample outside the group.
//! - Claim 4 (non-vacuity): each fixture has removing samples in the Entry
//!   group; the shelf in the Cleanup and the Link groups; the Profile has
//!   entry plunges with no number; the dogbone Pocket has a Cleanup group;
//!   the Adaptive has helix samples.
//! - Claim 5: no cell is `NotJudged` on a milling op (operator Q1: "I think
//!   all moves should be judged"), a milling op holds no Drill-group sample,
//!   and every group that is `Within` gives no diagnostic (ruling Q5).
//! - Claim 6 (a synthetic trace): a group that exceeds a bound reaches
//!   `criteria()` placed in its group, gives one diagnostic whose face and
//!   message name the group, and on export gives a warning row that names
//!   the group; nothing refuses (K6: the export outcome has no refusing
//!   arm).
//! - Claim 7 (a simulated drill cycle): the headline group is the drill
//!   group, every side headline is `NotApplicableForOp` (`limits::scope`),
//!   and the diagnostics list holds no side-gate row.
//! - Claim 8 (P29-11 11-5, F6-a, lead ruling L1): the depth gate and the
//!   force reading read one engagement door. In every group of every
//!   fixture where the reading has a force budget and a sample, the two
//!   populations are equal, and the depth row `Exceeds` exactly when the
//!   force peak is over the budget (else it is `Within`, never `Reported`).
//!   Both sides: the five fixtures hold only `Within` groups; the Pocket
//!   judged on a machine whose force budget is half its measured body
//!   force peak holds an `Exceeds` group (`the_depth_row_is_the_force_record`).
//!   Claim 8b (P29-11 11-6): the narration's peak-depth line is `⚠` exactly
//!   when a depth row exceeds, and it names the row.
//!
//! `cargo test -p rs_cam_core -q --test no_removing_sample_is_outside_the_gates_p29_6`

#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use common::kind_fixtures::{adaptive_session, pocket_session, profile_session, shelf_session};

use rs_cam_core::compute::cutter::build_cutter;
use rs_cam_core::limits::{
    ByGroup, CapTable, CheckRow, CutGroup, CutKind, EntryKind, LimitId, SampleContext,
};
use rs_cam_core::session::ProjectSession;
use rs_cam_core::tool_load::GroupCell;
use rs_cam_core::tool_load::locality::judged_group;
use rs_cam_core::tool_load::verdict::{
    DepthVerdict, GatePopulation, LoadState, ToolpathLoadVerdict,
};
use rs_cam_core::trace::narrate::{
    DepthRows, ToolpathNarrationContext, narrate_toolpath_with_context,
};

/// What one fixture gives.
struct Readout {
    removing: usize,
    by_group: ByGroup<usize>,
    entry_plunges: usize,
    helix: usize,
    verdict: ToolpathLoadVerdict,
}

fn readout(session: &ProjectSession) -> Readout {
    let tc = session.toolpath_config_at(0).expect("the op");
    let tool = build_cutter(
        session
            .tool_by_id(tc.tool_id.expect("a tool"))
            .expect("the tool"),
    )
    .expect("the cutter builds");
    let result = session.result_at(0).expect("a result");
    let annotated = result.annotated();
    let trace = session
        .simulation_result()
        .and_then(|s| s.cut_trace.as_deref())
        .expect("a cut trace");
    let spans = annotated.spans_valid.then_some(annotated.spans.as_slice());
    let caps = CapTable::of_toolpath(&annotated.toolpath, result.semantic_trace.as_deref());
    let ctx = SampleContext::new(
        &tool,
        spans,
        &caps,
        tc.operation
            .cuts(&rs_cam_core::feeds::entry_rule::ConfiguredEntry::registry_default(&tc.operation))
            .body_facts(),
    );
    let mut removing = 0;
    let mut by_group = ByGroup::from_fn(|_| 0_usize);
    let (mut entry_plunges, mut helix) = (0, 0);
    for s in trace.samples.iter().filter(|s| s.toolpath_id == tc.id) {
        let group = judged_group(s, &ctx);
        // Claim 2.
        assert_eq!(
            group.is_some(),
            s.removes_material(),
            "a sample is in a group exactly when it removes material"
        );
        let Some(group) = group else {
            continue;
        };
        removing += 1;
        *by_group.get_mut(group) += 1;
        match CutKind::of_sample(s, &ctx) {
            CutKind::Entry(EntryKind::Plunge) => entry_plunges += 1,
            CutKind::Entry(EntryKind::Helix) => helix += 1,
            CutKind::Entry(EntryKind::Ramp | EntryKind::PeckLadder)
            | CutKind::Body
            | CutKind::FirstPass(_)
            | CutKind::SpringPass
            | CutKind::WallFinish
            | CutKind::EntryApproach
            | CutKind::Cleanup(_)
            | CutKind::CappedStep(_)
            | CutKind::SlotRing
            | CutKind::LeadIn
            | CutKind::LeadOut
            | CutKind::Link
            | CutKind::DressupRelief
            | CutKind::Drill => {}
        }
    }
    let report = session.tool_load_report();
    let verdict = report
        .per_toolpath
        .into_iter()
        .find(|v| v.toolpath_id == tc.id)
        .expect("the verdict of the op");
    Readout {
        removing,
        by_group,
        entry_plunges,
        helix,
        verdict,
    }
}

/// The population of a judged cell, or of the headline verdict.
fn population_of<V>(
    cell: &GroupCell<V>,
    headline: &V,
    status: impl Fn(&V) -> Option<GatePopulation>,
) -> Option<GatePopulation> {
    match cell {
        GroupCell::Headline => status(headline),
        GroupCell::Judged { verdict, .. } => status(verdict),
        GroupCell::NoSamples | GroupCell::NotJudged { .. } => None,
    }
}

/// Claim 8: the depth row against the force reading, group by group.
/// Returns the count of `Within` and `Exceeds` groups.
fn depth_is_the_force_record(name: &str, v: &ToolpathLoadVerdict) -> (usize, usize) {
    let (mut within, mut exceeds) = (0, 0);
    for group in CutGroup::ALL {
        let depth = match v.groups.depth.get(group) {
            GroupCell::Headline => Some(&v.depth),
            GroupCell::Judged { verdict, .. } => Some(verdict),
            GroupCell::NoSamples | GroupCell::NotJudged { .. } => None,
        };
        let force = match v.groups.cut_force.get(group) {
            GroupCell::Headline => v.cut_force.as_ref(),
            GroupCell::Judged { verdict, .. } => verdict.as_ref(),
            GroupCell::NoSamples | GroupCell::NotJudged { .. } => None,
        };
        let (Some(depth), Some(force)) = (depth, force) else {
            continue;
        };
        let Some(budget) = force.budget_n else {
            continue;
        };
        let depth_population = depth
            .as_criterion_status()
            .population
            .expect("a depth population");
        assert_eq!(
            depth_population.contributing, force.population.contributing,
            "{name}: {group:?}: the depth and the force populations"
        );
        if force.population.contributing == 0 {
            continue;
        }
        let over = force.peak_n > budget;
        match depth {
            DepthVerdict::Within { .. } => {
                assert!(
                    !over,
                    "{name}: {group:?}: depth Within, force {} over {budget}",
                    force.peak_n
                );
                within += 1;
            }
            DepthVerdict::Exceeds { .. } => {
                assert!(
                    over,
                    "{name}: {group:?}: depth Exceeds, force {} at or under {budget}",
                    force.peak_n
                );
                exceeds += 1;
            }
            DepthVerdict::Reported { .. } | DepthVerdict::Unmodeled { .. } => panic!(
                "{name}: {group:?}: a group with a force budget and a force has a depth \
                 bound, got {depth:?}"
            ),
        }
    }
    (within, exceeds)
}

fn check(name: &str, r: &Readout) {
    // Claim 8 (P29-11 11-5): on the five fixtures every judged group is
    // inside its force budget.
    let (within, exceeds) = depth_is_the_force_record(name, &r.verdict);
    assert!(
        within > 0 && exceeds == 0,
        "{name}: {within} Within and {exceeds} Exceeds force-budget groups"
    );
    // Claim 1: one group per removing sample; the groups sum to the count.
    let sum: usize = r.by_group.iter().map(|(_, n)| *n).sum();
    assert_eq!(
        sum, r.removing,
        "{name}: the groups sum to the removing count"
    );
    assert!(
        r.removing > 0,
        "{name}: non-vacuity: the op removes material"
    );
    assert_eq!(
        *r.by_group.get(CutGroup::Drill),
        0,
        "{name}: a milling op holds no drill sample"
    );
    let v = &r.verdict;
    let groups = &v.groups;
    assert_eq!(
        groups.headline,
        CutGroup::Body,
        "{name}: a milling op heads with the body"
    );
    for (group, &n) in r.by_group.iter() {
        // Claim 5 and the cell of each group: a group with samples is
        // judged by every gate (the headline group heads the row).
        let cells = [
            (
                "chipload",
                population_of(groups.chipload.get(group), &v.chipload, |x| {
                    x.as_criterion_status().population
                }),
                matches!(groups.chipload.get(group), GroupCell::NotJudged { .. }),
            ),
            (
                "power",
                population_of(groups.power.get(group), &v.power, |x| {
                    x.as_criterion_status().population
                }),
                matches!(groups.power.get(group), GroupCell::NotJudged { .. }),
            ),
            (
                "deflection",
                population_of(groups.deflection.get(group), &v.deflection, |x| {
                    x.as_criterion_status().population
                }),
                matches!(groups.deflection.get(group), GroupCell::NotJudged { .. }),
            ),
            (
                "depth",
                population_of(groups.depth.get(group), &v.depth, |x| {
                    x.as_criterion_status().population
                }),
                matches!(groups.depth.get(group), GroupCell::NotJudged { .. }),
            ),
            (
                "cut force",
                population_of(groups.cut_force.get(group), &v.cut_force, |x| {
                    x.as_ref().map(|r| r.population)
                }),
                matches!(groups.cut_force.get(group), GroupCell::NotJudged { .. }),
            ),
        ];
        for (gate, population, not_judged) in cells {
            match group {
                CutGroup::Drill => {}
                CutGroup::Body | CutGroup::Entry | CutGroup::Cleanup | CutGroup::Link => {
                    assert!(
                        !not_judged,
                        "{name}: {gate} judges the {group:?} group (Q1)"
                    );
                }
            }
            if n == 0 {
                continue;
            }
            // Claim 3: the population of a cell counts no sample outside
            // its group.
            let p = population
                .unwrap_or_else(|| panic!("{name}: {gate} states a population on {group:?}"));
            assert!(
                p.contributing <= n,
                "{name}: {gate} on {group:?} counts {} samples, the group holds {n}",
                p.contributing
            );
        }
    }
    // Claim 5: a `Within` group gives no row and no diagnostic.
    for status in v.criteria() {
        if let Some(group @ (CutGroup::Entry | CutGroup::Cleanup | CutGroup::Link)) = status.group {
            assert_eq!(
                status.state,
                LoadState::Exceeds,
                "{name}: only a group that exceeds reaches the criteria ({group:?})"
            );
        }
    }
    let ids: Vec<String> =
        rs_cam_core::diagnostics::adapters::from_tool_load::diagnostics_from_load_verdict(v)
            .into_iter()
            .filter(|d| d.message.contains(" group ("))
            .map(|d| d.message)
            .collect();
    let any_group_exceeds = v.criteria().iter().any(|s| {
        matches!(
            s.group,
            Some(CutGroup::Entry | CutGroup::Cleanup | CutGroup::Link)
        )
    });
    assert_eq!(
        ids.is_empty(),
        !any_group_exceeds,
        "{name}: a group diagnostic exactly when a group exceeds: {ids:?}"
    );
}

#[test]
fn no_pocket_sample_is_outside_the_gates() {
    let r = readout(&pocket_session(false));
    check("pocket", &r);
    assert!(
        *r.by_group.get(CutGroup::Entry) > 0,
        "pocket: an Entry group"
    );
}

#[test]
fn no_dogbone_sample_is_outside_the_gates() {
    let r = readout(&pocket_session(true));
    check("pocket with dogbones", &r);
    assert!(
        *r.by_group.get(CutGroup::Entry) > 0,
        "dogbones: an Entry group"
    );
    assert!(
        *r.by_group.get(CutGroup::Cleanup) > 0,
        "dogbones: the relief is a Cleanup group"
    );
    assert!(
        matches!(
            r.verdict.groups.power.get(CutGroup::Cleanup),
            GroupCell::Judged { .. }
        ),
        "dogbones: the power gate judges the relief"
    );
}

#[test]
fn no_profile_sample_is_outside_the_gates() {
    let r = readout(&profile_session());
    check("profile", &r);
    assert!(
        *r.by_group.get(CutGroup::Entry) > 0,
        "profile: an Entry group"
    );
    assert!(r.entry_plunges > 0, "profile: entry plunges");
    // The plunges have no side engagement: the Entry population of the
    // power gate counts fewer samples than the group holds.
    match r.verdict.groups.power.get(CutGroup::Entry) {
        GroupCell::Judged { samples, verdict } => {
            let p = verdict
                .as_criterion_status()
                .population
                .expect("a population");
            assert!(
                p.contributing < *samples,
                "profile: the plunges are judged with no number ({} of {samples})",
                p.contributing
            );
        }
        other @ (GroupCell::Headline | GroupCell::NoSamples | GroupCell::NotJudged { .. }) => {
            panic!("profile: the Entry group is judged, got {other:?}")
        }
    }
}

#[test]
fn no_shelf_sample_is_outside_the_gates() {
    let r = readout(&shelf_session());
    check("shelf", &r);
    for group in [CutGroup::Entry, CutGroup::Cleanup, CutGroup::Link] {
        assert!(*r.by_group.get(group) > 0, "shelf: a {group:?} group");
    }
}

/// Ruling Q6: the helix entry.
#[test]
fn no_adaptive_helix_sample_is_outside_the_gates() {
    let r = readout(&adaptive_session());
    check("adaptive", &r);
    assert!(r.helix > 0, "adaptive: helix samples");
    assert!(
        matches!(
            r.verdict.groups.power.get(CutGroup::Entry),
            GroupCell::Judged { .. }
        ),
        "adaptive: the power gate judges the helix"
    );
}

/// Claim 8, the `Exceeds` side: the Pocket fixture judged on its machine
/// with the force budget set to half of its measured body force peak (a
/// budget derived from the fixture, not a typed number). The body force is
/// then over the budget, and the depth row of the body exceeds; the groups
/// still agree with the force reading.
#[test]
fn the_depth_row_is_the_force_record() {
    let session = pocket_session(false);
    let trace = session
        .simulation_result()
        .and_then(|s| s.cut_trace.as_deref())
        .expect("a cut trace");
    let parts = session.load_context_parts(0).expect("the context parts");
    let ctx = parts.ctx();
    let tolerance = rs_cam_core::tool_load::ToleranceBands::default();
    let base = rs_cam_core::tool_load::evaluate_toolpath(
        &ctx,
        Some(trace),
        Some(session.machine()),
        &tolerance,
    );
    let body_peak = base
        .cut_force
        .as_ref()
        .expect("a body force reading")
        .peak_n;
    assert!(body_peak > 0.0, "non-vacuity: a body force");
    let mut machine = session.machine().clone();
    machine.rigidity.force_budget_n = 0.5 * body_peak;
    let low =
        rs_cam_core::tool_load::evaluate_toolpath(&ctx, Some(trace), Some(&machine), &tolerance);
    let (_, exceeds) = depth_is_the_force_record("pocket on half its force", &low);
    assert!(
        exceeds > 0,
        "non-vacuity: a group over the force budget, depth {:?}",
        low.depth
    );
    assert!(low.depth.is_exceeded(), "the body depth row exceeds");
    // Claim 8b (P29-11 11-6): the narration repeats the depth row. With the
    // rows of the half-budget verdict the peak-depth line is `⚠` and names
    // the worst exceeding row; with the rows of the real machine it is `ℹ`.
    let tc = session.toolpath_config_at(0).expect("the op");
    let tool = build_cutter(
        session
            .tool_by_id(tc.tool_id.expect("a tool"))
            .expect("the tool"),
    )
    .expect("the cutter builds");
    let result = session.result_at(0).expect("a result");
    let narrate = |verdict: &ToolpathLoadVerdict| {
        let rows: Vec<CheckRow> = rs_cam_core::limits::rows_of_verdict(verdict)
            .into_iter()
            .filter(|r| r.check().limit == LimitId::ForceBudget)
            .collect();
        let text = narrate_toolpath_with_context(
            result.annotated(),
            None,
            Some(trace),
            None,
            &tool,
            &ToolpathNarrationContext {
                toolpath_id: Some(tc.id),
                depth_per_pass_mm: tc.operation.depth_per_pass(),
                ..Default::default()
            },
            DepthRows::Rows(&rows),
        );
        let line = text
            .lines()
            .find(|l| l.contains("peak axial DOC"))
            .unwrap_or_default()
            .to_owned();
        (line, rows)
    };
    let (low_line, low_rows) = narrate(&low);
    let worst = low_rows
        .iter()
        .filter(|r| r.tone().exceeds())
        .min_by(|a, b| a.rank_cmp(b))
        .expect("an exceeding depth row");
    assert!(
        low_line.contains('\u{26a0}') && low_line.contains(worst.face()),
        "the line repeats the exceeding row {}: {low_line}",
        worst.face()
    );
    let (base_line, _) = narrate(&base);
    assert!(
        base_line.contains('\u{2139}') && !base_line.contains('\u{26a0}'),
        "no row exceeds: {base_line}"
    );
}

/// Claim 6: a synthetic trace with one body sample and one waterline
/// cleanup sample whose depth puts its deflection over the bound.
#[test]
fn a_group_that_exceeds_warns_named_by_its_group_and_refuses_nothing() {
    use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
    use rs_cam_core::compute::tool_config::ToolMaterial;
    use rs_cam_core::ids::ToolpathId;
    use rs_cam_core::machine::{MachineProfile, SteadyFeedMmMin};
    use rs_cam_core::material::{Material, WoodSpecies};
    use rs_cam_core::stock::simulation_cut::{
        CutKinematics, Engagement, SimulationCutSample, SimulationCutTrace,
    };
    use rs_cam_core::tool::{FlatEndmill, ToolDefinition};
    use rs_cam_core::tool_load::{ToleranceBands, ToolpathLoadContext, evaluate_toolpath};
    use rs_cam_core::trace::toolpath_spans::{Span, SpanId, SpanKind};

    let sample = |idx: usize, axial: f64, path: Vec<SpanId>| SimulationCutSample {
        toolpath_id: ToolpathId::from_raw(0),
        move_index: idx,
        sample_index: idx,
        position: [0.0, 0.0, -axial],
        segment_time_s: 0.1,
        is_cutting: true,
        cut_kinematics: CutKinematics::Linear,
        feed_rate_mm_min: 1500.0,
        spindle_rpm: 18_000,
        flute_count: 2,
        axial_doc_mm: axial,
        axial_engagement_mm: axial,
        arc_engagement_radians: Some(std::f64::consts::PI),
        chipload_mm_per_tooth: 1500.0 / 36_000.0,
        effective_chip_thickness_mm: Some(1500.0 / 36_000.0),
        engagement: Engagement::with_radial_woc(1.0),
        removed_volume_est_mm3: 0.1,
        mrr_mm3_s: 1.0,
        span_path: path,
        ..SimulationCutSample::test_fixture()
    };
    let spans = vec![
        Span::new(0, 2, SpanKind::Operation),
        Span::new(0, 1, SpanKind::DepthPass),
        Span::new(1, 2, SpanKind::WaterlineCleanup),
    ];
    let trace = SimulationCutTrace::from_samples(
        1.0,
        vec![
            sample(0, 1.0, vec![SpanId(0), SpanId(1)]),
            sample(1, 20.0, vec![SpanId(0), SpanId(2)]),
        ],
    );
    // A slender tool: a 3 mm end mill at 45 mm stickout, so a 20 mm slot
    // bends it past the 200 um finish target.
    let tool = ToolDefinition::new(
        Box::new(FlatEndmill::new(3.0, 40.0)),
        3.0,
        45.0,
        2,
        ToolMaterial::Carbide,
    );
    let material = Material::SolidWood {
        species: WoodSpecies::HardMaple,
    };
    let op = OperationConfig::new_default(OperationType::Adaptive3d);
    let ctx = ToolpathLoadContext::of_operation(
        ToolpathId::from_raw(0),
        &op,
        &tool,
        &material,
        SteadyFeedMmMin::op_feed_only(1500.0),
        Some(&spans),
        CapTable::empty(),
        None,
    );
    let machine = MachineProfile::default();
    let v = evaluate_toolpath(
        &ctx,
        Some(&trace),
        Some(&machine),
        &ToleranceBands::default(),
    );
    assert_ne!(
        v.deflection.state(),
        LoadState::Exceeds,
        "the headline reads the 1 mm body sample: {:?}",
        v.deflection
    );
    let rows: Vec<_> = v
        .criteria()
        .into_iter()
        .filter(|s| s.group == Some(CutGroup::Cleanup))
        .collect();
    assert!(
        rows.iter().any(|s| s.state == LoadState::Exceeds
            && s.kind == rs_cam_core::tool_load::verdict::CriterionKind::Deflection),
        "the Cleanup deflection row exceeds and reaches the criteria: {:?}",
        v.groups.deflection.get(CutGroup::Cleanup)
    );
    assert!(
        rows.iter().all(|s| s.row_label().starts_with("cleanup ")),
        "each Cleanup row names its group"
    );
    let diags =
        rs_cam_core::diagnostics::adapters::from_tool_load::diagnostics_from_load_verdict(&v);
    let named: Vec<_> = diags
        .iter()
        .filter(|d| {
            d.face.starts_with("cleanup group: ") && d.message.starts_with("cleanup group (")
        })
        .collect();
    assert!(
        named
            .iter()
            .any(|d| d.id.as_str() == rs_cam_core::diagnostics::ids::LOAD_DEFLECTION_EXCEEDS),
        "one deflection diagnostic names the Cleanup group: {:?}",
        diags
            .iter()
            .map(|d| (&d.face, &d.message))
            .collect::<Vec<_>>()
    );
    for d in &named {
        assert!(
            d.face.chars().count() <= rs_cam_core::diagnostics::presentation::FACE_MAX_CHARS,
            "the face stays short: {}",
            d.face
        );
    }
    // K6: the export findings warn and advise; the outcome has no refusing
    // arm, so nothing refuses.
    let found = rs_cam_core::gcode::exceedance_findings(&[(v.toolpath_id, v.criteria())]);
    assert!(
        found
            .rows
            .iter()
            .any(|r| r.detail.contains("cleanup group")),
        "the export warns on the Cleanup deflection, named by its group: {:?}",
        found.rows.iter().map(|r| &r.detail).collect::<Vec<_>>()
    );
}

/// Claim 7: a simulated drill cycle.
#[test]
fn a_drill_cycle_heads_with_the_drill_group_and_no_side_row() {
    use common::op_fixtures::{CaseSpec, Fixture, Frame, Prior, case_session, simulate};
    use rs_cam_core::compute::catalog::OperationConfig;
    use rs_cam_core::compute::config::StockSource;
    use rs_cam_core::compute::operation_configs::DrillConfig;
    use rs_cam_core::tool_load::verdict::UnmodeledReason;

    let (mut session, index) = case_session(CaseSpec {
        name: "drill",
        op: OperationConfig::Drill(DrillConfig::default()),
        prior: Prior::None,
        stock_source: StockSource::Fresh,
        frame: Frame::TopUp,
        fixture: Fixture::Standard,
        tool: None,
    });
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let _ = session
        .generate_toolpath(index, &cancel)
        .expect("the drill generates");
    simulate(&mut session);
    let id = session.toolpath_config_at(index).expect("the op").id;
    let report = session.tool_load_report();
    let v = report
        .per_toolpath
        .iter()
        .find(|v| v.toolpath_id == id)
        .expect("the drill verdict");
    assert_eq!(
        v.groups.headline,
        CutGroup::Drill,
        "a drill heads with the drill group"
    );
    assert!(
        matches!(v.groups.power.get(CutGroup::Drill), GroupCell::Headline),
        "the drill cell is the headline"
    );
    for (gate, reason) in [
        ("chipload", v.chipload.unmodeled_reason()),
        ("power", v.power.unmodeled_reason()),
        ("deflection", v.deflection.unmodeled_reason()),
        ("depth", v.depth.unmodeled_reason()),
    ] {
        assert!(
            matches!(reason, Some(UnmodeledReason::NotApplicableForOp(_))),
            "drill: the {gate} headline does not apply: {reason:?}"
        );
    }
    assert!(v.cut_force.is_none(), "drill: no force reading");
    let side = [
        "load.chipload",
        "load.power",
        "load.deflection",
        "load.depth",
    ];
    let diags =
        rs_cam_core::diagnostics::adapters::from_tool_load::diagnostics_from_load_verdict(v);
    assert!(
        diags
            .iter()
            .all(|d| side.iter().all(|p| !d.id.as_str().starts_with(p))),
        "drill: no side-gate row in the list: {:?}",
        diags.iter().map(|d| d.id.as_str()).collect::<Vec<_>>()
    );
}
