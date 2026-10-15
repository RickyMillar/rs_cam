//! G-OPGOLDEN — every operation keeps its toolpath.
//!
//! # Why this exists
//!
//! The operation-architecture refactor (`planning/op_architecture_2026-10-14/`
//! PLAN.md, wave W0) moves every op adapter onto shared kernels. Each step
//! claims "no change in output". This file is the proof of that claim: it
//! generates every operation in `OperationType::ALL` through the real
//! session path and compares a fingerprint of each result with a blessed
//! file.
//!
//! # The path
//!
//! Each case builds a `ProjectSession` and calls `generate_toolpath`, the
//! door the GUI worker, MCP and the CLI share. So the fingerprint covers the
//! generator, the dressups, the boundary clip and the entry-descent split.
//!
//! # The fixtures
//!
//! `common::op_fixtures`: one fixture for each `GeometryRequirement` kind
//! (a flat board with an outline, two drill targets and a pin; a spherical
//! cap on flat ground, plus a curve for `Both`), and the registry tool. W4
//! (`every_op_conforms_w4.rs`) builds its sessions from the same module.
//!
//! # The arms
//!
//! - `fresh`: every op, default parameters, fresh stock.
//! - A non-default arm for each op with a cheap pattern or mode switch.
//! - `rest`: every op that reads the remaining stock in its adapter
//!   (`ctx.initial_stock`). A prior op cuts part of the stock, the session
//!   simulates it, and the op then generates with `FromRemainingStock`.
//!
//! # The fingerprint
//!
//! Move counts, cut and rapid length, the XYZ bounding box and an FNV-1a
//! hash (`common::fingerprint`) of the quantized moves. The quantum of a
//! coordinate is the finest precision a shipped post emits (the most
//! decimals over the four posts in `gcode::post`). A change below that
//! quantum cannot change a G-code word, and float noise across platforms
//! (about 1e-12 relative) is many decades below it. The hash also covers the
//! move kind, the feed (at the finest feed precision) and the move intent.
//!
//! # Blessing
//!
//! `RS_CAM_BLESS_OPGOLDEN=1` rewrites the golden file. Without it the test
//! compares and prints one line per changed field: op/arm, field, old, new.
//! Re-bless only the ops that a change names on purpose, and say so in the
//! commit body.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test every_op_keeps_its_toolpath_g_opgolden
//! RS_CAM_BLESS_OPGOLDEN=1 cargo test -p rs_cam_core -q --test every_op_keeps_its_toolpath_g_opgolden
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
// The per-field diff and the bless notice ARE the record.
#![allow(clippy::print_stderr)]

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use common::op_fixtures::{
    CaseSpec, Fixture, Frame, Golden, Prior, case_session, run_prior, unknown_intents,
};
use rayon::prelude::*;
use rs_cam_core::compute::catalog::{OperationConfig, OperationType, RestUse};
use rs_cam_core::compute::config::StockSource;
use rs_cam_core::compute::operation_configs::{DrillCycleType, PocketPattern};
use rs_cam_core::ops::profile::ProfileSide;

/// The environment switch that rewrites the golden file.
const BLESS_ENV: &str = "RS_CAM_BLESS_OPGOLDEN";

// ── Cases ───────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
enum Source {
    Fresh,
    Rest,
}

/// A change to an op's default parameters.
type Tweak = fn(&mut OperationConfig);

/// One generated toolpath: an op, an arm name, a parameter change and a
/// stock source.
#[derive(Clone, Copy)]
struct Case {
    op: OperationType,
    arm: &'static str,
    tweak: Tweak,
    source: Source,
}

impl Case {
    fn key(&self) -> String {
        format!("{}/{}", self.op.kind_str(), self.arm)
    }
}

fn no_tweak(_: &mut OperationConfig) {}

fn pocket_zigzag(op: &mut OperationConfig) {
    let OperationConfig::Pocket(cfg) = op else {
        unreachable!("pocket arm on a non-pocket op")
    };
    cfg.pattern = PocketPattern::Zigzag;
}

