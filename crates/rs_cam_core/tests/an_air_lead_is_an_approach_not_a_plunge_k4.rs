//! K4 (feeds contract P3): the air lead above an entry is
//! `MoveIntent::EntryApproach`, not `EntryPlunge`.
//!
//! # The claim
//!
//! - A dressup ramp and a dressup helix over a flat stock top: the first
//!   fed move of the entry is `EntryApproach`, it is vertical, and it ends
//!   at or above the stock top plus `entry_clearance_mm` (air only). The
//!   ramp or the helix starts there.
//! - The Pencil crease entry on rest stock (the G-OPGOLDEN `pencil/rest`
//!   arm): each vertical air descent before the crease ramp is
//!   `EntryApproach`, and it ends at or above every lap of the ramp that
//!   follows it (the ladder starts at the stock ceiling). No fed
//!   `EntryPlunge` leads a crease ramp.
//!
//! Red before P3: the four air leads were `EntryPlunge`, and the intent
//! `EntryApproach` did not exist.

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use std::sync::atomic::AtomicBool;

use common::op_fixtures::{CaseSpec, Fixture, Frame, Prior, case_session, run_prior};
use rs_cam_core::compute::catalog::{EntryContract, OperationConfig, OperationType};
use rs_cam_core::compute::config::{DressupConfig, DressupEntryStyle, StockSource};
use rs_cam_core::compute::execute::{DressupContext, apply_dressups};
use rs_cam_core::geo::P3;
use rs_cam_core::toolpath::{Move, MoveIntent, MoveType, Toolpath};
use rs_cam_core::trace::toolpath_spans::AnnotatedToolpath;
use rs_cam_core::trace::transform_provenance::ReconcileSet;

/// The retract plane (mm).
const SAFE_Z: f64 = 20.0;
/// The stock top (mm).
const STOCK_TOP: f64 = 0.0;
/// The cut depth (mm).
const DEPTH: f64 = -3.0;
/// The cut feed (mm/min).
const FEED: f64 = 1200.0;

/// A rapid over the entry, a plunge to the depth, then a cut.
fn plunging_toolpath() -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to_with_intent(P3::new(0.0, 0.0, SAFE_Z), MoveIntent::Linking);
    tp.feed_to_with_intent(P3::new(0.0, 0.0, DEPTH), FEED, MoveIntent::EntryPlunge);
    tp.feed_to_with_intent(P3::new(30.0, 0.0, DEPTH), FEED, MoveIntent::ClearingCut);
    tp
}

fn dress(style: DressupEntryStyle) -> (Toolpath, f64) {
    let cfg = DressupConfig {
        entry_style: style,
        ..DressupConfig::default()
    };
    let clearance = cfg.entry_clearance_mm;
    let dressed = apply_dressups(
        AnnotatedToolpath::new(plunging_toolpath()),
        DressupContext {
            material: &rs_cam_core::material::Material::default(),
            cfg: &cfg,
            nominal_feed_rate: FEED,
            plunge_rate_mm_min: None,
            ramp_feed_rate_mm_min: None,
            tool_diameter: 6.0,
            safe_z: SAFE_Z,
            stock_top: STOCK_TOP,
            prior_stock: None,
            link_kinematics: None,
            op_cutting_ceiling: None,
            feed_opt_stock: None,
            cutter: None,
            entry_surface: None,
            transform_capabilities: OperationType::Pocket.capabilities().transform,
            entry: EntryContract::ANY_DRESSUP_FROM_ROLE,
            debug_ctx: None,
            semantic_ctx: None,
            entry_containment: None,
        },
        &mut ReconcileSet::empty(),
    );
    (dressed.toolpath, clearance)
}

fn fed(m: &Move) -> bool {
    m.move_type != MoveType::Rapid
}

