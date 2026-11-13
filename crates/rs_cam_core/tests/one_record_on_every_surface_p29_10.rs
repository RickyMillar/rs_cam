//! Feeds contract P29-10
//! (`planning/feeds_contract_2026-11-03/phases/P29-10_DESIGN.md` section 6):
//! one record on every surface.
//!
//! Every surface reads `limits::CheckRow`. The envelope writes its planned
//! rows and its stated rows (`SafeEnvelope::rows`); `limits::rows_of_verdict`
//! projects the gate verdict; `limits::rows_of_rpm_choice` the RPM choice.
//! `ProjectSession::limit_rows` is the one door.
//!
//! - Claim 1: the planned rows hold exactly one row per declared cut and
//!   planned limit (each limit with rows has one row per cut, and no pair
//!   repeats), and the stated rows are `of_envelope` at the stated point,
//!   one per check, in order.
//! - Claim 2: `rows_of_verdict` gives exactly one row per gate and cut group
//!   (`NotJudged` and `NoSamples` included), plus the gantry-push row, one
//!   row per drill gate, and one row per own material and gate (S6). The
//!   force reading gives no row (the F6 merge stopped, section 13.3). The
//!   headline row of each gate is `of_criterion` of the headline verdict
//!   (the gate's own state). The door gives the same rows.
//! - Claim 3: every row with a value, a bound and a margin has
//!   `margin == side.margin(judged value, bound)` (a stored-feed row judges
//!   the end of its interval, so its state follows its margin); a planned or
//!   stated row `Exceeds` exactly when its margin is below `-1e-9`.
//! - Claim 4: every exemption id is a row of `EXEMPTIONS`; every `NoBound`,
//!   `NotJudgedHere` and may-occur row has one.
//! - Claim 5: `WorstCase` holds exactly on a ramp entry row with an own
//!   depth; `LowerBound` only on a row of the body plane of a Project
//!   Curve (`BodyDepth::AtLeast`);
//!   `StoredFeed` only on an entry row of a chip limit. The tone follows
//!   `Tone::of`.
//! - Claim 6: every face is at most `ROW_FACE_MAX_CHARS` characters and no
//!   face prints "0 %" for a margin that is not zero.
//!
//! `cargo test -p rs_cam_core -q --test one_record_on_every_surface_p29_10`

#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use std::collections::{BTreeMap, BTreeSet};

use common::kind_fixtures::{adaptive_session, pocket_session, profile_session, shelf_session};

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::{DressupConfig, DressupEntryStyle};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolFact, ToolId, ToolType};
use rs_cam_core::feeds::SpindleStrategy;
use rs_cam_core::feeds::envelope::{EnvelopeInput, SafeEnvelope};
use rs_cam_core::feeds::suggest::{NoProject, SuggestContext};
use rs_cam_core::limits::{
    self, Caveat, CheckRow, CheckState, CutGroup, CutKind, EXEMPTIONS, EntryKind, LimitId,
    NoNumberReason, ROW_FACE_MAX_CHARS, Subject, Tone,
};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::session::ProjectSession;
use rs_cam_core::tool_load::GroupCell;

fn tool(kind: ToolType, d: f64) -> ToolConfig {
    let mut t = ToolConfig::test_default(ToolId::from_raw(1), kind);
    t.diameter = d;
    t.set_flute_count(ToolFact::Stated(2));
    t.set_cutting_length(ToolFact::Stated((d * 3.0).max(12.0)));
    t.set_shank_diameter(ToolFact::Stated(d.max(3.0)));
    t.shaft_diameter = d.max(3.0);
    t.set_stickout(ToolFact::Stated(t.cutting_length_planning_mm() + 8.0));
    t
}

fn envelope(
    op: &OperationConfig,
    tool: &ToolConfig,
    dressups: Option<&DressupConfig>,
) -> Option<SafeEnvelope> {
    SafeEnvelope::for_cut(EnvelopeInput {
        operation: op,
        tool,
        machine: &MachineProfile::default(),
        material: &Material::SolidWood {
            species: WoodSpecies::GenericHardwood,
        },
        lut: rs_cam_core::feeds::embedded_vendor_lut(),
        spindle_strategy: SpindleStrategy::default(),
        context: SuggestContext::no_project(NoProject {
            dressups,
            ..NoProject::NONE
        }),
    })
    .ok()
}