fn profile_inside(op: &mut OperationConfig) {
    let OperationConfig::Profile(cfg) = op else {
        unreachable!("profile arm on a non-profile op")
    };
    cfg.side = ProfileSide::Inside;
}

fn scallop_continuous(op: &mut OperationConfig) {
    let OperationConfig::Scallop(cfg) = op else {
        unreachable!("scallop arm on a non-scallop op")
    };
    cfg.continuous = !cfg.continuous;
}

fn drill_simple_cycle(op: &mut OperationConfig) {
    let OperationConfig::Drill(cfg) = op else {
        unreachable!("drill arm on a non-drill op")
    };
    cfg.cycle = DrillCycleType::Simple;
}

/// The ops whose adapter reads the remaining stock today
/// (`ctx.initial_stock` in `compute/execute/`): Pocket, Zigzag, Profile,
/// Trace, Face and 2D Adaptive (`clearing_2d.rs`, the Layered rest rules
/// of W3.4), Chamfer (`curve_engrave.rs`, the Layered level skip of W3.5),
/// Drill (`drilling.rs`, the stock column of D8), Adaptive3d,
/// Pencil and Unified Finish (`finish_3d.rs`), the raster link stage of
/// Drop Cutter and Scallop, the contour filter of Waterline
/// (`finish_raster.rs`, W6.1), and Project Curve (`curve_engrave.rs`). Each is an op whose `Capabilities::rest_use`
/// is not `RestUse::None`.
const REST_READERS: &[OperationType] = &[
    OperationType::Pocket,
    OperationType::Zigzag,
    OperationType::Profile,
    OperationType::Trace,
    OperationType::Face,
    OperationType::Chamfer,
    OperationType::Drill,
    OperationType::Adaptive,
    OperationType::Adaptive3d,
    OperationType::Pencil,
    OperationType::UnifiedFinish,
    OperationType::DropCutter,
    OperationType::Scallop,
    OperationType::Waterline,
    OperationType::ProjectCurve,
];

fn cases() -> Vec<Case> {
    let mut cases: Vec<Case> = OperationType::ALL
        .iter()
        .map(|&op| Case {
            op,
            arm: "fresh",
            tweak: no_tweak,
            source: Source::Fresh,
        })
        .collect();
    let variants: [(OperationType, &'static str, Tweak); 4] = [
        (OperationType::Pocket, "zigzag", pocket_zigzag),
        (OperationType::Profile, "inside", profile_inside),
        (
            OperationType::Scallop,
            "continuous_flipped",
            scallop_continuous,
        ),
        (OperationType::Drill, "simple_cycle", drill_simple_cycle),
    ];
    for (op, arm, tweak) in variants {
        cases.push(Case {
            op,
            arm,
            tweak,
            source: Source::Fresh,
        });
    }
    for &op in REST_READERS {
        cases.push(Case {
            op,
            arm: "rest",
            tweak: no_tweak,
            source: Source::Rest,
        });
    }
    cases
}

/// One generated case: its fingerprint and the count of its moves with
/// `MoveIntent::Unknown` (B11).
struct Generated {
    golden: Golden,
    unknown_intents: usize,
}

/// Generate one case and fingerprint it. A refusal is a result too: a
/// refactor must not change it.
fn run(case: &Case) -> Generated {
    let mut op = OperationConfig::new_default(case.op);
    (case.tweak)(&mut op);
    let rest = case.source == Source::Rest;
    let (mut session, index) = case_session(CaseSpec {
        name: &case.key(),
        op,
        prior: if rest { Prior::RestPrior } else { Prior::None },
        stock_source: if rest {
            StockSource::FromRemainingStock
        } else {
            StockSource::Fresh
        },
        frame: Frame::TopUp,
        fixture: Fixture::Standard,
        tool: None,
    });
    if rest {
        run_prior(&mut session);
    }
    let cancel = AtomicBool::new(false);
    match session.generate_toolpath(index, &cancel) {
        Ok(result) => Generated {
            golden: Golden::of(result.toolpath()),
            unknown_intents: unknown_intents(result.toolpath()),
        },
        Err(e) => Generated {
            golden: Golden::refused(&e.to_string()),
            unknown_intents: 0,
        },
    }
}

// ── The golden file ─────────────────────────────────────────────────────

fn golden_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/op_golden_g_opgolden.toml")
}

