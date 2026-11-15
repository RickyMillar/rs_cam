//! P32 (operator rulings 2026-11-15): feed modulation has three settings,
//! and the force budget sets no feed while it is advisory.
//!
//! 1. **Cap only is the default and never raises a feed.** After a
//!    simulation, for EVERY move of the result (cutting, entries, air
//!    leads, leads, links) the feed is at or below the feed that the
//!    dressed path gave that move. A second simulation holds it too (the
//!    run reads the dressed path, not the feeds of the first run). The cell
//!    states a feed over the chip maximum, so the run lowers moves
//!    (non-vacuity), and no move binds on the force budget.
//! 2. **A result inside every limit takes the modulated tag in cap only.**
//!    No feed changes, the stage is current, the rows read the modulation
//!    of the run (not "until modulated"), and the header says "capped".
//! 3. **Full can raise a feed**, and the warning names the modulation.
//! 4. **Off changes nothing.**
//! 5. **A change of mode, for every pair of different settings, drops the
//!    simulation and puts the dressed path back** in a modulated result.
//! 6. **The floor.** With a spindle too weak for the cut, no load cap takes
//!    a move below `min(rubbing floor x rpm x Z, dressed feed)`. The held
//!    moves are counted in the record, the export warns once
//!    (`export.feed_held_at_floor`), and the "Feeds that run" record holds
//!    the line. Cap only and full.
//!
//! 7. **The baseline of each run is the dressed path.** A cap-only run at
//!    feed scale 0.5, then a run at feed scale 1 with no edit between, give
//!    the feeds of a new session that runs once at feed scale 1. A run that
//!    reads the feeds in the slot cannot go back up.
//!
//! The cell is the review cell P_thin_005 of
//! `the_session_modulator_binds_on_the_force_budget_p16` (a deep, wide
//! Pocket slot), where the force budget bound moves before P32.
//!
//! # Red on a defect (each injected once, 2026-11-15)
//!
//! - Cap only made to raise (the clamp of the modulator loop and the clamp
//!   of the solver both removed; each alone holds the rule): claim 2 is red
//!   (a move of the slow cell runs above its dressed feed). Claim 1 stays
//!   green: no limit of the fast cell is above its dressed feeds.
//! - `Basis::force_budget_bound` changed to a basis that gates: claim 1 is
//!   red on the force-budget binding, and so is
//!   `the_session_modulator_binds_on_the_force_budget_p16`.
//! - The rubbing floor removed from `LoadArm::push_into`: claim 6 is red in
//!   the full mode (cap only holds the floor a second time in the solver).
//! - `set_feed_modulation_impl` restores only for off: claims 2, 5 and 6
//!   are red.
//!
//! - The session modulates the path in the slot, not the dressed path
//!   (`annotated_arc = slot_arc`): claim 7 is red.
//!
//! `cargo test -p rs_cam_core -q --test feed_modulation_has_three_modes_p32`

#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use std::collections::BTreeSet;
use std::sync::atomic::AtomicBool;

use common::session::{polygon_model, single_op_session_with, square_polygon, stock_under};
use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolFact, ToolId, ToolType};
use rs_cam_core::diagnostics::ids;
use rs_cam_core::dressup::feed_modulation::{BindingConstraint, ModulationMode, RunModulation};
use rs_cam_core::limits::ExportFeed;
use rs_cam_core::machine::{MachineProfile, PowerModel};
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::session::{
    Command, FeedModulationSwitch, FeedStage, ProjectSession, SetFeedModulationArgs,
    SetMachineArgs, SimulationDropCause, SimulationOptions, SimulationResolution,
};
use rs_cam_core::toolpath::MoveIntent;

/// The review cell P_thin_005: a 6 mm two-flute flat end mill, one 6 mm
/// pass at a 5 mm stepover (the first ring is a slot).
const D_MM: f64 = 6.0;
const FLUTES: u32 = 2;
const DEPTH_MM: f64 = 6.0;
const STEPOVER_MM: f64 = 5.0;
const PLUNGE_MM_MIN: f64 = 2999.0;
const RPM: u32 = 10_296;
const HALF_MM: f64 = 20.0;
const STOCK_Z_MM: f64 = 12.0;
const RESOLUTION_MM: f64 = 0.5;
/// The stated feed of the fast cell: over the chip maximum of the row.
const FAST_FEED: f64 = 5999.0;
/// The stated feed of the slow cell: under every limit of the cut.
const SLOW_FEED: f64 = 600.0;