/// Every op at its fixture values on four tools, with the registry entry
/// and with a ramp entry.
fn envelopes() -> Vec<(String, OperationType, SafeEnvelope)> {
    let ramp = DressupConfig {
        entry_style: DressupEntryStyle::Ramp,
        ..Default::default()
    };
    let mut out = Vec::new();
    for t in [
        tool(ToolType::EndMill, 3.175),
        tool(ToolType::EndMill, 6.0),
        tool(ToolType::BallNose, 6.0),
        tool(ToolType::VBit, 6.35),
    ] {
        for &op_type in OperationType::ALL {
            let op = rs_cam_core::compute::fixture_values::fixture_op(op_type);
            for (label, dressups) in [("registry", None), ("ramp", Some(&ramp))] {
                if let Some(env) = envelope(&op, &t, dressups) {
                    out.push((
                        format!("{op_type:?}/{label}/{:?}{}", t.tool_type, t.diameter),
                        op_type,
                        env,
                    ));
                }
            }
        }
    }
    out
}

/// Claim 6: the face holds no "0 %" that is not a zero margin.
fn prints_a_false_zero(face: &str) -> bool {
    let bytes = face.as_bytes();
    face.match_indices("0 %").any(|(i, _)| {
        i == 0
            || !bytes
                .get(i - 1)
                .is_some_and(|b| b.is_ascii_digit() || *b == b'.')
    })
}

/// Claims 3 to 6 on one row.
fn check_row(name: &str, row: &CheckRow) {
    let c = row.check();
    // Claim 3.
    let stored = row.caveats().contains(&Caveat::StoredFeed);
    if let (Some(v), Some(b), Some(m)) = (c.value, c.bound, c.margin)
        && !stored
    {
        let want = c.side.margin(v, b).expect("a positive bound");
        assert!(
            (m - want).abs() <= 1e-12 * want.abs().max(1.0),
            "{name}: {}: margin {m} against {want}",
            row.face()
        );
    }
    let judged_here = matches!(row.subject(), Subject::Planned { .. } | Subject::Stated);
    if judged_here && let Some(m) = c.margin {
        assert_eq!(
            c.state == CheckState::Exceeds,
            m < -1e-9,
            "{name}: {}: the state {:?} against the margin {m}",
            row.face(),
            c.state
        );
    }
    // Claim 4.
    if let Some(id) = row.exemption() {
        assert!(
            EXEMPTIONS.iter().any(|e| e.id == id),
            "{name}: the row {id} is not in EXEMPTIONS"
        );
    }
    let needs_row = matches!(
        c.state,
        CheckState::NoNumber(NoNumberReason::NoBound | NoNumberReason::NotJudgedHere)
    ) || row.may_occur_row().is_some();
    assert!(
        !needs_row || row.exemption().is_some(),
        "{name}: {}: no exemption row",
        row.face()
    );
    // Claim 5.
    let worst = row.caveats().contains(&Caveat::WorstCase);
    let ramp_own =
        row.cut() == Some(CutKind::Entry(EntryKind::Ramp)) && row.own_depth_mm().is_some();
    assert_eq!(worst, ramp_own, "{name}: {}: worst case", row.face());
    if stored {
        assert!(
            matches!(row.cut(), Some(CutKind::Entry(_)))
                && matches!(
                    c.limit,
                    LimitId::ChipMax | LimitId::ChipMin | LimitId::RubbingFloor
                ),
            "{name}: {}: a stored feed off an entry chip row",
            row.face()
        );
    }
    assert_eq!(row.tone(), Tone::of(c, row.caveats()), "{name}: the tone");
    // Only the machine force budget does not gate an export.
    if row.caveats().contains(&Caveat::Advisory) {
        assert_eq!(
            c.limit,
            LimitId::ForceBudget,
            "{name}: {}: an advisory row off the force budget",
            row.face()
        );
    }
    // Claim 6.
    assert!(
        row.face().chars().count() <= ROW_FACE_MAX_CHARS,
        "{name}: the face is too long: {}",
        row.face()
    );
    if c.margin.is_some_and(|m| m != 0.0) {
        assert!(
            !prints_a_false_zero(row.face()),
            "{name}: a false zero: {}",
            row.face()
        );
    }
    assert!(!row.detail().is_empty(), "{name}: an empty detail");
}