const GOLDEN_HEADER: &str = "\
# G-OPGOLDEN: blessed toolpath fingerprints of every operation.
# Written by tests/every_op_keeps_its_toolpath_g_opgolden.rs with
# RS_CAM_BLESS_OPGOLDEN=1. Do not edit by hand. Re-bless only the ops a
# change names on purpose, and say so in the commit body.

";

fn write_golden(actual: &BTreeMap<String, Golden>) {
    let body = toml::to_string(actual).expect("the goldens serialize");
    std::fs::write(golden_path(), format!("{GOLDEN_HEADER}{body}")).expect("write the goldens");
}

fn read_golden() -> BTreeMap<String, Golden> {
    let path = golden_path();
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "missing golden file {} ({e}); bless it with {BLESS_ENV}=1",
            path.display()
        )
    });
    toml::from_str(&text).expect("the golden file parses")
}

/// One line per changed field: `op/arm  field  old -> new`.
fn diff(expected: &BTreeMap<String, Golden>, actual: &BTreeMap<String, Golden>) -> Vec<String> {
    let mut lines = Vec::new();
    let keys: BTreeSet<&String> = expected.keys().chain(actual.keys()).collect();
    for key in keys {
        match (expected.get(key), actual.get(key)) {
            (Some(_), None) => lines.push(format!("{key}: in the golden file, not generated")),
            (None, Some(_)) => lines.push(format!("{key}: generated, not in the golden file")),
            (Some(old), Some(new)) if old != new => {
                let old_v = toml::Value::try_from(old).expect("a table");
                let new_v = toml::Value::try_from(new).expect("a table");
                let (Some(old_t), Some(new_t)) = (old_v.as_table(), new_v.as_table()) else {
                    unreachable!("a struct serializes to a table")
                };
                for (field, ov) in old_t {
                    let nv = &new_t[field];
                    if ov != nv {
                        lines.push(format!("{key}  {field}  {ov} -> {nv}"));
                    }
                }
            }
            _ => {}
        }
    }
    lines
}

// ── The test ────────────────────────────────────────────────────────────

#[test]
fn every_op_keeps_its_toolpath() {
    let cases = cases();

    // The case list covers the whole registry.
    let covered: BTreeSet<&str> = cases
        .iter()
        .filter(|c| c.arm == "fresh")
        .map(|c| c.op.kind_str())
        .collect();
    let registered: BTreeSet<&str> = OperationType::ALL.iter().map(|op| op.kind_str()).collect();
    assert_eq!(covered, registered, "every registered op has a fresh arm");

    // W3.4: an op with a rest rule has a rest arm, and no other op has one.
    let with_rest_rule: BTreeSet<&str> = OperationType::ALL
        .iter()
        .filter(|op| op.capabilities().rest_use != RestUse::None)
        .map(|op| op.kind_str())
        .collect();
    let rest_arms: BTreeSet<&str> = REST_READERS.iter().map(|op| op.kind_str()).collect();
    assert_eq!(
        rest_arms, with_rest_rule,
        "the rest arms are the ops whose capabilities state a rest rule"
    );

    let generated: BTreeMap<String, Generated> = cases
        .par_iter()
        .map(|case| (case.key(), run(case)))
        .collect();

    // B11: every move of every arm carries a known intent. A dressup that
    // turns an air row into a rapid tags it; it never leaves `Unknown`.
    let unknown: Vec<(&String, usize)> = generated
        .iter()
        .filter(|(_, g)| g.unknown_intents > 0)
        .map(|(k, g)| (k, g.unknown_intents))
        .collect();
    assert!(
        unknown.is_empty(),
        "these arms emit moves with MoveIntent::Unknown (arm, count): {unknown:?}"
    );

    let actual: BTreeMap<String, Golden> =
        generated.into_iter().map(|(k, g)| (k, g.golden)).collect();

    // Non-vacuity: every op cuts something in its fresh arm, so a golden
    // of an empty toolpath cannot pass for a kept one.
    let empty: Vec<&String> = actual
        .iter()
        .filter(|(k, g)| k.ends_with("/fresh") && g.move_count == 0)
        .map(|(k, _)| k)
        .collect();
    assert!(
        empty.is_empty(),
        "these fresh arms emit no moves, so their golden proves nothing: {empty:?}\n{actual:#?}"
    );

    if std::env::var_os(BLESS_ENV).is_some() {
        write_golden(&actual);
        eprintln!(
            "blessed {} cases into {}",
            actual.len(),
            golden_path().display()
        );
        return;
    }

    let expected = read_golden();
    let lines = diff(&expected, &actual);
    assert!(
        lines.is_empty(),
        "the generated toolpaths moved ({} changes). Re-bless with {BLESS_ENV}=1 only for an \
         intended change:\n{}",
        lines.len(),
        lines.join("\n")
    );
}