fn tool() -> ToolConfig {
    let mut t = ToolConfig::test_default(ToolId::from_raw(1), ToolType::EndMill);
    t.diameter = D_MM;
    t.set_flute_count(ToolFact::Stated(FLUTES));
    t.set_cutting_length(ToolFact::Stated(20.0));
    t.set_shank_diameter(ToolFact::Stated(D_MM));
    t.shaft_diameter = D_MM;
    t.set_stickout(ToolFact::Stated(28.0));
    t
}

/// The cell at `feed`, generated, with feed optimisation off (so the
/// dressed feeds are the stated feeds) and the switch at `switch`.
fn cell(feed: f64, switch: FeedModulationSwitch) -> ProjectSession {
    let stock = StockConfig {
        material: Material::SolidWood {
            species: WoodSpecies::GenericHardwood,
        },
        ..stock_under(HALF_MM, STOCK_Z_MM)
    };
    let op = OperationConfig::Pocket(PocketConfig {
        stepover: Some(STEPOVER_MM),
        floor: rs_cam_core::compute::floor::Floor::top(DEPTH_MM),
        depth_per_pass: Some(DEPTH_MM),
        feed_rate: Some(feed),
        plunge_rate: Some(PLUNGE_MM_MIN.min(feed)),
        spindle_rpm: Some(RPM),
        ..PocketConfig::default()
    });
    let mut session = single_op_session_with(
        stock,
        tool(),
        polygon_model(vec![square_polygon(HALF_MM)], "p32_square"),
        "Pocket",
        op,
        |tc| tc.dressups.feed_optimization = false,
    );
    let _ = set_switch(&mut session, switch);
    session
        .generate_toolpath(0, &AtomicBool::new(false))
        .expect("the pocket generates");
    session
}

fn set_switch(session: &mut ProjectSession, switch: FeedModulationSwitch) -> BTreeSet<usize> {
    session
        .apply(Command::SetFeedModulation(SetFeedModulationArgs { switch }))
        .expect("the command applies")
        .modulated
}

fn simulate(session: &mut ProjectSession) {
    session
        .run_simulation(
            &SimulationOptions {
                resolution: SimulationResolution::Fixed(RESOLUTION_MM),
                ..SimulationOptions::default()
            },
            &AtomicBool::new(false),
        )
        .expect("the session simulates");
}

/// The feed and the intent of each move of the stored result (`None` for a
/// rapid).
fn feeds(session: &ProjectSession) -> Vec<(Option<f64>, MoveIntent)> {
    session
        .result_at(0)
        .expect("a result")
        .toolpath()
        .moves
        .iter()
        .map(|m| (m.move_type.feed_rate(), m.intent))
        .collect()
}

/// The count of moves above and below their dressed feed. It panics when
/// the two paths do not pair move for move.
fn against(
    dressed: &[(Option<f64>, MoveIntent)],
    now: &[(Option<f64>, MoveIntent)],
) -> (usize, usize) {
    assert_eq!(dressed.len(), now.len(), "the modulation keeps every move");
    let (mut above, mut below) = (0, 0);
    for ((d, d_intent), (n, n_intent)) in dressed.iter().zip(now) {
        assert_eq!(d_intent, n_intent);
        match (d, n) {
            (Some(d), Some(n)) => {
                above += usize::from(*n > *d + 1e-9);
                below += usize::from(*n < *d - 1e-9);
            }
            (None, None) => {}
            (d, n) => panic!("a move changed its type: {d:?} to {n:?}"),
        }
    }
    (above, below)
}

fn bindings(session: &ProjectSession) -> Vec<(usize, f64, BindingConstraint)> {
    let trace = session
        .simulation_result()
        .and_then(|s| s.cut_trace.clone())
        .expect("a cut trace");
    trace
        .modulated_feeds
        .iter()
        .map(|((_, index), (feed, binding))| (*index, *feed, *binding))
        .collect()
}