/// The first fed move with an entry intent, and its index.
fn first_entry_move(tp: &Toolpath) -> (usize, &Move) {
    tp.moves
        .iter()
        .enumerate()
        .find(|(_, m)| {
            fed(m)
                && matches!(
                    m.intent,
                    MoveIntent::EntryApproach
                        | MoveIntent::EntryPlunge
                        | MoveIntent::EntryRamp
                        | MoveIntent::EntryHelix
                )
        })
        .expect("the dressed toolpath has an entry")
}

#[test]
fn a_ramp_and_a_helix_start_with_an_approach_through_air() {
    for (style, body) in [
        (DressupEntryStyle::Ramp, MoveIntent::EntryRamp),
        (DressupEntryStyle::Helix, MoveIntent::EntryHelix),
    ] {
        let (tp, clearance) = dress(style);
        // Non-vacuity: the entry style acts.
        assert!(
            tp.moves.iter().any(|m| m.intent == body),
            "{style:?}: the dressup emits no {body:?} move"
        );
        let (i, lead) = first_entry_move(&tp);
        assert_eq!(
            lead.intent,
            MoveIntent::EntryApproach,
            "{style:?}: the first fed move of the entry is {:?}, not EntryApproach",
            lead.intent
        );
        let before = tp.moves[i - 1].target;
        assert!(
            (lead.target.x - before.x).abs() < 1e-9 && (lead.target.y - before.y).abs() < 1e-9,
            "{style:?}: the air lead is not vertical"
        );
        assert!(
            lead.target.z >= STOCK_TOP + clearance - 1e-9,
            "{style:?}: the air lead ends at Z {} below the stock top {STOCK_TOP} + the entry \
             clearance {clearance}",
            lead.target.z
        );
        assert_eq!(
            tp.moves[i + 1].intent,
            body,
            "{style:?}: the {body:?} does not start where the air lead ends"
        );
    }
}

#[test]
fn a_pencil_crease_ramp_starts_with_an_approach_through_air() {
    let (mut session, index) = case_session(CaseSpec {
        name: "pencil/rest",
        op: OperationConfig::new_default(OperationType::Pencil),
        prior: Prior::RestPrior,
        stock_source: StockSource::FromRemainingStock,
        frame: Frame::TopUp,
        fixture: Fixture::Standard,
        tool: None,
    });
    run_prior(&mut session);
    let cancel = AtomicBool::new(false);
    let result = session
        .generate_toolpath(index, &cancel)
        .expect("the pencil rest arm generates");
    let moves = &result.toolpath().moves;
    let mut approaches = 0;
    for (i, m) in moves.iter().enumerate() {
        let next = moves.get(i + 1);
        let leads_a_ramp = next.is_some_and(|n| n.intent == MoveIntent::EntryRamp);
        let vertical_from_prev = i > 0 && {
            let p = moves[i - 1].target;
            (m.target.x - p.x).abs() < 1e-9 && (m.target.y - p.y).abs() < 1e-9 && m.target.z < p.z
        };
        assert!(
            !(fed(m) && m.intent == MoveIntent::EntryPlunge && leads_a_ramp && vertical_from_prev),
            "move {i}: a fed EntryPlunge leads a crease ramp; the air lead is EntryApproach"
        );
        if m.intent != MoveIntent::EntryApproach {
            continue;
        }
        approaches += 1;
        assert!(fed(m), "move {i}: an EntryApproach is fed");
        assert!(
            vertical_from_prev,
            "move {i}: the air lead is not a vertical descent"
        );
        assert!(leads_a_ramp, "move {i}: the air lead does not lead a ramp");
        let laps_top = moves[i + 1..]
            .iter()
            .take_while(|n| n.intent == MoveIntent::EntryRamp)
            .map(|n| n.target.z)
            .fold(f64::NEG_INFINITY, f64::max);
        assert!(
            m.target.z >= laps_top - 1e-9,
            "move {i}: the air lead ends at Z {} below a lap of its ramp ({laps_top})",
            m.target.z
        );
    }
    assert!(
        approaches > 0,
        "pencil/rest: no crease air lead, so the claim is vacuous"
    );
}
