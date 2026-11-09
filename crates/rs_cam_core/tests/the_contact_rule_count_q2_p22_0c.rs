//! P22-0c-4 (operator ruling Q2): the count that decides the rule of the
//! contact zones at or above the flute top on a fed move.
//!
//! For each project of `planning/fixtures/rivmap100/` (the roughing
//! benchmark and its arms), the instrument walks the generation plan as
//! the CLI `project` command does (`generation_plan::plan`, a generate per
//! step, a simulation per `Simulate` step), then runs the closing
//! simulation twice: with the pre-move rule and with the in-move rule
//! (`stock::contact::override_rule_for_count`). The stock and the moves
//! are the same in both arms: the rule changes only what the checks
//! record. It writes one table per project to
//! `$CARGO_TARGET_TMPDIR/q2_contact_rule_count.md`: the flute-top and
//! body contacts of each arm, and each report the in-move rule removes
//! with the descent of its move.
//!
//! `RS_CAM_Q2_PROJECTS` (comma-separated file names) limits the run.
//!
//! `cargo test -p rs_cam_core --test the_contact_rule_count_q2_p22_0c -- --ignored --nocapture`

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::sync::atomic::AtomicBool;

use rs_cam_core::session::generation_plan::{self, Scope, Step};
use rs_cam_core::session::{ProjectSession, SimulationOptions, SimulationResolution};
use rs_cam_core::stock::contact::{ContactRule, override_rule_for_count};

/// The projects of the count. `rivmap100_memory_repro.toml` is left out:
/// its README says to run it only under a memory cap.
const PROJECTS: &[&str] = &[
    "rivmap100.toml",
    "rivmap100_live_0925.toml",
    "rivmap100_live_0925_notsp.toml",
    "rivmap100_ladder_demo.toml",
    "rivmap100_single_step.toml",
    "rivmap100_memory_small.toml",
    "rivmap100_tiered_finish.toml",
    "rivmap100_boundary_strike.toml",
];

/// One report of an arm: toolpath id, move index, zone.
type Report = (usize, usize, String);

fn reports(session: &ProjectSession) -> BTreeSet<Report> {
    let sim = session.simulation_result().expect("a simulation");
    let mut out = BTreeSet::new();
    for hit in &sim.flute_overreach {
        out.insert((
            hit.toolpath_id.raw(),
            hit.overreach.move_index,
            "flute_top".to_owned(),
        ));
    }
    for hit in &sim.body_contacts {
        out.insert((
            hit.toolpath_id.raw(),
            hit.contact.move_index,
            format!("{} {:?}", hit.contact.zone_text(), hit.contact.motion),
        ));
    }
    out
}

fn options(session: &ProjectSession) -> SimulationOptions {
    SimulationOptions {
        resolution: SimulationResolution::Fixed(session.simulation_resolution_mm()),
        skip_ids: Vec::new(),
        use_predicted_feed_in_gates: true,
        adaptive_feed_modulation: false,
        modulation_strategy:
            rs_cam_core::dressup::feed_modulation::ModulationStrategy::ConstrainedMax,
        modulation_feed_scale: 1.0,
    }
}

/// The descent text of move `move_index` of toolpath `id`.
fn descent(session: &ProjectSession, id: usize, move_index: usize) -> String {
    let Some(index) = (0..session.toolpath_count()).find(|&i| {
        session
            .toolpath_config_at(i)
            .is_some_and(|tc| tc.id.raw() == id)
    }) else {
        return "no toolpath".to_owned();
    };
    let Some(result) = session.result_at(index) else {
        return "no result".to_owned();
    };
    let moves = &result.toolpath().moves;
    if move_index == 0 || move_index >= moves.len() {
        return "no move".to_owned();
    }
    let (a, b) = (moves[move_index - 1].target, moves[move_index].target);
    let dxy = (b.x - a.x).hypot(b.y - a.y);
    let dz = b.z - a.z;
    format!(
        "{:?} {:?}: dz {dz:.3} mm over {dxy:.3} mm ({:.2} deg), from Z{:.3} to Z{:.3}",
        moves[move_index].move_type,
        moves[move_index].intent,
        (-dz).atan2(dxy).to_degrees(),
        a.z,
        b.z
    )
}