fn export(session: &ProjectSession) -> rs_cam_core::gcode::ExportedGcode {
    let trace = session
        .simulation_result()
        .and_then(|s| s.cut_trace.clone());
    session
        .export_gcode_checked(trace.as_deref())
        .expect("a warning never refuses the export")
}

#[test]
fn cap_only_is_the_default_and_never_raises_a_feed_p32() {
    assert_eq!(
        FeedModulationSwitch::default(),
        FeedModulationSwitch::CapOnly
    );
    assert_eq!(
        ProjectSession::new_empty().feed_modulation(),
        FeedModulationSwitch::CapOnly
    );
    let mut session = cell(FAST_FEED, FeedModulationSwitch::default());
    let dressed = feeds(&session);
    let classes: Vec<_> = rs_cam_core::session::FeedClass::ALL
        .into_iter()
        .filter(|class| {
            dressed.iter().any(|(feed, intent)| {
                feed.is_some() && rs_cam_core::session::FeedClass::of(*intent) == *class
            })
        })
        .collect();
    assert!(
        classes.len() >= 2,
        "non-vacuity: more than one class: {classes:?}"
    );

    for run in 1..=2 {
        simulate(&mut session);
        let now = feeds(&session);
        let (above, below) = against(&dressed, &now);
        assert_eq!(above, 0, "run {run}: cap only raises no feed of any class");
        assert!(below > 0, "run {run}, non-vacuity: a cap lowers a move");
        assert!(matches!(
            session.feed_stage(0),
            Some(FeedStage::ModulatedCurrent { .. })
        ));
        // Ruling 2: the advisory force budget sets no feed.
        let tags = bindings(&session);
        assert!(!tags.is_empty(), "non-vacuity: the run visited moves");
        assert!(
            tags.iter()
                .all(|(_, _, b)| *b != BindingConstraint::ForceBudget),
            "run {run}: no move binds on the force budget"
        );
    }
    let record = session.emitted_feeds(0).expect("a record");
    assert_eq!(record.modulation_mode, Some(ModulationMode::CapOnly));
    assert_eq!(
        record.above_stated_warning(),
        None,
        "no generated feed is above the stated feed, and cap only raises none"
    );
    assert!(
        export(&session)
            .warnings
            .iter()
            .all(|w| w.id != ids::EXPORT_FEED_ABOVE_STATED)
    );
}

#[test]
fn a_cut_inside_every_limit_is_capped_with_no_change_p32() {
    let mut session = cell(SLOW_FEED, FeedModulationSwitch::CapOnly);
    let dressed = feeds(&session);
    simulate(&mut session);
    assert_eq!(against(&dressed, &feeds(&session)), (0, 0), "no feed moves");
    let run = session.simulation_run().expect("a run");
    assert_eq!(
        session.feed_stage(0),
        Some(FeedStage::ModulatedCurrent {
            simulation_run: run
        }),
        "the run judged the result, so the result holds its modulation"
    );
    assert!(
        matches!(session.run_modulation_of(0), RunModulation::Current { .. }),
        "the rows read the modulation of the run, not \"until modulated\""
    );
    let exported = export(&session);
    assert!(
        exported
            .gcode
            .lines()
            .any(|l| l.trim()
                == format!("(feeds: capped from simulation run {run}, no feed raised)")),
        "the stage line of a cap-only result"
    );
    let record = session.emitted_feeds(0).expect("a record");
    assert!(
        record.line().ends_with(&format!("capped, run {run}")),
        "{}",
        record.line()
    );

    // Claim 3: full raises moves of the same cell, and the warning names
    // the modulation.
    let restored = set_switch(&mut session, FeedModulationSwitch::Full);
    assert_eq!(restored, BTreeSet::from([0]));
    simulate(&mut session);
    let (above, _) = against(&dressed, &feeds(&session));
    assert!(above > 0, "full raises a move above its dressed feed");
    let record = session.emitted_feeds(0).expect("a record");
    assert_eq!(record.modulation_mode, Some(ModulationMode::Full));
    let run = session.simulation_run().expect("a run");
    let text = record.above_stated_warning().expect("full warns");
    assert!(
        text.ends_with(&format!("; feed modulation of simulation run {run}")),
        "{text}"
    );
}

