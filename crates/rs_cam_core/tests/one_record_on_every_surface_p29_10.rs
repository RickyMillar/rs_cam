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
//!   `StoredFeed` (lead ruling 2) exactly on an entry chip row whose
//!   stored-feed chip interval holds the bound: a Face ramp one write step
//!   under the band has it, the same ramp at half the band feed has not and
//!   reads the plain tone. The tone follows `Tone::of`.
//! - Claim 3b (lead ruling 3): a face says what its row compares. A planned
//!   force-budget, deflection or depth-table row and the depth gate row
//!   compare a depth (`Compared::Depth`): the face names "depth" and the
//!   limit as the cause of the cap; a stated row compares the axis of its
//!   edge; every other row names its limit.
//! - Claim 7: `CheckRow::worse_than` is a total order: an `Exceeds` row
//!   before an `ExceedsAdvisory` row before every other row; inside one
//!   tone the stated point first (ruling O6), then the lower margin.
//! - Claim 8 (lead ruling 6): the door never gives an empty verdict list.
//!   A disabled toolpath (the report leaves it out) gets its rows with no
//!   trace; a drill cycle gives its three drill rows and the side gates as
//!   not applicable.
//! - Claim 6: every face is at most `ROW_FACE_MAX_CHARS` characters and no
//!   face prints "0 %" for a margin that is not zero.
//! - Claim 9 (lead ruling after 10e): a face follows its state. No row with
//!   state `Within` holds "over" or "under" in its face (a value past the
//!   bound inside the gate tolerance reads "at limit"); no row with an
//!   exceed tone holds "% of" without "over" or "under". A drill plunge
//!   feed inside its envelope near the floor states the floor as a floor.
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
    self, Caveat, CheckRow, CheckState, Compared, CutGroup, CutKind, EXEMPTIONS, EntryKind,
    LimitId, NoNumberReason, ROW_FACE_MAX_CHARS, Subject, Tone,
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
    // Claim 3. An entry chip row reads a stored feed: its margin follows
    // the end of the stored-feed interval toward the inside (P29-9), so its
    // state follows its margin (below) but the margin is not the one of the
    // stated value.
    let stored = row.caveats().contains(&Caveat::StoredFeed);
    let reads_stored_feed = matches!(row.cut(), Some(CutKind::Entry(_)))
        && matches!(
            c.limit,
            LimitId::ChipMax | LimitId::ChipMin | LimitId::RubbingFloor
        );
    if let (Some(v), Some(b), Some(m)) = (c.value, c.bound, c.margin)
        && !reads_stored_feed
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
    // Claim 9: the face follows the state.
    face_follows_state(name, row);
    // Claim 3b: a face names what the row compares (lead ruling 3), with
    // or without a number (P29-10 10e: "depth not judged here"); a row that
    // compares its limit's own quantity names the limit.
    match row.compared() {
        Compared::Depth => assert!(
            row.face().contains("depth "),
            "{name}: a depth row names the depth: {}",
            row.face()
        ),
        Compared::Width => assert!(
            row.face().contains("width "),
            "{name}: a width row names the width: {}",
            row.face()
        ),
        Compared::Load => assert!(
            row.face().contains(c.limit.text().short),
            "{name}: a load row names its limit: {}",
            row.face()
        ),
    }
    if matches!(row.subject(), Subject::Planned { .. }) {
        let depth_ceiling = matches!(
            c.limit,
            LimitId::ForceBudget | LimitId::ToolDeflection | LimitId::VendorDepthTable
        );
        assert_eq!(
            row.compared() == Compared::Depth,
            depth_ceiling,
            "{name}: {}: the compared quantity of a planned row",
            row.face()
        );
    }
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
    let mut all_rows: Vec<CheckRow> = Vec::new();
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
        // Claim 3b: a stated row compares the axis of its edge.
        for (i, r) in stated.iter().enumerate() {
            let want = env
                .boundary
                .get(i)
                .map_or(Compared::Load, |l| Compared::of_edge(&l.edge));
            assert_eq!(r.compared(), want, "{name}: the stated row {i}");
        }
        all_rows.extend(env.rows().iter().cloned());
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
    worse_than_is_a_total_order(&all_rows);
    // Claim 3b: a planned force-budget row compares a depth: its face says
    // "depth" and names the force cap, never a force share.
    let force_rows: Vec<&CheckRow> = all_rows
        .iter()
        .filter(|r| {
            r.is_planned() && r.check().limit == LimitId::ForceBudget && r.check().margin.is_some()
        })
        .collect();
    assert!(
        !force_rows.is_empty(),
        "non-vacuity: planned force-budget rows"
    );
    for r in force_rows {
        assert!(
            r.face().contains("depth ") && r.face().contains("force cap"),
            "a force-budget row names the depth and the force cap: {}",
            r.face()
        );
    }
    for compared in [Compared::Depth, Compared::Width, Compared::Load] {
        assert!(
            all_rows.iter().any(|r| r.compared() == compared),
            "non-vacuity: no {compared:?} row"
        );
    }
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
        // `StoredFeed` is held by claim 5 (a Face with a Suggest-written
        // ramp): no fixture op here sits inside its stored-feed interval.
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
    // Claim 3b: the depth gate rows compare a depth.
    for r in &rows {
        if r.check().limit == LimitId::ForceBudget {
            assert_eq!(r.compared(), Compared::Depth, "{name}: {}", r.face());
        }
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

/// Claim 9 on one row.
fn face_follows_state(name: &str, row: &CheckRow) {
    let face = row.face();
    if row.check().state == CheckState::Within {
        assert!(
            !face.contains(" over ") && !face.contains(" under "),
            "{name}: a row inside its bound says over or under: {face}"
        );
    }
    if row.tone().exceeds() {
        assert!(
            !face.contains("% of") || face.contains(" over ") || face.contains(" under "),
            "{name}: an exceeding row reads a share of its limit: {face}"
        );
    }
}

/// Claim 9: the margin face of a `Within` check past its bound inside the
/// gate tolerance reads "at limit"; a floor that holds reads its share of
/// the floor; an `Exceeds` check reads its share over.
#[test]
fn claim_9_a_face_follows_its_state() {
    use rs_cam_core::limits::{BoundSide, Check, margin_face};
    let check = |side: BoundSide, value: f64, bound: f64, state: CheckState| Check {
        limit: LimitId::ChipMax,
        value: Some(value),
        bound: Some(bound),
        unit: "mm/tooth",
        side,
        margin: side.margin(value, bound),
        state,
        basis: None,
        binding: false,
        population: None,
    };
    let face = |c: &Check| margin_face(c, Compared::Load).expect("a margin");
    // The bulk rough of the operator file: 0.17912843029568415 against
    // 0.17912843029568412 (a margin of -1.5e-16), `Within` by the gate.
    let at = check(
        BoundSide::Ceiling,
        0.179_128_430_295_684_15,
        0.179_128_430_295_684_12,
        CheckState::Within,
    );
    assert_eq!(face(&at), "at limit");
    let past = check(BoundSide::Ceiling, 0.2, 0.1, CheckState::Exceeds);
    assert_eq!(face(&past), "100 % over limit");
    let inside = check(BoundSide::Ceiling, 0.05, 0.1, CheckState::Within);
    assert_eq!(face(&inside), "50 % of limit");
    // The pin drill of the operator file: 1000 mm/min over 6 mm against the
    // wood floor 50 mm/min per mm (`DRILL_PLUNGE_FLOOR_WOOD_PER_MM`).
    let floor = check(BoundSide::Floor, 1000.0 / 6.0, 50.0, CheckState::Within);
    assert_eq!(face(&floor), "333 % of floor");
    let under = check(BoundSide::Floor, 30.0, 50.0, CheckState::Exceeds);
    assert_eq!(face(&under), "40 % under floor");
    let barely = check(BoundSide::Floor, 49.999_999, 50.0, CheckState::Within);
    assert_eq!(face(&barely), "at floor");
    let exceeds_inside = check(BoundSide::Ceiling, 0.1, 0.1, CheckState::Exceeds);
    assert_eq!(face(&exceeds_inside), "at limit");
}

/// Claim 9 (lead ruling 3 of 10e2): the drill plunge-feed row reports the
/// bound the gate judged. The gate classifies the feed per mm against the
/// envelope `[lo, hi]` and reports the NEARER bound; inside the envelope
/// near its floor that bound is the floor, and the row states it as a
/// floor ("333 % of floor"), never a share over a ceiling.
#[test]
fn claim_9_the_drill_feed_row_states_its_floor_as_a_floor() {
    use rs_cam_core::tool_load::UnmodeledReason;
    use rs_cam_core::tool_load::drill_gates::{
        DrillCycleKind, DrillGateOutcome, DrillGatesVerdict, classify_plunge_feed,
    };
    use rs_cam_core::tool_load::verdict::{
        ChiploadVerdict, DeflectionVerdict, DepthVerdict, PowerVerdict, ToolpathLoadVerdict,
    };
    let wood = Material::SolidWood {
        species: WoodSpecies::GenericHardwood,
    };
    // The pin drill of the operator file: 1000 mm/min on a 6 mm cutter.
    let plunge = classify_plunge_feed(1000.0, 6.0, &wood);
    let (lo, hi) = plunge.two_sided_envelope().expect("a two-sided envelope");
    assert!(
        matches!(plunge, DrillGateOutcome::Within { .. }) && plunge.bound() == lo && lo < hi,
        "non-vacuity: the feed is inside the envelope, nearer its floor"
    );
    let na = || UnmodeledReason::NotApplicableForOp("drill cycle".to_owned());
    let within = |observed: f64, threshold: f64| DrillGateOutcome::Within {
        observed,
        threshold,
        envelope_lo: None,
        envelope_hi: Some(threshold),
    };
    let verdict = ToolpathLoadVerdict {
        toolpath_id: rs_cam_core::ToolpathId::from_raw(0),
        groups: rs_cam_core::tool_load::verdict::GroupVerdicts::headline_only_for_tests(
            CutGroup::Drill,
        ),
        chipload: ChiploadVerdict::Unmodeled { reason: na() },
        power: PowerVerdict::Unmodeled { reason: na() },
        deflection: DeflectionVerdict::Unmodeled { reason: na() },
        depth: DepthVerdict::Unmodeled { reason: na() },
        drill_gates: Some(DrillGatesVerdict {
            chip_welding: within(0.5, 6.0),
            peck_adequacy: within(0.3, 6.0),
            plunge_feed: plunge,
            worst_hole_id: None,
            population: None,
            cycle: DrillCycleKind::Peck,
        }),
        modulation_summary: None,
        feed_explanation: None,
        kinematic_utilization: None,
        material_split: None,
        cut_force: None,
    };
    let rows = limits::rows_of_verdict(&verdict);
    let feed = rows
        .iter()
        .find(|r| r.check().limit == LimitId::DrillPlungeFeed)
        .expect("the drill feed row");
    assert_eq!(
        feed.check().state,
        CheckState::Within,
        "the gate's own state"
    );
    assert_eq!(feed.check().bound, Some(lo), "the bound the gate reports");
    assert_eq!(feed.check().side, rs_cam_core::limits::BoundSide::Floor);
    assert!(
        feed.face().ends_with("333 % of floor"),
        "the face: {}",
        feed.face()
    );
    assert!(
        feed.detail().contains("the envelope is 50 to"),
        "the detail names the envelope: {}",
        feed.detail()
    );
    for row in &rows {
        face_follows_state("drill", row);
    }
}

/// Claim 7: `worse_than` over `rows` is a total order with the ruled keys.
fn worse_than_is_a_total_order(rows: &[CheckRow]) {
    let tone_rank = |t: Tone| match t {
        Tone::Exceeds => 0,
        Tone::ExceedsAdvisory => 1,
        Tone::Within | Tone::NoNumber | Tone::NotApplicable => 2,
    };
    let mut sorted: Vec<&CheckRow> = rows.iter().collect();
    sorted.sort_by(|a, b| a.rank_cmp(b));
    for pair in sorted.windows(2) {
        let &[a, b] = pair else {
            panic!("a window of two");
        };
        // Antisymmetry and agreement with the sort.
        assert!(!b.worse_than(a), "{} after {}", a.face(), b.face());
        let (ta, tb) = (tone_rank(a.tone()), tone_rank(b.tone()));
        assert!(ta <= tb, "the tone order: {} then {}", a.face(), b.face());
        if ta == tb && ta < 2 {
            let stated = |r: &CheckRow| matches!(r.subject(), Subject::Stated);
            assert!(
                stated(a) || !stated(b),
                "a stated row first: {} then {}",
                a.face(),
                b.face()
            );
            if stated(a) == stated(b) {
                let m = |r: &CheckRow| r.check().margin.unwrap_or(f64::INFINITY);
                assert!(m(a) <= m(b), "the lower margin first");
            }
        }
    }
    // Irreflexive.
    for r in rows.iter().take(50) {
        assert!(!r.worse_than(r), "a row is not worse than itself");
    }
    assert!(
        sorted.first().is_some_and(|r| r.tone() == Tone::Exceeds),
        "non-vacuity: an Exceeds row ranks first"
    );
}

/// The add-door Face of the FM1 instrument on a 6 mm end mill, with a ramp
/// entry and the speeds Suggest writes (the feed rounded to the write step).
fn suggested_face() -> (OperationConfig, ToolConfig, DressupConfig) {
    use rs_cam_core::feeds::suggest::{
        StockContext, SuggestForOperationInput, suggest_for_operation,
    };
    let machine = MachineProfile::default();
    let t = tool(ToolType::EndMill, 6.0);
    let material = Material::SolidWood {
        species: WoodSpecies::GenericHardwood,
    };
    let stock = StockContext {
        stock_top_z: 0.0,
        stock_bottom_z: -18.0,
        stock_z: Some(18.0),
        stock_padding: 2.0,
    };
    let (mut op, _) = rs_cam_core::feeds::suggest::default_operation_for_tool(
        OperationType::Face,
        &stock,
        rs_cam_core::compute::geometry_default::RuleContext {
            tool: &t,
            material: &material,
            machine: &machine,
        },
    );
    rs_cam_core::compute::fixture_values::fill_unset(&mut op);
    op.set_spindle_rpm(None);
    let dressups = DressupConfig {
        entry_style: DressupEntryStyle::Ramp,
        ..Default::default()
    };
    let suggested = suggest_for_operation(SuggestForOperationInput {
        operation: &op,
        tool: &t,
        machine: &machine,
        material: &material,
        lut: rs_cam_core::feeds::embedded_vendor_lut(),
        spindle_strategy: SpindleStrategy::default(),
        context: SuggestContext::no_project(NoProject {
            dressups: Some(&dressups),
            ..NoProject::NONE
        }),
    })
    .expect("the FM1 Face is backed");
    (suggested.operation, t, dressups)
}

/// The ramp entry `ChipMin` row of `op`.
fn ramp_chip_min(op: &OperationConfig, t: &ToolConfig, dressups: &DressupConfig) -> CheckRow {
    envelope(op, t, Some(dressups))
        .expect("the Face is backed")
        .rows()
        .iter()
        .find(|r| {
            r.cut() == Some(CutKind::Entry(EntryKind::Ramp)) && r.check().limit == LimitId::ChipMin
        })
        .cloned()
        .expect("the ramp ChipMin row")
}

#[test]
fn claim_5_a_stored_feed_caveat_holds_only_where_the_rounding_decides() {
    use rs_cam_core::feeds::suggest::FEED_WRITE_STEP_MM_MIN;
    let (mut op, t, dressups) = suggested_face();
    let ramp = op.ramp_feed_rate().expect("Suggest writes a ramp feed");
    let rpm = f64::from(op.spindle_rpm().expect("Suggest writes the speed"));
    let per_rev = rpm * f64::from(t.flute_count_planning());
    // Inside the interval: the stored ramp is under the band by less than
    // one write step, so the rounding decides the state.
    let near = ramp_chip_min(&op, &t, &dressups);
    let bound = near.check().bound.expect("a band minimum");
    assert!(
        ramp / per_rev <= bound && bound <= (ramp + FEED_WRITE_STEP_MM_MIN) / per_rev,
        "non-vacuity: the band minimum is inside the interval of the stored feed"
    );
    assert!(
        near.caveats().contains(&Caveat::StoredFeed),
        "a row inside the interval: {}",
        near.face()
    );
    // Far from the interval: half the band feed. No caveat, the plain tone.
    let _ = op
        .as_params_mut()
        .set_ramp_feed_rate(Some((ramp / 2.0).floor()));
    let far = ramp_chip_min(&op, &t, &dressups);
    assert_eq!(far.check().state, CheckState::Exceeds, "half the band feed");
    assert!(
        !far.caveats().contains(&Caveat::StoredFeed),
        "a row far from its bound: {}",
        far.face()
    );
    // The plain tone: the tone of the row's other caveats (the ramp's own
    // depth is a worst case, which softens on its own).
    let others: Vec<Caveat> = far
        .caveats()
        .iter()
        .copied()
        .filter(|c| *c != Caveat::StoredFeed)
        .collect();
    assert_eq!(far.tone(), Tone::of(far.check(), &others), "{}", far.face());
    assert!(
        far.caveats().contains(&Caveat::WorstCase),
        "the ramp row states its worst case"
    );
}

#[test]
fn claim_8_a_disabled_toolpath_and_a_drill_give_their_rows() {
    use common::op_fixtures::{CaseSpec, Fixture, Frame, Prior, case_session, simulate};
    use rs_cam_core::compute::config::StockSource;
    use rs_cam_core::compute::operation_configs::DrillConfig;
    use rs_cam_core::session::{Command, SetToolpathEnabledArgs};
    // A disabled toolpath: the report leaves it out, the door does not.
    let mut session = pocket_session(false);
    let _ = session
        .apply(Command::SetToolpathEnabled(SetToolpathEnabledArgs {
            index: 0,
            enabled: false,
        }))
        .expect("disable the op");
    assert!(
        session.tool_load_report().per_toolpath.is_empty(),
        "non-vacuity: the report leaves the disabled op out"
    );
    let rows = session.limit_rows(0, None).expect("the op").verdict();
    assert_eq!(
        rows.len(),
        4 * CutGroup::ALL.len() + 1,
        "the no-simulation rows"
    );
    assert!(
        rows.iter().any(|r| matches!(
            r.check().state,
            CheckState::NoNumber(NoNumberReason::NoSimulation)
        )),
        "a no-simulation row"
    );
    for r in &rows {
        check_row("disabled", r);
    }
    // A drill cycle: the drill rows and the side gates not applicable.
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
    let trace = session
        .simulation_result()
        .and_then(|s| s.cut_trace.as_ref());
    let rows = session
        .limit_rows(index, trace)
        .expect("the drill")
        .verdict();
    assert_eq!(
        rows.len(),
        4 * CutGroup::ALL.len() + 1 + 3,
        "the drill row count"
    );
    let drill: Vec<&CheckRow> = rows
        .iter()
        .filter(|r| {
            matches!(
                r.check().limit,
                LimitId::DrillChipWelding | LimitId::DrillPeckAdequacy | LimitId::DrillPlungeFeed
            )
        })
        .collect();
    assert_eq!(drill.len(), 3, "three drill rows");
    for r in &drill {
        assert!(
            matches!(
                r.subject(),
                Subject::Group {
                    group: CutGroup::Drill,
                    ..
                }
            ),
            "a drill row is in the drill group"
        );
    }
    assert!(
        rows.iter().any(|r| r.tone() == Tone::NotApplicable),
        "the side gates do not apply to a drill cycle"
    );
    for r in &rows {
        check_row("drill", r);
    }
}