// ── Instrument: the rest arms' moves and time ──────────────────────────

/// One arm of the instrument: the move count, the simulated runtime (s),
/// and the simulated entry time (s) of the case op, or `None` when the op
/// refuses.
fn timed_arm(op_type: OperationType, source: StockSource) -> Option<(usize, f64, f64)> {
    let (mut session, index) = case_session(CaseSpec {
        name: "timed",
        op: OperationConfig::new_default(op_type),
        prior: Prior::RestPrior,
        stock_source: source,
        frame: Frame::TopUp,
        fixture: Fixture::Standard,
        tool: None,
    });
    // The simulator integrates a runtime only with a kinematics block. The
    // generic block is the one `effective_kinematics` reads without it, so
    // the generated moves do not change.
    let _ = session
        .apply(rs_cam_core::session::Command::SetMachineKinematics(
            rs_cam_core::session::SetMachineKinematicsArgs {
                kinematics: Box::new(
                    rs_cam_core::machine::kinematics::MachineKinematics::generic_wood_router(),
                ),
            },
        ))
        .expect("the kinematics block");
    run_prior(&mut session);
    let cancel = AtomicBool::new(false);
    let result = session.generate_toolpath(index, &cancel).ok()?;
    let moves = result.toolpath().moves.len();
    common::op_fixtures::simulate(&mut session);
    let id = session.toolpath_configs()[index].id;
    let breakdown = session
        .simulation_result()
        .and_then(|sim| sim.cut_trace.as_deref())
        .and_then(|t| t.toolpath_runtimes.iter().find(|r| r.toolpath_id == id))
        .map(|r| r.breakdown)?;
    Some((moves, breakdown.total_s, breakdown.entry_s))
}

/// Instrument (not a gate): for each rest arm and its fresh twin (the same
/// session with the prior op, `StockSource::Fresh`), the move count, the
/// simulated runtime and the simulated entry time. Run it by name:
///
/// ```text
/// cargo test -p rs_cam_core -q --test every_op_keeps_its_toolpath_g_opgolden \
///     -- --ignored rest_arm_times --nocapture
/// ```
#[test]
#[ignore = "instrument: prints the rest arms' moves and times"]
fn rest_arm_times() {
    let rows: Vec<String> = REST_READERS
        .par_iter()
        .map(|&op_type| {
            let fmt = |a: Option<(usize, f64, f64)>| match a {
                Some((n, sim, entry)) => {
                    format!("moves {n:>6} sim {sim:>8.2} s (entry {entry:>7.2} s)")
                }
                None => "refused".to_owned(),
            };
            format!(
                "{:<16} rest: {} | fresh: {}",
                op_type.kind_str(),
                fmt(timed_arm(op_type, StockSource::FromRemainingStock)),
                fmt(timed_arm(op_type, StockSource::Fresh)),
            )
        })
        .collect();
    for row in rows {
        eprintln!("{row}");
    }
}