#[test]
fn off_changes_no_feed_p32() {
    let mut session = cell(FAST_FEED, FeedModulationSwitch::Off);
    let dressed = feeds(&session);
    simulate(&mut session);
    assert_eq!(feeds(&session), dressed);
    assert_eq!(session.feed_stage(0), Some(FeedStage::Planned));
    assert_eq!(session.run_modulation_of(0), RunModulation::Off);
    let record = session.emitted_feeds(0).expect("a record");
    assert_eq!(record.modulation_mode, None);
    assert!(record.held.is_empty());
}

#[test]
fn a_change_of_mode_puts_the_dressed_path_back_p32() {
    use FeedModulationSwitch::{CapOnly, Full, Off};
    for (from, to) in [
        (CapOnly, Full),
        (CapOnly, Off),
        (Full, CapOnly),
        (Full, Off),
        (Off, CapOnly),
        (Off, Full),
    ] {
        let mut session = cell(FAST_FEED, from);
        let dressed = feeds(&session);
        simulate(&mut session);
        let modulated = feeds(&session) != dressed;
        assert_eq!(modulated, from != Off, "{from:?}: non-vacuity");
        let epoch = session.simulation_epoch();

        let restored = set_switch(&mut session, to);
        assert_eq!(session.feed_modulation(), to);
        assert_eq!(
            restored,
            if modulated {
                BTreeSet::from([0])
            } else {
                BTreeSet::new()
            },
            "{from:?} to {to:?}"
        );
        assert!(
            session.simulation_result().is_none(),
            "the simulation drops"
        );
        assert!(
            session
                .simulation_drop_causes_since(epoch)
                .contains(SimulationDropCause::FeedModulation)
        );
        assert_eq!(
            feeds(&session),
            dressed,
            "{from:?} to {to:?}: the dressed feeds"
        );
        assert_eq!(session.feed_stage(0), Some(FeedStage::Planned));
        assert_eq!(
            session.run_modulation_of(0),
            match to {
                Off => RunModulation::Off,
                CapOnly | Full => RunModulation::NotRun {
                    export: ExportFeed::Stated
                },
            },
            "{from:?} to {to:?}"
        );

        // The next simulation runs under the new setting, from the dressed
        // feeds.
        simulate(&mut session);
        let (above, below) = against(&dressed, &feeds(&session));
        match to {
            Off => assert_eq!((above, below), (0, 0)),
            CapOnly => assert!(
                above == 0 && below > 0,
                "{from:?} to cap only: {above} above"
            ),
            Full => assert!(below > 0, "{from:?} to full: the chip cap lowers moves"),
        }

        // The same value again changes nothing and drops nothing.
        assert!(set_switch(&mut session, to).is_empty());
        assert!(session.simulation_result().is_some());
    }
}