#[test]
#[ignore = "evidence: P22-0c-4 Q2 count on the planning/fixtures projects (minutes per project)"]
fn the_contact_rule_count_on_the_fixture_projects() {
    let dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../planning/fixtures/rivmap100");
    let only: Option<Vec<String>> = std::env::var("RS_CAM_Q2_PROJECTS")
        .ok()
        .map(|v| v.split(',').map(str::to_owned).collect());
    let mut table = String::from("# P22-0c-4 Q2 count\n\n");
    let cancel = AtomicBool::new(false);
    for name in PROJECTS {
        if only.as_ref().is_some_and(|o| !o.iter().any(|n| n == name)) {
            continue;
        }
        let path = dir.join(name);
        let mut session = match ProjectSession::load(&path) {
            Ok(session) => session,
            Err(error) => {
                // A fixture the loader refuses today is a row of the
                // table, not a failure of the count.
                let _ = writeln!(table, "## {name}\n\nnot loaded: {error}\n");
                continue;
            }
        };
        override_rule_for_count(Some(ContactRule::PreMove));
        for step in generation_plan::plan(&session, Scope::Project) {
            match step {
                Step::Simulate { .. } => {
                    let opts = options(&session);
                    session
                        .run_simulation(&opts, &cancel)
                        .expect("the plan simulation runs");
                }
                Step::Generate { index, .. } => {
                    // A refused op is the project's state; the count reads
                    // the toolpaths that generate.
                    let _ = session.generate_toolpath(index, &cancel);
                }
            }
        }
        let opts = options(&session);
        session
            .run_simulation(&opts, &cancel)
            .expect("the closing simulation runs");
        let pre = reports(&session);
        override_rule_for_count(Some(ContactRule::InMove));
        session
            .run_simulation(&opts, &cancel)
            .expect("the in-move simulation runs");
        let inm = reports(&session);
        override_rule_for_count(None);

        let generated: Vec<String> = (0..session.toolpath_count())
            .filter_map(|i| {
                let tc = session.toolpath_config_at(i)?;
                let result = session.result_at(i)?;
                Some(format!(
                    "TP{} '{}' {} moves",
                    tc.id.raw(),
                    tc.name,
                    result.toolpath().moves.len()
                ))
            })
            .collect();
        let simulated = session.simulation_result().map_or(0, |s| s.total_moves);
        let _ = writeln!(
            table,
            "## {name} (setup)\n\nresults: {}; simulated moves {simulated}.\n",
            generated.join(", ")
        );
        let count = |set: &BTreeSet<Report>, zone: &str| {
            set.iter().filter(|r| r.2.starts_with(zone)).count()
        };
        let _ = writeln!(
            table,
            "## {name}\n\ncell {} mm. pre-move: {} reports (flute_top {}, other {}). in-move: {} \
             reports (flute_top {}, other {}).\n",
            session.simulation_resolution_mm(),
            pre.len(),
            count(&pre, "flute_top"),
            pre.len() - count(&pre, "flute_top"),
            inm.len(),
            count(&inm, "flute_top"),
            inm.len() - count(&inm, "flute_top"),
        );
        for r in pre.difference(&inm) {
            let _ = writeln!(
                table,
                "- removed: TP{} move {} {}: {}",
                r.0,
                r.1,
                r.2,
                descent(&session, r.0, r.1)
            );
        }
        for r in inm.difference(&pre) {
            let _ = writeln!(
                table,
                "- added: TP{} move {} {}: {}",
                r.0,
                r.1,
                r.2,
                descent(&session, r.0, r.1)
            );
        }
        for r in pre.intersection(&inm) {
            let _ = writeln!(
                table,
                "- kept: TP{} move {} {}: {}",
                r.0,
                r.1,
                r.2,
                descent(&session, r.0, r.1)
            );
        }
        table.push('\n');
        write_table(&table);
    }
    write_table(&table);
}

fn write_table(table: &str) {
    let out = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("q2_contact_rule_count.md");
    std::fs::write(&out, table).expect("the table writes");
}