#[test]
fn claim_1_one_planned_row_per_cut_and_limit_and_the_stated_rows() {
    let mut rows_seen = 0;
    let mut tones = BTreeSet::new();
    let mut caveats = BTreeSet::new();
    for (name, op_type, env) in envelopes() {
        let planned: Vec<&CheckRow> = env.rows().iter().filter(|r| r.is_planned()).collect();
        let stated: Vec<&CheckRow> = env
            .rows()
            .iter()
            .filter(|r| matches!(r.subject(), Subject::Stated))
            .collect();
        assert_eq!(
            planned.len() + stated.len(),
            env.rows().len(),
            "{name}: only planned and stated rows"
        );
        // The planned rows come first.
        if let Some(first_stated) = env
            .rows()
            .iter()
            .position(|r| matches!(r.subject(), Subject::Stated))
        {
            assert!(
                env.rows()
                    .iter()
                    .skip(first_stated)
                    .all(|r| !r.is_planned()),
                "{name}: a planned row after a stated row"
            );
        }
        let cuts: BTreeSet<String> = planned
            .iter()
            .map(|r| r.cut().expect("a cut").label().to_owned())
            .collect();
        let mut per_limit: BTreeMap<LimitId, BTreeSet<String>> = BTreeMap::new();
        for r in &planned {
            let label = r.cut().expect("a cut").label().to_owned();
            assert!(
                per_limit.entry(r.check().limit).or_default().insert(label),
                "{name}: a cut repeats under {:?}",
                r.check().limit
            );
        }
        for (limit, of_limit) in &per_limit {
            assert_eq!(of_limit, &cuts, "{name}: {limit:?} misses a declared cut");
        }
        // The stated rows are `of_envelope` at the stated point.
        let want = env
            .stated
            .map(|p| limits::of_envelope(&env, p))
            .unwrap_or_default();
        let got: Vec<_> = stated.iter().map(|r| r.check().clone()).collect();
        assert_eq!(got, want, "{name}: the stated rows");
        for r in env.rows() {
            check_row(&name, r);
            tones.insert(format!("{:?}", r.tone()));
            for c in r.caveats() {
                caveats.insert(match c {
                    Caveat::MayOccur { .. } => "MayOccur".to_owned(),
                    Caveat::SampleDepth { .. } => "SampleDepth".to_owned(),
                    Caveat::WorstCase
                    | Caveat::LowerBound
                    | Caveat::StoredFeed
                    | Caveat::Advisory
                    | Caveat::NoSamples
                    | Caveat::AtEarlierLeave => format!("{c:?}"),
                });
            }
            if r.caveats().contains(&Caveat::LowerBound) {
                assert!(
                    op_type == OperationType::ProjectCurve && r.own_depth_mm().is_none(),
                    "{name}: a lower bound off the body plane of a Project Curve: {}",
                    r.face()
                );
            }
            rows_seen += 1;
        }
    }
    assert!(rows_seen > 1000, "non-vacuity: {rows_seen} rows");
    for tone in [
        "Within",
        "Exceeds",
        "ExceedsAdvisory",
        "NoNumber",
        "NotApplicable",
    ] {
        assert!(
            tones.contains(tone),
            "non-vacuity: no {tone} row: {tones:?}"
        );
    }
    for caveat in [
        "WorstCase",
        "LowerBound",
        "StoredFeed",
        "MayOccur",
        "Advisory",
    ] {
        assert!(
            caveats.contains(caveat),
            "non-vacuity: no {caveat} caveat: {caveats:?}"
        );
    }
}