#[test]
fn a_move_held_at_the_rubbing_floor_is_counted_and_warned_p32() {
    for switch in [FeedModulationSwitch::CapOnly, FeedModulationSwitch::Full] {
        let mut session = cell(FAST_FEED, switch);
        // A spindle far too weak for a 6 mm slot in hardwood.
        let weak = MachineProfile {
            power: PowerModel::ConstantPower { power_kw: 0.05 },
            ..session.machine().clone()
        };
        let _ = session
            .apply(Command::SetMachine(SetMachineArgs {
                machine: Box::new(weak),
            }))
            .expect("the machine applies");
        session
            .generate_toolpath(0, &AtomicBool::new(false))
            .expect("the pocket generates");
        let dressed = feeds(&session);
        simulate(&mut session);
        let now = feeds(&session);

        let trace = session
            .simulation_result()
            .and_then(|s| s.cut_trace.clone())
            .expect("a cut trace");
        let id = session.toolpath_configs()[0].id;
        let band = rs_cam_core::tool_load::modulation_bands_for_session(&session, Some(&trace))
            .get(&id)
            .copied()
            .expect("the cell has a chip band");
        // The lift is at least 1, so this is the lowest rubbing floor feed.
        let floor = band.rubbing_floor_mm_per_tooth() * f64::from(RPM) * f64::from(FLUTES);
        assert!(floor > 100.0, "fixture: a real floor ({floor})");

        let mut power_bound = 0;
        for (index, feed, binding) in bindings(&session) {
            if binding != BindingConstraint::PowerMax {
                continue;
            }
            power_bound += 1;
            let dressed_feed = dressed[index].0.expect("a fed move");
            assert!(
                feed >= floor.min(dressed_feed) - 1e-6,
                "{switch:?}: move {index} at {feed} is below its floor {floor} (dressed \
                 {dressed_feed})"
            );
            assert_eq!(now[index].0, Some(feed));
        }
        assert!(
            power_bound > 0,
            "{switch:?}, non-vacuity: the power binds moves"
        );

        let record = session.emitted_feeds(0).expect("a record");
        assert!(
            record.held.moves > 0 && record.held.power == record.held.moves,
            "{switch:?}: the held moves are counted: {:?}",
            record.held
        );
        let run = session.simulation_run().expect("a run");
        let text = record.held_warning().expect("the record warns");
        assert!(
            text.starts_with(&format!(
                "{} moves held at the rubbing floor stay over a load limit: spindle power {}",
                record.held.moves, record.held.power
            )) && text.ends_with(&format!("; simulation run {run}")),
            "{text}"
        );

        let exported = export(&session);
        let held: Vec<_> = exported
            .warnings
            .iter()
            .filter(|w| w.id == ids::EXPORT_FEED_HELD_AT_FLOOR)
            .collect();
        assert_eq!(held.len(), 1, "{switch:?}: one warning per op");
        assert_eq!(held[0].detail, format!("Toolpath 'Pocket': {text}"));
        assert!(
            exported
                .gcode
                .lines()
                .any(|l| l.contains("WARNING") && l.contains(ids::EXPORT_FEED_HELD_AT_FLOOR)),
            "{switch:?}: the header names the warning"
        );
        assert!(
            exported
                .gcode
                .lines()
                .any(|l| l.trim() == format!("({text})")),
            "{switch:?}: the feed record of the header holds the line"
        );
        let report =
            rs_cam_core::session::emitted_feeds_report(&exported.emitted_feeds).expect("a report");
        assert!(report.contains(&text), "{report}");

        // A change of mode restores the dressed path and the count goes.
        let other = match switch {
            FeedModulationSwitch::CapOnly => FeedModulationSwitch::Full,
            FeedModulationSwitch::Full | FeedModulationSwitch::Off => FeedModulationSwitch::CapOnly,
        };
        let _ = set_switch(&mut session, other);
        assert!(session.emitted_feeds(0).expect("a record").held.is_empty());
        assert!(
            export(&session)
                .warnings
                .iter()
                .all(|w| w.id != ids::EXPORT_FEED_HELD_AT_FLOOR)
        );
    }
}

fn simulate_at_scale(session: &mut ProjectSession, feed_scale: f64) {
    session
        .run_simulation(
            &SimulationOptions {
                resolution: SimulationResolution::Fixed(RESOLUTION_MM),
                modulation_feed_scale: feed_scale,
                ..SimulationOptions::default()
            },
            &AtomicBool::new(false),
        )
        .expect("the session simulates");
}

#[test]
fn each_run_reads_the_dressed_path_p32() {
    let mut once = cell(FAST_FEED, FeedModulationSwitch::CapOnly);
    let dressed = feeds(&once);
    simulate_at_scale(&mut once, 1.0);
    let at_one = feeds(&once);

    let mut twice = cell(FAST_FEED, FeedModulationSwitch::CapOnly);
    simulate_at_scale(&mut twice, 0.5);
    let at_half = feeds(&twice);
    let (above, below) = against(&at_one, &at_half);
    assert!(
        above == 0 && below > 0,
        "non-vacuity: the run at feed scale 0.5 gives lower feeds ({above} above, {below} below)"
    );
    simulate_at_scale(&mut twice, 1.0);
    assert_eq!(
        feeds(&twice),
        at_one,
        "the second run reads the dressed feeds, not the feeds of the first run"
    );
    assert_eq!(against(&dressed, &feeds(&twice)).0, 0, "cap only");
}