/// Claim 2 on one simulated fixture.
fn verdict_rows(name: &str, session: &ProjectSession) {
    let report = session.tool_load_report();
    let verdict = report.per_toolpath.first().expect("one toolpath");
    let rows = limits::rows_of_verdict(verdict);
    let own = verdict
        .material_split
        .as_deref()
        .map_or(0, |s| s.own.len() * 4 + s.not_judged.len());
    let drill = verdict.drill_gates.as_ref().map_or(0, |_| 3);
    // Four gates (chip, power, deflection, depth) over every group, the
    // gantry row, the drill gates, the own materials.
    assert_eq!(
        rows.len(),
        4 * CutGroup::ALL.len() + 1 + drill + own,
        "{name}: the row count"
    );
    let mut cells = BTreeSet::new();
    for r in &rows {
        let Subject::Group {
            group, material, ..
        } = r.subject()
        else {
            panic!("{name}: a verdict row is a group row");
        };
        if material.is_none() && r.check().limit != LimitId::GantryPush {
            // The chip gate names ChipMax or ChipMin: one gate.
            let limit = r.check().limit;
            let gate = if limit == LimitId::ChipMin {
                LimitId::ChipMax
            } else {
                limit
            };
            assert!(
                cells.insert((gate, *group)),
                "{name}: the cell {gate:?} {group:?} repeats"
            );
        }
        check_row(name, r);
    }
    // The headline row of each gate is the gate's own record.
    let h = verdict.groups.headline;
    let headline = |limits: &[LimitId]| {
        rows.iter()
            .find(|r| {
                limits.contains(&r.check().limit)
                    && matches!(r.subject(), Subject::Group { group, headline: true, material: None, .. } if *group == h)
            })
            .map(|r| r.check().clone())
            .expect("a headline row")
    };
    assert_eq!(
        headline(&[LimitId::ChipMax, LimitId::ChipMin]),
        limits::of_criterion(&verdict.chipload.as_criterion_status()),
        "{name}: the chip row"
    );
    assert_eq!(
        headline(&[LimitId::SpindlePower]),
        limits::of_criterion(&verdict.power.as_criterion_status()),
        "{name}: the power row"
    );
    assert_eq!(
        headline(&[LimitId::ForceBudget]),
        limits::of_criterion(&verdict.depth.as_criterion_status()),
        "{name}: the depth row"
    );
    // Non-vacuity: a judged non-headline group and an empty cell.
    assert!(
        rows.iter().any(|r| matches!(
            r.subject(),
            Subject::Group {
                headline: false,
                samples: Some(n),
                ..
            } if *n > 0
        )),
        "{name}: a judged group beside the headline"
    );
    assert!(
        rows.iter().any(
            |r| r.check().state == CheckState::NoNumber(NoNumberReason::NotJudgedHere)
                && r.exemption().is_some()
        ),
        "{name}: a not-judged cell with its row"
    );
    // Each not-judged cell names its own row of `EXEMPTIONS`.
    let mut cells = Vec::new();
    not_judged_cells(&verdict.groups.chipload, &mut cells);
    not_judged_cells(&verdict.groups.power, &mut cells);
    not_judged_cells(&verdict.groups.deflection, &mut cells);
    not_judged_cells(&verdict.groups.depth, &mut cells);
    assert!(!cells.is_empty(), "{name}: non-vacuity: a not-judged cell");
    for (group, id) in &cells {
        assert!(
            rows.iter().any(|r| matches!(
                r.subject(),
                Subject::Group { group: g, .. } if g == group
            ) && r.exemption() == Some(id.as_str())),
            "{name}: the {group:?} cell row {id} is not on its row"
        );
    }
    // The one door gives the same rows (the session holds the trace).
    let trace = session
        .simulation_result()
        .and_then(|s| s.cut_trace.as_ref());
    let door = session.limit_rows(0, trace).expect("the toolpath");
    assert_eq!(door.verdict(), rows, "{name}: the door verdict rows");
    let env_rows = door
        .envelope()
        .expect("a tool")
        .expect("the envelope builds");
    let tc = session.toolpath_config_at(0).expect("the op");
    let direct = session
        .safe_envelope(tc)
        .expect("a tool")
        .expect("the envelope builds");
    assert_eq!(env_rows, direct.rows(), "{name}: the door envelope rows");
}

/// The not-judged cells of one gate, with their exemption row.
fn not_judged_cells<V>(
    cells: &rs_cam_core::limits::ByGroup<GroupCell<V>>,
    out: &mut Vec<(CutGroup, String)>,
) {
    for group in CutGroup::ALL {
        if let GroupCell::NotJudged { row, .. } = cells.get(group) {
            out.push((group, row.clone()));
        }
    }
}

#[test]
fn claim_2_one_row_per_gate_and_group() {
    verdict_rows("pocket", &pocket_session(false));
    verdict_rows("dogbone", &pocket_session(true));
    verdict_rows("profile", &profile_session());
    verdict_rows("adaptive", &adaptive_session());
    verdict_rows("shelf", &shelf_session());
}

#[test]
fn claim_6_a_tiny_margin_never_prints_zero() {
    assert!(prints_a_false_zero("chip min 0 % under floor"));
    assert!(!prints_a_false_zero("chip min < 0.1 % under floor"));
    assert!(!prints_a_false_zero("force 100 % of limit"));
    assert!(!prints_a_false_zero("force 10.0 % of limit"));
}
